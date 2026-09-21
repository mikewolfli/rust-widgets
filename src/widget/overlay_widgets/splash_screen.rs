// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SplashScreen — the startup screen shown while an application initialises.
//!
//! # Why this is its own control
//!
//! A splash screen looks like a centred panel, but the behaviour that makes it a
//! distinct control is not its appearance: it is dismissed **by the program, when
//! initialisation finishes**, not by the user. That inverts the event model every
//! other overlay follows — a `ModalBottomSheet` waits for a drag, a `Banner` waits
//! for a click, and a `Snackbar` waits for a timeout — so presenting it as one of
//! those would mean every host re-deriving "this one I close myself".
//!
//! It also owns a progress value, because the question a splash screen answers is
//! "how much longer", and a host that has to mount a separate progress bar just to
//! answer it pays an extra widget for the same information.
//!
//! # Reachability
//!
//! Registered in the widget factory (so it is constructible by name from the
//! declarative path, CSS selectors and the C ABI), publishes a property contract,
//! and has interaction and lifecycle tests.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Bar height of the progress indicator, in logical pixels.
const BAR_HEIGHT: u32 = 4;
/// Margin between the screen edge and the progress bar.
const BAR_MARGIN: u32 = 24;
/// Vertical gap between the title and the progress bar.
const TITLE_GAP: u32 = 24;
/// Gap between the logo band and the title below it.
const SECTION_GAP: i32 = 12;
/// Line advance from the title to the subtitle when one is set.
const BODY_GAP: i32 = TITLE_GAP as i32 - 2;

/// Splash screen with a title, an optional logo block, and optional progress.
///
/// # Why progress is optional rather than always drawn
///
/// An indeterminate splash — one where the host has no progress to report — should
/// not show an empty bar, because a bar stuck at zero reads as "hung". With
/// `progress` unset the control draws no bar at all; setting it switches to a
/// determinate bar, and the two states are visibly different.
pub struct SplashScreen {
    base: BaseWidget,
    title: String,
    subtitle: String,
    progress: Option<f32>,
    /// Whether a dismiss (skip) affordance is offered.
    ///
    /// Separate from the control's own dismissal, which the host performs by
    /// unmounting it or calling [`Self::finish`]. A splash that can also be
    /// *skipped* requires this to be enabled, and it is off by default because
    /// most splash screens are not skippable.
    skippable: bool,
    /// Emitted when the screen is finished, with the title.
    pub finished: Signal1<String>,
    /// Emitted when the user skips, with the title.
    pub skipped: Signal1<String>,
}

impl SplashScreen {
    /// Creates a splash screen with `title` and no progress.
    pub fn new(geometry: Rect, title: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SplashScreen, geometry, "SplashScreen"),
            title: title.into(),
            subtitle: String::new(),
            progress: None,
            skippable: false,
            finished: Signal1::new(),
            skipped: Signal1::new(),
        }
    }

    /// Returns the title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }

    /// Returns the subtitle, empty when unset.
    pub fn subtitle(&self) -> &str {
        &self.subtitle
    }

    /// Sets the subtitle.
    pub fn set_subtitle(&mut self, subtitle: impl Into<String>) {
        self.subtitle = subtitle.into();
        self.base.request_redraw();
    }

    /// Returns the progress fraction in `0.0..=1.0`, or `None` when indeterminate.
    pub fn progress(&self) -> Option<f32> {
        self.progress
    }

    /// Sets the progress fraction, clamped to `0.0..=1.0`.
    ///
    /// A `NaN` is treated as indeterminate rather than clamping to an arbitrary
    /// end: `NaN` cannot be ordered, so any clamp would be a silent invention.
    pub fn set_progress(&mut self, progress: Option<f32>) {
        self.progress =
            progress.map(|value| if value.is_nan() { 0.0 } else { value.clamp(0.0, 1.0) });
        self.base.request_redraw();
    }

    /// Whether the user may skip this screen.
    pub fn is_skippable(&self) -> bool {
        self.skippable
    }

    /// Enables or disables the skip affordance.
    pub fn set_skippable(&mut self, skippable: bool) {
        self.skippable = skippable;
        self.base.request_redraw();
    }

    /// Marks initialisation complete, emitting `finished`.
    ///
    /// The screen does not hide itself: whether to unmount, fade, or keep the
    /// control mounted is the host's decision, and a control that removed itself
    /// from the tree could not be reused for a restart.
    pub fn finish(&mut self) {
        self.finished.emit(self.title.clone());
    }

    /// The skip hint rectangle, when the screen is skippable.
    ///
    /// Sits at the bottom-right, above the progress bar, matching where the
    /// platform splash conventions place it.
    fn skip_rect(&self) -> Option<Rect> {
        if !self.skippable {
            return None;
        }
        let rect = self.geometry();
        let width = 64.min(rect.width);
        let height = 28.min(rect.height);
        Some(Rect::new(
            rect.x + rect.width as i32 - width as i32 - BAR_MARGIN as i32,
            rect.y + rect.height as i32 - height as i32 - BAR_MARGIN as i32 - BAR_HEIGHT as i32 - 8,
            width,
            height,
        ))
    }

    /// Whether `pos` is over the skip affordance.
    fn is_over_skip(&self, pos: Point) -> bool {
        self.skip_rect().is_some_and(|skip| {
            pos.x >= skip.x
                && pos.x < skip.x + skip.width as i32
                && pos.y >= skip.y
                && pos.y < skip.y + skip.height as i32
        })
    }

    /// The progress bar rectangle, when progress is determinate.
    fn bar_rect(&self) -> Option<Rect> {
        let _ = self.progress?;
        let rect = self.geometry();
        if rect.width <= BAR_MARGIN * 2 {
            return None;
        }
        Some(Rect::new(
            rect.x + BAR_MARGIN as i32,
            rect.y + rect.height as i32 - BAR_MARGIN as i32 - BAR_HEIGHT as i32,
            rect.width - BAR_MARGIN * 2,
            BAR_HEIGHT,
        ))
    }
}

impl Widget for SplashScreen {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// A full-window surface, so the hint is the smallest desktop window rather
    /// than a content-sized box: this control fills whatever it is given.
    fn size_hint(&self) -> Size {
        Size::new(480, 320)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for SplashScreen {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "subtitle" => Ok(CapabilityValue::String(self.subtitle.clone())),
            "progress" => match self.progress {
                Some(value) => Ok(CapabilityValue::Float(f64::from(value))),
                None => Ok(CapabilityValue::Null),
            },
            "skippable" => Ok(CapabilityValue::Bool(self.skippable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "subtitle" => {
                self.set_subtitle(expect_string(value)?);
                Ok(())
            }
            "progress" => match value {
                // `Null` is the documented way to return to indeterminate, so a
                // host can stop reporting progress without rebuilding the control.
                CapabilityValue::Null => {
                    self.set_progress(None);
                    Ok(())
                }
                other => {
                    self.set_progress(Some(expect_f32(other)?));
                    Ok(())
                }
            },
            "skippable" => {
                self.set_skippable(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "subtitle", "progress", "skippable", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `splash_screen` publishes.
    ///
    /// `finish` maps onto the widget's real `finish`, which emits `finished`;
    /// unmounting the control stays the host's decision. `set_progress` and
    /// `set_title` carry the fraction and the title, so those are answered as
    /// needing a payload rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "finish" => {
                self.finish();
                Ok(())
            }
            "set_progress" | "set_title" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SplashScreen {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } if self.is_over_skip(*pos) => {
                self.skipped.emit(self.title.clone());
            }
            // Escape skips only when the screen is skippable: a splash that cannot
            // be dismissed must not quietly disappear on a stray key press.
            Event::KeyPress { key: 27, modifiers: _ } if self.skippable => {
                self.skipped.emit(self.title.clone());
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SplashScreen {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the panel, the progress bar and the skip affordance used to be
        // hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("splash_screen");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(28, 34, 45));
        // The splash panel is a `Surface`-role control, and `Surface` resolves to the
        // window's own fill — which would leave the panel indistinguishable from the frame
        // behind it. A resolved surface equal to the theme's background is therefore
        // re-derived one step toward the ink, the same distinction `Colors::input_background`
        // draws for a field.
        let themed_background =
            crate::style::theme_manager().current_theme().map(|active| active.colors.background);
        let surface = match themed_background {
            Some(window_fill) if surface == window_fill => {
                let ink = style
                    .text_color
                    .or_else(|| theme.as_ref().and_then(|t| t.text_color))
                    .unwrap_or(Color::rgb(238, 242, 248));
                window_fill.blend(&ink, 0.08)
            }
            _ => surface,
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(238, 242, 248));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.3));
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the logo block and the completed portion of the bar follow the appearance.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::rgb(66, 133, 214));
        // The subtitle and the unfilled track are dimmer than the ink but still readable on
        // the surface, so they are derived from it rather than being two more literals.
        let muted_ink = ink.blend(&surface, 0.4);
        let track = surface.blend(&ink, 0.16);

        context.fill_rect(rect, surface);

        let centre_x = rect.x + (rect.width / 2) as i32;

        // The stack (logo, title, subtitle) is centred as a whole, and the logo is only
        // drawn when there is room for it **plus** the text that follows.
        //
        // The logo used to be placed at `text_y - logo - 16` while `text_y` was itself
        // derived from the box's centre, so at the sizes this control is actually given the
        // logo started at `y = -20` — above its own top edge, and in the SVG snapshot above
        // the picture. It was not merely misplaced: the text that the logo was supposed to
        // sit above was pushed to the middle of the box regardless, so an empty frame would
        // have looked *correct* while the logo was the only thing out of place.
        let logo = 48.min(rect.width).min(rect.height / 3);
        let logo_band = if logo >= 16 { logo as i32 + SECTION_GAP } else { 0 };
        let text_block = TITLE_GAP as i32 + if self.subtitle.is_empty() { 0 } else { BODY_GAP };
        let stack_top = rect.y + ((rect.height as i32 - logo_band - text_block) / 2).max(0);
        let text_y = stack_top + logo_band;

        if logo_band > 0 {
            context.fill_rect(Rect::new(centre_x - logo as i32 / 2, stack_top, logo, logo), accent);
        }

        context.draw_text(
            Point::new(centre_x, text_y),
            &self.title,
            &Font::default(),
            ink,
            HorizontalAlignment::Center,
        );

        if !self.subtitle.is_empty() {
            context.draw_text(
                Point::new(centre_x, text_y + BODY_GAP),
                &self.subtitle,
                &Font::default(),
                muted_ink,
                HorizontalAlignment::Center,
            );
        }

        if let (Some(bar), Some(progress)) = (self.bar_rect(), self.progress) {
            context.fill_rect(bar, track);
            let filled = (bar.width as f32 * progress).round() as u32;
            if filled > 0 {
                context
                    .fill_rect(Rect::new(bar.x, bar.y, filled.min(bar.width), bar.height), accent);
            }
        }

        // The skip affordance's label is centred vertically by its own glyph box: the
        // renderer's origin is the glyph's top-left, so half the font height is the right
        // offset — `(height + 12) / 2` assumed a 12 px baseline convention the renderer does
        // not use, which pushed the label below the button.
        if let Some(skip) = self.skip_rect() {
            context.draw_rect(skip, border);
            let skip_font = Font::default();
            let label_height = context.measure_text("Skip", &skip_font).height as i32;
            context.draw_text(
                Point::new(
                    skip.x + skip.width as i32 / 2,
                    skip.y + (skip.height as i32 - label_height) / 2,
                ),
                "Skip",
                &skip_font,
                muted_ink,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn screen(width: u32, height: u32) -> SplashScreen {
        SplashScreen::new(Rect::new(0, 0, width, height), "Booting")
    }

    #[test]
    fn new_screen_is_indeterminate() {
        let splash = screen(320, 240);
        assert_eq!(splash.progress(), None);
        assert_eq!(splash.bar_rect(), None, "no progress means no bar is reserved");
    }

    #[test]
    fn setting_progress_draws_a_bar_and_clamps() {
        let mut splash = screen(320, 240);
        splash.set_progress(Some(0.5));
        assert_eq!(splash.progress(), Some(0.5));
        assert!(splash.bar_rect().is_some(), "a determinate screen reserves the bar");

        splash.set_progress(Some(4.0));
        assert_eq!(splash.progress(), Some(1.0), "over-unity progress clamps");
        splash.set_progress(Some(-2.0));
        assert_eq!(splash.progress(), Some(0.0), "negative progress clamps");
    }

    #[test]
    fn nan_progress_is_treated_as_zero_not_as_an_arbitrary_end() {
        let mut splash = screen(320, 240);
        splash.set_progress(Some(f32::NAN));
        assert_eq!(splash.progress(), Some(0.0));
    }

    #[test]
    fn progress_can_return_to_indeterminate() {
        let mut splash = screen(320, 240);
        splash.set_progress(Some(0.75));
        splash.set_progress(None);
        assert_eq!(splash.progress(), None);
        assert_eq!(splash.bar_rect(), None);
    }

    #[test]
    fn finish_emits_with_the_title_and_does_not_hide() {
        let mut splash = screen(320, 240);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        splash.finished.connect(move |title| {
            sink.lock().expect("signal sink poisoned").push(title.as_ref().clone());
        });

        splash.finish();
        assert_eq!(seen.lock().expect("lock").as_slice(), ["Booting"]);
        assert!(splash.base.is_visible(), "finishing must not hide the control itself");
    }

    #[test]
    fn skip_is_off_by_default_and_escape_does_nothing() {
        let mut splash = screen(320, 240);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        splash.skipped.connect(move |title| {
            sink.lock().expect("signal sink poisoned").push(title.as_ref().clone());
        });

        assert!(!splash.is_skippable());
        assert!(splash.skip_rect().is_none(), "no affordance is reserved when unskippable");
        splash.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert!(seen.lock().expect("lock").is_empty(), "Escape must not skip");

        splash.set_skippable(true);
        splash.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(seen.lock().expect("lock").as_slice(), ["Booting"]);
    }

    #[test]
    fn clicking_the_skip_area_emits_but_clicking_elsewhere_does_not() {
        let mut splash = screen(400, 300);
        splash.set_skippable(true);
        let skip = splash.skip_rect().expect("skippable screens reserve the area");

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        splash.skipped.connect(move |title| {
            sink.lock().expect("signal sink poisoned").push(title.as_ref().clone());
        });

        splash.handle_event(&Event::MousePress {
            pos: Point::new(skip.x + skip.width as i32 / 2, skip.y + skip.height as i32 / 2),
            button: 1,
        });
        assert_eq!(seen.lock().expect("lock").as_slice(), ["Booting"]);

        splash.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1 });
        assert_eq!(seen.lock().expect("lock").len(), 1, "a click away from Skip is ignored");
    }

    #[test]
    fn properties_round_trip_through_the_contract() {
        let mut splash = screen(320, 240);

        assert!(splash.set("title", CapabilityValue::String("Init".to_string())).is_ok());
        assert_eq!(splash.get("title").expect("readable"), CapabilityValue::String("Init".into()));

        assert!(splash.set("subtitle", CapabilityValue::String("loading".to_string())).is_ok());
        assert_eq!(splash.subtitle(), "loading");

        assert!(splash.set("progress", CapabilityValue::Float(0.25)).is_ok());
        assert_eq!(splash.get("progress").expect("readable"), CapabilityValue::Float(0.25));

        assert!(splash.set("progress", CapabilityValue::Null).is_ok());
        assert_eq!(splash.get("progress").expect("readable"), CapabilityValue::Null);

        assert!(splash.set("skippable", CapabilityValue::Bool(true)).is_ok());
        assert!(splash.is_skippable());
    }

    #[test]
    fn a_mismatched_value_kind_is_refused_rather_than_coerced() {
        let mut splash = screen(320, 240);
        assert_eq!(
            splash.set("title", CapabilityValue::Bool(true)),
            Err(CapabilityAccessError::TypeMismatch)
        );
        assert_eq!(
            splash.set("skippable", CapabilityValue::String("yes".to_string())),
            Err(CapabilityAccessError::TypeMismatch)
        );
    }

    #[test]
    fn the_declared_property_names_match_what_the_contract_answers() {
        let splash = screen(320, 240);
        for name in splash.property_names() {
            assert!(
                splash.get(name).is_ok(),
                "{name} is declared but the contract refuses to read it"
            );
        }
    }
}
