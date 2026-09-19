// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{format, String};

/// Font descriptor used by text rendering and themes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "FontSerde"))]
pub struct Font {
    /// Font family name.
    family: String,
    /// Font point size.
    size: f32,
    /// Font weight in CSS-like scale (100..=900).
    #[cfg_attr(feature = "serde", serde(default = "Font::default_weight"))]
    weight: u16,
    /// Whether bold style is requested.
    bold: bool,
    /// Whether italic style is requested.
    italic: bool,
}
impl Font {
    /// Returns the font family name.
    pub fn family(&self) -> &str {
        &self.family
    }
    /// Returns the font point size.
    pub fn size(&self) -> f32 {
        self.size
    }
    /// Returns the font weight in CSS-like scale (100..=900).
    pub fn weight(&self) -> u16 {
        self.weight
    }
    /// Returns whether italic style is requested.
    pub fn is_italic(&self) -> bool {
        self.italic
    }
    /// Sets the font point size (mutable setter for CSS parser integration).
    pub fn set_size(&mut self, size: f32) -> &mut Self {
        self.size = size;
        self
    }
    /// Sets the font family (mutable setter for CSS parser integration).
    pub fn set_family(&mut self, family: impl Into<String>) -> &mut Self {
        self.family = family.into();
        self
    }
    /// Creates a `FontBuilder` for ergonomic construction.
    pub fn builder() -> FontBuilder {
        FontBuilder::new()
    }
}
impl Font {
    /// Shared regular text weight.
    pub const REGULAR_WEIGHT: u16 = 400;
    /// Shared bold text weight.
    pub const BOLD_WEIGHT: u16 = 700;
    /// Default weight used for backward-compatible deserialization.
    pub const fn default_weight() -> u16 {
        Self::REGULAR_WEIGHT
    }
    /// Creates a font descriptor.
    ///
    /// This compatibility constructor keeps existing call sites stable and
    /// derives `weight` from `bold` (`700` when bold, otherwise `400`).
    pub fn new(family: impl Into<String>, size: f32, bold: bool, italic: bool) -> Self {
        let weight = if bold { Self::BOLD_WEIGHT } else { Self::REGULAR_WEIGHT };
        Self::with_weight(family, size, weight, italic)
    }
    /// Creates a font descriptor with explicit weight.
    pub fn with_weight(family: impl Into<String>, size: f32, weight: u16, italic: bool) -> Self {
        let normalized_weight = Self::normalize_weight(weight);
        Self {
            family: family.into(),
            size,
            weight: normalized_weight,
            bold: normalized_weight >= Self::BOLD_WEIGHT,
            italic,
        }
    }
    /// Creates a font descriptor from i32 size.
    pub fn with_i32_size(family: impl Into<String>, size: i32, bold: bool, italic: bool) -> Self {
        Self::new(family, size as f32, bold, italic)
    }
    /// Creates a font descriptor from u32 size.
    pub fn with_u32_size(family: impl Into<String>, size: u32, bold: bool, italic: bool) -> Self {
        Self::new(family, size as f32, bold, italic)
    }
    /// Creates a font descriptor from f64 size.
    pub fn with_f64_size(family: impl Into<String>, size: f64, bold: bool, italic: bool) -> Self {
        Self::new(family, size as f32, bold, italic)
    }
    /// Creates a font descriptor with only family and size (regular, non-italic).
    pub fn simple(family: impl Into<String>, size: f32) -> Self {
        Self::new(family, size, false, false)
    }
    /// Creates a bold font descriptor.
    pub fn bold(family: impl Into<String>, size: f32) -> Self {
        Self::new(family, size, true, false)
    }
    /// Creates an italic font descriptor.
    pub fn italic(family: impl Into<String>, size: f32) -> Self {
        Self::new(family, size, false, true)
    }
    /// Creates a bold italic font descriptor.
    pub fn bold_italic(family: impl Into<String>, size: f32) -> Self {
        Self::new(family, size, true, true)
    }
    /// Creates a font descriptor from tuple (family, size).
    /// Use `simple` instead.
    #[deprecated(since = "0.7.0", note = "use `simple` instead")]
    pub fn from_tuple(family: impl Into<String>, size: f32) -> Self {
        Self::simple(family, size)
    }
    /// Creates a font descriptor from tuple (family, size, bold).
    pub fn from_tuple_with_bold(family: impl Into<String>, size: f32, bold: bool) -> Self {
        Self::new(family, size, bold, false)
    }
    /// Creates a font descriptor from tuple (family, size, bold, italic).
    pub fn from_full_tuple(family: impl Into<String>, size: f32, bold: bool, italic: bool) -> Self {
        Self::new(family, size, bold, italic)
    }
    /// Returns the shared default UI font descriptor.
    pub fn default_ui() -> Self {
        Self::with_weight("Arial", 14.0, Self::REGULAR_WEIGHT, false)
    }
    /// Returns a shared default bold UI descriptor.
    pub fn default_ui_bold() -> Self {
        Self::with_weight("Arial", 14.0, Self::BOLD_WEIGHT, false)
    }
    /// Normalizes arbitrary weight to nearest 100 in `[100, 900]`.
    pub const fn normalize_weight(weight: u16) -> u16 {
        let clamped = if weight < 100 {
            100
        } else if weight > 900 {
            900
        } else {
            weight
        };
        ((clamped + 50) / 100) * 100
    }
    /// Returns `true` if the font descriptor has a positive size and non-empty family.
    pub fn is_valid(&self) -> bool {
        !self.family.trim().is_empty()
            && self.size > 0.0
            && self.size.is_finite()
            && self.weight >= 100
            && self.weight <= 900
            && self.weight.is_multiple_of(100)
    }
    /// Parses a CSS-style shorthand font string.
    ///
    /// The accepted form is `"<family> <size>[ <style>...]"`, where the family may
    /// itself contain spaces:
    ///
    /// ```text
    /// "Monospace 20"          -> Monospace, 20, regular, upright
    /// "Segoe UI 14 bold"      -> Segoe UI, 14, bold, upright
    /// "Menlo 12 italic"       -> Menlo, 12, regular, italic
    /// "Menlo 12 bold italic"  -> Menlo, 12, bold, italic
    /// ```
    ///
    /// The size is the **last** token that parses as a positive number, so a family
    /// containing digits (`"Roboto 2020 12"`) resolves to family `"Roboto 2020"` and
    /// size `12` rather than treating `2020` as the size. Style tokens are recognised
    /// after the size. An empty family, a missing or non-positive size, or an
    /// unrecognised trailing token makes the whole string invalid and returns `None` —
    /// the caller then decides the fallback explicitly instead of silently receiving a
    /// default.
    ///
    /// This is the primitive the JSON loader previously assumed did not exist (its
    /// comment said "full font parsing can be added when `Font::from_string` or
    /// similar is available"), which is why a `fontdialog.value` was read and then
    /// discarded.
    pub fn parse(spec: &str) -> Option<Self> {
        let tokens: alloc::vec::Vec<&str> = spec.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }

        // Split into "everything before the size", the size, and "everything after".
        // Scanning from the end means a digit-bearing family stays intact.
        let mut size_index = None;
        let mut size = 0.0f32;
        for (index, token) in tokens.iter().enumerate().rev() {
            if let Ok(parsed) = token.parse::<f32>() {
                if parsed > 0.0 && parsed.is_finite() {
                    size_index = Some(index);
                    size = parsed;
                    break;
                }
            }
        }
        let size_index = size_index?;

        let family = tokens[..size_index].join(" ");

        let mut weight = Self::REGULAR_WEIGHT;
        let mut italic = false;
        for token in &tokens[size_index + 1..] {
            match token.to_ascii_lowercase().as_str() {
                "bold" | "bolder" => weight = Self::BOLD_WEIGHT,
                "italic" | "oblique" => italic = true,
                "normal" | "regular" => {}
                // A trailing token that is neither a style word nor a number makes the
                // string ambiguous; refuse rather than silently dropping it.
                _ => return None,
            }
        }

        let font = Self::with_weight(family, size, weight, italic);
        font.is_valid().then_some(font)
    }
    /// Creates a font with modified size.
    pub fn with_size(&self, size: f32) -> Self {
        Self::with_weight(&self.family, size, self.weight, self.italic)
    }
    /// Creates a font with modified weight.
    pub fn with_weight_value(&self, weight: u16) -> Self {
        Self::with_weight(&self.family, self.size, weight, self.italic)
    }
    /// Creates a font with bold style.
    pub fn with_bold(&self, bold: bool) -> Self {
        let weight = if bold { Self::BOLD_WEIGHT } else { Self::REGULAR_WEIGHT };
        Self::with_weight(&self.family, self.size, weight, self.italic)
    }
    /// Creates a font with italic style.
    pub fn with_italic(&self, italic: bool) -> Self {
        Self::with_weight(&self.family, self.size, self.weight, italic)
    }
    /// Creates a font with modified family.
    pub fn with_family(&self, family: impl Into<String>) -> Self {
        Self::with_weight(family, self.size, self.weight, self.italic)
    }
    /// Returns font size as i32 (rounded and clamped to the signed range).
    pub fn size_i32(&self) -> i32 {
        if !self.size.is_finite() {
            return 0;
        }
        self.size.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
    }
    /// Returns font size as u32 (rounded and clamped to the unsigned range).
    pub fn size_u32(&self) -> u32 {
        if !self.size.is_finite() {
            return 0;
        }
        self.size.round().clamp(0.0, u32::MAX as f32) as u32
    }
    /// Creates a larger font by scaling the size.
    pub fn scaled(&self, scale: f32) -> Self {
        if !scale.is_finite() || scale <= 0.0 {
            return self.clone();
        }
        Self::with_weight(&self.family, self.size * scale, self.weight, self.italic)
    }
    /// Creates a smaller font by scaling the size.
    pub fn scaled_down(&self, scale: f32) -> Self {
        if !scale.is_finite() || scale <= 0.0 {
            return self.clone();
        }
        Self::with_weight(&self.family, self.size / scale, self.weight, self.italic)
    }
    /// Returns whether the font is bold (weight >= 700).
    pub fn is_bold(&self) -> bool {
        self.weight >= Self::BOLD_WEIGHT
    }
    /// Returns whether the font is regular weight (400).
    pub fn is_regular(&self) -> bool {
        self.weight == Self::REGULAR_WEIGHT
    }
    /// Returns whether the font is light weight (<= 300).
    pub fn is_light(&self) -> bool {
        self.weight <= 300
    }
    /// Returns CSS-like font weight string.
    pub fn weight_css(&self) -> &'static str {
        match self.weight {
            100 => "100",
            200 => "200",
            300 => "300",
            400 => "400",
            500 => "500",
            600 => "600",
            700 => "700",
            800 => "800",
            900 => "900",
            _ => "400",
        }
    }
    /// Returns CSS-like font style string.
    pub fn style_css(&self) -> &'static str {
        if self.italic {
            "italic"
        } else {
            "normal"
        }
    }
    /// Returns CSS font shorthand string.
    pub fn to_css(&self) -> String {
        format!("{} {} {}px {}", self.style_css(), self.weight_css(), self.size, self.family)
    }
}
impl Default for Font {
    fn default() -> Self {
        Self::default_ui()
    }
}
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct FontSerde {
    family: String,
    size: f32,
    #[serde(default = "Font::default_weight")]
    weight: u16,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
}
#[cfg(feature = "serde")]
impl From<FontSerde> for Font {
    fn from(value: FontSerde) -> Self {
        let normalized_weight = if value.weight == Font::default_weight() && value.bold {
            Font::BOLD_WEIGHT
        } else {
            Font::normalize_weight(value.weight)
        };
        Font {
            family: value.family,
            size: value.size,
            weight: normalized_weight,
            bold: normalized_weight >= Font::BOLD_WEIGHT,
            italic: value.italic,
        }
    }
}
/// Builder for constructing [`Font`] instances with fine-grained control.
///
/// By default the builder creates a regular-weight, non-italic font (family: "Arial",
/// size: 14.0). All settings are optional — call only what you need.
///
/// # Example
///
/// ```
/// use rust_widgets::core::Font;
///
/// let font = Font::builder()
///     .family("Helvetica")
///     .size(16.0)
///     .weight(600)
///     .build();
///
/// assert_eq!(font.family(), "Helvetica");
/// assert_eq!(font.weight(), 600);
/// ```
#[derive(Debug, Clone)]
pub struct FontBuilder {
    family: String,
    size: f32,
    weight: u16,
    italic: bool,
}
impl FontBuilder {
    fn new() -> Self {
        Self {
            family: String::from("Arial"),
            size: 14.0,
            weight: Font::REGULAR_WEIGHT,
            italic: false,
        }
    }
    /// Sets the font family.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        self.family = family.into();
        self
    }
    /// Sets the font point size.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    /// Sets the font weight in CSS-like scale (100..=900).
    /// The value will be normalized via [`Font::normalize_weight`].
    pub fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }
    /// Sets the italic style.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
    /// Consumes the builder and creates a [`Font`].
    pub fn build(self) -> Font {
        Font::with_weight(self.family, self.size, self.weight, self.italic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font_default_and_validation_contract() {
        let font = Font::default_ui();
        assert!(font.is_valid());
        assert_eq!(Font::default(), font);
        assert!(!Font::new("", 12.0, false, false).is_valid());
        assert!(!Font::new("Sans", 0.0, false, false).is_valid());
        assert_eq!(font.weight(), Font::REGULAR_WEIGHT);
        assert_eq!(Font::default_ui_bold().weight(), Font::BOLD_WEIGHT);
        assert!(!font.is_bold());
        assert!(Font::default_ui_bold().is_bold());
    }
    #[test]
    fn font_weight_normalization_contract() {
        let light = Font::with_weight("Sans", 12.0, 149, false);
        let medium = Font::with_weight("Sans", 12.0, 550, false);
        let heavy = Font::with_weight("Sans", 12.0, 2000, false);
        assert_eq!(light.weight(), 100);
        assert_eq!(medium.weight(), 600);
        assert_eq!(heavy.weight(), 900);
        assert!(Font::with_weight("Sans", 12.0, 700, false).is_bold());
        assert!(!Font::with_weight("Sans", 12.0, 600, false).is_bold());
    }
    #[test]
    fn font_bold_is_derived_from_normalized_weight() {
        let normalized_to_bold = Font::with_weight("Sans", 12.0, 650, false);
        assert_eq!(normalized_to_bold.weight(), 700);
        assert!(normalized_to_bold.is_bold());
    }

    #[test]
    fn invalid_scale_preserves_font_validity() {
        let font = Font::default_ui();
        assert_eq!(font.scaled(0.0), font);
        assert_eq!(font.scaled_down(0.0), font);
        assert_eq!(font.scaled(f32::NAN), font);
        assert_eq!(font.scaled_down(f32::INFINITY), font);
        assert!(font.scaled(2.0).is_valid());
        assert!(font.scaled_down(2.0).is_valid());
    }

    #[test]
    fn invalid_font_size_conversions_are_deterministic() {
        let mut font = Font::default_ui();
        font.set_size(f32::NAN);
        assert_eq!(font.size_i32(), 0);
        assert_eq!(font.size_u32(), 0);

        font.set_size(f32::INFINITY);
        assert_eq!(font.size_i32(), 0);
        assert_eq!(font.size_u32(), 0);
    }
    #[cfg(all(test, feature = "serde", feature = "serde_json", not(embedded_surface)))]
    #[test]
    fn font_deserialize_normalizes_weight_and_bold_contract() {
        let parsed: Font = serde_json::from_str(
            r#"{"family":"Sans","size":12.0,"weight":650,"bold":false,"italic":true}"#,
        )
        .expect("font deserialize should succeed");
        assert_eq!(parsed.weight(), 700);
        assert!(parsed.is_bold());
        assert!(parsed.is_italic());
        let parsed_legacy: Font =
            serde_json::from_str(r#"{"family":"Sans","size":12.0,"bold":true,"italic":false}"#)
                .expect("legacy font deserialize should succeed");
        assert_eq!(parsed_legacy.weight(), 700);
        assert!(parsed_legacy.is_bold());
    }

    #[test]
    fn parse_reads_family_size_and_style() {
        let plain = Font::parse("Monospace 20").expect("family + size must parse");
        assert_eq!(plain.family(), "Monospace");
        assert_eq!(plain.size(), 20.0);
        assert!(!plain.is_bold());
        assert!(!plain.is_italic());

        // A family with spaces is preserved, not truncated at the first token.
        let spaced = Font::parse("Segoe UI 14 bold").expect("multi-word family must parse");
        assert_eq!(spaced.family(), "Segoe UI");
        assert_eq!(spaced.size(), 14.0);
        assert!(spaced.is_bold());

        let italic = Font::parse("Menlo 12 italic").expect("italic must parse");
        assert_eq!(italic.family(), "Menlo");
        assert!(italic.is_italic());
        assert!(!italic.is_bold());

        let both = Font::parse("Menlo 12 bold italic").expect("bold+italic must parse");
        assert!(both.is_bold() && both.is_italic());

        // Style words are order-independent after the size, and `normal` is a no-op.
        let reordered = Font::parse("Menlo 12 italic bold").expect("reordered styles must parse");
        assert!(reordered.is_bold() && reordered.is_italic());
        let normal = Font::parse("Menlo 12 normal").expect("normal must parse");
        assert!(!normal.is_bold() && !normal.is_italic());
    }

    #[test]
    fn parse_keeps_a_family_that_contains_digits() {
        // The size is the *last* numerically-parsable token, so "2020" stays part of
        // the family and "12" is the size. Taking the first number instead would
        // silently rename the family to "Roboto" and leave "12" unparsed.
        let font = Font::parse("Roboto 2020 12").expect("digit-bearing family must parse");
        assert_eq!(font.family(), "Roboto 2020");
        assert_eq!(font.size(), 12.0);
    }

    #[test]
    fn parse_accepts_a_family_that_is_only_a_number_like_word_plus_extent() {
        // "Segoe UI Optimized 11" — the family ends at the last number.
        let font = Font::parse("Segoe UI Optimized 11").expect("long family must parse");
        assert_eq!(font.family(), "Segoe UI Optimized");
        assert_eq!(font.size(), 11.0);
    }

    #[test]
    fn parse_accepts_a_fractional_size() {
        let font = Font::parse("Inter 10.5").expect("fractional size must parse");
        assert_eq!(font.size(), 10.5);
    }

    #[test]
    fn parse_refuses_input_it_cannot_fully_honour() {
        // No size at all.
        assert!(Font::parse("Monospace").is_none());
        // No family at all.
        assert!(Font::parse("20").is_none());
        // Empty and whitespace-only.
        assert!(Font::parse("").is_none());
        assert!(Font::parse("   ").is_none());
        // Non-positive and non-finite sizes are not sizes.
        assert!(Font::parse("Monospace 0").is_none());
        assert!(Font::parse("Monospace -12").is_none());
        assert!(Font::parse("Monospace inf").is_none());
        assert!(Font::parse("Monospace NaN").is_none());
        // A trailing token that is neither a style word nor a number is ambiguous.
        assert!(Font::parse("Monospace 12 heavy").is_none());
    }

    #[test]
    fn parse_output_always_satisfies_is_valid() {
        // The caller relies on `Some(..)` meaning "usable as-is", which is what stops
        // the JSON loader from having to re-validate.
        for spec in ["Monospace 20", "Segoe UI 14 bold", "Menlo 12 bold italic", "Inter 10.5"] {
            let font = Font::parse(spec).unwrap_or_else(|| panic!("{spec:?} should parse"));
            assert!(font.is_valid(), "{spec:?} produced an invalid font: {font:?}");
        }
    }
}
