// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! FloatingLabel widget — a text input with a floating label (Material Design style).
//!
//! The FloatingLabel widget combines a text input field with a label that
//! animates from inside the field to above it when the field is focused or
//! contains text. It also supports placeholder text that is shown when the
//! field is empty and unfocused.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A text input with a floating label (Material Design TextInputLayout style).
///
/// The label floats above the text field when the field is focused or contains
/// text. When the field is empty and unfocused, the label appears inside the
/// field (or a separate placeholder text is shown). The transition between these
/// two states is animated: [`FloatingLabel::tick`] advances an interpolation
/// value (`0.0` = label inside, `1.0` = label fully above) that [`FloatingLabel`]
/// consumes when drawing, so the label smoothly rises rather than teleporting.
pub struct FloatingLabel {
    base: BaseWidget,
    text: String,
    label: String,
    placeholder: String,
    is_focused: bool,
    show_label_above: bool,
    /// Interpolated float position, advanced toward `target_progress` by
    /// [`FloatingLabel::tick`] and consumed by the draw pass. `0.0` draws the
    /// label inline; `1.0` draws it fully above the field.
    animation_progress: f32,
    /// The value `animation_progress` moves toward — `1.0` when the label should
    /// float, `0.0` when it should rest inline.
    target_progress: f32,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
}

impl FloatingLabel {
    /// Creates a new FloatingLabel widget with the given geometry.
    ///
    /// By default, no text, empty label, empty placeholder, unfocused.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FloatingLabel, geometry, "FloatingLabel"),
            text: String::new(),
            label: String::new(),
            placeholder: String::new(),
            is_focused: false,
            show_label_above: false,
            animation_progress: 0.0,
            target_progress: 0.0,
            text_changed: Signal1::new(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content and emits `text_changed` signal.
    /// Also updates the floating label state based on whether text is non-empty.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.update_label_state();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Returns the label text.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Sets the label text (shown inside field or floating above).
    pub fn set_label(&mut self, label: String) {
        self.label = label;
        self.update_label_state();
        self.base.request_redraw();
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text (shown when empty and unfocused).
    pub fn set_placeholder(&mut self, placeholder: String) {
        self.placeholder = placeholder;
        self.base.request_redraw();
    }

    /// Returns whether the input field is currently focused.
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Returns whether the text content is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Sets the focused state. When focused, the label floats above.
    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.update_label_state();
            self.base.request_redraw();
        }
    }

    /// Updates whether the label should float above based on focus and content.
    fn update_label_state(&mut self) {
        let should_float = self.is_focused || !self.text.is_empty();
        if should_float != self.show_label_above {
            self.show_label_above = should_float;
            // Retarget, rather than jump: `tick` then interpolates the visible
            // position toward this target across subsequent frames.
            self.target_progress = if should_float { 1.0 } else { 0.0 };
        }
    }

    /// Advances the floating-label animation toward its target.
    ///
    /// `delta_ms` is the elapsed time since the previous frame. The label travels
    /// the full inline→floating distance in about 150 ms, then holds. Returns
    /// `true` when the interpolation changed and the widget needs a redraw, so the
    /// caller can schedule the next frame only while the animation is still moving
    /// (the same contract [`crate::widget::display_widgets::spinner::Spinner::tick`]
    /// and the other animated widgets follow).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if (self.animation_progress - self.target_progress).abs() < f32::EPSILON {
            return false;
        }
        // Interpolate a fixed fraction of the remaining distance; `clamp` to 1.0
        // means a single large `delta_ms` lands exactly on the target instead of
        // overshooting.
        const FULL_TRAVEL_MS: f32 = 150.0;
        let step = (delta_ms as f32 / FULL_TRAVEL_MS).clamp(0.0, 1.0);
        let next =
            self.animation_progress + (self.target_progress - self.animation_progress) * step;
        // Never step past the target: once the interpolation would cross it, settle
        // exactly on it so the value stays in `0.0 ..= 1.0`.
        self.animation_progress = if (next - self.target_progress).abs() < f32::EPSILON
            || (next > self.animation_progress) == (self.target_progress > self.animation_progress)
        {
            next
        } else {
            self.target_progress
        };
        self.base.request_redraw();
        true
    }

    /// Returns the current interpolation of the label's float position, in
    /// `0.0 ..= 1.0`. Exposed for tests and animation-aware hosts; the draw pass
    /// consumes the same value to place the label.
    pub fn animation_progress(&self) -> f32 {
        self.animation_progress
    }
}

impl Widget for FloatingLabel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 40)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FloatingLabel`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for FloatingLabel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "focused" => Ok(CapabilityValue::Bool(self.is_focused())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "focused" => {
                self.set_focused(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "placeholder", "focused", BASE_PROPERTY_NAMES]
    }
}

impl Draw for FloatingLabel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the field fill and
        // its underline were previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("floating_label");
        // The label kind classifies as plain text, so the theme leaves its background
        // unset; a floating *label* decorates an editable field, so the field interior is
        // read from the input role, which resolves a colour in every appearance.
        let field_background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .or_else(|| {
                crate::style::resolved_theme_style("line_edit")
                    .and_then(|input| input.background_color)
            })
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        // The label is a `Text` role, so its resolved ink is the theme's foreground; the
        // border colour carries the underline and the focused accent.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| ink.blend(&field_background, 0.55));

        // Draw the text field background
        let bg_color = if is_enabled { field_background } else { Color::rgba(240, 240, 240, 255) };
        context.fill_rounded_rect(rect, 4, bg_color);

        // Draw the underline/border. Focused is the resolved ink, undamped so it reads as
        // active; resting is the same ink damped toward the field it sits on.
        let underline_color = if self.is_focused { ink } else { border_color };
        let underline_y = rect.y + rect.height as i32 - 2;
        let underline_rect = Rect::new(rect.x + 2, underline_y, rect.width.saturating_sub(4), 2);
        context.fill_rounded_rect(underline_rect, 1, underline_color);

        // Fonts
        let input_font = Font::simple("sans-serif", 14.0);
        let label_font = Font::simple("sans-serif", 11.0);
        let padding = 8i32;
        let label_top_margin = 4i32;

        // Calculate positions
        let has_label = !self.label.is_empty();
        let text_field_top_offset = if has_label && self.show_label_above {
            16i32 // space for floating label
        } else {
            6i32
        };

        // Draw the label (floating above or inline), interpolating its vertical
        // position by `animation_progress` so the float transition is smooth.
        if has_label {
            let label_color = if self.is_focused {
                ink
            } else if is_enabled {
                ink.blend(&field_background, 0.3)
            } else {
                Color::rgba(180, 180, 180, 255)
            };

            // The two resting positions for the label baseline.
            let above_y = rect.y + label_top_margin + 10;
            let inline_y = rect.y + 6i32 + 14;
            if self.show_label_above || self.animation_progress > 0.0 {
                // Interpolate from the inline position up to the floating position.
                let label_y =
                    inline_y + ((above_y - inline_y) as f32 * self.animation_progress) as i32;
                let label_x = rect.x + padding;
                context.draw_text(
                    Point::new(label_x, label_y),
                    &self.label,
                    &label_font,
                    label_color,
                    HorizontalAlignment::Left,
                );
            } else if self.text.is_empty() && !self.is_focused {
                // Label inline acts as placeholder
                let label_x = rect.x + padding;
                context.draw_text(
                    Point::new(label_x, inline_y),
                    &self.label,
                    &input_font,
                    ink.blend(&field_background, 0.45),
                    HorizontalAlignment::Left,
                );
            }
        }

        // Show placeholder when empty, unfocused, and label is not shown inline
        let show_placeholder =
            self.text.is_empty() && !self.is_focused && (!has_label || self.show_label_above);

        if show_placeholder && !self.placeholder.is_empty() {
            let placeholder_x = rect.x + padding;
            let placeholder_y = rect.y + text_field_top_offset + 14;
            context.draw_text(
                Point::new(placeholder_x, placeholder_y),
                &self.placeholder,
                &input_font,
                ink.blend(&field_background, 0.55),
                HorizontalAlignment::Left,
            );
        }

        // Draw input text
        if !self.text.is_empty() {
            let text_x = rect.x + padding;
            let text_y = rect.y + text_field_top_offset + 14;
            let text_color = if is_enabled { ink } else { Color::rgba(160, 160, 160, 255) };
            context.draw_text(
                Point::new(text_x, text_y),
                &self.text,
                &input_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

const KEYCODE_TAB: u32 = 9;
const KEYCODE_ENTER: u32 = 13;
const KEYCODE_BACKSPACE: u32 = 8;

impl EventHandler for FloatingLabel {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                if rect.contains_point(*pos) {
                    self.set_focused(true);
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                if *key == KEYCODE_TAB {
                    // Tab — lose focus
                    self.set_focused(false);
                } else if *key == KEYCODE_ENTER {
                    // Enter — lose focus
                    self.set_focused(false);
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append to text
                    let c = char::from_u32(*key).unwrap_or(' ');
                    if self.is_focused {
                        let mut new_text = self.text.clone();
                        new_text.push(c);
                        self.set_text(new_text);
                    }
                } else if *key == KEYCODE_BACKSPACE {
                    // Backspace
                    if self.is_focused && !self.text.is_empty() {
                        let mut new_text = self.text.clone();
                        new_text.pop();
                        self.set_text(new_text);
                    }
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
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    #[test]
    fn floating_label_default_creation() {
        let fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert_eq!(fl.kind(), WidgetKind::FloatingLabel);
        assert!(fl.text().is_empty());
        assert!(fl.label().is_empty());
        assert!(fl.placeholder().is_empty());
        assert!(!fl.is_focused());
        assert!(fl.is_empty());
    }

    #[test]
    fn floating_label_set_text_and_label() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Username".to_string());
        assert_eq!(fl.label(), "Username");

        fl.set_text("hello".to_string());
        assert_eq!(fl.text(), "hello");
        assert!(!fl.is_empty());

        // Label should float above since text is non-empty; the target is 1.0 and
        // the interpolation animates toward it rather than jumping instantly.
        assert!(fl.show_label_above);
        assert_eq!(fl.animation_progress(), 0.0);
        assert!(fl.tick(16));
        assert!(fl.animation_progress() > 0.0);
    }

    #[test]
    fn floating_label_animation_reaches_target() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        fl.set_focused(true);
        assert!(fl.show_label_above);

        // A single large step lands exactly on the target and stays there.
        assert!(fl.tick(1000));
        assert_eq!(fl.animation_progress(), 1.0);
        assert!(!fl.tick(1000)); // already at target — no further work

        // Losing focus retargets back to inline.
        fl.set_focused(false);
        assert!(!fl.show_label_above);
        assert!(fl.tick(1000));
        assert_eq!(fl.animation_progress(), 0.0);
    }

    #[test]
    fn floating_label_focus_toggle() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Email".to_string());
        assert!(!fl.is_focused());
        assert!(!fl.show_label_above);

        fl.set_focused(true);
        assert!(fl.is_focused());
        assert!(fl.show_label_above);

        fl.set_focused(false);
        assert!(!fl.is_focused());
        // Should still float since text is empty? No, empty + not focused = not floating
        assert!(!fl.show_label_above);
    }

    #[test]
    fn floating_label_text_changed_signal() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        let captured = Arc::new(Mutex::new(None::<String>));
        fl.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        fl.set_text("World".to_string());
        assert_eq!(captured.lock().unwrap().as_deref(), Some("World"));
    }

    #[test]
    fn floating_label_placeholder() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_placeholder("Enter text here...".to_string());
        assert_eq!(fl.placeholder(), "Enter text here...");
    }

    #[test]
    fn floating_label_focus_on_click() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        assert!(!fl.is_focused());

        fl.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(fl.is_focused());
    }

    #[test]
    fn floating_label_keyboard_input() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_focused(true);

        // Type 'A'
        fl.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
        assert_eq!(fl.text(), "A");

        // Type 'B'
        fl.handle_event(&Event::KeyPress { key: 66, modifiers: 0 });
        assert_eq!(fl.text(), "AB");

        // Backspace
        fl.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
        assert_eq!(fl.text(), "A");
    }

    #[test]
    fn floating_label_svg_output() {
        let mut fl = FloatingLabel::new(Rect::new(0, 0, 200, 50));
        fl.set_label("Name".to_string());
        fl.set_placeholder("Enter name".to_string());
        fl.set_text("John".to_string());
        let svg = render_to_svg(&mut fl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
