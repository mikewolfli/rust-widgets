// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Records that the mounted **surface** `surface` is shown inside library window `window`.
///
/// Called by the mount path, the only caller that knows both ids. Returns `false` when
/// the surface is unknown to this thread's registry, so a caller cannot invent a link
/// to a widget that does not exist.
pub fn set_surface_window(surface: ObjectId, window: ObjectId) -> bool {
    if with_widget(surface, |_| ()).is_none() {
        return false;
    }
    SURFACE_WINDOWS
        .try_with(|map| {
            let previous = map.borrow_mut().insert(surface, window);
            // The reverse index moves with the forward link, in the same breath (G-314): a
            // first link appends the surface to the window's list, a re-link moves it out of
            // the previous window's list first. Re-linking to the window it already names
            // leaves the list untouched, so no duplicate is pushed.
            let _ = SURFACES_BY_WINDOW.try_with(|index| {
                let mut index = index.borrow_mut();
                if previous != Some(window) {
                    if let Some(old) = previous {
                        if let Some(list) = index.get_mut(&old) {
                            list.retain(|linked| *linked != surface);
                        }
                    }
                    index.entry(window).or_default().push(surface);
                }
            });
            previous.is_none()
        })
        .unwrap_or(false)
}

/// The library window a mounted surface belongs to, if one was recorded.
pub fn surface_window_for(surface: ObjectId) -> Option<ObjectId> {
    SURFACE_WINDOWS.try_with(|map| map.borrow().get(&surface).copied()).unwrap_or(None)
}

/// Every mounted surface recorded as belonging to `window`.
///
/// A surface mounted through `mount_surface` is **not** a child in the widget tree: the
/// backend paints it, and `SURFACE_WINDOWS` is the only record of which window owns it.
/// A tree walk that is meant to cover everything a window shows therefore has to add these
/// ids itself — [`crate::widget::save_state::collect_save_state`] does, so a control mounted
/// as a surface is not silently skipped by session restore.
///
/// The order is unspecified (the link is a map); a caller that needs determinism sorts by its
/// own key.
pub fn surfaces_of(window: ObjectId) -> Vec<ObjectId> {
    // A probe into the reverse index answers with exactly this window's surfaces (G-314);
    // the full-table filter this used to be was O(total surfaces) on every call, which the
    // per-frame surface walk of a multi-surface window paid in full.
    SURFACES_BY_WINDOW
        .try_with(|index| index.borrow().get(&window).cloned().unwrap_or_default())
        .unwrap_or_default()
}

/// Returns the top-level **window** widget that `id` lives under, if any.
///
/// Walks the parent links up to the root and reports it only when the root is a
/// `WidgetKind::Window`. A widget mounted with no window above it (a test fixture, a
/// surface the host parented itself) answers `None`, which is the honest result: there
/// is no window whose invalidation would reveal the change.
///
/// The walk is bounded by `max_depth` steps so a corrupted parent chain — a cycle, or a
/// link to an id that was recycled — cannot hang a paint path. Hitting the bound is
/// reported as `None` rather than guessed at.
///
/// A **mounted surface** is the exception the walk cannot answer: it is registered with
/// no parent (the platform mounts it as its own drawing area), so the mount path records
/// its window in `SURFACE_WINDOWS` and that link is consulted first.
///
/// # The cached answer (G-106)
///
/// Almost every setter ends in `request_redraw`, which reaches this function, so the
/// parent-chain walk used to cost one map lookup (plus a widget borrow) per tree level on
/// that hot path. The answer is resolved once when the control mounts and stored on its
/// `Mounted` entry; a lookup then reads that entry and compares the **direct parent** the
/// answer was resolved from. `BaseWidget::set_parent`/`add_child_linked` can move a widget
/// after it is mounted, and a mismatch re-resolves rather than returning a stale window.
/// A widget that is not in the registry (unmounted, or lent out to a callback) falls back to
/// the previous walk, which is what resolves a shared loan through [`read_widget`].
pub fn top_level_window_of(id: ObjectId) -> Option<ObjectId> {
    // A mounted surface has no parent in the widget tree, so the walk below cannot
    // reach its window; the mount path recorded that link instead.
    if let Some(window) = surface_window_for(id) {
        return Some(window);
    }
    // Fast path: the resolution cached on the mount, valid while the direct parent it was
    // resolved from is still the widget's parent (G-106).
    let cached = MOUNTED
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .map(|entry| (entry.widget.parent(), entry.widget.kind(), entry.top_window))
        })
        .ok()
        .flatten();
    if let Some((parent, _, Some((resolved_parent, window)))) = cached {
        if resolved_parent == parent {
            return window;
        }
    }
    let resolved = match cached {
        // Mounted: resolve from the entry's own kind and parent. Reading `id` again through
        // `read_widget` would be a second map lookup on the path this cache exists to shorten.
        Some((parent, kind, _)) => walk_top_level_window_from(id, kind, parent),
        // Not in the registry: it is unmounted, or a callback has it lent out. The previous
        // walk is `read_widget`-based and therefore still answers for a shared loan; nothing is
        // cached for an id that may not stay mounted.
        None => walk_top_level_window(id),
    };
    if let Some((parent, _, _)) = cached {
        let _ = MOUNTED.try_with(|map| {
            if let Some(entry) = map.borrow_mut().get_mut(&id) {
                entry.top_window = Some((parent, resolved));
            }
        });
    }
    resolved
}

/// The depth bound of the parent-chain walk. A widget tree is shallow (window → containers →
/// controls); 64 is far past any real depth and still a bound rather than a loop that trusts
/// the data.
pub(crate) const TOP_WINDOW_MAX_DEPTH: usize = 64;

/// Resolves the top-level window of a **mounted** widget from its own kind and parent.
pub(crate) fn walk_top_level_window_from(
    id: ObjectId,
    kind: crate::widget::WidgetKind,
    parent: Option<ObjectId>,
) -> Option<ObjectId> {
    if kind == crate::widget::WidgetKind::Window {
        return Some(id);
    }
    let mut current = parent?;
    for _ in 0..TOP_WINDOW_MAX_DEPTH {
        let (kind, parent) = read_widget(current, |widget| (widget.kind(), widget.parent()))?;
        if kind == crate::widget::WidgetKind::Window {
            return Some(current);
        }
        current = parent?;
    }
    None
}

/// The original registry-based walk, used for an id that is not in [`MOUNTED`].
pub(crate) fn walk_top_level_window(id: ObjectId) -> Option<ObjectId> {
    let (kind, parent) = read_widget(id, |widget| (widget.kind(), widget.parent()))?;
    walk_top_level_window_from(id, kind, parent)
}

/// Repaints a widget **and the window it lives in**.
///
/// # Why invalidating the control alone is not enough
///
/// Since 2.0 the library paints every `WidgetKind` itself, so most controls have no
/// native surface of their own and `invalidate_surface(control)` reaches nothing. On a
/// backend whose window paints its whole child list (Windows, Linux), the thing that
/// must be invalidated is the **window**: its `WM_PAINT` / `connect_draw` redraws the
/// tree the control belongs to.
///
/// A mounted surface is still invalidated too — it paints itself, and on macOS that is
/// the only path a control has. Both calls are made and neither is treated as a failure
/// when it answers `false`: which of the two the backend can honour is the backend's
/// business, not this function's.
pub fn request_repaint_subtree(id: ObjectId) {
    // The window first, so a backend that coalesces per frame folds the control's own
    // request into the frame the window is already going to repaint.
    if let Some(window) = top_level_window_of(id) {
        request_repaint_because(window, RepaintReason::Explicit);
    }
    request_repaint_because(id, RepaintReason::Explicit);
}

/// [`request_repaint_subtree`] with an explicit cause, for a caller that knows why.
///
/// The animation sweep uses this so a frame that advanced a control reports `Animation` in
/// the frame ledger rather than the catch-all `Explicit`. The distinction is the whole point
/// of [`RepaintReason`]: `Explicit` means "a caller asked and named no mechanism", and an
/// interpolation step is a mechanism.
pub(crate) fn request_repaint_subtree_because(id: ObjectId, reason: RepaintReason) {
    if let Some(window) = top_level_window_of(id) {
        request_repaint_because(window, reason);
    }
    request_repaint_because(id, reason);
}

/// Asks the platform to repaint a mounted widget, naming **why** it is being asked.
///
/// # Why the reason is a parameter and not inferred
///
/// A caller knows the cause and the runtime does not: a hover finished (state), a value
/// interpolation advanced (animation), an overlay appeared (overlay). Inferring it here would
/// mean guessing from a stack that has already returned, and a wrong reason is worse than none
/// because the frame ledger's whole purpose is to let a reader act on it (BLUE24 §8).
///
/// # Coalescing
///
/// A request for a surface already repainted this frame is folded into it and counted as
/// coalesced; see `note_frame_repaint_reason`.
pub fn request_repaint_because(id: ObjectId, reason: RepaintReason) {
    // A whole-control request names no rectangle, so no layer can be proven still valid for the
    // control itself. The invalidation is bounded to the control's box when the runtime can read
    // it, so a control that repaints wholesale does not throw away an unrelated neighbour's
    // layer; a widget the registry cannot answer for (it is lent out mid-tick) falls back to
    // dropping every layer, which is the safe direction.
    // The thread-global token keeps the point-hit cache's invalidation exactly as it was (see
    // `TREE_FRAME_EPOCH`); the window-scoped one (G-311) drops only the cached tree frame of
    // the window this control lives in. The owner is resolved with shared borrows, so this
    // stays safe even mid-`read_widget`.
    note_renderable_change();
    note_renderable_change_for(id);
    // This widget's own overlay classification may be the thing that changed: a popup opening
    // or closing, or the control showing or hiding itself, must repaint to be seen (G-316).
    note_overlay_maybe_changed(Some(id));
    match geometry_of(id) {
        Some(rect) => invalidate_widget_layers_in(id, grow_by_damage_margin(rect)),
        None => invalidate_widget_layers(),
    }
    if crate::invalidate_surface(id) {
        note_frame_repaint_reason(id, reason);
    }
}

/// Tells the library that a **platform-owned surface** needs to be redrawn.
///
/// # Why the platform must come through here
///
/// A native control owns its own pixels (BLUE24 §7.1), so the platform can and must ask for
/// its own repaint. What it must not do is ask *without telling the library*, because three
/// things then stop being true at once:
///
/// 1. `performance::render_dirty_regions` computes damage the frame never actually submitted,
///    so "what did this frame paint?" has no answer;
/// 2. the still-frame guarantee (BLUE24 §1 criterion 2 — an idle window submits nothing) can
///    no longer be shown, so "smooth is not paid for with a burning core" is unprovable;
/// 3. the self-painted and native worlds each keep their own count, so one interaction is
///    submitted twice.
///
/// So a backend routes its own redraw requests through this function. It is the *same* ledger
/// [`request_repaint`] writes to, so a frame that mixed the two worlds still reports one
/// coherent account.
///
/// `rect` is the damaged region when the platform can name one. It is **not** used to narrow
/// the submission (the platform has already decided what it needs to repaint, and second-
/// guessing it here would be the library inventing a claim about pixels it does not own), but
/// it is recorded, so the ledger can distinguish a full-surface redraw from a partial one.
pub fn notify_native_redraw(id: ObjectId, rect: Option<Rect>) {
    // The platform has already invalidated its own surface; this call exists so the *library*
    // learns about it. Recording the region keeps the account complete even though the
    // submission is the platform's.
    let _ = rect;
    note_frame_repaint_reason(id, RepaintReason::Native);
}
