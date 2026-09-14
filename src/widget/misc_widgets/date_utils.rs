// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Calendar tables and date arithmetic shared by the date-picker widgets.
//!
//! # Why this module exists
//!
//! Three pickers — [`MobileDatePicker`](super::mobile_date_picker::MobileDatePicker),
//! [`DateRangePicker`](super::date_range_picker::DateRangePicker) and
//! [`CupertinoDatePicker`](crate::widget::cupertino::date_picker::CupertinoDatePicker)
//! — each render a month/day wheel and each parse an ISO date off the property
//! contract. All three had grown their own copy of the month names, the leap-year
//! rule and the ISO parser: the same three facts stated in three places, which is
//! how a leap-year bug ends up fixed in only one picker.
//!
//! The tables and rules are stated once here. They are deliberately free functions
//! and `const` slices rather than a `Date` type: the pickers store dates as loose
//! `(year, month, day)` components, and introducing a type would mean rewriting
//! their state for no gain.

/// Three-letter English month names, `Jan` through `Dec`.
///
/// Indexed by `month - 1`. Callers must clamp before indexing; use
/// [`month_name`] when the value is not already known to be in range.
pub const MONTH_NAMES: &[&str] =
    &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Two-letter day-of-week names starting at Sunday.
pub const DAY_NAMES: &[&str] = &["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

/// Zero-padded day numbers `"01"` through `"31"`.
///
/// Pre-formatted so a draw call can slice it instead of formatting a `String` per
/// frame.
pub const DAY_STRINGS: &[&str] = &[
    "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
];

/// Returns the number of days in `month` of `year`, accounting for leap years.
///
/// Months outside `1..=12` return `30`, the middle value, so a caller that has not
/// validated its month cannot render a wildly wrong wheel.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Returns whether `year` is a Gregorian leap year.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Returns the three-letter name of `month`, or the empty string when out of range.
pub fn month_name(month: u32) -> &'static str {
    match month {
        1..=12 => MONTH_NAMES[(month - 1) as usize],
        _ => "",
    }
}

/// The shape an ISO date must have to be accepted by [`parse_iso_date`].
///
/// Returned as an enum rather than a bare `Option` so a caller can distinguish
/// "not a date at all" from "a date, but the day is out of range for the month" —
/// the two demand different error messages in a property contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsoDateError {
    /// The text is not three `-`-separated numeric fields.
    Malformed,
    /// The month was outside `1..=12`.
    MonthOutOfRange,
    /// The day was outside `1..=days_in_month(year, month)`.
    DayOutOfRange,
}

/// Parses an ISO `YYYY-MM-DD` date into `(year, month, day)`.
///
/// Returns `None` when the text does not match that shape or the components are
/// not a real calendar date. Use [`parse_iso_date_checked`] when the caller needs
/// to know *why* it was rejected.
pub fn parse_iso_date(text: &str) -> Option<(i32, u32, u32)> {
    parse_iso_date_checked(text).ok()
}

/// [`parse_iso_date`], reporting the reason for rejection.
///
/// The day is validated against the real length of the month, so `2026-02-30` and
/// `2025-02-29` are both rejected rather than silently clamped into a
/// valid-looking date.
pub fn parse_iso_date_checked(text: &str) -> Result<(i32, u32, u32), IsoDateError> {
    let mut parts = text.split('-');
    let year = parts.next().and_then(|v| v.parse::<i32>().ok()).ok_or(IsoDateError::Malformed)?;
    let month = parts.next().and_then(|v| v.parse::<u32>().ok()).ok_or(IsoDateError::Malformed)?;
    let day = parts.next().and_then(|v| v.parse::<u32>().ok()).ok_or(IsoDateError::Malformed)?;
    if parts.next().is_some() {
        return Err(IsoDateError::Malformed);
    }
    if !(1..=12).contains(&month) {
        return Err(IsoDateError::MonthOutOfRange);
    }
    if !(1..=days_in_month(year, month)).contains(&day) {
        return Err(IsoDateError::DayOutOfRange);
    }
    Ok((year, month, day))
}

/// Formats `(year, month, day)` as a zero-padded ISO `YYYY-MM-DD` string.
pub fn format_iso_date(year: i32, month: u32, day: u32) -> alloc::string::String {
    alloc::format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2025));
    }

    #[test]
    fn february_length_follows_the_leap_rule() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
    }

    #[test]
    fn month_names_are_one_based_and_bounded() {
        assert_eq!(month_name(1), "Jan");
        assert_eq!(month_name(12), "Dec");
        assert_eq!(month_name(0), "");
        assert_eq!(month_name(13), "");
        assert_eq!(MONTH_NAMES.len(), 12);
    }

    #[test]
    fn day_strings_cover_every_month_length() {
        assert_eq!(DAY_STRINGS.len(), 31);
        assert_eq!(DAY_STRINGS[0], "01");
        assert_eq!(DAY_STRINGS[30], "31");
    }

    #[test]
    fn iso_round_trip() {
        assert_eq!(parse_iso_date("2026-09-14"), Some((2026, 9, 14)));
        assert_eq!(format_iso_date(2026, 9, 14), "2026-09-14");
        assert_eq!(format_iso_date(7, 1, 2), "0007-01-02");
    }

    #[test]
    fn iso_rejects_bad_shapes_and_impossible_dates() {
        assert_eq!(parse_iso_date("not-a-date"), None);
        assert_eq!(parse_iso_date("2026-09"), None);
        assert_eq!(parse_iso_date("2026-09-14-1"), None);
        assert_eq!(parse_iso_date_checked("2026-13-01"), Err(IsoDateError::MonthOutOfRange));
        assert_eq!(parse_iso_date_checked("2026-00-01"), Err(IsoDateError::MonthOutOfRange));
        assert_eq!(parse_iso_date_checked("2026-02-30"), Err(IsoDateError::DayOutOfRange));
        // 2025 is not a leap year, so 29 February does not exist in it.
        assert_eq!(parse_iso_date_checked("2025-02-29"), Err(IsoDateError::DayOutOfRange));
        assert_eq!(parse_iso_date_checked("2024-02-29"), Ok((2024, 2, 29)));
    }
}
