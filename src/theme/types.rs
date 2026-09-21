// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::HashMap;
use crate::core::{Color, Font};
#[cfg(not(alloc_frugal))]
use serde::{Deserialize, Serialize};

use crate::style::Shadow;

/// High-level theme definition used by runtime style resolution.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme unique name.
    pub name: String,
    /// Which appearance this theme provides.
    ///
    /// Recorded on the theme itself rather than inferred from the background
    /// colour: a caller switching between a light and a dark theme needs to know
    /// which is which before it has resolved any colour, and an app with a custom
    /// palette cannot be classified by luminance.
    ///
    /// Named `Appearance`, not `ThemeMode`: [`crate::style::ThemeMode`] already
    /// means the *user's preference* (including `Auto`), and both modules are
    /// glob-re-exported through `crate::style`, so two types sharing that name
    /// would collide at the re-export.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub appearance: AppearanceMode,
    /// Semantic color tokens.
    pub colors: Colors,
    /// Font tokens.
    pub fonts: Fonts,
    /// Spacing tokens.
    pub spacing: Spacing,
    /// Border/elevation tokens.
    pub borders: Borders,
    /// Class-level style overrides applied after base resolution.
    pub overrides: ThemeOverrides,
}

/// Which appearance a theme provides.
///
/// Distinct from [`crate::style::ThemeMode`], which is the *user's* preference
/// (including "follow the system"). This enum records a fact about the theme
/// data; that one records an intent about which theme to choose. The two are
/// deliberately separate types rather than one, because they answer different
/// questions and merging them would force a theme file to claim a preference it
/// cannot know.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppearanceMode {
    /// Light background, dark foreground.
    #[default]
    Light,
    /// Dark background, light foreground.
    Dark,
}

/// The role a widget plays in the theme's colour scheme.
///
/// Resolution used to be a `match` over thirteen hardcoded lowercase strings, so
/// a control whose name was not in that list got the generic `_` colours and
/// there was no way for a third-party widget to declare its role at all. A role
/// is a *small, closed* set of visual treatments (how many distinct colour
/// treatments does a widget library really have?), so it is an enum the caller
/// names, not a string the resolver guesses at.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetRole {
    /// The generic surface: panels, windows, dialogs, and anything unclassified.
    #[default]
    Surface,
    /// A filled, brand-coloured call to action (buttons, toggles).
    Primary,
    /// Plain text on the surface (labels, headings).
    Text,
    /// An editable field's interior (line edits, text edits).
    Input,
    /// A value indicator drawn in the accent colour (sliders, progress bars).
    Accent,
    /// A selectable indicator's box (check boxes, radio buttons, switches).
    Choice,
    /// A destructive or error action.
    Danger,
}

impl WidgetRole {
    /// The role a widget kind plays, for the kinds the library ships.
    ///
    /// Kinds not named here resolve to [`WidgetRole::Surface`], which is the same
    /// treatment they received from the old `_` arm — so the classification is a
    /// superset of the previous behaviour, not a change in it.
    ///
    /// A name-based lookup rather than a `WidgetKind` match so the resolver keeps
    /// working for a kind the library has never heard of: the caller passes the
    /// kind's name, and an unknown name gets the honest default.
    pub fn for_kind_name(kind_name: &str) -> Self {
        let normalized: String = kind_name
            .chars()
            .filter(|c| !matches!(c, '_' | '-' | ' '))
            .flat_map(char::to_lowercase)
            .collect();
        match normalized.as_str() {
            // Filled, brand-coloured actions.
            "button" | "toggle" | "togglebutton" => Self::Primary,
            // Plain text on the surface.
            "label" | "floatinglabel" | "text" | "heading" | "breadcrumb" | "commandlink"
            | "lcdnumber" | "fontpreview" => Self::Text,
            // Editable field interiors.
            "lineedit"
            | "textedit"
            | "textareainput"
            | "textarea"
            | "spinbox"
            | "doublespinbox"
            | "combobox"
            | "editablecombobox"
            | "fontcombobox"
            | "autocompleteedit"
            | "maskededit"
            | "multiselectcombobox"
            | "searchbar"
            | "searchbox"
            | "taginput"
            | "codeeditor"
            | "richedit"
            | "dateedit"
            | "timeedit"
            | "datetimeedit"
            | "inplaceeditor"
            | "shortcuteditor"
            | "coloreditor" => Self::Input,
            // Value indicators in the accent colour.
            "slider" | "rangeslider" | "progressbar" | "progresscircle" | "activityindicator"
            | "meter" | "lcdprogress" | "sparkline" => Self::Accent,
            // Selectable indicators.
            "checkbox" | "radiobutton" | "switch" | "checklistbox" | "rating" | "stepper" => {
                Self::Choice
            }
            // Scrolling, selectable fields whose whole rectangle is the control.
            //
            // # Why these are `Input` rather than falling through to `Surface`
            //
            // They used to reach the `_` arm, which resolves to `theme.colors.background` —
            // the very colour a window paints. A list box, a combo box's list and a text area
            // were therefore filled with the window's own background, so their extents were
            // invisible: the frame rendered correctly and showed nothing where the control
            // was. The distinction a reader needs is "this rectangle is a field I can put the
            // cursor in" versus "this rectangle is bare surface", and only `Input` carries it.
            "listbox" | "listview" | "tableview" | "treeview" | "scrollarea" | "textbrowser"
            | "plaintextedit" => Self::Input,
            // Destructive or error presentation.
            "errordialog" | "trash" | "deletebutton" => Self::Danger,
            _ => Self::Surface,
        }
    }
}

/// Semantic color palette tokens.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Colors {
    /// Default background color.
    pub background: Color,
    /// Default foreground/text color.
    pub foreground: Color,
    /// Primary brand/action color.
    pub primary: Color,
    /// Secondary neutral color.
    pub secondary: Color,
    /// Accent color.
    pub accent: Color,
    /// Error state color.
    pub error: Color,
    /// Warning state color.
    pub warning: Color,
    /// Success state color.
    pub success: Color,
    /// Disabled-state color.
    pub disabled: Color,
    /// Informational state color.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_info_color"))]
    pub info: Color,
}

/// Default info color used for backward-compatible deserialization.
#[cfg(not(alloc_frugal))]
const fn default_info_color() -> Color {
    Color::INFO
}

impl Color {
    /// Parses a hex color string (`"#RRGGBB"` or `"#RRGGBBAA"`) into a `Color`.
    ///
    /// # Errors
    /// Returns an error if the hex string is malformed or missing the `#` prefix.
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        Self::parse_hex(hex).ok_or_else(|| format!("Invalid hex color string: '{hex}'"))
    }

    /// Serializes the color to `"#RRGGBBAA"` hex format.
    pub fn to_hex(&self) -> String {
        self.to_hex_rgba()
    }

    /// Returns a darkened variant of this color by reducing each RGB component
    /// by the given `factor` (clamped to `[0.0, 1.0]`).
    ///
    /// A factor of `0.0` leaves the color unchanged; `1.0` produces black.
    /// Alpha is preserved unchanged.
    pub fn dark_variant(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            (self.g as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            (self.b as f32 * (1.0 - f)).round().clamp(0.0, 255.0) as u8,
            self.a,
        )
    }

    /// Returns a lightened variant of this color by increasing each RGB component
    /// toward 255 by the given `factor` (clamped to `[0.0, 1.0]`).
    ///
    /// A factor of `0.0` leaves the color unchanged; `1.0` produces white.
    /// Alpha is preserved unchanged.
    pub fn light_variant(&self, factor: f32) -> Self {
        let f = factor.clamp(0.0, 1.0);
        Self::rgba(
            (self.r as f32 + (255.0 - self.r as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.g as f32 + (255.0 - self.g as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.b as f32 + (255.0 - self.b as f32) * f).round().clamp(0.0, 255.0) as u8,
            self.a,
        )
    }
}

/// Font token set used by theme consumers.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Fonts {
    /// Regular text font token.
    pub regular: Font,
    /// Bold text font token.
    pub bold: Font,
    /// Italic text font token.
    pub italic: Font,
    /// Monospace font token.
    pub monospace: Font,
    /// Caption / footnote font token (small, secondary text).
    #[cfg_attr(not(alloc_frugal), serde(default = "default_caption_font"))]
    pub caption: Font,
    /// Body text font token (default paragraph text).
    #[cfg_attr(not(alloc_frugal), serde(default = "default_body_font"))]
    pub body: Font,
    /// Title font token (section or widget titles).
    #[cfg_attr(not(alloc_frugal), serde(default = "default_title_font"))]
    pub title: Font,
    /// Headline font token (prominent section headings).
    #[cfg_attr(not(alloc_frugal), serde(default = "default_headline_font"))]
    pub headline: Font,
    /// Display font token (large, decorative text).
    #[cfg_attr(not(alloc_frugal), serde(default = "default_display_font"))]
    pub display: Font,
}

/// Default caption font: Arial 11px, regular.
#[cfg(not(alloc_frugal))]
fn default_caption_font() -> Font {
    Font::simple("Arial", 11.0)
}

/// Default body font: Arial 14px, regular.
#[cfg(not(alloc_frugal))]
fn default_body_font() -> Font {
    Font::simple("Arial", 14.0)
}

/// Default title font: Arial 16px, bold.
#[cfg(not(alloc_frugal))]
fn default_title_font() -> Font {
    Font::bold("Arial", 16.0)
}

/// Default headline font: Arial 20px, bold.
#[cfg(not(alloc_frugal))]
fn default_headline_font() -> Font {
    Font::bold("Arial", 20.0)
}

/// Default display font: Arial 28px, bold.
#[cfg(not(alloc_frugal))]
fn default_display_font() -> Font {
    Font::bold("Arial", 28.0)
}

/// Spacing scale tokens.
///
/// # Recommendation
/// For more granular spacing, consider adding additional levels such as:
/// - `extra_small: u32` — 2px for tight spacing
/// - `huge: u32` — 48px for generous layout gaps
/// - `massive: u32` — 64px for section separators
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Spacing {
    /// Small spacing unit.
    pub small: u32,
    /// Medium spacing unit.
    pub medium: u32,
    /// Large spacing unit.
    pub large: u32,
    /// Extra-large spacing unit.
    pub extra_large: u32,
}

/// Border and elevation behavior tokens.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Borders {
    /// Default border width.
    pub width: u32,
    /// Default corner radius.
    pub radius: u32,
    /// Whether drop shadows are enabled.
    pub shadow: bool,
}

/// Style override map used for class-level theme customization.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ThemeOverrides {
    /// Overrides keyed by style/class name.
    pub styles: HashMap<String, ThemeStyleToken>,
}

/// Optional style tokens used to override resolved widget styles.
///
/// The `Option`-valued fields are what make an override *partial*: a field left
/// as `None` keeps the value base resolution produced, so a theme can adjust one
/// property of one role without restating the rest.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeStyleToken {
    /// Optional background override.
    pub background: Option<Color>,
    /// Optional foreground/text override.
    pub foreground: Option<Color>,
    /// Optional border color override.
    pub border: Option<Color>,
    /// Optional border width override.
    pub border_width: Option<u32>,
    /// Optional corner radius override.
    pub radius: Option<u32>,
    /// Optional font override, applied to the resolved style's `font`.
    ///
    /// Without this the theme's nine font tokens were unreachable from
    /// resolution: `resolve_style` never read `theme.fonts`, so every control
    /// kept the font its constructor chose.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub font: Option<Font>,
    /// Optional opacity override (clamped to `[0.0, 1.0]` on application).
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub opacity: Option<f32>,
    /// Optional drop-shadow override.
    ///
    /// A three-way choice, not `Option<Option<ShadowToken>>`: serde's `Option`
    /// deserializer maps a JSON `null` to `None` at the outer level, so the nested
    /// form can never distinguish "clear the shadow" from "do not mention it" —
    /// `Some(None)` is unreachable from a file, and the intent was silently lost.
    /// A named enum makes all three cases representable.
    ///
    /// Carried as a serde-friendly record rather than [`Shadow`], which is a
    /// render-layer type with no serialisation contract of its own. Keeping the
    /// theme schema free of render types also means a theme file cannot depend on
    /// how the renderer happens to represent a shadow today.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub shadow: ShadowOverride,
    /// Optional minimum touch-target override, as `[width, height]` in logical
    /// pixels. A two-element array rather than `core::Size` for the same reason as
    /// `shadow`: the theme schema stays on primitives.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub touch_target: Option<[u32; 2]>,
}

/// What a [`ThemeStyleToken`] says about a widget's drop shadow.
///
/// Three cases, because there are three distinct intents and collapsing any two of
/// them loses information: a theme may want to set a shadow, remove the shadow the
/// base resolution supplied, or say nothing and inherit.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadowOverride {
    /// Leave the shadow as the base resolution left it.
    #[default]
    Inherit,
    /// Remove the shadow, leaving a flat surface.
    None,
    /// Replace the shadow with this one.
    Set(ShadowToken),
}

impl ShadowOverride {
    /// Applies this override to a resolved shadow value.
    ///
    /// Takes and returns the `Option<Shadow>` a `WidgetStyle` holds, so the caller
    /// does not have to re-derive the three-way mapping.
    pub fn apply(self, current: Option<Shadow>) -> Option<Shadow> {
        match self {
            Self::Inherit => current,
            Self::None => None,
            Self::Set(token) => Some(token.into()),
        }
    }
}

/// A drop-shadow described with primitives, for theme files.
///
/// Converts to the render-layer [`Shadow`] at application time, which is where
/// the two representations meet.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowToken {
    /// Horizontal offset in logical pixels.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub x: i32,
    /// Vertical offset in logical pixels.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub y: i32,
    /// Blur radius in logical pixels.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub blur: u32,
    /// Shadow colour.
    pub color: Color,
}

impl From<ShadowToken> for Shadow {
    fn from(token: ShadowToken) -> Self {
        Self { x: token.x, y: token.y, blur: token.blur, color: token.color }
    }
}
