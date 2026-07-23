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
    /// Creates a [`FontBuilder`] for ergonomic construction.
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
    /// Use [`simple`] instead.
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
    /// Returns font size as i32 (rounded).
    pub fn size_i32(&self) -> i32 {
        self.size.round() as i32
    }
    /// Returns font size as u32 (rounded and clamped to positive).
    pub fn size_u32(&self) -> u32 {
        self.size.round().max(0.0) as u32
    }
    /// Creates a larger font by scaling the size.
    pub fn scaled(&self, scale: f32) -> Self {
        Self::with_weight(&self.family, self.size * scale, self.weight, self.italic)
    }
    /// Creates a smaller font by scaling the size.
    pub fn scaled_down(&self, scale: f32) -> Self {
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
    #[cfg(not(feature = "embedded"))]
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
}
