//! Special widgets: canvas, chart, grid, freeform shape, etc.
pub mod canvas;
pub mod chart;
pub mod freeform_shape;
pub mod grid;
// Re-export special widgets
pub use canvas::Canvas;
pub use chart::ChartWidget;
pub use freeform_shape::FreeformShapeWidget;
pub use grid::GridWidget;
