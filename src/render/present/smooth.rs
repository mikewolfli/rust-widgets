// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `Smooth` intent's two halves: recording a frame's commands, and auditing whether they can
//! be drawn faithfully.
//!
//! # Where this stops today
//!
//! The recording, the audit, the conversion (`wgpu_backend::bridge_render_commands`) and the
//! delivery (`render::present::present_recorded_commands`) all exist. A stream is delivered as
//! commands only when **every** command has a GPU form; the families that do are measured in the
//! tests below and in `docs/plans/blue29.md` (B29.10). The two shapes of answer are:
//!
//! * exact — the GPU and the reference rasteriser write the same bytes (`FillRect`, clip state,
//!   blend state, uniform-radius rounded fills, the centred line/outline brush, ...);
//! * named divergence — the picture differs only where the plan says it may (`FillCircle`'s rim,
//!   `DrawText`'s one-LSB alpha rounding, a translucent image, a path boundary pixel).
//!
//! A command outside both sets still forces the whole frame back to `Exact`, which is why the
//! `ForcesCpu` names are printed instead of being folded into a silent fallback.
//!
//! # Why recording is separate from drawing
//!
//! A frame is a `Vec<RenderCommand>` produced by the widget tree. `Smooth` wants to hand that
//! *stream* to the GPU instead of a finished pixel buffer, so the first half is a [`PaintBackend`]
//! that keeps every command and draws nothing. Keeping it separate means the recording can be
//! tested without a device, and the decision about whether the stream is drawable is a pure
//! function over the commands (plan B29.10).
//!
//! # Why every command is classified, with no wildcard
//!
//! The enumeration in [`classify`] lists all [`RenderCommand`] variants explicitly. Adding a
//! variant therefore breaks the build instead of silently falling into a default — a new command
//! must be given a class before the crate compiles again. That is the compile-time form of
//! principle #106's "anchor the gate to the shape".
//!
//! # What the classes mean
//!
//! * [`CommandClass::Mapped`] — the GPU bridge reproduces the command's pixels.
//! * [`CommandClass::MappedWithDivergence`] — the GPU draws it, and the picture differs in a
//!   named way (`docs/plans/blue29.md` §1.2: an edge-feathered shape's rim, and a stream whose
//!   premultiplied storage differs on a transparent target).
//! * [`CommandClass::ForcesCpu`] — no faithful GPU form exists yet. A frame containing one falls
//!   back to `Exact` **as a whole**; mixing a partly-GPU frame with a partly-CPU frame would make
//!   the divergence depend on z-order, which is unpredictable for the host.

use super::super::core::RenderCommand;
use super::super::PaintBackend;
use crate::compat::Rc;
use crate::compat::Vec;
use crate::core::{Color, Font, Size};
use crate::render::{ShapedText, TextMetrics};

/// How the GPU path may draw one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    /// The GPU reproduces the command's pixels.
    Mapped,
    /// The GPU draws it with a difference named in `docs/plans/blue29.md` §1.2.
    MappedWithDivergence,
    /// No faithful GPU form exists; the frame must use the `Exact` path.
    ForcesCpu,
}

impl CommandClass {
    /// A stable name for logs and gate output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mapped => "mapped",
            Self::MappedWithDivergence => "mapped-with-divergence",
            Self::ForcesCpu => "forces-cpu",
        }
    }
}

/// The class of `command`, with the reason stated for a `ForcesCpu` answer.
///
/// The match has no wildcard arm on purpose (see the module docs): a new [`RenderCommand`]
/// variant is a compile error here until it is classified.
pub fn classify(command: &RenderCommand) -> (CommandClass, &'static str) {
    match command {
        // ── Bridged today (see `wgpu_backend::bridge::bridge_render_commands`) ──
        RenderCommand::FillRect { color, .. } => {
            if color.a == 255 {
                (CommandClass::Mapped, "the bridge converts an opaque filled rectangle verbatim")
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent rectangle composites through an offscreen run; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::FillPathNonZero { .. } => (
            CommandClass::MappedWithDivergence,
            "the GPU fan may include one boundary pixel the reference scanline fill excludes",
        ),
        RenderCommand::PushClip { .. } | RenderCommand::PopClip => {
            (CommandClass::Mapped, "clip state is part of the bridged subset")
        }
        RenderCommand::SetBlendMode { .. } => {
            (CommandClass::Mapped, "blend state is part of the bridged subset")
        }
        RenderCommand::DrawRect { color, .. } | RenderCommand::DrawRectStroke { color, .. } => {
            if color.a == 255 {
                (
                    CommandClass::Mapped,
                    "the bridge converts the outline to four centred line stamps",
                )
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent outline composites through an offscreen run; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::FillRoundedRect { .. } => (
            CommandClass::MappedWithDivergence,
            "the reference antialiases the corner rim; the GPU row spans are hard-edged (§1.2 class C)",
        ),
        RenderCommand::FillRoundedRectAA { .. } => (
            CommandClass::MappedWithDivergence,
            "both paths antialias the corner rim, by different sample rules (§1.2 class C)",
        ),
        RenderCommand::DrawLine { color, .. } | RenderCommand::DrawLineStroke { color, .. } => {
            if color.a == 255 {
                (CommandClass::Mapped, "the bridge uses the reference's centred line brush")
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent line composites through an offscreen run; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::DrawPath { filled: true, .. } => (
            CommandClass::MappedWithDivergence,
            "the GPU fan may include one boundary pixel the reference scanline fill excludes",
        ),
        RenderCommand::DrawPath { filled: false, color, .. } => {
            if color.a == 255 {
                (
                    CommandClass::Mapped,
                    "the bridge expands the outline into the reference's own centred line stamps",
                )
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent outline composites through offscreen runs; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::DrawImage { .. } => (
            CommandClass::MappedWithDivergence,
            "a translucent image's source-over composite rounds one LSB differently (§1.2 class B)",
        ),

        // ── Edge-feathered shapes that are bridged ──
        RenderCommand::FillCircle { .. } => (
            CommandClass::MappedWithDivergence,
            "the SDF pipeline feathers the rim while the reference fills an integer predicate",
        ),
        RenderCommand::FillCircleAA { .. } => (
            CommandClass::MappedWithDivergence,
            "both paths antialias the rim, by different sample rules (§1.2 class C)",
        ),
        // The ring now uses the backend's inside-stroke mode, so its geometry matches the
        // reference; only the edge sample rule differs.
        RenderCommand::DrawCircle { .. } | RenderCommand::DrawCircleStroke { .. } => (
            CommandClass::MappedWithDivergence,
            "the GPU ring is feathered by a one-pixel SDF; the reference samples a 2×2 grid (§1.2 class C)",
        ),
        RenderCommand::DrawArc { filled: false, color, .. } => {
            if color.a == 255 {
                (
                    CommandClass::Mapped,
                    "the bridge expands the arc into the reference's own centred line samples",
                )
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent arc composites through offscreen runs; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::DrawArc { filled: true, .. } => (
            CommandClass::MappedWithDivergence,
            "the bridge fills the reference's own triangle fan; the GPU may include one fan-boundary pixel the row spans exclude",
        ),
        RenderCommand::DrawLineAA { .. } | RenderCommand::DrawLineStrokeAA { .. } => (
            CommandClass::MappedWithDivergence,
            "the bridge uploads the reference's sample-grid coverage; its byte quantisation may round one LSB (§1.2 B)",
        ),
        RenderCommand::DrawPathDashed { color, .. } => {
            if color.a == 255 {
                (
                    CommandClass::Mapped,
                    "the bridge expands the dashes with the reference's own kernel and brushes each segment centred",
                )
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent dashed stroke composites through offscreen runs; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::DrawRoundedRectStroke { radii, .. }
        | RenderCommand::DrawRoundedRectStrokeAA { radii, .. } => {
            if radii.iter().all(|value| *value == radii[0]) {
                (
                    CommandClass::MappedWithDivergence,
                    "the reference antialiases the ring; the GPU row spans are hard-edged (§1.2 class C)",
                )
            } else {
                (
                    CommandClass::ForcesCpu,
                    "per-corner rounded strokes have no single-radius GPU form yet",
                )
            }
        }

        // ── Everything else: no faithful GPU form yet ──
        #[cfg(feature = "3d")]
        RenderCommand::FillQuad { .. } => {
            (CommandClass::ForcesCpu, "quad fills are not bridged yet")
        }
        #[cfg(feature = "3d")]
        RenderCommand::FillQuadAA { .. } => {
            (CommandClass::ForcesCpu, "antialiased quads are not bridged yet")
        }
        #[cfg(feature = "3d")]
        RenderCommand::DrawMesh3D { .. } => {
            (CommandClass::ForcesCpu, "3D meshes are outside the 2D bridge")
        }
        #[cfg(feature = "3d")]
        RenderCommand::StrokeQuad { .. } => {
            (CommandClass::ForcesCpu, "quad strokes are not bridged yet")
        }
        #[cfg(feature = "3d")]
        RenderCommand::DrawImageQuad { .. } => {
            (CommandClass::ForcesCpu, "transformed images are not bridged yet")
        }
        RenderCommand::DrawText { .. } => (
            CommandClass::MappedWithDivergence,
            "the GPU composites shaped coverage with the same ink; alpha rounding may differ by one LSB (§1.2 class B)",
        ),
        RenderCommand::DrawTextDecorated { .. } => (
            CommandClass::MappedWithDivergence,
            "the bridge expands the rules into centred strokes; the glyph alpha may round one LSB (§1.2 B)",
        ),
        RenderCommand::DrawTextRecorded { .. } => {
            (CommandClass::ForcesCpu, "recorded text is not bridged yet")
        }
        RenderCommand::DrawImageScaled { width, source_width, height, source_height, .. } => {
            if *width == *source_width && *height == *source_height {
                (
                    CommandClass::Mapped,
                    "an unscaled draw is the reference's own `draw_image` blit, which is byte-exact",
                )
            } else if crate::wgpu_backend::scaled_image_drift_is_bounded(*width, *height) {
                (
                    CommandClass::MappedWithDivergence,
                    "the GPU samples the reference's four texels with its own weights; bilinear sampling \
                     is continuous, so the accumulated sample drift changes the result by at most one \
                     level (§1.2 B)",
                )
            } else {
                (
                    CommandClass::ForcesCpu,
                    "past ~3300 px the reference's accumulated sample position can move the interpolation \
                     by more than one level, which this bridge does not declare",
                )
            }
        }
        RenderCommand::PushClipRoundedRect { .. } => {
            (CommandClass::ForcesCpu, "rounded clips are not bridged yet")
        }
        RenderCommand::PushLayer { effect, .. } => {
            if effect.is_none() {
                (
                    CommandClass::Mapped,
                    "a plain layer is exactly the clip its bounds push; its pop pops that clip",
                )
            } else {
                (
                    CommandClass::ForcesCpu,
                    "a layer effect captures the backdrop and composites a foreground, which the \
                     command stream has no form for",
                )
            }
        }
        // `PopLayer` is answered by the census, which tracks the layer stack: a pop can only be a
        // clip when it closes a plain push. Asked alone, it names the plain case, which is what the
        // audit counts for a well-formed stream.
        RenderCommand::PushLayerGroup { effect, opacity, .. } => {
            if crate::render::layer_paints_like_a_clip(*effect, *opacity) {
                (
                    CommandClass::Mapped,
                    "a layer group with no effect and full opacity paints what a clip paints; a replay \
                     cache only changes what the reference rasteriser caches, and the bridge replays \
                     the commands it covers (an opaque background is required for that, which the \
                     bridge checks)",
                )
            } else {
                (
                    CommandClass::ForcesCpu,
                    "a layer group separates a foreground to composite it, which the command stream has no form for",
                )
            }
        }
        RenderCommand::PopLayer => (
            CommandClass::Mapped,
            "a pop of a plain layer is a clip pop; the audit rejects a pop of an isolating layer",
        ),
        RenderCommand::DrawGradient { gradient, .. } => {
            gradient_class(crate::wgpu_backend::ramp_is_smooth(&crate::render::gradient_ramp_lut(gradient)))
        }
        RenderCommand::DrawConicGradient { stops, .. } => {
            gradient_class(crate::wgpu_backend::ramp_is_smooth(&crate::render::ramp_lut_for_stops(stops)))
        }
        RenderCommand::DrawPathDashedRecorded { .. }
        | RenderCommand::DrawPathRecorded { .. }
        | RenderCommand::FillPathNonZeroRecorded { .. } => {
            (CommandClass::ForcesCpu, "only the borrowed path forms are bridged")
        }
        RenderCommand::BoxShadow { blur_radius: 0, color, .. } => {
            if color.a == 255 {
                (
                    CommandClass::Mapped,
                    "an unblurred shadow is the reference's own translated, spread-adjusted rectangle",
                )
            } else {
                (
                    CommandClass::MappedWithDivergence,
                    "a translucent unblurred shadow composites through an offscreen run; one LSB may round (§1.2 B)",
                )
            }
        }
        RenderCommand::BoxShadow { .. } => (
            CommandClass::MappedWithDivergence,
            "the body is the reference's own offset rectangle and the blur is its own kernel; a \
             translucent body composites through an offscreen run, which may round one LSB (§1.2 B)",
        ),
        RenderCommand::BoxShadowRounded { blur_radius: 0, .. } => (
            CommandClass::MappedWithDivergence,
            "an unblurred rounded shadow is the same rounded fill; the corner rim samples differ (§1.2 class C)",
        ),
        RenderCommand::BoxShadowRounded { .. } => (
            CommandClass::MappedWithDivergence,
            "the body is the reference's own rounded fill and the blur is its own kernel; the corner \
             rim samples differ and a translucent body may round one LSB (§1.2 B/C)",
        ),
        RenderCommand::Blur { .. } => (
            CommandClass::MappedWithDivergence,
            "the bridged form blurs the frame region with the reference's own three box widths; \
             the region is copied out and back, so only the pipeline's colour round trip can differ",
        ),
        RenderCommand::ClipPath { .. } => {
            (CommandClass::ForcesCpu, "path clips are not bridged yet")
        }
        RenderCommand::PushMatte { .. } | RenderCommand::PopMatte | RenderCommand::EndMatte => {
            (CommandClass::ForcesCpu, "matte composition is not bridged yet")
        }
    }
}

/// The class of a gradient whose ramp is (or is not) smooth enough to bridge.
///
/// The bridge uploads the reference's **own** table, so the colours agree by construction; the one
/// thing it cannot guarantee is *which* of two neighbouring entries a pixel reads, because the ramp
/// position is an `f32` expression evaluated on a different unit and can land on either side of a
/// bucket boundary. The plan's allowed difference for that is "≤1 LSB", which holds exactly when
/// neighbouring entries differ by at most one level — so a ramp that jumps further has no bridged
/// form and says so instead of quietly exceeding its declared divergence.
fn gradient_class(ramp_is_smooth: bool) -> (CommandClass, &'static str) {
    if ramp_is_smooth {
        (
            CommandClass::MappedWithDivergence,
            "the GPU uploads the reference's own 256-entry ramp; a position on a table-bucket \
             boundary may select the neighbouring entry, which differs by at most one level (§1.2 B)",
        )
    } else {
        (
            CommandClass::ForcesCpu,
            "this ramp changes by more than one level per table entry, so a neighbouring-entry read \
             would exceed one LSB",
        )
    }
}

// `gradient_class` decides, through [`crate::wgpu_backend::ramp_is_smooth`], whether a gradient's
// ramp is bridgeable; keeping the predicate beside the conversion is what stops the audit and the
// adapter from disagreeing about a ramp.

/// The audit of one frame's command stream.
#[derive(Debug, Clone, Default)]
pub struct FrameCensus {
    /// Commands the bridge reproduces exactly.
    pub mapped: usize,
    /// Commands drawn with the divergence named in the plan.
    pub mapped_with_divergence: usize,
    /// Commands with no faithful GPU form.
    pub forces_cpu: usize,
    /// The distinct `ForcesCpu` variant names, in first-seen order.
    pub forces_cpu_names: Vec<&'static str>,
}

impl FrameCensus {
    /// Whether the whole frame can be drawn by the `Smooth` path.
    ///
    /// False as soon as one command forces CPU, which is the plan's rule: a frame is drawn by
    /// exactly one path, never half of each.
    pub fn can_draw_on_gpu(&self) -> bool {
        self.forces_cpu == 0
    }

    /// The number of commands audited.
    pub fn total(&self) -> usize {
        self.mapped + self.mapped_with_divergence + self.forces_cpu
    }
}

/// Counts the classes of every command in a frame.
pub fn census(commands: &[RenderCommand]) -> FrameCensus {
    let mut result = FrameCensus::default();
    // The layer stack: a plain layer is a clip, so its pop is only faithful while it closes one of
    // its own. `classify` cannot answer that for a single command, so the audit tracks the pushes
    // and answers for the pops — a pop that closes an isolating layer, or nothing at all, forces
    // the frame to the CPU exactly as the bridge does.
    let mut layers: Vec<bool> = Vec::new();
    for command in commands {
        let (class, _) = match command {
            RenderCommand::PushLayer { effect, .. } => {
                layers.push(effect.is_none());
                classify(command)
            }
            RenderCommand::PushLayerGroup { effect, opacity, .. } => {
                layers.push(crate::render::layer_paints_like_a_clip(*effect, *opacity));
                classify(command)
            }
            RenderCommand::PopLayer => match layers.pop() {
                Some(true) => (
                    CommandClass::Mapped,
                    "the pop closes a plain layer, which is a clip",
                ),
                Some(false) => (
                    CommandClass::ForcesCpu,
                    "the pop would composite an isolating layer, which the command stream has no form for",
                ),
                None => (
                    CommandClass::ForcesCpu,
                    "a layer pop with no matching push closes nothing, but the reference would pop a clip",
                ),
            },
            other => classify(other),
        };
        match class {
            CommandClass::Mapped => result.mapped += 1,
            CommandClass::MappedWithDivergence => result.mapped_with_divergence += 1,
            CommandClass::ForcesCpu => {
                result.forces_cpu += 1;
                let name = variant_name(command);
                if !result.forces_cpu_names.contains(&name) {
                    result.forces_cpu_names.push(name);
                }
            }
        }
    }
    result
}

/// The variant name of `command`, for the census report.
///
/// Spelled from the same match the classifier uses rather than from `Debug`, so a report line
/// cannot start naming a struct field or a payload.
fn variant_name(command: &RenderCommand) -> &'static str {
    match command {
        RenderCommand::FillRect { .. } => "FillRect",
        #[cfg(feature = "3d")]
        RenderCommand::FillQuad { .. } => "FillQuad",
        #[cfg(feature = "3d")]
        RenderCommand::FillQuadAA { .. } => "FillQuadAA",
        #[cfg(feature = "3d")]
        RenderCommand::DrawMesh3D { .. } => "DrawMesh3D",
        #[cfg(feature = "3d")]
        RenderCommand::StrokeQuad { .. } => "StrokeQuad",
        #[cfg(feature = "3d")]
        RenderCommand::DrawImageQuad { .. } => "DrawImageQuad",
        RenderCommand::DrawRect { .. } => "DrawRect",
        RenderCommand::DrawRectStroke { .. } => "DrawRectStroke",
        RenderCommand::FillRoundedRect { .. } => "FillRoundedRect",
        RenderCommand::FillRoundedRectAA { .. } => "FillRoundedRectAA",
        RenderCommand::DrawRoundedRectStroke { .. } => "DrawRoundedRectStroke",
        RenderCommand::DrawRoundedRectStrokeAA { .. } => "DrawRoundedRectStrokeAA",
        RenderCommand::DrawLine { .. } => "DrawLine",
        RenderCommand::DrawLineAA { .. } => "DrawLineAA",
        RenderCommand::DrawLineStrokeAA { .. } => "DrawLineStrokeAA",
        RenderCommand::DrawLineStroke { .. } => "DrawLineStroke",
        RenderCommand::FillCircle { .. } => "FillCircle",
        RenderCommand::FillCircleAA { .. } => "FillCircleAA",
        RenderCommand::DrawCircle { .. } => "DrawCircle",
        RenderCommand::DrawCircleStroke { .. } => "DrawCircleStroke",
        RenderCommand::DrawText { .. } => "DrawText",
        RenderCommand::DrawTextDecorated { .. } => "DrawTextDecorated",
        RenderCommand::DrawImage { .. } => "DrawImage",
        RenderCommand::DrawImageScaled { .. } => "DrawImageScaled",
        RenderCommand::PushClip { .. } => "PushClip",
        RenderCommand::PushClipRoundedRect { .. } => "PushClipRoundedRect",
        RenderCommand::PopClip => "PopClip",
        RenderCommand::PushLayer { .. } => "PushLayer",
        RenderCommand::PopLayer => "PopLayer",
        RenderCommand::PushLayerGroup { .. } => "PushLayerGroup",
        RenderCommand::DrawGradient { .. } => "DrawGradient",
        RenderCommand::DrawArc { .. } => "DrawArc",
        RenderCommand::DrawPath { .. } => "DrawPath",
        RenderCommand::DrawPathDashed { .. } => "DrawPathDashed",
        RenderCommand::FillPathNonZero { .. } => "FillPathNonZero",
        RenderCommand::BoxShadow { .. } => "BoxShadow",
        RenderCommand::BoxShadowRounded { .. } => "BoxShadowRounded",
        RenderCommand::Blur { .. } => "Blur",
        RenderCommand::ClipPath { .. } => "ClipPath",
        RenderCommand::SetBlendMode { .. } => "SetBlendMode",
        RenderCommand::PushMatte { .. } => "PushMatte",
        RenderCommand::PopMatte => "PopMatte",
        RenderCommand::EndMatte => "EndMatte",
        RenderCommand::DrawPathRecorded { .. } => "DrawPathRecorded",
        RenderCommand::DrawPathDashedRecorded { .. } => "DrawPathDashedRecorded",
        RenderCommand::FillPathNonZeroRecorded { .. } => "FillPathNonZeroRecorded",
        RenderCommand::DrawTextRecorded { .. } => "DrawTextRecorded",
        RenderCommand::DrawConicGradient { .. } => "DrawConicGradient",
    }
}

/// A paint backend that records the frame's commands and draws nothing.
///
/// # What it is for
///
/// The `Smooth` path needs the command stream, and the widget tree produces one through the same
/// [`PaintBackend`] interface the software backend implements. Recording here means the tree's
/// drawing code is unchanged: the host swaps the backend, gets the commands, and audits them with
/// [`census`] before deciding to bridge them.
///
/// # Measurement is delegated
///
/// `measure_text` / `shape_text` must give the same answers the software path gives, or the
/// recorded frame's geometry would differ from the frame it is replacing. They are forwarded to an
/// owned [`crate::render::SoftwarePaintBackend`] instead of being reimplemented (principle #101).
pub struct GpuPaintBackend {
    commands: Vec<RenderCommand>,
    clear: Color,
    size: Size,
    dpi_scale: f32,
    measurement: crate::render::SoftwarePaintBackend,
}

impl GpuPaintBackend {
    /// Creates a recorder for a `size` target at `dpi_scale`.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        Self {
            commands: Vec::new(),
            clear: Color::TRANSPARENT,
            size,
            dpi_scale,
            measurement: crate::render::SoftwarePaintBackend::new(size, dpi_scale),
        }
    }

    /// The recorded commands, in draw order.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }

    /// The colour `begin_frame` was called with.
    pub fn clear_color(&self) -> Color {
        self.clear
    }

    /// Takes the recorded commands, leaving the recorder empty for the next frame.
    pub fn take_commands(&mut self) -> Vec<RenderCommand> {
        core::mem::take(&mut self.commands)
    }

    /// The audit of the recorded frame.
    pub fn census(&self) -> FrameCensus {
        census(&self.commands)
    }
}

impl PaintBackend for GpuPaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.clear = clear;
        self.commands.clear();
        self.measurement.begin_frame(clear);
    }

    fn end_frame(&mut self) {
        self.measurement.end_frame();
    }

    fn execute_command(&mut self, command: &RenderCommand) {
        // Recorded, not drawn. The clone is the contract: a `PaintBackend` receives commands by
        // reference and the frame outlives the call that produced it.
        self.commands.push(command.clone());
    }

    fn size(&self) -> Size {
        self.size
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
        self.measurement.set_size(size);
    }

    fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale;
        self.measurement.set_dpi_scale(dpi_scale);
    }

    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.measurement.measure_text(text, font)
    }

    fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        self.measurement.shape_text(text, font)
    }

    fn frame_rgba(&self) -> &[u8] {
        // A recorder has no pixels: `Smooth` delivers commands, and a caller that needs a frame
        // after recording must ask the software backend for one (the `Exact` fallback). Returning
        // the measurement backend's (empty) buffer keeps the contract total without pretending
        // this backend drew anything.
        self.measurement.frame_rgba()
    }
}

/// One line per class, for a gate or a log.
pub fn census_report(census: &FrameCensus) -> Vec<(&'static str, usize)> {
    vec![
        ("mapped", census.mapped),
        ("mapped-with-divergence", census.mapped_with_divergence),
        ("forces-cpu", census.forces_cpu),
    ]
}

#[cfg(test)]
mod tests {
    use super::{census, classify, variant_name, CommandClass, FrameCensus, GpuPaintBackend};
    use crate::core::{Color, Point, Rect, Size};
    use crate::render::{CompoundPath, PaintBackend, RenderCommand};

    fn rect_command() -> RenderCommand {
        RenderCommand::FillRect { rect: Rect::new(0, 0, 4, 4), color: Color::BLACK }
    }

    fn path_command() -> RenderCommand {
        let path = CompoundPath::new(
            vec![Point::new(0, 0), Point::new(8, 0), Point::new(8, 8), Point::new(0, 8)],
            vec![(0, 4)],
        )
        .expect("a single contour");
        RenderCommand::FillPathNonZero { path, color: Color::BLACK }
    }

    #[test]
    fn the_bridged_subset_is_mapped() {
        for command in [
            RenderCommand::PushClip { x: 0, y: 0, width: 4, height: 4 },
            RenderCommand::PopClip,
            RenderCommand::SetBlendMode { mode: crate::render::BlendMode::Normal },
        ] {
            assert_eq!(classify(&command).0, CommandClass::Mapped, "{command:?}");
        }
    }

    #[test]
    fn an_edge_feathered_shape_is_mapped_with_divergence() {
        let command =
            RenderCommand::FillCircle { center: Point::new(4, 4), radius: 4, color: Color::BLACK };
        assert_eq!(classify(&command).0, CommandClass::MappedWithDivergence);
    }

    #[test]
    fn an_unbridged_command_forces_the_whole_frame_to_cpu() {
        let clip_path = RenderCommand::ClipPath {
            points: vec![Point::new(0, 0), Point::new(8, 0), Point::new(8, 8)].into(),
        };
        let commands = [path_command(), clip_path];
        let audit = census(&commands);
        assert_eq!(audit.mapped_with_divergence, 1);
        assert_eq!(audit.forces_cpu, 1);
        assert!(!audit.can_draw_on_gpu(), "one unbridged command is enough");
        assert_eq!(audit.forces_cpu_names, vec!["ClipPath"]);
        assert_eq!(audit.total(), 2);
    }

    #[test]
    fn the_forces_cpu_names_are_distinct_and_in_first_seen_order() {
        let clip_path = |width| RenderCommand::ClipPath {
            points: vec![Point::new(0, 0), Point::new(width, 0), Point::new(width, 8)].into(),
        };
        let commands = [clip_path(4), clip_path(6), path_command()];
        let audit = census(&commands);
        assert_eq!(audit.forces_cpu, 2);
        assert_eq!(audit.forces_cpu_names, vec!["ClipPath"]);
        assert_eq!(audit.mapped_with_divergence, 1);
        assert_eq!(census(&[]).total(), 0);
    }

    #[test]
    fn a_frame_of_only_bridged_commands_can_be_drawn() {
        let audit = census(&[path_command(), RenderCommand::PopClip]);
        assert!(audit.can_draw_on_gpu());
        assert_eq!(audit.forces_cpu, 0);
    }

    #[test]
    fn the_recorder_keeps_order_and_the_clear_colour() {
        let mut backend = GpuPaintBackend::new(Size::new(16, 16), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&rect_command());
        backend.execute_command(&path_command());
        assert_eq!(backend.clear_color(), Color::WHITE);
        assert_eq!(backend.commands().len(), 2);
        assert!(matches!(backend.commands()[0], RenderCommand::FillRect { .. }));
        assert_eq!(backend.census().mapped, 1);
        assert_eq!(backend.census().mapped_with_divergence, 1);
        assert_eq!(backend.census().forces_cpu, 0);
        assert!(backend.census().can_draw_on_gpu(), "a rectangle plus a path is entirely bridged");
    }

    #[test]
    fn a_new_frame_clears_the_previous_one() {
        let mut backend = GpuPaintBackend::new(Size::new(8, 8), 2.0);
        backend.begin_frame(Color::BLACK);
        backend.execute_command(&rect_command());
        backend.begin_frame(Color::WHITE);
        assert!(backend.commands().is_empty());
        assert_eq!(backend.clear_color(), Color::WHITE);
        assert_eq!(backend.dpi_scale(), 2.0);
        assert_eq!(backend.size(), Size::new(8, 8));
    }

    #[test]
    fn taking_the_commands_empties_the_recorder() {
        let mut backend = GpuPaintBackend::new(Size::new(8, 8), 1.0);
        backend.begin_frame(Color::BLACK);
        backend.execute_command(&rect_command());
        let taken = backend.take_commands();
        assert_eq!(taken.len(), 1);
        assert!(backend.commands().is_empty());
    }

    #[test]
    fn measurement_is_delegated_not_reimplemented() {
        let backend = GpuPaintBackend::new(Size::new(64, 32), 1.0);
        let font = crate::core::Font::default();
        let metrics = backend.measure_text("hello", &font);
        assert!(
            metrics.width > 0,
            "the delegate measured the run; a recorder that returned zeros would lay every label \
             out at zero width"
        );
        // The recorder drew nothing, so its pixel buffer is the delegate's (never-drawn) one,
        // not a buffer of the recorder's own.
        let delegate = crate::render::SoftwarePaintBackend::new(Size::new(64, 32), 1.0);
        assert_eq!(backend.frame_rgba().len(), delegate.frame_rgba().len());
    }

    /// The audit and the bridge must agree: a command classified `Mapped` has to convert, and one
    /// classified `ForcesCpu` must not. Two tables that can drift are how a frame gets classified
    /// drawable and then fails to convert (principle #101).
    #[test]
    fn the_census_agrees_with_the_bridge_for_every_bridged_family() {
        let families = [
            rect_command(),
            path_command(),
            RenderCommand::PushClip { x: 0, y: 0, width: 4, height: 4 },
            RenderCommand::PopClip,
            RenderCommand::SetBlendMode { mode: crate::render::BlendMode::Normal },
            RenderCommand::DrawRect { rect: Rect::new(0, 0, 8, 8), color: Color::BLACK },
            RenderCommand::DrawRectStroke {
                rect: Rect::new(0, 0, 8, 8),
                color: Color::BLACK,
                width: 2,
            },
            RenderCommand::FillRoundedRect {
                rect: Rect::new(0, 0, 8, 8),
                radii: [2; 4],
                color: Color::BLACK,
            },
            RenderCommand::DrawLine {
                from: Point::new(0, 0),
                to: Point::new(8, 8),
                color: Color::BLACK,
            },
            RenderCommand::FillCircle { center: Point::new(4, 4), radius: 4, color: Color::BLACK },
            RenderCommand::FillCircleAA {
                center: Point::new(4, 4),
                radius: 4,
                color: Color::BLACK,
            },
            RenderCommand::DrawCircle { center: Point::new(4, 4), radius: 4, color: Color::BLACK },
            RenderCommand::DrawCircle { center: Point::new(4, 4), radius: 4, color: Color::BLACK },
            RenderCommand::DrawCircleStroke {
                center: Point::new(4, 4),
                radius: 4,
                color: Color::BLACK,
                width: 2,
            },
            RenderCommand::DrawArc {
                center: Point::new(4, 4),
                radius: 4,
                start_angle: 0.0,
                end_angle: 1.0,
                color: Color::BLACK,
                filled: false,
            },
            RenderCommand::DrawArc {
                center: Point::new(4, 4),
                radius: 4,
                start_angle: 0.0,
                end_angle: 1.0,
                color: Color::BLACK,
                filled: true,
            },
            RenderCommand::DrawRoundedRectStroke {
                rect: Rect::new(0, 0, 8, 8),
                radii: [2; 4],
                color: Color::BLACK,
                width: 1,
            },
            RenderCommand::DrawPath {
                points: vec![Point::new(0, 0), Point::new(4, 0), Point::new(4, 4)].into(),
                closed: false,
                color: Color::BLACK,
                filled: false,
                width: 1,
            },
            RenderCommand::DrawPath {
                points: vec![Point::new(0, 0), Point::new(4, 0), Point::new(4, 4)].into(),
                closed: true,
                color: Color::BLACK,
                filled: true,
                width: 1,
            },
            RenderCommand::DrawPathDashed {
                points: vec![Point::new(0, 0), Point::new(6, 6), Point::new(12, 0)].into(),
                closed: false,
                color: Color::BLACK,
                width: 1,
                dash: crate::render::core::command::DashPattern { mark: 3, gap: 2 },
            },
            RenderCommand::DrawLineAA {
                from: Point::new(0, 0),
                to: Point::new(6, 6),
                color: Color::BLACK,
            },
            RenderCommand::DrawLineStrokeAA {
                from: Point::new(0, 0),
                to: Point::new(6, 6),
                color: Color::BLACK,
                width: 2,
            },
            RenderCommand::PushLayer { bounds: Rect::new(0, 0, 8, 8), effect: None },
            RenderCommand::PushLayerGroup {
                bounds: Rect::new(0, 0, 8, 8),
                effect: None,
                opacity: 1.0,
                cache_key: None,
            },
            RenderCommand::PushLayerGroup {
                bounds: Rect::new(0, 0, 8, 8),
                effect: None,
                opacity: 1.0,
                cache_key: Some(1),
            },
            RenderCommand::DrawGradient {
                rect: Rect::new(0, 0, 8, 8),
                gradient: crate::style::Gradient::linear(Point::new(0, 0), Point::new(7, 7))
                    .add_stop(0.0, Color::BLACK)
                    .add_stop(1.0, Color::BLACK)
                    .into(),
            },
            RenderCommand::DrawConicGradient {
                center: Point::new(4, 4),
                start_angle: 0.0,
                stops: crate::compat::Arc::from([(0.0f32, Color::BLACK), (1.0, Color::BLACK)]),
            },
            RenderCommand::BoxShadow {
                rect: Rect::new(2, 2, 8, 6),
                color: Color::BLACK,
                offset_x: 1,
                offset_y: 2,
                blur_radius: 0,
                spread: 2,
            },
            RenderCommand::BoxShadowRounded {
                rect: Rect::new(2, 2, 8, 6),
                radii: [2; 4],
                color: Color::BLACK,
                offset_x: 1,
                offset_y: 2,
                blur_radius: 0,
                spread: 0,
            },
            RenderCommand::BoxShadow {
                rect: Rect::new(2, 2, 8, 6),
                color: Color::BLACK,
                offset_x: 1,
                offset_y: 2,
                blur_radius: 3,
                spread: 0,
            },
            RenderCommand::BoxShadowRounded {
                rect: Rect::new(2, 2, 8, 6),
                radii: [2; 4],
                color: Color::BLACK,
                offset_x: 1,
                offset_y: 2,
                blur_radius: 3,
                spread: 0,
            },
            RenderCommand::Blur { radius: 3 },
            RenderCommand::DrawImage {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                data: vec![0u8; 2 * 2 * 4].into(),
            },
            RenderCommand::DrawImageScaled {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
                source_width: 2,
                source_height: 2,
                data: vec![0u8; 2 * 2 * 4].into(),
            },
            RenderCommand::DrawText {
                origin: Point::new(0, 0),
                text: "x".into(),
                font: crate::core::Font::default(),
                color: Color::BLACK,
                alignment: crate::core::HorizontalAlignment::Left,
            },
            RenderCommand::DrawTextDecorated {
                origin: Point::new(0, 0),
                text: "x".into(),
                font: crate::core::Font::default(),
                color: Color::BLACK,
                alignment: crate::core::HorizontalAlignment::Left,
                decoration: crate::render::TextDecoration::UNDERLINE,
            },
        ];
        for command in families {
            let class = classify(&command).0;
            let converts = crate::wgpu_backend::bridge_render_commands(
                core::slice::from_ref(&command),
                1.0,
                (48, 32),
                true,
            )
            .is_ok();
            assert_eq!(
                class != CommandClass::ForcesCpu,
                converts,
                "{command:?} is classified {class:?} but the bridge answers {converts}"
            );
        }
    }

    #[test]
    fn the_bridge_converts_a_rectangle_stream_and_reports_the_first_gap() {
        let commands = [
            RenderCommand::PushClip { x: 0, y: 0, width: 8, height: 8 },
            rect_command(),
            RenderCommand::PopClip,
            RenderCommand::SetBlendMode { mode: crate::render::BlendMode::Normal },
        ];
        let bridged = crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (48, 32), true)
            .expect("every command has a GPU form");
        assert_eq!(bridged.len(), commands.len());

        let mut with_text = commands.to_vec();
        with_text.push(RenderCommand::DrawText {
            origin: Point::new(0, 0),
            text: "x".into(),
            font: crate::core::Font::default(),
            color: Color::BLACK,
            alignment: crate::core::HorizontalAlignment::Left,
        });
        let with_text_bridged =
            crate::wgpu_backend::bridge_render_commands(&with_text, 1.0, (48, 32), true)
                .expect("text now has a shaped-coverage form");
        assert_eq!(with_text_bridged.len(), with_text.len());
        assert!(
            matches!(
                with_text_bridged[4],
                crate::wgpu_backend::WgpuDrawCommand::DrawTextCoverage { .. }
            ),
            "a text command becomes its coverage mask, not its characters"
        );

        let mut with_blur = commands.to_vec();
        with_blur.push(RenderCommand::Blur { radius: 2 });
        let blurred = crate::wgpu_backend::bridge_render_commands(&with_blur, 1.0, (48, 32), true)
            .expect("a blur region now has a GPU form");
        assert!(
            matches!(blurred[4], crate::wgpu_backend::WgpuDrawCommand::BlurRegion { .. }),
            "a blur becomes an in-place region blur, not a rectangle approximation"
        );

        let mut with_clip_path = commands.to_vec();
        with_clip_path.push(RenderCommand::ClipPath {
            points: vec![Point::new(0, 0), Point::new(8, 0), Point::new(8, 8)].into(),
        });
        let gap = crate::wgpu_backend::bridge_render_commands(&with_clip_path, 1.0, (48, 32), true)
            .expect_err("a clip path has no GPU form yet");
        assert_eq!(gap.index, 4, "the gap names the first command that cannot be converted");
        assert_eq!(gap.variant, "ClipPath");
        assert!(gap.to_string().contains("ClipPath"), "the message names the variant");
    }

    /// The GPU and the reference rasteriser must paint the same bytes for the bridged subset.
    ///
    /// The stream really runs on the GPU (`selects_accelerated_pixels` says so, and the cost gate is
    /// off), so a difference here is a pipeline difference rather than a measurement artifact. A
    /// host with no adapter reports the skip, exactly like the `wgpu_backend` GPU tests.
    #[test]
    fn a_bridged_rectangle_frame_paints_the_same_bytes_on_the_gpu() {
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::WgpuRenderer;

        let (width, height) = (32, 24);
        let commands = [
            RenderCommand::FillRect {
                rect: Rect::new(2, 3, 10, 8),
                color: Color::rgba(200, 40, 90, 255),
            },
            RenderCommand::PushClip { x: 0, y: 0, width: 16, height: 12 },
            RenderCommand::FillRect {
                rect: Rect::new(8, 6, 12, 10),
                color: Color::rgba(10, 220, 30, 255),
            },
            RenderCommand::PopClip,
        ];
        assert!(census(&commands).can_draw_on_gpu(), "the frame is entirely bridged");

        let mut reference = SoftwarePaintBackend::new(Size::new(width, height), 1.0);
        reference.begin_frame(Color::WHITE);
        for command in &commands {
            reference.execute_command(command);
        }
        reference.end_frame();
        let expected = reference.frame_rgba().to_vec();

        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                // The pixel-path cost gate is a policy with its own tests; leaving it armed would
                // route this small frame to the CPU and turn the comparison into CPU == CPU.
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        let mut bridged =
            crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (48, 32), true)
                .expect("bridged");
        // The reference backend's `begin_frame(WHITE)` is the background; a GPU command stream has
        // no such call, so the same background is an explicit `Clear` at the front of the stream.
        bridged.insert(
            0,
            crate::wgpu_backend::WgpuDrawCommand::Clear {
                color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            },
        );
        assert!(
            renderer.selects_accelerated_pixels(width, height, &bridged),
            "the stream must really be drawn by the pipelines, not by the CPU fallback"
        );
        let actual =
            renderer.render_draw_commands_rgba8(width, height, &bridged).expect("a GPU frame");
        report_difference(width, &expected, &actual);
        assert_eq!(actual, expected, "the bridged rectangle stream must match the reference");
    }

    /// The centred-brush families must be byte-identical to the reference rasteriser, with and
    /// without a clip.
    ///
    /// [`WgpuDrawCommand::DrawLine`] and `StrokeRect` were rejected for this bridge precisely
    /// because their brush geometry differs from the reference; these four commands use the new
    /// centred forms, and this test is the evidence that the difference is gone.
    #[test]
    fn the_centred_brush_families_paint_the_same_bytes_on_the_gpu() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::WgpuRenderer;

        let size = CoreSize::new(48, 32);
        let (width, height) = (size.width, size.height);
        let color = Color::rgba(30, 120, 210, 255);
        let families: &[(&str, RenderCommand)] = &[
            ("DrawRect", RenderCommand::DrawRect { rect: Rect::new(4, 4, 20, 14), color }),
            (
                "DrawRectStroke",
                RenderCommand::DrawRectStroke { rect: Rect::new(4, 4, 20, 14), color, width: 3 },
            ),
            (
                "DrawLine",
                RenderCommand::DrawLine { from: Point::new(2, 2), to: Point::new(38, 26), color },
            ),
            (
                "DrawLineStroke",
                RenderCommand::DrawLineStroke {
                    from: Point::new(2, 26),
                    to: Point::new(38, 5),
                    color,
                    width: 3,
                },
            ),
            (
                "DrawArc",
                RenderCommand::DrawArc {
                    center: Point::new(24, 14),
                    radius: 10,
                    start_angle: 0.3,
                    end_angle: 2.5,
                    color,
                    filled: false,
                },
            ),
        ];

        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        for (name, command) in families {
            let commands = [
                RenderCommand::PushClip { x: 0, y: 0, width: 34, height: 28 },
                command.clone(),
                RenderCommand::PopClip,
            ];
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in &commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();

            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (48, 32), true)
                    .unwrap_or_else(|gap| {
                        panic!("{name}: the bridge rejected a Mapped family: {gap}")
                    });
            bridged.insert(
                0,
                crate::wgpu_backend::WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            report_difference(width, &expected, &actual);
            assert_eq!(actual, expected, "{name}: the centred brush must match the reference");
        }
    }

    /// names: one LSB for a translucent image, and the corner rim for an antialiased rounded fill.
    ///
    /// This is the test that keeps `MappedWithDivergence` honest. Without it a family could quietly
    /// move from "one LSB" to "a visibly different picture" and still be classified the same.
    #[test]
    fn the_divergent_families_differ_only_where_the_plan_says() {
        use crate::core::{Point, Size as CoreSize};
        use crate::render::{CompoundPath, SoftwarePaintBackend};
        use crate::wgpu_backend::WgpuRenderer;

        let size = CoreSize::new(40, 28);
        let (width, height) = (size.width, size.height);
        let image: crate::compat::Arc<[u8]> = (0..8 * 6)
            .flat_map(|index| {
                let alpha = if index % 3 == 0 { 128 } else { 255 };
                [(index * 7 % 256) as u8, 90, 160, alpha]
            })
            .collect::<Vec<_>>()
            .into();
        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };

        let run = |commands: &[RenderCommand]| {
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();
            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(commands, 1.0, (48, 32), true)
                    .expect("a divergence family still has a GPU form");
            bridged.insert(
                0,
                crate::wgpu_backend::WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            (expected, actual)
        };

        // Class B: a translucent image may round one LSB differently.
        let (expected, actual) = run(&[RenderCommand::DrawImage {
            x: 3,
            y: 3,
            width: 8,
            height: 6,
            data: crate::compat::Arc::clone(&image),
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left != right {
                differing += 1;
                assert!(
                    left.abs_diff(*right) <= 1,
                    "DrawImage byte {index} differs by more than one LSB: {left} vs {right}"
                );
            }
        }
        assert!(differing > 0, "the translucent image must exercise the one-LSB path");

        // A translucent rectangle goes through the offscreen run so the rect pipeline's REPLACE
        // write becomes source-over. The composite may still round one LSB differently.
        let (expected, actual) = run(&[RenderCommand::FillRect {
            rect: Rect::new(3, 4, 18, 12),
            color: Color::rgba(30, 120, 210, 160),
        }]);
        let mut worst = 0u8;
        for (left, right) in expected.iter().zip(&actual) {
            worst = worst.max(left.abs_diff(*right));
        }
        assert!(worst <= 1, "a translucent rectangle may differ by one LSB; got {worst}");

        // Class B: shaped text is the reference compositor's own coverage; only the composite's
        // rounding can differ, and it must stay within one LSB per channel.
        let text = RenderCommand::DrawText {
            origin: Point::new(4, 5),
            text: "Ag".into(),
            font: crate::core::Font::default(),
            color: Color::rgba(20, 30, 200, 160),
            alignment: crate::core::HorizontalAlignment::Left,
        };
        assert_eq!(classify(&text).0, CommandClass::MappedWithDivergence);
        let bridged = crate::wgpu_backend::bridge_render_commands(
            core::slice::from_ref(&text),
            1.0,
            (48, 32),
            true,
        )
        .expect("text has a coverage form");
        assert!(matches!(
            bridged[0],
            crate::wgpu_backend::WgpuDrawCommand::DrawTextCoverage { .. }
        ));
        let (expected, actual) = run(&[text]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left != right {
                differing += 1;
                assert!(
                    left.abs_diff(*right) <= 1,
                    "DrawText byte {index} differs by more than one LSB: {left} vs {right}"
                );
            }
        }
        assert!(
            differing <= 1,
            "the text composite may differ by at most one byte; got {differing}"
        );

        // Decorated text: the glyph run plus the reference's rules, expanded into lines. Every
        // byte must still be within the text composite's one-LSB bound.
        let decorated = RenderCommand::DrawTextDecorated {
            origin: Point::new(4, 5),
            text: "Ag".into(),
            font: crate::core::Font::default(),
            color: Color::rgba(20, 30, 200, 160),
            alignment: crate::core::HorizontalAlignment::Left,
            decoration: crate::render::TextDecoration::UNDERLINE
                | crate::render::TextDecoration::WAVY,
        };
        assert_eq!(classify(&decorated).0, CommandClass::MappedWithDivergence);
        let bridged = crate::wgpu_backend::bridge_render_commands(
            core::slice::from_ref(&decorated),
            1.0,
            (48, 32),
            true,
        )
        .expect("decorated text has a coverage + rule form");
        assert!(
            bridged.iter().any(|command| matches!(
                command,
                crate::wgpu_backend::WgpuDrawCommand::DrawLineCentered { .. }
            )),
            "the decoration must reach the GPU as centred line rules"
        );
        let (expected, actual) = run(&[decorated]);
        let mut worst = 0u8;
        let mut first = None;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            let diff = left.abs_diff(*right);
            if diff > 0 && first.is_none() {
                let pixel = index / 4;
                first = Some((
                    pixel as u32 % width,
                    pixel as u32 / width,
                    &expected[pixel * 4..pixel * 4 + 4],
                    &actual[pixel * 4..pixel * 4 + 4],
                ));
            }
            worst = worst.max(diff);
        }
        assert!(
            worst <= 1,
            "decorated text may differ by at most one byte; got {worst} (first {first:?})"
        );

        // Class C: the rounded fill's antialiased rim vs the GPU's hard corner.
        let rect = Rect::new(6, 4, 20, 14);
        let radius = 5;
        let (expected, actual) = run(&[RenderCommand::FillRoundedRect {
            rect,
            radii: [radius; 4],
            color: Color::rgba(200, 60, 40, 255),
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as i32;
            let y = (pixel as u32 / width) as i32;
            let near_x = x < rect.x + radius as i32 + 1 || x >= rect.right() - radius as i32 - 1;
            let near_y = y < rect.y + radius as i32 + 1 || y >= rect.bottom() - radius as i32 - 1;
            assert!(
                near_x && near_y,
                "the rounded fill differs outside its corner rim at ({x}, {y}): {left} vs {right}"
            );
        }
        assert!(differing > 0, "the antialiased corner must actually differ from the hard span");

        // Class C: a per-corner fill. Only the two rounded corners may differ from the hard row
        // spans; the square corners and the interior must be identical.
        let per_corner = Rect::new(6, 4, 24, 18);
        let (expected, actual) = run(&[RenderCommand::FillRoundedRect {
            rect: per_corner,
            radii: [8, 0, 8, 0],
            color: Color::rgba(200, 60, 40, 255),
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as i32;
            let y = (pixel as u32 / width) as i32;
            let near_top_left = x < per_corner.x + 10 && y < per_corner.y + 10;
            let near_bottom_right = x >= per_corner.right() - 10 && y >= per_corner.bottom() - 10;
            assert!(
                near_top_left || near_bottom_right,
                "the per-corner fill differs away from its two rounded corners at ({x}, {y})"
            );
        }
        assert!(differing > 0, "the per-corner rims must differ from the hard spans");

        // Class C: a filled disc. The reference fills an integer predicate, the GPU shades a
        // feathered SDF; the two may differ only on the rim, never inside or outside it.
        let center = (20, 14);
        let radius = 8u32;
        let (expected, actual) = run(&[RenderCommand::FillCircle {
            center: Point::new(center.0, center.1),
            radius,
            color: Color::rgba(200, 60, 40, 255),
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as i32;
            let y = (pixel as u32 / width) as i32;
            let dx = (x as f32 + 0.5) - center.0 as f32;
            let dy = (y as f32 + 0.5) - center.1 as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            assert!(
                (distance - radius as f32).abs() <= 2.0,
                "the disc differs away from its rim at ({x}, {y}), {distance} px from the centre"
            );
        }
        assert!(differing > 0, "the feathered rim must differ from the integer predicate");

        // Class C: a circle stroke lies **inside** its radius. The GPU's inside-ring mode must
        // draw the same band; differences are allowed only on its two rims.
        let ring_width = 2u32;
        let (expected, actual) = run(&[RenderCommand::DrawCircleStroke {
            center: Point::new(center.0, center.1),
            radius,
            color: Color::rgba(200, 60, 40, 255),
            width: ring_width,
        }]);
        let inner_radius = radius as f32 - ring_width as f32;
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as i32;
            let y = (pixel as u32 / width) as i32;
            let dx = (x as f32 + 0.5) - center.0 as f32;
            let dy = (y as f32 + 0.5) - center.1 as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            let start = (index / 4) * 4;
            let expected_pixel = &expected[start..start + 4];
            let actual_pixel = &actual[start..start + 4];
            assert!(
                (distance - radius as f32).abs() <= 2.0 || (distance - inner_radius).abs() <= 2.0,
                "the circle stroke differs away from both rims at ({x}, {y}), \
                 {distance} px from the centre: expected {expected_pixel:?} actual {actual_pixel:?}"
            );
        }
        assert!(differing > 0, "the ring's feathered rims must differ from the 2x2 samples");

        // Class C: a non-zero fill is the same shape, but the GPU fan can include one boundary
        // pixel per edge that the reference scanline fill excludes. The test pins that limit: a
        // differing pixel must be a full swap between the fill colour and the background, never a
        // different colour or an interior pixel.
        let path = CompoundPath::new(
            vec![Point::new(4, 4), Point::new(30, 6), Point::new(18, 22), Point::new(10, 18)],
            vec![(0, 4)],
        )
        .expect("a single closed contour");
        let fill = [200u8, 60, 40, 255];
        let background = [255u8, 255, 255, 255];
        let (expected, actual) = run(&[RenderCommand::FillPathNonZero {
            path,
            color: Color::rgba(fill[0], fill[1], fill[2], fill[3]),
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let start = pixel * 4;
            let expected_pixel = &expected[start..start + 4];
            let actual_pixel = &actual[start..start + 4];
            let swap = (expected_pixel == fill && actual_pixel == background)
                || (expected_pixel == background && actual_pixel == fill);
            assert!(
                swap,
                "the path fill changed a pixel that is neither a boundary swap nor the same colour \
                 at ({}, {}): {:?} vs {:?}",
                pixel as u32 % width,
                pixel as u32 / width,
                expected_pixel,
                actual_pixel
            );
        }
        let pixels = (width * height) as usize;
        assert!(
            differing <= (pixels / 4) * 4,
            "a boundary difference must stay small; got {differing}/{} bytes",
            expected.len()
        );
        assert!(differing > 0, "the boundary difference must be exercised, not assumed away");

        // Class C: an inside-stroke rounded ring. The GPU draws the same ring with hard edges;
        // every pixel that differs must sit on one of the two rims.
        let ring_rect = Rect::new(6, 4, 26, 18);
        let ring_radius = 5u32;
        let ring_width = 2u32;
        let (expected, actual) = run(&[RenderCommand::DrawRoundedRectStroke {
            rect: ring_rect,
            radii: [ring_radius; 4],
            color: Color::rgba(200, 60, 40, 255),
            width: ring_width,
        }]);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as f32;
            let y = (pixel as u32 / width) as f32;
            let outer = rounded_rect_signed_distance(x, y, ring_rect, ring_radius as f32);
            let inner_rect = Rect::new(
                ring_rect.x + ring_width as i32,
                ring_rect.y + ring_width as i32,
                ring_rect.width - ring_width * 2,
                ring_rect.height - ring_width * 2,
            );
            let inner = rounded_rect_signed_distance(
                x,
                y,
                inner_rect,
                ring_radius as f32 - ring_width as f32,
            );
            assert!(
                outer.abs() <= 1.5 || inner.abs() <= 1.5,
                "a rounded stroke changed a pixel away from both rims at ({x}, {y}): \
                 outer {outer}, inner {inner}"
            );
        }
        assert!(differing > 0, "the antialiased ring must differ from the hard spans");
    }

    /// The seventh round's families: a dashed polyline, an antialiased segment and a filled arc.
    ///
    /// Each is pinned to its **declared** class rather than to "it roughly works": the dashed
    /// stroke must be byte-identical, the antialiased stroke may differ only by the coverage byte's
    /// quantisation, and the filled arc may differ only on the boundary of the triangle fan the
    /// reference itself fills.
    #[test]
    fn the_dashed_antialiased_and_filled_arc_families_match_the_reference() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        let size = CoreSize::new(48, 32);
        let (width, height) = (size.width, size.height);
        let color = Color::rgba(30, 120, 210, 255);
        let background = [255u8, 255, 255, 255];
        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        let run = |commands: &[RenderCommand]| {
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();
            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(commands, 1.0, (48, 32), true)
                    .expect("these families all have a GPU form");
            bridged.insert(
                0,
                WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            (expected, actual, bridged)
        };

        // ── A stroked path: the reference's per-segment centred brush ──
        let outline = vec![Point::new(5, 5), Point::new(40, 9), Point::new(26, 27)];
        for (name, closed) in [("open", false), ("closed", true)] {
            let path = RenderCommand::DrawPath {
                points: outline.clone().into(),
                closed,
                color,
                filled: false,
                width: 2,
            };
            assert_eq!(classify(&path).0, CommandClass::Mapped, "{name}");
            let (expected, actual, _) = run(core::slice::from_ref(&path));
            assert!(
                expected.chunks(4).any(|pixel| pixel != background),
                "{name}: the reference must stroke the outline for the comparison to mean anything"
            );
            report_difference(width, &expected, &actual);
            assert_eq!(actual, expected, "{name}: a stroked path must match the reference");
        }

        // ── A filled path: the reference's *even-odd* rule, which is what the filled-path form
        // draws. A pentagram is the shape that proves it: its centre pentagon is crossed twice, so
        // an even-odd fill leaves it empty while a non-zero fill paints it. A rule mix-up is an
        // interior difference that the boundary band below cannot hide. ──
        let star = vec![
            Point::new(24, 4),
            Point::new(31, 26),
            Point::new(13, 12),
            Point::new(35, 12),
            Point::new(17, 26),
        ];
        let filled_path = RenderCommand::DrawPath {
            points: star.clone().into(),
            closed: true,
            color,
            filled: true,
            width: 1,
        };
        assert_eq!(classify(&filled_path).0, CommandClass::MappedWithDivergence);
        let (expected, actual, bridged) = run(core::slice::from_ref(&filled_path));
        let polygon: Vec<(i32, i32)> = star.iter().map(|point| (point.x, point.y)).collect();
        assert!(
            matches!(bridged[1], WgpuDrawCommand::DrawPath { filled: true, .. }),
            "a filled path must bridge to the backend's filled-path form"
        );
        // The centre pentagon is covered twice: the reference's even-odd rule leaves it empty.
        let centre_at = (16 * width as usize + 24) * 4;
        assert_eq!(
            &expected[centre_at..centre_at + 4],
            background,
            "the reference fills a pentagram with the even-odd rule, so its centre stays empty"
        );
        assert_eq!(
            &actual[centre_at..centre_at + 4],
            background,
            "the bridged fill must keep the even-odd rule, not the non-zero one"
        );
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as f32 + 0.5;
            let y = (pixel as u32 / width) as f32 + 0.5;
            let distance = distance_to_polygon_boundary(x, y, &polygon);
            assert!(
                distance <= 2.0,
                "the filled path differs {distance} px from its boundary at ({x}, {y})"
            );
        }
        eprintln!("filled path: {differing} divergent bytes");

        // ── An unblurred shadow: the reference's own translated, spread-adjusted rectangle ──
        for (name, shadow) in [
            (
                "BoxShadow",
                RenderCommand::BoxShadow {
                    rect: Rect::new(6, 6, 20, 14),
                    color,
                    offset_x: 3,
                    offset_y: 4,
                    blur_radius: 0,
                    spread: 2,
                },
            ),
            (
                "BoxShadow (no spread)",
                RenderCommand::BoxShadow {
                    rect: Rect::new(8, 4, 18, 12),
                    color,
                    offset_x: -2,
                    offset_y: 2,
                    blur_radius: 0,
                    spread: 0,
                },
            ),
        ] {
            assert_eq!(classify(&shadow).0, CommandClass::Mapped, "{name}");
            let (expected, actual, _) = run(core::slice::from_ref(&shadow));
            assert!(
                expected.chunks(4).any(|pixel| pixel != background),
                "{name}: the reference must paint the shadow"
            );
            report_difference(width, &expected, &actual);
            assert_eq!(actual, expected, "{name}: an unblurred shadow must match the reference");
        }

        // A blurred shadow is a body fill plus an in-place region blur of the reference's own
        // kernel. It must reach the GPU as exactly that pair — not as its unblurred rectangle.
        let blurred = RenderCommand::BoxShadow {
            rect: Rect::new(6, 6, 20, 14),
            color,
            offset_x: 2,
            offset_y: 2,
            blur_radius: 4,
            spread: 0,
        };
        assert_eq!(classify(&blurred).0, CommandClass::MappedWithDivergence);
        let bridged = crate::wgpu_backend::bridge_render_commands(
            core::slice::from_ref(&blurred),
            1.0,
            (48, 32),
            true,
        )
        .expect("a blurred shadow now has a GPU form");
        assert!(
            matches!(bridged[0], WgpuDrawCommand::FillRect { .. })
                && matches!(bridged[1], WgpuDrawCommand::BlurRegion { .. }),
            "a blurred shadow must bridge to its body fill plus a region blur, got {bridged:?}"
        );

        // ── An unblurred rounded shadow: the same rounded fill, rim difference only ──
        let rounded_shadow = RenderCommand::BoxShadowRounded {
            rect: Rect::new(6, 6, 20, 14),
            radii: [4; 4],
            color,
            offset_x: 2,
            offset_y: 3,
            blur_radius: 0,
            spread: 1,
        };
        assert_eq!(classify(&rounded_shadow).0, CommandClass::MappedWithDivergence);
        let (expected, actual, bridged) = run(core::slice::from_ref(&rounded_shadow));
        assert!(
            matches!(bridged[1], WgpuDrawCommand::FillRoundedRectRadii { .. }),
            "a rounded shadow must bridge to the per-corner rounded fill"
        );
        let shadow_rect = crate::render::box_shadow_spread_rect(Rect::new(6, 6, 20, 14), 2, 3, 1);
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as f32 + 0.5;
            let y = (pixel as u32 / width) as f32 + 0.5;
            let distance = rounded_rect_signed_distance(x, y, shadow_rect, 4.0);
            assert!(
                distance.abs() <= 1.5,
                "the rounded shadow differs {distance} px from its rim at ({x}, {y})"
            );
        }
        eprintln!("rounded shadow: {differing} divergent bytes");

        // ── A dashed polyline: the reference's dash kernel, so every byte must match ──
        let points =
            vec![Point::new(4, 6), Point::new(30, 6), Point::new(34, 22), Point::new(8, 26)];
        for (name, closed, stroke_width) in [("open", false, 1u32), ("closed", true, 2u32)] {
            let dashed = RenderCommand::DrawPathDashed {
                points: points.clone().into(),
                closed,
                color,
                width: stroke_width,
                dash: crate::render::core::command::DashPattern { mark: 4, gap: 3 },
            };
            assert_eq!(
                classify(&dashed).0,
                CommandClass::Mapped,
                "{name}: an opaque dashed stroke is byte-exact geometry"
            );
            let (expected, actual, bridged) = run(core::slice::from_ref(&dashed));
            assert!(
                expected.chunks(4).any(|pixel| pixel != background),
                "{name}: the reference must draw the dashes for the comparison to mean anything"
            );
            assert!(
                bridged
                    .iter()
                    .filter(|command| !matches!(command, WgpuDrawCommand::Clear { .. }))
                    .any(|command| matches!(command, WgpuDrawCommand::DrawLineCentered { .. })),
                "{name}: the dashes must reach the GPU as centred line stamps"
            );
            report_difference(width, &expected, &actual);
            assert_eq!(actual, expected, "{name}: a dashed stroke must match the reference");
        }

        // ── An antialiased segment: the reference's sample grid, quantised to a byte ──
        for (name, aa) in [
            (
                "DrawLineAA",
                RenderCommand::DrawLineAA { from: Point::new(4, 5), to: Point::new(42, 25), color },
            ),
            (
                "DrawLineStrokeAA",
                RenderCommand::DrawLineStrokeAA {
                    from: Point::new(6, 24),
                    to: Point::new(40, 7),
                    color,
                    width: 3,
                },
            ),
        ] {
            assert_eq!(classify(&aa).0, CommandClass::MappedWithDivergence, "{name}");
            let (expected, actual, bridged) = run(core::slice::from_ref(&aa));
            assert!(
                matches!(bridged[1], WgpuDrawCommand::DrawTextCoverage { .. }),
                "{name}: the antialiased stroke must reach the GPU as its coverage mask"
            );
            // The reference really antialiases: some pixel is neither the background nor the ink.
            let ink = [color.r, color.g, color.b, color.a];
            let soft = (0..(width * height) as usize)
                .filter(|pixel| {
                    let at = pixel * 4;
                    let pixel = &expected[at..at + 4];
                    pixel != background && pixel != ink
                })
                .count();
            assert!(soft > 0, "{name}: the reference must produce a soft edge to compare");
            let mut differing = 0usize;
            let mut worst = 0u8;
            for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
                if left != right {
                    differing += 1;
                    worst = worst.max(left.abs_diff(*right));
                }
                // A difference away from the soft edge is a shape difference, not rounding.
                if left != right {
                    let pixel = index / 4;
                    let at = pixel * 4;
                    let reference_pixel = &expected[at..at + 4];
                    assert!(
                        reference_pixel != background && reference_pixel != ink,
                        "{name}: byte {index} differs on a hard pixel: {left} vs {right}"
                    );
                }
            }
            assert!(worst <= 1, "{name}: the coverage quantisation may round one LSB; got {worst}");
            assert!(
                differing <= soft * 4,
                "{name}: {differing} divergent bytes for {soft} soft pixels is not rounding"
            );
            eprintln!("{name}: {soft} soft pixels, {differing} divergent bytes, worst {worst}");
        }

        // ── A filled arc: the reference's triangle fan, expressed as one contour ──
        let arc = RenderCommand::DrawArc {
            center: Point::new(24, 16),
            radius: 12,
            start_angle: 0.4,
            end_angle: 2.6,
            color,
            filled: true,
        };
        assert_eq!(classify(&arc).0, CommandClass::MappedWithDivergence);
        let (expected, actual, bridged) = run(core::slice::from_ref(&arc));
        let polygon = match &bridged[1] {
            WgpuDrawCommand::FillPathNonZero { points, contours, .. } => {
                assert_eq!(contours.as_ref(), &[(0usize, points.len())], "one wedge contour");
                points.to_vec()
            }
            other => panic!("a filled arc must bridge to the fan, got {other:?}"),
        };
        assert!(polygon.len() >= 4, "the fan needs the centre plus two samples");
        assert!(
            expected.chunks(4).any(|pixel| pixel != background),
            "the reference must fill the wedge for the comparison to mean anything"
        );
        let mut differing = 0usize;
        for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let pixel = index / 4;
            let x = (pixel as u32 % width) as f32 + 0.5;
            let y = (pixel as u32 / width) as f32 + 0.5;
            let distance = distance_to_polygon_boundary(x, y, &polygon);
            assert!(
                distance <= 2.0,
                "the filled arc differs {distance} px from the fan boundary at ({x}, {y})"
            );
        }
        assert!(
            differing <= expected.len(),
            "the fan difference must stay on the boundary; got {differing}/{} bytes",
            expected.len()
        );
        eprintln!("filled arc: {} fan vertices, {differing} divergent bytes", polygon.len());
    }

    /// Blurred shadows and the standalone blur: the reference blurs the **frame region**, so the
    /// bridged form is a body fill plus an in-place region blur, and the kernel is the reference's
    /// own three box widths.
    ///
    /// The standalone blur and the square shadow bodies are pinned to §1.2 **B** (≤1 LSB: the body
    /// of a translucent shadow composites through an offscreen run). A rounded body is **C**: the
    /// reference samples its corner bands on a 2×2 grid while the GPU spans them hard-edged, so the
    /// rim itself differs — and the blur then spreads that rim difference over its own reach. The
    /// allowance is therefore a *distance* from the rim, exactly as the unblurred rounded shadow
    /// test states it, not a blanket magnitude.
    #[test]
    fn a_blurred_region_matches_the_reference_kernel() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        /// A case's body: when it is a rounded fill, the rim the reference samples and the GPU
        /// spans, so a difference above one level may only land inside the blur's reach of it.
        struct Rim {
            rect: Rect,
            radius: f32,
            blur_radius: u32,
        }

        let size = CoreSize::new(48, 36);
        let (width, height) = (size.width, size.height);
        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        let stripes = RenderCommand::FillRect {
            rect: Rect::new(2, 3, 20, 9),
            color: Color::rgba(230, 40, 60, 255),
        };
        let rounded_shadow = RenderCommand::BoxShadowRounded {
            rect: Rect::new(10, 8, 22, 14),
            radii: [5; 4],
            color: Color::rgba(10, 20, 40, 120),
            offset_x: 0,
            offset_y: 3,
            blur_radius: 3,
            spread: 0,
        };
        let cases: Vec<(&str, Vec<RenderCommand>, Option<Rim>)> = vec![
            ("Blur over stripes", vec![stripes.clone(), RenderCommand::Blur { radius: 4 }], None),
            (
                "Blur, radius 1 (one box width only)",
                vec![stripes.clone(), RenderCommand::Blur { radius: 1 }],
                None,
            ),
            (
                "BoxShadow, opaque body",
                vec![RenderCommand::BoxShadow {
                    rect: Rect::new(8, 6, 22, 14),
                    color: Color::rgba(0, 0, 0, 255),
                    offset_x: 1,
                    offset_y: 2,
                    blur_radius: 4,
                    spread: 1,
                }],
                None,
            ),
            (
                "BoxShadow, translucent body",
                vec![RenderCommand::BoxShadow {
                    rect: Rect::new(8, 6, 22, 14),
                    color: Color::rgba(0, 0, 0, 48),
                    offset_x: 0,
                    offset_y: 4,
                    blur_radius: 6,
                    spread: 0,
                }],
                None,
            ),
            (
                "BoxShadowRounded",
                vec![rounded_shadow.clone()],
                Some(Rim {
                    rect: crate::render::box_shadow_spread_rect(Rect::new(10, 8, 22, 14), 0, 3, 0),
                    radius: 5.0,
                    blur_radius: 3,
                }),
            ),
        ];

        for (name, commands, rim) in cases {
            assert_eq!(
                classify(&commands[commands.len() - 1]).0,
                CommandClass::MappedWithDivergence,
                "{name}"
            );
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in &commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();

            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), true)
                    .unwrap_or_else(|gap| panic!("{name}: the bridge rejected a blur: {gap}"));
            assert!(
                bridged.iter().any(|command| matches!(command, WgpuDrawCommand::BlurRegion { .. })),
                "{name}: the blur must reach the frame as an in-place region blur"
            );
            bridged.insert(
                0,
                WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            // The same bridged stream through the **CPU parity** rasteriser: it shares the
            // reference's own `box_blur_region`, so the blur itself cannot drift — that is what
            // makes the fallback path a reproduction rather than a second implementation.
            let parity =
                crate::wgpu_backend::rasterize_draw_commands_rgba8(width, height, &bridged)
                    .expect("the CPU parity path accepts the bridged stream");

            // One comparison for both renderers, because the rule is about the *picture*, not about
            // which device drew it: a difference above one level may only land inside the blur's
            // reach of a rounded body's rim (the doc comment above says why).
            let compare = |label: &str, bytes: &[u8]| {
                let mut differing = 0usize;
                let mut worst = 0u8;
                for (index, (left, right)) in expected.iter().zip(bytes).enumerate() {
                    if left == right {
                        continue;
                    }
                    differing += 1;
                    let difference = left.abs_diff(*right);
                    worst = worst.max(difference);
                    if difference <= 1 {
                        continue;
                    }
                    let Some(rim) = &rim else {
                        panic!("{name} ({label}): may differ by one level; got {difference}");
                    };
                    let pixel = index / 4;
                    let x = (pixel as u32 % width) as f32 + 0.5;
                    let y = (pixel as u32 / width) as f32 + 0.5;
                    let distance = rounded_rect_signed_distance(x, y, rim.rect, rim.radius).abs();
                    // The blur expands its region by the kernel's total reach, so a rim difference
                    // legitimately lands anywhere inside that band; `1.5` is the reference's 2×2
                    // sample slack around the rim.
                    let allowed = blur_reach(rim.blur_radius) + 1.5;
                    assert!(
                        distance <= allowed,
                        "{name} ({label}): a difference of {difference} at ({x}, {y}) is \
                         {distance} px from the rim, beyond the blur's {allowed} px reach"
                    );
                }
                eprintln!("{name} ({label}): {differing} divergent bytes, worst {worst}");
                worst
            };
            let worst = compare("gpu", &actual);
            report_difference(width, &expected, &actual);
            assert!(
                worst <= 1 || rim.is_some(),
                "{name}: the region blur may differ by one level; got {worst}"
            );
            compare("cpu parity", &parity);
        }
    }

    /// How far a blur of `radius` reaches beyond its region: the sum of the three box kernels'
    /// half-widths, rounded up — exactly the expansion `box_blur_region` applies.
    fn blur_reach(radius: u32) -> f32 {
        let (b0, b1, b2) = crate::render::box_blur_widths(radius as f32 / 2.0);
        (b0.div_ceil(2) + b1.div_ceil(2) + b2.div_ceil(2)) as f32
    }

    /// A plain layer is exactly a clip, on the GPU too.
    ///
    /// The reference never materialises a foreground for a layer with no effect, full opacity and no
    /// cache key: `begin_layer` pushes the bounds as a clip and its pop only pops it. The bridge says
    /// the same thing by converting the pair to `PushClip`/`PopClip`, and this test is the pixel
    /// evidence that nothing else changed.
    #[test]
    fn a_plain_layer_paints_the_same_bytes_as_a_clip_on_the_gpu() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        let size = CoreSize::new(40, 28);
        let (width, height) = (size.width, size.height);
        let bounds = Rect::new(6, 4, 20, 16);
        let commands = [
            RenderCommand::PushLayer { bounds, effect: None },
            RenderCommand::FillRect {
                rect: Rect::new(0, 0, 40, 28),
                color: Color::rgba(40, 120, 200, 255),
            },
            RenderCommand::PopLayer,
            RenderCommand::FillRect {
                rect: Rect::new(24, 12, 12, 12),
                color: Color::rgba(200, 60, 40, 255),
            },
        ];
        assert_eq!(census(&commands).forces_cpu, 0, "a plain layer is bridged");

        let mut reference = SoftwarePaintBackend::new(size, 1.0);
        reference.begin_frame(Color::WHITE);
        for command in &commands {
            reference.execute_command(command);
        }
        reference.end_frame();
        let expected = reference.frame_rgba().to_vec();

        let mut bridged =
            crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), true)
                .expect("a plain layer converts");
        assert!(
            matches!(bridged[0], WgpuDrawCommand::PushClip { .. })
                && matches!(bridged[2], WgpuDrawCommand::PopClip),
            "a plain layer must bridge to the clip pair"
        );
        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        bridged.insert(
            0,
            WgpuDrawCommand::Clear {
                color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            },
        );
        assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
        let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
        report_difference(width, &expected, &actual);
        assert_eq!(actual, expected, "a plain layer must paint exactly what the clip paints");
    }

    /// A scaled image: the reference's four-texel bilinear sample, and its unscaled shortcut.
    ///
    /// `blit_scaled_software` short-circuits an unscaled draw to `draw_image`, so that case must be
    /// **byte-identical** (the bridge maps it to `DrawImage`). A genuinely scaled draw goes through
    /// `draw_image_quad`'s bilinear sample: because interpolation is continuous in `(u, v)`, the only
    /// difference the GPU can have is the interpolation's rounding — one level per channel.
    #[test]
    fn a_scaled_image_matches_the_reference_bilinear_sample() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        let size = CoreSize::new(36, 28);
        let (width, height) = (size.width, size.height);
        let (source_width, source_height) = (8u32, 6u32);
        // Neighbouring texels at opposite ends of the range: the worst case for a resampler.
        let source: crate::compat::Arc<[u8]> = (0..source_width * source_height)
            .flat_map(|index| {
                let x = index % source_width;
                let y = index / source_width;
                let value = ((x * 37 + y * 53) % 256) as u8;
                [value, 255 - value, ((x + y) % 2 * 255) as u8, 255]
            })
            .collect::<Vec<_>>()
            .into();

        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        let cases = [
            ("identity", 8u32, 6u32, true),
            ("upscaled", 21, 15, false),
            ("downscaled", 4, 3, false),
            ("wide, non-integer ratio", 33, 6, false),
        ];
        for (name, dest_width, dest_height, exact) in cases {
            let command = RenderCommand::DrawImageScaled {
                x: 2,
                y: 2,
                width: dest_width,
                height: dest_height,
                source_width,
                source_height,
                data: crate::compat::Arc::clone(&source),
            };
            let expected_class =
                if exact { CommandClass::Mapped } else { CommandClass::MappedWithDivergence };
            assert_eq!(classify(&command).0, expected_class, "{name}");

            let commands = [command];
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in &commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();

            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), true)
                    .unwrap_or_else(|gap| {
                        panic!("{name}: the bridge rejected a scaled image: {gap}")
                    });
            assert!(
                matches!(
                    bridged[0],
                    WgpuDrawCommand::DrawImage { .. } | WgpuDrawCommand::DrawImageScaled { .. }
                ),
                "{name}: the draw must reach the image pipelines"
            );
            bridged.insert(
                0,
                WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            if exact {
                report_difference(width, &expected, &actual);
                assert_eq!(actual, expected, "{name}: an unscaled draw must be byte-identical");
                continue;
            }
            let mut differing = 0usize;
            let mut worst = 0u8;
            for (left, right) in expected.iter().zip(&actual) {
                if left != right {
                    differing += 1;
                    worst = worst.max(left.abs_diff(*right));
                }
            }
            eprintln!("{name}: {differing} divergent bytes, worst {worst}");
            assert!(worst <= 1, "{name}: bilinear rounding may differ by one level; got {worst}");
        }

        // Past the drift bound the sample position is no longer provably inside one level, so the
        // family must say so instead of claiming it.
        let huge = RenderCommand::DrawImageScaled {
            x: 0,
            y: 0,
            width: 4096,
            height: 3,
            source_width,
            source_height,
            data: crate::compat::Arc::clone(&source),
        };
        assert_eq!(classify(&huge).0, CommandClass::ForcesCpu);
        assert!(
            crate::wgpu_backend::bridge_render_commands(&[huge], 1.0, (width, height), true)
                .is_err(),
            "the bridge must reject what the census refuses"
        );
    }

    /// A layer with a replay cache but no effect and full opacity paints what a clip paints, as long
    /// as the frame's background is opaque — and the whole-frame clear is what makes that true.
    #[test]
    fn a_cached_layer_is_a_clip_over_an_opaque_background() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        let size = CoreSize::new(40, 28);
        let (width, height) = (size.width, size.height);
        let bounds = Rect::new(6, 4, 20, 16);
        let commands = [
            RenderCommand::PushLayerGroup {
                bounds,
                effect: None,
                opacity: 1.0,
                cache_key: Some(7),
            },
            RenderCommand::FillRect {
                rect: Rect::new(0, 0, 40, 28),
                color: Color::rgba(40, 120, 200, 200),
            },
            RenderCommand::PopLayer,
        ];
        assert_eq!(census(&commands).forces_cpu, 0, "a cached layer is bridged");

        let mut reference = SoftwarePaintBackend::new(size, 1.0);
        reference.begin_frame(Color::WHITE);
        for command in &commands {
            reference.execute_command(command);
        }
        reference.end_frame();
        let expected = reference.frame_rgba().to_vec();

        let mut bridged =
            crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), true)
                .expect("a cached layer converts over an opaque background");
        assert!(
            matches!(bridged[0], WgpuDrawCommand::PushClip { .. })
                && matches!(bridged[2], WgpuDrawCommand::PopClip),
            "a cached layer must bridge to the clip pair"
        );
        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        // The frame's own background: the same whole-frame clear the presenter now prepends.
        bridged.insert(
            0,
            WgpuDrawCommand::Clear {
                color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            },
        );
        assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
        let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
        report_difference(width, &expected, &actual);
        assert_eq!(actual, expected, "a cached layer must paint what the clip paints");

        // The same stream over a translucent frame is *not* the identity, so it must be declined.
        assert!(
            crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), false)
                .is_err(),
            "a cached layer over a translucent background is not provably a clip"
        );
    }

    /// The audit tracks the layer stack, because a pop is only a clip when it closes a plain push.
    #[test]
    fn the_audit_tracks_the_layer_stack() {
        let bounds = Rect::new(0, 0, 8, 8);
        let plain = [RenderCommand::PushLayer { bounds, effect: None }, RenderCommand::PopLayer];
        let audit = census(&plain);
        assert_eq!(audit.forces_cpu, 0, "a plain layer pair is a clip pair");
        assert_eq!(audit.mapped, 2);

        let group = [
            RenderCommand::PushLayerGroup { bounds, effect: None, opacity: 1.0, cache_key: None },
            RenderCommand::PopLayer,
        ];
        assert_eq!(census(&group).forces_cpu, 0, "a group with nothing to isolate is plain");

        let effect = [
            RenderCommand::PushLayer {
                bounds,
                effect: Some(crate::render::LayerEffect::BackdropBlur { radius: 4 }),
            },
            RenderCommand::PopLayer,
        ];
        let audit = census(&effect);
        assert_eq!(audit.forces_cpu, 2, "both the push and its pop have no command form");
        assert_eq!(audit.forces_cpu_names, vec!["PushLayer", "PopLayer"]);

        let translucent =
            [RenderCommand::PushLayerGroup { bounds, effect: None, opacity: 0.5, cache_key: None }];
        let audit = census(&translucent);
        assert_eq!(audit.forces_cpu, 1, "a subtree opacity needs a foreground");
        assert_eq!(audit.forces_cpu_names, vec!["PushLayerGroup"]);

        // A pop with no push closes nothing, while the reference would pop a clip.
        let stray = [RenderCommand::PopLayer];
        let audit = census(&stray);
        assert_eq!(audit.forces_cpu, 1);
        assert_eq!(audit.forces_cpu_names, vec!["PopLayer"]);
        assert!(
            crate::wgpu_backend::bridge_render_commands(&stray, 1.0, (8, 8), true).is_err(),
            "an unbalanced pop must be rejected by the bridge too"
        );
    }

    /// The gradient families: the reference's own ramp table on the GPU, in all three geometries and
    /// through both commands that carry one.
    ///
    /// The carrier shares the reference's **table**, so the colours are the same bytes by
    /// construction; what this test measures is the one thing that can still differ — the per-pixel
    /// ramp position, whose f32 evaluation decides which table entry a pixel reads. A position that
    /// lands on the other side of a bucket boundary selects the neighbouring entry, which for these
    /// ramps is at most one level per channel.
    #[test]
    fn the_gradient_families_match_the_reference_within_one_table_step() {
        use crate::core::Size as CoreSize;
        use crate::render::SoftwarePaintBackend;
        use crate::style::Gradient;
        use crate::wgpu_backend::{WgpuDrawCommand, WgpuRenderer};

        let size = CoreSize::new(48, 32);
        let (width, height) = (size.width, size.height);
        // Smooth on purpose: neighbouring table entries differ by at most one level, which is what
        // the bridge's declared divergence allows. A steep ramp is rejected below instead of
        // silently exceeding it.
        let stops = [
            (0.0, Color::rgba(8, 16, 40, 255)),
            (0.5, Color::rgba(100, 90, 150, 255)),
            (1.0, Color::rgba(200, 180, 255, 255)),
        ];
        let with_stops = |gradient: Gradient| {
            stops
                .iter()
                .fold(gradient, |gradient, (position, color)| gradient.add_stop(*position, *color))
        };
        let rect = Rect::new(2, 2, 44, 28);
        let families: Vec<(&str, RenderCommand, u8)> = vec![
            (
                "linear",
                RenderCommand::DrawGradient {
                    rect,
                    gradient: with_stops(Gradient::linear(Point::new(4, 3), Point::new(42, 29)))
                        .into(),
                },
                0,
            ),
            (
                "linear (vertical, one axis)",
                RenderCommand::DrawGradient {
                    rect,
                    gradient: with_stops(Gradient::linear(Point::new(20, 2), Point::new(20, 30)))
                        .into(),
                },
                0,
            ),
            (
                "radial",
                RenderCommand::DrawGradient {
                    rect,
                    gradient: with_stops(Gradient::radial(Point::new(24, 16), 18.0)).into(),
                },
                1,
            ),
            (
                "conic fill",
                RenderCommand::DrawGradient {
                    rect,
                    gradient: with_stops(Gradient::conic(Point::new(24, 16), 30.0)).into(),
                },
                2,
            ),
            (
                "conic command",
                RenderCommand::DrawConicGradient {
                    center: Point::new(24, 16),
                    start_angle: 0.5,
                    stops: crate::compat::Arc::from(stops),
                },
                2,
            ),
        ];

        let renderer = match WgpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_cost_gate_enabled(false);
                renderer
            }
            Err(error) => {
                eprintln!("skipping GPU comparison: no wgpu adapter available ({error})");
                return;
            }
        };
        for (name, command, kind) in families {
            assert_eq!(classify(&command).0, CommandClass::MappedWithDivergence, "{name}");
            let commands = [command];
            let mut reference = SoftwarePaintBackend::new(size, 1.0);
            reference.begin_frame(Color::WHITE);
            for command in &commands {
                reference.execute_command(command);
            }
            reference.end_frame();
            let expected = reference.frame_rgba().to_vec();

            let mut bridged =
                crate::wgpu_backend::bridge_render_commands(&commands, 1.0, (width, height), true)
                    .unwrap_or_else(|gap| panic!("{name}: the bridge rejected a gradient: {gap}"));
            assert!(
                matches!(
                    bridged[0],
                    WgpuDrawCommand::FillGradient { kind: bridged_kind, .. } if bridged_kind == kind
                ),
                "{name}: the geometry tag must survive the bridge"
            );
            bridged.insert(
                0,
                WgpuDrawCommand::Clear {
                    color: crate::wgpu_backend::Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                },
            );
            assert!(renderer.selects_accelerated_pixels(width, height, &bridged));
            let actual = renderer.render_draw_commands_rgba8(width, height, &bridged).unwrap();
            let mut differing = 0usize;
            let mut worst = 0u8;
            let mut first = None;
            for (index, (left, right)) in expected.iter().zip(&actual).enumerate() {
                if left != right {
                    differing += 1;
                    worst = worst.max(left.abs_diff(*right));
                    let pixel = index / 4;
                    let at = pixel * 4;
                    if first.is_none() {
                        first = Some((
                            pixel as u32 % width,
                            pixel as u32 / width,
                            [expected[at], expected[at + 1], expected[at + 2], expected[at + 3]],
                            [actual[at], actual[at + 1], actual[at + 2], actual[at + 3]],
                        ));
                    }
                    assert!(
                        reference_pixel_is_gradient(&expected[at..at + 4]),
                        "{name}: byte {index} differs on a pixel the reference left as background"
                    );
                }
            }
            eprintln!("{name}: {differing} divergent bytes, worst {worst}, first {first:?}");
            assert!(
                worst <= 1,
                "{name}: the table entry may move by one level; got {worst} (of {differing} bytes)"
            );
            // The gradient must actually cover the rectangle in both paths, or the comparison would
            // be satisfied by two empty frames.
            assert!(
                expected.chunks(4).filter(|pixel| *pixel != [255, 255, 255, 255]).count()
                    >= (rect.width * rect.height) as usize,
                "{name}: the reference must paint the whole rectangle"
            );
        }

        // A ramp whose neighbouring entries jump by more than one level cannot be bridged within
        // the class it would have to declare, so it stays on the reference rasteriser and says so.
        let steep = RenderCommand::DrawGradient {
            rect,
            gradient: Gradient::linear(Point::new(4, 3), Point::new(42, 29))
                .add_stop(0.0, Color::rgba(255, 0, 0, 255))
                .add_stop(0.2, Color::rgba(0, 0, 255, 255))
                .into(),
        };
        assert_eq!(
            classify(&steep).0,
            CommandClass::ForcesCpu,
            "a steep ramp must not claim a one-level divergence"
        );
        assert!(
            crate::wgpu_backend::bridge_render_commands(
                core::slice::from_ref(&steep),
                1.0,
                (width, height),
                true
            )
            .is_err(),
            "the bridge must reject a ramp the census refuses"
        );
    }

    /// Whether `pixel` is anything but the white background the gradient tests begin from.
    fn reference_pixel_is_gradient(pixel: &[u8]) -> bool {
        pixel != [255, 255, 255, 255]
    }

    /// The distance from `(x, y)` to the boundary of a closed polygon, in pixels.
    fn distance_to_polygon_boundary(x: f32, y: f32, points: &[(i32, i32)]) -> f32 {
        let mut best = f32::MAX;
        for index in 0..points.len() {
            let (ax, ay) = points[index];
            let (bx, by) = points[(index + 1) % points.len()];
            let (ax, ay, bx, by) = (ax as f32, ay as f32, bx as f32, by as f32);
            let (dx, dy) = (bx - ax, by - ay);
            let length = dx * dx + dy * dy;
            let t = if length > 0.0 {
                (((x - ax) * dx + (y - ay) * dy) / length).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (px, py) = (ax + t * dx, ay + t * dy);
            best = best.min(((x - px).powi(2) + (y - py).powi(2)).sqrt());
        }
        best
    }

    /// The signed distance from `(x, y)` to a rounded rectangle's boundary.
    ///
    /// Negative inside, in pixels. Used by the ring test to state "the difference is on the rim"
    /// as a distance instead of a colour predicate: the reference's 2×2 sampling can cover one
    /// more edge pixel than the integer span, which is still a rim difference.
    fn rounded_rect_signed_distance(x: f32, y: f32, rect: Rect, radius: f32) -> f32 {
        let half_w = rect.width as f32 / 2.0;
        let half_h = rect.height as f32 / 2.0;
        let cx = rect.x as f32 + half_w;
        let cy = rect.y as f32 + half_h;
        let r = radius.min(half_w).min(half_h).max(0.0);
        let qx = (x - cx).abs() - (half_w - r);
        let qy = (y - cy).abs() - (half_h - r);
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - r
    }

    /// Prints where two frames differ, so a failing comparison names the region instead of dumping
    /// the whole buffer into the test output.
    fn report_difference(width: u32, expected: &[u8], actual: &[u8]) {
        if expected == actual {
            return;
        }
        let mut differing = 0usize;
        let mut first = None;
        for (index, (a, b)) in expected.iter().zip(actual).enumerate() {
            if a != b {
                differing += 1;
                if first.is_none() {
                    let pixel = index / 4;
                    first = Some((pixel as u32 % width, pixel as u32 / width, *a, *b));
                }
            }
        }
        eprintln!(
            "frame differs in {differing}/{} bytes; first at {:?} (x, y, expected, actual)",
            expected.len(),
            first
        );
    }

    #[test]
    fn every_class_and_variant_has_a_stable_name() {
        assert_eq!(CommandClass::Mapped.label(), "mapped");
        assert_eq!(CommandClass::MappedWithDivergence.label(), "mapped-with-divergence");
        assert_eq!(CommandClass::ForcesCpu.label(), "forces-cpu");
        assert_eq!(variant_name(&rect_command()), "FillRect");
        assert_eq!(variant_name(&RenderCommand::PopClip), "PopClip");
    }

    #[test]
    fn the_report_has_one_line_per_class() {
        let audit = FrameCensus::default();
        assert_eq!(
            super::census_report(&audit),
            vec![("mapped", 0), ("mapped-with-divergence", 0), ("forces-cpu", 0)]
        );
    }
}
