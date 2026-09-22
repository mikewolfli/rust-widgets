// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{format, String};

/// RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    fn channel_from_unit(value: f32) -> u8 {
        if !value.is_finite() {
            0
        } else {
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    }

    /// Convenience constructor for an RGBA color.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    /// Returns an opaque RGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }
    /// Backward-compatible alias for `rgb`.
    #[deprecated(since = "0.7.0", note = "use `rgb` instead")]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(r, g, b)
    }
    /// Backward-compatible alias for `rgba`.
    #[deprecated(since = "0.7.0", note = "use `rgba` instead")]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::rgba(r, g, b, a)
    }
    /// Creates a color from f32 values (0.0-1.0 range).
    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: Self::channel_from_unit(r),
            g: Self::channel_from_unit(g),
            b: Self::channel_from_unit(b),
            a: Self::channel_from_unit(a),
        }
    }
    /// Creates a color from f32 RGB values with full alpha.
    pub fn from_f32_rgb(r: f32, g: f32, b: f32) -> Self {
        Self::from_f32(r, g, b, 1.0)
    }
    /// Creates a color from i32 values (0-255 range).
    pub fn from_i32(r: i32, g: i32, b: i32, a: i32) -> Self {
        Self {
            r: r.clamp(0, 255) as u8,
            g: g.clamp(0, 255) as u8,
            b: b.clamp(0, 255) as u8,
            a: a.clamp(0, 255) as u8,
        }
    }
    /// Creates a color from i32 RGB values with full alpha.
    pub fn from_i32_rgb(r: i32, g: i32, b: i32) -> Self {
        Self::from_i32(r, g, b, 255)
    }
    /// Creates a color from u32 values (0xRRGGBBAA format).
    pub const fn from_u32_rgba(value: u32) -> Self {
        Self {
            r: ((value >> 24) & 0xFF) as u8,
            g: ((value >> 16) & 0xFF) as u8,
            b: ((value >> 8) & 0xFF) as u8,
            a: (value & 0xFF) as u8,
        }
    }
    /// Creates a color from u32 values (0xRRGGBB format, full alpha).
    pub const fn from_u32_rgb(value: u32) -> Self {
        Self::from_u32_rgba((value << 8) | 0xFF)
    }
    /// Creates a color from tuple of u8 (r, g, b, a).
    pub const fn from_u8_tuple((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self::rgba(r, g, b, a)
    }
    /// Creates a color from tuple of u8 (r, g, b) with full alpha.
    pub const fn from_u8_rgb_tuple((r, g, b): (u8, u8, u8)) -> Self {
        Self::rgb(r, g, b)
    }
    /// Creates a color from tuple of f32 (r, g, b, a) in 0.0-1.0 range.
    pub fn from_f32_tuple((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::from_f32(r, g, b, a)
    }
    /// Creates a color from tuple of f32 (r, g, b) with full alpha.
    pub fn from_f32_rgb_tuple((r, g, b): (f32, f32, f32)) -> Self {
        Self::from_f32_rgb(r, g, b)
    }
    /// Common color constants.
    /// Opaque black (`#000000`).
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// Opaque white (`#FFFFFF`).
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    /// Opaque pure red (`#FF0000`).
    pub const RED: Self = Self::rgb(255, 0, 0);
    /// Opaque pure green (`#00FF00`).
    pub const GREEN: Self = Self::rgb(0, 255, 0);
    /// Opaque pure blue (`#0000FF`).
    pub const BLUE: Self = Self::rgb(0, 0, 255);
    /// Opaque pure yellow (`#FFFF00`).
    pub const YELLOW: Self = Self::rgb(255, 255, 0);
    /// Opaque pure cyan (`#00FFFF`).
    pub const CYAN: Self = Self::rgb(0, 255, 255);
    /// Opaque pure magenta (`#FF00FF`).
    pub const MAGENTA: Self = Self::rgb(255, 0, 255);
    /// Opaque mid gray (`#808080`).
    pub const GRAY: Self = Self::rgb(128, 128, 128);
    /// Opaque light gray (`#C8C8C8`).
    pub const LIGHT_GRAY: Self = Self::rgb(200, 200, 200);
    /// Opaque dark gray (`#404040`).
    pub const DARK_GRAY: Self = Self::rgb(64, 64, 64);
    /// Opaque extra-light gray (`#E6E6E6`), lighter than [`Color::LIGHT_GRAY`].
    pub const EXTRA_LIGHT_GRAY: Self = Self::rgb(230, 230, 230);
    /// Opaque medium gray (`#A0A0A0`), between [`Color::GRAY`] and [`Color::LIGHT_GRAY`].
    pub const MEDIUM_GRAY: Self = Self::rgb(160, 160, 160);
    /// Opaque extra-dark gray (`#202020`), darker than [`Color::DARK_GRAY`].
    pub const EXTRA_DARK_GRAY: Self = Self::rgb(32, 32, 32);
    /// Fully transparent black: every channel is zero, so the alpha channel is 0 (`#00000000`).
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    /// Color variants.
    /// Opaque bright red tint (`#FF6464`).
    pub const LIGHT_RED: Self = Self::rgb(255, 100, 100);
    /// Opaque deep red shade (`#960000`).
    pub const DARK_RED: Self = Self::rgb(150, 0, 0);
    /// Opaque bright green tint (`#64FF64`).
    pub const LIGHT_GREEN: Self = Self::rgb(100, 255, 100);
    /// Opaque deep green shade (`#009600`).
    pub const DARK_GREEN: Self = Self::rgb(0, 150, 0);
    /// Opaque bright blue tint (`#6464FF`).
    pub const LIGHT_BLUE: Self = Self::rgb(100, 100, 255);
    /// Opaque deep blue shade (`#000096`).
    pub const DARK_BLUE: Self = Self::rgb(0, 0, 150);
    /// Opaque pale yellow tint (`#FFFF96`).
    pub const LIGHT_YELLOW: Self = Self::rgb(255, 255, 150);
    /// Opaque dark olive yellow shade (`#969600`).
    pub const DARK_YELLOW: Self = Self::rgb(150, 150, 0);
    /// UI color constants.
    /// Accent for primary actions and selected controls (`#488EF6`).
    pub const PRIMARY: Self = Self::rgb(72, 142, 246);
    /// Muted accent for less prominent actions (`#787C84`).
    pub const SECONDARY: Self = Self::rgb(120, 124, 132);
    /// Positive outcome indicator (`#3EA552`).
    pub const SUCCESS: Self = Self::rgb(62, 165, 82);
    /// Caution indicator (`#F5A623`).
    pub const WARNING: Self = Self::rgb(245, 166, 35);
    /// Failure indicator (`#EA3943`).
    pub const ERROR: Self = Self::rgb(234, 57, 67);
    /// Default light canvas behind content (`#F5F6F8`).
    pub const BACKGROUND: Self = Self::rgb(245, 246, 248);
    /// Default text color drawn on [`Color::BACKGROUND`] (`#1A1C20`).
    pub const FOREGROUND: Self = Self::rgb(26, 28, 32);
    /// Hyperlink color in its resting state (`#0070C9`).
    pub const LINK: Self = Self::rgb(0, 112, 201);
    /// Hyperlink color while hovered (`#008EFB`), lighter than [`Color::LINK`].
    pub const LINK_HOVER: Self = Self::rgb(0, 142, 251);
    /// Outline around interactive surfaces (`#C0C4CC`).
    pub const BORDER: Self = Self::rgb(192, 196, 204);
    /// Hairline rule between adjacent sections (`#DFE1E6`).
    pub const DIVIDER: Self = Self::rgb(223, 225, 230);
    /// Fill behind selected text or list rows (`#ADD8E6`).
    pub const SELECTION: Self = Self::rgb(173, 216, 230);
    /// Backing color for tooltip popups (`#FFFFE0`).
    pub const TOOLTIP: Self = Self::rgb(255, 255, 224);
    /// Backing color for menu popups (`#FFFFFF`).
    pub const MENU_BACKGROUND: Self = Self::rgb(255, 255, 255);
    /// Text color inside menu popups (`#1A1C20`), matching [`Color::FOREGROUND`].
    pub const MENU_FOREGROUND: Self = Self::rgb(26, 28, 32);
    /// Semantic colors.
    /// Neutral informational accent (`#4285F4`).
    pub const INFO: Self = Self::rgb(66, 133, 244);
    /// Accent for notification badges and banners (`#673AB7`).
    pub const NOTIFICATION: Self = Self::rgb(103, 58, 183);
    /// Background of a control that cannot be interacted with (`#F5F5F5`).
    pub const DISABLED_BACKGROUND: Self = Self::rgb(245, 245, 245);
    /// Text of a control that cannot be interacted with (`#999999`).
    pub const DISABLED_FOREGROUND: Self = Self::rgb(153, 153, 153);

    /// How much of `DISABLED_FOREGROUND` is mixed into a disabled fill, in percent.
    ///
    /// Two thirds keeps the widget's own hue readable while making the reduced
    /// emphasis obvious at a glance. Expressed as a constant so every disabled
    /// surface desaturates by the same amount and the appearance is auditable in one
    /// place rather than re-derived at each call site.
    const DISABLED_MIX_PERCENT: u32 = 66;

    /// A dimmed version of `color`, for a control that cannot be interacted with.
    ///
    /// # Why a mix rather than a flat replacement
    ///
    /// `DISABLED_BACKGROUND` is a single near-white grey. Painting it over every
    /// disabled widget would erase the difference between a disabled primary button
    /// and a disabled danger button — a caller who disabled the wrong one would lose
    /// the only cue identifying it. Mixing toward the disabled tint instead keeps the
    /// hue recognisable while making the state unmistakable.
    ///
    /// The alpha channel is preserved, so a translucent disabled surface stays as
    /// translucent as its enabled counterpart.
    ///
    /// ```
    /// # use rust_widgets::core::Color;
    /// // A disabled fill is pulled toward the disabled tint on every channel.
    /// let primary = Color::PRIMARY;
    /// let dimmed = Color::disabled_variant_of(primary);
    /// let tint = Color::DISABLED_FOREGROUND;
    /// assert_ne!(dimmed, primary, "a disabled fill must differ from the enabled one");
    /// // Each channel moves toward the tint, so the result is a genuine blend.
    /// assert!(dimmed.r >= primary.r.min(tint.r) && dimmed.r <= primary.r.max(tint.r));
    /// assert!(dimmed.g >= primary.g.min(tint.g) && dimmed.g <= primary.g.max(tint.g));
    /// assert!(dimmed.b >= primary.b.min(tint.b) && dimmed.b <= primary.b.max(tint.b));
    /// // Alpha is preserved so a translucent surface stays translucent.
    /// let translucent = Color::rgba(10, 20, 30, 128);
    /// assert_eq!(Color::disabled_variant_of(translucent).a, 128);
    /// // A colour already equal to the tint is unchanged.
    /// assert_eq!(Color::disabled_variant_of(tint), tint);
    /// ```
    pub fn disabled_variant_of(color: Color) -> Color {
        let mix = |channel: u8, target: u8| -> u8 {
            let keep = 100 - Self::DISABLED_MIX_PERCENT;
            (((channel as u32) * keep + (target as u32) * Self::DISABLED_MIX_PERCENT) / 100) as u8
        };
        Color {
            r: mix(color.r, Self::DISABLED_FOREGROUND.r),
            g: mix(color.g, Self::DISABLED_FOREGROUND.g),
            b: mix(color.b, Self::DISABLED_FOREGROUND.b),
            a: color.a,
        }
    }
    /// Neutral colors.
    /// Pale blue-white tint (`#F0F8FF`).
    pub const ALICE_BLUE: Self = Self::rgb(240, 248, 255);
    /// Pale warm cream (`#F5F5DC`).
    pub const BEIGE: Self = Self::rgb(245, 245, 220);
    /// Orange-pink accent (`#FF7F50`).
    pub const CORAL: Self = Self::rgb(255, 127, 80);
    /// Metallic yellow (`#FFD700`).
    pub const GOLD: Self = Self::rgb(255, 215, 0);
    /// Very pale yellow, slightly warmer than white (`#FFFFF0`).
    pub const IVORY: Self = Self::rgb(255, 255, 240);
    /// Pale violet tint (`#E6E6FA`).
    pub const LAVENDER: Self = Self::rgb(230, 230, 250);
    /// Bright pink (`#FF69B4`).
    pub const ROSE: Self = Self::rgb(255, 105, 180);
    /// Neutral metallic gray (`#C0C0C0`).
    pub const SILVER: Self = Self::rgb(192, 192, 192);
    /// Sandy brown-beige (`#D2B48C`).
    pub const TAN: Self = Self::rgb(210, 180, 140);
    /// Additional QT-like colors.
    /// Cyan accent, channel-identical to [`Color::CYAN`] (`#00FFFF`).
    pub const AQUA: Self = Self::rgb(0, 255, 255);
    /// Dark reddish-brown (`#A52A2A`).
    pub const BROWN: Self = Self::rgb(165, 42, 42);
    /// Deep leafy green (`#228B22`).
    pub const FOREST_GREEN: Self = Self::rgb(34, 139, 34);
    /// Deep violet-blue (`#4B0082`).
    pub const INDIGO: Self = Self::rgb(75, 0, 130);
    /// Dark brownish red (`#800000`).
    pub const MAROON: Self = Self::rgb(128, 0, 0);
    /// Dark blue (`#000080`).
    pub const NAVY: Self = Self::rgb(0, 0, 128);
    /// Dark yellow-green (`#808000`).
    pub const OLIVE: Self = Self::rgb(128, 128, 0);
    /// Bright orange (`#FFA500`).
    pub const ORANGE: Self = Self::rgb(255, 165, 0);
    /// Soft pink (`#FFC0CB`).
    pub const PINK: Self = Self::rgb(255, 192, 203);
    /// Dark violet (`#800080`).
    pub const PURPLE: Self = Self::rgb(128, 0, 128);
    /// Dark blue-green (`#008080`).
    pub const TEAL: Self = Self::rgb(0, 128, 128);
    /// Additional WX-like colors.
    /// Light azure blue (`#87CEEB`).
    pub const SKY_BLUE: Self = Self::rgb(135, 206, 235);
    /// Muted blue-gray (`#4682B4`).
    pub const STEEL_BLUE: Self = Self::rgb(70, 130, 180);
    /// Neutral blue-tinted gray (`#708090`).
    pub const SLATE_GRAY: Self = Self::rgb(112, 128, 144);
    /// Dark blue-tinted gray (`#2F4F4F`).
    pub const DARK_SLATE_GRAY: Self = Self::rgb(47, 79, 79);
    /// Lighter blue-tinted gray (`#778899`), between [`Color::SLATE_GRAY`] and [`Color::SILVER`].
    pub const LIGHT_SLATE_GRAY: Self = Self::rgb(119, 136, 153);
    /// Very pale cyan (`#E0FFFF`).
    pub const LIGHT_CYAN: Self = Self::rgb(224, 255, 255);
    /// Pale yellow (`#FAFAD2`).
    pub const LIGHT_GOLDENROD_YELLOW: Self = Self::rgb(250, 250, 210);
    /// Pale pink (`#FFB6C1`).
    pub const LIGHT_PINK: Self = Self::rgb(255, 182, 193);
    /// Pale salmon orange (`#FFA07A`).
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
        if !hex.is_ascii() {
            return None;
        }
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
    /// Use `from_u32_rgba` instead.
    #[deprecated(since = "0.7.0", note = "use `from_u32_rgba` instead")]
    pub const fn from_rgba_u32(value: u32) -> Self {
        Self::from_u32_rgba(value)
    }
    /// Creates color from tuple of i32 (r, g, b, a) in 0-255 range.
    pub fn from_i32_tuple((r, g, b, a): (i32, i32, i32, i32)) -> Self {
        Self::from_i32(r, g, b, a)
    }
    /// Converts color to f32 tuple (r, g, b, a) in 0.0-1.0 range.
    pub fn to_f32(&self) -> (f32, f32, f32, f32) {
        (self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, self.a as f32 / 255.0)
    }
    /// Converts color to i32 tuple (r, g, b, a) in 0-255 range.
    pub fn to_i32(&self) -> (i32, i32, i32, i32) {
        (self.r as i32, self.g as i32, self.b as i32, self.a as i32)
    }
    /// Creates a color with modified alpha.
    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self::rgba(self.r, self.g, self.b, alpha)
    }
    /// Creates a color with modified alpha (f32 in 0.0-1.0 range).
    pub fn with_alpha_f32(&self, alpha: f32) -> Self {
        Self::rgba(self.r, self.g, self.b, Self::channel_from_unit(alpha))
    }
    /// Blends two colors with given weight (0.0 = self, 1.0 = other).
    pub fn blend(&self, other: &Self, weight: f32) -> Self {
        let w = if weight.is_finite() { weight.clamp(0.0, 1.0) } else { 0.0 };
        let inv_w = 1.0 - w;
        Self::from_f32(
            self.r as f32 / 255.0 * inv_w + other.r as f32 / 255.0 * w,
            self.g as f32 / 255.0 * inv_w + other.g as f32 / 255.0 * w,
            self.b as f32 / 255.0 * inv_w + other.b as f32 / 255.0 * w,
            self.a as f32 / 255.0 * inv_w + other.a as f32 / 255.0 * w,
        )
    }
    /// Returns luminance (perceived brightness) in 0.0-1.0 range.
    pub fn luminance(&self) -> f32 {
        // Standard luminance formula
        (0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32) / 255.0
    }
    /// Returns whether the color is dark (luminance < 0.5).
    pub fn is_dark(&self) -> bool {
        self.luminance() < 0.5
    }
    /// Returns whether the color is light (luminance >= 0.5).
    pub fn is_light(&self) -> bool {
        !self.is_dark()
    }
    /// Returns the sRGB relative luminance, in `0.0..=1.0`, as WCAG 2.x defines it.
    ///
    /// Linearises each channel before weighting, so the result tracks perceived
    /// brightness rather than the raw byte average. This is the figure a contrast
    /// decision must use: [`Color::luminance`] uses the older Rec. 601 weights on
    /// *gamma-encoded* values, which is cheaper but answers a different question.
    /// The two disagree about which foreground is legible for a substantial band of
    /// colours (notably saturated blues), so contrast decisions must all go through
    /// this method or [`Color::contrast_color`].
    pub fn relative_luminance(&self) -> f32 {
        fn linearise(raw: u8) -> f32 {
            let c = raw as f32 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linearise(self.r) + 0.7152 * linearise(self.g) + 0.0722 * linearise(self.b)
    }

    /// Returns the WCAG contrast ratio between this colour and `other`, in
    /// `1.0..=21.0`.
    ///
    /// WCAG's AA threshold for normal text is 4.5 and for large text 3.0. Exposed
    /// so a caller can assert a pairing rather than rely on it by construction,
    /// which is what [`Color::contrast_color`] alone cannot express.
    pub fn contrast_ratio(&self, other: Self) -> f32 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Creates a contrasting color (black for light colors, white for dark colors).
    ///
    /// The choice is made on [`Color::relative_luminance`] against the WCAG 2.x
    /// threshold of 0.179, which is where white and black text reach the same
    /// contrast ratio against a mid-tone background. Deciding on the Rec. 601
    /// `luminance` instead picks the *less* legible option for saturated colours,
    /// so this method deliberately does not use it.
    pub fn contrast_color(&self) -> Self {
        if self.relative_luminance() > 0.179 {
            Self::BLACK
        } else {
            Self::WHITE
        }
    }

    /// Pushes this colour away from `surface` until it is legible on it, keeping the hue.
    ///
    /// A semantic token is chosen for its *meaning* (green means "ready", blue means
    /// "this is the active line"), not for its lightness against whatever surface a
    /// particular control happens to paint. A token whose luminance matches the surface is
    /// invisible however correct its hue — the terminal's success green measured 1.13:1 on
    /// the terminal body, and the markdown editor's primary-blue caret line measured 1.51:1
    /// on the editor surface. Stepping the token toward white on a dark surface and toward
    /// black on a light one keeps the hue recognisable while guaranteeing the glyph is not
    /// the same colour as what is behind it.
    ///
    /// The step is applied in sixteenths and the loop is bounded, so the function is total:
    /// the worst case returns `surface.contrast_color()`, which by construction contrasts
    /// with anything. Returns `self` unchanged when it already meets `min_ratio`.
    ///
    /// This is the shared home for the rule; `Color::contrast_color` remains the right call
    /// when the hue carries no meaning and only legibility matters.
    pub fn legible_on(&self, surface: Self, min_ratio: f32) -> Self {
        if self.contrast_ratio(surface) >= min_ratio {
            return *self;
        }
        let target = surface.contrast_color();
        for step in 1..=12 {
            let weight = step as f32 / 16.0;
            let candidate = self.blend(&target, weight);
            if candidate.contrast_ratio(surface) >= min_ratio {
                return candidate;
            }
        }
        target
    }
    /// Returns the inverted color (RGB channels negated, alpha preserved).
    pub fn invert(&self) -> Self {
        Self::rgba(255 - self.r, 255 - self.g, 255 - self.b, self.a)
    }
}
impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
impl From<&str> for Color {
    fn from(s: &str) -> Self {
        match Self::parse_hex(s) {
            Some(c) => c,
            None => {
                log::warn!("Color::from(\"{s}\") failed to parse, falling back to BLACK");
                Self::BLACK
            }
        }
    }
}
impl crate::compat::fmt::Display for Color {
    fn fmt(&self, f: &mut crate::compat::fmt::Formatter<'_>) -> crate::compat::fmt::Result {
        write!(f, "Color(#{:02X}{:02X}{:02X}{:02X})", self.r, self.g, self.b, self.a)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_hex_parse_and_serialize_are_deterministic() {
        assert_eq!(Color::parse_hex("#112233"), Some(Color::rgba(0x11, 0x22, 0x33, 0xFF)));
        assert_eq!(Color::parse_hex("#11223344"), Some(Color::rgba(0x11, 0x22, 0x33, 0x44)));
        assert_eq!(Color::parse_hex("#abc"), Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xFF)));
        assert_eq!(Color::parse_hex(" #AbCd "), Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xDD)));
        assert_eq!(Color::parse_hex("112233"), None);
        assert_eq!(Color::parse_hex("#aééa"), None);
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
        assert_eq!(Color::from_u32_rgba(packed), color);
    }
    #[test]
    fn color_constructors_from_different_types() {
        let c1 = Color::from_f32(0.5, 0.25, 0.75, 1.0);
        assert_eq!(c1, Color::rgba(128, 64, 191, 255));

        let c2 = Color::from_f32_rgb(0.5, 0.25, 0.75);
        assert_eq!(c2, Color::rgba(128, 64, 191, 255));

        let c3 = Color::from_i32(128, 64, 191, 255);
        assert_eq!(c3, Color::rgba(128, 64, 191, 255));

        let c4 = Color::from_i32(-10, 300, 128, 255);
        assert_eq!(c4, Color::rgba(0, 255, 128, 255));
    }

    #[test]
    fn non_finite_float_inputs_are_deterministic() {
        assert_eq!(
            Color::from_f32(f32::NAN, f32::INFINITY, 0.5, f32::NEG_INFINITY),
            Color::rgba(0, 0, 128, 0)
        );
        assert_eq!(Color::WHITE.with_alpha_f32(f32::NAN), Color::rgba(255, 255, 255, 0));
        assert_eq!(Color::RED.blend(&Color::BLUE, f32::NAN), Color::RED);
    }
    #[test]
    fn color_tuple_constructors() {
        let c1 = Color::from_u8_tuple((128, 64, 191, 255));
        assert_eq!(c1, Color::rgba(128, 64, 191, 255));

        let c2 = Color::from_f32_tuple((0.5, 0.25, 0.75, 1.0));
        assert_eq!(c2, Color::rgba(128, 64, 191, 255));

        let c3 = Color::from_i32_tuple((128, 64, 191, 255));
        assert_eq!(c3, Color::rgba(128, 64, 191, 255));
    }
    #[test]
    fn color_conversion_methods() {
        let color = Color::rgba(128, 64, 191, 255);

        let (r, g, b, a) = color.to_f32();
        assert!((r - 0.50196).abs() < 0.01);
        assert!((g - 0.25098).abs() < 0.01);
        assert!((b - 0.74902).abs() < 0.01);
        assert!((a - 1.0).abs() < 0.01);

        let (r, g, b, a) = color.to_i32();
        assert_eq!(r, 128);
        assert_eq!(g, 64);
        assert_eq!(b, 191);
        assert_eq!(a, 255);
    }
    #[test]
    fn color_with_alpha() {
        let color = Color::rgba(128, 64, 191, 255);

        let color2 = color.with_alpha(128);
        assert_eq!(color2, Color::rgba(128, 64, 191, 128));

        let color3 = color.with_alpha_f32(0.5);
        assert_eq!(color3, Color::rgba(128, 64, 191, 128));
    }
    #[test]
    fn color_blending() {
        let black = Color::BLACK;
        let white = Color::WHITE;

        let gray = black.blend(&white, 0.5);
        assert_eq!(gray, Color::rgba(128, 128, 128, 255));

        let quarter = black.blend(&white, 0.25);
        assert_eq!(quarter, Color::rgba(64, 64, 64, 255));

        let three_quarters = black.blend(&white, 0.75);
        assert_eq!(three_quarters, Color::rgba(191, 191, 191, 255));
    }
    #[test]
    fn color_luminance_and_contrast() {
        let black = Color::BLACK;
        let white = Color::WHITE;
        let gray = Color::rgba(128, 128, 128, 255);
        let red = Color::RED;
        let green = Color::GREEN;
        let blue = Color::BLUE;

        assert_eq!(black.luminance(), 0.0);
        assert_eq!(white.luminance(), 1.0);
        assert!((gray.luminance() - 0.5).abs() < 0.01);

        assert!(black.is_dark());
        assert!(!black.is_light());
        assert!(white.is_light());
        assert!(!white.is_dark());

        assert_eq!(black.contrast_color(), Color::WHITE);
        assert_eq!(white.contrast_color(), Color::BLACK);
        assert_eq!(gray.contrast_color(), Color::BLACK);

        // WCAG relative luminance is the figure contrast decisions use: black is 0
        // and white is 1 by definition, and the two extremes are 21:1 apart.
        assert_eq!(black.relative_luminance(), 0.0);
        assert!((white.relative_luminance() - 1.0).abs() < 1e-6);
        assert!((black.contrast_ratio(white) - 21.0).abs() < 1e-3);
        assert!((white.contrast_ratio(white) - 1.0).abs() < 1e-6);
        // Symmetric, so argument order never matters.
        assert_eq!(red.contrast_ratio(white), white.contrast_ratio(red));
        // WCAG AA for normal text is 4.5:1.
        assert!(black.contrast_ratio(white) >= 4.5);

        // A saturated blue is a colour where the two luminance models disagree, so
        // it pins that contrast decisions follow WCAG. `Color::is_dark` uses Rec.
        // 601 on gamma-encoded bytes and calls this colour *dark* (so the old
        // `contrast_color` picked white); WCAG's relative luminance puts it above
        // the 0.179 threshold, so the legible choice is black.
        let saturated_blue = Color::rgb(0, 121, 220);
        assert!(saturated_blue.is_dark(), "Rec. 601 sees this as dark");
        assert!(
            saturated_blue.relative_luminance() > 0.179,
            "WCAG sees this as light; contrast_color must therefore pick black"
        );
        assert_eq!(saturated_blue.contrast_color(), Color::BLACK);
        assert!(
            saturated_blue.contrast_ratio(Color::BLACK)
                > saturated_blue.contrast_ratio(Color::WHITE),
            "black must genuinely be the more legible of the two on this colour"
        );

        assert!(red.is_dark());
        // Pure green (0,255,0) has luminance=0.587 > 0.5, so it's light by standard formula
        assert!(green.is_light());
        assert!(blue.is_dark());
    }
    #[test]
    fn predefined_colors() {
        assert_eq!(Color::TRANSPARENT, Color::rgba(0, 0, 0, 0));
        assert_eq!(Color::BLACK, Color::rgba(0, 0, 0, 255));
        assert_eq!(Color::WHITE, Color::rgba(255, 255, 255, 255));
        assert_eq!(Color::RED, Color::rgba(255, 0, 0, 255));
        assert_eq!(Color::GREEN, Color::rgba(0, 255, 0, 255));
        assert_eq!(Color::BLUE, Color::rgba(0, 0, 255, 255));
        assert_eq!(Color::YELLOW, Color::rgba(255, 255, 0, 255));
        assert_eq!(Color::CYAN, Color::rgba(0, 255, 255, 255));
        assert_eq!(Color::MAGENTA, Color::rgba(255, 0, 255, 255));
        assert_eq!(Color::GRAY, Color::rgba(128, 128, 128, 255));
        assert_eq!(Color::LIGHT_GRAY, Color::rgba(200, 200, 200, 255));
        assert_eq!(Color::DARK_GRAY, Color::rgba(64, 64, 64, 255));
    }
}
