// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-coercion helpers for the capability layer.
//!
//! These functions convert [`CapabilityValue`] variants into concrete Rust types,
//! performing string parsing and numeric coercion as needed.

#[cfg(full_widgets)]
use chrono::{NaiveDate, Weekday};

use super::CapabilityAccessError;
use super::CapabilityValue;
use crate::core::{Alignment, Orientation};
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::time_edit::Time;
use crate::widget::base_widgets::checkbox::CheckState;
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

pub fn widget_as<T: Widget + 'static>(widget: &dyn Widget) -> Option<&T> {
    (widget as &dyn std::any::Any).downcast_ref::<T>()
}

pub fn widget_as_mut<T: Widget + 'static>(widget: &mut dyn Widget) -> Option<&mut T> {
    (widget as &mut dyn std::any::Any).downcast_mut::<T>()
}

// ---------------------------------------------------------------------------
// Primitive value extractors
// ---------------------------------------------------------------------------

pub fn expect_bool(value: CapabilityValue) -> Result<bool, CapabilityAccessError> {
    match value {
        CapabilityValue::Bool(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub fn expect_string(value: CapabilityValue) -> Result<String, CapabilityAccessError> {
    match value {
        CapabilityValue::String(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

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

pub fn expect_f32(value: CapabilityValue) -> Result<f32, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v as f32),
        CapabilityValue::UInt(v) => Ok(v as f32),
        CapabilityValue::Int(v) => Ok(v as f32),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub fn expect_f64(value: CapabilityValue) -> Result<f64, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v),
        CapabilityValue::Int(v) => Ok(v as f64),
        CapabilityValue::UInt(v) => Ok(v as f64),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub fn expect_i64(value: CapabilityValue) -> Result<i64, CapabilityAccessError> {
    match value {
        CapabilityValue::Int(v) => Ok(v),
        CapabilityValue::UInt(v) => {
            i64::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

pub fn expect_u32(value: CapabilityValue) -> Result<u32, CapabilityAccessError> {
    let raw = expect_usize(value)?;
    u32::try_from(raw).map_err(|_| CapabilityAccessError::TypeMismatch)
}

// ---------------------------------------------------------------------------
// Date / time extractors
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
pub fn expect_naive_date(value: CapabilityValue) -> Result<NaiveDate, CapabilityAccessError> {
    let text = expect_string(value)?;
    NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(|_| CapabilityAccessError::TypeMismatch)
}

#[cfg(full_widgets)]
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

#[cfg(full_widgets)]
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
pub fn naive_date_to_string(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

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
pub const fn check_state_to_str(state: CheckState) -> &'static str {
    match state {
        CheckState::Unchecked => "unchecked",
        CheckState::PartiallyChecked => "partially_checked",
        CheckState::Checked => "checked",
    }
}

/// Formats an [`Orientation`] as its published token.
pub const fn orientation_to_str(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Horizontal => "horizontal",
        Orientation::Vertical => "vertical",
    }
}
