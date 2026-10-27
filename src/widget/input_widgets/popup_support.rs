// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Conversions and layout facts shared by the input family's companion popups (COMP-B01..B03).
//!
//! The three companions — [`SuggestionPopup`](super::suggestion_popup::SuggestionPopup) (which
//! is also the single home of COMP-C01), [`ValidationBubble`](super::validation_bubble::ValidationBubble)
//! and [`RecentValuesPopup`](super::recent_values_popup::RecentValuesPopup) — paint the same row
//! rhythm and publish the same rectangle representation. Keeping both here is what makes
//! "COMP-B03 shares COMP-B01's row spec" a fact rather than a comment: they call one function.

use crate::compat::{String, ToString, Vec};
use crate::core::{Font, Rect};
use crate::widget::capability::coercion::expect_i64;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};

/// The height of one popup row for `font`: the font's size plus eight pixels of leading.
///
/// The two input popups that already exist inline disagreed (24 px vs 28 px); one derivation
/// removes the second numeral.
pub(crate) fn row_height_for(font: &Font) -> u32 {
    (font.size().max(1.0) + 8.0) as u32
}

/// Encodes a rectangle as the `[x, y, width, height]` tuple property representation.
pub(crate) fn rect_value(rect: Rect) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::Int(i64::from(rect.x)),
        CapabilityValue::Int(i64::from(rect.y)),
        CapabilityValue::Int(i64::from(rect.width)),
        CapabilityValue::Int(i64::from(rect.height)),
    ])
}

/// Decodes the `[x, y, width, height]` property representation.
pub(crate) fn expect_rect(value: CapabilityValue) -> Result<Rect, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    if items.len() != 4 {
        return Err(CapabilityAccessError::TypeMismatch);
    }
    let mut items = items.into_iter();
    let mut next = || -> Result<i64, CapabilityAccessError> {
        expect_i64(items.next().ok_or(CapabilityAccessError::TypeMismatch)?)
    };
    let x = next()?;
    let y = next()?;
    let width = next()?;
    let height = next()?;
    if width < 0 || height < 0 {
        return Err(CapabilityAccessError::OutOfRange);
    }
    Ok(Rect::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        width.min(i64::from(u32::MAX)) as u32,
        height.min(i64::from(u32::MAX)) as u32,
    ))
}

/// Encodes a list of rows as a tuple of string values.
pub(crate) fn strings_value(items: &[String]) -> CapabilityValue {
    CapabilityValue::Tuple(items.iter().map(|item| CapabilityValue::String(item.clone())).collect())
}

/// Decodes a tuple of string values into a row list.
pub(crate) fn expect_strings(value: CapabilityValue) -> Result<Vec<String>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    items.into_iter().map(|item| expect_string(item).map(|text| text.to_string())).collect()
}

/// Clamps `rect` into `viewport` on all four sides.
///
/// A popup that is larger than the viewport cannot be clamped without shrinking, so it
/// is pinned to the viewport's origin on the axis it overflows. Both the validation
/// bubble and the recent-values popup use this, so "stays on screen" is one rule (I-125).
pub(crate) fn clamp_rect_into(rect: Rect, viewport: Rect) -> Rect {
    let max_x = viewport.x.saturating_add(viewport.width as i32).saturating_sub(rect.width as i32);
    let max_y =
        viewport.y.saturating_add(viewport.height as i32).saturating_sub(rect.height as i32);
    let x = if max_x < viewport.x { viewport.x } else { rect.x.clamp(viewport.x, max_x) };
    let y = if max_y < viewport.y { viewport.y } else { rect.y.clamp(viewport.y, max_y) };
    Rect::new(x, y, rect.width, rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rectangle_round_trips_through_the_property_representation() {
        let rect = Rect::new(4, 8, 120, 32);
        assert_eq!(expect_rect(rect_value(rect)).unwrap(), rect);
    }

    #[test]
    fn a_negative_size_is_refused() {
        let value = CapabilityValue::Tuple(vec![
            CapabilityValue::Int(0),
            CapabilityValue::Int(0),
            CapabilityValue::Int(-1),
            CapabilityValue::Int(10),
        ]);
        assert!(matches!(expect_rect(value), Err(CapabilityAccessError::OutOfRange)));
    }

    #[test]
    fn rows_round_trip() {
        let rows = vec!["one".to_string(), "two".to_string()];
        assert_eq!(expect_strings(strings_value(&rows)).unwrap(), rows);
    }

    #[test]
    fn a_rectangle_is_clamped_on_all_four_sides() {
        let viewport = Rect::new(0, 0, 100, 100);
        assert_eq!(clamp_rect_into(Rect::new(-5, -3, 20, 10), viewport), Rect::new(0, 0, 20, 10));
        assert_eq!(clamp_rect_into(Rect::new(95, 98, 20, 10), viewport), Rect::new(80, 90, 20, 10));
        // Larger than the viewport: pinned to its origin rather than pushed off it.
        assert_eq!(
            clamp_rect_into(Rect::new(10, 10, 200, 200), viewport),
            Rect::new(0, 0, 200, 200)
        );
    }
}
