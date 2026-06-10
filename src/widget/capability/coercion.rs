//! Type-coercion helpers for the capability layer.
//!
//! These functions convert [`CapabilityValue`] variants into concrete Rust types,
//! performing string parsing and numeric coercion as needed.

use chrono::{NaiveDate, Weekday};

use super::CapabilityAccessError;
use super::CapabilityValue;
use crate::core::{Alignment, Orientation};
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(not(feature = "mini"))]
use crate::widget::advanced_widgets::time_edit::Time;
use crate::widget::base_widgets::checkbox::CheckState;
#[cfg(not(feature = "mini"))]
use crate::widget::display_widgets::lcd_number::{LCDNumberMode, SegmentStyle};
#[cfg(not(feature = "mini"))]
use crate::widget::display_widgets::slider::TickPosition;
use crate::widget::input_widgets::listbox::SelectionMode as ListBoxSelectionMode;
#[cfg(not(feature = "mini"))]
use crate::widget::menu_toolbar::tool_bar::ToolBarOrientation;
#[cfg(not(feature = "mini"))]
use crate::widget::view_widgets::data_grid::{ColumnFilter, SortSpec};
#[cfg(not(feature = "mini"))]
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

pub fn expect_naive_date(value: CapabilityValue) -> Result<NaiveDate, CapabilityAccessError> {
    let text = expect_string(value)?;
    NaiveDate::parse_from_str(&text, "%Y-%m-%d").map_err(|_| CapabilityAccessError::TypeMismatch)
}

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

    match token.as_str() {
        "none" | "noselection" => Ok(ListBoxSelectionMode::NoSelection),
        "single" | "singleselection" => Ok(ListBoxSelectionMode::SingleSelection),
        "multi" | "multiselection" => Ok(ListBoxSelectionMode::MultiSelection),
        "extended" | "extendedselection" => Ok(ListBoxSelectionMode::ExtendedSelection),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

#[cfg(not(feature = "mini"))]
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

// ---------------------------------------------------------------------------
// Other helpers
// ---------------------------------------------------------------------------

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
