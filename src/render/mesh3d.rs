// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Triangle meshes and a look-at/perspective camera for the `3d` feature.
//!
//! The module is compiled only with the `3d` feature. It owns the data a mesh draw needs —
//! positions, per-vertex colours, indices and a camera — and the one projection routine both the
//! software rasteriser and the SVG backend call, so the two cannot disagree about where a triangle
//! lands or which triangle is nearer.
//!
//! # Projection conventions
//!
//! * Model and eye space are right-handed, with the camera looking down its own negative `z`.
//! * [`Camera::view_projection`] produces clip coordinates; a point with `w <= 0` is on or behind
//!   the view plane and cannot be divided.
//! * [`viewport_matrix`] maps normalised device coordinates onto a rectangle of the render target
//!   and remaps depth from `-1..=1` to `0..=1`, the range WebGPU's depth attachment expects. The
//!   software z-buffer uses the same range, so "smaller depth is nearer" is one statement for every
//!   backend.
//! * Screen coordinates use the framework's top-left origin and y-down convention, so the y axis is
//!   flipped between NDC and pixels.

#![cfg(feature = "3d")]

use super::math3d::Mat4;
use crate::compat::Vec;
use crate::core::{Color, Rect, Size};

/// The default vertical field of view of [`Camera::new`], in radians (60 degrees).
pub const DEFAULT_FOV_Y: f32 = core::f32::consts::FRAC_PI_3;

/// The default near distance of [`Camera::new`].
pub const DEFAULT_NEAR: f32 = 0.1;

/// The default far distance of [`Camera::new`].
pub const DEFAULT_FAR: f32 = 1000.0;

/// Clip-space `w` at or below this value is treated as behind the view plane.
///
/// The tolerance is not zero because a point exactly on the plane has an infinite projection and
/// every arithmetic step after it is a NaN. A triangle with one such vertex is dropped whole — this
/// module does not clip geometry against the near plane.
const PROJECTION_W_EPSILON: f32 = 1.0e-6;

/// A position or direction in model, world or eye space, depending on the caller.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl core::ops::Sub for Vec3 {
    type Output = Self;

    /// Component-wise subtraction.
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Vec3 {
    /// The zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    /// Builds a vector from its three components.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The dot product.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// The right-handed cross product, `self × other`.
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// The Euclidean length.
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// The unit vector in this direction, or `None` when the vector has no usable direction.
    ///
    /// A zero-length vector, a non-finite component, or a length too small to divide by all answer
    /// `None`, so a degenerate camera basis becomes an explicit fallback rather than a NaN matrix.
    pub fn normalized(self) -> Option<Self> {
        let length = self.length();
        if !length.is_finite() || length <= f32::EPSILON {
            return None;
        }
        Some(Self::new(self.x / length, self.y / length, self.z / length))
    }
}

/// One mesh vertex: a position and the colour it contributes to its triangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex3D {
    /// Position in model space.
    pub position: Vec3,
    /// Colour at this vertex.
    pub color: Color,
}

impl Vertex3D {
    /// Builds a vertex from a position and a colour.
    pub const fn new(position: Vec3, color: Color) -> Self {
        Self { position, color }
    }
}

/// Why a [`Mesh3D`] could not be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mesh3DError {
    /// The index count is not a multiple of three, so the list cannot be read as triangles. The
    /// contained value is the offending count.
    IndexCountNotMultipleOfThree(usize),
    /// An index names a vertex outside the vertex list. `index` is the value and `vertex_count` is
    /// how many vertices the mesh has.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of vertices available.
        vertex_count: usize,
    },
}

/// An indexed triangle mesh.
///
/// Every three consecutive indices name one triangle. The constructor validates the index list
/// once, so the rasteriser can read a triangle without re-checking it. A mesh is plain owned data
/// and can therefore live inside a storable [`crate::render::RenderCommand`].
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh3D {
    vertices: Vec<Vertex3D>,
    indices: Vec<u32>,
}

impl Mesh3D {
    /// Builds an indexed triangle mesh after validating every index.
    ///
    /// The index list is interpreted as triangles in the order given. No winding is enforced and no
    /// triangle is culled: the depth buffer decides what is visible, so a mesh whose triangles face
    /// the camera in either direction still draws.
    pub fn new(vertices: Vec<Vertex3D>, indices: Vec<u32>) -> Result<Self, Mesh3DError> {
        if !indices.len().is_multiple_of(3) {
            return Err(Mesh3DError::IndexCountNotMultipleOfThree(indices.len()));
        }
        let vertex_count = vertices.len();
        for &index in &indices {
            if index as usize >= vertex_count {
                return Err(Mesh3DError::IndexOutOfRange { index, vertex_count });
            }
        }
        Ok(Self { vertices, indices })
    }

    /// The mesh vertices, in model space.
    pub fn vertices(&self) -> &[Vertex3D] {
        &self.vertices
    }

    /// The triangle index list, three entries per triangle.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// How many triangles the mesh has.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Builds an axis-aligned cube centred on `center`.
    ///
    /// `size` is the edge length, and `face_colors` gives the colour of the `+X`, `-X`, `+Y`, `-Y`,
    /// `+Z` and `-Z` faces in that order. Each face is four vertices with its own colour, so the
    /// triangles of one face shade uniformly and adjacent faces stay distinguishable — which is
    /// what makes a headless snapshot readable as a cube rather than as one silhouette.
    ///
    /// A non-finite or non-positive `size` produces a degenerate cube (zero-area triangles) rather
    /// than an error, so a widget that computes a zero extent still yields a valid mesh.
    pub fn cube(center: Vec3, size: f32, face_colors: [Color; 6]) -> Self {
        let half = if size.is_finite() && size > 0.0 { size * 0.5 } else { 0.0 };
        let (x, y, z) = (center.x, center.y, center.z);
        // Corners of each face, in perimeter order. The winding is not consulted by any backend.
        let faces = [
            [
                (x + half, y - half, z + half),
                (x + half, y - half, z - half),
                (x + half, y + half, z - half),
                (x + half, y + half, z + half),
            ],
            [
                (x - half, y - half, z - half),
                (x - half, y - half, z + half),
                (x - half, y + half, z + half),
                (x - half, y + half, z - half),
            ],
            [
                (x - half, y + half, z + half),
                (x + half, y + half, z + half),
                (x + half, y + half, z - half),
                (x - half, y + half, z - half),
            ],
            [
                (x - half, y - half, z - half),
                (x + half, y - half, z - half),
                (x + half, y - half, z + half),
                (x - half, y - half, z + half),
            ],
            [
                (x - half, y - half, z + half),
                (x + half, y - half, z + half),
                (x + half, y + half, z + half),
                (x - half, y + half, z + half),
            ],
            [
                (x + half, y - half, z - half),
                (x - half, y - half, z - half),
                (x - half, y + half, z - half),
                (x + half, y + half, z - half),
            ],
        ];

        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (face, corners) in faces.iter().enumerate() {
            let base = vertices.len() as u32;
            for &(px, py, pz) in corners {
                vertices.push(Vertex3D::new(Vec3::new(px, py, pz), face_colors[face]));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        // The index list was built from the vertex list above, so it is valid by construction.
        Self { vertices, indices }
    }
}

/// A look-at camera with a perspective projection.
///
/// The camera is plain data so it can be stored in a render command. [`Self::view`] and
/// [`Self::projection`] sanitise degenerate parameters instead of producing NaN matrices: an eye
/// that coincides with its target, an `up` parallel to the view direction, a zero field of view or a
/// non-positive clip range all fall back to a defined value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position in world space.
    pub eye: Vec3,
    /// Point the camera looks at.
    pub target: Vec3,
    /// Approximate up direction; the view basis uses its component perpendicular to the view.
    pub up: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Near clip distance, a positive distance in front of the eye.
    pub near: f32,
    /// Far clip distance, greater than [`Self::near`].
    pub far: f32,
}

impl Camera {
    /// Builds a camera with [`DEFAULT_FOV_Y`], [`DEFAULT_NEAR`] and [`DEFAULT_FAR`].
    pub fn new(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        Self { eye, target, up, fov_y: DEFAULT_FOV_Y, near: DEFAULT_NEAR, far: DEFAULT_FAR }
    }

    /// Replaces the vertical field of view, in radians.
    ///
    /// The value is sanitised by [`Self::projection`] rather than here, so a non-finite value
    /// answers the default field of view instead of being stored as a working angle.
    pub fn with_field_of_view(mut self, fov_y: f32) -> Self {
        self.fov_y = fov_y;
        self
    }

    /// Replaces the near and far clip distances.
    pub fn with_clip(mut self, near: f32, far: f32) -> Self {
        self.near = near;
        self.far = far;
        self
    }

    /// The world-to-eye view matrix.
    ///
    /// Returns [`Mat4::IDENTITY`] when the eye and target coincide or `up` is parallel to the view
    /// direction; both make a basis undefined, and an identity is a picture that can be diagnosed
    /// rather than a matrix full of NaNs that silently draws nothing.
    pub fn view(&self) -> Mat4 {
        let Some(forward) = (self.target - self.eye).normalized() else {
            return Mat4::IDENTITY;
        };
        let Some(right) = forward.cross(self.up).normalized() else {
            return Mat4::IDENTITY;
        };
        let up = right.cross(forward);
        Mat4 {
            m: [
                right.x,
                right.y,
                right.z,
                -right.dot(self.eye), //
                up.x,
                up.y,
                up.z,
                -up.dot(self.eye), //
                -forward.x,
                -forward.y,
                -forward.z,
                forward.dot(self.eye), //
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// The eye-to-clip perspective matrix for `aspect` (width divided by height).
    ///
    /// A non-finite or non-positive aspect is treated as `1.0`; a field of view outside
    /// `0 < fov < π` falls back to [`DEFAULT_FOV_Y`]; a non-positive `near` falls back to
    /// [`DEFAULT_NEAR`]; and a `far` that is not greater than `near` becomes `near + 1`.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let fov =
            if self.fov_y.is_finite() && self.fov_y > 0.0 && self.fov_y < core::f32::consts::PI {
                self.fov_y
            } else {
                DEFAULT_FOV_Y
            };
        let near = if self.near.is_finite() && self.near > 0.0 { self.near } else { DEFAULT_NEAR };
        let far = if self.far.is_finite() && self.far > near { self.far } else { near + 1.0 };
        let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
        Mat4::perspective(fov, aspect, near, far)
    }

    /// The composed world-to-clip matrix for `aspect`.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.view().then(&self.projection(aspect))
    }
}

/// The matrix that maps normalised device coordinates onto `viewport` inside `target`.
///
/// The result remaps `x`/`y` from `-1..=1` to the viewport rectangle and depth from `-1..=1` to
/// `0..=1`. The depth remap is part of the matrix for two reasons: WebGPU's depth attachment
/// compares in `0..=1`, and the software z-buffer uses the same range, so a triangle that is nearer
/// on one backend is nearer on the other. A zero-sized target is treated as `1x1`.
pub fn viewport_matrix(viewport: Rect, target: Size) -> Mat4 {
    let width = target.width.max(1) as f32;
    let height = target.height.max(1) as f32;
    let sx = viewport.width as f32 / width;
    let sy = viewport.height as f32 / height;
    let tx = (2.0 * viewport.x as f32 + viewport.width as f32) / width - 1.0;
    let ty = 1.0 - (2.0 * viewport.y as f32 + viewport.height as f32) / height;
    Mat4 {
        m: [
            sx, 0.0, 0.0, tx, //
            0.0, sy, 0.0, ty, //
            0.0, 0.0, 0.5, 0.5, //
            0.0, 0.0, 0.0, 1.0,
        ],
    }
}

/// A projected triangle vertex in surface pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedVertex {
    /// X position in surface pixels.
    pub x: f32,
    /// Y position in surface pixels, with the origin at the surface's top-left corner.
    pub y: f32,
    /// Normalised depth in `0..=1`; smaller is nearer.
    pub depth: f32,
}

/// A triangle projected to surface pixels, ready for a painter or a rasteriser.
///
/// Both the software rasteriser and the SVG backend consume this type, which is why the projection
/// lives in one place: a snapshot and a frame then place the same triangle at the same pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedTriangle {
    /// The three vertices, in the mesh's index order.
    pub vertices: [ProjectedVertex; 3],
    /// The flat colour the triangle is painted with: the mean of its three vertex colours.
    ///
    /// Flat shading is deliberate. The task of a mesh command is geometry and occlusion; sampling a
    /// per-vertex attribute would need perspective-correct interpolation in the rasteriser and a
    /// gradient in the SVG backend, and the two could then disagree about a pixel. Both backends
    /// use this one colour instead.
    pub color: Color,
}

impl ProjectedTriangle {
    /// The mean depth of the three vertices, used as the painter's-algorithm sort key.
    pub fn depth(&self) -> f32 {
        (self.vertices[0].depth + self.vertices[1].depth + self.vertices[2].depth) / 3.0
    }
}

/// One projection shared by [`project_point`] and [`project_mesh`].
struct Projector {
    mvp: Mat4,
    width: f32,
    height: f32,
}

impl Projector {
    fn new(model: &Mat4, camera: &Camera, viewport: Rect, target: Size) -> Self {
        let aspect =
            if viewport.height > 0 { viewport.width as f32 / viewport.height as f32 } else { 1.0 };
        let mvp =
            model.then(&camera.view_projection(aspect)).then(&viewport_matrix(viewport, target));
        Self { mvp, width: target.width.max(1) as f32, height: target.height.max(1) as f32 }
    }

    /// Projects one point, or answers `None` when it cannot be divided.
    fn project(&self, point: Vec3) -> Option<[f32; 3]> {
        let clip = self.mvp.transform_point(point.x, point.y, point.z);
        let w = clip[3];
        if !w.is_finite() || w <= PROJECTION_W_EPSILON {
            return None;
        }
        let inverse_w = 1.0 / w;
        let ndc_x = clip[0] * inverse_w;
        let ndc_y = clip[1] * inverse_w;
        let depth = clip[2] * inverse_w;
        if !ndc_x.is_finite() || !ndc_y.is_finite() || !depth.is_finite() {
            return None;
        }
        Some([(ndc_x * 0.5 + 0.5) * self.width, (0.5 - ndc_y * 0.5) * self.height, depth])
    }
}

/// Projects one model-space point into surface pixels and normalised depth.
///
/// Returns `[x, y, depth]`, or `None` when the point is on or behind the view plane. The viewport
/// and target follow [`viewport_matrix`]; `camera`'s field of view is measured vertically.
pub fn project_point(
    point: Vec3,
    model: &Mat4,
    camera: &Camera,
    viewport: Rect,
    target: Size,
) -> Option<[f32; 3]> {
    Projector::new(model, camera, viewport, target).project(point)
}

/// Projects every triangle of `mesh` into surface pixels.
///
/// A triangle is skipped whole when any of its vertices is on or behind the view plane; this
/// module does not clip geometry against the near plane, so a mesh that pokes through the camera
/// loses the offending triangle rather than drawing a wrong one. The returned triangles keep the
/// mesh's order and carry [`ProjectedTriangle::color`] as their flat colour.
pub fn project_mesh(
    mesh: &Mesh3D,
    model: &Mat4,
    camera: &Camera,
    viewport: Rect,
    target: Size,
) -> Vec<ProjectedTriangle> {
    let mut triangles = Vec::new();
    project_mesh_into(mesh, model, camera, viewport, target, &mut triangles);
    triangles
}

/// [`project_mesh`] into a caller-owned vector, reusing its allocation.
///
/// The vector is cleared first and then reserved for the mesh's triangle count, so a renderer that
/// keeps one vector on its surface — the way it keeps its text coverage scratch — allocates only
/// on the first draw or when a larger mesh appears. The projection and the triangles produced are
/// identical to [`project_mesh`], which is now this function's thin wrapper.
pub fn project_mesh_into(
    mesh: &Mesh3D,
    model: &Mat4,
    camera: &Camera,
    viewport: Rect,
    target: Size,
    triangles: &mut Vec<ProjectedTriangle>,
) {
    triangles.clear();
    triangles.reserve(mesh.triangle_count());
    let projector = Projector::new(model, camera, viewport, target);
    for indices in mesh.indices.as_chunks::<3>().0 {
        // `Mesh3D::new` validated every index against the vertex list, so the lookups cannot fail.
        let a = mesh.vertices[indices[0] as usize];
        let b = mesh.vertices[indices[1] as usize];
        let c = mesh.vertices[indices[2] as usize];
        let (Some(pa), Some(pb), Some(pc)) = (
            projector.project(a.position),
            projector.project(b.position),
            projector.project(c.position),
        ) else {
            continue;
        };
        triangles.push(ProjectedTriangle {
            vertices: [projected_vertex(pa), projected_vertex(pb), projected_vertex(pc)],
            color: mean_color([a.color, b.color, c.color]),
        });
    }
}

/// Narrows a `[x, y, depth]` triple into a [`ProjectedVertex`].
fn projected_vertex(value: [f32; 3]) -> ProjectedVertex {
    ProjectedVertex { x: value[0], y: value[1], depth: value[2] }
}

/// The per-channel mean of three colours, truncated to whole bytes.
fn mean_color(colors: [Color; 3]) -> Color {
    let mean = |a: u8, b: u8, c: u8| ((a as u32 + b as u32 + c as u32) / 3) as u8;
    Color::rgba(
        mean(colors[0].r, colors[1].r, colors[2].r),
        mean(colors[0].g, colors[1].g, colors[2].g),
        mean(colors[0].b, colors[1].b, colors[2].b),
        mean(colors[0].a, colors[1].a, colors[2].a),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!((actual - expected).abs() < 0.01, "{what}: expected {expected}, got {actual}");
    }

    fn triangle_mesh(z: f32, color: Color) -> Mesh3D {
        Mesh3D::new(
            vec![
                Vertex3D::new(Vec3::new(-1.0, -1.0, z), color),
                Vertex3D::new(Vec3::new(1.0, -1.0, z), color),
                Vertex3D::new(Vec3::new(0.0, 1.0, z), color),
            ],
            vec![0, 1, 2],
        )
        .expect("a three-vertex triangle is valid")
    }

    #[test]
    fn cube_has_twelve_triangles_and_valid_indices() {
        let cube = Mesh3D::cube(Vec3::ZERO, 2.0, [Color::RED; 6]);
        assert_eq!(cube.vertices().len(), 24, "six faces of four vertices each");
        assert_eq!(cube.triangle_count(), 12);
        assert_eq!(cube.indices().len(), 36);
        for &index in cube.indices() {
            assert!((index as usize) < cube.vertices().len());
        }
    }

    #[test]
    fn mesh_rejects_malformed_index_lists() {
        let vertices = vec![Vertex3D::new(Vec3::ZERO, Color::WHITE)];
        assert_eq!(
            Mesh3D::new(vertices.clone(), vec![0, 0]).unwrap_err(),
            Mesh3DError::IndexCountNotMultipleOfThree(2)
        );
        assert_eq!(
            Mesh3D::new(vertices, vec![0, 0, 1]).unwrap_err(),
            Mesh3DError::IndexOutOfRange { index: 1, vertex_count: 1 }
        );
    }

    #[test]
    fn a_degenerate_cube_is_still_a_valid_mesh() {
        let cube = Mesh3D::cube(Vec3::ZERO, f32::NAN, [Color::RED; 6]);
        assert_eq!(cube.triangle_count(), 12);
        assert!(cube.vertices().iter().all(|vertex| vertex.position.x == 0.0));
    }

    #[test]
    fn camera_view_falls_back_to_identity_for_a_degenerate_basis() {
        let eye = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(Camera::new(eye, eye, Vec3::new(0.0, 1.0, 0.0)).view(), Mat4::IDENTITY);
        let parallel = Camera::new(eye, Vec3::new(1.0, 2.0, 4.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(parallel.view(), Mat4::IDENTITY);
        assert_eq!(parallel.projection(f32::NAN), parallel.projection(1.0));
    }

    /// A point one unit to the right of a 90-degree camera three units away lands at the expected
    /// pixel, and the same point above the axis lands above the centre.
    #[test]
    fn perspective_projection_puts_a_known_point_at_the_expected_pixel() {
        let camera = Camera::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0))
            .with_field_of_view(core::f32::consts::FRAC_PI_2)
            .with_clip(0.1, 100.0);
        let viewport = Rect::new(0, 0, 100, 100);
        let target = Size::new(100, 100);

        let right =
            project_point(Vec3::new(1.0, 0.0, -3.0), &Mat4::IDENTITY, &camera, viewport, target)
                .expect("the point is in front of the eye");
        assert_close(right[0], 100.0 * (0.5 + 0.5 / 3.0), "x after the perspective divide");
        assert_close(right[1], 50.0, "a point on the horizontal axis stays at the centre row");

        let up =
            project_point(Vec3::new(0.0, 1.0, -3.0), &Mat4::IDENTITY, &camera, viewport, target)
                .expect("the point is in front of the eye");
        assert_close(up[1], 100.0 * (0.5 - 0.5 / 3.0), "screen y grows downward");

        assert!(
            project_point(Vec3::new(0.0, 0.0, 1.0), &Mat4::IDENTITY, &camera, viewport, target)
                .is_none(),
            "a point behind the eye has no perspective divide"
        );
    }

    #[test]
    fn projection_lands_inside_the_requested_viewport() {
        let cube = Mesh3D::cube(Vec3::ZERO, 2.0, [Color::RED; 6]);
        let camera = Camera::new(Vec3::new(4.0, 3.0, 6.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let viewport = Rect::new(10, 20, 80, 60);
        let triangles =
            project_mesh(&cube, &Mat4::IDENTITY, &camera, viewport, Size::new(100, 100));

        assert!(!triangles.is_empty(), "a cube in front of the camera projects to triangles");
        for triangle in &triangles {
            for vertex in triangle.vertices {
                assert!(vertex.depth >= 0.0 && vertex.depth <= 1.0, "depth stays in range");
                assert!(
                    vertex.x >= viewport.x as f32 - 0.01
                        && vertex.x <= (viewport.x + viewport.width as i32) as f32 + 0.01,
                    "x stays inside the viewport, got {}",
                    vertex.x
                );
                assert!(
                    vertex.y >= viewport.y as f32 - 0.01
                        && vertex.y <= (viewport.y + viewport.height as i32) as f32 + 0.01,
                    "y stays inside the viewport, got {}",
                    vertex.y
                );
            }
        }
    }

    #[test]
    fn a_triangle_behind_the_eye_is_dropped_whole() {
        let mesh = triangle_mesh(1.0, Color::RED);
        let camera = Camera::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        assert!(project_mesh(
            &mesh,
            &Mat4::IDENTITY,
            &camera,
            Rect::new(0, 0, 10, 10),
            Size::new(10, 10)
        )
        .is_empty());
    }

    #[test]
    fn a_triangles_flat_colour_is_the_mean_of_its_vertices() {
        let mesh = Mesh3D::new(
            vec![
                Vertex3D::new(Vec3::new(-1.0, -1.0, -3.0), Color::rgb(0, 0, 0)),
                Vertex3D::new(Vec3::new(1.0, -1.0, -3.0), Color::rgb(30, 60, 90)),
                Vertex3D::new(Vec3::new(0.0, 1.0, -3.0), Color::rgb(60, 120, 180)),
            ],
            vec![0, 1, 2],
        )
        .expect("valid triangle");
        let camera = Camera::new(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0));
        let triangles = project_mesh(
            &mesh,
            &Mat4::IDENTITY,
            &camera,
            Rect::new(0, 0, 10, 10),
            Size::new(10, 10),
        );
        assert_eq!(triangles.len(), 1);
        assert_eq!(triangles[0].color, Color::rgb(30, 60, 90));
    }
}
