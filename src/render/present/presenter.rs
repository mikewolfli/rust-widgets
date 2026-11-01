// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The presentation target: one per window, owned by the host's paint callback.
//!
//! # What this does, in order
//!
//! 1. Turns the backend's native handle into a presentation target (once, at creation).
//! 2. Keeps a full-frame texture that mirrors the last frame it was given.
//! 3. On each present, uploads **only the damaged rows** into that texture, draws it over the
//!    target, and hands the result to the compositor.
//!
//! Step 3 is why the target is redrawn in full even when the upload is partial: a presented
//! target's previous contents are undefined, so anything not drawn would flicker. The expensive
//! half — moving pixels from the CPU — is what damage reporting removes; the GPU-side draw is one
//! full-screen triangle either way.
//!
//! # Byte fidelity
//!
//! `Exact` asks for the software frame's bytes. The configuration picks a `*Unorm` format for that
//! intent, the full-screen pass samples the frame with a nearest filter, and the only transforms
//! applied are the ones the target *requires*:
//! [`needs_channel_swap`] for a BGRA readback,
//! [`needs_premultiplication`] for a premultiplied alpha mode, and
//! [`needs_srgb_decode`] when the target encodes on store. The
//! `exact_presentation_reproduces_the_frame_bytes` test on macOS reads the presented frame back
//! and compares it with the input, which is the only way to know the three flags are right.
//!
//! # Safety boundary
//!
//! [`FramePresenter::new`] is the only place a raw handle becomes a target. Its contract is
//! documented there: the handle must name a live window, the object must outlive the presenter,
//! and the call must run on the thread the windowing system requires (the AppKit main thread,
//! the Win32 UI thread, the Android UI thread). Everything above this module gets a
//! `&mut FramePresenter` and no pointer.

use super::config::{
    choose_config, fallback_present_mode, is_supported_by, needs_channel_swap,
    needs_premultiplication, needs_srgb_decode,
};
use super::damage::PresentDamage;
use super::status::{PresentHealth, PresentationStatus, RecoveryAction, SurfaceFailure};
use crate::compat::Vec;
use crate::wgpu_backend::{RenderQuality, WgpuDrawCommand, WgpuRenderer, GPU_READBACK_TIMEOUT};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

/// Counters a host can read without touching the target.
///
/// They answer "is this path actually doing anything?" — the question B29.12's acceptance asks
/// (an idle window must submit nothing) and B29.09's leak check asks (frames presented keeps pace
/// with frames offered).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PresentStats {
    /// Frames the host offered, whether or not anything was dirty.
    pub frames_offered: u64,
    /// Frames handed to the compositor.
    pub frames_presented: u64,
    /// Frames dropped (transient acquisition failure, or nothing to present).
    pub frames_skipped: u64,
    /// Rows uploaded from the CPU across all frames.
    pub rows_uploaded: u64,
}

/// One window's presentation target.
pub struct FramePresenter {
    adapter: wgpu::Adapter,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Full-frame mirror of the last frame uploaded, as `Rgba8Unorm`.
    frame_texture: wgpu::Texture,
    frame_view: wgpu::TextureView,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group: wgpu::BindGroup,
    /// Kept alive alongside the bind group that refers to it.
    _blit_uniform: wgpu::Buffer,
    /// Layout and sampler the blit bind group is built from, so a second group can bind the
    /// command renderer's target texture without rebuilding the pipeline.
    blit_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    /// What the adapter reported, needed to build the command renderer on this device.
    adapter_info: wgpu::AdapterInfo,
    /// The command-stream renderer, created on first use (see `present_commands`).
    renderer: Option<WgpuRenderer>,
    /// Why the command renderer could not be created, remembered so it is not retried per frame.
    renderer_error: Option<String>,
    damage: PresentDamage,
    health: PresentHealth,
    stats: PresentStats,
    quality: RenderQuality,
    logical_size: (u32, u32),
    scale: f32,
    /// Whether the target was configured with a source-copy usage, which
    /// [`Self::present_and_read_back`] needs and a plain present does not.
    copy_src: bool,
    /// The buffer a verification readback copies into, allocated per readback and dropped after.
    read_back: Option<ReadBackBuffer>,
    /// The most recent device-side failure, for diagnostics.
    last_failure: Option<SurfaceFailure>,
}

/// One verification readback's staging buffer.
struct ReadBackBuffer {
    buffer: wgpu::Buffer,
    /// Row stride the copy used, rounded up to [`wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`].
    padded_row_bytes: usize,
}

impl FramePresenter {
    /// Creates the target for `window_handle`.
    ///
    /// # Safety contract (enforced by the caller, documented here)
    ///
    /// * `window_handle` (and `display_handle`, when given) must name a live window, and that
    ///   window must stay alive until the returned presenter is dropped. `wgpu` keeps the handle
    ///   but does not own the window.
    /// * The call must run on the thread the windowing system allows a surface to be created on:
    ///   the AppKit main thread on macOS, the thread that owns the `HWND` on Windows, the UI
    ///   thread on Android. `Platform::frame_presentation_supported` answering `true` is the
    ///   backend's own statement that this is satisfied for the calling host.
    /// * Nothing else may reconfigure or destroy the window concurrently. The presenter is not
    ///   `Sync`, so a caller cannot move it to another thread by accident.
    ///
    /// # Errors
    ///
    /// A string describing which step failed: no adapter that can present to this window, no
    /// device, a target format this backend does not offer, or a zero-sized window.
    pub fn new(
        window_handle: RawWindowHandle,
        display_handle: Option<RawDisplayHandle>,
        size: (u32, u32),
        scale: f32,
        quality: RenderQuality,
    ) -> Result<Self, String> {
        if size.0 == 0 || size.1 == 0 {
            return Err(format!(
                "a presentation target cannot be created for a {size:?} window; \
                 a zero-sized window has nothing to present into"
            ));
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: crate::gpu::backend_ladder::instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        // SAFETY: the contract above is the caller's, and it is the same one `wgpu` documents for
        // `SurfaceTargetUnsafe::RawHandle`: the handle must name a live window that outlives the
        // surface. This module never drops the window, and the presenter is not `Send`, so the
        // handle cannot be used after the host's paint callback returned the presenter.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: display_handle,
                    raw_window_handle: window_handle,
                })
                .map_err(|error| format!("the window cannot be presented into: {error}"))?
        };

        let (adapter, tier) =
            pollster::block_on(crate::gpu::backend_ladder::select_adapter_with_gl_fallback(
                &instance,
                wgpu::PowerPreference::HighPerformance,
                Some(&surface),
            ))
            .ok_or_else(|| {
                "no adapter can present to this window (every backend tier refused it)".to_string()
            })?;
        if tier.is_degraded() {
            log::warn!("[present] using degraded backend tier for presentation: {}", tier.label());
        }

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rw_frame_presenter"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("the presentation device could not be created: {error}"))?;

        let wgpu_caps = surface.get_capabilities(&adapter);
        let config = choose_config(&wgpu_caps, size, scale, quality);
        if !is_supported_by(&config, &wgpu_caps) {
            return Err(format!(
                "this window offers no 8-bit sRGB-family format the presenter can write \
                 (offered: {:?}); the window stays on the default delivery path",
                wgpu_caps.formats
            ));
        }
        let copy_src = config.usage.contains(wgpu::TextureUsages::COPY_SRC);
        surface.configure(&device, &config);

        let (frame_texture, frame_view) =
            create_frame_texture(&device, config.width, config.height);
        let (blit_pipeline, blit_bind_group, blit_uniform, blit_layout, blit_sampler) = create_blit(
            &device,
            &queue,
            &frame_view,
            config.format,
            needs_premultiplication(config.alpha_mode),
            needs_srgb_decode(config.format),
        );

        let adapter_info = adapter.get_info();
        let mut damage = PresentDamage::new();
        damage.mark_full();
        Ok(Self {
            adapter,
            surface,
            device,
            queue,
            config,
            frame_texture,
            frame_view,
            blit_pipeline,
            blit_bind_group,
            _blit_uniform: blit_uniform,
            blit_layout,
            blit_sampler,
            adapter_info,
            renderer: None,
            renderer_error: None,
            damage,
            health: PresentHealth::new(),
            stats: PresentStats::default(),
            quality,
            logical_size: size,
            scale: normalize_scale(scale),
            copy_src,
            read_back: None,
            last_failure: None,
        })
    }

    /// The configuration in force.
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// The device this target presents with.
    ///
    /// Anything that draws **behind** this window's widgets must draw on this device and submit
    /// on [`Self::queue`]: a swapchain texture belongs to the device that created it, so a second
    /// device could produce pixels but could not hand them to this target. The colour format and
    /// size that a 3D layer has to match are in [`Self::config`].
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue this target submits on; see [`Self::device`].
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The mode to use if the target rejects the configured presentation mode.
    pub fn fallback_present_mode(&self) -> Option<wgpu::PresentMode> {
        fallback_present_mode(self.config.present_mode)
    }

    /// The state of the last presentation attempt.
    pub fn status(&self) -> PresentationStatus {
        self.health.status()
    }

    /// The counters for this target.
    pub fn stats(&self) -> PresentStats {
        self.stats
    }

    /// Whether a present is owed: some part of the frame changed since the last one.
    pub fn damage_pending(&self) -> bool {
        !self.damage.is_empty()
    }

    /// Marks damaged rectangles, in physical pixels of the current target.
    ///
    /// The tuples are the layout [`crate::render::PaintBackend::damage_regions`] returns, so a
    /// caller forwards that slice without converting it.
    pub fn mark_damage(&mut self, regions: &[(i32, i32, u32, u32)]) {
        self.damage.mark_regions(regions, self.config.height);
    }

    /// Marks the whole target dirty: first frame, resize, a restored window, a recreated surface.
    pub fn mark_full_damage(&mut self) {
        self.damage.mark_full();
    }

    /// Re-applies the current configuration to the target.
    ///
    /// The answer is the semantic outcome: `Reconfigured` on success, or the recovery decision's
    /// status when even re-applying fails, so a caller can tell "fixed" from "still broken"
    /// without inspecting an error string.
    pub fn configure(&mut self) -> PresentationStatus {
        match self.try_configure() {
            Ok(()) => {
                self.damage.mark_full();
                self.health.on_present_status(PresentationStatus::Reconfigured);
                PresentationStatus::Reconfigured
            }
            Err(failure) => self.handle_failure(failure),
        }
    }

    /// Applies a new logical size and scale factor.
    ///
    /// A target whose physical size did not change is left alone (a scale change that rounds to
    /// the same pixels is not a resize). Otherwise the target is reconfigured and marked fully
    /// damaged, because nothing of the old contents is valid at the new size.
    pub fn resize(&mut self, size: (u32, u32), scale: f32) -> PresentationStatus {
        let scale = normalize_scale(scale);
        // A canvas calls this on every frame. The capability query behind `choose_config` is a
        // surface call, so an unchanged size and scale must not repeat it.
        if (size, scale) == (self.logical_size, self.scale) {
            return PresentationStatus::Idle;
        }
        self.logical_size = size;
        self.scale = scale;
        let next = choose_config(&self.caps(), self.logical_size, self.scale, self.quality);
        if (next.width, next.height) == (self.config.width, self.config.height) {
            self.config = next;
            return PresentationStatus::Idle;
        }
        self.config = next;
        match self.try_configure() {
            Ok(()) => {
                let (texture, view) =
                    create_frame_texture(&self.device, self.config.width, self.config.height);
                self.frame_texture = texture;
                self.frame_view = view;
                let (pipeline, bind_group, uniform, layout, sampler) = create_blit(
                    &self.device,
                    &self.queue,
                    &self.frame_view,
                    self.config.format,
                    needs_premultiplication(self.config.alpha_mode),
                    needs_srgb_decode(self.config.format),
                );
                self.blit_pipeline = pipeline;
                self.blit_bind_group = bind_group;
                self._blit_uniform = uniform;
                self.blit_layout = layout;
                self.blit_sampler = sampler;
                self.damage.mark_full();
                self.health.on_present_status(PresentationStatus::Reconfigured);
                PresentationStatus::Reconfigured
            }
            Err(failure) => self.handle_failure(failure),
        }
    }

    /// Hands `frame` — `width × height` RGBA8 bytes matching [`Self::config`] — to the compositor.
    ///
    /// # Nothing dirty, nothing submitted
    ///
    /// A frame with no damage returns [`PresentationStatus::Idle`] and touches neither the queue
    /// nor the device. This is what makes an idle window cost nothing (B29.12).
    ///
    /// # Errors
    ///
    /// `Err` only for a caller mistake: a buffer whose length is not `width * height * 4`. Every
    /// device-side outcome is reported through [`Self::status`], because those are conditions the
    /// presenter repairs rather than errors the caller can act on.
    pub fn present(&mut self, frame: &[u8]) -> Result<PresentationStatus, String> {
        self.present_inner(frame, None)
    }

    /// Presents `frame` and returns what the compositor received, mapped back to RGBA order.
    ///
    /// This exists for verification — the acceptance of B29.09 is "the presented pixels equal
    /// the frame that was handed over", and there is no other way to observe a presented target.
    /// It costs a full readback and a device wait, so a frame loop must never call it; the
    /// presentation smoke example and the plan's script are its callers.
    ///
    /// # Errors
    ///
    /// The same caller mistakes as [`Self::present`], plus "this target cannot be read back":
    /// a surface that does not advertise source-copy usage is presented correctly but cannot be
    /// observed, and saying so is better than returning an empty buffer.
    pub fn present_and_read_back(
        &mut self,
        frame: &[u8],
    ) -> Result<(PresentationStatus, Vec<u8>), String> {
        if !self.copy_src {
            return Err("this window's target does not allow reading the presented frame back \
                 (no source-copy usage); present without verification instead"
                .to_string());
        }
        let mut captured = Vec::new();
        let status = self.present_inner(frame, Some(&mut captured))?;
        Ok((status, captured))
    }

    /// Presents a frame described as GPU draw commands (the `Smooth` path).
    ///
    /// The stream is drawn by the existing pipelines on **this target's device** into their own
    /// colour/stencil texture, then copied onto the surface with one full-screen pass. That is one
    /// extra GPU pass and no readback: the alternative — handing the commands to a second renderer
    /// with its own device — cannot work, because a swapchain texture belongs to the device that
    /// created it.
    ///
    /// A stream the pipelines cannot draw faithfully is the caller's problem to detect: the
    /// `Smooth` intent must audit the frame first (`render::present::census`) and fall back to
    /// [`Self::present`] when any command forces CPU. This method draws what it is given.
    ///
    /// # Errors
    ///
    /// A string when the device rejects the stream (a validation error inside the command
    /// renderer). Device-side surface conditions are reported through [`Self::status`] instead,
    /// exactly as in [`Self::present`].
    pub fn present_commands(
        &mut self,
        commands: &[WgpuDrawCommand],
    ) -> Result<PresentationStatus, String> {
        self.present_commands_inner(commands, None)
    }

    /// [`Self::present_commands`] plus what the compositor received, mapped back to RGBA order.
    ///
    /// Same purpose and same cost as [`Self::present_and_read_back`]: verification only.
    pub fn present_commands_and_read_back(
        &mut self,
        commands: &[WgpuDrawCommand],
    ) -> Result<(PresentationStatus, Vec<u8>), String> {
        if !self.copy_src {
            return Err("this window's target does not allow reading the presented frame back \
                 (no source-copy usage); present without verification instead"
                .to_string());
        }
        let mut captured = Vec::new();
        let status = self.present_commands_inner(commands, Some(&mut captured))?;
        Ok((status, captured))
    }

    fn present_commands_inner(
        &mut self,
        commands: &[WgpuDrawCommand],
        capture: Option<&mut Vec<u8>>,
    ) -> Result<PresentationStatus, String> {
        self.stats.frames_offered += 1;
        if !self.health.status().is_usable() {
            self.stats.frames_skipped += 1;
            return Ok(self.health.status());
        }
        let surface_texture = match self.acquire_surface() {
            Ok(texture) => texture,
            Err(status) => {
                self.stats.frames_skipped += 1;
                return Ok(status);
            }
        };
        // Draw the stream with the pipelines. The mutable borrow ends here so the bind group can
        // be built from the device and the stored layout afterwards.
        let (width, height) = (self.config.width, self.config.height);
        {
            let renderer = match self.ensure_command_renderer() {
                Ok(renderer) => renderer,
                Err(error) => {
                    log::warn!(
                        "[present] the command renderer is unavailable ({error}); this frame is \
                         not presented"
                    );
                    self.stats.frames_skipped += 1;
                    return Ok(PresentationStatus::Skipped);
                }
            };
            renderer.encode_draw_commands_frame(width, height, commands)?;
        }
        let Some(encoded) = self.renderer.as_ref().and_then(WgpuRenderer::encoded_target_view)
        else {
            self.stats.frames_skipped += 1;
            return Ok(PresentationStatus::Skipped);
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rw_present_smooth_bind_group"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&encoded),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self._blit_uniform.as_entire_binding(),
                },
            ],
        });
        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rw_present") });
        self.encode_blit(&mut encoder, &view, &bind_group);
        // A command stream redraws the frame whole, so nothing is left pending for the next one.
        self.damage.clear();
        self.finish_frame(encoder, surface_texture, capture)
    }

    fn present_inner(
        &mut self,
        frame: &[u8],
        capture: Option<&mut Vec<u8>>,
    ) -> Result<PresentationStatus, String> {
        self.stats.frames_offered += 1;
        let expected = self.config.width as usize * self.config.height as usize * 4;
        if frame.len() != expected {
            return Err(format!(
                "a {}x{} frame needs {expected} bytes, got {}",
                self.config.width,
                self.config.height,
                frame.len()
            ));
        }
        self.damage.finish();
        if self.damage.is_empty() {
            self.stats.frames_skipped += 1;
            return Ok(PresentationStatus::Idle);
        }
        if !self.health.status().is_usable() {
            // A stopped target does not try again: the host was told to use its default path.
            self.stats.frames_skipped += 1;
            return Ok(self.health.status());
        }
        let surface_texture = match self.acquire_surface() {
            Ok(texture) => texture,
            Err(status) => {
                self.stats.frames_skipped += 1;
                return Ok(status);
            }
        };
        self.upload_damaged_rows(frame);
        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rw_present") });
        self.encode_blit(&mut encoder, &view, &self.blit_bind_group);
        self.finish_frame(encoder, surface_texture, capture)
    }

    /// Encodes a copy of the presented texture into a fresh read-back buffer.
    fn encode_read_back(&mut self, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) {
        let width = self.config.width;
        let height = self.config.height;
        let unpadded = width as usize * 4;
        let padded = align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
        self.read_back = Some(ReadBackBuffer {
            buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rw_present_read_back"),
                size: (padded * height as usize) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            padded_row_bytes: padded,
        });
        let buffer = &self.read_back.as_ref().expect("just stored").buffer;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
    }

    /// Maps the buffer [`Self::encode_read_back`] filled and returns the frame in RGBA order.
    fn read_mapped_frame(&mut self) -> Result<Vec<u8>, String> {
        let Some(read_back) = self.read_back.take() else {
            return Err("no read-back buffer was encoded for this frame".to_string());
        };
        let slice = read_back.buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(GPU_READBACK_TIMEOUT),
            })
            .map_err(|error| format!("the presented frame could not be read back: {error}"))?;
        let unpadded = self.config.width as usize * 4;
        let mut out = Vec::with_capacity(unpadded * self.config.height as usize);
        {
            let mapped = slice
                .get_mapped_range()
                .map_err(|error| format!("the presented frame could not be mapped: {error}"))?;
            for row in 0..self.config.height as usize {
                let start = row * read_back.padded_row_bytes;
                out.extend_from_slice(&mapped[start..start + unpadded]);
            }
        }
        read_back.buffer.unmap();
        if needs_channel_swap(self.config.format) {
            // The target stores BGRA; a caller comparing with the RGBA frame it handed over must
            // see the same order it wrote. `as_chunks_mut` (not `chunks_exact_mut`) keeps the
            // stride a compile-time constant: the loop body is then a three-byte swap on a known
            // length, with no tail check per iteration.
            for pixel in out.as_mut_slice().as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Ok(out)
    }

    /// Acquires the next frame from the surface, or reports the semantic status to return.
    fn acquire_surface(&mut self) -> Result<wgpu::SurfaceTexture, PresentationStatus> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => Ok(texture),
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                // Usable now; reconfigure so the next frame is optimal. Presenting first keeps
                // this frame from being dropped over a cosmetic mismatch.
                let _ = self.try_configure();
                Ok(texture)
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                Err(self.handle_failure(SurfaceFailure::Timeout))
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                Err(self.handle_failure(SurfaceFailure::Occluded))
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                Err(self.handle_failure(SurfaceFailure::Outdated))
            }
            wgpu::CurrentSurfaceTexture::Lost => Err(self.handle_failure(SurfaceFailure::Lost)),
            wgpu::CurrentSurfaceTexture::Validation => {
                Err(self.handle_failure(SurfaceFailure::Validation))
            }
        }
    }

    /// Draws the frame texture over the whole target.
    fn encode_blit(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rw_present_blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.blit_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Submits the encoded frame, optionally copies it out for verification, presents, and updates
    /// the counters.
    fn finish_frame(
        &mut self,
        mut encoder: wgpu::CommandEncoder,
        surface_texture: wgpu::SurfaceTexture,
        capture: Option<&mut Vec<u8>>,
    ) -> Result<PresentationStatus, String> {
        if let Some(target) = capture {
            self.encode_read_back(&mut encoder, &surface_texture.texture);
            self.queue.submit(Some(encoder.finish()));
            let pixels = self.read_mapped_frame()?;
            target.extend_from_slice(&pixels);
        } else {
            self.queue.submit(Some(encoder.finish()));
        }
        self.queue.present(surface_texture);
        self.damage.clear();
        self.health.on_present();
        self.stats.frames_presented += 1;
        Ok(PresentationStatus::Presented)
    }

    /// The command renderer, created on this target's device the first time a stream is drawn.
    ///
    /// The failure is remembered: a host that asked for `Smooth` on a device whose pipelines cannot
    /// be built would otherwise pay the whole pipeline construction on every frame.
    fn ensure_command_renderer(&mut self) -> Result<&mut WgpuRenderer, String> {
        if let Some(error) = self.renderer_error.clone() {
            return Err(error);
        }
        if self.renderer.is_none() {
            match WgpuRenderer::from_device(
                self.device.clone(),
                self.queue.clone(),
                self.adapter_info.clone(),
            ) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    self.renderer_error = Some(error.clone());
                    return Err(error);
                }
            }
        }
        self.renderer.as_mut().ok_or_else(|| "the command renderer vanished".to_string())
    }

    /// Blocks until every submission already handed to the device has completed.
    ///
    /// The same intent as [`crate::wgpu_backend::WgpuRenderer::wait_idle`], with the same budget:
    /// callers get "is the device idle?" and no queue or fence. Test and benchmark use only; a
    /// frame loop must not call this.
    pub fn wait_idle(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(GPU_READBACK_TIMEOUT),
            })
            .map(|_| ())
            .map_err(|error| {
                format!("the presentation device did not reach idle within {GPU_READBACK_TIMEOUT:?}: {error}")
            })
    }

    /// The logical size the target was configured for.
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical_size
    }

    /// Uploads the damaged row bands of `frame` into the mirror texture.
    fn upload_damaged_rows(&mut self, frame: &[u8]) {
        let row_bytes = self.config.width as usize * 4;
        if self.damage.is_full() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.frame_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                frame,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(self.config.height),
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
            );
            self.stats.rows_uploaded += u64::from(self.config.height);
            return;
        }
        for band in self.damage.bands() {
            let start = band.y as usize * row_bytes;
            let end = band.end() as usize * row_bytes;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.frame_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: band.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &frame[start..end],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(band.height),
                },
                wgpu::Extent3d {
                    width: self.config.width,
                    height: band.height,
                    depth_or_array_layers: 1,
                },
            );
            self.stats.rows_uploaded += u64::from(band.height);
        }
    }

    /// The most recent device-side failure, when the last attempt did not present.
    ///
    /// The status alone says *what was decided* (skipped, reconfigure); this says *why*, which is
    /// what a host log needs to explain a window that stopped presenting.
    pub fn last_failure(&self) -> Option<SurfaceFailure> {
        self.last_failure
    }

    fn try_configure(&mut self) -> Result<(), SurfaceFailure> {
        if !is_supported_by(&self.config, &self.caps()) {
            return Err(SurfaceFailure::Outdated);
        }
        self.surface.configure(&self.device, &self.config);
        Ok(())
    }

    /// The live target's capabilities, re-read rather than cached.
    ///
    /// A window that moved to another display can offer a different format set, and a presenter
    /// holding the start-up answer would configure a format the new display does not accept.
    fn caps(&self) -> wgpu::SurfaceCapabilities {
        self.surface.get_capabilities(&self.adapter)
    }

    fn handle_failure(&mut self, failure: SurfaceFailure) -> PresentationStatus {
        self.last_failure = Some(failure);
        let action = self.health.on_failure(failure);
        match action {
            RecoveryAction::Continue => {}
            RecoveryAction::SkipFrame => {
                log::debug!("[present] {}: frame skipped", failure.label());
            }
            RecoveryAction::Reconfigure => {
                log::info!("[present] {}: reconfiguring the target", failure.label());
                // The health record has already counted the attempt, so a reconfigure that keeps
                // failing escalates on the next frame instead of looping here.
                self.surface.configure(&self.device, &self.config);
                self.damage.mark_full();
            }
            RecoveryAction::Recreate => {
                log::warn!(
                    "[present] {}: the target must be recreated; the host owns the window and is \
                     the only party that can ask for a new presenter",
                    failure.label()
                );
            }
            RecoveryAction::Stop => {
                log::error!(
                    "[present] {}: presentation stopped for this window; it stays on the default \
                     delivery path until the host recreates the presenter",
                    failure.label()
                );
            }
        }
        self.health.status()
    }
}

fn normalize_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn align_to(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment > 0, "alignment must be a power of two greater than zero");
    // `div_ceil` and not `(value + alignment - 1) / alignment`: the latter overflows for a value
    // near `usize::MAX`, which a corrupt buffer size would be.
    value.div_ceil(alignment) * alignment
}

fn create_frame_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rw_present_frame"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_blit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame_view: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    premultiply: bool,
    srgb_decode: bool,
) -> (wgpu::RenderPipeline, wgpu::BindGroup, wgpu::Buffer, wgpu::BindGroupLayout, wgpu::Sampler) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rw_present_blit_shader"),
        source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rw_present_blit_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(wgpu::BufferSize::new(16).expect("16 bytes fits")),
                },
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rw_present_blit_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rw_present_blit_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("rw_present_sampler"),
        // Nearest, because a filtering sampler would blend neighbouring texels at the frame edge
        // and change the bytes this pass is supposed to copy verbatim.
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    // Channel order is **not** a flag: rendering into a `Bgra8*` target stores the shader's red
    // component in the blue slot by format definition, so a manual swap would double the mapping.
    // The format does it; only premultiplication and sRGB encoding are transforms the target
    // cannot perform on its own.
    let flags = [u32::from(premultiply), u32::from(srgb_decode), 0, 0];
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rw_present_blit_flags"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut packed = [0u8; 16];
    for (index, value) in flags.iter().enumerate() {
        packed[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    queue.write_buffer(&uniform, 0, &packed);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rw_present_blit_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(frame_view),
            },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
        ],
    });
    (pipeline, bind_group, uniform, bind_group_layout, sampler)
}

/// The full-screen pass: one triangle, nearest sampling, the three target-mandated transforms.
const BLIT_SHADER: &str = r#"
struct Flags {
    premultiply: u32,
    srgb_decode: u32,
    pad0: u32,
    pad1: u32,
};

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;
@group(0) @binding(2) var<uniform> flags: Flags;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let corner = corners[index];
    var out: VsOut;
    out.position = vec4<f32>(corner, 0.0, 1.0);
    out.uv = vec2<f32>((corner.x + 1.0) * 0.5, (1.0 - corner.y) * 0.5);
    return out;
}

fn to_linear(channel: vec3<f32>) -> vec3<f32> {
    let low = channel / 12.92;
    let high = pow((channel + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, channel <= vec3<f32>(0.04045));
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var colour = textureSample(frame, frame_sampler, in.uv);
    if (flags.srgb_decode != 0u) {
        colour = vec4<f32>(to_linear(colour.rgb), colour.a);
    }
    if (flags.premultiply != 0u) {
        colour = vec4<f32>(colour.rgb * colour.a, colour.a);
    }
    return colour;
}
"#;
