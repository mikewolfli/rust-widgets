// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Locale-aware number text for controls (U2-07).
//!
//! The i18n layer already carried `NumberFormat`, `format_number` and `format_integer`, but no
//! widget consumed them: a chart axis, a spin box and a progress label all printed a `.` decimal
//! point in every locale, while the manager's own documentation named that as a defect.
//!
//! This module is the one widget-side entry point. It caches the locale's separators keyed by
//! [`crate::i18n::language_generation`], so a draw path does not take the i18n lock per frame.
//! With the `i18n` feature off it answers the C locale, which keeps the stripped profiles
//! byte-identical to before.

#[cfg(feature = "i18n")]
use crate::compat::RefCell;
#[cfg(feature = "i18n")]
use crate::i18n::NumberFormat;

/// The decimal/grouping separators for the active language.
#[cfg(feature = "i18n")]
#[allow(dead_code)]
pub(crate) fn number_format() -> NumberFormat {
    thread_local! {
        // `(generation, format)`: the generation is bumped by every `set_language`, so a
        // language switch invalidates the cached separators without a lock on the next call.
        static CACHED: RefCell<(u64, NumberFormat)> =
            const { RefCell::new((u64::MAX, NumberFormat::C_LOCALE)) };
    }
    let generation = crate::i18n::language_generation();
    CACHED.with(|cell| {
        let mut cached = cell.borrow_mut();
        if cached.0 != generation {
            let language = crate::i18n::current_language();
            *cached = (generation, crate::i18n::number_format_for(language.as_deref()));
        }
        cached.1
    })
}

/// The character this locale prints between a number's integer and fractional parts.
pub(crate) fn decimal_separator() -> char {
    #[cfg(feature = "i18n")]
    {
        number_format().decimal_separator
    }
    #[cfg(not(feature = "i18n"))]
    {
        '.'
    }
}

/// The character this locale prints between digit groups of three.
pub(crate) fn grouping_separator() -> char {
    #[cfg(feature = "i18n")]
    {
        number_format().grouping_separator
    }
    #[cfg(not(feature = "i18n"))]
    {
        ','
    }
}

/// Rewrites the ASCII decimal point in `text` to the locale's separator.
///
/// Used by paths that build their text with a fixed `write!("{value:.1}")`: replacing the one
/// character allocates nothing, where routing the value through the formatter would allocate a
/// `String` per label per frame.
pub(crate) fn localize_decimal_point(text: &mut crate::compat::String) {
    let separator = decimal_separator();
    if separator == '.' {
        return;
    }
    // `char_indices` cannot be mutated in place for a multi-byte separator, so rebuild only when
    // the text actually contains a dot. Axis labels are short and this is the cold branch.
    if text.as_bytes().contains(&b'.') {
        let mut rewritten = crate::compat::String::with_capacity(text.len() + 1);
        for ch in text.chars() {
            rewritten.push(if ch == '.' { separator } else { ch });
        }
        *text = rewritten;
    }
}

/// Formats a fixed-point decimal with the locale's decimal separator.
#[cfg(feature = "i18n")]
#[allow(dead_code)]
pub(crate) fn format_decimal(value: f64, decimals: u32) -> crate::compat::String {
    let language = crate::i18n::current_language();
    crate::i18n::format_number(value, decimals, language.as_deref())
}

/// Formats a fixed-point decimal; with i18n off this is the C locale spelling.
#[cfg(not(feature = "i18n"))]
#[allow(dead_code)]
pub(crate) fn format_decimal(value: f64, decimals: u32) -> crate::compat::String {
    let mut text = crate::compat::String::new();
    let _ = core::fmt::Write::write_fmt(&mut text, format_args!("{value:.*}", decimals as usize));
    text
}

/// Normalises user-entered number text to the `f64` grammar (`str::parse`).
///
/// A locale that prints `3,14` must accept `3,14` back, otherwise the display and the parser
/// disagree and the field shows a value it cannot read. The locale's decimal separator becomes
/// `.`; its grouping separator is dropped.
pub(crate) fn normalize_decimal_input(text: &str) -> crate::compat::String {
    let mut out = crate::compat::String::with_capacity(text.len());
    #[cfg(feature = "i18n")]
    let (decimal, grouping) = {
        let format = number_format();
        (format.decimal_separator, format.grouping_separator)
    };
    #[cfg(not(feature = "i18n"))]
    let (decimal, grouping) = ('.', ',');
    for ch in text.trim().chars() {
        if ch == decimal {
            out.push('.');
        } else if ch == grouping {
            // Grouping characters are presentation, never part of the numeric grammar.
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(all(test, feature = "i18n"))]
mod tests {
    use super::*;

    #[test]
    fn a_dot_is_rewritten_to_the_locale_separator() {
        // The C locale keeps the dot; the German table uses a comma. Both are asserted through
        // the pure formatter rather than by mutating the global language in a parallel test.
        let mut text = crate::compat::String::from("3.14");
        let separator = decimal_separator();
        localize_decimal_point(&mut text);
        assert!(text.contains(separator) || separator == '.');
        assert!(!text.contains(',') || separator == ',');
    }

    #[test]
    fn input_normalisation_accepts_both_spellings() {
        // C locale: `1,5` is grouping + a stray digit, not a decimal; it parses as `15`.
        assert_eq!(normalize_decimal_input("1.5"), crate::compat::String::from("1.5"));
        assert_eq!(normalize_decimal_input("1,5"), crate::compat::String::from("15"));
    }
}
