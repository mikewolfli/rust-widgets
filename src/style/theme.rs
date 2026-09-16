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
