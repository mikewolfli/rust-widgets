// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// One cached window-tree frame: the size it was rendered at, its pixels, the content epoch it
/// describes, and the global-repaint generation it was painted under.
pub(crate) type CachedTreeFrame = (Size, Rc<Vec<u8>>, u64, u64);

thread_local! {
    /// The widget registry id for a widget's own `BaseWidget::id()`.
    ///
    /// # Two id spaces, one lookup
    ///
    /// `register` allocates a registry id and keys [`MOUNTED`] by it, while a widget's
    /// `BaseWidget::id()` comes from its own `Object` counter. Those numbers differ (a
    /// fresh control reports `1`; the registry hands out
    /// `0x5345_4C46_0000_0001`), so a call that only has the widget's own id —
    /// `request_redraw`, called from `&self` in ~1000 places — cannot address the
    /// registry without this map.
    ///
    /// Kept here rather than on the widget so it cannot go stale on a move, and so a
    /// widget that is dropped with the registry still holding it cannot be resurrected.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static OWN_IDS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// Per-widget repaint state, keyed by the same id as `MOUNTED`.
    ///
    /// A separate cell rather than a field on `Mounted` so the damage tracker can
    /// be mutated while a widget is borrowed mutably — the render path holds a
    /// `&mut dyn Widget` across the whole draw, and would otherwise deadlock.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static REPAINT: RefCell<HashMap<ObjectId, RepaintState>> = RefCell::new(HashMap::new());

    /// Scratch tracker for [`dirty_rects_into`].
    ///
    /// The buffer-reusing form promised to avoid allocation, but rebuilt the tracker on every
    /// call anyway (G-312). One tracker per thread is reused, cleared, and merged in place.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DIRTY_OUT: RefCell<crate::performance::DirtyRegionTracker> =
        RefCell::new(crate::performance::DirtyRegionTracker::new());

    /// The most recently rendered frame per widget, so an incremental repaint has
    /// something to carry forward.
    ///
    /// # Why this lives here rather than in each backend
    ///
    /// Every backend that draws a mounted widget needs the previous frame to seed a
    /// partial repaint, and all three would otherwise keep their own copy — three
    /// caches answering the same question, each free to forget to invalidate on a
    /// resize. Keeping it beside the damage tracker means the frame and the damage it
    /// describes cannot get out of step.
    ///
    /// # Why the frame is shared, not owned (D09-RENDER-04)
    ///
    /// The frame is held behind an [`Rc`] so handing it to an incremental repaint —
    /// or answering a frame that has no damage — is a pointer clone, not a `width * height * 4`
    /// copy. Before this, `render_frame_cached` cloned the whole buffer to obtain `previous`,
    /// then `render_frame_incremental` copied it again to return it, then the result was cloned
    /// once more into this cache: three full-frame copies for a frame that drew nothing.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAST_FRAME: RefCell<HashMap<ObjectId, (Size, Rc<Vec<u8>>)>> =
        RefCell::new(HashMap::new());

    /// The most recently rendered **window tree** frame per window, with the content epoch it
    /// was rendered at.
    ///
    /// A separate cell from [`LAST_FRAME`]: a window id and a mounted-surface id are different
    /// kinds of id, and a window is rendered by walking its children rather than by drawing one
    /// widget. Sharing the map would make the two entry points able to answer each other's
    /// lookups with a frame of the wrong shape.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAST_TREE_FRAME: RefCell<HashMap<ObjectId, CachedTreeFrame>> =
        RefCell::new(HashMap::new());

    /// Counts "something that affects a rendered frame changed" for this thread.
    ///
    /// # Why this no longer guards the window frame cache by itself (G-311)
    ///
    /// This used to be the only freshness token the tree-frame cache compared, which made any
    /// window's repaint invalidate every window's cached frame on the thread: a cursor blink in
    /// one window re-rasterised every other window on its next paint. The tree-frame cache now
    /// compares the **per-window** epoch in [`TREE_FRAME_EPOCHS`] instead; a change bumps only
    /// the window it belongs to (see [`note_renderable_change_for`]).
    ///
    /// This counter stays, unconditional as before, as the freshness token for the one cache
    /// that has no window to key on — the single-slot point-hit cache in `widget_at_point` —
    /// so that cache keeps exactly the invalidation behaviour it has always had.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static TREE_FRAME_EPOCH: core::cell::Cell<u64> = const { core::cell::Cell::new(1) };

    /// The frame-content epoch of each window that has a cached tree frame (G-311).
    ///
    /// Keyed by the window's id, because that is the key [`LAST_TREE_FRAME`] caches under: a
    /// repaint of window A bumps A's entry and leaves B's untouched, so B's next paint reuses
    /// its cached frame instead of re-walking and re-rasterising it. A window's entry is
    /// created when its frame is stored and removed when the frame is dropped
    /// (see [`forget_cached_frame`]), so the map never holds more than the cached windows.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static TREE_FRAME_EPOCHS: RefCell<HashMap<ObjectId, u64>> = RefCell::new(HashMap::new());
}

/// Records that a rendered frame's content may have changed. See `TREE_FRAME_EPOCH`.
pub(crate) fn note_renderable_change() {
    let _ = TREE_FRAME_EPOCH.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
}

/// Records that the window tree frame `id` belongs to may have changed (G-311).
///
/// Bumps the owning window's entry in [`TREE_FRAME_EPOCHS`] only, so the other windows' cached
/// frames stay valid. When the owner cannot be resolved — a widget lent out mid-tick, or a tree
/// whose root is not a window — every window that holds a cached frame is bumped, which is what
/// the old single global epoch did for every change; the conservative direction costs repaints,
/// never staleness.
pub(crate) fn note_renderable_change_for(id: ObjectId) {
    let owner = repaint_frame_owner(id);
    let _ = TREE_FRAME_EPOCHS.try_with(|epochs| {
        let mut epochs = epochs.borrow_mut();
        match owner {
            Some(window) => {
                *epochs.entry(window).or_insert(1) += 1;
            }
            None => {
                for epoch in epochs.values_mut() {
                    *epoch = epoch.wrapping_add(1);
                }
            }
        }
    });
}

/// The window whose cached tree frame a change to `id` invalidates.
///
/// This is [`top_level_window_of`]'s answer resolved with **shared** registry borrows only: the
/// G-106 fast path reads the mount's cached resolution when its parent still matches, and the
/// walk otherwise. It never takes the mutable borrow the G-106 cache *write* needs, so it is
/// safe to call from anywhere — including inside a `read_widget` callback, where
/// [`request_repaint_because`] and [`mark_dirty_rect`] can now arrive.
pub(crate) fn repaint_frame_owner(id: ObjectId) -> Option<ObjectId> {
    // A mounted surface has no parent in the widget tree, so the walk below cannot reach its
    // window; the mount path recorded that link instead.
    if let Some(window) = surface_window_for(id) {
        return Some(window);
    }
    let mounted = MOUNTED
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .map(|entry| (entry.widget.kind(), entry.widget.parent(), entry.top_window))
        })
        .ok()
        .flatten()?;
    let (kind, parent, cached) = mounted;
    if let Some((resolved_parent, window)) = cached {
        if resolved_parent == parent {
            return window;
        }
    }
    walk_top_level_window_from(id, kind, parent)
}

/// The current frame-content epoch of `window`'s cached tree frame. See [`TREE_FRAME_EPOCHS`].
pub(crate) fn renderable_epoch_for(window: ObjectId) -> u64 {
    TREE_FRAME_EPOCHS
        .try_with(|epochs| epochs.borrow().get(&window).copied())
        .ok()
        .flatten()
        .unwrap_or(1)
}

/// The current content epoch for this thread.
pub(crate) fn renderable_epoch() -> u64 {
    TREE_FRAME_EPOCH.try_with(|epoch| epoch.get()).unwrap_or(1)
}

thread_local! {
    /// Generation of the per-widget layer cache for this thread.
    ///
    /// # Why this is separate from `TREE_FRAME_EPOCH`
    ///
    /// The tree-frame epoch answers "does the window frame I cached still describe the tree?" —
    /// a whole-frame question, so any change must invalidate the whole frame. A per-widget layer
    /// answers a narrower question: "do these pixels describe *this* control's box?". Reusing
    /// the tree epoch for both meant one changed control threw away every control's layer, and a
    /// hover on one button re-shaped the text of the whole window.
    ///
    /// This epoch is therefore only bumped by causes no rectangle describes — a geometry change
    /// and a whole-control repaint. Rectangle damage is answered by dropping exactly the layers
    /// it touches (see [`invalidate_widget_layers_in`]).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAYER_EPOCH: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };

    /// Narrow damage submitted while the registry had lent a control out (a `tick`).
    ///
    /// A control's `tick` runs inside `with_widget_mut`, which takes its entry out of the
    /// registry map. Anything that has to read the registry in the same breath — the box a
    /// repaint is bounded by, the window above it — would see a missing widget, so those two
    /// lookups are deferred to [`flush_deferred_damage`], which runs after the sweep.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DEFERRED_DAMAGE: core::cell::RefCell<alloc::vec::Vec<(ObjectId, Rect, RepaintReason)>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };

    /// `ObjectId -> index in DEFERRED_DAMAGE`, so a second rectangle for the same control in one
    /// frame is merged in place in O(1) instead of scanning the queue (G-104). Maintained only
    /// by [`request_repaint_rect_deferred`] and cleared by [`flush_deferred_damage`], which is
    /// the only place the queue is emptied.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DEFERRED_DAMAGE_INDEX: RefCell<HashMap<ObjectId, usize>> = RefCell::new(HashMap::new());
}

/// The current per-widget layer epoch for this thread.
pub(crate) fn layer_epoch() -> u64 {
    LAYER_EPOCH.try_with(|epoch| epoch.get()).unwrap_or(0)
}

/// Drops every per-widget layer on this thread.
///
/// Used by a change that no rectangle describes: a moved or resized subtree (the parent's layer
/// rect can stay the same while its child shrinks inside it) and a whole-control repaint.
pub(crate) fn invalidate_widget_layers() {
    let _ = LAYER_EPOCH.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
}

/// Drops the per-widget layers whose pixels `rect` overlaps.
///
/// # Why a rectangle is the right granularity
///
/// A layer holds the pixels of one subtree inside its bounds. Damage at `rect` can only make a
/// layer stale when the two overlap, so everything else keeps serving its pixels. The rule is
/// deliberately conservative in one direction: a sibling whose bounds happen to overlap keeps
/// being invalidated too, which costs a repaint but can never show a stale pixel.
///
/// # Why it is not a whole-table operation (G-105)
///
/// `id` names the control that submitted the damage, so its own layer is dropped by key. The
/// overlap scan that follows is still needed — an **ancestor** layer contains this control's
/// pixels, which is why deleting only `id` would replay a stale parent — but it now runs only
/// after a cheap rejection against the union of every resident layer. A frame with no layers,
/// or damage nowhere near one, therefore pays neither the scan nor a map iteration.
pub(crate) fn invalidate_widget_layers_in(id: ObjectId, rect: Rect) {
    let _ = WIDGET_LAYERS.try_with(|layers| {
        let mut layers = layers.borrow_mut();
        if layers.is_empty() {
            return;
        }
        // The union is a conservative superset (see `WIDGET_LAYER_BOUNDS`), so a miss here is a
        // proof that no layer can overlap; a hit (or a missing union) falls through to the scan.
        if let Some(bounds) = WIDGET_LAYER_BOUNDS.with(|bounds| bounds.get()) {
            if !bounds.intersects(&rect) {
                return;
            }
        }
        // The submitting control is addressed directly; ancestors are caught by the scan below.
        layers.remove(&id);
        layers.retain(|_, layer| !layer.rect.intersects(&rect));
        // The eviction ring must keep exactly one entry per resident layer. Leaving the dropped
        // ids behind lets the ring outgrow the map; because the store path only evicts while the
        // *map* is full, those stale entries are then never popped and the ring grows without
        // bound (ENG-A1).
        retain_resident_layer_order(&layers);
    });
}

/// How many per-widget offscreen layers may be resident at once.
///
/// A layer is a full RGBA copy of one widget's painted box, so the bound is on the number of
/// boxes rather than on bytes: a small window full of tiny controls and a large window full of
/// small controls then have the same worst case. Eviction follows insertion order (G-105), so a
/// page that keeps repainting cannot pin the cache forever; the resident set is capped, which is
/// the property that matters here.
pub(crate) const MAX_CACHED_WIDGET_LAYERS: usize = 128;

/// One widget subtree rendered into its own transparent RGBA layer.
///
/// # Why the key carries more than the id
///
/// A layer's pixels describe one rectangle, drawn at one time, under one clip. An id alone would
/// let a moved, resized, differently clipped, or pre-theme-change control replay pixels that no
/// longer answer its box. Every field below is therefore part of the match: the id selects the
/// candidate, and the remaining fields decide whether it still describes this paint.
pub(crate) struct CachedWidgetLayer {
    /// Bounds of the layer in absolute (window) coordinates.
    pub(crate) rect: Rect,
    /// Clip the ancestors imposed when the layer was captured.
    pub(crate) clip: Option<Rect>,
    /// Origin of the frame the layer was captured for.
    pub(crate) frame_origin: (i32, i32),
    /// Size of the frame the layer was captured for.
    pub(crate) frame_size: Size,
    /// Tree/damage epoch at which the layer was captured.
    pub(crate) epoch: u64,
    /// Global repaint generation at which the layer was captured.
    pub(crate) global_epoch: u64,
    /// Layer pixels, row-major RGBA, `width * height * 4` bytes.
    pub(crate) pixels: Rc<Vec<u8>>,
    /// Monotonic stamp from the moment this layer was inserted.
    ///
    /// Eviction is a FIFO ring ordered by this stamp (G-105). It is deliberately separate from
    /// any "last used" notion: a lookup no longer rewrites the entry, so the ring entry that
    /// names a layer stays identifiable until that layer is replaced.
    pub(crate) insert_stamp: u64,
}

thread_local! {
    /// Per-widget offscreen layers, keyed by registry id.
    ///
    /// The existing [`LAST_TREE_FRAME`] cache answers "has the whole window changed?"; this map
    /// answers the finer question "which control changed?" so one dirty control no longer forces
    /// every clean sibling to run `draw` (and re-shape and re-rasterise its text) again.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static WIDGET_LAYERS: RefCell<HashMap<ObjectId, CachedWidgetLayer>> =
        RefCell::new(HashMap::new());

    /// Monotonic clock used to order [`WIDGET_LAYERS`] for eviction.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static WIDGET_LAYER_CLOCK: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };

    /// Insertion order for [`WIDGET_LAYERS`], so eviction pops the oldest entry instead of
    /// scanning the map for a minimum (G-105). Every entry names a resident layer: the map's
    /// removal paths drop the matching entries and a replacement drops its own old entry first
    /// (ENG-A1), so the ring length never exceeds the resident count.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static WIDGET_LAYER_ORDER: RefCell<alloc::collections::VecDeque<(ObjectId, u64)>> =
        RefCell::new(alloc::collections::VecDeque::new());

    /// Conservative union of every resident layer's rectangle.
    ///
    /// Used as a fast reject before the overlap scan in [`invalidate_widget_layers_in`]: if the
    /// damage does not intersect the union, it cannot intersect any layer. The union is never
    /// shrunk on a removal (only recomputed when the cache is emptied), so it can be larger than
    /// the true union — that costs at most an unnecessary scan, never a missed invalidation
    /// (G-105).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static WIDGET_LAYER_BOUNDS: core::cell::Cell<Option<Rect>> =
        const { core::cell::Cell::new(None) };
}

/// Drops the eviction-ring entries that no longer name a resident layer.
///
/// The ring is an ordering hint for insertion-order eviction, so every entry must name a layer
/// the map still holds. Without this the ring accumulates entries for layers that were replaced,
/// invalidated or forgotten; the store path only evicts while the map is full, so a bounded map
/// would no longer bound the ring (ENG-A1).
pub(crate) fn retain_resident_layer_order(layers: &HashMap<ObjectId, CachedWidgetLayer>) {
    let _ = WIDGET_LAYER_ORDER.try_with(|order| {
        order.borrow_mut().retain(|entry| layers.contains_key(&entry.0));
    });
}

/// Advances and returns the layer insertion stamp.
pub(crate) fn next_widget_layer_clock() -> u64 {
    WIDGET_LAYER_CLOCK.with(|clock| {
        let next = clock.get().wrapping_add(1);
        clock.set(next);
        next
    })
}

/// Looks up a layer that still describes this exact paint.
pub(crate) fn lookup_widget_layer(
    id: ObjectId,
    rect: Rect,
    clip: Option<Rect>,
    frame_origin: (i32, i32),
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
) -> Option<Rc<Vec<u8>>> {
    WIDGET_LAYERS
        .try_with(|layers| {
            let layers = layers.borrow();
            let layer = layers.get(&id)?;
            let matches = layer.rect == rect
                && layer.clip == clip
                && layer.frame_origin == frame_origin
                && layer.frame_size == frame_size
                && layer.epoch == epoch
                && layer.global_epoch == global_epoch;
            matches.then(|| Rc::clone(&layer.pixels))
        })
        .ok()
        .flatten()
}

/// Stores a freshly painted layer, evicting the oldest inserted one when full.
///
/// Eviction follows insertion order rather than scanning for a least-recently-used entry
/// (G-105): the cache holds at most [`MAX_CACHED_WIDGET_LAYERS`] entries, so the scan and the
/// ring differ by a constant, but the ring removes an O(N) map walk from every store past the
/// bound. Replacing an existing id does not evict anything: its previous ring entry is dropped
/// first, so the ring holds exactly one entry per resident layer (ENG-A1).
#[allow(clippy::too_many_arguments)]
pub(crate) fn store_widget_layer(
    id: ObjectId,
    rect: Rect,
    clip: Option<Rect>,
    frame_origin: (i32, i32),
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    pixels: Rc<Vec<u8>>,
) {
    let stamp = next_widget_layer_clock();
    let _ = WIDGET_LAYERS.try_with(|layers| {
        let mut layers = layers.borrow_mut();
        if layers.contains_key(&id) {
            // Replacing a resident layer must not append a second ring entry for it: a control
            // that repaints every frame would otherwise add one entry per frame while the map
            // stays at its bound, so the eviction loop never runs and the ring grows forever
            // (ENG-A1). Dropping the entry here keeps the ring in step with the map.
            WIDGET_LAYER_ORDER.with(|order| order.borrow_mut().retain(|entry| entry.0 != id));
        } else {
            while layers.len() >= MAX_CACHED_WIDGET_LAYERS {
                let evicted = WIDGET_LAYER_ORDER.with(|order| order.borrow_mut().pop_front());
                let Some((oldest, old_stamp)) = evicted else {
                    // The ring and the map disagree (only possible if a caller cleared one
                    // without the other); fall back to a single scan rather than growing past
                    // the bound.
                    let oldest = layers
                        .iter()
                        .min_by_key(|(_, layer)| layer.insert_stamp)
                        .map(|(id, _)| *id);
                    if let Some(oldest) = oldest {
                        if let Some(old) = layers.remove(&oldest) {
                            recycle_layer_pixels(old.pixels);
                        }
                    }
                    break;
                };
                if layers.get(&oldest).is_some_and(|layer| layer.insert_stamp == old_stamp) {
                    if let Some(old) = layers.remove(&oldest) {
                        recycle_layer_pixels(old.pixels);
                    }
                }
            }
        }
        if let Some(old) = layers.insert(
            id,
            CachedWidgetLayer {
                rect,
                clip,
                frame_origin,
                frame_size,
                epoch,
                global_epoch,
                pixels,
                insert_stamp: stamp,
            },
        ) {
            // The replaced entry's buffer returns to the pool instead of being freed, so the
            // next same-size miss reuses the allocation rather than making a new one (G-313).
            recycle_layer_pixels(old.pixels);
        }
        // Keep the fast-reject union a superset of every resident layer (G-105).
        WIDGET_LAYER_BOUNDS.with(|bounds| {
            let merged = match bounds.get() {
                Some(existing) => existing.union(&rect),
                None => rect,
            };
            bounds.set(Some(merged));
        });
        WIDGET_LAYER_ORDER.with(|order| order.borrow_mut().push_back((id, stamp)));
    });
}

thread_local! {
    /// Offscreen backends pooled for layer misses, each stored with the size it was built for
    /// (G-313).
    ///
    /// A layer-cache miss paints the subtree into its own transparent surface, which used to mean a
    /// fresh [`SoftwarePaintBackend`] — two zero-filled full-size buffers — on every miss. A pooled
    /// backend at the same size re-allocates nothing; a size change is the only reason to build a
    /// new one. Nested layer captures hold their backend for the duration of the recursion, so the
    /// pool is taken from and returned to like a stack (`take_paint_children`), not a single slot:
    /// several entries of the same size may coexist.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAYER_BACKEND_POOL: RefCell<Vec<(Size, SoftwarePaintBackend)>> = RefCell::new(Vec::new());

    /// Frame buffers recycled from dead layer-cache entries, so the copy-out of a reused backend
    /// also allocates nothing once a layer of that size has lived and died (G-313).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAYER_BUFFER_POOL: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
}

/// How many layer backends the pool retains; beyond it a backend is dropped, which is the
/// pre-pool behaviour for that paint.
pub(crate) const MAX_POOLED_LAYER_BACKENDS: usize = 4;

/// How many frame buffers the pool retains; see [`LAYER_BUFFER_POOL`].
pub(crate) const MAX_POOLED_LAYER_BUFFERS: usize = 8;

/// Takes a backend ready to paint a `layer_size` layer into: a pooled one when the size
/// matches, a fresh one only when the size changed (G-313).
pub(crate) fn take_layer_backend(layer_size: Size) -> SoftwarePaintBackend {
    LAYER_BACKEND_POOL.with(|pool| {
        let reused = {
            let mut pool = pool.borrow_mut();
            pool.iter()
                .position(|(size, _)| *size == layer_size)
                .map(|index| pool.swap_remove(index))
        };
        match reused {
            Some((_, mut backend)) => {
                // A reused surface must present the same starting state a fresh one does:
                // `Color::TRANSPARENT` fills every byte of the back buffer with zero, which is
                // byte-for-byte what the fresh construction's `vec![0; …]` held, and the
                // render-level layer-foreground cache is dropped so it cannot answer for
                // content captured under an earlier layer. `end_frame` clears the rest of the
                // per-frame surface state, and the paint path's push/pop pairs are balanced.
                backend.begin_frame(crate::core::Color::TRANSPARENT);
                backend.clear_layer_cache();
                backend
            }
            None => SoftwarePaintBackend::new(layer_size, 1.0),
        }
    })
}

/// Returns a layer's backend to the pool for the next miss at its size (G-313).
pub(crate) fn return_layer_backend(backend: SoftwarePaintBackend, layer_size: Size) {
    let _ = LAYER_BACKEND_POOL.try_with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_POOLED_LAYER_BACKENDS {
            pool.push((layer_size, backend));
        }
    });
}

/// Takes a frame buffer of exactly `len` bytes, preferring the smallest pooled one whose
/// capacity fits — the same rule the render-side scratch pools follow (G-313).
pub(crate) fn take_layer_buffer(len: usize) -> Vec<u8> {
    LAYER_BUFFER_POOL.with(|pool| {
        let reused = {
            let mut pool = pool.borrow_mut();
            pool.iter()
                .enumerate()
                .filter(|(_, buffer)| buffer.capacity() >= len)
                .min_by_key(|(_, buffer)| buffer.capacity())
                .map(|(index, _)| index)
                .map(|index| pool.swap_remove(index))
        };
        match reused {
            Some(mut buffer) => {
                buffer.clear();
                buffer.resize(len, 0);
                buffer
            }
            None => vec![0; len],
        }
    })
}

/// Returns a frame buffer to the pool, keeping its capacity for the next layer of its size.
pub(crate) fn return_layer_buffer(mut buffer: Vec<u8>) {
    buffer.clear();
    let _ = LAYER_BUFFER_POOL.try_with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_POOLED_LAYER_BUFFERS {
            pool.push(buffer);
        }
    });
}

/// Recycles a layer's pixel buffer when its cache entry dies (G-313): an `Rc` nobody else
/// holds gives up its `Vec`, which the next same-size miss reuses instead of allocating.
pub(crate) fn recycle_layer_pixels(pixels: Rc<Vec<u8>>) {
    if let Ok(buffer) = Rc::try_unwrap(pixels) {
        return_layer_buffer(buffer);
    }
}

thread_local! {
    /// Software backends retained between whole-frame paints (G-801).
    ///
    /// The three frame entry points used to build a fresh [`SoftwarePaintBackend`] per frame,
    /// which allocated and zero-filled both halves of its double buffer (16.6 MB at 1080p).
    /// The same size-keyed take/return discipline as [`LAYER_BACKEND_POOL`] keeps the back
    /// buffer across frames; only the buffer handed to the caller is allocated anew.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_BACKEND_POOL: RefCell<Vec<(Size, SoftwarePaintBackend)>> = RefCell::new(Vec::new());
}

/// How many whole-frame backends the pool retains; beyond it a backend is dropped, which is the
/// pre-pool behaviour for that frame.
pub(crate) const MAX_POOLED_FRAME_BACKENDS: usize = 4;

/// Takes a backend ready to paint a whole frame of `size`.
pub(crate) fn take_frame_backend(size: Size) -> SoftwarePaintBackend {
    FRAME_BACKEND_POOL.with(|pool| {
        let reused = {
            let mut pool = pool.borrow_mut();
            pool.iter()
                .position(|(pooled_size, _)| *pooled_size == size)
                .map(|index| pool.swap_remove(index))
        };
        match reused {
            Some((_, mut backend)) => {
                // A reused backend must not answer with layer state captured for different
                // content, and an unbalanced clip push from a previous paint must not survive.
                backend.clear_layer_cache();
                backend.surface_mut().clip_stack.clear();
                backend
            }
            None => SoftwarePaintBackend::new(size, 1.0),
        }
    })
}

/// Returns a whole-frame backend to the pool for the next frame at its size.
///
/// The caller has already moved the frame out with
/// [`SoftwarePaintBackend::take_frame_rgba`], so what is returned here is the buffer pair — the
/// allocation this pool exists to keep.
pub(crate) fn return_frame_backend(backend: SoftwarePaintBackend, size: Size) {
    let _ = FRAME_BACKEND_POOL.try_with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_POOLED_FRAME_BACKENDS {
            pool.push((size, backend));
        }
    });
}

// Test-only count of layer replays, so a regression test can prove a clean control returned its
// cached pixels instead of running `draw` (and therefore instead of shaping or rasterising text).
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static WIDGET_LAYER_REPLAYS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn note_widget_layer_replay() {
    WIDGET_LAYER_REPLAYS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
pub(crate) fn note_widget_layer_replay() {}

/// Resets and returns the layer replay counter.
#[cfg(all(test, full_widgets))]
pub(crate) fn take_widget_layer_replays_for_test() -> usize {
    WIDGET_LAYER_REPLAYS.with(|count| count.replace(0))
}

/// How many per-widget layers are resident on this thread.
#[cfg(all(test, full_widgets))]
pub(crate) fn cached_widget_layer_count_for_test() -> usize {
    WIDGET_LAYERS.try_with(|layers| layers.borrow().len()).unwrap_or(0)
}

/// Drops every per-widget layer on this thread.
#[cfg(all(test, full_widgets))]
pub(crate) fn clear_widget_layers_for_test() {
    let _ = WIDGET_LAYERS.try_with(|layers| layers.borrow_mut().clear());
    // The eviction ring and the fast-reject union describe the map that was just emptied, so
    // they must be dropped with it or a later store would reason about layers that no longer
    // exist (G-105).
    let _ = WIDGET_LAYER_ORDER.try_with(|order| order.borrow_mut().clear());
    let _ = WIDGET_LAYER_BOUNDS.try_with(|bounds| bounds.set(None));
    let _ = WIDGET_LAYER_REPLAYS.try_with(|count| count.set(0));
    // The pools are part of the layer state this helper resets: a test that clears the cache
    // must not inherit backends or buffers from whatever ran before it on the same thread.
    let _ = LAYER_BACKEND_POOL.try_with(|pool| pool.borrow_mut().clear());
    let _ = LAYER_BUFFER_POOL.try_with(|pool| pool.borrow_mut().clear());
}

// Counts full-frame buffer copies performed by the incremental frame cache. Test-only.
//
// # Why a counter rather than a custom allocator (D09-RENDER-04)
//
// The defect is a *count* — a no-damage frame copied its RGBA buffer three times. A global
// `GlobalAlloc` shim would count every allocation in the process, including the renderer's own
// scratch buffers, and could not separate the cache's copies from them. The cache performs a
// small, known set of full-buffer copies, so counting exactly those is the precise witness the
// regression test asserts on. Compiled only under `cfg(test)` (principle #28).
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FRAME_CACHE_COPIES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Records one full-frame copy made by the incremental cache. (D09-RENDER-04)
#[cfg(test)]
pub(crate) fn note_frame_cache_copy() {
    FRAME_CACHE_COPIES.with(|cell| cell.set(cell.get() + 1));
}

/// Resets and returns the full-frame copy counter. (D09-RENDER-04)
///
/// Gated on its callers and not just on `test`: they live in the `full_widgets` test module, so a
/// stripped-but-testing build (a sized profile with `icons` on, say) compiled this and reported it
/// as dead code.
#[cfg(all(test, full_widgets))]
pub(crate) fn take_frame_cache_copies() -> usize {
    FRAME_CACHE_COPIES.with(|cell| cell.replace(0))
}

/// No-op stand-in so non-test builds compile the same call sites. (D09-RENDER-04)
#[cfg(not(test))]
#[inline(always)]
pub(crate) fn note_frame_cache_copy() {}
