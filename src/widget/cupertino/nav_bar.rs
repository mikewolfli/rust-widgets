// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CupertinoNavigationBar — iOS-style large title navigation bar.
//!
//! An iOS-style navigation bar with optional large title (similar to the
//! iOS 13+ large title nav bar), back button with arrow, and translucent
//! background effect.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// iOS-style large title navigation bar.
///
/// Renders a translucent background with an optional large title and a
/// back button. Emits `back_pressed` when the back button is clicked.
pub struct CupertinoNavigationBar {
    base: BaseWidget,
    title: String,
    large_title: bool,
    back_button_visible: bool,
    back_button_text: String,
    /// Emitted when the back button is pressed.
    pub back_pressed: Signal1<()>,
}

impl CupertinoNavigationBar {
    /// Creates a new CupertinoNavigationBar with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let base =
            BaseWidget::new(WidgetKind::CupertinoNavigationBar, geometry, "CupertinoNavigationBar");
        Self {
            base,
            title: String::new(),
            large_title: true,
            back_button_visible: false,
            back_button_text: "Back".to_string(),
            back_pressed: Signal1::new(),
        }
    }

    /// Sets whether the back button is visible.
    pub fn show_back_button(&mut self, visible: bool) {
        self.back_button_visible = visible;
        self.base.request_redraw();
    }

    /// Sets the title text.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns whether large title mode is enabled.
    pub fn is_large_title(&self) -> bool {
        self.large_title
    }

    /// Enables or disables large title mode.
    pub fn set_large_title(&mut self, enabled: bool) {
        self.large_title = enabled;
        self.base.request_redraw();
    }

    /// Returns whether the back button is visible.
    pub fn is_back_button_visible(&self) -> bool {
        self.back_button_visible
    }

    /// Sets the text for the back button.
    pub fn set_back_button_text(&mut self, text: &str) {
        self.back_button_text = text.to_string();
        self.base.request_redraw();
    }

    /// Returns the back button text.
    pub fn back_button_text(&self) -> &str {
        &self.back_button_text
    }
}

impl Widget for CupertinoNavigationBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 44)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoNavigationBar
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoNavigationBar`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Both properties now report
/// the bar's real state instead of the placeholder defaults that dispatch
/// returned.
impl WidgetProperties for CupertinoNavigationBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "large_title" => Ok(CapabilityValue::Bool(self.is_large_title())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(&expect_string(value)?);
                Ok(())
            }
            "large_title" => {
                self.set_large_title(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "large_title", BASE_PROPERTY_NAMES]
    }
}

impl Draw for CupertinoNavigationBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be
        // a literal, so a light/dark switch left the bar, its rule, its title and its back
        // affordance unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("cupertino_navigation_bar");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. The bar is not in the role table, so it
        // classifies as `Surface` and its resolved background is the window fill itself; the
        // bar below therefore derives its own distinct surface rather than painting the
        // window's. The back affordance is iOS blue only because it used to be hardcoded — it
        // is the bar's action colour, so it reads the theme's primary token.
        let (window_fill, foreground, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.primary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(0, 122, 255)),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // A translucent fill cannot tint anything on this surface: `fill_rect` writes raw
        // pixels, so `rgba(255, 255, 255, 230)` *replaced* the page with white at alpha 230
        // instead of frosting it, and the bar kept that colour through a theme switch. Mixing
        // the bar's own surface with the page it covers produces the same translucent
        // appearance with an opaque result that follows the appearance.
        //
        // The filter is on the **resolved** value, not only on the theme's: the active theme
        // is applied to every control before it is drawn, so `style.background_color` already
        // holds `Surface`'s window fill and letting it through unfiltered is exactly the
        // invisible-bar defect this guards against. A caller's own colour still wins.
        let bar_from_theme = window_fill.blend(&ink, 0.06);
        let bar_surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => bar_from_theme,
        };
        let bar = bar_surface.blend(&window_fill, 0.10);
        context.fill_rect(rect, bar);

        // ── Bottom border line ──
        // A `Surface` role resolves no border colour, so the rule is derived one visible
        // step from the bar and a caller's explicit border still wins.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != bar)
            .unwrap_or_else(|| bar.blend(&ink, 0.20));
        let border_y = rect.y + rect.height as i32 - 1;
        context.draw_line(
            Point::new(rect.x, border_y),
            Point::new(rect.x + rect.width as i32, border_y),
            border,
        );

        if self.large_title {
            // ── Large title ──
            let title_font = Font::new("sans-serif", 34.0, true, false);
            if !self.title.is_empty() {
                let metrics = context.measure_text(&self.title, &title_font);
                let title_x = rect.x + 16;
                let title_y = rect.y + (rect.height as i32 / 2) + (metrics.ascent as i32 / 2);
                context.draw_text(
                    Point::new(title_x, title_y),
                    &self.title,
                    &title_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        } else {
            // ── Compact title (centered in navigation bar area) ──
            let title_font = Font::new("sans-serif", 18.0, false, false);
            if !self.title.is_empty() {
                let metrics = context.measure_text(&self.title, &title_font);
                let title_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
                let title_y =
                    rect.y + 22 + (metrics.ascent as i32 / 2) - (metrics.descent as i32 / 2);
                context.draw_text(
                    Point::new(title_x, title_y),
                    &self.title,
                    &title_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Back button (left side) ──
        if self.back_button_visible {
            let arrow_font = Font::new("sans-serif", 20.0, false, false);
            let label_font = Font::new("sans-serif", 17.0, false, false);
            let arrow_symbol = "\u{2190}"; // ←
                                           // The affordance sits on the bar, so the theme's primary is contrast-checked
                                           // against it rather than assumed legible.
            let action = primary.contrast_color().blend(&primary, 0.85);

            let arrow_metrics = context.measure_text(arrow_symbol, &arrow_font);
            let arrow_x = rect.x + 8;
            let arrow_y = rect.y + 22 + (arrow_metrics.ascent as i32 / 2);

            // Draw arrow
            context.draw_text(
                Point::new(arrow_x, arrow_y),
                arrow_symbol,
                &arrow_font,
                action,
                HorizontalAlignment::Left,
            );

            // Draw text label next to arrow
            if !self.back_button_text.is_empty() {
                let label_metrics = context.measure_text(&self.back_button_text, &label_font);
                let label_x = arrow_x + arrow_metrics.width as i32 + 4;
                let label_y = rect.y + 22 + (label_metrics.ascent as i32 / 2);
                context.draw_text(
                    Point::new(label_x, label_y),
                    &self.back_button_text,
                    &label_font,
                    action,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for CupertinoNavigationBar {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseRelease { pos, button } => {
                // A disabled nav bar must not emit navigation. Without this,
                // `set_enabled(false)` had no effect: the back button still fired.
                if !self.base.is_enabled() {
                    self.base.handle_event(event);
                    return;
                }
                if *button != 1 {
                    return;
                }

                // Only handle back button area clicks
                if !self.back_button_visible {
                    return;
                }

                let rect = self.geometry();
                // Back button area: left ~80px of the nav bar, top 44px
                let back_area = Rect::new(rect.x, rect.y, 80, 44);
                if back_area.contains_point(*pos) {
                    self.back_pressed.emit(());
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

    #[test]
    fn cupertino_nav_bar_creation() {
        let bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert_eq!(bar.kind(), WidgetKind::CupertinoNavigationBar);
        assert!(bar.title().is_empty());
        assert!(bar.is_large_title());
        assert!(!bar.is_back_button_visible());
    }

    #[test]
    fn cupertino_nav_bar_title_accessors() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.set_title("Home");
        assert_eq!(bar.title(), "Home");
    }

    #[test]
    fn cupertino_nav_bar_back_button_visibility() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert!(!bar.is_back_button_visible());

        bar.show_back_button(true);
        assert!(bar.is_back_button_visible());

        bar.show_back_button(false);
        assert!(!bar.is_back_button_visible());
    }

    #[test]
    fn cupertino_nav_bar_large_title_toggle() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert!(bar.is_large_title());

        bar.set_large_title(false);
        assert!(!bar.is_large_title());

        bar.set_large_title(true);
        assert!(bar.is_large_title());
    }

    #[test]
    fn cupertino_nav_bar_back_button_text() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        assert_eq!(bar.back_button_text(), "Back");

        bar.set_back_button_text("Settings");
        assert_eq!(bar.back_button_text(), "Settings");
    }

    #[test]
    fn cupertino_nav_bar_back_pressed_signal() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.show_back_button(true);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move |_: std::sync::Arc<()>| {
            f.store(true, Ordering::SeqCst);
        });

        // Click on back button area
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 22), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
    }

    /// A disabled nav bar must not emit `back_pressed`.
    ///
    /// `handle_event` went straight to the hit test, so `set_enabled(false)` had no
    /// effect on this widget: the back affordance still fired navigation. Every
    /// interactive widget in the crate gates on `is_enabled()` first, so a missing
    /// gate turns `set_enabled` into a silent no-op rather than a policy choice.
    #[test]
    fn cupertino_nav_bar_disabled_ignores_back_click() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.show_back_button(true);
        bar.set_enabled(false);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move |_: std::sync::Arc<()>| {
            f.store(true, Ordering::SeqCst);
        });

        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 22), button: 1 });
        assert!(!fired.load(Ordering::SeqCst), "a disabled nav bar must not navigate");
    }

    #[test]
    fn cupertino_nav_bar_back_pressed_not_fired_when_hidden() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        // back_button_visible is false by default

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        bar.back_pressed.connect(move |_: std::sync::Arc<()>| {
            f.store(true, Ordering::SeqCst);
        });

        // Click where back button would be
        bar.handle_event(&Event::MouseRelease { pos: Point::new(20, 22), button: 1 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_nav_bar_svg_output() {
        let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
        bar.set_title("Settings");
        bar.show_back_button(true);
        let svg = render_to_svg(&mut bar);
        assert!(svg.starts_with("<svg"));
    }
}
