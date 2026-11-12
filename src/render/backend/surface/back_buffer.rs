use super::*;

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
    pub(crate) size: Size,
    pub(crate) dpi_scale: f32,
    pub(crate) front: Vec<u8>,
    pub(crate) back: Vec<u8>,
    /// Open layer captures, innermost last.
    ///
    /// Kept on the buffer rather than on the surface because the buffers are the only
    /// per-frame storage the surface owns that is constructed in one place. A capture is
    /// invalidated by a resize (the region it indexes may no longer exist) and by a present
    /// (unbalanced pushes must not leak into the next frame), so both clear the stack.
    pub(crate) layers: Vec<LayerCapture>,
    /// Open track-matte captures, innermost last.
    pub(crate) mattes: Vec<MatteCapture>,
    /// Regions the caller reported as changed since the last present.
    ///
    /// This is the damage list, not a list of pixels that were actually written: the surface
    /// cannot know which primitives a frame will paint without charging every write, so the
    /// caller (a scene compositor that knows which controls were invalidated) reports the
    /// rectangles instead. [`BackBuffer::present_damage`] publishes only those rectangles and
    /// leaves the rest of the front buffer as the previous frame.
    pub(crate) damage: Vec<(i32, i32, u32, u32)>,
    /// Cached layer foregrounds, keyed by the caller's cache identity.
    ///
    /// This is the per-subtree replay cache: a control that knows its own drawing has not changed
    /// can pass the same key again and the surface skips the commands between its layer push and
    /// pop, reusing the foreground captured on the previous frame. Entries survive a present
    /// (that is the point of a cache) and are dropped on resize, when their regions no longer
    /// describe the buffer.
    pub(crate) layer_cache: Vec<(u64, CachedLayer)>,
    /// Physical slot of the oldest entry in [`BackBuffer::layer_cache`] once it is full.
    ///
    /// The cache is a ring once it reaches its capacity: a new entry overwrites the slot at this
    /// cursor instead of moving every entry down, and the cursor advances. While the cache is still
    /// growing the cursor is zero and the physical order is the use order (G-05).
    pub(crate) layer_cache_cursor: usize,
    /// Reusable buffer for one layer foreground capture, `width * height * 4` bytes (G-06).
    ///
    /// A layer pop captures at most once at a time (inner layers are popped first), so one slot is
    /// enough; it is taken before the capture and returned afterwards.
    pub(crate) layer_foreground_scratch: Vec<u8>,
    /// Reusable backdrop buffers for isolated layers and mattes (G-06).
    ///
    /// A pool rather than one slot because layers nest: the outer layer's backdrop is still live
    /// while the inner layer captures its own. The pool is bounded so a deep stack cannot pin an
    /// unbounded amount of pixel memory.
    pub(crate) layer_backdrop_pool: Vec<Vec<u8>>,
    /// Reusable blur row/column/copy buffers (G-07).
    pub(crate) blur_scratch: Option<BoxBlurScratch>,
    /// How many commands are currently being suppressed because their layer was a cache hit.
    ///
    /// A non-zero depth makes [`crate::render::backend::paint::route_command_to_surface`] discard
    /// drawing commands while still counting nested push/pop pairs, so the matching pop of the
    /// cached layer is found without letting the skipped content disturb the clip or layer stacks.
    pub(crate) suppress_depth: usize,
}
impl BackBuffer {
    /// Creates a new back buffer of `size` physical pixels at `dpi_scale`.
    ///
    /// Both front and back are allocated and zero-filled. `dpi_scale` is
    /// clamped to a minimum of `0.1`; values above `1.0` denote HiDPI scaling.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let bytes = pixel_bytes_len(size);
        Self {
            size,
            dpi_scale: dpi_scale.max(0.1),
            front: vec![0; bytes],
            back: vec![0; bytes],
            layers: Vec::new(),
            mattes: Vec::new(),
            damage: Vec::new(),
            layer_cache: Vec::new(),
            layer_cache_cursor: 0,
            layer_foreground_scratch: Vec::new(),
            layer_backdrop_pool: Vec::new(),
            blur_scratch: None,
            suppress_depth: 0,
        }
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
        self.layers.clear();
        self.mattes.clear();
        self.damage.clear();
        // A cached region indexes the old buffer's geometry, so it cannot survive a resize.
        self.layer_cache.clear();
        self.layer_cache_cursor = 0;
        // The scratch pools are geometry-independent, so they survive a resize. A blur buffer
        // sized for the old surface is still valid grow-only storage.
        self.suppress_depth = 0;
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
    /// Moves the most recently presented frame out of the buffer.
    ///
    /// # Why a move rather than a copy
    ///
    /// The render entry points own their backend and return the frame, so the final
    /// `frame_rgba().to_vec()` was a full `width * height * 4` copy on every painted
    /// frame — 4 MB at 1280×800, on the same thread that has to deliver the next input
    /// event. Taking the buffer gives the caller the same bytes with no copy. The
    /// surface is consumed by the caller immediately afterwards, so the emptied front
    /// buffer is never read or drawn into again.
    pub(crate) fn take_front(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.front)
    }
    /// Moves the presented frame out and restores a full-size front buffer.
    ///
    /// Unlike [`Self::take_front`], the buffer stays usable: the front slot is refilled so the
    /// next `begin_frame`/`end_frame` pair has both halves of the double buffer.
    ///
    /// # Why this reuses the backend rather than the bytes
    ///
    /// The returned frame leaves the crate, so one allocation per frame is unavoidable. What is
    /// avoidable is the *second* one: a fresh backend allocated and zero-filled both front and
    /// back (16.6 MB at 1080p). Reusing the backend across frames keeps the back buffer, so only
    /// the buffer that is handed to the caller is allocated anew.
    pub(crate) fn take_front_and_recycle(&mut self) -> Vec<u8> {
        let frame = core::mem::take(&mut self.front);
        let bytes = pixel_bytes_len(self.size);
        if self.front.len() != bytes {
            self.front = vec![0; bytes];
        }
        frame
    }
    /// Swaps the front and back buffers, publishing the drawn frame.
    ///
    /// After the swap the former back buffer is readable through
    /// [`BackBuffer::front`], while the back buffer holds the previous frame's
    /// pixels rather than a cleared surface; callers typically clear it at the
    /// start of the next frame.
    pub fn present(&mut self) {
        core::mem::swap(&mut self.front, &mut self.back);
        // A balanced frame pops every layer it pushes. Clearing here turns an unbalanced
        // push into a lost effect rather than a stale capture that would be applied to the
        // *next* frame's pixels.
        self.layers.clear();
        self.mattes.clear();
        self.damage.clear();
        self.suppress_depth = 0;
    }

    /// Reports `rect` as changed since the last present.
    ///
    /// The rectangle is clipped to the buffer here, so a caller may pass an untrusted or
    /// off-screen rectangle and still get a damage list that every consumer can index safely.
    /// Zero-area and fully off-screen rectangles are dropped.
    pub fn mark_damage(&mut self, rect: crate::core::Rect) {
        let Some(mut region) = clipped_damage(rect, self.size) else {
            return;
        };
        // Containment is the common case: a control repaints inside a region an ancestor already
        // reported. Dropping it keeps the list small without changing which pixels are published.
        if self.damage.iter().any(|existing| contains_region(*existing, region)) {
            return;
        }
        // Fold this report into the existing list. Present used to copy every rectangle in the
        // list individually, so a frame that reported many overlapping small regions paid one
        // row copy per rectangle for the same pixels (G-29). This is a single forward pass: when a
        // union absorbs an entry, `swap_remove` moves a not-yet-seen entry into the current slot
        // and the loop re-tests it against the expanded region instead of restarting from zero
        // (G-18). Earlier entries cannot become reachable later: the list is pairwise disjoint, so
        // a region that grows by `existing` can only touch entries that already intersect
        // `existing`, and those would have been merged before it was inserted.
        let mut index = 0;
        while index < self.damage.len() {
            let existing = self.damage[index];
            if contains_region(region, existing) {
                self.damage.swap_remove(index);
                continue;
            }
            if regions_intersect(region, existing) {
                region = union_region(region, existing);
                self.damage.swap_remove(index);
                continue;
            }
            index += 1;
        }
        self.damage.push(region);
        // An unbounded list would turn an input storm into an unbounded amount of present work.
        // Past the bound, one rectangle covering everything keeps every damaged pixel published
        // (the same trade `DirtyRegionTracker::add_coalescing_beyond_capacity` makes).
        if self.damage.len() > MAX_DAMAGE_REGIONS {
            let mut covering = self.damage[0];
            for region in &self.damage[1..] {
                covering = union_region(covering, *region);
            }
            self.damage.clear();
            self.damage.push(covering);
        }
    }

    /// The damaged rectangles reported since the last present, clipped to the buffer.
    pub fn damage_regions(&self) -> &[(i32, i32, u32, u32)] {
        &self.damage
    }

    /// Forgets the reported damage without publishing it.
    pub fn clear_damage(&mut self) {
        self.damage.clear();
    }

    /// Publishes only the damaged regions from the back buffer to the front buffer.
    ///
    /// Returns `false` when nothing was reported, leaving both buffers untouched. Otherwise each
    /// reported rectangle is copied row by row into the front buffer and the list is cleared. The
    /// front buffer keeps the previous frame's pixels everywhere else, which is exactly the
    /// contract: a caller that draws into the back buffer seeded from the previous frame and then
    /// reports what it touched gets a correct frame without a full-frame swap or copy.
    ///
    /// A damage list that covers the whole surface falls back to a swap: copying the same bytes
    /// region by region would do the same work with more overhead.
    pub fn present_damage(&mut self) -> bool {
        if self.damage.is_empty() {
            return false;
        }
        if self.damage.contains(&(0, 0, self.size.width, self.size.height)) {
            self.present();
            return true;
        }
        let width = self.size.width as usize;
        for (x, y, region_width, region_height) in self.damage.iter().copied() {
            let row_bytes = region_width as usize * 4;
            for row in 0..region_height as usize {
                let start = (y as usize + row) * width * 4 + x as usize * 4;
                let end = start + row_bytes;
                if end > self.back.len() || end > self.front.len() {
                    continue;
                }
                self.front[start..end].copy_from_slice(&self.back[start..end]);
            }
        }
        self.damage.clear();
        true
    }
}
