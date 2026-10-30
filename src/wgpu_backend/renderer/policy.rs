// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! How a stream's delivery path is chosen: the quality intent (L0) and the cost gate (L2).
//!
//! The proposal in `docs/plans/blue29.md` fixes the decision order: the host states what it wants
//! from the picture, then the dispatcher says whether the pipelines can draw the stream at all,
//! and only the remaining streams are compared on cost. This module owns the two policy halves;
//! [`super::dispatch`] still owns "can the GPU draw this?" and [`super::stream`] owns the entry
//! points that apply the order.
//!
//! # What each half may decide
//!
//! * [`RenderQuality`] describes an *intent*, not a mechanism: `ByteExact` means the caller needs
//!   the software reference's bytes, and `Antialiased` means smoother edges are worth accepting a
//!   difference. It never names a pipeline.
//! * [`CostGate`] answers "is this stream's affected area large enough that the GPU's delivery
//!   cost is repaid?". It is a whitelist, not a fitted cost model — the evidence in
//!   `docs/plans/blue29.md` §1.1 covers two workload families on one machine, which is enough for
//!   a threshold and not enough for coefficients.
//!
//! Neither half changes pixels: both branches are existing implementations.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::blend_mode_from_id;
use super::super::types::PixelRect;
use super::dispatch::{blend_bbox, draws_under_blend};
use crate::render::BlendMode;
use std::cell::Cell;

/// What the host needs from the picture.
///
/// The two values differ only in the edge treatment of the shapes whose pipelines feather their
/// edges. A host that compares frames byte for byte asks for [`RenderQuality::ByteExact`]; a host
/// that prefers a smoother edge over exact equality asks for [`RenderQuality::Antialiased`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderQuality {
    /// Every pixel must equal the software reference rasteriser's output.
    ///
    /// The streams that cannot promise that stay on the reference rasteriser: the ones containing
    /// an edge-feathered shape, and the ones drawn onto a transparent target with no whole-frame
    /// opaque clear (the premultiplied storage difference is independent of edge treatment).
    ByteExact,
    /// Smoother edges are preferred over byte-for-byte equality with the reference rasteriser.
    ///
    /// This is the default because it preserves the pre-existing picture. A host that needs exact
    /// equality opts in to [`RenderQuality::ByteExact`].
    #[default]
    Antialiased,
}

/// Whether `quality` requires `commands` to take the reference rasteriser.
///
/// The `ByteExact` checks are deliberately independent of whether the dispatcher would claim the
/// stream, so a host's intent is answered first (see `docs/plans/blue29.md` §2.1). The two
/// `ByteExact` rules are:
///
/// 1. an edge-feathered shape (`FillCircle` / `DrawCircle` / `DrawArc` / `StrokeRoundedRect`) is
///    present — those pipelines feather the rim while the reference fills an integer predicate;
/// 2. the stream contains no whole-frame opaque `Clear`, so drawing started on a transparent
///    target and the premultiplied storage rule can differ by much more than one level.
///
/// A filled rounded rectangle is *not* in the first set: it is drawn as the same row spans the CPU
/// fills, so it is already byte-identical.
pub(super) fn quality_refuses_stream(quality: RenderQuality, commands: &[WgpuDrawCommand]) -> bool {
    match quality {
        RenderQuality::Antialiased => false,
        RenderQuality::ByteExact => {
            has_edge_feathered_shape(commands) || !has_opaque_whole_frame_clear(commands)
        }
    }
}

/// The shapes whose pipelines feather their edge over one pixel.
fn has_edge_feathered_shape(commands: &[WgpuDrawCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            WgpuDrawCommand::FillCircle { .. }
                | WgpuDrawCommand::DrawCircle { .. }
                | WgpuDrawCommand::DrawArc { .. }
                | WgpuDrawCommand::StrokeRoundedRect { .. }
        )
    })
}

/// Whether a whole-frame opaque clear makes the destination opaque for this stream.
///
/// A `Clear` repaints the entire frame, so one opaque clear anywhere in the stream either starts
/// the frame opaque or erases the content drawn before it. Draws after it composite over an opaque
/// destination, which is the case the pipelines reproduce exactly.
fn has_opaque_whole_frame_clear(commands: &[WgpuDrawCommand]) -> bool {
    commands
        .iter()
        .any(|command| matches!(command, WgpuDrawCommand::Clear { color } if color.a == 255))
}

/// The delivery a stream should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeliveryPath {
    /// The GPU pipelines, including whatever delivery the caller asked for.
    Gpu,
    /// The reference rasteriser.
    Cpu,
}

/// The shape family whose area a stream is gated on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CostlyShape {
    /// A gradient whose own rectangle is large.
    Gradient,
    /// Geometry drawn under a non-`Normal` blend mode.
    Blend,
}

/// The previous answer, kept so a nearly identical frame reuses it instead of flickering.
///
/// `Copy` because it lives in a [`Cell`]: the entry points take `&self`, so the hysteresis state
/// needs interior mutability rather than `&mut self`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LastChoice {
    shape: CostlyShape,
    area: u64,
    path: DeliveryPath,
}

/// The area at or above which a whitelisted stream is expected to repay the GPU's delivery cost.
///
/// Source: `docs/plans/blue29.md` §1.1 ("面积 ≥ ~1 MP") together with the smallest frame the
/// benchmark found a GPU win on, 1280×720 = 921 600 px. A literal megapixel would exclude that
/// frame, so the constant sits just below it. The value is a starting point for the adapter this
/// machine has, not a universal truth; `WgpuRenderer::set_cost_gate_enabled` lets a host (or the
/// benchmark) bypass it while re-calibrating.
pub(super) const COST_GATE_AREA_THRESHOLD: u64 = 900_000;

/// How close an area must be to the previous decision's area to reuse that decision.
///
/// `docs/plans/blue29.md` §2.1 asks for a ±10% hysteresis band so a frame that jitters around a
/// boundary does not alternate paths frame by frame.
pub(super) const COST_GATE_HYSTERESIS_PERCENT: u64 = 10;

/// The cost gate for the pixel-returning delivery paths.
///
/// The gate is intentionally *not* consulted for the texture delivery: its alternative form is a
/// CPU raster followed by an upload, which B29.04 measures, but the first version still applies no
/// gate there (`blue29.md` §2.4). Treating "no gate" as an already-proven choice would be the same
/// kind of unfounded claim the gate exists to avoid.
pub(super) struct CostGate {
    enabled: bool,
    last: Cell<Option<LastChoice>>,
}

impl CostGate {
    /// A gate with the whitelist armed and no previous decision.
    pub(super) fn new() -> Self {
        Self { enabled: true, last: Cell::new(None) }
    }

    /// Whether the gate is allowed to move a stream off the GPU path.
    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Arms or disarms the gate.
    ///
    /// Disarming restores the pre-gate behaviour — a dispatchable stream goes to the GPU — which
    /// is what the benchmark uses to measure the raw GPU column that §1.1's facts rest on.
    pub(super) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Chooses the delivery for a stream the dispatcher has already claimed.
    ///
    /// Only the largest qualifying shape matters: a stream is as expensive as the biggest thing
    /// that makes the GPU path worth comparing. A nearly identical area reuses the previous
    /// answer (hysteresis); otherwise the whitelist decides.
    pub(super) fn choose(
        &self,
        framebuffer: PixelRect,
        commands: &[WgpuDrawCommand],
    ) -> DeliveryPath {
        if !self.enabled {
            return DeliveryPath::Gpu;
        }
        let Some((shape, area)) = costly_shape_area(framebuffer, commands) else {
            self.last.set(None);
            return DeliveryPath::Cpu;
        };
        if let Some(last) = self.last.get() {
            if last.shape == shape && within_hysteresis(last.area, area) {
                return last.path;
            }
        }
        let path =
            if area >= COST_GATE_AREA_THRESHOLD { DeliveryPath::Gpu } else { DeliveryPath::Cpu };
        self.last.set(Some(LastChoice { shape, area, path }));
        path
    }
}

/// Whether `current` is close enough to `previous` to reuse the previous answer.
fn within_hysteresis(previous: u64, current: u64) -> bool {
    previous.abs_diff(current).saturating_mul(100)
        <= previous.saturating_mul(COST_GATE_HYSTERESIS_PERCENT)
}

/// The largest qualifying shape in the stream, or `None` when no whitelist family is present.
///
/// The two families are the ones §1.1 measured a GPU win for at a large area: a gradient whose own
/// rectangle is large, and geometry drawn under a non-`Normal` blend mode. The area is the shape's
/// own clipped extent, not the frame's, which is what keeps a half-frame gradient or a small
/// blended rectangle on the reference path.
fn costly_shape_area(
    framebuffer: PixelRect,
    commands: &[WgpuDrawCommand],
) -> Option<(CostlyShape, u64)> {
    fn consider(shape: CostlyShape, rect: PixelRect, best: &mut Option<(CostlyShape, u64)>) {
        let area = rect.width as u64 * rect.height as u64;
        let larger = match *best {
            Some((_, previous)) => area > previous,
            None => true,
        };
        if area > 0 && larger {
            *best = Some((shape, area));
        }
    }

    let mut blend = BlendMode::Normal;
    let mut best: Option<(CostlyShape, u64)> = None;
    for command in commands {
        if let WgpuDrawCommand::SetBlendMode { mode } = command {
            blend = blend_mode_from_id(*mode);
            continue;
        }
        if let WgpuDrawCommand::DrawGradient { rect, .. }
        | WgpuDrawCommand::FillLinearGradient { rect, .. }
        | WgpuDrawCommand::FillRadialGradient { rect, .. } = command
        {
            consider(CostlyShape::Gradient, *rect, &mut best);
        }
        if blend != BlendMode::Normal && draws_under_blend(command) {
            if let Some(bbox) = blend_bbox(command, framebuffer, None) {
                consider(CostlyShape::Blend, bbox, &mut best);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::super::super::types::Rgba8;
    use super::*;

    fn frame(width: u32, height: u32) -> PixelRect {
        PixelRect { x: 0, y: 0, width, height }
    }

    fn opaque(r: u8, g: u8, b: u8) -> Rgba8 {
        Rgba8 { r, g, b, a: 255 }
    }

    fn gradient(rect: PixelRect) -> WgpuDrawCommand {
        WgpuDrawCommand::FillLinearGradient {
            rect,
            gradient_data: vec![0, 0, 0, 255].into(),
            clip: None,
        }
    }

    fn blended(rect: PixelRect) -> Vec<WgpuDrawCommand> {
        vec![
            WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 },
            WgpuDrawCommand::FillRect { rect, color: opaque(10, 20, 30), clip: None },
        ]
    }

    /// The small frame the benchmark measures every one of its streams on stays on the reference
    /// rasteriser, including the two whitelist families.
    #[test]
    fn a_small_frame_stays_on_the_reference_rasteriser() {
        let gate = CostGate::new();
        let small = frame(320, 200);
        assert_eq!(gate.choose(small, &[gradient(small)]), DeliveryPath::Cpu);
        assert_eq!(gate.choose(small, &blended(small)), DeliveryPath::Cpu);
    }

    /// A full-frame gradient crosses the threshold; a half-frame one does not.
    #[test]
    fn only_a_large_gradient_crosses_the_threshold() {
        let gate = CostGate::new();
        let large = frame(1280, 720);
        assert_eq!(gate.choose(large, &[gradient(large)]), DeliveryPath::Gpu);

        let gate = CostGate::new();
        let half = PixelRect { x: 0, y: 360, width: 1280, height: 360 };
        assert_eq!(gate.choose(large, &[gradient(half)]), DeliveryPath::Cpu);
    }

    /// A full-frame blended rectangle crosses; a small one does not.
    #[test]
    fn only_a_large_blended_run_crosses_the_threshold() {
        let gate = CostGate::new();
        let large = frame(1280, 720);
        assert_eq!(gate.choose(large, &blended(large)), DeliveryPath::Gpu);

        let gate = CostGate::new();
        let small = PixelRect { x: 16, y: 16, width: 48, height: 24 };
        assert_eq!(gate.choose(large, &blended(small)), DeliveryPath::Cpu);
    }

    /// A stream with neither whitelist family is never moved onto the GPU by the gate.
    #[test]
    fn a_plain_stream_is_refused_by_the_gate() {
        let gate = CostGate::new();
        let large = frame(1280, 720);
        let commands =
            [WgpuDrawCommand::FillRect { rect: large, color: opaque(1, 2, 3), clip: None }];
        assert_eq!(gate.choose(large, &commands), DeliveryPath::Cpu);
    }

    /// A nearly identical area reuses the previous answer instead of flickering across the
    /// threshold.
    #[test]
    fn the_hysteresis_reuses_the_previous_answer() {
        let gate = CostGate::new();
        let above = PixelRect { x: 0, y: 0, width: 1000, height: 910 };
        assert_eq!(gate.choose(above, &[gradient(above)]), DeliveryPath::Gpu);

        // 895 000 px is below the 900 000 threshold, so a fresh gate would answer `Cpu`; the
        // previous answer is within 10% of it, so the gate keeps `Gpu`.
        let below = PixelRect { x: 0, y: 0, width: 1000, height: 895 };
        assert_eq!(gate.choose(below, &[gradient(below)]), DeliveryPath::Gpu);

        let fresh = CostGate::new();
        assert_eq!(fresh.choose(below, &[gradient(below)]), DeliveryPath::Cpu);
    }

    /// Turning the gate off restores the raw GPU path, which is how the benchmark keeps measuring
    /// the baseline §1.1's numbers came from.
    #[test]
    fn a_disarmed_gate_keeps_the_stream_on_the_gpu_path() {
        let mut gate = CostGate::new();
        gate.set_enabled(false);
        assert!(!gate.enabled());
        let small = frame(320, 200);
        assert_eq!(gate.choose(small, &[gradient(small)]), DeliveryPath::Gpu);
    }

    /// The intent tier answers without consulting the dispatcher: an edge-feathered shape is
    /// refused by `ByteExact` even when the pipelines could draw it.
    #[test]
    fn byte_exact_refuses_edge_feathered_shapes() {
        let commands = [
            WgpuDrawCommand::Clear { color: opaque(0, 0, 0) },
            WgpuDrawCommand::FillCircle {
                center: (8, 8),
                radius: 4,
                color: opaque(255, 0, 0),
                clip: None,
            },
        ];
        assert!(quality_refuses_stream(RenderQuality::ByteExact, &commands));
        assert!(!quality_refuses_stream(RenderQuality::Antialiased, &commands));
    }

    /// `ByteExact` also refuses a stream drawn on a transparent target, even without a shape whose
    /// edge is feathered: the storage difference is independent of the edge treatment.
    #[test]
    fn byte_exact_refuses_a_transparent_target_without_an_opaque_clear() {
        let commands = [WgpuDrawCommand::FillRect {
            rect: frame(8, 8),
            color: Rgba8 { r: 10, g: 20, b: 30, a: 128 },
            clip: None,
        }];
        assert!(quality_refuses_stream(RenderQuality::ByteExact, &commands));

        let opaque_clear = [WgpuDrawCommand::Clear { color: opaque(0, 0, 0) }, commands[0].clone()];
        assert!(!quality_refuses_stream(RenderQuality::ByteExact, &opaque_clear));
    }

    /// A translucent clear is not an opaque destination, so it does not satisfy `ByteExact`.
    #[test]
    fn a_translucent_clear_does_not_make_the_destination_opaque() {
        let commands = [WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 128 } }];
        assert!(quality_refuses_stream(RenderQuality::ByteExact, &commands));
    }

    /// A filled rounded rectangle is drawn as the same row spans the CPU fills, so `ByteExact`
    /// must not refuse it when the destination is opaque.
    #[test]
    fn a_filled_rounded_rectangle_is_not_an_edge_feathered_shape() {
        let commands = [
            WgpuDrawCommand::Clear { color: opaque(0, 0, 0) },
            WgpuDrawCommand::FillRoundedRect {
                rect: frame(8, 8),
                radius: 2,
                color: opaque(255, 0, 0),
                clip: None,
            },
        ];
        assert!(!quality_refuses_stream(RenderQuality::ByteExact, &commands));
    }
}
