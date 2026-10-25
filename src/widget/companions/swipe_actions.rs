// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `SwipeActions` — a multi-action background for a swiped row (COMP-A03).
//!
//! `SwipeToDismiss` reveals one label per side. The reference toolkit's swiped rows reveal
//! several actions per side ("delete", "pin", "mark"), each with its own colour and icon,
//! appearing in order as the row travels. This companion owns that arrangement: the host
//! feeds it the row's offset and draws it behind the content, and it reports which action a
//! release landed on.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_u32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_draw_bridge, impl_widget_property_hooks, property_names_of};

use super::helpers::expect_color;

/// Which side of the row an action belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeSide {
    /// Revealed by swiping toward the trailing edge (a positive offset).
    Leading,
    /// Revealed by swiping toward the leading edge (a negative offset).
    Trailing,
}

impl SwipeSide {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            SwipeSide::Leading => "leading",
            SwipeSide::Trailing => "trailing",
        }
    }

    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "leading" => SwipeSide::Leading,
            "trailing" => SwipeSide::Trailing,
            _ => return None,
        })
    }
}

/// One action revealed behind a swiped row.
#[derive(Debug, Clone, PartialEq)]
pub struct SwipeAction {
    /// The action's label.
    pub text: String,
    /// An optional icon token.
    pub icon: Option<String>,
    /// An optional fill, overriding the semantic destructive red.
    pub fill: Option<Color>,
}

impl SwipeAction {
    /// Creates an action with the default (destructive) fill.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), icon: None, fill: None }
    }

    /// Adds an icon token.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets the fill colour.
    pub fn with_fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }
}

/// The multi-action background painted behind a swiped row.
pub struct SwipeActions {
    base: BaseWidget,
    leading_actions: Vec<SwipeAction>,
    trailing_actions: Vec<SwipeAction>,
    max_reveal_width: u32,
    icon_side: u32,
    /// The row's current horizontal offset, set by the host.
    offset: f32,
    /// Emitted with the side and index of the action a press landed on.
    pub action_triggered: Signal1<(SwipeSide, usize)>,
}

impl Default for SwipeActions {
    fn default() -> Self {
        Self::new()
    }
}

impl SwipeActions {
    /// Creates an empty background. The host sets its geometry through [`Widget::set_geometry`].
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::SwipeToDismiss,
                Rect::new(0, 0, 0, 0),
                "SwipeActions",
            ),
            leading_actions: Vec::new(),
            trailing_actions: Vec::new(),
            max_reveal_width: 240,
            icon_side: 18,
            offset: 0.0,
            action_triggered: Signal1::new(),
        }
    }

    /// Replaces the leading-side actions.
    pub fn set_leading_actions(&mut self, actions: Vec<SwipeAction>) {
        self.leading_actions = actions;
        self.base.request_redraw();
    }

    /// Returns the leading-side actions.
    pub fn leading_actions(&self) -> &[SwipeAction] {
        &self.leading_actions
    }

    /// Replaces the trailing-side actions.
    pub fn set_trailing_actions(&mut self, actions: Vec<SwipeAction>) {
        self.trailing_actions = actions;
        self.base.request_redraw();
    }

    /// Returns the trailing-side actions.
    pub fn trailing_actions(&self) -> &[SwipeAction] {
        &self.trailing_actions
    }

    /// The furthest either side may be revealed.
    pub fn max_reveal_width(&self) -> u32 {
        self.max_reveal_width
    }

    /// Sets the furthest either side may be revealed.
    pub fn set_max_reveal_width(&mut self, width: u32) {
        if self.max_reveal_width == width {
            return;
        }
        self.max_reveal_width = width;
        self.base.request_redraw();
    }

    /// The square side of an action's icon.
    pub fn icon_side(&self) -> u32 {
        self.icon_side
    }

    /// Sets the square side of an action's icon.
    pub fn set_icon_side(&mut self, side: u32) {
        if self.icon_side == side {
            return;
        }
        self.icon_side = side;
        self.base.request_redraw();
    }

    /// Reports the row's current offset so the revealed boxes follow the gesture.
    pub fn set_offset(&mut self, offset: f32) {
        if !offset.is_finite() || (self.offset - offset).abs() < f32::EPSILON {
            return;
        }
        self.offset = offset;
        self.base.request_redraw();
    }

    /// The row's current offset.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// The revealed box of `index` on `side`, or `None` when the offset has not uncovered it.
    pub fn action_rect(&self, side: SwipeSide, index: usize) -> Option<Rect> {
        self.action_rect_at(side, index, self.offset)
    }

    /// The action under `pos` for a given offset, or `None`.
    ///
    /// Taking the offset as an argument lets a host hit-test the release position against the
    /// same arithmetic a later `set_offset` will draw with.
    pub fn action_at(&self, offset: f32, pos: Point) -> Option<(SwipeSide, usize)> {
        for side in [SwipeSide::Leading, SwipeSide::Trailing] {
            let actions = self.actions_for(side);
            for index in 0..actions.len() {
                if self
                    .action_rect_at(side, index, offset)
                    .is_some_and(|rect| rect.contains_point(pos))
                {
                    return Some((side, index));
                }
            }
        }
        None
    }

    /// The actions for `side`.
    fn actions_for(&self, side: SwipeSide) -> &[SwipeAction] {
        match side {
            SwipeSide::Leading => &self.leading_actions,
            SwipeSide::Trailing => &self.trailing_actions,
        }
    }

    /// The width one action claims on `side`.
    fn action_width(&self, side: SwipeSide) -> u32 {
        let count = self.actions_for(side).len().max(1) as u32;
        (self.max_reveal_width / count).max(1)
    }

    /// The box of `index` at `offset`, clipped to what the offset has revealed.
    fn action_rect_at(&self, side: SwipeSide, index: usize, offset: f32) -> Option<Rect> {
        let actions = self.actions_for(side);
        if index >= actions.len() {
            return None;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let width = self.action_width(side);
        let reveal = match side {
            SwipeSide::Leading => offset.max(0.0).min(self.max_reveal_width as f32) as u32,
            SwipeSide::Trailing => (-offset).max(0.0).min(self.max_reveal_width as f32) as u32,
        };
        let consumed = (index as u32).saturating_mul(width);
        if reveal <= consumed {
            return None;
        }
        let visible = (reveal - consumed).min(width);
        let x = match side {
            SwipeSide::Leading => rect.x + consumed as i32,
            SwipeSide::Trailing => rect.x + rect.width as i32 - (consumed + visible) as i32,
        };
        Some(Rect::new(x, rect.y, visible, rect.height))
    }

    /// The fill an action is painted with.
    fn fill_of(&self, action: &SwipeAction) -> Color {
        action.fill.unwrap_or_else(|| {
            crate::style::semantic_color(crate::style::SemanticColor::Error)
                .unwrap_or(Color::rgba(255, 59, 48, 255))
        })
    }

    /// Paints one side's revealed actions.
    fn draw_side(&self, context: &mut RenderContext, side: SwipeSide) {
        let actions = self.actions_for(side);
        if actions.is_empty() {
            return;
        }
        let font = role_font(self.base.style(), 14.0);
        for (index, action) in actions.iter().enumerate() {
            let Some(rect) = self.action_rect(side, index) else {
                continue;
            };
            context.fill_rect(rect, self.fill_of(action));
            if let Some(icon) = action.icon.as_deref() {
                let side_px = self.icon_side.min(rect.width).min(rect.height).max(1);
                let icon_rect = Rect::new(
                    rect.x + (rect.width.saturating_sub(side_px) / 2) as i32,
                    rect.y + (rect.height.saturating_sub(side_px) / 2) as i32,
                    side_px,
                    side_px,
                );
                if !crate::widget::draw_icon_named(
                    context,
                    icon_rect,
                    self.fill_of(action).contrast_color(),
                    icon,
                ) {
                    context.draw_text(
                        Point::new(icon_rect.x, icon_rect.y),
                        icon,
                        &font,
                        self.fill_of(action).contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }
            }
            let line = context.text_line(rect, &font);
            context.draw_text_fitted_cow(
                Rect::new(rect.x + 4, line.y, rect.width.saturating_sub(8), line.height.max(1)),
                &action.text,
                &font,
                self.fill_of(action).contrast_color(),
                HorizontalAlignment::Center,
            );
        }
    }
}

impl EventHandler for SwipeActions {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some((side, index)) = self.action_at(self.offset, *pos) {
                self.action_triggered.emit((side, index));
            }
        }
    }
}

impl Draw for SwipeActions {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // The background is chrome: it only occupies the revealed band, so an unswiped row is
        // left untouched (the row's own content paints over the same rectangle).
        if self.offset > 0.0 {
            self.draw_side(context, SwipeSide::Leading);
        } else if self.offset < 0.0 {
            self.draw_side(context, SwipeSide::Trailing);
        }
    }
}

impl Widget for SwipeActions {
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
        Size::new(self.max_reveal_width, 48)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |value| {
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::String(value.0.as_str().to_string()),
                        CapabilityValue::UInt(value.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// Encodes one action as `[text, icon-or-null, fill-or-null]`.
fn action_value(action: &SwipeAction) -> CapabilityValue {
    CapabilityValue::Tuple(Vec::from([
        CapabilityValue::String(action.text.clone()),
        match action.icon.as_deref() {
            Some(icon) => CapabilityValue::String(icon.to_string()),
            None => CapabilityValue::Null,
        },
        match action.fill {
            Some(fill) => CapabilityValue::Color(fill),
            None => CapabilityValue::Null,
        },
    ]))
}

/// Decodes an action-list property.
fn expect_actions(value: CapabilityValue) -> Result<Vec<SwipeAction>, CapabilityAccessError> {
    let CapabilityValue::Tuple(rows) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut actions = Vec::with_capacity(rows.len());
    for row in rows {
        let CapabilityValue::Tuple(parts) = row else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let text = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(text) => text,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let icon = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(icon) => Some(icon),
            CapabilityValue::Null => None,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let fill = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::Null => None,
            other => Some(expect_color(other)?),
        };
        actions.push(SwipeAction { text, icon, fill });
    }
    Ok(actions)
}

impl WidgetProperties for SwipeActions {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "leading_actions" => {
                Ok(CapabilityValue::Tuple(self.leading_actions.iter().map(action_value).collect()))
            }
            "trailing_actions" => {
                Ok(CapabilityValue::Tuple(self.trailing_actions.iter().map(action_value).collect()))
            }
            "max_reveal_width" => Ok(CapabilityValue::UInt(self.max_reveal_width as u64)),
            "icon_side" => Ok(CapabilityValue::UInt(self.icon_side as u64)),
            "offset" => Ok(CapabilityValue::Float(f64::from(self.offset))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "max_reveal_width" => {
                self.set_max_reveal_width(expect_u32(value)?);
                Ok(())
            }
            "icon_side" => {
                self.set_icon_side(expect_u32(value)?);
                Ok(())
            }
            "leading_actions" => {
                self.set_leading_actions(expect_actions(value)?);
                Ok(())
            }
            "trailing_actions" => {
                self.set_trailing_actions(expect_actions(value)?);
                Ok(())
            }
            "offset" => {
                let CapabilityValue::Float(offset) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_offset(offset as f32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "leading_actions",
            "trailing_actions",
            "max_reveal_width",
            "icon_side",
            "offset",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn actions() -> SwipeActions {
        let mut actions = SwipeActions::new();
        actions.set_geometry(Rect::new(0, 0, 300, 60));
        actions.set_max_reveal_width(200);
        actions.set_trailing_actions(vec![SwipeAction::new("Delete"), SwipeAction::new("Pin")]);
        actions.set_leading_actions(vec![SwipeAction::new("Archive")]);
        actions
    }

    #[test]
    fn the_offset_reveals_actions_in_order() {
        let mut actions = actions();
        actions.set_offset(-50.0);
        assert!(actions.action_rect(SwipeSide::Trailing, 0).is_some());
        assert!(actions.action_rect(SwipeSide::Trailing, 1).is_none());
        actions.set_offset(-200.0);
        assert!(actions.action_rect(SwipeSide::Trailing, 1).is_some());
        assert!(actions.action_rect(SwipeSide::Leading, 0).is_none());
        actions.set_offset(20.0);
        assert!(actions.action_rect(SwipeSide::Leading, 0).is_some());
    }

    #[test]
    fn a_press_reports_the_action_under_it() {
        let mut actions = actions();
        actions.set_offset(-200.0);
        let rect = actions.action_rect(SwipeSide::Trailing, 1).expect("second action");
        let pos = Point::new(rect.x + rect.width as i32 / 2, rect.y + 4);
        assert_eq!(actions.action_at(-200.0, pos), Some((SwipeSide::Trailing, 1)));
        let fired = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&fired);
        actions.action_triggered.connect(move |value| sink.lock().unwrap().push(*value));
        actions.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(*fired.lock().unwrap(), vec![(SwipeSide::Trailing, 1)]);
    }
}
