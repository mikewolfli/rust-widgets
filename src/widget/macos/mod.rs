// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! macOS (desktop) widget collection.
//!
//! # Why this sits beside `cupertino` instead of inside it
//!
//! `cupertino` styles the **touch** platform: pill-shaped tracks, iOS colours and 51x31
//! switch geometry. A desktop application needs different controls and different metrics —
//! a segmented *picker* instead of a pill switcher, a stepper with two half-height zones, a
//! toolbar strip, a sidebar source list and a window title bar. Folding those into the touch
//! module would have left one module describing two platforms and one name
//! (`cupertino_segmented_control`) claiming two metric sets.
//!
//! This module is therefore the desktop sibling: it owns the controls a desktop surface needs
//! and draws all of them with [`ControlMetricsProfile::Macos`]. It shares the crate's existing
//! machinery — [`BaseWidget`] for state, `Draw` for painting, [`FocusRing`] for keyboard focus
//! and the theme palette for colours — rather than introducing a second styling subsystem.
//!
//! # Sub-modules
//!
//! - [`segmented_control`] — `MacSegmentedControl`, a multi-way picker on a unified track
//! - [`source_list`] — `MacSourceList`, a sectioned sidebar with single-row selection
//! - [`stepper`] — `MacStepper`, an increment/decrement control with an optional wrap
//! - [`toolbar`] — `MacToolbar`, a horizontal strip of item slots
//! - [`window_chrome`] — `MacWindowChrome`, a title bar with traffic-light controls
//!
//! # Metrics
//!
//! Every dimension below is the desktop platform's own, expressed once here so the five
//! controls cannot drift from each other. The profile table in
//! [`crate::widget::metrics::profile_metrics`] supplies the tokens the crate already models
//! (a button's corner radius, a push button's height); the constants below cover the shapes
//! that table does not name yet.

use crate::core::{Color, Rect};
use crate::render::RenderContext;
use crate::widget::metrics::{FocusRing, FOCUS_RING_WIDTH};
use crate::widget::BaseWidget;

pub mod segmented_control;
pub mod source_list;
pub mod stepper;
pub mod toolbar;
pub mod window_chrome;

pub use segmented_control::MacSegmentedControl;
pub use source_list::{MacSourceList, SourceListRow, SourceListSection};
pub use stepper::MacStepper;
pub use toolbar::{MacToolbar, MacToolbarItem};
pub use window_chrome::{MacWindowChrome, WindowChromeButton};

/// A segmented picker's height: one row of 24-point segments.
pub const MAC_SEGMENTED_CONTROL_HEIGHT: u32 = 24;

/// A segmented picker's width when nothing constrains it.
pub const MAC_SEGMENTED_DEFAULT_WIDTH: u32 = 240;

/// Horizontal inset of a segment's label from the segment's own edge: 10.
pub const MAC_SEGMENT_LABEL_INSET: i32 = 10;

/// A stepper's width: two stacked half-height zones, 19 points wide.
pub const MAC_STEPPER_WIDTH: u32 = 19;

/// A stepper's natural height: the desktop profile's push-button floor (28 points).
///
/// A function rather than a constant because it *is* the profile's token: retuning the desktop
/// push-button height must move the stepper with it, and a copy here could not follow.
pub fn mac_stepper_height() -> u32 {
    crate::widget::metrics::profile_metrics(crate::widget::metrics::ControlMetricsProfile::Macos)
        .button_min
        .height
}

/// A toolbar strip's height.
pub const MAC_TOOLBAR_HEIGHT: u32 = 38;

/// A toolbar's natural width when nothing constrains it.
pub const MAC_TOOLBAR_DEFAULT_WIDTH: u32 = 320;

/// Horizontal inset of the first toolbar slot from the strip's leading edge.
pub const MAC_TOOLBAR_INSET: i32 = 8;

/// Horizontal padding inside one toolbar slot, on each side of its label.
pub const MAC_TOOLBAR_ITEM_PADDING: i32 = 8;

/// A source list row's height.
pub const MAC_SOURCE_LIST_ROW_HEIGHT: u32 = 24;

/// A source list section header's height.
pub const MAC_SOURCE_LIST_SECTION_HEIGHT: u32 = 22;

/// A source list's natural width.
pub const MAC_SOURCE_LIST_DEFAULT_WIDTH: u32 = 220;

/// A source list's natural height.
pub const MAC_SOURCE_LIST_DEFAULT_HEIGHT: u32 = 240;

/// A title bar's height.
pub const MAC_TITLE_BAR_HEIGHT: u32 = 28;

/// Leading inset of the first traffic-light control from the bar's edge.
pub const MAC_TRAFFIC_LIGHT_INSET: i32 = 12;

/// Gap between two traffic-light controls.
pub const MAC_TRAFFIC_LIGHT_GAP: i32 = 8;

/// The colours a macOS control draws with.
///
/// Resolved once from the active theme so the five controls agree with each other and with a
/// light/dark switch. The literal fallback is the desktop platform's own light palette, used
/// only when no theme is installed — the same arrangement the crate's other controls use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MacPalette {
    /// The face of a control that sits on the window background.
    pub(crate) surface: Color,
    /// One step above [`Self::surface`], for a raised element such as a title bar.
    pub(crate) raised: Color,
    /// The default ink colour.
    pub(crate) ink: Color,
    /// The accent colour, used for selection and focus.
    pub(crate) accent: Color,
    /// Hairline separators between adjacent regions.
    pub(crate) separator: Color,
    /// De-emphasised ink for secondary text.
    pub(crate) muted: Color,
    /// The three status colours a window's traffic lights take.
    pub(crate) close: Color,
    /// The minimise control's fill.
    pub(crate) minimize: Color,
    /// The zoom control's fill.
    pub(crate) zoom: Color,
}

/// The palette the macOS controls draw with.
///
/// # Why the theme is read here rather than at each draw site
///
/// Five controls that each resolved their own colours could disagree about what "surface"
/// means — the exact drift the crate's rendering census reports as a theme-blind control. One
/// resolver keeps them in step, and it takes the manager lock and releases it inside this
/// function, so no caller holds the lock across a draw.
pub(crate) fn palette() -> MacPalette {
    let manager = crate::style::theme_manager();
    match manager.current_theme() {
        Some(theme) => MacPalette {
            surface: theme.colors.surface_container,
            raised: theme.colors.surface_container_high,
            ink: theme.colors.foreground,
            accent: theme.colors.primary,
            separator: theme.colors.outline,
            muted: theme.colors.secondary,
            close: theme.colors.error,
            minimize: theme.colors.warning,
            zoom: theme.colors.success,
        },
        None => MacPalette {
            surface: Color::rgb(236, 236, 238),
            raised: Color::rgb(246, 246, 248),
            ink: Color::rgb(29, 29, 31),
            accent: Color::rgb(0, 122, 255),
            separator: Color::rgb(209, 209, 214),
            muted: Color::rgb(142, 142, 147),
            close: Color::rgb(255, 95, 87),
            minimize: Color::rgb(255, 189, 46),
            zoom: Color::rgb(40, 200, 64),
        },
    }
}

/// The horizontally full, vertically centred band `height` tall inside `rect`.
///
/// A control's chrome is a row, not its whole allocation: a segmented picker given a 240x120
/// cell paints a 240x24 track in the middle of it. Every control in this module derives its
/// painted band through this one function, so the paint and the hit test cannot disagree.
pub(crate) fn centred_band(rect: Rect, height: u32) -> Rect {
    let height = height.min(rect.height);
    let top = rect.y + ((rect.height - height) / 2) as i32;
    Rect::new(rect.x, top, rect.width, height)
}

/// A rectangle of `size`, clamped to `rect` and centred inside it.
///
/// Used by the fixed-size controls (a stepper is 19x28 whatever it is given), so a small
/// allocation shrinks the control rather than letting it paint outside its own box.
pub(crate) fn centred_size(rect: Rect, width: u32, height: u32) -> Rect {
    let width = width.min(rect.width);
    let height = height.min(rect.height);
    Rect::new(
        rect.x + ((rect.width - width) / 2) as i32,
        rect.y + ((rect.height - height) / 2) as i32,
        width,
        height,
    )
}

/// Paints the inset keyboard-focus ring for `rect`, when the control should show one.
///
/// The ring is drawn **inside** the control's own rectangle, through the shared
/// [`FocusRing`] geometry, so a focused control stays within its allocation. Pointer focus
/// never draws it: [`BaseWidget::draws_focus_ring`] answers `false` for that reason, which
/// keeps a click from dressing a control up as a keyboard target.
pub(crate) fn draw_focus_ring(
    context: &mut RenderContext,
    rect: Rect,
    radius: u32,
    base: &BaseWidget,
    ring_color: Color,
) {
    if !base.draws_focus_ring() {
        return;
    }
    let ring = FocusRing::for_control(rect, radius);
    if ring.is_drawable() {
        context.draw_rounded_rect_stroke(ring.rect, ring.radius, ring_color, FOCUS_RING_WIDTH);
    }
}

/// Test-only helpers shared by this module's control tests.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::core::{Color, Font, Size};
    use crate::render::{
        PaintBackend, RenderCommand, ShapedText, SoftwarePaintBackend, SoftwareRenderConfig,
        TextMetrics,
    };

    /// A backend that records every command in draw order.
    ///
    /// Records rather than compares pixels, so a test can assert *what* a control painted and in
    /// which order without a golden image. The software backend is kept behind it so command
    /// semantics (text shaping, clipping) are the real ones.
    pub(crate) struct Recorder {
        /// Every command the control issued, in order.
        pub(crate) commands: Vec<RenderCommand>,
        inner: SoftwarePaintBackend,
    }

    impl Recorder {
        /// Creates a recorder for a surface of `width` x `height` at 1.0 DPI scale.
        pub(crate) fn new(width: u32, height: u32) -> Self {
            Self {
                commands: Vec::new(),
                inner: SoftwarePaintBackend::new(Size::new(width, height), 1.0),
            }
        }

        /// Begins a frame, clearing the surface to `clear`.
        ///
        /// An inherent method as well as the trait one, so a test module only has to import
        /// [`Recorder`] to drive a frame; requiring `PaintBackend` in scope at every call site
        /// would be a second, unrelated import per test file.
        pub(crate) fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }

        /// Ends the frame, flushing the recorded commands to the inner backend.
        pub(crate) fn end_frame(&mut self) {
            self.inner.end_frame();
        }
    }

    impl PaintBackend for Recorder {
        fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }
        fn end_frame(&mut self) {
            self.inner.end_frame();
        }
        fn execute_command(&mut self, command: &RenderCommand) {
            self.commands.push(command.clone());
            self.inner.execute_command(command);
        }
        fn size(&self) -> Size {
            self.inner.size()
        }
        fn set_size(&mut self, size: Size) {
            self.inner.set_size(size);
        }
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, scale: f32) {
            self.inner.set_dpi_scale(scale);
        }
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            self.inner.measure_text(text, font)
        }
        fn shape_text(&self, text: &str, font: &Font) -> crate::compat::Rc<ShapedText> {
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
        fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
            self.inner.apply_render_config(config);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every control in this module paints, is visible against the surface it sits on, and
    /// responds to a light/dark switch.
    ///
    /// # Why this repeats the crate-wide census assertion for five names
    ///
    /// The crate-wide gate (`tests/control_rendering_census_test.rs`) is an integration test, so
    /// it cannot be run while a neighbouring slice is mid-edit in this shared worktree. Keeping
    /// the same three judgements next to the controls means the module still proves its own
    /// rendering contract from `cargo test --lib widget::macos`.
    #[cfg(full_widgets)]
    #[test]
    fn every_control_paints_visibly_and_follows_the_theme() {
        const NAMES: [&str; 5] = [
            "mac_segmented_control",
            "mac_source_list",
            "mac_stepper",
            "mac_toolbar",
            "mac_window_chrome",
        ];

        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rows = crate::widget::census::census_all_controls();

        for name in NAMES {
            let row = rows
                .iter()
                .find(|row| row.name == name)
                .unwrap_or_else(|| panic!("{name} is not in the rendering census"));
            assert!(row.light.non_background > 0, "{name} painted nothing at the census geometry");
            assert!(
                row.visible_against_its_surface(),
                "{name} paints in the colour of the surface it sits on"
            );
            assert!(
                row.differs_between_appearances(),
                "{name} renders identically in light and dark, so its chrome is hardcoded"
            );
        }
    }

    /// The painted band is centred and never leaves the rectangle it was given.
    #[test]
    fn the_centred_band_stays_inside_its_rectangle() {
        let rect = Rect::new(10, 20, 240, 120);
        let band = centred_band(rect, MAC_SEGMENTED_CONTROL_HEIGHT);
        assert_eq!(band.width, rect.width);
        assert_eq!(band.height, MAC_SEGMENTED_CONTROL_HEIGHT);
        assert!(band.y >= rect.y);
        assert!(band.bottom() <= rect.bottom());
    }

    /// A band taller than its rectangle is clamped rather than overflowing it.
    #[test]
    fn a_band_taller_than_its_rectangle_is_clamped() {
        let rect = Rect::new(0, 0, 100, 10);
        let band = centred_band(rect, MAC_TOOLBAR_HEIGHT);
        assert_eq!(band.height, 10);
        assert_eq!(band.y, 0);
    }

    /// The centring helper keeps a fixed-size control inside a smaller allocation.
    #[test]
    fn a_fixed_size_control_is_clamped_and_centred() {
        let rect = Rect::new(0, 0, 240, 120);
        let stepper = centred_size(rect, MAC_STEPPER_WIDTH, mac_stepper_height());
        assert_eq!(stepper.width, MAC_STEPPER_WIDTH);
        assert_eq!(stepper.height, mac_stepper_height());
        assert_eq!(stepper.x, (240 - MAC_STEPPER_WIDTH as i32) / 2);

        let narrow = centred_size(Rect::new(0, 0, 10, 10), MAC_STEPPER_WIDTH, mac_stepper_height());
        assert_eq!(narrow.width, 10);
        assert_eq!(narrow.height, 10);
    }
}
