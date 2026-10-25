// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rich text rendering — supports multiple fonts, colors, and styles in one text block.
//!
//! A `RichText` is a sequence of `TextSpan` values, each with its own `TextStyle`.
//! Use the `add_span` / `add_text` builders to compose styled text, then call
//! `measure()` to obtain the overall dimensions.

use core::cell::RefCell;

use crate::compat::{MiniToString, String, Vec};
use crate::core::Color;
use crate::render::text_shaper::TextShaper;

/// Text style for rich text spans.
///
/// Controls font family, size, color, and typographic emphasis.
#[derive(Debug, Clone)]
pub struct TextStyle {
    /// Font family name (e.g. "Arial", "sans-serif").
    pub font_family: String,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Text color.
    pub color: Color,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text is underlined.
    pub underline: bool,
    /// Whether the text has a strikethrough line.
    pub strikethrough: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: String::from("Arial"),
            font_size: 14.0,
            color: Color::BLACK,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

/// A span of text with a specific style.
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// The text content of this span.
    pub text: String,
    /// The style applied to this span.
    pub style: TextStyle,
}

/// A rich text block composed of multiple styled spans.
///
/// `RichText` aggregates `TextSpan` values laid out sequentially as they
/// were added.  Use `measure()` to compute total width and height.
///
/// # Example
///
/// ```
/// use rust_widgets::render::rich_text::{RichText, TextStyle};
/// use rust_widgets::core::Color;
///
/// let mut rt = RichText::new();
/// rt.add_text("Hello ");
/// let mut bold = TextStyle::default();
/// bold.bold = true;
/// bold.color = Color::RED;
/// rt.add_span("World", bold);
/// assert!(!rt.is_empty());
/// assert_eq!(rt.plain_text(), "Hello World");
/// ```
#[derive(Debug, Clone, Default)]
pub struct RichText {
    /// The ordered list of text spans composing this rich text block.
    pub spans: Vec<TextSpan>,
    /// Memo for the last [`RichText::measure`] call: `(key, (width, height))`.
    ///
    /// The key is the shaper's [`TextShaper::cache_identity`] combined with a fingerprint of
    /// `spans`, so mutating the public span list invalidates the result without any setter (G-17).
    measure_cache: RefCell<Option<(u64, (f32, f32))>>,
}

impl RichText {
    /// Fingerprints everything `measure` reads: every span's text and style, in order.
    fn measure_fingerprint(&self) -> u64 {
        // FNV-1a. A short label is tens of bytes, so this is far cheaper than measuring it, and it
        // needs no hasher allocation on any profile.
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for span in &self.spans {
            mix(span.text.as_bytes());
            // A separator keeps `("ab", "c")` from hashing like `("a", "bc")`.
            mix(&[0xff]);
            let style = &span.style;
            mix(style.font_family.as_bytes());
            mix(&style.font_size.to_bits().to_le_bytes());
            mix(&[style.color.r, style.color.g, style.color.b, style.color.a]);
            mix(&[
                u8::from(style.bold),
                u8::from(style.italic),
                u8::from(style.underline),
                u8::from(style.strikethrough),
            ]);
        }
        hash
    }
}

impl RichText {
    /// Creates a new empty `RichText`.
    pub fn new() -> Self {
        Self { spans: Vec::new(), measure_cache: RefCell::new(None) }
    }

    /// Adds a span of text with the given style.
    pub fn add_span(&mut self, text: &str, style: TextStyle) {
        if text.is_empty() {
            return;
        }
        // Coalesce with the previous span if the style is identical.
        if let Some(last) = self.spans.last_mut() {
            if last.style.font_family == style.font_family
                && (last.style.font_size - style.font_size).abs() <= f32::EPSILON
                && last.style.color == style.color
                && last.style.bold == style.bold
                && last.style.italic == style.italic
                && last.style.underline == style.underline
                && last.style.strikethrough == style.strikethrough
            {
                last.text.push_str(text);
                return;
            }
        }
        self.spans.push(TextSpan { text: text.to_string(), style });
    }

    /// Adds a span of text with the default style.
    pub fn add_text(&mut self, text: &str) {
        self.add_span(text, TextStyle::default());
    }

    /// Returns `true` if the rich text block contains no spans.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Returns the concatenated plain text of all spans.
    pub fn plain_text(&self) -> String {
        let mut out = String::with_capacity(self.spans.iter().map(|s| s.text.len()).sum());
        for span in &self.spans {
            out.push_str(&span.text);
        }
        out
    }

    /// Measures the total width and height of the rich text block using the given shaper.
    ///
    /// Returns `(width, height)` in logical pixels.  Height is the maximum
    /// line height across all spans; width is the sum of the span widths.
    pub fn measure(&self, shaper: &dyn TextShaper) -> (f32, f32) {
        // A shaper that cannot describe its state returns `0`; measuring unconditionally is the
        // only correct answer for it, so the memo is bypassed rather than risk a wrong hit.
        let identity = shaper.cache_identity();
        let key =
            if identity == 0 { 0 } else { identity ^ self.measure_fingerprint().rotate_left(17) };
        if key != 0 {
            if let Some((cached_key, result)) = *self.measure_cache.borrow() {
                if cached_key == key {
                    return result;
                }
            }
        }

        let mut total_width = 0.0f32;
        let mut max_height = 0.0f32;
        for span in &self.spans {
            total_width += shaper.measure_width(&span.text, span.style.font_size);
            let span_height = shaper.measure_height(&span.text, span.style.font_size);
            if span_height > max_height {
                max_height = span_height;
            }
        }
        let result = (total_width, max_height);
        if key != 0 {
            *self.measure_cache.borrow_mut() = Some((key, result));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text_shaper::SimpleTextShaper;

    #[test]
    fn test_rich_text_empty() {
        let rt = RichText::new();
        assert!(rt.is_empty());
        assert_eq!(rt.plain_text(), "");
    }

    #[test]
    fn test_rich_text_add_text() {
        let mut rt = RichText::new();
        rt.add_text("Hello");
        rt.add_text(" World");
        assert_eq!(rt.spans.len(), 1); // coalesced
        assert_eq!(rt.plain_text(), "Hello World");
    }

    #[test]
    fn test_rich_text_add_span_with_different_style() {
        let mut rt = RichText::new();
        rt.add_text("Normal ");
        let style = TextStyle { bold: true, font_size: 18.0, ..Default::default() };
        rt.add_span("Bold", style);
        assert_eq!(rt.spans.len(), 2);
        assert_eq!(rt.plain_text(), "Normal Bold");
    }

    #[test]
    fn test_rich_text_measure_returns_non_zero() {
        let shaper = SimpleTextShaper::new();
        let mut rt = RichText::new();
        rt.add_text("Hello");
        let (w, h) = rt.measure(&shaper);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_rich_text_add_empty_span_does_nothing() {
        let mut rt = RichText::new();
        rt.add_span("", TextStyle::default());
        assert!(rt.is_empty());
    }

    #[test]
    fn test_rich_text_measure_with_multiple_spans() {
        let shaper = SimpleTextShaper::new();
        let mut rt = RichText::new();
        rt.add_text("A");
        let big = TextStyle { font_size: 28.0, ..Default::default() };
        rt.add_span("B", big);
        let (w, h) = rt.measure(&shaper);
        // Height should come from the bigger font size span
        assert!((h - 33.6).abs() < 0.01); // 28 * 1.2
                                          // Width is sum: A at 14pt + B at 28pt
        assert!(w > 0.0);
    }

    /// A repeat measure reuses the memo; touching any span invalidates it (G-17).
    #[test]
    fn measure_is_memoised_and_invalidated_by_a_span_change() {
        use crate::render::ShapedGlyphRun;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingShaper {
            calls: AtomicUsize,
        }
        impl TextShaper for CountingShaper {
            fn shape(&self, _text: &str, _font_size: f32) -> Vec<ShapedGlyphRun> {
                Vec::new()
            }
            fn measure_width(&self, text: &str, font_size: f32) -> f32 {
                self.calls.fetch_add(1, Ordering::Relaxed);
                text.len() as f32 * font_size
            }
            fn measure_height(&self, _text: &str, font_size: f32) -> f32 {
                font_size
            }
            fn char_advance(&self, _c: char, font_size: f32) -> f32 {
                font_size
            }
            fn cache_identity(&self) -> u64 {
                0x434f_554e_5449_4e47
            }
        }

        let shaper = CountingShaper { calls: AtomicUsize::new(0) };
        let mut rich = RichText::new();
        rich.add_text("Hello");

        let first = rich.measure(&shaper);
        assert_eq!(shaper.calls.load(Ordering::Relaxed), 1);
        assert_eq!(rich.measure(&shaper), first, "an unchanged block must reuse the memo");
        assert_eq!(shaper.calls.load(Ordering::Relaxed), 1, "the second measure must not reshape");

        rich.spans[0].text.push('!');
        let changed = rich.measure(&shaper);
        assert_eq!(shaper.calls.load(Ordering::Relaxed), 2, "a changed span must be re-measured");
        assert!(changed.0 > first.0);
    }

    #[test]
    fn test_rich_text_default_style_is_sensible() {
        let style = TextStyle::default();
        assert_eq!(style.font_family, "Arial");
        assert!((style.font_size - 14.0).abs() < f32::EPSILON);
        assert_eq!(style.color, Color::BLACK);
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.underline);
        assert!(!style.strikethrough);
    }
}
