// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `SpeedDial` — an expandable group of secondary actions beside a primary floating button
//! (COMP-A02).
//!
//! A `FAB` is one action. The common shape that was missing is the *speed dial*: a primary
//! button that opens a short stack of labelled secondary actions ("new mail", "new draft").
//! This companion owns that stack — its expansion animation, its layout and its hit areas —
//! while the host keeps ownership of what each action does.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_draw_bridge, impl_widget_property_hooks, property_names_of};

/// Edge length of the primary and secondary buttons, in logical pixels.
const SPEED_DIAL_BUTTON: u32 = 40;
/// Gap between an action's circle and its label.
const SPEED_DIAL_LABEL_GAP: i32 = 8;

/// The direction the secondary actions open toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeedDialDirection {
    /// Actions stack upward from the main button.
    #[default]
    Up,
    /// Actions extend to the left of the main button.
    Left,
}

impl SpeedDialDirection {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            SpeedDialDirection::Up => "up",
            SpeedDialDirection::Left => "left",
        }
    }

    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "up" => SpeedDialDirection::Up,
            "left" => SpeedDialDirection::Left,
            _ => return None,
        })
    }
}

/// One secondary action in a [`SpeedDial`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedDialAction {
    /// The action's label, drawn beside its button.
    pub label: String,
    /// The short glyph displayed inside the button.
    pub icon_text: String,
}

impl SpeedDialAction {
    /// Creates an action.
    pub fn new(label: impl Into<String>, icon_text: impl Into<String>) -> Self {
        Self { label: label.into(), icon_text: icon_text.into() }
    }
}

/// A primary button with an expandable stack of labelled secondary actions.
pub struct SpeedDial {
    base: BaseWidget,
    actions: Vec<SpeedDialAction>,
    expanded: bool,
    direction: SpeedDialDirection,
    spacing: u32,
    collapse_on_activate: bool,
    /// The expansion, `0.0` collapsed and `1.0` fully open.
    progress: PropertyDriver,
    /// Emitted with the activated action's index.
    pub action_activated: Signal1<usize>,
    /// Emitted when the expansion state changes.
    pub expanded_changed: Signal1<bool>,
}

impl SpeedDial {
    /// Creates a collapsed speed dial.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FAB, geometry, "SpeedDial"),
            actions: Vec::new(),
            expanded: false,
            direction: SpeedDialDirection::Up,
            spacing: 12,
            collapse_on_activate: true,
            progress: PropertyDriver::at(0.0, MotionSlot::Normal),
            action_activated: Signal1::new(),
            expanded_changed: Signal1::new(),
        }
    }

    /// Replaces the secondary actions.
    pub fn set_actions(&mut self, actions: Vec<SpeedDialAction>) {
        self.actions = actions;
        self.base.request_redraw();
    }

    /// Returns the secondary actions.
    pub fn actions(&self) -> &[SpeedDialAction] {
        &self.actions
    }

    /// Whether the dial is expanded.
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Opens the dial.
    pub fn open(&mut self) {
        self.set_expanded(true);
    }

    /// Closes the dial.
    pub fn close(&mut self) {
        self.set_expanded(false);
    }

    /// Toggles the dial.
    pub fn toggle(&mut self) {
        self.set_expanded(!self.expanded);
    }

    /// Sets the expansion state and animates toward it.
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        self.progress.set_target(if expanded { 1.0 } else { 0.0 });
        self.expanded_changed.emit(expanded);
        self.base.request_redraw();
    }

    /// The direction the actions open toward.
    pub fn direction(&self) -> SpeedDialDirection {
        self.direction
    }

    /// Sets the opening direction.
    pub fn set_direction(&mut self, direction: SpeedDialDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
    }

    /// The gap between actions, in logical pixels.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Sets the gap between actions.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.base.request_redraw();
    }

    /// Whether activating an action collapses the dial.
    pub fn collapse_on_activate(&self) -> bool {
        self.collapse_on_activate
    }

    /// Sets whether activating an action collapses the dial.
    pub fn set_collapse_on_activate(&mut self, collapse: bool) {
        self.collapse_on_activate = collapse;
    }

    /// Advances the expansion animation by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.progress.tick(delta_ms)
    }

    /// Whether the expansion is between two states.
    pub fn is_animating(&self) -> bool {
        self.progress.is_moving()
    }

    /// The primary button's square.
    fn main_rect(&self) -> Rect {
        let rect = self.geometry();
        let size = SPEED_DIAL_BUTTON.min(rect.width).min(rect.height).max(1);
        Rect::new(
            rect.x + rect.width as i32 - size as i32,
            rect.y + rect.height as i32 - size as i32,
            size,
            size,
        )
    }

    /// The fully expanded rectangle of action `index`.
    ///
    /// A host uses this for its own labels or for a hit area that outlives one frame. The
    /// animated position is derived from this rectangle and the progress driver.
    pub fn action_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.actions.len() {
            return None;
        }
        let main = self.main_rect();
        let step = SPEED_DIAL_BUTTON.saturating_add(self.spacing) as i32;
        let offset = step * (index as i32 + 1);
        Some(match self.direction {
            SpeedDialDirection::Up => Rect::new(main.x, main.y - offset, main.width, main.height),
            SpeedDialDirection::Left => Rect::new(main.x - offset, main.y, main.width, main.height),
        })
    }

    /// The action under `pos` while the dial is open.
    fn action_at(&self, pos: Point) -> Option<usize> {
        if !self.expanded {
            return None;
        }
        (0..self.actions.len())
            .find(|index| self.action_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// Activates `index`, reporting whether it ran.
    fn activate(&mut self, index: usize) -> bool {
        if index >= self.actions.len() {
            return false;
        }
        self.action_activated.emit(index);
        if self.collapse_on_activate {
            self.close();
        }
        true
    }
}

impl EventHandler for SpeedDial {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.main_rect().contains_point(*pos) {
                    self.toggle();
                    return;
                }
                if let Some(index) = self.action_at(*pos) {
                    let _ = self.activate(index);
                }
            }
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE && self.expanded => {
                self.close();
            }
            Event::KeyPress { key, .. }
                if (*key == key_codes::ENTER || *key == key_codes::SPACE) && !self.expanded =>
            {
                self.open();
            }
            _ => {}
        }
    }
}

impl Draw for SpeedDial {
    fn draw(&mut self, context: &mut RenderContext) {
        let progress = self.progress.value().clamp(0.0, 1.0);
        let style = self.style();
        let surface = style.background_color.unwrap_or(Color::WHITE);
        let ink = style.text_color.unwrap_or(Color::BLACK);
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let main = self.main_rect();
        let label_font = role_font(style, 12.0);

        // Secondary actions first, so the primary button sits on top of its own shadow.
        if progress > 0.0 {
            for (index, action) in self.actions.iter().enumerate() {
                let Some(target) = self.action_rect(index) else {
                    continue;
                };
                let center = Point::new(
                    main.x
                        + main.width as i32 / 2
                        + ((target.x + target.width as i32 / 2 - (main.x + main.width as i32 / 2))
                            as f32
                            * progress) as i32,
                    main.y
                        + main.height as i32 / 2
                        + ((target.y + target.height as i32 / 2 - (main.y + main.height as i32 / 2))
                            as f32
                            * progress) as i32,
                );
                let radius = ((target.width as f32 / 2.0) * progress) as u32;
                context.fill_circle_aa(center, radius, accent.blend(&surface, 0.2));
                let alpha = (255.0 * progress) as u8;
                let glyph_color = ink.with_alpha(alpha);
                let line = context.text_line(
                    Rect::new(
                        center.x - radius as i32,
                        center.y - radius as i32,
                        radius * 2,
                        radius * 2,
                    ),
                    &label_font,
                );
                context.draw_text_fitted_cow(
                    Rect::new(center.x - radius as i32, line.y, radius * 2, line.height.max(1)),
                    &action.icon_text,
                    &label_font,
                    glyph_color,
                    HorizontalAlignment::Center,
                );
                // The caption sits on the side **away** from the primary button, unless that side
                // has no room. A horizontal dial used to print every label to the right of its
                // circle, which is where the next circle and the primary button are; a vertical
                // dial near the trailing edge has no room on the right at all.
                let right_room = self.geometry().x + self.geometry().width as i32
                    - (target.x + target.width as i32);
                let expands_left =
                    target.x + target.width as i32 / 2 < main.x + main.width as i32 / 2;
                let put_left = expands_left || right_room < target.width as i32 / 2;
                let gap = SPEED_DIAL_LABEL_GAP;
                let (label_rect, alignment) = if put_left {
                    let right = target.x - gap;
                    let left = self.geometry().x;
                    (
                        Rect::new(left, line.y, (right - left).max(0) as u32, line.height.max(1)),
                        HorizontalAlignment::Right,
                    )
                } else {
                    let left = target.x + target.width as i32 + gap;
                    let right = self.geometry().x + self.geometry().width as i32;
                    (
                        Rect::new(left, line.y, (right - left).max(0) as u32, line.height.max(1)),
                        HorizontalAlignment::Left,
                    )
                };
                if label_rect.width > 0 {
                    context.draw_text_fitted_cow(
                        label_rect,
                        &action.label,
                        &label_font,
                        ink.with_alpha((255.0 * progress) as u8),
                        alignment,
                    );
                }
            }
        }

        // Primary button.
        let center = Point::new(main.x + main.width as i32 / 2, main.y + main.height as i32 / 2);
        let radius = main.width / 2;
        context.fill_circle_aa(center, radius, accent);
        let mark = accent.contrast_color();
        let inset = (radius as f32 * 0.35) as i32;
        let half = radius as i32;
        // A plus when collapsed, a cross when open: drawn as strokes so no font is needed.
        context.draw_line(
            Point::new(center.x - half + inset, center.y),
            Point::new(center.x + half - inset, center.y),
            mark,
        );
        if !self.expanded {
            context.draw_line(
                Point::new(center.x, center.y - half + inset),
                Point::new(center.x, center.y + half - inset),
                mark,
            );
        } else {
            context.draw_line(
                Point::new(center.x - half + inset, center.y - half + inset),
                Point::new(center.x + half - inset, center.y + half - inset),
                mark,
            );
            context.draw_line(
                Point::new(center.x + half - inset, center.y - half + inset),
                Point::new(center.x - half + inset, center.y + half - inset),
                mark,
            );
        }
    }
}

impl Widget for SpeedDial {
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
        let step = SPEED_DIAL_BUTTON.saturating_add(self.spacing);
        let extent = step * self.actions.len() as u32 + SPEED_DIAL_BUTTON;
        match self.direction {
            SpeedDialDirection::Up => {
                Size::new(SPEED_DIAL_BUTTON + 160, extent.max(SPEED_DIAL_BUTTON))
            }
            SpeedDialDirection::Left => Size::new(extent.max(SPEED_DIAL_BUTTON), SPEED_DIAL_BUTTON),
        }
    }
    fn tick(&mut self, delta_ms: u32) -> bool {
        SpeedDial::tick(self, delta_ms)
    }
    fn is_animating(&self) -> bool {
        SpeedDial::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "action_activated" => {
                Some(EventSignalRef::mapped("action_activated", &self.action_activated, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "expanded_changed" => {
                Some(EventSignalRef::mapped("expanded_changed", &self.expanded_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            _ => None,
        }
    }
}

/// Encodes one action as `[label, icon_text]`.
fn action_value(action: &SpeedDialAction) -> CapabilityValue {
    CapabilityValue::Tuple(Vec::from([
        CapabilityValue::String(action.label.clone()),
        CapabilityValue::String(action.icon_text.clone()),
    ]))
}

/// Decodes the action-list property.
fn expect_actions(value: CapabilityValue) -> Result<Vec<SpeedDialAction>, CapabilityAccessError> {
    let CapabilityValue::Tuple(rows) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut actions = Vec::with_capacity(rows.len());
    for row in rows {
        let CapabilityValue::Tuple(parts) = row else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 2 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let label = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(label) => label,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let icon_text = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(text) => text,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        actions.push(SpeedDialAction { label, icon_text });
    }
    Ok(actions)
}

impl WidgetProperties for SpeedDial {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "actions" => {
                Ok(CapabilityValue::Tuple(self.actions.iter().map(action_value).collect()))
            }
            "expanded" => Ok(CapabilityValue::Bool(self.expanded)),
            "direction" => Ok(CapabilityValue::String(self.direction.as_str().to_string())),
            "spacing" => Ok(CapabilityValue::UInt(self.spacing as u64)),
            "collapse_on_activate" => Ok(CapabilityValue::Bool(self.collapse_on_activate)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "actions" => {
                self.set_actions(expect_actions(value)?);
                Ok(())
            }
            "expanded" => {
                self.set_expanded(expect_bool(value)?);
                Ok(())
            }
            "direction" => {
                let CapabilityValue::String(token) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let direction = SpeedDialDirection::from_name(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_direction(direction);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_usize(value)? as u32);
                Ok(())
            }
            "collapse_on_activate" => {
                self.set_collapse_on_activate(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "actions",
            "expanded",
            "direction",
            "spacing",
            "collapse_on_activate",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn dial() -> SpeedDial {
        let mut dial = SpeedDial::new(Rect::new(0, 0, 200, 240));
        dial.set_actions(vec![
            SpeedDialAction::new("Mail", "✉"),
            SpeedDialAction::new("Draft", "✎"),
        ]);
        dial
    }

    #[test]
    fn actions_stack_upward_from_the_main_button() {
        let dial = dial();
        let first = dial.action_rect(0).expect("first action");
        let second = dial.action_rect(1).expect("second action");
        let main = dial.main_rect();
        assert!(first.y < main.y && second.y < first.y, "the stack grows upward");
        assert_eq!(first.x, main.x);
    }

    #[test]
    fn activating_an_action_reports_it_and_collapses() {
        let mut dial = dial();
        dial.open();
        while dial.tick(16) {}
        let fired = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&fired);
        dial.action_activated.connect(move |index| sink.lock().unwrap().push(*index));
        let target = dial.action_rect(1).expect("second action");
        let pos = Point::new(target.x + 4, target.y + 4);
        dial.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(*fired.lock().unwrap(), vec![1]);
        assert!(!dial.is_expanded(), "the default contract collapses on activation");
        assert!(dial.is_animating(), "the collapse is a movement");
    }

    #[test]
    fn the_left_direction_extends_to_the_left() {
        let mut dial = dial();
        dial.set_direction(SpeedDialDirection::Left);
        let first = dial.action_rect(0).expect("first action");
        assert!(first.x < dial.main_rect().x);
    }
}
