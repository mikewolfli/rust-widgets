// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The 2D and mesh encoders: the packing helpers plus every entry point that records GPU draws
//! through the renderer's cached resources.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{
    bbox_for_circle, blend_mode_from_id, combine, effective_clip, gradient_ramp, path_bounds,
    path_fan_vertices, GradientAxis,
};
use super::super::types::PixelRect;
use super::dispatch::{
    blend_bbox, draws_under_blend, pack_rect_command, rect_command_alpha, SHAPE_ARC_STROKE,
    SHAPE_ARC_WEDGE, SHAPE_CIRCLE_INSIDE, SHAPE_CIRCLE_STROKE, SHAPE_ROUNDED_OUTLINE,
};
use super::frame::WgpuRenderer;
use super::pass::StepRecorder;
use super::resources::{
    padded_row_bytes, readback_frame_bytes, rgba_buffer_len, BlendKind, BlendRun, DrawStep,
    RenderCache,
};
use super::shadow::pack_blurred_shadow;
use super::uploads::PathDraw;

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

/// Packs a row-major [`crate::render::math3d::Mat4`] as the column-major bytes WGSL expects.
///
/// WGSL's `mat4x4f` uniform layout is column-major, while the maths module stores rows to
/// match the affine matrix's natural reading order. Transposing here is what keeps the two
/// conventions from being silently swapped — the failure mode would be a transposed transform
/// that still compiles and still draws *something*.
#[cfg(feature = "3d")]
#[allow(clippy::needless_range_loop)] // row and column index the source row-major matrix.
pub(super) fn mvp_column_major_bytes(m: &crate::render::math3d::Mat4) -> [u8; 64] {
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
    pub(super) fn encode_draw_commands_gpu(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let mut frame = Vec::new();
        self.encode_draw_commands_gpu_into(width, height, expected_len, commands, &mut frame)?;
        Ok(frame)
    }

    /// [`Self::encode_draw_commands_gpu`], reading the frame back into `out`.
    ///
    /// The caller that already owns a reusable buffer (the borrowed entry point) passes it here, so
    /// the readback is the only copy the frame pays for.
    pub(super) fn encode_draw_commands_gpu_into(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        commands: &[WgpuDrawCommand],
        out: &mut Vec<u8>,
    ) -> Result<(), String> {
        self.encode_draw_commands_frame(width, height, commands)?;
        // The frame encoder draws without a readback buffer; this half is what needs one.
        {
            let mut cache = self.cache.borrow_mut();
            cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        }
        let cache = self.cache.borrow();
        let readback =
            cache.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_draw_commands_readback"),
        });
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
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
        self.read_back_into(&readback.buffer, width, height, expected_len, out)
    }

    /// Encodes `commands` on the GPU into the cached render target, without reading it back.
    ///
    /// The frame-returning entry points add a copy of the target into the readback buffer; a caller
    /// that only needs the picture *on* the GPU — to sample it, to hand it to another pipeline, or
    /// to present it — stops here. See [`Self::with_draw_commands_texture`].
    pub(crate) fn encode_draw_commands_frame(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<(), String> {
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
        // The frame's side tables are the packer's own, cleared rather than reallocated (RD-02).
        // Binding them by reference keeps every use below reading the same name it always did; the
        // borrow lives only inside the packing block, because the target/vertex setup below needs
        // `cache` mutably too.
        cache.tables.reset();
        let (
            instance_len,
            circle_vertex_len,
            path_vertex_len,
            shadow_vertex_len,
            uniform_len,
            circle_uniform_len,
            plan_len,
        ) = {
            // One destructure rather than two field borrows: `cache` is a `RefMut`, so every field
            // access goes through one `DerefMut` and the compiler cannot split it by field name.
            let RenderCache { pack, tables, .. } = &mut *cache;
            let image_draws = &mut tables.image_draws;
            let text_draws = &mut tables.text_draws;
            let shape_draws = &mut tables.shape_draws;
            let radial_draws = &mut tables.radial_draws;
            let gradient_draws = &mut tables.gradient_draws;
            let path_draws = &mut tables.path_draws;
            let shadow_draws = &mut tables.shadow_draws;
            // One entry per `DrawStep::BlurRegion`, filled while the plan is packed.
            let frame_blur_specs = &mut tables.frame_blur_specs;
            pack.instances.clear();
            pack.circle_vertices.clear();
            pack.path_vertices.clear();
            pack.shadow_instances.clear();
            pack.blend_runs.clear();
            pack.uniform.resize(stride * clear_count, 0);
            pack.circle_uniforms.resize(stride * 2 * circle_count, 0);
            pack.plan.clear();
            pack.clip_stack.clear();
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
            // The stack lives in the pack scratch: cleared above, capacity kept across frames.
            let clip_stack = &mut pack.clip_stack;
            let mut current_clip: Option<PixelRect> = None;
            for command in commands {
                if let WgpuDrawCommand::SetBlendMode { mode } = command {
                    blend = blend_mode_from_id(*mode);
                    continue;
                }
                // `Clear`, `PushClip` and `PopClip` are not draws: a clear repaints the whole
                // frame through the clear pipeline and ignores the mode on the CPU too.
                //
                // A translucent rectangle-class command is wrapped even under `Normal`: the rect
                // pipeline writes with `REPLACE`, while the reference composites. The offscreen
                // run plus the normal-mode composite is what gives the straight alpha its
                // source-over result; opaque rectangles stay on the direct, cheaper path.
                let translucent_rect = blend == crate::render::BlendMode::Normal
                    && rect_command_alpha(command).is_some_and(|alpha| alpha < 255);
                let wrapped = translucent_rect
                    || (blend != crate::render::BlendMode::Normal && draws_under_blend(command));
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
                    WgpuDrawCommand::BlurRegion { rect, radius, clip } => {
                        let radius = (*radius).min(100);
                        if radius == 0 {
                            continue;
                        }
                        // The region the reference blurs: `rect` intersected with the running clip
                        // and the framebuffer. `box_blur_region` then expands it by the kernel's
                        // reach and clamps to the surface, which is what makes a shadow's soft edge
                        // reach past its body — so the expansion is part of the geometry here too.
                        let Some(region) =
                            effective_clip(framebuffer, *rect, combine(clip, current_clip))
                        else {
                            continue;
                        };
                        if super::resources::frame_blur_pass_widths(radius).is_empty() {
                            continue;
                        }
                        let reach = super::resources::frame_blur_reach(radius) as i64;
                        let left = (region.x as i64 - reach).max(0);
                        let top = (region.y as i64 - reach).max(0);
                        let right = (region.right() as i64 + reach).min(width as i64);
                        let bottom = (region.bottom() as i64 + reach).min(height as i64);
                        if right <= left || bottom <= top {
                            continue;
                        }
                        let layer = PixelRect {
                            x: left as i32,
                            y: top as i32,
                            width: (right - left) as u32,
                            height: (bottom - top) as u32,
                        };
                        // Flush the rectangle run: the blur reads pixels this frame has already
                        // painted, so it has to be its own pass.
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        frame_blur_specs.push((layer, radius));
                        pack.plan.push(DrawStep::BlurRegion(frame_blur_specs.len() - 1));
                    }
                    WgpuDrawCommand::FillRect { .. }
                    | WgpuDrawCommand::StrokeRect { .. }
                    | WgpuDrawCommand::StrokeRectCentered { .. }
                    | WgpuDrawCommand::FillRoundedRect { .. }
                    | WgpuDrawCommand::FillRoundedRectRadii { .. }
                    | WgpuDrawCommand::StrokeRoundedRectInside { .. }
                    | WgpuDrawCommand::BoxShadow { .. }
                    | WgpuDrawCommand::DrawLine { .. }
                    | WgpuDrawCommand::DrawLineCentered { .. }
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
                    WgpuDrawCommand::DrawCircleInside { center, radius, color, width: w, clip } => {
                        if *radius == 0 || *w == 0 {
                            continue;
                        }
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
                        let draw = self.upload_shape_draw(
                            bounds,
                            [center.0 as f32, center.1 as f32, *radius as f32, 0.0],
                            [0.0, 0.0, SHAPE_CIRCLE_INSIDE, *w as f32],
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
                    WgpuDrawCommand::DrawImageScaled {
                        rect,
                        rgba8,
                        image_width,
                        image_height,
                        u_dx,
                        v_dy,
                        clip,
                    } => {
                        let expected = (*image_width as usize) * (*image_height as usize) * 4;
                        if rgba8.len() != expected {
                            return Err(format!(
                                "DrawImageScaled carries {} byte(s) for a {}x{} source, expected \
                                 {expected}",
                                rgba8.len(),
                                image_width,
                                image_height
                            ));
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
                        let draw = self.upload_image_draw_with_sample(
                            *rect,
                            rgba8,
                            *image_width,
                            *image_height,
                            (*u_dx, *v_dy),
                            true,
                        );
                        image_draws.push(draw);
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Image(image_draws.len() - 1));
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
                    WgpuDrawCommand::FillGradient {
                        rect,
                        kind,
                        start,
                        end,
                        ramp,
                        clip,
                        verbatim,
                    } => {
                        if rect.width == 0 || rect.height == 0 {
                            continue;
                        }
                        let expected = 256 * 4;
                        if ramp.len() != expected {
                            return Err(format!(
                                "FillGradient carries {} byte(s) of ramp, expected {expected}",
                                ramp.len()
                            ));
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
                        gradient_draws.push(
                            self.upload_gradient_draw(*rect, *kind, *start, *end, ramp, *verbatim),
                        );
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Gradient(gradient_draws.len() - 1));
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
                    WgpuDrawCommand::DrawTextCoverage { rect, coverage, color, clip } => {
                        let expected = rect.width as usize * rect.height as usize;
                        if rect.width == 0 || rect.height == 0 || coverage.is_empty() {
                            continue;
                        }
                        if coverage.len() != expected {
                            return Err(format!(
                                "DrawTextCoverage carries {} byte(s) for a {}x{} box, expected {expected}",
                                coverage.len(),
                                rect.width,
                                rect.height
                            ));
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
                        text_draws.push(self.upload_coverage_draw(*rect, coverage, *color));
                        pack.plan.push(DrawStep::Scissor(scissor));
                        pack.plan.push(DrawStep::Text(text_draws.len() - 1));
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
                        // The dispatcher accepted this stream, so no command can reach here. An
                        // error is the honest answer now that this half of the encoder produces no
                        // CPU frame of its own: the caller reports it instead of drawing a partial
                        // stream.
                        return Err(
                            "a command reached the GPU encoder that the dispatcher did not accept"
                                .to_string(),
                        );
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
        {
            // Scoped so the mutable borrow of the table ends before `ensure_shadow_targets` takes
            // the cache again.
            let tables = &mut cache.tables;
            tables.shadow_specs.clear();
            tables.shadow_specs.extend(tables.shadow_draws.iter().map(|draw| draw.layer));
        }
        // Moved out for the call and put straight back: `ensure_shadow_targets` takes `&mut self`,
        // so it cannot also borrow a field of `self` as an argument. The move is a pointer copy.
        let shadow_specs = core::mem::take(&mut cache.tables.shadow_specs);
        cache.ensure_shadow_targets(
            &self.device,
            &self.blur_bind_group_layout,
            &self.image_bind_group_layout,
            &self.transformed_bind_group_layout,
            &shadow_specs,
        );
        cache.tables.shadow_specs = shadow_specs;
        let frame_blur_count = cache.tables.frame_blur_specs.len();
        if frame_blur_count > 0 {
            let frame_blur_specs = core::mem::take(&mut cache.tables.frame_blur_specs);
            cache.ensure_frame_blur_targets(
                &self.device,
                &self.blur_bind_group_layout,
                &frame_blur_specs,
            );
            cache.tables.frame_blur_specs = frame_blur_specs;
            if let Some(targets) = cache.frame_blur.as_ref() {
                for (target, (_, radius)) in
                    targets.targets.iter().zip(cache.tables.frame_blur_specs.iter())
                {
                    let widths = super::resources::frame_blur_pass_widths(*radius);
                    // One chunk of two passes per box width: horizontal then vertical. Zipping the
                    // chunks with the widths is what keeps the pass count and the width list from
                    // drifting apart — a radius below two yields fewer than three widths, and the
                    // target is rebuilt for exactly that many passes.
                    for (chunk, width) in target.passes.chunks(2).zip(&widths) {
                        for (index, (_, params)) in chunk.iter().enumerate() {
                            // The shader's parameter is the window's **half** width, the same unit
                            // `blur_row` divides its box width by; passing the width would double
                            // the window and blur the region twice as far.
                            let half = (*width / 2) as u32;
                            let direction = if index % 2 == 0 { [1.0f32, 0.0] } else { [0.0, 1.0] };
                            let bytes = super::resources::shadow_blur_params(direction, half);
                            self.queue.write_buffer(params, 0, &bytes);
                        }
                    }
                }
            }
        }
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
            shadow,
            blend,
            frame_blur,
            pack,
            tables,
            ..
        } = &mut *cache;
        let image_draws = &tables.image_draws;
        let text_draws = &tables.text_draws;
        let shape_draws = &tables.shape_draws;
        let radial_draws = &tables.radial_draws;
        let gradient_draws = &tables.gradient_draws;
        let path_draws = &tables.path_draws;
        let shadow_draws = &tables.shadow_draws;
        let uniform = uniform.as_ref().expect("the colour arena was just ensured");
        let target = target.as_ref().expect("the target was just ensured");
        let stencil = stencil.as_ref().expect("the stencil target was just ensured");
        let vertices = vertices.as_ref().expect("the vertex buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");
        let circle_arena = circle_uniforms.as_ref().expect("the circle arena was just ensured");

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
        self.write_shadow_layer_inputs(shadow_draws, &mut shadow_targets.layers);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_draw_commands_encoder"),
        });
        // Every blurred shadow is rendered first, so the frame's pass only sees finished layers.
        self.encode_shadow_layers(
            &mut encoder,
            shadow_draws,
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
            image_draws,
            text_draws,
            shape_draws,
            radial_draws,
            gradient_draws,
            path_draws,
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
        let frame_blurs: &[super::resources::FrameBlurTarget] =
            frame_blur.as_ref().map_or(&[], |targets| &targets.targets);
        self.record_frame_passes(
            &mut encoder,
            &recorder,
            &pack.plan[..plan_len],
            target,
            &stencil.view,
            blend_pair,
            frame_blurs,
        );

        self.queue.submit(Some(encoder.finish()));
        // Release the draw handles this frame owned now that the pass is submitted, and keep the
        // tables' capacity for the next frame (RD-02). Their previous per-call form dropped the same
        // handles at this exact point.
        cache.tables.release_gpu_owned();
        Ok(())
    }
}
