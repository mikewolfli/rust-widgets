// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Frame widget.
use crate::compat::{format, Rc, RefCell, ToString};
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;

use crate::render::{Bevel, BevelDirection};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, InteractionFade, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How far the border moves toward its contrasting tone at full interaction strength.
///
/// Named rather than inlined because the draw path and its tests read the same value; at
/// rest the tint is skipped entirely, so this only describes the hovered/pressed picture.
const FRAME_INTERACTION_TINT: f32 = 0.22;

/// Frame widget.
pub struct Frame {
    base: BaseWidget,
    frame_shape: FrameShape,
    frame_shadow: FrameShadow,
    line_width: f32,
    mid_line_width: f32,
    /// The frame's drawn width, when it is not the same as [`Frame::line_width`].
    ///
    /// The reference toolkit's `frameWidth` is the width the frame actually paints and
    /// `lineWidth` is the style-sheet spelling of the same fact; keeping them as two fields means
    /// a caller that sets one does not silently rewrite the other, and the painter reads the
    /// effective value through [`Frame::frame_width`].
    frame_width: Option<f32>,
    /// Space kept between the frame's own rectangle and its content.
    ///
    /// Zero by default, so the historical behaviour — the child draws wherever its own geometry
    /// says — is unchanged until a caller asks for margins.
    contents_margins: EdgeOffsets,
    /// Whether the bevel's two tones come from the theme's surface instead of fixed greys.
    ///
    /// Off by default: the fixed pair is what the exported `frame_*` snapshots contain, and a
    /// visual change behind a flag is the only way to keep those byte-identical while still
    /// offering the theme-derived expression.
    theme_bevel: bool,
    widget: Option<ObjectId>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// The hover/press fade that tints the outline while the pointer interacts.
    ///
    /// Both drivers rest at exactly `0.0`, so a frame that has never been pointed at paints
    /// its declared border byte-for-byte (see `Frame::interactive_border`).
    fade: InteractionFade,
}
/// Frame shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShape {
    /// No frame
    NoFrame,
    /// Box frame
    #[default]
    Box,
    /// Panel frame
    Panel,
    /// Styled panel frame
    StyledPanel,
    /// HLine frame
    HLine,
    /// VLine frame
    VLine,
    /// WinPanel frame
    WinPanel,
}

impl FrameShape {
    /// The factory spelling of this shape, matching `FRAME_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameShape::NoFrame => "no_frame",
            FrameShape::Box => "box",
            FrameShape::Panel => "panel",
            FrameShape::StyledPanel => "styled_panel",
            FrameShape::HLine => "hline",
            FrameShape::VLine => "vline",
            FrameShape::WinPanel => "win_panel",
        }
    }

    /// Parses a factory spelling into a shape.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "no_frame" => FrameShape::NoFrame,
            "box" => FrameShape::Box,
            "panel" => FrameShape::Panel,
            "styled_panel" => FrameShape::StyledPanel,
            "hline" => FrameShape::HLine,
            "vline" => FrameShape::VLine,
            "win_panel" => FrameShape::WinPanel,
            _ => return None,
        })
    }
}
/// Frame shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameShadow {
    /// Plain shadow
    #[default]
    Plain,
    /// Raised shadow
    Raised,
    /// Sunken shadow
    Sunken,
}

impl FrameShadow {
    /// The factory spelling of this shadow, matching `FRAME_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameShadow::Plain => "plain",
            FrameShadow::Raised => "raised",
            FrameShadow::Sunken => "sunken",
        }
    }

    /// Parses a factory spelling into a shadow.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "plain" => FrameShadow::Plain,
            "raised" => FrameShadow::Raised,
            "sunken" => FrameShadow::Sunken,
            _ => return None,
        })
    }
}
/// Parses a `top,right,bottom,left` margin string.
///
/// The same spelling [`format_margins`] writes, so the property round-trips. A missing or extra
/// component is a parse failure rather than a defaulted side: silently treating `"4"` as four
/// equal sides would hide a malformed document behind a plausible picture.
fn parse_margins(text: &str) -> Option<EdgeOffsets> {
    let mut parts = text.split(',');
    let mut side = || -> Option<u32> { parts.next()?.trim().parse().ok() };
    let top = side()?;
    let right = side()?;
    let bottom = side()?;
    let left = side()?;
    if parts.next().is_some() {
        return None;
    }
    Some(EdgeOffsets { top, right, bottom, left })
}

/// Writes a margin as `top,right,bottom,left`, the inverse of [`parse_margins`].
fn format_margins(margins: EdgeOffsets) -> crate::compat::String {
    format!("{},{},{},{}", margins.top, margins.right, margins.bottom, margins.left)
}

/// The corner radius a **stroke** should follow.
///
/// A theme's own default radius belongs to its fill treatment, not to the frame's rule: the
/// exported frame appearances paint a rounded panel face with a square border, and that is what
/// the committed snapshots contain. A radius the caller declared explicitly (so the style is no
/// longer theme-derived) is a statement about the shape and is honoured for the stroke too.
fn stroke_radius(style: &crate::style::WidgetStyle) -> u32 {
    if style.theme_derived {
        0
    } else {
        style.border_radius.unwrap_or(0)
    }
}

impl Frame {
    /// Creates a frame.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Frame, geometry, "Frame"),
            frame_shape: FrameShape::Box,
            frame_shadow: FrameShadow::Plain,
            line_width: 1.0,
            mid_line_width: 0.0,
            frame_width: None,
            contents_margins: EdgeOffsets::all(0),
            theme_bevel: false,
            widget: None,
            registry: None,
            fade: InteractionFade::new(),
        }
    }
    /// Sets the shared widget registry for child forwarding.
    ///
    /// A registry that is already installed (the same allocation) returns early: the field is a
    /// handle, so re-storing an equal handle cannot change what any lookup answers.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        if self.registry.as_ref().is_some_and(|current| Rc::ptr_eq(current, &registry)) {
            return;
        }
        self.registry = Some(registry);
        self.base.request_redraw();
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Returns frame shape.
    pub fn frame_shape(&self) -> FrameShape {
        self.frame_shape
    }
    /// Sets frame shape.
    pub fn set_frame_shape(&mut self, shape: FrameShape) {
        if self.frame_shape == shape {
            return;
        }
        self.frame_shape = shape;
        self.base.request_redraw();
    }
    /// Returns frame shadow.
    pub fn frame_shadow(&self) -> FrameShadow {
        self.frame_shadow
    }
    /// Sets frame shadow.
    pub fn set_frame_shadow(&mut self, shadow: FrameShadow) {
        if self.frame_shadow == shadow {
            return;
        }
        self.frame_shadow = shadow;
        self.base.request_redraw();
    }
    /// Returns line width.
    pub fn line_width(&self) -> f32 {
        self.line_width
    }
    /// Sets line width.
    ///
    /// The stored value is floored at zero first, so `-1.0` and `0.0` are the same write and the
    /// second is a no-op.
    pub fn set_line_width(&mut self, width: f32) {
        let width = width.max(0.0);
        if self.line_width == width {
            return;
        }
        self.line_width = width;
        self.base.request_redraw();
    }
    /// Returns mid line width.
    pub fn mid_line_width(&self) -> f32 {
        self.mid_line_width
    }
    /// Sets mid line width.
    ///
    /// Floored at zero first, for the same reason as [`Frame::set_line_width`].
    pub fn set_mid_line_width(&mut self, width: f32) {
        let width = width.max(0.0);
        if self.mid_line_width == width {
            return;
        }
        self.mid_line_width = width;
        self.base.request_redraw();
    }
    /// The width the frame actually paints.
    ///
    /// Returns the explicit [`Frame::set_frame_width`] value when one was set, and the line width
    /// otherwise — so a frame whose caller never distinguished the two behaves exactly as before.
    pub fn frame_width(&self) -> f32 {
        self.frame_width.unwrap_or(self.line_width)
    }

    /// Sets the width the frame paints.
    ///
    /// Floored at zero first, so a negative width is the same write as zero.
    pub fn set_frame_width(&mut self, width: f32) {
        let width = width.max(0.0);
        if self.frame_width == Some(width) {
            return;
        }
        self.frame_width = Some(width);
        self.base.request_redraw();
    }

    /// The integer stroke width the painter uses.
    ///
    /// Rounded rather than truncated: a caller who asks for `1.5` pixels means "two-pixel-looking",
    /// and the renderer's stroke width is an integer. A width that rounds to zero paints no frame,
    /// which is what `frameWidth(0)` means.
    fn painted_width(&self) -> u32 {
        self.frame_width().round().max(0.0) as u32
    }

    /// The space kept between the frame's rectangle and its content.
    pub fn contents_margins(&self) -> EdgeOffsets {
        self.contents_margins
    }

    /// Sets the space between the frame and its content.
    ///
    /// The margin is subtracted from the frame's own rectangle when the child is positioned, so a
    /// caller can reserve the frame's chrome without computing the inset itself. A non-zero margin
    /// moves the registered child through [`crate::widget::SimpleRegistry::set_widget_geometry`];
    /// zero leaves the child's geometry untouched (the historical behaviour).
    pub fn set_contents_margins(&mut self, margins: EdgeOffsets) {
        if self.contents_margins == margins {
            return;
        }
        self.contents_margins = margins;
        self.relayout_contents();
        self.base.request_redraw();
    }

    /// The rectangle a child of this frame occupies: the frame's geometry inset by the margins.
    pub fn contents_rect(&self) -> Rect {
        ControlMetrics::content_box(self.geometry(), self.contents_margins)
    }

    /// Whether the bevel's tones are derived from the active theme.
    pub fn theme_bevel(&self) -> bool {
        self.theme_bevel
    }

    /// Selects theme-derived bevel tones instead of the fixed light/dark pair.
    ///
    /// The derived highlights are the active theme's background stepped 50% toward white and its
    /// shade stepped 50% toward black. That is the same relationship the fixed pair expresses, but
    /// taken from the surface the frame is actually on, so a custom palette gets a bevel that
    /// belongs to it rather than two greys that only suit a light one.
    ///
    /// Opt-in because the fixed pair is what the exported frame snapshots contain.
    pub fn set_theme_bevel(&mut self, themed: bool) {
        if self.theme_bevel == themed {
            return;
        }
        self.theme_bevel = themed;
        self.base.request_redraw();
    }

    /// Positions the registered child inside [`Frame::contents_rect`].
    ///
    /// A frame with zero margins keeps the child where it was: moving it by "geometry minus zero"
    /// would be a no-op for an already-correct child but a silent move for one that the caller had
    /// deliberately offset.
    fn relayout_contents(&mut self) {
        let margins = self.contents_margins;
        if margins.top == 0 && margins.right == 0 && margins.bottom == 0 && margins.left == 0 {
            return;
        }
        let (Some(child), Some(registry)) = (self.widget, self.registry.as_ref()) else {
            return;
        };
        let contents = self.contents_rect();
        registry.borrow_mut().set_widget_geometry(child, contents);
    }

    /// The bevel tones the painter should use.
    ///
    /// Returns `None` when the historical fixed pair applies. When [`Frame::theme_bevel`] is on and
    /// a theme is active, the tones come from the theme's background: the highlight is that
    /// surface stepped halfway to white and the shade halfway to black, which is the same pair of
    /// relationships [`crate::render::Bevel::from_base`] states — only taken from the page rather
    /// than from a fixed grey.
    fn themed_bevel_tones(&self) -> Option<(Color, Color)> {
        if !self.theme_bevel {
            return None;
        }
        let base = {
            let manager = crate::style::theme_manager();
            manager.current_theme()?.colors.background
        };
        Some((base.blend(&Color::WHITE, 0.5), base.blend(&Color::BLACK, 0.5)))
    }

    /// Strokes a two-tone ring around a rounded rectangle.
    ///
    /// Used only when a caller asked for both a bevel and a corner radius: a four-line bevel cannot
    /// turn a corner, so the two tones are stated as two concentric rings instead. The leading tone
    /// is the outer ring, which is where a raised face catches the light; the trailing tone is one
    /// stroke width in.
    fn stroke_rounded_bevel(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        radius: u32,
        bevel: Bevel,
        width: u32,
    ) {
        let width = width.max(1);
        context.draw_rounded_rect_stroke(rect, radius, bevel.leading_tone(), width);
        if rect.width > width && rect.height > width {
            let inner = Rect::new(
                rect.x.saturating_add(width as i32),
                rect.y.saturating_add(width as i32),
                rect.width - width,
                rect.height - width,
            );
            context.draw_rounded_rect_stroke(
                inner,
                radius.saturating_sub(width),
                bevel.trailing_tone(),
                width,
            );
        }
    }

    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        // Remove old child from widget tree before replacing
        if let Some(old) = self.widget {
            self.base.request_redraw();
            self.base.remove_child_linked(old);
            if Some(old) != widget {
                // The old content is no longer this frame's content; leaving its visibility flag
                // set would let a host that renders by id draw it where the new content belongs.
                // Only a build with the widget runtime has that flag to clear.
                #[cfg(not(alloc_frugal))]
                crate::set_widget_visible(old, false);
            }
        }
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(widget_id, true);
        }
        self.relayout_contents();
        self.base.request_redraw();
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Retargets the interaction fade from the pointer facts the base recorded.
    ///
    /// A disabled frame is inert, so both targets collapse to rest instead of letting a
    /// hover linger on a control the user cannot interact with. `sync` itself re-aims rather
    /// than restarts, so a repeated event cannot make the tint stutter.
    fn sync_fade(&mut self) {
        let enabled = self.base.is_enabled();
        self.fade.sync(enabled && self.base.is_hovered(), enabled && self.base.is_pressed());
    }
    /// The outline colour with the interaction tint applied.
    ///
    /// At rest the intensity is exactly `0.0` and `color` is returned unchanged, which is
    /// what keeps a static frame's output byte-identical to the pre-fade rendering. A
    /// hovered or pressed frame steps toward the border's contrasting tone, so the feedback
    /// is visible on either appearance rather than assuming a light one.
    fn interactive_border(&self, color: Color) -> Color {
        let intensity = self.fade.intensity();
        if intensity <= 0.0 {
            return color;
        }
        color.blend(&color.contrast_color(), FRAME_INTERACTION_TINT * intensity)
    }
    /// Draws frame border.
    fn draw_frame(&self, context: &mut RenderContext) {
        let rect = self.geometry();
        match self.frame_shape {
            FrameShape::NoFrame => {}
            FrameShape::Box => self.draw_box_frame(context, rect),
            FrameShape::Panel => self.draw_panel_frame(context, rect),
            FrameShape::StyledPanel => self.draw_styled_panel_frame(context, rect),
            FrameShape::HLine => self.draw_hline_frame(context, rect),
            FrameShape::VLine => self.draw_vline_frame(context, rect),
            FrameShape::WinPanel => self.draw_win_panel_frame(context, rect),
        }
    }
    /// Draws box frame.
    fn draw_box_frame(&self, context: &mut RenderContext, rect: Rect) {
        // A frame's whole appearance is its outline, so every literal below is the
        // fallback for `border_color`. Two of them are *derived* rather than dropped:
        //
        //  * The raised/sunken bevel needs two tones that read as "lit" and "shaded".
        //    They are made by lightening/darkening the border colour, which is what
        //    keeps the 3D effect readable on a dark surface instead of hardcoding a
        //    white highlight that would glow.
        //  * Explicit black/white border colours are recognised and the bevel is
        //    suppressed, because that is the only nonzero `border_width` a theme can
        //    express today; inventing a white highlight on top of a theme that asked
        //    for a plain black rule would be inventing chrome the caller did not ask
        //    for. An unset or grey border keeps the historical light/dark bevel.
        let style = self.style();
        let raw_border = style.border_color.unwrap_or(Color::rgb(0, 0, 0));
        let themed = style.border_color.is_some();
        let plain = themed
            && (raw_border == Color::rgb(0, 0, 0) || raw_border == Color::rgb(255, 255, 255));
        // The bevel/plain decisions read the declared border; only the painted tone follows
        // the interaction. At rest the two are the same value.
        let border = self.interactive_border(raw_border);
        match self.frame_shadow {
            FrameShadow::Plain => {
                // Draw single border, honouring the declared corner radius when there is one.
                // A zero radius keeps the historical `draw_rect` call byte-for-byte; a non-zero
                // one draws the same line as a rounded stroke instead of a square rule through
                // the middle of the corners.
                let width = self.painted_width();
                let radius = stroke_radius(style);
                if width > 0 && radius > 0 {
                    context.draw_rounded_rect_stroke(rect, radius, border, width);
                } else if width > 0 {
                    context.draw_rect(rect, border);
                }
                // `frameWidth(0)` is the reference toolkit's "no frame", so a zero width paints
                // nothing rather than a hairline nobody asked for.
            }
            FrameShadow::Raised => {
                // Draw raised border
                //
                // # Why this is now the shared primitive
                //
                // The four lines below and their two tones were spelled out here, and the same
                // relationship was spelled out again in the `Sunken` arm (with the tones exchanged)
                // and a third time in `draw_win_panel_frame`. "Which edges are lit" was therefore
                // carried by the *order of two nearly identical blocks* — so reversing an inset was a
                // copy-paste edit that nothing could check.
                //
                // `Bevel` states that relationship once: a direction decides which pair of edges
                // gets which tone. The **derivation is unchanged** (base blended 0.5 toward white and
                // black), which is why this is a refactor rather than a visual change — the
                // byte-identical snapshot is the evidence, and `Bevel`'s own tests pin the two
                // expressions.
                //
                // The plain-border case still suppresses the bevel by handing `Bevel` the same tone
                // for both sides: a theme that asked for a plain black rule must not get a white
                // highlight invented on top of it.
                let bevel = if plain {
                    Bevel::from_tones(border, border, border)
                } else if let Some((light, shade)) = self.themed_bevel_tones() {
                    // The theme-derived pair; see `Frame::themed_bevel_tones`.
                    Bevel::from_tones(border, light, shade)
                } else if themed {
                    Bevel::from_base(border)
                } else {
                    // No theme: the historical hard-coded pair, which is deliberately *not* derived
                    // from the fallback black border — `rgb(0,0,0)` blended would be `rgb(0,0,0)`,
                    // and the bevel would vanish on exactly the build that has no palette to fall
                    // back on.
                    Bevel::from_tones(
                        Color::rgb(128, 128, 128),
                        Color::rgb(255, 255, 255),
                        Color::rgb(128, 128, 128),
                    )
                }
                .with_direction(BevelDirection::Raised);
                // The painted width is `frameWidth`, which falls back to `lineWidth` — the
                // historical hard-coded 1 px is now the default value of that derivation rather
                // than a number stated at the paint site.
                let width = self.painted_width();
                let radius = stroke_radius(style);
                if width > 0 {
                    if radius > 0 {
                        self.stroke_rounded_bevel(context, rect, radius, bevel, width);
                    } else {
                        bevel.stroke(context, rect, width);
                    }
                }

                // The mid lines are a **four-line groove**, not the inner half of a two-line bevel:
                // a softer highlight and a *deeper* shade, which is why they keep their own weights
                // rather than `Bevel::stroke_inner`. See `BEVEL_INNER_SHADE_WEIGHT`'s note — the
                // numbers are right for this shape and wrong for the other one.
                let mid_light = if themed {
                    border.blend(&Color::rgb(255, 255, 255), 0.25)
                } else {
                    Color::rgb(192, 192, 192)
                };
                let mid_dark = if themed {
                    border.blend(&Color::rgb(0, 0, 0), 0.75)
                } else {
                    Color::rgb(64, 64, 64)
                };
                self.draw_mid_lines(context, rect, mid_light, mid_dark);
            }
            FrameShadow::Sunken => {
                // Draw sunken border
                //
                // The same relationship the `Raised` arm states, with the tones exchanged — and
                // that exchange is now the *direction* rather than the order of two blocks. Before
                // this the arm carried a full second copy of the four `draw_line` calls, so "make
                // this raised" was a copy-paste edit across a dozen lines instead of one token.
                let bevel = if plain {
                    Bevel::from_tones(border, border, border)
                } else if let Some((light, shade)) = self.themed_bevel_tones() {
                    Bevel::from_tones(border, light, shade)
                } else if themed {
                    Bevel::from_base(border)
                } else {
                    // No theme: the historical hard-coded pair. `from_base` would blend the
                    // fallback black border and the bevel would vanish, so the tones are stated —
                    // and, for `Inset`, `shade` is the top-left edge and `light` the bottom-right.
                    Bevel::from_tones(
                        Color::rgb(128, 128, 128),
                        Color::rgb(255, 255, 255),
                        Color::rgb(128, 128, 128),
                    )
                }
                .with_direction(BevelDirection::Inset);
                let width = self.painted_width();
                let radius = stroke_radius(style);
                if width > 0 {
                    if radius > 0 {
                        self.stroke_rounded_bevel(context, rect, radius, bevel, width);
                    } else {
                        bevel.stroke(context, rect, width);
                    }
                }

                // Draw mid line if needed — the same four-line groove the `Raised` arm keeps,
                // with the pair exchanged.
                let mid_light = if themed {
                    border.blend(&Color::rgb(0, 0, 0), 0.75)
                } else {
                    Color::rgb(64, 64, 64)
                };
                let mid_dark = if themed {
                    border.blend(&Color::rgb(255, 255, 255), 0.25)
                } else {
                    Color::rgb(192, 192, 192)
                };
                self.draw_mid_lines(context, rect, mid_light, mid_dark);
            }
        }
    }
    /// Draws the mid-line section shared by Raised/Sunken box frames.
    fn draw_mid_lines(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        mid_light: Color,
        mid_dark: Color,
    ) {
        let line_width = self.painted_width();
        let mid_line_width = self.mid_line_width;
        if mid_line_width <= 0.0 || line_width == 0 {
            return;
        }
        let line_width = line_width as f32;
        context.draw_line(
            Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + line_width,
            ),
            mid_light,
        );
        context.draw_line(
            Point::from_f32(rect.x as f32 + line_width, rect.y as f32 + line_width),
            Point::from_f32(
                rect.x as f32 + line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_light,
        );
        context.draw_line(
            Point::from_f32(
                rect.x as f32 + line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_dark,
        );
        context.draw_line(
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + line_width,
            ),
            Point::from_f32(
                rect.x as f32 + rect.width as f32 - line_width,
                rect.y as f32 + rect.height as f32 - line_width,
            ),
            mid_dark,
        );
    }
    /// Draws panel frame with a subtle background fill and simple border.
    fn draw_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(236, 233, 216));
        context.face_with_gradient(
            rect,
            bg_color,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        let border = self.interactive_border(style.border_color.unwrap_or(Color::rgb(64, 64, 64)));
        let width = self.painted_width();
        if width > 0 {
            let radius = stroke_radius(style);
            if radius > 0 {
                context.draw_rounded_rect_stroke(rect, radius, border, width);
            } else {
                context.draw_rect(rect, border);
            }
        }
    }
    /// Draws styled panel frame.
    fn draw_styled_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // More sophisticated panel with gradient
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        context.face_with_gradient(
            rect,
            bg_color,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        self.draw_box_frame(context, rect);
    }
    /// Draws horizontal line frame.
    fn draw_hline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let y = rect.y + (rect.height as i32) / 2;
        let color =
            self.interactive_border(self.style().border_color.unwrap_or(Color::rgb(0, 0, 0)));
        let width = self.painted_width();
        if width == 0 {
            return;
        }
        context.draw_line_stroke(
            Point::new(rect.x, y),
            Point::new(rect.x + rect.width as i32, y),
            color,
            width,
        );
    }
    /// Draws vertical line frame.
    fn draw_vline_frame(&self, context: &mut RenderContext, rect: Rect) {
        let x = rect.x + (rect.width as i32) / 2;
        let color =
            self.interactive_border(self.style().border_color.unwrap_or(Color::rgb(0, 0, 0)));
        let width = self.painted_width();
        if width == 0 {
            return;
        }
        context.draw_line_stroke(
            Point::new(x, rect.y),
            Point::new(x, rect.y + rect.height as i32),
            color,
            width,
        );
    }
    /// Draws Windows panel frame.
    fn draw_win_panel_frame(&self, context: &mut RenderContext, rect: Rect) {
        // Windows-style panel
        //
        // The fill is themed, but the white/grey bevel is not: it is the illusion of
        // a raised edge, so unlike `draw_box_frame` it is left alone rather than
        // derived from the border colour. A panel that has asked for a dark background
        // still wants its highlight to read as a highlight.
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        context.face_with_gradient(
            rect,
            bg_color,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Draw 3D border
        //
        // This is a **double bevel**: an outer sunken edge and an inner raised one, which is
        // the classic two-tone panel. Both halves are the relationship `Bevel` states, so they
        // are two strokes with opposite directions rather than sixteen hand-placed lines — the
        // whole point of the primitive being that "which edges are lit" is a direction and not
        // the order of two blocks. The tones are stated with `from_tones` rather than derived
        // from the fill: as the note above says, the white/grey pair *is* the illusion — unless
        // the caller asked for the theme-derived pair, which replaces both tones at once.
        let (light_color, dark_color) = self
            .themed_bevel_tones()
            .unwrap_or((Color::rgb(255, 255, 255), Color::rgb(128, 128, 128)));
        let width = self.painted_width();
        if width == 0 {
            return;
        }
        let radius = stroke_radius(style);
        let outer = Bevel::from_tones(bg_color, light_color, dark_color)
            .with_direction(BevelDirection::Inset);
        if radius > 0 {
            self.stroke_rounded_bevel(context, rect, radius, outer, width);
        } else {
            outer.stroke(context, rect, width);
        }
        // Inner border (raised), one stroke width in so the double edge keeps its thickness at
        // every `frameWidth` instead of always occupying exactly one pixel.
        let inner_rect = Rect::new(
            rect.x.saturating_add(width as i32),
            rect.y.saturating_add(width as i32),
            rect.width.saturating_sub(width.saturating_mul(2)),
            rect.height.saturating_sub(width.saturating_mul(2)),
        );
        let inner = Bevel::from_tones(bg_color, light_color, dark_color)
            .with_direction(BevelDirection::Raised);
        if radius > 0 {
            self.stroke_rounded_bevel(
                context,
                inner_rect,
                radius.saturating_sub(width),
                inner,
                width,
            );
        } else {
            inner.stroke(context, inner_rect, width);
        }
    }
}
// Implement Widget trait
impl Widget for Frame {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(dimensions::FRAME_DEFAULT_WIDTH, dimensions::FRAME_DEFAULT_HEIGHT)
    }

    /// Moves the frame and keeps a margined child inside it.
    ///
    /// A frame that positions its content from [`Frame::contents_rect`] has to redo that
    /// positioning when its own rectangle moves; otherwise the child stays at the old inset and
    /// the margin slowly stops being a margin. Zero margins are a no-op, so the historical
    /// "the child owns its own geometry" behaviour is unchanged.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.relayout_contents();
    }

    // The outline is what a frame paints, so a hovered or pressed frame owes frames while
    // the tint is in flight. A resting frame's drivers report no movement, so it costs
    // nothing on the animation bus.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.fade.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.fade.is_animating()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Frame`'s property contract.
///
/// # Why this is not empty
///
/// The old dispatch grouped `WidgetKind::Panel | WidgetKind::Frame` into the
/// breadcrumb arm, which only ever matched `Breadcrumb` instances and answered
/// `UnsupportedOnWidget` for a `Frame`. Declaring nothing was honest about *that*
/// bug but wrong about the control: `Frame` has four real, independently settable
/// fields (`frame_shape`, `frame_shadow`, `line_width`, `mid_line_width`), and back
/// when it had no capability the factory could not construct one at all —
/// `factory_name_for_kind(WidgetKind::Frame)` returned `""`, so
/// `create_frame(..)` silently produced id `0`.
///
/// The names match `FRAME_PROPERTIES` one for one; a token that appears in the
/// schema but not in the `set` match below is a property that answers
/// `UnknownProperty` on write, which
/// `published_enum_tokens_are_accepted_by_their_control` rejects.
impl WidgetProperties for Frame {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "frame_shape" => Ok(CapabilityValue::String(self.frame_shape.as_str().to_string())),
            "frame_shadow" => Ok(CapabilityValue::String(self.frame_shadow.as_str().to_string())),
            "line_width" => Ok(CapabilityValue::Float(self.line_width as f64)),
            "mid_line_width" => Ok(CapabilityValue::Float(self.mid_line_width as f64)),
            "frame_width" => Ok(CapabilityValue::Float(self.frame_width() as f64)),
            "contents_margins" => {
                Ok(CapabilityValue::String(format_margins(self.contents_margins)))
            }
            "theme_bevel" => Ok(CapabilityValue::Bool(self.theme_bevel)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "frame_shape" => {
                let text = expect_string(value)?;
                let Some(shape) = FrameShape::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_frame_shape(shape);
                Ok(())
            }
            "frame_shadow" => {
                let text = expect_string(value)?;
                let Some(shadow) = FrameShadow::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_frame_shadow(shadow);
                Ok(())
            }
            "line_width" => {
                self.set_line_width(expect_f32(value)?);
                Ok(())
            }
            "mid_line_width" => {
                self.set_mid_line_width(expect_f32(value)?);
                Ok(())
            }
            "frame_width" => {
                self.set_frame_width(expect_f32(value)?);
                Ok(())
            }
            "contents_margins" => {
                let text = expect_string(value)?;
                let Some(margins) = parse_margins(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_contents_margins(margins);
                Ok(())
            }
            "theme_bevel" => {
                self.set_theme_bevel(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `FRAME_PROPERTIES`.
        property_names_of![
            "frame_shape",
            "frame_shadow",
            "line_width",
            "mid_line_width",
            "frame_width",
            "contents_margins",
            "theme_bevel",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for Frame {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // The four pointer events are the only ones that can change the interaction fade.
        // They are handled before the enabled early return so a disabled frame still
        // retargets both drivers to rest instead of leaving a hovered tint frozen in place.
        match event {
            Event::MouseEnter { .. }
            | Event::MouseLeave { .. }
            | Event::MousePress { .. }
            | Event::MouseRelease { .. } => self.sync_fade(),
            _ => { /* Other events carry no fade-relevant pointer state */ }
        }
        // Skip event forwarding when disabled
        if !self.base.is_enabled() {
            return;
        }
        // Forward events to widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().forward_event(widget_id, event);
            }
        }
    }
}
impl Draw for Frame {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw frame
        self.draw_frame(context);
        // Draw widget via registry
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().draw_widget(widget_id, context);
            }
        }
        // Dim content when disabled.
        //
        // A **recession toward the surface**, not a grey wash: the fixed
        // `rgba(128,128,128,80)` this used to paint has no direction, so it lightened the contents
        // on a dark appearance and darkened them on a light one — "disabled" reading as "more
        // contrast" exactly where legibility was worst. Same defect and same fix as the modal
        // scrim (BLUE21 B23); see [`dimensions::DISABLED_VEIL_ALPHA`].
        if !self.base.is_enabled() {
            let rect = self.geometry();
            let surface = self.style().background_color.or_else(|| {
                crate::style::theme_manager().current_theme().map(|active| active.colors.background)
            });
            match surface {
                Some(surface) => {
                    context.fill_rect(rect, surface.with_alpha(dimensions::DISABLED_VEIL_ALPHA))
                }
                // No surface to fade toward: the historical grey is the honest fallback, and it is
                // the one case where a direction cannot be derived.
                None => context.fill_rect(rect, Color::rgba(128, 128, 128, 80)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{MiniToString, String, Vec};
    use crate::core::{Color, ObjectId, Point, Rect, Size};
    use crate::style::WidgetStyle;

    #[test]
    fn frame_creation_defaults() {
        let f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(f.frame_shape(), FrameShape::Box);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
        assert!((f.line_width() - 1.0).abs() < f32::EPSILON);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
        assert!(f.widget().is_none());
        assert!(f.registry().is_none());
    }

    #[test]
    fn frame_shape_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shape(FrameShape::Panel);
        assert_eq!(f.frame_shape(), FrameShape::Panel);
        f.set_frame_shape(FrameShape::NoFrame);
        assert_eq!(f.frame_shape(), FrameShape::NoFrame);
        f.set_frame_shape(FrameShape::StyledPanel);
        assert_eq!(f.frame_shape(), FrameShape::StyledPanel);
        f.set_frame_shape(FrameShape::HLine);
        assert_eq!(f.frame_shape(), FrameShape::HLine);
        f.set_frame_shape(FrameShape::VLine);
        assert_eq!(f.frame_shape(), FrameShape::VLine);
        f.set_frame_shape(FrameShape::WinPanel);
        assert_eq!(f.frame_shape(), FrameShape::WinPanel);
        f.set_frame_shape(FrameShape::Box);
        assert_eq!(f.frame_shape(), FrameShape::Box);
    }

    #[test]
    fn frame_shadow_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_frame_shadow(FrameShadow::Raised);
        assert_eq!(f.frame_shadow(), FrameShadow::Raised);
        f.set_frame_shadow(FrameShadow::Sunken);
        assert_eq!(f.frame_shadow(), FrameShadow::Sunken);
        f.set_frame_shadow(FrameShadow::Plain);
        assert_eq!(f.frame_shadow(), FrameShadow::Plain);
    }

    #[test]
    fn frame_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_line_width(3.0);
        assert!((f.line_width() - 3.0).abs() < f32::EPSILON);
        f.set_line_width(0.5);
        assert!((f.line_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_mid_line_width_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_mid_line_width(2.0);
        assert!((f.mid_line_width() - 2.0).abs() < f32::EPSILON);
        f.set_mid_line_width(0.0);
        assert!((f.mid_line_width() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_widget_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.widget().is_none());
        let wid: ObjectId = 42;
        f.set_widget(Some(wid));
        assert_eq!(f.widget(), Some(wid));
        f.set_widget(None);
        assert!(f.widget().is_none());
    }

    #[test]
    fn frame_geometry_delegation() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        f.set_geometry(Rect::new(10, 20, 300, 150));
        assert_eq!(f.geometry(), Rect::new(10, 20, 300, 150));
    }

    #[test]
    fn frame_visibility() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_visible());
        f.hide();
        assert!(!f.is_visible());
        f.show();
        assert!(f.is_visible());
    }

    #[test]
    fn frame_enabled() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.is_enabled());
        f.set_enabled(false);
        assert!(!f.is_enabled());
        f.set_enabled(true);
        assert!(f.is_enabled());
    }

    #[test]
    fn frame_parent_children() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.parent().is_none());
        let pid: ObjectId = 10;
        f.set_parent(Some(pid));
        assert_eq!(f.parent(), Some(pid));
        f.set_parent(None);
        assert!(f.parent().is_none());

        let cid: ObjectId = 20;
        f.add_child(cid);
        assert_eq!(f.children().len(), 1);
        assert_eq!(f.children()[0], cid);
        f.remove_child(cid);
        assert!(f.children().is_empty());
    }

    #[test]
    fn frame_tooltip_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert!(f.tooltip().is_empty());
        f.set_tooltip("Frame info".to_string());
        assert_eq!(f.tooltip(), "Frame info");
        f.set_tooltip(String::new());
        assert!(f.tooltip().is_empty());
    }

    #[test]
    fn frame_style_roundtrip() {
        let mut f = Frame::new(Rect::new(0, 0, 200, 100));
        assert_eq!(*f.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        f.set_style(custom.clone());
        assert_eq!(*f.style(), custom);
    }

    #[test]
    fn frame_id_kind() {
        let f_a = Frame::new(Rect::new(0, 0, 100, 50));
        let f_b = Frame::new(Rect::new(0, 0, 100, 50));
        assert_ne!(f_a.id(), f_b.id());
        assert_eq!(f_a.kind(), WidgetKind::Frame);
        assert_eq!(f_b.kind(), WidgetKind::Frame);
    }

    /// A raised box frame paints the **two tones the hand-written code painted**, on the same edges.
    ///
    /// # Why this is a pixel assertion and not a snapshot assertion
    ///
    /// The exported `frame` uses the **default** `FrameShadow::Plain`, which paints a single border
    /// and no bevel at all — so `snapshots/svg/frame.svg` never exercises this branch, and "the
    /// snapshot is byte-identical" would be evidence of nothing. This test drives the branch
    /// directly and reads the rendered pixels.
    ///
    /// # What it pins about the migration
    ///
    /// `draw_box_frame`'s `Raised` arm used to spell out four `draw_line` calls and two
    /// `border.blend(...)` expressions; it now hands a `Bevel` a direction. The *derivation* is
    /// required to be unchanged, or the migration is a visual edit wearing the word "refactor":
    ///
    /// * the top and left edges carry `border.blend(WHITE, 0.5)`,
    /// * the bottom and right edges carry `border.blend(BLACK, 0.5)`,
    /// * each edge is **one pixel** wide, which is what `draw_line` painted.
    ///
    /// The tones are computed here from `Color` rather than through `Bevel`, so a mutation inside
    /// the primitive cannot make the expectation move with it.
    #[test]
    fn a_raised_box_frame_paints_the_hand_written_tones() {
        use crate::core::{Color, Font, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        // Silence an unused-import warning in the profiles that gate `Font` out of this path.
        let _ = Font::default();

        let border = Color::rgb(120, 140, 160);
        let rect = Rect::new(4, 4, 12, 8);
        let mut frame = Frame::new(rect);
        frame.set_frame_shadow(FrameShadow::Raised);
        frame.set_style(WidgetStyle::default().with_border(border, 1, 0));

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 20), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let pixel = |x: i32, y: i32| -> Color {
            let at = ((y as u32 * 24 + x as u32) * 4) as usize;
            Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
        };

        let lit = border.blend(&Color::WHITE, 0.5);
        let shaded = border.blend(&Color::BLACK, 0.5);
        let mid_x = rect.x + rect.width as i32 / 2;
        let mid_y = rect.y + rect.height as i32 / 2;

        assert_eq!(pixel(mid_x, rect.y), lit, "the top edge must be the highlight");
        assert_eq!(pixel(rect.x, mid_y), lit, "the left edge must be the highlight");
        assert_eq!(
            pixel(mid_x, rect.y + rect.height as i32),
            shaded,
            "the bottom edge must be the shade"
        );
        assert_eq!(
            pixel(rect.x + rect.width as i32, mid_y),
            shaded,
            "the right edge must be the shade"
        );
        // One pixel wide: the row *inside* the top edge is not part of the bevel.
        assert_ne!(
            pixel(mid_x, rect.y + 1),
            lit,
            "a one-pixel bevel must not bleed into the second row"
        );
    }

    /// A raised frame given a **plain black** border gets no invented highlight.
    ///
    /// The rule the hand-written arm documented: the only nonzero `border_width` a theme can express
    /// today is an explicit black or white rule, and inventing a white bevel on top of a theme that
    /// asked for a plain black line would be adding chrome the caller did not ask for.
    #[test]
    fn a_plain_border_suppresses_the_bevel_tones() {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        let rect = Rect::new(4, 4, 12, 8);
        let mut frame = Frame::new(rect);
        frame.set_frame_shadow(FrameShadow::Raised);
        frame.set_style(WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 1, 0));

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 20), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let pixel = |x: i32, y: i32| -> Color {
            let at = ((y as u32 * 24 + x as u32) * 4) as usize;
            Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
        };
        let mid_x = rect.x + rect.width as i32 / 2;
        let top = pixel(mid_x, rect.y);
        let bottom = pixel(mid_x, rect.y + rect.height as i32);
        assert_eq!(top, bottom, "a plain border must not light one edge and shade the other");
        assert_eq!(top, Color::rgb(0, 0, 0), "the border's own colour, nothing invented");
    }

    #[test]
    fn frame_signal_accessors() {
        let f = Frame::new(Rect::new(0, 0, 100, 50));
        let _hover = f.base().hover_signal();
        let _mouse_down = f.base().mouse_down_signal();
        let _mouse_up = f.base().mouse_up_signal();
    }

    /// A panel-shaped frame's fill follows the appearance.
    ///
    /// # What is actually being pinned, and what is **not**
    ///
    /// `draw_panel_frame`, `draw_styled_panel_frame` and `draw_win_panel_frame` each fall back to
    /// an opaque literal — `rgb(236,233,216)`, `rgb(240,240,240)`, `rgb(240,240,240)` — when no
    /// colour was set. Those literals look like the defect this appendix is full of (a constant that
    /// does not move with the theme), so this test was first written to pin them.
    ///
    /// It **does not**. Measured: `apply_active_theme` classifies a `frame` into a role and fills
    /// `style.background_color` with `surface_container`, so the literal arm is never reached in a
    /// themed build — and the reverse injection (putting the literal back) **still passed**. The
    /// mutation test is what caught that; a re-derivation that cannot fail is not a test.
    ///
    /// What it pins instead is the part that *is* uncovered: the exported `frame` uses the
    /// **default** `Box` shape, which paints no fill at all, so the three panel arms are reachable
    /// only through a caller that asks for a shape and **no snapshot ever does**. This is the only
    /// coverage those arms have, and it is what the appendix's `frame` row should say.
    #[test]
    #[cfg(device_profile)]
    fn a_panels_fill_moves_with_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 100, 50);

        let fill_of = |appearance| {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut frame = Frame::new(rect);
            frame.set_frame_shape(FrameShape::Panel);
            crate::theme::apply_theme_to_widget(&mut frame);
            let surface = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg_on(&mut frame, rect, surface);
            // The panel's fill is the first `fill="rgba(` after the backdrop.
            let key = "fill=\"rgba(";
            let backdrop = svg.find(key).expect("a backdrop") + key.len();
            let face = svg[backdrop..].find(key).expect("the panel's fill") + backdrop + key.len();
            let end = svg[face..].find(')').expect("the fill's close") + face;
            let mut parts = svg[face..end].split(',');
            let r = parts.next().and_then(|v| v.trim().parse().ok()).expect("r");
            let g = parts.next().and_then(|v| v.trim().parse().ok()).expect("g");
            let b = parts.next().and_then(|v| v.trim().parse().ok()).expect("b");
            (Color::rgb(r, g, b), surface)
        };

        let dark = fill_of(crate::theme::AppearanceMode::Dark);
        let light = fill_of(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark.0, light.0,
            "a panel's fill must follow the appearance; both were {:?}",
            dark.0
        );
        // Each must be the **surface role** rather than any literal — the panel reads
        // `surface_container`, which is a step off the page in that appearance, so the fill must
        // differ from the window it sits on. A literal that happened to equal the page would make
        // a panel invisible; the literals this replaced did exactly that in one appearance each.
        for (label, (fill, surface)) in [("dark", dark), ("light", light)] {
            assert_ne!(
                fill, surface,
                "the {label} panel fill {fill:?} must be a step off the page {surface:?}, or the \
                 panel has no edge to see"
            );
        }
    }

    /// A `Panel` frame that declares a `background_gradient` paints the ramp, not the solid fill.
    ///
    /// # Why this is the assertion, and not "the SVG mentions a gradient"
    ///
    /// `background_gradient` was settable, mergeable and CSS-expressible but read by **no** painter
    /// except `Button`: a caller that set one still got a solid `background_color`. Before this
    /// coverage the panel's `face` call passed no gradient, so the rendered document had no
    /// `<linearGradient>` at all. The test renders the same frame with and without the gradient and
    /// asserts the documents **differ** and that the gradient one carries the paint server — so a
    /// regression that dropped the argument fails on both counts.
    #[test]
    #[cfg(device_profile)]
    fn a_panel_paints_a_declared_background_gradient() {
        use crate::core::Point;
        use crate::style::Gradient;

        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 100, 50);
        let backdrop = Color::WHITE;

        let render = |gradient: Option<Gradient>| {
            let mut frame = Frame::new(rect);
            frame.set_frame_shape(FrameShape::Panel);
            let style = WidgetStyle {
                background_color: Some(Color::rgb(240, 240, 240)),
                background_gradient: gradient,
                ..WidgetStyle::default()
            };
            frame.set_style(style);
            crate::widget::svg::render_widget_to_svg_on(&mut frame, rect, backdrop)
        };

        let solid = render(None);
        let ramp = render(Some(
            Gradient::linear(Point::new(0, 0), Point::new(100, 0))
                .add_stop(0.0, Color::BLACK)
                .add_stop(1.0, Color::WHITE),
        ));

        assert!(!solid.contains("linearGradient"), "a solid panel needs no paint server");
        assert!(
            ramp.contains("<linearGradient") && ramp.contains("url(#"),
            "a declared gradient must emit a paint server in the panel's document"
        );
        assert_ne!(solid, ramp, "the gradient frame must differ from the solid one");
    }

    // ------------------------------------------------------------------
    // Interaction fade (P1-9)
    // ------------------------------------------------------------------

    /// Rasterises `frame` into a frame buffer a test can compare byte-for-byte.
    fn render_rgba(frame: &mut Frame, size: Size) -> Vec<u8> {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// Reads one RGBA pixel out of a frame buffer produced by [`render_rgba`].
    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> Color {
        let at = ((y * width + x) * 4) as usize;
        Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
    }

    /// A pointer entering the frame retargets the hover driver at once.
    ///
    /// The answer has to be true before any `tick` has run: the animation bus asks
    /// `is_animating` first, and a control that only admitted movement on its next tick
    /// would never be scheduled for one.
    #[test]
    fn mouse_enter_makes_the_frame_animating() {
        let mut frame = Frame::new(Rect::new(4, 4, 12, 8));
        assert!(!Widget::is_animating(&frame), "a new frame rests and owes no frames");

        frame.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });

        assert!(
            Widget::is_animating(&frame),
            "MouseEnter must make the frame's fade moving before any tick"
        );
        assert!(Widget::tick(&mut frame, 16), "the first fade frame must report work");
    }

    /// A resting frame paints its declared border byte-for-byte, and returns to exactly
    /// that picture after a hover has come and gone.
    #[test]
    fn a_resting_frame_paints_the_untinted_border() {
        let rect = Rect::new(4, 4, 12, 8);
        let border = Color::rgb(120, 140, 160);
        let size = Size::new(24, 20);
        let mid_x = rect.x as u32 + rect.width / 2;

        let mut frame = Frame::new(rect);
        frame.set_style(WidgetStyle::default().with_border(border, 1, 0));
        let fresh = render_rgba(&mut frame, size);
        assert_eq!(
            pixel(&fresh, size.width, mid_x, rect.y as u32),
            border,
            "at rest the outline must be the caller's border colour, not a tinted one"
        );

        frame.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });
        while Widget::tick(&mut frame, 16) {}
        let hovered = render_rgba(&mut frame, size);
        assert_ne!(
            pixel(&hovered, size.width, mid_x, rect.y as u32),
            border,
            "a settled hover must actually tint the outline"
        );

        frame.handle_event(&Event::MouseLeave { pos: Point::new(5, 5) });
        while Widget::tick(&mut frame, 16) {}
        assert_eq!(frame.fade.hover.value(), 0.0, "the fade returns to rest exactly");
        assert_eq!(
            render_rgba(&mut frame, size),
            fresh,
            "a frame back at rest must repaint the same bytes it did before the hover"
        );
    }

    /// Animating to the hover terminal state produces the same picture as setting the
    /// terminal value directly, so the fade is purely a path between two identical ends.
    #[test]
    fn a_settled_hover_matches_the_manually_set_terminal_state() {
        let rect = Rect::new(4, 4, 12, 8);
        let border = Color::rgb(120, 140, 160);
        let size = Size::new(24, 20);
        let make = || {
            let mut frame = Frame::new(rect);
            frame.set_style(WidgetStyle::default().with_border(border, 1, 0));
            frame
        };

        let mut animated = make();
        animated.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });
        let mut frames = 0;
        while Widget::tick(&mut animated, 16) {
            frames += 1;
            assert!(frames < 500, "the fade must terminate");
        }
        assert_eq!(animated.fade.hover.value(), 1.0, "the hover settles at its target");

        let mut manual = make();
        manual.fade.hover.jump_to(1.0);
        manual.fade.press.jump_to(0.0);

        assert_eq!(
            render_rgba(&mut animated, size),
            render_rgba(&mut manual, size),
            "the animated end state must be the manually set terminal state"
        );
    }

    /// Reduced motion collapses the fast tempo to zero, so one tick reaches the target.
    #[test]
    fn reduced_motion_reaches_the_frame_target_in_one_frame() {
        use crate::compat::Box;
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Reduced));

        let mut frame = Frame::new(Rect::new(4, 4, 12, 8));
        frame.handle_event(&Event::MouseEnter { pos: Point::new(5, 5) });
        let moving = Widget::tick(&mut frame, 16);

        assert!(!moving, "a zero-duration fade settles on the first tick");
        assert_eq!(frame.fade.hover.value(), 1.0, "and reaches the target, not half way");
        assert!(!Widget::is_animating(&frame));

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }

    // ── Contents margins, frame width and radius (R010–R012) ───────────

    #[test]
    fn frame_width_falls_back_to_the_line_width() {
        let mut f = Frame::new(Rect::new(0, 0, 40, 20));
        assert!((f.frame_width() - 1.0).abs() < f32::EPSILON, "the default is the line width");
        f.set_frame_width(3.0);
        assert!((f.frame_width() - 3.0).abs() < f32::EPSILON);
        assert!((f.line_width() - 1.0).abs() < f32::EPSILON, "the two are independent");
        assert_eq!(f.painted_width(), 3);
        f.set_frame_width(-2.0);
        assert_eq!(f.frame_width(), 0.0);
        assert_eq!(f.painted_width(), 0, "zero is the reference toolkit's 'no frame'");
    }

    #[test]
    fn contents_margins_inset_the_contents_rect() {
        let mut f = Frame::new(Rect::new(10, 20, 100, 60));
        assert_eq!(f.contents_rect(), Rect::new(10, 20, 100, 60), "zero margins are identity");
        f.set_contents_margins(EdgeOffsets::new(2, 3, 4, 5));
        assert_eq!(
            f.contents_rect(),
            Rect::new(15, 22, 92, 54),
            "left/top are added and the right/bottom are removed"
        );
        assert_eq!(f.contents_margins(), EdgeOffsets::new(2, 3, 4, 5));
    }

    #[test]
    fn a_margined_frame_positions_its_registered_child() {
        use crate::compat::Rc;
        use crate::render::RenderContext;
        use std::sync::{Arc, Mutex};

        let recorded: Arc<Mutex<Option<Rect>>> = Arc::new(Mutex::new(None));
        let sink = recorded.clone();
        let mut registry = SimpleRegistry::new();
        registry.register_with_geometry(
            7,
            |_context: &mut RenderContext| {},
            |_event: &Event| {},
            move |geometry: Rect| {
                if let Ok(mut slot) = sink.lock() {
                    *slot = Some(geometry);
                }
            },
        );

        let mut frame = Frame::new(Rect::new(10, 20, 100, 60));
        frame.set_registry(Rc::new(RefCell::new(registry)));
        frame.set_widget(Some(7));
        assert_eq!(*recorded.lock().unwrap(), None, "zero margins leave the child alone");

        frame.set_contents_margins(EdgeOffsets::all(4));
        assert_eq!(
            *recorded.lock().unwrap(),
            Some(Rect::new(14, 24, 92, 52)),
            "the child is placed inside the contents rectangle"
        );

        frame.set_geometry(Rect::new(0, 0, 50, 30));
        assert_eq!(
            *recorded.lock().unwrap(),
            Some(Rect::new(4, 4, 42, 22)),
            "moving the frame moves the margined child with it"
        );
    }

    #[test]
    fn margins_round_trip_through_the_property_contract() {
        let mut f = Frame::new(Rect::new(0, 0, 20, 20));
        f.set("contents_margins", CapabilityValue::String("1,2,3,4".into()))
            .expect("the four-part spelling is accepted");
        assert_eq!(f.get("contents_margins"), Ok(CapabilityValue::String("1,2,3,4".into())));
        assert_eq!(
            f.set("contents_margins", CapabilityValue::String("1,2".into())),
            Err(CapabilityAccessError::TypeMismatch),
            "a partial spelling is refused rather than half-applied"
        );
        f.set("frame_width", CapabilityValue::Float(2.0)).expect("frame_width is writable");
        assert_eq!(f.get("frame_width"), Ok(CapabilityValue::Float(2.0)));
    }

    #[test]
    fn a_declared_radius_rounds_the_plain_border() {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        let rect = Rect::new(4, 4, 20, 16);
        let mut frame = Frame::new(rect);
        frame.set_style(WidgetStyle::default().with_border(Color::rgb(0, 0, 0), 1, 6));

        let mut backend = SoftwarePaintBackend::new(Size::new(32, 28), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let pixel = |x: i32, y: i32| -> Color {
            let at = ((y as u32 * 32 + x as u32) * 4) as usize;
            Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3])
        };

        assert_eq!(pixel(rect.x + 10, rect.y), Color::rgb(0, 0, 0), "the straight edge is drawn");
        assert_ne!(
            pixel(rect.x, rect.y),
            Color::rgb(0, 0, 0),
            "a rounded corner leaves its extreme corner pixel unpainted"
        );
    }

    #[test]
    fn every_published_property_is_readable() {
        let f = Frame::new(Rect::new(0, 0, 20, 20));
        for name in f.property_names() {
            assert!(f.get(name).is_ok(), "{name:?} is published but not readable");
        }
    }

    /// The opt-in bevel derives its highlight from the theme's own surface.
    ///
    /// The default pair is a fixed white/grey, which is the only pair that suits a light page and
    /// is what the committed snapshots contain. `theme_bevel` replaces it with the active
    /// background stepped halfway to white and black, so a dark palette gets a highlight that
    /// belongs to it. This test reads the rendered pixel, not the field, so a draw path that
    /// stopped consulting the derivation would fail it.
    #[test]
    #[cfg(device_profile)]
    fn the_theme_bevel_derives_its_tones_from_the_surface() {
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

        let rect = Rect::new(4, 4, 12, 8);
        let mut frame = Frame::new(rect);
        frame.set_frame_shadow(FrameShadow::Raised);
        frame.set_theme_bevel(true);
        crate::theme::apply_theme_to_widget(&mut frame);
        let background = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .expect("a preset is active");

        let mut backend = SoftwarePaintBackend::new(Size::new(24, 20), 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            frame.draw(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let at = (((rect.y as u32) * 24) + (rect.x as u32 + 6)) * 4;
        let top = Color::rgba(
            rgba[at as usize],
            rgba[at as usize + 1],
            rgba[at as usize + 2],
            rgba[at as usize + 3],
        );

        assert_eq!(
            top,
            background.blend(&Color::WHITE, 0.5),
            "the lit edge is the theme surface stepped toward white"
        );
        assert_ne!(top, Color::rgb(255, 255, 255), "not the fixed historical highlight");
    }
}
