// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Radio button widget.
use crate::compat::{String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler, FocusReason};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics, FocusRing, FOCUS_RING_WIDTH};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Radius of the indicator disc, in logical pixels.
///
/// A constant, for the same reason [`crate::widget::CheckBox`]'s indicator is: the disc is
/// chrome the control owns, while the rectangle it is laid out in belongs to whoever placed
/// it. `min(w, h) / 4` made a 240x120 census cell draw a 60 px circle, so the same control
/// was a different shape in every layout.
///
/// The value comes from the shared table (`dimensions::RADIO_OUTER_RADIUS`) rather than being
/// restated here, so the indicator, the ring's stroke and the dot cannot drift apart — and so
/// a checkbox and a radio, which sit side by side in a form, stay within a pixel of each
/// other's size.
const INDICATOR_RADIUS: u32 = dimensions::RADIO_OUTER_RADIUS;

/// Stroke width of the indicator's ring.
const RING_WIDTH: u32 = dimensions::RADIO_STROKE;

/// Distance from the control's left edge to the ring's outer edge.
///
/// Only a few pixels: a radio button's indicator is close to its own edge, which is what
/// lets several of them in a group line up as a column of discs.
const INDICATOR_INSET: i32 = 1;

/// The line box a single line of `font` occupies, centred vertically in `rect`.
///
/// A copy of what `RenderContext::text_line` computes, for the sizing path: `hit_area` is a
/// `&self` query with no render context to hand, and it must place the indicator exactly
/// where the painter will. Keeping the arithmetic in one place here — rather than restating
/// `rect.height / 2` at each of the two call sites — is what stops the pair drifting.
fn vertical_line_box(rect: Rect, font: &Font) -> Rect {
    let height = font.size().max(1.0) as u32;
    let height = height.min(rect.height);
    Rect::new(rect.x, rect.y + (rect.height.saturating_sub(height) / 2) as i32, rect.width, height)
}

/// Radio button widget.
pub struct RadioButton {
    base: BaseWidget,
    checked: bool,
    group_id: Option<String>,
    text: String,
    /// Whether this control owns keyboard focus.
    focused: bool,
    /// Why it got focus — decides whether a focus ring is painted.
    focus_reason: FocusReason,
    /// Whether the pointer is over the control.
    hovered: bool,
    /// Emitted without a payload when this button becomes the selected member of
    /// its peer group. Only fires on a `false` -> `true` transition; deselection
    /// does not emit.
    pub selected: GenericSignal,
    /// Emitted with the new state after `checked` changes, in both directions.
    pub checked_changed: Signal1<bool>,
}
impl RadioButton {
    /// The gap between the indicator and the label.
    ///
    /// Read from the style so a theme can tune it, falling back to the shared table. This is
    /// what `spacing` means throughout the crate: the distance from a control's *own*
    /// indicator to its *own* text — never the distance between two siblings, which is the
    /// parent layout's decision (QML draws the same line: `CheckBox.qml:61` uses `spacing`
    /// for this pair only).
    fn label_gap(&self) -> i32 {
        self.style().spacing.unwrap_or(dimensions::INDICATOR_TEXT_SPACING) as i32
    }

    /// The indicator's centre and radius.
    ///
    /// # Why one function, not two
    ///
    /// The hit test and the painter must agree on where the disc is and how big it is, or a
    /// press lands in a place that looks empty (or misses a place that looks like the
    /// control). They used to be derived independently — the hit side from `rect.height`, the
    /// paint side from the label's line box — so a press in the indicator's corner was inside
    /// the hit area but outside the drawn ring. Deriving both from here makes that class of
    /// drift unrepresentable.
    ///
    /// `line` supplies the *row* the disc is centred on. The disc's own diameter is fixed
    /// chrome, so it is not clamped to the line's height — only to the control's rectangle.
    fn indicator_geometry(&self, line: &Rect) -> (Point, u32) {
        let rect = self.geometry();
        let row_centre_y = line.y + line.height as i32 / 2;
        let diameter = (INDICATOR_RADIUS * 2).min(rect.width).min(rect.height);
        let radius = diameter / 2;
        let center_x = rect.x + INDICATOR_INSET + radius as i32;
        (Point::new(center_x.min(rect.x + rect.width as i32 - radius as i32), row_centre_y), radius)
    }

    /// The region a press must land in to select this radio button.
    ///
    /// The indicator plus the label beside it, **not** the whole rectangle the caller laid out.
    ///
    /// This handler used to ignore the pointer entirely (`MousePress { pos: _, .. }`), so a press
    /// anywhere in the control's rectangle selected it. A radio button given a wide row by its
    /// layout therefore selected when the user clicked empty space well to the right of its own
    /// label. Testing the control's **contents** is what every toolkit does — `QRadioButton`
    /// reacts to its indicator and text — and it is a different statement from "the minimum touch
    /// target is at least N points", which then widens this region rather than replacing it.
    fn hit_area(&self) -> Rect {
        let rect = self.geometry();
        // The line box, derived the same way `RenderContext::text_line` derives it: a single
        // line of `Font::default()` centred in the control's rectangle. The two must agree,
        // because the disc's vertical position follows the line.
        let line = vertical_line_box(rect, &Font::default());
        let (center, radius) = self.indicator_geometry(&line);
        let indicator =
            Rect::new(center.x - radius as i32, center.y - radius as i32, radius * 2, radius * 2);
        // The interactive region is the **contents** — indicator through the end of the
        // label — not the whole laid-out rectangle. A radio given a wide row by its layout
        // must not select when the user clicks empty space far to the right of its label.
        let contents = if self.text.is_empty() {
            // No label to reach, so the indicator alone is the target.
            indicator
        } else {
            // The label's width is an *estimate* (`chars * 3/5 em`), deliberately conservative:
            // the region must never extend past the text the user can see, and this path has no
            // render context to measure with. The painter may therefore draw a slightly wider
            // label than the hit region — the safe direction.
            let label_width = self.text.chars().count() as u32 * (line.height * 3 / 5).max(1);
            Rect::new(
                indicator.x,
                indicator.y,
                indicator.width + self.label_gap() as u32 + label_width,
                indicator.height,
            )
        };
        match self.style().touch_target {
            Some(min_size) => contents.expand_to_touch_target(min_size),
            None => contents,
        }
    }

    /// Creates an unchecked radio button with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RadioButton, geometry, "RadioButton"),
            checked: false,
            group_id: None,
            text: String::new(),
            focused: false,
            focus_reason: FocusReason::Programmatic,
            hovered: false,
            selected: GenericSignal::new(),
            checked_changed: Signal1::new(),
        }
    }
    /// Returns current checked state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Returns the text label displayed next to the radio button.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets the text label displayed next to the radio button and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }
    /// Sets optional group identifier.
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
    }
    /// Returns optional group identifier.
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets checked state and emits deterministic signals.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.checked_changed.emit(checked);
        if checked {
            self.selected.emit();
        }
    }
    /// Selects one radio button within a peer group.
    pub fn select_in_group(peers: &mut [&mut RadioButton], selected_index: usize) -> bool {
        if selected_index >= peers.len() {
            return false;
        }
        let selected_group = peers[selected_index].group_id.clone();
        for (index, peer) in peers.iter_mut().enumerate() {
            if selected_group.is_some() && peer.group_id != selected_group {
                continue;
            }
            peer.set_checked(index == selected_index);
        }
        true
    }

    /// Whether this control currently owns keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether a focus ring should be painted right now.
    ///
    /// The same single predicate every control uses — `focused && reason.draws_focus_ring()`
    /// — so "has focus" cannot be mistaken for "draw the ring" in one control and not in
    /// another.
    pub fn visual_focus(&self) -> bool {
        self.focused && self.focus_reason.draws_focus_ring()
    }

    /// Whether the pointer is over this control.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the hovered flag and requests a redraw.
    pub fn set_hovered(&mut self, hovered: bool) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        self.base.request_redraw();
    }
}
// Implement Widget trait
impl Widget for RadioButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let text_w = self.text().len() as u32 * 8 + 24;
        Size::new(text_w.max(75), 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `RadioButton`'s property contract.
///
/// `group_id` is optional, so it is published as `Null` when unset and accepts
/// `Null` on write — matching the old dispatch, which cleared the group on
/// `Null` and parsed a string otherwise.
impl WidgetProperties for RadioButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "group_id" => match self.group_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "group_id" => {
                match value {
                    CapabilityValue::Null => self.set_group_id(None),
                    other => self.set_group_id(Some(expect_string(other)?)),
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "checked", "group_id", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `radio_button` publishes.
    ///
    /// Both published names assign state (`set_checked`, `set_group_id`) and so need
    /// a payload; neither can be a zero-argument action. They are therefore refused
    /// as [`CapabilityAccessError::OutOfRange`], which tells the caller the name is
    /// right and the value belongs on the property route — not `UnknownCommand`,
    /// which would say the name does not exist.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_checked" | "set_group_id" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for RadioButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if self.hit_area().contains_point(*pos) {
                    self.set_checked(true);
                    self.base.clicked.emit();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.hit_area().contains_point(*pos) => {
                self.set_checked(true);
                self.base.clicked.emit();
            }
            // A `Tap` carries no position, so it is accepted as-is: the platform has already
            // resolved it to this control, which is the same basis `hit_area` narrows.
            #[cfg(feature = "touch")]
            Event::Tap { .. } => {
                self.set_checked(true);
                self.base.clicked.emit();
            }
            Event::KeyPress { key, .. } if *key == 32 || *key == 13 => {
                // Space or Enter
                self.set_checked(true);
                self.base.clicked.emit();
            }
            Event::FocusGained { reason } => {
                self.focused = true;
                self.focus_reason = *reason;
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.base.request_redraw();
            }
            Event::MouseEnter { .. } => {
                self.hovered = true;
                self.base.request_redraw();
            }
            Event::MouseLeave { .. } => {
                self.hovered = false;
                self.base.request_redraw();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for RadioButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let enabled = self.base.is_enabled();
        let font = Font::default();
        let line = context.text_line(rect, &font);

        // The indicator is a **fixed-size** disc at the left edge, vertically centred on the
        // label's own line box, not a fraction of the caller's rectangle. `min(w, h) / 4` made
        // a 240x120 census cell draw a 60 px circle centred on the middle of the cell — which
        // is neither the size nor the position of a radio button, and it moved the label to the
        // control's centre as well. The fixed radius is the one every toolkit uses because a
        // radio's disc is chrome the control owns, while the rectangle is the caller's.
        //
        // Derived through `indicator_geometry`, the *same* function the hit test uses, so the
        // disc the user sees and the disc the user must hit are the same circle. They used to be
        // computed separately — one from `rect.height`, one from the label's line box.
        let (center, radius) = self.indicator_geometry(&line);

        let ink = style.text_color.unwrap_or_else(|| {
            // The control paints no fill of its own, so the ink is derived from the surface
            // the theme already resolved for this control. `text_color` is normally set, so
            // this only covers a control whose style never met the theme.
            let surface = style.background_color.unwrap_or(Color::WHITE);
            if enabled {
                surface.contrast_color()
            } else {
                surface.contrast_color().with_alpha(150)
            }
        });

        // The ring is a stroke, so it is drawn as one rather than as a filled disc with a
        // second disc punched out: two stacked discs at this radius leave a seam where the
        // anti-aliased edges meet.
        //
        // A hovered control steps the ring one shade toward its own ink, which is how a radio
        // acknowledges the pointer without needing a ripple layer.
        let ink =
            if self.hovered && enabled { ink.blend(&ink.contrast_color(), 0.25) } else { ink };
        let ring = if enabled { ink } else { ink.with_alpha(140) };
        context.draw_circle_stroke(center, radius, ring, RING_WIDTH);

        if self.checked {
            // The dot is the control's *meaning*, so it takes the accent — the same token a
            // checked switch's track takes. It used to read `style.background_color`, which is
            // the **surface** the control sits on: on a default theme that painted a near-white
            // dot on a near-white surface, so a checked radio was indistinguishable from an
            // unchecked one, and the doc comment claiming "the caller's or the theme's accent"
            // described a colour the code never read.
            //
            // The caller's explicit colour still wins, so a themed radio can be re-coloured;
            // the accent rung is what a *theme-derived* style falls through to.
            #[cfg(device_profile)]
            let accent = crate::style::semantic_color(crate::style::SemanticColor::Info);
            #[cfg(not(device_profile))]
            let accent: Option<Color> = None;
            let explicit = if style.theme_derived { None } else { style.background_color };
            let dot = explicit.or(accent).unwrap_or(ink);
            let dot = if enabled { dot } else { dot.with_alpha(140) };
            // Sized from the shared table rather than as a ratio of the ring: Flutter's
            // inner/outer ratio is 0.5625, which this table rounds to a fixed radius so the
            // dot cannot drift when the outer radius moves.
            let dot_radius = dimensions::RADIO_DOT_RADIUS.min(radius.saturating_sub(RING_WIDTH));
            context.fill_circle(center, dot_radius, dot);
        }

        if !self.text.is_empty() {
            let label_x = center.x + radius as i32 + self.label_gap();
            context.draw_text_fitted(
                Rect::new(
                    label_x,
                    line.y,
                    rect.width.saturating_sub((label_x - rect.x) as u32),
                    line.height,
                ),
                &self.text,
                &font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // ── Focus ring ──
        //
        // Inset inside the control's own rectangle, and gated on the *reason* focus arrived so a
        // click focuses without painting a ring.
        if self.visual_focus() {
            let ring_outer = FocusRing::for_control(
                rect,
                ControlMetrics::focus_ring_radius(dimensions::RADIO_OUTER_RADIUS),
            );
            if ring_outer.is_drawable() {
                context.draw_rounded_rect_stroke(
                    ring_outer.rect,
                    ring_outer.radius,
                    crate::widget::metrics::focus_ring_color(ink.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::Rect;
    use crate::core::Size;
    use crate::style::WidgetStyle;

    // -----------------------------------------------------------------------
    // 1. Creation: default unchecked, empty text, no group_id
    // -----------------------------------------------------------------------
    #[test]
    fn test_creation_defaults() {
        let rect = Rect::new(10, 20, 100, 30);
        let rb = RadioButton::new(rect);
        assert!(!rb.is_checked(), "new radio button should be unchecked");
        assert_eq!(rb.text(), "", "new radio button text should be empty");
        assert_eq!(rb.group_id(), None, "new radio button should have no group_id");
        assert_eq!(rb.geometry(), rect, "geometry should match");
    }

    // -----------------------------------------------------------------------
    // 2. set_checked(true/false) state changes
    // -----------------------------------------------------------------------
    #[test]
    fn test_set_checked_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    #[test]
    fn test_set_checked_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
    }

    #[test]
    fn test_set_checked_toggle() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    // -----------------------------------------------------------------------
    // 3. checked_changed signal (on true, on false, not on noop)
    // -----------------------------------------------------------------------
    #[test]
    fn test_checked_changed_emitted_on_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to true"
        );
        assert!(
            fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be true"
        );
    }

    #[test]
    fn test_checked_changed_emitted_on_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to false"
        );
        assert!(
            !fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be false"
        );
    }

    #[test]
    fn test_checked_changed_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |_val| {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // First set to true -> should fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should fire once on true");
        // Set to true again (noop) -> should NOT fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should NOT fire on noop");
    }

    // -----------------------------------------------------------------------
    // 4. selected signal (on becoming true, not on uncheck)
    // -----------------------------------------------------------------------
    #[test]
    fn test_selected_emitted_on_becoming_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "selected signal should fire when checked becomes true"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_uncheck() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on uncheck"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false); // already false -> noop
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on noop"
        );
        rb.set_checked(true); // becomes true -> fires
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "selected signal should fire once on becoming true"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Text set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_text_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.text(), "");
        rb.set_text("Option A".to_string());
        assert_eq!(rb.text(), "Option A");
    }

    #[test]
    fn test_text_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("First".to_string());
        assert_eq!(rb.text(), "First");
        rb.set_text("Second".to_string());
        assert_eq!(rb.text(), "Second");
    }

    #[test]
    fn test_text_empty_after_set() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("Something".to_string());
        rb.set_text(String::new());
        assert_eq!(rb.text(), "");
    }

    // -----------------------------------------------------------------------
    // 6. Group ID set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_group_id_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.group_id(), None);
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
    }

    #[test]
    fn test_group_id_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group_a".to_string()));
        assert_eq!(rb.group_id(), Some("group_a"));
        rb.set_group_id(Some("group_b".to_string()));
        assert_eq!(rb.group_id(), Some("group_b"));
    }

    #[test]
    fn test_group_id_clear() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
        rb.set_group_id(None);
        assert_eq!(rb.group_id(), None);
    }

    // -----------------------------------------------------------------------
    // 7. select_in_group static method
    // -----------------------------------------------------------------------
    #[test]
    fn test_select_in_group_selects_correct_peer() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb2 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));
        rb2.set_group_id(Some("g".to_string()));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1, &mut rb2];
        let result = RadioButton::select_in_group(&mut peers, 1);
        assert!(result, "select_in_group should return true on success");
        drop(peers);
        assert!(!rb0.is_checked(), "peer 0 should be unchecked");
        assert!(rb1.is_checked(), "peer 1 should be checked");
        assert!(!rb2.is_checked(), "peer 2 should be unchecked");
    }

    #[test]
    fn test_select_in_group_deselects_others() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));

        rb0.set_checked(true);
        assert!(rb0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);
        assert!(!rb0.is_checked(), "previously checked peer 0 should be deselected");
        assert!(rb1.is_checked(), "peer 1 should be selected");
    }

    #[test]
    fn test_select_in_group_leaves_other_groups() {
        let mut rb_a0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_a1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        rb_a0.set_group_id(Some("A".to_string()));
        rb_a1.set_group_id(Some("A".to_string()));
        rb_b0.set_group_id(Some("B".to_string()));
        rb_b1.set_group_id(Some("B".to_string()));

        rb_a0.set_checked(true);
        rb_b0.set_checked(true);
        assert!(rb_a0.is_checked());
        assert!(rb_b0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb_a0, &mut rb_a1, &mut rb_b0, &mut rb_b1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);

        assert!(!rb_a0.is_checked(), "group A peer 0 should be deselected");
        assert!(rb_a1.is_checked(), "group A peer 1 should be selected");
        assert!(rb_b0.is_checked(), "group B peer 0 should remain checked");
        assert!(!rb_b1.is_checked(), "group B peer 1 should remain unchecked");
    }

    #[test]
    fn test_select_in_group_out_of_bounds_returns_false() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        let result = RadioButton::select_in_group(&mut peers, 5);
        assert!(!result, "out-of-bounds index should return false");
        drop(peers);
        assert!(!rb0.is_checked());
        assert!(!rb1.is_checked());
    }

    #[test]
    fn test_select_in_group_empty_slice_returns_false() {
        let mut empty_peers: Vec<&mut RadioButton> = vec![];
        let result = RadioButton::select_in_group(&mut empty_peers, 0);
        assert!(!result, "empty slice should return false");
    }

    // -----------------------------------------------------------------------
    // 8. Widget trait delegation
    // -----------------------------------------------------------------------
    #[test]
    fn test_widget_id_kind() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.id(), rb.base.id());
        assert_eq!(rb.kind(), WidgetKind::RadioButton);
    }

    #[test]
    fn test_widget_geometry() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.geometry(), Rect::new(0, 0, 50, 20));
        rb.set_geometry(Rect::new(10, 10, 80, 30));
        assert_eq!(rb.geometry(), Rect::new(10, 10, 80, 30));
    }

    #[test]
    fn test_widget_min_max_size() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.min_size().is_none());
        assert!(rb.max_size().is_none());
        rb.set_min_size(Some(Size::new(20, 10)));
        rb.set_max_size(Some(Size::new(200, 100)));
        assert_eq!(rb.min_size(), Some(Size::new(20, 10)));
        assert_eq!(rb.max_size(), Some(Size::new(200, 100)));
    }

    #[test]
    fn test_widget_parent_children() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let pid = 42u64;
        assert!(rb.parent().is_none());
        rb.set_parent(Some(pid));
        assert_eq!(rb.parent(), Some(pid));
        rb.set_parent(None);
        assert!(rb.parent().is_none());

        let cid = 100u64;
        assert!(rb.children().is_empty());
        rb.add_child(cid);
        assert_eq!(rb.children(), &[cid]);
        rb.remove_child(cid);
        assert!(rb.children().is_empty());
    }

    #[test]
    fn test_widget_visibility() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_visible());
        rb.hide();
        assert!(!rb.is_visible());
        rb.show();
        assert!(rb.is_visible());
    }

    #[test]
    fn test_widget_enabled() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_enabled());
        rb.set_enabled(false);
        assert!(!rb.is_enabled());
        rb.set_enabled(true);
        assert!(rb.is_enabled());
    }

    #[test]
    fn test_widget_tooltip() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.tooltip(), "");
        rb.set_tooltip("Click me".to_string());
        assert_eq!(rb.tooltip(), "Click me");
    }

    #[test]
    fn test_widget_style() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let style = WidgetStyle::default();
        rb.set_style(style.clone());
        let _ = rb.style();
    }

    #[test]
    fn test_widget_signals_exist() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.hover_signal();
        let _ = rb.mouse_down_signal();
        let _ = rb.mouse_up_signal();
        let _ = rb.key_down_signal();
        let _ = rb.key_up_signal();
        let _ = rb.focus_gained_signal();
        let _ = rb.focus_lost_signal();
        let _ = rb.redraw_requested_signal();
        let _ = rb.layout_requested_signal();
    }

    #[test]
    fn test_widget_connection_scope() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.connection_scope();
    }

    #[test]
    fn test_widget_request_redraw_signal() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // Setting text triggers request_redraw, which should emit the signal
        rb.set_text("Test".to_string());
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "redraw_requested_signal should fire when text is set"
        );
    }
}
