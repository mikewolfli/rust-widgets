//! Projection screen rendering adaptation (BLUE8 P4-5b).
//!
//! This module provides projection/presentation-mode rendering support
//! gated behind the `projection` feature flag. It includes:
//!
//! - [`PresentationController`] — slide/page state management with
//!   page navigation, total-page tracking, and transition animation hints.
//! - [`ProjectionRenderConfig`] — rendering configuration for projection
//!   mode: text scale factor, read-only mode, full-screen flag, and
//!   page-indicator display.
//!
//! # Integration
//!
//! When the `projection` feature is enabled, platform backends that
//! detect `DeviceClass::Projector` should call [`ProjectionRenderConfig`]
//! to adjust the rendering pipeline:
//!
//! 1. Install `ProjectionRenderConfig` into the render context.
//! 2. Attach a `PresentationController` to track slide state.
//! 3. Render the page indicator overlay (optional, controlled by
//!    `show_page_indicator`).
//!
//! All controls automatically become read-only in projection mode
//! (no hover effects, no mouse/touch interaction — only swipe-to-page
//! or remote-control input).

use crate::core::Size;

// ────────────────────────────────────────────────────────
// PresentationController — slide/page state management
// ────────────────────────────────────────────────────────

/// Controller for slide/presentation state management.
///
/// Tracks the current page, total page count, and transition
/// animation metadata. Designed for projection-mode only.
#[derive(Debug, Clone)]
pub struct PresentationController {
    /// Current page index (0-based).
    current_page: usize,
    /// Total number of pages in the presentation.
    total_pages: usize,
    /// Optional animation duration in milliseconds for page transitions.
    transition_ms: u32,
    /// Whether the page indicator overlay is visible.
    show_indicator: bool,
}

impl PresentationController {
    /// Create a new controller with the given total number of pages.
    ///
    /// Starts at page 0 (the first page).
    pub fn new(total_pages: usize) -> Self {
        Self {
            current_page: 0,
            total_pages: total_pages.max(1),
            transition_ms: 300,
            show_indicator: true,
        }
    }

    /// Current page index (0-based).
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Total number of pages.
    pub fn total_pages(&self) -> usize {
        self.total_pages
    }

    /// Advance to the next page. Returns `false` if already on the last page.
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.total_pages {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    /// Go back to the previous page. Returns `false` if already on the first page.
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    /// Jump to a specific page (0-based). Clamped to valid range.
    pub fn go_to_page(&mut self, page: usize) {
        self.current_page = page.min(self.total_pages.saturating_sub(1));
    }

    /// Reset to the first page.
    pub fn reset(&mut self) {
        self.current_page = 0;
    }

    /// Returns `true` if the current page is the first page.
    pub fn is_first(&self) -> bool {
        self.current_page == 0
    }

    /// Returns `true` if the current page is the last page.
    pub fn is_last(&self) -> bool {
        self.current_page + 1 >= self.total_pages
    }

    /// Set the total number of pages. The current page is clamped if necessary.
    pub fn set_total_pages(&mut self, total: usize) {
        self.total_pages = total.max(1);
        self.current_page = self.current_page.min(self.total_pages.saturating_sub(1));
    }

    /// Transition animation duration in milliseconds.
    pub fn transition_ms(&self) -> u32 {
        self.transition_ms
    }

    /// Set transition animation duration in milliseconds.
    pub fn set_transition_ms(&mut self, ms: u32) {
        self.transition_ms = ms;
    }

    /// Whether the page indicator overlay is shown.
    pub fn show_indicator(&self) -> bool {
        self.show_indicator
    }

    /// Set whether to show the page indicator overlay.
    pub fn set_show_indicator(&mut self, show: bool) {
        self.show_indicator = show;
    }

    /// Returns a string like "3 / 12" for display.
    pub fn page_label(&self) -> String {
        format!("{} / {}", self.current_page + 1, self.total_pages)
    }
}

impl Default for PresentationController {
    fn default() -> Self {
        Self::new(1)
    }
}

// ────────────────────────────────────────────────────────
// ProjectionRenderConfig — projection-mode rendering config
// ────────────────────────────────────────────────────────

/// Rendering configuration for projection/presentation mode.
///
/// Adjusts the rendering pipeline for large-screen, read-only
/// presentation displays typically driven by remote control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionRenderConfig {
    /// Text and chrome scale factor (default 1.2 for projection readability).
    pub text_scale: f32,
    /// Whether the UI is in read-only presentation mode.
    pub read_only: bool,
    /// Whether the window should be full-screen.
    pub full_screen: bool,
    /// Whether to show the page indicator at the bottom of the screen.
    pub show_page_indicator: bool,
    /// Whether to hide the title bar / chrome.
    pub hide_chrome: bool,
    /// Pixel height of the page indicator bar (0 = auto, default 0).
    pub indicator_height: u32,
}

impl ProjectionRenderConfig {
    /// Default projection configuration with 1.2x text scale.
    pub const fn new() -> Self {
        Self {
            text_scale: 1.2,
            read_only: true,
            full_screen: true,
            show_page_indicator: true,
            hide_chrome: true,
            indicator_height: 0,
        }
    }

    /// Apply the text scale factor to a base font size, returning the
    /// projection-adjusted size.
    pub fn scale_font_size(&self, base_pt: f32) -> f32 {
        base_pt * self.text_scale
    }

    /// Apply the layout scale factor to a dimension (width, height, spacing).
    pub fn scale_dimension(&self, base: u32) -> u32 {
        (base as f32 * self.text_scale).round() as u32
    }

    /// Returns the indicator bar height in logical pixels. If
    /// `indicator_height` is 0, computes a reasonable default based
    /// on the screen height.
    pub fn indicator_bar_height(&self, screen_height: u32) -> u32 {
        if self.indicator_height > 0 {
            self.indicator_height
        } else {
            (screen_height as f32 * 0.04).round().max(24.0) as u32
        }
    }

    /// Enable full-screen mode.
    pub fn with_full_screen(mut self, enabled: bool) -> Self {
        self.full_screen = enabled;
        self
    }

    /// Enable/disable the page indicator.
    pub fn with_page_indicator(mut self, show: bool) -> Self {
        self.show_page_indicator = show;
        self
    }

    /// Set a custom text scale factor.
    pub fn with_text_scale(mut self, scale: f32) -> Self {
        self.text_scale = scale.clamp(0.5, 3.0);
        self
    }

    /// Set a custom indicator bar height in logical pixels.
    pub fn with_indicator_height(mut self, height: u32) -> Self {
        self.indicator_height = height;
        self
    }
}

impl Default for ProjectionRenderConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────
// ProjectionLayoutHelper — layout calculations for projection
// ────────────────────────────────────────────────────────

/// Helper for calculating projection-mode layout adjustments.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionLayoutHelper {
    /// Screen size in logical pixels.
    pub screen_size: Size,
    /// Projection render config.
    pub config: ProjectionRenderConfig,
}

impl ProjectionLayoutHelper {
    /// Create a new layout helper for the given screen and config.
    pub fn new(screen_size: Size, config: ProjectionRenderConfig) -> Self {
        Self { screen_size, config }
    }

    /// Returns the content area rectangle (screen minus indicator bar).
    pub fn content_area(&self) -> (u32, u32, u32, u32) {
        let indicator_h = if self.config.show_page_indicator {
            self.config.indicator_bar_height(self.screen_size.height)
        } else {
            0
        };
        (0, 0, self.screen_size.width, self.screen_size.height.saturating_sub(indicator_h))
    }

    /// Returns the page indicator bar rectangle at the bottom of the screen.
    pub fn indicator_bar_rect(&self) -> (u32, u32, u32, u32) {
        let indicator_h = if self.config.show_page_indicator {
            self.config.indicator_bar_height(self.screen_size.height)
        } else {
            0
        };
        (
            0,
            self.screen_size.height.saturating_sub(indicator_h),
            self.screen_size.width,
            indicator_h,
        )
    }

    /// Scale a dimension by the projection text scale.
    pub fn scale(&self, base: u32) -> u32 {
        self.config.scale_dimension(base)
    }
}

// ────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_controller_default() {
        let ctrl = PresentationController::default();
        assert_eq!(ctrl.current_page(), 0);
        assert_eq!(ctrl.total_pages(), 1);
        assert!(ctrl.is_first());
        assert!(ctrl.is_last());
        assert_eq!(ctrl.page_label(), "1 / 1");
    }

    #[test]
    fn presentation_controller_navigation() {
        let mut ctrl = PresentationController::new(5);
        assert_eq!(ctrl.current_page(), 0);
        assert!(ctrl.is_first());
        assert!(!ctrl.is_last());

        assert!(ctrl.next_page());
        assert_eq!(ctrl.current_page(), 1);
        assert!(!ctrl.is_first());
        assert!(!ctrl.is_last());

        assert!(ctrl.next_page());
        assert_eq!(ctrl.current_page(), 2);

        assert!(ctrl.prev_page());
        assert_eq!(ctrl.current_page(), 1);

        ctrl.go_to_page(4);
        assert_eq!(ctrl.current_page(), 4);
        assert!(ctrl.is_last());

        // Next from last page should return false
        assert!(!ctrl.next_page());
        assert_eq!(ctrl.current_page(), 4);

        // Prev from first page should return false
        ctrl.go_to_page(0);
        assert!(!ctrl.prev_page());
        assert_eq!(ctrl.current_page(), 0);
    }

    #[test]
    fn presentation_controller_clamp() {
        let mut ctrl = PresentationController::new(3);
        ctrl.go_to_page(100);
        assert_eq!(ctrl.current_page(), 2);
        assert!(ctrl.is_last());
    }

    #[test]
    fn presentation_controller_set_total_pages() {
        let mut ctrl = PresentationController::new(10);
        ctrl.go_to_page(9);
        ctrl.set_total_pages(3);
        assert_eq!(ctrl.current_page(), 2);
        assert_eq!(ctrl.total_pages(), 3);
    }

    #[test]
    fn presentation_controller_reset() {
        let mut ctrl = PresentationController::new(10);
        ctrl.go_to_page(5);
        ctrl.reset();
        assert_eq!(ctrl.current_page(), 0);
        assert!(ctrl.is_first());
    }

    #[test]
    fn presentation_controller_label() {
        let ctrl = PresentationController::new(12);
        assert_eq!(ctrl.page_label(), "1 / 12");

        let mut ctrl = PresentationController::new(7);
        ctrl.go_to_page(3);
        assert_eq!(ctrl.page_label(), "4 / 7");
    }

    #[test]
    fn projection_config_default() {
        let cfg = ProjectionRenderConfig::new();
        assert!((cfg.text_scale - 1.2).abs() < f32::EPSILON);
        assert!(cfg.read_only);
        assert!(cfg.full_screen);
        assert!(cfg.show_page_indicator);
        assert!(cfg.hide_chrome);
        assert_eq!(cfg.indicator_height, 0);
    }

    #[test]
    fn projection_config_scale_font() {
        let cfg = ProjectionRenderConfig::new();
        let scaled = cfg.scale_font_size(12.0);
        assert!((scaled - 14.4).abs() < 1e-4, "expected ~14.4, got {}", scaled);
    }

    #[test]
    fn projection_config_scale_dimension() {
        let cfg = ProjectionRenderConfig::new();
        assert_eq!(cfg.scale_dimension(100), 120);
    }

    #[test]
    fn projection_config_indicator_bar_height() {
        let cfg = ProjectionRenderConfig::new();
        let h = cfg.indicator_bar_height(1080);
        assert_eq!(h, 43); // 1080 * 0.04 = 43.2 -> 43
    }

    #[test]
    fn projection_config_custom_indicator_height() {
        let cfg = ProjectionRenderConfig::new().with_indicator_height(60);
        assert_eq!(cfg.indicator_height, 60);
        assert_eq!(cfg.indicator_bar_height(1080), 60);
    }

    #[test]
    fn projection_config_with_full_screen() {
        let cfg = ProjectionRenderConfig::new().with_full_screen(false);
        assert!(!cfg.full_screen);
        assert!(cfg.read_only);
    }

    #[test]
    fn projection_config_with_text_scale_clamp() {
        let cfg = ProjectionRenderConfig::new().with_text_scale(5.0);
        assert!((cfg.text_scale - 3.0).abs() < f32::EPSILON);

        let cfg = ProjectionRenderConfig::new().with_text_scale(0.1);
        assert!((cfg.text_scale - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn projection_layout_helper_content_area() {
        let size = Size::new(1920, 1080);
        let cfg = ProjectionRenderConfig::new();
        let helper = ProjectionLayoutHelper::new(size, cfg);
        let (x, y, w, h) = helper.content_area();
        assert_eq!(x, 0);
        assert_eq!(y, 0);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080 - 43); // 1080 - indicator bar
    }

    #[test]
    fn projection_layout_helper_indicator_bar_rect() {
        let size = Size::new(1920, 1080);
        let cfg = ProjectionRenderConfig::new();
        let helper = ProjectionLayoutHelper::new(size, cfg);
        let (x, y, w, h) = helper.indicator_bar_rect();
        assert_eq!(x, 0);
        assert_eq!(y, 1080 - 43);
        assert_eq!(w, 1920);
        assert_eq!(h, 43);
    }

    #[test]
    fn projection_hide_indicator_removes_bar() {
        let size = Size::new(1920, 1080);
        let cfg = ProjectionRenderConfig::new().with_page_indicator(false);
        let helper = ProjectionLayoutHelper::new(size, cfg);
        let (_x, _y, _w, h) = helper.indicator_bar_rect();
        assert_eq!(h, 0);
        let (_, _, _, ch) = helper.content_area();
        assert_eq!(ch, 1080);
    }

    #[test]
    fn projection_controller_transition_ms() {
        let mut ctrl = PresentationController::new(10);
        assert_eq!(ctrl.transition_ms(), 300);
        ctrl.set_transition_ms(500);
        assert_eq!(ctrl.transition_ms(), 500);
    }

    #[test]
    fn projection_controller_show_indicator_toggle() {
        let mut ctrl = PresentationController::new(5);
        assert!(ctrl.show_indicator());
        ctrl.set_show_indicator(false);
        assert!(!ctrl.show_indicator());
    }
}
