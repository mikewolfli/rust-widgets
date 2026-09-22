// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Snackbar widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Horizontal padding of a snackbar's message from the bar's leading edge: 10.
///
/// One value for the message inset and the action button's trailing margin, so the two ends of
/// the bar are inset by the same amount rather than by `10` on one side and `14` on the other.
const SNACKBAR_PADDING_H: i32 = 10;

/// A snackbar action button's width: 76.
const SNACKBAR_ACTION_WIDTH: u32 = 76;

/// A snackbar action button's height: 20.
const SNACKBAR_ACTION_HEIGHT: u32 = 20;

/// The thickness of a snackbar's progress rule: 3.
const SNACKBAR_PROGRESS_HEIGHT: u32 = 3;

/// Snackbar with optional action and progress display.
pub struct Snackbar {
    base: BaseWidget,
    message: String,
    action_label: Option<String>,
    visible: bool,
    progress: Option<f32>,
    /// Emitted when action is triggered.
    pub action_triggered: Signal1<String>,
    /// Emitted when snackbar is dismissed.
    pub dismissed: Signal1<()>,
}

impl Snackbar {
    /// Creates hidden snackbar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "Snackbar"),
            message: String::new(),
            action_label: None,
            visible: false,
            progress: None,
            action_triggered: Signal1::new(),
            dismissed: Signal1::new(),
        }
    }

    /// Returns visibility.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns current message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Shows snackbar without action.
    pub fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.action_label = None;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Shows snackbar with action button.
    pub fn show_with_action(
        &mut self,
        message: impl Into<String>,
        action_label: impl Into<String>,
    ) {
        self.message = message.into();
        self.action_label = Some(action_label.into());
        self.visible = true;
        self.base.request_redraw();
    }

    /// Dismisses snackbar.
    pub fn dismiss(&mut self) {
        if self.visible {
            self.visible = false;
            self.dismissed.emit(());
            self.base.request_redraw();
        }
    }

    /// Sets optional progress indicator.
    pub fn set_progress(&mut self, progress: Option<f32>) {
        self.progress = progress.map(|value| value.clamp(0.0, 1.0));
        self.base.request_redraw();
    }

    /// Returns action label.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    fn trigger_action(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        let Some(label) = self.action_label.clone() else {
            return false;
        };
        self.action_triggered.emit(label);
        true
    }

    fn action_rect(&self) -> Option<Rect> {
        self.action_label.as_ref()?;
        // Derived from the same bar the message is painted in, so the button sits on the right
        // end of the snackbar rather than at a fixed offset from the control's bottom edge —
        // which placed it 34 px below the bar in a 120 px cell.
        let bar = ControlMetrics::full_width_band(self.geometry(), dimensions::SNACKBAR_HEIGHT);
        let height = SNACKBAR_ACTION_HEIGHT.min(bar.height);
        let width = SNACKBAR_ACTION_WIDTH.min(bar.width);
        Some(Rect::new(
            bar.x + bar.width as i32 - width as i32 - SNACKBAR_PADDING_H,
            bar.y + (bar.height.saturating_sub(height) / 2) as i32,
            width,
            height,
        ))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }
}

impl Widget for Snackbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 48)
    }

    impl_widget_property_hooks!();
}

/// `Snackbar`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_base.in.rs` dispatch. `visible` is deliberately *not* listed
/// here: the snackbar's own visibility flag hides it through `dismiss`, and the
/// shared four already publish a `visible` property, so a second name for the
/// same concept would be ambiguous. `action_label` is owned by `show_with_action`
/// and has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for Snackbar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "message" => Ok(CapabilityValue::String(self.message().to_string())),
            "action_label" => match self.action_label() {
                Some(label) => Ok(CapabilityValue::String(label.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "message" => {
                // `show` clears the action label, so setting a plain message
                // matches the legacy "message" write, which showed without action.
                self.show(expect_string(value)?);
                Ok(())
            }
            "action_label" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["message", "action_label", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `snackbar` publishes.
    ///
    /// `dismiss` maps onto the widget's real `dismiss`; it needs no payload.
    /// `show` and `show_with_action` both require the message (and the action
    /// label, and the slot behind it) the caller wants shown, so a bare
    /// invocation is reported as needing one rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "dismiss" => {
                self.dismiss();
                Ok(())
            }
            "show" | "show_with_action" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Snackbar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(rect) = self.action_rect() {
                    if Self::point_in_rect(*pos, rect) {
                        let _ = self.trigger_action();
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 => {
                    let _ = self.trigger_action();
                }
                27 => self.dismiss(),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Snackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("snackbar");
        // `Snackbar` is built on `WidgetKind::StatusBar` and classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The panel's own fill is therefore a step toward
        // the foreground, so it reads as a surface of its own rather than as bare
        // window.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(248, 250, 253));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // The floating bar is the control's own emphasis surface: a first step away
        // from the panel, so it stays distinct in either appearance.
        let bar_background = background.blend(&text_color, 0.85);
        let bar_border = bar_background.blend(&text_color, 0.18);
        // Text on the bar is pushed back toward the panel, which is the reading the
        // literal pair encoded (dark bar, near-white text) without hardcoding it.
        let bar_text = bar_background.blend(&background, 0.92);
        // The action is a brand action, so it reads the theme's primary token rather
        // than a literal blue.
        let action_background = crate::style::resolved_theme_style("button")
            .and_then(|button| button.background_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.55));
        let action_border = action_background.blend(&text_color, 0.18);
        let action_text = action_background.blend(&background, 0.92);
        // Progress is a completion *state*, so it reads the theme's success token.
        let track = bar_background.blend(&text_color, 0.12);
        let progress_fill = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| token.blend(&bar_background, 0.2))
            .unwrap_or_else(|| bar_background.blend(&background, 0.5));

        // ── The bar actually painted ──
        //
        // `rect` is the area the control was *given*; a snackbar is one floating bar of
        // [`dimensions::SNACKBAR_HEIGHT`], centred in that area. Filling the whole rectangle
        // made a 240x120 census cell a full-bleed panel with a 26 px bar pinned near its
        // bottom — a surface shaped like a snackbar rather than a snackbar — and the panel's
        // own fill is not part of this control's chrome at all: a snackbar sits *over* the
        // page, it does not paint the page. The band is the single derivation the message, the
        // action button and the progress rule are all placed from.
        let bar = ControlMetrics::full_width_band(rect, dimensions::SNACKBAR_HEIGHT);
        context.fill_rect(bar, bar_background);
        context.draw_rect(bar, bar_border);

        if !self.visible {
            return;
        }

        // Both labels are centred on their own band. The origins were fixed offsets
        // (`+ 16` and `+ 13`) written for one font size: a 14 px line in a 26 px bar spans
        // 16..30, four pixels past the bar's bottom edge, and the action label likewise.
        let message_font = Font::default();
        let message_h = context.measure_text("M", &message_font).height;
        if !self.message.is_empty() {
            context.draw_text(
                Point::new(
                    bar.x + SNACKBAR_PADDING_H,
                    bar.y + (bar.height as i32 - message_h as i32) / 2,
                ),
                &self.message,
                &message_font,
                bar_text,
                HorizontalAlignment::Left,
            );
        }

        if let Some(action_rect) = self.action_rect() {
            context.fill_rect(action_rect, action_background);
            context.draw_rect(action_rect, action_border);
            if let Some(label) = &self.action_label {
                if !label.is_empty() {
                    context.draw_text_fitted(
                        Rect::new(
                            action_rect.x + 4,
                            action_rect.y + (action_rect.height as i32 - message_h as i32) / 2,
                            action_rect.width.saturating_sub(8),
                            message_h,
                        ),
                        label,
                        &message_font,
                        action_text,
                        HorizontalAlignment::Center,
                    );
                }
            }
        }

        if let Some(progress) = self.progress {
            // The progress rule sits on the bar's own bottom edge, not on a literal 3 px above
            // a bar whose position moved with the canvas.
            let progress_bar = Rect::new(
                bar.x,
                bar.y + bar.height as i32 - SNACKBAR_PROGRESS_HEIGHT as i32,
                bar.width,
                SNACKBAR_PROGRESS_HEIGHT,
            );
            context.fill_rect(progress_bar, track);
            let fill = (progress_bar.width as f32 * progress).round() as u32;
            if fill > 0 {
                context.fill_rect(
                    Rect::new(progress_bar.x, progress_bar.y, fill, progress_bar.height),
                    progress_fill,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn show_and_dismiss_change_visibility() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        assert!(!bar.is_visible());

        bar.show("Saved successfully");
        assert!(bar.is_visible());
        assert_eq!(bar.message(), "Saved successfully");

        bar.dismiss();
        assert!(!bar.is_visible());
    }

    #[test]
    fn enter_key_triggers_action_signal() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Retry deployment", "Retry");

        let actions = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = actions.clone();
        bar.action_triggered.connect(move |label| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(label.as_ref().clone());
            }
        });

        bar.handle_event(&Event::key_press(13, 0));

        let got = actions.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["Retry".to_string()]);
    }

    #[test]
    fn esc_key_dismisses() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Sync pending", "Open");
        assert!(bar.is_visible());

        bar.handle_event(&Event::key_press(27, 0));
        assert!(!bar.is_visible());
    }

    /// The bar is one floating row whatever height the control was given.
    ///
    /// The defect this pins: the control painted a full-canvas panel and then a 26 px bar
    /// pinned 34 px above its bottom, so a 240x120 census cell was a 120 px surface with the
    /// snackbar somewhere near its lower edge. A snackbar is its bar; it does not paint the
    /// page behind it.
    #[test]
    fn the_bar_keeps_its_own_height_in_any_rectangle() {
        use crate::widget::metrics::{dimensions, ControlMetrics};
        for height in [48u32, 60, 120, 300] {
            let mut bar = Snackbar::new(Rect::new(0, 0, 320, height));
            bar.show("Saved successfully");
            let svg = crate::widget::svg::render_to_svg(&mut bar);
            // The bar is the filled rectangle the control paints. It is at most
            // `SNACKBAR_HEIGHT` tall, centred in the control, so no emitted fill rectangle may
            // span the control's own full height (which is what a panel fill would do).
            let expected = ControlMetrics::full_width_band(
                Rect::new(0, 0, 320, height),
                dimensions::SNACKBAR_HEIGHT,
            );
            let fill = format!(
                "x=\"0\" y=\"{}\" width=\"320\" height=\"{}\"",
                expected.y, expected.height
            );
            assert!(
                svg.contains(&fill),
                "at control height {height} the bar must be {expected:?}, in:\n{svg}"
            );
        }
    }

    /// The action button sits on the bar, not below it.
    #[test]
    fn the_action_button_sits_on_the_bar() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 320, 120));
        bar.show_with_action("Retry deployment", "Retry");
        let action = bar.action_rect().expect("an action button");
        let bar_band = crate::widget::metrics::ControlMetrics::full_width_band(
            Rect::new(0, 0, 320, 120),
            crate::widget::metrics::dimensions::SNACKBAR_HEIGHT,
        );
        assert!(action.y >= bar_band.y, "the button starts on the bar");
        assert!(
            action.y + action.height as i32 <= bar_band.y + bar_band.height as i32,
            "the button ends on the bar"
        );
    }
}
