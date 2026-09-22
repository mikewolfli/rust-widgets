// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The plot geometry the financial panes share.
//!
//! # Why this module exists
//!
//! A price chart is not one pane. A K-line pane, a volume histogram, an indicator pane
//! and a depth curve are separate controls that are drawn stacked in one viewport, and
//! they must agree about the horizontal position of every bar — otherwise an overlay
//! lands at the wrong index, which is a chart that is confidently wrong.
//!
//! That agreement cannot come from each control computing its own mapping, because two
//! implementations of "where is bar 17" will differ by a pixel and the difference is
//! invisible until it matters. So the mapping lives here, once, and every pane converts
//! through it.
//!
//! # Why an index-to-x mapping is a range and not a width
//!
//! `slot_width` alone is not enough to place a bar: the caller also has to know where the
//! axis starts. Returning both from one type means the two cannot be taken from different
//! computations — the failure mode of passing a width around and each pane deriving its
//! own origin.
//!
//! # Why the price mapping is inverted at the source
//!
//! Screen y grows downward while price grows upward. Getting that backwards produces a
//! chart that looks plausible and is upside down, which is the single most common mistake
//! in chart code. [`PriceAxis::y_for`] is the only place the inversion happens, so there
//! is one place for it to be right.
//!
//! # Why the pane chrome resolves here too
//!
//! The same argument applies to colour. Four panes that must agree about where bar 17 is
//! must also agree about what the surface behind it looks like, because a stacked chart
//! whose panes disagree about their backdrop reads as four unrelated rectangles. The four
//! finance panes each carried their own literal `rgb(18, 22, 28)`, so the appearance could
//! not reach any of them and the plot area was a black slab in a white window. The
//! derivation is named once here, beside the geometry, so the two cannot drift apart.
//!
//! # Why the margin presets are named and not parameters
//!
//! [`PlotArea::of`], [`PlotArea::indicator_pane`] and [`PlotArea::volume_pane`] exist
//! because a margin typed at the call site is a margin that can be typed differently at
//! the next call site. That had already happened: the indicator pane reserved 52 px on the
//! left where the K-line, volume and depth panes reserved 48, so the same bar index landed
//! four pixels apart in two stacked panes. Naming the preset is what makes "the price
//! panes agree with each other" a property of the type rather than of four call sites
//! having been edited together.

use crate::core::{Color, Rect};
use crate::style::WidgetStyle;

/// The horizontal mapping from bar index to screen x.
///
/// One slot per bar, with the first bar at `origin_x`. A slot is the bar's own column;
/// the bar is drawn inset within it so adjacent bars do not touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexAxis {
    /// Left edge of the plot area.
    pub origin_x: i32,
    /// Width of one bar's column, in pixels. Always at least 1.
    pub slot_width: i32,
    /// How many bars the axis holds.
    pub count: usize,
}

impl IndexAxis {
    /// Builds an axis placing `count` bars across `width` pixels starting at `origin_x`.
    ///
    /// The slot width is the integer division of the width by the count, floored at 1 so
    /// a caller asking for more bars than pixels gets overlapping bars rather than a
    /// division by zero or an invisible chart. Overlap is the honest degradation: a chart
    /// squeezed past readability should still show the shape of the data.
    ///
    /// `count == 0` yields a zero-width axis; nothing is drawn from it, and the 1-pixel
    /// floor is only applied when there is at least one bar to place.
    pub fn new(origin_x: i32, width: i32, count: usize) -> Self {
        let slot_width = if count == 0 { 0 } else { (width.max(0) / count as i32).max(1) };
        Self { origin_x, slot_width, count }
    }

    /// The left edge of bar `index`'s column.
    pub fn x_for(&self, index: usize) -> i32 {
        self.origin_x + self.slot_width * index as i32
    }

    /// The centre of bar `index`'s column, where a line series or a wick is drawn.
    pub fn center_for(&self, index: usize) -> i32 {
        self.x_for(index) + self.slot_width / 2
    }

    /// The width a bar body should be drawn at, leaving a one-pixel gap either side.
    ///
    /// Floored at 1 so a slot of 1 or 2 pixels still produces a visible body — a bar that
    /// rounds to zero width is a bar that silently disappears at high zoom. Zero when the
    /// axis holds no bars, because `total_width()` is zero there and a positive body would
    /// describe a bar that cannot exist; callers multiply this into a rect's width.
    pub fn body_width(&self) -> i32 {
        if self.count == 0 {
            0
        } else if self.slot_width <= 2 {
            1
        } else {
            self.slot_width - 2
        }
    }

    /// The bar index under a screen x, if any.
    ///
    /// The inverse mapping the pointer path needs for a crosshair or a tooltip. Returns
    /// `None` outside the plot rather than clamping, because "the pointer is not over the
    /// chart" is a different answer from "the pointer is over the first bar", and a caller
    /// that needs a clamped index can ask for one.
    ///
    /// Both edges are honoured: an `x` left of `origin_x` **or** past `total_width()` is
    /// outside the plot. Checking only the left edge — as this did — meant an axis with a
    /// negative origin reported a bar for a point to the left of the whole plot, because
    /// the offset there is still positive.
    pub fn index_at(&self, x: i32) -> Option<usize> {
        if self.slot_width <= 0 || self.count == 0 {
            return None;
        }
        let offset = x - self.origin_x;
        if offset < 0 || offset >= self.slot_width * self.count as i32 {
            return None;
        }
        Some((offset / self.slot_width) as usize)
    }

    /// The width the axis occupies, in pixels.
    pub fn total_width(&self) -> i32 {
        self.slot_width * self.count as i32
    }
}

/// The vertical mapping from price to screen y.
///
/// Carries the inverted direction — a higher price maps to a smaller y — and the
/// degenerate cases, so no pane has to re-derive either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceAxis {
    /// Top edge of the plot area, which is the **high** end of the range.
    pub top: i32,
    /// Bottom edge of the plot area, which is the **low** end of the range.
    pub bottom: i32,
    /// Lowest price shown.
    pub low: f64,
    /// Highest price shown.
    pub high: f64,
}

impl PriceAxis {
    /// Builds a price axis over `low..=high` mapped onto `top..=bottom`.
    ///
    /// # Degenerate ranges are expanded, not rejected
    ///
    /// A flat series has `low == high`, and an empty one has neither. Both would divide
    /// by zero in [`Self::y_for`]. Instead of refusing — which would leave a caller with
    /// nothing to draw and no explanation — the range is widened by a nominal amount so
    /// the series draws as a straight line through the middle. That is what a flat price
    /// actually looks like, and it keeps every downstream call total.
    ///
    /// A non-finite bound is treated the same way, since it comes from an empty series.
    pub fn new(top: i32, bottom: i32, low: f64, high: f64) -> Self {
        let mut adjusted_low = low;
        let mut adjusted_high = high;
        if !adjusted_low.is_finite() || !adjusted_high.is_finite() {
            adjusted_low = 0.0;
            adjusted_high = 1.0;
        } else if adjusted_high <= adjusted_low {
            // Widen by a thousandth of the level, or by 1.0 at zero, so a flat or
            // single-valued series still has a non-zero span to divide by.
            let magnitude = adjusted_low.abs().max(1.0) * 0.001;
            adjusted_low -= magnitude;
            adjusted_high += magnitude;
        }
        Self { top, bottom, low: adjusted_low, high: adjusted_high }
    }

    /// The y for `price`, clamped into the plot area.
    ///
    /// Clamped rather than allowed to overflow: a value slightly outside the axis — an
    /// indicator that overshoots the price range, which is normal for a Bollinger band —
    /// would otherwise draw outside the pane and over its neighbour. Clamping keeps it at
    /// the edge, which reads correctly as "off the top of this scale".
    pub fn y_for(&self, price: f64) -> i32 {
        if !price.is_finite() {
            return self.bottom;
        }
        let span = self.high - self.low;
        if span <= 0.0 {
            return self.bottom;
        }
        let fraction = (price - self.low) / span;
        // Inverted: `fraction` 1.0 (the high) is at `top`.
        let y = self.bottom as f64 - fraction * (self.bottom - self.top) as f64;
        (y.round() as i32).clamp(self.top.min(self.bottom), self.top.max(self.bottom))
    }

    /// The height of the plot area.
    pub fn height(&self) -> i32 {
        (self.bottom - self.top).abs()
    }

    /// The price at a screen y, for a crosshair readout.
    ///
    /// The inverse of [`Self::y_for`], used to turn a pointer position into a price for a
    /// tooltip. Not clamped, because a readout should be able to report "above the top of
    /// the scale" rather than lying about a value that is visibly off-chart.
    pub fn price_at(&self, y: i32) -> f64 {
        let height = (self.bottom - self.top) as f64;
        if height == 0.0 {
            return self.low;
        }
        let fraction = (self.bottom - y) as f64 / height;
        self.low + fraction * (self.high - self.low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hit-testing must reject both plot edges, not only the left one.
    ///
    /// Regression: the guard checked `offset < 0` but never the right edge, so any `x`
    /// whose offset happened to be in `[0, slot_width * count)` reported a bar — including
    /// points outside a plot whose origin is negative.
    #[test]
    fn index_at_honours_both_edges() {
        let axis = IndexAxis::new(10, 100, 10);
        assert_eq!(axis.index_at(9), None, "one pixel left of the plot is outside");
        assert_eq!(axis.index_at(10), Some(0), "the plot's own left edge is bar 0");
        assert_eq!(axis.index_at(109), Some(9), "the last bar's last pixel");
        assert_eq!(axis.index_at(110), None, "the first pixel past the plot is outside");
    }

    /// A negative origin must not make a point outside the plot resolve to a bar.
    #[test]
    fn index_at_rejects_points_outside_a_negative_origin_plot() {
        let axis = IndexAxis::new(-30, 100, 10);
        assert_eq!(axis.index_at(-30), Some(0), "the plot starts at -30");
        assert_eq!(axis.index_at(-31), None, "left of the plot's own left edge");
        assert_eq!(axis.index_at(69), Some(9), "the last bar inside the plot");
        assert_eq!(axis.index_at(70), None, "the first pixel past the plot");
    }

    /// An axis with no bars has no body to draw.
    ///
    /// Regression: `body_width()` never consulted `count`, so an empty axis reported a
    /// 1-pixel body while `total_width()` was zero — a bar that cannot exist.
    #[test]
    fn an_empty_axis_has_no_body_width() {
        let axis = IndexAxis::new(0, 100, 0);
        assert_eq!(axis.slot_width, 0);
        assert_eq!(axis.body_width(), 0);
        assert_eq!(axis.total_width(), 0);
    }

    /// A one-pixel slot still yields a visible body, which is the documented floor.
    #[test]
    fn a_one_pixel_slot_still_has_a_visible_body() {
        let axis = IndexAxis::new(0, 10, 10);
        assert_eq!(axis.slot_width, 1);
        assert_eq!(axis.body_width(), 1);
    }

    /// A flat series must draw through the middle rather than divide by zero.
    #[test]
    fn a_degenerate_price_range_is_widened() {
        let axis = PriceAxis::new(0, 100, 50.0, 50.0);
        assert!(axis.high > axis.low, "the range must be usable");
        let middle = axis.y_for(50.0);
        assert!((0..=100).contains(&middle));
    }

    /// The price mapping is inverted: a higher price is a smaller y.
    #[test]
    fn a_higher_price_maps_to_a_smaller_y() {
        let axis = PriceAxis::new(0, 100, 90.0, 110.0);
        assert!(axis.y_for(110.0) < axis.y_for(90.0));
        assert_eq!(axis.y_for(110.0), 0);
        assert_eq!(axis.y_for(90.0), 100);
    }

    /// Every stacked pane must reserve the **same** horizontal margins, or a bar index
    /// lands at two different x values in two panes that are meant to line up.
    ///
    /// Regression: the indicator pane reserved 52 px on the left while the K-line, volume
    /// and depth panes reserved 48, so an indicator at bar 17 was drawn four pixels to the
    /// right of the candle at bar 17.
    #[test]
    fn every_stacked_pane_reserves_the_same_horizontal_margins() {
        let rect = Rect::new(0, 0, 240, 120);
        let reference = PlotArea::price_pane(rect);
        for (name, area) in [
            ("volume", PlotArea::volume_pane(rect)),
            ("indicator", PlotArea::indicator_pane(rect)),
            ("generic", PlotArea::of(rect)),
        ] {
            assert_eq!(area.rect.x, reference.rect.x, "{name} pane's left edge");
            assert_eq!(area.rect.width, reference.rect.width, "{name} pane's width");
        }
    }

    /// An indicator pane still reuses the shared margins after the fix, and the vertical
    /// reservation stays smaller than a price pane's because it carries no index labels.
    #[test]
    fn an_indicator_pane_keeps_the_shared_margins_and_a_shorter_bottom() {
        let area = PlotArea::indicator_pane(Rect::new(0, 0, 240, 120));
        assert_eq!(area.left_margin, PlotArea::DEFAULT_LEFT_MARGIN);
        assert_eq!(area.right_margin, PlotArea::DEFAULT_RIGHT_MARGIN);
        assert!(area.bottom_margin < PlotArea::DEFAULT_BOTTOM_MARGIN);
    }

    /// The panel's ink must be legible on the panel, whatever pairing the caller supplies.
    ///
    /// This is the property the literal could not have: `rgb(18, 22, 28)` produced one
    /// surface for both appearances, so a light appearance drew light-surface ink onto a
    /// dark slab while a dark appearance drew dark-surface ink onto it.
    #[test]
    fn the_panel_ink_is_legible_on_the_panel_it_is_painted_on() {
        let style = WidgetStyle::default()
            .with_background(Color::rgb(18, 22, 28))
            .with_text_color(Color::rgb(30, 34, 40));
        let colors = panel_colors(Some(&style));
        assert_eq!(colors.surface, Color::rgb(18, 22, 28), "the caller's surface wins");
        assert!(
            colors.ink.contrast_ratio(colors.surface) >= PANEL_MIN_CONTRAST,
            "ink {} on surface {},{},{} measured {:.2}:1",
            colors.ink.r,
            colors.surface.r,
            colors.surface.g,
            colors.surface.b,
            colors.ink.contrast_ratio(colors.surface)
        );
    }

    /// A gridline is a hairline of the ink in the surface, so it must be neither invisible
    /// against the panel nor as strong as the text.
    #[test]
    fn the_gridline_sits_between_the_surface_and_the_ink() {
        let colors =
            panel_colors(Some(&WidgetStyle::default().with_background(Color::rgb(18, 22, 28))));
        assert_ne!(colors.grid, colors.surface, "a gridline in the panel colour is not a gridline");
        assert_ne!(colors.grid, colors.ink, "a gridline at full ink is a rule, not a grid");
    }

    /// A surface the theme would make identical to the window behind it is stepped away from
    /// it, so the pane has a visible extent — while a colour the caller set is never moved.
    #[test]
    fn a_surface_equal_to_the_window_fill_is_stepped_away_from_it() {
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::rgb(240, 240, 240));
        let theme_derived = panel_colors(None);
        assert_ne!(
            theme_derived.surface, window_fill,
            "a panel painted in the window fill has no visible extent"
        );
        let caller_choice = Color::rgb(1, 2, 3);
        let caller_derived =
            panel_colors(Some(&WidgetStyle::default().with_background(caller_choice)));
        assert_eq!(
            caller_derived.surface, caller_choice,
            "the caller's colour is honoured verbatim"
        );
    }
}

/// The plot area inside a control's rectangle, after margins.
///
/// # Why the margins are named here rather than at each call site
///
/// Every pane must leave the same room for its axis labels, or two stacked panes' plot
/// areas start at different x and their bars no longer align. Naming the margins in one
/// place is what makes that agreement structural.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotArea {
    /// The area bars and lines are drawn inside.
    pub rect: Rect,
    /// Pixels reserved at the left for price labels.
    pub left_margin: i32,
    /// Pixels reserved at the right, so the last bar is not flush against the edge.
    pub right_margin: i32,
    /// Pixels reserved at the top for a title or the top of the scale.
    pub top_margin: i32,
    /// Pixels reserved at the bottom for the index axis labels.
    pub bottom_margin: i32,
}

impl PlotArea {
    /// The horizontal margin every stacked price pane reserves on its left.
    ///
    /// Wide enough for a four-character price label at the 10 pt axis font plus the tick
    /// gap. A **reservation**, not a measurement, for the same reason the chart's own label
    /// column is: a column sized to the widest label would move the whole plot area every
    /// time a digit boundary is crossed, which reads as the chart jumping when a price
    /// updates.
    const DEFAULT_LEFT_MARGIN: i32 = 48;
    /// The right margin, so the last bar is not flush against the pane's edge.
    const DEFAULT_RIGHT_MARGIN: i32 = 8;
    /// The top margin.
    const DEFAULT_TOP_MARGIN: i32 = 8;
    /// The bottom margin: room for the index axis labels under a price pane.
    pub(crate) const DEFAULT_BOTTOM_MARGIN: i32 = 20;

    /// Splits `rect` into a plot area and the margins around it.
    pub fn of(rect: Rect) -> Self {
        Self::with_margins(
            rect,
            Self::DEFAULT_LEFT_MARGIN,
            Self::DEFAULT_RIGHT_MARGIN,
            Self::DEFAULT_TOP_MARGIN,
            Self::DEFAULT_BOTTOM_MARGIN,
        )
    }

    /// The plot area of a **price pane** — a K-line pane, a depth pane, or any other pane
    /// that is stacked with them and carries the index axis labels itself.
    ///
    /// Not just `of(rect)` spelled differently: this is the named entry point for "this
    /// pane is one of the stacked ones", and it is the same reservation [`Self::of`] makes,
    /// so a pane that switches between the two cannot shift its bars.
    pub fn price_pane(rect: Rect) -> Self {
        Self::of(rect)
    }

    /// The plot area of a **volume pane**, which carries no index labels of its own.
    ///
    /// The price pane above already has them, and a second copy would be misleading about
    /// which rows they belong to, so the bottom reservation is reduced. The **horizontal**
    /// margins are deliberately [`Self::DEFAULT_LEFT_MARGIN`] and
    /// [`Self::DEFAULT_RIGHT_MARGIN`] rather than the caller's own numbers: a volume pane's
    /// bar 17 has to sit under the price pane's bar 17, which is arithmetic here or nowhere.
    pub fn volume_pane(rect: Rect) -> Self {
        Self::with_margins(rect, Self::DEFAULT_LEFT_MARGIN, Self::DEFAULT_RIGHT_MARGIN, 6, 6)
    }

    /// The plot area of an **indicator pane**.
    ///
    /// The vertical margins are reduced for the same reason the volume pane's are: an
    /// oscillator pane reads against reference levels and zero, not against an index axis,
    /// so it does not need the label row. The horizontal margins are the shared ones.
    ///
    /// This used to reserve 52 px on the left while the panes above and below it reserved
    /// 48. Four pixels is small enough to miss on inspection and large enough to matter:
    /// the indicator at bar 17 was drawn under a different x from the candle at bar 17, so
    /// the marker a reader lined up with a price bar was describing a different bar. The
    /// mapping came from [`Self::index_axis`] either way — what was wrong was the rectangle
    /// it was handed, which is why the fix is a named preset and not a corrected number at
    /// one call site.
    pub fn indicator_pane(rect: Rect) -> Self {
        Self::with_margins(
            rect,
            Self::DEFAULT_LEFT_MARGIN,
            Self::DEFAULT_RIGHT_MARGIN,
            Self::DEFAULT_TOP_MARGIN,
            Self::DEFAULT_TOP_MARGIN,
        )
    }

    /// Splits `rect` with explicit margins.
    ///
    /// The margins are clamped so the plot area never inverts: a control resized smaller
    /// than its own margins would otherwise produce a negative width, and every
    /// subsequent division would be nonsense. Shrinking the margins to fit is the honest
    /// degradation for a control that is briefly too small during a resize.
    pub fn with_margins(rect: Rect, left: i32, right: i32, top: i32, bottom: i32) -> Self {
        let max_horizontal = (rect.width as i32).saturating_sub(4).max(0) / 2;
        let left_margin = left.min(max_horizontal).max(0);
        let right_margin = right.min(max_horizontal).max(0);
        let max_vertical = (rect.height as i32).saturating_sub(2).max(0) / 2;
        let top_margin = top.min(max_vertical).max(0);
        let bottom_margin = bottom.min(max_vertical).max(0);

        let width = (rect.width as i32 - left_margin - right_margin).max(0) as u32;
        let height = (rect.height as i32 - top_margin - bottom_margin).max(0) as u32;
        Self {
            rect: Rect::new(rect.x + left_margin, rect.y + top_margin, width, height),
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
        }
    }

    /// The index axis for `count` bars across this plot area.
    pub fn index_axis(&self, count: usize) -> IndexAxis {
        IndexAxis::new(self.rect.x, self.rect.width as i32, count)
    }

    /// The price axis for `low..=high` across this plot area.
    pub fn price_axis(&self, low: f64, high: f64) -> PriceAxis {
        PriceAxis::new(self.rect.y, self.rect.y + self.rect.height as i32, low, high)
    }

    /// The right edge of the plot area.
    pub fn right(&self) -> i32 {
        self.rect.x + self.rect.width as i32
    }

    /// The bottom edge of the plot area.
    pub fn bottom(&self) -> i32 {
        self.rect.y + self.rect.height as i32
    }

    /// The x where the left-hand axis labels are right-aligned.
    ///
    /// One pixel inside the plot, because a label touching the first bar reads as part of
    /// it. Named so every pane's labels line up on the same column.
    pub fn label_x(&self) -> i32 {
        self.rect.x - 4
    }
}

/// The whole chrome of one financial pane: the surface it is painted on, the ink drawn on
/// top of that surface, and the hairline its frame and gridlines are drawn in.
///
/// # Why this is a type and not three more `Color` parameters
///
/// The three colours are not independent: `ink` is only legible *against `surface`*, and
/// `grid` is a blend of the two. Passing them as separate arguments is how a caller supplies
/// an ink derived from the window behind the panel rather than from the panel itself — which
/// is exactly the defect that made the K-line pane's price labels unreadable in earlier
/// rounds, and the same one that made this crate's `chart` control paint the dark theme's
/// ink onto a hardcoded white slab (2.52:1). One value carries all three, so they cannot be
/// taken from different derivations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelColors {
    /// The colour the plot panel is filled with.
    pub surface: Color,
    /// The colour text is drawn in on `surface`, already pushed to legibility.
    pub ink: Color,
    /// The colour the frame and gridlines are drawn in: `ink` blended into `surface`.
    pub grid: Color,
}

/// How far the ink used for axis text and labels is stepped toward legibility.
///
/// WCAG's AA threshold for normal text, so an axis label is not merely *present* on the
/// panel but readable on it. Named rather than inlined because the empty-state message and
/// the axis labels must be legible by the same standard — a chart whose labels pass and
/// whose "No data" message does not is a chart that fails in the state a new user sees.
pub const PANEL_MIN_CONTRAST: f32 = 4.5;

/// Resolves the plot panel's chrome from the active theme.
///
/// # Resolution order
///
/// A control's own explicit (caller-authored) style wins, then the theme's resolved style
/// for that control, then a value derived from the theme's palette, and only then a literal.
/// The literal is never the sole answer: `Color::rgb(18, 22, 28)` standing alone is what
/// made these panes theme-blind, so it is demoted to a last resort for a build with no theme
/// registered at all.
///
/// `style` is the caller's override for this widget when the paint path has one — the four
/// `draw` methods pass `Some(self.base.style())`. Callers that are reading a panel somebody
/// else already resolved pass `None`.
///
/// # Why the widget name does not select the panel
///
/// `resolved_theme_style` classifies by *control kind*, and none of `candlestick_chart`,
/// `volume_chart`, `depth_chart` or `indicator_chart` is in `WidgetRole::for_kind_name`'s
/// table, so all four classify as [`WidgetRole::Surface`]
/// (`src/theme/types.rs`) and the theme writes **the window fill itself** into
/// `style.background_color`. Painting the panel in that colour would make a pane on a window
/// byte-identical to the window behind it — geometrically correct, visually absent — so a
/// resolved surface that *equals* the theme background is re-derived a visible step away from
/// the ink, while a colour a caller set is honoured verbatim. This is the rule
/// `OrderBookWidget` already applies; it lives here now so four panes cannot each get it
/// slightly wrong.
///
/// The theme reads are separate manager lock acquisitions, taken and released inside
/// `resolved_theme_style` and `theme_manager()`, so no guard is held across the caller's
/// draw. Colour decisions go through [`Color::contrast_color`] and the derived ink via
/// [`Color::legible_on`], the crate's single pair of contrast rules.
pub fn panel_colors(style: Option<&WidgetStyle>) -> PanelColors {
    let theme = crate::style::resolved_theme_style("chart");
    // Read as its own lock acquisition and copied out as values, so the guard is dropped
    // before anything else touches the theme. Only the theme's *background* is taken: it is
    // needed to detect a surface the resolver has made identical to the window. The ink is
    // derived from the surface itself below, so that a theme declaring a dark foreground for
    // a light window still produces ink that reads on the panel it is actually painted on.
    let window_fill = {
        let manager = crate::style::theme_manager();
        manager
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::rgb(240, 240, 240))
    };

    let resolved = style
        .and_then(|s| s.background_color)
        .or_else(|| theme.as_ref().and_then(|t| t.background_color));
    let surface = match resolved {
        Some(colour) if colour != window_fill => colour,
        // Either nothing was resolved, or what was resolved is the window's own fill.
        _ => {
            let base = resolved.unwrap_or(window_fill);
            base.blend(&base.contrast_color(), 0.08)
        }
    };

    // Deriving the fallback ink from the *surface* is the point of the ordering: a theme
    // that declares a dark foreground for a light window still gets ink that reads on the
    // panel it is actually painted on.
    let ink = style
        .and_then(|s| s.text_color)
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or_else(|| surface.contrast_color());
    let ink = ink.legible_on(surface, PANEL_MIN_CONTRAST);

    PanelColors { surface, ink, grid: surface.blend(&ink, 0.22) }
}
