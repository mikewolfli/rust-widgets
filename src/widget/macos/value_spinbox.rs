// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ValueSpinBox` — the numeric field that pairs with a stepper (COMP-L05).
//!
//! `MacStepper` draws only its two arrows; a desktop form pairs it with a value field. This
//! companion owns the value, the range, the step, the prefix/suffix text and the inline editor,
//! and emits the value once per change. It renders a text field plus stepper arrows in one
//! control, so a host does not have to keep two widgets' geometry in sync.
//!
//! Registration surface (out of scope for this file): canonical name `value_spinbox`, kind
//! [`WidgetKind::SpinBox`].

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// An editable integer with a stepper.
pub struct ValueSpinBox {
    base: BaseWidget,
    value: i64,
    minimum: i64,
    maximum: i64,
    step: i64,
    prefix: String,
    suffix: String,
    editable: bool,
    /// Whether the inline editor has focus.
    editing: bool,
    /// The text being edited; committed on Enter and discarded on Escape.
    draft: String,
    /// The non-edit display text, rebuilt when the value or an affix changes.
    display_cache: String,
    /// Emitted with the new value whenever it changes.
    pub value_changed: Signal1<i64>,
    /// Emitted with the final value when editing ends.
    pub editing_finished: Signal1<i64>,
}

impl ValueSpinBox {
    /// Creates a spin box over `0..=100` with a step of 1.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SpinBox, geometry, "ValueSpinBox"),
            value: 0,
            minimum: 0,
            maximum: 100,
            step: 1,
            prefix: String::new(),
            suffix: String::new(),
            editable: true,
            editing: false,
            draft: String::new(),
            display_cache: String::from("0"),
            value_changed: Signal1::new(),
            editing_finished: Signal1::new(),
        }
    }

    /// The current value.
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Sets the value, clamped to the range. Emits only on a real change.
    pub fn set_value(&mut self, value: i64) {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.rebuild_display_cache();
        self.base.request_redraw();
        self.value_changed.emit(clamped);
    }

    /// The accepted range.
    pub fn range(&self) -> (i64, i64) {
        (self.minimum, self.maximum)
    }

    /// Sets the range, re-clamping the current value and refusing an inverted range.
    pub fn set_range(&mut self, minimum: i64, maximum: i64) {
        if minimum > maximum {
            return;
        }
        if self.minimum == minimum && self.maximum == maximum {
            return;
        }
        self.minimum = minimum;
        self.maximum = maximum;
        let clamped = self.value.clamp(minimum, maximum);
        if clamped != self.value {
            self.value = clamped;
            self.rebuild_display_cache();
            self.value_changed.emit(clamped);
        }
        self.base.request_redraw();
    }

    /// The step applied by the arrows and the arrow keys.
    pub fn step(&self) -> i64 {
        self.step
    }

    /// Sets the step; zero is refused because a step that changes nothing is not a step.
    pub fn set_step(&mut self, step: i64) {
        if step == 0 || self.step == step {
            return;
        }
        self.step = step;
    }

    /// The prefix drawn before the number.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Sets the prefix.
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
        self.rebuild_display_cache();
        self.base.request_redraw();
    }

    /// The suffix drawn after the number.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Sets the suffix.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        self.suffix = suffix.into();
        self.rebuild_display_cache();
        self.base.request_redraw();
    }

    /// Whether inline typing is allowed.
    pub fn editable(&self) -> bool {
        self.editable
    }

    /// Turns inline typing on or off.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        if !editable {
            self.editing = false;
            self.draft.clear();
        }
        self.base.request_redraw();
    }

    /// Rebuilds the non-edit display text from the current value and affixes.
    fn rebuild_display_cache(&mut self) {
        self.display_cache.clear();
        self.display_cache.push_str(&self.prefix);
        let _ =
            core::fmt::Write::write_fmt(&mut self.display_cache, format_args!("{}", self.value));
        self.display_cache.push_str(&self.suffix);
    }

    /// The field's rectangle: everything left of the stepper buttons.
    fn field_rect(&self) -> Rect {
        let rect = self.geometry();
        let buttons = 18.min(rect.width / 2);
        Rect::new(rect.x, rect.y, rect.width.saturating_sub(buttons), rect.height)
    }

    /// The up arrow's rectangle.
    fn up_rect(&self) -> Rect {
        let rect = self.geometry();
        let buttons = 18.min(rect.width / 2);
        Rect::new(rect.right() - buttons as i32, rect.y, buttons, rect.height / 2)
    }

    /// The down arrow's rectangle.
    fn down_rect(&self) -> Rect {
        let rect = self.geometry();
        let buttons = 18.min(rect.width / 2);
        let top = rect.y + rect.height as i32 / 2;
        Rect::new(rect.right() - buttons as i32, top, buttons, rect.height - rect.height / 2)
    }

    /// Applies one step and emits.
    fn step_by(&mut self, direction: i64) {
        let next = self.value.saturating_add(self.step.saturating_mul(direction));
        self.set_value(next);
    }

    /// Commits the draft, if one is being edited.
    fn commit_draft(&mut self) {
        if !self.editing {
            return;
        }
        if let Ok(parsed) = self.draft.trim().parse::<i64>() {
            self.set_value(parsed);
        }
        self.draft.clear();
        self.editing = false;
        self.base.request_redraw();
        self.editing_finished.emit(self.value);
    }
}

impl Widget for ValueSpinBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(120, 24)
    }
    impl_draw_bridge!();
}

impl Draw for ValueSpinBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(250, 250, 250, 255));
        let font = &role_font(style, 12.0);
        let field = self.field_rect();
        context.fill_rounded_rect(field, 4, surface);
        context.draw_rounded_rect_stroke(
            field,
            4,
            if self.editing { ink } else { ink.with_alpha(60) },
            1,
        );
        let text: &str = if self.editing {
            // Borrowed, not cloned: the draft already holds the text being edited.
            self.draft.as_str()
        } else {
            &self.display_cache
        };
        context.draw_text_fitted_cow(
            Rect::new(
                field.x + 6,
                field.y + 2,
                field.width.saturating_sub(12),
                field.height.saturating_sub(4),
            ),
            text,
            font,
            ink,
            HorizontalAlignment::Left,
        );

        for (rect, icon) in
            [(self.up_rect(), IconName::ChevronUp), (self.down_rect(), IconName::ChevronDown)]
        {
            context.fill_rect(rect, surface);
            context.draw_rect_stroke(rect, ink.with_alpha(50), 1);
            crate::widget::draw_icon_at(context, rect, ink, icon);
        }
    }
}

impl EventHandler for ValueSpinBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if !self.geometry().contains_point(*pos) {
                    return;
                }
                if self.up_rect().contains_point(*pos) {
                    self.step_by(1);
                    return;
                }
                if self.down_rect().contains_point(*pos) {
                    self.step_by(-1);
                    return;
                }
                if self.editable && self.field_rect().contains_point(*pos) {
                    self.editing = true;
                    self.draft = self.value.to_string();
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } if self.editing => match *key {
                13 => self.commit_draft(),
                27 => {
                    self.editing = false;
                    self.draft.clear();
                    self.base.request_redraw();
                }
                8 => {
                    self.draft.pop();
                    self.base.request_redraw();
                }
                38 => self.step_by(1),
                40 => self.step_by(-1),
                _ => {}
            },
            Event::KeyPress { key: 38, .. } => self.step_by(1),
            Event::KeyPress { key: 40, .. } => self.step_by(-1),
            Event::TextInput { text } if self.editing => {
                self.draft.push_str(text);
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_step_and_clamp() {
        let mut box_ = ValueSpinBox::new(Rect::new(0, 0, 120, 24));
        box_.set_range(0, 2);
        box_.set_value(2);
        let up = box_.up_rect();
        box_.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(up.center().x, up.center().y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(box_.value(), 2, "a step past the maximum is clamped");
        let down = box_.down_rect();
        box_.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(down.center().x, down.center().y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(box_.value(), 1);
    }

    #[test]
    fn typing_commits_on_enter() {
        let mut box_ = ValueSpinBox::new(Rect::new(0, 0, 120, 24));
        box_.set_range(0, 50);
        box_.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        box_.handle_event(&Event::TextInput { text: "42".to_string() });
        box_.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(box_.value(), 42);
    }
}
