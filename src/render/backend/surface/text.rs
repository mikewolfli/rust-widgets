use super::*;

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

/// Breaks a polyline into alternating mark/gap stroke segments (G-014).
///
/// This is the one dash kernel both consumers share: the
/// [`RenderCommand::DrawPathDashed`] route in `route_command_to_surface` expands
/// the returned segments into `DrawLineStroke` commands, and
/// [`RenderContext::draw_edge_pattern`] walks its border edges through it, so the
/// two cannot drift apart on where a mark starts or ends.
///
/// # Semantics
///
/// The pattern restarts at **each vertex** — the same convention
/// `draw_edge_pattern` has always used per border edge, so the two agree on any
/// geometry they both see. `mark` runs are emitted first: a run is `[position,
/// position + mark)` along the edge, truncated at the edge's end, and the
/// position then skips the `gap`. A zero `mark` contributes no ink and returns
/// an empty list; a zero `gap` degenerates to the whole polyline in one segment
/// per run (a caller wanting a solid stroke should use `DrawPath`, but the
/// degenerate input is handled rather than mis-dashed).
///
/// Endpoints are interpolated with integer arithmetic — `from + delta * t /
/// length` with an integer `length` — matching the walk `draw_edge_pattern` has
/// always used, so for an axis-aligned edge the kernel emits byte-identical
/// segments to the loop it replaced. `closed` appends the closing run from the
/// last point back to the first, matching [`RenderCommand::DrawPath`]'s closing
/// convention.
/// The owned form of [`dash_line_segments_into`], kept for tests that compare the kernel's
/// output against the historical inline walk. Production call sites use the `_into`/visitor form,
/// so this wrapper is compiled only for tests (ENG-B1).
#[cfg(test)]
pub(crate) fn dash_line_segments(
    points: &[Point],
    closed: bool,
    dash: crate::render::core::command::DashPattern,
) -> Vec<(Point, Point)> {
    let mut segments = Vec::new();
    dash_line_segments_into(points, closed, dash, &mut segments);
    segments
}

thread_local! {
    /// Fitted (elided) labels reused by the paint path.
    ///
    /// A label that genuinely truncates has to be assembled once — that is the content of the
    /// ellipsised line — but it is usually the same label, at the same width, in the same face,
    /// on every frame. This cache keeps the assembled form so a steady frame borrows it instead
    /// of rebuilding it. The key is complete: the source text, the available width, the whole
    /// font and the DPI the fit was computed at. Bounded by [`FITTED_LABEL_CACHE_DEPTH`].
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FITTED_LABEL_CACHE: RefCell<Vec<FittedLabelEntry>> =
        const { RefCell::new(Vec::new()) };
    /// Dash segments reused by the per-frame dash call sites (ENG-B1).
    ///
    /// A dashed border expands four straight edges and a dashed path command expands once per
    /// frame; both used to allocate two `Vec`s per call. `try_borrow_mut` keeps a re-entrant call
    /// from panicking — the busy path below falls back to a local buffer.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DASH_SEGMENT_SCRATCH: RefCell<Vec<(Point, Point)>> =
        const { RefCell::new(Vec::new()) };
    /// Paths whose translated vertex slice was built recently.
    ///
    /// `RenderCommand::DrawPath` carries an `Arc<[Point]>`, so a polygon painted every frame
    /// allocated its vertices again on every frame even when nothing moved — an arrow, a chevron,
    /// a gauge polyline. The key is the complete geometry: the translation, the vertex contents
    /// and whether the path is closed. Colour/fill/width are command fields, not part of the
    /// shared slice, so a fill and its outline share one arc. Bounded by
    /// [`PATH_POINT_CACHE_DEPTH`].
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static PATH_POINT_CACHE: RefCell<Vec<PathPointCacheEntry>> =
        const { RefCell::new(Vec::new()) };
    /// Images whose owning copy was recorded into a damage bucket recently.
    ///
    /// A bucketed frame records [`RenderCommand::DrawImage`]/[`RenderCommand::DrawImageScaled`],
    /// and those commands carry the pixels by ownership — so the borrowed slice a widget draws
    /// from was copied in full on every frame. The copy is only needed once: an image's decoded
    /// pixels are the same allocation, with the same contents, on the next frame too. This keeps
    /// the `Arc` that was built so later frames share it, which is also what lets the GPU backend
    /// find the texture it already uploaded (RD2-01). Bounded by
    /// [`RECORDED_IMAGE_CACHE_DEPTH`].
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static RECORDED_IMAGE_CACHE: RefCell<Vec<RecordedImageCacheEntry>> =
        const { RefCell::new(Vec::new()) };
}

/// How many elided labels the process keeps at once.
///
/// A screen paints a bounded number of distinct truncated captions; sixty-four covers a dense
/// table while keeping the cache from becoming a label store.
pub(crate) const FITTED_LABEL_CACHE_DEPTH: usize = 64;

/// How many translated paths the process keeps at once.
///
/// A frame paints a handful of small static polygons (arrows, chevrons, gauge arcs); sixteen
/// covers them without the cache becoming an unbounded vertex store.
pub(crate) const PATH_POINT_CACHE_DEPTH: usize = 16;

/// One cached translated path: translation, original vertices, closed flag and shared slice.
pub(crate) type PathPointCacheEntry = (i32, i32, Vec<Point>, bool, Arc<[Point]>);

/// How many recorded images the process keeps at once.
///
/// The same bounded-FIFO shape — and the same size — as the crate's other content caches
/// (`GRADIENT_LUT_CACHE_LIMIT`, `COLOR_GLYPH_CACHE_LIMIT`): a screen shows a small set of images,
/// and a host that streams frames allocates one entry per frame while the oldest is dropped.
pub(crate) const RECORDED_IMAGE_CACHE_DEPTH: usize = 32;

/// One cached recorded image: the borrowed buffer's identity, its grid and the shared copy.
pub(crate) struct RecordedImageCacheEntry {
    /// Address of the caller's slice when it was recorded.
    pub(crate) ptr: usize,
    /// Its length in bytes.
    pub(crate) len: usize,
    /// The source grid the draw declares.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// 128-bit fingerprint of the bytes that were copied (see [`content_fingerprint`]).
    ///
    /// Replaced the whole-buffer `==` on every hit. That comparison was a `memcmp` per image per
    /// frame — a widget layer blit goes through this path, so a 1000x600 layer paid 2.4 MB of
    /// comparison every frame it was drawn from cache. The fingerprint is computed over the same
    /// bytes in one branch-free pass and compared as two words.
    pub(crate) fingerprint: ContentFingerprint,
    /// The bytes a recorded command owns, shared with every frame that records it again.
    pub(crate) data: Arc<[u8]>,
}

/// A 128-bit fingerprint of an image buffer's bytes.
///
/// Two independent 64-bit accumulators over 8-byte words: the cost is one pass with two
/// multiplies per word, and a false "same content" needs both words to collide, which is why a
/// probe that matches on `(ptr, len, width, height, fingerprint)` can be trusted. The previous
/// design compared the bytes themselves; the correctness argument it protected — a caller may
/// rewrite a `Vec` in place, and an allocator may reuse a freed address — still holds, because a
/// rewritten buffer changes the fingerprint.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContentFingerprint {
    pub(crate) a: u64,
    pub(crate) b: u64,
}

/// Computes the fingerprint of `data` without allocating.
pub(crate) fn content_fingerprint(data: &[u8]) -> ContentFingerprint {
    const OFFSET_A: u64 = 0xcbf2_9ce4_8422_2325;
    const OFFSET_B: u64 = 0x9e37_79b9_7f4a_7c15;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut a = OFFSET_A;
    let mut b = OFFSET_B;
    let (chunks, remainder) = data.as_chunks::<8>();
    for chunk in chunks {
        // `as_chunks` guarantees eight bytes per element, so the conversion is infallible.
        let word = u64::from_le_bytes(*chunk);
        a = (a ^ word).wrapping_mul(PRIME);
        b = (b ^ word.rotate_left(29) ^ a).wrapping_mul(PRIME);
    }
    let mut tail = 0u64;
    for (index, byte) in remainder.iter().enumerate() {
        tail |= u64::from(*byte) << (8 * index);
    }
    a = (a ^ tail ^ data.len() as u64).wrapping_mul(PRIME);
    b = (b ^ tail.rotate_left(17)).wrapping_mul(PRIME);
    ContentFingerprint { a, b }
}

/// Returns an `Arc` holding `data`, reusing the copy recorded for the same buffer last frame.
///
/// The address, length and grid narrow the lookup to one entry; the fingerprint then decides
/// whether the bytes are the same. Comparison is what makes a reused buffer safe: a caller may
/// rewrite a `Vec` in place — animated frames, a re-rendered canvas cache — and the allocator may
/// hand a freed address to a different image, so identity alone would serve stale pixels. Only a
/// miss allocates.
pub(crate) fn recorded_image(data: &[u8], width: u32, height: u32) -> Arc<[u8]> {
    let ptr = data.as_ptr() as usize;
    let len = data.len();
    RECORDED_IMAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let existing = cache.iter().position(|entry| {
            entry.ptr == ptr && entry.len == len && entry.width == width && entry.height == height
        });
        if let Some(index) = existing {
            let fingerprint = content_fingerprint(data);
            if cache[index].fingerprint == fingerprint {
                return Arc::clone(&cache[index].data);
            }
            // Same buffer identity, different pixels: replace the stale copy in place so the
            // entry keeps its position and the cache does not grow a duplicate per rewrite.
            let shared: Arc<[u8]> = Arc::from(data);
            cache[index].data = Arc::clone(&shared);
            cache[index].fingerprint = fingerprint;
            return shared;
        }
        let shared: Arc<[u8]> = Arc::from(data);
        if cache.len() >= RECORDED_IMAGE_CACHE_DEPTH {
            cache.remove(0);
        }
        cache.push(RecordedImageCacheEntry {
            ptr,
            len,
            width,
            height,
            fingerprint: content_fingerprint(data),
            data: Arc::clone(&shared),
        });
        shared
    })
}

/// One cached fitted label and the complete inputs it was built from.
pub(crate) struct FittedLabelEntry {
    pub(crate) source: String,
    pub(crate) available_bits: u32,
    pub(crate) font: Font,
    pub(crate) dpi_bits: u32,
    /// The assembled `prefix…` string.
    pub(crate) fitted: String,
    /// The advance the fitted string occupies, used to place it by alignment.
    pub(crate) advance: i32,
}

/// [`dash_line_segments`] writing into a caller-held buffer.
///
/// The buffer is cleared first; the walk itself is the historical one, so the segments are
/// byte-identical to the owned form.
pub(crate) fn dash_line_segments_into(
    points: &[Point],
    closed: bool,
    dash: crate::render::core::command::DashPattern,
    segments: &mut Vec<(Point, Point)>,
) {
    segments.clear();
    if points.len() < 2 || dash.mark == 0 {
        return;
    }
    let mark = i64::from(dash.mark);
    let gap = i64::from(dash.gap);
    // The last edge closes the loop back to the first point. Iterating indices instead of
    // materialising `runs` removes the second allocation without changing the order.
    let edges = if closed && points.len() >= 3 { points.len() } else { points.len() - 1 };
    for index in 0..edges {
        let from = points[index];
        let to = points[(index + 1) % points.len()];
        let dx = i64::from(to.x) - i64::from(from.x);
        let dy = i64::from(to.y) - i64::from(from.y);
        // The edge's length in path pixels. An integer square root keeps the walk
        // in integers: for an axis-aligned edge `isqrt(dx²) == |dx|` exactly, so
        // the interpolation below reproduces the historical walk bit for bit. The
        // square is widened to `i128`; opposite `i32` extremes overflow both the old
        // `i32` subtraction and the `i64` product.
        let dx_wide = i128::from(dx);
        let dy_wide = i128::from(dy);
        let length =
            ((dx_wide * dx_wide + dy_wide * dy_wide) as u128).isqrt().min(i64::MAX as u128) as i64;
        if length <= 0 {
            continue;
        }
        let point_at = |t: i64| {
            Point::new(from.x + ((dx * t) / length) as i32, from.y + ((dy * t) / length) as i32)
        };
        let mut position = 0i64;
        while position < length {
            let end = (position + mark).min(length);
            segments.push((point_at(position), point_at(end)));
            position = end + gap;
        }
    }
}

/// Calls `visit` for every dash segment of `points`, reusing a thread-local buffer (ENG-B1).
///
/// The busy case (a re-entrant dash expansion while the buffer is borrowed) uses a local buffer,
/// so the result is the same either way.
pub(crate) fn for_each_dash_segment(
    points: &[Point],
    closed: bool,
    dash: crate::render::core::command::DashPattern,
    mut visit: impl FnMut(Point, Point),
) {
    let mut done = false;
    let _ = DASH_SEGMENT_SCRATCH.try_with(|scratch| {
        let Ok(mut segments) = scratch.try_borrow_mut() else {
            return;
        };
        dash_line_segments_into(points, closed, dash, &mut segments);
        for &(from, to) in segments.iter() {
            visit(from, to);
        }
        done = true;
    });
    if !done {
        let mut segments = Vec::new();
        dash_line_segments_into(points, closed, dash, &mut segments);
        for &(from, to) in segments.iter() {
            visit(from, to);
        }
    }
}

/// Where a single line of text sits inside the band it is drawn in.
///
/// This is [`crate::core::VerticalAlignment`], re-exported under the path this module has
/// always used: the two types had an identical variant set (principle #54), so the copy here
/// was removed rather than kept in step by hand. The **vertical** type stays distinct from
/// [`HorizontalAlignment`] because the two axes have different owners — horizontal placement
/// is chosen per label, while vertical placement is a property of the band.
///
/// # The contract it encodes
///
/// `RenderContext`'s text origin is the glyph box's **top-left** (see
/// [`RenderContext::draw_text`]). So "centred" is not `band.y + band.height / 2`; it is
/// `band.y + (band.height - line_height) / 2`. The first form puts the box's *top edge* on
/// the band's middle line and draws the whole label half a line low — the single most
/// repeated placement error in this crate (76 sites across 40 files before this type
/// existed).
pub use crate::core::VerticalAlignment;

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
/// Test-only convenience: the fitted text without its measured width.
#[cfg(test)]
pub(crate) fn fit_text_to_width<'a>(
    text: &'a str,
    max_width: f32,
    font: &Font,
    backend: &dyn PaintBackend,
) -> Cow<'a, str> {
    fit_text_to_width_measured(text, max_width, font, backend).0
}

/// [`fit_text_to_width`] that also returns the fitted text's advance when it is already known.
///
/// Every control that draws a label asks for the fitted text and then measures it again to
/// centre it. In the common case — the label already fits — that second measurement repeats the
/// exactly same backend call the fit check just made. This variant hands the measured advance
/// back, so the caller does not pay for it twice. `None` means the truncation path built the
/// string itself and the caller still has to measure it.
pub(crate) fn fit_text_to_width_measured<'a>(
    text: &'a str,
    max_width: f32,
    font: &Font,
    backend: &dyn PaintBackend,
) -> (Cow<'a, str>, Option<f32>) {
    let parts = fit_text_parts(text, max_width, font, backend);
    if !parts.ellipsis {
        return (Cow::Borrowed(parts.prefix), parts.advance);
    }
    let mut kept = crate::compat::String::with_capacity(parts.prefix.len() + 3);
    kept.push_str(parts.prefix);
    kept.push('\u{2026}');
    (Cow::Owned(kept), parts.advance)
}

/// The fitted form of `text`, borrowed from the caller's string.
///
/// Splitting the answer into "the prefix that fits" and "is an ellipsis appended" is what lets a
/// painter draw the two runs without building the concatenated string. The `Cow` form above still
/// has to assemble one, because that is its contract; the paint path only needs the pixels.
pub(crate) struct FittedText<'a> {
    /// The prefix of `text` that is drawn.
    pub(crate) prefix: &'a str,
    /// Whether an ellipsis is drawn after `prefix`.
    pub(crate) ellipsis: bool,
    /// The advance of the drawn text when it is already known (the text fit, or it is a lone
    /// ellipsis); `None` when the caller must measure the prefix and add the ellipsis.
    pub(crate) advance: Option<f32>,
}

/// [`fit_text_to_width_measured`] without assembling the elided string.
pub(crate) fn fit_text_parts<'a>(
    text: &'a str,
    max_width: f32,
    font: &Font,
    backend: &dyn PaintBackend,
) -> FittedText<'a> {
    if text.is_empty() || max_width <= 0.0 {
        return FittedText { prefix: "", ellipsis: false, advance: Some(0.0) };
    }
    let measured = backend.measure_text(text, font).width as f32;
    if measured <= max_width {
        return FittedText { prefix: text, ellipsis: false, advance: Some(measured) };
    }
    // A single ellipsis stands in when the budget cannot hold any glyph at all: an empty
    // label would read as "this control has no text", which is a different statement.
    const ELLIPSIS_TEXT: &str = "\u{2026}";
    let ellipsis_width = backend.measure_text(ELLIPSIS_TEXT, font).width as f32;
    if ellipsis_width > max_width {
        return FittedText { prefix: "", ellipsis: true, advance: Some(ellipsis_width) };
    }
    let budget = max_width - ellipsis_width;

    // One shaping pass for the whole string. Its cluster advances are the renderer's own model.
    let shaped = backend.shape_text(text, font);
    let clusters = shaped.clusters();

    // # Why the order is checked before the linear pass (D09-RENDER-03)
    //
    // See the historical note above: a visual prefix is not a string prefix for a reordered line,
    // so that case keeps the per-prefix walk. The check itself is a prefix chain over the shaped
    // clusters and needs no rebuilt string.
    let shaped_matches_text = {
        let mut rest = text;
        let mut matches = true;
        for cluster in clusters {
            match rest.strip_prefix(&*cluster.text) {
                Some(tail) => rest = tail,
                None => {
                    matches = false;
                    break;
                }
            }
        }
        matches && rest.is_empty()
    };
    if !shaped_matches_text {
        let bytes = fit_text_prefix_bytes(text, budget, font, backend);
        return FittedText { prefix: &text[..bytes], ellipsis: true, advance: None };
    }

    // Tracking adds one inter-cluster gap per adjacent pair, so a run of `n` clusters carries
    // `n - 1` gaps; it is part of the shaped *total* but not of any single cluster's advance.
    let tracking = font.letter_spacing() * backend.dpi_scale();

    // A cluster's advance comes from shaping the **whole** string, so the running sum is a very
    // good *estimate* of where the split falls, not a proof of it. Only the prefixes near that
    // estimate are confirmed with the renderer's own `measure_text`.
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

    // True measure of a prefix of `length` clusters, matching `measure_text`'s contract. The
    // prefix is a **slice of `text`**, not a rebuilt string: the cluster-order check above just
    // proved the clusters concatenate to `text`.
    let prefix_bytes_at = |length: usize| -> usize {
        clusters[..length].iter().map(|cluster| cluster.text.len()).sum()
    };
    let prefix_fits = |bytes: usize| -> bool {
        if bytes == 0 {
            return true;
        }
        backend.measure_text(&text[..bytes], font).width as f32 <= budget
    };

    // Confirm the estimate by finding the **first** prefix that does not fit.
    const CONFIRM_SLACK: usize = 2;
    let search_from = estimated.saturating_sub(CONFIRM_SLACK);
    let mut prefix_bytes = prefix_bytes_at(search_from.min(clusters.len()));
    let mut confirmed = None;
    let mut length = search_from;
    while length <= clusters.len() {
        if !prefix_fits(prefix_bytes) {
            confirmed = Some(length - 1);
            break;
        }
        if let Some(cluster) = clusters.get(length) {
            prefix_bytes += cluster.text.len();
        }
        length += 1;
    }
    let confirmed = match confirmed {
        Some(value) => value,
        None if length > clusters.len() => clusters.len(),
        None => {
            let bytes = fit_text_prefix_bytes(text, budget, font, backend);
            return FittedText { prefix: &text[..bytes], ellipsis: true, advance: None };
        }
    };
    let accepted_bytes = prefix_bytes_at(confirmed);
    FittedText { prefix: &text[..accepted_bytes], ellipsis: true, advance: None }
}

/// The byte length of the longest character prefix of `text` that fits in `budget`.
///
/// The pre-D09-RENDER-03 walk, returning an index rather than an assembled string so the caller
/// can borrow the prefix it already has.
pub(crate) fn fit_text_prefix_bytes(
    text: &str,
    budget: f32,
    font: &Font,
    backend: &dyn PaintBackend,
) -> usize {
    let mut kept = 0usize;
    let mut candidate = crate::compat::String::new();
    for ch in text.chars() {
        candidate.clear();
        candidate.push_str(&text[..kept]);
        candidate.push(ch);
        if backend.measure_text(&candidate, font).width as f32 > budget {
            break;
        }
        kept += ch.len_utf8();
    }
    kept
}
