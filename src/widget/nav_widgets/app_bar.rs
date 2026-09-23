// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AppBar (Top Bar) widget — a mobile-style top navigation bar with title,
//! optional back button, and optional action text.
//!
//! The AppBar is a Material Design-inspired top app bar that displays a title
//! centered in the bar, an optional back arrow, and optional action text. It
//! emits `back_pressed` when the back area is tapped and `action_pressed` when
//! the action area is tapped.
//!
//! # Which side the affordances sit on
//!
//! The two text affordances are placed by *reading order*, not by geometry: the back arrow is the
//! bar's **leading** affordance and the action is its **trailing** one, so in a right-to-left
//! interface the arrow belongs at the right edge and the action at the left.
//! [`AppBar::direction`] is the one place that is decided, and both the drawing and the tap zones
//! read it — a bar whose arrow moved without its hit test moving would look right and be unusable.
//!
//! The title's *centring* is deliberately not affected: a centred box has the same left edge in
//! either direction, which is why the defect survived review on the middle of the bar.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, TextDirection};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::{
    expect_string, expect_text_direction, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The width of the bar's leading zone, which holds the back arrow.
const APP_BAR_LEADING_ZONE: i32 = 48;

/// The width of the bar's trailing zone, which holds the action.
const APP_BAR_TRAILING_ZONE: i32 = 80;

/// Space kept between the bar's leading edge and the back arrow's glyph.
const APP_BAR_ARROW_INSET: i32 = 12;

/// Space reserved either side of the title so it does not collide with the affordances.
///
/// These are the *reservations* the centring arithmetic uses, and they are narrower than the tap
/// zones on purpose: a 48 px zone holding a 12 px glyph would push the title a third of the bar
/// inward. The pair is applied through [`TextDirection`] like everything else on the horizontal axis.
const APP_BAR_TITLE_LEADING_RESERVE: i32 = 40;
const APP_BAR_TITLE_TRAILING_RESERVE: i32 = 80;

/// The title's reserve when there is nothing on that side to clear.
const APP_BAR_TITLE_BARE_RESERVE: i32 = 16;

/// Space kept between the action's glyph and the bar's trailing edge.
const APP_BAR_ACTION_INSET: i32 = 16;

/// AppBar / Top Bar widget — mobile-style top navigation bar.
///
/// Displays a title centered in the bar, an optional back arrow in the bar's
/// *leading* zone, and optional action text in its *trailing* zone. Emits signals
/// when the back or action areas are pressed.
pub struct AppBar {
    base: BaseWidget,
    /// The title text centered in the bar.
    title: String,
    /// Whether to show the back arrow in the bar's leading zone.
    show_back: bool,
    /// Optional action text displayed in the bar's trailing zone.
    action_text: String,
    /// The writing direction the bar's two affordances are placed by.
    ///
    /// # Why this is not a cosmetic setting
    ///
    /// The back arrow and the action are the bar's *leading* and *trailing* affordances. Placing them
    /// at fixed screen edges was not a translation problem but a wrong one: in an Arabic or Hebrew
    /// interface the back arrow belongs where the reader starts, and an action that sits on the
    /// wrong side reads as a different control. Both the drawing and the tap zones are derived from
    /// this field, so the picture and the hit test cannot drift apart.
    ///
    /// Defaults to left-to-right, so a bar that never asks behaves exactly as it did.
    direction: TextDirection,
    /// Emitted when the back arrow area is pressed.
    pub back_pressed: GenericSignal,
    /// Emitted when the action text area is pressed.
    pub action_pressed: GenericSignal,
}

impl AppBar {
    /// Creates a new AppBar widget with the given title and geometry.
    ///
    /// By default, the back arrow is hidden and the action text is empty.
    pub fn new(title: &str, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AppBar, geometry, "AppBar"),
            title: title.to_string(),
            show_back: false,
            action_text: String::new(),
            direction: TextDirection::default(),
            back_pressed: GenericSignal::new(),
            action_pressed: GenericSignal::new(),
        }
    }

    /// Sets the title text displayed centered in the bar.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the current title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets whether the back arrow is shown in the bar's leading zone.
    pub fn set_show_back(&mut self, show_back: bool) {
        self.show_back = show_back;
        self.base.request_redraw();
    }

    /// Returns whether the back arrow is currently shown.
    pub fn show_back(&self) -> bool {
        self.show_back
    }

    /// Sets the action text displayed in the bar's trailing zone.
    ///
    /// Pass an empty string to hide the action area.
    pub fn set_action_text(&mut self, text: &str) {
        self.action_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Returns the writing direction the bar's affordances are placed by.
    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    /// Sets the writing direction, and repaints.
    ///
    /// A right-to-left bar puts its back arrow at the **right** edge and its action at the left — the
    /// mirror of the left-to-right arrangement, applied to both the drawing and the tap zones. See the
    /// field for why the two must move together.
    pub fn set_direction(&mut self, direction: TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }

    /// The leading zone: where the back arrow is drawn and hit-tested.
    ///
    /// # Why this is a method rather than two inline expressions
    ///
    /// `draw` and `handle_event` both need the same answer to "which side is the leading edge".
    /// Deriving it twice is how a mirrored bar ends up with its arrow on one side and its hit test on
    /// the other — a defect that looks correct in a screenshot. One derivation, two consumers.
    fn leading_zone(&self) -> Rect {
        let rect = self.geometry();
        let x = if self.direction.is_right_to_left() {
            rect.right() - APP_BAR_LEADING_ZONE
        } else {
            rect.x
        };
        Rect::new(x, rect.y, APP_BAR_LEADING_ZONE as u32, rect.height)
    }

    /// The trailing zone: where the action is drawn and hit-tested.
    fn trailing_zone(&self) -> Rect {
        let rect = self.geometry();
        let x = if self.direction.is_right_to_left() {
            rect.x
        } else {
            rect.right() - APP_BAR_TRAILING_ZONE
        };
        Rect::new(x, rect.y, APP_BAR_TRAILING_ZONE as u32, rect.height)
    }
}

impl Widget for AppBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 56)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `AppBar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for AppBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(&expect_string(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "direction", BASE_PROPERTY_NAMES]
    }
}

impl Draw for AppBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let bar_height = rect.height;

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because every colour below
        // was previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style`, so none is held across the draw or across another
        // accessor — the global manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("app_bar");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(if is_enabled {
                Color::rgba(248, 248, 250, 255)
            } else {
                Color::DISABLED_BACKGROUND
            });
        let border_color = if is_enabled {
            style
                .border_color
                .or_else(|| theme.as_ref().and_then(|t| t.border_color))
                .unwrap_or(Color::DIVIDER)
        } else {
            Color::DISABLED_FOREGROUND
        };
        // The bar's ink follows the same resolution: a title, a back arrow and an
        // action all read the control's resolved text colour rather than a literal.
        let text_color = if is_enabled {
            style
                .text_color
                .or_else(|| theme.as_ref().and_then(|t| t.text_color))
                .unwrap_or(Color::FOREGROUND)
        } else {
            Color::DISABLED_FOREGROUND
        };

        // Draw background
        context.fill_rect(rect, background);

        // Draw bottom border line
        let border_y = rect.y + bar_height as i32 - 1;
        context.draw_line_stroke(
            Point::new(rect.x, border_y),
            Point::new(rect.x + rect.width as i32, border_y),
            border_color,
            1,
        );

        // Determine font sizes based on bar height, **scaled by the device's text-size preference**.
        //
        // The upper clamp of 22 was a silent ceiling on text scaling: `2x` text on a standard
        // 56 px bar asks for 42 px, the clamp returned 22, and the control looked unchanged — an
        // accessibility setting that appears to do nothing is worse than one that is not offered.
        // The ceiling is now raised in proportion to the requested scale, so the control follows
        // the preference it is given while still not letting a 3x preference produce a title taller
        // than its own bar. `bar_height` itself grows under a scaled layout, which is what makes the
        // raised ceiling reachable rather than merely permitted.
        let text_scale = crate::platform::profile::text_scale();
        let title_font_size =
            (bar_height as f32 * 0.38 * text_scale).clamp(14.0, 22.0 * text_scale);
        let action_font_size =
            (bar_height as f32 * 0.32 * text_scale).clamp(12.0, 18.0 * text_scale);

        // ── Back arrow (the bar's leading zone) ──
        if self.show_back {
            let back_font = Font::new("sans-serif", action_font_size + 2.0, false, false);
            let back_text = "←";
            let metrics = context.measure_text(back_text, &back_font);
            // The arrow sits `APP_BAR_ARROW_INSET` from the bar's *leading* edge, whichever edge that
            // is. Vertically the glyph origin is the box's top edge, so centring is half the
            // difference of the line boxes — the `bar/2 + ascent/2 - descent/2` form that used to be
            // here began the glyph box roughly a third of a line below the bar's middle.
            let back_x = if self.direction.is_right_to_left() {
                rect.right() - APP_BAR_ARROW_INSET - metrics.width as i32
            } else {
                rect.x + APP_BAR_ARROW_INSET
            };
            let back_y = rect.y + (bar_height as i32 - metrics.height as i32) / 2;
            context.draw_text(
                Point::new(back_x, back_y),
                back_text,
                &back_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Centered title ──
        //
        // The two reserves are named for the *role* they clear (leading / trailing) and mapped
        // through the direction together with everything else, so an RTL bar reserves the same total
        // width on the same sides its affordances actually occupy. A centered box is unaffected by the
        // direction, which is why an unmirrored title on an unmirrored bar went unnoticed.
        if !self.title.is_empty() {
            let title_font = Font::new("sans-serif", title_font_size, false, false);
            let metrics = context.measure_text(&self.title, &title_font);

            let (leading_reserve, trailing_reserve) = (
                if self.show_back {
                    APP_BAR_TITLE_LEADING_RESERVE
                } else {
                    APP_BAR_TITLE_BARE_RESERVE
                },
                if self.action_text.is_empty() {
                    APP_BAR_TITLE_BARE_RESERVE
                } else {
                    APP_BAR_TITLE_TRAILING_RESERVE
                },
            );
            let title_width = metrics.width as i32;
            // Overflow falls back to "left-align inside the leading reserve", which in RTL means the
            // title grows from its right edge — the same rule read in the other direction.
            let overflow_x = if self.direction.is_right_to_left() {
                rect.right() - leading_reserve - title_width
            } else {
                rect.x + leading_reserve
            };
            let available_width = rect.width as i32 - leading_reserve - trailing_reserve;
            let title_x = if title_width > available_width {
                overflow_x
            } else {
                rect.x + (rect.width as i32 / 2) - (title_width / 2)
            };
            let title_y = rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            context.draw_text(
                Point::new(title_x, title_y),
                &self.title,
                &title_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Action text (the bar's trailing zone) ──
        if !self.action_text.is_empty() {
            let action_font = Font::new("sans-serif", action_font_size, false, false);
            let metrics = context.measure_text(&self.action_text, &action_font);

            let action_x = if self.direction.is_right_to_left() {
                rect.x + APP_BAR_ACTION_INSET
            } else {
                rect.right() - metrics.width as i32 - APP_BAR_ACTION_INSET
            };
            let action_y = rect.y + (bar_height as i32 - metrics.height as i32) / 2;

            // The action is the bar's one accented affordance: when the app bar is
            // enabled it is tinted toward the resolved ink so it reads as an action
            // against the bar's fill, and it moves with the appearance like the rest
            // of the chrome.
            let action_color = if is_enabled {
                text_color.blend(&background, 0.25)
            } else {
                Color::DISABLED_FOREGROUND
            };
            context.draw_text(
                Point::new(action_x, action_y),
                &self.action_text,
                &action_font,
                action_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for AppBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }

                // The zones come from the same accessors the drawing uses, so a mirrored bar's arrow
                // is tappable where it is *drawn*. Deriving them twice is how "looks right, is
                // unusable" happens.
                //
                // Tap zones use the crate's **exclusive** far edge convention:
                // `Rect::contains_point` uses `x < x + width`, so a hand-written `<=`
                // made the boundary column belong to both zones. The leading zone is
                // tested first, so testing it as a rect is what keeps the middle zone's
                // first column out of it.
                if self.show_back && self.leading_zone().contains_point(*pos) {
                    self.back_pressed.emit();
                    self.base.request_redraw();
                    return;
                }

                if !self.action_text.is_empty() && self.trailing_zone().contains_point(*pos) {
                    self.action_pressed.emit();
                    self.base.request_redraw();
                    return;
                }

                // If neither back nor action matched, treat as back when show_back,
                // otherwise as general click on the bar.
                if self.show_back {
                    self.back_pressed.emit();
                    self.base.request_redraw();
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_app_bar() -> AppBar {
        AppBar::new("Home", Rect::new(0, 0, 375, 56))
    }

    #[test]
    fn app_bar_default_creation() {
        let bar = make_app_bar();
        assert_eq!(bar.kind(), WidgetKind::AppBar);
        assert_eq!(bar.title(), "Home");
        assert!(!bar.show_back());
        assert_eq!(bar.action_text(), "");
        assert!(bar.is_visible());
        assert!(bar.is_enabled());
        assert_eq!(bar.geometry(), Rect::new(0, 0, 375, 56));
    }

    #[test]
    fn app_bar_title_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.title(), "Home");

        bar.set_title("Settings");
        assert_eq!(bar.title(), "Settings");

        bar.set_title("");
        assert_eq!(bar.title(), "");
    }

    #[test]
    fn app_bar_show_back_accessors() {
        let mut bar = make_app_bar();
        assert!(!bar.show_back());

        bar.set_show_back(true);
        assert!(bar.show_back());

        bar.set_show_back(false);
        assert!(!bar.show_back());
    }

    #[test]
    fn app_bar_action_text_accessors() {
        let mut bar = make_app_bar();
        assert_eq!(bar.action_text(), "");

        bar.set_action_text("Save");
        assert_eq!(bar.action_text(), "Save");

        bar.set_action_text("");
        assert_eq!(bar.action_text(), "");
    }

    #[test]
    fn app_bar_back_pressed_signal_emits_on_left_tap() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in left zone (first 48px)
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_back_pressed_emits_on_center_tap_when_back_shown() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in center — falls through to back when show_back is true
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_signal_emits_on_right_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.action_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Tap in right action zone (last 80px)
        bar.handle_event(&Event::MousePress { pos: Point::new(340, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_not_emitted_on_center_tap() {
        let mut bar = make_app_bar();
        bar.set_action_text("Save");
        bar.set_show_back(true);

        let action_fired = Arc::new(AtomicBool::new(false));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.store(true, Ordering::SeqCst);
        });

        // Tap in center — action should NOT fire
        bar.handle_event(&Event::MousePress { pos: Point::new(188, 28), button: 1 });
        assert!(!action_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_disabled_blocks_events() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_enabled(false);
        bar.set_action_text("Save");

        let back_fired = Arc::new(AtomicBool::new(false));
        let b = back_fired.clone();
        bar.back_pressed.connect(move || {
            b.store(true, Ordering::SeqCst);
        });

        let action_fired = Arc::new(AtomicBool::new(false));
        let a = action_fired.clone();
        bar.action_pressed.connect(move || {
            a.store(true, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 1 });
        assert!(!back_fired.load(Ordering::SeqCst));
        assert!(!action_fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_svg_output() {
        let mut bar = make_app_bar();
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_svg_with_back_and_action() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);
        bar.set_action_text("Cancel");
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"375\""));
        assert!(svg.contains("height=\"56\""));
    }

    #[test]
    fn app_bar_back_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.back_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_action_pressed_signal_accessor() {
        let bar = make_app_bar();
        let signal = &bar.action_pressed;
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        signal.connect(move || {
            f.store(true, Ordering::SeqCst);
        });
        signal.emit();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_other_button_noop() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        // Right-button click should be ignored
        bar.handle_event(&Event::MousePress { pos: Point::new(10, 28), button: 2 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn app_bar_release_also_emits() {
        let mut bar = make_app_bar();
        bar.set_show_back(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move || {
            f.store(true, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MouseRelease { pos: Point::new(10, 28), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    /// The back arrow is the bar's **leading** affordance, so a right-to-left bar draws it at the
    /// right edge and one drawn at the left would contradict the reader's own order.
    ///
    /// # The defect this pins
    ///
    /// Both affordances were placed at fixed *screen* edges — the arrow always at `rect.x`, the action
    /// always at `rect.right()`. In an Arabic or Hebrew interface that puts the way *forward* where the
    /// reader looks last, and there was no way to say otherwise.
    ///
    /// # How the ink is located
    ///
    /// The SVG backend draws text as glyph **rectangles**, not `<text>` elements (a `<text>` would be
    /// rendered by the viewer's own font rather than this crate's), so the assertion reads the x of the
    /// leftmost and rightmost glyph rect the bar emitted. Giving each affordance its own render keeps
    /// the two apart without inventing a text layout.
    #[test]
    fn a_right_to_left_bar_mirrors_both_affordances() {
        fn glyph_xs(svg: &str) -> (i32, i32) {
            let mut min = i32::MAX;
            let mut max = i32::MIN;
            for command in svg.split("M").skip(1) {
                let x = command
                    .split_whitespace()
                    .next()
                    .and_then(|token| token.parse::<f32>().ok())
                    .map(|v| v as i32);
                if let Some(x) = x {
                    min = min.min(x);
                    max = max.max(x);
                }
            }
            assert!(min <= max, "the bar painted no ink at all");
            (min, max)
        }

        // The title is useless for this comparison because it is centred and must not move, so each
        // case is rendered with one affordance only.
        fn only_back(direction: TextDirection) -> String {
            let mut bar = AppBar::new("", Rect::new(0, 0, 400, 56));
            bar.set_show_back(true);
            bar.set_direction(direction);
            render_to_svg(&mut bar)
        }
        fn only_action(direction: TextDirection) -> String {
            let mut bar = AppBar::new("", Rect::new(0, 0, 400, 56));
            bar.set_action_text("Save");
            bar.set_direction(direction);
            render_to_svg(&mut bar)
        }

        let (ltr_back_left, _) = glyph_xs(&only_back(TextDirection::LeftToRight));
        let (rtl_back_left, _) = glyph_xs(&only_back(TextDirection::RightToLeft));
        assert_eq!(ltr_back_left, APP_BAR_ARROW_INSET, "LTR draws the arrow at the left inset");
        assert!(
            rtl_back_left > 400 / 2,
            "RTL must draw the arrow on the right half, drew its ink at {rtl_back_left}"
        );

        let (_, ltr_action_right) = glyph_xs(&only_action(TextDirection::LeftToRight));
        let (rtl_action_left, _) = glyph_xs(&only_action(TextDirection::RightToLeft));
        assert!(
            ltr_action_right > 400 - APP_BAR_ACTION_INSET - 40,
            "LTR must draw the action against the right edge, its ink ended at {ltr_action_right}"
        );
        assert_eq!(rtl_action_left, APP_BAR_ACTION_INSET, "RTL draws the action at the left inset");
    }

    /// The tap zones are the same derivation the drawing uses, so they move with it.
    #[test]
    fn the_leading_and_trailing_zones_follow_the_direction() {
        let mut bar = AppBar::new("Home", Rect::new(0, 0, 400, 56));
        bar.set_show_back(true);
        bar.set_action_text("Save");
        assert_eq!(bar.leading_zone().x, 0);
        assert_eq!(bar.trailing_zone().x, 400 - APP_BAR_TRAILING_ZONE);

        bar.set_direction(TextDirection::RightToLeft);
        assert_eq!(bar.leading_zone().right(), 400);
        assert_eq!(bar.leading_zone().width, APP_BAR_LEADING_ZONE as u32);
        assert_eq!(bar.trailing_zone().x, 0);
    }

    /// The picture and the hit test must move together.
    ///
    /// A mirrored arrow that still answers only at the old edge is the worst of both: the bar reads
    /// correctly and nothing happens when the user taps what they see. This drives real events at the
    /// mirrored position rather than reading the zone accessor, so a `draw`/`handle_event` divergence
    /// fails here.
    #[test]
    fn a_mirrored_bars_tap_zones_follow_its_arrow() {
        let mut bar = AppBar::new("Home", Rect::new(0, 0, 400, 56));
        bar.set_show_back(true);
        bar.set_action_text("Save");
        bar.set_direction(TextDirection::RightToLeft);

        let back = Arc::new(AtomicBool::new(false));
        let b = back.clone();
        bar.back_pressed.connect(move || {
            b.store(true, Ordering::SeqCst);
        });
        let action = Arc::new(AtomicBool::new(false));
        let a = action.clone();
        bar.action_pressed.connect(move || {
            a.store(true, Ordering::SeqCst);
        });

        // Right edge: the arrow. Its zone is 48 px wide, so 380 is inside it.
        bar.handle_event(&Event::MousePress { pos: Point::new(380, 28), button: 1 });
        assert!(back.load(Ordering::SeqCst), "tapping the drawn arrow must press back");
        assert!(!action.load(Ordering::SeqCst), "and must not press the action");

        // Left edge: the action, in an 80 px zone.
        back.store(false, Ordering::SeqCst);
        bar.handle_event(&Event::MousePress { pos: Point::new(20, 28), button: 1 });
        assert!(action.load(Ordering::SeqCst), "tapping the drawn action must fire the action");
        assert!(!back.load(Ordering::SeqCst), "and must not press back");
    }

    /// The default must remain byte-identical so a bar that never asks behaves exactly as it did.
    #[test]
    fn the_default_bar_is_still_left_to_right() {
        let mut untouched = make_app_bar();
        untouched.set_show_back(true);
        untouched.set_action_text("Save");
        let mut ltr = make_app_bar();
        ltr.set_show_back(true);
        ltr.set_action_text("Save");
        ltr.set_direction(TextDirection::LeftToRight);
        assert_eq!(render_to_svg(&mut untouched), render_to_svg(&mut ltr));
        assert_eq!(
            AppBar::new("x", Rect::new(0, 0, 10, 10)).direction(),
            TextDirection::LeftToRight
        );
    }

    /// The direction is reachable from the property API with a matching read-back token.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut bar = make_app_bar();
        assert_eq!(bar.get("direction").unwrap().as_str(), Some("ltr"));
        bar.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
        assert_eq!(bar.direction(), TextDirection::RightToLeft);
        assert_eq!(bar.get("direction").unwrap().as_str(), Some("rtl"));
    }
}
