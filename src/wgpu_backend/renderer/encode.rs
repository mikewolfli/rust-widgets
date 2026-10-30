// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The 2D and mesh encoders: the packing helpers plus every entry point that records GPU draws
//! through the renderer's cached resources.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{
    bbox_for_circle, blend_mode_from_id, combine, effective_clip, gradient_ramp, path_bounds,
    path_fan_vertices, rasterize_draw_commands_rgba8, GradientAxis,
};
use super::super::types::{PixelRect, Rgba8};
use super::dispatch::{
    blend_bbox, commands_gpu_dispatchable_with, draws_under_blend, invalid_image_payload,
    pack_rect_command, SHAPE_ARC_STROKE, SHAPE_ARC_WEDGE, SHAPE_CIRCLE_STROKE,
    SHAPE_ROUNDED_OUTLINE,
};
use super::frame::WgpuRenderer;
use super::pass::StepRecorder;
use super::resources::{
    padded_row_bytes, readback_frame_bytes, rgba_buffer_len, BlendKind, BlendRun, DrawStep,
    RenderCache, ShadowDraw,
};
use super::shadow::pack_blurred_shadow;
use super::uploads::{ImageDraw, PathDraw, RadialDraw, ShapeDraw, TextDraw};
use std::cell::Ref;

/// Packs a row-major [`crate::render::math3d::Mat4`] as the column-major bytes WGSL expects.
///
/// WGSL's `mat4x4f` uniform layout is column-major, while the maths module stores rows to
/// match the affine matrix's natural reading order. Transposing here is what keeps the two
/// conventions from being silently swapped — the failure mode would be a transposed transform
/// that still compiles and still draws *something*.
#[cfg(feature = "3d")]
#[allow(clippy::needless_range_loop)] // row and column index the source row-major matrix.
fn mvp_column_major_bytes(m: &crate::render::math3d::Mat4) -> [u8; 64] {
    let mut bytes = [0u8; 64];
    for column in 0..4 {
        for row in 0..4 {
            let value = m.m[row * 4 + column];
            let start = (column * 4 + row) * 4;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

/// Bytes of one instanced quad record: two `vec4f` corner pairs and one `vec4f` colour.
///
/// Kept as a named constant because the vertex-buffer stride, the writer and the chunk offsets
/// must agree exactly; a mismatch is a pipeline validation error rather than a silent misread.
pub(crate) const INSTANCE_BYTES: usize = 48;

/// Upper bound on the instances submitted by one instanced draw.
///
/// A longer run is streamed in cap-sized chunks from the same grow-only buffer. The chunks are
/// drawn in order, so splitting never reorders the run and never allocates per quad.
pub(crate) const MAX_INSTANCES_PER_DRAW: usize = 4096;

/// Column-major bytes of the pixel-to-clip matrix for a `width × height` target.
///
/// `x` maps `0..width` to `-1..1` and `y` maps `0..height` to `1..-1`, matching the renderer's
/// top-left origin. These are the bytes `Mat4::from_affine(ortho)` produces, written directly so
/// the 2D rectangle path can use the instanced pipeline without the `3d` feature's maths module.
pub(super) fn ortho_mvp_bytes(width: u32, height: u32) -> [u8; 64] {
    let w = width as f32;
    let h = height as f32;
    // Column-major: scale x, flip and scale y, keep z, then translate to the clip-space corner.
    let values =
        [2.0 / w, 0.0, 0.0, 0.0, 0.0, -2.0 / h, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0];
    let mut bytes = [0u8; 64];
    for (index, value) in values.iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Bytes of one SDF circle draw's vertex data: six `vec2f` vertices bounding the disc (G-219).
pub(super) const CIRCLE_QUAD_BYTES: usize = 48;

/// Clamped bounding quad of a disc, in the pixel-centre frame the SDF expects (G-219).
///
/// Returns `(x0, y0, x1, y1)` for a disc whose centre sits at `(cx + 0.5, cy + 0.5)`: a pixel
/// centre at `(k + 0.5, j + 0.5)` lies within `radius` of that centre exactly when the software
/// rasteriser paints pixel `(k, j)`, so this quad covers precisely the pixels the disc can
/// touch. The quad is clamped to the viewport — fragments outside it are clipped anyway, and
/// clamping keeps visible-pixel positions exact because interpolation is affine in screen
/// space — and a disc that covers no pixel returns `None`, matching the software path's
/// off-screen skip. The arithmetic runs in `i64` so extreme centres and radii cannot wrap.
pub(crate) fn circle_quad(
    center: (i32, i32),
    radius: u32,
    width: u32,
    height: u32,
) -> Option<(i64, i64, i64, i64)> {
    let (cx, cy) = (center.0 as i64, center.1 as i64);
    let r = radius as i64;
    let x0 = (cx - r).max(0);
    let y0 = (cy - r).max(0);
    let x1 = (cx + r + 1).min(width as i64);
    let y1 = (cy + r + 1).min(height as i64);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    Some((x0, y0, x1, y1))
}

/// Writes one circle's bounding quad (six vertices) and its two uniform slots — the fill
/// colour, then the radius/centre parameters [`super::shaders::CIRCLE_FRAG`] reads — into the
/// reused packing buffers (G-219).
///
/// `uniforms` is indexed at the aligned slot stride: slot `2 * index` is the colour, slot
/// `2 * index + 1` the `(radius, centre.x, centre.y)` parameter vector. The centre carries the
/// half-pixel offset so the SDF's pixel-centre coordinates line up with the vertex-frame
/// positions the vertex stage interpolates.
fn pack_circle(
    pack_vertices: &mut Vec<u8>,
    pack_uniforms: &mut [u8],
    stride: usize,
    index: usize,
    center: (i32, i32),
    radius: u32,
    colour: &[f64; 4],
    quad: (i64, i64, i64, i64),
) {
    let (x0, y0, x1, y1) = quad;
    let mut vertices = [0u8; CIRCLE_QUAD_BYTES];
    // Two triangles from the quad: (0, 1, 2) and (0, 2, 3) in corner order
    // (x0,y0), (x1,y0), (x1,y1), (x0,y1).
    for (slot, (x, y)) in
        [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)].iter().enumerate()
    {
        vertices[slot * 8..slot * 8 + 4].copy_from_slice(&(*x as f32).to_le_bytes());
        vertices[slot * 8 + 4..slot * 8 + 8].copy_from_slice(&(*y as f32).to_le_bytes());
    }
    pack_vertices.extend_from_slice(&vertices);

    let mut colour_slot = [0u8; 16];
    for (channel, value) in colour.iter().enumerate() {
        colour_slot[channel * 4..channel * 4 + 4]
            .copy_from_slice(&((*value).clamp(0.0, 1.0) as f32).to_le_bytes());
    }
    let mut params_slot = [0u8; 16];
    params_slot[0..4].copy_from_slice(&(radius as f32).to_le_bytes());
    params_slot[4..8].copy_from_slice(&((center.0 as f32) + 0.5).to_le_bytes());
    params_slot[8..12].copy_from_slice(&((center.1 as f32) + 0.5).to_le_bytes());
    let base = index * 2 * stride;
    pack_uniforms[base..base + 16].copy_from_slice(&colour_slot);
    pack_uniforms[base + stride..base + stride + 16].copy_from_slice(&params_slot);
}

/// Packs a run of quads into the 48-byte-per-instance layout described on
/// [`super::shaders::FILL_RECT_VS_TRANSFORMED`], appending to the reused scratch buffer
/// `out` (G-216).
///
/// Each record is the four corners in perimeter order, then straight RGBA with every channel
/// clamped to `0.0..=1.0`. Returns an explicit error rather than wrapping if the run is too
/// large to address, so a hostile count cannot produce a short buffer. `out` is cleared first;
/// the capacity a peak frame built up is retained across calls.
pub(crate) fn pack_instanced_quads(
    quads: &[([crate::core::Point; 4], [f64; 4])],
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let capacity = quads
        .len()
        .checked_mul(INSTANCE_BYTES)
        .ok_or_else(|| format!("{} quads overflow the instance buffer size", quads.len()))?;
    out.clear();
    out.reserve(capacity);
    for (corners, colour) in quads {
        for point in corners {
            out.extend_from_slice(&(point.x as f32).to_le_bytes());
            out.extend_from_slice(&(point.y as f32).to_le_bytes());
        }
        for channel in colour {
            out.extend_from_slice(&((*channel).clamp(0.0, 1.0) as f32).to_le_bytes());
        }
    }
    Ok(())
}

impl WgpuRenderer {
    /// Render filled rectangles using the GPU pipeline with vertex buffer shaders.
    ///
    /// Each rect is defined as `(x, y, width, height, [r, g, b, a])` where:
    /// - `(x, y)` is the top-left corner in pixel coordinates
    /// - `(width, height)` is the rectangle size in pixels
    /// - `color` is an RGBA float array with values in `[0.0, 1.0]`
    ///
    /// A single rectangle goes through the `FILL_RECT_VS`/`FILL_RECT_FS` pipeline, which selects
    /// its colour from a per-rect uniform. A longer run is expanded to quads and drawn in
    /// `MAX_INSTANCES_PER_DRAW`-sized instanced draws, which produces the same picture with a
    /// draw count that no longer grows with the run (G-64).
    /// Encodes the fill-rect pass into the cached target and submits it.
    ///
    /// Split out from [`Self::render_fill_rect_gpu`] so a compositor can use the rendered
    /// texture directly (REND-02) instead of paying for a full `copy_texture_to_buffer` and a
    /// blocking map. The readback buffer is deliberately **not** ensured here.
    fn encode_fill_rects(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        // A single rectangle is already one draw, and the per-rect pipeline avoids the matrix
        // uniform upload for that common case. Two or more are packed into instance records and
        // drawn in `MAX_INSTANCES_PER_DRAW`-sized chunks, so the draw count no longer grows with
        // the run (G-64). Chunk order preserves painter order.
        if rects.len() <= 1 {
            self.encode_fill_rects_per_rect(width, height, rects)
        } else {
            self.encode_fill_rects_instanced(width, height, rects)
        }
    }

    /// The single-rectangle path: two triangles and one draw, with the colour selected by a
    /// dynamic uniform offset.
    ///
    /// A run of more than one rectangle takes the instanced path below instead, which is what
    /// keeps the draw count bounded (G-64).
    fn encode_fill_rects_per_rect(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        let w = width as f32;
        let h = height as f32;

        // Cached resources for the whole call: one colour arena, one bind group, one vertex
        // buffer and one target (REND-02).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, rects.len())?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_stencil(&self.device, width, height);
        let stride = cache.uniform.as_ref().expect("the arena was just ensured").stride as usize;

        // Generate transformed vertex positions in NDC and one clamped colour slot per rect,
        // directly into the reused packing buffers: the previous locals allocated the vertex
        // bytes, the colour list and the uniform staging on every call (G-216). Only the 16
        // colour bytes of each slot are written, so the alignment padding `resize` fills with
        // zero stays zero.
        let vertex_len = {
            let pack = &mut cache.pack;
            pack.vertices.clear();
            pack.uniform.resize(stride * rects.len(), 0);
            for (index, &(x, y, rw, rh, color)) in rects.iter().enumerate() {
                // Convert pixel coordinates to NDC: [-1, 1]
                // WebGPU/wgpu NDC: y = +1 maps to top of viewport, y = -1 to bottom
                // (OpenGL convention, not Vulkan).
                let x0 = (x as f32 / w) * 2.0 - 1.0;
                let y0 = 1.0 - (y as f32 / h) * 2.0;
                let x1 = ((x.saturating_add(rw as i32)) as f32 / w) * 2.0 - 1.0;
                let y1 = 1.0 - ((y.saturating_add(rh as i32)) as f32 / h) * 2.0;

                // Two triangles: bottom-left, bottom-right, top-left, then bottom-right, top-right, top-left
                for value in [x0, y0, x1, y0, x0, y1, x1, y0, x1, y1, x0, y1] {
                    pack.vertices.extend_from_slice(&value.to_le_bytes());
                }

                let slot = index * stride;
                for (channel, value) in color.iter().enumerate() {
                    pack.uniform[slot + channel * 4..slot + channel * 4 + 4]
                        .copy_from_slice(&((*value).clamp(0.0, 1.0) as f32).to_le_bytes());
                }
            }
            pack.vertices.len()
        };
        cache.ensure_vertices(&self.device, vertex_len as u64);
        let RenderCache { uniform, target, stencil, vertices, pack, .. } = &mut *cache;
        let uniform = uniform.as_ref().expect("the arena was just ensured");
        let target = target.as_ref().expect("the target was just ensured");
        let stencil = stencil.as_ref().expect("the stencil target was just ensured");
        let vertices = vertices.as_ref().expect("the vertices were just ensured");

        // One upload for the whole colour arena and one for every vertex, instead of one of
        // each per rectangle (REND-02).
        if !pack.uniform.is_empty() {
            self.queue.write_buffer(&uniform.buffer, 0, &pack.uniform);
        }
        if !pack.vertices.is_empty() {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.vertices);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_rect_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_rect_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // The stencil attachment every 2D pass carries: path fills read and write it, and
                // WebGPU requires the pipelines in a pass to name the attachment's format even if
                // they never touch it. Cleared here and discarded at the end of the pass.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &stencil.view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.rect_pipeline);
            rp.set_vertex_buffer(0, vertices.buffer.slice(..vertex_len as u64));

            for index in 0..rects.len() {
                let offset = (index * stride) as u32;
                rp.set_bind_group(0, &uniform.bind_group, &[offset]);
                let start = (index * 6) as u32;
                let end = ((index + 1) * 6) as u32;
                rp.draw(start..end, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// The batched path: turns every rectangle into a quad and renders the whole run as an
    /// instanced draw through the quad pipeline.
    ///
    /// The identity affine composed with the orthographic pixel-to-clip matrix is exactly the
    /// matrix [`ortho_mvp_bytes`] writes, so the geometry is identical to the per-rect path; only
    /// the draw count changes (G-64).
    fn encode_fill_rects_instanced(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        let mut quads: Vec<([crate::core::Point; 4], [f64; 4])> = Vec::with_capacity(rects.len());
        for &(x, y, rw, rh, colour) in rects {
            // Saturating addition mirrors the 2D path's treatment of an extreme width/height.
            let x1 = x.saturating_add(rw as i32);
            let y1 = y.saturating_add(rh as i32);
            quads.push((
                [
                    crate::core::Point::new(x, y),
                    crate::core::Point::new(x1, y),
                    crate::core::Point::new(x1, y1),
                    crate::core::Point::new(x, y1),
                ],
                colour,
            ));
        }
        let mvp_bytes = ortho_mvp_bytes(width, height);
        self.encode_instances(width, height, &mvp_bytes, &quads)
    }

    /// Renders a run of quads into the cached target with instanced draws.
    ///
    /// `mvp_bytes` is the column-major model-view-projection matrix every instance is multiplied
    /// by. All instance records are written once into the grow-only buffer and the run is drawn in
    /// `MAX_INSTANCES_PER_DRAW`-sized chunks. Each chunk reads its own slice of that one upload,
    /// so no chunk overwrites another and no quad allocates a buffer. Chunks are submitted in
    /// order, which is what keeps overlapping quads in painter order.
    ///
    /// The target is cleared to transparent first, so an empty run is a transparent frame rather
    /// than stale pixels from a previous call.
    fn encode_instances(
        &self,
        width: u32,
        height: u32,
        mvp_bytes: &[u8; 64],
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<(), String> {
        // Cached resources for the whole call: one grow-only instance buffer, one target and
        // one mvp uniform (REND-02). The records are packed into the reused scratch buffer so
        // the frame allocates nothing on the CPU side (G-216).
        let mut cache = self.cache.borrow_mut();
        let instance_len = {
            let pack = &mut cache.pack;
            pack_instanced_quads(quads, &mut pack.instances)?;
            pack.instances.len()
        };
        cache.ensure_target(&self.device, width, height);
        cache.ensure_stencil(&self.device, width, height);
        cache.ensure_vertices(&self.device, instance_len as u64);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let RenderCache { target, stencil, vertices, transformed_uniform, pack, .. } = &mut *cache;
        let target = target.as_ref().expect("the target was just ensured");
        let stencil = stencil.as_ref().expect("the stencil target was just ensured");
        let instances = vertices.as_ref().expect("the instance buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, mvp_bytes);
        if !pack.instances.is_empty() {
            self.queue.write_buffer(&instances.buffer, 0, &pack.instances);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_quads_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_quads_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // The stencil attachment every 2D pass carries: path fills read and write it, and
                // WebGPU requires the pipelines in a pass to name the attachment's format even if
                // they never touch it. Cleared here and discarded at the end of the pass.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &stencil.view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.transformed_pipeline);
            rp.set_bind_group(0, &transformed.bind_group, &[]);
            for (chunk_index, chunk) in quads.chunks(MAX_INSTANCES_PER_DRAW).enumerate() {
                let start = chunk_index * MAX_INSTANCES_PER_DRAW * INSTANCE_BYTES;
                let end = start + chunk.len() * INSTANCE_BYTES;
                rp.set_vertex_buffer(0, instances.buffer.slice(start as u64..end as u64));
                rp.draw(0..6, 0..chunk.len() as u32);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Renders a run of quads through an arbitrary affine transform (`3d` feature).
    ///
    /// The affine is applied to the local corners before the orthographic pixel-to-clip matrix,
    /// matching the software `FillQuad` path.
    #[cfg(feature = "3d")]
    fn encode_quads_instanced(
        &self,
        width: u32,
        height: u32,
        affine: &crate::render::math3d::Mat3,
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<(), String> {
        let w = width as f32;
        let h = height as f32;
        let ortho = crate::render::math3d::Mat3::scaling(2.0 / w, -2.0 / h)
            .then(&crate::render::math3d::Mat3::translation(-1.0, 1.0));
        let mvp = affine.then(&ortho);
        let mvp_bytes = mvp_column_major_bytes(&crate::render::math3d::Mat4::from_affine(mvp));
        self.encode_instances(width, height, &mvp_bytes, quads)
    }

    /// Renders filled rectangles into the cached target and hands the **view** to `f`.
    ///
    /// This is the texture/view frame path REND-02 asked for: a caller that composites the
    /// result on the GPU never pays for `copy_texture_to_buffer` or a blocking `map_async`.
    /// [`Self::render_fill_rect_gpu`] remains for callers that genuinely need pixels (tests,
    /// screenshots, a host with no GPU compositor).
    pub fn with_fill_rect_texture<R>(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
        f: impl FnOnce(&wgpu::TextureView) -> R,
    ) -> Result<R, String> {
        rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Err("with_fill_rect_texture needs at least one rectangle".to_string());
        }
        self.encode_fill_rects(width, height, rects)?;
        let cache = self.cache.borrow();
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        Ok(f(&target.view))
    }

    /// Render filled rectangles using the GPU pipeline with vertex buffer shaders.
    ///
    /// Each rect is defined as `(x, y, width, height, [r, g, b, a])` where:
    /// - `(x, y)` is the top-left corner in pixel coordinates
    /// - `(width, height)` is the rectangle size in pixels
    /// - `color` is an RGBA float array with values in `[0.0, 1.0]`
    ///
    /// Uses the WGSL vertex shader (`FILL_RECT_VS`) with a vertex buffer and the
    /// fragment shader (`FILL_RECT_FS`) with a per-rect uniform colour for a single rectangle.
    /// A longer run is drawn as instanced quad batches through the matrix pipeline (G-64).
    pub fn render_fill_rect_gpu(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Reject zero/overflowing dimensions up front so the empty-rects fast
        // path cannot allocate a wrapped-around buffer either (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        self.encode_fill_rects(width, height, rects)?;

        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().expect("the target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_rect_readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Renders arbitrary quadrilaterals through the instanced `3d` matrix pipeline.
    ///
    /// Each quad is a `([Point; 4], [r, g, b, a])` pair. The four corners are **local logical
    /// pixels** in perimeter order; `affine` is applied to them in the vertex stage, and the
    /// renderer composes that with an orthographic pixel-to-clip matrix. The result is the same
    /// geometry the software backend draws for
    /// [`crate::render::RenderCommand::FillQuad`], which is what lets a 45-degree rotated
    /// rectangle agree across the two paths.
    ///
    /// The whole run is one upload and, below the per-draw cap, one instanced draw; a longer run
    /// streams in order from the same buffer. Culling is disabled so a mirrored (negative-scale)
    /// transform still draws. There is no depth attachment: this path is a 2D affine, and scene
    /// layers keep their existing painter's-order responsibility for control-level occlusion.
    #[cfg(feature = "3d")]
    pub fn render_fill_quads_gpu(
        &self,
        width: u32,
        height: u32,
        affine: &crate::render::math3d::Mat3,
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if quads.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        self.encode_quads_instanced(width, height, affine, quads)?;

        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().expect("the target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_quads_readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Renders an indexed triangle mesh through the depth-tested `3d` pipeline.
    ///
    /// `model` places the mesh, `camera` supplies the projection and `viewport` selects the part of
    /// the `width × height` target the camera looks through. The composition is
    /// `model × view-projection × viewport-matrix`, the same one
    /// [`crate::render::mesh3d::project_mesh`] uses, so the GPU and software pictures describe the
    /// same geometry.
    ///
    /// The pass clears colour to transparent and depth to `1.0`, then runs one indexed draw with a
    /// `Depth32Float` attachment compared by `Less`. No triangle is culled: the depth test decides
    /// visibility, matching the documented software policy. The target is read back and returned as
    /// tightly packed RGBA8.
    ///
    /// An empty mesh or an empty viewport returns a transparent frame rather than an error, so a
    /// caller may drop a mesh without special-casing the render call.
    #[cfg(feature = "3d")]
    pub fn render_mesh_3d_gpu(
        &self,
        width: u32,
        height: u32,
        mesh: &crate::render::mesh3d::Mesh3D,
        model: &crate::render::math3d::Mat4,
        camera: &crate::render::mesh3d::Camera,
        viewport: crate::core::Rect,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if mesh.triangle_count() == 0 || viewport.width == 0 || viewport.height == 0 {
            return Ok(vec![0u8; expected_len]);
        }

        let aspect = viewport.width as f32 / viewport.height as f32;
        let mvp = model.then(&camera.view_projection(aspect)).then(
            &crate::render::mesh3d::viewport_matrix(
                viewport,
                crate::core::Size::new(width, height),
            ),
        );
        let mvp_bytes = mvp_column_major_bytes(&mvp);
        let index_count = mesh.indices().len() as u32;
        if index_count == 0 {
            return Ok(vec![0u8; expected_len]);
        }

        let mut cache = self.cache.borrow_mut();
        // Pack straight into the reused scratch buffers so a mesh frame allocates nothing on
        // the CPU side (G-216). The strides, the shader's attribute offsets and these writers
        // must agree exactly; a mismatch is a pipeline validation error rather than a silent
        // misread.
        let (vertex_len, index_len) = {
            let pack = &mut cache.pack;
            pack.mesh_vertices.clear();
            pack.mesh_indices.clear();
            for vertex in mesh.vertices() {
                for value in [vertex.position.x, vertex.position.y, vertex.position.z, 1.0] {
                    pack.mesh_vertices.extend_from_slice(&value.to_le_bytes());
                }
                for channel in [vertex.color.r, vertex.color.g, vertex.color.b, vertex.color.a] {
                    pack.mesh_vertices.extend_from_slice(&((channel as f32) / 255.0).to_le_bytes());
                }
            }
            for index in mesh.indices() {
                pack.mesh_indices.extend_from_slice(&index.to_le_bytes());
            }
            (pack.mesh_vertices.len(), pack.mesh_indices.len())
        };
        cache.ensure_target(&self.device, width, height);
        cache.ensure_depth_target(&self.device, width, height);
        cache.ensure_vertices(&self.device, vertex_len as u64);
        cache.ensure_mesh_indices(&self.device, index_len as u64);
        cache.ensure_mesh_uniform(&self.device, &self.transformed_bind_group_layout);
        let RenderCache {
            target, depth_target, vertices, mesh_indices, mesh_uniform, pack, ..
        } = &mut *cache;
        let target = target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        let depth = depth_target.as_ref().ok_or_else(|| "depth target missing".to_string())?;
        let vertices = vertices.as_ref().ok_or_else(|| "vertex buffer missing".to_string())?;
        let indices = mesh_indices.as_ref().ok_or_else(|| "index buffer missing".to_string())?;
        let uniform = mesh_uniform.as_ref().ok_or_else(|| "mvp uniform missing".to_string())?;

        self.queue.write_buffer(&uniform.buffer, 0, &mvp_bytes);
        if !pack.mesh_vertices.is_empty() {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.mesh_vertices);
        }
        if !pack.mesh_indices.is_empty() {
            self.queue.write_buffer(&indices.buffer, 0, &pack.mesh_indices);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_mesh_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_mesh_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.mesh_pipeline);
            rp.set_bind_group(0, &uniform.bind_group, &[]);
            rp.set_vertex_buffer(0, vertices.buffer.slice(..vertex_len as u64));
            rp.set_index_buffer(
                indices.buffer.slice(..index_len as u64),
                wgpu::IndexFormat::Uint32,
            );
            rp.draw_indexed(0..index_count, 0, 0..1);
        }
        drop(cache);
        self.queue.submit(Some(encoder.finish()));
        self.read_target(width, height, expected_len, "gpu_mesh_readback")
    }

    /// Render stroked (outlined) rectangles using the GPU pipeline.
    /// Each stroke rect is decomposed into 4 thin filled rectangles
    /// (top, bottom, left, right edges) and rendered via the existing FillRect pipeline.
    #[allow(clippy::type_complexity)]
    pub fn render_stroke_rect_gpu(
        &self,
        width: u32,
        height: u32,
        rects: &[((i32, i32, u32, u32, u32), [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        // Decompose each stroke rect into 4 fill rects
        let mut fill_rects: Vec<((i32, i32, u32, u32), [f64; 4])> =
            Vec::with_capacity(rects.len() * 4);

        for &((x, y, rw, rh, sw), color) in rects {
            let sw_i = sw as i32;
            // Top edge
            fill_rects.push(((x, y, rw, sw), color));
            // Bottom edge
            fill_rects.push(((x, y + rh as i32 - sw_i, rw, sw), color));
            // Left edge (excludes corners already covered by top/bottom)
            let inner_h = rh.saturating_sub(sw * 2);
            if inner_h > 0 {
                fill_rects.push(((x, y + sw_i, sw, inner_h), color));
            }
            // Right edge
            if inner_h > 0 {
                fill_rects.push(((x + rw as i32 - sw_i, y + sw_i, sw, inner_h), color));
            }
        }

        // Convert fill_rects (without stroke_width) back to the (i32, i32, u32, u32, [f64; 4]) format
        let rect_input: Vec<(i32, i32, u32, u32, [f64; 4])> =
            fill_rects.into_iter().map(|((x, y, w, h), color)| (x, y, w, h, color)).collect();
        self.render_fill_rect_gpu(width, height, &rect_input)
    }

    /// Render lines using the GPU pipeline.
    /// Each line is approximated as a thin filled rectangle.
    /// Horizontal/vertical-aligned lines are rendered accurately;
    /// diagonal lines are approximated as axis-aligned thin rects.
    #[allow(clippy::type_complexity)]
    pub fn render_draw_line_gpu(
        &self,
        width: u32,
        height: u32,
        lines: &[((i32, i32, i32, i32, u32), [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if lines.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        let mut fill_rects: Vec<(i32, i32, u32, u32, [f64; 4])> = Vec::with_capacity(lines.len());

        for &((x1, y1, x2, y2, lw), color) in lines {
            let lw_i = lw as i32;
            let dx = (x2 - x1).abs();
            let dy = (y2 - y1).abs();

            if dx >= dy {
                // More horizontal: render as horizontal thin rect
                let min_x = x1.min(x2);
                let line_len = dx.max(1) as u32;
                fill_rects.push((min_x, y1 - lw_i / 2, line_len, lw, color));
            } else {
                // More vertical: render as vertical thin rect
                let min_y = y1.min(y2);
                let line_len = dy.max(1) as u32;
                fill_rects.push((x1 - lw_i / 2, min_y, lw, line_len, color));
            }
        }

        self.render_fill_rect_gpu(width, height, &fill_rects)
    }

    /// Render filled circles using the SDF circle pipeline (G-219).
    ///
    /// Each circle is **one draw**: a bounding quad is transformed by the shared pixel-to-clip
    /// matrix and [`super::shaders::CIRCLE_FRAG`] shades it with a distance-field test, so the
    /// silhouette is evaluated per fragment instead of being approximated by per-column
    /// one-pixel strips. Where the strips needed `2r + 1` rectangles per circle, this path
    /// needs one 48-byte quad, two uniform slots and one draw call, and the SDF edge is
    /// anti-aliased rather than stair-stepped.
    ///
    /// Behaviour notes: a zero-radius circle draws nothing, matching the software rasteriser's
    /// skip, and a circle that covers no pixel is skipped exactly like the software path. The
    /// anti-aliased edge means boundary pixels take partial alpha where the strip approximation
    /// painted hard steps — that difference is the point of the item.
    #[allow(clippy::type_complexity)]
    pub fn render_fill_circle_gpu(
        &self,
        width: u32,
        height: u32,
        circles: &[((i32, i32, u32), [f64; 4])], // ((cx, cy, radius), color)
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        // A zero-radius circle draws nothing on the software path either, so a frame of only
        // zero-radius circles is a transparent frame with no GPU work, exactly as before.
        if circles.iter().all(|((_, _, radius), _)| *radius == 0) {
            return Ok(vec![0u8; expected_len]);
        }

        // Cached resources for the whole call: one two-slot-per-circle uniform arena, one
        // vertex buffer, one mvp uniform and one target (REND-02, G-216).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_circle_uniforms(
            &self.device,
            &self.circle_bind_group_layout,
            circles.len(),
        )?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_stencil(&self.device, width, height);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let stride = RenderCache::uniform_stride(&self.device) as usize;

        // Pack every drawable circle into the reused scratch buffers: one vertex upload and
        // one uniform upload for the whole frame (G-216). `packed` stays dense: a circle
        // skipped as fully off-screen occupies no vertex bytes and no uniform slots, so the
        // draw loop's indices line up with the packed data exactly.
        let (vertex_len, uniform_len, packed) = {
            let pack = &mut cache.pack;
            pack.circle_vertices.clear();
            pack.circle_uniforms.resize(stride * 2 * circles.len(), 0);
            let mut packed = 0usize;
            for &((cx, cy, radius), colour) in circles {
                if radius == 0 {
                    continue;
                }
                let Some(quad) = circle_quad((cx, cy), radius, width, height) else {
                    continue;
                };
                pack_circle(
                    &mut pack.circle_vertices,
                    &mut pack.circle_uniforms,
                    stride,
                    packed,
                    (cx, cy),
                    radius,
                    &colour,
                    quad,
                );
                packed += 1;
            }
            (pack.circle_vertices.len(), pack.circle_uniforms.len(), packed)
        };
        cache.ensure_vertices(&self.device, vertex_len as u64);
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let RenderCache {
            target,
            stencil,
            vertices,
            transformed_uniform,
            circle_uniforms,
            readback,
            pack,
            ..
        } = &mut *cache;
        let target = target.as_ref().expect("the target was just ensured");
        let stencil = stencil.as_ref().expect("the stencil target was just ensured");
        let vertices = vertices.as_ref().expect("the vertex buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");
        let circle_arena = circle_uniforms.as_ref().expect("the circle arena was just ensured");
        let readback = readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, &ortho_mvp_bytes(width, height));
        if vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.circle_vertices);
        }
        if uniform_len > 0 {
            self.queue.write_buffer(&circle_arena.buffer, 0, &pack.circle_uniforms);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_circle_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_circle_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // The stencil attachment every 2D pass carries: path fills read and write it, and
                // WebGPU requires the pipelines in a pass to name the attachment's format even if
                // they never touch it. Cleared here and discarded at the end of the pass.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &stencil.view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.circle_pipeline);
            // The mvp matrix is group 1 for every draw; group 0's dynamic offsets select each
            // circle's colour and parameter slots. One draw per circle (G-219).
            rp.set_bind_group(1, &transformed.bind_group, &[]);
            for index in 0..packed {
                let quad_start = (index * CIRCLE_QUAD_BYTES) as u64;
                let quad_end = quad_start + CIRCLE_QUAD_BYTES as u64;
                let colour_offset = (index * 2 * stride) as u32;
                let params_offset = ((index * 2 + 1) * stride) as u32;
                rp.set_bind_group(0, &circle_arena.bind_group, &[colour_offset, params_offset]);
                rp.set_vertex_buffer(0, vertices.buffer.slice(quad_start..quad_end));
                rp.draw(0..6, 0..1);
            }
        }

        let padded_bytes_per_row = padded_row_bytes(width)?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Render command list with deterministic ordering and clipping.
    ///
    /// # GPU dispatch (G-218)
    ///
    /// A stream the existing pipelines can cover — whole-frame clears, unclipped filled
    /// rectangles and unclipped filled circles, with blending left at `Normal` — is drawn
    /// entirely on the GPU in one render pass: clears repaint through the full-screen clear
    /// pipeline, rectangle runs go through the instanced quad pipeline, and each circle is
    /// one SDF circle draw. Painter order is replayed exactly, so mid-stream clears and
    /// overlaps behave as on the CPU. The circles' SDF edge is anti-aliased, which is the
    /// mandated difference from the CPU rasteriser's hard-edged disc.
    ///
    /// Every other stream falls back to [`rasterize_draw_commands_rgba8`], **returned
    /// directly**: the old upload-and-readback round trip reproduced those exact bytes
    /// (`upload_round_trips_the_given_pixels` pins the identity), so skipping it also skips
    /// the texture upload, the copy and the blocking readback for frames that never needed
    /// the GPU (G-217). A caller that only uploads or reads the frame can use
    /// [`Self::render_draw_commands_cached_rgba8`] to reuse the renderer's buffer instead.
    pub fn render_draw_commands_rgba8(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        // Before deciding which path draws the stream: a payload that disagrees with its declared
        // grid is an error the *caller* asked for, so it is reported here with the CPU rasteriser's
        // wording instead of being turned into a reason to use the CPU.
        if let Some(error) = invalid_image_payload(commands) {
            return Err(error);
        }
        if commands_gpu_dispatchable_with(commands, self.blend_modes_on_gpu) {
            return self.encode_draw_commands_gpu(width, height, expected_len, commands);
        }
        rasterize_draw_commands_rgba8(width, height, commands)
    }

    /// The borrowed counterpart of [`Self::render_draw_commands_rgba8`] (G-221).
    ///
    /// Same dispatch decision, same picture, same errors — only the ownership differs: this one
    /// keeps the bytes in the renderer's reusable buffer and lends them back, so a caller that
    /// uploads or reads the frame never owns it. A dispatchable stream is encoded on the GPU and
    /// read back into that buffer (one copy), and anything the dispatcher refuses is rasterised on
    /// the CPU into it directly, exactly as the owned entry point would have.
    ///
    /// An earlier form of this method always rasterised on the CPU, which meant one command stream
    /// could produce two different pictures depending on which entry point a host happened to call.
    pub fn render_draw_commands_cached_rgba8(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Ref<'_, [u8]>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if let Some(error) = invalid_image_payload(commands) {
            return Err(error);
        }
        if commands_gpu_dispatchable_with(commands, self.blend_modes_on_gpu) {
            let pixels = self.encode_draw_commands_gpu(width, height, expected_len, commands)?;
            return self.commands_frame.store(width, height, &pixels);
        }
        self.commands_frame.rasterize(width, height, commands)
    }

    /// Draws a dispatchable command stream entirely on the GPU (G-218).
    ///
    /// One render pass covers the frame. The pass loads transparent — the software
    /// rasteriser's zeroed start — and every [`WgpuDrawCommand::Clear`] repaints the whole
    /// target through the clear pipeline, so position in the list decides the result exactly
    /// as the command documentation promises. Geometry is packed once into the reused scratch
    /// buffers (one instance upload, one circle-quad upload, one uniform upload) and the draw
    /// order is recorded in `DrawStep`s, which the pass replays in list order.
    /// Uploads one image draw and returns the bind group and scissor the pass draws it with.
    ///
    /// The texture is created per draw because the pixels belong to this frame's image. The
    /// params uniform is 32 bytes (the destination rectangle and the source grid, each a
    /// `vec4f`), and the bind group binds the two together; the bind group keeps the texture, its
    /// view and the buffer alive for as long as the pass needs them, so nothing else holds them.
    ///
    /// Rows are uploaded at the image's own pitch: `Queue::write_texture` stages the copy itself
    /// and accepts the tightly packed buffer, so no padded copy is made here.
    fn encode_draw_commands_gpu(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let framebuffer = PixelRect { x: 0, y: 0, width, height };
        let clear_count = commands
            .iter()
            .filter(|command| matches!(command, WgpuDrawCommand::Clear { .. }))
            .count();
        let circle_count = commands
            .iter()
            .filter(|command| matches!(command, WgpuDrawCommand::FillCircle { radius, .. } if *radius > 0))
            .count();

        // Cached resources for the whole call (REND-02). The clear pipeline's colour slots
        // and the circles' two-slot records share the dynamic-offset stride.
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, clear_count)?;
        cache.ensure_circle_uniforms(&self.device, &self.circle_bind_group_layout, circle_count)?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_stencil(&self.device, width, height);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let stride = RenderCache::uniform_stride(&self.device) as usize;

        // Pack the frame: instance records for rectangles, bounding quads and two uniform
        // slots per circle, one colour slot per clear, one uploaded texture per image, and the
        // draw order (G-216).
        //
        // The image table is not part of the pack scratch because its entries own GPU handles;
        // it lives for the whole call so the pass can bind each one.
        let mut image_draws: Vec<ImageDraw> = Vec::new();
        let mut text_draws: Vec<TextDraw> = Vec::new();
        let mut shape_draws: Vec<ShapeDraw> = Vec::new();
        let mut radial_draws: Vec<RadialDraw> = Vec::new();
        let mut path_draws: Vec<PathDraw> = Vec::new();
        let mut shadow_draws: Vec<ShadowDraw> = Vec::new();
        let (
            instance_len,
            circle_vertex_len,
            path_vertex_len,
            shadow_vertex_len,
            uniform_len,
            circle_uniform_len,
            plan_len,
        ) = {
            let pack = &mut cache.pack;
            pack.instances.clear();
            pack.circle_vertices.clear();
            pack.path_vertices.clear();
            pack.shadow_instances.clear();
            pack.blend_runs.clear();
            pack.uniform.resize(stride * clear_count, 0);
            pack.circle_uniforms.resize(stride * 2 * circle_count, 0);
            pack.plan.clear();
            let mut clear_slot = 0usize;
            let mut circle_index = 0usize;
            let mut run_start = 0usize;
            // Sticky blend state, exactly as the CPU rasteriser treats `SetBlendMode`. With the
            // GPU blend option on, a command drawn under a non-`Normal` mode becomes its own run:
            // its steps stay in the plan but are bracketed by a trailer marker the renderer reads
            // back to replay them into an offscreen layer.
            let mut blend = crate::render::BlendMode::Normal;
            // The running clip is stream state, exactly as it is for the CPU rasteriser: a push
            // intersects the visible region and a pop restores the previous one. Rectangle draws
            // consume it by narrowing their geometry; circles and images consume it as a scissor.
            let mut clip_stack: Vec<PixelRect> = Vec::new();
            let mut current_clip: Option<PixelRect> = None;
            for command in commands {
                if let WgpuDrawCommand::SetBlendMode { mode } = command {
                    blend = blend_mode_from_id(*mode);
                    continue;
                }
                // `Clear`, `PushClip` and `PopClip` are not draws: a clear repaints the whole
                // frame through the clear pipeline and ignores the mode on the CPU too.
                let wrapped =
                    blend != crate::render::BlendMode::Normal && draws_under_blend(command);
                if wrapped {
                    // Flush the pending rectangle run so the instances drawn so far stay with the
                    // commands before this one, and the run holds only what this command draws.
                    if run_start < pack.instances.len() / INSTANCE_BYTES {
                        pack.plan.push(DrawStep::Rects(
                            run_start,
                            pack.instances.len() / INSTANCE_BYTES,
                        ));
                        run_start = pack.instances.len() / INSTANCE_BYTES;
                    }
                }
                let steps_before = pack.plan.len();
                match command {
                    WgpuDrawCommand::Clear { color } => {
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let slot = clear_slot;
                        clear_slot += 1;
                        let base = slot * stride;
                        for (channel, value) in
                            [color.r, color.g, color.b, color.a].iter().enumerate()
                        {
                            pack.uniform[base + channel * 4..base + channel * 4 + 4]
                                .copy_from_slice(&((*value as f32 / 255.0).to_le_bytes()));
                        }
                        pack.plan.push(DrawStep::Clear(slot));
                    }
                    // A blurred shadow's form is a separate pass (`super::shadow`): its body into
                    // an offscreen layer, two box-blur passes, then a composite here in stream order.
                    WgpuDrawCommand::BoxShadow { blur_radius, .. } if *blur_radius > 0 => {
                        let Some((draw, paint_region)) = pack_blurred_shadow(
                            command,
                            framebuffer,
                            current_clip,
                            &mut pack.shadow_instances,
                        ) else {
                            continue;
                        };
                        // Flush the rectangle run so the shadow composites in painter order.
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        shadow_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(paint_region));
                        pack.plan.push(DrawStep::Shadow(shadow_draws.len() - 1));
                    }
                    WgpuDrawCommand::FillRect { .. }
                    | WgpuDrawCommand::StrokeRect { .. }
                    | WgpuDrawCommand::FillRoundedRect { .. }
                    | WgpuDrawCommand::BoxShadow { .. }
                    | WgpuDrawCommand::DrawLine { .. }
                    | WgpuDrawCommand::DrawPath { filled: false, .. }
                        if pack_rect_command(
                            command,
                            framebuffer,
                            current_clip,
                            &mut pack.instances,
                        ) => {}
                    WgpuDrawCommand::PushClip { rect } => {
                        // Intersect rather than replace: a nested clip can only shrink the visible
                        // area, and a non-overlapping pair becomes an empty region that suppresses
                        // drawing rather than silently ignoring the inner clip.
                        let next = match current_clip {
                            Some(existing) => existing.intersect(*rect).unwrap_or(PixelRect {
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                            }),
                            None => *rect,
                        };
                        clip_stack.push(current_clip.unwrap_or(framebuffer));
                        current_clip = Some(next);
                    }
                    WgpuDrawCommand::PopClip => match clip_stack.pop() {
                        Some(restored) => {
                            current_clip =
                                if restored == framebuffer { None } else { Some(restored) };
                        }
                        None => log::warn!(
                            "[wgpu-encode] PopClip without a matching PushClip; the current clip \
                             is left unchanged"
                        ),
                    },
                    WgpuDrawCommand::FillCircle { center, radius, color, clip } => {
                        if *radius == 0 {
                            continue;
                        }
                        let Some(quad) = circle_quad(*center, *radius, width, height) else {
                            continue;
                        };
                        // A disc cannot be clipped by narrowing a rectangle, so the running clip
                        // becomes the scissor. The CPU arm clips against the circle's bounding box,
                        // which is the same region.
                        let bounds = bbox_for_circle(*center, *radius, 0);
                        let Some(scissor) =
                            effective_clip(framebuffer, bounds, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack_circle(
                            &mut pack.circle_vertices,
                            &mut pack.circle_uniforms,
                            stride,
                            circle_index,
                            *center,
                            *radius,
                            &[
                                color.r as f64 / 255.0,
                                color.g as f64 / 255.0,
                                color.b as f64 / 255.0,
                                color.a as f64 / 255.0,
                            ],
                            quad,
                        );
                        pack.plan.push(DrawStep::Circle(circle_index));
                        circle_index += 1;
                    }
                    WgpuDrawCommand::DrawCircle { center, radius, color, width: w, clip } => {
                        if *radius == 0 || *w == 0 {
                            continue;
                        }
                        let bounds = bbox_for_circle(*center, *radius, *w);
                        let Some(scissor) =
                            effective_clip(framebuffer, bounds, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let draw = self.upload_shape_draw(
                            bounds,
                            [center.0 as f32, center.1 as f32, *radius as f32, *w as f32],
                            [0.0, 0.0, SHAPE_CIRCLE_STROKE, 0.0],
                            *color,
                        );
                        shape_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Shape(shape_draws.len() - 1));
                    }
                    WgpuDrawCommand::DrawArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        color,
                        filled,
                        clip,
                    } => {
                        if *radius == 0 {
                            continue;
                        }
                        // Full circles are expressed as a 0..TAU window, exactly as the CPU arm
                        // normalises them, so the shader never has to special-case it.
                        let (start, end) = if (start_angle - end_angle).abs() < 0.001 {
                            (0.0f32, core::f32::consts::TAU)
                        } else {
                            let start = start_angle.rem_euclid(core::f32::consts::TAU);
                            let mut end = end_angle.rem_euclid(core::f32::consts::TAU);
                            if end <= start {
                                end += core::f32::consts::TAU;
                            }
                            (start, end)
                        };
                        let bounds = bbox_for_circle(*center, *radius, if *filled { 0 } else { 1 });
                        let Some(scissor) =
                            effective_clip(framebuffer, bounds, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let mode = if *filled { SHAPE_ARC_WEDGE } else { SHAPE_ARC_STROKE };
                        let draw = self.upload_shape_draw(
                            bounds,
                            [center.0 as f32, center.1 as f32, *radius as f32, 1.0],
                            [start, end, mode, 0.0],
                            *color,
                        );
                        shape_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Shape(shape_draws.len() - 1));
                    }
                    WgpuDrawCommand::StrokeRoundedRect { rect, radius, color, thickness, clip } => {
                        if *radius == 0 || *thickness == 0 {
                            continue;
                        }
                        let Some(scissor) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        // The same corner clamp the CPU arm applies, so the arcs agree.
                        let corner = (*radius).min(rect.width / 2).min(rect.height / 2) as f32;
                        let draw = self.upload_shape_draw(
                            *rect,
                            [0.0, 0.0, 0.0, *thickness as f32],
                            [0.0, 0.0, SHAPE_ROUNDED_OUTLINE, corner],
                            *color,
                        );
                        shape_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Shape(shape_draws.len() - 1));
                    }
                    WgpuDrawCommand::DrawImage { rect, rgba8, image_width, image_height, clip } => {
                        // The CPU rasteriser writes only the intersection of the destination, the
                        // framebuffer and the running clip. That intersection becomes this draw's
                        // scissor rectangle, so both paths paint the same pixels.
                        let Some(scissor) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        // An image is its own draw, so the rectangle run built so far is flushed
                        // first: painter order must survive the mixed frame.
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let draw =
                            self.upload_image_draw(*rect, rgba8, *image_width, *image_height);
                        image_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Image(image_draws.len() - 1));
                    }
                    WgpuDrawCommand::DrawGradient { rect, gradient_data, clip }
                    | WgpuDrawCommand::FillLinearGradient { rect, gradient_data, clip } => {
                        if rect.width == 0 || rect.height == 0 || gradient_data.len() < 4 {
                            continue;
                        }
                        let axis = if matches!(command, WgpuDrawCommand::DrawGradient { .. }) {
                            GradientAxis::Vertical
                        } else {
                            GradientAxis::Horizontal
                        };
                        let Some(scissor) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        // One colour per row (vertical) or per column (horizontal), from the CPU's
                        // own stop rule; the image pipeline repeats it across the span with its
                        // identity integer mapping, so no new shader is needed.
                        let (ramp, tex_w, tex_h) = match axis {
                            GradientAxis::Vertical => {
                                (gradient_ramp(*rect, gradient_data, axis), 1, rect.height)
                            }
                            GradientAxis::Horizontal => {
                                (gradient_ramp(*rect, gradient_data, axis), rect.width, 1)
                            }
                        };
                        let draw = self.upload_image_draw(*rect, &ramp, tex_w, tex_h);
                        image_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Image(image_draws.len() - 1));
                    }
                    WgpuDrawCommand::FillRadialGradient { rect, gradient_data, clip } => {
                        if rect.width == 0 || rect.height == 0 || gradient_data.len() < 4 {
                            continue;
                        }
                        let Some(scissor) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        radial_draws.push(self.upload_radial_draw(*rect, gradient_data));
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Radial(radial_draws.len() - 1));
                    }
                    WgpuDrawCommand::FillPathNonZero { points, contours, color, clip } => {
                        if points.len() < 3 || contours.is_empty() {
                            continue;
                        }
                        let Some(bounds) = path_bounds(points, framebuffer) else {
                            continue;
                        };
                        let Some(scissor) =
                            effective_clip(framebuffer, bounds, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        let fan = path_fan_vertices(points, contours);
                        if fan.is_empty() {
                            continue;
                        }
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let first_vertex = pack.path_vertices.len() as u32 / 8;
                        for (x, y) in &fan {
                            pack.path_vertices.extend_from_slice(&x.to_le_bytes());
                            pack.path_vertices.extend_from_slice(&y.to_le_bytes());
                        }
                        path_draws.push(PathDraw {
                            cover_bind_group: self.upload_path_cover(bounds, *color),
                            first_vertex,
                            vertex_count: fan.len() as u32,
                            even_odd: false,
                        });
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Path(path_draws.len() - 1));
                    }
                    WgpuDrawCommand::DrawPath { points, color, filled: true, clip, .. } => {
                        if points.len() < 3 {
                            continue;
                        }
                        let Some(bounds) = path_bounds(points, framebuffer) else {
                            continue;
                        };
                        let Some(scissor) =
                            effective_clip(framebuffer, bounds, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        // A filled `DrawPath` is one implicitly closed contour and uses the
                        // even-odd rule, which is what the command documents.
                        let contours = [(0usize, points.len())];
                        let fan = path_fan_vertices(points, &contours);
                        if fan.is_empty() {
                            continue;
                        }
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let first_vertex = pack.path_vertices.len() as u32 / 8;
                        for (x, y) in &fan {
                            pack.path_vertices.extend_from_slice(&x.to_le_bytes());
                            pack.path_vertices.extend_from_slice(&y.to_le_bytes());
                        }
                        path_draws.push(PathDraw {
                            cover_bind_group: self.upload_path_cover(bounds, *color),
                            first_vertex,
                            vertex_count: fan.len() as u32,
                            even_odd: true,
                        });
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Path(path_draws.len() - 1));
                    }
                    WgpuDrawCommand::DrawText { rect, text, color, clip } => {
                        if rect.width == 0 || rect.height == 0 || text.is_empty() {
                            continue;
                        }
                        // The CPU arm clips the text box; the same intersection is this draw's
                        // scissor, so glyphs outside it are never shaded.
                        let Some(scissor) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        // Text is its own draw: flush the rectangle run so painter order survives.
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        text_draws.push(self.upload_text_draw(*rect, text, *color));
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Text(text_draws.len() - 1));
                    }
                    // Handled above: the mode is stream state, not a draw.
                    WgpuDrawCommand::SetBlendMode { .. } => {}
                    // `commands_gpu_dispatchable` filters the stream, so no other command can
                    // reach this match; if one ever does, the CPU rasteriser still produces
                    // the correct frame, in the fallback frame that `cache`'s borrow cannot
                    // reach (G-221).
                    _ => {
                        return Ok(self
                            .fallback_frame
                            .rasterize(width, height, commands)?
                            .to_vec());
                    }
                }
                if wrapped {
                    // Everything this command packed belongs to the run, including the rectangle
                    // run it may have appended to.
                    if run_start < pack.instances.len() / INSTANCE_BYTES {
                        pack.plan.push(DrawStep::Rects(
                            run_start,
                            pack.instances.len() / INSTANCE_BYTES,
                        ));
                        run_start = pack.instances.len() / INSTANCE_BYTES;
                    }
                    let steps = pack.plan.len() - steps_before;
                    // A command that drew nothing (empty geometry, or clipped away) leaves no steps
                    // and therefore no run: the composite would be a no-op over an empty layer.
                    if steps > 0 {
                        let bbox =
                            blend_bbox(command, framebuffer, current_clip).unwrap_or(framebuffer);
                        // What the layer holds follows from how the run was drawn: text carries a
                        // coverage in its alpha, images and gradients arrive premultiplied, and
                        // the rectangle class is already straight.
                        let steps_of_run = &pack.plan[steps_before..];
                        let kind =
                            if steps_of_run.iter().any(|step| matches!(step, DrawStep::Text(_))) {
                                BlendKind::Coverage
                            } else if steps_of_run
                                .iter()
                                .all(|step| matches!(step, DrawStep::Rects(..)))
                            {
                                BlendKind::Straight
                            } else {
                                BlendKind::Premultiplied
                            };
                        pack.blend_runs.push(BlendRun { bbox, mode: blend, kind });
                        pack.plan.push(DrawStep::Blend { run: pack.blend_runs.len() - 1, steps });
                    }
                }
            }
            if run_start < pack.instances.len() / INSTANCE_BYTES {
                pack.plan.push(DrawStep::Rects(run_start, pack.instances.len() / INSTANCE_BYTES));
            }
            (
                pack.instances.len(),
                pack.circle_vertices.len(),
                pack.path_vertices.len(),
                pack.shadow_instances.len(),
                pack.uniform.len(),
                pack.circle_uniforms.len(),
                pack.plan.len(),
            )
        };

        // One grow-only vertex buffer serves both the instance records and the circle quads:
        // instances at `[0, instance_len)`, circle quads appended after.
        let circle_vertex_offset = instance_len as u64;
        let path_vertex_offset = (instance_len + circle_vertex_len) as u64;
        let shadow_vertex_offset = (instance_len + circle_vertex_len + path_vertex_len) as u64;
        cache.ensure_vertices(
            &self.device,
            (instance_len + circle_vertex_len + path_vertex_len + shadow_vertex_len) as u64,
        );
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let shadow_specs: Vec<PixelRect> = shadow_draws.iter().map(|draw| draw.layer).collect();
        cache.ensure_shadow_targets(
            &self.device,
            &self.blur_bind_group_layout,
            &self.image_bind_group_layout,
            &self.transformed_bind_group_layout,
            &shadow_specs,
        );
        // Only a frame that actually blends pays for the offscreen pair.
        let blend_run_count = cache.pack.blend_runs.len();
        if blend_run_count > 0 {
            cache.ensure_blend_targets(
                &self.device,
                &self.blend_bind_group_layout,
                width,
                height,
                blend_run_count,
            );
        }
        let RenderCache {
            uniform,
            target,
            stencil,
            vertices,
            transformed_uniform,
            circle_uniforms,
            readback,
            shadow,
            blend,
            pack,
            ..
        } = &mut *cache;
        let uniform = uniform.as_ref().expect("the colour arena was just ensured");
        let target = target.as_ref().expect("the target was just ensured");
        let stencil = stencil.as_ref().expect("the stencil target was just ensured");
        let vertices = vertices.as_ref().expect("the vertex buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");
        let circle_arena = circle_uniforms.as_ref().expect("the circle arena was just ensured");
        let readback = readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, &ortho_mvp_bytes(width, height));
        if uniform_len > 0 {
            self.queue.write_buffer(&uniform.buffer, 0, &pack.uniform);
        }
        if circle_uniform_len > 0 {
            self.queue.write_buffer(&circle_arena.buffer, 0, &pack.circle_uniforms);
        }
        if instance_len > 0 {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.instances);
        }
        if circle_vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, circle_vertex_offset, &pack.circle_vertices);
        }
        if path_vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, path_vertex_offset, &pack.path_vertices);
        }
        if shadow_vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, shadow_vertex_offset, &pack.shadow_instances);
        }
        let shadow_targets = shadow.as_mut().expect("the shadow layers were just ensured");
        self.write_shadow_layer_inputs(&shadow_draws, &mut shadow_targets.layers);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_draw_commands_encoder"),
        });
        // Every blurred shadow is rendered first, so the frame's pass only sees finished layers.
        self.encode_shadow_layers(
            &mut encoder,
            &shadow_draws,
            &shadow_targets.layers,
            &vertices.buffer,
            shadow_vertex_offset,
        );
        let recorder = StepRecorder {
            renderer: self,
            width,
            height,
            stride,
            vertices: &vertices.buffer,
            uniform: &uniform.bind_group,
            transformed: &transformed.bind_group,
            circle_arena: &circle_arena.bind_group,
            circle_vertex_offset,
            path_vertex_offset,
            image_draws: &image_draws,
            text_draws: &text_draws,
            shape_draws: &shape_draws,
            radial_draws: &radial_draws,
            path_draws: &path_draws,
            shadow_layers: &shadow_targets.layers,
            text_pipeline: &self.text_pipeline,
        };
        if let Some(targets) = blend.as_ref() {
            for (unit, run) in targets.units.iter().zip(&pack.blend_runs) {
                let mut bytes = [0u8; 16];
                bytes[0..4].copy_from_slice(&(run.mode as u32).to_le_bytes());
                let kind = match run.kind {
                    BlendKind::Straight => 0u32,
                    BlendKind::Premultiplied => 1,
                    BlendKind::Coverage => 2,
                };
                bytes[4..8].copy_from_slice(&kind.to_le_bytes());
                self.queue.write_buffer(&unit.params, 0, &bytes);
            }
        }
        let blend_pair = if pack.blend_runs.is_empty() {
            None
        } else {
            blend.as_ref().map(|targets| (targets, &pack.blend_runs[..]))
        };
        self.record_frame_passes(
            &mut encoder,
            &recorder,
            &pack.plan[..plan_len],
            target,
            &stencil.view,
            blend_pair,
        );

        let padded_bytes_per_row = padded_row_bytes(width)?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }
    /// Applies a separable Gaussian blur to an RGBA8 image on the GPU.
    ///
    /// # Why two passes
    ///
    /// A Gaussian kernel is separable: blurring rows and then columns costs `O(radius)` per pixel
    /// instead of `O(radius²)`. Each pass is one full-screen draw through [`super::shaders::BLUR_FS`],
    /// ping-ponging between two cached textures. The input is uploaded once, the result read back
    /// once, and every intermediate stage stays on the GPU.
    ///
    /// `radius` is the Gaussian's 2-sigma point, matching [`crate::render::RenderCommand::Blur`], and is clamped
    /// to the same 100-pixel ceiling the software path uses. A zero radius is an identity and
    /// returns the input bytes unchanged (no GPU round trip).
    ///
    /// This is the hardware path the backdrop-blur layer uses. It is a 2D effect, so it is compiled
    /// in every build rather than behind the `3d` feature.
    pub fn render_blur_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
        radius: u32,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if rgba8.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                rgba8.len(),
            ));
        }
        let radius = radius.min(100);
        if radius == 0 {
            return Ok(rgba8.to_vec());
        }
        self.encode_blur(width, height, rgba8, radius, None)?;
        let readback = self.cache.borrow();
        let readback =
            readback.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Encodes the two-pass separable blur — and, when `tint` is present, the tint composite
    /// folded into the vertical pass — into the cached blur targets, queues the copy of the
    /// result into the readback buffer and submits (G-220).
    ///
    /// Split from [`Self::render_blur_rgba8`] so the backdrop entry point can ride the same
    /// two passes with the tint on board: the composite happens in the fragment shader
    /// ([`super::shaders::BLUR_FS`]) instead of a read-back-to-CPU shading step, so the frame
    /// never leaves the GPU between the convolution and the tint.
    ///
    /// The tint carries the exact semantics the former CPU tint step implemented: a non-finite
    /// weight is ignored (rather than clamped, since `f32::clamp` propagates `NaN`), the weight
    /// clamps to `0.0..=1.0`, and a non-positive resulting alpha leaves the frame untouched.
    fn encode_blur(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
        radius: u32,
        tint: Option<(Rgba8, f32)>,
    ) -> Result<(), String> {
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_blur_targets(&self.device, &self.blur_bind_group_layout, width, height);
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let targets = cache.blur.as_ref().expect("the blur targets were just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let tint_state = tint.and_then(|(tint, weight)| {
            if !weight.is_finite() {
                return None;
            }
            let weight = weight.clamp(0.0, 1.0);
            let alpha = (tint.a as f32 / 255.0) * weight;
            (alpha > 0.0).then_some((
                [tint.r as f32 / 255.0, tint.g as f32 / 255.0, tint.b as f32 / 255.0],
                alpha,
            ))
        });
        let write_params = |buffer: &wgpu::Buffer, direction: [f32; 2], apply_tint: bool| {
            // `BlurParams` is 32 bytes: `direction: vec2f`, `radius: f32`, `apply_tint: f32`,
            // then (16-byte aligned) `tint_color: vec3f` and `tint_alpha: f32`. Only the
            // vertical pass composites the tint.
            let mut bytes = [0u8; 32];
            bytes[0..4].copy_from_slice(&direction[0].to_ne_bytes());
            bytes[4..8].copy_from_slice(&direction[1].to_ne_bytes());
            bytes[8..12].copy_from_slice(&(radius as f32).to_ne_bytes());
            if let Some((colour, alpha)) = tint_state.filter(|_| apply_tint) {
                bytes[12..16].copy_from_slice(&1.0f32.to_ne_bytes());
                for (channel, value) in colour.iter().enumerate() {
                    bytes[16 + channel * 4..16 + channel * 4 + 4]
                        .copy_from_slice(&value.to_ne_bytes());
                }
                bytes[28..32].copy_from_slice(&alpha.to_ne_bytes());
            }
            self.queue.write_buffer(buffer, 0, &bytes);
        };
        write_params(&targets.horizontal_params, [1.0, 0.0], false);
        write_params(&targets.vertical_params, [0.0, 1.0], true);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("wgpu_blur_encoder"),
        });
        let mut blur_pass = |view: &wgpu::TextureView, group: &wgpu::BindGroup, label: &str| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        };
        blur_pass(&targets.scratch_view, &targets.horizontal, "wgpu_blur_horizontal");
        blur_pass(&targets.source_view, &targets.vertical, "wgpu_blur_vertical");

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Applies one backdrop-blur pass to a frame that has already been composed.
    ///
    /// # Where the passes run
    ///
    /// The convolution and the tint both run on the GPU: the frame is uploaded straight into
    /// the blur source texture, the two [`super::shaders::BLUR_FS`] passes ping-pong between
    /// the cached targets, and the tint — when present — is composited in the second pass's
    /// fragment shader, so the frame never comes back to the CPU between the blur and the
    /// tint (G-220). The tint semantics are the former CPU composite's: the weight scales the
    /// tint's alpha, a non-finite weight is ignored, and a non-positive effective alpha leaves
    /// the frame untouched.
    ///
    /// `radius` is the Gaussian's 2-sigma point, matching [`crate::render::RenderCommand::Blur`].
    ///
    /// Only the final result is read back. An earlier form uploaded and read the backdrop first
    /// and then uploaded it again for the blur, paying two round trips for one result (G-65).
    /// A zero radius keeps the tint: the shader's `radius <= 0` branch is a passthrough of the
    /// input plus the tint composite, matching the software layer composition where the tint
    /// sits on the unblurred backdrop.
    pub fn render_backdrop_blur_rgba8(
        &self,
        width: u32,
        height: u32,
        backdrop: &[u8],
        radius: u32,
        tint: Option<(Rgba8, f32)>,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if backdrop.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                backdrop.len(),
            ));
        }
        self.encode_blur(width, height, backdrop, radius.min(100), tint)?;
        let readback = self.cache.borrow();
        let readback =
            readback.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }
}
