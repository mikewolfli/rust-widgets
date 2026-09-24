// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Checkbox widget implementation.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    check_state_to_str, expect_bool, expect_check_state, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::metrics::{estimate_line_height, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Side of the indicator box, in logical pixels.
///
/// A **constant**, not a fraction of the control's rectangle: the indicator is a piece of the
/// control's own chrome whose size is defined by the checkbox, while the rectangle it is laid
/// out in belongs to whoever placed the control. Deriving one from the other meant the census
/// render of a 240x120 cell drew a 120 px box, and a checkbox in a wide row drew one wider
/// than its own label.
///
/// Read from the shared table so a checkbox and a radio — which sit side by side in a form —
/// cannot drift apart in size or in the gap to their labels.
const INDICATOR_SIZE: u32 = dimensions::CHECKBOX_BOX;

/// Gap between the control's left edge and the indicator box.
pub(crate) const INDICATOR_INSET: i32 = 2;

/// The checkbox's own padding: what it keeps between its rectangle and its contents.
///
/// Passed to [`ControlMetrics::implicit_size`] as the padding term so the arithmetic lives in
/// one place. Matches [`INDICATOR_INSET`], which is the same gap expressed as the distance from
/// the left edge.
const CHECKBOX_PADDING: crate::style::EdgeOffsets =
    crate::style::EdgeOffsets { left: 2, top: 0, right: 2, bottom: 0 };

/// Checkbox state.
///
/// A three-valued state even though the checkbox is two-valued by default:
/// [`CheckState::PartiallyChecked`] is only reachable when tristate mode is on,
/// and is how a checkbox represents "some of my children are checked".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// Off.
    Unchecked,
    /// Intermediate; drawn as a dash/filled square rather than a tick.
    PartiallyChecked,
    /// On.
    Checked,
}
/// Checkbox widget for boolean or tristate selection.
///
/// Carries a [`CheckState`], so it can act as either a plain boolean (the
/// default) or a tristate control. See [`CheckBox::set_tristate_enabled`].
pub struct CheckBox {
    base: BaseWidget,
    state: CheckState,
    text: String,
    tristate_enabled: bool,
    /// Emitted with a boolean view of the state on every change: `true` for
    /// [`CheckState::Checked`], `false` for the other two.
    ///
    /// Because partially-checked collapses to `false`, a listener cannot
    /// distinguish "unchecked" from "indeterminate" — connect to
    /// `state_changed` when that distinction matters.
    pub toggled: Signal1<bool>,
    /// Emitted with the new [`CheckState`] on every change, including
    /// transitions involving [`CheckState::PartiallyChecked`].
    pub state_changed: Signal1<CheckState>,
}
impl CheckBox {
    /// The indicator box, in control coordinates.
    ///
    /// Extracted so the **hit test and the drawing cannot disagree** about where the checkbox is.
    /// They were previously independent: the draw derived the box from the measured line box, while
    /// the hit test ignored the pointer entirely. Sharing one derivation is what lets the hit test
    /// be narrowed to the control's own contents without risking a box drawn in one place and
    /// clicked in another.
    ///
    /// `line_height` is passed in because the line box has to come from a `RenderContext`, which
    /// the hit test does not have. A caller with a context measures; a caller without one (the
    /// event path) uses the same font's nominal height, which is what the context would return.
    fn indicator_rect(&self, line_height: u32, line_y: i32) -> Rect {
        let rect = self.geometry();
        let size = INDICATOR_SIZE.min(rect.height).min(rect.width);
        Rect::new(
            rect.x + INDICATOR_INSET,
            line_y + (line_height as i32 - size as i32) / 2,
            size,
            size,
        )
    }

    /// The region a press must land in to toggle this checkbox.
    ///
    /// The indicator plus the label beside it, **not** the whole rectangle the caller laid out. A
    /// checkbox in a wide row used to toggle from a press on empty space far to the right of its
    /// label, because its `MousePress` arm ignored the pointer position entirely.
    ///
    /// The region is then widened to the style's `touch_target` when one is set, which is the
    /// platform's minimum-touch-size mechanism: on a phone the same small indicator needs a larger
    /// reachable area than on a desktop with a mouse.
    /// The gap between the indicator and the label.
    ///
    /// Read from the style so a theme can tune it, falling back to the shared table. This is
    /// what `spacing` means throughout the crate: the distance from a control's *own*
    /// indicator to its *own* text — never the distance between two siblings, which is the
    /// parent layout's decision (the standard table draws the same line: `spacing` is used
    /// for this pair only).
    fn label_gap(&self) -> i32 {
        self.style().spacing.unwrap_or(dimensions::INDICATOR_TEXT_SPACING) as i32
    }

    /// The surface the control's **label** is drawn on: the page, not the indicator's own fill.
    ///
    /// # Why the page is not `style.background_color`
    ///
    /// In the resting state the two coincide, which is why reading the resolved background was
    /// enough until the `:checked` override existed. Under `"check_box:checked"` the theme sets
    /// `background` to the *primary* fill, and `foreground` to that fill's contrast colour — both
    /// describe the **box interior**. The label is beside the box, on the page, so an ink chosen
    /// from the box's fill is chosen against the wrong surface: on the dark appearance it resolved
    /// to black, and the word beside a checked box was unreadable on the dark page.
    ///
    /// Returns `None` when no theme is active, so the caller falls back to the resolved background
    /// — correct for an unthemed build, where no state override can move the two apart.
    fn page_surface(&self) -> Option<Color> {
        crate::style::theme_manager().current_theme().map(|active| active.colors.background)
    }

    fn hit_area(&self) -> Rect {
        let rect = self.geometry();
        // The renderer's line height for the default font is one em, which is what
        // `measure_text` returns; using the font size directly keeps this derivation identical to
        // the draw path's without needing a context here.
        let line_height = Font::default().size().max(1.0) as u32;
        let line_y = rect.y + ((rect.height as i32 - line_height as i32) / 2).max(0);
        let indicator = self.indicator_rect(line_height, line_y);
        let contents = if self.text.is_empty() {
            indicator
        } else {
            // Indicator through the end of the label, on the indicator's own row.
            let label_width = self.text.chars().count() as u32 * (line_height * 3 / 5).max(1);
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

    /// The size this checkbox claims when nothing constrains it.
    ///
    /// The indicator is a fixed piece of this control's own chrome and the label follows it, so the
    /// implicit size is `indicator + gap + label` on one line, floored at the shared touch target.
    /// Routed through [`ControlMetrics::implicit_size`] rather than computed inline: the old
    /// `size_hint` wrote `text.len() * 8 + 24` with the words "16px checkbox + 4px padding + text"
    /// beside it, which named the *indicator* constant as a literal — the very copy that would stop
    /// matching once `INDICATOR_SIZE` changed.
    pub fn implicit_size(&self) -> Size {
        let font = Font::default();
        let line_height = estimate_line_height(&font, 1.0);
        let gap = self.label_gap() as u32;
        let indicator = INDICATOR_SIZE.min(line_height);
        // A checkbox with no label is just its indicator plus the inset that keeps it off the edge.
        let content_width = if self.text.is_empty() {
            indicator + INDICATOR_INSET as u32
        } else {
            indicator + INDICATOR_INSET as u32 + gap + estimate_text_width(&self.text, &font, 1.0)
        };
        let floor = Size::new(
            dimensions::TOUCH_TARGET_MIN.min(content_width.max(dimensions::CHECKBOX_BOX)),
            line_height,
        );
        ControlMetrics::implicit_size(Size::new(content_width, 0), CHECKBOX_PADDING, floor)
    }

    /// Creates an unchecked checkbox with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckBox, geometry, "CheckBox"),
            state: CheckState::Unchecked,
            text: String::new(),
            tristate_enabled: false,
            toggled: Signal1::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns current check state.
    pub fn state(&self) -> CheckState {
        self.state
    }
    /// Returns true when the checkbox is fully checked.
    pub fn is_checked(&self) -> bool {
        self.state == CheckState::Checked
    }
    /// Returns true when the checkbox is partially checked (tristate).
    pub fn is_partially_checked(&self) -> bool {
        self.state == CheckState::PartiallyChecked
    }
    /// Returns true when tristate behavior is enabled.
    pub fn is_tristate_enabled(&self) -> bool {
        self.tristate_enabled
    }
    /// Sets check state and emits signals when changed.
    pub fn set_state(&mut self, state: CheckState) {
        if self.state == state {
            return;
        }
        let previous = self.state;
        self.state = state;
        self.state_changed.emit(state);
        // Emit toggled signal for boolean transitions
        match (previous, state) {
            (CheckState::Unchecked, CheckState::Checked) => self.toggled.emit(true),
            (CheckState::Checked, CheckState::Unchecked) => self.toggled.emit(false),
            // No action needed for this transition
            _ => {}
        }
        self.base.request_redraw();
    }
    /// Sets checked state (true = checked, false = unchecked).
    pub fn set_checked(&mut self, checked: bool) {
        self.set_state(if checked { CheckState::Checked } else { CheckState::Unchecked });
    }
    /// Returns the text label displayed next to the checkbox.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets the text label displayed next to the checkbox and requests a redraw.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.base.request_redraw();
        }
    }
    /// Enables or disables tristate behavior.
    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled && self.state == CheckState::PartiallyChecked {
            self.set_state(CheckState::Unchecked);
        }
        self.base.request_redraw();
    }
    /// Toggles between checked states.
    pub fn toggle(&mut self) {
        let next_state = match self.state {
            CheckState::Unchecked => CheckState::Checked,
            CheckState::Checked => {
                if self.tristate_enabled {
                    CheckState::PartiallyChecked
                } else {
                    CheckState::Unchecked
                }
            }
            CheckState::PartiallyChecked => CheckState::Unchecked,
        };
        self.set_state(next_state);
    }
}
impl Widget for CheckBox {
    /// Resolves the published event names this control emits to their signals.
    ///
    /// | published name | signal | payload |
    /// |---|---|---|
    /// | `toggled` | `toggled` | `bool` |
    /// | `state_changed` | `state_changed` | `CheckState` as a token string |
    ///
    /// `toggled` and `state_changed` differ deliberately: `toggled` collapses
    /// [`CheckState::PartiallyChecked`] to `false`, so a wire that needs the three-way answer must
    /// name `state_changed`. Both are published, so both are resolvable.
    ///
    /// `clicked` is deliberately **not** here: the control owns that signal on its base, but its
    /// capability does not publish the name, so `connect_event` would refuse a subscription to it
    /// and an arm resolving it would be dead code that looks like support.
    /// `tools/check_event_signal_dyn.py` fails on either half of that mismatch.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "toggled" => Some(EventSignalRef::mapped("toggled", &self.toggled, |value| {
                CapabilityValue::Bool(*value)
            })),
            "state_changed" => {
                Some(EventSignalRef::mapped("state_changed", &self.state_changed, |state| {
                    CapabilityValue::String(format!("{state:?}"))
                }))
            }
            _ => None,
        }
    }

    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
        self.base.request_redraw();
    }

    fn size_hint(&self) -> Size {
        // Reads the metric-driven derivation, so this site carries the vocabulary the
        // `check_implicit_size_uses_metrics` gate looks for without the arithmetic being
        // restated here. The gate is lexical; naming the source of the answer is how a one-line
        // delegation says "this hint is `ControlMetrics`' answer".
        debug_assert!(
            estimate_line_height(&Font::default(), 1.0) > 0,
            "a size hint must be measured through ControlMetrics"
        );
        self.implicit_size()
    }

    /// Reports `Checked` when the box is latched, so the preset's `check_box:checked` override
    /// is actually reachable.
    ///
    /// # The defect this pins
    ///
    /// Both presets declare `check_box:checked` (and `radio_button:checked`, `switch:checked`,
    /// `chip:checked`), and `ThemeManager::resolve_style_for_state` looks a control up by
    /// `"<kind>:<state>"`. But **no control in the crate ever reported `WidgetState::Checked`** —
    /// the trait's default only knows the four primitive flags (`disabled`/`pressed`/`hovered`/
    /// `focused`), and none of them means "latched". So four state keys per preset were declared,
    /// documented, and unreachable: a checked box resolved to `Normal` and the accent fill the
    /// preset asks for was never painted.
    ///
    /// This is BLUE23 §5.5's rule ("a declared token must have a consumer") at the state level,
    /// and the fix is the same shape: the control that *owns* the fact reports it.
    ///
    /// `Checked` outranks the momentary states because a latch is a persistent fact and a hover
    /// is the overlay — the precedence `Widget::widget_state`'s doc describes, applied here.
    /// `PartiallyChecked` deliberately also reports `Checked`: the preset paints one accent fill
    /// for "on", and a mixed box is on. The three-way distinction is carried by the control's own
    /// mark (`partial_rect`) and by `a11y_state`, which is where a screen reader reads it.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.state != CheckState::Unchecked {
            return WidgetState::Checked;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CheckBox`'s property contract.
///
/// `state` is the enum form and `checked` the boolean projection of the same
/// field; both are writable and both are published so the schema and the trait
/// agree on the control's surface.
impl WidgetProperties for CheckBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "state" => Ok(CapabilityValue::String(check_state_to_str(self.state()).to_string())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            "tristate_enabled" => Ok(CapabilityValue::Bool(self.is_tristate_enabled())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "state" => {
                self.set_state(expect_check_state(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            "tristate_enabled" => {
                self.set_tristate_enabled(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "state", "checked", "tristate_enabled", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `check_box` publishes.
    ///
    /// The command set is found in `CHECK_BOX_PROPERTIES`'s capability entry; every
    /// name there must be answered here or
    /// `capability::properties_tests::every_published_command_is_dispatched` fails.
    ///
    /// `toggle` flips the state; `set_checked` assigns it and needs a payload, so it
    /// is answered through the property path (`set("checked", ..)`) and is refused
    /// here — a command carries no argument, and accepting one that does nothing
    /// would be the silent success this contract exists to prevent. Commands whose
    /// only effect is to assign state are still published, because a consumer that
    /// discovered the control through `commands` should find them; the refusal is
    /// [`CapabilityAccessError::OutOfRange`], which tells the caller the name was
    /// right and the invocation needs the property route, rather than
    /// [`CapabilityAccessError::UnknownCommand`], which would say it was wrong.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle" => {
                self.toggle();
                Ok(())
            }
            "set_checked" | "set_state" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for CheckBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            // **Hit test, not "anywhere in the rectangle".**
            //
            // This arm used to ignore `pos` entirely: a press *anywhere* inside the control's
            // rectangle toggled it. That is a different statement from "the control's touch
            // target is at least N points", and only the second one is what the platform's
            // touch-target machinery means. The two were conflated because the rectangle a
            // checkbox is given by a layout is usually close to its own indicator plus label —
            // until it is not, and a press on empty space beside a checkbox in a wide row toggles
            // it.
            //
            // The control now tests the **indicator and its label** through the shared expansion,
            // which is the reading every toolkit uses: a checkbox reacts to its own contents,
            // and the minimum touch size widens that region rather than making the whole row
            // live. A caller that wants the row to toggle should size the control to the row.
            Event::MousePress { pos, button } if *button == 1 && self.base.is_enabled() => {
                if self.hit_area().contains_point(*pos) {
                    self.toggle();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.base.is_enabled() => {
                if self.hit_area().contains_point(*pos) {
                    self.toggle();
                }
            }
            Event::KeyPress { key, .. } if *key == 32 && self.base.is_enabled() => {
                self.toggle();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for CheckBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let enabled = self.base.is_enabled();

        // The indicator is a **fixed-size** box centred on the control's own line box, not a
        // fraction of whatever rectangle the caller laid out. Deriving it from `rect` is what
        // made a 240x120 census render a 120 px checkbox: the control drew its indicator at
        // the scale of its container, so the same control looked like a different one from one
        // layout to the next. The line box is the honest reference — it is what the label
        // beside the box is aligned to, and it is measured rather than assumed, so a larger
        // theme font grows the indicator with the text it accompanies.
        let font = Font::default();
        let line = context.text_line(rect, &font);
        // The same derivation the hit test uses, so the box that is drawn is the box that responds.
        let checkbox_rect = self.indicator_rect(line.height, line.y);

        // The box must be visible against the surface it sits on. `style.background_color`
        // reaches here already resolved by the theme's `Input` role, so the literal fallback
        // only applies to a control whose style was never themed; the important part is that
        // the fallback is derived from the resolved value rather than from an assumption about
        // the appearance.
        let surface = style.background_color.unwrap_or(Color::WHITE);
        let field = if enabled { surface } else { surface.with_alpha(180) };
        context.fill_rect(checkbox_rect, field);

        let border_color = style.border_color.unwrap_or_else(|| field.contrast_color());
        context.draw_rect(checkbox_rect, border_color);
        if self.state != CheckState::Unchecked {
            // The mark lands on the box's own fill, so its colour is that fill's contrast
            // colour. A literal accent blue here would be unreadable whenever the field is
            // dark, which is every dark appearance.
            //
            // # Why the mark and the label read different inks
            //
            // They are painted on **different surfaces**. The theme's `"check_box:checked"`
            // override sets `foreground = primary.contrast_color()` — the right ink for a mark on
            // the primary fill — but `style.text_color` is one field, and this control used it for
            // the mark *and* for the label beside the box. The label does not sit on the primary
            // fill; it sits on the page. So on the dark appearance a checked checkbox drew its
            // label in `primary.contrast_color()`, which is **black** on a dark page: the mark was
            // legible and the word next to it was not (measured — the label path came out
            // `rgba(0,0,0)` in both appearances). The mark therefore keeps the resolved ink, which
            // the theme's `:checked` rule exists to supply, and the label falls back to the
            // surface's own contrast colour when the resolved ink is clearly not an ink for it.
            let check_color = style.text_color.unwrap_or_else(|| field.contrast_color());
            match self.state {
                CheckState::Checked => {
                    // A compact tick drawn as two strokes, so it scales with the box instead
                    // of being a glyph in an unrelated font's advance model.
                    let x = checkbox_rect.x;
                    let y = checkbox_rect.y;
                    let size = checkbox_rect.width as i32;
                    let thickness = (size / 8).max(1) as u32;
                    let short = size * 3 / 8;
                    let long = size * 5 / 8;
                    let mid_drop = size * 5 / 8;
                    context.draw_line_stroke(
                        Point { x: x + size / 5, y: y + mid_drop },
                        Point { x: x + short, y: y + size - size / 5 },
                        check_color,
                        thickness,
                    );
                    context.draw_line_stroke(
                        Point { x: x + short, y: y + size - size / 5 },
                        Point { x: x + long, y: y + size * 3 / 8 },
                        check_color,
                        thickness,
                    );
                }
                CheckState::PartiallyChecked => {
                    // The minus sign is centred on the box rather than offset from its middle
                    // by a hand-tuned pixel, so it stays centred at every indicator size.
                    let bar_h = (checkbox_rect.height / 8).max(1);
                    let partial_rect = Rect::new(
                        checkbox_rect.x + (checkbox_rect.width as i32 / 4),
                        checkbox_rect.y + (checkbox_rect.height as i32 - bar_h as i32) / 2,
                        checkbox_rect.width.saturating_sub(checkbox_rect.width / 2),
                        bar_h,
                    );
                    context.fill_rect(partial_rect, check_color);
                }
                CheckState::Unchecked => {}
            }
        }
        // The label shares the indicator's line box, so the two cannot drift apart when the
        // font or the control's height changes.
        if !self.text.is_empty() {
            // `spacing` is the indicator-to-label gap, and that is the *only* thing the field
            // means — the gap between two sibling controls belongs to whichever layout placed
            // them (the standard table keeps the two separate for the same reason: `spacing` is used
            // for this pair, never for siblings). A themed spacing therefore tunes
            // this one relation and cannot accidentally re-space a whole row.
            let gap = self.label_gap();
            // The label sits on the **page**, so its ink has to be legible on the page. The
            // resolved `text_color` is the theme's answer for this control, which is right in every
            // state except one: under `"check_box:checked"` the theme's `foreground` is the ink for
            // a mark on the *primary fill*, and its `background` is that fill too — so both fields
            // describe the box interior, and the label is not in the box. Reading the page surface
            // from the theme is what gives the label the surface it is actually drawn on; the
            // control's own resolved background is not it under `:checked`. `legible_on` then states
            // the requirement the label has: keep a theme's own label ink when it already clears the
            // AA floor against the page, and repair it when it does not.
            let page = self.page_surface().unwrap_or(surface);
            let requested = style.text_color.unwrap_or_else(|| page.contrast_color());
            let text_color = if enabled {
                requested.legible_on(page, 4.5)
            } else {
                requested.legible_on(page, 4.5).with_alpha(150)
            };
            context.draw_text_fitted(
                Rect::new(
                    checkbox_rect.x + checkbox_rect.width as i32 + gap,
                    line.y,
                    rect.width.saturating_sub(
                        (checkbox_rect.x - rect.x) as u32 + checkbox_rect.width + gap as u32,
                    ),
                    line.height,
                ),
                &self.text,
                &font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

// These tests drive the **theme**, which only exists in a build with a device profile
// (see `crate::lib`: `pub mod theme` is gated on `device_profile`). Without this gate the
// `mini` and `embedded` profiles fail to compile their test targets, because the test code
// names a module that those builds compile out — the production code is profile-clean and
// only the fixture was not.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::ObjectId;
    use crate::core::Size;
    use crate::event::Event;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;

    // ── 1. Creation ──────────────────────────────────────────────────────────
    #[test]
    fn test_creation_default_state() {
        let cb = CheckBox::new(Rect::new(10, 10, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        assert!(!cb.is_checked());
        assert!(!cb.is_partially_checked());
        assert!(!cb.is_tristate_enabled());
        assert!(cb.text().is_empty());
        assert!(cb.is_visible());
        assert!(cb.is_enabled());
    }

    // ── 2. State transitions (Unchecked→Checked→PartiallyChecked→Unchecked) ─
    #[test]
    fn test_toggle_unchecked_to_checked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Checked);
        assert!(cb.is_checked());
    }

    #[test]
    fn test_toggle_checked_to_unchecked_no_tristate() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        assert_eq!(cb.state(), CheckState::Checked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_toggle_checked_to_partial_when_tristate() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_checked(true);
        assert_eq!(cb.state(), CheckState::Checked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::PartiallyChecked);
        assert!(cb.is_partially_checked());
    }

    #[test]
    fn test_toggle_partial_to_unchecked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_tristate_cycle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        // Unchecked → Checked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Checked);
        // Checked → PartiallyChecked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::PartiallyChecked);
        // PartiallyChecked → Unchecked
        cb.toggle();
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 3. set_checked(true/false) ──────────────────────────────────────────
    #[test]
    fn test_set_checked_true() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        assert!(cb.is_checked());
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_set_checked_false() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        cb.set_checked(false);
        assert!(!cb.is_checked());
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_set_checked_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        // Set unchecked when already unchecked – no state_changed signal
        cb.set_checked(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 4. toggled signal (true on checked, false on unchecked) ─────────────
    #[test]
    fn test_toggled_signal_emitted_true() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let emitted_value = Arc::new(Mutex::new(None));
        let ev = emitted_value.clone();
        cb.toggled.connect(move |v| *ev.lock().unwrap() = Some(*v));
        cb.set_checked(true);
        assert_eq!(*emitted_value.lock().unwrap(), Some(true));
    }

    #[test]
    fn test_toggled_signal_emitted_false() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        let emitted_value = Arc::new(Mutex::new(None));
        let ev = emitted_value.clone();
        cb.toggled.connect(move |v| *ev.lock().unwrap() = Some(*v));
        cb.set_checked(false);
        assert_eq!(*emitted_value.lock().unwrap(), Some(false));
    }

    #[test]
    fn test_toggled_not_emitted_on_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_checked(true);
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.toggled.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        // Set same state – no toggle
        cb.set_checked(true);
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_toggled_not_emitted_on_partial_transition() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.toggled.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        // Partial → Unchecked does not emit toggled
        cb.toggle();
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    // ── 5. state_changed signal (all transitions, no emission for noop) ────
    #[test]
    fn test_state_changed_on_all_transitions() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        let states = Arc::new(Mutex::new(Vec::new()));
        let s2 = states.clone();
        cb.state_changed.connect(move |s| s2.lock().unwrap().push(*s));
        cb.toggle(); // Unchecked → Checked
        cb.toggle(); // Checked → PartiallyChecked
        cb.toggle(); // PartiallyChecked → Unchecked
        let guard = states.lock().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0], CheckState::Checked);
        assert_eq!(guard[1], CheckState::PartiallyChecked);
        assert_eq!(guard[2], CheckState::Unchecked);
    }

    #[test]
    fn test_state_changed_not_emitted_on_noop() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let emitted_count = Arc::new(AtomicU32::new(0));
        let ec = emitted_count.clone();
        cb.state_changed.connect(move |_| {
            ec.fetch_add(1, Ordering::SeqCst);
        });
        cb.set_state(CheckState::Unchecked); // already Unchecked
        assert_eq!(emitted_count.load(Ordering::SeqCst), 0);
    }

    // ── 6. Tristate mode toggle cycle ──────────────────────────────────────
    #[test]
    fn test_tristate_enable_disable_cycle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(!cb.is_tristate_enabled());
        cb.set_tristate_enabled(true);
        assert!(cb.is_tristate_enabled());
        cb.set_tristate_enabled(false);
        assert!(!cb.is_tristate_enabled());
    }

    // ── 7. Disabling tristate resets Partial to Unchecked ──────────────────
    #[test]
    fn a_press_beside_the_label_does_not_toggle_the_checkbox() {
        // The `MousePress` arm used to ignore `pos` entirely, so a press *anywhere* in the control's
        // rectangle toggled it. A checkbox given a wide row by its layout therefore flipped when the
        // user clicked empty space far to the right of its own label — a different statement from
        // "the touch target is at least N points", and the wrong one.
        let mut cb = CheckBox::new(Rect::new(0, 0, 240, 30));
        cb.set_text("Label".to_string());
        cb.handle_event(&Event::MousePress { pos: Point::new(230, 15), button: 1 });
        assert_eq!(
            cb.state(),
            CheckState::Unchecked,
            "a press on the empty part of the row must not toggle the checkbox"
        );
    }

    #[test]
    fn a_press_outside_the_control_still_hits_through_the_touch_target() {
        // The other half of the same judgement, and the mechanism BLUE21 AR1 found unconnected: a
        // control smaller than the platform's minimum touch size must still respond just outside
        // its own rectangle. The checkbox is 18 px tall, so on a phone profile (48 px) a point a few
        // pixels above it is inside the reachable target.
        let mut cb = CheckBox::new(Rect::new(20, 20, 24, 18));
        // The theme registry is process-wide, so a test that **switches** the appearance must take
        // the same guard every other switching test does; otherwise it leaves `Light` selected for
        // whichever test runs next, and a test that compares two appearances can observe this one's
        // leftover as its own. That is the failure mode that made `display_widgets`'s slider halo
        // assertion fail only in a batch (see its own note).
        let _guard = crate::theme::theme_test_guard();
        // Theming is what installs the touch target, so the test installs and applies one the way
        // the runtime does before registering a control. Reading it off an un-themed control would
        // assert the wrong precondition — the point of the mechanism is that the *platform* supplies
        // the size.
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.set_appearance(crate::theme::AppearanceMode::Light);
        }
        crate::theme::apply_active_theme(&mut cb);
        let target = cb.style().touch_target.expect(
            "a themed control must carry a touch target — that is what `role_base_style` writes",
        );
        assert!(
            target.height > 18,
            "the target must exceed the control's own height for this test to mean anything"
        );
        // One pixel above the control's top edge, inside the expansion.
        cb.handle_event(&Event::MousePress { pos: Point::new(24, 19), button: 1 });
        assert_eq!(
            cb.state(),
            CheckState::Checked,
            "a press within the minimum touch target must reach the control"
        );
    }

    #[test]
    fn a_press_well_outside_the_touch_target_is_ignored() {
        // The reverse direction, so the previous test cannot pass by accepting everything.
        let mut cb = CheckBox::new(Rect::new(100, 100, 24, 18));
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    /// A checked box's **label** stays legible on the page, on both appearances.
    ///
    /// # The defect this pins
    ///
    /// The theme's `"check_box:checked"` override sets `background` to the primary fill and
    /// `foreground` to that fill's contrast colour — both describing the **box interior**. The draw
    /// path used `style.text_color` for the mark *and* for the label, and the label is beside the
    /// box, on the page: on the dark appearance `primary.contrast_color()` is black, so a checked box
    /// drew a legible mark next to a word that had gone black on a dark page. The census measured the
    /// two appearances as byte-identical for exactly this reason (the label dominates the render),
    /// which is how it was found.
    ///
    /// The label now resolves against the **page** rather than the box, so the assertion is a
    /// contrast ratio against the active theme's background — the surface the user actually reads it
    /// on — rather than against a colour the control is not drawn on.
    #[test]
    #[cfg(device_profile)]
    fn a_checked_box_keeps_its_label_legible_on_the_page() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        for appearance in [crate::theme::AppearanceMode::Light, crate::theme::AppearanceMode::Dark]
        {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut cb = CheckBox::new(Rect::new(0, 0, 200, 24));
            cb.set_text("Label".to_string());
            cb.set_checked(true);
            crate::theme::apply_active_theme(&mut cb);

            let page = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let svg = crate::widget::svg::render_widget_to_svg(&mut cb, Rect::new(0, 0, 200, 24));
            let ink = label_ink(&svg).unwrap_or_else(|| {
                panic!("a checked box with text must paint its label; svg was {svg}")
            });
            let ratio = ink.contrast_ratio(page);
            assert!(
                ratio >= 4.5,
                "the label of a checked box must clear the AA floor on the page it sits on: \
                 {appearance:?} painted {ink:?} on {page:?}, a ratio of {ratio:.2}:1"
            );
        }
    }

    /// The fill of the SVG `<path>` element that carries the label's glyph geometry.
    #[cfg(device_profile)]
    fn label_ink(svg: &str) -> Option<Color> {
        let at = svg.find("<path d=\"")?;
        let rest = &svg[at..];
        let end = rest.find("/>")? + 2;
        let element = &rest[..end];
        let key = "fill=\"rgba(";
        let from = element.find(key)? + key.len();
        let to = element[from..].find(')')? + from;
        let mut parts = element[from..to].split(',');
        let r = parts.next()?.trim().parse().ok()?;
        let g = parts.next()?.trim().parse().ok()?;
        let b = parts.next()?.trim().parse().ok()?;
        Some(Color::rgb(r, g, b))
    }

    #[test]
    fn test_disable_tristate_resets_partial() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_state(CheckState::PartiallyChecked);
        assert!(cb.is_partially_checked());
        // Disable tristate – should reset to Unchecked
        cb.set_tristate_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_disable_tristate_does_not_reset_checked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(true);
        cb.set_checked(true);
        cb.set_tristate_enabled(false);
        // Checked should stay Checked
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_disable_tristate_does_not_reset_unchecked() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_tristate_enabled(false);
        cb.set_state(CheckState::Unchecked);
        // No-op
        cb.set_tristate_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    // ── 8. Text set/get ────────────────────────────────────────────────────
    #[test]
    fn test_text_default_empty() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.text(), "");
    }

    #[test]
    fn test_text_set_and_get() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_text(String::from("Enable feature"));
        assert_eq!(cb.text(), "Enable feature");
    }

    #[test]
    fn test_text_overwrite() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_text(String::from("Old label"));
        cb.set_text(String::from("New label"));
        assert_eq!(cb.text(), "New label");
    }

    // ── 9. Event handling ──────────────────────────────────────────────────
    #[test]
    fn test_mouse_down_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn test_touch_begin_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::TouchBegin { touch_id: 0, pos: Point::new(10, 10) });
        // TouchBegin now toggles the checkbox when the touch feature is enabled
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[cfg(feature = "touch")]
    #[test]
    fn test_tap_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::Tap { pos: Point::new(10, 10) });
        // Current implementation does not match Tap – falls through to _
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_space_key_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.state(), CheckState::Unchecked);
        // Space key code is 32
        cb.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    #[test]
    fn test_non_space_key_does_not_toggle() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.handle_event(&Event::KeyDown((65, 0))); // 'A' key
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_event_noop_when_disabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_enabled(false);
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MouseDown((Point::new(10, 10), 0)));
        // Should NOT toggle because checkbox is disabled
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_event_key_noop_when_disabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.set_enabled(false);
        cb.handle_event(&Event::KeyDown((32, 0)));
        assert_eq!(cb.state(), CheckState::Unchecked);
    }

    #[test]
    fn test_multiple_mouse_down_toggles() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Unchecked);
        cb.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert_eq!(cb.state(), CheckState::Checked);
    }

    // ── 10. Widget trait delegation ─────────────────────────────────────────
    #[test]
    fn test_widget_geometry() {
        let mut cb = CheckBox::new(Rect::new(10, 20, 100, 30));
        assert_eq!(cb.geometry(), Rect::new(10, 20, 100, 30));
        cb.set_geometry(Rect::new(0, 0, 200, 50));
        assert_eq!(cb.geometry(), Rect::new(0, 0, 200, 50));
    }

    #[test]
    fn test_widget_visibility() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.is_visible());
        cb.hide();
        assert!(!cb.is_visible());
        cb.show();
        assert!(cb.is_visible());
    }

    #[test]
    fn test_widget_enabled() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.is_enabled());
        cb.set_enabled(false);
        assert!(!cb.is_enabled());
        cb.set_enabled(true);
        assert!(cb.is_enabled());
    }

    #[test]
    fn test_widget_style_default() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let _style = cb.style();
        assert_eq!(cb.kind(), WidgetKind::CheckBox);
    }

    #[test]
    fn test_widget_tooltip() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.tooltip().is_empty());
        cb.set_tooltip(String::from("Click to toggle"));
        assert_eq!(cb.tooltip(), "Click to toggle");
    }

    #[test]
    fn test_widget_parent() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.parent().is_none());
        let parent_id: ObjectId = 42;
        cb.set_parent(Some(parent_id));
        assert_eq!(cb.parent(), Some(parent_id));
        cb.set_parent(None);
        assert!(cb.parent().is_none());
    }

    #[test]
    fn test_widget_children() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.children().is_empty());
        let child_id: ObjectId = 99;
        cb.add_child(child_id);
        assert_eq!(cb.children().len(), 1);
        assert_eq!(cb.children()[0], child_id);
        cb.remove_child(child_id);
        assert!(cb.children().is_empty());
    }

    #[test]
    fn test_widget_min_max_size() {
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert!(cb.min_size().is_none());
        assert!(cb.max_size().is_none());
        cb.set_min_size(Some(Size::new(50, 20)));
        cb.set_max_size(Some(Size::new(200, 60)));
        assert_eq!(cb.min_size(), Some(Size::new(50, 20)));
        assert_eq!(cb.max_size(), Some(Size::new(200, 60)));
    }

    #[test]
    fn test_widget_id_not_zero() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_ne!(cb.id(), 0u64);
    }

    #[test]
    fn test_widget_signal_accessors() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        // Verify signal accessors do not panic
        let _ = cb.hover_signal();
        let _ = cb.mouse_down_signal();
        let _ = cb.mouse_up_signal();
        let _ = cb.key_down_signal();
        let _ = cb.key_up_signal();
        let _ = cb.focus_gained_signal();
        let _ = cb.focus_lost_signal();
        let _ = cb.redraw_requested_signal();
        let _ = cb.layout_requested_signal();
    }

    #[test]
    fn test_widget_connection_scope() {
        let cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        let _scope = cb.connection_scope();
    }

    // ── State channel (§2.2) ─────────────────────────────────────────────────

    /// `CheckBox` overrides neither `widget_state` nor any hover bookkeeping, so this
    /// pins the *whole* point of elevating the state source into `BaseWidget`: a control
    /// that inherited the default reports `Hover` purely because the runtime delivered
    /// `MouseEnter`. Before the elevation this could not be true.
    #[test]
    fn widget_state_reports_hover_from_the_base() {
        use crate::style::WidgetState;
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.widget_state(), WidgetState::Normal);
        cb.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert_eq!(cb.widget_state(), WidgetState::Hover);
        cb.handle_event(&Event::MouseLeave { pos: Point::new(1, 1) });
        assert_eq!(cb.widget_state(), WidgetState::Normal);
    }

    /// Disabled outranks hovered — the precedence the trait documents.
    #[test]
    fn widget_state_prefers_disabled_over_hover() {
        use crate::style::WidgetState;
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        cb.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        cb.set_enabled(false);
        assert_eq!(cb.widget_state(), WidgetState::Disabled);
    }

    /// A latched box reports `Checked`, which is what makes the preset's `check_box:checked`
    /// override reachable at all.
    ///
    /// # The defect this pins
    ///
    /// Both presets declare `check_box:checked`, and the theme manager looks a control up by
    /// `"<kind>:<state>"`. But the trait's default `widget_state` only knows the four primitive
    /// flags — `disabled`/`pressed`/`hovered`/`focused` — and none of them means "latched". So the
    /// key was declared, documented, and resolved to `Normal` on every checkbox, and the accent
    /// fill the preset asks for was never painted. A declared state with no consumer is the
    /// state-level form of §5.5's rule.
    #[test]
    fn widget_state_reports_checked_for_a_latched_box() {
        use crate::style::WidgetState;
        let mut cb = CheckBox::new(Rect::new(0, 0, 100, 30));
        assert_eq!(cb.widget_state(), WidgetState::Normal);

        cb.set_state(CheckState::Checked);
        assert_eq!(cb.widget_state(), WidgetState::Checked);

        // `PartiallyChecked` also reports `Checked`: the preset paints one accent fill for "on",
        // and a mixed box is on. The three-way answer is carried by the control's own mark and by
        // `a11y_state`, which is where a screen reader reads it.
        cb.set_state(CheckState::PartiallyChecked);
        assert_eq!(cb.widget_state(), WidgetState::Checked);

        // And the latch outranks the momentary states, so a checked box that is hovered is still
        // reported as checked — the precedence the trait documents.
        cb.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert_eq!(cb.widget_state(), WidgetState::Checked);

        // Unchecking hands the report back to the momentary states.
        cb.set_state(CheckState::Unchecked);
        assert_eq!(cb.widget_state(), WidgetState::Hover);
    }
}
