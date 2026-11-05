// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The renderer itself: pipeline construction, the clear path, the readback plumbing and the
//! [`GpuRenderer`] implementation.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::rasterize_draw_commands_rgba8;
use super::super::shaders::ShaderModule;
use super::policy::{CostGate, RenderQuality};
use super::resources::{
    padded_row_bytes, readback_frame_bytes, rgba8_from_unit, rgba_buffer_len, CpuFrame,
    ParamsGroupCache, ParamsGroups, ParamsScratch, RenderCache, TextureCache, GPU_READBACK_TIMEOUT,
};
use crate::render::gpu::{GpuCapability, GpuRenderer};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::sync::mpsc;

/// Lightweight GPU renderer context backed by `wgpu`.
pub struct WgpuRenderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) clear_pipeline: wgpu::RenderPipeline,
    /// The 2D per-rectangle pipeline. Used only for the single-rectangle case; a longer run goes
    /// through the instanced quad pipeline instead (G-64).
    pub(crate) rect_pipeline: wgpu::RenderPipeline,
    /// The separable-blur pipeline: one full-screen triangle, no vertex buffer, two directional
    /// passes. A 2D effect, so it is compiled in every build rather than behind `3d`.
    pub(crate) blur_pipeline: wgpu::RenderPipeline,
    /// The shadow layer's box-blur pipeline: the backdrop blur's layout and vertex stage, with the
    /// clamped moving-average fragment stage a `BoxShadow` needs.
    pub(crate) shadow_blur_pipeline: wgpu::RenderPipeline,
    /// The instanced quad pipeline without a stencil state, used to paint a shadow's body into its
    /// own offscreen layer.
    pub(crate) shadow_body_pipeline: wgpu::RenderPipeline,
    /// The composite pass of a GPU blend run: the run's layer blended with the frame's backdrop.
    pub(crate) blend_composite_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the composite pass: the mode uniform plus the layer and backdrop
    /// textures.
    pub(crate) blend_bind_group_layout: wgpu::BindGroupLayout,
    /// Whether streams that leave `Normal` are drawn on the GPU. Off by default — see
    /// [`Self::set_blend_modes_on_gpu`].
    pub(crate) blend_modes_on_gpu: bool,
    /// The host's quality intent. See [`Self::set_render_quality`].
    pub(crate) quality: RenderQuality,
    /// The pixel-path cost gate. See [`Self::set_cost_gate_enabled`].
    pub(super) cost_gate: CostGate,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout of the blur pass: a non-filterable input texture and a 32-byte
    /// uniform (`direction`, `radius`, and the G-220 tint block the vertical pass composites).
    pub(crate) blur_bind_group_layout: wgpu::BindGroupLayout,
    /// Instanced quad pipeline. It renders a whole run of rectangles (and, with `3d`, arbitrary
    /// transformed quads) in `MAX_INSTANCES_PER_DRAW`-sized draws (G-64).
    pub(crate) transformed_pipeline: wgpu::RenderPipeline,
    /// The vertex-stage layout that carries the mvp matrix (group 0 of the instanced pipeline).
    pub(crate) transformed_bind_group_layout: wgpu::BindGroupLayout,
    /// The SDF circle pipeline: one bounding quad per circle, shaded by a distance field in
    /// the fragment stage (G-219).
    pub(crate) circle_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the SDF circle pipeline: two dynamic-offset uniforms (the fill
    /// colour and the radius/centre parameters) at group 0; the mvp matrix rides group 1.
    pub(crate) circle_bind_group_layout: wgpu::BindGroupLayout,
    /// The image pipeline: one axis-aligned quad per draw, shaded by a single texel fetch at the
    /// source coordinate [`super::shaders::IMAGE_FRAG`] derives from the draw's own uniform.
    /// Compositing is straight-alpha source-over, matching the CPU rasteriser's blend rule.
    pub(crate) image_pipeline: wgpu::RenderPipeline,
    /// The scaled-image pipeline: the same quad, bind group and blend state as the image pipeline,
    /// shaded by four texels and the reference's bilinear weights instead of one.
    pub(crate) image_scaled_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the image pipeline: the draw's params uniform and its source texture
    /// at group 0; the mvp matrix rides group 1, as in the SDF circle pipeline.
    pub(crate) image_bind_group_layout: wgpu::BindGroupLayout,
    /// The text pipeline: one quad per text box, shaded by the box's coverage texture multiplied
    /// by the text colour. Compositing is straight-alpha source-over, like the image pipeline.
    pub(crate) text_pipeline: wgpu::RenderPipeline,
    /// The same text pipeline writing with `REPLACE`, used inside a blend run's offscreen layer:
    /// there the coverage has to survive as the pixel's alpha, not be composited away.
    pub(crate) text_layer_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the text pipeline: the box's params uniform and its coverage texture
    /// at group 0; the mvp matrix rides group 1.
    pub(crate) text_bind_group_layout: wgpu::BindGroupLayout,
    /// The ring / wedge / rounded-outline pipeline: one SDF per draw, selected by a mode uniform.
    pub(crate) shape_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the shape pipeline: the draw's 64-byte params uniform at group 0; the
    /// mvp matrix rides group 1.
    pub(crate) shape_bind_group_layout: wgpu::BindGroupLayout,
    /// The radial-gradient pipeline: one quad, shaded by a distance-to-stop lookup.
    pub(crate) radial_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the radial-gradient pipeline: the params uniform and the stop texture
    /// at group 0; the mvp matrix rides group 1.
    pub(crate) radial_bind_group_layout: wgpu::BindGroupLayout,
    /// The general gradient pipeline: one quad shaded by the reference's own ramp, along an
    /// arbitrary linear, radial or conic geometry.
    pub(crate) gradient_pipeline: wgpu::RenderPipeline,
    /// The same pipeline with `REPLACE` blending: the standalone conic command stores its ramp
    /// colour verbatim, so it must not be composited.
    pub(crate) gradient_verbatim_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the general gradient pipeline: the 64-byte params uniform and the
    /// 256-entry ramp texture at group 0; the mvp matrix rides group 1.
    pub(crate) gradient_bind_group_layout: wgpu::BindGroupLayout,
    /// Path fill, pass 1: fan triangles into the stencil buffer, even-odd rule.
    pub(crate) path_stencil_evenodd_pipeline: wgpu::RenderPipeline,
    /// Path fill, pass 1: fan triangles into the stencil buffer, non-zero winding rule.
    pub(crate) path_stencil_nonzero_pipeline: wgpu::RenderPipeline,
    /// Path fill, pass 2: the covering quad, kept where the stencil is non-zero.
    pub(crate) path_cover_pipeline: wgpu::RenderPipeline,
    /// Path fill, pass 3: redraws the same fan with a `Zero` store, resetting the stencil for the
    /// next path in the same render pass.
    pub(crate) path_stencil_clear_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the path cover pass: the bounding box and fill colour at group 0.
    pub(crate) path_cover_bind_group_layout: wgpu::BindGroupLayout,
    /// Depth-tested triangle pipeline for mesh draws (`3d` only).
    #[cfg(feature = "3d")]
    pub(crate) mesh_pipeline: wgpu::RenderPipeline,
    /// Cached shader module pipeline references (BLUE11 R5.2).
    pub(crate) shader_cache: HashMap<String, wgpu::ShaderModule>,
    /// Device resources reused across calls (REND-02).
    pub(crate) cache: RefCell<RenderCache>,
    /// Content-keyed uploads of image / ramp / stop-table pixels (RD2-01).
    ///
    /// Separate from [`Self::cache`] because the packer holds `cache` mutably for the whole
    /// command-stream build, and an image draw needs to look its texture up in the middle of
    /// that build. Folding it into `RenderCache` made the lookup re-borrow the very cell the
    /// caller already held mutably, which is a panic on the first cached image.
    pub(crate) texture_cache: RefCell<TextureCache>,
    /// Grow-only uniform buffer the per-draw params of one frame are carved out of (RD2-01).
    ///
    /// Separate from [`Self::cache`] for the same reason as `texture_cache`: the packer holds
    /// `cache` mutably while it uploads draws, so the arena cannot live inside it.
    pub(crate) params_scratch: RefCell<ParamsScratch>,
    /// The dynamic-offset bind groups built over [`Self::params_scratch`], rebuilt when the
    /// arena buffer grows. Kept next to the arena because both describe the same allocation.
    pub(crate) params_groups: RefCell<Option<ParamsGroups>>,
    /// Textured families' params bind groups, reused while the texture repeats (RD2-01).
    pub(crate) params_group_cache: RefCell<ParamsGroupCache>,
    /// Reusable CPU frame for whole-frame clears (G-221).
    pub(crate) clear_frame: CpuFrame,
    /// Reusable CPU frame for command streams (G-221).
    pub(crate) commands_frame: CpuFrame,
    /// What `wgpu` reported for the selected adapter, kept so [`GpuRenderer::vendor_info`] can
    /// answer with the real device name instead of a constant (G-232).
    pub(crate) adapter_info: wgpu::AdapterInfo,
    /// Hardware-adaptive GPU management for this device (issue-20261104 RD-04).
    ///
    /// Built from the adapter facts `wgpu` already reported for the device above, so it runs **no**
    /// second adapter-selection pass (`GpuManager::from_adapter_info`, not `GpuManager::new`). What
    /// it contributes to the live path:
    ///
    /// * `begin_frame`/`end_frame` bracket every offscreen frame, which is what gives
    ///   `gpu::performance` real frame durations, the trap detector real low-FPS input and the
    ///   quality tracker real streaks — before this they were state nothing ever fed.
    /// * `begin_frame` advances the staging pool's ring cursor (`GpuStagingBufferPool::next_frame`),
    ///   so that pool is on the frame path rather than behind it.
    /// * Its operation mode, warnings, pool stats and quality recommendations are readable through
    ///   the renderer's own accessors, so a host can act on the same facts the library recorded
    ///   instead of probing the GPU a second time.
    pub(crate) gpu: crate::gpu::GpuManager,
}

/// The depth-stencil state every 2D pipeline in this renderer declares.
///
/// The 2D passes carry a `Stencil8` attachment for path fills, and WebGPU requires every pipeline
/// used in a pass to declare a state that matches the attachment's format — even a pipeline that
/// never touches it. This state writes no depth, compares nothing and leaves the stencil value
/// alone, so a pipeline declared with it behaves exactly as it did before the attachment existed.
pub(super) fn passive_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Stencil8,
        // A stencil-only format has no depth aspect, so both depth fields must be absent.
        depth_write_enabled: None,
        depth_compare: None,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState::IGNORE,
            back: wgpu::StencilFaceState::IGNORE,
            read_mask: 0,
            write_mask: 0,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

impl WgpuRenderer {
    /// Create a new renderer by requesting a default GPU adapter and logical device.
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::new_async())
    }
    async fn new_async() -> Result<Self, String> {
        // Build the instance from the whole degradation ladder so a host with no
        // primary driver can still reach the GL (OpenGL ES) rung, and report
        // which tier we landed on rather than failing outright.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: crate::gpu::backend_ladder::instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let (adapter, tier) = crate::gpu::backend_ladder::select_adapter_with_gl_fallback(
            &instance,
            wgpu::PowerPreference::HighPerformance,
            None,
        )
        .await
        .ok_or_else(|| {
            // Distinguish "this host has no GPU backend at all" from "you pinned
            // a backend this host cannot provide". The second case is a user
            // error worth naming, not a mystery to debug.
            match crate::gpu::backend_ladder::backends_from_env() {
                Some(pinned) => format!(
                    "wgpu adapter request failed: WGPU_BACKEND pinned {pinned:?}, \
                     which this host does not provide"
                ),
                None => "wgpu adapter request failed on every backend tier".to_string(),
            }
        })?;

        let adapter_info = adapter.get_info();
        if tier.is_degraded() {
            // Surface the degradation once, at start-up, so a slow session is
            // explainable instead of mysterious.
            log::warn!(
                "[wgpu] using degraded backend tier: {} ({:?}, adapter '{}')",
                tier.label(),
                adapter_info.backend,
                adapter_info.name,
            );
        } else {
            log::info!(
                "[wgpu] using {} backend ({:?}, adapter '{}')",
                tier.label(),
                adapter_info.backend,
                adapter_info.name,
            );
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rw_wgpu_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| {
                format!(
                    "the GPU device could not be requested: {error}; the adapter may have been \
                     lost or the requested limits unsupported"
                )
            })?;
        Self::from_device(device, queue, adapter_info)
    }

    /// Returns whether non-`Normal` blend modes are drawn on the GPU.
    ///
    /// See [`Self::set_blend_modes_on_gpu`] for what the switch means.
    pub fn blend_modes_on_gpu(&self) -> bool {
        self.blend_modes_on_gpu
    }

    /// Chooses whether a stream that leaves `Normal` blending is drawn on the GPU.
    ///
    /// Off by default, because the GPU path costs a frame copy per blended command: WebGPU cannot
    /// read the destination from a fragment shader, so a `Multiply`/`Overlay` command is rendered
    /// into an offscreen layer and composited against a snapshot of the frame that was taken just
    /// before it. The result is the software rasteriser's own formula (see
    /// [`super::shaders::BLEND_COMPOSITE_FS`]) applied once per command, which is why it is exact
    /// for opaque destinations and within one level on a translucent one.
    ///
    /// While it is off, a stream containing `SetBlendMode` with any mode but `Normal` is drawn by
    /// the CPU rasteriser as a whole, exactly as before.
    ///
    /// ```text
    /// let mut renderer = WgpuRenderer::new()?;
    /// renderer.set_blend_modes_on_gpu(true);
    /// ```
    pub fn set_blend_modes_on_gpu(&mut self, enabled: bool) {
        self.blend_modes_on_gpu = enabled;
    }

    /// Returns the host's quality intent.
    ///
    /// See [`Self::set_render_quality`] for what the two values mean.
    pub fn render_quality(&self) -> RenderQuality {
        self.quality
    }

    /// States what the host needs from the picture.
    ///
    /// [`RenderQuality::ByteExact`] keeps the streams that cannot be reproduced byte for byte on
    /// the reference rasteriser: a stream containing an edge-feathered shape (circle fill, ring,
    /// arc, rounded outline), and a stream drawn onto a transparent target with no whole-frame
    /// opaque clear. Every other stream is unaffected. [`RenderQuality::Antialiased`] is the
    /// default, because it preserves the picture this renderer produced before the intent existed.
    ///
    /// The intent only describes the result; it never names the mechanism that produces it.
    pub fn set_render_quality(&mut self, quality: RenderQuality) {
        self.quality = quality;
    }

    /// Returns whether the cost gate may move a pixel-returning stream off the GPU path.
    pub fn cost_gate_enabled(&self) -> bool {
        self.cost_gate.enabled()
    }

    /// Arms or disarms the pixel-path cost gate.
    ///
    /// Armed (the default), a stream the dispatcher claims is still drawn on the reference
    /// rasteriser unless its largest qualifying shape reaches the whitelist's area threshold. This
    /// is what stops a small frame from paying the GPU's delivery cost for no gain. Disarming
    /// restores the pre-gate behaviour, which is what the benchmark uses to measure the raw GPU
    /// column and what a host uses to re-calibrate the threshold on its own adapter.
    ///
    /// The texture entry point is not gated in either case: its alternative form is a CPU raster
    /// followed by an upload. B29.04 measured that branch, but the first version still does not
    /// gate the texture path (`docs/plans/blue29.md` §2.4).
    pub fn set_cost_gate_enabled(&mut self, enabled: bool) {
        self.cost_gate.set_enabled(enabled);
    }

    // ── Hardware-adaptive facts (issue-20261104 RD-04) ──────────────────────
    //
    // These read the `GpuManager` this renderer owns — the same object `begin_frame`/`end_frame`
    // drive on every frame. They exist so a host can act on the facts the library already
    // measured instead of probing the adapter a second time (principle #101: one answer per
    // question).
    //
    // The recommended-quality pair is deliberately **not** applied automatically. This renderer
    // promises a specific picture for a specific stream (that is what `RenderQuality` is about),
    // and silently moving streams to the CPU because a frame was slow would break that promise
    // while looking like a stall. A host that wants the adaptive behaviour calls
    // `set_render_quality`/`set_cost_gate_enabled` with its own policy.

    /// The adapter facts this renderer's manager was built from.
    pub fn gpu_adapter_info(&self) -> &crate::gpu::AdapterInfo {
        self.gpu.adapter_info()
    }

    /// Whether this renderer is on a hardware device or on the CPU fallback.
    pub fn gpu_operation_mode(&self) -> crate::gpu::GpuOperationMode {
        self.gpu.operation_mode()
    }

    /// The most recent frame's performance sample, once a frame has been drawn.
    pub fn gpu_performance_stats(&self) -> Option<crate::gpu::performance::PerformanceStats> {
        self.gpu.performance_stats()
    }

    /// Construction-time advisories (degraded backend tier, browser-forced integrated GPU).
    pub fn gpu_warnings(&self) -> Vec<String> {
        self.gpu.warnings()
    }

    /// The actions the trap detector recommends for the current streak, most urgent first.
    pub fn gpu_recommended_actions(&self) -> Vec<crate::gpu::GpuManagerAction> {
        self.gpu.recommended_actions()
    }

    /// Whether the recorded frames say quality should come down.
    pub fn should_degrade_quality(&self) -> bool {
        self.gpu.should_degrade_quality()
    }

    /// Whether the recorded frames say quality can come back up.
    pub fn should_upgrade_quality(&self) -> bool {
        self.gpu.should_upgrade_quality()
    }

    /// The texture the last command stream was drawn into, as a fresh view.
    ///
    /// Gated on `raw-window-handle` because its only consumer is the presentation target
    /// (`render::present::FramePresenter`), which does not exist without it.
    ///
    /// `None` before anything has been drawn. The presentation target uses this to put a bridged
    /// `Smooth` frame on the window: the stream is drawn by these pipelines (which need their own
    /// colour and stencil attachments), and the resulting texture is then copied onto the surface
    /// with a single full-screen pass — no readback, and the same device throughout.
    #[cfg(feature = "raw-window-handle")]
    pub(crate) fn encoded_target_view(&self) -> Option<wgpu::TextureView> {
        // The cache already holds this target's view: creating a second one per presented frame was
        // a `wgpu::TextureView` allocation for a descriptor identical to the cached one (RD-02).
        let cache = self.cache.borrow();
        cache.target.as_ref().map(|target| target.view.clone())
    }

    /// Blocks until every submission already handed to the device has completed.
    ///
    /// The public API exposes no queue or fence, and this method deliberately does not: it answers
    /// the caller's question ("is the device idle?") rather than exposing the mechanism. The wait
    /// is bounded by `GPU_READBACK_TIMEOUT`, the same budget the readback uses, so a lost device
    /// reports an error instead of hanging the process.
    pub fn wait_idle(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(GPU_READBACK_TIMEOUT),
            })
            .map(|_| ())
            .map_err(|error| {
                format!("GPU did not reach idle within {GPU_READBACK_TIMEOUT:?}: {error}")
            })
    }

    /// Render one offscreen RGBA frame by clearing a texture with the given color and read pixels back.
    ///
    /// A solid clear needs no GPU work, so this fills the buffer on the CPU and returns it
    /// directly. The previous implementation routed the same fill through a texture upload and a
    /// blocking readback, which could only reproduce the bytes it was given (G-66). A caller
    /// that only uploads or reads the frame can use [`Self::render_clear_cached_rgba8`] instead
    /// of taking this owned copy.
    pub fn render_clear_rgba8(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        rasterize_draw_commands_rgba8(
            width,
            height,
            &[WgpuDrawCommand::Clear { color: rgba8_from_unit(color) }],
        )
    }

    /// Rasterises a clear frame into the renderer's reusable buffer and lends it back (G-221).
    ///
    /// The zero-copy counterpart of [`Self::render_clear_rgba8`]: a caller that uploads or
    /// inspects the frame can use the borrow instead of taking an owned copy each frame. The
    /// buffer grows only and the rasteriser clears it, so every call sees a fresh frame.
    pub fn render_clear_cached_rgba8(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Ref<'_, [u8]>, String> {
        let clear = [WgpuDrawCommand::Clear { color: rgba8_from_unit(color) }];
        self.clear_frame.rasterize(width, height, &clear)
    }

    /// Render a clear frame using the GPU pipeline with a WGSL shader.
    /// Uses a full-screen triangle and a uniform buffer for the clear color.
    pub fn render_clear_gpu(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before touching the GPU: a zero-sized texture is
        // not a valid render target and an overflowing byte count must not be
        // truncated (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        let mut colour_bytes = [0u8; 16];
        for (channel, value) in color.iter().enumerate() {
            let clamped = value.clamp(0.0, 1.0) as f32;
            colour_bytes[channel * 4..channel * 4 + 4].copy_from_slice(&clamped.to_le_bytes());
        }
        let frame_bytes = readback_frame_bytes(width, height)?;

        // One cached uniform arena, target and readback buffer for every call, rather than a new
        // set of GPU resources per call (REND-02).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, 1)?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_stencil(&self.device, width, height);
        cache.ensure_readback(&self.device, frame_bytes);
        let uniform = cache.uniform.as_ref().expect("the arena was just ensured");
        let target = cache.target.as_ref().expect("the target was just ensured");
        let stencil = cache.stencil.as_ref().expect("the stencil target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&uniform.buffer, 0, &colour_bytes);

        // Encode render pass with full-screen triangle
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_clear_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                // The clear pipeline declares the 2D passes' `Stencil8` state, so this pass must
                // carry the attachment too; it is unused and discarded.
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
            rp.set_pipeline(&self.clear_pipeline);
            rp.set_bind_group(0, &uniform.bind_group, &[0]);
            rp.draw(0..3, 0..1); // 3 vertices for full-screen triangle
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

    /// Copies the cached render target into a tight RGBA8 buffer.
    ///
    /// The target must already hold the picture; this helper only arranges the copy and the blocking
    /// read. Kept separate so an entry point that renders through a new pipeline does not repeat the
    /// row-padding arithmetic.
    #[cfg(feature = "3d")]
    pub(crate) fn read_target(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        label: &str,
    ) -> Result<Vec<u8>, String> {
        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        let readback =
            cache.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
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

    /// Maps a readback buffer, copies its rows into a caller-provided RGBA8 image and unmaps it.
    ///
    /// The output goes through `out` rather than being allocated here (G-217): a caller that
    /// renders repeatedly can carry the vector across calls and only ever pay for growth, not
    /// a fresh allocation per frame. The entry points that must hand an owned frame back pass
    /// a fresh vector — the mapping, polling and row-copying work is identical either way.
    /// `out` is grown to `expected_len` and every row is overwritten, so no stale byte from a
    /// previous frame can leak through.
    ///
    /// The buffer belongs to [`RenderCache`], so it must be unmapped before it can be reused:
    /// leaving it mapped made the next `map_async` fail silently, which is why this is one
    /// helper rather than a copy in every entry point.
    ///
    /// The wait is bounded by [`GPU_READBACK_TIMEOUT`]; on expiry the pending mapping is
    /// cancelled with `unmap` and an error is returned, so a lost device reports a failure
    /// instead of hanging the process (G-63).
    pub(crate) fn read_back_into(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        expected_len: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), String> {
        let unpadded = width
            .checked_mul(4)
            .ok_or_else(|| format!("row byte width for {width} pixels overflows u32"))?
            as usize;
        let padded = padded_row_bytes(width)? as usize;
        let buffer_slice = buffer.slice(..(padded * height as usize) as u64);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(
            wgpu::MapMode::Read,
            move |result: Result<(), wgpu::BufferAsyncError>| {
                let _ = sender.send(result);
            },
        );
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_READBACK_TIMEOUT),
        }) {
            // Cancel the still-pending mapping so the cached buffer is usable next call.
            buffer.unmap();
            return Err(format!(
                "GPU readback did not complete within {GPU_READBACK_TIMEOUT:?}: {error}"
            ));
        }
        let map_result = match receiver.recv_timeout(GPU_READBACK_TIMEOUT) {
            Ok(result) => result,
            Err(error) => {
                buffer.unmap();
                return Err(format!(
                    "GPU map callback did not arrive within {GPU_READBACK_TIMEOUT:?}: {error}"
                ));
            }
        };
        if let Err(error) = map_result {
            buffer.unmap();
            return Err(format!("GPU buffer could not be mapped for CPU readback: {error:?}"));
        }
        let mapped = match buffer_slice.get_mapped_range() {
            Ok(mapped) => mapped,
            Err(error) => {
                buffer.unmap();
                return Err(format!("mapped GPU buffer range could not be read back: {error:?}"));
            }
        };

        // Copy row by row, stripping padding.
        out.resize(expected_len, 0);
        for row in 0..height as usize {
            let src_start = row * padded;
            let dst_start = row * unpadded;
            out[dst_start..dst_start + unpadded]
                .copy_from_slice(&mapped[src_start..src_start + unpadded]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(())
    }

    /// Reads the cached readback buffer into a freshly owned RGBA8 frame.
    ///
    /// Every public entry point returns the frame to its caller, so the output vector cannot
    /// be carried across calls; this is the thin adapter from [`Self::read_back_into`] to
    /// that contract (G-217).
    pub(crate) fn read_back_owned(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        expected_len: usize,
    ) -> Result<Vec<u8>, String> {
        let mut frame = Vec::new();
        self.read_back_into(buffer, width, height, expected_len, &mut frame)?;
        Ok(frame)
    }

    /// Upload a full RGBA8 frame to GPU texture and read it back deterministically.
    pub fn upload_rgba8_and_readback(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Result<Vec<u8>, String> {
        // Zero dimensions have no GPU texture representation and must be an
        // explicit error, not a zero-sized texture that silently succeeds
        // (N-S-66).
        if width == 0 || height == 0 {
            return Err(format!("width/height must be > 0, got {width}x{height}"));
        }
        // Compute the expected byte length with checked wide-type arithmetic: in
        // `u32` the product wraps (65536×16384 = 2^32), so this both reports
        // overflow and validates the input length against the real size (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rgba8.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                rgba8.len(),
            ));
        }
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let output_buffer_size = readback_frame_bytes(width, height)?;

        // Cached upload texture and readback buffer (REND-02). Both are fully overwritten by this
        // call — `write_texture` covers the whole extent — so no previous frame can leak through.
        let mut cache = self.cache.borrow_mut();
        cache.ensure_upload_target(&self.device, width, height);
        cache.ensure_readback(&self.device, output_buffer_size);
        let target = cache.upload_target.as_ref().expect("the upload target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
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
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rw_wgpu_encoder"),
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
    /// Get the shader module enum for integration (BLUE11 R5.2).
    pub fn shader_modules(&self) -> Vec<ShaderModule> {
        vec![
            ShaderModule::FillRect,
            ShaderModule::DrawImage,
            ShaderModule::DrawText,
            ShaderModule::FillRoundedRect,
            ShaderModule::FillCircle,
        ]
    }

    /// Return a reference to the cached wgpu `ShaderModule` for the given module name.
    pub fn get_shader_module(&self, name: &str) -> Option<&wgpu::ShaderModule> {
        self.shader_cache.get(name)
    }
}

impl GpuRenderer for WgpuRenderer {
    fn initialize(&mut self) -> Result<(), String> {
        // Device and queue are already initialized in `new()`, so this is a no-op.
        Ok(())
    }

    fn capabilities(&self) -> &[GpuCapability] {
        &[GpuCapability::Basic2D, GpuCapability::Texture]
    }

    fn has_capability(&self, capability: GpuCapability) -> bool {
        matches!(capability, GpuCapability::Basic2D | GpuCapability::Texture)
    }

    fn render_frame(&mut self) -> Result<(), String> {
        // # Why this is an error and not a silent `Ok` (D09-GPU-01)
        //
        // This renderer exposes GPU draw entry points (`render_fill_rect_gpu`,
        // `render_draw_commands_rgba8`, …) that rasterise offscreen, but it has no swapchain or
        // present path: there is no way to put a frame on a window from here. Returning `Ok(())`
        // while drawing nothing is the same kind of untruth as reporting a GPU-accelerated scene
        // backend that only round-trips through the CPU — it tells the caller a frame was
        // presented when none was. Offscreen work belongs to the `render_*_rgba8` methods; a
        // real frame loop must be added before this can succeed.
        Err("WgpuRenderer has no present/swapchain path; render offscreen with the \
             render_*_rgba8 methods instead"
            .to_string())
    }

    fn memory_usage(&self) -> Option<u64> {
        // `wgpu` exposes no portable bytes-in-use query: `AdapterInfo` carries a device name and
        // limits, not an allocation total. Reporting `None` is the honest answer (G-232).
        None
    }

    fn vendor_info(&self) -> Option<&str> {
        // The real adapter name, captured at construction. An empty name means the backend had
        // nothing to report, which is also `None` rather than an invented string (G-232).
        (!self.adapter_info.name.is_empty()).then_some(self.adapter_info.name.as_str())
    }
}
