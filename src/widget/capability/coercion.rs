// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-coercion helpers for the capability layer.
//!
//! These functions convert [`CapabilityValue`] variants into concrete Rust types,
//! performing string parsing and numeric coercion as needed.
//!
//! # Coercion rules
//!
//! A property write is accepted or rejected by these functions, so the rules
//! below are the contract callers can rely on:
//!
//! * **Booleans and strings never coerce.** [`expect_bool`] accepts only
//!   [`CapabilityValue::Bool`]; [`expect_string`] accepts only
//!   [`CapabilityValue::String`]. No `"true"`/`"1"` parsing happens.
//! * **Numeric widening is allowed, narrowing is not.** [`expect_f32`] and
//!   [`expect_f64`] accept floats and both signed and unsigned integers;
//!   [`expect_i64`] accepts signed integers and unsigned integers in range;
//!   [`expect_usize`] and [`expect_u32`] accept unsigned integers and
//!   non-negative signed integers. A value that does not fit the target type
//!   (for example `-1` for `usize`, or `2^64 - 1` for `i64`) is rejected rather
//!   than being wrapped or saturated, as is a float targeted at an integer type.
//! * **Enumerations parse from strings only.** The `expect_*` functions for
//!   control enums require a string, which is normalised by [`normalize_key`]
//!   first: underscores, hyphens, and spaces are removed and the result is
//!   lower-cased. Several historical spellings are accepted per variant; the
//!   accepted synonym for each is listed on the function.
//! * **Dates and times parse from strings only**, using the same normalisation
//!   rules as the corresponding `Display` implementations so values round-trip.
//!
//! Every rejection is reported as [`CapabilityAccessError::TypeMismatch`]; the
//! helpers never panic on malformed input and never silently substitute a
//! default value.
//!
//! Each function with a `*_to_str` counterpart is half of a codec pair: the
//! parser is the inverse of the formatter, and the pair is kept in this module
//! so that a control can read back whatever a caller can write.

#[cfg(full_widgets)]
use chrono::{NaiveDate, Weekday};

use super::CapabilityAccessError;
use super::CapabilityValue;
use crate::compat::String;
use crate::core::{Alignment, Orientation, TextDirection};
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::time_edit::Time;
use crate::widget::base_widgets::checkbox::CheckState;
#[cfg(full_widgets)]
use crate::widget::dialog::message_box::MessageBoxIcon;
#[cfg(widgets_unstripped)]
use crate::widget::display_widgets::badge::BadgeLevel;
#[cfg(widgets_unstripped)]
use crate::widget::display_widgets::lcd_number::{LCDNumberMode, SegmentStyle};
#[cfg(widgets_unstripped)]
use crate::widget::display_widgets::slider::TickPosition;
use crate::widget::input_widgets::listbox::SelectionMode as ListBoxSelectionMode;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::tool_bar::ToolBarOrientation;
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::{ColumnFilter, SortSpec};
#[cfg(full_widgets)]
use crate::widget::view_widgets::list_view::{SelectionMode, ViewMode};
use crate::widget::Widget;

// ---------------------------------------------------------------------------
// Low-level downcast helpers
// ---------------------------------------------------------------------------

/// Downcasts a widget reference to its concrete type, or returns `None` when
/// the widget is not a `T`.
///
/// The cast goes through `dyn Any`, so it requires `T: 'static` and compares
/// types exactly; a trait object of a supertype will not match its subtypes.
pub fn widget_as<T: Widget + 'static>(widget: &dyn Widget) -> Option<&T> {
    (widget as &dyn crate::compat::Any).downcast_ref::<T>()
}

/// Mutable counterpart of [`widget_as`], for writing to a widget's properties.
pub fn widget_as_mut<T: Widget + 'static>(widget: &mut dyn Widget) -> Option<&mut T> {
    (widget as &mut dyn crate::compat::Any).downcast_mut::<T>()
}

// ---------------------------------------------------------------------------
// Primitive value extractors
// ---------------------------------------------------------------------------

/// Extracts the boolean payload, without coercion.
///
/// Returns [`CapabilityAccessError::TypeMismatch`] for any other variant,
/// including strings such as `"true"`.
pub fn expect_bool(value: CapabilityValue) -> Result<bool, CapabilityAccessError> {
    match value {
        CapabilityValue::Bool(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts the string payload, without coercion.
///
/// Returns [`CapabilityAccessError::TypeMismatch`] for any other variant; in
/// particular numbers are not stringified.
pub fn expect_string(value: CapabilityValue) -> Result<String, CapabilityAccessError> {
    match value {
        CapabilityValue::String(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts a non-negative index.
///
/// Accepts [`CapabilityValue::UInt`] and non-negative [`CapabilityValue::Int`].
/// Negative integers and values too large for `usize` yield
/// [`CapabilityAccessError::TypeMismatch`].
pub fn expect_usize(value: CapabilityValue) -> Result<usize, CapabilityAccessError> {
    match value {
        CapabilityValue::UInt(v) => {
            usize::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        CapabilityValue::Int(v) if v >= 0 => {
            usize::try_from(v as u64).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts a single-precision float.
///
/// Accepts [`CapabilityValue::Float`], [`CapabilityValue::UInt`], and
/// [`CapabilityValue::Int`]. Integer payloads are converted with `as`, so
/// magnitudes beyond 24 bits lose precision silently; nothing else is accepted.
pub fn expect_f32(value: CapabilityValue) -> Result<f32, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v as f32),
        CapabilityValue::UInt(v) => Ok(v as f32),
        CapabilityValue::Int(v) => Ok(v as f32),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts a double-precision float.
///
/// Accepts float, unsigned, and signed integer payloads, widening integers to
/// `f64`. Integers above `2^53` lose precision without an error.
pub fn expect_f64(value: CapabilityValue) -> Result<f64, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v),
        CapabilityValue::Int(v) => Ok(v as f64),
        CapabilityValue::UInt(v) => Ok(v as f64),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts a signed 64-bit integer.
///
/// Accepts signed integers, and unsigned integers that fit in `i64`. Unsigned
/// values above `i64::MAX` are rejected. Floats are not converted, even when
/// they hold a whole number.
pub fn expect_i64(value: CapabilityValue) -> Result<i64, CapabilityAccessError> {
    match value {
        CapabilityValue::Int(v) => Ok(v),
        CapabilityValue::UInt(v) => {
            i64::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Extracts an unsigned 32-bit integer.
///
/// Applies the [`expect_usize`] rules first, then rejects values that do not
/// fit in `u32` with [`CapabilityAccessError::TypeMismatch`] rather than
/// truncating them.
pub fn expect_u32(value: CapabilityValue) -> Result<u32, CapabilityAccessError> {
    let raw = expect_usize(value)?;
    u32::try_from(raw).map_err(|_| CapabilityAccessError::TypeMismatch)
}

// ---------------------------------------------------------------------------
// Date / time extractors
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
/// Parses a `chrono` date from the ISO-8601 `"YYYY-MM-DD"` spelling.
///
/// Requires a string payload. Unlike [`expect_date`], the calendar validity is
/// checked by `chrono`, which rejects impossible dates such as `2026-02-30`.
pub fn expect_naive_date(value: CapabilityValue) -> Result<NaiveDate, CapabilityAccessError> {
    let text = expect_string(value)?;
    NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(|_| CapabilityAccessError::TypeMismatch)
}

#[cfg(full_widgets)]
/// Parses a [`Date`] from the `"YYYY-MM-DD"` spelling used by
/// [`Date`]'s `Display` implementation.
///
/// Requires a string payload. Year, month, and day are accepted as
/// decimal integers separated by `-`, so the month and day are 1-based as
/// usual. Extra components are rejected, as are non-numeric ones. The parsed
/// values must satisfy [`Date::is_valid`]; out-of-range input (month `0` or
/// `13`, day `0` or `32`) is rejected with
/// [`CapabilityAccessError::TypeMismatch`] rather than being clamped.
pub fn expect_date(value: CapabilityValue) -> Result<Date, CapabilityAccessError> {
    let text = expect_string(value)?;
    let mut parts = text.split('-');
    let year = parts
        .next()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let month = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let day = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    if parts.next().is_some() {
        return Err(CapabilityAccessError::TypeMismatch);
    }
    let date = Date::new(year, month, day);
    if date.is_valid() {
        Ok(date)
    } else {
        Err(CapabilityAccessError::TypeMismatch)
    }
}

#[cfg(full_widgets)]
/// Parses a [`Time`] from the `"HH:MM:SS[.fff]"` spelling used by
/// [`Time`]'s `Display` implementation.
///
/// Requires a string payload. The hour is 24-hour (`0`-`23`), the minute and
/// second are 1-based fields ranging over `0`-`59`, and the optional fractional
/// part gives milliseconds; it is right-padded to three digits, so `".5"` means
/// 500 ms and digits beyond the third are dropped rather than rounded. The
/// seconds component is mandatory. Invalid clock values are rejected with
/// [`CapabilityAccessError::TypeMismatch`] via [`Time::is_valid`], never
/// wrapped or clamped.
pub fn expect_time(value: CapabilityValue) -> Result<Time, CapabilityAccessError> {
    let text = expect_string(value)?;
    let mut parts = text.split(':');
    let hour = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let minute = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let second_part = parts.next().ok_or(CapabilityAccessError::TypeMismatch)?;
    if parts.next().is_some() {
        return Err(CapabilityAccessError::TypeMismatch);
    }

    let (second, msec) = if let Some((sec, frac)) = second_part.split_once('.') {
        let second = sec.parse::<u8>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        let frac_trimmed = frac.chars().take(3).collect::<String>();
        let scale = 10u16.pow((3usize.saturating_sub(frac_trimmed.len())) as u32);
        let raw = frac_trimmed.parse::<u16>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        (second, raw * scale)
    } else {
        let second = second_part.parse::<u8>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        (second, 0)
    };

    let time = Time::new(hour, minute, second, msec);
    if time.is_valid() {
        Ok(time)
    } else {
        Err(CapabilityAccessError::TypeMismatch)
    }
}

/// Parses the `"<date> <time>"` spelling that [`crate::widget::advanced_widgets::date_time_edit::DateTime`]
/// renders through its `Display` impl.
///
/// The read side of the contract publishes exactly that string, so this is its
/// inverse: a writer can round-trip whatever a reader returned. The two halves are
/// split on the first space and delegated to [`expect_date`] / [`expect_time`], so
/// the date and time rules are stated once each rather than repeated here.
#[cfg(full_widgets)]
pub fn expect_datetime(
    value: CapabilityValue,
) -> Result<crate::widget::advanced_widgets::date_time_edit::DateTime, CapabilityAccessError> {
    let text = expect_string(value)?;
    let (date_part, time_part) = text.split_once(' ').ok_or(CapabilityAccessError::TypeMismatch)?;
    let date = expect_date(CapabilityValue::String(date_part.to_string()))?;
    let time = expect_time(CapabilityValue::String(time_part.to_string()))?;
    Ok(crate::widget::advanced_widgets::date_time_edit::DateTime::new(date, time))
}

#[cfg(full_widgets)]
/// Parses a weekday name.
///
/// Accepts an abbreviated or full English name (`"Mon"`, `"monday"`, …), in
/// any case and with separators ignored. A string payload is required; unknown
/// names yield [`CapabilityAccessError::TypeMismatch`].
pub fn expect_weekday(value: CapabilityValue) -> Result<Weekday, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    match token.as_str() {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

// ---------------------------------------------------------------------------
// Composite type extractors
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
/// Parses a sort specification list of the form `"col:order,col:order"`.
///
/// The column is a 0-based index into the view's columns; the order is `asc`
/// or `desc`, case-insensitive and separator-insensitive. An empty or
/// whitespace-only string means "no sorting" and yields an empty vector.
/// Sorting is applied in the order given. A missing or unrecognised order, a
/// non-numeric column, or a token with more than one `:` is rejected with
/// [`CapabilityAccessError::TypeMismatch`]; whitespace around each field is
/// ignored.
pub fn expect_sort_specs(value: CapabilityValue) -> Result<Vec<SortSpec>, CapabilityAccessError> {
    let text = expect_string(value)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut specs = Vec::new();
    for token in text.split(',') {
        let mut parts = token.split(':');
        let column = parts
            .next()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let order = parts
            .next()
            .map(|v| normalize_key(v.trim()))
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        if parts.next().is_some() {
            return Err(CapabilityAccessError::TypeMismatch);
        }

        let descending = match order.as_str() {
            "asc" => false,
            "desc" => true,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        specs.push(SortSpec { column, descending });
    }
    Ok(specs)
}

#[cfg(full_widgets)]
/// Parses a per-column filter list of the form `"col=query,col=query"`.
///
/// The column is a 0-based index; the query is the remaining text after the
/// first `=` and is taken verbatim, so it may itself contain `=` signs. An
/// empty or whitespace-only string means "no filters" and yields an empty
/// vector. A token with no `=` or a non-numeric column index is rejected with
/// [`CapabilityAccessError::TypeMismatch`].
pub fn expect_column_filters(
    value: CapabilityValue,
) -> Result<Vec<ColumnFilter>, CapabilityAccessError> {
    let text = expect_string(value)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut filters = Vec::new();
    for token in text.split(',') {
        let mut parts = token.splitn(2, '=');
        let column = parts
            .next()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let query =
            parts.next().map(|v| v.to_string()).ok_or(CapabilityAccessError::TypeMismatch)?;
        filters.push(ColumnFilter { column, query });
    }
    Ok(filters)
}

#[cfg(full_widgets)]
/// Parses a list-view selection mode: `single`, `multi`, or `extended`.
///
/// A string payload is required; the match is case- and separator-insensitive.
/// Any other value yields [`CapabilityAccessError::TypeMismatch`].
pub fn expect_selection_mode(
    value: CapabilityValue,
) -> Result<SelectionMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "single" => Ok(SelectionMode::Single),
        "multi" => Ok(SelectionMode::Multi),
        "extended" => Ok(SelectionMode::Extended),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses a list-box selection mode: `none`, `single`, `multi`, or `extended`.
///
/// Both the short names above and the historical `noselection`,
/// `singleselection`, `multiselection`, and `extendedselection` spellings are
/// accepted, in any case and with separators ignored. Any other value yields
/// [`CapabilityAccessError::TypeMismatch`].
pub fn expect_list_box_selection_mode(
    value: CapabilityValue,
) -> Result<ListBoxSelectionMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    // Both spellings are accepted: the canonical variant names and the historical
    // list-box names (`noselection`, `singleselection`, …) that config files and
    // bindings may still use.
    match token.as_str() {
        "none" | "noselection" => Ok(ListBoxSelectionMode::None),
        "single" | "singleselection" => Ok(ListBoxSelectionMode::Single),
        "multi" | "multiselection" => Ok(ListBoxSelectionMode::Multi),
        "extended" | "extendedselection" => Ok(ListBoxSelectionMode::Extended),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[cfg(full_widgets)]
/// Parses a list-view display mode: `list`, `icon`, `details`, or
/// `thumbnails`.
///
/// A string payload is required; the match ignores case and separators.
pub fn expect_view_mode(value: CapabilityValue) -> Result<ViewMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "list" => Ok(ViewMode::List),
        "icon" => Ok(ViewMode::Icon),
        "details" => Ok(ViewMode::Details),
        "thumbnails" => Ok(ViewMode::Thumbnails),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[cfg(full_widgets)]
/// Parses a tool-bar orientation: `horizontal` or `vertical`.
///
/// Case- and separator-insensitive; accepts a string payload only.
pub fn expect_toolbar_orientation(
    value: CapabilityValue,
) -> Result<ToolBarOrientation, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "horizontal" => Ok(ToolBarOrientation::Horizontal),
        "vertical" => Ok(ToolBarOrientation::Vertical),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses an [`Alignment`]: `left`, `center` (or the British `centre`),
/// `right`, `top`, or `bottom`.
///
/// Case- and separator-insensitive; accepts a string payload only. Note that
/// the accepted horizontal and vertical values are not interchangeable: the
/// caller is responsible for picking one consistent with the widget.
pub fn expect_alignment(value: CapabilityValue) -> Result<Alignment, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "left" => Ok(Alignment::Left),
        "center" | "centre" => Ok(Alignment::Center),
        "right" => Ok(Alignment::Right),
        "top" => Ok(Alignment::Top),
        "bottom" => Ok(Alignment::Bottom),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses a tri-state [`CheckState`].
///
/// Accepts `unchecked`/`off`, `partiallychecked`/`partial`/`indeterminate`,
/// and `checked`/`on` (case- and separator-insensitive), matching the tokens
/// emitted by [`check_state_to_str`].
pub fn expect_check_state(value: CapabilityValue) -> Result<CheckState, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "unchecked" | "off" => Ok(CheckState::Unchecked),
        "partiallychecked" | "partial" | "indeterminate" => Ok(CheckState::PartiallyChecked),
        "checked" | "on" => Ok(CheckState::Checked),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses an [`Orientation`]: `horizontal` or `vertical`.
///
/// Case- and separator-insensitive; accepts a string payload only.
pub fn expect_orientation(value: CapabilityValue) -> Result<Orientation, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "horizontal" => Ok(Orientation::Horizontal),
        "vertical" => Ok(Orientation::Vertical),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses a [`TextDirection`]: `ltr`/`left_to_right` or `rtl`/`right_to_left`.
///
/// # Why both a short and a long spelling
///
/// `ltr`/`rtl` is what a designer or a locale table carries and what appears in the SVG snapshot
/// names, while `left_to_right`/`right_to_left` is what the enum variant reads as. Accepting one and
/// not the other would force every caller to know which convention this crate picked, so both are
/// taken and [`text_direction_to_str`] publishes the short one.
///
/// Case- and separator-insensitive; accepts a string payload only.
pub fn expect_text_direction(
    value: CapabilityValue,
) -> Result<TextDirection, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "ltr" | "lefttoright" => Ok(TextDirection::LeftToRight),
        "rtl" | "righttoleft" => Ok(TextDirection::RightToLeft),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// The spelling [`expect_text_direction`] publishes for a direction.
///
/// `get` and `set` have to agree about the token or a read → write → read round-trip would not
/// close, so both sides come from this pair rather than each holding its own literal.
pub const fn text_direction_to_str(direction: TextDirection) -> &'static str {
    match direction {
        TextDirection::LeftToRight => "ltr",
        TextDirection::RightToLeft => "rtl",
    }
}

#[cfg(full_widgets)]
/// Parses a slider [`TickPosition`].
///
/// Accepts `none`/`noticks`, `above`/`ticksabove`/`left`,
/// `below`/`ticksbelow`/`right`, and `both`/`ticksbothsides`, so the tokens
/// also work for the vertical orientation where "left"/"right" are the
/// spelling used. Case- and separator-insensitive; a string payload is
/// required.
pub fn expect_tick_position(value: CapabilityValue) -> Result<TickPosition, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "noticks" => Ok(TickPosition::NoTicks),
        "above" | "ticksabove" | "left" => Ok(TickPosition::TicksAbove),
        "below" | "ticksbelow" | "right" => Ok(TickPosition::TicksBelow),
        "both" | "ticksbothsides" => Ok(TickPosition::TicksBothSides),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[cfg(widgets_unstripped)]
/// Parses an [`LCDNumberMode`]: `hex`, `dec`/`decimal`, `oct`/`octal`, or
/// `bin`/`binary`.
///
/// Case- and separator-insensitive; the tokens match those produced by
/// [`lcd_mode_to_str`].
pub fn expect_lcd_mode(value: CapabilityValue) -> Result<LCDNumberMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "hex" => Ok(LCDNumberMode::Hex),
        "dec" | "decimal" => Ok(LCDNumberMode::Dec),
        "oct" | "octal" => Ok(LCDNumberMode::Oct),
        "bin" | "binary" => Ok(LCDNumberMode::Bin),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[cfg(widgets_unstripped)]
/// Parses a [`SegmentStyle`]: `outline`, `filled`, or `flat`.
///
/// Case- and separator-insensitive; the tokens match those produced by
/// [`segment_style_to_str`].
pub fn expect_segment_style(value: CapabilityValue) -> Result<SegmentStyle, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "outline" => Ok(SegmentStyle::Outline),
        "filled" => Ok(SegmentStyle::Filled),
        "flat" => Ok(SegmentStyle::Flat),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Parses a [`BadgeLevel`]: `info`, `success`, `warning`, or `error`.
///
/// Case- and separator-insensitive; the tokens match [`badge_level_to_str`].
///
/// `Badge`'s severity decides its colour, and it was reachable only through the
/// inherent `set_level` — a caller using the documented property route got
/// `UnknownProperty` for the property that explains the badge's appearance.
#[cfg(widgets_unstripped)]
pub fn expect_badge_level(value: CapabilityValue) -> Result<BadgeLevel, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "info" | "information" => Ok(BadgeLevel::Info),
        "success" | "ok" => Ok(BadgeLevel::Success),
        "warning" | "warn" => Ok(BadgeLevel::Warning),
        "error" | "danger" | "critical" => Ok(BadgeLevel::Error),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// The string spelling of a [`BadgeLevel`], the inverse of [`expect_badge_level`].
#[cfg(widgets_unstripped)]
pub fn badge_level_to_str(level: BadgeLevel) -> &'static str {
    match level {
        BadgeLevel::Info => "info",
        BadgeLevel::Success => "success",
        BadgeLevel::Warning => "warning",
        BadgeLevel::Error => "error",
    }
}

/// Parses a [`MessageBoxIcon`]: `none`, `information`, `question`, `warning`, or
/// `critical`.
///
/// Case- and separator-insensitive, and the tokens match those produced by
/// [`message_box_icon_to_str`]. The accepted spellings are the same ones the JSON
/// loader accepts for a message box's `icon` key, so a document and a property write
/// cannot disagree about what `"warning"` means.
///
/// # Why this exists
///
/// `MessageBox` has always had an icon — it is drawn, and `warning()` / `critical()`
/// construct it — but the property layer could not reach it: `get`/`set` did not
/// answer `"icon"`, `property_names` omitted it, and the capability table did not
/// declare it. A caller using the documented property route therefore got
/// `UnknownProperty` for a property the control plainly has, while the JSON loader
/// happily accepted it. This codec is the missing half of that pair.
#[cfg(full_widgets)]
pub fn expect_message_box_icon(
    value: CapabilityValue,
) -> Result<MessageBoxIcon, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "no_icon" | "nicon" => Ok(MessageBoxIcon::NoIcon),
        "information" | "info" => Ok(MessageBoxIcon::Information),
        "question" => Ok(MessageBoxIcon::Question),
        "warning" | "warn" => Ok(MessageBoxIcon::Warning),
        "critical" | "error" => Ok(MessageBoxIcon::Critical),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// The string spelling of a [`MessageBoxIcon`], the inverse of
/// [`expect_message_box_icon`].
///
/// The pair lives together so a control that can write an icon can always read it
/// back, and so the two spellings cannot drift apart (BLUE15 §2).
#[cfg(full_widgets)]
pub fn message_box_icon_to_str(icon: MessageBoxIcon) -> &'static str {
    match icon {
        MessageBoxIcon::NoIcon => "none",
        MessageBoxIcon::Information => "information",
        MessageBoxIcon::Question => "question",
        MessageBoxIcon::Warning => "warning",
        MessageBoxIcon::Critical => "critical",
    }
}

/// The string spelling of an [`LCDNumberMode`], the inverse of [`expect_lcd_mode`].
///
/// Lives beside its inverse rather than in the property-access module: the two are
/// a codec pair, and a control that can write a mode must be able to read it back
/// in every profile it exists in (BLUE15 §2, the `full_widgets` gate drift).
#[cfg(widgets_unstripped)]
pub fn lcd_mode_to_str(mode: LCDNumberMode) -> &'static str {
    match mode {
        LCDNumberMode::Hex => "hex",
        LCDNumberMode::Dec => "dec",
        LCDNumberMode::Oct => "oct",
        LCDNumberMode::Bin => "bin",
    }
}

/// The string spelling of a [`SegmentStyle`], the inverse of [`expect_segment_style`].
#[cfg(widgets_unstripped)]
pub fn segment_style_to_str(style: SegmentStyle) -> &'static str {
    match style {
        SegmentStyle::Outline => "outline",
        SegmentStyle::Filled => "filled",
        SegmentStyle::Flat => "flat",
    }
}

/// Formats a [`TickPosition`] as its published token.
///
/// The authoritative mapping lives beside the `Slider` widget that owns the type
/// (`display_widgets::slider::tick_position_to_str`), because `Slider` exists in
/// every profile while this module's other converters are gated. This delegates
/// rather than repeating the match, so the two cannot drift (principle #54).
#[cfg(widgets_unstripped)]
pub fn tick_position_to_str(tick_position: TickPosition) -> &'static str {
    crate::widget::display_widgets::slider::tick_position_to_str(tick_position)
}

// ---------------------------------------------------------------------------
// Other helpers
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
/// Formats a `chrono` date as `"YYYY-MM-DD"`, the inverse of
/// [`expect_naive_date`].
///
/// The year is not zero-padded beyond four digits, matching `chrono`'s
/// `%Y` specifier.
pub fn naive_date_to_string(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Normalises an enumeration token for matching.
///
/// Removes all underscores, hyphens, and spaces, then lower-cases the result,
/// so `"Partially_Checked"`, `"partially-checked"`, and `"PARTIALLY CHECKED"`
/// all become `"partiallychecked"`. Used by every string-based `expect_*`
/// parser, which is why separators in the accepted spellings do not matter.
pub fn normalize_key(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !matches!(*ch, '_' | '-' | ' '))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

// ── Value → stable string ───────────────────────────────────────────────────
//
// These conversions have no profile dependency: they map a *core* or *base*
// control enum to the lower-case token the property layer publishes. They live
// here, beside their `expect_*` inverses, so a control's `WidgetProperties::get`
// can format a value in every profile — `access` (which used to host them) is
// gated to device profiles, but `coercion` is not.

/// Formats an [`Alignment`] as its published token.
///
/// The inverse of [`expect_alignment`] for the canonical values; it emits
/// `"center"` (never `"centre"`).
pub const fn alignment_to_str(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Top => "top",
        Alignment::Bottom => "bottom",
    }
}

/// Formats a [`CheckState`] as its published token.
///
/// The inverse of [`expect_check_state`]; note the underscore in
/// `"partially_checked"`, which [`normalize_key`] strips on the way back in.
pub const fn check_state_to_str(state: CheckState) -> &'static str {
    match state {
        CheckState::Unchecked => "unchecked",
        CheckState::PartiallyChecked => "partially_checked",
        CheckState::Checked => "checked",
    }
}

/// Formats an [`Orientation`] as its published token.
///
/// The inverse of [`expect_orientation`].
pub const fn orientation_to_str(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    }
}
