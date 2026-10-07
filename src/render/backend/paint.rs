// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Paint backend trait and software implementation.
use super::batch::BatchState;
use crate::compat::vec;
use crate::core::{Color, Font, Size};
use crate::render::pipeline::{
    sat_add_i32, sat_mul_i32, sat_neg_i32, set_pixel, u32_to_i32_saturating,
};
use crate::render::{
    RenderCommand, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextMetrics,
};

/// Pluggable paint backend strategy used by render scene composition.
///
/// A backend receives a frame's worth of [`RenderCommand`]s between
/// [`PaintBackend::begin_frame`] and [`PaintBackend::end_frame`] and is
/// responsible for rasterising them into some surface. Implementations may be
/// software (see [`SoftwarePaintBackend`]) or hardware. Commands arriving
/// outside a frame are outside the contract.
pub trait PaintBackend {
    /// Starts a frame, filling the target with `clear` first.
    ///
    /// Must be paired with [`PaintBackend::end_frame`]. Anything drawn before
    /// this call is not guaranteed to survive it.
    fn begin_frame(&mut self, clear: Color);
    /// Finishes the frame, making its contents available for presentation.
    ///
    /// For a software backend this is what flushes any pending batched work;
    /// reading the surface before this point may see a partially drawn frame.
    fn end_frame(&mut self);
    /// Executes a single drawing command in the current frame.
    ///
    /// The command is applied in sequence, so ordering matters: a clip push
    /// affects every command until its matching pop.
    fn execute_command(&mut self, command: &RenderCommand);
    /// Returns the size of the target surface in logical (DPI-independent)
    /// units.
    fn size(&self) -> Size;
    /// Resizes the target surface, discarding its contents.
    fn set_size(&mut self, size: Size);
    /// Returns the device pixel ratio the backend rasterises at; `1.0` is the
    /// 96-DPI baseline.
    fn dpi_scale(&self) -> f32;
    /// Sets the device pixel ratio used for subsequent rasterisation.
    fn set_dpi_scale(&mut self, dpi_scale: f32);
    /// Measures `text` in `font`, returning advance and bounding-box metrics.
    ///
    /// Measurement is independent of the current frame, so it is valid to call
    /// outside `begin_frame`/`end_frame` — layout code relies on this.
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics;
    /// Shapes `text` in `font`, returning positioned glyphs.
    ///
    /// Like [`PaintBackend::measure_text`], valid outside a frame. Backends
    /// without real shaping may return a trivial single-run result.
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText;
    /// Returns the final frame as tightly packed RGBA bytes, 8 bits per
    /// channel, row-major, top row first.
    ///
    /// The slice length is `width * height * 4` and the buffer is owned by the
    /// backend: it is invalidated by the next frame, resize, or mutable access.
    fn frame_rgba(&self) -> &[u8];
    /// Apply backend-specific render quality configuration.
    ///
    /// The default implementation ignores the config, which is the honest
    /// behaviour for a backend with no quality knobs; such a backend still
    /// accepts the call rather than failing, so callers need not feature-detect.
    fn apply_render_config(&mut self, _config: SoftwareRenderConfig) {}
    /// Read backend-specific render quality configuration.
    ///
    /// The default returns [`SoftwareRenderConfig::default`], which does **not**
    /// necessarily reflect what a non-software backend is actually doing — a
    /// backend with real quality settings should override both this and
    /// [`PaintBackend::apply_render_config`].
    fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig::default()
    }
}
/// Software implementation of the paint backend strategy.
///
/// Rasterises commands into an in-memory [`SoftwareSurface`]. This is the
/// reference implementation and the fallback when no accelerated backend is
/// available.
pub struct SoftwarePaintBackend {
    pub(crate) surface: SoftwareSurface,
    pub(crate) batch_state: BatchState,
}
impl SoftwarePaintBackend {
    /// Creates a software paint backend with a target size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        Self { surface: SoftwareSurface::new(size, dpi_scale), batch_state: BatchState::new() }
    }
    /// Returns immutable access to the underlying software surface.
    pub fn surface(&self) -> &SoftwareSurface {
        &self.surface
    }
    /// Returns mutable access to the underlying software surface.
    pub fn surface_mut(&mut self) -> &mut SoftwareSurface {
        &mut self.surface
    }
    /// Apply software render quality configuration via backend facade.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    /// Get software render quality configuration via backend facade.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }

    /// Seeds the back buffer with a previously rendered frame.
    ///
    /// # Why this is needed for partial repaint
    ///
    /// `begin_frame` clears — that is what a full-frame render wants. A partial
    /// repaint must instead start from the previous frame and overwrite only the
    /// damaged pixels, so it needs a way to load that frame without clearing. This is
    /// that way.
    ///
    /// # Why the length is checked rather than trusted
    ///
    /// A caller can hand in a frame rendered at a different size — the widget was
    /// resized between frames, or the buffer came from somewhere else. Copying a
    /// short buffer would leave the tail of the surface holding whatever the clear
    /// left, so a mismatch is reported instead of silently producing a frame that is
    /// half stale. Returns `false` and leaves the surface untouched.
    pub fn seed_from(&mut self, frame: &[u8]) -> bool {
        let target = &mut self.surface.buffer.back;
        if frame.len() != target.len() {
            return false;
        }
        target.copy_from_slice(frame);
        true
    }
}
impl PaintBackend for SoftwarePaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.surface.begin_frame(clear);
    }
    fn end_frame(&mut self) {
        self.surface.end_frame();
    }
    fn execute_command(&mut self, command: &RenderCommand) {
        match command {
            RenderCommand::FillRect { rect, color } => self.surface.fill_rect(*rect, *color),
            RenderCommand::DrawRect { rect, color } => self.surface.draw_rect(*rect, *color),
            RenderCommand::DrawRectStroke { rect, color, width } => {
                self.surface.draw_rect_with_width(*rect, *color, *width)
            }
            RenderCommand::FillRoundedRect { rect, radius, color } => {
                self.surface.fill_rounded_rect(*rect, *radius, *color)
            }
            RenderCommand::FillRoundedRectAA { rect, radius, color } => {
                self.surface.fill_rounded_rect_aa(*rect, *radius, *color)
            }
            RenderCommand::DrawRoundedRectStroke { rect, radius, color, width } => {
                self.surface.draw_rounded_rect_with_width(*rect, *radius, *color, *width)
            }
            RenderCommand::DrawRoundedRectStrokeAA { rect, radius, color, width } => {
                self.surface.draw_rounded_rect_aa_with_width(*rect, *radius, *color, *width)
            }
            RenderCommand::DrawLine { from, to, color } => {
                self.surface.draw_line(*from, *to, *color)
            }
            RenderCommand::DrawLineAA { from, to, color } => {
                self.surface.draw_line_aa(*from, *to, *color)
            }
            RenderCommand::DrawLineStrokeAA { from, to, color, width } => {
                self.surface.draw_line_aa_with_width(*from, *to, *color, *width)
            }
            RenderCommand::DrawLineStroke { from, to, color, width } => {
                self.surface.draw_line_with_width(*from, *to, *color, *width)
            }
            RenderCommand::FillCircle { center, radius, color } => {
                self.surface.fill_circle(*center, *radius, *color)
            }
            RenderCommand::FillCircleAA { center, radius, color } => {
                self.surface.fill_circle_aa(*center, *radius, *color)
            }
            RenderCommand::DrawCircle { center, radius, color } => {
                self.surface.draw_circle(*center, *radius, *color)
            }
            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                self.surface.draw_circle_with_width(*center, *radius, *color, *width)
            }
            RenderCommand::DrawText { origin, text, font, color, alignment } => {
                self.surface.draw_text(*origin, text, font, *color, *alignment)
            }
            RenderCommand::DrawImage { x, y, width, height, data } => {
                self.surface.draw_image(*x, *y, *width, *height, data)
            }
            RenderCommand::PushClip { x, y, width, height } => {
                self.surface.push_clip(*x, *y, *width, *height)
            }
            RenderCommand::PopClip => self.surface.pop_clip(),
            RenderCommand::DrawGradient { rect, gradient } => {
                self.surface.fill_rect_gradient(*rect, gradient);
            }
            RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
                self.surface.draw_arc(*center, *radius, *start_angle, *end_angle, *color, *filled);
            }
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                self.surface.draw_path(points, *closed, *color, *filled, *width);
            }
            RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread } => {
                // Render shadow rect with offset and optional spread. Every term is `i32` and the
                // operands come from a public command, so the arithmetic is saturating: a wrapped
                // `spread` used to flip a large shadow into a tiny one (or a negative width that
                // `.max(0)` silently collapsed), which is a wrong picture rather than a rejected one.
                let spread_rect = crate::core::Rect::new(
                    sat_add_i32(sat_add_i32(rect.x, *offset_x), sat_neg_i32(*spread)),
                    sat_add_i32(sat_add_i32(rect.y, *offset_y), sat_neg_i32(*spread)),
                    sat_mul_i32(*spread, 2).saturating_add(u32_to_i32_saturating(rect.width)).max(0)
                        as u32,
                    sat_mul_i32(*spread, 2)
                        .saturating_add(u32_to_i32_saturating(rect.height))
                        .max(0) as u32,
                );
                let shadow_color =
                    Color::rgba(color.r, color.g, color.b, (color.a as f32 * 0.5) as u8);
                self.surface.fill_rect(spread_rect, shadow_color);
                // Apply box blur to the shadow region if blur_radius > 0
                if *blur_radius > 0 {
                    let size = self.surface.size();
                    let w = size.width as usize;
                    let h = size.height as usize;
                    if w > 0 && h > 0 {
                        let back = self.surface.buffer.back_mut();
                        let radius = (*blur_radius).min(100) as usize;
                        let blur_x0 = spread_rect.x.max(0) as usize;
                        let blur_y0 = spread_rect.y.max(0) as usize;
                        let blur_w = ((spread_rect.x as usize + spread_rect.width as usize).min(w))
                            .saturating_sub(blur_x0);
                        let blur_h = ((spread_rect.y as usize + spread_rect.height as usize)
                            .min(h))
                        .saturating_sub(blur_y0);
                        box_blur_region(back, w, h, blur_x0, blur_y0, blur_w, blur_h, radius);
                    }
                }
            }
            RenderCommand::Blur { radius } => {
                let r = (*radius).min(100) as usize;
                if r == 0 {
                    return;
                }
                let size = self.surface.size();
                let w = size.width as usize;
                let h = size.height as usize;
                if w == 0 || h == 0 {
                    return;
                }
                let back = self.surface.buffer.back_mut();
                box_blur_region(back, w, h, 0, 0, w, h, r);
            }
            RenderCommand::ClipPath { points } => {
                // Approximate clip path: push bounding rect of points
                if points.is_empty() {
                    return;
                }
                let min_x = points.iter().map(|p| p.x).min().unwrap();
                let max_x = points.iter().map(|p| p.x).max().unwrap();
                let min_y = points.iter().map(|p| p.y).min().unwrap();
                let max_y = points.iter().map(|p| p.y).max().unwrap();
                if min_x < max_x && min_y < max_y {
                    let cw = (max_x - min_x) as u32;
                    let ch = (max_y - min_y) as u32;
                    if cw > 0 && ch > 0 {
                        self.surface.push_clip(min_x, min_y, cw, ch);
                    }
                }
            }
            RenderCommand::SetBlendMode { mode } => {
                // Frame state, like the clip stack: it applies to everything drawn until it changes.
                // The writes it governs each go through the surface's blend-aware pixel path.
                self.surface.set_blend_mode(*mode);
            }
            RenderCommand::DrawConicGradient { center, start_angle, stops } => {
                if stops.is_empty() {
                    return;
                }
                let size = self.surface.size();
                let w = size.width as usize;
                let h = size.height as usize;
                if w == 0 || h == 0 {
                    return;
                }
                let back = self.surface.buffer.back_mut();
                let cx = center.x as f32;
                let cy = center.y as f32;
                let angle_offset = *start_angle;
                // Iterate over all pixels on the surface
                for py in 0..h {
                    for px in 0..w {
                        let dx = px as f32 - cx;
                        let dy = py as f32 - cy;
                        let mut t = dy.atan2(dx) + core::f32::consts::PI;
                        t = (t + angle_offset) % (2.0 * core::f32::consts::PI);
                        let pos = t / (2.0 * core::f32::consts::PI);
                        // Find the two stops surrounding pos
                        let color = if pos <= stops[0].0 {
                            stops[0].1
                        } else if pos >= stops.last().unwrap().0 {
                            stops.last().unwrap().1
                        } else {
                            let mut lo = 0usize;
                            let mut hi = stops.len() - 1;
                            while hi - lo > 1 {
                                let mid = (lo + hi) / 2;
                                if stops[mid].0 <= pos {
                                    lo = mid;
                                } else {
                                    hi = mid;
                                }
                            }
                            let t_local =
                                (pos - stops[lo].0) / (stops[hi].0 - stops[lo].0).max(0.0001);
                            let ca = stops[lo].1;
                            let cb = stops[hi].1;
                            Color::rgba(
                                (ca.r as f32 + (cb.r as f32 - ca.r as f32) * t_local) as u8,
                                (ca.g as f32 + (cb.g as f32 - ca.g as f32) * t_local) as u8,
                                (ca.b as f32 + (cb.b as f32 - ca.b as f32) * t_local) as u8,
                                (ca.a as f32 + (cb.a as f32 - ca.a as f32) * t_local) as u8,
                            )
                        };
                        set_pixel(back, w as u32, px as u32, py as u32, color);
                    }
                }
            }
        }
    }
    fn size(&self) -> Size {
        self.surface.size()
    }
    fn set_size(&mut self, size: Size) {
        self.surface.resize(size);
    }
    fn dpi_scale(&self) -> f32 {
        self.surface.dpi_scale()
    }
    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.surface.set_dpi_scale(dpi_scale);
    }
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.surface.measure_text(text, font)
    }
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        self.surface.shape_text(text, font)
    }
    fn frame_rgba(&self) -> &[u8] {
        self.surface.frame_rgba()
    }
    fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }
}

/// Apply a **Gaussian-approximating** blur to a rectangular region of an RGBA pixel buffer.
///
/// # Why a box blur is not what the command asks for
///
/// A `BoxShadow` declares a `blur_radius`, and the only blur a Gaussian has is a *standard
/// deviation*. One pass of a box blur is not that: it produces a flat plateau with hard corners, so
/// a shadow reads as a rectangle with a halo rather than as a falloff — the wider the radius, the
/// more visible the plateau. The crate's snapshots never showed it because no control drew a shadow
/// until the surface channel was wired (`render::surface`), which is exactly when this had to be
/// fixed: a shadow that is *drawn* is a shadow whose quality is visible.
///
/// # The approximation, and why it is the right one here
///
/// Three successive box blurs approximate a Gaussian to within a few percent (the central limit
/// theorem, applied deliberately). Ivan Kutskir's box-width formulas give the three widths whose
/// iterated result has the requested standard deviation, and they are computed for the **first**
/// box so that all three differ by at most one pixel — which is what keeps the result symmetric.
///
/// # Why this is also **faster**, not a cost paid for quality
///
/// The naive implementation this replaces summed `2r + 1` pixels per output pixel per pass, i.e.
/// `O(w * h * r)`, and it re-derived each sum from scratch. A box blur is a convolution with a
/// constant kernel, so the sum is a **sliding window**: leave one pixel, enter one pixel. That makes
/// each pass `O(w * h)` *regardless of radius* across all three boxes — so the Gaussian is not
/// merely comparable to the box blur it replaces, it is asymptotically better and the cost does not
/// grow with the blur the caller asks for.
///
/// # Edge handling
///
/// The region is expanded by the total kernel reach in each direction before the passes run, so an
/// edge pixel is blurred against its neighbours rather than against a shortened window (which would
/// darken or lighten the border). The window itself clamps its indices for the outermost pixels, so
/// the expansion only affects *which* pixels are written, never the arithmetic.
fn box_blur_region(
    back: &mut [u8],
    w: usize,
    h: usize,
    region_x: usize,
    region_y: usize,
    region_w: usize,
    region_h: usize,
    radius: usize,
) {
    if w == 0 || h == 0 || region_w == 0 || region_h == 0 || radius == 0 {
        return;
    }
    // The three box widths whose iteration approximates `sigma = radius / 2`.
    //
    // The divisor is the convention a Gaussian blur's radius follows: `radius` is the point at
    // which the kernel has decayed to roughly `1 / e^2` of its peak, which is about `2 sigma`. Using
    // `radius` directly as a standard deviation would make every existing shadow roughly four times
    // too soft, so the caller's unit is preserved and only the *shape* is corrected.
    let sigma = (radius as f32) / 2.0;
    let (b0, b1, b2) = box_blur_widths(sigma);
    let reach = b0.div_ceil(2) + b1.div_ceil(2) + b2.div_ceil(2);

    // Expand region by the kernel's total reach in all directions (clamped to surface bounds).
    let ex0 = region_x.saturating_sub(reach);
    let ey0 = region_y.saturating_sub(reach);
    let ex1 = (region_x + region_w + reach).min(w);
    let ey1 = (region_y + region_h + reach).min(h);
    let ew = ex1 - ex0;
    let eh = ey1 - ey0;
    if ew == 0 || eh == 0 {
        return;
    }

    // One scratch row and one scratch column, reused across all three passes.
    //
    // The previous implementation copied the whole expanded region into a `Vec` and ran both passes
    // over that copy. A sliding window does not need the copy — it needs somewhere to stage the
    // values a horizontal pass is about to overwrite while the pass is still reading them, which is
    // one row (and one column for the vertical pass). That is a *bounded* allocation per call rather
    // than one proportional to the blurred area, which matters because a full-surface `Blur` command
    // is the common case for a modal scrim.
    let mut row = vec![0u8; ew * 4];
    let mut col = vec![0u8; eh * 4];
    // The untouched row/column being blurred, kept because a pass writes into the same buffer it
    // slides its window over (see `blur_row`).
    let mut scratch = vec![0u8; ew.max(eh) * 4];

    let stride = w * 4;
    for width in [b0, b1, b2] {
        if width < 2 {
            continue;
        }
        // Horizontal pass: stage each row, blur it, write it back.
        for y in ey0..ey1 {
            let base = y * stride + ex0 * 4;
            row.copy_from_slice(&back[base..base + ew * 4]);
            scratch[..ew * 4].copy_from_slice(&row);
            blur_row(&mut row, ew, width, &scratch[..ew * 4]);
            back[base..base + ew * 4].copy_from_slice(&row);
        }
        // Vertical pass: stage each column, blur it, write it back.
        for x in ex0..ex1 {
            for y in 0..eh {
                let si = (ey0 + y) * stride + x * 4;
                col[y * 4..y * 4 + 4].copy_from_slice(&back[si..si + 4]);
            }
            scratch[..eh * 4].copy_from_slice(&col[..eh * 4]);
            blur_row(&mut col, eh, width, &scratch[..eh * 4]);
            for y in 0..eh {
                let di = (ey0 + y) * stride + x * 4;
                back[di..di + 4].copy_from_slice(&col[y * 4..y * 4 + 4]);
            }
        }
    }
}

/// The three box widths whose successive application approximates a Gaussian of `sigma`.
///
/// Ivan Kutskir's derivation, which is the standard one for this approximation. Three boxes of
/// widths `wl`, `wl`, `wu` give a variance of `(2 wl^2 + wu^2 - 3) / 12`, so solving for the `wl`
/// that lands nearest the requested `sigma` and taking the next odd width up as `wu` is the whole
/// of it. The two differ by exactly two, which keeps the iterated kernel symmetric — a set like
/// `(2, 5, 2)` would have a different falloff on each side.
///
/// # The per-pass share, which this used to omit
///
/// Variances **add** across independent passes, so the three boxes together must carry `sigma^2`,
/// i.e. each one carries `sigma^2 / 3` and therefore has a width that solves
/// `(w^2 - 1) / 12 = sigma^2 / 3`, i.e. `w = sqrt(12 * (sigma / sqrt(3))^2 + 1)`.
///
/// The width was computed as `sqrt(12 * sigma^2 + 1)` instead — the *whole* variance put into each
/// of the three passes — so the kernel came out `sqrt(3)` times too wide. Measured against the
/// intended standard deviation (the code's own `sigma = radius / 2`):
///
/// | radius | intended | produced | over-blur |
/// |---|---|---|---|
/// | 2 px | 1.0 | 1.83 | 1.83x |
/// | 6 px | 3.0 | 4.83 | 1.61x |
/// | 12 px | 6.0 | 9.83 | 1.64x |
/// | 24 px | 12.0 | 20.83 | 1.74x |
///
/// Every shadow a theme drew was therefore roughly **1.6–1.8x softer than its own elevation said**,
/// which also made `Theme::elevation`'s levels less distinguishable from one another than the ladder
/// intends. With the share applied, the realised variance lands within the integer-width quantisation
/// of the target at every radius (`sigma=3`: 8.00 against 9.00, `sigma=6`: 34.00 against 36.00,
/// `sigma=12`: 140.00 against 144.00).
///
/// Returns `(0, 0, 0)` for a non-positive `sigma`, which the caller reads as "no blur": a zero
/// width is a no-op pass rather than a division by zero.
fn box_blur_widths(sigma: f32) -> (usize, usize, usize) {
    if sigma <= 0.0 {
        return (0, 0, 0);
    }
    // Each of the three passes carries an equal share of the total variance.
    const PASSES: f32 = 3.0;
    let per_pass = sigma / PASSES.sqrt();
    // The width that contributes exactly that share: `variance = (w^2 - 1) / 12`, so
    // `w = sqrt(12 * per_pass^2 + 1)`. Rounded down, then forced odd so the box has a true centre.
    let ideal = (12.0 * per_pass * per_pass + 1.0).sqrt();
    let wl = match ideal.floor() as usize {
        w if w % 2 == 1 => w,
        w => w.saturating_sub(1),
    };
    (wl, wl, wl + 2)
}

/// One box-blur pass over a single row (or column) of interleaved RGBA pixels.
///
/// The window sums are maintained incrementally: each step subtracts the pixel leaving the window
/// and adds the one entering it, so the pass costs one add and one subtract per channel per pixel
/// rather than `2r + 1` of each. Indices are clamped at both ends, which is what makes the edge
/// pixels blur against the image rather than against a shorter (and therefore darker) window.
///
/// # Why the sum reads from ``source`` and not from ``pixels``
///
/// A blur output pixel must be a function of the **input** pixels around it, and this pass writes its
/// output into the same row it reads. Sliding the window over `pixels` would therefore subtract a
/// pixel it had already overwritten, and the error compounds along the row — the *first* output
/// pixel is correct (nothing has been written yet) and every one after it drifts, which is the exact
/// shape of the failure the agreement test caught. `source` is the untouched row, held by the caller;
/// `pixels` is only ever written to.
///
/// `pixels` is the single row/column, `len` its pixel count, `width` the box width in pixels
/// (odd, and at least 2 by the caller's check), `source` the pre-pass copy of `pixels`.
fn blur_row(pixels: &mut [u8], len: usize, width: usize, source: &[u8]) {
    if len == 0 || width < 2 {
        return;
    }
    let half = width / 2;

    // The window for output pixel `x` spans the **clamped** index range `lo..=hi`.
    //
    // Deriving the divisor from the same two bounds the sum comes from is what makes an edge pixel
    // exact rather than approximately right; and `lo`/`hi` are also the two pixels the window
    // exchanges when it slides, so the sum and the slide cannot disagree about where the window is.
    let lo_of = |x: usize| x.saturating_sub(half);
    let hi_of = |x: usize| (x + half).min(len - 1);

    // Signed accumulators.
    //
    // The slide adds one pixel and subtracts another *before* dividing, and the subtracted one can be
    // the brighter of the two — so a `u32` sum underflows on the pixel where the window stops growing
    // and starts sliding, which is every real image. `i64` makes the intermediate honest and the
    // final value is provably in `0..=255` because it is a mean of bytes.
    let mut sums = [0i64; 4];
    // The window for pixel 0, which is `[0, half]` with the upper end clamped.
    for i in 0..=hi_of(0) {
        let idx = i * 4;
        for c in 0..4 {
            sums[c] += source[idx + c] as i64;
        }
    }

    for x in 0..len {
        let count = (hi_of(x) - lo_of(x) + 1) as i64;
        let out = x * 4;
        for c in 0..4 {
            pixels[out + c] = (sums[c] / count) as u8;
        }
        if x + 1 == len {
            break;
        }
        // Slide to the window for `x + 1`.
        //
        // # Why neither side of the slide is unconditional
        //
        // A window resting against an edge does not slide, it **resizes**, and treating it as a slide
        // is wrong in both directions:
        //
        // * Near the **left** edge the window grows: at `x = 0` it is `[0, half]` and at `x = 1` it is
        //   `[0, half + 1]`, so a pixel enters and none leaves. Subtracting `lo_of(x)` there removes a
        //   pixel that is still inside the window.
        // * Near the **right** edge it shrinks: the last pixel enters when `x + 1 + half` reaches
        //   `len - 1`, and after that each step only drops one from the left. Adding `hi_of(x + 1)`
        //   there adds the same pixel a second time.
        //
        // Both errors are *constant* once made rather than cumulative, so they read as a uniform shift
        // and a slightly bright tail rather than as an obvious tear — the reason this survived a naive
        // box-blur-to-box-blur comparison and only fell out against a direct convolution.
        //
        // A pixel enters while the window is still growing, and a pixel leaves once the left edge can
        // advance: `x >= half` is exactly when the left edge stops being pinned at 0, and
        // `x + 1 + half <= len - 1` is exactly when the right edge has not yet been pinned.
        let mut delta = [0i64; 4];
        if x + 1 + half < len {
            let entering = (x + 1 + half) * 4;
            for c in 0..4 {
                delta[c] += source[entering + c] as i64;
            }
        }
        if x >= half {
            let leaving = lo_of(x) * 4;
            for c in 0..4 {
                delta[c] -= source[leaving + c] as i64;
            }
        }
        for c in 0..4 {
            sums[c] += delta[c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `Vec` from `compat` rather than the std prelude. These tests build pixel buffers, and on the
    // `ohos`/`android` cross targets there is no `std` prelude to supply `Vec` -- which is how they
    // broke `check_harmony_cross.sh` (`cannot find type Vec in this scope`) while every host build
    // stayed green. `compat` re-exports `alloc::vec::Vec`, so importing it makes the tests mean the
    // same thing on every target rather than only on the ones with a `std`.
    use crate::compat::MiniToString;
    use crate::compat::Vec;
    use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};

    // ── SoftwarePaintBackend construction ───────────────────────────────

    #[test]
    fn software_paint_backend_new_creates_surface() {
        let size = Size::new(100, 100);
        let backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), size);
        assert!((backend.dpi_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_zero_size() {
        let backend = SoftwarePaintBackend::new(Size::new(0, 0), 1.0);
        assert_eq!(backend.size(), Size::new(0, 0));
        let rgba = backend.frame_rgba();
        assert!(rgba.is_empty());
    }

    #[test]
    fn software_paint_backend_high_dpi() {
        let backend = SoftwarePaintBackend::new(Size::new(50, 50), 2.0);
        assert!((backend.dpi_scale() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_minimum_dpi_scale() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 0.0);
        assert!((backend.dpi_scale() - 0.1).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend surface access ─────────────────────────────

    #[test]
    fn software_paint_backend_surface_accessor() {
        let backend = SoftwarePaintBackend::new(Size::new(30, 30), 1.0);
        let surface = backend.surface();
        assert_eq!(surface.size(), Size::new(30, 30));
    }

    #[test]
    fn software_paint_backend_surface_mut_accessor() {
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        {
            let surface = backend.surface_mut();
            assert_eq!(surface.size(), Size::new(40, 40));
        }
    }

    // ── PaintBackend trait - frame lifecycle ────────────────────────────

    #[test]
    fn paint_backend_begin_end_frame_clears() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);

        backend.begin_frame(Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255); // R
            assert_eq!(chunk[1], 0); // G
            assert_eq!(chunk[2], 0); // B
            assert_eq!(chunk[3], 255); // A
        }
    }

    #[test]
    fn paint_backend_execute_fill_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(2, 2, 10, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 5 * stride + 5 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn paint_backend_execute_draw_rect_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRectStroke {
            rect: Rect::new(0, 0, 20, 20),
            color: Color::GREEN,
            width: 1,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_line() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawLine {
            from: Point::new(0, 0),
            to: Point::new(9, 9),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_push_pop_clip() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::PushClip { x: 0, y: 0, width: 5, height: 5 });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopClip);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_circle() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillCircleAA {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn paint_backend_size_set_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), Size::new(10, 10));

        let new_size = Size::new(50, 50);
        backend.set_size(new_size);
        assert_eq!(backend.size(), new_size);
    }

    #[test]
    fn paint_backend_dpi_scale_set_dpi_scale() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.set_dpi_scale(1.5);
        assert!((backend.dpi_scale() - 1.5).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend config ─────────────────────────────────────

    #[test]
    fn paint_backend_render_config_default() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = backend.render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn paint_backend_apply_render_config() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 2);
    }

    #[test]
    fn paint_backend_apply_render_config_clamps_to_normalized_range() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 99 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 8);

        let config = SoftwareRenderConfig { aa_samples_per_axis: 0 };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 1);
    }

    #[test]
    fn paint_backend_default_render_config() {
        let config = <SoftwarePaintBackend as PaintBackend>::render_config(
            &SoftwarePaintBackend::new(Size::new(1, 1), 1.0),
        );
        assert_eq!(config, SoftwareRenderConfig::default());
    }

    #[test]
    fn paint_backend_execute_draw_text_does_not_panic() {
        let size = Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let font = Font::simple("Arial", 12.0);
        backend.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 20),
            text: "Hello".to_string(),
            font,
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn paint_backend_execute_draw_image() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let data = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ];
        backend.execute_command(&RenderCommand::DrawImage {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            data,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_rect() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_rounded_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRoundedRect {
            rect: Rect::new(2, 2, 16, 16),
            radius: 4,
            color: Color::GREEN,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx + 1], 255); // G
    }

    #[test]
    fn paint_backend_execute_draw_circle_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawCircleStroke {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::RED,
            width: 2,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── PaintBackend measure_text / shape_text ──────────────────────────

    #[test]
    fn paint_backend_measure_text_returns_metrics() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let metrics = backend.measure_text("Hello", &font);
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
    }

    #[test]
    fn paint_backend_shape_text_returns_shaped() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let shaped = backend.shape_text("Hi", &font);
        assert!(!shaped.clusters.is_empty());
    }

    #[test]
    fn paint_backend_frame_rgba_after_clear() {
        let size = Size::new(5, 5);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgb(128, 64, 32));
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let expected_len = 5 * 5 * 4;
        assert_eq!(rgba.len(), expected_len);

        assert_eq!(rgba[0], 128);
        assert_eq!(rgba[1], 64);
        assert_eq!(rgba[2], 32);
        assert_eq!(rgba[3], 255);
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn paint_backend_execute_fill_rect_out_of_bounds() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(100, 100, 50, 50),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
            assert_eq!(chunk[1], 255);
            assert_eq!(chunk[2], 255);
        }
    }

    #[test]
    fn paint_backend_fill_rect_zero_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 0, 0),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
        }
    }

    #[test]
    fn paint_backend_multiple_commands_in_frame() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 5, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(5, 0, 5, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let left_idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[left_idx], 255); // R
        assert_eq!(rgba[left_idx + 2], 0); // B

        let right_idx = 5 * stride + 7 * 4;
        assert_eq!(rgba[right_idx], 0); // R
        assert_eq!(rgba[right_idx + 2], 255); // B
    }

    /// The blur's cost must not grow with the radius the caller asks for.
    ///
    /// This is the *reason* for the sliding window, so it is asserted rather than asserted-about
    /// (principle #1). The naive implementation this replaced summed `2r + 1` pixels per output pixel
    /// per pass, so a 40 px blur cost twenty times a 2 px one; the window costs the same for both,
    /// because each pixel enters the sum once and leaves once.
    ///
    /// # Why the bound is generous
    ///
    /// A ratio measured on a shared machine is noisy, and a test that fails on a scheduling hiccup is
    /// worse than no test. So this asserts the **asymptotic** property with a wide margin: the linear
    /// implementation would be ~20x here, and anything under 4x is unambiguously the constant-cost
    /// shape. The measurement is best-of-three so a single preemption cannot decide it.
    #[test]
    fn the_blur_cost_does_not_grow_with_the_radius() {
        use std::time::Instant;

        let (w, h) = (400usize, 400usize);
        let budget = |radius: usize| {
            let mut best = core::time::Duration::MAX;
            for _ in 0..3 {
                let mut buf: Vec<u8> =
                    (0..w * h * 4).map(|i| ((i as f32 * 0.37).sin().abs() * 255.0) as u8).collect();
                let start = Instant::now();
                box_blur_region(&mut buf, w, h, 0, 0, w, h, radius);
                best = best.min(start.elapsed());
            }
            best
        };

        let small = budget(2);
        let large = budget(40);
        // A blur of 40 is 20x the kernel of a blur of 2. Anything near that is the old shape.
        let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 4.0,
            "a 40 px blur took {ratio:.1}x a 2 px one ({large:?} vs {small:?}) — the cost is still \
             growing with the radius, so the window is not sliding"
        );
    }

    // ── The Gaussian-approximating blur ─────────────────────────────────

    /// The sliding window must agree with the naive sum it replaced.
    ///
    /// This is the whole risk of the optimisation: an incremental window that adds and subtracts the
    /// wrong pixels is *plausible* — it produces a smooth-looking image — and wrong. So it is checked
    /// against a direct convolution, which is the definition, on a signal with no symmetry that could
    /// hide a mirrored index error.
    #[test]
    fn the_sliding_window_agrees_with_a_direct_convolution() {
        // A row of distinct values, so an off-by-one in either direction changes the output.
        let len = 23usize;
        let width = 5usize;
        let half = width / 2;
        let source: Vec<u8> =
            (0..len * 4).map(|i| ((i as f32 * 37.0).sin().abs() * 200.0 + 20.0) as u8).collect();
        let mut blurred = source.clone();
        blur_row(&mut blurred, len, width, &source);

        for x in 0..len {
            for c in 0..4 {
                // The reference: clamp the window at both ends *and* divide by the number of pixels
                // actually accumulated, which is what `blur_row` documents it does.
                let lo = x.saturating_sub(half);
                let hi = (x + half).min(len - 1);
                let sum: u32 = (lo..=hi).map(|k| source[k * 4 + c] as u32).sum();
                let expected = (sum / (hi - lo + 1) as u32) as u8;
                assert_eq!(
                    blurred[x * 4 + c],
                    expected,
                    "pixel {x} channel {c}: the window and the direct sum must agree"
                );
            }
        }
    }

    /// The window count must be exact, not merely close.
    ///
    /// A divisor that is off by one at an edge produces a subtle gradient rather than an obvious
    /// band, and the agreement test above would catch it only because it re-derives the same count.
    /// Pinning the count directly is what makes that test meaningful rather than circular.
    #[test]
    fn a_constant_field_survives_the_edge_clamping() {
        // A flat field must stay flat: every clamped window sums `count` copies of the same value, so
        // a divisor larger than `count` would darken the border and one smaller would lighten it.
        let len = 16usize;
        let mut row = vec![200u8; len * 4];
        blur_row(&mut row, len, 7, &vec![200u8; len * 4]);
        assert!(
            row.iter().all(|&v| v == 200),
            "a constant field must be unchanged by a blur, including at the edges"
        );
    }

    /// The three box widths must number three, be odd, and grow with the requested sigma.
    ///
    /// Odd widths give each box a true centre pixel, which is what makes the iterated kernel
    /// symmetric; a monotonic response is what makes `radius` mean something to a caller.
    #[test]
    fn the_box_widths_are_odd_and_monotonic() {
        let mut previous = 0usize;
        for radius in [1usize, 2, 4, 8, 16, 32, 64] {
            let (a, b, c) = box_blur_widths(radius as f32 / 2.0);
            assert!(a % 2 == 1 && b % 2 == 1 && c % 2 == 1, "widths must be odd: {a} {b} {c}");
            assert_eq!(a, b, "the pair must match, or the kernel is asymmetric");
            let reach = a + c;
            assert!(reach >= previous, "radius {radius} must not blur less than a smaller one");
            previous = reach;
        }
        assert_eq!(box_blur_widths(0.0), (0, 0, 0), "a zero sigma is not a blur");
        assert_eq!(box_blur_widths(-1.0), (0, 0, 0), "and neither is a negative one");
    }

    /// The three boxes together must realise the requested `sigma`, not three times its variance.
    ///
    /// # The defect this pins
    ///
    /// `box_blur_widths` solved each width from `variance = (w^2 - 1) / 12 == sigma^2`, i.e. it put
    /// the **whole** requested variance into **each** of the three passes. Variances add, so the
    /// kernel that came out had three times the intended variance -- about `sqrt(3)` times too wide,
    /// which measured out as every shadow being 1.6-1.8x softer than its own elevation said:
    ///
    /// | radius | intended sigma | produced |
    /// |---|---|---|
    /// | 2 px | 1.0 | 1.83 |
    /// | 6 px | 3.0 | 4.83 |
    /// | 12 px | 6.0 | 9.83 |
    /// | 24 px | 12.0 | 20.83 |
    ///
    /// The widths are still *odd* and still *monotonic* with such a bug present, which is exactly
    /// why `the_box_widths_are_odd_and_monotonic` above cannot see it: it asserts the shape of the
    /// response, not its scale. This test asserts the scale.
    ///
    /// # Why the radii below are the ladder's own, and why the band is wide at the small end
    ///
    /// A box width is an integer, so the realised variance can only land where some `(w^2 - 1) / 12`
    /// falls. The residual is therefore quantisation, and it shrinks as a fraction of the target as
    /// sigma grows. Measured after the fix, over exactly the four radii
    /// [`crate::render::default_shadow`] uses (`blur` 3/6/12/24):
    ///
    /// | blur | sigma | realised / target | standard deviation |
    /// |---|---|---|---|
    /// | 3 | 1.5 | 1.48 | 1.22x |
    /// | 6 | 3.0 | 0.889 | 0.94x |
    /// | 12 | 6.0 | 0.944 | 0.97x |
    /// | 24 | 12.0 | 0.972 | 0.99x |
    ///
    /// So the band is `0.6..=1.6` on variance (0.77x-1.26x on standard deviation): it admits the
    /// quantisation at `blur: 3`, and it still rejects the `sqrt(3)`-scale error by a wide margin --
    /// that bug produces 2.59-3.67x, i.e. outside the band at every radius.
    ///
    /// # Why `radius: 1` is deliberately not in the list
    ///
    /// Because at that radius the test **cannot tell the fix from the bug**, and a case that passes
    /// either way is not evidence. `box_blur_widths(0.5)` is `(1, 1, 3)` both before and after the
    /// fix -- every width floors to the same odd integers -- so both kernels realise 0.67 against a
    /// target of 0.25, a ratio of 2.667. Asserting it would either fail the *correct* code or force a
    /// band wide enough to admit the *broken* one, and both of those make the test worse than not
    /// having it.
    ///
    /// The smallest radius where a separating integer kernel exists is 2 (`sigma` 1.0), and the
    /// separation is complete from there up: fixed gives 0.667/1.48/0.83/0.89/0.92/0.94/0.97/0.99,
    /// buggy gives 3.33/3.56/3.67/2.59/2.92/2.69/3.01/3.04. Excluding one radius for a stated
    /// reason is the honest form of this exclusion; silently widening the band to cover it would
    /// have hidden exactly the defect the test exists for.
    #[test]
    fn the_three_boxes_realise_the_requested_sigma_not_three_times_it() {
        let variance_of = |w: usize| {
            if w == 0 {
                0.0
            } else {
                (w * w - 1) as f32 / 12.0
            }
        };

        // `default_shadow`'s four levels, plus a finer and a coarser radius so a regression that
        // only shows at one scale cannot hide behind the ladder's particular values. `radius: 1` is
        // excluded on purpose -- see this test's rustdoc.
        for radius in [2usize, 3, 4, 6, 8, 12, 24, 48, 96] {
            let sigma = radius as f32 / 2.0;
            let (a, b, c) = box_blur_widths(sigma);
            let realised = variance_of(a) + variance_of(b) + variance_of(c);
            let target = sigma * sigma;
            let ratio = realised / target;
            assert!(
                (0.6..=1.6).contains(&ratio),
                "radius {radius}: the kernel realises variance {realised:.2} against a target of \
                 {target:.2} (ratio {ratio:.3}), i.e. a standard deviation {:.2}x the one asked for. \
                 Widths were ({a}, {b}, {c}). A ratio near 3 means the per-pass variance share was \
                 dropped again.",
                ratio.sqrt()
            );
        }
    }

    /// The blur is separable: three boxes in x and three in y must equal the same work done the
    /// other way round.
    ///
    /// This is the property that lets the implementation do a row pass and a column pass instead of
    /// a two-dimensional kernel, and it is the one an accidental transposition would break.
    #[test]
    fn the_blur_is_separable() {
        let (w, h) = (9usize, 7usize);
        let source: Vec<u8> =
            (0..w * h * 4).map(|i| ((i as f32 * 91.0).cos().abs() * 180.0 + 30.0) as u8).collect();

        let mut horizontal_first = source.clone();
        box_blur_region(&mut horizontal_first, w, h, 0, 0, w, h, 6);

        // The same region blurred after a vertical translation of the *source* by zero pixels is
        // the only honest comparison: `box_blur_region` always runs x then y, so "y then x" is
        // expressed by blurring a transposed copy and transposing back.
        let mut transposed = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                transposed[(x * h + y) * 4..(x * h + y) * 4 + 4]
                    .copy_from_slice(&source[(y * w + x) * 4..(y * w + x) * 4 + 4]);
            }
        }
        box_blur_region(&mut transposed, h, w, 0, 0, h, w, 6);

        // A box blur is symmetric, so transposing the result back is a reference for the original.
        // Tolerated by one level: the three boxes are applied in a different order, so integer
        // truncation can land on either side of the exact value.
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    let a = horizontal_first[(y * w + x) * 4 + c] as i32;
                    let b = transposed[(x * h + y) * 4 + c] as i32;
                    assert!(
                        (a - b).abs() <= 1,
                        "({x},{y}) channel {c}: {a} vs {b} — the passes are not separable"
                    );
                }
            }
        }
    }
}
