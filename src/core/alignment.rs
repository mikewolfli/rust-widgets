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
