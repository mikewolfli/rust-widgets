// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::BTreeMap;
use crate::core::{Color, Font};
#[cfg(not(alloc_frugal))]
use serde::{Deserialize, Serialize};

use crate::style::{EasingFunction, MotionCurve, Padding, PaddingSpec, Shadow};

/// High-level theme definition used by runtime style resolution.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme unique name.
    pub name: String,
    /// Which appearance a theme provides.
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
    /// Icon size tokens (THM-02).
    ///
    /// # Why icon sizes are themed
    ///
    /// Icon sizes are a scale, not a constant: a button, a menu row and a tab bar must agree on
    /// what "small" and "medium" mean, or the same glyph reads as two different weights in one
    /// window. Before this group existed each control carried its own literal
    /// (`BUTTON_ICON_SIZE`, `TAB_ICON_SIZE`, `LARGE_ICON_SIZE`), so a theme could not state a
    /// tighter or roomier icon system and there was no single value a new control could adopt.
    ///
    /// The defaults are the literals those controls already used, so a theme file written before
    /// this field existed keeps rendering the sizes it always did.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub icons: IconTokens,
    /// Border/elevation tokens.
    pub borders: Borders,
    /// Whether this theme draws **flat** faces: no bevel, no elevation, no material.
    ///
    /// # Why this is one flag rather than a table of kind overrides
    ///
    /// The distinction BLUE24 §10A draws is between a *flat* style and a *dimensional* one, and
    /// it applies to **every** control: a theme that says "I am flat" means it about the card, the
    /// button, the dialog and the widget nobody has written yet. Expressing it as a table of
    /// `"<kind>": { "bevel": null }` entries means the next control added is **not** flat until
    /// someone remembers to add its key — measured, the first attempt at that table flattened 22
    /// kinds and left the fallback role casting shadows again.
    ///
    /// So the flat style is a property of the *preset*, read once where the role default is
    /// resolved, and the per-kind/per-state `overrides.styles` keys remain available for a theme
    /// that wants to make a **specific** control differ from its preset — which is a different, and
    /// still useful, statement.
    ///
    /// Default `false`: a theme built by a caller who does not think about surfaces keeps the role
    /// table's character, which is what every theme written before this field expects.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub flat_surfaces: bool,
    /// The host accent this theme was derived from.
    ///
    /// macOS lets the user choose one of eight system accent colours, and the system colours a
    /// control draws with follow that choice. A theme that hardcodes a single accent therefore
    /// disagrees with every window around it as soon as the user picks a different one.
    ///
    /// The value is recorded on the theme rather than read at paint time for the same reason
    /// [`Theme::appearance`] is: a theme file is a description of a palette, and a palette whose
    /// accent came from the host needs to say which accent that was so a caller can re-derive it
    /// (or explain it) without asking the device again. `SystemAccent::Blue` is the default,
    /// which is also the accent macOS reports when the user has not changed it, so an older theme
    /// file that omits the field keeps meaning what it meant before.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub system_accent: SystemAccent,
    /// The control-metrics profile this theme's controls are sized for.
    ///
    /// Control sizes, corner radii, switch tracks and text-field heights are platform conventions,
    /// not universal constants: a desktop control is shorter and more square than a touch target.
    /// Keeping the profile on the theme is what lets one preset select the sizes its own controls
    /// assume, rather than making every theme inherit one hardcoded table.
    ///
    /// Defaults to [`crate::widget::metrics::ControlMetricsProfile::Material`], the table the
    /// library shipped with, so a theme file written before this field existed loads unchanged.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub metrics_profile: crate::widget::metrics::ControlMetricsProfile,
    /// Class-level style overrides applied after base resolution.
    pub overrides: ThemeOverrides,
    /// Animation timing tokens.
    ///
    /// # Why motion is themed at all
    ///
    /// Durations are a design decision, not a constant: a platform that respects a user's
    /// reduced-motion preference shortens them, a game-like skin lengthens them, and a test harness
    /// sets them to zero to make an animated control reach its end state deterministically. Before
    /// this, `src/style/animation.rs` carried a complete engine whose durations could only be passed
    /// in per call site, so "how fast does this library feel" was not something a theme could state.
    ///
    /// The shared Material values are (`kThemeChangeDuration` 200 ms,
    /// `kRadialReactionDuration` 100 ms, the switch's 300 ms toggle), so an unstyled theme moves at
    /// the tempo a Material application is expected to.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub motion: Motion,
}

/// How long an interaction takes, and how it eases.
///
/// Values are milliseconds, matching [`AnimationConfig::duration`](crate::style::AnimationConfig).
/// Grouped by *role* rather than by control so a theme describes its rhythm once: a hover reaction
/// and a press ripple are both `fast`, and a control's own state transition is `normal` regardless
/// of which control it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(not(alloc_frugal), serde(default))]
pub struct Motion {
    /// A direct reaction to the pointer — a highlight, a ripple. Material's
    /// `kRadialReactionDuration` is 100 ms.
    pub fast: u32,
    /// A control's own state change — a toggle moving, a colour settling. Material's
    /// `kThemeChangeDuration` is 200 ms.
    pub normal: u32,
    /// A larger transition — a sheet opening, a panel expanding. Material's switch toggle is
    /// 300 ms, which is the longest any control interaction should take before it reads as slow.
    pub slow: u32,
    /// The easing applied to a state transition.
    ///
    /// `EaseOut` by default: an interaction should start immediately and settle, because a slow
    /// start reads as a missed input. This is the shape Material's standard curve has.
    pub easing: EasingFunction,
    /// Which family of motion shapes a transition uses.
    ///
    /// [`MotionCurve::Easing`] is the default and reads [`Motion::easing`], so every theme
    /// written before this field existed keeps its exact timing and shape. A theme that wants
    /// the settle of a physical spring states [`MotionCurve::Spring`] with one of the presets;
    /// a driver that asks for a curve explicitly (see `PropertyDriver::with_curve`) can also
    /// override the token per property without changing the theme.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub curve: MotionCurve,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            fast: 100,
            normal: 200,
            slow: 300,
            easing: EasingFunction::EaseOut,
            curve: MotionCurve::Easing,
        }
    }
}

/// A request for the host's translucent surface treatment.
///
/// # Why this is a request and not a colour
///
/// A material is not a fill: it asks the compositor to sample what is behind the surface,
/// blur it, and lay a tint over it. macOS exposes that as the window-chrome, sidebar and
/// content-under materials, and each keeps text legible over arbitrary content precisely
/// because the backdrop is part of the colour. Naming the material keeps that intent
/// expressible; a caller cannot reproduce it by lowering an alpha.
///
/// # Degradation
///
/// A host that reports reduced transparency must not draw a translucent surface. The
/// material then degrades to [`Material::opaque_surface`], an ordinary palette colour — the
/// same decision a window manager makes when it disables vibrancy. `Material::None` is the
/// identity request: no layer, no tint, and the control's own fill stays authoritative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
pub enum Material {
    /// No material: an opaque control-drawn surface.
    #[default]
    None,
    /// The window's titlebar/chrome material.
    WindowChrome,
    /// A source-list/sidebar material.
    Sidebar,
    /// Content that sits under a translucent bar or panel.
    ContentUnder,
}

impl Material {
    /// The backdrop blur radius this material asks for, or `None` for [`Material::None`].
    ///
    /// The values follow the platform's visual weight: the chrome is the most diffuse, a
    /// sidebar less so, and content-under the least, so a window keeps a readable hierarchy
    /// instead of every surface blurring by the same amount.
    pub const fn backdrop_blur_radius(self) -> Option<u32> {
        match self {
            Self::None => None,
            Self::WindowChrome => Some(30),
            Self::Sidebar => Some(20),
            Self::ContentUnder => Some(12),
        }
    }

    /// The tint this material lays over its blurred backdrop, or `None`.
    ///
    /// The tint comes from the active palette rather than a fixed colour, so a light and a
    /// dark theme keep their own surface hierarchy over the same backdrop.
    pub fn tint(self, colors: &Colors) -> Option<(Color, f32)> {
        match self {
            Self::None => None,
            Self::WindowChrome => Some((colors.surface_container_high, 0.65)),
            Self::Sidebar => Some((colors.surface_container, 0.55)),
            Self::ContentUnder => Some((colors.background, 0.40)),
        }
    }

    /// The opaque surface colour this material degrades to when transparency is reduced.
    ///
    /// Each material maps to the palette role that stands in for it, rather than to a single
    /// "fallback" grey: a sidebar that becomes opaque should still read as a sidebar.
    pub fn opaque_surface(self, colors: &Colors) -> Color {
        match self {
            Self::None | Self::ContentUnder => colors.background,
            Self::WindowChrome => colors.surface_container_high,
            Self::Sidebar => colors.surface_container,
        }
    }
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

/// The host's system accent choice.
///
/// # Why a closed set of eight
///
/// macOS offers exactly these accent choices, and each has a distinct pair of light/dark values
/// that the system keeps legible on its own surfaces. A free-form colour would let a theme claim
/// an accent the host never offered, and would lose the light/dark polarity the system maintains
/// (the dark value is deliberately lighter so it stays readable on a near-black window).
///
/// The variants are the *choice*, not a colour: [`SystemAccent::color`] is the only place the
/// two values live, so a caller never has to decide which appearance's value to use.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[cfg_attr(not(alloc_frugal), serde(rename_all = "lowercase"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemAccent {
    /// The default blue accent.
    #[default]
    Blue,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Red.
    Red,
    /// Orange.
    Orange,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Graphite (the neutral grey choice).
    Graphite,
}

impl SystemAccent {
    /// Every variant, in the order the system presents them.
    pub const ALL: [SystemAccent; 8] = [
        SystemAccent::Blue,
        SystemAccent::Purple,
        SystemAccent::Pink,
        SystemAccent::Red,
        SystemAccent::Orange,
        SystemAccent::Yellow,
        SystemAccent::Green,
        SystemAccent::Graphite,
    ];

    /// The accent's colour for an appearance.
    ///
    /// `dark` selects the light-on-dark value the system uses in dark mode. Both values are the
    /// host's own, not a derived tint: deriving one from the other would lose the intentional
    /// lightness increase the system applies to keep the accent legible on a dark surface.
    pub const fn color(self, dark: bool) -> Color {
        match (self, dark) {
            (SystemAccent::Blue, false) => Color::rgb(0x00, 0x7A, 0xFF),
            (SystemAccent::Blue, true) => Color::rgb(0x0A, 0x84, 0xFF),
            (SystemAccent::Purple, false) => Color::rgb(0xAF, 0x52, 0xDE),
            (SystemAccent::Purple, true) => Color::rgb(0xBF, 0x5A, 0xF2),
            (SystemAccent::Pink, false) => Color::rgb(0xFF, 0x2D, 0x55),
            (SystemAccent::Pink, true) => Color::rgb(0xFF, 0x37, 0x5F),
            (SystemAccent::Red, false) => Color::rgb(0xFF, 0x3B, 0x30),
            (SystemAccent::Red, true) => Color::rgb(0xFF, 0x45, 0x3A),
            (SystemAccent::Orange, false) => Color::rgb(0xFF, 0x95, 0x00),
            (SystemAccent::Orange, true) => Color::rgb(0xFF, 0x9F, 0x0A),
            (SystemAccent::Yellow, false) => Color::rgb(0xFF, 0xCC, 0x00),
            (SystemAccent::Yellow, true) => Color::rgb(0xFF, 0xD6, 0x0A),
            (SystemAccent::Green, false) => Color::rgb(0x34, 0xC7, 0x59),
            (SystemAccent::Green, true) => Color::rgb(0x30, 0xD1, 0x58),
            (SystemAccent::Graphite, false) => Color::rgb(0x8E, 0x8E, 0x93),
            (SystemAccent::Graphite, true) => Color::rgb(0x98, 0x98, 0x9D),
        }
    }

    /// The named accent closest to a colour read back from the host.
    ///
    /// The host reports a colour, not a name: the query returns whatever the system resolved for
    /// the current appearance, including a custom value a future OS may add. Rather than invent a
    /// ninth variant or fail, this picks the nearest of the eight in RGB space, comparing against
    /// **both** appearance values because the read colour's appearance is not part of the report.
    ///
    /// A read value that is not near any of them still lands on the nearest one; callers that
    /// need to know whether the match was exact should compare the result's
    /// [`SystemAccent::color`] with the colour they read.
    pub fn nearest(color: Color) -> Self {
        let mut best = SystemAccent::Blue;
        let mut best_distance = u32::MAX;
        for accent in Self::ALL {
            for dark in [false, true] {
                let candidate = accent.color(dark);
                let dr = i32::from(candidate.r) - i32::from(color.r);
                let dg = i32::from(candidate.g) - i32::from(color.g);
                let db = i32::from(candidate.b) - i32::from(color.b);
                let distance = (dr * dr + dg * dg + db * db) as u32;
                if distance < best_distance {
                    best_distance = distance;
                    best = accent;
                }
            }
        }
        best
    }
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
        // The normalisation lives in one place (the same rule `role_surface_style` applies), and
        // the fast path keeps it off the heap: this runs once per created control and once per
        // interaction-state change, where an owned `String` per classification was pure cost.
        crate::widget::capability::coercion::with_normalized_key(kind_name, Self::for_normalized)
    }

    /// The role table itself, over an already-normalised kind name.
    fn for_normalized(normalized: &str) -> Self {
        match normalized {
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
            // The `TimePicker` kind's own spelling (`time_picker` is its canonical row), so both
            // names of the time field classify as a field. Without this the kind's spelling fell
            // through to `Surface` and the field was filled with the window's own colour.
            | "timepicker"
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
            // A scrollbar's trough is a control surface with a movable thumb in it, so it
            // belongs with the fields above rather than with bare surface.
            //
            // It used to reach the `_` arm and resolve to `theme.colors.background` — the window
            // fill. That mattered twice over, because this control read the *same* field for its
            // thumb and its trough: with both resolving to the window colour, the thumb was
            // separated from its groove by nothing at all, and `scroll_bar.svg` carried two
            // byte-identical `rgba(18,18,18)` rectangles where a scrollbar should be.
            "scrollbar" | "scroll_bar" => Self::Input,
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
    /// Hairline separators, borders and focus rings.
    ///
    /// # Why this is separate from `foreground`
    ///
    /// A divider and a focus ring are both "a thin line", but they are not the same
    /// line: a divider should recede while a focus ring must announce itself. Deriving
    /// both from `foreground` meant a focused control looked identical to a bordered
    /// one, which is the BLUE21 §7 B defect. Material 3 names the pair `outline` and
    /// `outlineVariant`; this is the stronger of the two.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_outline_color"))]
    pub outline: Color,
    /// A weaker secondary separator, for nested or low-emphasis dividers.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_outline_variant_color"))]
    pub outline_variant: Color,
    /// The dimming layer behind a modal surface.
    ///
    /// # Why a theme needs a scrim of its own
    ///
    /// A modal dialog dims what is behind it to focus attention on itself. That dimming
    /// used to be an inline translucent black, which does nothing on a dark theme —
    /// "dimming" a surface that is already near-black leaves it near-black, so a modal
    /// and a non-modal window looked the same (BLUE21 B23). A named role lets a dark
    /// theme lighten the scrim instead.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_scrim_color"))]
    pub scrim: Color,
    /// The container colour for cards and panels sitting on `background`.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_surface_container_color"))]
    pub surface_container: Color,
    /// A raised step above [`Colors::surface_container`].
    #[cfg_attr(not(alloc_frugal), serde(default = "default_surface_container_high_color"))]
    pub surface_container_high: Color,
    /// A surface whose text is [`Colors::on_inverse_surface`], for snack bars and
    /// tooltips that deliberately invert to stand out.
    #[cfg_attr(not(alloc_frugal), serde(default = "default_inverse_surface_color"))]
    pub inverse_surface: Color,
    /// The ink legible on [`Colors::inverse_surface`].
    #[cfg_attr(not(alloc_frugal), serde(default = "default_on_inverse_surface_color"))]
    pub on_inverse_surface: Color,
}

/// Default info color used for backward-compatible deserialization.
#[cfg(not(alloc_frugal))]
const fn default_info_color() -> Color {
    Color::INFO
}

// The defaults below exist for **deserialization** (rule: every new field must be
// optional in a theme file an older build wrote). Each one is a mid-grey chosen to be
// legible on either appearance, so a theme file that omits the role still resolves to
// something renderable rather than to `Color::default()`'s transparent black.

/// Default separator colour (`#79747E`, Material 3's light-theme `outline`).
#[cfg(not(alloc_frugal))]
const fn default_outline_color() -> Color {
    Color::rgb(121, 116, 126)
}

/// Default secondary separator colour (`#CAC4D0`, M3's light `outlineVariant`).
#[cfg(not(alloc_frugal))]
const fn default_outline_variant_color() -> Color {
    Color::rgb(202, 196, 208)
}

/// Default modal scrim: translucent black, Material's 32 % overlay.
#[cfg(not(alloc_frugal))]
const fn default_scrim_color() -> Color {
    Color::rgba(0, 0, 0, 82)
}

/// Default card/panel container colour (`#F3EDF7`).
#[cfg(not(alloc_frugal))]
const fn default_surface_container_color() -> Color {
    Color::rgb(243, 237, 247)
}

/// Default raised container colour (`#ECE6F0`), one step above
/// [`default_surface_container_color`].
#[cfg(not(alloc_frugal))]
const fn default_surface_container_high_color() -> Color {
    Color::rgb(236, 230, 240)
}

/// Default inverse surface (`#313033`), M3's dark surface used on snack bars.
#[cfg(not(alloc_frugal))]
const fn default_inverse_surface_color() -> Color {
    Color::rgb(49, 48, 51)
}

/// Default ink on the inverse surface (`#F4EFF4`, M3's `inverseOnSurface`).
#[cfg(not(alloc_frugal))]
const fn default_on_inverse_surface_color() -> Color {
    Color::rgb(244, 239, 244)
}

impl Colors {
    /// Interpolates every token toward `other` by `weight` (`0.0` keeps `self`, `1.0` takes
    /// `other`).
    ///
    /// # Why a palette-wide blend and not a per-call-site helper
    ///
    /// A theme transition has to move **every** token together: blending only the background
    /// leaves text and borders jumping to the target mid-fade, which reads as a flicker rather
    /// than as a transition. Each field goes through [`Color::blend`], the crate's one colour
    /// interpolation primitive, so a future colour space change lands in one place rather than in
    /// a second copy of the arithmetic here.
    pub fn blend(&self, other: &Self, weight: f32) -> Self {
        Self {
            background: self.background.blend(&other.background, weight),
            foreground: self.foreground.blend(&other.foreground, weight),
            primary: self.primary.blend(&other.primary, weight),
            secondary: self.secondary.blend(&other.secondary, weight),
            accent: self.accent.blend(&other.accent, weight),
            error: self.error.blend(&other.error, weight),
            warning: self.warning.blend(&other.warning, weight),
            success: self.success.blend(&other.success, weight),
            disabled: self.disabled.blend(&other.disabled, weight),
            info: self.info.blend(&other.info, weight),
            outline: self.outline.blend(&other.outline, weight),
            outline_variant: self.outline_variant.blend(&other.outline_variant, weight),
            scrim: self.scrim.blend(&other.scrim, weight),
            surface_container: self.surface_container.blend(&other.surface_container, weight),
            surface_container_high: self
                .surface_container_high
                .blend(&other.surface_container_high, weight),
            inverse_surface: self.inverse_surface.blend(&other.inverse_surface, weight),
            on_inverse_surface: self.on_inverse_surface.blend(&other.on_inverse_surface, weight),
        }
    }
}

impl Default for Colors {
    /// A complete, renderable palette with every role populated.
    ///
    /// # Why this exists at all
    ///
    /// There was no `Default for Colors`, so the two places that needed one —
    /// `Default for Theme` and every test that wanted "a palette" — spelled out a
    /// struct literal with every field. Adding a role therefore broke an unbounded
    /// number of call sites, which is exactly what made a colour role expensive to
    /// add and is why the palette stayed at eleven roles. A `Default` makes adding one
    /// a local change: new field, one line here, one line in `Theme::default`.
    ///
    /// The ten original roles reproduce `Theme::default()`'s long-standing palette
    /// **exactly** (including `background = 240`), because every other preset and every
    /// test derives its expected colours from this theme. `Colors::default()` is a
    /// *base*, not a redesign: only the roles the palette never had are new values.
    fn default() -> Self {
        Self {
            background: Color::rgb(240, 240, 240),
            foreground: Color::rgb(0, 0, 0),
            primary: Color::rgb(33, 150, 243),
            secondary: Color::rgb(158, 158, 158),
            accent: Color::rgb(255, 152, 0),
            error: Color::rgb(244, 67, 54),
            warning: Color::rgb(255, 193, 7),
            success: Color::rgb(76, 175, 80),
            disabled: Color::rgb(200, 200, 200),
            info: Color::INFO,
            outline: Color::rgb(121, 116, 126),
            outline_variant: Color::rgb(202, 196, 208),
            scrim: Color::rgba(0, 0, 0, 82),
            surface_container: Color::rgb(243, 237, 247),
            surface_container_high: Color::rgb(236, 230, 240),
            inverse_surface: Color::rgb(49, 48, 51),
            on_inverse_surface: Color::rgb(244, 239, 244),
        }
    }
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
    ///
    /// # Non-finite factors (D09-THEME-02)
    ///
    /// [`f32::clamp`] passes `NaN` through, and a `NaN` channel rounds to `NaN` and casts to `0`,
    /// so the previous implementation silently produced **black** — unrelated to either factor
    /// endpoint. The shared policy in [`Color::normalized_factor`] maps `NaN` to `0.0` (the
    /// identity transform: the original colour) and `±∞` to the nearer endpoint, so a bad factor
    /// yields a defined colour rather than black.
    pub fn dark_variant(&self, factor: f32) -> Self {
        let f = Self::normalized_factor(factor);
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
    ///
    /// # Non-finite factors (D09-THEME-02)
    ///
    /// Shares [`Color::normalized_factor`] with [`Color::dark_variant`] so both helpers agree:
    /// `NaN` → `0.0` (the identity transform), `±∞` → the nearer endpoint.
    pub fn light_variant(&self, factor: f32) -> Self {
        let f = Self::normalized_factor(factor);
        Self::rgba(
            (self.r as f32 + (255.0 - self.r as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.g as f32 + (255.0 - self.g as f32) * f).round().clamp(0.0, 255.0) as u8,
            (self.b as f32 + (255.0 - self.b as f32) * f).round().clamp(0.0, 255.0) as u8,
            self.a,
        )
    }

    /// The documented non-finite policy for a colour-mix factor, shared by both variant helpers
    /// (D09-THEME-02).
    ///
    /// `NaN` → `0.0` (the identity transform, so the colour is left as-is rather than zeroed);
    /// `±∞` → `0.0` / `1.0`; a finite value → clamped into `0.0..=1.0`.
    pub fn normalized_factor(factor: f32) -> f32 {
        if factor.is_nan() {
            0.0
        } else {
            factor.clamp(0.0, 1.0)
        }
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

/// Icon size tokens, one per nominal slot (THM-02).
///
/// # Why three slots and not one size
///
/// The controls that carry icons fall into three visual weights: a compact row (tab bar, tool
/// button), the default affordance (a push button), and a prominent one (ribbon bar, empty
/// state). Naming the three slots keeps every control on one scale while still letting a larger
/// icon be a statement about *which slot it occupies* rather than a literal each control
/// happened to pick.
///
/// The values are the literals the shipped controls already carried — a tab's 16, a button's 18
/// (`dimensions::BUTTON_ICON_SIZE`), a ribbon item's 32 — so adding the group changes no preset's
/// rendering.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(alloc_frugal), serde(default))]
pub struct IconTokens {
    /// The compact slot: inline and toolbar icons, 16 px.
    pub small: u32,
    /// The default slot: a push button's icon, 18 px.
    pub medium: u32,
    /// The prominent slot: a ribbon or empty-state icon, 32 px.
    pub large: u32,
}

impl Default for IconTokens {
    fn default() -> Self {
        Self { small: 16, medium: 18, large: 32 }
    }
}

/// Which [`IconTokens`] slot a caller asks for.
///
/// A closed set on purpose: a caller states the *role* of the icon it is drawing, and the theme
/// owns the pixel value. An open `u32` argument would put the number back at the call site, which
/// is what the token group exists to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconSlot {
    /// Inline and toolbar icons — the compact slot.
    Small,
    /// A push button's default icon — the medium slot.
    Medium,
    /// A prominent icon (ribbon bar, empty state) — the large slot.
    Large,
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

impl Theme {
    /// The nominal side of `slot`, in logical pixels (THM-02).
    ///
    /// The single read point for an icon size: a control names the slot its icon occupies and the
    /// theme answers the number, so changing the icon scale is a theme edit rather than a sweep
    /// over control literals.
    pub fn icon_size(&self, slot: IconSlot) -> u32 {
        match slot {
            IconSlot::Small => self.icons.small,
            IconSlot::Medium => self.icons.medium,
            IconSlot::Large => self.icons.large,
        }
    }

    /// The drop shadow a face at `elevation` casts under this theme, or `None`.
    ///
    /// # Why this is a method here and not a serialised field
    ///
    /// A shadow is **derived** from two things the theme already states — whether this theme
    /// uses shadows at all ([`Borders::shadow`]) and how far off the page the face sits — so
    /// materialising it into the schema would be a second spelling of the same fact, and the
    /// two would drift (principle #101). Keeping it a lookup also means `themes/*.json` needs
    /// no new key, so an existing theme file keeps loading unchanged and this channel ships
    /// without a fixture regeneration.
    ///
    /// # Why the levels are not all the same shadow
    ///
    /// This is the whole point of the method. Before it, `role_base_style` built one literal
    /// shadow and gave it to every control, so a floating toast and a flush list row were
    /// painted at the same height and a theme could not say "cards are level 1". Reading the
    /// shadow through a *level* is what makes that expressible, and the ladder itself lives in
    /// `render::surface::default_shadow` so the relationship between levels is stated once.
    ///
    /// `tint` is the shadow's hue (normally black); each level applies its own alpha to it, so a
    /// theme states the hue once and the levels own their opacity.
    ///
    /// A theme that has shadows switched off gets `None` for every level, which is the honest
    /// answer rather than a zero-alpha shadow.
    pub fn elevation(&self, elevation: crate::render::Elevation, tint: Color) -> Option<Shadow> {
        if !self.borders.shadow {
            return None;
        }
        crate::render::default_shadow(elevation).map(|level| Shadow {
            x: 0,
            y: level.y,
            blur: level.blur,
            color: tint.with_alpha(level.alpha),
        })
    }
}

/// Style override map used for class-level theme customization.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct ThemeOverrides {
    /// Overrides keyed by style/class name.
    ///
    /// # Why this is a `BTreeMap` and not a `HashMap`
    ///
    /// These entries are **serialised into the theme fixtures** (`themes/*.json`), and a
    /// `HashMap` iterates in an unspecified order. With an empty override table that never
    /// mattered; once the presets carried 26 state keys, a fresh generation produced a
    /// different key order on every run and `check_theme_fixtures.sh` -- which regenerates
    /// and compares -- failed intermittently for a reason that had nothing to do with the
    /// theme's content. A sorted map makes the file deterministic, so a diff means a real
    /// change.
    pub styles: BTreeMap<String, ThemeStyleToken>,
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
    /// Optional padding override (THM-01).
    ///
    /// # Why a token-level padding
    ///
    /// The only control-level padding a theme could reach was the global
    /// [`Spacing::medium`], written into every role's base style by `role_base_style`. That makes
    /// "buttons are tighter than text fields" and a compact preset impossible without moving the
    /// spacing every other control also uses. This field is the per-kind/per-state statement: a
    /// theme writes `"button": { "padding": { "all": 4 } }` and only that role moves.
    ///
    /// The cascade is carried as [`PaddingToken`] rather than as the style layer's
    /// [`PaddingSpec`] because the theme schema stays on serialisable primitives (the rule
    /// [`ShadowToken`] follows); `apply_token` resolves it into the style's `padding`.
    ///
    /// Default `None` (and `serde(default)`), so a theme file written before this field existed
    /// keeps the base padding it always had.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub padding: Option<PaddingToken>,
    /// Optional font override, applied to the resolved style's `font`.
    ///
    /// Without this the theme's nine font tokens were unreachable from
    /// resolution: `resolve_style` never read `theme.fonts`, so every control
    /// kept the font its constructor chose.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub font: Option<Font>,
    /// Optional opacity override (normalised to `[0.0, 1.0]` on application).
    ///
    /// Applied through [`crate::style::normalized_opacity`], so a programmatic `NaN` becomes
    /// `1.0` rather than reaching the resolved style (D09-STYLE-04).
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

    // ── The four surface dimensions (BLUE24 §10A.4) ──
    //
    // Each is an optional override of one orthogonal dimension of the face this control is
    // drawn as. They reach a control through the same `overrides.styles` channel every other
    // key here uses, at both the `<kind>` / `<class>` layer and the `<kind>:<state>` layer —
    // so `"button:pressed": { "bevel": "inset" }` expresses the press feedback every 3D
    // button has, without a new theme layer and without a line of control code.
    /// Optional elevation override: which layer the face sits on.
    ///
    /// A **named** level (`"flat"`, `"level1"` …) rather than a number, so a theme file speaks
    /// the same word the role table does and an unrecognised word is a load error rather than a
    /// silent fallback (BLUE24 §10A.6 criterion 7).
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub elevation: Option<SurfaceElevationToken>,
    /// Optional bevel override: which way the face's edge is lit.
    ///
    /// Three-way like [`ShadowOverride`] and for the same reason: "inset a face the role
    /// raises", "flatten a face the role insets", and "say nothing" are three intents, and
    /// `Option` can only express two.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub bevel: BevelOverride,
    /// Optional material override: an opaque face or tinted glass.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub material: Option<SurfaceMaterialToken>,
    /// Optional edge override: whether the face draws a stroke, defers to its shadow, or has
    /// no visible edge.
    #[cfg_attr(not(alloc_frugal), serde(default))]
    pub hairline: Option<SurfaceHairlineToken>,
}

/// A padding override as a theme file spells it: the four-level cascade on primitives (THM-01).
///
/// # Why this mirrors [`PaddingSpec`] instead of storing it
///
/// [`PaddingSpec`] is the style layer's type and carries private layers with no serialisation
/// contract, while the theme schema deliberately stays on serialisable primitives (the rule
/// [`ShadowToken`] follows). This record keeps the same seven statements and resolves through the
/// style layer's own cascade, so there is one arithmetic for "side → axis → all → 0" rather than
/// a second copy that could drift.
///
/// A field left as `None` inherits the layer below it, exactly as the style builder does:
/// `{ "all": 8, "horizontal": 12 }` is 8 px above and below and 12 px left and right.
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(alloc_frugal), serde(default))]
pub struct PaddingToken {
    /// The least specific layer: all four sides.
    pub all: Option<u32>,
    /// Both vertical sides (top and bottom).
    pub vertical: Option<u32>,
    /// Both horizontal sides (left and right).
    pub horizontal: Option<u32>,
    /// The top side.
    pub top: Option<u32>,
    /// The right side.
    pub right: Option<u32>,
    /// The bottom side.
    pub bottom: Option<u32>,
    /// The left side.
    pub left: Option<u32>,
}

impl PaddingToken {
    /// Resolves the layers to concrete per-side values: `side → axis → all → 0`.
    ///
    /// The resolution runs through [`PaddingSpec`], so a theme token and a programmatic
    /// `with_padding_spec` call cannot disagree about what the same cascade means.
    pub fn resolve(&self) -> Padding {
        let mut spec = PaddingSpec::new();
        if let Some(all) = self.all {
            spec = spec.set_uniform(all);
        }
        if let Some(vertical) = self.vertical {
            spec = spec.set_vertical(vertical);
        }
        if let Some(horizontal) = self.horizontal {
            spec = spec.set_horizontal(horizontal);
        }
        if let Some(top) = self.top {
            spec = spec.top(top);
        }
        if let Some(right) = self.right {
            spec = spec.right(right);
        }
        if let Some(bottom) = self.bottom {
            spec = spec.bottom(bottom);
        }
        if let Some(left) = self.left {
            spec = spec.left(left);
        }
        spec.resolve()
    }
}

/// A surface elevation as a theme file spells it.
///
/// # Why a newtype and not the render type directly
///
/// The theme schema deliberately stays on its own spellings (the rule [`ShadowOverride`]
/// follows), and [`crate::render::surface::Elevation`] has no serialisation contract. The
/// newtype is also where an unknown token can be **rejected**: a theme that writes
/// `"elevation": "high"` gets an error naming the word, not a face that silently sits on the
/// page (BLUE24 §10A.6 criterion 7).
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(alloc_frugal), serde(try_from = "String", into = "String"))]
pub struct SurfaceElevationToken(pub crate::render::surface::Elevation);

/// A surface material as a theme file spells it. See [`SurfaceElevationToken`].
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(alloc_frugal), serde(try_from = "String", into = "String"))]
pub struct SurfaceMaterialToken(pub crate::render::surface::Material);

/// Which edge a face draws, as a theme file spells it. See [`SurfaceElevationToken`].
#[cfg_attr(not(alloc_frugal), derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(alloc_frugal), serde(try_from = "String", into = "String"))]
pub struct SurfaceHairlineToken(pub crate::render::surface::Hairline);

/// The message an unrecognised surface token produces.
///
/// One spelling for all of them, so a reader who mistypes any gets the same shape of message:
/// the key, the offending word, and the words that would have worked.
#[cfg(not(alloc_frugal))]
pub(crate) fn unknown_surface_token(key: &str, value: &str) -> String {
    let accepted = match key {
        "elevation" => "flat, level1, level2, level3, level4",
        "material" => "solid, translucent",
        "hairline" => "outline, shadow, none",
        "bevel" => "raised, inset, none, inherit",
        _ => "see the surface-style documentation",
    };
    format!("unknown {key} token {value:?} (accepted: {accepted})")
}

#[cfg(not(alloc_frugal))]
impl TryFrom<String> for SurfaceElevationToken {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        crate::render::surface::Elevation::parse(&value)
            .map(Self)
            .ok_or_else(|| unknown_surface_token("elevation", &value))
    }
}

#[cfg(not(alloc_frugal))]
impl TryFrom<String> for SurfaceMaterialToken {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        crate::render::surface::Material::parse(&value)
            .map(Self)
            .ok_or_else(|| unknown_surface_token("material", &value))
    }
}

#[cfg(not(alloc_frugal))]
impl TryFrom<String> for SurfaceHairlineToken {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        crate::render::surface::Hairline::parse(&value)
            .map(Self)
            .ok_or_else(|| unknown_surface_token("hairline", &value))
    }
}

// The round trip a theme file's `"keep it"` edit takes: reading a stored theme and writing it
// back must not change the token. Falling back to a default here would make an export lossy.
#[cfg(not(alloc_frugal))]
impl From<SurfaceElevationToken> for String {
    fn from(value: SurfaceElevationToken) -> Self {
        value.0.as_str().to_string()
    }
}

#[cfg(not(alloc_frugal))]
impl From<SurfaceMaterialToken> for String {
    fn from(value: SurfaceMaterialToken) -> Self {
        value.0.as_str().to_string()
    }
}

#[cfg(not(alloc_frugal))]
impl From<SurfaceHairlineToken> for String {
    fn from(value: SurfaceHairlineToken) -> Self {
        value.0.as_str().to_string()
    }
}

/// What a [`ThemeStyleToken`] says about a face's bevel.
///
/// # Why three cases rather than `Option<BevelDirection>`
///
/// Because there are three intents: **set** a direction (which may differ from the role
/// default — a theme that flattens buttons still wants a pressed field to read as a well),
/// **remove** the bevel the role default supplied (the flat-design case), and **inherit**.
/// `Option<BevelDirection>` can only say two of them (`Some` sets, `None` is ambiguous between
/// remove and inherit), which is the identical defect [`ShadowOverride`] fixes.
///
/// A theme file spells it `"raised"`, `"inset"`, `"none"` or `"inherit"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BevelOverride {
    /// Leave the bevel as the base resolution left it.
    #[default]
    Inherit,
    /// Remove the bevel, leaving a flat face.
    None,
    /// Bevel the face in this direction.
    Set(crate::render::BevelDirection),
}

#[cfg(not(alloc_frugal))]
impl<'de> serde::Deserialize<'de> for BevelOverride {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let token = String::deserialize(deserializer)?;
        match token.as_str() {
            "inherit" => Ok(Self::Inherit),
            "none" | "flat" => Ok(Self::None),
            other => crate::render::BevelDirection::parse(other)
                .map(Self::Set)
                // `"sunken"` is a plausible mistake for `"inset"`, and degrading it to "no
                // bevel" would make the face look flat while the author believes it is bevelled
                // (BLUE24 §10A.6 criterion 7).
                .ok_or_else(|| {
                    D::Error::custom(crate::theme::types::unknown_surface_token("bevel", other))
                }),
        }
    }
}

#[cfg(not(alloc_frugal))]
impl serde::Serialize for BevelOverride {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let token = match self {
            Self::Inherit => "inherit",
            Self::None => "none",
            Self::Set(direction) => direction.as_str(),
        };
        serializer.serialize_str(token)
    }
}

impl BevelOverride {
    /// Applies this override to a resolved bevel, or `None` when it should be removed.
    pub fn apply(
        self,
        current: Option<crate::render::BevelSpec>,
    ) -> Option<crate::render::BevelSpec> {
        match self {
            Self::Inherit => current,
            Self::None => None,
            Self::Set(direction) => Some(crate::render::BevelSpec::new(direction)),
        }
    }
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

#[cfg(test)]
mod system_accent_tests {
    use super::*;

    /// Every one of the eight system accents has the host's light and dark value.
    ///
    /// The pair matters more than either value: a theme that used the light value in dark mode
    /// would draw a saturated blue on a near-black window, and the host's dark values are
    /// deliberately lighter for that reason. Pinning the whole table is what keeps a later
    /// "tidy-up" from replacing it with one value and a computed tint.
    #[test]
    fn the_eight_system_accents_use_the_hosts_light_and_dark_values() {
        let table = [
            (SystemAccent::Blue, Color::rgb(0x00, 0x7A, 0xFF), Color::rgb(0x0A, 0x84, 0xFF)),
            (SystemAccent::Purple, Color::rgb(0xAF, 0x52, 0xDE), Color::rgb(0xBF, 0x5A, 0xF2)),
            (SystemAccent::Pink, Color::rgb(0xFF, 0x2D, 0x55), Color::rgb(0xFF, 0x37, 0x5F)),
            (SystemAccent::Red, Color::rgb(0xFF, 0x3B, 0x30), Color::rgb(0xFF, 0x45, 0x3A)),
            (SystemAccent::Orange, Color::rgb(0xFF, 0x95, 0x00), Color::rgb(0xFF, 0x9F, 0x0A)),
            (SystemAccent::Yellow, Color::rgb(0xFF, 0xCC, 0x00), Color::rgb(0xFF, 0xD6, 0x0A)),
            (SystemAccent::Green, Color::rgb(0x34, 0xC7, 0x59), Color::rgb(0x30, 0xD1, 0x58)),
            (SystemAccent::Graphite, Color::rgb(0x8E, 0x8E, 0x93), Color::rgb(0x98, 0x98, 0x9D)),
        ];
        assert_eq!(table.len(), SystemAccent::ALL.len(), "the table covers every variant");
        for (accent, light, dark) in table {
            assert_eq!(accent.color(false), light, "{accent:?} light value");
            assert_eq!(accent.color(true), dark, "{accent:?} dark value");
            assert_ne!(light, dark, "{accent:?} must not reuse one value for both appearances");
        }
    }

    /// A colour read back from the host maps to the accent it came from, in either appearance.
    ///
    /// The read is a colour, not a name, so this is the join between the platform fact and the
    /// theme field. Matching against both appearance values is what makes a dark-mode read of a
    /// light-mode value still resolve to the same variant.
    #[test]
    fn a_read_colour_maps_back_to_the_accent_it_came_from() {
        for accent in SystemAccent::ALL {
            for dark in [false, true] {
                assert_eq!(
                    SystemAccent::nearest(accent.color(dark)),
                    accent,
                    "{accent:?} in dark={dark} must round-trip"
                );
            }
        }
        // A value between two neighbours still resolves to one of the named accents rather than
        // inventing a variant or failing.
        assert_eq!(SystemAccent::nearest(Color::rgb(0x00, 0x7A, 0xFE)), SystemAccent::Blue);
    }

    /// A palette blend moves every token, and the endpoints are the two palettes themselves.
    #[test]
    fn a_palette_blend_is_per_token() {
        let light = Colors::default();
        let dark = crate::theme::Theme::dark().colors;
        let at_start = light.blend(&dark, 0.0);
        assert_eq!(at_start.background, light.background, "weight 0 is the source palette");
        assert_eq!(at_start.accent, light.accent);
        // `Colors` deliberately has no `PartialEq`; compare the tokens a transition would show.
        let at_end = light.blend(&dark, 1.0);
        assert_eq!(at_end.background, dark.background);
        assert_eq!(at_end.foreground, dark.foreground);
        assert_eq!(at_end.accent, dark.accent);
        let middle = light.blend(&dark, 0.5);
        assert_ne!(middle.background, light.background);
        assert_ne!(middle.background, dark.background);
        assert_ne!(middle.foreground, light.foreground);
        assert_ne!(middle.foreground, dark.foreground);
    }
}
