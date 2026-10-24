// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Container utilities: SoftwareSurface lifecycle management, configuration,
//! frame operations, text shaping, gradient fill, and clip stack.
//!
//! Rendering primitives (rect, circle, line, text, etc.) are in the
//! `primitives` sub-module.

use crate::compat::atomic::{AtomicU64, Ordering};
use crate::compat::{HashMap, Rc, RefCell, Vec, VecDeque};
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::backend::surface::PathClip;
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

/// Shaped runs for one text epoch, keyed by the same measurement fingerprint as the metrics.
///
/// The values are `Rc<ShapedText>`: a hit is a refcount bump. `ShapedText` owns a
/// `Vec<TextCluster>` whose every entry owns a `String`, so an owned cache value made each hit
/// clone the whole run to hand back a value nothing mutates (REND-03).
type ShapeCache = RefCell<(u64, HashMap<(u64, u64), Rc<ShapedText>>)>;

thread_local! {
    // Two independently seeded hashes rather than an owning key: the pair is computed from the
    // borrowed inputs, so a lookup allocates nothing, and two 64-bit values make an accidental
    // match between different inputs effectively impossible. A cache miss is the only way an
    // input can reach the shaping path, so a collision would show as a wrong width, not as
    // corruption.
    static TEXT_METRICS: RefCell<(u64, TextMetricsCache)> =
        RefCell::new((0, HashMap::new()));
    // Insertion order, for FIFO eviction. The caches used to `clear()` when full, so one entry
    // over the bound discarded the whole working set and the next frame re-shaped everything.
    static TEXT_METRICS_ORDER: RefCell<VecDeque<(u64, u64)>> =
        const { RefCell::new(VecDeque::new()) };
    static TEXT_SHAPES: ShapeCache =
        RefCell::new((0, HashMap::new()));
    static TEXT_SHAPES_ORDER: RefCell<VecDeque<(u64, u64)>> =
        const { RefCell::new(VecDeque::new()) };
}

/// Returns two fingerprints over everything a text measurement depends on.
///
/// # Why the second pass hashes a summary instead of the whole string
///
/// The pair only has to be a *function of the same inputs* — a cache collision needs both halves to
/// match, and the first half is SipHash over every byte of the text, so the second half contributes
/// independence rather than coverage. Hashing the entire text a second time therefore doubled the
/// cost of every lookup for no extra safety (REND-14); it now hashes the length plus the first and
/// last eight bytes, which still distinguishes the strings a label cache actually sees (a status
/// line that ends differently, a number that grew) without reading the middle again.
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

    let bytes = text.as_bytes();
    let head = &bytes[..bytes.len().min(8)];
    let tail = &bytes[bytes.len().saturating_sub(8)..];

    let mut second = std::collections::hash_map::DefaultHasher::new();
    0xA5u8.hash(&mut second);
    bytes.len().hash(&mut second);
    head.hash(&mut second);
    tail.hash(&mut second);
    scale.hash(&mut second);
    line.hash(&mut second);
    word.hash(&mut second);
    letter.hash(&mut second);
    font.is_italic().hash(&mut second);
    font.is_bold().hash(&mut second);
    font.weight().hash(&mut second);
    size.hash(&mut second);
    font.family().hash(&mut second);

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
            path_clips: Vec::new(),
            nonzero_intersections: Vec::new(),
            text_coverage_scratch: Vec::new(),
            blend_mode: BlendMode::Normal,
            glyph_run_text: false,
        }
    }

    /// Sets whether text is painted glyph by glyph through a shaped run.
    ///
    /// Off by default. Turning it on is an opt-in rendering mode: when a face matches
    /// `Font::family`, `draw_text` shapes the line with the face's standard ligatures enabled and
    /// fills each glyph's outline, so `ffi` is drawn as the face's ligature. A face the build does
    /// not ship leaves the cluster renderer in charge, unchanged.
    ///
    /// The measured advance of a glyph-run line comes from the shaped run (`letter_spacing`
    /// included), not from `shape_text`; a caller that mixes the two for the same string can see
    /// a difference in width when the face applies a ligature.
    pub fn set_glyph_run_text(&mut self, enabled: bool) {
        if self.glyph_run_text == enabled {
            return;
        }
        self.glyph_run_text = enabled;
    }

    /// Whether text is painted glyph by glyph through a shaped run.
    pub fn glyph_run_text(&self) -> bool {
        self.glyph_run_text
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

    /// Reports `rect` as changed since the last present.
    ///
    /// Pass the region a frame painted rather than the whole surface and end the frame with
    /// [`Self::present_damage`]: the front buffer keeps the previous frame outside the reported
    /// rectangles, so an incremental repaint costs one copy per damaged row instead of a swap.
    /// A caller that redraws everything should keep using [`Self::end_frame`].
    pub fn mark_damage(&mut self, rect: Rect) {
        self.buffer.mark_damage(rect);
    }

    /// The damaged rectangles reported since the last present.
    pub fn damage_regions(&self) -> &[(i32, i32, u32, u32)] {
        self.buffer.damage_regions()
    }

    /// Forgets the reported damage without publishing it.
    pub fn clear_damage(&mut self) {
        self.buffer.clear_damage();
    }

    /// Publishes only the reported damage, returning whether anything was published.
    ///
    /// The back buffer must contain the previous frame outside the damaged regions (seed it from
    /// [`Self::frame_rgba`] or let the caller adopt the previous frame); this is what makes a
    /// region-sized present correct rather than a frame with stale holes.
    pub fn present_damage(&mut self) -> bool {
        self.buffer.present_damage()
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
                let _ = TEXT_METRICS_ORDER.try_with(|order| order.borrow_mut().clear());
            }
            TEXT_METRICS_ORDER.with(|order| {
                let mut order = order.borrow_mut();
                if order.len() >= TEXT_METRICS_CACHE_LIMIT {
                    if let Some(oldest) = order.pop_front() {
                        cache.1.remove(&oldest);
                    }
                }
                order.push_back(key);
            });
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
    pub fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        let scale = self.buffer.dpi_scale();
        let epoch = TEXT_METRICS_EPOCH.load(Ordering::Relaxed);
        let key = text_measurement_fingerprint(text, font, scale);
        // The cache holds `Rc<ShapedText>` and the function hands that handle out, so a hit is a
        // refcount bump. Returning an owned `ShapedText` cloned the whole run on every hit —
        // one `Vec<TextCluster>` plus one `String` per cluster, per label, per frame — to hand
        // back a value nobody mutates.
        if let Some(hit) = TEXT_SHAPES.with(|cache| {
            let cache = cache.borrow();
            (cache.0 == epoch).then(|| cache.1.get(&key).cloned()).flatten()
        }) {
            return hit;
        }
        let shaped = Rc::new(crate::render::text::shape_line(text, font, scale));
        note_shape_built();
        TEXT_SHAPES.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.0 != epoch {
                cache.0 = epoch;
                cache.1.clear();
                let _ = TEXT_SHAPES_ORDER.try_with(|order| order.borrow_mut().clear());
            }
            TEXT_SHAPES_ORDER.with(|order| {
                let mut order = order.borrow_mut();
                if order.len() >= TEXT_SHAPES_CACHE_LIMIT {
                    if let Some(oldest) = order.pop_front() {
                        cache.1.remove(&oldest);
                    }
                }
                order.push_back(key);
            });
            cache.1.insert(key, Rc::clone(&shaped));
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

        // Resolve the ramp once into a 256-entry table. Every pixel previously ran
        // `Gradient::interpolate`, which searches the stops and lerps each channel; for a
        // full-window gradient that was width×height stop searches per frame. Sampling a
        // fixed table is both cheaper and consistent across the three gradient types.
        const LUT_SIZE: usize = 256;
        let mut lut = [Color::BLACK; LUT_SIZE];
        for (index, slot) in lut.iter_mut().enumerate() {
            *slot = gradient.interpolate(index as f32 / (LUT_SIZE - 1) as f32);
        }
        let lut_index = |pos: f32| -> usize {
            ((pos.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32) as usize).min(LUT_SIZE - 1)
        };

        // Linear geometry is constant across the whole rectangle: hoist it and walk the
        // projection with an increment per pixel instead of recomputing it from x/y.
        let linear_dx = gradient.end_point.x as f32 - gradient.start_point.x as f32;
        let linear_dy = gradient.end_point.y as f32 - gradient.start_point.y as f32;
        let linear_dot = linear_dx * linear_dx + linear_dy * linear_dy;
        let linear_step = if linear_dot == 0.0 { 0.0 } else { linear_dx / linear_dot };

        for y in y0..y1 {
            let row_start = if linear_dot == 0.0 {
                0.0
            } else {
                ((x0 as f32 - gradient.start_point.x as f32) * linear_dx
                    + (y as f32 - gradient.start_point.y as f32) * linear_dy)
                    / linear_dot
            };
            for x in x0..x1 {
                let pos = match gradient.gradient_type {
                    GradientType::Linear => {
                        if linear_dot == 0.0 {
                            0.0
                        } else {
                            (row_start + (x - x0) as f32 * linear_step).clamp(0.0, 1.0)
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
                let color = lut[lut_index(pos)];
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
        // A rectangle clip is the `None` member of the path-clip stack. Keeping the two stacks
        // the same height is what lets `pop_clip` release exactly one entry of each, whichever
        // kind of clip was pushed.
        self.path_clips.push(None);
    }
    /// Pops the top clip rectangle from the clip stack.
    ///
    /// When the popped entry was a path clip (pushed by `RenderCommand::ClipPath`), the path's
    /// coverage mask is applied
    /// to the pixels drawn since: fully covered pixels stay, uncovered pixels are replaced by the
    /// backdrop captured at the push, and partial coverage composites the two. The mask therefore
    /// clips every primitive exactly, including the antialiased edge of the path itself.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        let Some(Some(clip)) = self.path_clips.pop() else { return };
        let size = self.buffer.size();
        apply_path_clip(&mut self.buffer.back, size, &clip);
    }

    /// Pushes a path clip: a rectangle clip over the path's bounds plus an exact coverage mask.
    ///
    /// The rectangle half restricts every primitive to the path's bounding box, which is the cheap
    /// early-out. The mask half is what makes the clip exact: at the matching
    /// [`Self::pop_clip`] every pixel inside the box that the path does not cover is restored from
    /// the backdrop captured here, so no primitive needs a path-aware raster path of its own.
    ///
    /// A degenerate path (fewer than three points) pushes an empty rectangle, so a malformed clip
    /// draws nothing rather than falling back to an unclipped surface.
    pub(crate) fn push_path_clip(&mut self, points: &[Point]) {
        let bounds = path_bounds(points);
        match bounds {
            Some((x, y, width, height)) => self.push_clip(x, y, width, height),
            None => self.push_clip(0, 0, 0, 0),
        }
        let size = self.buffer.size();
        let Some(rect) = visible_path_region(self.current_clip(), size) else {
            return;
        };
        let coverage = path_coverage(points, rect);
        let backdrop = capture_region(self.buffer.back.as_slice(), size.width as usize, rect);
        if let Some(slot) = self.path_clips.last_mut() {
            *slot = Some(PathClip { rect, coverage, backdrop });
        }
    }
    /// Returns the current effective clip rect, or `None` if the clip stack is empty.
    pub fn current_clip(&self) -> Option<(i32, i32, u32, u32)> {
        self.clip_stack.last().copied()
    }
}

/// The bounding box of `points`, or `None` when the path cannot enclose an area.
fn path_bounds(points: &[Point]) -> Option<(i32, i32, u32, u32)> {
    if points.len() < 3 {
        return None;
    }
    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;
    for point in points {
        min_x = min_x.min(point.x as i64);
        min_y = min_y.min(point.y as i64);
        max_x = max_x.max(point.x as i64);
        max_y = max_y.max(point.y as i64);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if width <= 0 || height <= 0 {
        return None;
    }
    // Clamp the origin into `i32` before casting: an extreme pair of points can overflow the
    // `i32` differences even when their ordering is valid.
    let x = min_x.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y = min_y.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let width = width.min(u32::MAX as i64) as u32;
    let height = height.min(u32::MAX as i64) as u32;
    Some((x, y, width, height))
}

/// The part of `clip` inside `size`, or `None` when there is no visible part.
fn visible_path_region(
    clip: Option<(i32, i32, u32, u32)>,
    size: Size,
) -> Option<(i32, i32, u32, u32)> {
    let (x, y, width, height) = clip?;
    let x0 = (x as i64).max(0);
    let y0 = (y as i64).max(0);
    let x1 = (x as i64 + width as i64).min(size.width as i64);
    let y1 = (y as i64 + height as i64).min(size.height as i64);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Vertical sub-samples per pixel row used to antialias a path clip's edge.
const PATH_CLIP_SUBSAMPLES: usize = 4;

/// Rasterises `points` (implicitly closed) into per-pixel coverage over `rect`.
///
/// Each pixel row is sampled on [`PATH_CLIP_SUBSAMPLES`] sub-scanlines. On each sub-scanline the
/// non-zero winding rule produces horizontal spans, whose exact overlap with each pixel column is
/// accumulated. Four sub-rows therefore give 4×1 horizontal resolution (the horizontal overlap is
/// analytic, not sampled), which is the same order of antialiasing the geometry rasteriser uses
/// for a polygon edge.
fn path_coverage(points: &[Point], rect: (i32, i32, u32, u32)) -> Vec<u8> {
    let (_, y0, width, height) = rect;
    let pixel_count = width as usize * height as usize;
    let mut coverage = vec![0u8; pixel_count];
    if points.len() < 3 || width == 0 || height == 0 {
        return coverage;
    }
    let mut accumulated = vec![0f32; pixel_count];
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    for row in 0..height as usize {
        for sub in 0..PATH_CLIP_SUBSAMPLES {
            let sample_y =
                y0 as f32 + row as f32 + (sub as f32 + 0.5) / PATH_CLIP_SUBSAMPLES as f32;
            crossings.clear();
            for index in 0..points.len() {
                let first = points[index];
                let second = points[(index + 1) % points.len()];
                let first_y = first.y as f32;
                let second_y = second.y as f32;
                let winding = if first_y <= sample_y && second_y > sample_y {
                    1
                } else if second_y <= sample_y && first_y > sample_y {
                    -1
                } else {
                    continue;
                };
                let t = (sample_y - first_y) / (second_y - first_y);
                let x = first.x as f32 + (second.x - first.x) as f32 * t;
                crossings.push((x, winding));
            }
            crossings.sort_by(|left, right| {
                left.0.partial_cmp(&right.0).unwrap_or(core::cmp::Ordering::Equal)
            });
            let mut index = 0;
            let mut winding = 0;
            let mut span_start: Option<f32> = None;
            while index < crossings.len() {
                let x = crossings[index].0;
                if winding != 0 {
                    if let Some(start) = span_start {
                        add_span(&mut accumulated, width, row, start, x);
                    }
                }
                let mut delta = 0;
                while index < crossings.len() && crossings[index].0 == x {
                    delta += crossings[index].1;
                    index += 1;
                }
                winding += delta;
                span_start = Some(x);
            }
        }
    }
    let scale = 255.0 / PATH_CLIP_SUBSAMPLES as f32;
    for (slot, value) in coverage.iter_mut().zip(accumulated.iter()) {
        *slot = (value * scale).round().clamp(0.0, 255.0) as u8;
    }
    coverage
}

/// Adds the horizontal overlap of the span `start..end` with each pixel of `row`.
fn add_span(accumulated: &mut [f32], width: u32, row: usize, start: f32, end: f32) {
    if !(start.is_finite() && end.is_finite()) || end <= start {
        return;
    }
    let width_f = width as f32;
    let start = start.max(0.0);
    let end = end.min(width_f);
    if end <= start {
        return;
    }
    let first = start.floor() as usize;
    let last = (end.ceil() as usize).min(width as usize);
    let row_start = row * width as usize;
    for column in first..last {
        let left = (column as f32).max(start);
        let right = (column as f32 + 1.0).min(end);
        if right > left {
            accumulated[row_start + column] += right - left;
        }
    }
}

/// Applies a popped path clip's mask to the pixels it covers.
fn apply_path_clip(frame: &mut [u8], size: Size, clip: &PathClip) {
    let (x, y, width, height) = clip.rect;
    if width == 0 || height == 0 {
        return;
    }
    let pixel_count = width as usize * height as usize;
    if clip.coverage.len() != pixel_count || clip.backdrop.len() != pixel_count * 4 {
        return;
    }
    let surface_width = size.width as usize;
    for row in 0..height as usize {
        for column in 0..width as usize {
            let mask_index = row * width as usize + column;
            let coverage = clip.coverage[mask_index];
            if coverage == 255 {
                continue;
            }
            let frame_index = ((y as usize + row) * surface_width + x as usize + column) * 4;
            let backdrop_index = mask_index * 4;
            let Some(pixel) = frame.get_mut(frame_index..frame_index + 4) else { continue };
            let backdrop = &clip.backdrop[backdrop_index..backdrop_index + 4];
            if coverage == 0 {
                pixel.copy_from_slice(backdrop);
                continue;
            }
            // Masked composite: the backdrop is the base and the clipped painting is the source,
            // weighted by the path's coverage. That is exactly source-over with a scaled source
            // alpha, so the partial edge of the path blends the same way a partial glyph does.
            let foreground = [pixel[0], pixel[1], pixel[2], pixel[3]];
            pixel.copy_from_slice(backdrop);
            crate::render::pipeline::pixel_ops::blend_pixel(
                frame,
                surface_width as u32,
                (x as usize + column) as u32,
                (y as usize + row) as u32,
                Color::rgba(foreground[0], foreground[1], foreground[2], foreground[3]),
                coverage as f32 / 255.0,
            );
        }
    }
}

/// Copies a clipped region out of `back` into a tightly packed RGBA buffer.
fn capture_region(back: &[u8], surface_width: usize, rect: (i32, i32, u32, u32)) -> Vec<u8> {
    let (x, y, width, height) = rect;
    let row_bytes = width as usize * 4;
    let mut out = vec![0u8; row_bytes * height as usize];
    for row in 0..height as usize {
        let source = (y as usize + row) * surface_width * 4 + x as usize * 4;
        let destination = row * row_bytes;
        if let (Some(source), Some(target)) = (
            back.get(source..source + row_bytes),
            out.get_mut(destination..destination + row_bytes),
        ) {
            target.copy_from_slice(source);
        }
    }
    out
}

// Test-only counters of how many times the shaping and measurement work actually ran. They make
// "the cache is on the path" observable, because equal results alone cannot distinguish a hit
// from a re-computation.
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static TEXT_SHAPES_BUILT: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    #[allow(clippy::missing_const_for_thread_local)]
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
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
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

    /// A cache hit hands back the **same** shaped run, not a copy of it.
    ///
    /// `ShapedText` owns a `Vec<TextCluster>` and every cluster owns a `String`, so returning it by
    /// value cloned all of that on every hit: the cache avoided the shaping cost and paid for it in
    /// allocator traffic instead. The handle comparison is the evidence that nothing was cloned;
    /// the build counter is the evidence that the value came from the cache rather than a rebuild.
    #[test]
    fn a_shaped_run_hit_is_the_same_allocation_not_a_copy() {
        #[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
        let surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        let font = Font::default();

        let first = surface.shape_text("hello", &font);
        let _ = take_builds();
        let second = surface.shape_text("hello", &font);

        assert!(Rc::ptr_eq(&first, &second), "a cache hit must return the cached run itself");
        assert_eq!(take_builds(), (0, 0), "and it must not have shaped anything");
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
        let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
        let surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        let font = Font::default();
        let _ = surface.measure_text("hello", &font);
        let _ = take_builds();

        invalidate_text_measurements();
        let _ = surface.measure_text("hello", &font);
        assert_eq!(take_builds(), (1, 1), "a new epoch must miss the old entry");
    }

    /// The two fingerprints still separate inputs the summary half cannot see.
    ///
    /// # What this pins
    ///
    /// The second half of the key hashes the text's length plus its first and last eight bytes
    /// rather than every byte (REND-14). Two strings that agree on that summary — a long middle
    /// differing, which is exactly what an edited line or a changed number looks like — must still
    /// get different keys, or the cache would serve one label's width for another. The full-text
    /// first half is what makes that true, and this asserts it end to end through the pair.
    #[test]
    fn fingerprints_separate_texts_that_share_a_summary() {
        let font = Font::default();
        let head = "head-head-";
        let tail = "-tail-tail";
        let a = format!("{head}alpha{tail}");
        let b = format!("{head}bravo{tail}");
        assert_eq!(a.len(), b.len(), "the fixture must agree on the summary's length");
        assert_eq!(
            fingerprint_summary(&a),
            fingerprint_summary(&b),
            "and on the head/tail bytes, or this test proves nothing"
        );

        let key_a = text_measurement_fingerprint(&a, &font, 1.0);
        let key_b = text_measurement_fingerprint(&b, &font, 1.0);
        assert_ne!(key_a, key_b, "different middles must still be different keys");

        // The pair is a function, not a random value.
        assert_eq!(text_measurement_fingerprint(&a, &font, 1.0), key_a);
        // And a font difference is part of the key on both halves.
        let mut bigger = Font::default();
        bigger.set_size(28.0);
        assert_ne!(text_measurement_fingerprint(&a, &bigger, 1.0), key_a);
    }

    /// The summary the second fingerprint is built from, for the test above.
    fn fingerprint_summary(text: &str) -> (usize, &[u8], &[u8]) {
        let bytes = text.as_bytes();
        (bytes.len(), &bytes[..bytes.len().min(8)], &bytes[bytes.len().saturating_sub(8)..])
    }

    /// Reads one pixel out of a presented frame.
    fn pixel(frame: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * width + x) * 4) as usize;
        [frame[offset], frame[offset + 1], frame[offset + 2], frame[offset + 3]]
    }

    /// A path clip removes the pixels its bounding box would otherwise keep.
    ///
    /// The triangle covers the lower-left half of a 4×4 box. A rectangle clip (the previous
    /// implementation) would leave the upper-right triangle untouched too, so asserting that the
    /// upper-right corner is still the backdrop is the evidence that the mask, not the box, is
    /// what clipped the fill.
    #[test]
    fn a_path_clip_keeps_only_the_pixels_the_path_covers() {
        let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
        surface.begin_frame(Color::WHITE);
        // A triangle occupying the lower-left half of the box.
        surface.push_path_clip(&[Point::new(0, 0), Point::new(0, 4), Point::new(4, 4)]);
        surface.fill_rect(Rect::new(0, 0, 4, 4), Color::RED);
        surface.pop_clip();
        surface.end_frame();

        let frame = surface.frame_rgba();
        assert_eq!(pixel(frame, 4, 0, 3), [255, 0, 0, 255], "inside the triangle");
        assert_eq!(pixel(frame, 4, 3, 0), [255, 255, 255, 255], "outside it, inside the box");
    }

    /// Popping a path clip restores the enclosed region's backdrop before the next draw.
    ///
    /// `PopClip` is what applies the mask, so a later primitive must see the restored pixels. If
    /// the mask leaked past the pop, the second fill would be clipped by the first path.
    #[test]
    fn popping_a_path_clip_restores_the_backdrop_for_the_next_draw() {
        let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.push_path_clip(&[Point::new(0, 0), Point::new(0, 2), Point::new(2, 2)]);
        surface.fill_rect(Rect::new(0, 0, 4, 4), Color::RED);
        surface.pop_clip();
        surface.fill_rect(Rect::new(3, 0, 1, 1), Color::GREEN);
        surface.end_frame();

        let frame = surface.frame_rgba();
        assert_eq!(pixel(frame, 4, 3, 0), [0, 255, 0, 255], "the later fill is unclipped");
        assert_eq!(pixel(frame, 4, 3, 3), [255, 255, 255, 255], "and the mask was removed");
    }

    /// A degenerate path draws nothing rather than silently not clipping.
    #[test]
    fn a_degenerate_path_clip_draws_nothing() {
        let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.push_path_clip(&[Point::new(1, 1), Point::new(2, 2)]);
        surface.fill_rect(Rect::new(0, 0, 4, 4), Color::RED);
        surface.pop_clip();
        surface.end_frame();
        assert!(
            surface
                .frame_rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [255, 255, 255, 255]),
            "an unclosed path encloses no area and must clip everything away"
        );
    }
}
