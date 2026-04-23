/// Font descriptor used by text rendering and themes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "FontSerde")]
pub struct Font {
    /// Font family name.
    pub family: String,
    /// Font point size.
    pub size: f32,
    /// Font weight in CSS-like scale (100..=900).
    #[serde(default = "Font::default_weight")]
    pub weight: u16,
    /// Whether bold style is requested.
    pub bold: bool,
    /// Whether italic style is requested.
    pub italic: bool,
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
        let weight = if bold {
            Self::BOLD_WEIGHT
        } else {
            Self::REGULAR_WEIGHT
        };
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
}
impl Default for Font {
    fn default() -> Self {
        Self::default_ui()
    }
}
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
        assert_eq!(font.weight, Font::REGULAR_WEIGHT);
        assert_eq!(Font::default_ui_bold().weight, Font::BOLD_WEIGHT);
    }
    #[test]
    fn font_weight_normalization_contract() {
        let light = Font::with_weight("Sans", 12.0, 149, false);
        let medium = Font::with_weight("Sans", 12.0, 550, false);
        let heavy = Font::with_weight("Sans", 12.0, 2000, false);
        assert_eq!(light.weight, 100);
        assert_eq!(medium.weight, 600);
        assert_eq!(heavy.weight, 900);
        assert!(Font::with_weight("Sans", 12.0, 700, false).bold);
        assert!(!Font::with_weight("Sans", 12.0, 600, false).bold);
    }
    #[test]
    fn font_bold_is_derived_from_normalized_weight() {
        let normalized_to_bold = Font::with_weight("Sans", 12.0, 650, false);
        assert_eq!(normalized_to_bold.weight, 700);
        assert!(normalized_to_bold.bold);
    }
    #[cfg(not(feature = "embedded"))]
    #[test]
    fn font_deserialize_normalizes_weight_and_bold_contract() {
        let parsed: Font = serde_json::from_str(
            r#"{"family":"Sans","size":12.0,"weight":650,"bold":false,"italic":true}"#,
        )
        .expect("font deserialize should succeed");
        assert_eq!(parsed.weight, 700);
        assert!(parsed.bold);
        assert!(parsed.italic);
        let parsed_legacy: Font =
            serde_json::from_str(r#"{"family":"Sans","size":12.0,"bold":true,"italic":false}"#)
                .expect("legacy font deserialize should succeed");
        assert_eq!(parsed_legacy.weight, 700);
        assert!(parsed_legacy.bold);
    }
}
