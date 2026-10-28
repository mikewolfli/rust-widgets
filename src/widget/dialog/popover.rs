// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Popover widget — a floating bubble card with an anchor arrow.
//!
//! The Popover widget displays a floating card near an anchor rectangle,
//! optionally containing a child widget. It supports show/hide with an
//! arrow pointing toward the anchor, and auto-dismisses when the user
//! clicks outside the popover area.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Arrow size in pixels from tip to base.
const ARROW_SIZE: i32 = 10;
/// Corner radius of the popover body.
const CORNER_RADIUS: u32 = 8;
/// How tall the card is at the instant it appears, as a fraction of its settled height.
///
/// A popover unfolds **downward from the anchor**: the arrow and the anchor edge stay put and the
/// card grows along the axis the arrow points down. Growing from a corner or from the centre would
/// read as a dialog or a tooltip arriving, which are different affordances; the card's own origin is
/// what makes it read as "this thing belongs to that button". The floor is not zero, so the first
/// visible frame already has the card's width and a visible sliver of its body.
const REVEAL_MIN_SCALE: f32 = 0.06;
/// The alpha of the shadow under the card, out of 255. A constant rather than a literal in the draw
/// so the reveal can scale it without a second magic number.
const SHADOW_ALPHA: u8 = 40;
/// How far the shadow is offset below the card, in pixels.
///
/// AppKit's popover shadow falls mostly downward, so the offset is vertical only; a
/// diagonal offset reads as a material elevation shadow rather than a floating panel.
const SHADOW_OFFSET_Y: i32 = 4;
/// The shadow's blur radius, in pixels, passed to the renderer's box-blur primitive.
///
/// Shared with the popup menu's blurred tier (12), so a popover and a menu float at the
/// same height. The soft edge is what makes the card read as detached from the page; the
/// previous hard offset rectangle read as a second border.
const SHADOW_BLUR_RADIUS: u32 = 12;

/// The alpha the card's fill carries in translucent mode, out of 255.
///
/// A translucent card composites with the page behind it, which is the closest the render
/// path's per-shape alpha gets to a material: the renderer has no backdrop-blur primitive, so
/// the bubble is genuinely semi-transparent but its backdrop is not sampled and blurred.
const TRANSLUCENT_CARD_ALPHA: u8 = 217;

/// How a popover decides to close itself.
///
/// The three names mirror the behaviour families a floating card is usually offered:
/// a transient card closes on any outside interaction, a semi-transient one tolerates the
/// keyboard's Escape, and a permanent one closes only when the caller says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverBehavior {
    /// Closes on an outside click and on Escape. The default, and the behaviour the control
    /// had before the property existed.
    #[default]
    Transient,
    /// Closes on an outside click; Escape is left to the owner.
    SemiTransient,
    /// Never closes itself; the owner calls [`Popover::hide`] (or clears `shown`).
    Permanent,
}

impl PopoverBehavior {
    /// The spelling used by the `behavior` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::SemiTransient => "semi_transient",
            Self::Permanent => "permanent",
        }
    }

    /// Parses the `behavior` property's spelling.
    pub fn from_token(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "transient" => Some(Self::Transient),
            "semi_transient" | "semitransient" => Some(Self::SemiTransient),
            "permanent" => Some(Self::Permanent),
            _ => None,
        }
    }

    /// Whether an outside click closes the card.
    pub fn closes_on_outside_click(self) -> bool {
        !matches!(self, Self::Permanent)
    }

    /// Whether Escape closes the card.
    pub fn closes_on_escape(self) -> bool {
        matches!(self, Self::Transient)
    }
}

/// Where the caller wants the arrow, and therefore the card, to sit relative to the anchor.
///
/// `Auto` is the default and keeps the original placement rule (below the anchor, flipped
/// above when there is no room). The four fixed directions make the card's side a caller
/// decision, which is what a vertical-only arrow could not express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverArrow {
    /// Below the anchor when it fits, otherwise above it.
    #[default]
    Auto,
    /// The card sits below the anchor and the arrow points up.
    Up,
    /// The card sits above the anchor and the arrow points down.
    Down,
    /// The card sits to the right of the anchor and the arrow points left.
    Left,
    /// The card sits to the left of the anchor and the arrow points right.
    Right,
}

impl PopoverArrow {
    /// The spelling used by the `arrow_direction` property.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Parses the `arrow_direction` property's spelling.
    pub fn from_token(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// The edge of the anchor a popover hangs from.
///
/// The name a caller writes in `set_placement`; it is expressed through the existing
/// arrow-direction field so the placement and the arrow cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopoverEdge {
    /// Below the anchor when it fits, above otherwise.
    #[default]
    Auto,
    /// Above the anchor, arrow pointing down.
    Top,
    /// Below the anchor, arrow pointing up.
    Bottom,
    /// Left of the anchor, arrow pointing right.
    Left,
    /// Right of the anchor, arrow pointing left.
    Right,
}

/// Popover widget — a floating bubble card with an anchor arrow.
///
/// Shows a rounded rectangle with a triangular arrow pointing toward an
/// anchor rectangle. When visible, it can contain an optional child widget
/// rendered inside the card body. Clicking outside the popover area
/// automatically hides it; [`Popover::set_behavior`] changes that rule.
pub struct Popover {
    base: BaseWidget,
    content: Option<Box<dyn Widget>>,
    anchor_rect: Rect,
    visible: bool,
    /// How the card decides to close itself.
    behavior: PopoverBehavior,
    /// The caller's arrow placement preference.
    arrow: PopoverArrow,
    /// Caller-supplied placeholder shown when no content is installed.
    placeholder: Option<String>,
    /// Whether the card is painted semi-transparently.
    translucent: bool,
    /// Cached popover body rectangle (computed during draw).
    body_rect: Rect,
    /// How far the card has unfolded, `0.0` closed and `1.0` fully open.
    ///
    /// # Why this is separate from `visible`
    ///
    /// `visible` is the *logical* state and answers the instant it changes — `is_visible` tells
    /// whether the popover is up, not how far through appearing it is. The draw reads this, so the
    /// card unfolds rather than appearing whole on one frame and vanishing on the next. Same split,
    /// same reason, as `Switch`'s `checked`/`travel` and `Dialog`'s `visible`/`reveal`.
    reveal: PropertyDriver,
    /// Emitted just before the card becomes visible.
    pub about_to_show: GenericSignal,
    /// Emitted when the card becomes visible.
    pub shown: GenericSignal,
    /// Emitted just before the card is hidden.
    pub about_to_hide: GenericSignal,
    /// Emitted when the card has been hidden.
    pub dismissed: GenericSignal,
}

impl Popover {
    /// Creates a new Popover widget with the given geometry.
    ///
    /// Initially hidden with no content and an empty anchor rectangle.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Popover, geometry, "Popover"),
            content: None,
            anchor_rect: Rect::default(),
            visible: false,
            behavior: PopoverBehavior::default(),
            arrow: PopoverArrow::default(),
            placeholder: None,
            translucent: false,
            body_rect: Rect::default(),
            // At rest at the closed end: a freshly built popover is hidden, so it must not
            // animate itself away on its first frame.
            reveal: PropertyDriver::at(0.0, MotionSlot::Normal),
            about_to_show: GenericSignal::new(),
            shown: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
            dismissed: GenericSignal::new(),
        }
    }

    /// Shows the popover positioned relative to the given anchor rectangle.
    pub fn show(&mut self, anchor: Rect) {
        self.anchor_rect = anchor;
        if self.visible {
            // Already up: repositioning is not a lifecycle transition and must not replay the
            // show signals.
            self.base.request_redraw();
            return;
        }
        self.about_to_show.emit();
        self.visible = true;
        self.reveal.set_target(1.0);
        self.base.request_redraw();
        self.shown.emit();
    }

    /// Hides the popover.
    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.about_to_hide.emit();
        self.visible = false;
        self.reveal.set_target(0.0);
        self.base.request_redraw();
        self.dismissed.emit();
    }

    /// The edge of the anchor the card is placed against.
    pub fn placement(&self) -> PopoverEdge {
        match self.arrow {
            PopoverArrow::Auto => PopoverEdge::Auto,
            PopoverArrow::Down => PopoverEdge::Top,
            PopoverArrow::Up => PopoverEdge::Bottom,
            PopoverArrow::Right => PopoverEdge::Left,
            PopoverArrow::Left => PopoverEdge::Right,
        }
    }

    /// Places the card against `edge` and requests a redraw.
    ///
    /// This is the edge spelling of [`Popover::set_arrow_direction`]: the edge determines the
    /// arrow, and the single `arrow` field means the two setters can never disagree.
    pub fn set_placement(&mut self, edge: PopoverEdge) {
        let arrow = match edge {
            PopoverEdge::Auto => PopoverArrow::Auto,
            PopoverEdge::Top => PopoverArrow::Down,
            PopoverEdge::Bottom => PopoverArrow::Up,
            PopoverEdge::Left => PopoverArrow::Right,
            PopoverEdge::Right => PopoverArrow::Left,
        };
        self.set_arrow_direction(arrow);
    }

    /// Returns how far the card has unfolded, `0.0` closed and `1.0` fully open.
    ///
    /// The value the draw measures with, as distinct from the logical [`is_visible`](Self::is_visible).
    pub fn reveal_progress(&self) -> f32 {
        self.reveal.value()
    }

    /// Returns whether the popover is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Reports the popover's own shown-state.
    ///
    /// Deliberately distinct from [`Widget::is_visible`], which this widget
    /// overrides to return the popup state. The property contract publishes the
    /// inherited `visible` for the control, so the popup state is published under
    /// the `shown` name instead; without this separation the base property would
    /// be unreachable.
    pub fn is_shown(&self) -> bool {
        self.visible
    }

    /// Sets the popover's shown-state without moving its anchor.
    ///
    /// The counterpart to [`is_shown`](Self::is_shown). Showing keeps the existing
    /// anchor rectangle, so a caller that wants to reposition first uses
    /// [`show`](Self::show) with the new anchor.
    pub fn set_shown(&mut self, shown: bool) {
        if shown {
            self.show(self.anchor_rect);
        } else {
            self.hide();
        }
    }

    /// Returns the title of the popover's content widget, or an empty string when
    /// it has no content.
    ///
    /// A `Box<dyn Widget>` exposes no text of its own, so the concrete `Label`
    /// case is read through a downcast; anything else answers honestly with the
    /// empty string rather than inventing a rendering.
    pub fn content_text(&self) -> String {
        self.content
            .as_deref()
            .and_then(
                crate::widget::capability::coercion::widget_as::<
                    crate::widget::base_widgets::label::Label,
                >,
            )
            .map_or_else(String::new, |label| label.text().to_string())
    }

    /// Sets the content widget displayed inside the popover.
    ///
    /// The child has never been laid out: a control is constructed at a caller-chosen rectangle
    /// (usually `0x0`), and this card is the only thing that knows the box it should occupy. The
    /// first layout request is issued here rather than left to the first paint, so a host that
    /// is waiting for layout to settle before it shows the card sees that request (DLG-007).
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns a reference to the content widget, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the content widget, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Returns the anchor rectangle.
    pub fn anchor_rect(&self) -> Rect {
        self.anchor_rect
    }

    /// Returns how the card decides to close itself.
    pub fn behavior(&self) -> PopoverBehavior {
        self.behavior
    }

    /// Sets how the card decides to close itself and requests a redraw.
    ///
    /// A same-value write is a no-op. [`PopoverBehavior::Transient`] is the default and
    /// preserves the original outside-click-and-Escape dismissal.
    pub fn set_behavior(&mut self, behavior: PopoverBehavior) {
        if self.behavior == behavior {
            return;
        }
        self.behavior = behavior;
        self.base.request_redraw();
    }

    /// Returns the caller's arrow placement preference.
    pub fn arrow_direction(&self) -> PopoverArrow {
        self.arrow
    }

    /// Sets the arrow placement preference and requests a redraw.
    ///
    /// `Auto` keeps the original rule (below the anchor, flipped above when there is no
    /// room); the fixed directions pin the card to one side of the anchor.
    pub fn set_arrow_direction(&mut self, arrow: PopoverArrow) {
        if self.arrow == arrow {
            return;
        }
        self.arrow = arrow;
        self.base.request_redraw();
    }

    /// Returns the caller-supplied placeholder, if any.
    pub fn placeholder_text(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    /// Overrides the placeholder label; `None` restores the built-in labels.
    ///
    /// The built-in text distinguishes an empty card from a content-bearing one, so a
    /// caller that supplies its own string is choosing to lose that distinction. The
    /// placeholder is never painted once real content is installed unless the caller sets
    /// it explicitly.
    pub fn set_placeholder_text(&mut self, text: Option<String>) {
        let text = text.filter(|value| !value.is_empty());
        if self.placeholder == text {
            return;
        }
        self.placeholder = text;
        self.base.request_redraw();
    }

    /// Returns whether the card is painted semi-transparently.
    pub fn is_translucent(&self) -> bool {
        self.translucent
    }

    /// Enables or disables the translucent card fill and requests a redraw.
    ///
    /// The renderer has no backdrop-blur primitive, so this paints a genuinely
    /// semi-transparent card over the page rather than sampling and blurring it. The
    /// default is opaque, which is the frame the control has always painted.
    pub fn set_translucent(&mut self, translucent: bool) {
        if self.translucent == translucent {
            return;
        }
        self.translucent = translucent;
        self.base.request_redraw();
    }

    /// The label painted inside an open card.
    ///
    /// A caller-supplied placeholder wins; otherwise the built-in text reports whether
    /// content is installed.
    fn placeholder_label(&self) -> &str {
        match self.placeholder.as_deref() {
            Some(text) => text,
            None if self.content.is_some() => "Popover",
            None => "Popover (empty)",
        }
    }

    /// Sets the anchor rectangle.
    ///
    /// The card's placement is derived from the anchor at paint time, so moving the anchor and
    /// calling this is enough for the popover to follow it — no `show` call is required (DLG-003).
    pub fn set_anchor_rect(&mut self, rect: Rect) {
        if self.anchor_rect == rect {
            return;
        }
        self.anchor_rect = rect;
        self.base.request_redraw();
    }

    /// Re-derives the card rectangle from the current anchor and requests a repaint (DLG-003).
    ///
    /// [`Self::set_anchor_rect`] already invalidates the paint, so this is the explicit spelling
    /// for a host that changed the anchor control's geometry and wants one call that says so.
    /// It does nothing while the popover is hidden, matching the paint path.
    pub fn refresh_placement(&mut self) {
        if self.visible {
            self.base.request_redraw();
        }
    }

    /// Computes the popover body rectangle beside the anchor and the arrow's tip.
    ///
    /// `Auto` derives the side from the available space (below first, flipped above when below
    /// does not fit), which is the historical rule. A fixed direction is honoured as the
    /// *preference*, then the card is clamped inside the viewport on all four edges (DLG-011) —
    /// a card pinned to a caller-chosen side still belongs on the page.
    fn compute_layout(&self) -> (Rect, Point, ArrowDirection) {
        let viewport = self.geometry();
        let body_size = Size::new(viewport.width.clamp(100, 400), viewport.height.clamp(60, 400));
        // The card never grows past the area it is drawn into; an oversized card is clipped by
        // the clamp below rather than painted outside the control.
        let body_size =
            Size::new(body_size.width.min(viewport.width), body_size.height.min(viewport.height));

        let preferred = match self.arrow {
            PopoverArrow::Auto => {
                // Position below the anchor by default; flip above if that side does not fit.
                let below_space = viewport.y + viewport.height as i32
                    - (self.anchor_rect.y + self.anchor_rect.height as i32);
                let above_space = self.anchor_rect.y - viewport.y;
                let below_fits = below_space >= body_size.height as i32 + ARROW_SIZE;
                let above_fits = above_space >= body_size.height as i32 + ARROW_SIZE;
                if below_fits {
                    ArrowDirection::Up
                } else if above_fits {
                    ArrowDirection::Down
                } else {
                    ArrowDirection::Up
                }
            }
            PopoverArrow::Up => ArrowDirection::Up,
            PopoverArrow::Down => ArrowDirection::Down,
            PopoverArrow::Left => ArrowDirection::Left,
            PopoverArrow::Right => ArrowDirection::Right,
        };

        // The opposite side is the flip target. When the preferred side does not fit but its
        // opposite does, the card flips rather than being shoved away from the anchor.
        let opposite = match preferred {
            ArrowDirection::Up => ArrowDirection::Down,
            ArrowDirection::Down => ArrowDirection::Up,
            ArrowDirection::Left => ArrowDirection::Right,
            ArrowDirection::Right => ArrowDirection::Left,
        };
        let preferred_rect = self.place_beside(self.anchor_rect, body_size, preferred);
        let arrow_dir = if fits(preferred_rect, viewport) {
            preferred
        } else {
            let flipped = self.place_beside(self.anchor_rect, body_size, opposite);
            if fits(flipped, viewport) {
                opposite
            } else {
                preferred
            }
        };

        let raw = self.place_beside(self.anchor_rect, body_size, arrow_dir);
        let body_rect = clamp_into(raw, viewport);

        // The tip follows the **clamped** card, not the preferred position, and is kept inside
        // the card's own width so a card shifted off the anchor still points at something.
        let half_base = ARROW_SIZE / 2;
        let anchor_center_x = self.anchor_rect.x + self.anchor_rect.width as i32 / 2;
        let anchor_center_y = self.anchor_rect.y + self.anchor_rect.height as i32 / 2;
        let min_x = body_rect.x + half_base;
        let max_x = (body_rect.x + body_rect.width as i32 - half_base).max(min_x);
        let min_y = body_rect.y + half_base;
        let max_y = (body_rect.y + body_rect.height as i32 - half_base).max(min_y);
        let arrow_tip = match arrow_dir {
            ArrowDirection::Up => {
                Point::new(anchor_center_x.clamp(min_x, max_x), body_rect.y - ARROW_SIZE)
            }
            ArrowDirection::Down => Point::new(
                anchor_center_x.clamp(min_x, max_x),
                body_rect.y + body_rect.height as i32 + ARROW_SIZE,
            ),
            ArrowDirection::Left => {
                Point::new(body_rect.x - ARROW_SIZE, anchor_center_y.clamp(min_y, max_y))
            }
            ArrowDirection::Right => Point::new(
                body_rect.x + body_rect.width as i32 + ARROW_SIZE,
                anchor_center_y.clamp(min_y, max_y),
            ),
        };

        (body_rect, arrow_tip, arrow_dir)
    }

    /// The unclamped card position for `dir`, centred on the anchor's cross axis.
    fn place_beside(&self, anchor: Rect, body: Size, dir: ArrowDirection) -> Rect {
        let anchor_center_x = anchor.x + anchor.width as i32 / 2;
        let anchor_center_y = anchor.y + anchor.height as i32 / 2;
        match dir {
            ArrowDirection::Up => Rect::new(
                anchor_center_x - body.width as i32 / 2,
                anchor.y + anchor.height as i32 + ARROW_SIZE,
                body.width,
                body.height,
            ),
            ArrowDirection::Down => Rect::new(
                anchor_center_x - body.width as i32 / 2,
                anchor.y - body.height as i32 - ARROW_SIZE,
                body.width,
                body.height,
            ),
            ArrowDirection::Left => Rect::new(
                anchor.x + anchor.width as i32 + ARROW_SIZE,
                anchor_center_y - body.height as i32 / 2,
                body.width,
                body.height,
            ),
            ArrowDirection::Right => Rect::new(
                anchor.x - body.width as i32 - ARROW_SIZE,
                anchor_center_y - body.height as i32 / 2,
                body.width,
                body.height,
            ),
        }
    }
}

/// Whether `rect` lies entirely inside `viewport`.
fn fits(rect: Rect, viewport: Rect) -> bool {
    rect.x >= viewport.x
        && rect.y >= viewport.y
        && rect.x + rect.width as i32 <= viewport.x + viewport.width as i32
        && rect.y + rect.height as i32 <= viewport.y + viewport.height as i32
}

/// Clamps `rect` fully inside `viewport` on all four edges (DLG-011).
///
/// # Why four edges and not two
///
/// The placement path only floored the card at the viewport's left/top edge
/// (`.max(geom.x)`), so an anchor near the right or bottom edge pushed the card out the other
/// side of the page. Clamping both ends of both axes is the placement every popup in this
/// crate needs, so the arithmetic is one function rather than four `max` calls per side.
fn clamp_into(rect: Rect, viewport: Rect) -> Rect {
    let width = rect.width.min(viewport.width);
    let height = rect.height.min(viewport.height);
    let max_x = (viewport.x + viewport.width as i32 - width as i32).max(viewport.x);
    let max_y = (viewport.y + viewport.height as i32 - height as i32).max(viewport.y);
    Rect::new(rect.x.clamp(viewport.x, max_x), rect.y.clamp(viewport.y, max_y), width, height)
}

impl Widget for Popover {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::POPOVER_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::POPOVER_DEFAULT_HEIGHT,
        )
    }

    /// Advances the unfold by `delta_ms`, returning whether another frame is owed.
    ///
    /// The driver is what decides "settled": it reports false the step it reaches its target, so a
    /// settled popover costs no frames and the idle screen repaints nothing.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.reveal.tick(delta_ms)
    }

    /// Whether the popover is mid-unfold and owes the driver another frame.
    fn is_animating(&self) -> bool {
        self.reveal.is_moving()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published lifecycle name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "about_to_show" => Some(EventSignalRef::unit("about_to_show", &self.about_to_show)),
            "shown" => Some(EventSignalRef::unit("shown", &self.shown)),
            "about_to_hide" => Some(EventSignalRef::unit("about_to_hide", &self.about_to_hide)),
            "dismissed" => Some(EventSignalRef::unit("dismissed", &self.dismissed)),
            _ => None,
        }
    }
}

/// `Popover`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The legacy table served a `visible` arm for this kind, but there is a name
/// collision to resolve: this widget overrides [`Widget::is_visible`] to return its
/// *popup* state, so a `visible` arm here would shadow the shared `visible` that
/// [`base_property_get`] serves — the two would be indistinguishable and the base
/// one unreachable. The popup state is therefore published as `shown`, and bare
/// `visible` keeps meaning what it means for every other control.
impl WidgetProperties for Popover {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "shown" => Ok(CapabilityValue::Bool(self.is_shown())),
            "text" => Ok(CapabilityValue::String(self.content_text())),
            "behavior" => Ok(CapabilityValue::String(self.behavior.as_str().to_string())),
            "arrow_direction" => Ok(CapabilityValue::String(self.arrow.as_str().to_string())),
            "placeholder_text" => {
                Ok(CapabilityValue::String(self.placeholder.clone().unwrap_or_default()))
            }
            "translucent" => Ok(CapabilityValue::Bool(self.is_translucent())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "shown" => {
                self.set_shown(expect_bool(value)?);
                Ok(())
            }
            // The text mirrors the content widget, so writing it would either be
            // dropped by the next layout or require synthesising a `Label` the
            // caller never asked for. A caller that means to set it installs the
            // content widget through `set_content`.
            "text" => Err(CapabilityAccessError::ReadOnlyProperty),
            "behavior" => {
                let text = crate::widget::capability::coercion::expect_string(value)?;
                let behavior =
                    PopoverBehavior::from_token(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_behavior(behavior);
                Ok(())
            }
            "arrow_direction" => {
                let text = crate::widget::capability::coercion::expect_string(value)?;
                let arrow =
                    PopoverArrow::from_token(&text).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_arrow_direction(arrow);
                Ok(())
            }
            "placeholder_text" => {
                let text = crate::widget::capability::coercion::expect_string(value)?;
                self.set_placeholder_text(Some(text));
                Ok(())
            }
            "translucent" => {
                self.set_translucent(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "shown",
            "text",
            "behavior",
            "arrow_direction",
            "placeholder_text",
            "translucent",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `popover` publishes.
    ///
    /// Both names carry a value — `set_visible` the boolean the property route
    /// already accepts, `set_text` the content the popover mirrors and cannot be
    /// given as a bare string — so a payload-less invocation is reported as
    /// needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_visible" | "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Popover {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal —
        // and the whole card used to be skipped unless the popover was already showing — so
        // the census reported `ink = 0` *and* no response to a light/dark switch.
        //
        // The theme reads are the cached shared snapshots, so no manager lock is held across
        // the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("popover");
        // `popover` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and resolves to `theme.colors.background` — the window's own fill. A card
        // painted in that colour would be byte-identical to the frame behind it, so a resolved
        // surface equal to the window fill is re-derived a visible step away from it, the same
        // distinction `Colors::input_background` draws for a field.
        let window_fill = {
            crate::style::palette_shared().map(|colors| colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(40, 40, 40));
        let card = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != card)
            .unwrap_or_else(|| card.blend(&ink, 0.35));
        let muted_ink = ink.blend(&card, 0.45);

        // A hidden popover is still laid out, so the card it will occupy is visible before it
        // opens: the control used to paint nothing at rest. It is drawn at reduced opacity so
        // the two states stay distinguishable.
        let visible = self.visible;

        // How far the card has unfolded. At rest (0.0) it is the thin closed strip below; at 1.0
        // every dimension below is exactly what it was before this animation existed.
        let reveal = self.reveal.value();

        let (body_rect, arrow_tip, arrow_dir) = self.compute_layout();
        self.body_rect = body_rect;
        let (body_rect, arrow_tip) = if visible {
            (body_rect, arrow_tip)
        } else {
            // Anchor-less at rest: the card fills the control's own rect rather than being
            // positioned against an anchor rectangle that has never been set.
            (
                Rect::new(
                    rect.x + ARROW_SIZE,
                    rect.y,
                    rect.width.saturating_sub(ARROW_SIZE as u32),
                    rect.height,
                ),
                arrow_tip,
            )
        };
        // The body **unfolds downward from its own top edge**: the anchor-facing edge is the fixed
        // one, and that is what makes a popover read as belonging to the button it points at. At
        // `reveal = 1.0` the multiplication is an identity, so the settled card is unchanged to the
        // pixel — which is what keeps the resting snapshot valid.
        //
        // The floor is the *closed* height: a fully hidden card is a thin strip behind its own top
        // edge rather than a zero-height rectangle, so the control still has a shape when its
        // content is not up. It is a fraction of the **control**, not of the body, because the
        // body's own height is derived from the anchor and would make the closed strip's size
        // anchor-dependent.
        let closed_height = ((rect.height as f32 * REVEAL_MIN_SCALE) + 0.5) as u32;
        let revealed_height = ((body_rect.height as f32 * reveal) + 0.5) as u32;
        let body_rect = Rect::new(
            body_rect.x,
            body_rect.y,
            body_rect.width,
            revealed_height.max(closed_height),
        );
        let card = if self.translucent { card.with_alpha(TRANSLUCENT_CARD_ALPHA) } else { card };
        // One value drives the body, the shadow, the arrow and the ink alike, so they cannot get
        // out of step: a half-open card with a full-strength shadow would read as a rendering fault.
        let card = if visible { card } else { window_fill.blend(&card, 0.45) };
        let border = if visible { border } else { window_fill.blend(&border, 0.45) };

        // ── Draw shadow ──
        //
        // A real blur, not a second hard rectangle: the renderer's `BoxShadow` command is the one
        // primitive both backends already soften (three-pass box blur / `feGaussianBlur`), and the
        // previously offset solid fill read as a border rather than as elevation. `SHADOW_ALPHA` is
        // the weight and the reveal scales it, so the shadow grows with the unfold instead of
        // arriving at full strength beside a half-open card.
        //
        // The blur makes the shadow **bleed past the card by design**. Nothing in the render path
        // clips a widget to its layout slot, so the shadow is painted rather than dropped when it
        // would extend past the control's rectangle: the old "does it fit?" guard silently removed
        // the shadow at exactly the anchors where the card sits nearest an edge. No extra intrinsic
        // bounds are needed here, because the control's own rectangle is not a clip.
        if body_rect.height > 0 && reveal > 0.0 {
            // The shadow is gone at `reveal = 0`, where the card is a closed strip: a shadow under
            // a strip that has not opened yet is a smudge on the page.
            context.draw_box_shadow(
                body_rect,
                Color::rgba(0, 0, 0, (SHADOW_ALPHA as f32 * reveal) as u8),
                0,
                SHADOW_OFFSET_Y,
                SHADOW_BLUR_RADIUS,
                0,
            );
        }

        // ── Draw popover body ──
        if body_rect.height > 0 {
            context.fill_rounded_rect(body_rect, CORNER_RADIUS, card);
            context.draw_rounded_rect_stroke(body_rect, CORNER_RADIUS, border, 1);
        }

        // ── Draw arrow ──
        //
        // The arrow exists **only once the card is open**. While closed it would be a free-floating
        // triangle on the page with nothing behind it, which is what the census's closed-state
        // figure used to show; a closed popover *is* the thin strip, and nothing else.
        if visible && reveal > 0.0 {
            self.draw_arrow(context, arrow_tip, arrow_dir, card, border);
        }

        // ── Draw placeholder content indicator ──
        // The label reads the theme rather than the previous fixed grey, so a dark card does
        // not carry light-theme text.
        let content_padding = 8i32;
        // The label lives inside the card, so it is clipped to what has **unfolded so far** rather
        // than to the settled body: an unclipped label in a half-open card paints below the card's
        // own bottom edge, which is the one thing that would make the unfold visibly wrong. At
        // `reveal = 1.0` this box is exactly the padded body, so a settled label is unchanged.
        let content_rect = crate::core::Rect::new(
            body_rect.x + content_padding,
            body_rect.y + content_padding,
            body_rect.width.saturating_sub((content_padding as u32) * 2),
            body_rect.height.saturating_sub((content_padding as u32) * 2),
        );
        if content_rect.height == 0 {
            return;
        }

        // ── The content child ──
        //
        // DLG-007: the content was mounted and never visited by the paint path, so an open
        // popover with real content showed the empty-state placeholder and nothing else. The
        // card owns the box the child is drawn into (the child keeps its own internal layout),
        // and the child is drawn here because this control is the one that knows the unfolded
        // body rectangle. `popover content` is `Box<dyn Widget>`, so the paint entry point is
        // the shared `as_draw_mut` bridge the mounted tree uses.
        if let Some(content) = self.content.as_mut() {
            content.set_geometry(content_rect);
            content.show();
            if let Some(drawable) = content.as_draw_mut() {
                drawable.draw(context);
            }
        }

        // The placeholder says "this card is empty". Once real content is installed it would
        // paint over that content, so it is drawn only when the card is empty — unless the
        // caller explicitly set a placeholder (which is documented as their choice to keep).
        if self.content.is_some() && self.placeholder.is_none() {
            return;
        }

        // The **effective font** — the resolved theme/caller font — rather than the fixed 13 px
        // sans-serif this used to build (D09-STYLE-01).
        let font = effective_font(style);
        let label = self.placeholder_label();
        // The label is centred on the content box — **both ways**. `draw_text_fitted` aligns
        // horizontally only, so passing the padded content box pinned the label to the card's
        // top edge: `popover.svg` drew a 13 px line at `y = 8` inside a 120 px card. Deriving
        // the line box first is what makes «centred» true on the vertical axis too, and the
        // fit/ellipsis behaviour is unchanged (the box is 224 px wide at the census geometry
        // and `Popover (empty)` is wider than that).
        let line = context.text_line(content_rect, font);
        context.draw_text_fitted_cow(line, label, font, muted_ink, HorizontalAlignment::Center);
    }
}

/// Direction the popover arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

impl Popover {
    /// Draws the triangular arrow pointing toward the anchor.
    ///
    /// The fill and outline are passed in rather than literals so the arrow is the same
    /// colour as the card it belongs to — otherwise the arrow stayed white on a dark card.
    fn draw_arrow(
        &self,
        context: &mut RenderContext,
        tip: Point,
        dir: ArrowDirection,
        fill: Color,
        outline: Color,
    ) {
        let half_base = ARROW_SIZE / 2;
        let (_base_center, base_left, base_right) = match dir {
            ArrowDirection::Up => {
                let base_center = Point::new(tip.x, tip.y + ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y + ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y + ARROW_SIZE);
                (base_center, base_left, base_right)
            }
            ArrowDirection::Down => {
                let base_center = Point::new(tip.x, tip.y - ARROW_SIZE);
                let base_left = Point::new(tip.x - half_base, tip.y - ARROW_SIZE);
                let base_right = Point::new(tip.x + half_base, tip.y - ARROW_SIZE);
                (base_center, base_left, base_right)
            }
            ArrowDirection::Left => {
                let base_center = Point::new(tip.x + ARROW_SIZE, tip.y);
                let base_left = Point::new(tip.x + ARROW_SIZE, tip.y - half_base);
                let base_right = Point::new(tip.x + ARROW_SIZE, tip.y + half_base);
                (base_center, base_left, base_right)
            }
            ArrowDirection::Right => {
                let base_center = Point::new(tip.x - ARROW_SIZE, tip.y);
                let base_left = Point::new(tip.x - ARROW_SIZE, tip.y - half_base);
                let base_right = Point::new(tip.x - ARROW_SIZE, tip.y + half_base);
                (base_center, base_left, base_right)
            }
        };

        // Draw filled triangle using DrawPath
        let points = vec![tip, base_left, base_right];
        context.execute_command(RenderCommand::DrawPath {
            points: points.as_slice().into(),
            closed: true,
            color: fill,
            filled: true,
            width: 1,
        });
        // Draw triangle outline
        context.execute_command(RenderCommand::DrawPath {
            points: points.into(),
            closed: true,
            color: outline,
            filled: false,
            width: 1,
        });
    }
}

impl EventHandler for Popover {
    fn handle_event(&mut self, event: &Event) {
        if !self.visible {
            self.base.handle_event(event);
            return;
        }

        // A disabled popover must not act on input. `is_enabled` was unchecked here,
        // so `set_enabled(false)` still let a click outside — or Escape — close it,
        // which is a state change the caller explicitly asked to suspend.
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1
                    && self.behavior.closes_on_outside_click()
                    && !self.body_rect.contains_point(*pos)
                {
                    // Auto-dismiss on click outside
                    self.hide();
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == key_codes::ESCAPE && self.behavior.closes_on_escape() {
                    // Escape key
                    self.hide();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;
    use crate::widget::svg::render_to_svg;

    /// How far the empty-state placeholder's line box may sit from the card's middle and
    /// still count as centred.
    ///
    /// The line box is derived from a measured line height, so its top edge lands on a whole
    /// pixel that need not be the exact middle. One line's worth is generous enough to
    /// absorb that rounding while still rejecting a top-edge origin, which is half the card
    /// away.
    const LINE_TOLERANCE: i32 = 8;

    /// The `(y, height)` of the document's first **rounded** rectangle.
    ///
    /// The card is the popover's only rounded rectangle, and it is what both the placeholder's
    /// centring and its containment are measured against — the control's own rectangle is not,
    /// because the card tracks the anchor and need not be inside the control at all. Shared rather
    /// than repeated so the two assertions cannot drift onto different rectangles.
    fn first_rounded_rect(svg: &str) -> Option<(i32, i32)> {
        svg.split("<rect ").filter(|chunk| chunk.contains("rx=\"")).find_map(|chunk| {
            let y = chunk.split("y=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
            let h = chunk.split("height=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
            Some((y, h))
        })
    }

    /// A simple test widget used as content inside popover tests.
    struct TestContent {
        base: BaseWidget,
        text: String,
    }

    impl TestContent {
        fn new(text: &str, geometry: Rect) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, geometry, "TestContent"),
                text: text.to_string(),
            }
        }
    }

    impl Widget for TestContent {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn set_state_theme_hook(&mut self) {
            crate::style::reapply_active_theme_state(self);
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
    }

    impl Draw for TestContent {
        fn draw(&mut self, context: &mut RenderContext) {
            let font = Font::simple("sans-serif", 12.0);
            context.draw_text(
                Point::new(self.geometry().x, self.geometry().y),
                &self.text,
                &font,
                Color::BLACK,
                HorizontalAlignment::Left,
            );
        }
    }

    impl EventHandler for TestContent {
        fn handle_event(&mut self, _event: &Event) {}
    }

    /// A content stand-in that records the box it was painted into and lays down one colour.
    struct PaintedContent {
        base: BaseWidget,
        painted: std::rc::Rc<std::cell::Cell<Option<Rect>>>,
    }

    impl PaintedContent {
        fn new(painted: std::rc::Rc<std::cell::Cell<Option<Rect>>>) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 0, 0), "PaintedContent"),
                painted,
            }
        }
    }

    impl Widget for PaintedContent {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }

    impl Draw for PaintedContent {
        fn draw(&mut self, context: &mut RenderContext) {
            self.painted.set(Some(self.geometry()));
            context.fill_rect(self.geometry(), Color::rgb(3, 4, 5));
        }
    }

    impl EventHandler for PaintedContent {
        fn handle_event(&mut self, _event: &Event) {}
    }

    #[test]
    fn popover_default_creation() {
        let popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert_eq!(popover.kind(), WidgetKind::Popover);
        assert!(!popover.is_visible());
        assert!(popover.content().is_none());
        assert_eq!(popover.geometry(), Rect::new(0, 0, 300, 200));
    }

    #[test]
    fn popover_show_hide() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(!popover.is_visible());

        popover.show(Rect::new(100, 100, 50, 20));
        assert!(popover.is_visible());

        popover.hide();
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_anchor_rect() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        let anchor = Rect::new(100, 100, 50, 20);
        popover.show(anchor);
        assert_eq!(popover.anchor_rect(), anchor);

        let new_anchor = Rect::new(50, 50, 80, 30);
        popover.set_anchor_rect(new_anchor);
        assert_eq!(popover.anchor_rect(), new_anchor);
    }

    #[test]
    fn popover_set_content() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert!(popover.content().is_none());

        let content = TestContent::new("Hello", Rect::new(0, 0, 100, 30));
        popover.set_content(Box::new(content));
        assert!(popover.content().is_some());
    }

    /// The content child is visited by the paint path and placed in the card's body (DLG-007).
    ///
    /// # The defect this pins
    ///
    /// `set_content` mounted the child and the draw path never asked it to paint, so an open
    /// popover with real content showed the empty-state placeholder over a blank body.
    #[test]
    fn the_card_paints_its_content_child_inside_the_body() {
        let painted = std::rc::Rc::new(std::cell::Cell::new(None));
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        popover.set_content(Box::new(PaintedContent::new(std::rc::Rc::clone(&painted))));
        popover.show(Rect::new(100, 40, 50, 20));
        while popover.tick(1000) {}

        let svg = render_to_svg(&mut popover);
        assert!(
            svg.contains("rgba(3,4,5,1.00)"),
            "the installed content must reach the pixels: {svg}"
        );
        let inner = painted.get().expect("the content child was painted");
        let body = popover.body_rect;
        assert!(inner.width > 0 && inner.height > 0, "the child must get a real box: {inner:?}");
        assert!(
            inner.x >= body.x
                && inner.y >= body.y
                && inner.x + inner.width as i32 <= body.x + body.width as i32
                && inner.y + inner.height as i32 <= body.y + body.height as i32,
            "the child is placed inside the card: {inner:?} vs {body:?}"
        );
    }

    /// Installing content asks for its first layout (DLG-007).
    #[test]
    fn installing_content_requests_its_first_layout() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        let asked = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&asked);
        popover.base.layout_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        popover.set_content(Box::new(PaintedContent::new(std::rc::Rc::new(std::cell::Cell::new(
            None,
        )))));
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the new child has never been laid out");
    }

    #[test]
    fn popover_auto_dismiss_on_click_outside() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        let anchor = Rect::new(150, 100, 50, 20);
        popover.show(anchor);
        assert!(popover.is_visible());

        // Click far outside the body rect
        popover.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        assert!(!popover.is_visible());
    }

    #[test]
    fn popover_escape_key_dismisses() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.show(Rect::new(150, 100, 50, 20));
        assert!(popover.is_visible());

        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!popover.is_visible());
    }

    /// A disabled popover must not close on a click outside or on Escape.
    ///
    /// `handle_event` checked `visible` but never `enabled`, so a caller that
    /// suspended the popover with `set_enabled(false)` still had it dismissed by a
    /// stray click. Auto-dismissing is exactly the kind of state change `enabled`
    /// exists to gate.
    #[test]
    fn popover_disabled_ignores_dismissal_input() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.show(Rect::new(150, 100, 50, 20));
        popover.set_enabled(false);

        // Click far outside the body rect: would normally auto-dismiss.
        popover.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        assert!(popover.is_visible(), "a disabled popover must not auto-dismiss");

        // Escape: would normally dismiss.
        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(popover.is_visible(), "a disabled popover must not close on Escape");

        // Re-enabling must restore the behaviour, so the gate is a suspension and not
        // a permanent lock.
        popover.set_enabled(true);
        popover.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(!popover.is_visible(), "re-enabling must restore dismissal");
    }

    #[test]
    fn popover_svg_output_visible() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        popover.show(Rect::new(100, 100, 50, 20));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    /// Opening a popover unfolds it rather than placing it there at full height.
    ///
    /// # The defect this pins
    ///
    /// `show`/`hide` carried only the logical state and the draw read it directly, so the card
    /// appeared whole on the frame it was shown and vanished on the frame it was hidden. The
    /// assertions are in two halves, because a progress nothing reads is not an unfold: the model
    /// must take an interior value, and the *painted* card must be shorter at that moment.
    #[test]
    fn opening_a_popover_unfolds_it() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        assert_eq!(popover.reveal_progress(), 0.0, "a fresh popover is closed");
        assert!(!popover.is_animating(), "and owes no frames");

        popover.show(Rect::new(100, 100, 50, 20));
        assert!(popover.is_visible(), "the logical state answers at once");
        assert!(popover.is_animating(), "while the drawn card owes frames");

        // The painted card's height, read from the document: the card is the only rounded
        // rectangle the *open* popover paints that is at least as wide as the arrow gap.
        fn card_height(svg: &str) -> u32 {
            svg.split("<rect ")
                .filter(|chunk| chunk.contains("rx=\""))
                .filter_map(|chunk| {
                    let h = chunk.split("height=\"").nth(1)?;
                    h.split('"').next()?.parse::<u32>().ok()
                })
                .max()
                .unwrap_or(0)
        }

        assert!(popover.tick(20), "still unfolding after one step");
        let mid = popover.reveal_progress();
        assert!(
            mid > 0.0 && mid < 1.0,
            "the popover must pass through an interior reveal (got {mid})"
        );
        let mid_height = card_height(&render_to_svg(&mut popover));

        while popover.tick(1000) {}
        assert_eq!(popover.reveal_progress(), 1.0, "and settle fully open");
        let open_height = card_height(&render_to_svg(&mut popover));

        assert!(
            mid_height > 0 && mid_height < open_height,
            "an unfolding card must be shorter than a settled one: mid={mid_height} open={open_height}"
        );
    }

    #[test]
    fn popover_svg_output_hidden() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut popover = Popover::new(Rect::new(0, 0, 300, 200));
        let svg = render_to_svg(&mut popover);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // A closed popover is a **thin strip**, not a blank page: the card it will occupy is
        // already there at its closed height, so a control that has been created but not opened
        // still has a rendered extent instead of vanishing. `draw` used to `return` early here,
        // which made the control invisible at rest — the defect the rendering census reported as
        // `ink = 0`. The card is a rounded rect, so it is read as the rounded `fill` elements.
        let rounded = svg.matches("rx=\"").count();
        assert!(rounded > 0, "a closed popover must still paint its card: {svg}");
        // The placeholder label is **not** painted while the popover is closed: it lives inside
        // the card, which has no interior at that height, so painting it would put ink outside
        // the card's own bottom edge.
        assert!(
            crate::widget::svg::text_ink_box(&svg).is_none(),
            "a closed popover paints no interior label: {svg}"
        );

        // A closed popover and an open one must differ: the open card is opaque, is at its full
        // height, carries the placeholder and carries its anchor arrow; the closed one is a strip.
        //
        // This geometry deliberately positions the control so the whole anchored card fits inside
        // it: the card tracks the anchor, so at an anchor near the top edge the settled card sits
        // *below* the control's bottom — which is where the census-and-gate-relevant form of the
        // assertion, "the label is inside the card", belongs. It is asserted against the card's own
        // rectangle below rather than against the control's, because that is what is true.
        let mut open = Popover::new(Rect::new(0, 0, 300, 400));
        open.show(Rect::new(100, 20, 50, 20));
        while open.tick(1000) {}
        let shown = render_to_svg(&mut open);
        assert_ne!(svg, shown, "showing the popover must change what is painted");
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&shown)
            .unwrap_or_else(|| panic!("the placeholder label must be painted when open: {shown}"));
        assert!(right > left, "the placeholder laid down ink: {left}..{right}");
        let (card_y, card_h) = first_rounded_rect(&shown).expect("the open popover paints a card");
        assert!(
            top >= card_y && bottom <= card_y + card_h,
            "the label must sit inside the card: ink {top}..{bottom}, card {card_y}..{}",
            card_y + card_h
        );
    }

    /// The empty-state placeholder is centred in the card on **both** axes.
    ///
    /// It was pinned to the card's top edge while `draw_text_fitted` was being handed the
    /// padded content box directly — that method aligns horizontally only, so its origin is
    /// `bounds.y` unchanged. Deriving the line box with `context.text_line` first is what
    /// centres it vertically, and this asserts the *result* rather than the call, so a
    /// future change that reintroduces a bare `bounds.y` fails here.
    ///
    /// The assertion is on the **ink**, not on an element's `y` attribute: text leaves the
    /// backend as the `font8x8` rectangles the rasteriser fills, so there is no `y` to read and
    /// no baseline to undo — the document holds a picture of the run. That is the stronger
    /// check, because the old form verified the coordinate the backend had written down rather
    /// than where the ink actually landed.
    ///
    /// The centre is read from the **card's own** rectangle in the document, not from the
    /// control's: the card is positioned against the anchor, so it need not be the control's
    /// middle (at this anchor it sits at `y = 130` inside a 120-tall control). Measuring against
    /// the control would have been asserting the anchor, not the centring.
    #[test]
    fn popover_empty_placeholder_is_centred_in_the_card() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut popover = Popover::new(rect);
        // The placeholder lives inside the card, so it is only painted once the card is open.
        popover.show(Rect::new(100, 100, 50, 20));
        while popover.tick(1000) {}
        let svg = render_to_svg(&mut popover);
        let (left, top, right, bottom) = crate::widget::svg::text_ink_box(&svg)
            .expect("the empty-state placeholder must be drawn");
        assert!(right > left && bottom > top, "the placeholder laid down ink: {svg}");

        // The card: the rounded rectangle that is not the control's background. It is the one
        // the label is centred in, so it is the one the centre is measured against.
        let (card_y, card_h) = first_rounded_rect(&svg).expect("the open popover paints a card");
        let middle = card_y + card_h / 2;
        assert!(
            (top - middle).abs() <= LINE_TOLERANCE,
            "the empty-state placeholder must be vertically centred in the card: top={top}, \
             card centre={middle} (card y={card_y} h={card_h})"
        );
        // And it must sit *below* the card's padding, which a top-edge origin would not.
        assert!(
            top >= card_y + 8,
            "a top-edge origin would pin the placeholder at the padding, got {top} in card y={card_y}"
        );
    }

    /// The open card casts a **blurred** shadow, not a second hard rectangle.
    ///
    /// The old shadow was the card's own shape offset by two pixels and filled solid, which
    /// reads as a second border. The assertion is on the renderer's box-blur marker
    /// (`feGaussianBlur` and its filter reference) rather than on any fill alpha, so it fails
    /// if the shadow regresses to a rounded-rect fill.
    #[test]
    fn an_open_popover_paints_a_blurred_shadow() {
        let rect = Rect::new(0, 0, 300, 400);
        let mut popover = Popover::new(rect);
        popover.show(Rect::new(100, 20, 50, 20));
        while popover.tick(1000) {}

        let svg = render_to_svg(&mut popover);
        assert!(
            svg.contains(&format!("shadow_blur_{SHADOW_BLUR_RADIUS}")),
            "the shadow must go through the box-blur primitive: {svg}"
        );
        assert!(
            svg.contains(&format!("filter=\"url(#shadow_blur_{SHADOW_BLUR_RADIUS})\"")),
            "the emitted shadow rectangle must reference the blur filter: {svg}"
        );
    }

    /// The blur does not make the shadow disappear where the card reaches the control's edge.
    ///
    /// This is the geometry where the card (and therefore the shadow) extends below the 120 px
    /// control: the old `shadow_fits` guard dropped the shadow entirely there. The renderer does
    /// not clip a widget to its slot, so the shadow must still be emitted.
    #[test]
    fn the_shadow_is_not_dropped_when_it_extends_past_the_control() {
        let mut popover = Popover::new(Rect::new(0, 0, 240, 120));
        popover.show(Rect::new(100, 100, 50, 20));
        while popover.tick(1000) {}

        let svg = render_to_svg(&mut popover);
        assert!(
            svg.contains(&format!("shadow_blur_{SHADOW_BLUR_RADIUS}")),
            "the shadow must survive a card that sits past the control's bottom edge: {svg}"
        );
    }

    /// A closed popover paints no shadow; the reveal drives the shadow to nothing.
    #[test]
    fn a_closed_popover_paints_no_shadow() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        assert_eq!(popover.reveal_progress(), 0.0);
        let svg = render_to_svg(&mut popover);
        assert!(!svg.contains("shadow_blur_"), "a closed popover must not cast a shadow: {svg}");
    }

    /// The close rules are a property, not a hard-coded pair of arms.
    #[test]
    fn behavior_decides_which_gestures_close_the_card() {
        let outside = Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 };
        let escape = Event::KeyPress { key: 27, modifiers: 0 };

        let mut semi = Popover::new(Rect::new(0, 0, 400, 400));
        semi.show(Rect::new(150, 100, 50, 20));
        semi.set_behavior(PopoverBehavior::SemiTransient);
        semi.handle_event(&escape);
        assert!(semi.is_visible(), "a semi-transient popover ignores Escape");
        semi.handle_event(&outside);
        assert!(!semi.is_visible(), "an outside click still closes it");

        let mut permanent = Popover::new(Rect::new(0, 0, 400, 400));
        permanent.show(Rect::new(150, 100, 50, 20));
        permanent.set_behavior(PopoverBehavior::Permanent);
        permanent.handle_event(&escape);
        assert!(permanent.is_visible());
        permanent.handle_event(&outside);
        assert!(permanent.is_visible(), "a permanent popover closes only when asked");

        assert!(PopoverBehavior::from_token("semi-transient")
            .expect("hyphenated spelling")
            .closes_on_outside_click());
        assert_eq!(
            PopoverBehavior::from_token("semi_transient").map(PopoverBehavior::as_str),
            Some("semi_transient")
        );
    }

    /// The horizontal arrow variants place the card beside the anchor, not above/below it.
    ///
    /// The fixtures leave room on the requested side: a card clamped inside the viewport
    /// (DLG-011) cannot sit beside an anchor that has no space beside it.
    #[test]
    fn a_horizontal_arrow_places_the_card_beside_the_anchor() {
        let mut left = Popover::new(Rect::new(0, 0, 700, 500));
        left.set_arrow_direction(PopoverArrow::Left);
        left.show(Rect::new(100, 100, 40, 20));
        while left.tick(1000) {}
        let _ = render_to_svg(&mut left);
        assert!(
            left.body_rect.x >= 140,
            "a left-pointing arrow puts the card to the anchor's right: card x={}",
            left.body_rect.x
        );

        let mut right = Popover::new(Rect::new(0, 0, 700, 500));
        right.set_arrow_direction(PopoverArrow::Right);
        right.show(Rect::new(450, 100, 40, 20));
        while right.tick(1000) {}
        let _ = render_to_svg(&mut right);
        assert!(
            right.body_rect.x + right.body_rect.width as i32 <= 450,
            "a right-pointing arrow puts the card to the anchor's left: card right={}",
            right.body_rect.x + right.body_rect.width as i32
        );
        assert_eq!(PopoverArrow::from_token("left"), Some(PopoverArrow::Left));
    }

    /// The card is clamped on all four edges, and an auto-placement flips when its first side
    /// does not fit (DLG-011).
    #[test]
    fn the_card_is_clamped_into_the_viewport_and_flips_when_it_must() {
        // The viewport is larger than the card's 400 px cap, so the clamp has room to act.
        let mut clamped = Popover::new(Rect::new(0, 0, 700, 900));
        clamped.set_arrow_direction(PopoverArrow::Left);
        clamped.show(Rect::new(650, 100, 40, 20));
        while clamped.tick(1000) {}
        let _ = render_to_svg(&mut clamped);
        let card = clamped.body_rect;
        assert!(card.x >= 0, "the left edge stays inside the page: {card:?}");
        assert!(card.y >= 0, "the top edge stays inside the page: {card:?}");
        assert!(
            card.x + card.width as i32 <= 700,
            "the right edge is clamped instead of overflowing: {card:?}"
        );
        assert!(
            card.y + card.height as i32 <= 900,
            "the bottom edge is clamped instead of overflowing: {card:?}"
        );

        // An anchor near the bottom has no room below: the card flips above it.
        let mut flipped = Popover::new(Rect::new(0, 0, 700, 900));
        flipped.show(Rect::new(100, 600, 40, 20));
        while flipped.tick(1000) {}
        let _ = render_to_svg(&mut flipped);
        let card = flipped.body_rect;
        assert!(
            card.y + card.height as i32 <= 600,
            "the card flips to the anchor's other side: {card:?}"
        );
    }

    /// The placeholder is caller-configurable instead of hard-coded.
    #[test]
    fn the_placeholder_text_is_configurable() {
        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        popover.show(Rect::new(100, 20, 50, 20));
        while popover.tick(1000) {}
        let built_in = render_to_svg(&mut popover);

        popover.set_placeholder_text(Some("No notifications".to_string()));
        let custom = render_to_svg(&mut popover);
        assert_ne!(built_in, custom, "a custom placeholder must reach the pixels");
        // An empty string means "use the built-in label", not "paint nothing".
        popover.set_placeholder_text(Some(String::new()));
        assert_eq!(popover.placeholder_text(), None);
    }

    /// The translucent material is opt-in and paints a genuinely alpha-blended card.
    #[test]
    fn the_translucent_material_is_opt_in() {
        fn card_alpha(svg: &str) -> f32 {
            let rect = svg
                .split("<rect ")
                .filter(|chunk| chunk.contains("rx=\""))
                .find(|chunk| chunk.contains("fill=\"rgba("))
                .expect("the card carries a fill");
            let fill = rect.split("rgba(").nth(1).expect("rgba fill");
            let channels: Vec<&str> =
                fill.split(')').next().expect("close paren").split(',').collect();
            channels[3].trim().parse::<f32>().expect("an alpha channel")
        }

        let mut opaque = Popover::new(Rect::new(0, 0, 300, 400));
        opaque.show(Rect::new(100, 20, 50, 20));
        while opaque.tick(1000) {}
        assert!(!opaque.is_translucent(), "the default card is opaque");
        let opaque_svg = render_to_svg(&mut opaque);

        let mut translucent = Popover::new(Rect::new(0, 0, 300, 400));
        translucent.set_translucent(true);
        translucent.show(Rect::new(100, 20, 50, 20));
        while translucent.tick(1000) {}
        let translucent_svg = render_to_svg(&mut translucent);

        assert_eq!(card_alpha(&opaque_svg), 1.0, "an opaque card is fully opaque");
        let expected = f32::from(TRANSLUCENT_CARD_ALPHA) / 255.0;
        assert!(
            (card_alpha(&translucent_svg) - expected).abs() < 0.01,
            "the translucent card must carry the material's alpha: got {}",
            card_alpha(&translucent_svg)
        );
        assert_ne!(opaque_svg, translucent_svg);
    }

    #[test]
    fn the_show_and_hide_lifecycle_is_observable() {
        use std::sync::{Arc, Mutex};

        let mut popover = Popover::new(Rect::new(0, 0, 300, 400));
        let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));
        let sink = Arc::clone(&events);
        popover.about_to_show.connect({
            let sink = Arc::clone(&sink);
            move || sink.lock().expect("lock").push("about_to_show")
        });
        popover.shown.connect({
            let sink = Arc::clone(&sink);
            move || sink.lock().expect("lock").push("shown")
        });
        popover.about_to_hide.connect({
            let sink = Arc::clone(&sink);
            move || sink.lock().expect("lock").push("about_to_hide")
        });
        popover.dismissed.connect(move || sink.lock().expect("lock").push("dismissed"));

        popover.show(Rect::new(10, 10, 40, 20));
        assert_eq!(
            *events.lock().expect("lock"),
            vec!["about_to_show", "shown"],
            "showing announces both halves of the transition"
        );
        // Repositioning an open card is not a second show.
        popover.show(Rect::new(20, 20, 40, 20));
        assert_eq!(events.lock().expect("lock").len(), 2);

        popover.hide();
        assert_eq!(
            *events.lock().expect("lock"),
            vec!["about_to_show", "shown", "about_to_hide", "dismissed"]
        );
        // Hiding a hidden card is not a second dismissal.
        popover.hide();
        assert_eq!(events.lock().expect("lock").len(), 4);
    }

    #[test]
    fn placement_selects_the_arrow_that_points_at_the_anchor() {
        let mut popover = Popover::new(Rect::new(0, 0, 400, 400));
        popover.set_placement(PopoverEdge::Bottom);
        assert_eq!(popover.placement(), PopoverEdge::Bottom);
        assert_eq!(popover.arrow_direction(), PopoverArrow::Up);
        popover.set_placement(PopoverEdge::Top);
        assert_eq!(popover.arrow_direction(), PopoverArrow::Down);
        popover.set_placement(PopoverEdge::Left);
        assert_eq!(popover.arrow_direction(), PopoverArrow::Right);
        popover.set_placement(PopoverEdge::Right);
        assert_eq!(popover.arrow_direction(), PopoverArrow::Left);
        popover.set_placement(PopoverEdge::Auto);
        assert_eq!(popover.arrow_direction(), PopoverArrow::Auto);
    }
}
