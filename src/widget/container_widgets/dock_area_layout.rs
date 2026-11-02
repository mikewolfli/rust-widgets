// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`crate::layout::Layout`] adapter for [`DockArea`] (RW-1).
//!
//! # Why this exists
//!
//! `DockArea` is a layout owner, not a `Widget`: it holds child ids and computes their
//! rectangles, but it is not a `WidgetKind` and has no handle factory. A host that wanted to
//! mount a dock surface had to drive `set_area`/`arrange` by hand and forward every pointer
//! position to the edge-resize gesture itself.
//!
//! This adapter is the missing `Layout` implementation. It owns a `DockArea` (through interior
//! mutability, because `Layout::update` takes `&self`), moves the surface to the rectangle the
//! parent layout hands it, arranges the docks into that rectangle, and forwards the edge-drag
//! entries the same way [`crate::layout::SplitterLayout`] exposes its handles.
//!
//! It deliberately does **not** claim a `WidgetKind`: `DockArea` remains the owner of the
//! arrangement and its `dock_area` capability row, so adding a second kind here would give one
//! concept two identities (rules #23 and #101).

use crate::core::{ObjectId, Point, Rect, Size};
use crate::layout::{ChildInfo, Layout, LayoutContext};
use core::any::Any;
use std::cell::RefCell;

use super::dockwidget::{DockArea, DockWidgetArea};

/// A `Layout` manager that drives a [`DockArea`].
///
/// # `stretch` and dock bands
///
/// A dock band has no stretch weight: every member group of a band receives an equal share,
/// raised to the group's minimum extent. `Layout::add_widget`'s `stretch` argument is therefore
/// applied as the dock's **minimum extent** through [`DockArea::set_dock_min_size`], which is the
/// only per-dock sizing knob the host publishes. A `stretch` of zero leaves the dock's floor
/// untouched.
pub struct DockAreaLayout {
    area: RefCell<DockArea>,
    /// The edge a plain [`Layout::add_widget`] call docks to.
    default_edge: DockWidgetArea,
}

impl DockAreaLayout {
    /// Wraps `area` and docks plain [`Layout::add_widget`] calls to the left edge.
    pub fn new(area: DockArea) -> Self {
        Self { area: RefCell::new(area), default_edge: DockWidgetArea::LeftDockWidgetArea }
    }

    /// Creates an empty dock surface over `rect`.
    pub fn from_rect(rect: Rect) -> Self {
        Self::new(DockArea::new(rect))
    }

    /// The edge [`Layout::add_widget`] uses.
    pub fn default_edge(&self) -> DockWidgetArea {
        self.default_edge
    }

    /// Sets the edge [`Layout::add_widget`] uses.
    ///
    /// Returns `false` for [`DockWidgetArea::NoDockWidgetArea`], which is not a dock target.
    pub fn set_default_edge(&mut self, edge: DockWidgetArea) -> bool {
        if edge == DockWidgetArea::NoDockWidgetArea {
            return false;
        }
        self.default_edge = edge;
        true
    }

    /// Runs `f` with the owned area.
    pub fn with_area<R>(&self, f: impl FnOnce(&DockArea) -> R) -> R {
        f(&self.area.borrow())
    }

    /// Runs `f` with the owned area mutably.
    ///
    /// Use this for operations the layout contract does not model, such as
    /// [`DockArea::tabify`] or [`DockArea::split_dock`]. The mutator runs its own
    /// arrangement through `DockArea::arrange`, so the geometry is applied immediately.
    pub fn with_area_mut<R>(&self, f: impl FnOnce(&mut DockArea) -> R) -> R {
        f(&mut self.area.borrow_mut())
    }

    /// Registers `id` on `edge` and applies `stretch` as its minimum extent.
    ///
    /// Returns `false` when the edge is not a dock target or when `id` is already registered.
    /// The id need not be mounted here: [`DockArea`] stores ids and pushes geometry into the
    /// widget table, so an arrangement can be described before its controls exist.
    pub fn add_dock(&mut self, id: ObjectId, edge: DockWidgetArea, stretch: u32) -> bool {
        if edge == DockWidgetArea::NoDockWidgetArea {
            return false;
        }
        let added = self.area.borrow_mut().add_dock(id, edge);
        if added && stretch > 0 {
            self.area.borrow_mut().set_dock_min_size(id, stretch);
        }
        added
    }

    /// Removes `id` from the surface, returning whether it was registered.
    pub fn remove_dock(&mut self, id: ObjectId) -> bool {
        self.area.borrow_mut().remove_dock(id)
    }

    /// Sets or clears the centre widget.
    pub fn set_central_widget(&self, widget: Option<ObjectId>) {
        self.area.borrow_mut().set_central_widget(widget);
    }

    /// The centre widget, if any.
    pub fn central_widget(&self) -> Option<ObjectId> {
        self.area.borrow().central_widget()
    }

    /// The registered dock ids, in insertion order.
    pub fn dock_ids(&self) -> Vec<ObjectId> {
        self.area.borrow().dock_ids()
    }

    /// The grab strips at the inner boundary of every occupied edge.
    pub fn edge_handle_rects(&self) -> Vec<(DockWidgetArea, Rect)> {
        self.area.borrow().edge_handle_rects()
    }

    /// The occupied edge whose resize handle is under `pos`, if any.
    pub fn edge_handle_at(&self, pos: Point) -> Option<DockWidgetArea> {
        self.area.borrow().edge_handle_at(pos)
    }

    /// Begins an edge-resize gesture at `pos`, returning whether a handle was grabbed.
    pub fn begin_edge_drag(&self, pos: Point) -> bool {
        self.area.borrow_mut().begin_edge_drag(pos)
    }

    /// Moves the grabbed edge so its inner boundary follows `pos`.
    pub fn drag_edge_to(&self, pos: Point) -> bool {
        self.area.borrow_mut().drag_edge_to(pos)
    }

    /// Ends the edge-resize gesture, if one is in progress.
    pub fn end_edge_drag(&self) {
        self.area.borrow_mut().end_edge_drag();
    }

    /// The edge currently being resized, if any.
    pub fn edge_dragging(&self) -> Option<DockWidgetArea> {
        self.area.borrow().edge_dragging()
    }

    /// Applies each child's preferred extent as its dock's minimum, honouring the band axis.
    ///
    /// The hint channel is the one thing `Layout::arrange` carries and `Layout::update` does
    /// not. A left/right band grows along `x`, so the width hint is the relevant one; a
    /// top/bottom band grows along `y`.
    fn apply_child_hints(&self, children: &[ChildInfo]) {
        if children.is_empty() {
            return;
        }
        let mut area = self.area.borrow_mut();
        for child in children {
            let Some(edge) = area.dock_area_of(child.id) else {
                continue;
            };
            let extent = match edge {
                DockWidgetArea::LeftDockWidgetArea | DockWidgetArea::RightDockWidgetArea => {
                    child.hints.width.pref
                }
                DockWidgetArea::TopDockWidgetArea | DockWidgetArea::BottomDockWidgetArea => {
                    child.hints.height.pref
                }
                DockWidgetArea::NoDockWidgetArea => 0,
            };
            if extent > 0 {
                area.set_dock_min_size(child.id, extent);
            }
        }
    }
}

impl Layout for DockAreaLayout {
    fn size_hint(&self) -> Size {
        let area = self.area.borrow().area();
        Size::new(area.width, area.height)
    }

    /// A dock surface consumes whatever room its parent gives it.
    fn expanding_directions(&self) -> (bool, bool) {
        (true, true)
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        let edge = self.default_edge;
        let _ = self.add_dock(widget_id, edge, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        let _ = self.area.borrow_mut().remove_dock(widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let layout = {
            let mut area = self.area.borrow_mut();
            // Every mutator on `DockArea` ends in `arrange`, so a real surface change is
            // arranged by `set_area` itself; an unchanged rectangle needs no second pass.
            area.set_area(rect);
            area.layout()
        };
        for (id, child_rect) in layout {
            widgets(id, child_rect);
        }
    }

    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
        self.apply_child_hints(children);
        self.update(rect, out);
    }

    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        _context: &LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        // Dock bands are driven by the host's explicit edge extents; there is no
        // device-scale term to apply, so the context-aware entry is the plain arrangement.
        self.arrange(rect, children, out);
    }

    fn update_with_context(
        &self,
        rect: Rect,
        _context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(rect, widgets);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        let area = self.area.borrow();
        let mut ids = area.dock_ids();
        if let Some(central) = area.central_widget() {
            ids.push(central);
        }
        ids
    }

    fn has_child(&self, id: ObjectId) -> bool {
        let area = self.area.borrow();
        area.dock_ids().contains(&id) || area.central_widget() == Some(id)
    }

    fn clear(&mut self) {
        let mut area = self.area.borrow_mut();
        for id in area.dock_ids() {
            area.remove_dock(id);
        }
        area.set_central_widget(None);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::container_widgets::dockwidget::DockWidgetArea;

    #[test]
    fn the_layout_moves_the_surface_and_emits_every_dock() {
        let mut layout = DockAreaLayout::from_rect(Rect::new(0, 0, 100, 100));
        assert!(layout.with_area_mut(|area| {
            area.add_dock(1, DockWidgetArea::LeftDockWidgetArea);
            area.add_dock(2, DockWidgetArea::RightDockWidgetArea);
            area.set_central_widget(Some(3));
            true
        }));
        let mut rects = Vec::new();
        layout.update(Rect::new(10, 20, 300, 200), &mut |id, rect| rects.push((id, rect)));
        assert_eq!(rects.len(), 3, "two docks and the centre are placed");
        assert!(layout.with_area(|area| area.area()) == Rect::new(10, 20, 300, 200));
        let left = rects.iter().find(|(id, _)| *id == 1).map(|(_, rect)| *rect);
        assert_eq!(left.map(|rect| rect.x), Some(10), "the left band follows the surface");
        assert_eq!(layout.child_ids(), vec![1, 2, 3]);
        assert!(layout.has_child(3));
        layout.remove_widget(2);
        assert_eq!(layout.child_ids(), vec![1, 3]);
    }

    #[test]
    fn add_widget_docks_to_the_default_edge_and_applies_the_floor() {
        let mut layout = DockAreaLayout::from_rect(Rect::new(0, 0, 200, 100));
        let id = crate::widget::runtime::register(Box::new(
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 10, 10)),
        ))
        .expect("a group box mounts");
        layout.set_default_edge(DockWidgetArea::BottomDockWidgetArea);
        layout.add_widget(id, 30);
        assert_eq!(
            layout.with_area(|area| area.dock_area_of(id)),
            Some(DockWidgetArea::BottomDockWidgetArea)
        );
        assert_eq!(layout.with_area(|area| area.dock_min_size(id)), Some(30));
        layout.clear();
        assert_eq!(layout.child_ids(), Vec::<ObjectId>::new());
        crate::widget::runtime::unregister(id);
    }

    #[test]
    fn the_edge_drag_entries_forward_to_the_area() {
        let layout = DockAreaLayout::from_rect(Rect::new(0, 0, 200, 100));
        assert!(layout.with_area_mut(|area| {
            area.add_dock(1, DockWidgetArea::LeftDockWidgetArea);
            true
        }));
        let handle = layout.edge_handle_rects().into_iter().next().expect("a handle");
        let pos = Point::new(handle.1.x + 1, handle.1.y + 1);
        assert!(layout.begin_edge_drag(pos));
        assert_eq!(layout.edge_dragging(), Some(DockWidgetArea::LeftDockWidgetArea));
        assert!(layout.drag_edge_to(Point::new(50, 10)));
        assert!(
            layout.with_area(|area| area.edge_extent_for(DockWidgetArea::LeftDockWidgetArea)) > 0
        );
        layout.end_edge_drag();
        assert_eq!(layout.edge_dragging(), None);
    }

    #[test]
    fn child_hints_become_dock_floors_on_arrange() {
        use crate::layout::{ChildInfo, Hints};
        let layout = DockAreaLayout::from_rect(Rect::new(0, 0, 300, 200));
        assert!(layout.with_area_mut(|area| {
            area.add_dock(7, DockWidgetArea::LeftDockWidgetArea);
            true
        }));
        let child = ChildInfo::new(7, Hints::at_least(55, 0));
        let mut rects = Vec::new();
        layout.arrange(Rect::new(0, 0, 300, 200), &[child], &mut |id, rect| rects.push((id, rect)));
        assert_eq!(rects.len(), 1);
        assert_eq!(
            layout.with_area(|area| area.dock_min_size(7)),
            Some(55),
            "the hint raises the dock floor"
        );
    }
}
