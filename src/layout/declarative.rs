// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime storage and application for declarative layouts.
//!
//! # Why this module exists separately from `json::layout`
//!
//! The declarative engine was one module: it read `serde_json` values into layout
//! kinds, built the layout objects, stored them per parent, and applied them by
//! writing child geometries. Only the *parsing* needs `serde_json`. Keeping the rest
//! there meant the C ABI — which builds a layout from a kind and four numbers, and
//! has no JSON to parse — could not reach any of it without depending on the JSON
//! module.
//!
//! So the split is by dependency, not by taste:
//!
//! * **here** — the registry and the geometry application, depending only on
//!   `crate::layout` and the widget runtime;
//! * **`json::layout`** — the `serde_json` → layout-kind translation, which calls
//!   into this module.
//!
//! Both paths then share one implementation of "store a layout and move the
//! children", which is what stops the JSON loader and the C ABI from drifting into
//! two subtly different layout engines.
//!
//! # Thread affinity
//!
//! The registry is thread-local, so a layout stored on one thread is invisible to
//! another. That is deliberate rather than incidental: a layout holds `ObjectId`s
//! that are only meaningful in the thread that mounted them, so sharing the map
//! across threads would let one thread apply a layout to another's widgets. The
//! consequence is that declarative layouts are single-threaded by construction, and
//! applying one from the wrong thread is a silent no-op rather than a corruption.
//!
//! # G-406 convergence boundary (what lives here, what lives in `view`)
//!
//! `view/engine` owns **identity** for a declarative retained tree: keyed paths,
//! diff/patch, and the id map. This module owns only the **layout registry** — which
//! layout sits on which parent — and the geometry application. It is deliberately not a
//! second retained-tree implementation.
//!
//! The bridge is `SharedLayoutRegistry` plus `SharedLayoutRegistry::apply_with_hints`
//! (and the free `apply_layout_with_hints`): a caller that has the child hints (the view
//! engine does) can drive the stored layouts through `Layout::arrange`, instead of the
//! id-only `update` path `apply_layout` uses. The thread-local `LAYOUT_MAP` remains the
//! transitional default. What the view side still has to do is hold one registry instance
//! and call the hint-aware entry when it applies a subtree; that edit belongs to
//! `src/view/**`, which this module does not own.

use crate::compat::{Box, HashMap, Vec};
use core::cell::RefCell;

use crate::core::{ObjectId, Rect};
use crate::layout::{ChildInfo, Layout};

/// Sentinel child id a layout uses to reserve stretchable space.
///
/// The generic [`Layout`] trait has no spacer method, so the declarative engine
/// smuggles a spacer through as a child whose id is this value — the widest possible
/// `ObjectId`, which no real widget can hold. Layouts that do not know the sentinel
/// treat it as an ordinary child and allocate it space, which is the intended effect.
///
/// It is exposed rather than private because a layout implementation outside this
/// module needs to recognise it, and because [`apply_layout`] must exclude it from
/// the geometries it writes: writing a geometry for this id would address nothing.
pub const SPACER_ID: ObjectId = ObjectId::MAX;

thread_local! {
    /// Layouts by parent id. See the module docs for why this is thread-local.
    #[allow(clippy::missing_const_for_thread_local)]
    static LAYOUT_MAP: RefCell<HashMap<ObjectId, Box<dyn Layout>>> =
        RefCell::new(HashMap::new());
    /// Parents whose layout has been invalidated since its last application.
    ///
    /// A `Vec` rather than a set: a UI tree has tens of containers, and the linear scan
    /// is cheaper than hashing an `ObjectId` on every call.
    #[allow(clippy::missing_const_for_thread_local)]
    static DIRTY_LAYOUTS: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
}

/// Marks `parent_id`'s stored layout as needing a fresh application.
///
/// Returns `false` when no layout is stored for the parent. Invalidation is merged: a
/// second call before [`apply_layout_if_dirty`] is a no-op, which is the "many edits, one
/// recompute" behaviour a layout pass wants.
pub fn invalidate_layout(parent_id: ObjectId) -> bool {
    if !has_layout(parent_id) {
        return false;
    }
    DIRTY_LAYOUTS.with(|dirty| {
        let mut dirty = dirty.borrow_mut();
        if !dirty.contains(&parent_id) {
            dirty.push(parent_id);
        }
    });
    true
}

/// Whether `parent_id` has been invalidated and not yet re-applied.
pub fn is_layout_dirty(parent_id: ObjectId) -> bool {
    DIRTY_LAYOUTS.with(|dirty| dirty.borrow().contains(&parent_id))
}

/// Applies the layout only when it has been invalidated.
///
/// Returns `None` when the layout is clean or absent, so a caller can skip the whole
/// arrangement (and the geometry writes) for an unchanged tree.
pub fn apply_layout_if_dirty(parent_id: ObjectId, rect: Rect) -> Option<Vec<(ObjectId, Rect)>> {
    if !is_layout_dirty(parent_id) {
        return None;
    }
    DIRTY_LAYOUTS.with(|dirty| dirty.borrow_mut().retain(|id| *id != parent_id));
    Some(apply_layout(parent_id, rect))
}

/// A layout registry that may be shared across threads.
///
/// The default registry is thread-local by design (see the module docs), which makes a
/// declarative tree owned by a worker thread invisible to the UI thread. This type is the
/// opt-in alternative: a caller that owns the tree on one thread can hand the registry to
/// the UI thread and still apply its layouts.
///
/// A layout stored here must be `Send`; the thread-local registry keeps accepting every
/// `Layout`, so this is a widening of the API rather than a new requirement on existing
/// layouts.
#[derive(Default)]
pub struct SharedLayoutRegistry {
    layouts: std::sync::Mutex<HashMap<ObjectId, Box<dyn Layout + Send>>>,
    dirty: std::sync::Mutex<Vec<ObjectId>>,
}

impl SharedLayoutRegistry {
    /// Creates an empty shared registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a layout, replacing any previous one for the parent.
    pub fn store(&self, parent_id: ObjectId, layout: Box<dyn Layout + Send>) {
        if let Ok(mut layouts) = self.layouts.lock() {
            layouts.insert(parent_id, layout);
        }
        if let Ok(mut dirty) = self.dirty.lock() {
            if !dirty.contains(&parent_id) {
                dirty.push(parent_id);
            }
        }
    }

    /// Removes a layout, returning whether one was present.
    pub fn forget(&self, parent_id: ObjectId) -> bool {
        let removed = self
            .layouts
            .lock()
            .map(|mut layouts| layouts.remove(&parent_id).is_some())
            .unwrap_or(false);
        if let Ok(mut dirty) = self.dirty.lock() {
            dirty.retain(|id| *id != parent_id);
        }
        removed
    }

    /// Whether a layout is stored for the parent.
    pub fn has(&self, parent_id: ObjectId) -> bool {
        self.layouts.lock().map(|layouts| layouts.contains_key(&parent_id)).unwrap_or(false)
    }

    /// Number of stored layouts.
    pub fn len(&self) -> usize {
        self.layouts.lock().map(|layouts| layouts.len()).unwrap_or(0)
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Marks a stored layout as dirty, returning whether it exists.
    pub fn invalidate(&self, parent_id: ObjectId) -> bool {
        if !self.has(parent_id) {
            return false;
        }
        if let Ok(mut dirty) = self.dirty.lock() {
            if !dirty.contains(&parent_id) {
                dirty.push(parent_id);
            }
        }
        true
    }

    /// Applies a stored layout, clearing its dirty flag.
    pub fn apply(&self, parent_id: ObjectId, rect: Rect) -> Vec<(ObjectId, Rect)> {
        let geometries = self.run(parent_id, |layout| {
            let mut geometries = Vec::new();
            layout.update(rect, &mut |child_id, child_rect| {
                if child_id != SPACER_ID {
                    geometries.push((child_id, child_rect));
                }
            });
            geometries
        });
        if let Ok(mut dirty) = self.dirty.lock() {
            dirty.retain(|id| *id != parent_id);
        }
        geometries
    }

    /// Applies a stored layout through the hint channel, clearing its dirty flag.
    ///
    /// [`Self::apply`] can only call `Layout::update`, which has no way to ask a child how
    /// big it wants to be. A caller that owns the hints — the declarative view engine does —
    /// uses this entry so the stored layout's `arrange` implementation runs instead. That is
    /// the layout-side half of G-406: the identity/diff side stays in `view/engine`, and this
    /// registry no longer forces the production path onto the hint-less solver.
    pub fn apply_with_hints(
        &self,
        parent_id: ObjectId,
        rect: Rect,
        children: &[ChildInfo],
    ) -> Vec<(ObjectId, Rect)> {
        let geometries = self.run(parent_id, |layout| {
            let mut geometries = Vec::new();
            layout.arrange(rect, children, &mut |child_id, child_rect| {
                if child_id != SPACER_ID {
                    geometries.push((child_id, child_rect));
                }
            });
            geometries
        });
        if let Ok(mut dirty) = self.dirty.lock() {
            dirty.retain(|id| *id != parent_id);
        }
        geometries
    }

    /// Runs `read` against the stored layout, or returns an empty list when there is none.
    fn run<R>(&self, parent_id: ObjectId, read: impl FnOnce(&dyn Layout) -> R) -> R
    where
        R: Default,
    {
        match self.layouts.lock() {
            Ok(layouts) => match layouts.get(&parent_id) {
                Some(layout) => read(layout.as_ref()),
                None => R::default(),
            },
            Err(_) => R::default(),
        }
    }

    /// Applies the layout only when it is dirty.
    pub fn apply_if_dirty(&self, parent_id: ObjectId, rect: Rect) -> Option<Vec<(ObjectId, Rect)>> {
        let dirty = self.dirty.lock().map(|dirty| dirty.contains(&parent_id)).unwrap_or(false);
        if !dirty {
            return None;
        }
        Some(self.apply(parent_id, rect))
    }
}

/// Stores a layout manager for a parent widget, replacing any previous one.
///
/// Replacing rather than merging is the only defensible behaviour: two layouts on one
/// parent would each compute geometries for the same children and the second would
/// win arbitrarily.
pub fn store_layout(parent_id: ObjectId, layout: Box<dyn Layout>) {
    LAYOUT_MAP.with(|map| {
        map.borrow_mut().insert(parent_id, layout);
    });
    let _ = invalidate_layout(parent_id);
}

/// Removes the layout stored for `parent_id`.
///
/// Returns `true` when there was one. Unmounting a container should call this, or the
/// registry keeps a layout whose children no longer exist and whose ids may later be
/// reused.
pub fn forget_layout(parent_id: ObjectId) -> bool {
    let removed = LAYOUT_MAP.with(|map| map.borrow_mut().remove(&parent_id).is_some());
    DIRTY_LAYOUTS.with(|dirty| dirty.borrow_mut().retain(|id| *id != parent_id));
    removed
}

/// Whether a layout is stored for `parent_id`.
pub fn has_layout(parent_id: ObjectId) -> bool {
    LAYOUT_MAP.with(|map| map.borrow().contains_key(&parent_id))
}

/// How many layouts are stored on this thread.
pub fn layout_count() -> usize {
    LAYOUT_MAP.with(|map| map.borrow().len())
}

/// Registers a widget as a layout child with its stretch factor.
///
/// Silent no-op when no layout is stored for `parent_id`. The stretch factor is
/// stored verbatim; `0` is not a valid stretch in most layout implementations and may
/// make the child invisible.
///
/// Returns `true` when a layout was found and told about the child.
pub fn add_widget_to_layout(child_id: ObjectId, stretch: u32, parent_id: ObjectId) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        match map.get_mut(&parent_id) {
            Some(layout) => {
                layout.add_widget(child_id, stretch);
                true
            }
            None => false,
        }
    })
}

/// Where a declaratively placed child sits, for the layouts that accept explicit cells.
///
/// # Why this is a separate call from [`add_widget_to_layout`]
///
/// Only some layouts have cells. The `Layout` trait's `add_widget` takes a widget and a
/// stretch factor and nothing else, and widening it for the one grid-shaped member would make
/// every one of the fifteen implementations carry a parameter all but one ignores. The cell
/// is therefore offered through the trait's downcasting door: a layout that understands cells
/// implements it, and one that does not is told nothing rather than being handed a placement
/// it would have to discard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildPlacement {
    /// Column of the child's top-left cell.
    pub col: u32,
    /// Row of the child's top-left cell.
    pub row: u32,
    /// Columns covered, at least 1.
    pub col_span: u32,
    /// Rows covered, at least 1.
    pub row_span: u32,
}

/// Places `child_id` at an explicit cell of the layout stored for `parent_id`.
///
/// Returns `true` when the layout both exists and understands cells, so a caller can report
/// a placement that would otherwise be silently dropped — the failure this exists to prevent.
/// A layout that does not use cells (`BoxLayout`, `FlexLayout`, …) returns `false`, and the
/// caller decides whether that is a problem: the declarative loader turns it into a warning,
/// because a document that wrote `col`/`row` on a non-grid parent asked for something the
/// parent cannot do.
pub fn place_widget_in_layout(
    child_id: ObjectId,
    placement: ChildPlacement,
    parent_id: ObjectId,
) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        let Some(layout) = map.get_mut(&parent_id) else {
            return false;
        };
        if let Some(grid) = layout.as_any_mut().downcast_mut::<crate::layout::GridLayout>() {
            grid.set_widget_spanning(
                placement.row,
                placement.col,
                placement.col_span,
                placement.row_span,
                child_id,
            );
            // `set_widget_spanning` logs a refusal past the cell cap but has no return
            // value (it is a public placement API whose signature predates this door).
            // Asking the layout whether the child is now placed is the honest answer the
            // caller needs; reporting `true` unconditionally would turn a refused
            // placement into a silent success.
            return grid.has_child(child_id);
        }
        if let Some(grid) = layout.as_any_mut().downcast_mut::<crate::layout::UniformGridLayout>() {
            return grid.set_widget_spanning(
                placement.row,
                placement.col,
                placement.col_span,
                placement.row_span,
                child_id,
            );
        }
        false
    })
}

/// Places `child_id` in a named region of a [`BorderLayout`](crate::layout::BorderLayout).
///
/// # Why this is a separate entry point from [`place_widget_in_layout`]
///
/// The two layouts accept different placement facts: a grid takes a cell and a span, a
/// border layout takes a region and a fixed extent. Widening [`ChildPlacement`] would make
/// every caller of the cell path supply fields that mean nothing to it. The declarative
/// loader calls whichever entry point the child's attributes name.
///
/// Returns `true` when the stored layout is a border layout and the child was a member of
/// it. A child that was never added is **not** silently inserted: the caller's registration
/// step is what made it a child, and inventing a second registration here would make the
/// generic `add_widget` and this call disagree about ownership.
pub fn place_widget_in_region_layout(
    child_id: ObjectId,
    region: crate::layout::BorderRegion,
    extent: u32,
    parent_id: ObjectId,
) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        let Some(layout) = map.get_mut(&parent_id) else {
            return false;
        };
        match layout.as_any_mut().downcast_mut::<crate::layout::BorderLayout>() {
            Some(border) => border.set_region(child_id, region, extent),
            None => false,
        }
    })
}

/// Removes a widget from the layout stored for `parent_id`.
///
/// Returns `true` when a layout was found and told about the removal.
pub fn remove_widget_from_layout(child_id: ObjectId, parent_id: ObjectId) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        match map.get_mut(&parent_id) {
            Some(layout) => {
                layout.remove_widget(child_id);
                true
            }
            None => false,
        }
    })
}

/// Records a spacer with `stretch` for a parent layout.
///
/// See [`SPACER_ID`] for how a spacer is represented and why. Note the signature has
/// no layout argument: the earlier version took one and ignored it, looking the layout
/// up by parent id anyway, which made the parameter a misleading promise that the
/// caller's layout would be used.
///
/// Returns `true` when a layout was found.
pub fn add_spacer_to_layout(stretch: u32, parent_id: ObjectId) -> bool {
    add_widget_to_layout(SPACER_ID, stretch, parent_id)
}

/// Recomputes child geometries and writes them to the widgets.
///
/// Returns the geometries that were applied, so a caller can assert on the result
/// rather than on a count of side effects.
///
/// # Why the spacer is excluded
///
/// A layout reports a geometry for every child it holds, including [`SPACER_ID`].
/// Writing that one would call `set_widget_geometry` for an id that addresses no
/// widget, which is at best a wasted lookup and at worst a collision with an id the
/// runtime later issues. It is filtered here, at the one place geometries become
/// widget state.
pub fn apply_layout(parent_id: ObjectId, rect: Rect) -> Vec<(ObjectId, Rect)> {
    // A declarative layout can be nested inside another container whose layout places it.
    // When the outer layout moves the inner container, the inner layout has to be asked
    // again against its new rectangle; otherwise its children keep the boxes they were
    // given at load time. The bound is the same one the widget runtime uses for parent
    // walks, and it keeps a corrupted/self-referential document from recursing forever.
    const MAX_LAYOUT_DEPTH: u32 = 64;
    apply_layout_bounded(parent_id, rect, 0, MAX_LAYOUT_DEPTH)
}

fn apply_layout_bounded(
    parent_id: ObjectId,
    rect: Rect,
    depth: u32,
    max_depth: u32,
) -> Vec<(ObjectId, Rect)> {
    // The recursion below is compiled out with `set_widget_geometry` on alloc-frugal
    // builds (there is no registry to move), so the two traversal parameters have no
    // reader there. Naming them as intentionally unused keeps `mini` warning-free.
    #[cfg(alloc_frugal)]
    let _ = (depth, max_depth);
    let geometries = LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update(rect, &mut |child_id, child_rect| {
            if child_id != SPACER_ID {
                geometries.push((child_id, child_rect));
            }
        });
        geometries
    });

    // `set_widget_geometry` is compiled out in the alloc-frugal profile (there is no
    // widget runtime to address), so there is nothing to move the geometries onto.
    // The computed list is still returned: a caller in that profile can use it to
    // place whatever it draws, which is the honest answer rather than a silent no-op.
    #[cfg(not(alloc_frugal))]
    for (child_id, child_rect) in &geometries {
        crate::set_widget_geometry(
            *child_id,
            child_rect.x,
            child_rect.y,
            child_rect.width,
            child_rect.height,
        );
        if depth < max_depth && has_layout(*child_id) {
            apply_layout_bounded(*child_id, *child_rect, depth + 1, max_depth);
        } else if depth >= max_depth && has_layout(*child_id) {
            log::warn!(
                "declarative layout for widget {parent_id} exceeded the {max_depth}-level nesting \
                 bound; child {child_id}'s layout was not applied"
            );
        }
    }

    geometries
}

/// The geometry a layout would compute, without writing anything.
///
/// Lets a caller preview or validate a layout — a design-time inspector, or a test
/// asserting the arrangement — without moving the widgets.
pub fn preview_layout(parent_id: ObjectId, rect: Rect) -> Vec<(ObjectId, Rect)> {
    LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update(rect, &mut |child_id, child_rect| {
            if child_id != SPACER_ID {
                geometries.push((child_id, child_rect));
            }
        });
        geometries
    })
}

/// Recomputes `parent_id`'s geometry through the hint channel.
///
/// The thread-local counterpart of [`SharedLayoutRegistry::apply_with_hints`]. It exists so
/// a caller that has the child hints — the view engine, or a test — can run
/// `Layout::arrange` on the stored layout without first moving it into a shared registry.
/// The spacer is still filtered, for the same reason [`apply_layout`] filters it.
pub fn apply_layout_with_hints(
    parent_id: ObjectId,
    rect: Rect,
    children: &[ChildInfo],
) -> Vec<(ObjectId, Rect)> {
    LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.arrange(rect, children, &mut |child_id, child_rect| {
            if child_id != SPACER_ID {
                geometries.push((child_id, child_rect));
            }
        });
        geometries
    })
}

/// How many children `parent_id`'s declarative layout holds, **not** counting spacers.
///
/// The C ABI used to answer this with `preview_layout(parent, Rect::new(0, 0, 0, 0)).len()`, which
/// runs the whole arrangement — every child's size arithmetic, a `Vec` of geometries, and an
/// `update` callback per child — to throw all of it away except the length. A caller polling the
/// count (a live inspector, a test loop) paid that on every call.
///
/// A spacer is still not a child here: [`add_spacer_to_layout`] stores it under [`SPACER_ID`],
/// which carries no control, so counting it would overstate the number of children a host can
/// address by id. That is the same rule `preview_layout` applies.
pub fn layout_child_count(parent_id: ObjectId) -> usize {
    LAYOUT_MAP.with(|map| {
        map.borrow().get(&parent_id).map_or(0, |layout| {
            layout.child_ids().iter().filter(|child| **child != SPACER_ID).count()
        })
    })
}

/// Name of a `DeclarativeLayoutKind`, for diagnostics and the C ABI.
///
/// Defined here rather than on the enum so this module does not depend on the
/// `serde_json`-facing one; the enum's own `Display`-like spelling is a presentation
/// concern this module can answer without owning the type.
pub fn layout_kind_name(kind: LayoutKindName) -> &'static str {
    match kind {
        LayoutKindName::HorizontalBox => "hbox",
        LayoutKindName::VerticalBox => "vbox",
        LayoutKindName::Grid => "grid",
        LayoutKindName::Border => "border",
        LayoutKindName::Form => "form",
        LayoutKindName::Stack => "stack",
        LayoutKindName::HBox => "hbox",
    }
}

/// The layout spellings this module can name.
///
/// A separate enum rather than a reference to `json::DeclarativeLayoutKind`, because
/// naming a layout must work in profiles where the JSON module is not compiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKindName {
    /// A row of children.
    HorizontalBox,
    /// A column of children.
    VerticalBox,
    /// A row/column grid.
    Grid,
    /// Fixed edge tracks around an elastic center.
    Border,
    /// Two-column label/field rows.
    Form,
    /// One visible child at a time.
    Stack,
    /// Alias of [`Self::HorizontalBox`], kept because the declarative spelling uses it.
    HBox,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{BoxLayout, Orientation};

    fn box_layout() -> Box<dyn Layout> {
        Box::new(BoxLayout::new(Orientation::Horizontal, 0, 0))
    }

    /// A layout stored on one thread is invisible to another, which is the documented
    /// thread affinity rather than an accident.
    ///
    /// Spawned thread rather than a second `thread_local` cell so the assertion is
    /// about the real map.
    #[test]
    fn layouts_are_thread_local() {
        const PARENT: ObjectId = 0x5EED;
        store_layout(PARENT, box_layout());
        assert!(has_layout(PARENT));

        let seen = std::thread::spawn(move || has_layout(PARENT))
            .join()
            .expect("the probe thread must not panic");
        assert!(!seen, "a layout stored on one thread must not be visible on another");

        assert!(forget_layout(PARENT));
    }

    #[test]
    fn storing_replaces_rather_than_accumulates() {
        const PARENT: ObjectId = 0x5EEF;
        let before = layout_count();

        store_layout(PARENT, box_layout());
        store_layout(PARENT, box_layout());
        assert_eq!(layout_count(), before + 1, "one parent holds one layout");

        assert!(forget_layout(PARENT));
        assert_eq!(layout_count(), before);
    }

    #[test]
    fn forgetting_an_unknown_parent_is_false() {
        assert!(!forget_layout(0xDEAD_BEEF));
    }

    /// Adding a child to a parent with no layout must report the miss rather than
    /// silently doing nothing.
    #[test]
    fn adding_a_child_without_a_layout_reports_failure() {
        assert!(!add_widget_to_layout(1, 0, 0xDEAD_BEEF));
        assert!(!add_spacer_to_layout(1, 0xDEAD_BEEF));
    }

    /// Applying a layout must produce one geometry per child, and must not produce one
    /// for the spacer sentinel.
    #[test]
    fn applying_a_layout_returns_child_geometries_and_skips_the_spacer() {
        const PARENT: ObjectId = 0x5EF0;
        store_layout(PARENT, box_layout());
        assert!(add_widget_to_layout(101, 0, PARENT));
        assert!(add_widget_to_layout(102, 0, PARENT));
        assert!(add_spacer_to_layout(1, PARENT));

        let geometries = preview_layout(PARENT, Rect::new(0, 0, 300, 100));
        assert_eq!(geometries.len(), 2, "the spacer must not appear as a widget");
        let ids: Vec<ObjectId> = geometries.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![101, 102]);

        assert!(forget_layout(PARENT));
    }

    /// A horizontal box places its two children side by side, which is the assertion
    /// that separates "the layout ran" from "the layout did something".
    #[test]
    fn a_horizontal_box_places_children_side_by_side() {
        const PARENT: ObjectId = 0x5EF1;
        store_layout(PARENT, box_layout());
        add_widget_to_layout(201, 0, PARENT);
        add_widget_to_layout(202, 0, PARENT);

        let geometries = preview_layout(PARENT, Rect::new(0, 0, 300, 100));
        let first = geometries.iter().find(|(id, _)| *id == 201).expect("first child").1;
        let second = geometries.iter().find(|(id, _)| *id == 202).expect("second child").1;

        assert!(
            second.x > first.x,
            "the second child must start after the first: {first:?} then {second:?}"
        );
        assert_eq!(first.y, second.y, "a horizontal box keeps one row");

        assert!(forget_layout(PARENT));
    }

    #[test]
    fn applying_an_unknown_parent_yields_nothing() {
        assert!(apply_layout(0xDEAD_BEEF, Rect::new(0, 0, 10, 10)).is_empty());
        assert!(preview_layout(0xDEAD_BEEF, Rect::new(0, 0, 10, 10)).is_empty());
    }

    /// G-406: the hint-aware entry runs `Layout::arrange`, not the id-only `update`, so a
    /// caller that owns the child hints gets the hinted geometry. This is the layout-side
    /// bridge the view engine uses; identity/diff remain in `view/engine`.
    #[test]
    fn applying_with_hints_runs_the_arrange_path() {
        use crate::layout::{GridLayout, Hints};

        const PARENT: ObjectId = 0x5EF3;
        let mut grid = GridLayout::new(1, 2, 0, 0);
        grid.set_widget(0, 0, 201);
        grid.set_widget(0, 1, 202);
        store_layout(PARENT, Box::new(grid));
        let children =
            [ChildInfo::new(201, Hints::fixed(150, 10)), ChildInfo::new(202, Hints::fixed(50, 10))];

        let without = preview_layout(PARENT, Rect::new(0, 0, 200, 20));
        let without_first = without.iter().find(|(id, _)| *id == 201).expect("child").1;
        assert_eq!(without_first.width, 100, "the id-only path splits evenly");

        let with = apply_layout_with_hints(PARENT, Rect::new(0, 0, 200, 20), &children);
        let with_first = with.iter().find(|(id, _)| *id == 201).expect("child").1;
        assert_eq!(with_first.width, 150, "the hint-aware path honours the preferred width");

        assert!(forget_layout(PARENT));
    }

    /// The same bridge on the `Send` registry the view engine is meant to own.
    #[test]
    fn the_shared_registry_applies_with_hints() {
        use crate::layout::{GridLayout, Hints};

        const PARENT: ObjectId = 0x5EF4;
        let mut grid = GridLayout::new(1, 2, 0, 0);
        grid.set_widget(0, 0, 301);
        grid.set_widget(0, 1, 302);
        let registry = SharedLayoutRegistry::new();
        registry.store(PARENT, Box::new(grid));
        let children =
            [ChildInfo::new(301, Hints::fixed(70, 10)), ChildInfo::new(302, Hints::fixed(130, 10))];

        let out = registry.apply_with_hints(PARENT, Rect::new(0, 0, 200, 20), &children);
        assert_eq!(out.iter().find(|(id, _)| *id == 301).expect("child").1.width, 70);
        assert_eq!(out.iter().find(|(id, _)| *id == 302).expect("child").1.width, 130);
    }

    /// Unregistering must be possible, or a reused id would inherit a stale layout.
    #[test]
    fn removing_a_child_reaches_the_layout() {
        const PARENT: ObjectId = 0x5EF2;
        store_layout(PARENT, box_layout());
        add_widget_to_layout(301, 0, PARENT);
        assert!(remove_widget_from_layout(301, PARENT));

        let geometries = preview_layout(PARENT, Rect::new(0, 0, 100, 50));
        assert!(
            geometries.iter().all(|(id, _)| *id != 301),
            "a removed child must not be laid out"
        );

        assert!(!remove_widget_from_layout(301, 0xDEAD_BEEF));
        assert!(forget_layout(PARENT));
    }

    /// Invalidation is merged and an unchanged layout is not recomputed.
    #[test]
    fn dirty_layouts_apply_once_and_then_stay_clean() {
        const PARENT: ObjectId = 0x5EF3;
        store_layout(PARENT, box_layout());
        add_widget_to_layout(401, 0, PARENT);
        assert!(is_layout_dirty(PARENT));
        assert!(invalidate_layout(PARENT));
        let first = apply_layout_if_dirty(PARENT, Rect::new(0, 0, 80, 20));
        assert_eq!(first.map(|entries| entries.len()), Some(1));
        assert!(!is_layout_dirty(PARENT));
        assert!(apply_layout_if_dirty(PARENT, Rect::new(0, 0, 80, 20)).is_none());
        assert!(!invalidate_layout(0xDEAD_BEEF));
        assert!(forget_layout(PARENT));
    }

    /// The shared registry carries a layout across threads, which the thread-local
    /// default deliberately cannot do.
    #[test]
    fn the_shared_registry_is_usable_from_another_thread() {
        struct OneChild;
        impl Layout for OneChild {
            fn add_widget(&mut self, _widget_id: ObjectId, _stretch: u32) {}
            fn remove_widget(&mut self, _widget_id: ObjectId) {}
            fn update(&self, rect: Rect, out: &mut dyn FnMut(ObjectId, Rect)) {
                out(501, rect);
            }
            fn as_any(&self) -> &dyn crate::compat::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn crate::compat::Any {
                self
            }
        }

        let registry = std::sync::Arc::new(SharedLayoutRegistry::new());
        registry.store(0x5EF4, Box::new(OneChild));
        let other = std::sync::Arc::clone(&registry);
        let placed =
            std::thread::spawn(move || other.apply_if_dirty(0x5EF4, Rect::new(1, 2, 30, 40)))
                .join()
                .expect("the worker thread finishes");
        assert_eq!(placed, Some(vec![(501, Rect::new(1, 2, 30, 40))]));
        assert!(registry.apply_if_dirty(0x5EF4, Rect::new(0, 0, 1, 1)).is_none());
    }
}
