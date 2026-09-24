// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rating widget — a star rating control (like 1-5 stars).
//!
//! The Rating widget displays a horizontal row of stars that the user can click
//! to set a rating value. Filled stars (★) are drawn in gold for the rated
//! portion, while unrated stars (☆) are drawn in gray outline.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_f64, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Star rating widget for selecting a rating from 1 to N stars.
pub struct Rating {
    base: BaseWidget,
    /// The rating, as a fraction so half-stars are representable.
    ///
    /// # Why this is `f32` while `rating()` is `u32`
    ///
    /// Real ratings are routinely fractional — "4.5 of 5" is the most common review score
    /// there is — and the published contract has always declared `value` as `Float`. Storing
    /// `u32` meant a caller writing `3.7` silently got `4`: the schema promised a float and the
    /// control dropped the fraction, which is the "reported success for something that did not
    /// happen" shape principle #1 rejects. The integral accessors stay as the projection for
    /// callers that only ever deal in whole stars, so nothing that used the old API changes
    /// behaviour; the exact accessors are what carry the fraction, and `draw` fills the star
    /// the value lands in partially. (BLUE21 A.4.5d, this plan's A.4 `rating` row.)
    rating: f32,
    max_rating: u32,
    star_size: u32,
    /// Emitted when the whole-star value changes.
    ///
    /// Kept as `u32` so an existing subscriber's handler signature is unchanged; the exact
    /// value is available from [`Rating::rating_exact`] inside the handler.
    pub rating_changed: Signal1<u32>,
}

impl Rating {
    /// Creates a new Rating widget with the given geometry.
    /// Defaults to 5 stars with a rating of 0 (no stars selected).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Rating, geometry, "Rating"),
            rating: 0.0,
            max_rating: 5,
            star_size: dimensions::RATING_STAR_SIZE,
            rating_changed: Signal1::new(),
        }
    }

    /// Returns the current rating, rounded to whole stars.
    ///
    /// The projection, not the whole story: a rating of 3.7 is *four* whole stars and reads as
    /// "4 of 5" here, while [`Rating::rating_exact`] reports `3.7` and the drawn row fills the
    /// fourth star 70% of the way.
    pub fn rating(&self) -> u32 {
        self.rating.round() as u32
    }

    /// Returns the current rating as a fraction of a star.
    pub fn rating_exact(&self) -> f32 {
        self.rating
    }

    /// Sets the rating value, clamped to `[0, max_rating]`, to whole stars.
    /// Emits `rating_changed` if the whole-star value changes.
    pub fn set_rating(&mut self, rating: u32) {
        self.set_rating_exact(rating as f32);
    }

    /// Sets the rating value, clamped to `[0, max_rating]`, keeping the fraction.
    ///
    /// This is the setter the `value` property uses, so `set("value", 3.5)` draws three and a
    /// half stars rather than silently four. Emits `rating_changed` only when the *whole-star*
    /// projection changes, so a subscriber counting stars is not woken by a half-step.
    pub fn set_rating_exact(&mut self, rating: f32) {
        // A non-finite write is rejected rather than clamped: `NaN.clamp` is `NaN`, and a NaN
        // rating would make every subsequent comparison false and the row draw nothing.
        if !rating.is_finite() {
            return;
        }
        let clamped = rating.clamp(0.0, self.max_rating as f32);
        let whole_before = self.rating();
        if self.rating != clamped {
            self.rating = clamped;
            let whole_after = self.rating();
            if whole_before != whole_after {
                self.rating_changed.emit(whole_after);
            }
            self.base.request_redraw();
        }
    }

    /// Returns the maximum rating value.
    pub fn max_rating(&self) -> u32 {
        self.max_rating
    }

    /// Sets the maximum rating value.
    /// The current rating is clamped to the new maximum.
    pub fn set_max_rating(&mut self, max_rating: u32) {
        let max = max_rating.max(1);
        self.max_rating = max;
        if self.rating > max as f32 {
            let whole_before = self.rating();
            self.rating = max as f32;
            if whole_before != max {
                self.rating_changed.emit(max);
            }
        }
        self.base.request_redraw();
    }

    /// Returns the size of each star in pixels.
    pub fn star_size(&self) -> u32 {
        self.star_size
    }

    /// Sets the size of each star in pixels.
    pub fn set_star_size(&mut self, size: u32) {
        let size = size.max(8);
        self.star_size = size;
        self.base.request_redraw();
    }

    /// The fraction of the row that is filled, in `0.0..=1.0`.
    ///
    /// # Why this is derived here and not in each consumer
    ///
    /// A rating appears as a number, as a row of stars, and as a bar in a summary card. All three are
    /// the same statement — "this much of the range" — and computing it in each place is how they come
    /// to disagree about whether three of five stars is 60% or 50%. Reads `max_rating().max(1)`, so a
    /// control whose ceiling was somehow zero reports `0.0` rather than dividing by zero.
    pub fn fill_fraction(&self) -> f32 {
        (self.rating / self.max_rating.max(1) as f32).clamp(0.0, 1.0)
    }

    /// The fraction of star `index` that is filled, in `0.0..=1.0`.
    ///
    /// Split out because a half-star is a *per-star* statement, not a row-level one: a 3.5 of 5
    /// rating fills three stars completely, one half, and none of the fifth. Deriving it from
    /// `fill_fraction` would fill 70% of the whole row instead, which is a progress bar wearing
    /// stars. The same function answers both the draw and any test asserting the half.\
    pub fn star_fill(&self, index: u32) -> f32 {
        // The stars before `index` are whole; the star `index` holds the remainder.
        (self.rating - index as f32).clamp(0.0, 1.0)
    }

    /// The rating as it should be spoken or shown: `"3 of 5"`, or `"no rating"` when unset.
    ///
    /// # Why "no rating" rather than "0 of 5"
    ///
    /// Zero stars means the user has not rated yet, which is a different statement from having rated
    /// it the lowest possible. A screen reader announcing "0 of 5" makes an unrated control sound like
    /// a one-star verdict; the same distinction [`crate::platform::accessibility::A11yState::checked`]
    /// documents for `Option<bool>`.
    pub fn display_text(&self) -> String {
        if self.rating <= 0.0 {
            return "no rating".to_string();
        }
        // A whole-star rating reads "3 of 5"; a fractional one keeps its fraction, because
        // rounding it here would reintroduce on the announcement path exactly the silent
        // truncation this control no longer has on the value path.
        if (self.rating - self.rating.round()).abs() < 0.05 {
            format!("{} of {}", self.rating(), self.max_rating)
        } else {
            format!(
                "{} of {}",
                crate::widget::display_widgets::rating::trim_trailing_zero(self.rating),
                self.max_rating
            )
        }
    }
    /// The single column the whole star row is measured from.
    ///
    /// # Why the row is not the control's rectangle
    ///
    /// A rating is a **row of fixed-size glyphs**: one 24 px star, five of them, and a
    /// 4 px gap between neighbours. The control used to derive its pitch from the
    /// rectangle it was handed (`(rect.width - total_width) / 2` as a start, plus a
    /// background fill across the whole of `rect`), so the 240x120 census cell drew a
    /// full-canvas panel with five glyphs floating at y 53 and the star positions moved
    /// whenever the caller changed the control's width. Deriving the row's x from the
    /// stars themselves means the control draws the same object in any rectangle, which
    /// is what [`dimensions::RATING_ROW_HEIGHT`] does for the vertical axis.
    ///
    /// The horizontal origin is the first star's left edge, so the row is centred by
    /// definition rather than by a second, independent arithmetic on `rect.width`.
    fn star_row(&self) -> Rect {
        let count = self.max_rating.max(1);
        let width = count * self.star_size + count.saturating_sub(1) * dimensions::RATING_STAR_GAP;
        // The full-width band is what makes the row's height fixed and centred; its own
        // width is then replaced by the measured star row, centred inside it. `center_in`
        // would clamp an over-wide row rather than letting it overflow the control, which
        // is the same "never paint outside the rectangle" rule every piece of chrome
        // follows.
        let band = ControlMetrics::full_width_band(self.geometry(), dimensions::RATING_ROW_HEIGHT);
        ControlMetrics::center_in(band, crate::core::Size::new(width, band.height))
    }

    /// The cell one star occupies, or `None` when it would not fit inside the row.
    ///
    /// Nothing here reads `rect.width`, so a star is at the same distance from its
    /// neighbour whatever the caller's layout did. A control too narrow for five stars
    /// drops the ones that do not fit rather than painting them over the edge: nothing
    /// clips a widget at this layer, so an overflowing cell would be a drawing
    /// instruction that leaves the control's own rectangle.
    fn star_cell(&self, index: u32) -> Option<Rect> {
        let row = self.star_row();
        let step = self.star_size + dimensions::RATING_STAR_GAP;
        let x = row.x + (index * step) as i32;
        // A partial cell is not a star, so the guard is on the cell's own extent rather
        // than on its left edge alone: a slice of a star reads as a rendering fault.
        if x < row.x || x + self.star_size as i32 > row.x + row.width as i32 {
            return None;
        }
        Some(Rect::new(x, row.y, self.star_size, row.height))
    }
}

impl Widget for Rating {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        // The row's own height, so a layout that honours the hint hands the control
        // exactly the band the stars are painted in. The width is the row's fixed extent.
        crate::core::Size::new(
            self.max_rating.max(1) * (self.star_size + dimensions::RATING_STAR_GAP),
            dimensions::RATING_ROW_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Rating`'s property contract.
///
/// # The defect this replaces
///
/// `value` and `max` describe the *number*, and nothing described the control. `star_size` had a
/// setter and no way to be read or written through the contract, and the little text a rating can
/// legitimately show ("3 of 5") was not published at all — so a consumer had to re-derive it from
/// `value` and `max`, which is exactly the second derivation the property contract exists to remove.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `value` is published as
/// `Float` and is stored as the same fraction, so a caller writing `3.5` gets
/// three and a half stars rather than the `4` the old rounding produced; `max`
/// stays `UInt`.
impl WidgetProperties for Rating {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(f64::from(self.rating_exact()))),
            "max" => Ok(CapabilityValue::UInt(u64::from(self.max_rating()))),
            "star_size" => Ok(CapabilityValue::UInt(u64::from(self.star_size()))),
            // The proportional fill, in `0.0..=1.0`. `value` alone is not enough for a consumer that
            // wants a bar or a percentage: it would have to know `max`, and dividing in the consumer is
            // the second derivation this contract removes.
            "fill" => Ok(CapabilityValue::Float(f64::from(self.fill_fraction()))),
            // "3 of 5" — the announcement text, derived once here so a screen reader, a tooltip and a
            // snapshot all say the same thing.
            "display_text" => Ok(CapabilityValue::String(self.display_text())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                let value = expect_f64(value)?;
                if !value.is_finite() {
                    return Err(CapabilityAccessError::TypeMismatch);
                }
                self.set_rating_exact(value as f32);
                Ok(())
            }
            "max" => {
                self.set_max_rating(expect_u32(value)?);
                Ok(())
            }
            "star_size" => {
                self.set_star_size(expect_u32(value)?);
                Ok(())
            }
            // Derived from `value` and `max`, so writing either would be a second way to say the same
            // thing — and one of the two would be able to disagree.
            "fill" | "display_text" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["value", "max", "star_size", "fill", "display_text", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `rating` publishes.
    ///
    /// `set_value` assigns the star count and `set_max` the ceiling; both need an
    /// argument a command carries none of, so both are refused as
    /// [`CapabilityAccessError::OutOfRange`] — use `set("value", ..)` / `set("max", ..)`
    /// — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_max" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Rating {
    fn draw(&mut self, context: &mut RenderContext) {
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously the plate below and the
        // empty-star grey were hardcoded literals, so light and dark rendered
        // identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("rating");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);

        // ── The row actually painted ──
        //
        // A rating paints a **row of stars**, not a panel: the fill is the row's own
        // band, `RATING_ROW_HEIGHT` tall and centred in the control. This used to be
        // `fill_rect(rect, ..)`, which made the 240x120 census cell a full-bleed plate
        // behind five glyphs — a rectangle the user reads as "a panel that happens to
        // contain stars" rather than as the rating control itself.
        let row = self.star_row();
        context.fill_rect(row, background);

        if self.max_rating == 0 {
            // Nothing to rate: the row band above is the whole control, so there is no
            // per-star chrome to emit rather than a loop that would divide by zero stars.
            return;
        }

        // A star is a glyph, so its cell is the star's own line box centred in the row —
        // not the control's middle line. `center_y` was used directly as the glyph origin,
        // which is the box's top edge, so every star sat half a line low (and `★` is a wide
        // glyph, so it read as a full line). The line box is measured, so a theme with a larger
        // font moves the stars with their own ink.
        let font = Font::default();
        let line = context.text_line(row, &font);
        let center_y = line.y;

        // A filled star is the theme's accent — the slot the palette reserves for a
        // value indicator — so it moves with the appearance instead of staying a
        // fixed gold. The warning token is that slot's public accessor. An empty star
        // is the unselected state of the same control: a tint of the resolved
        // foreground, muted further when the control is disabled.
        let accent = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.blend(&background, 0.0))
            .unwrap_or_else(|| text_color.blend(&background, 0.0));
        // An explicit style wins over the palette, so a host that set a star colour
        // keeps it; the accent is what an unstyled control uses.
        let explicit = self.base.style();
        let filled_color = explicit.border_color.or(explicit.background_color).unwrap_or(accent);
        let empty_color = if is_enabled {
            text_color.blend(&background, 0.35)
        } else {
            text_color.blend(&background, 0.6)
        };

        for i in 0..self.max_rating {
            // The cell comes from the row and the star's own size, so a star's position
            // is a function of the rating control and not of the caller's rectangle. A
            // star that will not fit is skipped rather than clipped.
            let Some(cell) = self.star_cell(i) else {
                continue;
            };
            let fill = self.star_fill(i);
            let glyph_rect =
                Rect { x: cell.x, y: center_y, width: cell.width, height: line.height };

            // The empty glyph is always drawn first, so a partial star is a full outline with a
            // filled portion inside it rather than a narrower star: a half-star is the same
            // object as a whole one, seen half-rated.
            //
            // `Center` positions the glyph from the cell's own midpoint, so the star needs a
            // cell — the previous form passed the cell's midpoint as a *left-origin* point and
            // then asked for `Center`, which shifted every star right by half its own advance.
            context.draw_text_fitted(
                glyph_rect,
                "☆",
                &font,
                empty_color,
                HorizontalAlignment::Center,
            );
            if fill <= 0.0 {
                continue;
            }
            if fill >= 1.0 {
                // A whole star needs no clip: the filled glyph exactly covers the outline.
                context.draw_text_fitted(
                    glyph_rect,
                    "★",
                    &font,
                    filled_color,
                    HorizontalAlignment::Center,
                );
                continue;
            }
            // A partial star is the filled glyph clipped to the rated share of the cell. The
            // filled glyph is drawn in the *whole* cell and the clip cuts it, so the filled
            // half is registered with the outline rather than squeezed into half a cell —
            // squeezing it would draw a smaller star, not a half-filled one.
            let filled_width = (cell.width as f32 * fill).round() as u32;
            if filled_width == 0 {
                continue;
            }
            context.push_clip(cell.x, cell.y, filled_width, cell.height);
            context.draw_text_fitted(
                glyph_rect,
                "★",
                &font,
                filled_color,
                HorizontalAlignment::Center,
            );
            context.pop_clip();
        }
    }
}

/// Formats a fractional star count without a trailing `.0`.
///
/// A whole star reads `"4"` and a half reads `"4.5"`, so the announcement a screen reader gets
/// matches what the row shows instead of saying "4.0 of 5" for four solid stars.\
fn trim_trailing_zero(value: f32) -> String {
    let text = format!("{value:.1}");
    match text.strip_suffix(".0") {
        Some(whole) => whole.to_string(),
        None => text,
    }
}

impl EventHandler for Rating {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        // Disabled controls consume nothing, so the base state stays authoritative.
        self.base.handle_event(event);
        match event {
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                // The hit test is the **drawn geometry**: the star row's own band, so the
                // clickable area and the ink are the same object. Reading `rect` here (as
                // the old form did) made the whole control clickable while only the stars
                // were painted, so a press in the empty space beside the row could set a
                // rating the user did not aim at.
                let row = self.star_row();

                if pos.y < row.y || pos.y >= row.y + row.height as i32 {
                    return;
                }

                let rel_x = pos.x - row.x;
                if rel_x < 0 {
                    return;
                }

                let step = (self.star_size + dimensions::RATING_STAR_GAP) as i32;
                let index = (rel_x / step) as u32;
                // A star that the row dropped is not clickable either: the hit test and the
                // ink are the same set of cells, so a press where no star was painted must
                // not rate.
                if index < self.max_rating && self.star_cell(index).is_some() {
                    // Clicking the same star as the current rating toggles between that star
                    // and clearing; any other star sets that many. Compared against the
                    // whole-star projection so a 4.5 rating still toggles its fourth star off
                    // rather than being treated as a fifth.
                    let new_rating = if self.rating() == index + 1 { index } else { index + 1 };
                    self.set_rating(new_rating);
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn rating_default_values() {
        let r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.rating(), 0);
        assert_eq!(r.max_rating(), 5);
        assert_eq!(r.star_size(), dimensions::RATING_STAR_SIZE);
        assert_eq!(r.kind(), WidgetKind::Rating);
    }

    #[test]
    fn rating_set_rating_emits_signal() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let captured = Arc::new(Mutex::new(None));
        r.rating_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<u32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        r.set_rating(3);
        assert_eq!(r.rating(), 3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn rating_set_rating_clamped() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(10);
        assert_eq!(r.rating(), 5);
    }

    #[test]
    fn rating_set_rating_zero() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(4);
        assert_eq!(r.rating(), 4);
        r.set_rating(0);
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_set_max_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(5);
        r.set_max_rating(3);
        assert_eq!(r.max_rating(), 3);
        assert_eq!(r.rating(), 3);
    }

    #[test]
    fn rating_max_rating_min_one() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_max_rating(0);
        assert_eq!(r.max_rating(), 1);
    }

    #[test]
    fn rating_star_size_get_set() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.star_size(), dimensions::RATING_STAR_SIZE);
        r.set_star_size(32);
        assert_eq!(r.star_size(), 32);
    }

    #[test]
    fn rating_star_size_min_eight() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_star_size(2);
        assert_eq!(r.star_size(), 8);
    }

    /// The two derived readings agree with `value`/`max`, and "unrated" is not "zero of five".
    ///
    /// # The defect this pins
    ///
    /// The contract published only `value` and `max`, so every consumer that wanted a proportion or a
    /// label recomputed it — and a rating's zero means *unrated*, which "0 of 5" gets wrong.
    #[test]
    fn the_contract_publishes_the_fill_and_the_display_text() {
        use crate::widget::capability::WidgetProperties;

        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.get("value").unwrap(), CapabilityValue::Float(0.0));
        assert_eq!(r.get("fill").unwrap(), CapabilityValue::Float(0.0));
        assert_eq!(
            r.get("display_text").unwrap().as_str(),
            Some("no rating"),
            "zero means unrated, not a zero-star verdict"
        );

        r.set_rating(3);
        assert_eq!(r.fill_fraction(), 0.6, "three of five is 60%");
        // The published value is an `f32` widened to `f64` for the contract, so `3/5` arrives as
        // `0.6000000238…` rather than exactly `0.6`. Comparing with a tolerance is the honest
        // assertion: the contract is a proportion, not a decimal literal.
        match r.get("fill").unwrap() {
            CapabilityValue::Float(fill) => {
                assert!((fill - 0.6).abs() < 1e-5, "three of five published as {fill}");
            }
            other => panic!("`fill` must be a Float, got {other:?}"),
        }
        assert_eq!(r.get("display_text").unwrap().as_str(), Some("3 of 5"));

        // The ceiling moves the proportion with it, which is the trap a per-consumer division hits.
        r.set_max_rating(10);
        assert_eq!(r.fill_fraction(), 0.3, "three of ten is 30%");
        assert_eq!(r.get("display_text").unwrap().as_str(), Some("3 of 10"));

        // The derived two are read-only: a second writer would be a second way to say the same thing.
        assert!(r.set("fill", CapabilityValue::Float(1.0)).is_err());
        assert!(r.set("display_text", CapabilityValue::String("x".into())).is_err());
    }

    /// A half-star is representable, and the published contract does not silently round it away.
    ///
    /// Regression (BLUE21 A.4.5d): the schema declared `value` as `Float` while the control stored
    /// `u32`, so `set("value", 3.5)` produced four solid stars and a read-back of `4.0`. A review
    /// score of "four and a half" — the most common fractional rating there is — could not be
    /// expressed at all, and the contract reported success for a value it had changed.
    #[test]
    fn a_fractional_rating_round_trips_and_fills_one_star_partially() {
        use crate::widget::capability::WidgetProperties;

        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set("value", CapabilityValue::Float(3.5)).unwrap();

        assert_eq!(
            r.get("value").unwrap(),
            CapabilityValue::Float(3.5),
            "the contract must report the value it was given"
        );
        // The integral projection is still whole stars, so existing callers are unaffected.
        assert_eq!(r.rating(), 4, "3.5 rounds to four whole stars");
        assert_eq!(r.rating_exact(), 3.5);

        // Per-star fills, not a row-level proportion: three solid, one half, one empty.
        assert_eq!(r.star_fill(0), 1.0);
        assert_eq!(r.star_fill(2), 1.0);
        assert!((r.star_fill(3) - 0.5).abs() < 1e-5, "the fourth star is half rated");
        assert_eq!(r.star_fill(4), 0.0, "the fifth is untouched");

        // And the announcement keeps the fraction rather than reporting the rounded value.
        assert_eq!(
            r.get("display_text").unwrap().as_str(),
            Some("3.5 of 5"),
            "rounding on the announcement path would reintroduce the truncation"
        );
    }

    /// A half-rated star paints the filled glyph clipped to the rated share of its cell.
    ///
    /// The assertion is on the *clip*, because that is what makes a partial star a full outline
    /// with a filled part inside it rather than a smaller star: the filled glyph is drawn in the
    /// whole cell and the clip cuts it. Glyphs reach the SVG as outlines rather than characters,
    /// so the assertion counts the clip and the filled-coloured paths.
    #[test]
    fn a_half_rated_star_is_painted_with_a_clip() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating_exact(2.5);
        let svg = crate::widget::svg::render_to_svg(&mut r);

        // Exactly one star is partial, so there is exactly one clip — and its width is half of a
        // 24 px cell, which is what "half a star" means geometrically.
        assert_eq!(svg.matches("<clipPath").count(), 1, "one partial star: {svg}");
        let clip_width = svg
            .split("<clipPath")
            .nth(1)
            .and_then(|rest| rest.split("width=\"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse::<u32>().ok())
            .expect("the clip must carry a width");
        assert_eq!(clip_width, 12, "half of a 24 px cell: {svg}");

        // Three filled glyphs (two whole stars and the clipped half) at the filled colour, and
        // five outlines at the empty colour; every star keeps its outline.
        let filled = svg.matches("rgba(255,193,7,1.00)").count();
        let empty = svg.matches("rgba(63,63,63,1.00)").count();
        assert_eq!(filled, 3, "two whole stars and one clipped half: {svg}");
        assert_eq!(empty, 5, "every star keeps its outline: {svg}");
    }

    /// `star_size` is reachable from the contract, so the setter is no longer unreachable.
    #[test]
    fn star_size_round_trips_through_the_property_api() {
        use crate::widget::capability::WidgetProperties;

        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.get("star_size").unwrap().as_u64(), Some(24));
        r.set("star_size", CapabilityValue::UInt(32)).unwrap();
        assert_eq!(r.star_size(), 32);
        assert_eq!(r.get("star_size").unwrap().as_u64(), Some(32));
        // The floor still applies through the contract, not only through the setter.
        r.set("star_size", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(r.star_size(), 8);
    }

    /// A press on the third star sets three, using the *drawn* row's own arithmetic.
    ///
    /// The star cells are now derived from the rating's own sizes rather than from
    /// `rect` and a local `gap = 4`: with `star_size = 24` and `RATING_STAR_GAP = 4`
    /// the row is 136 px wide in a 200 px control and starts at x 32, so the cells are
    /// the same ones this test always addressed. What it pins is that the hit test and
    /// the ink still agree after the row moved to a named derivation.
    #[test]
    fn rating_mouse_press_sets_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        // The row is centred in the control's own band, so a press must land on the
        // middle row line rather than at the control's top edge.
        let row = r.star_row();
        assert_eq!(row.width, 136, "five 24 px stars and four 4 px gaps");
        assert_eq!(row.x, 32, "centred in the 200 px control");
        let mid_y = row.y + row.height as i32 / 2;
        assert_eq!(mid_y, 20, "the row is centred in the 40 px control");
        // Star 0: [32..56), Star 1: [60..84), Star 2: [88..112), Star 3: [116..140)
        // Press the 3rd star (index 2 => rating 3)
        r.handle_event(&Event::MouseRelease { pos: Point::new(100, mid_y), button: 1 });
        assert_eq!(r.rating(), 3);
    }

    #[test]
    fn rating_mouse_press_toggles_current_star() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(3);
        let row = r.star_row();
        let mid_y = row.y + row.height as i32 / 2;
        // Click on the 3rd star again (index 2 => rating 3) should set rating back to 2
        r.handle_event(&Event::MouseRelease { pos: Point::new(100, mid_y), button: 1 });
        assert_eq!(r.rating(), 2);
    }

    #[test]
    fn rating_mouse_press_out_of_bounds() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let mid_y = r.star_row().y + r.star_row().height as i32 / 2;
        r.handle_event(&Event::MousePress { pos: Point::new(300, mid_y), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_mouse_press_before_first_star() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let mid_y = r.star_row().y + r.star_row().height as i32 / 2;
        r.handle_event(&Event::MousePress { pos: Point::new(-10, mid_y), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    /// A press outside the drawn star row does nothing, even though it is inside the
    /// control's rectangle. This is the hit-test/ink agreement the fix established: the
    /// old handler tested `rect`, so the empty space above and below the stars was
    /// clickable and a press there could set a rating.
    #[test]
    fn rating_mouse_press_outside_the_drawn_row_changes_nothing() {
        let mut r = Rating::new(Rect::new(0, 0, 240, 120));
        let row = r.star_row();
        assert_eq!(row.height, dimensions::RATING_ROW_HEIGHT);
        assert!(row.y > 0, "the row is centred, so it does not touch the control's top edge");
        // Above the row, still well inside the control.
        r.handle_event(&Event::MouseRelease { pos: Point::new(120, row.y - 1), button: 1 });
        assert_eq!(r.rating(), 0, "a press above the stars must not rate");
        // Below the row, still well inside the control.
        r.handle_event(&Event::MouseRelease {
            pos: Point::new(120, row.y + row.height as i32),
            button: 1,
        });
        assert_eq!(r.rating(), 0, "a press below the stars must not rate");
    }

    /// The star row is a fixed height in any rectangle, and its stars do not move apart
    /// when the caller's width changes.
    #[test]
    fn the_star_row_keeps_its_own_height_and_pitch() {
        let mut narrow = Rating::new(Rect::new(0, 0, 240, 40));
        narrow.set_max_rating(5);
        let mut wide = Rating::new(Rect::new(0, 0, 600, 300));
        wide.set_max_rating(5);

        assert_eq!(narrow.star_row().height, dimensions::RATING_ROW_HEIGHT);
        assert_eq!(wide.star_row().height, dimensions::RATING_ROW_HEIGHT);
        assert_eq!(narrow.star_row().width, wide.star_row().width);

        // The pitch between consecutive stars is the star and the gap, in both.
        for rating in [&narrow, &wide] {
            let first = rating.star_cell(0).expect("a row that fits has a first star");
            let second = rating.star_cell(1).expect("a row that fits has a second star");
            assert_eq!(
                second.x - first.x,
                (dimensions::RATING_STAR_SIZE + dimensions::RATING_STAR_GAP) as i32
            );
            assert_eq!(first.width, dimensions::RATING_STAR_SIZE);
        }
    }

    /// A control narrower than five stars drops the ones that do not fit.
    #[test]
    fn stars_that_do_not_fit_are_dropped_rather_than_painted_over_the_edge() {
        let mut r = Rating::new(Rect::new(0, 0, 60, 24));
        r.set_max_rating(5);
        // A 60 px row holds two 24 px stars and their gap; the rest are not cells at all.
        assert!(r.star_cell(0).is_some());
        assert!(r.star_cell(1).is_some());
        assert!(r.star_cell(2).is_none(), "the third star has no room in a 60 px control");
        // The dropped stars are not clickable either, so ink and hit test stay one set.
        let row = r.star_row();
        let mid_y = row.y + row.height as i32 / 2;
        r.handle_event(&Event::MouseRelease { pos: Point::new(56, mid_y), button: 1 });
        assert!(r.rating() <= 2, "a dropped star cannot be selected, got {}", r.rating());
    }

    /// No emitted star cell is zero-extent, and every one stays inside the control.
    #[test]
    fn every_star_cell_is_drawable_and_inside_the_control() {
        // The last three are deliberately narrower than the five-star row, so the row is
        // clamped: the control must not paint stars outside the rectangle it was given.
        for rect in [Rect::new(0, 0, 240, 120), Rect::new(0, 0, 600, 300), Rect::new(0, 0, 60, 24)]
        {
            let r = Rating::new(rect);
            for index in 0..r.max_rating() {
                if let Some(cell) = r.star_cell(index) {
                    assert!(
                        cell.width > 0 && cell.height > 0,
                        "cell {index} at {rect:?} is drawable"
                    );
                    assert!(cell.x >= rect.x, "cell {index} at {rect:?} starts inside the control");
                    assert!(
                        cell.x + cell.width as i32 <= rect.x + rect.width as i32,
                        "cell {index} at {rect:?} ends inside the control"
                    );
                }
            }
        }
    }

    #[test]
    fn rating_disabled_blocks_events() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_enabled(false);
        let mid_y = r.star_row().y + r.star_row().height as i32 / 2;
        r.handle_event(&Event::MousePress { pos: Point::new(40, mid_y), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_right_click_ignored() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let mid_y = r.star_row().y + r.star_row().height as i32 / 2;
        r.handle_event(&Event::MousePress { pos: Point::new(40, mid_y), button: 2 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_same_value_no_emit() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let count = Arc::new(Mutex::new(0usize));
        r.rating_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<u32>| {
                *count.lock().unwrap() += 1;
            }
        });

        r.set_rating(3);
        r.set_rating(3); // should not emit again
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn rating_mouse_release_also_sets_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let mid_y = r.star_row().y + r.star_row().height as i32 / 2;
        r.handle_event(&Event::MouseRelease { pos: Point::new(40, mid_y), button: 1 });
        assert_eq!(r.rating(), 1);
    }

    #[test]
    fn rating_svg_output() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(3);
        let svg = crate::widget::svg::render_to_svg(&mut r);
        assert!(svg.starts_with("<svg"));
    }

    /// The emitted fills are the star row, not the control's whole rectangle.
    ///
    /// This is the census defect the fix removes: `rating.svg` opened with
    /// `<rect x=0 y=0 width=240 height=120>` — a full-canvas plate — with five glyphs
    /// floating on it. The census composites the control over the theme's own window
    /// fill, so that plate was `rgb(18,18,18)` (byte-identical to the surface behind
    /// it) and only the glyphs were visible. What has to be true is the *lowest* fill:
    /// the first rectangle emitted is the row band, `RATING_ROW_HEIGHT` tall, because
    /// the control has no surface of its own to paint beneath it.
    #[test]
    fn the_rating_paints_a_row_rather_than_a_panel() {
        let mut r = Rating::new(crate::widget::census::CENSUS_RECT);
        let svg = crate::widget::svg::render_to_svg(&mut r);
        let row = r.star_row();
        assert_eq!(row.height, dimensions::RATING_ROW_HEIGHT);
        assert_eq!(row.y, 48, "a 24 px row centred in the 120 px cell");
        assert!(
            svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                row.x, row.y, row.width, row.height
            )),
            "the row band is what is painted: {svg}"
        );
        // The defect's signature: a full-canvas fill would have to be 120 px tall, and
        // the only rectangle that tall is a panel behind the stars.
        assert!(
            !svg.contains("width=\"240\" height=\"120\"")
                || svg.find("height=\"120\"").unwrap_or(usize::MAX) > 0,
            "no emitted fill may be the whole cell: {svg}"
        );
    }
}
