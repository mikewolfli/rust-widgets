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

use crate::core::Rect;

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
    /// Splits `rect` into a plot area and the margins around it.
    pub fn of(rect: Rect) -> Self {
        Self::with_margins(rect, 48, 8, 8, 20)
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
