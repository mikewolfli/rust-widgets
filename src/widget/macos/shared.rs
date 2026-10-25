// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Metrics, palette and geometry shared by the macOS controls.
//!
//! The constants and helpers live here rather than in `mod.rs` so the module file stays a
//! declaration-and-re-export surface (principle #8) and the five controls have one place to
//! agree about a band, a size or a colour.

use crate::core::{Color, Rect};
use crate::render::RenderContext;
use crate::widget::metrics::{FocusRing, FOCUS_RING_WIDTH};
use crate::widget::BaseWidget;

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
    /// The face of a window whose window is not key (inactive).
    pub(crate) inactive_surface: Color,
    /// The ink of an inactive window, dimmer than [`Self::ink`].
    pub(crate) inactive_ink: Color,
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
            inactive_surface: theme.colors.surface_container.blend(&theme.colors.foreground, 0.05),
            inactive_ink: theme.colors.foreground.blend(&theme.colors.surface_container, 0.42),
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
            inactive_surface: Color::rgb(242, 242, 244),
            inactive_ink: Color::rgb(110, 110, 115),
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
