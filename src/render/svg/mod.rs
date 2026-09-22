// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
// Public because it is the *definition* of the SVG stream's textual form: a test that asserts on an
// emitted element's attributes has to build the same strings the backend does, and a private helper
// would force every such test to re-implement `color_to_rgba`/`rect_attrs` and drift from the
// producer. Exposing the formatter keeps "the snapshot says X" and "the producer writes X" one fact
// rather than two.
pub mod convert;

pub use backend::SvgPaintBackend;
