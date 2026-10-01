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
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, lerp_f32, lerp_i32, lerp_u32, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The width of a Cupertino navigation bar's leading back-control area.
///
/// The affordance is the arrow plus a short label ("Back"), so it is fixed rather than
/// proportional: a bar twice as wide does not make the back control twice as wide.
const BACK_BUTTON_WIDTH: u32 = 80;

/// The edge length of the back button's arrow icon box, in pixels.
///
/// A square sized to the label's cap height rather than the `←` glyph's 20 px text run, so the
/// affordance reads as an icon beside the text instead of a stray character within it.
const ARROW_ICON_SIZE: u32 = 18;

/// iOS-style large title navigation bar.
///
/// Renders a translucent background with an optional large title and a
/// back button. Emits `back_pressed` when the back button is clicked.
pub struct CupertinoNavigationBar {
    base: BaseWidget,
    title: String,
    large_title: bool,
    /// How far the large title is **shown**, `1.0` full and `0.0` collapsed to the compact bar.
    ///
    /// This is the *drawn* fraction, not a cache of [`Self::large_title`]: the bar's height, the
    /// title's point size and the title's x/y are all read off it, so the collapse is a movement
    /// through the intermediate sizes rather than a jump between the two ends (the defect this
    /// field replaces -- BLUE23 §A.16.4 / §A.18). The target is the logical flag, so setting the
    /// flag re-aims the driver and the tick and the "am I moving?" query cannot disagree.
    collapse: PropertyDriver,
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
            // Born at the target end, because the default is a bar that is already large. A driver
            // built at `0.0` would make a freshly constructed bar animate *away* from its own
            // state on the first frame it is drawn -- the defect `PropertyDriver::at` documents.
            collapse: PropertyDriver::at(1.0, MotionSlot::Normal),
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
    ///
    /// This is the **logical** state; the bar animates to it rather than jumping. A bar being
    /// told to collapse therefore owes frames for the duration of the movement, which is what
    /// makes iOS's signature large-title collapse continuous instead of a one-frame swap.
    ///
    /// # Why the driver is aimed here and not in `tick`
    ///
    /// The frame bus asks [`Widget::is_animating`] *before* it decides whether to advance
    /// anything, so a control that only aims its target inside `tick` reports "still" at the
    /// moment it is asked and is never ticked at all -- the movement silently never happens.
    /// Aiming here means the state change and the answer to "do I owe frames?" agree from the
    /// same instant, which is the property `CollapsiblePane::set_collapsed` establishes for its
    /// own disclosure.
    pub fn set_large_title(&mut self, enabled: bool) {
        if self.large_title == enabled {
            return;
        }
        self.large_title = enabled;
        self.collapse.set_target(if enabled { 1.0 } else { 0.0 });
        self.base.request_redraw();
    }

    /// How far the large title is shown, `1.0` full and `0.0` collapsed.
    ///
    /// This is the *drawn* fraction: the bar's height, the title's point size and its x/y are
    /// all functions of it, so a test can assert the collapse slid rather than jumped by
    /// sampling it at successive frames -- the same reading `CollapsiblePane::open_progress`
    /// gives for its own disclosure.
    pub fn collapse_progress(&self) -> f32 {
        self.collapse.value()
    }

    /// Advances the large-title collapse by `delta_ms`; `true` while it is still moving.
    ///
    /// The target is re-derived from the logical flag as well as being aimed by
    /// [`Self::set_large_title`], so a bar whose flag was set through a path that did not go
    /// through the setter still converges, and the tick and the "am I moving?" query cannot
    /// disagree about which end the flag means.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.collapse.set_target(if self.large_title { 1.0 } else { 0.0 });
        self.collapse.tick(delta_ms)
    }

    /// Whether the bar is between two collapse fractions -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.collapse.is_moving()
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
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

    // The collapse is the control's own; the trait spelling is what the frame bus reaches
    // through `&mut dyn Widget`, which is the only way the movement actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        CupertinoNavigationBar::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.collapse.is_moving()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "back_pressed" => {
                Some(EventSignalRef::mapped("back_pressed", &self.back_pressed, |_| {
                    CapabilityValue::Null
                }))
            }
            _ => None,
        }
    }
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
        // ── The bar actually painted ──
        //
        // `rect` is the area the control was *given*; a navigation bar is a strip pinned to the
        // **top** of that area, [`dimensions::NAV_BAR_HEIGHT`] tall in compact mode and
        // [`dimensions::NAV_BAR_LARGE_HEIGHT`] in large-title mode. Filling the whole rectangle
        // made a 240x120 census cell a 120 px navigation bar whose compact title sat at `y + 22`
        // — a third of the way down a bar three times its proper height — and it put the bottom
        // rule on the *canvas* edge rather than on the bar's. `top_band` is the shared
        // derivation for "a strip pinned to my top edge", and the hit test below reads the same
        // band so the back affordance and its ink cannot part company.
        //
        // The two modes are the **ends of a movement**, so the height is read off the drawn
        // fraction instead of the boolean: at `1.0` this is exactly `NAV_BAR_LARGE_HEIGHT` and at
        // `0.0` exactly `NAV_BAR_HEIGHT`, so a settled bar is byte-identical to the un-animated
        // one and only the frames in between are new.
        let collapse = self.collapse.value();
        let bar_height =
            lerp_u32(dimensions::NAV_BAR_HEIGHT, dimensions::NAV_BAR_LARGE_HEIGHT, collapse);
        let bar_rect = ControlMetrics::top_band(rect, bar_height);
        context.fill_rect(bar_rect, bar);

        // ── Bottom border line ──
        // A `Surface` role resolves no border colour, so the rule is derived one visible
        // step from the bar and a caller's explicit border still wins.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != bar)
            .unwrap_or_else(|| bar.blend(&ink, 0.20));
        let border_y = bar_rect.y + bar_rect.height as i32 - 1;
        context.draw_line(
            Point::new(bar_rect.x, border_y),
            Point::new(bar_rect.x + bar_rect.width as i32, border_y),
            border,
        );

        // ── Title ──
        //
        // The large and compact titles are two ends of one movement, so the point size and the
        // x/y are interpolated off the drawn fraction rather than switched:
        //
        //   * **size** `18 -> 34`, so the title grows rather than swapping glyphs;
        //   * **x** leading `+ 16` -> centred, which is the iOS large-title slide;
        //   * **y** the text line of whatever band the height currently is, so the title stays on
        //     the bar's middle line at every intermediate height.
        //
        // At `1.0` this is the old large-title branch and at `0.0` the old compact one, so a bar
        // settled at either end draws exactly what it drew before.
        if !self.title.is_empty() {
            let title_font =
                Font::new("sans-serif", lerp_f32(18.0, 34.0, collapse), collapse > 0.5, false);
            let metrics = context.measure_text(&self.title, &title_font);
            let leading_x = bar_rect.x + 16;
            let centred_x = bar_rect.x + (bar_rect.width as i32 - metrics.width as i32) / 2;
            let title_x = lerp_i32(centred_x, leading_x, collapse);
            // Centre the title through the shared primitive, so it sits on the bar's middle line
            // whatever height the band was clamped to; the `+ 22` this replaces was a literal for
            // the 44 px bar and landed elsewhere on any other.
            let line = context.text_line(bar_rect, &title_font);
            context.draw_text(
                Point::new(title_x, line.y),
                &self.title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // ── Back button (left side) ──
        if self.back_button_visible {
            let label_font = Font::new("sans-serif", 17.0, false, false);
            // The affordance sits on the bar, so the theme's primary is contrast-checked
            // against it rather than assumed legible.
            let action = primary.contrast_color().blend(&primary, 0.85);

            // Drawn as an icon rather than the `←` text glyph: no bundled face covers
            // U+2190, so it degraded to an 8x8 fallback bitmap and read as a blocky
            // blob. The icon is geometry, so both backends agree on the same shape.
            let arrow_x = bar_rect.x + 8;
            // On the compact bar's own middle line, shared with the title; the removed
            // `+ 22` was a literal for the 44 px bar.
            let arrow_line = context.text_line(bar_rect, &label_font);
            let icon_y = arrow_line.y + (arrow_line.height as i32 - ARROW_ICON_SIZE as i32) / 2;
            let icon_rect = Rect::new(arrow_x, icon_y, ARROW_ICON_SIZE, ARROW_ICON_SIZE);
            crate::widget::draw_icon_at(context, icon_rect, action, IconName::ArrowLeft);

            // Draw text label next to arrow
            if !self.back_button_text.is_empty() {
                let label_x = arrow_x + ARROW_ICON_SIZE as i32 + 4;
                // Reads as a small title and shares the compact bar's middle line with the
                // title and the arrow; no literal offset, which had moved it with the bar's
                // height rather than its own line.
                let label_line = context.text_line(bar_rect, &label_font);
                context.draw_text(
                    Point::new(label_x, label_line.y),
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

                // The back area is the leading end of the bar itself, so it tracks the band the
                // bar is painted in rather than a fixed 80x44 literal: the clickable region and
                // the ink the user aims at are the same rectangle by construction.
                let rect = self.geometry();
                let bar_height = if self.large_title {
                    dimensions::NAV_BAR_LARGE_HEIGHT
                } else {
                    dimensions::NAV_BAR_HEIGHT
                };
                let bar_rect = ControlMetrics::top_band(rect, bar_height);
                let back_area =
                    Rect::new(bar_rect.x, bar_rect.y, BACK_BUTTON_WIDTH, bar_rect.height);
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

    /// The bar is a top-anchored strip, and its rule sits on the bar's own bottom edge.
    ///
    /// The defect this pins: the bar filled its whole rectangle, so a 240x120 census cell was
    /// a 120 px navigation bar whose compact title sat at a literal `y + 22` — a third of the
    /// way down a bar three times its proper height — and whose bottom rule landed on the
    /// *canvas* edge rather than the bar's own.
    #[test]
    fn the_bar_is_a_top_strip_that_keeps_its_own_height() {
        use crate::widget::metrics::{dimensions, ControlMetrics};
        for height in [96u32, 120, 300] {
            let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 240, height));
            bar.set_title("Settings");
            // The large-title bar is the taller of the two modes.
            let band = ControlMetrics::top_band(
                Rect::new(0, 0, 240, height),
                dimensions::NAV_BAR_LARGE_HEIGHT,
            );
            let svg = render_to_svg(&mut bar);
            let fill = format!("x=\"0\" y=\"0\" width=\"240\" height=\"{}\"", band.height);
            assert!(
                svg.contains(&fill),
                "at control height {height} the bar must be {band:?}, in:\n{svg}"
            );
            // And the rule is on the bar, not on the canvas.
            let rule = format!(
                "x1=\"0\" y1=\"{}\" x2=\"240\" y2=\"{}\"",
                band.y + band.height as i32 - 1,
                band.y + band.height as i32 - 1
            );
            assert!(svg.contains(&rule), "the rule must sit on the bar's bottom edge:\n{svg}");
        }
    }
}
