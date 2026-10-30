// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Replaying packed [`DrawStep`]s into a render pass.
//!
//! The frame's steps are recorded by one function so that a pass can be recorded against *any*
//! matching target: the frame itself, and — when non-`Normal` blend modes are enabled on the GPU —
//! the offscreen blend layer, which is the same size as the frame and therefore shares the stencil
//! attachment and the mvp.
//!
//! Split out of [`super::encode`] so that file stays about building the stream; this one only knows
//! how to turn already-packed handles into draw calls.
use super::encode::{CIRCLE_QUAD_BYTES, INSTANCE_BYTES, MAX_INSTANCES_PER_DRAW};
use super::frame::WgpuRenderer;
use super::resources::DrawStep;
use super::resources::ShadowLayer;
use super::uploads::{ImageDraw, PathDraw, RadialDraw, ShapeDraw, TextDraw};

/// The handles one recorded stream needs, resolved once per call.
///
/// Everything here borrows from the cache for the duration of the encode, so a recorded pass never
/// re-borrows the `RefCell` — that is what keeps the frame's own encoding free of interior-mutable
/// lookups.
pub(super) struct StepRecorder<'a> {
    pub(super) renderer: &'a WgpuRenderer,
    pub(super) width: u32,
    pub(super) height: u32,
    /// Dynamic-offset stride of the colour arena.
    pub(super) stride: usize,
    pub(super) vertices: &'a wgpu::Buffer,
    pub(super) uniform: &'a wgpu::BindGroup,
    pub(super) transformed: &'a wgpu::BindGroup,
    pub(super) circle_arena: &'a wgpu::BindGroup,
    pub(super) circle_vertex_offset: u64,
    pub(super) path_vertex_offset: u64,
    pub(super) image_draws: &'a [ImageDraw],
    pub(super) text_draws: &'a [TextDraw],
    pub(super) shape_draws: &'a [ShapeDraw],
    pub(super) radial_draws: &'a [RadialDraw],
    pub(super) path_draws: &'a [PathDraw],
    pub(super) shadow_layers: &'a [ShadowLayer],
    /// The pipeline a text draw uses. The frame's own pass composites text source-over; a blend
    /// run's layer needs the same coverage written straight, so it swaps in
    /// `text_layer_pipeline` through [`Self::for_layer`].
    pub(super) text_pipeline: &'a wgpu::RenderPipeline,
}

impl<'a> StepRecorder<'a> {
    /// The same recorder, drawing text into a blend layer.
    ///
    /// Over a transparent layer the frame's source-over text pipeline would store premultiplied
    /// values and lose the distinction between the glyph's colour alpha and its coverage; the
    /// `REPLACE` variant keeps both, which is what the composite's `Coverage` kind expects.
    pub(super) fn for_layer(&self) -> StepRecorder<'a> {
        StepRecorder { text_pipeline: &self.renderer.text_layer_pipeline, ..*self }
    }

    /// Records `steps` into `rp` in order.
    ///
    /// `DrawStep::Blend` must not appear here: the caller splits the plan at those markers and hands
    /// the inner steps to their own layer pass, so a step that tried to start a new pass would have
    /// nowhere to put it.
    pub(super) fn record(&self, rp: &mut wgpu::RenderPass<'_>, steps: &[DrawStep]) {
        for step in steps {
            match step {
                DrawStep::Scissor(rect) => {
                    rp.set_scissor_rect(rect.x as u32, rect.y as u32, rect.width, rect.height);
                }
                DrawStep::Clear(slot) => {
                    rp.set_pipeline(&self.renderer.clear_pipeline);
                    rp.set_bind_group(0, self.uniform, &[(*slot * self.stride) as u32]);
                    rp.set_scissor_rect(0, 0, self.width, self.height);
                    rp.draw(0..3, 0..1);
                }
                DrawStep::Rects(start, end) => {
                    rp.set_pipeline(&self.renderer.transformed_pipeline);
                    rp.set_bind_group(0, self.transformed, &[]);
                    rp.set_scissor_rect(0, 0, self.width, self.height);
                    let mut cursor = *start;
                    while cursor < *end {
                        let chunk_end = (cursor + MAX_INSTANCES_PER_DRAW).min(*end);
                        let byte_start = (cursor * INSTANCE_BYTES) as u64;
                        let byte_end = (chunk_end * INSTANCE_BYTES) as u64;
                        rp.set_vertex_buffer(0, self.vertices.slice(byte_start..byte_end));
                        rp.draw(0..6, 0..(chunk_end - cursor) as u32);
                        cursor = chunk_end;
                    }
                }
                DrawStep::Circle(index) => {
                    rp.set_pipeline(&self.renderer.circle_pipeline);
                    let colour_offset = (*index * 2 * self.stride) as u32;
                    let params_offset = ((*index * 2 + 1) * self.stride) as u32;
                    rp.set_bind_group(0, self.circle_arena, &[colour_offset, params_offset]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    let quad_start =
                        self.circle_vertex_offset + (*index * CIRCLE_QUAD_BYTES) as u64;
                    let quad_end = quad_start + CIRCLE_QUAD_BYTES as u64;
                    rp.set_vertex_buffer(0, self.vertices.slice(quad_start..quad_end));
                    rp.draw(0..6, 0..1);
                }
                DrawStep::Image(index) => {
                    let draw = &self.image_draws[*index];
                    rp.set_pipeline(&self.renderer.image_pipeline);
                    rp.set_bind_group(0, &draw.bind_group, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);
                }
                DrawStep::Text(index) => {
                    let draw = &self.text_draws[*index];
                    rp.set_pipeline(self.text_pipeline);
                    rp.set_bind_group(0, &draw.bind_group, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);
                }
                DrawStep::Path(index) => {
                    let draw = &self.path_draws[*index];
                    // Three passes over the same fan: count into the stencil buffer with the rule
                    // the command asked for, cover where the count says "inside", then reset the
                    // count so the next path starts from zero.
                    let byte_start = self.path_vertex_offset + draw.first_vertex as u64 * 8;
                    let byte_end = byte_start + draw.vertex_count as u64 * 8;
                    let stencil_pipeline = if draw.even_odd {
                        &self.renderer.path_stencil_evenodd_pipeline
                    } else {
                        &self.renderer.path_stencil_nonzero_pipeline
                    };
                    rp.set_pipeline(stencil_pipeline);
                    rp.set_bind_group(0, self.transformed, &[]);
                    rp.set_vertex_buffer(0, self.vertices.slice(byte_start..byte_end));
                    rp.draw(0..draw.vertex_count, 0..1);

                    rp.set_pipeline(&self.renderer.path_cover_pipeline);
                    rp.set_bind_group(0, &draw.cover_bind_group, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);

                    rp.set_pipeline(&self.renderer.path_stencil_clear_pipeline);
                    rp.set_bind_group(0, self.transformed, &[]);
                    rp.set_vertex_buffer(0, self.vertices.slice(byte_start..byte_end));
                    rp.draw(0..draw.vertex_count, 0..1);
                }
                DrawStep::Radial(index) => {
                    let draw = &self.radial_draws[*index];
                    rp.set_pipeline(&self.renderer.radial_pipeline);
                    rp.set_bind_group(0, &draw.bind_group, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);
                }
                DrawStep::Shape(index) => {
                    let draw = &self.shape_draws[*index];
                    rp.set_pipeline(&self.renderer.shape_pipeline);
                    rp.set_bind_group(0, &draw.bind_group, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);
                }
                // The finished blurred layer, drawn as an image whose params map it 1:1 onto its
                // own frame rectangle; the scissor step before this one is the paint region, so
                // nothing outside the caller's clip is touched.
                DrawStep::Shadow(index) => {
                    let layer = &self.shadow_layers[*index];
                    rp.set_pipeline(&self.renderer.image_pipeline);
                    rp.set_bind_group(0, &layer.composite, &[]);
                    rp.set_bind_group(1, self.transformed, &[]);
                    rp.draw(0..6, 0..1);
                }
                // Handled by the caller's plan walk, never inside a recorded segment.
                DrawStep::Blend { .. } => {}
            }
        }
    }
}

impl WgpuRenderer {
    /// Records the frame: the plan's own segments, and between them the blend runs.
    ///
    /// A blend run cannot be recorded inside the frame's pass — it needs a different attachment —
    /// so the pass is split around every run. A run's marker comes *after* its steps and carries how
    /// many of them it owns, so the driver learns each run's span without the packer having to know
    /// the count in advance. The first segment clears the frame (the software rasteriser starts from
    /// zeroed pixels); every later one loads what is already there. Each run then copies the frame
    /// into the backdrop texture, replays its own steps into the shared layer, and composites the
    /// layer back over the frame inside the run's bounding box.
    pub(super) fn record_frame_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        recorder: &StepRecorder<'_>,
        plan: &[DrawStep],
        target: &super::resources::TargetCache,
        stencil_view: &wgpu::TextureView,
        blend: Option<(&super::resources::BlendTargets, &[super::resources::BlendRun])>,
    ) {
        // The packer emits a marker only after it has recorded the run in `pack.blend_runs`, and
        // the encoder only skips `ensure_blend_targets` when that table is empty. This is the
        // invariant tying the two together: a marker without the offscreen pair would silently drop
        // the command from the frame, so it is asserted rather than skipped.
        debug_assert!(
            blend.is_some() || !plan.iter().any(|step| matches!(step, DrawStep::Blend { .. })),
            "a blend run reached the frame without its offscreen pair"
        );
        let mut cursor = 0usize;
        let mut started = false;
        for (index, step) in plan.iter().enumerate() {
            let DrawStep::Blend { run, steps } = *step else { continue };
            let run_start = index.saturating_sub(steps);
            if run_start > cursor {
                self.record_frame_segment(
                    encoder,
                    recorder,
                    target,
                    stencil_view,
                    &plan[cursor..run_start],
                    started,
                );
                started = true;
            }
            if let Some((targets, runs)) = blend {
                debug_assert!(runs.get(run).is_some(), "the plan named a blend run that was not packed");
                if let Some(entry) = runs.get(run) {
                    self.record_blend_run(
                        encoder,
                        recorder,
                        targets,
                        run,
                        entry,
                        &plan[run_start..index],
                        target,
                        stencil_view,
                    );
                    started = true;
                }
            }
            cursor = index + 1;
        }
        if cursor < plan.len() {
            self.record_frame_segment(
                encoder,
                recorder,
                target,
                stencil_view,
                &plan[cursor..],
                started,
            );
            started = true;
        }
        if !started {
            // Nothing was recorded — an empty plan — so the frame still has to start from
            // transparent black.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_draw_commands_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(stencil_attachment(stencil_view)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }

    /// Records one straight run of plan steps into the frame's own pass.
    fn record_frame_segment(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        recorder: &StepRecorder<'_>,
        target: &super::resources::TargetCache,
        stencil_view: &wgpu::TextureView,
        steps: &[DrawStep],
        load_existing: bool,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpu_draw_commands_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // The software rasteriser starts from zeroed pixels; mid-stream clears repaint
                    // through the clear pipeline instead (G-218). A segment after a blend run must
                    // keep what the run composited, hence the load.
                    load: if load_existing {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(stencil_attachment(stencil_view)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        recorder.record(&mut pass, steps);
    }

    /// Renders one blended command: backdrop copy, layer, composite.
    #[allow(clippy::too_many_arguments)] // the recorder, the run and both attachments are distinct.
    fn record_blend_run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        recorder: &StepRecorder<'_>,
        targets: &super::resources::BlendTargets,
        index: usize,
        run: &super::resources::BlendRun,
        steps: &[DrawStep],
        target: &super::resources::TargetCache,
        stencil_view: &wgpu::TextureView,
    ) {
        let bbox = run.bbox;
        if bbox.width == 0 || bbox.height == 0 {
            return;
        }
        // The backdrop the run is about to be blended against.
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: bbox.x.max(0) as u32, y: bbox.y.max(0) as u32, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &targets.backdrop,
                mip_level: 0,
                origin: wgpu::Origin3d { x: bbox.x.max(0) as u32, y: bbox.y.max(0) as u32, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d { width: bbox.width, height: bbox.height, depth_or_array_layers: 1 },
        );
        // The run's own steps, into a transparent layer.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_blend_layer_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.layer_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(stencil_attachment(stencil_view)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            recorder.for_layer().record(&mut pass, steps);
        }
        // Composite the layer over the frame's own pixels, inside the run's bounding box.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_blend_composite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(stencil_attachment(stencil_view)),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blend_composite_pipeline);
            pass.set_bind_group(0, &targets.units[index].bind_group, &[]);
            pass.set_scissor_rect(
                bbox.x.max(0) as u32,
                bbox.y.max(0) as u32,
                bbox.width,
                bbox.height,
            );
            pass.draw(0..3, 0..1);
        }
    }
}

/// The stencil attachment every 2D pass carries, cleared for the pass.
fn stencil_attachment<'a>(
    view: &'a wgpu::TextureView,
) -> wgpu::RenderPassDepthStencilAttachment<'a> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: None,
        stencil_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(0),
            store: wgpu::StoreOp::Discard,
        }),
    }
}
