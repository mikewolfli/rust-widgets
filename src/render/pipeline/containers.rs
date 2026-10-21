// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Container utilities: SoftwareSurface lifecycle management, configuration,
//! frame operations, text shaping, gradient fill, and clip stack.
//!
//! Rendering primitives (rect, circle, line, text, etc.) are in the
//! `primitives` sub-module.

use crate::compat::atomic::{AtomicU64, Ordering};
use crate::compat::{HashMap, RefCell, Vec};
use crate::core::{Color, Font, Rect, Size};
use crate::render::default_software_render_config;
use crate::render::pipeline::pixel_ops::{fill_pixels, pixel_visible, write_pixel_with_mode};
use crate::render::{
    BackBuffer, BlendMode, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextMetrics,
};
use crate::style::Gradient;
use crate::style::GradientType;

/// Changes whenever the text engine's answer for a `(text, font)` pair can change.
///
/// The measurement cache below is keyed by that pair, and the pair alone is not quite the whole
/// input: a host can register a runtime face (`runtime-fonts`), which replaces the bytes a family
/// resolves to without changing any `Font` field. Bumping this epoch on such a change makes the
/// cached widths unreachable instead of silently wrong. Initialised to one so the zero epoch is
/// never a valid key.
static TEXT_METRICS_EPOCH: AtomicU64 = AtomicU64::new(1);

/// Invalidates every cached text measurement.
///
/// Called when the set of faces a family resolves to changes (runtime font registration/clearing).
/// A text cache keyed only by `(text, font)` cannot see that change on its own.
///
/// Only a build with runtime fonts has such a change: the compiled faces are fixed for the
/// process, so every other profile can keep its measurements forever.
#[cfg(feature = "runtime-fonts")]
pub(crate) fn invalidate_text_measurements() {
    TEXT_METRICS_EPOCH.fetch_add(1, Ordering::Relaxed);
}

/// The largest number of measurements the per-thread cache keeps.
const TEXT_METRICS_CACHE_LIMIT: usize = 2048;

/// The largest number of shaped runs the per-thread cache keeps.
///
/// Shaped runs are larger than measurements (clusters own their text), so this bound is tighter.
/// A repaint asks for the same captions frame after frame, which is the set this keeps.
const TEXT_SHAPES_CACHE_LIMIT: usize = 512;

/// A cached measurement: the two fingerprints of `(text, font, scale)` and the answer.
type TextMetricsCache = HashMap<(u64, u64), TextMetrics>;

thread_local! {
    // Two independently seeded hashes rather than an owning key: the pair is computed from the
    // borrowed inputs, so a lookup allocates nothing, and two 64-bit values make an accidental
    // match between different inputs effectively impossible. A cache miss is the only way an
    // input can reach the shaping path, so a collision would show as a wrong width, not as
    // corruption.
    static TEXT_METRICS: RefCell<(u64, TextMetricsCache)> =
        RefCell::new((0, HashMap::new()));
    static TEXT_SHAPES: RefCell<(u64, HashMap<(u64, u64), ShapedText>)> =
        RefCell::new((0, HashMap::new()));
}

/// Returns two fingerprints over everything a text measurement depends on.
fn text_measurement_fingerprint(text: &str, font: &Font, scale: f32) -> (u64, u64) {
    use core::hash::{Hash, Hasher};
    let size = font.size().to_bits();
    let letter = font.letter_spacing().to_bits();
    let word = font.word_spacing().to_bits();
    let line = font.line_height().to_bits();
    let scale = scale.to_bits();

    let mut first = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut first);
    font.family().hash(&mut first);
    size.hash(&mut first);
    font.weight().hash(&mut first);
    font.is_bold().hash(&mut first);
    font.is_italic().hash(&mut first);
    letter.hash(&mut first);
    word.hash(&mut first);
    line.hash(&mut first);
    scale.hash(&mut first);

    let mut second = std::collections::hash_map::DefaultHasher::new();
    0xA5u8.hash(&mut second);
    scale.hash(&mut second);
    line.hash(&mut second);
    word.hash(&mut second);
    letter.hash(&mut second);
    font.is_italic().hash(&mut second);
    font.is_bold().hash(&mut second);
    font.weight().hash(&mut second);
    size.hash(&mut second);
    font.family().hash(&mut second);
    text.hash(&mut second);

    (first.finish(), second.finish())
}

impl SoftwareSurface {
    /// Creates a software surface with size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let config = default_software_render_config();
        Self {
            buffer: BackBuffer::new(size, dpi_scale),
            aa_samples_per_axis: config.aa_samples_per_axis,
            clip_stack: Vec::new(),
            nonzero_intersections: Vec::new(),
            text_coverage_scratch: Vec::new(),
            blend_mode: BlendMode::Normal,
        }
    }

    /// Sets the blend mode every subsequent pixel write composites through.
    ///
    /// Driven by [`crate::render::RenderCommand::SetBlendMode`]. The mode is frame state, like the
    /// clip stack, because it applies to everything drawn until it changes rather than to one
    /// primitive. `Normal` restores plain source-over compositing.
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    /// The mode the next pixel write will composite through.
    pub fn blend_mode(&self) -> BlendMode {
        self.blend_mode
    }
    /// Get current software render configuration.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig { aa_samples_per_axis: self.aa_samples_per_axis }
    }
    /// Apply software render configuration.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        let normalized = config.normalized();
        self.aa_samples_per_axis = normalized.aa_samples_per_axis;
    }
    /// Set anti-aliasing sample grid size per axis for high-sample raster paths.
    pub fn set_aa_samples_per_axis(&mut self, samples: u8) {
        self.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: samples });
    }
    /// Get anti-aliasing sample grid size per axis.
    pub fn aa_samples_per_axis(&self) -> u8 {
        self.aa_samples_per_axis
    }
    /// Clears the current back buffer with a solid color.
    pub fn begin_frame(&mut self, clear: Color) {
        fill_pixels(self.buffer.back_mut(), clear);
    }
    /// Presents the back buffer as the current frame.
    pub fn end_frame(&mut self) {
        self.buffer.present();
    }
    /// Returns logical surface size.
    pub fn size(&self) -> Size {
        self.buffer.size()
    }
    /// Resizes the surface buffers.
    pub fn resize(&mut self, size: Size) {
        self.buffer.resize(size);
    }
    /// Sets logical DPI scale for text and geometry.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.buffer.set_dpi_scale(dpi_scale);
    }
    /// Returns logical DPI scale.
    pub fn dpi_scale(&self) -> f32 {
        self.buffer.dpi_scale()
    }
    /// Returns RGBA bytes of the presented frame.
    pub fn frame_rgba(&self) -> &[u8] {
        self.buffer.front()
    }

    /// Consumes the surface and returns the presented frame without copying it.
    ///
    /// See [`BackBuffer::take_front`] for why this exists: the render entry points own
    /// the backend, so moving the buffer is always safe there and saves one full-frame
    /// copy per painted frame.
    pub fn into_frame_rgba(mut self) -> Vec<u8> {
        self.buffer.take_front()
    }
    /// Measures text bounds and baseline metrics.
    ///
    /// The line box is the font's **effective** line height, so an explicit `line_height` — the
    /// field a text scale needs — actually sets how tall a line of this font is. The
    /// height/ascent/descent triple comes from `TextMetrics::for_font` rather than being derived
    /// here, because the SVG backend derives the same triple and the two had drifted.
    ///
    /// # Why the result is cached
    ///
    /// `measure_text` shapes the line (`shape_text`), which is the expensive half of laying out a
    /// label: it segments clusters, resolves a face per cluster and collects advances. A single
    /// `draw_text_fitted` asks for the same string **twice** (once to decide whether it fits and
    /// once to place the fitted result), and layout asks again for the same caption on every
    /// frame. Measured on this crate's own benchmark, shaping was ~1.7µs a call, so an unscaled
    /// window paid milliseconds a frame for answers it had already computed.
    ///
    /// The answer depends only on the text, the font and the device scale; the epoch guards the
    /// one input a `Font` does not describe (which face a family resolves to).
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        let scale = self.buffer.dpi_scale();
        let epoch = TEXT_METRICS_EPOCH.load(Ordering::Relaxed);
        let key = text_measurement_fingerprint(text, font, scale);
        if let Some(hit) = TEXT_METRICS.with(|cache| {
            let cache = cache.borrow();
            (cache.0 == epoch).then(|| cache.1.get(&key).cloned()).flatten()
        }) {
            return hit;
        }

        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        let metrics = TextMetrics { width, ..TextMetrics::for_font(font, scale) };
        note_measurement_built();
        TEXT_METRICS.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.0 != epoch {
                cache.0 = epoch;
                cache.1.clear();
            }
            if cache.1.len() >= TEXT_METRICS_CACHE_LIMIT {
                cache.1.clear();
            }
            cache.1.insert(key, metrics);
        });
        metrics
    }
    /// Shape text into unicode-aware clusters, in **visual** order for painting.
    ///
    /// The derivation itself lives in [`crate::render::text`] (`shape_line`), so this backend and
    /// the SVG backend cannot drift: a change to clustering, advances or bidirectional reordering
    /// reaches both at once. All this method supplies is the device scale it already knows.
    ///
    /// # Why the result is cached
    ///
    /// Shaping is the expensive half of text: it segments the string, resolves a face for every
    /// cluster and collects advances (~1.7µs for a short caption, measured). `draw_text` asked for
    /// the shaped run and then `draw_text_fitted` asked again for the same string and font on the
    /// next frame; the answer is immutable for a given `(text, font, scale)`.
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        let scale = self.buffer.dpi_scale();
        let epoch = TEXT_METRICS_EPOCH.load(Ordering::Relaxed);
        let key = text_measurement_fingerprint(text, font, scale);
        if let Some(hit) = TEXT_SHAPES.with(|cache| {
            let cache = cache.borrow();
            (cache.0 == epoch).then(|| cache.1.get(&key).cloned()).flatten()
        }) {
            return hit;
        }
        let shaped = crate::render::text::shape_line(text, font, scale);
        note_shape_built();
        TEXT_SHAPES.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.0 != epoch {
                cache.0 = epoch;
                cache.1.clear();
            }
            if cache.1.len() >= TEXT_SHAPES_CACHE_LIMIT {
                cache.1.clear();
            }
            cache.1.insert(key, shaped.clone());
        });
        shaped
    }
    /// Fills a rectangle with a gradient.
    pub fn fill_rect_gradient(&mut self, rect: Rect, gradient: &Gradient) {
        let size = self.buffer.size();
        let clip = self.current_clip();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = rect.x.saturating_add(rect.width as i32).max(0) as u32;
        let y1 = rect.y.saturating_add(rect.height as i32).max(0) as u32;
        let x1 = x1.min(size.width);
        let y1 = y1.min(size.height);
        let frame = self.buffer.back_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                let pos = match gradient.gradient_type {
                    GradientType::Linear => {
                        let dx = gradient.end_point.x as f32 - gradient.start_point.x as f32;
                        let dy = gradient.end_point.y as f32 - gradient.start_point.y as f32;
                        let dot_len = dx * dx + dy * dy;
                        if dot_len == 0.0 {
                            0.0
                        } else {
                            let px = x as f32 - gradient.start_point.x as f32;
                            let py = y as f32 - gradient.start_point.y as f32;
                            ((px * dx + py * dy) / dot_len).clamp(0.0, 1.0)
                        }
                    }
                    GradientType::Radial => {
                        let dx = x as f32 - gradient.center.x as f32;
                        let dy = y as f32 - gradient.center.y as f32;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if gradient.radius <= 0.0 {
                            0.0
                        } else {
                            (dist / gradient.radius).clamp(0.0, 1.0)
                        }
                    }
                    GradientType::Conic => {
                        let dx = x as f32 - gradient.center.x as f32;
                        let dy = y as f32 - gradient.center.y as f32;
                        // The per-pixel angle is in **radians** (that is what `atan2` returns),
                        // while `Gradient::angle` is documented in **degrees** — so it is converted
                        // before being added. Adding it raw rotated the ramp by `angle` *radians*
                        // (~117° for the common `conic(center, 90.0)`) and made a caller's degree
                        // value meaningless. The same conversion is applied by the SVG backend's
                        // `conic_sample`, so the two agree.
                        let angle = dy.atan2(dx) + core::f32::consts::PI;
                        let start = gradient.angle.to_radians();
                        let angle = (angle + start) % core::f32::consts::TAU;
                        let angle =
                            if angle < 0.0 { angle + core::f32::consts::TAU } else { angle };
                        angle / core::f32::consts::TAU
                    }
                };
                let color = gradient.interpolate(pos);
                if pixel_visible(clip, x as i32, y as i32) {
                    // Through the blend-aware write, so a gradient drawn under a `SetBlendMode`
                    // composites the same way an opaque fill does.
                    write_pixel_with_mode(self.blend_mode, frame, size.width, x, y, color, 1.0);
                }
            }
        }
    }
    /// Pushes a clip rectangle onto the clip stack.
    /// The clip rectangle is intersected with any existing clip region.
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        if let Some(&(cx, cy, cw, ch)) = self.clip_stack.last() {
            let nx = x.max(cx);
            let ny = y.max(cy);
            let nx2 = (x.saturating_add(width as i32)).min(cx.saturating_add(cw as i32));
            let ny2 = (y.saturating_add(height as i32)).min(cy.saturating_add(ch as i32));
            let nw = (nx2 - nx).max(0) as u32;
            let nh = (ny2 - ny).max(0) as u32;
            self.clip_stack.push((nx, ny, nw, nh));
        } else {
            self.clip_stack.push((x, y, width, height));
        }
    }
    /// Pops the top clip rectangle from the clip stack.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }
    /// Returns the current effective clip rect, or `None` if the clip stack is empty.
    pub fn current_clip(&self) -> Option<(i32, i32, u32, u32)> {
        self.clip_stack.last().copied()
    }
}

// Test-only counters of how many times the shaping and measurement work actually ran. They make
// "the cache is on the path" observable, because equal results alone cannot distinguish a hit
// from a re-computation.
#[cfg(test)]
thread_local! {
    static TEXT_SHAPES_BUILT: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static TEXT_METRICS_BUILT: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn note_shape_built() {
    TEXT_SHAPES_BUILT.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
fn note_shape_built() {}

#[cfg(test)]
fn note_measurement_built() {
    TEXT_METRICS_BUILT.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
fn note_measurement_built() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Font, Size};
    use crate::render::SoftwareSurface;

    fn take_builds() -> (usize, usize) {
        let shapes = TEXT_SHAPES_BUILT.with(|count| count.replace(0));
        let metrics = TEXT_METRICS_BUILT.with(|count| count.replace(0));
        (shapes, metrics)
    }

    /// A repeated measurement and shape answer from the cache instead of recomputing.
    #[test]
    fn repeated_text_measurements_are_cached() {
        // A host face registered by another test (in this process) invalidates the measurement
        // cache, which would turn the expected hit below into a miss. The registry lock is the
        // one serialisation point for that: hold it for the whole sequence.
        #[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
        let _guard = crate::render::text::runtime_fonts::test_lock();
        let surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        let font = Font::default();

        let warm = surface.measure_text("hello", &font);
        let (shapes, metrics) = take_builds();
        assert_eq!((shapes, metrics), (1, 1), "the first measurement has to build its answer");

        assert_eq!(surface.measure_text("hello", &font), warm);
        assert_eq!(take_builds(), (0, 0), "a repeated measurement must be a cache hit");

        let _ = surface.shape_text("hello", &font);
        assert_eq!(take_builds(), (0, 0), "a repeated shape must be a cache hit too");

        let _ = surface.measure_text("world", &font);
        let (shapes, metrics) = take_builds();
        assert_eq!((shapes, metrics), (1, 1), "a different string is a different key");
    }

    /// The epoch hook makes every cached measurement unreachable.
    ///
    /// Runtime font registration replaces the bytes a family resolves to, which no `Font` field
    /// describes. Without this the cache would keep serving advances from the old face.
    #[cfg(feature = "runtime-fonts")]
    #[test]
    fn invalidating_text_measurements_forces_a_rebuild() {
        // Same reason as `repeated_text_measurements_are_cached`: the build counters are process-
        // wide and a parallel registration also bumps the epoch this test is about.
        #[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
        let _guard = crate::render::text::runtime_fonts::test_lock();
        let surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        let font = Font::default();
        let _ = surface.measure_text("hello", &font);
        let _ = take_builds();

        invalidate_text_measurements();
        let _ = surface.measure_text("hello", &font);
        assert_eq!(take_builds(), (1, 1), "a new epoch must miss the old entry");
    }
}
