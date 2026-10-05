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
    /// Extra space inserted after every cluster, in logical pixels.
    ///
    /// # Why this is needed for text scaling
    ///
    /// A 2× text scale is not the same as a 2× point size. At large sizes the *spaces* between
    /// letters and lines become the dominant part of the reading rhythm: a font scaled only in
    /// points reads as cramped, which is why every desktop toolkit exposes tracking (CSS
    /// `letter-spacing`) alongside size. Without this field the crate could only scale a font's
    /// size, so a 2× build had no way to restore the spacing that the original design assumed.
    ///
    /// Zero means "the font's own natural tracking", which is what every existing call site and
    /// every existing serialised theme means — so this field is additive.
    #[cfg_attr(feature = "serde", serde(default))]
    letter_spacing: f32,
    /// Extra space inserted at each **whitespace** cluster, in logical pixels.
    ///
    /// # Why this is a second field and not folded into `letter_spacing`
    ///
    /// They are two different requests, and one number cannot express both. `letter_spacing` applies
    /// to every inter-cluster gap, so a caller who wants looser *words* would have to loosen every
    /// *letter* as well — which is a different (and usually unwanted) typographic change. CSS keeps
    /// them apart as `letter-spacing` and `word-spacing` for exactly this reason, and a designer
    /// moving between the two vocabularies should not have to relearn what the number means.
    ///
    /// # Why zero is the right default
    ///
    /// `0` means "the face's own space glyph", and that is the answer that stays correct across
    /// scripts: a space's width is a property of the font, not of the layout, so letting the shaper
    /// report it means the inter-word gap is right in Latin, in Arabic, and in a CJK face alike. A
    /// fixed em fraction here would be the same class of mistake as the flat cluster advance this
    /// crate already had to remove — correct for one script, wrong for the next.
    ///
    /// This field is therefore a **knob, not a correction**: it exists for a caller who wants to
    /// change the gap deliberately (a sparse title, a dense table).
    #[cfg_attr(feature = "serde", serde(default))]
    word_spacing: f32,
    /// Line height (leading) in logical pixels, or `0` for "derive from the font's own size".
    ///
    /// # Why `0` rather than a default number
    ///
    /// A line height is a *ratio applied to the size* (1.2 em is the common default), not a fixed
    /// pixel count. Storing an absolute number would make a font's leading wrong the moment its size
    /// changed — the two facts would have to be kept in step by hand. `0` therefore means "no
    /// explicit leading: use the renderer's own", and a caller that wants 1.2 em writes
    /// `size * 1.2` once. This keeps the field honest for the overwhelmingly common case where the
    /// caller has no opinion.
    #[cfg_attr(feature = "serde", serde(default))]
    line_height: f32,
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
    /// Returns the extra space inserted after every cluster, in logical pixels.
    ///
    /// `0.0` means the font's own natural tracking — see the field's documentation for why a text
    /// scale needs this alongside a point size.
    pub fn letter_spacing(&self) -> f32 {
        self.letter_spacing
    }
    /// Returns the extra space inserted at each whitespace cluster, in logical pixels.
    ///
    /// `0.0` means "the face's own space glyph" — see the field's documentation for why that, and
    /// not a fraction of an em, is the default.
    pub fn word_spacing(&self) -> f32 {
        self.word_spacing
    }
    /// Returns the explicit line height in logical pixels, or `0.0` for "derive from the size".
    pub fn line_height(&self) -> f32 {
        self.line_height
    }
    /// Sets the extra space after every cluster, and returns self for chaining.
    ///
    /// A negative value tightens; the value is a distance, so it is clamped to the point where it
    /// could not collapse a cluster to nothing. A tracking that large is a mistake rather than a
    /// style, and letting it through would make text overlap itself in a way no caller can debug.
    pub fn set_letter_spacing(&mut self, spacing: f32) -> &mut Self {
        self.letter_spacing = if spacing.is_finite() { spacing.max(-self.size) } else { 0.0 };
        self
    }
    /// Sets the extra space at each whitespace cluster, and returns self for chaining.
    ///
    /// Clamped by the same rule as [`Self::set_letter_spacing`], and for the same reason: a negative
    /// value tightens, and one large enough to consume a whole space would make two words overlap
    /// with nothing to indicate that a word boundary was there at all.
    pub fn set_word_spacing(&mut self, spacing: f32) -> &mut Self {
        self.word_spacing = if spacing.is_finite() { spacing.max(-self.size) } else { 0.0 };
        self
    }
    /// Sets an explicit line height in logical pixels, and returns self for chaining.
    ///
    /// `0` (or a non-finite value) means "derive from the size". A positive value is clamped to at
    /// least the font's own size: a line shorter than its glyphs would make consecutive lines
    /// overlap, which is a layout defect rather than a leading choice.
    pub fn set_line_height(&mut self, height: f32) -> &mut Self {
        self.line_height =
            if height.is_finite() && height > 0.0 { height.max(self.size) } else { 0.0 };
        self
    }
    /// The effective line height: the explicit one when set, otherwise the font's own size.
    ///
    /// # Why the fallback is `1.0` em and not a "typographic" `1.2`
    ///
    /// `1.2 em` is the conventional leading ratio, and using it here was the first spelling of this
    /// method. It was wrong: the crate's line box has always been `size` pixels (see
    /// `measure_text`), so a `1.2` fallback silently grew every measured line by 2–3 px — and because
    /// 176 call sites lay text out from that measurement, it re-laid-out the whole control set for
    /// callers who had asked for nothing. Measured: a 14 px font's line box went `14 -> 17`, a 11 px
    /// font's `11 -> 13`. Three tests caught it, which is the only reason it is not in the snapshots.
    ///
    /// So the contract is: **the fallback reproduces the previous behaviour exactly**, and `1.2 em` is
    /// something a caller writes when it wants it (`set_line_height(font.size() * 1.2)`). That is the
    /// same rule the rest of this crate's additive changes follow — a new field means what its absence
    /// has always meant.
    pub fn effective_line_height(&self) -> f32 {
        if self.line_height > 0.0 {
            self.line_height
        } else {
            self.size
        }
    }
    /// Sets the font point size (mutable setter for CSS parser integration).
    ///
    /// Changing the size re-applies the size-relative constraints to the text-scaling fields, because
    /// those fields were validated against the *old* size: a `line_height` that was legal at size 10
    /// may be shorter than the glyphs at size 30, and a negative `letter_spacing` legal at size 10 no
    /// longer is. The **precedence** is: an absolute value is preserved where it is still valid and
    /// clamped to the safe bound where it is not — so the caller's explicit leading/tracking survives
    /// a size change, but never in a form that would overlap text on itself.
    pub fn set_size(&mut self, size: f32) -> &mut Self {
        self.size = size;
        self.reapply_size_relative_constraints();
        self
    }

    /// Re-clamps the size-relative text-scaling fields against the current size.
    ///
    /// This is the single place that knows the constraints `set_line_height`/
    /// `set_letter_spacing`/`set_word_spacing` enforce, so a size change cannot leave a field the
    /// setters would have rejected. `0` means "no explicit value" for both spacing kinds and is left
    /// alone; a positive `line_height` is raised at least to the size; a negative spacing is raised at
    /// least to `-size`.
    fn reapply_size_relative_constraints(&mut self) {
        if self.line_height > 0.0 {
            self.line_height = self.line_height.max(self.size);
        }
        if self.letter_spacing < 0.0 {
            self.letter_spacing = self.letter_spacing.max(-self.size);
        }
        if self.word_spacing < 0.0 {
            self.word_spacing = self.word_spacing.max(-self.size);
        }
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
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 0.0,
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
    ///
    /// The text-scaling fields (`letter_spacing`, `word_spacing`, `line_height`) are **carried over**.
    /// They formerly were not — every one of these deriving constructors rebuilt through
    /// `with_weight`, which starts with no tracking and no leading, so a font that had a leading set
    /// lost it as soon as anything asked for a derived font. `theme/manager.rs` derives on every
    /// resolution (`theme.fonts.body.clone().scaled(effective_text_scale())`), so a theme's configured
    /// body leading disappeared exactly when a host set a text scale.
    pub fn with_size(&self, size: f32) -> Self {
        self.derived(size, self.weight, self.italic)
    }
    /// Creates a font with modified weight. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    pub fn with_weight_value(&self, weight: u16) -> Self {
        self.derived(self.size, weight, self.italic)
    }
    /// Creates a font with bold style. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    pub fn with_bold(&self, bold: bool) -> Self {
        let weight = if bold { Self::BOLD_WEIGHT } else { Self::REGULAR_WEIGHT };
        self.derived(self.size, weight, self.italic)
    }
    /// Creates a font with italic style. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    pub fn with_italic(&self, italic: bool) -> Self {
        self.derived(self.size, self.weight, italic)
    }
    /// Creates a font with modified family. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    pub fn with_family(&self, family: impl Into<String>) -> Self {
        self.derived(self.size, self.weight, self.italic).with_family_name(family)
    }

    /// Rebuilds this font at `weight`/`italic`, **preserving the text-scaling fields**.
    ///
    /// Every deriving constructor above funnels through here rather than through
    /// [`Font::with_weight`], which cannot carry them: it is the *constructor* a caller uses to build
    /// a fresh font from a family name, so it has no prior value to preserve. Keeping the two paths
    /// separate is what stops the drop from being reintroduced one constructor at a time.
    fn derived(&self, size: f32, weight: u16, italic: bool) -> Self {
        let mut derived = Font::with_weight(&self.family, size, weight, italic);
        derived.letter_spacing = self.letter_spacing;
        derived.word_spacing = self.word_spacing;
        derived.line_height = self.line_height;
        // The carried-over fields were validated against the original size, not `size`, so re-apply
        // the size-relative constraints here too; a derived font must satisfy the same invariants a
        // freshly built one does.
        derived.reapply_size_relative_constraints();
        derived
    }

    /// [`Self::with_family`]'s body, so the caller above can keep one funnel.
    fn with_family_name(mut self, family: impl Into<String>) -> Self {
        self.family = family.into();
        self
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
    /// Creates a larger font by scaling the size. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    ///
    /// A scale whose product with the size is not a finite, positive number is refused and `self` is
    /// returned unchanged: `scaled(f32::MAX)` would otherwise produce an infinite size
    /// (`is_valid() == false`) from an input that passed the finite/positive check. Returning the
    /// original keeps the font usable instead of handing back a broken one.
    pub fn scaled(&self, scale: f32) -> Self {
        if !scale.is_finite() || scale <= 0.0 {
            return self.clone();
        }
        let size = self.size * scale;
        if !size.is_finite() || size <= 0.0 {
            return self.clone();
        }
        self.with_size(size)
    }
    /// Creates a smaller font by scaling the size. The text-scaling fields are carried over; see
    /// [`Self::with_size`] for why that matters.
    ///
    /// As with [`Self::scaled`], a result that is not finite and positive is refused: dividing by
    /// `f32::MIN_POSITIVE` overflows to infinity, and returning the original is the only outcome that
    /// leaves the font valid.
    pub fn scaled_down(&self, scale: f32) -> Self {
        if !scale.is_finite() || scale <= 0.0 {
            return self.clone();
        }
        let size = self.size / scale;
        if !size.is_finite() || size <= 0.0 {
            return self.clone();
        }
        self.with_size(size)
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
    // Both default to `0.0`, so a theme serialised before these fields existed loads unchanged and
    // keeps meaning the same thing: "no explicit tracking, no explicit leading".
    #[serde(default)]
    letter_spacing: f32,
    #[serde(default)]
    word_spacing: f32,
    #[serde(default)]
    line_height: f32,
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
            letter_spacing: if value.letter_spacing.is_finite() {
                value.letter_spacing.max(-value.size)
            } else {
                0.0
            },
            word_spacing: if value.word_spacing.is_finite() {
                value.word_spacing.max(-value.size)
            } else {
                0.0
            },
            line_height: if value.line_height.is_finite() && value.line_height > 0.0 {
                value.line_height.max(value.size)
            } else {
                0.0
            },
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
    letter_spacing: f32,
    word_spacing: f32,
    line_height: f32,
}
impl FontBuilder {
    fn new() -> Self {
        Self {
            family: String::from("Arial"),
            size: 14.0,
            weight: Font::REGULAR_WEIGHT,
            italic: false,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 0.0,
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
    /// Sets the extra space after every cluster, in logical pixels.
    ///
    /// See [`Font::letter_spacing`] for why a text scale needs this next to a point size. The value
    /// is clamped by [`Font::set_letter_spacing`] when the font is built.
    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }
    /// Sets the extra space at each whitespace cluster, in logical pixels.
    ///
    /// `0` (the default) means "the face's own space glyph", which is the answer that stays correct
    /// across scripts — see [`Font::word_spacing`]. The value is clamped by
    /// [`Font::set_word_spacing`] when the font is built.
    pub fn word_spacing(mut self, spacing: f32) -> Self {
        self.word_spacing = spacing;
        self
    }
    /// Sets the line height in logical pixels; `0` means "derive from the size".
    ///
    /// The value is clamped by [`Font::set_line_height`] when the font is built.
    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }
    /// Consumes the builder and creates a [`Font`].
    pub fn build(self) -> Font {
        let mut font = Font::with_weight(self.family, self.size, self.weight, self.italic);
        // Through the setters rather than the fields, so the builder and the mutating API cannot
        // disagree about what a legal value is: `build()` is the path a caller takes when it has not
        // read the setters, which is exactly when a silent difference would go unnoticed.
        font.set_letter_spacing(self.letter_spacing);
        font.set_word_spacing(self.word_spacing);
        font.set_line_height(self.line_height);
        font
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

    /// A scale that passes the finite/positive check but overflows the *product* must be refused
    /// rather than producing an infinite, invalid size. `scaled(f32::MAX)` and
    /// `scaled_down(f32::MIN_POSITIVE)` both overflow; the font must come back unchanged and valid.
    #[test]
    fn scaling_that_would_overflow_the_size_returns_the_font_unchanged() {
        let font = Font::default_ui();
        let scaled = font.scaled(f32::MAX);
        assert_eq!(scaled.size(), font.size(), "an unrepresentable size is refused");
        assert!(scaled.is_valid(), "the refused result must stay valid");

        let scaled_down = font.scaled_down(f32::MIN_POSITIVE);
        assert_eq!(scaled_down.size(), font.size(), "underflow div is refused");
        assert!(scaled_down.is_valid());

        // Normal multipliers still scale, in both directions.
        assert!((font.scaled(2.0).size() - font.size() * 2.0).abs() < 1e-6);
        assert!((font.scaled_down(2.0).size() - font.size() / 2.0).abs() < 1e-6);
    }

    /// Changing the size must re-check the size-relative constraints on the text-scaling fields.
    /// A `line_height`/spacing legal at the old size can become illegal at the new one: the defect
    /// was that `set_size` left `line_height` below the new size and let negative tracking fall below
    /// the `-size` bound.
    #[test]
    fn set_size_reapplies_the_size_relative_constraints() {
        let mut font = Font::new("Arial", 10.0, false, false);
        font.set_line_height(10.0);
        assert_eq!(font.effective_line_height(), 10.0);

        font.set_size(30.0);
        assert!(font.is_valid());
        assert_eq!(
            font.line_height(),
            30.0,
            "an explicit leading shorter than the new glyphs must be raised"
        );
        assert_eq!(font.effective_line_height(), 30.0, "the effective leading follows");

        // A leading already larger than the new size is preserved, not shrunk to the size.
        let mut sized = Font::new("Arial", 40.0, false, false);
        sized.set_line_height(50.0);
        sized.set_size(10.0);
        assert_eq!(sized.line_height(), 50.0, "a valid absolute leading is preserved");

        // `0` still means "derive from the size" and is not turned into an explicit value.
        let mut plain = Font::new("Arial", 10.0, false, false);
        plain.set_size(30.0);
        assert_eq!(plain.line_height(), 0.0);
        assert_eq!(plain.effective_line_height(), 30.0);
    }

    /// The size-relative spacing bound is `-size`, so shrinking the size can push stored negative
    /// tracking below it. That is the case the re-check must catch: a `-9.0` tracking legal at size
    /// 10 is illegal once the size drops to 5 and must be clamped up to `-5`.
    #[test]
    fn shrinking_the_size_re_clamps_negative_spacing() {
        let mut font = Font::new("Arial", 10.0, false, false);
        font.set_letter_spacing(-9.0);
        font.set_word_spacing(-9.0);
        assert_eq!(font.letter_spacing(), -9.0);

        font.set_size(5.0);
        assert_eq!(font.letter_spacing(), -5.0, "tracking is clamped to the new -size bound");
        assert_eq!(font.word_spacing(), -5.0, "word spacing is clamped to the new -size bound");

        // Growing the size leaves already-valid negative spacing alone.
        let mut growing = Font::new("Arial", 10.0, false, false);
        growing.set_letter_spacing(-8.0);
        growing.set_size(30.0);
        assert_eq!(growing.letter_spacing(), -8.0, "-8 is still within the -30 bound");
    }

    /// The deriving constructors funnel through the same re-check, so a `with_size` that shrinks the
    /// font also re-clamps negative tracking against the smaller bound.
    #[test]
    fn derived_size_changes_also_reapply_the_constraints() {
        let base = Font::builder().size(20.0).line_height(20.0).letter_spacing(-18.0).build();
        let shrunk = base.with_size(10.0);
        assert_eq!(shrunk.line_height(), 20.0, "20 is still at least the new size of 10");
        assert_eq!(shrunk.letter_spacing(), -10.0, "tracking is re-clamped against the new size");
        assert!(shrunk.is_valid());

        // Growing the size raises a now-too-short leading and leaves valid tracking alone.
        let small = Font::builder().size(10.0).line_height(10.0).letter_spacing(-5.0).build();
        let grown = small.with_size(40.0);
        assert_eq!(grown.line_height(), 40.0, "with_size(40) must raise a 10px leading");
        assert_eq!(grown.letter_spacing(), -5.0, "-5 is still within the -40 bound");
        assert!(grown.is_valid());

        // `scaled` takes the same path, so an internal size change is consistent with `with_size`.
        let scaled = small.scaled(4.0);
        assert_eq!(scaled.line_height(), 40.0);
        assert_eq!(scaled.letter_spacing(), -5.0);
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

    /// A font with no explicit tracking or leading means exactly what it always meant.
    ///
    /// The two fields added for text scaling (BLUE22 · F-10) must be **additive**: every existing
    /// call site and every already-serialised theme omitted them, and if their absence changed any
    /// metric then adopting the fields would silently re-lay-out every control in the crate.
    #[test]
    fn the_text_scaling_fields_default_to_no_effect() {
        let font = Font::default();
        assert_eq!(font.letter_spacing(), 0.0, "no tracking unless asked for");
        assert_eq!(font.line_height(), 0.0, "no explicit leading unless asked for");
        // The effective leading is still the crate's own line box, so a caller that asks for a number
        // gets the same one the renderer uses for an uninstructed font — which is the whole point of
        // the fallback.
        assert!((font.effective_line_height() - font.size()).abs() < 1e-6);
    }

    /// The builder and the mutating setters agree on what a legal value is.
    ///
    /// They must: `build()` is the path a caller takes when it has *not* read the setters, so a
    /// difference between the two would be a silent one. The assertions are the clamps' contracts —
    /// tracking cannot collapse a cluster, and a line cannot be shorter than its glyphs — plus the
    /// degenerate inputs (`0`, negative, non-finite) that a deserialised theme can carry.
    #[test]
    fn the_text_scaling_clamps_are_the_same_through_both_apis() {
        // A line shorter than the glyphs is a layout defect, so it is raised to the size.
        let via_builder = Font::builder().size(20.0).line_height(4.0).build();
        let via_setter = {
            let mut font = Font::default();
            font.set_size(20.0);
            font.set_line_height(4.0);
            font
        };
        assert_eq!(via_builder.line_height(), 20.0);
        assert_eq!(via_builder.line_height(), via_setter.line_height());

        // Tracking tighter than the cluster itself would overlap text, so it is clamped to `-size`.
        let tight = Font::builder().size(10.0).letter_spacing(-999.0).build();
        let tighter = {
            let mut font = Font::default();
            font.set_size(10.0);
            font.set_letter_spacing(-999.0);
            font
        };
        assert_eq!(tight.letter_spacing(), -10.0);
        assert_eq!(tight.letter_spacing(), tighter.letter_spacing());

        // Zero and non-finite both mean "no opinion" rather than a broken font: a theme file with
        // `"line_height": 0` is the common case, and a `NaN` from arithmetic must not propagate into
        // every measurement in the frame.
        for bad in [0.0f32, -0.0, f32::NAN, f32::INFINITY] {
            let font = Font::builder().line_height(bad).letter_spacing(bad).build();
            assert_eq!(font.line_height(), 0.0, "{bad} must mean 'derive from the size'");
            assert_eq!(font.letter_spacing(), 0.0, "{bad} must mean 'no tracking'");
        }
    }

    /// The word spacing must round-trip through the builder and through serde.
    ///
    /// # Why both paths, in one test
    ///
    /// The two are separate spellings of the same field, and a field that survives one but not the
    /// other is the failure mode this crate has paid for before — a JSON-overridden control whose
    /// property silently did nothing. `serde(default)` matters as much as the value: a theme written
    /// before this field existed must load as "the face's own space", not fail to parse.
    ///
    /// # Why the gate matches the sibling test above
    ///
    /// The JSON half needs `serde` *and* `serde_json`, both optional, and a target that has
    /// `embedded_surface` has neither staged for a host test. Without this gate the test compiled
    /// unconditionally and the `serde_json` path failed to resolve on the `ohos` cross target -- a
    /// build break in `check_harmony_cross.sh` for a test that target cannot run anyway. The gate is
    /// spelled exactly as the neighbouring `font_*_serde` test spells it, so the two cannot drift
    /// into disagreeing about which configuration can parse a theme.
    #[cfg(all(test, feature = "serde", feature = "serde_json", not(embedded_surface)))]
    #[test]
    fn word_spacing_survives_the_builder_and_serde() {
        use crate::compat::MiniToString;

        let built = Font::builder().size(12.0).word_spacing(3.0).build();
        assert_eq!(built.word_spacing(), 3.0, "the builder carries it");

        // The same clamp as letter spacing: a gap that could consume a whole space would leave two
        // words touching with nothing to mark the boundary.
        let mut clamped = Font::new("Arial", 10.0, false, false);
        clamped.set_word_spacing(-999.0);
        assert_eq!(clamped.word_spacing(), -10.0, "clamped to the point size");

        // A theme from before the field existed: absent means `0.0`, which means the face's own space.
        let old =
            "{\"family\":\"Arial\",\"size\":12.0,\"weight\":400,\"bold\":false,\"italic\":false}";
        let parsed: Font = serde_json::from_str(old).expect("an older theme must still load");
        assert_eq!(parsed.word_spacing(), 0.0, "absent must mean 'no opinion'");

        // And a theme that states it keeps the value it stated.
        let stated = "{\"family\":\"Arial\",\"size\":12.0,\"weight\":400,\"bold\":false,\"italic\":false,\"word_spacing\":4.5}";
        let parsed: Font = serde_json::from_str(stated).expect("it parses");
        assert_eq!(parsed.word_spacing(), 4.5, "a stated value survives");
        let _ = "".to_string();
    }

    /// A tracked font measures wider than the same font untracked, by exactly its tracking.
    ///
    /// This is the property that makes the field usable at all: a metric that does not affect
    /// measurement would let a tracked label be laid out at its untracked width and then paint
    /// outside its own box. The count is `clusters - 1` gaps — a trailing tracking would offset a
    /// centred label.
    #[test]
    fn tracking_widens_a_measurement_by_one_gap_per_cluster_boundary() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(200, 40));
        let context = crate::render::RenderContext::new(&mut backend);
        let plain = Font::default();
        let tracked = Font::builder().size(plain.size()).letter_spacing(2.0).build();

        let text = "abc";
        let plain_width = context.measure_text(text, &plain).width;
        let tracked_width = context.measure_text(text, &tracked).width;
        // Three clusters: two gaps of 2 px.
        assert_eq!(
            tracked_width,
            plain_width + 4,
            "the run widens by `clusters - 1` gaps: {plain_width} -> {tracked_width}"
        );
        // A single cluster has no gap to pay, so it measures identically: the tracking is a gap
        // between clusters rather than a trailing space.
        assert_eq!(
            context.measure_text("a", &tracked).width,
            context.measure_text("a", &plain).width,
            "one cluster has no inter-cluster gap to pay"
        );
    }

    /// Every font-*deriving* constructor keeps the text-scaling fields.
    ///
    /// # The defect this pins
    ///
    /// `with_size` / `with_weight_value` / `with_bold` / `with_italic` / `with_family` / `scaled` /
    /// `scaled_down` all rebuilt the font through `with_weight`, which starts with no tracking and no
    /// leading. A font that had `line_height(24.0)` set therefore **lost it** the moment anything
    /// asked for a derived font — which the theme layer does on every resolution:
    /// `theme.fonts.body.clone().scaled(effective_text_scale())` in `theme/manager.rs`. So a theme
    /// that configured its body leading for a 1.5-em reading measure got 1.0 em as soon as the host
    /// set a text scale, and the two spellings of "the same font" disagreed.
    ///
    /// # Why the assertion is on all seven
    ///
    /// They are one defect repeated: each is a `Self::with_weight(..)` call that drops the fields.
    /// Testing one would let the next person fix only that one.
    #[test]
    fn a_derived_font_keeps_its_tracking_and_leading() {
        let base = Font::builder()
            .family("Monospace")
            .size(14.0)
            .weight(400)
            .letter_spacing(1.5)
            .word_spacing(3.0)
            .line_height(21.0)
            .build();

        // (name, derived) for every constructor that produces a *font from a font*.
        let derived: [(&str, Font); 7] = [
            ("with_size", base.with_size(20.0)),
            ("with_weight_value", base.with_weight_value(700)),
            ("with_bold", base.with_bold(true)),
            ("with_italic", base.with_italic(true)),
            ("with_family", base.with_family("Inter")),
            ("scaled", base.scaled(1.5)),
            ("scaled_down", base.scaled_down(1.5)),
        ];
        for (name, font) in &derived {
            assert_eq!(
                font.letter_spacing(),
                base.letter_spacing(),
                "{name} dropped the letter spacing"
            );
            assert_eq!(font.word_spacing(), base.word_spacing(), "{name} dropped the word spacing");
            assert_eq!(font.line_height(), base.line_height(), "{name} dropped the line height");
        }

        // And the scaled one really did scale, so the assertion above is not vacuous.
        assert!((derived[5].1.size() - 21.0).abs() < 1e-6, "scaled(1.5) of 14 is 21");
        // While an invalid scale is a no-op that still preserves the fields.
        assert_eq!(base.scaled(f32::NAN).size(), base.size());
        assert_eq!(base.scaled_down(0.0).letter_spacing(), base.letter_spacing());
    }
}
