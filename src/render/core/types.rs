//! Core rendering data types for text and geometry.

pub struct TextMetrics {
    /// Measured text width in logical pixels.
    pub width: u32,
    /// Measured text height in logical pixels.
    pub height: u32,
    /// Baseline ascent in logical pixels.
    pub ascent: u32,
    /// Baseline descent in logical pixels.
    pub descent: u32,
}
/// One shaped text cluster produced by the render text shaper.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    /// Cluster source text (one or more unicode scalars).
    pub text: String,
    /// Logical horizontal advance in pixels.
    pub advance: f32,
}
/// Shaped text run composed from ordered clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    pub(crate) clusters: Vec<TextCluster>,
    pub(crate) advance: f32,
}
impl ShapedText {
    /// Returns ordered text clusters in this shaped run.
    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }
    /// Returns cluster count in this shaped run.
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }
    /// Returns total horizontal advance in logical pixels.
    pub fn advance(&self) -> f32 {
        self.advance
    }
}
