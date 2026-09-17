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

use crate::compat::HashMap;
use core::cell::RefCell;

use crate::core::{ObjectId, Rect};
use crate::layout::Layout;

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
}

/// Removes the layout stored for `parent_id`.
///
/// Returns `true` when there was one. Unmounting a container should call this, or the
/// registry keeps a layout whose children no longer exist and whose ids may later be
/// reused.
pub fn forget_layout(parent_id: ObjectId) -> bool {
    LAYOUT_MAP.with(|map| map.borrow_mut().remove(&parent_id).is_some())
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
}
