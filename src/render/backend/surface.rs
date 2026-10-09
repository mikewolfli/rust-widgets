// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Software rendering surface: back buffer, surface, and configuration.
use crate::compat::{lock, vec, MiniToString, Mutex, OnceLock, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::pixel_bytes_len;
use crate::render::{BlendMode, PaintBackend, RenderCommand, ShapedText, TextMetrics};

/// Double-buffered 8-bit RGBA pixel storage used by software rendering.
///
/// Pixels are stored as `[R, G, B, A]` bytes with no padding: the buffers are
/// exactly `width * height * 4` bytes long and rows are tightly packed, so the
/// byte offset of pixel `(x, y)` is `(y * width + x) * 4`.
///
/// Two buffers are held. The **back** buffer is the drawing target; all
/// rasterisation writes there. The **front** buffer holds the last presented
/// frame. [`BackBuffer::present`] swaps them, and reads through
/// [`BackBuffer::front`] therefore observe the previously *back* buffer, not
/// the pixels most recently written via [`BackBuffer::back_mut`].
///
/// Both buffers are zero-initialised (fully transparent black) on creation and
/// on growth during [`BackBuffer::resize`].
#[derive(Debug, Clone)]
pub struct BackBuffer {
    size: Size,
    dpi_scale: f32,
    front: Vec<u8>,
    pub(crate) back: Vec<u8>,
}
impl BackBuffer {
    /// Creates a new back buffer of `size` physical pixels at `dpi_scale`.
    ///
    /// Both front and back are allocated and zero-filled. `dpi_scale` is
    /// clamped to a minimum of `0.1`; values above `1.0` denote HiDPI scaling.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let bytes = pixel_bytes_len(size);
        Self { size, dpi_scale: dpi_scale.max(0.1), front: vec![0; bytes], back: vec![0; bytes] }
    }
    /// Resizes front and back buffers to `size` pixels.
    ///
    /// Growing a buffer appends zero bytes, so newly exposed pixels are
    /// transparent black. Shrinking truncates, discarding the overflowed rows
    /// and columns; no attempt is made to preserve or rescale the remaining
    /// content, so the retained prefix is *not* a correctly re-strided image.
    pub fn resize(&mut self, size: Size) {
        self.size = size;
        let bytes = pixel_bytes_len(size);
        self.front.resize(bytes, 0);
        self.back.resize(bytes, 0);
    }
    /// Returns the buffer size in physical pixels, not in logical units.
    pub fn size(&self) -> Size {
        self.size
    }
    /// Returns the logical DPI scale; `1.0` means unscaled.
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
    /// Updates the logical DPI scale, clamped to a minimum of `0.1`.
    ///
    /// Only the recorded scale changes; existing pixel data is not resized or
    /// resampled.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale.max(0.1);
    }
    /// Returns the back buffer's pixels for writing, in RGBA byte order.
    ///
    /// This is the buffer that rendering targets. Its length is
    /// `size.width * size.height * 4`.
    pub fn back_mut(&mut self) -> &mut [u8] {
        &mut self.back
    }
    /// Returns the front buffer's pixels in RGBA byte order.
    ///
    /// These are the pixels of the most recently presented frame; writes made
    /// through [`BackBuffer::back_mut`] since the last [`BackBuffer::present`]
    /// are not visible here.
    pub fn front(&self) -> &[u8] {
        &self.front
    }
    /// Swaps the front and back buffers, publishing the drawn frame.
    ///
    /// After the swap the former back buffer is readable through
    /// [`BackBuffer::front`], while the back buffer holds the previous frame's
    /// pixels rather than a cleared surface; callers typically clear it at the
    /// start of the next frame.
    pub fn present(&mut self) {
        core::mem::swap(&mut self.front, &mut self.back);
    }
}
/// Software raster surface with quality controls and RGBA frame output.
///
/// Wraps a double-buffered [`BackBuffer`]: drawing commands and
/// [`SoftwareSurface::begin_frame`] act on the back buffer, and
/// [`SoftwareSurface::end_frame`] presents it. Reads via
/// [`SoftwareSurface::frame_rgba`] therefore return the last presented frame
/// (the front buffer), which is one frame behind the buffer currently being
/// drawn into.
///
/// Pixels are 8-bit RGBA, tightly packed at 4 bytes per pixel regardless of
/// DPI scale; `dpi_scale` only affects text metrics and geometry that opts in.
pub struct SoftwareSurface {
    pub(crate) buffer: BackBuffer,
    pub(crate) aa_samples_per_axis: u8,
    /// Active clip rectangle stack. An empty stack means no clipping.
    pub(crate) clip_stack: Vec<(i32, i32, u32, u32)>,
    /// The blend mode the next pixel write composites through.
    ///
    /// Set by [`SoftwareSurface::set_blend_mode`] (driven by
    /// [`crate::render::RenderCommand::SetBlendMode`]) and read by every pixel write. It lives on
    /// the surface rather than on a primitive because a mode applies to *everything drawn until it
    /// changes* — a sequential fact, exactly like the clip stack — and because every opaque and
    /// antialiased primitive funnels its writes through [`SoftwareSurface::write_pixel`].
    pub(crate) blend_mode: BlendMode,
}
/// Public software render configuration for quality-related knobs.
///
/// This type is `Copy` and cheap to pass around; a [`SoftwareSurface`] inherits
/// the process-wide default returned by [`default_software_render_config`] at
/// construction time unless [`SoftwareSurface::apply_render_config`] overrides
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRenderConfig {
    /// Number of anti-aliasing samples taken along each axis, forming an
    /// `n x n` sample grid. Higher values cost more per primitive. The valid
    /// range is `1..=8` (where `1` disables anti-aliasing); out-of-range values
    /// are silently clamped by [`SoftwareRenderConfig::normalized`] or
    /// [`SoftwareSurface::apply_render_config`]. Defaults to `4`.
    pub aa_samples_per_axis: u8,
}
impl Default for SoftwareRenderConfig {
    fn default() -> Self {
        Self { aa_samples_per_axis: 4 }
    }
}
impl SoftwareRenderConfig {
    /// Returns a copy with every value clamped into its documented range.
    ///
    /// Currently this only clamps `aa_samples_per_axis` to `1..=8`.
    pub fn normalized(self) -> Self {
        Self { aa_samples_per_axis: self.aa_samples_per_axis.clamp(1, 8) }
    }
}
fn global_software_render_config() -> &'static Mutex<SoftwareRenderConfig> {
    static CONFIG: OnceLock<Mutex<SoftwareRenderConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(SoftwareRenderConfig::default()))
}

/// Horizontal breathing space [`RenderContext::draw_text_fitted`] keeps at each end.
///
/// Three pixels: enough that a fitted label does not touch the frame it sits in, small
/// enough not to visibly shorten a label that already fits. Named so the fit check and the
/// origin computation cannot disagree about it — they did while it was a literal in one of
/// the two places.
pub const TEXT_FIT_MARGIN: u32 = 3;

/// The glyph-box origin for `text` fitted into `bounds` under `alignment`.
///
/// # Why the usable extent is inset on **both** ends
///
/// `TEXT_FIT_MARGIN` exists so a fitted label does not touch the frame it sits in, and that is
/// a statement about both ends of the box. The width handed to the fitter was nevertheless
/// `bounds.width - inset`, i.e. inset at one end only, so a string measured to "fit" filled
/// `bounds.width - 3` and then had to be *positioned* in the full `bounds.width`.
///
/// For `Left` that was invisible, because the origin is the box's own left edge plus the inset.
/// For `Center` and `Right` it was a **half-inset displacement**: the string was centred in the
/// box, so the surplus `inset` was split as `inset / 2` on the right and `(inset - inset / 2)`
/// on the left — a floor division that lands one pixel to the left at the crate's default
/// margin. Measured over the census geometries, every centred label sat 1-2 px left of its
/// box's centre, and the error was largest exactly where it is most visible: the calendar's
/// ~34 px cells and the badge and keyboard key caps.
///
/// The two halves of the contract are now the same statement. The free space is
/// `width - advance`, both ends are inset by `inset`, and what is left over is split **evenly**
/// and with the same rounding on both sides — `inset + (free - inset + 1) / 2` has to be written
/// with the `+ 1` so that the odd pixel goes to the left half rather than being dropped every
/// time. A caller that wants a different inset changes `TEXT_FIT_MARGIN` and both the fit and
/// the position follow, which is what the constant is named for.
pub(crate) fn fitted_origin(bounds: Rect, advance: i32, alignment: HorizontalAlignment) -> Point {
    let inset = TEXT_FIT_MARGIN as i32;
    let left = bounds.x.saturating_add(inset);
    let right = (bounds.x + bounds.width as i32 - inset).max(left);
    let free = (right - left) - advance;
    match alignment {
        HorizontalAlignment::Left => Point::new(left, bounds.y),
        HorizontalAlignment::Center => Point::new(left + (free + 1) / 2, bounds.y),
        HorizontalAlignment::Right => Point::new(right - advance, bounds.y),
    }
}

/// Where a single line of text sits inside the band it is drawn in.
///
/// # Why this is not [`HorizontalAlignment`]
///
/// The two axes have different owners. Horizontal placement is chosen *per label* (a title
/// is centred, a value is right-aligned), so it is a per-call argument. Vertical placement
/// is a property of **what the band is**: the box around one line of text in a row, a cell
/// or a button is centred by construction, and a label that is top-aligned is one whose
/// author has a reason. Splitting the two is what lets a caller state the common case
/// (`Centered`) and get the arithmetic right, rather than re-deriving it — which is the
/// defect this type exists to remove.
///
/// # The contract it encodes
///
/// `RenderContext`'s text origin is the glyph box's **top-left** (see
/// [`RenderContext::draw_text`]). So "centred" is not `band.y + band.height / 2`; it is
/// `band.y + (band.height - line_height) / 2`. The first form puts the box's *top edge* on
/// the band's middle line and draws the whole label half a line low — the single most
/// repeated placement error in this crate (76 sites across 40 files before this type
/// existed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlignment {
    /// The line box's top edge coincides with the band's top edge.
    Top,
    /// The line box is centred in the band. The default, because a band named for a single
    /// line of text is almost always meant to hold it centred.
    #[default]
    Center,
    /// The line box's bottom edge coincides with the band's bottom edge.
    Bottom,
}

/// The line box a single line of `font` occupies when placed in `band`.
///
/// Returns a rectangle with `band`'s horizontal extent and the measured line height of
/// `font`, positioned vertically inside `band` according to `alignment`. Passing the result
/// to [`RenderContext::draw_text_fitted`] (or [`RenderContext::draw_text`]) puts the glyphs
/// where the author meant.
///
/// # Why an explicit function rather than a `draw_*` variant
///
/// The placement is needed *before* the draw call in most callers: a control that draws a
/// label and a rule under it needs the line box to know where the rule goes, and one that
/// draws a focus ring needs it to size the ring. Returning the box lets both read the same
/// derivation instead of one of them recomputing it — and a recomputation is exactly how a
/// label and the thing below it drift apart.
///
/// # Degenerate input
///
/// A band narrower than the line height, or one of zero height, clamps the offset to zero
/// rather than going negative: a band that cannot fit a line still draws the line, at its
/// top edge, which is the reading a caller can act on. Nothing here panics, and nothing
/// divides by a value read from the caller, so a zero-sized band is a supported input.
///
/// ```ignore
/// let line = text_line(cell, &font, context);
/// context.draw_text_fitted(line, label, &font, ink, HorizontalAlignment::Left);
/// ```
///
/// # Why there is no timing instrumentation here
///
/// This function sits on the hot path of every caption in the frame — `grep` counts 228 call sites
/// in `src/widget/` — and it briefly carried
///
/// ```ignore
/// let t0 = std::time::Instant::now();
/// let height = context.measure_text("M", font).height.max(1) as i32;
/// eprintln!("[MEAS] text_line measure_text('M') = {}us", t0.elapsed().as_micros());
/// ```
///
/// which printed one line to stderr **per measured caption, per frame**, together with the clocks
/// to time it. That is a permanently-armed diagnostic rather than a temporary one: it was
/// committed, not `cfg(test)`-gated, not behind a log level, and the only thing distinguishing it
/// from intentional output was a `[MEAS]` prefix. A `grep` for debug output does not find it either
/// — the crate's own `log::` macros are the sanctioned channel, and this bypassed them.
///
/// The measurement it was after is real work (a `text_line` per caption is worth knowing the cost
/// of), so the honest home for it is a `log::trace!` behind the `log` facade, where the host
/// chooses whether to pay for it. Until that exists, the timing is removed rather than left
/// un-flagged: a hot path may not print.
pub fn text_line(
    band: Rect,
    font: &Font,
    alignment: VerticalAlignment,
    context: &RenderContext,
) -> Rect {
    let height = context.measure_text("M", font).height.max(1) as i32;
    let offset = match alignment {
        VerticalAlignment::Top => 0,
        VerticalAlignment::Center => ((band.height as i32 - height) / 2).max(0),
        VerticalAlignment::Bottom => (band.height as i32 - height).max(0),
    };
    Rect::new(band.x, band.y.saturating_add(offset), band.width, height as u32)
}

/// The longest prefix of `text` that advances at most `max_width` pixels in `font`.
///
/// Returns `text` unchanged when it fits, an empty string when not even one cluster fits,
/// and otherwise a prefix ending in `…` whose advance stays within the budget.
///
/// # Why the advance model is repeated here
///
/// It matches [`PaintBackend::shape_text`]: one cluster per grapheme, and the shaped run's
/// total advance is the sum of its cluster advances plus the font's tracking between them.
/// Measuring with the model the renderer draws with is what makes the fitted string actually
/// fit; a private estimate here would reintroduce the mismatch this exists to remove (a CJK
/// title measured by `len()` is four times its drawn width).
///
/// # Why the prefix is found in one pass (D09-RENDER-03)
///
/// The previous loop built each candidate by cloning the kept prefix, pushing one char, and
/// calling `measure_text` on the whole candidate — so every accepted cluster was re-copied and
/// re-shaped on each later iteration, and the work grew as `1 + 2 + … + K = O(K²)` in the number
/// of clusters that fit. That is quadratic on exactly the inputs this helper exists for: long
/// labels, paths and status strings clipped to a narrow control.
///
/// The width of a prefix is *additive* over its clusters, so the whole text is shaped **once**,
/// the cluster advances are accumulated in a single running sum, and the split point is taken
/// where that sum first exceeds the budget. The candidate string is assembled once from the
/// accepted clusters' text; no candidate is ever re-measured. The result is the same clusters,
/// measured with the same advances, so the fitted string — including ellipsis, CJK and the
/// exact width boundary — is unchanged; only the cost is.
fn fit_text_to_width(
    text: &str,
    max_width: f32,
    font: &Font,
    backend: &dyn PaintBackend,
) -> crate::compat::String {
    if text.is_empty() || max_width <= 0.0 {
        return crate::compat::String::new();
    }
    if backend.measure_text(text, font).width as f32 <= max_width {
        return text.to_string();
    }
    // A single ellipsis stands in when the budget cannot hold any glyph at all: an empty
    // label would read as "this control has no text", which is a different statement.
    const ELLIPSIS: char = '\u{2026}';
    let ellipsis_width = backend.measure_text(&ELLIPSIS.to_string(), font).width as f32;
    if ellipsis_width > max_width {
        return ELLIPSIS.to_string();
    }
    let budget = max_width - ellipsis_width;

    // One shaping pass for the whole string. Its cluster advances are the renderer's own model.
    let shaped = backend.shape_text(text, font);
    let clusters = shaped.clusters();

    // # Why the order is checked before the linear pass (D09-RENDER-03)
    //
    // The accumulation below walks the shaped clusters in the order `shape_text` returned them,
    // which is **visual** order. For a left-to-right line (every Latin, CJK and combined-script
    // label) visual order equals logical order, so the accepted prefix is a string prefix.
    // Bidirectional reordering permutes the clusters, so a visual prefix is not a string prefix
    // there; the concatenation of the shaped clusters reproduces `text` exactly when no reorder
    // happened, and that comparison leaves the rare reordered line on the original per-prefix
    // walk — same behaviour, no quadratic cost paid where the reorder does not occur.
    let shaped_matches_text = {
        let mut rebuilt = crate::compat::String::new();
        for cluster in clusters {
            rebuilt.push_str(&cluster.text);
        }
        rebuilt == text
    };
    if !shaped_matches_text {
        return fit_text_to_width_by_prefix(text, budget, font, backend, ELLIPSIS);
    }

    // Tracking adds one inter-cluster gap per adjacent pair, so a run of `n` clusters carries
    // `n - 1` gaps; it is part of the shaped *total* but not of any single cluster's advance
    // (see `shape_line`), so an accumulated prefix has to add it the same way.
    let tracking = font.letter_spacing() * backend.dpi_scale();

    // # Why the candidates are re-checked (D09-RENDER-03)
    //
    // A cluster's advance comes from shaping the **whole** string, and with a real face the
    // shaper is context-sensitive: shaping a prefix in isolation can differ by a sub-pixel
    // amount (a kern pair, a boundary rounding). So the running sum is a very good *estimate* of
    // where the split falls, not a proof of it. Only the prefixes near that estimate are then
    // confirmed with the renderer's own `measure_text`, which is the same call the old loop made
    // and therefore the ground truth for the result. The estimate is within a rounding step, so
    // the confirmed window is a couple of clusters wide; the number of shaping *passes* is one
    // for the whole string plus a constant for the confirmation, not one per kept cluster.
    let mut estimated = 0usize;
    {
        let mut advance = 0.0f32;
        for cluster in clusters {
            let with_tracking = if estimated == 0 { 0.0 } else { tracking };
            let candidate_advance = advance + with_tracking + cluster.advance;
            if candidate_advance.round() > budget {
                break;
            }
            advance = candidate_advance;
            estimated += 1;
        }
    }

    // True measure of a prefix of `length` clusters, matching `measure_text`'s contract.
    let prefix_fits = |length: usize| -> bool {
        if length == 0 {
            return true;
        }
        let prefix: crate::compat::String =
            clusters[..length].iter().flat_map(|cluster| cluster.text.chars()).collect();
        backend.measure_text(&prefix, font).width as f32 <= budget
    };

    // Confirm the estimate by finding the **first** prefix that does not fit, which is exactly
    // what the old cluster-by-cluster loop returned (`kept` was the prefix just before the first
    // failure). The estimate comes from whole-string advances and can differ from the isolated
    // prefix measurement by a sub-pixel amount, so the search starts a couple of clusters below
    // it and walks forward. Two clusters of slack cover the rounding; if the true boundary is
    // further than that — which a context-sensitive shaper could in principle produce — the short
    // walk does not reach a failure and the exact per-prefix walk is used instead, so the result
    // is always the one the old code produced.
    const CONFIRM_SLACK: usize = 2;
    let search_from = estimated.saturating_sub(CONFIRM_SLACK);
    let mut confirmed = None;
    let mut length = search_from;
    while length <= clusters.len() {
        if !prefix_fits(length) {
            confirmed = Some(length - 1);
            break;
        }
        length += 1;
    }
    let confirmed = match confirmed {
        Some(value) => value,
        // No failure within the window: only trust it when the window actually reached the end
        // (every prefix fit), otherwise the estimate was off by more than the slack and the
        // guaranteed-exact walk takes over.
        None if length > clusters.len() => clusters.len(),
        None => return fit_text_to_width_by_prefix(text, budget, font, backend, ELLIPSIS),
    };

    let mut kept = crate::compat::String::new();
    for cluster in &clusters[..confirmed] {
        kept.push_str(&cluster.text);
    }

    kept.push(ELLIPSIS);
    kept
}

/// The pre-D09-RENDER-03 prefix walk, kept for the bidi order the linear pass cannot use.
///
/// # Why this is retained rather than deleted
///
/// The single-pass fitter above assumes the shaped clusters are in logical order. A
/// bidirectionally reordered line breaks that assumption, and the honest answer is to keep the
/// per-prefix search that is correct for it rather than to silently assemble the wrong string.
/// The quadratic cost only applies to the reordered line, which is a small minority of labels;
/// the common case never reaches here.
fn fit_text_to_width_by_prefix(
    text: &str,
    budget: f32,
    font: &Font,
    backend: &dyn PaintBackend,
    ellipsis: char,
) -> crate::compat::String {
    let mut kept = crate::compat::String::new();
    for ch in text.chars() {
        let candidate = {
            let mut next = kept.clone();
            next.push(ch);
            next
        };
        if backend.measure_text(&candidate, font).width as f32 > budget {
            break;
        }
        kept = candidate;
    }
    kept.push(ellipsis);
    kept
}

#[cfg(test)]
pub(crate) fn software_render_config_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
/// Sets the process-wide default software render configuration.
///
/// The value is normalized (clamped) before being stored. The setting is
/// global and guarded by a mutex, so it is not scoped to a thread or a window;
/// it affects surfaces created afterwards. Newly created [`SoftwareSurface`]s
/// pick it up, existing ones keep their own value until reconfigured.
///
/// # Panics
///
/// Panics if the internal config lock is poisoned by a previous panic.
///
/// ```text
/// // Illustrates the API shape; the crate's tests use an internal guard lock.
/// set_default_software_render_config(SoftwareRenderConfig { aa_samples_per_axis: 4 });
/// ```
pub fn set_default_software_render_config(config: SoftwareRenderConfig) {
    *lock(global_software_render_config()) = config.normalized();
}
/// Returns a copy of the process-wide default software render configuration.
///
/// This never fails on an unset value: it falls back to
/// [`SoftwareRenderConfig::default`] the first time it is called.
///
/// # Panics
///
/// Panics if the internal config lock is poisoned by a previous panic.
pub fn default_software_render_config() -> SoftwareRenderConfig {
    *lock(global_software_render_config())
}
/// Render context for custom widget drawing.
///
/// Handed to widget draw code and wrapping a [`PaintBackend`]. Every primitive
/// is translated by the context's current offset *before* being forwarded to
/// the backend, which lets containers translate their children without
/// mutating child geometry. Unless stated otherwise, coordinates are physical
/// pixels in surface space and each draw call is clipped by whatever clip
/// rectangle is currently active on the backend.
pub struct RenderContext<'a> {
    backend: &'a mut dyn PaintBackend,
    /// Current translation applied to all subsequent draw primitives.
    offset_x: i32,
    offset_y: i32,
    /// Stack of previous offsets, restored by [`RenderContext::pop_offset`].
    offset_stack: Vec<(i32, i32)>,
}
impl<'a> RenderContext<'a> {
    /// Wraps `backend` in a context whose translation offset starts at `(0, 0)`.
    ///
    /// The context borrows the backend mutably; it does not take ownership.
    pub fn new(backend: &'a mut dyn PaintBackend) -> Self {
        Self { backend, offset_x: 0, offset_y: 0, offset_stack: Vec::new() }
    }
    /// Returns the wrapped backend, for operations not exposed by the context.
    ///
    /// Commands issued directly on the backend bypass this context's
    /// translation offset and must be offset by the caller if required.
    pub fn backend(&mut self) -> &mut dyn PaintBackend {
        self.backend
    }
    /// Returns the target surface size in physical pixels.
    pub fn size(&self) -> Size {
        self.backend.size()
    }
    /// Returns the backend's logical DPI scale; `1.0` means unscaled.
    pub fn dpi_scale(&self) -> f32 {
        self.backend.dpi_scale()
    }
    /// Returns the currently active translation offset `(dx, dy)`.
    pub fn offset(&self) -> (i32, i32) {
        (self.offset_x, self.offset_y)
    }
    /// Pushes a translation offset that is applied to every draw primitive
    /// (including nested clips) until the matching [`RenderContext::pop_offset`]
    /// restores the previous offset. Offsets may be nested.
    ///
    /// Used by viewport containers such as `ScrollArea` to translate child
    /// content by the negative scroll position.
    pub fn push_offset(&mut self, dx: i32, dy: i32) {
        self.offset_stack.push((self.offset_x, self.offset_y));
        self.offset_x += dx;
        self.offset_y += dy;
    }
    /// Restores the translation offset in effect before the matching
    /// [`RenderContext::push_offset`]. No-op when no offset is pushed.
    pub fn pop_offset(&mut self) {
        if let Some(previous) = self.offset_stack.pop() {
            self.offset_x = previous.0;
            self.offset_y = previous.1;
        }
    }
    fn offset_rect(&self, rect: Rect) -> Rect {
        let mut translated = rect;
        translated.x += self.offset_x;
        translated.y += self.offset_y;
        translated
    }
    fn offset_point(&self, point: Point) -> Point {
        Point::new(point.x + self.offset_x, point.y + self.offset_y)
    }
    fn offset_points(&self, points: &[Point]) -> Vec<Point> {
        points.iter().map(|point| self.offset_point(*point)).collect()
    }
    /// Fills `rect` with a solid `color`, ignoring any alpha blending
    /// limitations of the backend's fill primitive.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::FillRect { rect, color });
    }

    /// Fills `rect` with a `gradient`.
    ///
    /// # Why this is a separate call rather than a flag on `fill_rect`
    ///
    /// A gradient is not "a colour with an extra" — it is a different paint server, and the two
    /// backends route it differently (`SoftwareSurface::fill_rect_gradient` versus the SVG backend's
    /// `<linearGradient>`/`<radialGradient>`/conic fan). Keeping it a distinct call is what lets the
    /// two stay honest about what they can express, and lets a caller fall back to a solid fill when
    /// a gradient is absent.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn fill_gradient(&mut self, rect: Rect, gradient: &crate::style::Gradient) {
        let rect = self.offset_rect(rect);
        self.backend
            .execute_command(&RenderCommand::DrawGradient { rect, gradient: gradient.clone() });
    }

    /// Paints a control's background: the `gradient` when the style declares one, otherwise the
    /// solid `color`.
    ///
    /// # Why this exists
    ///
    /// `WidgetStyle::background_gradient` was settable, mergeable and CSS-expressible, but **no
    /// painter ever read it**: a caller that set a gradient got a solid `background_color` (or the
    /// control's own default) and the gradient was silently dropped. This is the one-line call that
    /// closes that gap, so a control draws the gradient it was given and falls back to the colour it
    /// always drew. The identity case is `fill_rect`: a style with no gradient emits exactly what it
    /// did before.
    pub fn fill_background(
        &mut self,
        rect: Rect,
        color: Color,
        gradient: Option<&crate::style::Gradient>,
    ) {
        match gradient {
            Some(gradient) => self.fill_gradient(rect, gradient),
            None => self.fill_rect(rect, color),
        }
    }
    /// Draws a one-pixel-wide outline of `rect` (alias-aliased) in `color`.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::DrawRect { rect, color });
    }
    /// Draws an outline of `rect` in `color` with the given stroke `width` in
    /// pixels.
    ///
    /// `rect` is translated by the current offset before drawing.
    pub fn draw_rect_stroke(&mut self, rect: Rect, color: Color, width: u32) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::DrawRectStroke { rect, color, width });
    }
    /// Fills `rect` with `color` using rounded corners of `radius` pixels.
    ///
    /// The non-anti-aliased variant; use [`RenderContext::fill_rounded_rect_aa`]
    /// for smoother edges. `radius` is typically clamped by the backend to half
    /// the shorter side of `rect`.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::FillRoundedRect { rect, radius, color });
    }

    /// Paints a control's own **face**: its shadow, then its fill.
    ///
    /// # Why this exists beside `fill_rect`
    ///
    /// A control's face is not "a rectangle of colour". It is a rectangle whose *shape* and
    /// *depth* come from the theme: a `border_radius` the theme declares, a material (opaque or
    /// tinted glass), and an elevation whose shadow sits behind it. Writing
    /// `fill_rect(rect, bg)` states only the colour, so the other three were unreachable --
    /// measured, **88 controls** declared a radius the theme never gave them and **no control at
    /// all** ever drew the shadow its role resolved (BLUE24 §10A).
    ///
    /// This is the one-line replacement for that call. Its **identity case is `fill_rect`**: a
    /// solid face of radius 0 at elevation 0 emits exactly `FillRect`, so a control that has not
    /// been taught about surfaces draws byte-for-byte what it did before.
    ///
    /// # Why it takes the two resolved values rather than the style
    ///
    /// Because a call site has them and taking `&WidgetStyle` would not borrow-check at every one:
    /// some `draw` bodies hold the style by value, some by reference, and some only in an inner
    /// scope. `SurfaceStyle` and `u32` are both `Copy`, so passing them cannot conflict with a
    /// borrow the caller is still using -- which is what makes this a **mechanical** rewrite
    /// rather than one that has to reason about each control's borrows.
    ///
    /// The **outline is deliberately not drawn here**: a control's edge is often a semantic colour
    /// (a validation error, a checked state) that lives in the control rather than in the face, so
    /// a control keeps its own `draw_rect_stroke` call. [`crate::render::SurfaceStyle::paint`] is
    /// the fuller form for a control that wants the bevel and the edge derived too.
    ///
    /// `shadow_tint` is the shadow's hue; each elevation level owns its own alpha.
    ///
    /// This is [`Self::face_with_gradient`] with no gradient: it is the identity case for a face
    /// that declares a solid fill, so a control calling it emits exactly what it always did.
    pub fn face(
        &mut self,
        rect: Rect,
        fill: Color,
        surface: crate::render::SurfaceStyle,
        radius: u32,
        shadow_tint: Color,
    ) {
        self.face_with_gradient(rect, fill, None, surface, radius, shadow_tint);
    }

    /// [`Self::face`], but the fill is a `gradient` when the style declares one.
    ///
    /// # Why this exists beside `face`
    ///
    /// `WidgetStyle::background_gradient` was settable, mergeable and CSS-expressible, but only
    /// `Button` ever read it: every other control painted a solid `background_color` and silently
    /// dropped a declared gradient (issue §9 item #2). The gradient is a paint server rather than a
    /// colour, so it cannot be folded into `fill` (a [`Color`]); passing it as a separate
    /// `Option` keeps the identity exact — `None` emits precisely the solid face `face` always did,
    /// so a control that declares no gradient is unchanged.
    ///
    /// The shadow, the materialised fill and the radius are identical to `face`; only the fill step
    /// differs.
    ///
    /// # Why a rounded gradient is still a rectangular ramp
    ///
    /// A gradient is a paint server with no corner rounding, and the render command set has only a
    /// rectangular [`RenderContext::push_clip`] — there is no rounded clip to intersect the ramp
    /// with. So a gradient fill spans the full rectangle and the declared `radius` is honoured by
    /// the face's **outline** (which the control draws itself), exactly as
    /// [`SurfaceStyle::paint_with_gradient`](crate::render::SurfaceStyle::paint_with_gradient) does
    /// for a button. That keeps one gradient story in the crate rather than two.
    ///
    /// # Which controls route through here, and which do not
    ///
    /// Every control that paints its **own background face** calls this helper — the 100-plus call
    /// sites that used to call `face` (`label`, `frame`, the dialogs, the view widgets, `lineedit`,
    /// `textedit`, `listbox`, `combobox`, …). A handful of controls deliberately do **not**, and the
    /// rule is the same one principle #51 states: share only where the concept is actually shared.
    ///
    /// * `checkbox` / `radiobutton` paint only their small checked **indicator**, not a
    ///   control-sized background, so a `background_gradient` has no face to apply to.
    /// * `slider` / `progressbar` paint a centred **track band** inside their rectangle, not the
    ///   rectangle as a face; a widget-level gradient applied to a 4-pixel groove would be the wrong
    ///   shape rather than the right one.
    ///
    /// Those four keep their existing `fill_rect` / `fill_rounded_rect` calls. A future control
    /// whose background really is a face inherits gradient and radius by calling this helper, which
    /// is the point of the shared path.
    pub fn face_with_gradient(
        &mut self,
        rect: Rect,
        fill: Color,
        gradient: Option<&crate::style::Gradient>,
        surface: crate::render::SurfaceStyle,
        radius: u32,
        shadow_tint: Color,
    ) {
        // 1. The shadow, behind the face. `draws_shadow` is false at `Elevation::Flat`, so a face
        //    on the flat preset emits nothing here.
        if let Some(shadow) = surface.shadow(shadow_tint) {
            self.fill_rect(
                Rect::new(rect.x + shadow.x, rect.y + shadow.y, rect.width, rect.height),
                shadow.color,
            );
        }
        // 2. The fill. Without a gradient this is the solid, radius-clipped face `face` always
        //    drew; with one the ramp spans the full rectangle, matching `paint_with_gradient`.
        let materialised = surface.apply_fill(fill);
        match gradient {
            Some(gradient) => self.fill_gradient(rect, gradient),
            None => {
                if radius > 0 {
                    self.fill_rounded_rect(rect, radius, materialised);
                } else {
                    self.fill_rect(rect, materialised);
                }
            }
        }
    }
    /// Anti-aliased equivalent of [`RenderContext::fill_rounded_rect`].
    ///
    /// Costs more per call: the corners are sampled on the surface's
    /// configured `aa_samples_per_axis` grid.
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::FillRoundedRectAA { rect, radius, color });
    }
    /// Strokes the rounded-rectangle outline of `rect` in `color` with the
    /// given `width` in pixels.
    ///
    /// The non-anti-aliased variant; use
    /// [`RenderContext::draw_rounded_rect_stroke_aa`] for smoother edges.
    pub fn draw_rounded_rect_stroke(&mut self, rect: Rect, radius: u32, color: Color, width: u32) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::DrawRoundedRectStroke {
            rect,
            radius,
            color,
            width,
        });
    }
    /// Anti-aliased equivalent of
    /// [`RenderContext::draw_rounded_rect_stroke`].
    pub fn draw_rounded_rect_stroke_aa(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    ) {
        let rect = self.offset_rect(rect);
        self.backend.execute_command(&RenderCommand::DrawRoundedRectStrokeAA {
            rect,
            radius,
            color,
            width,
        });
    }
    /// Draws a one-pixel-wide, non-anti-aliased line from `from` to `to` in
    /// `color`.
    ///
    /// Both endpoints are translated by the current offset.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.backend.execute_command(&RenderCommand::DrawLine { from, to, color });
    }
    /// Anti-aliased equivalent of [`RenderContext::draw_line`].
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.backend.execute_command(&RenderCommand::DrawLineAA { from, to, color });
    }
    /// Draws a line from `from` to `to` in `color` with the given `width` in
    /// pixels, without anti-aliasing.
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.backend.execute_command(&RenderCommand::DrawLineStroke { from, to, color, width });
    }
    /// Anti-aliased equivalent of [`RenderContext::draw_line_stroke`].
    pub fn draw_line_stroke_aa(&mut self, from: Point, to: Point, color: Color, width: u32) {
        let from = self.offset_point(from);
        let to = self.offset_point(to);
        self.backend.execute_command(&RenderCommand::DrawLineStrokeAA { from, to, color, width });
    }
    /// Fills a circle centred on `center` with `radius` pixels, without
    /// anti-aliasing.
    ///
    /// The centre is translated by the current offset.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.backend.execute_command(&RenderCommand::FillCircle { center, radius, color });
    }
    /// Anti-aliased equivalent of [`RenderContext::fill_circle`].
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.backend.execute_command(&RenderCommand::FillCircleAA { center, radius, color });
    }
    /// Draws the one-pixel outline of a circle centred on `center` with
    /// `radius` pixels, without anti-aliasing.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        let center = self.offset_point(center);
        self.backend.execute_command(&RenderCommand::DrawCircle { center, radius, color });
    }
    /// Draws a circular outline of `width` pixels centred on `center` with
    /// `radius` pixels, without anti-aliasing.
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        let center = self.offset_point(center);
        self.backend.execute_command(&RenderCommand::DrawCircleStroke {
            center,
            radius,
            color,
            width,
        });
    }
    /// Draw a polyline/polygon path from a list of points.
    ///
    /// When `filled` is true the path is closed implicitly and filled with
    /// `color`; otherwise it is stroked with `width` and closed only when
    /// `closed` is true. Supported by the software (scanline fill) and
    /// SVG backends.
    pub fn draw_path(
        &mut self,
        points: &[Point],
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        let points = self.offset_points(points);
        self.backend.execute_command(&RenderCommand::DrawPath {
            points,
            closed,
            color,
            filled,
            width,
        });
    }
    /// Draws `text` with its origin (baseline start, per `alignment`) at
    /// `origin`, using `font` and `color`.
    ///
    /// The origin is translated by the current offset. `alignment` selects how
    /// the text is positioned horizontally relative to `origin`.
    pub fn draw_text(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        let origin = self.offset_point(origin);
        self.backend.execute_command(&RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
            alignment,
        });
    }
    /// Returns the measured bounds of `text` in `font` under the backend's DPI
    /// scale.
    ///
    /// The result is not affected by the current translation offset, so it is
    /// an unscaled layout measurement and carries no `(x, y)` origin.
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.backend.measure_text(text, font)
    }

    /// The line box a single line of `font` occupies, centred in `band`.
    ///
    /// The convenience form of [`text_line`] for the common case, and the entry point a
    /// control should reach for instead of writing `band.y + band.height / 2` — that
    /// expression places the glyph box's *top* edge on the band's middle line, which draws
    /// the label half a line too low (see [`VerticalAlignment`]).
    ///
    /// Use [`Self::text_line_aligned`] when the line belongs at the top or bottom of the
    /// band rather than centred.
    pub fn text_line(&self, band: Rect, font: &Font) -> Rect {
        text_line(band, font, VerticalAlignment::Center, self)
    }

    /// As [`Self::text_line`], with an explicit [`VerticalAlignment`].
    pub fn text_line_aligned(&self, band: Rect, font: &Font, alignment: VerticalAlignment) -> Rect {
        text_line(band, font, alignment, self)
    }

    /// Draws `text` fitted inside `bounds`, truncating with an ellipsis when it is wider.
    ///
    /// # Why this exists as a context method
    ///
    /// `draw_text` places a string and never asks whether it fits. A glyph's advance is
    /// `font.size()` pixels per cluster, so a 14 px font advances 14 px per character: a
    /// label longer than its control simply kept going, and the trailing glyphs landed
    /// outside the control's own rectangle. The software backend clips at the canvas edge,
    /// so the overflow was invisible there; the SVG backend emits absolute coordinates, so
    /// the same label appeared to run out of the picture. Around forty controls drew a
    /// placeholder, a file name or a status line wider than themselves.
    ///
    /// Naming the *rectangle* rather than a maximum width is the point: every call site
    /// already has the control's (or a cell's) rectangle in hand, and each was re-deriving
    /// "how much room is left" — or forgetting to. Passing the box makes the bound
    /// impossible to leave out by accident.
    ///
    /// `alignment` positions the *fitted* string, so a centred label stays centred inside
    /// `bounds` after truncation rather than drifting. The returned string is what was
    /// drawn, which lets a caller reserve the width it actually occupies.
    pub fn draw_text_fitted(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> crate::compat::String {
        let inset = TEXT_FIT_MARGIN as i32;
        // Both ends are inset, so the width a string may occupy is the box less one inset at
        // each end. The old form subtracted a single inset and then placed the result in the
        // full box, which is what shifted every centred label left; see `fitted_origin`.
        let available = (bounds.width as i32 - 2 * inset).max(0) as f32;
        let fitted = fit_text_to_width(text, available, font, self.backend);
        let advance = self.measure_text(&fitted, font).width as i32;
        let origin = fitted_origin(bounds, advance, alignment);
        self.draw_text(origin, &fitted, font, color, HorizontalAlignment::Left);
        fitted
    }

    /// Draws a single line of `text` fitted inside `bounds` and centred vertically in it.
    ///
    /// The vertical half of what [`Self::draw_text_fitted`] deliberately does not do. That
    /// method's contract is *fit horizontally, align horizontally* — its origin is
    /// `bounds.y` unchanged — so a caller that handed it a padded content box (a dialog's
    /// button row, a keyboard's key cap, a popover's empty state) got a label pinned to the
    /// box's top edge. Around twenty call sites read it as though it centred both ways; this
    /// entry point is the one that does, and naming it separately keeps the fit/align
    /// contract of the original intact for the callers that rely on it.
    ///
    /// `baseline.y` is ignored; the line box is derived from `bounds`. The horizontal half
    /// behaves exactly as in [`Self::draw_text_fitted`], including the fit inset and the
    /// truncation ellipsis.
    pub fn draw_text_line(
        &mut self,
        bounds: Rect,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> crate::compat::String {
        let line = self.text_line(bounds, font);
        self.draw_text_fitted(line, text, font, color, alignment)
    }
    /// Splits `text` into visual clusters (grapheme-like units) with per-cluster
    /// advances, as used for hit testing and caret placement.
    ///
    /// Unaffected by the current translation offset.
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        self.backend.shape_text(text, font)
    }
    /// Pushes a clip rectangle onto the backend's clip stack, translated by the
    /// current offset.
    ///
    /// Subsequent draws are restricted to this rectangle intersected with any
    /// enclosing clip. Every push must be matched by a
    /// [`RenderContext::pop_clip`]; the offsets pushed via
    /// [`RenderContext::push_offset`] also apply to it.
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        self.backend.execute_command(&RenderCommand::PushClip { x, y, width, height });
    }
    /// Pops the innermost clip rectangle from the backend's clip stack.
    ///
    /// A pop with an empty stack is a no-op on the software backend; unbalanced
    /// pops would otherwise widen the clip unexpectedly.
    pub fn pop_clip(&mut self) {
        self.backend.execute_command(&RenderCommand::PopClip);
    }

    /// Draws `data` as an image at `(x, y)` scaled to `width` x `height` pixels.
    ///
    /// `data` is interpreted as 8-bit RGBA, four bytes per pixel, tightly
    /// packed, and is expected to hold at least `width * height * 4` bytes;
    /// a shorter slice is cropped by the backend. The position is translated by
    /// the current offset.
    ///
    /// # Borrowed, not copied (D09-RENDER-02)
    ///
    /// This routes through [`PaintBackend::draw_image`], which hands the pixels over by
    /// reference. A backend that rasterises the image immediately — the software backend does —
    /// therefore draws straight from `data` with no copy. A backend that retains the command
    /// still gets an owning copy from that method's default implementation, so the caller's
    /// slice never needs to outlive the call on any backend. The earlier form always built an
    /// owning `RenderCommand::DrawImage` (a `to_vec()`), which cost one full-image allocation and
    /// copy per draw even when nothing retained it.
    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        self.backend.draw_image(x, y, width, height, data);
    }

    /// Execute an arbitrary render command directly.
    ///
    /// Useful for widgets like `Canvas` that store a command buffer
    /// and replay it during the draw pass.
    pub fn execute_command(&mut self, command: RenderCommand) {
        self.backend.execute_command(&command);
    }
}

#[cfg(test)]
mod tests {
    use super::super::paint::take_image_bytes_drawn_without_copy;
    use super::*;
    use crate::compat::{lock, MutexGuard};
    // `String`/`ToString` are not in scope under the `mini` profile (no std
    // prelude here), so the reference implementation in the fit test must import
    // the crate's own aliases explicitly.
    use crate::compat::{MiniToString, String};
    use crate::core::{Color, Font, Point, Rect, Size};
    use crate::render::SoftwarePaintBackend;
    use crate::render::{PaintBackend, RenderCommand};

    struct SoftwareRenderConfigTestGuard {
        _lock: MutexGuard<'static, ()>,
        original: SoftwareRenderConfig,
    }

    impl SoftwareRenderConfigTestGuard {
        fn new() -> Self {
            let lock = lock(software_render_config_test_lock());
            let original = default_software_render_config();
            Self { _lock: lock, original }
        }
    }

    impl Drop for SoftwareRenderConfigTestGuard {
        fn drop(&mut self) {
            set_default_software_render_config(self.original);
        }
    }

    // ── BackBuffer ──────────────────────────────────────────────────────

    #[test]
    fn back_buffer_new_creates_buffers() {
        let bb = BackBuffer::new(Size::new(10, 10), 1.0);
        assert_eq!(bb.size(), Size::new(10, 10));
        assert!((bb.dpi_scale() - 1.0).abs() < 1e-6);
        assert_eq!(bb.front().len(), 10 * 10 * 4);
        assert_eq!(bb.back.len(), 10 * 10 * 4);
    }

    #[test]
    fn back_buffer_zero_size() {
        let bb = BackBuffer::new(Size::new(0, 0), 1.0);
        assert_eq!(bb.size(), Size::new(0, 0));
        assert!(bb.front().is_empty());
        assert!(bb.back.is_empty());
    }

    #[test]
    fn back_buffer_resize() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.resize(Size::new(20, 20));
        assert_eq!(bb.size(), Size::new(20, 20));
        assert_eq!(bb.front().len(), 20 * 20 * 4);
        assert_eq!(bb.back.len(), 20 * 20 * 4);
    }

    #[test]
    fn back_buffer_resize_to_zero() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.resize(Size::new(0, 0));
        assert_eq!(bb.size(), Size::new(0, 0));
        assert!(bb.front().is_empty());
        assert!(bb.back.is_empty());
    }

    #[test]
    fn back_buffer_set_dpi_scale() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.set_dpi_scale(2.5);
        assert!((bb.dpi_scale() - 2.5).abs() < 1e-6);
    }

    #[test]
    fn back_buffer_set_dpi_scale_clamps_minimum() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.set_dpi_scale(0.0);
        assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn back_buffer_back_mut_returns_writable_slice() {
        let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
        let back = bb.back_mut();
        assert_eq!(back.len(), 5 * 5 * 4);
        back[0] = 255;
        assert_eq!(bb.back[0], 255);
    }

    #[test]
    fn back_buffer_present_swaps_front_and_back() {
        let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
        // Write to back
        bb.back_mut()[0] = 42;
        assert_ne!(bb.front()[0], 42);

        bb.present();

        // After present, back is now front
        assert_eq!(bb.front()[0], 42);
        // The old front (zeros) is now back
        assert_eq!(bb.back[0], 0);
    }

    #[test]
    fn back_buffer_present_swaps_again() {
        let mut bb = BackBuffer::new(Size::new(2, 2), 1.0);
        bb.back_mut()[0] = 10;
        bb.present();
        bb.back_mut()[0] = 20;
        bb.present();

        // Second present should show 20 on front
        assert_eq!(bb.front()[0], 20);
    }

    #[test]
    fn back_buffer_dpi_scale_initial_clamp() {
        let bb = BackBuffer::new(Size::new(1, 1), 0.0);
        assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
    }

    // ── SoftwareSurface ─────────────────────────────────────────────────

    #[test]
    fn software_surface_new_creates_surface() {
        let surface = SoftwareSurface::new(Size::new(100, 100), 1.0);
        assert_eq!(surface.size(), Size::new(100, 100));
        assert!((surface.dpi_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn software_surface_zero_size() {
        let surface = SoftwareSurface::new(Size::new(0, 0), 1.0);
        assert_eq!(surface.size(), Size::new(0, 0));
    }

    #[test]
    fn software_surface_begin_frame_clears() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::RED);
        surface.end_frame();

        let rgba = surface.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
            assert_eq!(chunk[1], 0);
            assert_eq!(chunk[2], 0);
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn software_surface_fill_rect() {
        let mut surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        surface.begin_frame(Color::WHITE);

        surface.fill_rect(Rect::new(2, 2, 10, 10), Color::BLUE);

        surface.end_frame();

        let rgba = surface.frame_rgba();
        let stride = 20 * 4;
        let idx = 5 * stride + 5 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 255); // B
    }

    /// `SetBlendMode` must actually composite on the software backend.
    ///
    /// The backend **stored** the mode and never read it, so a `SetBlendMode { Multiply }` frame
    /// rendered as plain source-over on the rasteriser while the SVG snapshot emitted a real
    /// `mix-blend-mode` group — one command, two pictures. This asserts the mode reaches the pixels.
    ///
    /// The expected value follows the W3C formula the SVG backend's `mix-blend-mode` also uses:
    /// a white backdrop (`1.0`) times a mid grey source (`0x80 / 255`) is `0x80` per channel.
    #[test]
    fn set_blend_mode_composites_on_the_software_backend() {
        use crate::render::RenderCommand;
        let mut surface = SoftwareSurface::new(Size::new(8, 8), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.set_blend_mode(BlendMode::Multiply);
        surface.fill_rect(Rect::new(0, 0, 4, 8), Color::rgb(128, 128, 128));
        surface.end_frame();

        let rgba = surface.frame_rgba();
        let blended = &rgba[..4];
        assert_eq!(
            (blended[0], blended[1], blended[2]),
            (128, 128, 128),
            "multiply against a white backdrop keeps the source colour"
        );

        // And against a non-white backdrop the two inputs genuinely multiply, which is the part a
        // stored-but-unread field could never produce.
        let mut dark = SoftwareSurface::new(Size::new(8, 8), 1.0);
        dark.begin_frame(Color::rgb(128, 128, 128));
        dark.set_blend_mode(BlendMode::Multiply);
        dark.fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(128, 128, 128));
        dark.end_frame();
        // 0.502 * 0.502 = 0.252 -> 64.
        assert_eq!(dark.frame_rgba()[0], 64, "two mid greys multiply to a quarter of full");

        // `Normal` restores plain source-over, and going back to it must undo the multiply.
        let mut restored = SoftwareSurface::new(Size::new(8, 8), 1.0);
        restored.begin_frame(Color::rgb(128, 128, 128));
        restored.set_blend_mode(BlendMode::Multiply);
        restored.set_blend_mode(BlendMode::Normal);
        restored.fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(128, 128, 128));
        restored.end_frame();
        assert_eq!(restored.frame_rgba()[0], 128, "Normal replaces the backdrop");

        // `SetBlendMode` is frame state and reaches the backend through the `SoftwarePaintBackend`
        // command, not only the direct setter, which is the path a real frame takes.
        use crate::render::PaintBackend;
        let mut via_command = crate::render::SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
        via_command.begin_frame(Color::rgb(128, 128, 128));
        via_command.execute_command(&RenderCommand::SetBlendMode { mode: BlendMode::Multiply });
        via_command.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 8, 8),
            color: Color::rgb(128, 128, 128),
        });
        via_command.end_frame();
        assert_eq!(
            via_command.surface().frame_rgba()[0],
            64,
            "the command path must set the mode too"
        );
    }

    #[test]
    fn software_surface_fill_rect_zero_size() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.fill_rect(Rect::new(0, 0, 0, 0), Color::RED);
        surface.end_frame();

        let rgba = surface.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255); // Still white, no fill
        }
    }

    #[test]
    fn software_surface_draw_rect() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::WHITE);

        surface.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);

        surface.end_frame();

        let rgba = surface.frame_rgba();
        // Top-left corner should be green (stroke edge)
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn software_surface_resize() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.resize(Size::new(50, 50));
        assert_eq!(surface.size(), Size::new(50, 50));
    }

    #[test]
    fn software_surface_set_dpi_scale() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.set_dpi_scale(1.5);
        assert!((surface.dpi_scale() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn software_surface_aa_samples_per_axis_default() {
        let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        assert_eq!(surface.aa_samples_per_axis(), 4);
    }

    #[test]
    fn software_surface_set_aa_samples() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.set_aa_samples_per_axis(2);
        assert_eq!(surface.aa_samples_per_axis(), 2);
    }

    #[test]
    fn software_surface_apply_render_config_clamps() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 99 });
        assert_eq!(surface.aa_samples_per_axis(), 8);
    }

    #[test]
    fn software_surface_render_config_roundtrip() {
        let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        let config = surface.render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    // ── RenderContext ───────────────────────────────────────────────────

    /// Builds a context over a throwaway surface, purely for measuring against.
    fn measurement_context<'a>(backend: &'a mut SoftwarePaintBackend) -> RenderContext<'a> {
        RenderContext::new(backend)
    }

    #[test]
    fn text_line_centres_the_line_box_in_the_band() {
        // The defect this primitive removes: `band.y + band.height / 2` places the glyph
        // box's TOP edge on the band's middle line, which draws the label half a line low.
        // Centring is `band.y + (band.height - line_height) / 2`.
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = measurement_context(&mut backend);
        let font = Font::default_ui();
        let line_height = ctx.measure_text("M", &font).height as i32;
        assert!(line_height > 0, "the measuring font must report a positive line height");

        let band = Rect::new(0, 0, 240, 120);
        let line = ctx.text_line(band, &font);
        assert_eq!(line.y, (120 - line_height) / 2, "the line box is centred, not top-dropped");
        assert_eq!(line.height, line_height as u32);
        assert_eq!((line.x, line.width), (band.x, band.width), "horizontally unchanged");

        // And it differs from the wrong expression, which is what makes the assertion
        // meaningful rather than a restatement of the implementation.
        assert_ne!(line.y, band.y + band.height as i32 / 2);
    }

    #[test]
    fn text_line_honours_each_vertical_alignment() {
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = measurement_context(&mut backend);
        let font = Font::default_ui();
        let line_height = ctx.measure_text("M", &font).height as i32;
        let band = Rect::new(0, 10, 240, 120);

        let top = ctx.text_line_aligned(band, &font, VerticalAlignment::Top);
        assert_eq!(top.y, 10, "top alignment pins the line box to the band's top edge");

        let bottom = ctx.text_line_aligned(band, &font, VerticalAlignment::Bottom);
        assert_eq!(bottom.y, 10 + 120 - line_height);

        let centre = ctx.text_line(band, &font);
        assert_eq!(centre.y, 10 + (120 - line_height) / 2);
        assert!(top.y < centre.y && centre.y < bottom.y, "the three orders must differ");
    }

    #[test]
    fn text_line_survives_degenerate_bands() {
        // A band that cannot hold a line, or has no extent at all, must not panic and must
        // not hand back a negative origin: a negative `y` would be a drawing instruction
        // outside the control, which is a defect rather than a graceful degradation.
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        let ctx = measurement_context(&mut backend);
        let font = Font::default_ui();

        for band in [
            Rect::new(0, 0, 0, 0),
            Rect::new(5, 5, 0, 0),
            Rect::new(0, 0, 100, 1),
            Rect::new(0, 120, 240, 0),
            Rect::new(-30, -30, 10, 4),
        ] {
            let line = ctx.text_line(band, &font);
            assert!(line.y >= band.y, "a clamped band never starts above itself: {band:?}");
            assert_eq!(line.x, band.x);
            assert_eq!(line.width, band.width);
            assert!(line.height >= 1, "a zero-height line box would be un-drawable");
        }
    }

    #[test]
    fn draw_text_line_places_ink_at_the_top_of_a_taller_band() {
        // The end-to-end reading of the contract: ink must land in the band's middle
        // third, not along its top edge. A 120 px band with a 14 px font leaves `line.y`
        // at 53, so row 54 is inside the glyph box and row 2 is not.
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        backend.begin_frame(Color::WHITE);
        let font = Font::default_ui();
        let band = Rect::new(0, 0, 240, 120);
        let line_y;
        {
            let mut ctx = RenderContext::new(&mut backend);
            line_y = ctx.text_line(band, &font).y;
            ctx.draw_text_line(band, "MMMM", &font, Color::BLACK, HorizontalAlignment::Left);
        }
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 240 * 4;
        let row_has_ink = |y: usize| {
            (0..240).any(|x| {
                let idx = y * stride + x * 4;
                rgba[idx] != 255 || rgba[idx + 1] != 255 || rgba[idx + 2] != 255
            })
        };
        assert!(row_has_ink(line_y as usize + 1), "the line box row carries ink");
        assert!(!row_has_ink(2), "the band's top edge must stay clear of a centred line");
    }

    #[test]
    fn render_context_new_wraps_backend() {
        let mut backend = SoftwarePaintBackend::new(Size::new(50, 50), 1.0);
        let ctx = RenderContext::new(&mut backend);
        assert_eq!(ctx.size(), Size::new(50, 50));
    }

    #[test]
    fn render_context_backend_accessor() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        let be = ctx.backend();
        assert_eq!(be.size(), Size::new(10, 10));
    }

    #[test]
    fn render_context_push_offset_translates_primitives() {
        let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
        backend.begin_frame(Color::WHITE);
        let offset_after;
        {
            let mut ctx = RenderContext::new(&mut backend);
            // Draw at (2, 2) then translate by (6, 8): fill must land at (8, 10).
            ctx.fill_rect(Rect::new(2, 2, 4, 4), Color::RED);
            ctx.push_offset(6, 8);
            ctx.fill_rect(Rect::new(2, 2, 4, 4), Color::BLUE);
            ctx.pop_offset();
            offset_after = ctx.offset();
        }
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 16 * 4;
        let pixel = |x: usize, y: usize| {
            let idx = y * stride + x * 4;
            (rgba[idx], rgba[idx + 1], rgba[idx + 2])
        };
        // (2,2) contains only the red fill.
        assert_eq!(pixel(3, 3), (255, 0, 0));
        // (8,10) region contains the translated blue fill.
        assert_eq!(pixel(9, 11), (0, 0, 255));
        // Offset is fully restored after pop.
        assert_eq!(offset_after, (0, 0));
    }

    #[test]
    fn render_context_offset_nesting_restores_previous() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_offset(1, 2);
        ctx.push_offset(3, 4);
        assert_eq!(ctx.offset(), (4, 6));
        ctx.pop_offset();
        assert_eq!(ctx.offset(), (1, 2));
        ctx.pop_offset();
        assert_eq!(ctx.offset(), (0, 0));
        // Popping an empty stack is a safe no-op.
        ctx.pop_offset();
        assert_eq!(ctx.offset(), (0, 0));
    }

    #[test]
    fn render_context_push_offset_translates_points_lines_and_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
        backend.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.push_offset(3, 4);
            // Line from (0,0) to (4,4) shifted to (3,4)-(7,8): pixel (5,6) is red.
            ctx.draw_line(Point::new(0, 0), Point::new(8, 8), Color::RED);
            ctx.pop_offset();
        }
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 16 * 4;
        let idx = 6 * stride + 5 * 4;
        assert_eq!(rgba[idx], 255); // R on the translated diagonal
    }

    #[test]
    fn render_context_fill_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        // Frame is still zero-initialized (not presented yet)
        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0);

        // After begin/end frame via the backend directly
        let mut backend2 = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend2.begin_frame(Color::WHITE);
        let mut ctx2 = RenderContext::new(&mut backend2);
        ctx2.fill_rect(Rect::new(0, 0, 5, 5), Color::BLUE);
        backend2.end_frame();

        let rgba = backend2.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[2], 255); // B
    }

    #[test]
    fn render_context_draw_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
    }

    #[test]
    fn render_context_draw_rect_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rect_stroke(Rect::new(0, 0, 10, 10), Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R on edge
    }

    #[test]
    fn render_context_fill_rounded_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rounded_rect(Rect::new(1, 1, 8, 8), 2, Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_fill_rounded_rect_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rounded_rect_aa(Rect::new(1, 1, 8, 8), 2, Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── The face path: gradient and radius ───────────────────────────────

    /// A declared gradient must change the pixels `face_with_gradient` paints. If the helper still
    /// routed to a solid fill, a `LEFT` pixel and a `RIGHT` pixel of a left-to-right black→white
    /// ramp would be identical; a gradient makes them differ.
    #[test]
    fn face_with_gradient_paints_a_ramp_not_a_solid() {
        use crate::core::Point;
        use crate::render::SurfaceStyle;
        use crate::style::Gradient;

        let rect = Rect::new(0, 0, 16, 4);
        let ramp = Gradient::linear(Point::new(0, 0), Point::new(16, 0))
            .add_stop(0.0, Color::BLACK)
            .add_stop(1.0, Color::WHITE);

        let mut backend = SoftwarePaintBackend::new(Size::new(16, 4), 1.0);
        backend.begin_frame(Color::rgb(1, 2, 3));
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.face_with_gradient(
                rect,
                Color::rgb(200, 0, 0),
                Some(&ramp),
                SurfaceStyle::solid(),
                0,
                Color::BLACK,
            );
        }
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 16 * 4;
        let left = rgba[2 * 4];
        let right = rgba[2 * stride + 14 * 4];
        assert!(
            (right as i32 - left as i32).abs() > 100,
            "a left-to-right ramp must differ across the face, got left={left} right={right}"
        );
    }

    /// `face` is documented as the identity of `face_with_gradient(None)`: a solid radius-0 face
    /// must emit exactly the same pixels either way, so teaching controls about gradients cannot
    /// have changed what a gradient-free control draws.
    #[test]
    fn face_is_the_identity_of_face_with_gradient_without_a_gradient() {
        use crate::render::SurfaceStyle;

        let rect = Rect::new(1, 1, 8, 8);
        let fill = Color::rgb(40, 90, 160);

        let mut plain = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        plain.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut plain);
            ctx.face(rect, fill, SurfaceStyle::solid(), 0, Color::BLACK);
        }
        plain.end_frame();

        let mut via_gradient = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        via_gradient.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut via_gradient);
            ctx.face_with_gradient(rect, fill, None, SurfaceStyle::solid(), 0, Color::BLACK);
        }
        via_gradient.end_frame();

        assert_eq!(
            plain.frame_rgba(),
            via_gradient.frame_rgba(),
            "face must be byte-for-byte identical to face_with_gradient(None)"
        );
    }

    /// The rounded-corner path: a radius must leave the corner un-painted by the face fill, so the
    /// backdrop shows through where a square face would have painted. This is the clipping half of
    /// the issue, exercised on the shared helper the controls call.
    #[test]
    fn face_radius_clips_the_corner_of_the_fill() {
        use crate::render::SurfaceStyle;

        let rect = Rect::new(0, 0, 12, 12);
        let fill = Color::rgb(10, 120, 220);

        let mut square = SoftwarePaintBackend::new(Size::new(12, 12), 1.0);
        square.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut square);
            ctx.face(rect, fill, SurfaceStyle::solid(), 0, Color::BLACK);
        }
        square.end_frame();

        let mut rounded = SoftwarePaintBackend::new(Size::new(12, 12), 1.0);
        rounded.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut rounded);
            ctx.face(rect, fill, SurfaceStyle::solid(), 5, Color::BLACK);
        }
        rounded.end_frame();

        // The very top-left pixel is inside a square face and outside a rounded one.
        let square_corner = &square.frame_rgba()[..4];
        let rounded_corner = &rounded.frame_rgba()[..4];
        assert_eq!(&square_corner[..3], &[10, 120, 220], "a square face fills its corner");
        assert_eq!(&rounded_corner[..3], &[255, 255, 255], "a radius must clip the corner");
    }

    #[test]
    fn render_context_draw_rounded_rect_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rounded_rect_stroke(Rect::new(1, 1, 8, 8), 2, Color::GREEN, 1);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_rounded_rect_stroke_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rounded_rect_stroke_aa(Rect::new(1, 1, 8, 8), 2, Color::BLUE, 1);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_line() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line(Point::new(0, 0), Point::new(9, 9), Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255);
    }

    #[test]
    fn render_context_draw_line_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_aa(Point::new(0, 0), Point::new(9, 9), Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_line_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_stroke(Point::new(0, 0), Point::new(9, 9), Color::GREEN, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[1], 255);
    }

    #[test]
    fn render_context_draw_line_stroke_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_stroke_aa(Point::new(0, 0), Point::new(9, 9), Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_fill_circle() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_circle(Point::new(10, 10), 5, Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn render_context_fill_circle_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_circle_aa(Point::new(10, 10), 5, Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255);
    }

    #[test]
    fn render_context_draw_circle() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_circle(Point::new(10, 10), 5, Color::GREEN);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_circle_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_circle_stroke(Point::new(10, 10), 5, Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        ctx.draw_text(Point::new(10, 20), "Hello", &font, Color::BLACK, HorizontalAlignment::Left);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_measure_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        let metrics = ctx.measure_text("Hello", &font);
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
    }

    #[test]
    fn render_context_shape_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        let shaped = ctx.shape_text("Hi", &font);
        assert!(!shaped.clusters.is_empty());
    }

    #[test]
    fn render_context_push_clip() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_clip(0, 0, 5, 5);
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        ctx.pop_clip();
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[idx], 255); // R inside clip region
    }

    #[test]
    fn render_context_clip_applies_to_non_rect_primitives() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_clip(0, 0, 8, 8);
        ctx.fill_circle(Point::new(6, 6), 6, Color::BLUE);
        ctx.draw_line(Point::new(0, 0), Point::new(19, 19), Color::RED);
        let image = (0..16).flat_map(|_| [0, 255, 0, 255]).collect::<Vec<_>>();
        ctx.draw_image(5, 5, 4, 4, &image);
        ctx.pop_clip();
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let pixel = |x: usize, y: usize| {
            let index = y * stride + x * 4;
            (rgba[index], rgba[index + 1], rgba[index + 2])
        };
        assert_eq!(pixel(6, 6), (0, 255, 0));
        assert_eq!(pixel(7, 7), (0, 255, 0));
        assert_eq!(pixel(15, 15), (255, 255, 255));
        assert_eq!(pixel(10, 10), (255, 255, 255));
    }

    #[test]
    fn render_context_pop_clip() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_clip(0, 0, 5, 5);
        ctx.pop_clip();
        // After pop, subsequent commands are not clipped
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 7 * stride + 7 * 4; // Outside original clip
        assert_eq!(rgba[idx], 255); // R should be visible
    }

    #[test]
    fn render_context_draw_image() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        let data = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        ctx.draw_image(0, 0, 2, 2, &data);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R from top-left pixel
    }

    #[test]
    fn render_context_size_and_dpi_scale() {
        let mut backend = SoftwarePaintBackend::new(Size::new(80, 60), 1.5);
        let ctx = RenderContext::new(&mut backend);
        assert_eq!(ctx.size(), Size::new(80, 60));
        assert!((ctx.dpi_scale() - 1.5).abs() < 1e-6);
    }

    // ── SoftwareRenderConfig ─────────────────────────────────────────────

    #[test]
    fn software_render_config_default_values() {
        let config = SoftwareRenderConfig::default();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn software_render_config_normalized_clamps_low() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 0 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 1);
    }

    #[test]
    fn software_render_config_normalized_clamps_high() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 100 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 8);
    }

    #[test]
    fn software_render_config_normalized_preserves_valid() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 2);
    }

    #[test]
    fn software_render_config_normalized_edge_cases() {
        let config_low = SoftwareRenderConfig { aa_samples_per_axis: 1 };
        assert_eq!(config_low.normalized().aa_samples_per_axis, 1);

        let config_high = SoftwareRenderConfig { aa_samples_per_axis: 8 };
        assert_eq!(config_high.normalized().aa_samples_per_axis, 8);
    }

    #[test]
    fn software_render_config_equality() {
        let a = SoftwareRenderConfig { aa_samples_per_axis: 4 };
        let b = SoftwareRenderConfig { aa_samples_per_axis: 4 };
        let c = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── Global config functions ──────────────────────────────────────────

    #[test]
    fn default_software_render_config_returns_default() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        set_default_software_render_config(SoftwareRenderConfig::default());
        let config = default_software_render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn set_default_software_render_config_updates_global() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        let custom = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        set_default_software_render_config(custom);
        let retrieved = default_software_render_config();
        assert_eq!(retrieved.aa_samples_per_axis, 2);
    }

    #[test]
    fn set_default_software_render_config_clamps() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        let custom = SoftwareRenderConfig { aa_samples_per_axis: 99 };
        set_default_software_render_config(custom);
        let retrieved = default_software_render_config();
        assert_eq!(retrieved.aa_samples_per_axis, 8);
    }

    // ── PaintBackend integration via RenderContext ───────────────────────

    #[test]
    fn render_context_dpi_scale_propagates() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 2.0);
        let ctx = RenderContext::new(&mut backend);
        assert!((ctx.dpi_scale() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn render_context_backend_mutability() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.fill_rect(Rect::new(0, 0, 5, 5), Color::RED);
        }
        // After ctx drops, backend can still be used
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── D09-RENDER-03: text fitting must not re-shape every prefix ──────────

    /// A [`PaintBackend`] that forwards to a real software backend while counting how many
    /// times `measure_text`/`shape_text` are called.
    ///
    /// # Why counting calls is the right observable
    ///
    /// The defect is an *algorithmic* one — the fitter re-shaped each candidate prefix, so the
    /// number of shaping calls grew with the square of the clusters that fit. Wall-clock timing
    /// on a shared CI machine is not a stable way to see that; the number of shaping calls is a
    /// direct, deterministic witness of the complexity, so the test asserts on it.
    struct CountingBackend<'a> {
        inner: &'a SoftwarePaintBackend,
        shapes: core::cell::Cell<usize>,
        measures: core::cell::Cell<usize>,
    }

    impl CountingBackend<'_> {
        fn shapes(&self) -> usize {
            self.shapes.get()
        }
    }

    impl PaintBackend for CountingBackend<'_> {
        fn begin_frame(&mut self, _clear: Color) {}
        fn end_frame(&mut self) {}
        fn execute_command(&mut self, _command: &RenderCommand) {}
        fn size(&self) -> Size {
            self.inner.size()
        }
        fn set_size(&mut self, _size: Size) {}
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, _dpi_scale: f32) {}
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            self.measures.set(self.measures.get() + 1);
            self.inner.measure_text(text, font)
        }
        fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
            self.shapes.set(self.shapes.get() + 1);
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
    }

    fn counting_backend() -> SoftwarePaintBackend {
        let mut backend = SoftwarePaintBackend::new(Size::new(400, 40), 1.0);
        backend.begin_frame(Color::WHITE);
        backend
    }

    /// `fit_text_to_width` must find the split point in a bounded number of shaping calls,
    /// independent of how many clusters fit — the old loop shaped once per accepted cluster.
    #[test]
    fn fitting_a_long_label_shapes_bounded_times() {
        let backend = counting_backend();
        let counter = CountingBackend {
            inner: &backend,
            shapes: core::cell::Cell::new(0),
            measures: core::cell::Cell::new(0),
        };
        let font = Font::default();

        // A label long enough that a narrow budget keeps many clusters. The width fits roughly
        // 10 ems of this font, so the old loop would have shaped ~10 times (and copied each
        // prefix); the new one shapes once.
        let long = "The quick brown fox jumps over the lazy dog repeatedly";
        let _fitted = fit_text_to_width(long, 60.0, &font, &counter);

        assert!(
            counter.shapes() <= 1,
            "fitting must shape the source at most once, but shaped {} times",
            counter.shapes()
        );
    }

    /// The number of shaping calls must not grow as the kept prefix grows: doubling the budget
    /// (which keeps roughly twice as many clusters) must not double the work.
    #[test]
    fn fitting_steps_do_not_grow_with_the_kept_prefix() {
        let backend = counting_backend();
        let font = Font::default();
        let long = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

        let measure = |max_width: f32| {
            let counter = CountingBackend {
                inner: &backend,
                shapes: core::cell::Cell::new(0),
                measures: core::cell::Cell::new(0),
            };
            let fitted = fit_text_to_width(long, max_width, &font, &counter);
            (counter.shapes(), fitted.chars().count())
        };

        let (narrow_shapes, narrow_len) = measure(40.0);
        let (wide_shapes, wide_len) = measure(200.0);

        assert!(wide_len > narrow_len, "the wider budget must keep more clusters");
        assert!(
            wide_shapes <= narrow_shapes && wide_shapes <= 1,
            "shaping calls must stay bounded as the kept prefix grows: narrow={narrow_shapes}, wide={wide_shapes}"
        );
    }

    /// The linear fitter must produce the same fitted string as a straightforward reference
    /// implementation, across Latin, CJK, combining characters and exact-boundary budgets.
    ///
    /// This is the pixel-equivalence half of the fix: a faster search that dropped or added a
    /// cluster would still return *a* string, so only comparing against the reference catches it.
    #[test]
    fn linear_fit_matches_a_prefix_walk_for_all_scripts() {
        let backend = counting_backend();
        let font = Font::default();

        // Byte-for-byte reference of the original per-prefix algorithm.
        fn reference(
            text: &str,
            max_width: f32,
            font: &Font,
            backend: &dyn PaintBackend,
        ) -> String {
            if text.is_empty() || max_width <= 0.0 {
                return String::new();
            }
            if backend.measure_text(text, font).width as f32 <= max_width {
                return text.to_string();
            }
            const ELLIPSIS: char = '\u{2026}';
            let ellipsis_width = backend.measure_text(&ELLIPSIS.to_string(), font).width as f32;
            if ellipsis_width > max_width {
                return ELLIPSIS.to_string();
            }
            let budget = max_width - ellipsis_width;
            let mut kept = String::new();
            for ch in text.chars() {
                let mut next = kept.clone();
                next.push(ch);
                if backend.measure_text(&next, font).width as f32 > budget {
                    break;
                }
                kept = next;
            }
            kept.push(ELLIPSIS);
            kept
        }

        let samples = [
            "The quick brown fox jumps over the lazy dog",
            "中文标签测试文本内容在这个控件里显示",
            "e\u{301}cole cafe\u{301} nai\u{308}ve re\u{301}sume\u{301}",
            "short",
            "oneverylongsinglewordwithoutanyspacesatall",
        ];
        // A range of budgets exercises the exact boundary, the ellipsis-only case and the fits
        // case. The step per sample is small so several boundaries are hit for each string.
        for sample in samples {
            let mut width = 0.0f32;
            while width <= 240.0 {
                let expected = reference(sample, width, &font, &backend);
                let actual = fit_text_to_width(sample, width, &font, &backend);
                assert_eq!(
                    actual, expected,
                    "fitted text differs from the prefix walk at width {width} for {sample:?}"
                );
                width += 3.0;
            }
        }
    }

    /// The ellipsis-only and no-fit boundaries are preserved by the linear path.
    #[test]
    fn fit_boundaries_match_the_prefix_walk() {
        let backend = counting_backend();
        let font = Font::default();
        let text = "hello world";

        // A budget that fits the whole string returns it unchanged.
        assert_eq!(fit_text_to_width(text, 1000.0, &font, &backend), text);

        // A budget too small for even the ellipsis yields just the ellipsis.
        assert_eq!(fit_text_to_width(text, 1.0, &font, &backend), "\u{2026}");

        // A non-positive budget yields an empty string, not an ellipsis.
        assert_eq!(fit_text_to_width(text, 0.0, &font, &backend), "");
    }

    // ── D09-RENDER-02: image draws must not copy the source on the immediate path ──

    /// Building a 2x2 image of four distinct opaque pixels, so a wrong stride or channel order
    /// shows up as a colour mismatch rather than as an all-same buffer.
    fn distinct_image() -> ([u8; 16], [(u8, u8, u8, u8); 4]) {
        let pixels =
            [(255u8, 0u8, 0u8, 255u8), (0, 255, 0, 255), (0, 0, 255, 255), (255, 255, 0, 255)];
        let mut data = [0u8; 16];
        for (index, px) in pixels.iter().enumerate() {
            data[index * 4] = px.0;
            data[index * 4 + 1] = px.1;
            data[index * 4 + 2] = px.2;
            data[index * 4 + 3] = px.3;
        }
        (data, pixels)
    }

    /// The borrowed draw must land the same pixels as the owning-command draw.
    #[test]
    fn borrowed_draw_image_matches_the_command_path_pixel_for_pixel() {
        let (data, pixels) = distinct_image();
        let size = Size::new(8, 8);

        // Reference: build an owning DrawImage command and execute it, exactly as the old
        // `RenderContext::draw_image` did.
        let mut reference = SoftwarePaintBackend::new(size, 1.0);
        reference.begin_frame(Color::BLACK);
        reference.execute_command(&RenderCommand::DrawImage {
            x: 2,
            y: 3,
            width: 2,
            height: 2,
            data: data.to_vec(),
        });
        reference.end_frame();
        let expected = reference.frame_rgba().to_vec();

        // Borrowed: the same draw through the context, which now takes the no-copy path.
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::BLACK);
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.draw_image(2, 3, 2, 2, &data);
        }
        backend.end_frame();

        assert_eq!(
            backend.frame_rgba(),
            expected.as_slice(),
            "borrowed and owning draws must agree"
        );

        // And the pixels really are the source's, at the requested offset.
        let stride = 8 * 4;
        for (index, px) in pixels.iter().enumerate() {
            let (x, y) = (2 + (index % 2), 3 + (index / 2));
            let at = y * stride + x * 4;
            let got = backend.frame_rgba();
            assert_eq!(
                (got[at], got[at + 1], got[at + 2], got[at + 3]),
                *px,
                "pixel {index} must come from the source at ({x}, {y})"
            );
        }
    }

    /// The immediate path must draw from the borrowed slice rather than building an owning
    /// command — this is the copy D09-RENDER-02 removes.
    #[test]
    fn the_immediate_path_draws_the_source_without_copying_it() {
        let (data, _) = distinct_image();
        let mut backend = SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
        backend.begin_frame(Color::BLACK);

        // Reset the counter, then draw through the context (the immediate path).
        let _ = take_image_bytes_drawn_without_copy();
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.draw_image(0, 0, 2, 2, &data);
        }
        let drawn = take_image_bytes_drawn_without_copy();

        assert_eq!(
            drawn,
            data.len(),
            "the software backend must draw the full source through the no-copy path"
        );
        backend.end_frame();
    }
}
