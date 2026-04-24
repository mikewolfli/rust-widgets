//! Image structure for widget icons and favicons.

/// Image structure for widget icons and favicons.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    // In a real implementation, this would contain image data
    // For now, we'll just use a placeholder
    pub data: Vec<u8>,
}
impl Image {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
}
impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}
