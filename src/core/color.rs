/// RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}
impl Color {
    /// Convenience constructor for an RGBA color.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    /// Returns an opaque RGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }
    /// Backward-compatible alias for `rgb`.
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(r, g, b)
    }
    /// Backward-compatible alias for `rgba`.
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(r, g, b, a)
    }
    /// Common color constants.
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const RED: Self = Self::rgb(255, 0, 0);
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    pub const CYAN: Self = Self::rgb(0, 255, 255);
    pub const MAGENTA: Self = Self::rgb(255, 0, 255);
    pub const GRAY: Self = Self::rgb(128, 128, 128);
    pub const LIGHT_GRAY: Self = Self::rgb(200, 200, 200);
    pub const DARK_GRAY: Self = Self::rgb(64, 64, 64);
    pub const EXTRA_LIGHT_GRAY: Self = Self::rgb(230, 230, 230);
    pub const MEDIUM_GRAY: Self = Self::rgb(160, 160, 160);
    pub const EXTRA_DARK_GRAY: Self = Self::rgb(32, 32, 32);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    /// Color variants.
    pub const LIGHT_RED: Self = Self::rgb(255, 100, 100);
    pub const DARK_RED: Self = Self::rgb(150, 0, 0);
    pub const LIGHT_GREEN: Self = Self::rgb(100, 255, 100);
    pub const DARK_GREEN: Self = Self::rgb(0, 150, 0);
    pub const LIGHT_BLUE: Self = Self::rgb(100, 100, 255);
    pub const DARK_BLUE: Self = Self::rgb(0, 0, 150);
    pub const LIGHT_YELLOW: Self = Self::rgb(255, 255, 150);
    pub const DARK_YELLOW: Self = Self::rgb(150, 150, 0);
    /// UI color constants.
    pub const PRIMARY: Self = Self::rgb(72, 142, 246);
    pub const SECONDARY: Self = Self::rgb(120, 124, 132);
    pub const SUCCESS: Self = Self::rgb(62, 165, 82);
    pub const WARNING: Self = Self::rgb(245, 166, 35);
    pub const ERROR: Self = Self::rgb(234, 57, 67);
    pub const BACKGROUND: Self = Self::rgb(245, 246, 248);
    pub const FOREGROUND: Self = Self::rgb(26, 28, 32);
    pub const LINK: Self = Self::rgb(0, 112, 201);
    pub const LINK_HOVER: Self = Self::rgb(0, 142, 251);
    pub const BORDER: Self = Self::rgb(192, 196, 204);
    pub const DIVIDER: Self = Self::rgb(223, 225, 230);
    pub const SELECTION: Self = Self::rgb(173, 216, 230);
    pub const TOOLTIP: Self = Self::rgb(255, 255, 224);
    pub const MENU_BACKGROUND: Self = Self::rgb(255, 255, 255);
    pub const MENU_FOREGROUND: Self = Self::rgb(26, 28, 32);
    /// Semantic colors.
    pub const INFO: Self = Self::rgb(66, 133, 244);
    pub const NOTIFICATION: Self = Self::rgb(103, 58, 183);
    pub const DISABLED_BACKGROUND: Self = Self::rgb(245, 245, 245);
    pub const DISABLED_FOREGROUND: Self = Self::rgb(153, 153, 153);
    /// Neutral colors.
    pub const ALICE_BLUE: Self = Self::rgb(240, 248, 255);
    pub const BEIGE: Self = Self::rgb(245, 245, 220);
    pub const CORAL: Self = Self::rgb(255, 127, 80);
    pub const GOLD: Self = Self::rgb(255, 215, 0);
    pub const IVORY: Self = Self::rgb(255, 255, 240);
    pub const LAVENDER: Self = Self::rgb(230, 230, 250);
    pub const ROSE: Self = Self::rgb(255, 105, 180);
    pub const SILVER: Self = Self::rgb(192, 192, 192);
    pub const TAN: Self = Self::rgb(210, 180, 140);
    /// Additional QT-like colors.
    pub const AQUA: Self = Self::rgb(0, 255, 255);
    pub const BROWN: Self = Self::rgb(165, 42, 42);
    pub const FOREST_GREEN: Self = Self::rgb(34, 139, 34);
    pub const INDIGO: Self = Self::rgb(75, 0, 130);
    pub const MAROON: Self = Self::rgb(128, 0, 0);
    pub const NAVY: Self = Self::rgb(0, 0, 128);
    pub const OLIVE: Self = Self::rgb(128, 128, 0);
    pub const ORANGE: Self = Self::rgb(255, 165, 0);
    pub const PINK: Self = Self::rgb(255, 192, 203);
    pub const PURPLE: Self = Self::rgb(128, 0, 128);
    pub const TEAL: Self = Self::rgb(0, 128, 128);
    /// Additional WX-like colors.
    pub const SKY_BLUE: Self = Self::rgb(135, 206, 235);
    pub const STEEL_BLUE: Self = Self::rgb(70, 130, 180);
    pub const SLATE_GRAY: Self = Self::rgb(112, 128, 144);
    pub const DARK_SLATE_GRAY: Self = Self::rgb(47, 79, 79);
    pub const LIGHT_SLATE_GRAY: Self = Self::rgb(119, 136, 153);
    pub const LIGHT_CYAN: Self = Self::rgb(224, 255, 255);
    pub const LIGHT_GOLDENROD_YELLOW: Self = Self::rgb(250, 250, 210);
    pub const LIGHT_PINK: Self = Self::rgb(255, 182, 193);
    pub const LIGHT_SALMON: Self = Self::rgb(255, 160, 122);
    /// Parses `#RRGGBB`, `#RRGGBBAA`, `#RGB` or `#RGBA` hex color strings.
    ///
    /// The parser is intentionally strict and deterministic:
    /// - leading `#` is required
    /// - only ASCII hex digits are accepted
    /// - short notation is normalized by nibble expansion (`#abc` -> `#AABBCC`)
    pub fn parse_hex(text: &str) -> Option<Self> {
        let raw = text.trim();
        let hex = raw.strip_prefix('#')?;
        let parse_byte = |slice: &str| u8::from_str_radix(slice, 16).ok();
        let parse_nibble = |ch: char| ch.to_digit(16).map(|n| (n as u8) * 17);
        match hex.len() {
            3 => {
                let mut chars = hex.chars();
                Some(Self::rgb(
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                ))
            }
            4 => {
                let mut chars = hex.chars();
                Some(Self::rgba(
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                    parse_nibble(chars.next()?)?,
                ))
            }
            6 => Some(Self::rgb(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
            )),
            8 => Some(Self::rgba(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
                parse_byte(&hex[6..8])?,
            )),
            _ => None,
        }
    }
    /// Returns canonical uppercase `#RRGGBB` serialization.
    pub fn to_hex_rgb(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    /// Returns canonical uppercase `#RRGGBBAA` serialization.
    pub fn to_hex_rgba(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
    /// Packs color channels into `0xRRGGBBAA` for stable transport/serialization.
    pub const fn to_rgba_u32(&self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | self.a as u32
    }
    /// Unpacks channels from `0xRRGGBBAA`.
    pub const fn from_rgba_u32(value: u32) -> Self {
        Self::rgba(
            ((value >> 24) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_hex_parse_and_serialize_are_deterministic() {
        assert_eq!(
            Color::parse_hex("#112233"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0xFF))
        );
        assert_eq!(
            Color::parse_hex("#11223344"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0x44))
        );
        assert_eq!(
            Color::parse_hex("#abc"),
            Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xFF))
        );
        assert_eq!(
            Color::parse_hex(" #AbCd "),
            Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xDD))
        );
        assert_eq!(Color::parse_hex("112233"), None);
        assert_eq!(Color::parse_hex("#12"), None);
        let color = Color::rgba(0x0A, 0x1B, 0x2C, 0x7D);
        assert_eq!(color.to_hex_rgb(), "#0A1B2C");
        assert_eq!(color.to_hex_rgba(), "#0A1B2C7D");
    }
    #[test]
    fn color_u32_pack_roundtrip_is_stable() {
        let color = Color::rgba(0x01, 0x23, 0x45, 0x67);
        let packed = color.to_rgba_u32();
        assert_eq!(packed, 0x01234567);
        assert_eq!(Color::from_rgba_u32(packed), color);
    }
}
