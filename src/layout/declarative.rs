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
//! The thread-local `LAYOUT_MAP` is the one registry: a caller that applies a layout directly
//! (the JSON loader, the C ABI, a panel) reaches the same store the window resize path uses.
//! The identity/diff side of G-406 lives in `view/engine` and is reached by the JSON front end
//! through `json::JsonViewSession`.

use crate::compat::{Box, HashMap, Vec};
use core::cell::RefCell;

use crate::core::{ObjectId, Rect};
use crate::layout::{ArrangeReport, ChildInfo, Layout, LayoutContext};

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
    // A window root that has just been given a layout has never had it applied, and the window
    // path skips a pass whose client size and scale are unchanged (LY3-01). Dropping that record
    // is what makes a layout installed at an unchanged size take effect immediately.
    #[cfg(full_widgets)]
    crate::app::handle::invalidate_applied_layout(parent_id);
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

/// Places `child_id` at an explicit absolute position of the layout stored for `parent_id`.
///
/// This is the declarative door to [`AbsoluteLayout`](crate::layout::absolute::AbsoluteLayout):
/// a `"absolute"` layout document carries one position per child
/// (`x`/`y`/`width`/`height`/`anchor`), and this call stores it. The child must already be a
/// member (the caller's `add_widget_to_layout` does that), so this only replaces the position.
///
/// Returns `true` when the stored layout is an absolute layout and holds the child.
pub fn place_widget_in_absolute_layout(
    child_id: ObjectId,
    x: i32,
    y: i32,
    width: Option<u32>,
    height: Option<u32>,
    anchor: Option<crate::layout::absolute::Anchor>,
    parent_id: ObjectId,
) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        let Some(layout) = map.get_mut(&parent_id) else {
            return false;
        };
        match layout.as_any_mut().downcast_mut::<crate::layout::absolute::AbsoluteLayout>() {
            Some(absolute) => {
                let mut position = crate::layout::absolute::AbsolutePosition::new(x, y);
                position.width = width;
                position.height = height;
                if let Some(anchor) = anchor {
                    position.anchor = anchor;
                }
                absolute.set_position_for(child_id, position)
            }
            None => false,
        }
    })
}

/// One declarative constraint, with its target already resolved to a control id.
///
/// The JSON reader resolves the document's `target` name through its own name map, because the
/// layout module has no view of the document; this type is the resolved form the layout accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConstraintSpec {
    /// The control the relationship refers to.
    pub target: ObjectId,
    /// The spatial relationship.
    pub kind: crate::layout::constraint::ConstraintType,
    /// Pixel offset applied after resolution.
    pub offset: i32,
    /// Scale multiplier (also the ratio for `AspectRatio`).
    pub multiplier: f32,
    /// Resolution strength; higher wins on a shared edge.
    pub priority: i32,
}

/// Records `child_id`'s resolved constraints on the layout stored for `parent_id`.
///
/// Returns `true` when the stored layout is a
/// [`ConstraintLayout`](crate::layout::constraint::ConstraintLayout) and the child is a member.
/// Existing constraints for the child are replaced, so re-applying a declaration is idempotent.
pub fn apply_constraints_to_layout(
    child_id: ObjectId,
    specs: &[LayoutConstraintSpec],
    parent_id: ObjectId,
) -> bool {
    LAYOUT_MAP.with(|map| {
        let mut map = map.borrow_mut();
        let Some(layout) = map.get_mut(&parent_id) else {
            return false;
        };
        let Some(constraints) =
            layout.as_any_mut().downcast_mut::<crate::layout::constraint::ConstraintLayout>()
        else {
            return false;
        };
        if !constraints.has_child(child_id) {
            return false;
        }
        constraints.remove_constraints(child_id);
        for spec in specs {
            constraints.add_constraint_with_priority(
                child_id,
                spec.target,
                spec.kind,
                spec.offset,
                spec.multiplier,
                spec.priority,
            );
        }
        true
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
    //
    // One device context for the whole solve, constructed at the same single point the window
    // and panel paths use (LY-03). The document path used to call `Layout::update`, so a layout
    // that honours `LayoutContext` was asked differently here than one level up.
    let context = LayoutContext::default();
    apply_layout_in_context(parent_id, rect, &context)
}

/// [`apply_layout`] under a device context the caller already holds (LY3-04).
///
/// The window path builds one context for its pass and hands it down, so a declarative root no
/// longer pays a second platform text-scale query for the same resize. The traversal, the depth
/// bound and the spacer filter are the ones [`apply_layout_bounded`] owns; this is only the
/// entry point that keeps the context instance shared.
///
/// # Why this name
///
/// [`apply_layout_with_context`] already exists and means something different: it arranges a
/// caller-supplied child list (the inspector/hint path), where this one applies the layout the
/// parent already has.
pub(crate) fn apply_layout_in_context(
    parent_id: ObjectId,
    rect: Rect,
    context: &LayoutContext,
) -> Vec<(ObjectId, Rect)> {
    const MAX_LAYOUT_DEPTH: u32 = 64;
    apply_layout_bounded(parent_id, rect, context, 0, MAX_LAYOUT_DEPTH)
}

fn apply_layout_bounded(
    parent_id: ObjectId,
    rect: Rect,
    context: &LayoutContext,
    depth: u32,
    max_depth: u32,
) -> Vec<(ObjectId, Rect)> {
    // The returned list is this call's result, so it is owned here rather than borrowed from the
    // scratch pool. Nested levels (below) reuse a pooled buffer instead of allocating one per
    // declarative container (RD4-02).
    let mut geometries = Vec::new();
    apply_node(parent_id, rect, context, depth, max_depth, &mut geometries);
    geometries
}

/// Solves one declarative container into `out` and applies the result recursively.
///
/// The traversal, the depth bound and the spacer filter are unchanged; only the buffer that
/// collects this level's rectangles is supplied by the caller, so a nested level can hand it a
/// pooled scratch list instead of allocating.
fn apply_node(
    parent_id: ObjectId,
    rect: Rect,
    context: &LayoutContext,
    depth: u32,
    max_depth: u32,
    out: &mut Vec<(ObjectId, Rect)>,
) {
    // The recursion below is compiled out with `set_widget_geometry` on alloc-frugal
    // builds (there is no registry to move), so the two traversal parameters have no
    // reader there. Naming them as intentionally unused keeps `mini` warning-free.
    #[cfg(alloc_frugal)]
    let _ = (depth, max_depth);
    LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return;
        };
        layout.update_with_context(rect, context, &mut |child_id, child_rect| {
            if child_id != SPACER_ID {
                out.push((child_id, child_rect));
            }
        });
    });

    // `set_widget_geometry` is compiled out in the alloc-frugal profile (there is no
    // widget runtime to address), so there is nothing to move the geometries onto.
    // The computed list is still returned: a caller in that profile can use it to
    // place whatever it draws, which is the honest answer rather than a silent no-op.
    #[cfg(not(alloc_frugal))]
    for (child_id, child_rect) in out.iter() {
        crate::set_widget_geometry(
            *child_id,
            child_rect.x,
            child_rect.y,
            child_rect.width,
            child_rect.height,
        );
        if depth < max_depth && has_layout(*child_id) {
            // The pool exists only in a full widget build; elsewhere a nested level simply
            // allocates its list, which is the behaviour this module started with.
            #[cfg(all(full_widgets, not(alloc_frugal)))]
            {
                let mut nested = crate::layout::scratch::take_geometry_scratch();
                apply_node(*child_id, *child_rect, context, depth + 1, max_depth, &mut nested);
                crate::layout::scratch::return_geometry_scratch(nested);
            }
            #[cfg(not(all(full_widgets, not(alloc_frugal))))]
            {
                let mut nested = Vec::new();
                apply_node(*child_id, *child_rect, context, depth + 1, max_depth, &mut nested);
            }
        } else if depth >= max_depth && has_layout(*child_id) {
            log::warn!(
                "declarative layout for widget {parent_id} exceeded the {max_depth}-level nesting \
                 bound; child {child_id}'s layout was not applied"
            );
        }
        // A panel layout nested inside a document container must follow the box the document
        // just gave it. The declaration walk used to consult only `LAYOUT_MAP`, so a panel
        // placed by a document kept its children at their old geometry (LY-02). The shared
        // bounded entry point is the panel layer's own recursion, so both paths use one depth
        // semantics and cannot disagree about the bound.
        #[cfg(full_widgets)]
        if depth < max_depth && crate::app::handle::widget_hosts_a_layout(*child_id) {
            crate::app::handle::apply_panel_geometry_bounded(
                *child_id,
                *child_rect,
                context,
                depth + 1,
                max_depth,
            );
        }
    }
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
/// The thread-local counterpart of the context-aware arrangement used by the window and panel
/// paths. It exists so a caller that has the child hints — the view engine, or a test — can run
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
                // RW-48: the inspector is fed from the one place child rectangles become
                // widget state, so a caller no longer has to remember to record them.
                crate::layout::inspector::LayoutInspector::record_geometry(child_id, child_rect);
                geometries.push((child_id, child_rect));
            }
        });
        geometries
    })
}

/// Recomputes `parent_id`'s geometry through the hint and device-context channels (RW-49).
///
/// # Why this exists
///
/// [`apply_layout_with_hints`] runs `Layout::arrange`, which cannot see a [`LayoutContext`]. A
/// layout that scales (flex, border, box) therefore produced one geometry in a design preview
/// and another through `update_with_context`, and the two disagreed only when the scale was not
/// 1 — the case a high-DPI host always runs in. This entry runs `arrange_with_context` on the
/// stored layout, so a preview and the runtime read the same scale and touch floor.
///
/// The spacer is filtered for the same reason [`apply_layout`] filters it, and every child
/// rectangle is recorded with [`LayoutInspector::record_geometry`](crate::layout::inspector::LayoutInspector::record_geometry)
/// (a no-op unless the inspector is enabled), which is the automatic wiring RW-48 asks for.
pub fn apply_layout_with_context(
    parent_id: ObjectId,
    rect: Rect,
    children: &[ChildInfo],
    context: &LayoutContext,
) -> Vec<(ObjectId, Rect)> {
    apply_layout_with_context_reported(parent_id, rect, children, context).0
}

/// [`apply_layout_with_context`] with the solver's clamp/overflow report (RW-48).
pub fn apply_layout_with_context_reported(
    parent_id: ObjectId,
    rect: Rect,
    children: &[ChildInfo],
    context: &LayoutContext,
) -> (Vec<(ObjectId, Rect)>, ArrangeReport) {
    LAYOUT_MAP.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&parent_id) else {
            return (Vec::new(), ArrangeReport::default());
        };
        let mut raw: Vec<(ObjectId, Rect)> = Vec::new();
        layout.arrange_with_context(rect, children, context, &mut |child_id, child_rect| {
            if child_id != SPACER_ID {
                crate::layout::inspector::LayoutInspector::record_geometry(child_id, child_rect);
            }
            raw.push((child_id, child_rect));
        });
        let geometries: Vec<(ObjectId, Rect)> =
            raw.iter().copied().filter(|(id, _)| *id != SPACER_ID).collect();
        let report = ArrangeReport::from_placements(rect, children, &geometries);
        (geometries, report)
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

    /// RW-49: the context-aware entry reaches `arrange_with_context`, so a scaled layout previews
    /// exactly as its runtime path lays out. RW-48: the reported twin names the clamp.
    #[test]
    fn applying_with_context_runs_the_context_aware_arrange_path() {
        use crate::layout::{Hints, LayoutContext};

        /// A one-child layout whose geometry is a function of the context's layout scale.
        struct ScaleProbe;
        impl Layout for ScaleProbe {
            fn add_widget(&mut self, _widget_id: ObjectId, _stretch: u32) {}
            fn remove_widget(&mut self, _widget_id: ObjectId) {}
            fn update(&self, rect: Rect, out: &mut dyn FnMut(ObjectId, Rect)) {
                out(1, Rect::new(rect.x, rect.y, 10, 10));
            }
            fn update_with_context(
                &self,
                rect: Rect,
                context: &LayoutContext,
                out: &mut dyn FnMut(ObjectId, Rect),
            ) {
                let width = (10.0 * context.layout_scale) as u32;
                out(1, Rect::new(rect.x, rect.y, width, 10));
            }
            fn arrange(
                &self,
                rect: Rect,
                _children: &[ChildInfo],
                out: &mut dyn FnMut(ObjectId, Rect),
            ) {
                out(1, Rect::new(rect.x, rect.y, 10, 10));
            }
            fn arrange_with_context(
                &self,
                rect: Rect,
                _children: &[ChildInfo],
                context: &LayoutContext,
                out: &mut dyn FnMut(ObjectId, Rect),
            ) {
                let width = (10.0 * context.layout_scale) as u32;
                out(1, Rect::new(rect.x, rect.y, width, 10));
            }
            fn as_any(&self) -> &dyn crate::compat::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn crate::compat::Any {
                self
            }
        }

        const PARENT: ObjectId = 0x5EF5;
        store_layout(PARENT, Box::new(ScaleProbe));
        let children = [ChildInfo::new(1, Hints::fixed(10, 10))];
        let rect = Rect::new(0, 0, 100, 20);

        let unscaled = apply_layout_with_hints(PARENT, rect, &children);
        assert_eq!(unscaled[0].1.width, 10);

        let context =
            LayoutContext { layout_scale: 2.0, font_scale: 1.0, ..LayoutContext::default() };
        let scaled = apply_layout_with_context(PARENT, rect, &children, &context);
        assert_eq!(
            scaled[0].1.width, 20,
            "the context entry must run `arrange_with_context`, not `arrange`"
        );

        let (reported, report) = apply_layout_with_context_reported(
            PARENT,
            rect,
            &[ChildInfo::new(1, Hints::fixed(40, 10))],
            &context,
        );
        assert_eq!(reported[0].1.width, 20);
        assert!(!report.is_clean(), "a 20 px child under a 40 px floor is clamped");
        assert_eq!(report.clamped_children(), 1);

        assert!(forget_layout(PARENT));
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
