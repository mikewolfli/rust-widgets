// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GPU blur encoding: the separable two-pass convolution and its tint composite.
//!
//! Split out of `encode.rs` so that file stays under the 2000-line structure limit (principle
//! #9). The seam is the blur pipeline itself: nothing here packs draw commands, and nothing in
//! `encode.rs` records a blur pass. `render_blur_rgba8` is the public read-back entry point;
//! `render_backdrop_blur_rgba8` is the layer-composition entry point with the same two passes.

use super::super::types::Rgba8;
use super::frame::WgpuRenderer;
use super::resources::{padded_row_bytes, readback_frame_bytes, rgba_buffer_len};

impl WgpuRenderer {
    /// Applies a separable Gaussian blur to an RGBA8 image on the GPU.
    ///
    /// # Why two passes
    ///
    /// A Gaussian kernel is separable: blurring rows and then columns costs `O(radius)` per pixel
    /// instead of `O(radius²)`. Each pass is one full-screen draw through [`super::super::shaders::BLUR_FS`],
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
    /// ([`super::super::shaders::BLUR_FS`]) instead of a read-back-to-CPU shading step, so the frame
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
    /// the blur source texture, the two [`super::super::shaders::BLUR_FS`] passes ping-pong between
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
