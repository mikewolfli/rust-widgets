// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Theme configuration types including high contrast mode.
use crate::core::Color;

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
