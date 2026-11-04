// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Explicit adapter from the render layer's commands to the WGPU draw stream.
//!
//! The bridge grows one measured family at a time: a command converts only when the GPU form has
//! been shown to reproduce the reference pixels (or to differ only where
//! `docs/plans/blue29.md` §1.2 permits). `render::present::smooth` classifies every command against
//! the same set, so the audit and this adapter cannot drift apart.

use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
use crate::compat::VecDeque;
use crate::render::RenderCommand;

/// Converts a render-layer rectangle into the backend's pixel rectangle.
///
/// Both types are already in logical pixels at this boundary, so the conversion is a field
/// copy. It exists so the bridge states the coordinate contract once instead of repeating four
/// field names per command.
fn pixel_rect(rect: &crate::core::Rect) -> PixelRect {
    PixelRect { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
}

/// Converts a render-layer colour into the backend's four bytes.
fn rgba8(color: &crate::core::Color) -> Rgba8 {
    Rgba8 { r: color.r, g: color.g, b: color.b, a: color.a }
}

/// Why a command stream has no faithful GPU form.
///
/// Names the first command that could not be converted, so a caller's log says *which* variant is
/// missing instead of "the bridge failed".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeGap {
    /// Index of the first command the bridge cannot convert.
    pub index: usize,
    /// That command's variant name.
    pub variant: &'static str,
}

impl core::fmt::Display for BridgeGap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "command {} ({}) has no faithful GPU form", self.index, self.variant)
    }
}

/// Converts every command in `commands` that has a faithful GPU form, or reports the first one
/// that does not (BLUE29 B29.10).
///
/// `text_scale` is the device scale the commands were recorded at. Text is the one family whose
/// conversion needs it: the shaped run's advances, line box and glyph cells all scale with it, and
/// a bridge that guessed would move every label. Every other family ignores the argument.
///
/// `target` is the frame's pixel size. One family needs it: a conic gradient command carries no
/// rectangle (the reference paints the whole clipped surface), so the bridged quad is the target
/// itself. Every other family ignores the argument.
///
/// # What "faithful" means here
///
/// The conversion must keep the command's pixels, not merely its shape: a filled rectangle becomes
/// a filled rectangle, the clip stack stays a clip stack, and the blend mode stays the blend mode.
/// Anything whose GPU pipeline emits a different picture is **rejected** rather than approximated,
/// because the caller's contract is "either the whole frame is the GPU's, or it is the reference
/// rasteriser's".
///
/// The set is deliberately small and grows one measured family at a time; `render::present::smooth`
/// classifies every [`RenderCommand`] variant against it, so a command this function rejects is
/// reported as `ForcesCpu` with a reason instead of being silently dropped.
pub fn bridge_render_commands(
    commands: &[RenderCommand],
    text_scale: f32,
    target: (u32, u32),
    opaque_background: bool,
) -> Result<Vec<WgpuDrawCommand>, BridgeGap> {
    let mut out = Vec::with_capacity(commands.len());
    bridge_render_commands_into(&mut out, commands, text_scale, target, opaque_background, None)?;
    Ok(out)
}

/// [`bridge_render_commands`] into a buffer the caller keeps across frames (RD-02).
///
/// The `Smooth` present path converts a stream once per presented frame, and the stream is nearly
/// the same as the previous frame's. Handing back a fresh `Vec` there was one heap allocation plus
/// a full regrow per frame; `out` is reused instead, exactly as the reference rasteriser reuses its
/// own buffers.
///
/// `out` is **cleared** first and cleared again on failure, so a caller can never present a
/// partially converted stream: a `BridgeGap` means the whole frame belongs to the reference
/// rasteriser.
///
/// `prefix_clear` is the frame-wide background the recorder began with. It becomes the stream's
/// first command. Pushing it before the conversion is what avoids the `Vec::insert(0, …)` shift of
/// the whole stream the caller used to perform afterwards.
pub fn bridge_render_commands_into(
    out: &mut Vec<WgpuDrawCommand>,
    commands: &[RenderCommand],
    text_scale: f32,
    target: (u32, u32),
    opaque_background: bool,
    prefix_clear: Option<Rgba8>,
) -> Result<(), BridgeGap> {
    out.clear();
    if let Some(color) = prefix_clear {
        out.push(WgpuDrawCommand::Clear { color });
    }
    // Four families are compound: a stroked arc is a sequence of centered lines, a filled arc is
    // the triangle fan the reference fills, a dashed polyline is the same dash kernel's segment
    // list, and a decorated label is the glyph run plus its rules. Expanding them before the
    // one-command-per-entry conversion keeps the rest of the table unchanged; `origins` maps an
    // expanded index back to the caller's command, so a `BridgeGap` still names the command the
    // caller passed.
    let result = if commands.iter().any(is_compound_command) {
        EXPANSION.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            let scratch = &mut *scratch;
            scratch.commands.clear();
            scratch.origins.clear();
            expand_compound_commands(
                commands,
                text_scale,
                &mut scratch.commands,
                &mut scratch.origins,
            );
            convert_commands(
                out,
                &scratch.commands,
                Some(&scratch.origins),
                text_scale,
                target,
                opaque_background,
            )
        })
    } else {
        convert_commands(out, commands, None, text_scale, target, opaque_background)
    };
    if result.is_err() {
        out.clear();
    }
    result
}

/// The expanded stream plus the caller's command index every expanded entry came from.
#[derive(Default)]
struct ExpansionScratch {
    commands: Vec<RenderCommand>,
    origins: Vec<usize>,
}

impl ExpansionScratch {
    /// An empty scratch set. `const` so the thread-local below needs no lazy initialiser.
    const fn new() -> Self {
        Self { commands: Vec::new(), origins: Vec::new() }
    }
}

thread_local! {
    /// Reused by [`bridge_render_commands_into`] (RD-02): one stroked path or arc makes the whole
    /// stream go through expansion, and two freshly allocated vectors per frame is exactly the
    /// cost this buffer removes.
    static EXPANSION: core::cell::RefCell<ExpansionScratch> =
        const { core::cell::RefCell::new(ExpansionScratch::new()) };
}

/// Converts an already-expanded stream, one command per arm.
///
/// Split from [`bridge_render_commands_into`] so the scratch borrow is released before the caller
/// inspects a failure, and so the non-compound path pays nothing for the expansion machinery.
fn convert_commands(
    out: &mut Vec<WgpuDrawCommand>,
    commands: &[RenderCommand],
    origins: Option<&[usize]>,
    text_scale: f32,
    target: (u32, u32),
    opaque_background: bool,
) -> Result<(), BridgeGap> {
    // Plain layers still open. A pop is only faithful while it closes one of its own.
    let mut layers = 0usize;
    // One command per arm, except the three families whose faithful form is a body **plus** a blur:
    // those push two entries, which is why this is a loop rather than a `map`.
    let mut one = |command: &RenderCommand, index: usize| -> Result<WgpuDrawCommand, BridgeGap> {
        match command {
            RenderCommand::FillRect { rect, color } => Ok(WgpuDrawCommand::FillRect {
                rect: pixel_rect(rect),
                color: rgba8(color),
                clip: None,
            }),
            RenderCommand::DrawRect { rect, color } => Ok(WgpuDrawCommand::StrokeRectCentered {
                rect: pixel_rect(rect),
                color: rgba8(color),
                thickness: 1,
                clip: None,
            }),
            RenderCommand::DrawRectStroke { rect, color, width } => {
                Ok(WgpuDrawCommand::StrokeRectCentered {
                    rect: pixel_rect(rect),
                    color: rgba8(color),
                    thickness: *width,
                    clip: None,
                })
            }
            RenderCommand::DrawLine { from, to, color } => Ok(WgpuDrawCommand::DrawLineCentered {
                from: (from.x, from.y),
                to: (to.x, to.y),
                color: rgba8(color),
                width: 1,
                clip: None,
            }),
            RenderCommand::DrawLineStroke { from, to, color, width } => {
                Ok(WgpuDrawCommand::DrawLineCentered {
                    from: (from.x, from.y),
                    to: (to.x, to.y),
                    color: rgba8(color),
                    width: *width,
                    clip: None,
                })
            }
            RenderCommand::DrawRoundedRectStroke { rect, radii, color, width }
            | RenderCommand::DrawRoundedRectStrokeAA { rect, radii, color, width } => {
                if radii.iter().all(|value| *value == radii[0]) {
                    Ok(WgpuDrawCommand::StrokeRoundedRectInside {
                        rect: pixel_rect(rect),
                        radius: radii[0],
                        color: rgba8(color),
                        thickness: *width,
                        clip: None,
                    })
                } else {
                    // The backend's inside-stroke ring carries one radius. A per-corner ring has no
                    // form here yet, so it stays on the reference rasteriser rather than losing
                    // the corner-specific shape.
                    Err(BridgeGap { index, variant: command_variant_name(command) })
                }
            }
            RenderCommand::DrawCircle { center, radius, color } => {
                Ok(WgpuDrawCommand::DrawCircleInside {
                    center: (center.x, center.y),
                    radius: *radius,
                    color: rgba8(color),
                    width: 1,
                    clip: None,
                })
            }
            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                Ok(WgpuDrawCommand::DrawCircleInside {
                    center: (center.x, center.y),
                    radius: *radius,
                    color: rgba8(color),
                    width: *width,
                    clip: None,
                })
            }
            RenderCommand::FillCircle { center, radius, color }
            | RenderCommand::FillCircleAA { center, radius, color } => {
                Ok(WgpuDrawCommand::FillCircle {
                    center: (center.x, center.y),
                    radius: *radius,
                    color: rgba8(color),
                    clip: None,
                })
            }
            RenderCommand::FillRoundedRect { rect, radii, color }
            | RenderCommand::FillRoundedRectAA { rect, radii, color } => {
                // A uniform radius keeps the original single-radius path; a per-corner fill
                // uses the row-span variant, whose radii come from the same CSS resolution the
                // reference applies.
                if radii.iter().all(|value| *value == radii[0]) {
                    Ok(WgpuDrawCommand::FillRoundedRect {
                        rect: pixel_rect(rect),
                        radius: radii[0],
                        color: rgba8(color),
                        clip: None,
                    })
                } else {
                    Ok(WgpuDrawCommand::FillRoundedRectRadii {
                        rect: pixel_rect(rect),
                        radii: *radii,
                        color: rgba8(color),
                        clip: None,
                    })
                }
            }
            RenderCommand::DrawText { origin, text, font, color, alignment } => {
                match crate::render::text::shaped_text_coverage(
                    *origin, text, font, *alignment, text_scale,
                ) {
                    Some(run) => Ok(WgpuDrawCommand::DrawTextCoverage {
                        rect: PixelRect {
                            x: run.x,
                            y: run.y,
                            width: run.width,
                            height: run.height,
                        },
                        coverage: run.bytes,
                        color: rgba8(color),
                        clip: None,
                    }),
                    // A colour glyph has no single-channel form; the caller keeps the frame on the
                    // reference rasteriser rather than losing the glyph's own colours.
                    None => Err(BridgeGap { index, variant: command_variant_name(command) }),
                }
            }
            RenderCommand::DrawImage { x, y, width, height, data } => {
                Ok(WgpuDrawCommand::DrawImage {
                    rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
                    rgba8: crate::compat::Arc::clone(data),
                    image_width: *width,
                    image_height: *height,
                    clip: None,
                })
            }
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                if *filled {
                    // A filled path is one implicitly closed even-odd contour, which is
                    // precisely what the backend's filled-path form draws.
                    Ok(WgpuDrawCommand::DrawPath {
                        points: points
                            .iter()
                            .map(|point| (point.x, point.y))
                            .collect::<Vec<_>>()
                            .into(),
                        closed: *closed,
                        color: rgba8(color),
                        filled: true,
                        width: *width,
                        clip: None,
                    })
                } else {
                    // The stroked form arrives as its segment list (see
                    // `expand_compound_commands`), so it cannot reach this arm.
                    Err(BridgeGap { index, variant: command_variant_name(command) })
                }
            }
            RenderCommand::FillPathNonZero { path, color } => {
                Ok(WgpuDrawCommand::FillPathNonZero {
                    points: path
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect::<Vec<_>>()
                        .into(),
                    contours: path.contours().into(),
                    color: rgba8(color),
                    clip: None,
                })
            }
            RenderCommand::DrawLineAA { from, to, color } => {
                line_coverage_command(*from, *to, *color, 1)
                    .ok_or(BridgeGap { index, variant: command_variant_name(command) })
            }
            RenderCommand::DrawLineStrokeAA { from, to, color, width } => {
                line_coverage_command(*from, *to, *color, *width)
                    .ok_or(BridgeGap { index, variant: command_variant_name(command) })
            }
            RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread }
                if *blur_radius == 0 =>
            {
                // With no blur the reference's arm is exactly `fill_rect` over the offset,
                // spread-adjusted rectangle — there is no shadow pipeline involved, so the
                // bridged form is that same fill.
                let shadow =
                    crate::render::box_shadow_spread_rect(*rect, *offset_x, *offset_y, *spread);
                Ok(WgpuDrawCommand::FillRect {
                    rect: pixel_rect(&shadow),
                    color: rgba8(color),
                    clip: None,
                })
            }
            RenderCommand::BoxShadowRounded {
                rect,
                radii,
                color,
                offset_x,
                offset_y,
                blur_radius,
                spread,
            } if *blur_radius == 0 => {
                // The rounded twin paints `fill_rounded_rect_radii` over the same rectangle; the
                // per-corner variant resolves the radii with the reference's CSS rule.
                let shadow =
                    crate::render::box_shadow_spread_rect(*rect, *offset_x, *offset_y, *spread);
                Ok(WgpuDrawCommand::FillRoundedRectRadii {
                    rect: pixel_rect(&shadow),
                    radii: *radii,
                    color: rgba8(color),
                    clip: None,
                })
            }
            RenderCommand::DrawGradient { rect, gradient } => {
                let ramp = crate::render::gradient_ramp_lut(gradient);
                if !ramp_is_smooth(&ramp) {
                    // One table entry moves more than one level, so a pixel that reads the
                    // neighbouring entry would exceed the class this bridge declares.
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                }
                let Some((kind, start, end)) = gradient_geometry(gradient) else {
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                };
                Ok(WgpuDrawCommand::FillGradient {
                    rect: pixel_rect(rect),
                    kind,
                    start,
                    end,
                    ramp: ramp_bytes_of(&ramp),
                    // The fill arm composites its ramp through the stream's blend mode.
                    verbatim: false,
                    clip: None,
                })
            }
            RenderCommand::DrawConicGradient { center, start_angle, stops } => {
                let ramp = crate::render::ramp_lut_for_stops(stops);
                if !ramp_is_smooth(&ramp) {
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                }
                Ok(WgpuDrawCommand::FillGradient {
                    // The command carries no rectangle: the reference paints the whole clipped
                    // surface, so the bridged quad is the target and the clip does the rest.
                    rect: PixelRect { x: 0, y: 0, width: target.0, height: target.1 },
                    kind: 2,
                    start: (center.x as f32, center.y as f32),
                    end: (*start_angle, 0.0),
                    ramp: ramp_bytes_of(&ramp),
                    // The standalone conic command stores its ramp colour verbatim.
                    verbatim: true,
                    clip: None,
                })
            }
            RenderCommand::DrawImageScaled {
                x,
                y,
                width,
                height,
                source_width,
                source_height,
                data,
            } => {
                // `blit_scaled_software` short-circuits an unscaled draw to `draw_image`, which is
                // the nearest-neighbour form this bridge already reproduces byte-for-byte.
                if *width == *source_width && *height == *source_height {
                    return Ok(WgpuDrawCommand::DrawImage {
                        rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
                        rgba8: crate::compat::Arc::clone(data),
                        image_width: *width,
                        image_height: *height,
                        clip: None,
                    });
                }
                if !scaled_image_drift_is_bounded(*width, *height) {
                    // See `scaled_image_drift_is_bounded`: past this extent the reference's
                    // accumulation of `u` can move a sample by more than one level, which the
                    // class this bridge declares does not allow.
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                }
                // The reference's parallelogram coefficients for a rectangle: `(ux, uy, vx, vy)
                // = (w, 0, 0, h)`, so `inv_det = 1 / (w * h)`, `u_dx = h * inv_det` and
                // `v_dy = w * inv_det`. Computed here exactly as `draw_image_quad` computes
                // them, because the shader compares against that walk.
                let w = *width as f32;
                let h = *height as f32;
                let inv_det = 1.0 / (w * h);
                Ok(WgpuDrawCommand::DrawImageScaled {
                    rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
                    rgba8: crate::compat::Arc::clone(data),
                    image_width: *source_width,
                    image_height: *source_height,
                    u_dx: h * inv_det,
                    v_dy: w * inv_det,
                    clip: None,
                })
            }
            RenderCommand::PushClip { x, y, width, height } => Ok(WgpuDrawCommand::PushClip {
                rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
            }),
            RenderCommand::PopClip => Ok(WgpuDrawCommand::PopClip),
            // A **plain** layer separates nothing: the reference pushes its bounds as a clip and
            // its pop only pops that clip, so the bridged form is the clip pair. `layers` counts
            // the plain pushes still open, so a pop that closes an effect layer (or nothing) is
            // rejected instead of silently popping someone else's clip.
            RenderCommand::PushLayer { bounds, effect } if effect.is_none() => {
                layers += 1;
                Ok(WgpuDrawCommand::PushClip {
                    rect: PixelRect {
                        x: bounds.x,
                        y: bounds.y,
                        width: bounds.width,
                        height: bounds.height,
                    },
                })
            }
            // A layer with no effect and full opacity paints exactly what a clip paints, with or
            // without a replay cache: the cache only saves the reference rasteriser work, and
            // `composite_foreground` is the identity over an **opaque** backdrop (every captured
            // pixel then has alpha 255, so the composite writes the frame back verbatim). On a
            // translucent frame that stops being true, so the bridge declines instead.
            RenderCommand::PushLayerGroup { bounds, effect, opacity, .. }
                if crate::render::layer_paints_like_a_clip(*effect, *opacity) =>
            {
                if !opaque_background {
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                }
                layers += 1;
                Ok(WgpuDrawCommand::PushClip {
                    rect: PixelRect {
                        x: bounds.x,
                        y: bounds.y,
                        width: bounds.width,
                        height: bounds.height,
                    },
                })
            }
            RenderCommand::PopLayer => {
                if layers == 0 {
                    return Err(BridgeGap { index, variant: command_variant_name(command) });
                }
                layers -= 1;
                Ok(WgpuDrawCommand::PopClip)
            }
            RenderCommand::SetBlendMode { mode } => {
                Ok(WgpuDrawCommand::SetBlendMode { mode: *mode as u8 })
            }
            other => Err(BridgeGap { index, variant: command_variant_name(other) }),
        }
    };
    for (expanded_index, command) in commands.iter().enumerate() {
        let index = origins.map_or(expanded_index, |map| map[expanded_index]);
        match command {
            // A blur is the frame region itself: the reference blurs whatever is already painted
            // there, so the bridged form is the region's own read-modify-write.
            RenderCommand::Blur { radius } => {
                let radius = (*radius).min(100);
                if radius > 0 {
                    out.push(WgpuDrawCommand::BlurRegion {
                        rect: PixelRect { x: 0, y: 0, width: target.0, height: target.1 },
                        radius,
                        clip: None,
                    });
                }
            }
            // A blurred shadow is the reference's own two steps: fill the offset body, then blur that
            // rectangle in the frame. The blur must see the body, so the pair is emitted in order.
            RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread }
                if *blur_radius > 0 =>
            {
                let shadow =
                    crate::render::box_shadow_spread_rect(*rect, *offset_x, *offset_y, *spread);
                out.push(WgpuDrawCommand::FillRect {
                    rect: pixel_rect(&shadow),
                    color: rgba8(color),
                    clip: None,
                });
                out.push(WgpuDrawCommand::BlurRegion {
                    rect: pixel_rect(&shadow),
                    radius: (*blur_radius).min(100),
                    clip: None,
                });
            }
            RenderCommand::BoxShadowRounded {
                rect,
                radii,
                color,
                offset_x,
                offset_y,
                blur_radius,
                spread,
            } if *blur_radius > 0 => {
                let shadow =
                    crate::render::box_shadow_spread_rect(*rect, *offset_x, *offset_y, *spread);
                out.push(WgpuDrawCommand::FillRoundedRectRadii {
                    rect: pixel_rect(&shadow),
                    radii: *radii,
                    color: rgba8(color),
                    clip: None,
                });
                out.push(WgpuDrawCommand::BlurRegion {
                    rect: pixel_rect(&shadow),
                    radius: (*blur_radius).min(100),
                    clip: None,
                });
            }
            other => out.push(one(other, index)?),
        }
    }
    Ok(())
}

/// Whether `command`'s faithful GPU form is more than one command.
///
/// One predicate decides both whether the stream is expanded at all and which arms
/// [`expand_compound_commands`] implements, so the two cannot drift apart: an arm added there and
/// forgotten here would silently reach the one-command table and be rejected.
fn is_compound_command(command: &RenderCommand) -> bool {
    matches!(
        command,
        RenderCommand::DrawArc { filled: false, .. }
            | RenderCommand::DrawArc { filled: true, .. }
            | RenderCommand::DrawTextDecorated { .. }
            | RenderCommand::DrawPathDashed { .. }
            | RenderCommand::DrawPath { filled: false, .. }
            // Not 1:N, but it *can* be 1:0: a scaled draw the reference skips must not reach the
            // table as a command the dispatcher cannot claim.
            | RenderCommand::DrawImageScaled { .. }
    )
}

/// Expands the commands whose faithful GPU form is more than one command.
///
/// Writes the expanded stream into `expanded` and, for every entry in it, the index of the caller's
/// command that produced it into `origins`. Both buffers are cleared by the caller and keep their
/// capacity across frames (RD-02). A zero-sweep arc expands to nothing (the reference draws
/// nothing) and therefore has no entry; an index that never appears cannot be the first gap, which
/// is the correct answer for a command that draws nothing.
fn expand_compound_commands(
    commands: &[RenderCommand],
    text_scale: f32,
    expanded: &mut Vec<RenderCommand>,
    origins: &mut Vec<usize>,
) {
    for (index, command) in commands.iter().enumerate() {
        let before = expanded.len();
        match command {
            RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle,
                color,
                filled: false,
            } => {
                push_arc_lines(*center, *radius, *start_angle, *end_angle, *color, expanded);
            }
            RenderCommand::DrawArc {
                center,
                radius,
                start_angle,
                end_angle,
                color,
                filled: true,
            } => {
                push_filled_arc(*center, *radius, *start_angle, *end_angle, *color, expanded);
            }
            RenderCommand::DrawPath { points, closed, color, filled: false, width } => {
                // `SoftwareSurface::draw_path` strokes one segment per edge with its centred
                // brush, and closes the outline when asked; each segment becomes the
                // `DrawLineStroke` the bridge already reproduces byte-for-byte. Fewer than two
                // points draws nothing in both paths.
                if points.len() >= 2 {
                    for index in 0..points.len().saturating_sub(1) {
                        expanded.push(RenderCommand::DrawLineStroke {
                            from: points[index],
                            to: points[index + 1],
                            color: *color,
                            width: *width,
                        });
                    }
                    if *closed {
                        expanded.push(RenderCommand::DrawLineStroke {
                            from: points[points.len() - 1],
                            to: points[0],
                            color: *color,
                            width: *width,
                        });
                    }
                }
            }
            // A scaled draw the reference skips draws nothing: an empty extent, an empty source, or
            // a payload shorter than the source grid. Dropping it here keeps the stream exactly as
            // drawable as it was, instead of replacing it with a zero-sized command the dispatcher
            // would refuse and turning one missing pixel into a whole CPU frame.
            RenderCommand::DrawImageScaled {
                width,
                height,
                source_width,
                source_height,
                data,
                ..
            } if *width == 0
                || *height == 0
                || *source_width == 0
                || *source_height == 0
                || data.len() < (*source_width as usize) * (*source_height as usize) * 4 => {}
            RenderCommand::DrawImageScaled { .. } => expanded.push(command.clone()),
            RenderCommand::DrawPathDashed { points, closed, color, width, dash } => {
                // One dash kernel, shared with the reference's immediate path: the polyline is
                // broken into the same segments, and each becomes the `DrawLineStroke` the centred
                // brush converts byte-for-byte. A zero width draws nothing in both paths.
                if *width > 0 && points.len() >= 2 {
                    crate::render::for_each_dash_segment(points, *closed, *dash, |from, to| {
                        expanded.push(RenderCommand::DrawLineStroke {
                            from,
                            to,
                            color: *color,
                            width: *width,
                        });
                    });
                }
            }
            RenderCommand::DrawTextDecorated {
                origin,
                text,
                font,
                color,
                alignment,
                decoration,
            } => {
                // The glyphs first, exactly as the reference does; the rules then follow its
                // metrics and placement formulas.
                expanded.push(RenderCommand::DrawText {
                    origin: *origin,
                    text: crate::compat::Arc::clone(text),
                    font: font.clone(),
                    color: *color,
                    alignment: *alignment,
                });
                push_decoration_lines(
                    *origin,
                    text,
                    font,
                    *color,
                    *alignment,
                    *decoration,
                    text_scale,
                    expanded,
                );
            }
            _ => expanded.push(command.clone()),
        }
        for _ in before..expanded.len() {
            origins.push(index);
        }
    }
}

/// Appends the reference's decoration rules for a label.
///
/// This mirrors `draw_text_decorated_on_surface` line for line: the same line box, the same
/// rounded advance, the same thickness and offsets, and the same triangle wave. Each rule is a
/// `DrawLineStroke`, which the centred brush bridges byte-for-byte.
#[allow(clippy::too_many_arguments)]
fn push_decoration_lines(
    origin: crate::core::Point,
    text: &str,
    font: &crate::core::Font,
    color: crate::core::Color,
    alignment: crate::core::HorizontalAlignment,
    decoration: crate::render::TextDecoration,
    scale: f32,
    out: &mut Vec<RenderCommand>,
) {
    use crate::core::Point;
    use crate::render::TextDecoration;
    if decoration.is_empty() {
        return;
    }
    let metrics = crate::render::TextMetrics::for_font(font, scale);
    let advance =
        crate::render::text::shape_line(text, font, scale).advance().round().max(0.0) as u32;
    if advance == 0 {
        return;
    }
    let start_x = match alignment {
        crate::core::HorizontalAlignment::Left => origin.x,
        crate::core::HorizontalAlignment::Center => origin.x - (advance / 2) as i32,
        crate::core::HorizontalAlignment::Right => origin.x - advance as i32,
    };
    let end_x = start_x + advance as i32;
    let baseline = origin.y + metrics.ascent as i32;
    let thickness = ((font.size() * scale) / 12.0).round().max(1.0) as u32;
    let underline_offset = (metrics.descent as i32 / 2).max(1);
    let push_rule = |out: &mut Vec<RenderCommand>, y: i32| {
        out.push(RenderCommand::DrawLineStroke {
            from: Point::new(start_x, y),
            to: Point::new(end_x, y),
            color,
            width: thickness,
        });
    };
    if decoration.contains(TextDecoration::UNDERLINE.0) {
        push_rule(out, baseline + underline_offset);
    }
    if decoration.contains(TextDecoration::STRIKETHROUGH.0) {
        push_rule(out, origin.y + (metrics.ascent / 2) as i32);
    }
    if decoration.contains(TextDecoration::WAVY.0) {
        let crest = baseline + underline_offset;
        let trough = baseline + underline_offset * 2;
        let mut x = start_x;
        let mut low = false;
        while x < end_x {
            let next = (x + 2).min(end_x);
            let (from_y, to_y) = if low { (trough, crest) } else { (crest, trough) };
            out.push(RenderCommand::DrawLineStroke {
                from: Point::new(x, from_y),
                to: Point::new(next, to_y),
                color,
                width: thickness,
            });
            x = next;
            low = !low;
        }
    }
}

/// Appends the reference's line sequence for a stroked arc.
///
/// This mirrors `SoftwareSurface::draw_arc` exactly: the sweep is normalised by the same helper,
/// the samples come from the same `for_each_arc_sample` kernel, and a full turn closes its last
/// chord explicitly. The resulting `DrawLine` commands are bridged by the centred brush, which is
/// byte-identical to the reference's `draw_line_with_width(.., 1)`.
fn push_arc_lines(
    center: crate::core::Point,
    radius: u32,
    start_angle: f32,
    end_angle: f32,
    color: crate::core::Color,
    out: &mut Vec<RenderCommand>,
) {
    use crate::core::Point;
    let (start, sweep) = crate::render::arc_helpers::normalized_arc_sweep(start_angle, end_angle);
    if sweep < 0.001 || radius == 0 {
        return;
    }
    let end = start + sweep;
    let full_turn = sweep >= core::f32::consts::TAU - 0.001;
    let mut first: Option<Point> = None;
    let mut previous: Option<Point> = None;
    crate::render::arc_helpers::for_each_arc_sample(center, radius as f32, start, end, |current| {
        if first.is_none() {
            first = Some(current);
        }
        if let Some(previous) = previous {
            out.push(RenderCommand::DrawLine { from: previous, to: current, color });
        }
        previous = Some(current);
    });
    if full_turn {
        if let (Some(first), Some(previous)) = (first, previous) {
            if first != previous {
                out.push(RenderCommand::DrawLine { from: previous, to: first, color });
            }
        }
    }
}

/// Appends the reference's triangle-fan form of a filled arc.
///
/// `SoftwareSurface::draw_arc` fills one triangle per chord sample, `(center, previous, current)`.
/// Those triangles are exactly a fan over the polygon `center, p0, p1, … pn`, so the bridged form
/// is one non-zero contour with the centre as its first vertex: the GPU's fan triangulation then
/// reproduces the reference's triangle set instead of an approximation of it.
///
/// A full turn closes its last chord explicitly, the same way the stroked form does, so the fan
/// has neither a one-chord gap nor a missing wedge. A sweep that samples fewer than two points
/// draws nothing in the reference (no chord, no triangle), so it expands to nothing.
fn push_filled_arc(
    center: crate::core::Point,
    radius: u32,
    start_angle: f32,
    end_angle: f32,
    color: crate::core::Color,
    out: &mut Vec<RenderCommand>,
) {
    use crate::core::Point;
    let (start, sweep) = crate::render::arc_helpers::normalized_arc_sweep(start_angle, end_angle);
    if sweep < 0.001 || radius == 0 {
        return;
    }
    let end = start + sweep;
    let full_turn = sweep >= core::f32::consts::TAU - 0.001;
    // Sample straight into the vertex list: the previous form built an intermediate `Vec<Point>`
    // and copied it into a second one (RD-02). The centre is the first vertex of the fan.
    let mut points: Vec<Point> = Vec::with_capacity(32);
    points.push(center);
    let mut first_sample: Option<Point> = None;
    let mut last_sample: Option<Point> = None;
    let mut sample_count = 0usize;
    crate::render::arc_helpers::for_each_arc_sample(center, radius as f32, start, end, |current| {
        if first_sample.is_none() {
            first_sample = Some(current);
        }
        last_sample = Some(current);
        sample_count += 1;
        points.push(current);
    });
    if sample_count < 2 {
        return;
    }
    if full_turn {
        if let (Some(first), Some(last)) = (first_sample, last_sample) {
            if last != first {
                points.push(first);
            }
        }
    }
    let vertex_count = points.len();
    let Ok(path) = crate::render::CompoundPath::new(points, vec![(0, vertex_count)]) else {
        return;
    };
    out.push(RenderCommand::FillPathNonZero { path, color });
}

/// The largest coverage box a bridged antialiased stroke may allocate, in pixels.
///
/// The reference clamps its coverage sweep to the framebuffer it draws into; this bridge runs
/// before any framebuffer is known, so the box is the segment's own extent. A pathologically long
/// segment would therefore ask for a whole-frame mask, which is exactly the case where a mask is
/// the wrong tool: past this bound the command reports a gap and the frame stays on the reference
/// rasteriser instead of allocating a mask that cannot be worth it.
const LINE_COVERAGE_LIMIT: i64 = 4 * 1024 * 1024;

/// The reference's own coverage for an antialiased segment, as an 8-bit mask command.
///
/// `SoftwareSurface::draw_line_aa_with_width` visits every pixel of the segment's padded box and
/// blends `color` with the sample-grid coverage `line_stroke_coverage_grid` returns. This function
/// reuses that same kernel and quantises its coverage to one byte per pixel, which the coverage
/// carrier composites exactly as the reference's blend does — so the difference is the byte's
/// quantisation, not a different shape.
///
/// Returns `None` when no faithful mask exists: a zero-width stroke and a box past
/// [`LINE_COVERAGE_LIMIT`].
fn line_coverage_command(
    from: crate::core::Point,
    to: crate::core::Point,
    color: crate::core::Color,
    width: u32,
) -> Option<WgpuDrawCommand> {
    let (rect, coverage) = line_coverage_mask(from, to, width)?;
    Some(WgpuDrawCommand::DrawTextCoverage { rect, coverage, color: rgba8(&color), clip: None })
}

fn line_coverage_mask(
    from: crate::core::Point,
    to: crate::core::Point,
    width: u32,
) -> Option<(PixelRect, crate::compat::Arc<[u8]>)> {
    if width == 0 {
        return Some((
            PixelRect { x: from.x, y: from.y, width: 0, height: 0 },
            crate::compat::Arc::from([]),
        ));
    }
    let grid = crate::render::default_software_render_config().aa_samples_per_axis;
    let half = width as f32 / 2.0;
    let pad = half.ceil() as i32 + 1;
    let min_x = from.x.min(to.x).saturating_sub(pad);
    let max_x = from.x.max(to.x).saturating_add(pad);
    let min_y = from.y.min(to.y).saturating_sub(pad);
    let max_y = from.y.max(to.y).saturating_add(pad);
    let box_width = i64::from(max_x) - i64::from(min_x) + 1;
    let box_height = i64::from(max_y) - i64::from(min_y) + 1;
    if box_width <= 0 || box_height <= 0 || box_width * box_height > LINE_COVERAGE_LIMIT {
        return None;
    }
    let mut bytes = vec![0u8; (box_width * box_height) as usize];
    let (ax, ay) = (from.x as f32, from.y as f32);
    let (bx, by) = (to.x as f32, to.y as f32);
    for row in 0..box_height {
        let py = min_y + row as i32;
        for column in 0..box_width {
            let px = min_x + column as i32;
            let coverage =
                crate::render::line_stroke_coverage_grid(px, py, ax, ay, bx, by, half, grid);
            if coverage > 0.0 {
                // The carrier multiplies the colour by `byte / 255`, so this is the closest byte
                // to the reference's fractional coverage.
                bytes[(row * box_width + column) as usize] =
                    (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    Some((
        PixelRect { x: min_x, y: min_y, width: box_width as u32, height: box_height as u32 },
        crate::compat::Arc::from(bytes),
    ))
}

/// The geometry a `RenderCommand::DrawGradient` maps onto, as `(kind, start, end)` in the
/// [`WgpuDrawCommand::FillGradient`] encoding, or `None` for a geometry this carrier has no kind
/// for.
fn gradient_geometry(gradient: &crate::style::Gradient) -> Option<(u8, (f32, f32), (f32, f32))> {
    use crate::style::GradientType;
    let point = |p: crate::core::Point| (p.x as f32, p.y as f32);
    match gradient.gradient_type {
        GradientType::Linear => Some((0, point(gradient.start_point), point(gradient.end_point))),
        GradientType::Radial => Some((1, point(gradient.center), (gradient.radius, 0.0))),
        GradientType::Conic => {
            // The reference converts the sweep start from degrees once per fill.
            Some((2, point(gradient.center), (gradient.angle.to_radians(), 0.0)))
        }
    }
}

/// Whether a ramp's neighbouring entries differ by at most one level in every channel.
///
/// The bridge uploads the reference's **own** table, so the colours agree by construction; the one
/// thing it cannot guarantee is *which* of two neighbouring entries a pixel reads, because the ramp
/// position is an `f32` expression evaluated on a different unit and can land on either side of a
/// bucket boundary. The plan's allowed difference for that is "≤1 LSB", which holds exactly when
/// neighbouring entries differ by at most one level — so a ramp that jumps further has no bridged
/// form and the caller reports it instead of quietly exceeding its declared divergence.
///
/// `render::present::smooth`'s audit reads this same predicate, so the census and the conversion
/// cannot disagree about which ramps are bridgeable.
pub fn ramp_is_smooth(ramp: &[crate::core::Color; 256]) -> bool {
    ramp.windows(2).all(|pair| {
        let (low, high) = (pair[0], pair[1]);
        low.r.abs_diff(high.r) <= 1
            && low.g.abs_diff(high.g) <= 1
            && low.b.abs_diff(high.b) <= 1
            && low.a.abs_diff(high.a) <= 1
    })
}

/// The reference's own 256-entry ramp for `gradient`, as `RGBA8` bytes.
///
/// The conversion is memoised by ramp content: a gradient keeps the same ramp across frames, so a
/// scene with a gradient present used to rebuild (and re-allocate) the same 1024 bytes every
/// present (RD-02). The cache is tiny and exact — it compares the full entry table rather than a
/// hash, so a collision cannot return another ramp's bytes.
fn ramp_bytes_of(lut: &[crate::core::Color; 256]) -> crate::compat::Arc<[u8]> {
    /// How many recent ramps are kept. A scene has a handful of gradients at most.
    const RAMP_CACHE_LIMIT: usize = 8;
    thread_local! {
        static RAMP_CACHE: core::cell::RefCell<
            VecDeque<([crate::core::Color; 256], crate::compat::Arc<[u8]>)>,
        > = const { core::cell::RefCell::new(VecDeque::new()) };
    }
    RAMP_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, bytes)) = cache.iter().find(|(stored, _)| stored == lut) {
            return crate::compat::Arc::clone(bytes);
        }
        let mut bytes = Vec::with_capacity(256 * 4);
        for color in lut.iter() {
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        let shared: crate::compat::Arc<[u8]> = crate::compat::Arc::from(bytes);
        if cache.len() >= RAMP_CACHE_LIMIT {
            cache.pop_front();
        }
        cache.push_back((*lut, crate::compat::Arc::clone(&shared)));
        shared
    })
}

/// The largest destination extent whose bilinear sample position is provably within one level of the
/// reference's.
///
/// A scaled draw's normalized sample position is accumulated per pixel by the reference
/// (`u += u_dx`) and evaluated directly by the shader. `u` stays inside `0..1`, so one addition
/// rounds by at most `3e-8`, and the walk is at most `extent` steps long — a worst-case drift of
/// `extent * 3e-8`. Bilinear sampling is **continuous** in `(u, v)`: crossing a texel boundary swaps
/// the texel pair and the weight together, so that drift changes the interpolated colour by
/// `255 * drift` levels. Holding `drift <= 1e-4` keeps the result inside one level, which is the
/// class this bridge declares; past this extent the command stays on the reference rasteriser.
pub fn scaled_image_drift_is_bounded(width: u32, height: u32) -> bool {
    const MAX_DRIFT: f32 = 1.0e-4;
    const DRIFT_PER_PIXEL: f32 = 3.0e-8;
    (width.max(height) as f32) * DRIFT_PER_PIXEL <= MAX_DRIFT
}

/// The variant name of `command`, for a [`BridgeGap`].
///
/// Spelled from a match rather than `Debug` so a gap line cannot start naming struct fields.
fn command_variant_name(command: &RenderCommand) -> &'static str {
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

/// Converts non-zero fills and their clip/blend state into the retained WGPU command format.
///
/// Geometry and clip coordinates are already expressed in the render command's pixel space;
/// conversion preserves them verbatim. The retained WGPU representation owns its vertices and
/// contour ranges, so this adapter necessarily copies those arrays. Commands outside this narrow
/// subset are rejected instead of being silently omitted or approximated.
///
/// This entry point predates [`bridge_render_commands`] and keeps its narrower contract for the
/// callers that want the non-zero-fill subset only; the wider adapter is the one the `Smooth`
/// path uses. Neither is a GPU rasterizer: the drawing happens in
/// [`super::WgpuRenderer::render_draw_commands_rgba8`] and the `Smooth` presenter.
pub fn bridge_non_zero_fill_commands(
    commands: &[RenderCommand],
) -> Result<Vec<WgpuDrawCommand>, String> {
    commands
        .iter()
        .enumerate()
        .map(|(index, command)| match command {
            RenderCommand::FillPathNonZero { path, color } => {
                Ok(WgpuDrawCommand::FillPathNonZero {
                    points: path
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect::<Vec<_>>()
                        .into(),
                    contours: path.contours().into(),
                    color: Rgba8 { r: color.r, g: color.g, b: color.b, a: color.a },
                    clip: None,
                })
            }
            RenderCommand::PushClip { x, y, width, height } => Ok(WgpuDrawCommand::PushClip {
                rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
            }),
            RenderCommand::PopClip => Ok(WgpuDrawCommand::PopClip),
            RenderCommand::SetBlendMode { mode } => {
                Ok(WgpuDrawCommand::SetBlendMode { mode: *mode as u8 })
            }
            _ => Err(format!(
                "RenderCommand at index {index} is unsupported by the WGPU non-zero-fill bridge"
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::bridge_non_zero_fill_commands;
    use crate::core::{Color, Point};
    use crate::render::{CompoundPath, RenderCommand};
    use crate::wgpu_backend::raster::rasterize_draw_commands_rgba8;

    #[test]
    fn bridge_preserves_winding_clip_order_and_blend_state() {
        let path = CompoundPath::new(
            vec![
                Point::new(0, 0),
                Point::new(16, 0),
                Point::new(16, 16),
                Point::new(0, 16),
                Point::new(4, 4),
                Point::new(4, 12),
                Point::new(12, 12),
                Point::new(12, 4),
            ],
            vec![(0, 4), (4, 8)],
        )
        .expect("valid outer contour and oppositely wound hole");
        let commands = vec![
            RenderCommand::PushClip { x: 0, y: 0, width: 8, height: 16 },
            RenderCommand::SetBlendMode { mode: crate::render::BlendMode::Normal },
            RenderCommand::FillPathNonZero { path, color: Color::rgba(220, 30, 40, 255) },
            RenderCommand::PopClip,
        ];

        let bridged = bridge_non_zero_fill_commands(&commands).expect("supported command subset");
        let pixels = rasterize_draw_commands_rgba8(16, 16, &bridged).expect("valid WGPU commands");
        let pixel = |x: u32, y: u32| {
            let offset = ((y * 16 + x) * 4) as usize;
            [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
        };

        assert_eq!(pixel(2, 8), [220, 30, 40, 255], "outer fill remains visible in clip");
        assert_eq!(pixel(6, 8), [0, 0, 0, 0], "opposite winding preserves the hole");
        assert_eq!(pixel(10, 2), [0, 0, 0, 0], "clip limits the converted command");
    }

    #[test]
    fn bridge_rejects_commands_outside_its_declared_subset() {
        let unsupported = RenderCommand::FillRect {
            rect: crate::core::Rect::new(0, 0, 2, 2),
            color: Color::BLACK,
        };
        let error = bridge_non_zero_fill_commands(&[unsupported]).expect_err("must not drop input");
        assert!(error.contains("index 0"));
        assert!(error.contains("unsupported"));
    }
}
