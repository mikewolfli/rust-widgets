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
//! and draws all of them with `ControlMetricsProfile::Macos`. It shares the crate's existing
//! machinery — [`BaseWidget`] for state, `Draw` for painting, `FocusRing` for keyboard focus
//! and the theme palette for colours — rather than introducing a second styling subsystem.
//!
//! # Sub-modules
//!
//! - `segmented_control` — `MacSegmentedControl`, a multi-way picker on a unified track
//! - `source_list` — `MacSourceList`, a sectioned sidebar with single-row selection
//! - `stepper` — `MacStepper`, an increment/decrement control with an optional wrap
//! - `toolbar` — `MacToolbar`, a horizontal strip of item slots
//! - `window_chrome` — `MacWindowChrome`, a title bar with traffic-light controls
//!
//! # Metrics
//!
//! Every dimension below is the desktop platform's own, expressed once here so the five
//! controls cannot drift from each other. The profile table in
//! [`crate::widget::metrics::profile_metrics`] supplies the tokens the crate already models
//! (a button's corner radius, a push button's height); the constants below cover the shapes
//! that table does not name yet.

pub mod popup_item_list;
pub mod segmented_control;
pub mod source_list;
pub mod stepper;
pub mod toolbar;
pub mod value_spinbox;
pub mod window_chrome;

pub use popup_item_list::{PopupAnchor, PopupItem, PopupItemList};
pub use segmented_control::{MacSegment, MacSegmentedControl};
pub use source_list::{MacSourceList, SourceListRow, SourceListSection};
pub use stepper::MacStepper;
pub use toolbar::{MacToolbar, MacToolbarItem, ToolbarItemDisplayMode, ToolbarItemKind};
pub use value_spinbox::ValueSpinBox;
pub use window_chrome::{MacWindowChrome, WindowChromeButton};

mod shared;

pub(crate) use shared::{centred_band, centred_size, draw_focus_ring, palette};
pub use shared::{
    mac_stepper_height, MAC_SEGMENTED_CONTROL_HEIGHT, MAC_SEGMENTED_DEFAULT_WIDTH,
    MAC_SEGMENT_LABEL_INSET, MAC_SOURCE_LIST_DEFAULT_HEIGHT, MAC_SOURCE_LIST_DEFAULT_WIDTH,
    MAC_SOURCE_LIST_ROW_HEIGHT, MAC_SOURCE_LIST_SECTION_HEIGHT, MAC_STEPPER_WIDTH,
    MAC_TITLE_BAR_HEIGHT, MAC_TOOLBAR_DEFAULT_WIDTH, MAC_TOOLBAR_HEIGHT, MAC_TOOLBAR_INSET,
    MAC_TOOLBAR_ITEM_PADDING, MAC_TRAFFIC_LIGHT_GAP, MAC_TRAFFIC_LIGHT_INSET,
};

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
    use crate::core::Rect;

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
