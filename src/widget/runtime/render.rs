// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Renders one frame, repainting only the damage when the policy allows it.
///
/// # Why the full frame is still returned
///
/// The return value is a complete `size.width * size.height * 4` buffer in every
/// mode, because a caller presents a frame and cannot present a rectangle. The
/// saving is in what is *drawn*, not in what is *returned*: in `Dirty` mode only the
/// damaged area is re-rasterised, and the untouched pixels are whatever the previous
/// frame left. Rebuilding the whole buffer per frame would give up that saving,
/// so `previous` carries the last frame in.
///
/// `previous` must be a buffer from an earlier call for the same widget and size.
/// Passing `None` forces a full paint, which is the honest answer when there is no
/// frame to preserve.
pub fn render_frame_incremental(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<&[u8]>,
) -> Option<Vec<u8>> {
    render_frame_incremental_cow(id, size, clear, previous.map(crate::compat::Cow::Borrowed))
}

/// [`render_frame_incremental`] with an **owned** previous frame.
///
/// The paint adopts the buffer as its draw target instead of copying it in. That copy is a full
/// frame — 4 MB at 1280×800 — and it used to cost more than painting a control-sized damage
/// region, which made "repaint only the damage" cheaper in pixels but not in time. The caller
/// only reaches this path when it owns the buffer outright (see `render_frame_cached_shared`).
pub(crate) fn render_frame_incremental_owned(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    render_frame_incremental_cow(id, size, clear, previous.map(crate::compat::Cow::Owned))
}

/// The shared body of the two [`render_frame_incremental`] entry points.
///
/// `previous` is a [`crate::compat::Cow`] because the two callers hold the previous frame
/// differently: the public API was handed a slice, while the cached path removed the frame from
/// its cache and may therefore own it. Borrowing and owning have to reach the same code — the
/// region painting below is the whole feature — so the difference is confined to how the buffer
/// is seeded and handed back.
pub(crate) fn render_frame_incremental_cow(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<crate::compat::Cow<'_, [u8]>>,
) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }

    let mode = repaint_mode(id);
    let expected = size.width as usize * size.height as usize * 4;
    // A buffer of the wrong size cannot be carried forward, so treat it as absent
    // rather than reading past its end.
    let carried = previous.filter(|buffer| buffer.len() == expected);

    // The region buffer is the widget's own reusable one (G-72): the coverage measurement and
    // `render_dirty_regions_into` below both write into it, so a frame allocates no region Vec.
    // The guard returns it on every exit path, including the full-paint returns below.
    let mut scratch = DirtyRectScratch::take(id);
    // Measured once: this used to call `dirty_rects` three times in the dirty path (once for the
    // empty check and twice for coverage), and each call cloned the tracker and merged it. The
    // predicate now asks the tracker directly and the rectangles go into the reusable buffer
    // (G-102).
    let use_dirty = if mode != RepaintMode::Full && carried.is_some() {
        dirty_rects_into(id, scratch.buffer())
    } else {
        scratch.buffer().clear();
        false
    };
    if mode == RepaintMode::Full || carried.is_none() {
        if let Some(frame) = render_frame(id, size, clear) {
            let _ = REPAINT.try_with(|map| {
                if let Some(state) = map.borrow_mut().get_mut(&id) {
                    state.tracker.clear();
                    // A full paint is not "large damage" evidence: it may be the first
                    // frame, or a resize. Resetting here keeps `Adaptive` from reading
                    // its own fallbacks as proof that damage tracking is hopeless.
                    state.large_damage_run = 0;
                }
            });
            return Some(frame);
        }
        return None;
    }

    // Measured *before* the regions are handed to `render_dirty_regions_into`, which clears
    // the tracker as it paints. Both the `Adaptive` decision and the run counter need
    // this frame's coverage, and after the call there is nothing left to measure.
    let covered_too_much = {
        let frame_area = u64::from(size.width) * u64::from(size.height);
        let covered: u64 = scratch
            .buffer()
            .iter()
            .map(|rect| u64::from(rect.width) * u64::from(rect.height))
            .sum();
        frame_area == 0
            || (covered as f32)
                >= (frame_area as f32) * crate::performance::render_dirty::FULL_REPAINT_AREA_RATIO
    };

    // `Adaptive` decides for itself whether measuring is worth it.
    //
    // This is the whole difference between `Adaptive` and `Dirty`. `Dirty` means the
    // caller has already decided, so every frame is measured. `Adaptive` means the
    // caller does not know, so a run of frames whose damage covers the surface is
    // taken as evidence that regioning cannot pay off here.
    //
    // # Why the run is checked *and* this frame's damage is measured
    //
    // The first version skipped regioning whenever the run was full, without looking
    // at the current frame — so a single small change after three large ones still
    // painted whole, and the run could never be cleared because the code that clears
    // it was never reached. The mode latched. Checking both conditions means the run
    // is only trusted while it is still true: as soon as a frame's damage is small,
    // that frame is measured (and resets the run), and the next frame regions again.
    if mode == RepaintMode::Adaptive {
        let learned = REPAINT
            .try_with(|map| {
                map.borrow()
                    .get(&id)
                    .is_some_and(|state| state.large_damage_run >= ADAPTIVE_LARGE_DAMAGE_RUN)
            })
            .unwrap_or(false);
        if learned && covered_too_much {
            if let Some(frame) = render_frame(id, size, clear) {
                let _ = REPAINT.try_with(|map| {
                    if let Some(state) = map.borrow_mut().get_mut(&id) {
                        state.tracker.clear();
                    }
                });
                return Some(frame);
            }
            return None;
        }
    }

    if !use_dirty {
        // Nothing was damaged: the previous frame is still correct, and redrawing
        // it would be work for no visible change.
        //
        // This is also the reset point for the `Adaptive` run: a frame with no damage
        // is proof the surface has settled.
        let _ = REPAINT.try_with(|map| {
            if let Some(state) = map.borrow_mut().get_mut(&id) {
                state.large_damage_run = 0;
            }
        });
        // `into_owned` moves the buffer when the caller owned it and copies when it borrowed —
        // exactly the difference between the two entry points.
        return Some(carried?.into_owned());
    }

    let mut backend = take_frame_backend(size);
    // Seed the backend with the previous frame so the regions this pass does not
    // touch keep their pixels. An owned buffer becomes the target; a borrowed one is copied in.
    let seeded = match carried? {
        crate::compat::Cow::Owned(buffer) => backend.seed_from_owned(buffer),
        crate::compat::Cow::Borrowed(slice) => backend.seed_from(slice),
    };
    if !seeded {
        // The length was checked above, so this is unreachable unless the surface and the frame
        // disagree about their size. Painting over a cleared frame is the honest recovery: the
        // region pass below only claims the damaged pixels, so a cleared backdrop is the one
        // state that cannot show a stale pixel.
        debug_assert!(seeded, "a length-checked frame must seed the backend");
        backend.begin_frame(clear);
    }

    // The widget draws in absolute coordinates and the frame is its own box, so the
    // frame origin is subtracted (see `render_frame`). This must be pushed *before*
    // the clip rectangles, which are also absolute, so that both translate together.
    let origin = frame_origin(id);

    let mut tracker = DirtyTrackerScratch::new(
        REPAINT
            .try_with(|map| {
                map.borrow_mut().get_mut(&id).map(|state| {
                    // Feed the `Adaptive` run counter — **only** in `Adaptive`. Updating it
                    // in `Dirty` too was the first version of this code, and it made the two
                    // modes observably identical again from the outside: `Dirty` reported a
                    // growing run while its contract says it never learns. The counter is
                    // `Adaptive`'s memory, so only `Adaptive` may write it.
                    if mode == RepaintMode::Adaptive {
                        state.large_damage_run = if covered_too_much {
                            state.large_damage_run.saturating_add(1)
                        } else {
                            0
                        };
                    }
                    // Swap the populated tracker out for an idle one instead of copying every
                    // region into a new tracker (ENG-A3): the state keeps an empty tracker, and
                    // the one it gives up already holds this frame's damage and the capacity
                    // earlier frames grew.
                    let mut spare = take_dirty_tracker_spare();
                    spare.clear();
                    core::mem::replace(&mut state.tracker, spare)
                })
            })
            .ok()
            .flatten()?,
    );
    tracker.get().merge();

    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            // The `_into` form keeps the region buffer across frames; the convenience form
            // allocated a fresh `Vec` for every rendered frame (G-72).
            crate::performance::render_dirty_regions_into(
                tracker.get(),
                &mut context,
                scratch.buffer(),
                |ctx| {
                    drawable.draw(ctx);
                },
            );
            context.pop_offset();
        }
        // Present the back buffer this pass drew into. Without this the frame read
        // below comes from the front buffer, which still holds the *pre-seed* state
        // — the seed went into `back`, so the untouched regions would read as blank
        // instead of as the previous frame.
        backend.end_frame();
        true
    })
    .unwrap_or(false);
    if !painted {
        return_frame_backend(backend, size);
        return None;
    }
    let frame = backend.take_frame_rgba();
    return_frame_backend(backend, size);
    Some(frame)
}

/// Renders a widget tree, reusing the previous frame when nothing that affects it has changed.
///
/// This is [`render_frame_tree`] with the one piece of memory a window painter should have: the
/// frame it produced last time. AppKit, Win32 and GTK all call their paint callback for reasons
/// that are not always a content change — an expose after another window moved away, an
/// occlusion change, a live-resize notification — and re-rasterising a whole tree for those is
/// pure cost. When the content epoch is unchanged the previous bytes are returned as they are;
/// when it moved, the frame is painted whole.
///
/// # Why the epoch is captured before painting
///
/// A control that animates requests its next frame *while it draws*. Storing the epoch read at
/// the end of the paint would record that request as already satisfied, and the animation would
/// advance once and then stop. Capturing it first means a request made during the paint belongs
/// to the next frame, which is what the control meant.
///
/// Returns `None` under the same conditions as [`render_frame_tree`].
///
/// # Migration note (G-108)
///
/// This **owned** form clones the whole frame that [`render_frame_tree_cached_shared`] already
/// holds — a 4K frame is about 33 MB of duplication for pixels nobody touched. It is kept only
/// for existing callers that need an owned `Vec<u8>`. **Every new call site must use
/// [`render_frame_tree_cached_shared`]**, which answers a refcount increment on the unchanged
/// path. The two are not renamed here so existing callers keep compiling; once the owned form has
/// no callers it can be collapsed into the shared entry point without changing that entry point's
/// name.
pub fn render_frame_tree_cached(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Vec<u8>> {
    let shared = render_frame_tree_cached_shared(id, size, clear)?;
    Some(shared.as_ref().clone())
}

/// [`render_frame_tree_cached`] returning the frame the cache holds, **not** a copy of it.
///
/// The unchanged-epoch path is the reason this exists: a 4K frame is about 33 MB, and the owned
/// form had to duplicate it to hand the caller a buffer whose pixels nobody had touched. The cache
/// already stores an `Rc<Vec<u8>>`, so this form answers a refcount increment. A caller that
/// presents the frame (or reads it as `&[u8]`) needs nothing more.
///
/// This is the **only recommended entry point for new callers** (G-108); the owned form exists
/// for legacy call sites and copies the frame unconditionally.
pub fn render_frame_tree_cached_shared(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Rc<Vec<u8>>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    // The window's own epoch (G-311): another window's repaint no longer turns this into a
    // miss. The process-wide repaint generation is unchanged — a global repaint still
    // invalidates every window.
    let epoch = renderable_epoch_for(id);
    let global_epoch = global_repaint_generation();
    let previous = LAST_TREE_FRAME
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .filter(|(stored, _, stored_epoch, stored_global)| {
                    *stored == size && *stored_epoch == epoch && *stored_global == global_epoch
                })
                .map(|(_, frame, _, _)| Rc::clone(frame))
        })
        .ok()
        .flatten();
    if let Some(previous) = previous {
        return Some(previous);
    }

    let shared = Rc::new(render_frame_tree(id, size, clear)?);
    let _ = LAST_TREE_FRAME.try_with(|map| {
        map.borrow_mut().insert(id, (size, Rc::clone(&shared), epoch, global_epoch));
    });
    // Record the epoch this frame was cached at in the per-window table (G-311), so a later
    // change whose owning window cannot be resolved can still find and bump this entry — and
    // so `renderable_epoch_for` reads the stored value rather than the default from here on.
    // A bump that raced the paint (a request made *during* the draw, which belongs to the next
    // frame) already inserted a newer value; `or_insert` keeps it.
    let _ = TREE_FRAME_EPOCHS.try_with(|epochs| {
        epochs.borrow_mut().entry(id).or_insert(epoch);
    });
    Some(shared)
}

/// Renders a window **as a platform painter presents it**: the window's widget tree, then every
/// mounted surface at its own rectangle.
///
/// # Why the tree alone is not the window
///
/// A mounted surface is not a child in the widget tree — `Platform::mount_surface` gives it a
/// native view of its own, and the platform paints each surface with `render_frame_cached` (see
/// `platform/macos/canvas.rs`). A caller that renders only [`render_frame_tree`] therefore gets a
/// picture with holes where the surfaced controls are: the demo's paging row and chart row were
/// empty rectangles. This is that missing half, in the order the painters use: the tree first, then
/// each surface composited over it at the surface's own geometry.
///
/// Surfaced pixels are straight (un-premultiplied) RGBA and may be transparent where the control
/// did not paint, which is why they are **composited** rather than copied.
///
/// Returns `None` when `id` is not mounted or the size is empty.
pub fn render_window_frame(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    let mut frame = render_frame_tree(id, size, clear)?;
    for surface in surfaces_of(id) {
        let Some(rect) = with_widget(surface, |widget| widget.geometry()) else {
            continue;
        };
        let surface_size = Size::new(rect.width, rect.height);
        if surface_size.width == 0 || surface_size.height == 0 {
            continue;
        }
        let Some(surface_frame) =
            render_frame_cached(surface, surface_size, crate::core::Color::TRANSPARENT)
        else {
            continue;
        };
        composite_surface(
            &mut frame,
            size.width,
            size.height,
            (rect.x, rect.y),
            &surface_frame,
            (surface_size.width, surface_size.height),
        );
    }
    Some(frame)
}

/// Alpha-composites a surface frame into a window frame at `(origin_x, origin_y)`.
///
/// Straight RGBA in and out, matching `SoftwareSurface::frame_rgba`'s contract: a surface paints as
/// much of its own rectangle as it covers and leaves the rest transparent. Rounding rather than
/// truncating matters — a fully opaque surface must land byte-exact, or a snapshot of a composited
/// window would differ from one of the same controls in the tree by one unit per channel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_surface(
    window: &mut [u8],
    window_width: u32,
    window_height: u32,
    origin: (i32, i32),
    surface: &[u8],
    surface_size: (u32, u32),
) {
    let (origin_x, origin_y) = origin;
    // Intersect the surface rectangle with the window once. The old loop tested every pixel of a
    // surface that was partly (or entirely) outside the window, which is the common case for a
    // control scrolled past an edge (G-315).
    let surface_width = surface_size.0 as i32;
    let surface_height = surface_size.1 as i32;
    let x0 = origin_x.max(0);
    let y0 = origin_y.max(0);
    let x1 = origin_x.saturating_add(surface_width).min(window_width as i32);
    let y1 = origin_y.saturating_add(surface_height).min(window_height as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for window_y in y0..y1 {
        let row = (window_y - origin_y) as u32;
        for window_x in x0..x1 {
            let column = (window_x - origin_x) as u32;
            let source = ((row * surface_size.0 + column) * 4) as usize;
            let destination = ((window_y as u32 * window_width + window_x as u32) * 4) as usize;
            let alpha = surface[source + 3] as u32;
            if alpha == 0 {
                continue;
            }
            if alpha == 255 {
                window[destination..destination + 4].copy_from_slice(&surface[source..source + 4]);
                continue;
            }
            let inverse = 255 - alpha;
            for channel in 0..4 {
                let source_value = surface[source + channel] as u32;
                let destination_value = window[destination + channel] as u32;
                window[destination + channel] =
                    ((source_value * alpha + destination_value * inverse + 127) / 255) as u8;
            }
        }
    }
}

/// Renders one frame of a mounted widget, reusing the previous one when the
/// repaint policy allows.
///
/// This is the entry point a backend's paint callback should call, and it is the one
/// that makes [`RepaintMode`] reach the screen. It differs from
/// [`render_frame_incremental`] in owning the previous frame: the caller passes only
/// the widget and the size, and this function remembers what it last produced.
///
/// # Why the cache is keyed by size
///
/// A frame of the wrong size cannot seed a repaint — it would be copied into a larger
/// surface leaving the tail untouched, or truncated. Storing the size beside the frame
/// means a resize is detected here and turns into a full paint automatically, without
/// the caller having to remember to tell anyone.
///
/// # Return value
///
/// The complete frame, always. A backend presents a frame and cannot present a
/// rectangle, so the saving is in what is *drawn*, not in what is returned. Returns
/// `None` when the widget is unmounted, has no `Draw` impl, or the size is empty.
pub fn render_frame_cached(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    let shared = render_frame_cached_shared(id, size, clear)?;
    // The owned form is the one that pays: this is the single copy that becomes the caller's
    // frame. `render_frame_cached_shared` makes none at all.
    note_frame_cache_copy();
    Some(shared.as_ref().clone())
}

/// [`render_frame_cached`] returning the cached frame itself, so "nothing changed" costs an
/// `Rc` increment rather than a full-buffer copy.
///
/// The owned form has to hand back a `Vec<u8>`; for a 4K surface that is ~33 MB duplicated to
/// return pixels the cache already holds. A caller that presents the frame, or reads it as
/// `&[u8]`, can take this form and never copy.
pub fn render_frame_cached_shared(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Rc<Vec<u8>>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }

    // Take a shared handle to the previous frame — a pointer clone, not a pixel copy. A frame of
    // the wrong size is treated as absent so the callee falls back to a full paint.
    // (D09-RENDER-04)
    let previous = LAST_FRAME
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .filter(|(stored, _)| *stored == size)
                .map(|(_, frame)| Rc::clone(frame))
        })
        .ok()
        .flatten();

    // # No-damage fast path (D09-RENDER-04)
    //
    // When the mode repaints only damage and nothing is damaged, the previous frame *is* the
    // frame. The old code still ran the incremental machinery: it copied the cached buffer into
    // `previous`, copied it again in `render_frame_incremental` via `carried.to_vec()`, and copied
    // it a third time back into the cache. Here the cache is left exactly as it is and only the
    // single copy needed to hand an owned buffer to the caller is made. `Full` mode is excluded
    // because it must repaint regardless, and a missing previous cannot take this path.
    if let Some(previous) = &previous {
        if repaint_mode(id) != RepaintMode::Full && !has_dirty_rects(id) {
            // A frame with no damage also proves the surface settled, which is the `Adaptive`
            // mode's reset point — `render_frame_incremental` clears the run on this path, and
            // this fast path must do the same or `Adaptive` would stay latched after an animation
            // ends (the exact failure the run counter exists to avoid).
            let _ = REPAINT.try_with(|map| {
                if let Some(state) = map.borrow_mut().get_mut(&id) {
                    state.large_damage_run = 0;
                }
            });
            return Some(Rc::clone(previous));
        }
    }

    // # Why the cache entry is removed and possibly moved out
    //
    // The damaged path starts from the previous frame's pixels. Copying them into the paint
    // target is a full-frame `memcpy`, which used to cost more than painting the damage region
    // itself — the incremental path was saving pixels and paying the time back in the copy. Taking
    // the entry out of the cache leaves this function as the only holder, so `try_unwrap` can hand
    // the buffer to the paint when the caller has already dropped its frame (the ordinary loop).
    // A caller that still holds the frame makes `try_unwrap` refuse, and the pixels are borrowed
    // and copied exactly as before. The entry is replaced with the new frame below, so the gap
    // cannot be observed.
    drop(previous);
    let taken = LAST_FRAME
        .try_with(|map| map.borrow_mut().remove(&id))
        .ok()
        .flatten()
        .filter(|(stored, _)| *stored == size);
    let frame = match taken {
        Some((_, previous)) => match Rc::try_unwrap(previous) {
            Ok(buffer) => render_frame_incremental_owned(id, size, clear, Some(buffer)),
            Err(previous) => render_frame_incremental(id, size, clear, Some(previous.as_slice())),
        },
        None => render_frame_incremental(id, size, clear, None),
    }?;

    // The freshly produced frame is moved into the `Rc` the cache holds — no pixel copy — and the
    // same handle is returned, so the painted path does not duplicate it either.
    let shared = Rc::new(frame);
    let _ = LAST_FRAME.try_with(|map| {
        map.borrow_mut().insert(id, (size, Rc::clone(&shared)));
    });
    Some(shared)
}

/// Forgets the cached frame for `id`.
///
/// Called when a widget is unmounted, so a frame cannot outlive the control it
/// describes — and so its id, if reused, does not start from someone else's pixels.
pub fn forget_cached_frame(id: ObjectId) {
    let _ = LAST_FRAME.try_with(|map| map.borrow_mut().remove(&id));
    let _ = LAST_TREE_FRAME.try_with(|map| map.borrow_mut().remove(&id));
    // The per-window epoch exists to describe the cached frame that just went away (G-311);
    // dropping it keeps the table at the windows that still hold a frame.
    let _ = TREE_FRAME_EPOCHS.try_with(|epochs| {
        epochs.borrow_mut().remove(&id);
    });
    let _ = WIDGET_LAYERS.try_with(|map| {
        let mut map = map.borrow_mut();
        if let Some(old) = map.remove(&id) {
            recycle_layer_pixels(old.pixels);
        }
        // Keep the eviction ring in step with the map (ENG-A1).
        retain_resident_layer_order(&map);
        // The fast-reject union is only a superset, so it may stay after a removal; when the
        // cache is empty it is reset, which is what restores the free "no layers" rejection.
        if map.is_empty() {
            WIDGET_LAYER_BOUNDS.with(|bounds| bounds.set(None));
        }
    });
}

/// The size of the cached frame for `id`, if one is held.
///
/// Exposed so a backend can tell "I have a frame for this size" from "the next paint
/// will be full", which is what a resize handler needs to know.
pub fn cached_frame_size(id: ObjectId) -> Option<Size> {
    LAST_FRAME.try_with(|map| map.borrow().get(&id).map(|(size, _)| *size)).ok().flatten()
}

/// How many consecutive over-threshold frames [`RepaintMode::Adaptive`] has seen.
///
/// # Why this is public
///
/// `Adaptive` differs from `Dirty` only in this memory, so it is the only way to tell
/// the two policies apart from outside — and a policy that cannot be observed cannot
/// be tested. A host can also read it to decide whether to switch a widget to `Full`
/// itself, which is the honest use of the number.
///
/// Zero for a widget in any other mode, and for one that is not mounted.
pub fn adaptive_large_damage_run(id: ObjectId) -> u32 {
    REPAINT
        .try_with(|map| map.borrow().get(&id).map(|state| state.large_damage_run))
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// Renders one frame of a mounted widget at `size` and returns the RGBA bytes.
///
/// `bytes.len() == size.width * size.height * 4`, top-down, straight (non
/// premultiplied) alpha — the layout all three desktop backends consume.
/// Returns `None` when `id` is not mounted or the size is empty.
///
/// # Why the widget's own origin is subtracted
///
/// Widget geometry is **absolute**: a control's rect is its position in the window, and
/// every control draws at those absolute coordinates (the same model the event router
/// uses, see [`widget_at`]). The frame here is only `size.width * size.height` — the
/// control's own box — so drawing at the absolute origin of a control placed at, say,
/// `(260, 42)` would land 260 px right and 42 px down inside its own buffer: the
/// top-left band stays clear and the bottom-right is clipped away. Pushing the negated
/// origin makes the control's absolute coordinates land on its own frame origin, which
/// is the only interpretation that can be correct for every control already written.
pub fn render_frame(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    // The frame is the control's own box, so it has to be drawn at the box's origin
    // rather than at the control's absolute position.
    let origin = frame_origin(id);
    let mut backend = take_frame_backend(size);
    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            drawable.draw(&mut context);
            context.pop_offset();
        }
        backend.end_frame();
        true
    })
    .unwrap_or(false);
    if !painted {
        return_frame_backend(backend, size);
        return None;
    }
    let frame = backend.take_frame_rgba();
    return_frame_backend(backend, size);
    Some(frame)
}

/// Renders one frame of a mounted widget using its own geometry as the size.
///
/// Convenience wrapper for backends that size the native canvas from the
/// widget's geometry rather than from the OS-reported rectangle.
pub fn render_frame_at_geometry(id: ObjectId, clear: crate::core::Color) -> Option<Vec<u8>> {
    let geometry = geometry_of(id)?;
    render_frame(id, Size::new(geometry.width, geometry.height), clear)
}

/// The cached overlay flag for one root child of [`ordered_root_children`] (G-316).
///
/// The two virtual calls (`is_visible`, `has_open_popup`) run only when no answer is cached —
/// the child's first frame, or the first frame after its state changed. A child the registry
/// cannot answer for (lent out mid-tick) is treated as an ordinary child exactly as before, and
/// the miss is **not** cached, so the next frame asks again rather than keeping a wrong answer.
pub(crate) fn overlay_child_flag(child: ObjectId) -> bool {
    OVERLAY_CHILD_FLAGS.with(|flags| {
        let cached = flags.borrow().get(&child).copied();
        if let Some(flag) = cached {
            return flag;
        }
        match read_widget(child, |widget| widget.is_visible() && widget.has_open_popup()) {
            Some(flag) => {
                flags.borrow_mut().insert(child, flag);
                flag
            }
            None => false,
        }
    })
}

/// Drops the cached overlay flag of `id`, or the whole cache when `id` is `None` (G-316).
///
/// The flag reads widget-internal state the runtime cannot observe directly, so the mutation
/// notification is the repaint the state change must request to become visible, plus the popup
/// grab lifecycle every popup's open and close goes through.
pub(crate) fn note_overlay_maybe_changed(id: Option<ObjectId>) {
    OVERLAY_CHILD_FLAGS.with(|flags| {
        let mut flags = flags.borrow_mut();
        match id {
            Some(id) => {
                flags.remove(&id);
            }
            None => flags.clear(),
        }
    });
}

/// Orders a window's children for painting: ordinary children first, open overlays last.
///
/// A drop-down list is painted by its own control, below the control's rectangle, and the window
/// may have siblings declared after it that occupy the same pixels (the next form row). Declaration
/// order would paint those siblings over the list — the same split the hit test had, where the row
/// under the pointer took the click. Painting overlays last makes what the user sees agree with what
/// the hit test already prefers (`has_open_popup`).
///
/// Only direct children are reordered: every popup control in this crate draws its list itself, so
/// the popup is the direct child. Nested containers keep their own declaration order.
pub(crate) fn ordered_root_children(children: &mut Vec<ObjectId>) {
    // Stable partition in place: ordinary children keep their order, then the overlays keep
    // theirs. The previous implementation allocated two new vectors and copied every id, on every
    // frame, just to move the few overlay children to the back (G-36). The overlays buffer itself
    // is pooled as well, so a static frame allocates nothing at all (G-109).
    let mut overlays = OVERLAY_CHILD_IDS.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
    overlays.clear();
    children.retain(|child| {
        // The classification comes from the flag cache (G-316); the two virtual calls run once
        // per child per membership change instead of once per child per frame.
        let overlay = overlay_child_flag(*child);
        if overlay {
            overlays.push(*child);
            false
        } else {
            true
        }
    });
    children.extend(overlays.iter().copied());
    OVERLAY_CHILD_IDS.with(|slot| {
        let mut slot = slot.borrow_mut();
        if overlays.capacity() > slot.capacity() {
            overlays.clear();
            *slot = overlays;
        }
    });
}

/// Renders one frame of a widget **and its descendants**, returning RGBA bytes.
///
/// # Why this exists
///
/// [`render_frame`] paints exactly one widget into its own box. That is right for a
/// self-drawn control mounted as a surface, but it means a widget that **contains** other
/// widgets paints none of them: a window drawn this way shows its own chrome over an empty
/// client area. A window whose children are ordinary library controls — a button, a check
/// box, a label — is the case that matters, and those children are never mounted as
/// surfaces of their own, so nothing painted them.
///
/// This walks the child list depth-first and draws each child that implements
/// [`Draw`][crate::widget::Draw], in tree order, so a later sibling paints over an earlier
/// one — the same order a container's own `draw` would use. Direct children with an open
/// overlay (a drop-down list) are painted last, so the list is not covered by a sibling that
/// happens to occupy the same pixels.
///
/// # Coordinates
///
/// Geometry is absolute (see [`render_frame`]), and the frame's origin is `id`'s own
/// position, so the negated origin is pushed once. Children draw at their own absolute
/// coordinates, which that single translation maps into this frame.
///
/// # What is skipped, and why silently
///
/// A child is skipped when it is not visible, when it is not in the registry, or when it
/// does not implement `Draw`. Skipping is not an error because all three are ordinary: a
/// hidden page, a child whose id was recycled, and a control that is a pure model for a
/// host to materialise are each a normal state, and reporting them would make a correct
/// tree look broken.
///
/// Returns `None` when `id` is not mounted or the size is empty.
pub fn render_frame_tree(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    // Captured before anything paints: a request made *during* a draw belongs to the next frame,
    // so the layer stored now must not carry the epoch that request produced. This is the layer
    // epoch rather than the tree epoch: a frame is repainted whole after any change, while the
    // controls whose pixels did not change keep their layers.
    let epoch = layer_epoch();
    let global_epoch = global_repaint_generation();
    let origin = frame_origin(id);
    let frame_bounds = Rect::new(origin.0, origin.1, size.width, size.height);
    let mut backend = take_frame_backend(size);

    // The root's own painting is part of the frame: a window draws its background and
    // chrome, which the children then sit on top of. Its descendant clip and child list are
    // read in that same loan, so the root is not visited twice before the traversal starts.
    let mut root_clip = None;
    let mut root_children = ROOT_CHILD_IDS.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
    root_children.clear();
    let mut painted = with_widget_mut(id, |widget| {
        root_clip = widget.children_clip();
        root_children.extend_from_slice(widget.base().children());
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            drawable.draw(&mut context);
            context.pop_offset();
        }
        true
    })
    .unwrap_or(false);

    if painted {
        // Each direct child is offered to the per-widget layer cache. A child whose subtree is
        // unchanged replays its cached pixels; a changed one is rasterised once into its own
        // transparent offscreen surface and then composited into this frame. The recursion below
        // owns the descent, so the root's children are the only stack this function needs.
        //
        // Every entry carries the clip **its ancestors** impose on it. A control answers
        // `children_clip` for its descendants (a scroll area's viewport, for example), and the
        // painter intersects that with the clip its own ancestors already imposed. Without the
        // carried rectangle a control's content would escape the viewport as soon as it scrolled.
        //
        // `SubtreeMetrics` is a per-frame memo: subtree bounds and the open-popup flag are each
        // computed once per node, not once per ancestor that asks for them. The table is taken from
        // a thread-local slot and returned afterwards, so a static frame allocates none (G-36).
        let mut metrics = TREE_METRICS.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
        metrics.clear();
        ordered_root_children(&mut root_children);
        // DLG-001: the scrim owner is resolved once per frame, and the plane is painted directly
        // before it. A modal that is one of these children is rasterised into its own layer, where
        // its scrim is clipped to its rectangle — so leaving the plane to the modal loses the
        // dimming entirely, and painting it in both places dims the page twice.
        #[cfg(full_widgets)]
        let scrim_owner = crate::widget::dialog::scrim_owner_for_root(id);
        #[cfg(full_widgets)]
        crate::widget::dialog::reset_plane_painted();
        for child in root_children.iter().copied() {
            #[cfg(full_widgets)]
            if painted && scrim_owner == Some(child) {
                let mut context = RenderContext::new(&mut backend);
                context.fill_rect(
                    Rect::new(0, 0, size.width, size.height),
                    crate::widget::dialog::scrim_plane_color(),
                );
                crate::widget::dialog::note_plane_painted(child);
            }
            // A node whose clip has no area cannot paint anything, and neither can its
            // descendants. Skipping the node skips the subtree, the same rule hiding uses.
            if root_clip.is_some_and(|rect| rect.width == 0 || rect.height == 0) {
                continue;
            }
            let painted_child = paint_widget_layer(
                child,
                root_clip,
                &mut backend,
                origin,
                frame_bounds,
                size,
                epoch,
                global_epoch,
                0,
                &mut metrics,
            );
            if painted_child.drew {
                painted = true;
            }
        }
        // ── Window-level modal scrim (DLG-001) ──
        //
        // A modal mounted as its own surface is not one of this frame's painted children, so its
        // own `draw` never runs here and the window it dims would keep its full brightness. When
        // the owning modal is painted by this frame, its own scrim already covers the page; the
        // window pass is therefore only owed when it is not among the visited subtrees.
        #[cfg(full_widgets)]
        if let Some(owner) = scrim_owner {
            let painted_here = metrics.contains_key(&owner) || root_children.contains(&owner);
            if !painted_here {
                let mut context = RenderContext::new(&mut backend);
                context.fill_rect(
                    Rect::new(0, 0, size.width, size.height),
                    crate::widget::dialog::scrim_plane_color(),
                );
            }
        }
        TREE_METRICS.with(|slot| {
            let mut slot = slot.borrow_mut();
            if metrics.capacity() > slot.capacity() {
                metrics.clear();
                *slot = metrics;
            }
        });
    }
    ROOT_CHILD_IDS.with(|slot| {
        let mut slot = slot.borrow_mut();
        if root_children.capacity() > slot.capacity() {
            root_children.clear();
            *slot = root_children;
        }
    });

    if !painted {
        return_frame_backend(backend, size);
        return None;
    }
    backend.end_frame();
    let frame = backend.take_frame_rgba();
    return_frame_backend(backend, size);
    Some(frame)
}

/// Bounds and painting facts for one widget subtree, computed once per frame.
#[derive(Clone, Copy)]
pub(crate) struct SubtreeMetrics {
    /// Union of the subtree's painted boxes, or `None` when the subtree is hidden.
    pub(crate) bounds: Option<Rect>,
    /// Whether any visible node in the subtree paints an overlay outside its own rectangle.
    pub(crate) has_open_popup: bool,
}

// The per-frame subtree memo, reused across frames. `render_frame_tree` empties it at the start
// and stores the grown buffer back at the end (G-36).
thread_local! {
    pub(crate) static TREE_METRICS: RefCell<HashMap<ObjectId, SubtreeMetrics>> = RefCell::new(HashMap::new());
}

/// The result of offering one widget subtree to the layer cache.
#[derive(Clone, Copy)]
pub(crate) struct LayerPaint {
    /// Whether this subtree painted anything.
    pub(crate) drew: bool,
    /// Whether the subtree was rendered into a rectangle that fully describes it.
    pub(crate) cacheable: bool,
}

/// The deepest subtree the painter will descend, so a corrupted parent/child cycle cannot
/// overflow the stack. Real trees are far shallower.
pub(crate) const MAX_LAYER_DEPTH: u32 = 64;

/// Returns the subtree's painted bounds and whether it paints outside its own box, memoised.
pub(crate) fn subtree_metrics(
    id: ObjectId,
    depth: u32,
    memo: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> SubtreeMetrics {
    if let Some(cached) = memo.get(&id) {
        return *cached;
    }
    let empty = SubtreeMetrics { bounds: None, has_open_popup: false };
    if depth >= MAX_LAYER_DEPTH {
        return empty;
    }
    // The child list is copied into a pooled per-depth buffer rather than a fresh `Vec` per node
    // (G-101). The copy is unavoidable: `read_widget` holds the registry borrow while this runs,
    // so the recursion below cannot read the children through it. The pool is the same one the
    // painter uses for its own child snapshots, and `take`/`return` survive re-entrancy because
    // they only pop from and push to a stack.
    let mut children = take_paint_children();
    children.clear();
    let info = read_widget(id, |widget| {
        children.extend_from_slice(widget.base().children());
        (widget.is_visible(), widget.has_open_popup(), grow_by_damage_margin(widget.geometry()))
    });
    let metrics = match info {
        Some((true, has_open_popup, mut bounds)) => {
            let mut has_open_popup = has_open_popup;
            for &child in &children {
                let child_metrics = subtree_metrics(child, depth + 1, memo);
                has_open_popup |= child_metrics.has_open_popup;
                if let Some(child_bounds) = child_metrics.bounds {
                    bounds = union_rect(bounds, child_bounds);
                }
            }
            SubtreeMetrics { bounds: Some(bounds), has_open_popup }
        }
        // A hidden node hides its whole subtree, and an id that is no longer mounted paints
        // nothing. Both are ordinary states, not errors.
        _ => empty,
    };
    return_paint_children(children);
    memo.insert(id, metrics);
    metrics
}

/// The union of two rectangles, saturating at the coordinate limits.
pub(crate) fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1.saturating_sub(x0).max(0) as u32, y1.saturating_sub(y0).max(0) as u32)
}

/// Narrows a subtree's painted bounds to what the frame and the ancestors can show.
pub(crate) fn layer_bounds(raw: Rect, clip: Option<Rect>, frame_bounds: Rect) -> Option<Rect> {
    let bounded = intersect_clip(Some(raw), clip)?;
    let bounded = intersect_clip(Some(bounded), Some(frame_bounds))?;
    (bounded.width > 0 && bounded.height > 0).then_some(bounded)
}

/// Draws a cached layer into `backend` at its absolute position.
pub(crate) fn blit_widget_layer(
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    bounds: Rect,
    pixels: &[u8],
) {
    let mut context = RenderContext::new(backend);
    context.draw_image(
        bounds.x - origin.0,
        bounds.y - origin.1,
        bounds.width,
        bounds.height,
        pixels,
    );
}

/// Paints one widget subtree, reusing its cached layer when one still describes the paint.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_widget_layer(
    id: ObjectId,
    clip: Option<Rect>,
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    frame_bounds: Rect,
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    depth: u32,
    metrics: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> LayerPaint {
    let subtree = subtree_metrics(id, depth, metrics);
    let Some(raw_bounds) = subtree.bounds else {
        return LayerPaint { drew: false, cacheable: true };
    };
    // A subtree that paints outside its own box (an open drop-down list) cannot be captured in
    // a bounded layer without clipping the overlay. Paint it straight into the target instead,
    // and mark it uncacheable so no ancestor captures a layer that omits the overlay.
    if subtree.has_open_popup {
        return paint_widget_direct(
            id,
            clip,
            backend,
            origin,
            frame_bounds,
            frame_size,
            epoch,
            global_epoch,
            depth,
            metrics,
        );
    }
    let Some(bounds) = layer_bounds(raw_bounds, clip, frame_bounds) else {
        return LayerPaint { drew: false, cacheable: true };
    };

    if let Some(pixels) =
        lookup_widget_layer(id, bounds, clip, origin, frame_size, epoch, global_epoch)
    {
        blit_widget_layer(backend, origin, bounds, pixels.as_slice());
        note_widget_layer_replay();
        return LayerPaint { drew: true, cacheable: true };
    }

    // A miss paints the subtree into its own transparent surface. Draws inside the layer are
    // translated by the layer's absolute origin, exactly as the frame path translates them by
    // the frame origin. `end_frame` publishes the drawn back buffer as the frame to take.
    let layer_size = Size::new(bounds.width, bounds.height);
    let mut offscreen = take_layer_backend(layer_size);
    let mut children = take_paint_children();
    let mut child_clip = None;
    let mut self_drew = false;
    let rendered = with_widget_mut(id, |widget| {
        children.clear();
        children.extend_from_slice(widget.base().children());
        child_clip = intersect_clip(clip, widget.children_clip());
        let Some(drawable) = widget.as_draw_mut() else {
            return;
        };
        let mut context = RenderContext::new(&mut offscreen);
        context.push_offset(-bounds.x, -bounds.y);
        drawable.draw(&mut context);
        context.pop_offset();
        self_drew = true;
    });
    if rendered.is_none() {
        return_paint_children(children);
        return_layer_backend(offscreen, layer_size);
        return LayerPaint { drew: false, cacheable: true };
    }

    let mut child_drew = false;
    let mut cacheable = true;
    for &child in &children {
        let painted = paint_widget_layer(
            child,
            child_clip,
            &mut offscreen,
            (bounds.x, bounds.y),
            bounds,
            layer_size,
            epoch,
            global_epoch,
            depth + 1,
            metrics,
        );
        child_drew |= painted.drew;
        cacheable &= painted.cacheable;
    }
    return_paint_children(children);
    if !self_drew && !child_drew {
        return_layer_backend(offscreen, layer_size);
        return LayerPaint { drew: false, cacheable };
    }

    offscreen.end_frame();
    // The presented frame is copied into a pooled buffer rather than taken: the backend that
    // drew it stays alive for the next layer miss, and the buffer that leaves came from (and
    // returns to) the pool of recycled layer buffers, so a same-size miss allocates nothing
    // at all (G-313). The bytes are identical to what `into_frame_rgba` moved out — the same
    // presented front buffer, at the same length.
    let frame = offscreen.surface().frame_rgba();
    let mut pixels = take_layer_buffer(frame.len());
    pixels.copy_from_slice(frame);
    return_layer_backend(offscreen, layer_size);
    let pixels = Rc::new(pixels);
    if cacheable {
        store_widget_layer(
            id,
            bounds,
            clip,
            origin,
            frame_size,
            epoch,
            global_epoch,
            Rc::clone(&pixels),
        );
    }
    blit_widget_layer(backend, origin, bounds, pixels.as_slice());
    LayerPaint { drew: true, cacheable }
}

/// Paints one widget and its descendants directly into `backend`.
///
/// This is the path for a subtree that paints outside its own rectangle. It keeps the original
/// clipping and ordering rules; descendants without an overlay can still be layered, which is
/// why it recurses through [`paint_widget_layer`] rather than drawing the whole subtree here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_widget_direct(
    id: ObjectId,
    clip: Option<Rect>,
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    frame_bounds: Rect,
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    depth: u32,
    metrics: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> LayerPaint {
    let mut children = take_paint_children();
    let mut child_clip = None;
    let mut self_drew = false;
    let rendered = with_widget_mut(id, |widget| {
        if !widget.is_visible() {
            return;
        }
        children.clear();
        children.extend_from_slice(widget.base().children());
        child_clip = intersect_clip(clip, widget.children_clip());
        let Some(drawable) = widget.as_draw_mut() else {
            return;
        };
        let mut context = RenderContext::new(backend);
        context.push_offset(-origin.0, -origin.1);
        if let Some(rect) = clip {
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
        }
        drawable.draw(&mut context);
        if clip.is_some() {
            context.pop_clip();
        }
        context.pop_offset();
        self_drew = true;
    });
    if rendered.is_none() {
        return_paint_children(children);
        return LayerPaint { drew: false, cacheable: false };
    }
    let mut child_drew = false;
    for &child in &children {
        let painted = paint_widget_layer(
            child,
            child_clip,
            backend,
            origin,
            frame_bounds,
            frame_size,
            epoch,
            global_epoch,
            depth + 1,
            metrics,
        );
        child_drew |= painted.drew;
    }
    return_paint_children(children);
    LayerPaint { drew: self_drew || child_drew, cacheable: false }
}

/// The overlap of two clip rectangles, or a zero-area rectangle when they do not overlap.
///
/// A zero area is a real answer ("nothing is visible"), not an absence of clipping: the painter
/// skips such a subtree. Returning `None` would widen the clip back to the parent's, which is the
/// one wrong direction.
pub(crate) fn intersect_clip(outer: Option<Rect>, inner: Option<Rect>) -> Option<Rect> {
    match (outer, inner) {
        (Some(a), Some(b)) => Some(Rect::new(
            a.x.max(b.x),
            a.y.max(b.y),
            (a.x.saturating_add(a.width as i32))
                .min(b.x.saturating_add(b.width as i32))
                .saturating_sub(a.x.max(b.x))
                .max(0) as u32,
            (a.y.saturating_add(a.height as i32))
                .min(b.y.saturating_add(b.height as i32))
                .saturating_sub(a.y.max(b.y))
                .max(0) as u32,
        )),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}

/// The direct children of `id`, or an empty list when it is not mounted.
///
/// Read through [`with_widget`] so a caller sees the same widget the traversal will.
pub(crate) fn direct_children_of(id: ObjectId) -> Vec<ObjectId> {
    read_widget(id, |widget| widget.base().children().to_vec()).unwrap_or_default()
}

/// The direct children of `id`, as a public read-only view of the mounted tree.
///
/// # Why this is public
///
/// The self-drawn backend positions and paints a window by walking this child list, so
/// "what does this window actually contain?" is a question a host, a test, and a designer
/// all need to ask. Only the private traversal could answer it before, which meant a
/// caller diagnosing a control that did not appear had to guess from creation order
/// instead of reading the tree (principle #100: designer readiness has to be inspectable,
/// not described).
///
/// Returns an empty vector for an unmounted id, matching
/// [`geometry_of`]: an id that addresses nothing has no children, and `[]` is the honest
/// answer rather than an error the caller must distinguish from "no children".
pub fn children_of(id: ObjectId) -> Vec<ObjectId> {
    direct_children_of(id)
}
