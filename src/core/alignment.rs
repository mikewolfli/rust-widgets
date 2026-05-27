/// Generic alignment options for layout/rendering APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    /// Align to left edge.
    Left,
    /// Align to center.
    Center,
    /// Align to right edge.
    Right,
    /// Align to top edge.
    Top,
    /// Align to bottom edge.
    Bottom,
}
/// Horizontal alignment options for widget and layout APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}
/// Vertical alignment options for widget and layout APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}
impl HorizontalAlignment {
    /// Maps generic alignment to horizontal alignment when possible.
    pub const fn from_alignment(alignment: Alignment) -> Option<Self> {
        match alignment {
            Alignment::Left => Some(Self::Left),
            Alignment::Center => Some(Self::Center),
            Alignment::Right => Some(Self::Right),
            Alignment::Top | Alignment::Bottom => None,
        }
    }
}
impl VerticalAlignment {
    /// Maps generic alignment to vertical alignment when possible.
    pub const fn from_alignment(alignment: Alignment) -> Option<Self> {
        match alignment {
            Alignment::Top => Some(Self::Top),
            Alignment::Center => Some(Self::Center),
            Alignment::Bottom => Some(Self::Bottom),
            Alignment::Left | Alignment::Right => None,
        }
    }
}
impl HorizontalAlignment {
    /// Creates horizontal alignment from string.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "left" | "l" => Some(Self::Left),
            "center" | "centre" | "c" => Some(Self::Center),
            "right" | "r" => Some(Self::Right),
            _ => None,
        }
    }
    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
    /// Returns true if alignment is left.
    pub const fn is_left(&self) -> bool {
        matches!(self, Self::Left)
    }
    /// Returns true if alignment is center.
    pub const fn is_center(&self) -> bool {
        matches!(self, Self::Center)
    }
    /// Returns true if alignment is right.
    pub const fn is_right(&self) -> bool {
        matches!(self, Self::Right)
    }
}
impl VerticalAlignment {
    /// Creates vertical alignment from string.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "top" | "t" => Some(Self::Top),
            "center" | "centre" | "c" => Some(Self::Center),
            "bottom" | "b" => Some(Self::Bottom),
            _ => None,
        }
    }
    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
    /// Returns true if alignment is top.
    pub const fn is_top(&self) -> bool {
        matches!(self, Self::Top)
    }
    /// Returns true if alignment is center.
    pub const fn is_center(&self) -> bool {
        matches!(self, Self::Center)
    }
    /// Returns true if alignment is bottom.
    pub const fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }
}
impl Alignment {
    /// Creates alignment from string.
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "left" | "l" => Some(Self::Left),
            "center" | "centre" | "c" => Some(Self::Center),
            "right" | "r" => Some(Self::Right),
            "top" | "t" => Some(Self::Top),
            "bottom" | "b" => Some(Self::Bottom),
            _ => None,
        }
    }
    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
    /// Returns true if alignment is horizontal.
    pub const fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Center | Self::Right)
    }
    /// Returns true if alignment is vertical.
    pub const fn is_vertical(&self) -> bool {
        matches!(self, Self::Top | Self::Center | Self::Bottom)
    }
    /// Converts to HorizontalAlignment if possible.
    pub const fn to_horizontal(&self) -> Option<HorizontalAlignment> {
        match self {
            Self::Left => Some(HorizontalAlignment::Left),
            Self::Center => Some(HorizontalAlignment::Center),
            Self::Right => Some(HorizontalAlignment::Right),
            Self::Top | Self::Bottom => None,
        }
    }
    /// Converts to VerticalAlignment if possible.
    pub const fn to_vertical(&self) -> Option<VerticalAlignment> {
        match self {
            Self::Top => Some(VerticalAlignment::Top),
            Self::Center => Some(VerticalAlignment::Center),
            Self::Bottom => Some(VerticalAlignment::Bottom),
            Self::Left | Self::Right => None,
        }
    }
    /// Returns true if alignment is left.
    pub const fn is_left(&self) -> bool {
        matches!(self, Self::Left)
    }
    /// Returns true if alignment is center.
    pub const fn is_center(&self) -> bool {
        matches!(self, Self::Center)
    }
    /// Returns true if alignment is right.
    pub const fn is_right(&self) -> bool {
        matches!(self, Self::Right)
    }
    /// Returns true if alignment is top.
    pub const fn is_top(&self) -> bool {
        matches!(self, Self::Top)
    }
    /// Returns true if alignment is bottom.
    pub const fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }
    /// Creates a combined alignment from horizontal and vertical components.
    pub fn from_components(
        horizontal: HorizontalAlignment,
        vertical: VerticalAlignment,
    ) -> (Self, Self) {
        (horizontal.into(), vertical.into())
    }
    /// Returns the opposite alignment.
    pub const fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Center => Self::Center,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
    /// Returns CSS text-align value for horizontal alignments.
    pub fn css_text_align(&self) -> Option<&'static str> {
        match self {
            Self::Left => Some("left"),
            Self::Center => Some("center"),
            Self::Right => Some("right"),
            _ => None,
        }
    }
    /// Returns CSS vertical-align value for vertical alignments.
    pub fn css_vertical_align(&self) -> Option<&'static str> {
        match self {
            Self::Top => Some("top"),
            Self::Center => Some("middle"),
            Self::Bottom => Some("bottom"),
            _ => None,
        }
    }
}
impl From<HorizontalAlignment> for Alignment {
    fn from(value: HorizontalAlignment) -> Self {
        match value {
            HorizontalAlignment::Left => Self::Left,
            HorizontalAlignment::Center => Self::Center,
            HorizontalAlignment::Right => Self::Right,
        }
    }
}
impl From<VerticalAlignment> for Alignment {
    fn from(value: VerticalAlignment) -> Self {
        match value {
            VerticalAlignment::Top => Self::Top,
            VerticalAlignment::Center => Self::Center,
            VerticalAlignment::Bottom => Self::Bottom,
        }
    }
}
impl TryFrom<Alignment> for HorizontalAlignment {
    type Error = ();

    fn try_from(value: Alignment) -> Result<Self, Self::Error> {
        match value {
            Alignment::Left => Ok(Self::Left),
            Alignment::Center => Ok(Self::Center),
            Alignment::Right => Ok(Self::Right),
            _ => Err(()),
        }
    }
}
impl TryFrom<Alignment> for VerticalAlignment {
    type Error = ();

    fn try_from(value: Alignment) -> Result<Self, Self::Error> {
        match value {
            Alignment::Top => Ok(Self::Top),
            Alignment::Center => Ok(Self::Center),
            Alignment::Bottom => Ok(Self::Bottom),
            _ => Err(()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn axis_alignment_mapping_is_explicit() {
        assert_eq!(
            HorizontalAlignment::from_alignment(Alignment::Left),
            Some(HorizontalAlignment::Left)
        );
        assert_eq!(HorizontalAlignment::from_alignment(Alignment::Top), None);
        assert_eq!(
            VerticalAlignment::from_alignment(Alignment::Bottom),
            Some(VerticalAlignment::Bottom)
        );
        assert_eq!(VerticalAlignment::from_alignment(Alignment::Right), None);
    }
}
