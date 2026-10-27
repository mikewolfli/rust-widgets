// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The input-mask grammar: parse, sanitise, format.
//!
//! # Why this is its own module (I-011)
//!
//! Two controls drive the same grammar: the dedicated [`MaskedEdit`](super::masked_edit) control
//! and the single-line [`LineEdit`](super::lineedit)'s `set_input_mask`. The engine is a *value
//! type* — no widget, no `WidgetKind`, no runtime — so keeping it inside the masked widget made
//! `LineEdit` (which the `mini`/`embedded` profiles do keep) unable to compile unless the whole
//! masked **widget** was compiled in too, which the profile's widget census deliberately excludes.
//! Parsing and formatting are the same code both controls use, so the two cannot drift.

use crate::compat::{String, Vec};

/// How case converters (`>`, `<`, `!`) transform subsequent letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaseMode {
    /// Leave the typed letter as it is.
    None,
    /// `>` — fold the letter to upper case.
    Upper,
    /// `<` — fold the letter to lower case.
    Lower,
}

/// A parsed segment in the mask — either a literal character or an input placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskSegment {
    /// A literal character that appears as-is (e.g., `(`, `)`, `-`, space).
    Literal { ch: char },
    /// A user-input position with a mask character and its case converter.
    Input { kind: MaskCharKind, case: CaseMode },
}

/// The kind of input allowed at a mask position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskCharKind {
    /// `0` — required digit.
    RequiredDigit,
    /// `9` — optional digit.
    OptionalDigit,
    /// `A` — required letter.
    RequiredLetter,
    /// `a` — optional letter.
    OptionalLetter,
    /// `X` — required alphanumeric.
    RequiredAlphaNum,
    /// `x` — optional alphanumeric.
    OptionalAlphaNum,
    /// `D` — required digit (locale digits allowed).
    RequiredNumeric,
    /// `d` — optional digit (locale digits allowed).
    OptionalNumeric,
    /// `#` — required digit or sign.
    RequiredSignDigit,
    /// `H` — required hexadecimal digit.
    RequiredHex,
    /// `h` — optional hexadecimal digit.
    OptionalHex,
    /// `B` — required binary digit.
    RequiredBinary,
    /// `b` — optional binary digit.
    OptionalBinary,
}

/// A parsed mask, shared with the single-line field (I-011).
///
/// `MaskedEdit` is a dedicated control; `LineEdit` needs the same grammar without the separate
/// widget, so the engine is exposed as a value type instead of being re-implemented. Parsing and
/// formatting are the same code the masked control uses, so the two cannot drift.
#[derive(Debug, Clone)]
pub(crate) struct MaskEngine {
    segments: Vec<MaskSegment>,
    placeholder: char,
}

impl MaskEngine {
    /// Parses `mask`, or returns `None` for an empty pattern.
    pub(crate) fn parse(mask: &str) -> Option<Self> {
        if mask.is_empty() {
            return None;
        }
        let (segments, placeholder) = parse_mask(mask);
        (!segments.is_empty()).then_some(Self { segments, placeholder })
    }

    /// Formats `raw` through the mask, filling unused cells with the placeholder.
    pub(crate) fn format(&self, raw: &str) -> String {
        build_display_text(&self.segments, raw, self.placeholder)
    }

    /// Keeps only the characters the mask can place, in cell order and with case converters applied.
    pub(crate) fn sanitize(&self, text: &str) -> String {
        let mut raw = String::new();
        let mut chars = text.chars();
        for segment in &self.segments {
            if let MaskSegment::Input { kind, case } = segment {
                for ch in chars.by_ref() {
                    let folded = apply_case(*case, ch);
                    if mask_char_matches(*kind, folded) {
                        raw.push(folded);
                        break;
                    }
                }
            }
        }
        raw
    }
}

/// Checks whether a character matches a mask character kind.
pub(crate) fn mask_char_matches(kind: MaskCharKind, ch: char) -> bool {
    match kind {
        MaskCharKind::RequiredDigit | MaskCharKind::OptionalDigit => ch.is_ascii_digit(),
        MaskCharKind::RequiredLetter | MaskCharKind::OptionalLetter => ch.is_ascii_alphabetic(),
        MaskCharKind::RequiredAlphaNum | MaskCharKind::OptionalAlphaNum => {
            ch.is_ascii_alphanumeric()
        }
        MaskCharKind::RequiredNumeric | MaskCharKind::OptionalNumeric => ch.is_numeric(),
        MaskCharKind::RequiredSignDigit => ch.is_ascii_digit() || ch == '+' || ch == '-',
        MaskCharKind::RequiredHex | MaskCharKind::OptionalHex => ch.is_ascii_hexdigit(),
        MaskCharKind::RequiredBinary | MaskCharKind::OptionalBinary => ch == '0' || ch == '1',
    }
}

/// Applies a mask case converter to a character.
///
/// Only letters are folded; a digit or a literal is returned unchanged, so `>` in a numeric mask
/// does not have surprising effects on the digits around it.
pub(crate) fn apply_case(case: CaseMode, ch: char) -> char {
    match case {
        CaseMode::None => ch,
        CaseMode::Upper => ch.to_uppercase().next().unwrap_or(ch),
        CaseMode::Lower => ch.to_lowercase().next().unwrap_or(ch),
    }
}

/// Parses a mask string into its segments and placeholder.
///
/// Supported input characters: `0`/`9` (digits), `A`/`a` (letters), `X`/`x` (alphanumerics),
/// `D`/`d` (numeric), `#` (digit or sign), `H`/`h` (hex), `B`/`b` (binary). `\\` escapes the next
/// character as a literal, `>`/`<` fold subsequent letters to upper/lower case, `!` turns the
/// conversion off, and everything after an unescaped `;` is the placeholder character for empty
/// cells (the first character after it; the default is `_`).
pub(crate) fn parse_mask(mask: &str) -> (Vec<MaskSegment>, char) {
    let mut segments = Vec::new();
    let mut case = CaseMode::None;
    let mut chars = mask.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ';' => {
                // The tail is metadata: the first character is the placeholder for empty cells.
                let placeholder = chars.next().unwrap_or('_');
                return (segments, placeholder);
            }
            '\\' => {
                if let Some(literal) = chars.next() {
                    segments.push(MaskSegment::Literal { ch: literal });
                }
            }
            '>' => case = CaseMode::Upper,
            '<' => case = CaseMode::Lower,
            '!' => case = CaseMode::None,
            '0' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredDigit, case }),
            '9' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalDigit, case }),
            'A' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredLetter, case }),
            'a' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalLetter, case }),
            'X' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredAlphaNum, case }),
            'x' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalAlphaNum, case }),
            'D' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredNumeric, case }),
            'd' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalNumeric, case }),
            '#' => {
                segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredSignDigit, case })
            }
            'H' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredHex, case }),
            'h' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalHex, case }),
            'B' => segments.push(MaskSegment::Input { kind: MaskCharKind::RequiredBinary, case }),
            'b' => segments.push(MaskSegment::Input { kind: MaskCharKind::OptionalBinary, case }),
            _ => segments.push(MaskSegment::Literal { ch }),
        }
    }
    (segments, '_')
}

/// Builds the display string from segments and raw text.
pub(crate) fn build_display_text(
    segments: &[MaskSegment],
    raw_text: &str,
    placeholder: char,
) -> String {
    let mut result = String::new();
    let mut raw_idx = 0;
    let raw_chars: Vec<char> = raw_text.chars().collect();

    for seg in segments {
        match seg {
            MaskSegment::Literal { ch } => {
                result.push(*ch);
            }
            MaskSegment::Input { .. } => {
                if raw_idx < raw_chars.len() {
                    result.push(raw_chars[raw_idx]);
                    raw_idx += 1;
                } else {
                    result.push(placeholder);
                }
            }
        }
    }
    result
}
