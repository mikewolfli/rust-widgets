//! Theme configuration types including high contrast mode.
use crate::core::Color;

/// High contrast theme mode detection and configuration (BLUE11 R7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HighContrastMode {
    #[default]
    None,
    BlackOnWhite,
    WhiteOnBlack,
    Custom { fg: Color, bg: Color },
}
