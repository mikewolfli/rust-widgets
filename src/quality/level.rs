//! Rendering quality level enum.
/// Rendering quality levels for adaptive performance control.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::quality::QualityLevel;
///
/// let high = QualityLevel::High;
/// let medium = QualityLevel::Medium;
/// let low = QualityLevel::Low;
///
/// assert!(low < medium);
/// assert!(medium < high);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualityLevel {
    /// High quality: full effects including anti-aliasing, shadows, complex shaders.
    #[default]
    High,
    /// Medium quality: basic effects with simple shaders, no shadows.
    Medium,
    /// Low quality: minimal rendering with solid fills, no textures, may skip non-critical elements.
    Low,
}
impl PartialOrd for QualityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QualityLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (QualityLevel::High, QualityLevel::High) => std::cmp::Ordering::Equal,
            (QualityLevel::High, _) => std::cmp::Ordering::Greater,
            (_, QualityLevel::High) => std::cmp::Ordering::Less,
            (QualityLevel::Medium, QualityLevel::Medium) => std::cmp::Ordering::Equal,
            (QualityLevel::Medium, QualityLevel::Low) => std::cmp::Ordering::Greater,
            (QualityLevel::Low, QualityLevel::Medium) => std::cmp::Ordering::Less,
            (QualityLevel::Low, QualityLevel::Low) => std::cmp::Ordering::Equal,
        }
    }
}
impl QualityLevel {
    /// Returns the next lower quality level, if any.
    pub fn lower(&self) -> Option<Self> {
        match self {
            Self::High => Some(Self::Medium),
            Self::Medium => Some(Self::Low),
            Self::Low => None,
        }
    }
    /// Returns the next higher quality level, if any.
    pub fn higher(&self) -> Option<Self> {
        match self {
            Self::Low => Some(Self::Medium),
            Self::Medium => Some(Self::High),
            Self::High => None,
        }
    }
}
