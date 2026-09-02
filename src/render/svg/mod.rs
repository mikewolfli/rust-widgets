//! SVG rendering backend — converts `RenderCommand`s into SVG elements.
//!
//! This backend implements `PaintBackend` by generating SVG markup
//! instead of rasterizing pixels. Any widget's `Draw::draw()` method
//! can produce SVG output by simply swapping the backend.
//!
//! # Usage
//!
//! ```rust
//! use rust_widgets::render::svg::SvgPaintBackend;
//! use rust_widgets::render::PaintBackend;
//! use rust_widgets::core::{Color, Size};
//!
//! let mut svg = SvgPaintBackend::new(Size::new(100, 50));
//! svg.begin_frame(Color::WHITE);
//! svg.end_frame();
//! let output = svg.finish();
//! ```

pub mod backend;
mod convert;

pub use backend::SvgPaintBackend;
