// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Depth-buffered software rasterisation of [`crate::render::mesh3d::Mesh3D`].
//!
//! The projection itself lives in `mesh3d`, so this module and the SVG backend consume the same
//! screen-space triangles. What is added here is the part a painter cannot express: a per-pixel
//! depth test against a buffer that lives for the whole frame.
//!
//! # What is rasterised
//!
//! Each triangle is filled at pixel centres with a hard edge (no antialiasing). Colours are flat:
//! every pixel of a triangle receives [`ProjectedTriangle::color`], the mean of its vertex colours.
//! This is the same value the SVG backend paints, so a snapshot and a software frame agree on
//! every pixel the depth test leaves visible.
//!
//! # Depth
//!
//! Depth is the projected normalised value in `0..=1`, where `0` is the near plane and `1` the far
//! plane. For a standard perspective matrix this value is affine in screen space, so the
//! barycentric weights used for coverage also interpolate depth exactly — no separate
//! perspective-correct attribute pass is needed. A fragment outside `0..=1` is rejected, which is
//! how geometry that projects past a clip plane is dropped; triangles are not clipped
//! geometrically.
//!
//! # Back faces
//!
//! No triangle is culled, deliberately. The depth test alone decides visibility, so a mesh whose
//! winding is inconsistent, or one that is meant to be visible from both sides, still renders. A
//! closed cube therefore shows its three near faces and hides the three far ones without the
//! renderer having to agree with the mesh author about winding order.

use crate::core::{Rect, Size};
use crate::render::math3d::Mat4;
use crate::render::mesh3d::{project_mesh, Camera, Mesh3D, ProjectedTriangle, ProjectedVertex};
use crate::render::pipeline::pixel_ops::write_pixel_with_mode;
use crate::render::pipeline::u32_to_i32_saturating;
use crate::render::{BlendMode, SoftwareSurface};

impl SoftwareSurface {
    /// Draws `mesh` through `camera`, testing and writing a per-frame depth buffer.
    ///
    /// `model` places the mesh in world space and `viewport` is the rectangle of this surface the
    /// camera projects into. The depth buffer is one `f32` per surface pixel; it is refilled with
    /// the far value on the first mesh draw after [`SoftwareSurface::begin_frame`], so two mesh
    /// commands in one frame are ordered by depth rather than by recording order. A two-dimensional
    /// command is unaffected by the buffer and keeps its painter order.
    ///
    /// A mesh with no triangles, an empty surface, or an empty viewport draws nothing.
    pub fn draw_mesh_3d(&mut self, mesh: &Mesh3D, model: &Mat4, camera: &Camera, viewport: Rect) {
        let size = self.buffer.size();
        if size.width == 0
            || size.height == 0
            || viewport.width == 0
            || viewport.height == 0
            || mesh.triangle_count() == 0
        {
            return;
        }
        let triangles = project_mesh(mesh, model, camera, viewport, size);
        if triangles.is_empty() {
            return;
        }
        self.ensure_depth_buffer(size);
        let clip = self.current_clip();
        let blend = self.blend_mode;
        let width = size.width;
        let frame = self.buffer.back_mut();
        let depth = &mut self.depth_buffer;
        for triangle in &triangles {
            rasterize_triangle(triangle, frame, depth, width, size.height, viewport, clip, blend);
        }
    }

    /// Resizes the depth buffer and fills it with the far value when it is stale.
    fn ensure_depth_buffer(&mut self, size: Size) {
        let needed = (size.width as usize).saturating_mul(size.height as usize);
        if self.depth_dirty || self.depth_buffer.len() != needed {
            self.depth_buffer.clear();
            self.depth_buffer.resize(needed, 1.0);
            self.depth_dirty = false;
        }
    }
}

/// Fills one projected triangle, depth-testing every pixel it covers.
///
/// The edge functions are evaluated in screen space. Dividing each by the signed area makes the
/// three values barycentric weights; because the area's sign is carried into all three, the test is
/// `>= 0` for either winding, which is why no triangle has to be reordered.
#[allow(clippy::too_many_arguments)]
fn rasterize_triangle(
    triangle: &ProjectedTriangle,
    frame: &mut [u8],
    depth_buffer: &mut [f32],
    width: u32,
    height: u32,
    viewport: Rect,
    clip: Option<(i32, i32, u32, u32)>,
    blend: BlendMode,
) {
    let [a, b, c] = triangle.vertices;
    let area = edge(a, b, c.x, c.y);
    if !area.is_finite() || area == 0.0 {
        return;
    }

    // The viewport is part of the clip: a camera looking through a small rectangle must not paint
    // outside it, even though the projection matrix already maps NDC into that rectangle.
    let view_left = viewport.x;
    let view_top = viewport.y;
    let view_right = viewport.x.saturating_add(u32_to_i32_saturating(viewport.width));
    let view_bottom = viewport.y.saturating_add(u32_to_i32_saturating(viewport.height));

    let min_x = (a.x.min(b.x).min(c.x).floor() as i32)
        .max(0)
        .max(view_left)
        .max(clip.map(|(x, ..)| x).unwrap_or(i32::MIN));
    let min_y = (a.y.min(b.y).min(c.y).floor() as i32)
        .max(0)
        .max(view_top)
        .max(clip.map(|(_, y, ..)| y).unwrap_or(i32::MIN));
    let max_x = (a.x.max(b.x).max(c.x).ceil() as i32)
        .min(width as i32 - 1)
        .min(view_right.saturating_sub(1))
        .min(
            clip.map(|(x, _, w, _)| x.saturating_add(u32_to_i32_saturating(w)))
                .unwrap_or(i32::MAX)
                .saturating_sub(1),
        )
        // `pixel_visible` compares against `clip_x + clip_width as i32` — a *wrapping* cast the
        // saturating conversion above does not reproduce. Folding that exact bound in keeps the
        // span below identical to the per-pixel check this file no longer pays for (G-010); for
        // every clip width that fits `i32` the two bounds agree and this is a no-op.
        .min(
            clip.map(|(x, _, w, _)| x.saturating_add(w as i32).saturating_sub(1))
                .unwrap_or(i32::MAX),
        );
    let max_y = (a.y.max(b.y).max(c.y).ceil() as i32)
        .min(height as i32 - 1)
        .min(view_bottom.saturating_sub(1))
        .min(
            clip.map(|(_, y, _, h)| y.saturating_add(u32_to_i32_saturating(h)))
                .unwrap_or(i32::MAX)
                .saturating_sub(1),
        )
        // The wrapping-cast bound, as for `max_x` above.
        .min(
            clip.map(|(_, y, _, h)| y.saturating_add(h as i32).saturating_sub(1))
                .unwrap_or(i32::MAX),
        );
    if max_x < min_x || max_y < min_y {
        return;
    }

    // The bounds above close the triangle's bbox under the viewport and the clip — the clip∩bbox
    // span G-010 asks the inner loop to scan — so every pixel inside them passes `pixel_visible`
    // and the loop no longer pays that test per pixel.
    for y in min_y..=max_y {
        let sample_y = y as f32 + 0.5;
        for x in min_x..=max_x {
            let sample_x = x as f32 + 0.5;
            let weight_a = edge(b, c, sample_x, sample_y) / area;
            let weight_b = edge(c, a, sample_x, sample_y) / area;
            let weight_c = edge(a, b, sample_x, sample_y) / area;
            if weight_a < 0.0 || weight_b < 0.0 || weight_c < 0.0 {
                continue;
            }
            let depth = weight_a * a.depth + weight_b * b.depth + weight_c * c.depth;
            // `!(0.0..=1.0).contains(..)` also rejects NaN, which a degenerate divide could
            // produce before the finite-area guard above.
            if !(0.0..=1.0).contains(&depth) {
                continue;
            }
            let index = y as usize * width as usize + x as usize;
            // `Less` on 0..=1 depth: a fragment is drawn only when it is strictly nearer than what
            // is already there. Equal depths keep the earlier fragment (and its colour), which is
            // what makes a shared triangle edge write one colour rather than two.
            if depth < depth_buffer[index] {
                depth_buffer[index] = depth;
                write_pixel_with_mode(blend, frame, width, x as u32, y as u32, triangle.color, 1.0);
            }
        }
    }
}

/// The signed area of the parallelogram `(a, b, p)`, used both as the triangle area and as a
/// barycentric numerator.
fn edge(a: ProjectedVertex, b: ProjectedVertex, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Size};
    use crate::render::mesh3d::{Vec3, Vertex3D};
    use crate::render::{PaintBackend, RenderCommand, SoftwarePaintBackend};

    const SIZE: u32 = 100;

    /// A triangle in the plane `z`, large enough to cover the centre of a 100x100 target.
    fn triangle(z: f32, color: Color) -> [Vertex3D; 3] {
        [
            Vertex3D::new(Vec3::new(-1.0, -1.0, z), color),
            Vertex3D::new(Vec3::new(1.0, -1.0, z), color),
            Vertex3D::new(Vec3::new(0.0, 1.0, z), color),
        ]
    }

    fn two_triangle_mesh(near: &[Vertex3D; 3], far: &[Vertex3D; 3], near_first: bool) -> Mesh3D {
        let mut vertices = crate::compat::Vec::with_capacity(6);
        let mut indices = crate::compat::Vec::with_capacity(6);
        if near_first {
            vertices.extend_from_slice(near);
            vertices.extend_from_slice(far);
            indices.extend_from_slice(&[0, 1, 2, 3, 4, 5]);
        } else {
            vertices.extend_from_slice(far);
            vertices.extend_from_slice(near);
            indices.extend_from_slice(&[3, 4, 5, 0, 1, 2]);
        }
        Mesh3D::new(vertices, indices).expect("the fixture indices are valid")
    }

    fn camera() -> Camera {
        Camera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0))
            .with_field_of_view(core::f32::consts::FRAC_PI_2)
            .with_clip(0.1, 100.0)
    }

    fn viewport() -> Rect {
        Rect::new(0, 0, SIZE, SIZE)
    }

    /// The RGBA triple at one pixel of a presented frame.
    fn pixel(frame: &[u8], x: u32, y: u32) -> (u8, u8, u8) {
        let index = ((y * SIZE + x) * 4) as usize;
        (frame[index], frame[index + 1], frame[index + 2])
    }

    #[test]
    fn a_nearer_triangle_occludes_a_farther_one_in_either_triangle_order() {
        for near_first in [true, false] {
            let mesh = two_triangle_mesh(
                &triangle(1.0, Color::RED),
                &triangle(-1.0, Color::BLUE),
                near_first,
            );
            let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
            surface.begin_frame(Color::WHITE);
            surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), viewport());
            surface.end_frame();

            assert_eq!(
                pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2),
                (255, 0, 0),
                "the near triangle must win with near_first = {near_first}"
            );
        }
    }

    #[test]
    fn a_nearer_mesh_occludes_a_farther_one_in_either_command_order() {
        let near =
            Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid triangle");
        let far = Mesh3D::new(triangle(-1.0, Color::BLUE).to_vec(), vec![0, 1, 2])
            .expect("valid triangle");

        for near_first in [true, false] {
            let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
            surface.begin_frame(Color::WHITE);
            let (first, second) = if near_first { (&near, &far) } else { (&far, &near) };
            surface.draw_mesh_3d(first, &Mat4::IDENTITY, &camera(), viewport());
            surface.draw_mesh_3d(second, &Mat4::IDENTITY, &camera(), viewport());
            surface.end_frame();

            assert_eq!(
                pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2),
                (255, 0, 0),
                "depth survives between mesh commands with near_first = {near_first}"
            );
        }
    }

    #[test]
    fn a_far_triangle_alone_is_visible() {
        let mesh = Mesh3D::new(triangle(-1.0, Color::BLUE).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), viewport());
        surface.end_frame();
        assert_eq!(pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2), (0, 0, 255));
    }

    #[test]
    fn a_cube_draws_a_non_empty_picture_through_the_command_path() {
        let mesh = Mesh3D::cube(
            Vec3::ZERO,
            2.0,
            [
                Color::RED,
                Color::GREEN,
                Color::BLUE,
                Color::rgb(200, 200, 0),
                Color::rgb(0, 200, 200),
                Color::rgb(200, 0, 200),
            ],
        );
        let camera = Camera::new(Vec3::new(3.0, 2.0, 6.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::DrawMesh3D {
            mesh,
            model: Mat4::IDENTITY,
            camera,
            viewport: Rect::new(0, 0, 200, 200),
        });
        backend.end_frame();

        let frame = backend.frame_rgba();
        let painted = frame
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] != 255 || pixel[1] != 255 || pixel[2] != 255)
            .count();
        assert!(painted > 1000, "a cube at this camera must cover a real area, got {painted}");

        // Three distinct face colours must be present: a silhouette alone would prove only that
        // *something* was drawn, not that the projection kept the faces apart. This camera sees the
        // `+X` (red), `+Y` (blue) and `+Z` (cyan) faces.
        let mut red = 0;
        let mut blue = 0;
        let mut cyan = 0;
        for pixel in frame.as_chunks::<4>().0 {
            red += usize::from(pixel[0] > 128 && pixel[1] < 64 && pixel[2] < 64);
            blue += usize::from(pixel[2] > 128 && pixel[0] < 64 && pixel[1] < 64);
            cyan += usize::from(pixel[1] > 128 && pixel[2] > 128 && pixel[0] < 64);
        }
        assert!(red > 100 && blue > 100 && cyan > 100, "{red} {blue} {cyan}");
    }

    #[test]
    fn a_mesh_outside_its_viewport_paints_nothing_inside_the_viewport_elsewhere() {
        // The camera projects into a 20x20 rectangle at the surface's top-left; the centre of the
        // 100x100 surface must stay background.
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), Rect::new(0, 0, 20, 20));
        surface.end_frame();
        assert_eq!(pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2), (255, 255, 255));
    }

    #[test]
    fn a_two_dimensional_command_after_a_mesh_paints_over_it() {
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), viewport());
        surface.fill_rect(Rect::new(0, 0, SIZE, SIZE), Color::GREEN);
        surface.end_frame();
        assert_eq!(
            pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2),
            (0, 255, 0),
            "flat drawing has no depth and keeps painter order"
        );
    }

    /// The half-open clip test must still bound the mesh. A clip that covers the left half leaves
    /// the right half at the background colour.
    #[test]
    fn a_mesh_is_clipped_by_the_surface_clip_stack() {
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.push_clip(0, 0, SIZE / 2, SIZE);
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), viewport());
        surface.pop_clip();
        surface.end_frame();
        assert_eq!(pixel(surface.frame_rgba(), 48, SIZE / 2), (255, 0, 0));
        assert_eq!(pixel(surface.frame_rgba(), 52, SIZE / 2), (255, 255, 255));
    }

    #[test]
    fn an_empty_mesh_or_viewport_draws_nothing() {
        let empty = Mesh3D::new(crate::compat::Vec::new(), crate::compat::Vec::new())
            .expect("an empty mesh is valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(&empty, &Mat4::IDENTITY, &camera(), viewport());
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), Rect::new(0, 0, 0, 0));
        surface.end_frame();
        assert_eq!(pixel(surface.frame_rgba(), SIZE / 2, SIZE / 2), (255, 255, 255));
    }

    #[test]
    fn a_point_outside_the_triangle_is_not_painted() {
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(&mesh, &Mat4::IDENTITY, &camera(), viewport());
        surface.end_frame();
        // The triangle's apex is above the centre, so the top-left corner stays clear.
        assert_eq!(pixel(surface.frame_rgba(), 2, 2), (255, 255, 255));
    }

    /// The command carries a viewport in surface pixels; the projection must place geometry there.
    #[test]
    fn the_viewport_origin_moves_the_picture() {
        let mesh = Mesh3D::new(triangle(1.0, Color::RED).to_vec(), vec![0, 1, 2]).expect("valid");
        let mut surface = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_mesh_3d(
            &mesh,
            &Mat4::IDENTITY,
            &camera(),
            Rect::new(SIZE as i32 / 2, 0, SIZE / 2, SIZE),
        );
        surface.end_frame();
        let frame = surface.frame_rgba();
        assert_eq!(pixel(frame, 75, 50), (255, 0, 0), "the picture follows the viewport origin");
        assert_eq!(pixel(frame, 25, 50), (255, 255, 255), "the old centre stays clear");
    }
}
