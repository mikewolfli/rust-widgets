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
use crate::render::pipeline::pixel_ops::{fill_pixels, set_pixel, write_pixel_with_mode};
use crate::render::scratch::ScratchI32;
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

/// A cached measurement: the two fingerprints of `(text, font, scale)`, the text epoch it was
/// built under, and the answer.
///
/// The epoch travels with each entry instead of gating the whole table. Invalidating used to
/// `clear()` everything, so a DPI change or a font registration threw the entire hot set away and
/// the next frame re-shaped every visible string (G-15). A mismatch now just misses that one key
/// and it is replaced on the next measurement.
type TextMetricsCache = HashMap<(u64, u64), (u64, TextMetrics)>;

/// Shaped runs, keyed by the same measurement fingerprint as the metrics.
///
/// The values are `Rc<ShapedText>`: a hit is a refcount bump. `ShapedText` owns a
/// `Vec<TextCluster>` whose every entry owns a `String`, so an owned cache value made each hit
/// clone the whole run to hand back a value nothing mutates (REND-03). Each entry also records the
/// epoch its run was shaped under (G-15).
type ShapeCache = HashMap<(u64, u64), (u64, Rc<ShapedText>)>;

thread_local! {
    // `HashMap::new` is const only when `compat::HashMap` is a `BTreeMap` (alloc_frugal); the
    // std profile uses `RandomState`, so the initializer cannot be wrapped in `const { .. }`.
    // Two independently seeded hashes rather than an owning key: the pair is computed from the
    // borrowed inputs, so a lookup allocates nothing, and two 64-bit values make an accidental
    // match between different inputs effectively impossible. A cache miss is the only way an
    // input can reach the shaping path, so a collision would show as a wrong width, not as
    // corruption.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static TEXT_METRICS: RefCell<TextMetricsCache> = RefCell::new(HashMap::new());
    // Insertion order, for FIFO eviction. The caches used to `clear()` when full, so one entry
    // over the bound discarded the whole working set and the next frame re-shaped everything.
    static TEXT_METRICS_ORDER: RefCell<VecDeque<(u64, u64)>> =
        const { RefCell::new(VecDeque::new()) };
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static TEXT_SHAPES: RefCell<ShapeCache> = RefCell::new(HashMap::new());
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
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    #[inline]
    fn mix(hash: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    #[inline]
    fn mix_u32(hash: &mut u64, value: u32) {
        mix(hash, &value.to_le_bytes());
    }

    #[inline]
    fn mix_u16(hash: &mut u64, value: u16) {
        mix(hash, &value.to_le_bytes());
    }

    #[inline]
    fn mix_bool(hash: &mut u64, value: bool) {
        mix(hash, &[u8::from(value)]);
    }

    let size = font.size().to_bits();
    let letter = font.letter_spacing().to_bits();
    let word = font.word_spacing().to_bits();
    let line = font.line_height().to_bits();
    let scale = scale.to_bits();

    // FNV-1a rather than the default SipHash: this runs for every label on every measure and
    // every shape, and the inputs are short ASCII strings, not attacker-controlled map keys
    // (G-003). Same coverage, so a collision still needs both halves to agree.
    let mut first = FNV_OFFSET;
    mix(&mut first, text.as_bytes());
    mix(&mut first, font.family().as_bytes());
    mix_u32(&mut first, size);
    mix_u16(&mut first, font.weight());
    mix_bool(&mut first, font.is_bold());
    mix_bool(&mut first, font.is_italic());
    mix_u32(&mut first, letter);
    mix_u32(&mut first, word);
    mix_u32(&mut first, line);
    mix_u32(&mut first, scale);

    let bytes = text.as_bytes();
    let head = &bytes[..bytes.len().min(8)];
    let tail = &bytes[bytes.len().saturating_sub(8)..];

    let mut second = FNV_OFFSET;
    mix(&mut second, &[0xA5]);
    mix_u32(&mut second, bytes.len() as u32);
    mix(&mut second, head);
    mix(&mut second, tail);
    mix_u32(&mut second, scale);
    mix_u32(&mut second, line);
    mix_u32(&mut second, word);
    mix_u32(&mut second, letter);
    mix_bool(&mut second, font.is_italic());
    mix_bool(&mut second, font.is_bold());
    mix_u16(&mut second, font.weight());
    mix_u32(&mut second, size);
    mix(&mut second, font.family().as_bytes());

    (first, second)
}

/// The number of samples in a resolved gradient ramp.
///
/// One table is built per distinct ramp and then shared: every pixel of a gradient fill used to
/// rebuild all 256 samples, which is 256 stop searches plus a 1 KB stack array per control per
/// frame (G-16). The ramp depends only on the stops, so a thread-local content-keyed cache is the
/// right lifetime for it.
const GRADIENT_LUT_SIZE: usize = 256;

/// How many resolved ramps one thread caches before evicting the oldest.
const GRADIENT_LUT_CACHE_LIMIT: usize = 16;

thread_local! {
    /// Resolved gradient ramps, keyed by a fingerprint of their stops, in insertion order.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static GRADIENT_LUTS: RefCell<GradientLutCache> = RefCell::new((VecDeque::new(), HashMap::new()));
}

/// Resolved ramps plus the FIFO order their keys are evicted in.
type GradientLutCache = (VecDeque<u64>, HashMap<u64, Rc<[Color; GRADIENT_LUT_SIZE]>>);

/// Fingerprints the ramp (the stop list), which is the only input a resolved table depends on.
fn gradient_ramp_fingerprint(gradient: &Gradient) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    gradient.stops.len().hash(&mut hasher);
    for stop in &gradient.stops {
        stop.position.to_bits().hash(&mut hasher);
        stop.color.r.hash(&mut hasher);
        stop.color.g.hash(&mut hasher);
        stop.color.b.hash(&mut hasher);
        stop.color.a.hash(&mut hasher);
    }
    hasher.finish()
}

/// Fingerprints a bare `(position, color)` stop slice exactly as [`gradient_ramp_fingerprint`]
/// hashes the equivalent `Gradient`, so both builders address the same cache entry (G-203).
fn gradient_stops_fingerprint(stops: &[(f32, Color)]) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    stops.len().hash(&mut hasher);
    for (position, color) in stops {
        position.to_bits().hash(&mut hasher);
        color.r.hash(&mut hasher);
        color.g.hash(&mut hasher);
        color.b.hash(&mut hasher);
        color.a.hash(&mut hasher);
    }
    hasher.finish()
}

/// Returns the cached ramp for `key`, building it with `build` on a miss.
///
/// The cache is FIFO-bounded by [`GRADIENT_LUT_CACHE_LIMIT`]; a hit costs one refcount bump.
fn cached_ramp_lut(
    key: u64,
    build: impl FnOnce() -> [Color; GRADIENT_LUT_SIZE],
) -> Rc<[Color; GRADIENT_LUT_SIZE]> {
    GRADIENT_LUTS.with(|cache| {
        let existing = {
            let cache = cache.borrow();
            cache.1.get(&key).cloned()
        };
        if let Some(lut) = existing {
            return lut;
        }
        let lut = Rc::new(build());
        let mut cache = cache.borrow_mut();
        if cache.0.len() >= GRADIENT_LUT_CACHE_LIMIT {
            if let Some(oldest) = cache.0.pop_front() {
                cache.1.remove(&oldest);
            }
        }
        cache.0.push_back(key);
        cache.1.insert(key, Rc::clone(&lut));
        lut
    })
}

/// Samples a ramp of `(position, color)` stops at `pos`.
///
/// This mirrors [`Gradient::interpolate`] step for step (boundary stops, `partition_point`
/// bracketing, the zero-span guard, and the truncating channel lerp), so a ramp built from a bare
/// stop slice is bit-identical to one built from the equivalent `Gradient`. It is the single
/// sampler both conic raster paths use (G-203); the removed private `conic_stop_color` differed
/// only for a stop span below `0.0001`, where it stretched the span instead of dividing by it.
fn ramp_stop_color(stops: &[(f32, Color)], position: f32) -> Color {
    if stops.is_empty() {
        return Color::TRANSPARENT;
    }
    if stops.len() == 1 {
        return stops[0].1;
    }
    let last = stops.len() - 1;
    if position <= stops[0].0 {
        return stops[0].1;
    }
    if position >= stops[last].0 {
        return stops[last].1;
    }
    let index = stops.partition_point(|stop| stop.0 <= position);
    let (current_position, current_color) = stops[index - 1];
    let (next_position, next_color) = stops[index];
    let span = next_position - current_position;
    if span <= 0.0 {
        return current_color;
    }
    let t = ((position - current_position) / span).clamp(0.0, 1.0);
    let channel = |from: u8, to: u8| -> u8 { ((1.0 - t) * from as f32 + t * to as f32) as u8 };
    Color::rgba(
        channel(current_color.r, next_color.r),
        channel(current_color.g, next_color.g),
        channel(current_color.b, next_color.b),
        channel(current_color.a, next_color.a),
    )
}

/// Resolves a full 256-sample ramp from `stops` with [`ramp_stop_color`].
fn build_ramp(stops: &[(f32, Color)]) -> [Color; GRADIENT_LUT_SIZE] {
    let mut lut = [Color::BLACK; GRADIENT_LUT_SIZE];
    for (index, slot) in lut.iter_mut().enumerate() {
        *slot = ramp_stop_color(stops, index as f32 / (GRADIENT_LUT_SIZE - 1) as f32);
    }
    lut
}

/// Returns the resolved 256-sample ramp for `gradient`, building it at most once per content.
fn gradient_ramp_lut(gradient: &Gradient) -> Rc<[Color; GRADIENT_LUT_SIZE]> {
    cached_ramp_lut(gradient_ramp_fingerprint(gradient), || {
        // The sampler and its cache speak the `(position, color)` pairs a conic command carries;
        // bridge once per build, not once per lookup.
        let stops: Vec<(f32, Color)> =
            gradient.stops.iter().map(|stop| (stop.position, stop.color)).collect();
        build_ramp(&stops)
    })
}

/// Returns the resolved 256-sample ramp for a bare `(position, color)` stop slice, building it at
/// most once per content.
///
/// Shares [`GRADIENT_LUTS`] with [`gradient_ramp_lut`], so a `Gradient` and the identical stop
/// pairs a conic command carries resolve to the same cached table (G-203).
pub(crate) fn ramp_lut_for_stops(stops: &[(f32, Color)]) -> Rc<[Color; GRADIENT_LUT_SIZE]> {
    cached_ramp_lut(gradient_stops_fingerprint(stops), || build_ramp(stops))
}

/// How the shared conic kernel commits each sampled pixel.
///
/// The two callers historically wrote differently — the `DrawConicGradient` command arm stored the
/// colour verbatim, while `fill_rect_gradient` composited through the surface blend mode — so the
/// write policy travels with the call rather than being folded into the loop, which keeps both
/// paths' pixels unchanged (G-203).
pub(crate) enum ConicWrite {
    /// Store the sampled colour, including its alpha, over the destination.
    Replace,
    /// Composite through `mode` at full coverage.
    Blend(BlendMode),
}

/// Rasterises a conic (swept) gradient over `bounds`, the one implementation both
/// `DrawConicGradient` and `GradientType::Conic` use (G-203).
///
/// `bounds` is `(x0, y0, x1, y1)` with `x1`/`y1` exclusive and already clipped by the caller.
/// `start_angle` is in radians and rotates the ramp index, not the geometry. The per-pixel angle
/// stays in radians because that is what `atan2` returns; the degree-to-radian conversion stays
/// with the caller, which needs it once per fill rather than once per pixel (G-12).
pub(crate) fn fill_conic_ramp(
    frame: &mut [u8],
    surface_width: u32,
    bounds: (u32, u32, u32, u32),
    center: Point,
    start_angle: f32,
    lut: &[Color; GRADIENT_LUT_SIZE],
    write: ConicWrite,
) {
    const TAU: f32 = core::f32::consts::TAU;
    let (x0, y0, x1, y1) = bounds;
    let cx = center.x as f32;
    let cy = center.y as f32;
    for y in y0..y1 {
        let dy = y as f32 - cy;
        for x in x0..x1 {
            let dx = x as f32 - cx;
            let angle = dy.atan2(dx) + core::f32::consts::PI;
            let angle = (angle + start_angle) % TAU;
            // A negative remainder wraps once more. `fill_rect_gradient` always did this; the
            // command arm used to clamp the negative position to the ramp start instead, which
            // differed only for a negative start angle.
            let angle = if angle < 0.0 { angle + TAU } else { angle };
            let pos = angle / TAU;
            let index = ((pos.clamp(0.0, 1.0) * (GRADIENT_LUT_SIZE - 1) as f32) as usize)
                .min(GRADIENT_LUT_SIZE - 1);
            match write {
                ConicWrite::Replace => set_pixel(frame, surface_width, x, y, lut[index]),
                ConicWrite::Blend(mode) => {
                    write_pixel_with_mode(mode, frame, surface_width, x, y, lut[index], 1.0)
                }
            }
        }
    }
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
            frame_number: 0,
            glyph_run_text: false,
            #[cfg(feature = "3d")]
            depth_buffer: Vec::new(),
            #[cfg(feature = "3d")]
            depth_dirty: true,
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
        // The real frame boundary (G-020): the text atlas advances its collection epoch from this
        // value instead of counting lookups, so eviction follows frames rather than text volume.
        self.frame_number = self.frame_number.wrapping_add(1);
        // A new frame starts with no geometry in front of the camera. The buffer is cleared lazily
        // by the first mesh draw of the frame, so a 2D-only frame pays nothing for the feature.
        #[cfg(feature = "3d")]
        {
            self.depth_dirty = true;
        }
    }
    /// The frame counter advanced by [`Self::begin_frame`] (G-020).
    pub fn frame_number(&self) -> u64 {
        self.frame_number
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
        #[cfg(feature = "3d")]
        {
            self.depth_dirty = true;
        }
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
    /// See `BackBuffer::take_front` for why this exists: the render entry points own
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
            cache.get(&key).and_then(|(built, metrics)| (*built == epoch).then_some(*metrics))
        }) {
            return hit;
        }

        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        let metrics = TextMetrics { width, ..TextMetrics::for_font(font, scale) };
        note_measurement_built();
        TEXT_METRICS.with(|cache| {
            let mut cache = cache.borrow_mut();
            let is_new = !cache.contains_key(&key);
            if is_new {
                TEXT_METRICS_ORDER.with(|order| {
                    let mut order = order.borrow_mut();
                    if order.len() >= TEXT_METRICS_CACHE_LIMIT {
                        if let Some(oldest) = order.pop_front() {
                            cache.remove(&oldest);
                        }
                    }
                    order.push_back(key);
                });
            }
            cache.insert(key, (epoch, metrics));
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
            cache.get(&key).and_then(|(built, shaped)| (*built == epoch).then(|| Rc::clone(shaped)))
        }) {
            return hit;
        }
        let shaped = Rc::new(crate::render::text::shape_line(text, font, scale));
        note_shape_built();
        TEXT_SHAPES.with(|cache| {
            let mut cache = cache.borrow_mut();
            let is_new = !cache.contains_key(&key);
            if is_new {
                TEXT_SHAPES_ORDER.with(|order| {
                    let mut order = order.borrow_mut();
                    if order.len() >= TEXT_SHAPES_CACHE_LIMIT {
                        if let Some(oldest) = order.pop_front() {
                            cache.remove(&oldest);
                        }
                    }
                    order.push_back(key);
                });
            }
            cache.insert(key, (epoch, Rc::clone(&shaped)));
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

        // A conic ramp is angular, so it cannot be expressed by the per-pixel position the other
        // types use; it is rasterised by the kernel shared with `DrawConicGradient` (G-203). The
        // bounds are intersected with the clip here, which admits exactly the pixels the per-pixel
        // `pixel_visible` check used to keep.
        if gradient.gradient_type == GradientType::Conic {
            let (visible_x0, visible_x1) = match clip {
                Some((clip_x, _, clip_w, _)) => {
                    intersect_axis(x0, x1, clip_x, clip_x.saturating_add(clip_w as i32))
                }
                None => (x0, x1),
            };
            let (visible_y0, visible_y1) = match clip {
                Some((_, clip_y, _, clip_h)) => {
                    intersect_axis(y0, y1, clip_y, clip_y.saturating_add(clip_h as i32))
                }
                None => (y0, y1),
            };
            if visible_x1 <= visible_x0 || visible_y1 <= visible_y0 {
                return;
            }
            // `Gradient::angle` is in **degrees** while the kernel takes radians, so the conversion
            // is paid once per fill instead of once per pixel (G-12).
            let lut = gradient_ramp_lut(gradient);
            let blend_mode = self.blend_mode;
            fill_conic_ramp(
                self.buffer.back_mut(),
                size.width,
                (visible_x0, visible_y0, visible_x1, visible_y1),
                gradient.center,
                gradient.angle.to_radians(),
                &lut,
                ConicWrite::Blend(blend_mode),
            );
            return;
        }

        // The linear/radial loops below used to test `pixel_visible` per pixel; intersecting the
        // bounds with the clip once admits exactly the same pixels, because `intersect_axis`'s end
        // is the saturating sum that check compares against (G-010, same form as the conic branch
        // above). The gradient arithmetic stays anchored to the un-intersected `x0`/`y0` so every
        // remaining pixel samples the same position, bit for bit.
        let ((x_lo, x_hi), (y_lo, y_hi)) = match clip {
            Some((clip_x, clip_y, clip_w, clip_h)) => (
                intersect_axis(x0, x1, clip_x, clip_x.saturating_add(clip_w as i32)),
                intersect_axis(y0, y1, clip_y, clip_y.saturating_add(clip_h as i32)),
            ),
            None => ((x0, x1), (y0, y1)),
        };

        let frame = self.buffer.back_mut();

        // Resolve the ramp through the shared per-content cache. Every pixel previously ran
        // `Gradient::interpolate`, and the 256-entry table itself was rebuilt for each fill; both
        // are now paid once per distinct ramp (G-16). Sampling a fixed table is also what keeps the
        // three gradient types visually consistent.
        let lut = gradient_ramp_lut(gradient);
        let lut_index = |pos: f32| -> usize {
            ((pos.clamp(0.0, 1.0) * (GRADIENT_LUT_SIZE - 1) as f32) as usize)
                .min(GRADIENT_LUT_SIZE - 1)
        };

        // Linear geometry is constant across the whole rectangle: hoist it and walk the
        // projection with an increment per pixel instead of recomputing it from x/y.
        let linear_dx = gradient.end_point.x as f32 - gradient.start_point.x as f32;
        let linear_dy = gradient.end_point.y as f32 - gradient.start_point.y as f32;
        let linear_dot = linear_dx * linear_dx + linear_dy * linear_dy;
        let linear_step = if linear_dot == 0.0 { 0.0 } else { linear_dx / linear_dot };

        // Radial geometry is constant along a row, so the row's `dy` and `dy*dy` are hoisted out
        // of the pixel loop. The `sqrt` stays per pixel: the ramp index is quantised from the
        // distance for every radius below `radius`, not only at the edge, so an annulus-only
        // shortcut would change pixels. Keeping the original arithmetic order is the item's
        // stated fallback when a bit-exact rewrite is not provable (G-12).
        let radial_cx = gradient.center.x as f32;

        for y in y_lo..y_hi {
            let row_start = if linear_dot == 0.0 {
                0.0
            } else {
                ((x0 as f32 - gradient.start_point.x as f32) * linear_dx
                    + (y as f32 - gradient.start_point.y as f32) * linear_dy)
                    / linear_dot
            };
            let radial_dy2 = if matches!(gradient.gradient_type, GradientType::Radial) {
                let dy = y as f32 - gradient.center.y as f32;
                dy * dy
            } else {
                0.0
            };
            for x in x_lo..x_hi {
                let pos = match gradient.gradient_type {
                    GradientType::Linear => {
                        if linear_dot == 0.0 {
                            0.0
                        } else {
                            (row_start + (x - x0) as f32 * linear_step).clamp(0.0, 1.0)
                        }
                    }
                    GradientType::Radial => {
                        let dx = x as f32 - radial_cx;
                        let dist = (dx * dx + radial_dy2).sqrt();
                        if gradient.radius <= 0.0 {
                            0.0
                        } else {
                            (dist / gradient.radius).clamp(0.0, 1.0)
                        }
                    }
                    // The conic case returned before the loop started. `return` rather than
                    // `unreachable!()`: a draw path should not panic if a later edit moves the
                    // early return, and skipping the fill is the safe failure (G-203).
                    GradientType::Conic => return,
                };
                let color = lut[lut_index(pos)];
                // Through the blend-aware write, so a gradient drawn under a `SetBlendMode`
                // composites the same way an opaque fill does.
                write_pixel_with_mode(self.blend_mode, frame, size.width, x, y, color, 1.0);
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
        let backdrop = crate::render::backend::paint::capture_region(
            self.buffer.back.as_slice(),
            size.width as usize,
            rect,
        );
        if let Some(slot) = self.path_clips.last_mut() {
            *slot = Some(PathClip { rect, coverage, backdrop });
        }
    }
    /// Returns the current effective clip rect, or `None` if the clip stack is empty.
    pub fn current_clip(&self) -> Option<(i32, i32, u32, u32)> {
        self.clip_stack.last().copied()
    }
}

/// Narrows the half-open range `start..end` to `clip_start..clip_end`.
///
/// `clip_end` is the saturating sum `pixel_visible` compares against, so turning the clip into a
/// loop bound keeps the exact pixel set the per-pixel check admitted.
fn intersect_axis(start: u32, end: u32, clip_start: i32, clip_end: i32) -> (u32, u32) {
    let clip_start = clip_start.max(0) as u32;
    let clip_end = clip_end.max(0) as u32;
    (start.max(clip_start), end.min(clip_end))
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
    let (x0, y0, width, height) = rect;
    let pixel_count = width as usize * height as usize;
    let mut coverage = vec![0u8; pixel_count];
    if points.len() < 3 || width == 0 || height == 0 {
        return coverage;
    }
    // Coordinate contract: mask byte `(row, column)` is the coverage of the absolute pixel
    // `(x0 + column, y0 + row)` — the same mapping `apply_path_clip` uses when it composites
    // the mask back onto the frame. y already honors it because `sample_y` is absolute while
    // rows are counted from `y0`; x honors it because every crossing is shifted into
    // rect-relative space below. `x0` is a surface coordinate, far below 2^24, so the
    // `i32 → f32` conversion is exact and the shift is lossless.
    // The two scratch buffers live in the shared word pool instead of being allocated per call
    // (G-011). `accumulated` stores the f32 sums as their bit patterns: every access round-trips
    // through `to_bits`/`from_bits` (the `i32`↔`u32` `as` casts re-interpret the same 32 bits
    // losslessly), so the accumulation is the same IEEE arithmetic in the same order and the
    // coverage bytes are unchanged. The pool zero-fills on acquire, and word 0 is `+0.0`, which
    // is exactly what the old `vec![0f32; …]` started from. The returned `coverage` itself
    // cannot be pooled: it escapes into the clip stack and lives as long as the clip does.
    let mut accumulated = ScratchI32::new(pixel_count);
    // Crossing list as interleaved `(x_bits, winding)` word pairs, one pair per edge crossing
    // the sample line; at most one crossing per edge, so `points.len()` pairs always suffice.
    let mut crossings = ScratchI32::new(points.len() * 2);
    for row in 0..height as usize {
        for sub in 0..PATH_CLIP_SUBSAMPLES {
            let sample_y =
                y0 as f32 + row as f32 + (sub as f32 + 0.5) / PATH_CLIP_SUBSAMPLES as f32;
            let mut count = 0usize;
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
                let x = first.x as f32 + (second.x - first.x) as f32 * t - x0 as f32;
                crossings[2 * count] = x.to_bits() as i32;
                crossings[2 * count + 1] = winding;
                count += 1;
            }
            let (pairs, _) = crossings[..2 * count].as_chunks_mut::<2>();
            pairs.sort_by(|left, right| {
                f32::from_bits(left[0] as u32)
                    .partial_cmp(&f32::from_bits(right[0] as u32))
                    .unwrap_or(core::cmp::Ordering::Equal)
            });
            let mut index = 0;
            let mut winding = 0;
            let mut span_start: Option<f32> = None;
            while index < count {
                let x = f32::from_bits(pairs[index][0] as u32);
                if winding != 0 {
                    if let Some(start) = span_start {
                        add_span(&mut accumulated, width, row, start, x);
                    }
                }
                let mut delta = 0;
                while index < count && f32::from_bits(pairs[index][0] as u32) == x {
                    delta += pairs[index][1];
                    index += 1;
                }
                winding += delta;
                span_start = Some(x);
            }
        }
    }
    let scale = 255.0 / PATH_CLIP_SUBSAMPLES as f32;
    for (slot, value) in coverage.iter_mut().zip(accumulated.iter()) {
        *slot = (f32::from_bits(*value as u32) * scale).round().clamp(0.0, 255.0) as u8;
    }
    coverage
}

/// Adds the horizontal overlap of the span `start..end` with each pixel of `row`.
///
/// `accumulated` holds the per-pixel f32 sums as bit patterns so the buffer can live in the
/// shared [`ScratchI32`] pool (G-011). The read-modify-write round-trips through `from_bits`/
/// `to_bits` (the `i32`↔`u32` `as` casts re-interpret the same 32 bits losslessly), so each
/// update is the same f32 add a direct `Vec<f32>` performed.
fn add_span(accumulated: &mut [i32], width: u32, row: usize, start: f32, end: f32) {
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
            let index = row_start + column;
            accumulated[index] =
                (f32::from_bits(accumulated[index] as u32) + (right - left)).to_bits() as i32;
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

    /// The gradient fill's clip∩rect hoisting (G-010) must move no ink: a fill drawn under a
    /// rectangle clip must agree, pixel for pixel, with the same fill drawn unclipped everywhere
    /// the clip admits, and must leave every clipped-out pixel as the untouched backdrop. The
    /// rect and the gradient geometry cross the clip on all four sides, so a hoist that shifted
    /// the sampling anchor or the admitted spans shows up as a mismatch. Conic is excluded: it
    /// returns through the kernel branch before the hoisted loops and was already intersecting.
    #[test]
    fn a_clipped_gradient_fill_agrees_with_the_unclipped_fill_inside_the_clip() {
        const SIZE: u32 = 16;
        let rect = Rect::new(2, 3, 11, 9);
        let mut blank = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
        blank.begin_frame(Color::WHITE);
        blank.end_frame();
        for (name, gradient) in [
            (
                "linear",
                Gradient::linear(Point::new(1, 2), Point::new(14, 10))
                    .add_stop(0.0, Color::RED)
                    .add_stop(1.0, Color::BLUE),
            ),
            (
                "radial",
                Gradient::radial(Point::new(6, 5), 7.0)
                    .add_stop(0.0, Color::GREEN)
                    .add_stop(1.0, Color::WHITE),
            ),
        ] {
            let mut whole = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
            whole.begin_frame(Color::WHITE);
            whole.fill_rect_gradient(rect, &gradient);
            whole.end_frame();
            // Clip ⊂ rect on both axes (every side cut), a containing clip (no cut), and a
            // disjoint clip (nothing may be drawn anywhere).
            for (clip_name, (cx, cy, cw, ch)) in [
                ("all four sides cut", (4, 5, 5, 4)),
                ("clip contains the rect", (0, 0, SIZE, SIZE)),
                ("clip misses the rect", (14, 0, 2, SIZE)),
            ] {
                let mut clipped = SoftwareSurface::new(Size::new(SIZE, SIZE), 1.0);
                clipped.begin_frame(Color::WHITE);
                clipped.push_clip(cx, cy, cw, ch);
                clipped.fill_rect_gradient(rect, &gradient);
                clipped.pop_clip();
                clipped.end_frame();
                let admitted = |x: u32, y: u32| {
                    x >= cx.max(0) as u32
                        && x < cx.saturating_add(cw as i32).max(0) as u32
                        && y >= cy.max(0) as u32
                        && y < cy.saturating_add(ch as i32).max(0) as u32
                };
                let frame = clipped.frame_rgba();
                let expected = whole.frame_rgba();
                let backdrop = blank.frame_rgba();
                for y in 0..SIZE {
                    for x in 0..SIZE {
                        let i = ((y * SIZE + x) * 4) as usize;
                        let want =
                            if admitted(x, y) { &expected[i..i + 4] } else { &backdrop[i..i + 4] };
                        assert_eq!(
                            &frame[i..i + 4],
                            want,
                            "{name} gradient, clip {clip_name} at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    /// The direct-buffer form of `path_coverage`: the per-call `Vec` scratch the pool replaced,
    /// kept verbatim as the reference the pooled kernel must match byte for byte (G-011).
    fn reference_path_coverage(points: &[Point], rect: (i32, i32, u32, u32)) -> Vec<u8> {
        let (x0, y0, width, height) = rect;
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
                    let x = first.x as f32 + (second.x - first.x) as f32 * t - x0 as f32;
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
                            reference_add_span(&mut accumulated, width, row, start, x);
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

    /// The reference twin of `add_span`, in the direct `Vec<f32>` form.
    fn reference_add_span(accumulated: &mut [f32], width: u32, row: usize, start: f32, end: f32) {
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

    /// Moving `path_coverage`'s scratch buffers into the shared word pool (G-011) must not move
    /// one coverage byte: every polygon below — full cover, disjoint, edge-exact, concave,
    /// self-intersecting, rect-clipped, degenerate — is compared against the direct-buffer
    /// reference, and the absolute anchors pin the reference itself. The suite runs twice over
    /// differently sized rects so a stale pooled block would surface as a mismatch.
    #[test]
    fn path_coverage_matches_the_direct_buffer_reference() {
        let cases: Vec<(Vec<Point>, (i32, i32, u32, u32))> = vec![
            // Exactly covers the rect: every byte must be full coverage.
            (
                vec![Point::new(2, 3), Point::new(11, 3), Point::new(11, 10), Point::new(2, 10)],
                (2, 3, 9, 7),
            ),
            // Entirely outside the rect: every byte must stay empty.
            (
                vec![
                    Point::new(20, 20),
                    Point::new(25, 20),
                    Point::new(25, 25),
                    Point::new(20, 25),
                ],
                (2, 3, 9, 7),
            ),
            // Shares the rect's left edge exactly.
            (
                vec![Point::new(2, 3), Point::new(7, 3), Point::new(7, 10), Point::new(2, 10)],
                (2, 3, 9, 7),
            ),
            // The lower-left triangle the surface-level path-clip tests use.
            (vec![Point::new(0, 0), Point::new(0, 4), Point::new(4, 4)], (0, 0, 4, 4)),
            // A concave L-shape with a reflex vertex inside the rect.
            (
                vec![
                    Point::new(0, 0),
                    Point::new(6, 0),
                    Point::new(6, 2),
                    Point::new(2, 2),
                    Point::new(2, 6),
                    Point::new(0, 6),
                ],
                (0, 0, 6, 6),
            ),
            // A self-intersecting bowtie: the non-zero winding of the two lobes must match.
            (
                vec![Point::new(0, 0), Point::new(4, 4), Point::new(4, 0), Point::new(0, 4)],
                (0, 0, 4, 4),
            ),
            // Larger than the rect on every side: the coverage is clipped to the rect.
            (
                vec![
                    Point::new(-4, -4),
                    Point::new(12, -4),
                    Point::new(12, 12),
                    Point::new(-4, 12),
                ],
                (0, 0, 5, 5),
            ),
            // Degenerate: fewer than three points returns the empty mask.
            (vec![Point::new(0, 0), Point::new(3, 3)], (0, 0, 4, 4)),
        ];
        for round in 0..2 {
            for (points, rect) in &cases {
                let rect = if round == 0 {
                    *rect
                } else {
                    // A second pass with a different rect size, so the pooled blocks are
                    // reacquired at a different capacity and any stale content would show.
                    (rect.0, rect.1, rect.2 + 3, rect.3 + 2)
                };
                let produced = path_coverage(points, rect);
                let expected = reference_path_coverage(points, rect);
                assert_eq!(
                    produced, expected,
                    "round {round}: pooled coverage diverged from the direct-buffer reference"
                );
            }
        }
        // Absolute anchors for the reference itself, on a rect at the origin (where a mask
        // column equals its path-space x, as `add_span` consumes the crossings directly): the
        // exact-cover case saturates every byte and the disjoint case empties every byte.
        let full = path_coverage(
            &[Point::new(0, 0), Point::new(9, 0), Point::new(9, 7), Point::new(0, 7)],
            (0, 0, 9, 7),
        );
        assert!(full.iter().all(|&byte| byte == 255), "exact cover saturates coverage");
        let empty = path_coverage(
            &[Point::new(20, 20), Point::new(25, 20), Point::new(25, 25), Point::new(20, 25)],
            (0, 0, 9, 7),
        );
        assert!(empty.iter().all(|&byte| byte == 0), "disjoint path leaves no coverage");
    }

    /// A coverage rect whose origin is not the world origin must still line the mask up with
    /// the absolute pixels it stands for: `apply_path_clip` composites mask column `c` onto
    /// absolute `rect.0 + c`, so the crossings have to be consumed relative to `rect.0`. The
    /// pre-fix kernel consumed them as path-space columns, which shifted (or dropped) every
    /// mask whose rect sat right of the origin.
    #[test]
    fn a_coverage_rect_off_the_origin_aligns_the_mask_with_the_absolute_pixels() {
        // Path occupies absolute x 20..26, y 10..16; the rect starts at (12, 6), so the
        // interior lands on mask columns 8..14 / rows 4..10.
        let points =
            &[Point::new(20, 10), Point::new(26, 10), Point::new(26, 16), Point::new(20, 16)];
        let coverage = path_coverage(points, (12, 6, 20, 16));
        assert_eq!(coverage.len(), 20 * 16);
        for row in 0..16usize {
            for column in 0..20usize {
                let inside = (4..10).contains(&row) && (8..14).contains(&column);
                let byte = coverage[row * 20 + column];
                if inside {
                    assert_eq!(byte, 255, "interior pixel ({column},{row}) must be fully covered");
                } else {
                    assert_eq!(byte, 0, "exterior pixel ({column},{row}) must stay empty");
                }
            }
        }
    }
}
