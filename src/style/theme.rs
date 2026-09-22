// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme configuration types including high contrast mode.
//!
//! # Why the theme *lookups* live here (BLUE20 layer 4, 2026-09-21)
//!
//! `crate::theme` is gated on `device_profile`, so it does not exist on `mini`/`embedded`
//! — yet ~120 control files read the theme while drawing, and those files *do* compile in
//! every profile that has [`crate::widget`]. That mismatch is what made
//! `cargo check --features mini` fail with `cannot find theme in crate` at each call site.
//!
//! The two functions below are the **always-available** form of those lookups. When
//! `crate::theme` is compiled in they forward to it, so there is exactly one
//! implementation and no chance of two palettes; when it is not, they answer `None`,
//! which every existing call site already handles (it is the same answer the theme gives
//! when no theme is active). A control therefore compiles everywhere and resolves to "no
//! theme" where there is no theme, rather than the crate pretending otherwise
//! (principle #37).
//!
//! Call sites in the widget layer should name `crate::style::resolved_theme_style` rather
//! than `crate::theme::resolved_theme_style`, which is why the two names are otherwise
//! identical: the re-export below is the single entry point.
use crate::core::Color;
use crate::style::WidgetStyle;

/// The resolved style for the control registered as `widget_name`, or `None` when no theme
/// is active or the build has no theme module.
///
/// The always-available counterpart of `crate::theme::resolved_theme_style`; see the
/// module docs for why it lives here.
#[cfg(device_profile)]
pub fn resolved_theme_style(widget_name: &str) -> Option<WidgetStyle> {
    crate::theme::resolved_theme_style(widget_name)
}

/// No theme module in this profile, so there is no style to resolve.
///
/// `None` rather than a fabricated default: a control that falls back to its own literal
/// is visibly un-themed, whereas a fabricated style would look themed while matching
/// nothing the user configured.
#[cfg(not(device_profile))]
pub fn resolved_theme_style(_widget_name: &str) -> Option<WidgetStyle> {
    None
}

/// The resolved style for `kind_name` with an optional CSS `class`, or `None` when no
/// theme is active or the build has no theme module.
///
/// The always-available counterpart of `crate::theme::resolved_theme_style_for`.
#[cfg(device_profile)]
pub fn resolved_theme_style_for(kind_name: &str, class_name: Option<&str>) -> Option<WidgetStyle> {
    crate::theme::resolved_theme_style_for(kind_name, class_name)
}

/// No theme module in this profile; see [`resolved_theme_style`].
#[cfg(not(device_profile))]
pub fn resolved_theme_style_for(
    _kind_name: &str,
    _class_name: Option<&str>,
) -> Option<WidgetStyle> {
    None
}

// ── The theme-facing types and lookups a control may name ───────────────────────
//
// A control that paints a *state* (a banner severity, a validation message, a completed
// progress run) reads a semantic token; a control that re-resolves its style mid-draw reads
// the manager. Those names live in `crate::theme`, which exists only on a device profile —
// but the controls that use them also compile in `--no-default-features --features
// "windows desktop-runtime controls-native controls-custom"`, a full widget set with no
// device profile. That mismatch was 78 compile errors at HEAD.
//
// So this module carries the always-available form of both. Where the theme module exists
// the names are **re-exports** of it (rule #54: one definition); where it does not, a
// minimal shape stands in whose reads answer "no theme", which is what every call site's
// own `Some(..) => themed, None => literal` ladder already handles. That keeps the
// condition here, once, rather than at each of the ~120 call sites (principle #47).

/// The theme data model, re-exported so there is exactly one definition of each type
/// wherever a theme module exists (rule #54).
#[cfg(device_profile)]
pub use crate::theme::{AppearanceMode, Colors, SemanticColor, Theme};

/// The four semantic colour tokens `Theme::colors` declares.
///
/// Defined here only where `crate::theme` is absent. A call site that *names* a token must
/// compile, and [`semantic_color`] then answers `None` regardless — so a control built
/// against this cannot render a token colour that no theme supplied.
#[cfg(not(device_profile))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticColor {
    /// The information indicator.
    Info,
    /// A completed, successful outcome.
    Success,
    /// Something that needs attention but is not a failure.
    Warning,
    /// A failure.
    Error,
}

/// The appearance selector where `crate::theme` is absent. See [`SemanticColor`].
#[cfg(not(device_profile))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum AppearanceMode {
    /// Light background, dark foreground.
    #[default]
    Light,
    /// Dark background, light foreground.
    Dark,
}

/// The palette shape a call site may name where `crate::theme` is absent.
///
/// It mirrors `Theme::colors` so `active.colors.background` type-checks, but no value of it
/// is ever produced — see [`ThemeManagerPlaceholder::current_theme`]. It exists so the code
#[cfg(not(device_profile))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Theme {
    /// The colour set, mirroring `crate::theme::Theme::colors`.
    pub colors: Colors,
}

/// The colour fields a control reads, mirroring `crate::theme::Colors`.
#[cfg(not(device_profile))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Colors {
    /// Window/background colour.
    pub background: Color,
    /// Default ink colour.
    pub foreground: Color,
    /// Primary brand/action colour.
    pub primary: Color,
    /// Secondary neutral colour.
    pub secondary: Color,
    /// Accent colour.
    pub accent: Color,
    /// Error-state colour.
    pub error: Color,
    /// Warning-state colour.
    pub warning: Color,
    /// Success-state colour.
    pub success: Color,
    /// Disabled-state colour.
    pub disabled: Color,
    /// Informational-state colour.
    pub info: Color,
    /// Separator / border / focus-ring colour.
    pub outline: Color,
    /// Weak secondary separator colour.
    pub outline_variant: Color,
    /// Modal dimming layer.
    pub scrim: Color,
    /// Card / panel container colour.
    pub surface_container: Color,
    /// Raised container colour.
    pub surface_container_high: Color,
    /// Deliberately inverted surface colour.
    pub inverse_surface: Color,
    /// Ink legible on [`Colors::inverse_surface`].
    pub on_inverse_surface: Color,
}

/// The active theme's colour for `token`, or `None` when no theme is active.
#[cfg(device_profile)]
pub fn semantic_color(token: SemanticColor) -> Option<Color> {
    crate::theme::semantic_color(token)
}

/// No theme module in this profile, so there is no palette to resolve a token against.
#[cfg(not(device_profile))]
pub fn semantic_color(_token: SemanticColor) -> Option<Color> {
    None
}

/// The active theme's motion tokens: `(fast, normal, slow)` in milliseconds.
///
/// # Why this is a function rather than a field read
///
/// `Theme::motion` exists only where the theme module does, and the two profiles have
/// different `Theme` shapes — so a caller in shared code cannot write `theme.motion`
/// without failing to compile in the profile that has no theme. Every animated control
/// needs the same three numbers, so the read happens here once.
///
/// The fallback is the crate's own `Motion::default()` tempo (100/200/300 ms), which is
/// what an unthemed build behaved as before the tokens existed. It is deliberately a
/// real, usable tempo rather than an error: a control with no theme still animates.
#[cfg(device_profile)]
pub fn motion_tokens() -> (u32, u32, u32) {
    crate::style::theme_manager()
        .current_theme()
        .map(|theme| (theme.motion.fast, theme.motion.normal, theme.motion.slow))
        .unwrap_or((100, 200, 300))
}

/// The active theme's motion tokens where there is no theme module to read them from.
#[cfg(not(device_profile))]
pub fn motion_tokens() -> (u32, u32, u32) {
    (100, 200, 300)
}

/// The process-wide theme manager, behind its lock.
///
/// The return type is spelled out rather than elided because it is a guard: a caller must
/// not hold it across a draw, and naming it makes that visible at the call site.
#[cfg(device_profile)]
pub fn theme_manager() -> crate::compat::MutexGuard<'static, crate::theme::ThemeManager> {
    crate::theme::global_theme_manager()
}

/// The process-wide theme manager, for a build that has none.
///
/// A build without a device profile has no theme module, so there is no palette to read.
/// This returns a manager-shaped value whose `current_theme()` is `None`, which is the same
/// answer the real manager gives when no theme is active — so every call site's existing
/// `Some(..) => themed, None => literal` ladder works unchanged and no `#[cfg]` is needed at
/// those ~90 sites. Inventing a palette here would be worse than `None`: a control would
/// paint colours no user configured while looking themed.
#[cfg(not(device_profile))]
pub fn theme_manager() -> ThemeManagerPlaceholder {
    ThemeManagerPlaceholder
}

/// The manager-shaped value a build without a theme module returns.
///
/// Deliberately not a `crate::theme::ThemeManager`: it has no registry, no setters and no
/// signal, so a caller cannot mutate or observe it. It exposes only the *read* path a
/// control's draw uses — `current_theme()` yielding a palette — and that read answers
/// `None`, so every call site's existing `Some(..) => themed, None => literal` ladder works
/// unchanged and no `#[cfg]` is needed at those ~90 sites. Inventing a palette here would be
/// worse than `None`: a control would paint colours no user configured while looking themed.
#[cfg(not(device_profile))]
#[derive(Debug, Clone, Copy, Default)]
pub struct ThemeManagerPlaceholder;

#[cfg(not(device_profile))]
impl ThemeManagerPlaceholder {
    /// There is never a theme in this profile, so this is always `None`.
    pub fn current_theme(&self) -> Option<Theme> {
        None
    }
}

/// High contrast theme mode detection and configuration (BLUE11 R7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HighContrastMode {
    /// No high contrast override; the normal theme applies.
    #[default]
    None,
    /// Force dark foreground on a light background. Chosen for users who need
    /// maximum luminance contrast and prefer a light surface.
    BlackOnWhite,
    /// Force light foreground on a dark background. The mirror of
    /// [`HighContrastMode::BlackOnWhite`], for users who find a light surface
    /// uncomfortable.
    WhiteOnBlack,
    /// Explicitly chosen foreground and background pair, for users with a
    /// preferred palette (for example a colour-blind-friendly one).
    Custom {
        /// Foreground (text and glyph) colour.
        fg: Color,
        /// Background colour. Nothing here enforces a contrast ratio against
        /// `fg`; any pairing the caller supplies is accepted.
        bg: Color,
    },
}

impl HighContrastMode {
    /// The `(background, foreground)` pair this mode forces, if any.
    ///
    /// `None` means "no override: let the theme resolve as usual". The pair is
    /// absolute — a forced mode replaces the palette rather than tinting it — which
    /// is what makes the resulting colours predictable enough to rely on for
    /// legibility.
    pub fn forced_pair(self) -> Option<(Color, Color)> {
        match self {
            Self::None => None,
            Self::BlackOnWhite => Some((Color::WHITE, Color::BLACK)),
            Self::WhiteOnBlack => Some((Color::BLACK, Color::WHITE)),
            Self::Custom { fg, bg } => Some((bg, fg)),
        }
    }

    /// The WCAG contrast ratio this mode's pair achieves, or `None` for
    /// [`HighContrastMode::None`].
    ///
    /// Lets a caller verify that a [`HighContrastMode::Custom`] pairing actually
    /// meets its accessibility target: the type accepts any pair, so the only way
    /// to know whether one is legible is to measure it. WCAG AA wants 4.5:1 for
    /// normal text and 3.0:1 for large text.
    pub fn contrast_ratio(self) -> Option<f32> {
        self.forced_pair().map(|(bg, fg)| bg.contrast_ratio(fg))
    }

    /// Whether this mode is active (i.e. forces a pair).
    pub fn is_active(self) -> bool {
        self != Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_forces_no_pair() {
        assert_eq!(HighContrastMode::None.forced_pair(), None);
        assert!(!HighContrastMode::None.is_active());
        assert_eq!(HighContrastMode::None.contrast_ratio(), None);
    }

    #[test]
    fn the_named_modes_force_the_documented_pairs() {
        assert_eq!(
            HighContrastMode::BlackOnWhite.forced_pair(),
            Some((Color::WHITE, Color::BLACK))
        );
        assert_eq!(
            HighContrastMode::WhiteOnBlack.forced_pair(),
            Some((Color::BLACK, Color::WHITE))
        );
        // `Custom { fg, bg }` returns `(background, foreground)` — the opposite
        // order from the struct fields, so this pins the mapping.
        assert_eq!(
            HighContrastMode::Custom { fg: Color::RED, bg: Color::BLUE }.forced_pair(),
            Some((Color::BLUE, Color::RED))
        );
    }

    #[test]
    fn the_named_modes_meet_wcag_aaa() {
        // Black on white is the maximum possible ratio, 21:1.
        let ratio = HighContrastMode::BlackOnWhite.contrast_ratio().expect("a forced pair");
        assert!((ratio - 21.0).abs() < 1e-3, "expected 21:1, got {ratio}");
        // Both named modes use the same two extremes, so they match.
        assert_eq!(HighContrastMode::WhiteOnBlack.contrast_ratio(), Some(ratio));
    }

    #[test]
    fn a_custom_pair_reports_its_actual_ratio() {
        // A low-contrast pairing is accepted, but its ratio is measurable — that is
        // the point of exposing `contrast_ratio`: the type cannot enforce a ratio,
        // so the caller needs the number to decide.
        let poor = HighContrastMode::Custom {
            fg: Color::rgb(128, 128, 128),
            bg: Color::rgb(120, 120, 120),
        };
        let ratio = poor.contrast_ratio().expect("a forced pair");
        assert!(ratio < 1.5, "expected a poor ratio, got {ratio}");

        let good = HighContrastMode::Custom { fg: Color::BLACK, bg: Color::WHITE };
        assert!(good.contrast_ratio().expect("a forced pair") >= 4.5, "WCAG AA for normal text");
    }

    #[test]
    fn is_active_distinguishes_none_from_the_rest() {
        assert!(!HighContrastMode::None.is_active());
        assert!(HighContrastMode::BlackOnWhite.is_active());
        assert!(HighContrastMode::WhiteOnBlack.is_active());
        assert!(HighContrastMode::Custom { fg: Color::BLACK, bg: Color::WHITE }.is_active());
    }
}
