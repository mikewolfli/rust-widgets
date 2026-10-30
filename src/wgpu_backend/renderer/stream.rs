// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The three ways a caller can ask for a frame: owned pixels, borrowed pixels, or a texture.
//!
//! All three answer the host's quality intent first, then ask the dispatcher whether the pipelines
//! can draw the stream. The two pixel-returning entries then apply the cost gate; the texture entry
//! does not, because its ungated alternative (a CPU raster followed by an upload) has not been
//! measured yet (`docs/plans/blue29.md` §2.4). They differ only in what they do with the result.
//! They live here, apart from [`super::encode`], because that is the seam the file-length rule asks
//! for: `encode` draws, this delivers. Measured cost of the three: `benches/wgpu_bench.rs`.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::rasterize_draw_commands_rgba8;
use super::super::types::PixelRect;
use super::dispatch::{commands_gpu_dispatchable_with, invalid_image_payload};
use super::frame::WgpuRenderer;
use super::policy::{quality_refuses_stream, DeliveryPath};
use super::resources::rgba_buffer_len;
use std::cell::Ref;

impl WgpuRenderer {
    /// Whether the current policy would deliver this frame through the accelerated path.
    ///
    /// This answers the same question the two pixel-returning entry points answer, without
    /// drawing anything: the quality intent is consulted first, then the dispatcher, then the cost
    /// gate. `true` means the frame would be drawn and read back on the device; `false` means the
    /// reference rasteriser would draw it.
    ///
    /// The texture entry point is not covered: it does not apply the cost gate, so its answer can
    /// differ for a stream the dispatcher claims. A benchmark that re-calibrates the gate needs
    /// this decision separately from its timings, because two timing columns can both be the
    /// reference rasteriser and differ only by measurement noise.
    pub fn selects_accelerated_pixels(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> bool {
        self.gpu_delivers_pixels(width, height, commands)
    }

    /// Whether the quality intent and the dispatcher together put this stream on the GPU.
    ///
    /// The intent is answered first (`docs/plans/blue29.md` §2.1): a `ByteExact` stream the
    /// pipelines cannot reproduce is refused before the dispatcher is consulted, so the answer does
    /// not depend on which pipelines happen to be wired.
    fn gpu_claims_stream(&self, commands: &[WgpuDrawCommand]) -> bool {
        !quality_refuses_stream(self.quality, commands)
            && commands_gpu_dispatchable_with(commands, self.blend_modes_on_gpu)
    }

    /// Whether a claimed stream should also be delivered through the readback path.
    ///
    /// The cost gate only sees streams the dispatcher has already claimed; a refused stream never
    /// reaches it (`docs/plans/blue29.md` §2.1).
    fn gpu_delivers_pixels(&self, width: u32, height: u32, commands: &[WgpuDrawCommand]) -> bool {
        if !self.gpu_claims_stream(commands) {
            return false;
        }
        self.cost_gate.choose(PixelRect { x: 0, y: 0, width, height }, commands)
            == DeliveryPath::Gpu
    }

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
    ///
    /// A claimed stream is still gated on cost: see [`Self::set_cost_gate_enabled`]. The gate
    /// never changes the picture, only which existing implementation draws it.
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
        if self.gpu_delivers_pixels(width, height, commands) {
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
        if self.gpu_delivers_pixels(width, height, commands) {
            // The readback writes the frame into this buffer directly: allocating a frame and then
            // copying it in again cost a second 3.7 MB memcpy per 720p frame, which the benchmark
            // (`benches/wgpu_bench.rs`) measures at 10-30% of the call.
            {
                let mut bytes = self.commands_frame.bytes.borrow_mut();
                self.encode_draw_commands_gpu_into(
                    width,
                    height,
                    expected_len,
                    commands,
                    &mut bytes,
                )?;
            }
            return Ok(Ref::map(self.commands_frame.bytes.borrow(), |bytes| {
                &bytes[..expected_len]
            }));
        }
        self.commands_frame.rasterize(width, height, commands)
    }

    /// Renders `commands` and hands the picture to `f` **as a texture**, without reading it back.
    ///
    /// This is the entry point for a host that consumes the frame on the GPU — sampling it into its
    /// own surface, feeding it to another pipeline, presenting it — and it exists because the
    /// measurement says the readback is the dominant cost of the pixel-returning calls
    /// (`benches/wgpu_bench.rs`): a 720p `clear` costs the CPU rasteriser 0.29 ms and the GPU path
    /// 1.07 ms, and the difference is almost entirely "copy the frame back and wait for it". With
    /// the readback gone, the GPU keeps its throughput advantage instead of donating it to the copy.
    ///
    /// The quality intent and the dispatcher decide this stream's path exactly as they do for the
    /// frame-returning entry points; the cost gate is not applied here (see the module docs). A
    /// stream the dispatcher refuses is rasterised on the CPU and uploaded into the same texture, so
    /// the caller sees one picture either way — the CPU rasteriser's own bytes, not a GPU
    /// approximation of them.
    ///
    /// `f` runs with the renderer's cache borrowed, so it must not call back into the renderer.
    pub fn with_draw_commands_texture<R>(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
        f: impl FnOnce(&wgpu::TextureView) -> R,
    ) -> Result<R, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if let Some(error) = invalid_image_payload(commands) {
            return Err(error);
        }
        if self.gpu_claims_stream(commands) {
            self.encode_draw_commands_frame(width, height, commands)?;
        } else {
            let frame = self.commands_frame.rasterize(width, height, commands)?;
            self.upload_frame_into_target(width, height, &frame)?;
        }
        let cache = self.cache.borrow();
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        debug_assert_eq!(target.width as usize * target.height as usize * 4, expected_len);
        Ok(f(&target.view))
    }

    /// Uploads a CPU-rasterised frame into the cached render target.
    ///
    /// The target carries `COPY_DST` for exactly this: a stream the GPU cannot draw still has to
    /// reach `with_draw_commands_texture`'s caller through the same handle as a GPU one.
    fn upload_frame_into_target(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), String> {
        let mut cache = self.cache.borrow_mut();
        cache.ensure_target(&self.device, width, height);
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        Ok(())
    }
}
