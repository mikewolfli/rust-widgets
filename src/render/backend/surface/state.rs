use super::*;

/// One active path clip, pushed by [`RenderCommand::ClipPath`].
///
/// A path clip is kept beside the rectangle clip stack because the two restrict drawing in
/// different ways: the rectangle stack decides *where a primitive may write*, while this mask
/// decides *how much of a written pixel survives*. The paint backend applies the mask when the
/// matching [`RenderCommand::PopClip`] arrives, so every primitive — geometry, glyphs, images and
/// blurs — is clipped by the path without any of them having to know it exists.
///
/// `rect` is the part of the path's bounding box that lies inside the surface and the enclosing
/// rectangle clip. `coverage` is one byte per pixel of that region (`width * height`), and
/// `backdrop` is the same region's RGBA bytes captured when the clip was pushed (`width * height
/// * 4`).
#[derive(Debug, Clone)]
pub(crate) struct PathClip {
    /// Device-space `(x, y, width, height)` the mask covers.
    pub(crate) rect: (i32, i32, u32, u32),
    /// Per-pixel path coverage, row-major, one byte per pixel.
    pub(crate) coverage: Vec<u8>,
    /// The pixels the region held before the clipped drawing, row-major RGBA.
    pub(crate) backdrop: Vec<u8>,
}

/// One captured backdrop layer, pushed by [`RenderCommand::PushLayer`].
///
/// The capture lives beside the pixel buffers because it is frame state of the surface, in
/// the same sense as the clip stack: it describes what the pixels currently mean, not what
/// they contain. `rect` is already clipped to the surface, so every consumer can index the
/// buffers without repeating the bounds checks.
#[derive(Debug, Clone)]
pub(crate) struct LayerCapture {
    /// Device-space `(x, y, width, height)` of the captured region.
    pub(crate) rect: (i32, i32, u32, u32),
    /// How the backdrop is processed at pop, or `None` when the layer has no effect.
    pub(crate) effect: Option<LayerEffect>,
    /// Backdrop pixels captured at push, row-major RGBA, `width * height * 4` bytes.
    pub(crate) backdrop: Vec<u8>,
    /// Whether the layer was isolated (backdrop captured, region cleared) at push.
    ///
    /// A plain [`RenderCommand::PushLayer`] with no effect draws straight into the surface, so its
    /// pop has nothing to composite. Opacity and a cache key both require the foreground to be
    /// separable from the backdrop, which is what `isolated` records.
    pub(crate) isolated: bool,
    /// Subtree opacity applied to the layer's foreground at pop, in `0.0..=1.0`.
    pub(crate) opacity: f32,
    /// Cache identity supplied by the caller, if any.
    pub(crate) cache_key: Option<u64>,
    /// Foreground pixels reused from a previous frame's cache entry.
    ///
    /// When this is `Some` the commands between push and pop were suppressed, because the caller
    /// asserted the subtree is unchanged, and the pop composites these pixels instead of whatever
    /// the (cleared) region contains. The buffer is shared with the cache entry (`Arc`), so a hit is
    /// a refcount bump rather than a copy of the whole region (G-01).
    pub(crate) cached_foreground: Option<Arc<Vec<u8>>>,
}

/// One cached layer foreground, keyed by the caller's cache identity.
#[derive(Debug, Clone)]
pub(crate) struct CachedLayer {
    /// Region the foreground covers.
    pub(crate) rect: (i32, i32, u32, u32),
    /// Opacity the foreground was captured with. A change invalidates the entry.
    pub(crate) opacity: f32,
    /// The layer's foreground pixels, row-major RGBA, `width * height * 4` bytes.
    ///
    /// Shared rather than owned: a cache hit hands the same pixels to the pop instead of copying
    /// the whole region, and the captured foreground is stored by refcount as well (G-01). `Arc`
    /// rather than `Rc` because `BackBuffer` is reachable from a `Sync` process-wide slot.
    pub(crate) foreground: Arc<Vec<u8>>,
}

/// Reusable row/column/copy buffers for one Gaussian-approximating box blur (G-07).
///
/// The three vectors grow to the largest region blurred and then stay allocated. A blur takes them
/// before running and hands them back afterwards, so after the first frame the per-call
/// allocations are gone. They live on [`BackBuffer`] for the same reason as the pixel buffers:
/// that is the per-frame storage constructed in one place.
#[derive(Debug, Clone, Default)]
pub(crate) struct BoxBlurScratch {
    /// Staged row for the horizontal pass, `region_width * 4` bytes.
    pub(crate) row: Vec<u8>,
    /// Staged column for the vertical pass, `region_height * 4` bytes.
    pub(crate) col: Vec<u8>,
    /// Untouched copy of the row/column being blurred.
    pub(crate) copy: Vec<u8>,
}

/// One active track matte, pushed by [`RenderCommand::PushMatte`].
///
/// The capture goes through three phases, which is why it holds both a backdrop and a mask:
/// `PushMatte` captures the backdrop and clears the region for the matte drawing; `PopMatte`
/// converts that drawing into `mask` and clears the region for the matted layer; `EndMatte`
/// multiplies the foreground by the mask and composites it over `backdrop`.
#[derive(Debug, Clone)]
pub(crate) struct MatteCapture {
    /// Device-space `(x, y, width, height)` of the matted region.
    pub(crate) rect: (i32, i32, u32, u32),
    /// Pixels the region held before the matte was pushed, row-major RGBA.
    pub(crate) backdrop: Vec<u8>,
    /// How the matte drawing becomes coverage.
    pub(crate) mode: crate::render::MatteMode,
    /// Per-pixel coverage (`0..=255`), one byte per pixel, filled by `PopMatte`.
    pub(crate) mask: Vec<u8>,
}
