// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SplitButton widget with primary action and drop-down action list.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One selectable action in a split button drop-down list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitAction {
    /// Stable action identifier.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SplitAction {
    /// Creates a split action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// SplitButton provides a default primary action plus selectable alternatives.
pub struct SplitButton {
    base: BaseWidget,
    text: String,
    actions: Vec<SplitAction>,
    primary_action_index: Option<usize>,
    highlighted_action_index: Option<usize>,
    menu_open: bool,
    pressed_primary: bool,
    pressed_arrow: bool,
    hovered_primary: bool,
    hovered_arrow: bool,
    arrow_width: u32,
    row_height: u32,
    /// Emitted when primary area triggers current action id.
    pub triggered: Signal1<String>,
    /// Emitted when a drop-down action is explicitly selected.
    pub action_selected: Signal1<String>,
    /// Emitted when menu open state changes.
    pub menu_toggled: Signal1<bool>,
}

impl SplitButton {
    /// Creates a split button.
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "SplitButton"),
            text: text.into(),
            actions: Vec::new(),
            primary_action_index: None,
            highlighted_action_index: None,
            menu_open: false,
            pressed_primary: false,
            pressed_arrow: false,
            hovered_primary: false,
            hovered_arrow: false,
            arrow_width: dimensions::SPLIT_ARROW_COLUMN_WIDTH,
            row_height: 22,
            triggered: Signal1::new(),
            action_selected: Signal1::new(),
            menu_toggled: Signal1::new(),
        }
    }

    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets button text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Replaces all actions.
    pub fn set_actions(&mut self, actions: Vec<SplitAction>) {
        self.actions = actions;
        self.primary_action_index = if self.actions.is_empty() { None } else { Some(0) };
        self.highlighted_action_index = self.primary_action_index;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all actions.
    pub fn actions(&self) -> &[SplitAction] {
        &self.actions
    }

    /// Adds one action and returns its index.
    pub fn add_action(&mut self, action: SplitAction) -> usize {
        let index = self.actions.len();
        self.actions.push(action);
        if self.primary_action_index.is_none() {
            self.primary_action_index = Some(index);
            self.highlighted_action_index = Some(index);
        }
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Returns current primary action index.
    pub fn primary_action_index(&self) -> Option<usize> {
        self.primary_action_index.filter(|index| *index < self.actions.len())
    }

    /// Returns whether menu is open.
    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    /// Returns highlighted action index when menu is open.
    pub fn highlighted_action_index(&self) -> Option<usize> {
        self.highlighted_action_index.filter(|index| *index < self.actions.len())
    }

    /// Sets menu row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        self.row_height = row_height.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns menu row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Triggers primary action.
    pub fn trigger_primary(&mut self) -> bool {
        let Some(index) = self.primary_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.triggered.emit(action.id.clone());
        true
    }

    /// Opens menu.
    pub fn open_menu(&mut self) {
        if self.menu_open {
            return;
        }
        self.menu_open = true;
        self.highlighted_action_index = self.primary_action_index();
        self.menu_toggled.emit(true);
        self.base.request_redraw();
    }

    /// Closes menu.
    pub fn close_menu(&mut self) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        self.menu_toggled.emit(false);
        self.base.request_redraw();
    }

    /// Toggles menu visibility.
    pub fn toggle_menu(&mut self) {
        if self.menu_open {
            self.close_menu();
        } else {
            self.open_menu();
        }
    }

    /// Selects highlighted drop-down action as primary and emits action_selected.
    pub fn select_highlighted_action(&mut self) -> bool {
        let Some(index) = self.highlighted_action_index() else {
            return false;
        };
        let Some(action) = self.actions.get(index) else {
            return false;
        };
        self.primary_action_index = Some(index);
        self.action_selected.emit(action.id.clone());
        self.close_menu();
        true
    }

    /// Moves highlighted action in menu by signed delta.
    pub fn move_highlight(&mut self, delta: isize) {
        if self.actions.is_empty() {
            self.highlighted_action_index = None;
            return;
        }

        let current = self.highlighted_action_index.unwrap_or(0) as isize;
        let max = self.actions.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.highlighted_action_index = Some(next);
        self.base.request_redraw();
    }

    /// The face the button actually paints: full width,
    /// `dimensions::SPLIT_BUTTON_HEIGHT` tall, centred in the rectangle it was given.
    ///
    /// # Why the face is not the rectangle
    ///
    /// A split button is chrome: one compact row split into a trigger and an arrow. Taking
    /// `rect.height` made a 240x120 census cell a 120 px-tall face whose two halves were also
    /// 120 tall — a slab shaped like a button rather than a button — and it disagreed with the
    /// 28 px `size_hint` the control reports. The band is the single derivation the paint, the
    /// hit tests and the drop-down's anchor all read.
    fn face_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SPLIT_BUTTON_HEIGHT)
    }

    fn primary_rect(&self) -> Rect {
        let band = self.face_band();
        let primary_width = band.width.saturating_sub(self.arrow_width);
        Rect::new(band.x, band.y, primary_width, band.height)
    }

    /// The trigger's label box: the primary face's interior, inset by the shared padding.
    ///
    /// # Why this is a box and not an `x`
    ///
    /// The label used to be drawn at `primary_rect.x + 8` with `HorizontalAlignment::Left`,
    /// and the menu rows at `action_rect.x + 8` — the same literal written twice, in two
    /// different coordinate systems, with no relation to the control's own padding constant.
    /// Returning the padded box instead gives the draw call the rectangle it is fitting into,
    /// so the label is bounded by the trigger rather than by a hand-chosen origin, and the
    /// trigger and the menu rows derive their leading space from one place.
    fn primary_label_box(&self, primary: Rect) -> Rect {
        ControlMetrics::content_box(
            primary,
            EdgeOffsets {
                left: dimensions::SPLIT_BUTTON_PADDING_H,
                right: dimensions::SPLIT_BUTTON_PADDING_H,
                top: 0,
                bottom: 0,
            },
        )
    }

    fn arrow_rect(&self) -> Rect {
        let band = self.face_band();
        let arrow_x = band.x + band.width as i32 - self.arrow_width as i32;
        Rect::new(arrow_x, band.y, self.arrow_width, band.height)
    }

    fn menu_rect(&self) -> Rect {
        // The popup hangs from the **face**'s bottom edge, not the control's, so it appears
        // directly under the button the user pressed rather than 120 px below it.
        let band = self.face_band();
        Rect::new(
            band.x,
            band.y + band.height as i32,
            band.width,
            self.row_height.saturating_mul(self.actions.len() as u32),
        )
    }

    fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let menu = self.menu_rect();
        let y = menu.y + index as i32 * self.row_height as i32;
        Some(Rect::new(menu.x, y, menu.width, self.row_height))
    }

    fn hit_primary(&self, pos: Point) -> bool {
        let rect = self.primary_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_arrow(&self, pos: Point) -> bool {
        let rect = self.arrow_rect();
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn hit_menu_index(&self, pos: Point) -> Option<usize> {
        if !self.menu_open {
            return None;
        }
        let menu = self.menu_rect();
        if pos.x < menu.x
            || pos.x >= menu.x + menu.width as i32
            || pos.y < menu.y
            || pos.y >= menu.y + menu.height as i32
        {
            return None;
        }
        let index = ((pos.y - menu.y) / self.row_height as i32) as usize;
        (index < self.actions.len()).then_some(index)
    }
}

impl Widget for SplitButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(100, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SplitButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `SplitButton` reports
/// `WidgetKind::ToolButton`, shared with `ToolButton`; dispatching on the concrete
/// type here is what keeps the two contracts separate.
impl WidgetProperties for SplitButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "action_count" => Ok(CapabilityValue::UInt(self.actions().len() as u64)),
            "menu_open" => Ok(CapabilityValue::Bool(self.menu_open())),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "action_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "menu_open" => {
                if expect_bool(value)? {
                    self.open_menu();
                } else {
                    self.close_menu();
                }
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "action_count", "menu_open", "row_height", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `split_button` publishes.
    ///
    /// `open_menu` and `close_menu` are the genuine zero-argument actions here.
    /// `trigger_primary` acts on the primary action and reports `false` when there
    /// is no action to trigger, which is the "could not handle it" case, so it is
    /// answered with [`CapabilityAccessError::OutOfRange`] rather than a success
    /// that did nothing. `add_action` takes the action to add, so it is refused the
    /// same way — the name is right and the argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open_menu" => {
                self.open_menu();
                Ok(())
            }
            "close_menu" => {
                self.close_menu();
                Ok(())
            }
            "trigger_primary" => {
                if self.trigger_primary() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "add_action" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SplitButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_primary = self.hit_primary(*pos);
                self.hovered_arrow = self.hit_arrow(*pos);
                if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered_primary = false;
                self.hovered_arrow = false;
                self.pressed_primary = false;
                self.pressed_arrow = false;
            }
            Event::MousePress { pos, button: 1 } => {
                if self.hit_primary(*pos) {
                    self.pressed_primary = true;
                } else if self.hit_arrow(*pos) {
                    self.pressed_arrow = true;
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                } else {
                    self.close_menu();
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if self.pressed_primary {
                    self.pressed_primary = false;
                    if self.hit_primary(*pos) {
                        let _ = self.trigger_primary();
                    }
                } else if self.pressed_arrow {
                    self.pressed_arrow = false;
                    if self.hit_arrow(*pos) {
                        self.toggle_menu();
                    }
                } else if let Some(index) = self.hit_menu_index(*pos) {
                    self.highlighted_action_index = Some(index);
                    let _ = self.select_highlighted_action();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 | 32 => {
                    if self.menu_open {
                        let _ = self.select_highlighted_action();
                    } else {
                        let _ = self.trigger_primary();
                    }
                }
                40 => {
                    if !self.menu_open {
                        self.open_menu();
                    } else {
                        self.move_highlight(1);
                    }
                }
                38 if self.menu_open => {
                    self.move_highlight(-1);
                }
                27 => {
                    self.close_menu();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SplitButton {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the button, its splits and its drop-down unchanged — the
        // rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("split_button");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `split_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A face painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&secondary, 0.45));

        // The states are raised from the face toward the theme's primary, so hover and press read
        // on either appearance rather than being a fixed pale blue that only worked on a light bar.
        let hover_bg = face.blend(&primary, 0.28);
        let pressed_bg = face.blend(&primary, 0.45);
        let arrow_face = face.blend(&ink, 0.06);

        // ── The face actually painted ──
        //
        // The button's chrome is one compact row, not a filled container; the band is where the
        // trigger and the arrow are drawn and where the popup is anchored.
        let face_rect = self.face_band();
        context.fill_rect(face_rect, face);
        context.draw_rect(face_rect, border);

        let primary_rect = self.primary_rect();
        let arrow = self.arrow_rect();

        let primary_bg = if self.pressed_primary {
            pressed_bg
        } else if self.hovered_primary {
            hover_bg
        } else {
            face
        };
        context.fill_rect(primary_rect, primary_bg);

        let arrow_bg = if self.pressed_arrow || self.menu_open {
            pressed_bg
        } else if self.hovered_arrow {
            hover_bg
        } else {
            arrow_face
        };
        context.fill_rect(arrow, arrow_bg);

        context.draw_line(
            Point::new(arrow.x, arrow.y),
            Point::new(arrow.x, arrow.y + arrow.height as i32),
            border,
        );

        // `draw_text`'s origin is the glyph box's top-left, so `primary_rect.y + height / 2` put
        // that top edge on the middle line and drew the label half a line low. The line box
        // centred in the trigger is the origin; the arrow glyph below shares its band's own.
        //
        // The label is centred both ways inside the trigger's padded box. A split button's
        // primary face *is* a button, so its label follows the same rule `Button` does
        // (Qt Quick centres `AbstractButton`'s `contentItem`; Flutter M3 centres the child) —
        // the previous `x + 8` origin left-aligned it against a literal.
        let primary_box = self.primary_label_box(primary_rect);
        context.draw_text_fitted(
            context.text_line(primary_box, &Font::default()),
            &self.text,
            &Font::default(),
            ink,
            HorizontalAlignment::Center,
        );

        // The arrow glyph is centred on its own column rather than offset by a half-glyph
        // literal: `(arrow.width / 2) - 3` hard-coded a 6 px-wide 'v', so a different font or
        // size put the glyph off the column's centre. `draw_text_fitted` with `Center` derives
        // the origin from the measured string inside the column.
        context.draw_text_fitted(
            context.text_line(arrow, &Font::default()),
            "v",
            &Font::default(),
            ink.blend(&arrow_bg, 0.35),
            HorizontalAlignment::Center,
        );

        if self.menu_open {
            let menu = self.menu_rect();
            context.fill_rect(menu, face.blend(&ink, 0.22));
            context.draw_rect(menu, border);

            for index in 0..self.actions.len() {
                let Some(action_rect) = self.action_rect(index) else {
                    continue;
                };

                if self.highlighted_action_index == Some(index) {
                    context.fill_rect(action_rect, hover_bg);
                }

                if let Some(action) = self.actions.get(index) {
                    // A menu row's label is centred through the shared primitive, since the
                    // glyph origin is a top edge and `action_rect.y + height / 2` placed it
                    // half a line low. The box is the row's own padded interior — the same
                    // `SPLIT_BUTTON_PADDING_H` the trigger uses, so a menu row and the trigger
                    // it hangs from share their leading space rather than each naming it.
                    let row_box = self.primary_label_box(action_rect);
                    context.draw_text_fitted(
                        context.text_line(row_box, &Font::default()),
                        &action.label,
                        &Font::default(),
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_actions() -> Vec<SplitAction> {
        vec![
            SplitAction::new("run.default", "Run"),
            SplitAction::new("run.debug", "Run with Debug"),
            SplitAction::new("run.profile", "Run with Profile"),
        ]
    }

    #[test]
    fn primary_trigger_emits_default_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 160, 28));
        split.set_actions(sample_actions());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(split.trigger_primary());
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.default".to_string()]);
    }

    #[test]
    fn keyboard_navigation_selects_action_from_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        split.action_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        split.handle_event(&Event::key_press(40, 0));
        assert!(split.menu_open());
        split.handle_event(&Event::key_press(40, 0));
        split.handle_event(&Event::key_press(13, 0));

        assert_eq!(split.primary_action_index(), Some(1));
        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["run.debug".to_string()]);
        assert!(!split.menu_open());
    }

    #[test]
    fn arrow_click_toggles_menu_and_mouse_selects_action() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 180, 28));
        split.set_actions(sample_actions());

        // Click arrow area to open menu.
        split.handle_event(&Event::mouse_press(172, 12, 1));
        split.handle_event(&Event::mouse_release(172, 12, 1));
        assert!(split.menu_open());

        // Click second row in menu.
        let menu_y = 28 + 24;
        split.handle_event(&Event::mouse_press(20, menu_y, 1));
        split.handle_event(&Event::mouse_release(20, menu_y, 1));

        assert_eq!(split.primary_action_index(), Some(1));
        assert!(!split.menu_open());
    }

    #[test]
    fn default_state() {
        let split = SplitButton::new("Run", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Run");
        assert!(split.actions().is_empty());
        assert_eq!(split.primary_action_index(), None);
        assert!(!split.menu_open());
    }

    #[test]
    fn set_text_get_text_roundtrip() {
        let mut split = SplitButton::new("Initial", Rect::new(0, 0, 800, 600));
        assert_eq!(split.text(), "Initial");

        split.set_text("Updated");
        assert_eq!(split.text(), "Updated");

        split.set_text("");
        assert_eq!(split.text(), "");
    }

    #[test]
    fn add_action_adds_to_menu() {
        let mut split = SplitButton::new("Action", Rect::new(0, 0, 800, 600));
        assert_eq!(split.actions().len(), 0);

        let idx = split.add_action(SplitAction::new("act1", "Action 1"));
        assert_eq!(idx, 0);
        assert_eq!(split.actions().len(), 1);
        assert_eq!(split.primary_action_index(), Some(0));

        let idx = split.add_action(SplitAction::new("act2", "Action 2"));
        assert_eq!(idx, 1);
        assert_eq!(split.actions().len(), 2);

        assert_eq!(split.actions()[0].id, "act1");
        assert_eq!(split.actions()[1].id, "act2");
    }

    #[test]
    fn enable_disable_states() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A")]);

        // Enabled by default
        assert!(split.trigger_primary());

        // Disable widget
        split.base_mut().set_enabled(false);

        // After disabling, events should be ignored
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        split.triggered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // Direct call still works but event handler ignores
        split.handle_event(&Event::key_press(40, 0));
        let _got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!split.menu_open(), "disabled widget should not open menu via events");
    }

    #[test]
    fn trigger_with_no_actions() {
        let mut split = SplitButton::new("Empty", Rect::new(0, 0, 800, 600));
        // No actions added yet
        assert!(!split.trigger_primary());

        // open_menu with no actions still opens an empty menu (code doesn't guard)
        split.open_menu();
        assert!(split.menu_open());

        // Close menu
        split.close_menu();
        assert!(!split.menu_open());
    }

    #[test]
    fn menu_toggle_signal_emission() {
        let mut split = SplitButton::new("Test", Rect::new(0, 0, 800, 600));
        split.set_actions(vec![SplitAction::new("a", "A"), SplitAction::new("b", "B")]);

        let emitted = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = emitted.clone();
        split.menu_toggled.connect(move |state| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*state.as_ref());
            }
        });

        split.open_menu();
        assert!(split.menu_open());

        split.close_menu();
        assert!(!split.menu_open());

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    /// The button's face is one compact row, and the popup hangs from it.
    ///
    /// The defect this pins: the face and its two halves were sized from `rect`, so a 240x120
    /// census cell drew a 120 px-tall face, and the drop-down was anchored to the control's
    /// bottom edge — 120 px below the button the user pressed.
    #[test]
    fn the_face_keeps_its_own_height_and_anchors_the_menu() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());

        let band = split.face_band();
        assert_eq!(
            band.height,
            crate::widget::metrics::dimensions::SPLIT_BUTTON_HEIGHT,
            "the face is a compact row, not the whole rectangle"
        );
        // The trigger and the arrow divide the face rather than the control.
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();
        assert_eq!(primary.height, band.height);
        assert_eq!(arrow.height, band.height);
        assert_eq!(primary.width + arrow.width, band.width);
        // The popup begins at the face's bottom edge.
        let menu = split.menu_rect();
        assert_eq!(menu.y, band.y + band.height as i32);
    }

    /// The trigger's label is centred in the trigger, and the arrow in its own column.
    ///
    /// Both used to be placed by hand: the label at `primary_rect.x + 8` (left-aligned against
    /// a literal, so a 240 px control drew a 50 px word flush to the left of a 218 px face) and
    /// the arrow at `arrow.x + (arrow.width / 2) - 3` (which hard-coded a 6 px-wide 'v', so any
    /// other glyph or font put the arrow off the column's centre).
    #[test]
    fn the_label_and_the_arrow_are_each_centred_in_their_own_box() {
        let rect = Rect::new(0, 0, 240, 120);
        let mut split = SplitButton::new("Sample", rect);
        let primary = split.primary_rect();
        let arrow = split.arrow_rect();

        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
        let context = crate::render::RenderContext::new(&mut backend);
        let font = Font::default();
        let label_w = context.measure_text("Sample", &font).width as i32;
        let arrow_w = context.measure_text("v", &font).width as i32;

        let expected_label_x = primary.x
            + dimensions::SPLIT_BUTTON_PADDING_H as i32
            + (primary.width as i32 - 2 * dimensions::SPLIT_BUTTON_PADDING_H as i32 - label_w) / 2;
        let expected_arrow_x = arrow.x + (arrow.width as i32 - arrow_w) / 2;

        let svg = crate::widget::svg::render_to_svg(&mut split);
        let found = |needle: &str| -> i32 {
            svg.lines()
                .find(|line| line.contains(&format!(">{needle}</text>")))
                .and_then(|line| {
                    let at = line.find("x=\"")? + 3;
                    let end = line[at..].find('"')? + at;
                    line[at..end].parse().ok()
                })
                .unwrap_or_else(|| panic!("{needle} was not rendered"))
        };
        assert_eq!(found("Sample"), expected_label_x, "the label belongs in the trigger's middle");
        assert_ne!(found("Sample"), primary.x + dimensions::SPLIT_BUTTON_PADDING_H as i32);
        assert_eq!(found("v"), expected_arrow_x, "the arrow belongs in its column's middle");
    }

    /// The trigger's label box and a menu row's label box share one padding derivation.
    #[test]
    fn the_trigger_and_the_menu_rows_share_their_leading_space() {
        let mut split = SplitButton::new("Run", Rect::new(0, 0, 240, 120));
        split.set_actions(sample_actions());
        let primary = split.primary_rect();
        let trigger_box = split.primary_label_box(primary);
        let row = split.action_rect(0).expect("the fixture has an action");
        let row_box = split.primary_label_box(row);
        assert_eq!(
            trigger_box.x - primary.x,
            row_box.x - row.x,
            "the trigger and its menu rows must inset their labels by the same amount"
        );
        assert_eq!(
            trigger_box.x - primary.x,
            dimensions::SPLIT_BUTTON_PADDING_H as i32,
            "and that amount is the named constant, not a literal"
        );
    }
}
