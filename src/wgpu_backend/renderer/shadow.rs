// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The blurred-shadow pass: what a `BoxShadow` with a radius becomes on the GPU.
//!
//! A blurred shadow is the one 2D command whose GPU form is more than one draw. Its body is painted
//! into an offscreen layer, the layer is filtered along both axes, and the result is composited
//! where the command sits in the stream. Those three steps are recorded *before* the frame's pass,
//! so the main pass only sees a finished texture — the same shape the software rasteriser has,
//! where the shadow is rendered into a private layer and composited back (N-S-63).
//!
//! Split out of [`super::encode`] so that file stays about the command stream: this module owns the
//! layer's geometry and passes, and nothing here decides which commands the GPU claims.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{combine, effective_clip};
use super::super::types::PixelRect;
use super::dispatch::push_instance;
use super::encode::{ortho_mvp_bytes, INSTANCE_BYTES};
use super::frame::WgpuRenderer;
use super::resources::{shadow_blur_params, ShadowDraw, ShadowLayer};

/// Packs one blurred shadow and returns its layer, or `None` when it draws nothing.
///
/// `body` is the shadow body — the rectangle translated by the offset — narrowed by the framebuffer
/// and the running clip exactly as the software rasteriser narrows it. The layer covers that body
/// grown by the blur radius and clamped to the frame, which is the CPU's own layer extent: the
/// filter's window is then bounded by the same edges on both paths.
///
/// The returned rectangle is the **paint region**: the caller's clip (or the whole framebuffer), so
/// the blur may feather outside the body but never outside what the caller allowed.
///
/// The body's instance record is appended to `instances` in **layer-local** pixels; the layer's own
/// matrix, written by the caller, is what maps them into that layer.
pub(super) fn pack_blurred_shadow(
    command: &WgpuDrawCommand,
    framebuffer: PixelRect,
    stream_clip: Option<PixelRect>,
    instances: &mut Vec<u8>,
) -> Option<(ShadowDraw, PixelRect)> {
    let WgpuDrawCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, clip } = command
    else {
        return None;
    };
    if rect.width == 0 || rect.height == 0 {
        return None;
    }
    let shadow = PixelRect {
        x: rect.x.saturating_add(*offset_x),
        y: rect.y.saturating_add(*offset_y),
        width: rect.width,
        height: rect.height,
    };
    let body = effective_clip(framebuffer, shadow, combine(clip, stream_clip))?;
    let paint_region = match combine(clip, stream_clip) {
        Some(region) => region.intersect(framebuffer)?,
        None => framebuffer,
    };
    let radius = *blur_radius as i64;
    let left = (body.x as i64 - radius).max(0);
    let top = (body.y as i64 - radius).max(0);
    let right = (body.right() as i64 + radius).clamp(0, framebuffer.width as i64);
    let bottom = (body.bottom() as i64 + radius).clamp(0, framebuffer.height as i64);
    if right <= left || bottom <= top {
        return None;
    }
    let layer = PixelRect {
        x: left as i32,
        y: top as i32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    };
    let local = PixelRect {
        x: body.x - layer.x,
        y: body.y - layer.y,
        width: body.width,
        height: body.height,
    };
    let body_instance = instances.len() / INSTANCE_BYTES;
    push_instance(instances, local, *color);
    Some((ShadowDraw { layer, body_instance, blur_radius: *blur_radius }, paint_region))
}

impl WgpuRenderer {
    /// Writes the inputs every layer needs before its passes run: the layer's own pixel-to-clip
    /// matrix, the two directional radii and the composite params that place the layer in the frame.
    ///
    /// These are per-layer buffers rather than one shared arena because a pass cannot read a value
    /// another pass already overwrote: the writes are queued per submission, not per draw.
    pub(super) fn write_shadow_layer_inputs(
        &self,
        draws: &[ShadowDraw],
        layers: &mut [ShadowLayer],
    ) {
        for (layer, draw) in layers.iter_mut().zip(draws) {
            self.queue.write_buffer(
                &layer.body_uniform.buffer,
                0,
                &ortho_mvp_bytes(layer.width, layer.height),
            );
            self.queue.write_buffer(
                &layer.horizontal_params,
                0,
                &shadow_blur_params([1.0, 0.0], draw.blur_radius),
            );
            self.queue.write_buffer(
                &layer.vertical_params,
                0,
                &shadow_blur_params([0.0, 1.0], draw.blur_radius),
            );
            let params = [
                draw.layer.x as f32,
                draw.layer.y as f32,
                draw.layer.width as f32,
                draw.layer.height as f32,
                draw.layer.width as f32,
                draw.layer.height as f32,
                0.0,
                0.0,
            ];
            let mut bytes = [0u8; 32];
            for (index, value) in params.iter().enumerate() {
                bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            self.queue.write_buffer(&layer.composite_params, 0, &bytes);
        }
    }

    /// Records every shadow layer: body, horizontal blur, vertical blur — in that order per shadow.
    ///
    /// The passes run before the frame's own pass, so each layer is complete by the time the frame
    /// composites it. `shadow_vertex_offset` is where the packed body instances begin in the shared
    /// vertex buffer; each layer binds its own one-instance slice of it.
    pub(super) fn encode_shadow_layers(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        draws: &[ShadowDraw],
        layers: &[ShadowLayer],
        vertices: &wgpu::Buffer,
        shadow_vertex_offset: u64,
    ) {
        for (draw, layer) in draws.iter().zip(layers) {
            let body_start = shadow_vertex_offset + (draw.body_instance * INSTANCE_BYTES) as u64;
            {
                let mut layer_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gpu_shadow_body_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &layer.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    // No stencil: path fills pass through the frame's attachment, and WebGPU
                    // requires every attachment in a pass to have the same size, which this layer
                    // does not share with the frame.
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                layer_pass.set_pipeline(&self.shadow_body_pipeline);
                layer_pass.set_bind_group(0, &layer.body_uniform.bind_group, &[]);
                layer_pass.set_vertex_buffer(
                    0,
                    vertices.slice(body_start..body_start + INSTANCE_BYTES as u64),
                );
                layer_pass.draw(0..6, 0..1);
            }
            self.encode_blur_direction(
                encoder,
                "gpu_shadow_blur_horizontal",
                &layer.scratch_view,
                &layer.horizontal,
            );
            self.encode_blur_direction(
                encoder,
                "gpu_shadow_blur_vertical",
                &layer.view,
                &layer.vertical,
            );
        }
    }

    /// Records one directional pass of the shadow's separable box filter.
    fn encode_blur_direction(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        target: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        pass.set_pipeline(&self.shadow_blur_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
