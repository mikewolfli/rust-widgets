// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Constraint layout manager — positions children using anchor relationships.
//!
//! A simplified constraint-based layout that resolves spatial relationships
//! between widgets sequentially (not a full Cassowary solver). Each constraint
//! references a target widget and describes how this widget relates to it.
use super::Layout;
use crate::compat::{Any, HashMap, Rc, RefCell, Vec};
use crate::core::{ObjectId, Rect};

/// A cached resolution plan: the topological order, that order sorted by priority, and
/// the set of subjects that have at least one constraint.
///
/// All three are functions of the constraint list, which only changes through the
/// add/remove/clear entry points. Caching them keeps `update` from rebuilding a
/// dependency map and re-sorting on every pass (G-157).
#[derive(Debug)]
struct ConstraintOrder {
    /// The application order: a constraint on a target is applied before one that references
    /// it, stably sorted by priority (lowest first).
    ranked: Vec<usize>,
    /// Subjects that carry at least one constraint.
    constrained: HashMap<ObjectId, ()>,
}

/// Describes the type of spatial relationship between two widgets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintType {
    /// Left edge of this widget aligns with left edge of target.
    LeftToLeft,
    /// Left edge of this widget aligns with right edge of target.
    LeftToRight,
    /// Right edge of this widget aligns with left edge of target.
    RightToLeft,
    /// Right edge of this widget aligns with right edge of target.
    RightToRight,
    /// Top edge of this widget aligns with top edge of target.
    TopToTop,
    /// Top edge of this widget aligns with bottom edge of target.
    TopToBottom,
    /// Bottom edge of this widget aligns with top edge of target.
    BottomToTop,
    /// Bottom edge of this widget aligns with bottom edge of target.
    BottomToBottom,
    /// Horizontal center of this widget aligns with horizontal center of target.
    CenterX,
    /// Vertical center of this widget aligns with vertical center of target.
    CenterY,
    /// Width of this widget matches target width.
    Width,
    /// Height of this widget matches target height.
    Height,
    /// Width/height ratio is enforced (width = height * ratio).
    AspectRatio(f32),
}

/// A single constraint entry linking a widget to a target with an offset and multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintRef {
    /// The target widget the constraint refers to.
    pub target_id: ObjectId,
    /// The type of constraint.
    pub constraint: ConstraintType,
    /// Pixel offset applied after resolving the constraint position.
    pub offset: i32,
    /// Scale multiplier applied to the resolved dimension.
    ///
    /// For [`ConstraintType::Width`] and [`ConstraintType::Height`] a non-finite value
    /// (`NaN`, `±∞`) is **ignored**: the constraint leaves the widget rect unchanged rather than
    /// rounding the product to a degenerate size (D09-LAYOUT-03). [`ConstraintType::AspectRatio`]
    /// carries its own ratio and applies the same finite-input rule.
    pub multiplier: f32,
    /// Resolution strength. Constraints are applied in ascending priority so a higher
    /// value wins when two constraints address the same edge.
    ///
    /// This is a deterministic arbitration rule, not a full Cassowary solver: it lets a
    /// caller express "this edge is stronger than that one" without changing the
    /// pull-based resolution model. `0` is the default and preserves declaration order.
    pub priority: i32,
}

/// A constraint-based layout that resolves anchor relationships between widgets.
#[derive(Debug)]
pub struct ConstraintLayout {
    /// All defined constraints stored as (subject_id, ConstraintRef).
    constraints: Vec<(ObjectId, ConstraintRef)>,
    /// The set of child widget IDs tracked by this layout.
    children: Vec<ObjectId>,
    /// Cached topological order of `constraints`.
    ///
    /// The order changes only when a constraint is added or removed, and resolving it is the
    /// expensive part of `update` (a full dependency scan per constraint). `None` means "not
    /// computed"; a cyclic graph is not cached, because its fallback is declaration order and
    /// the warning is part of the observable behaviour.
    order_cache: RefCell<Option<Rc<ConstraintOrder>>>,
}

impl ConstraintLayout {
    /// Create a new empty constraint layout.
    pub fn new() -> Self {
        Self { constraints: Vec::new(), children: Vec::new(), order_cache: RefCell::new(None) }
    }

    /// Adds a constraint with an explicit priority.
    ///
    /// A lower priority is applied first, so a later (higher) priority for the same edge
    /// overrides it. Equal priorities keep declaration order.
    #[allow(clippy::too_many_arguments)]
    pub fn add_constraint_with_priority(
        &mut self,
        widget_id: ObjectId,
        target_id: ObjectId,
        constraint: ConstraintType,
        offset: i32,
        multiplier: f32,
        priority: i32,
    ) {
        self.constraints.push((
            widget_id,
            ConstraintRef { target_id, constraint, offset, multiplier, priority },
        ));
        self.invalidate_order();
    }

    /// Whether the constraint graph contains a cycle.
    ///
    /// A cycle is legal but unresolvable in one pass: the resolution order below breaks
    /// it at the first repeated node and logs a warning, rather than recursing forever.
    pub fn has_constraint_cycle(&self) -> bool {
        self.resolve_order().is_none()
    }

    /// Topologically orders the constraints by their target dependency, or `None` when
    /// the graph has a cycle.
    ///
    /// The result is cached: the constraints only change through the add/remove/clear
    /// entry points, each of which invalidates the cache.
    fn resolve_order(&self) -> Option<Rc<ConstraintOrder>> {
        if let Some(cached) = self.order_cache.borrow().as_ref() {
            return Some(Rc::clone(cached));
        }
        let plan = Rc::new(self.compute_resolve_order()?);
        *self.order_cache.borrow_mut() = Some(Rc::clone(&plan));
        Some(plan)
    }

    /// Drops the cached resolution order after a constraint mutation.
    fn invalidate_order(&mut self) {
        self.order_cache.get_mut().take();
    }

    /// Orders the constraints with Kahn's algorithm.
    ///
    /// The previous body was a depth-first walk that, at every node, scanned the whole
    /// constraint list for the constraints whose subject was that node's target — O(V·E) over
    /// a graph that only changes on add/remove. The reverse index below turns each lookup
    /// into one bucket of successors, so the walk is O(V+E).
    ///
    /// A constraint whose subject is also its own target is ignored, matching the previous
    /// `other != index` guard (such an entry says "this widget relates to itself" and is
    /// resolvable against the already-seeded parent rect).
    fn compute_resolve_order(&self) -> Option<ConstraintOrder> {
        let count = self.constraints.len();
        // subject -> indices of constraints that have that subject.
        let mut by_subject: HashMap<ObjectId, Vec<usize>> = HashMap::new();
        for (index, (subject, _)) in self.constraints.iter().enumerate() {
            by_subject.entry(*subject).or_default().push(index);
        }

        // Edge `dependency -> dependent`: `dependent` references `dependency`'s subject.
        let mut indegree = vec![0usize; count];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (index, (_, constraint)) in self.constraints.iter().enumerate() {
            if let Some(dependencies) = by_subject.get(&constraint.target_id) {
                for &dependency in dependencies {
                    if dependency != index {
                        indegree[index] += 1;
                        dependents[dependency].push(index);
                    }
                }
            }
        }

        // A deque implemented as a vector plus a read cursor: every index is appended once,
        // so the total work is linear.
        let mut queue: Vec<usize> = (0..count).filter(|index| indegree[*index] == 0).collect();
        let mut head = 0usize;
        let mut order = Vec::with_capacity(count);
        while head < queue.len() {
            let index = queue[head];
            head += 1;
            order.push(index);
            for &dependent in &dependents[index] {
                indegree[dependent] -= 1;
                if indegree[dependent] == 0 {
                    queue.push(dependent);
                }
            }
        }
        if order.len() != count {
            return None;
        }

        let mut ranked = order;
        ranked.sort_by_key(|index| self.constraints[*index].1.priority);
        let constrained = self.constraints.iter().map(|(id, _)| (*id, ())).collect();
        Some(ConstraintOrder { ranked, constrained })
    }

    /// Add a constraint linking `widget_id` to `target_id`.
    pub fn add_constraint(
        &mut self,
        widget_id: ObjectId,
        target_id: ObjectId,
        constraint: ConstraintType,
        offset: i32,
        multiplier: f32,
    ) {
        self.constraints.push((
            widget_id,
            ConstraintRef { target_id, constraint, offset, multiplier, priority: 0 },
        ));
        self.invalidate_order();
    }

    /// Remove all constraints associated with the given widget.
    pub fn remove_constraints(&mut self, widget_id: ObjectId) {
        self.constraints.retain(|(id, _)| *id != widget_id);
        self.invalidate_order();
    }

    /// Remove all constraints.
    pub fn clear_constraints(&mut self) {
        self.constraints.clear();
        self.invalidate_order();
    }

    /// Returns all constraints.
    pub fn all_constraints(&self) -> &[(ObjectId, ConstraintRef)] {
        &self.constraints
    }

    /// Resolve the child rect given a parent rect and a set of constraint references.
    ///
    /// The resolution is sequential: constraints referencing earlier widgets
    /// must already have been resolved. Fallback to parent rect when targets
    /// are not yet resolved.
    fn resolve_rect(
        widget_rect: &mut Rect,
        constraint: &ConstraintRef,
        parent_rect: Rect,
        resolved: &HashMap<ObjectId, Rect>,
    ) {
        let target_rect = resolved.get(&constraint.target_id).copied().unwrap_or(parent_rect);

        match constraint.constraint {
            ConstraintType::LeftToLeft => {
                widget_rect.x = target_rect.x + constraint.offset;
            }
            ConstraintType::LeftToRight => {
                widget_rect.x =
                    target_rect.x.saturating_add_unsigned(target_rect.width) + constraint.offset;
            }
            ConstraintType::RightToLeft => {
                // Align widget right edge to target LEFT edge
                let left_edge = target_rect.x;
                widget_rect.x =
                    left_edge.saturating_sub_unsigned(widget_rect.width) + constraint.offset;
            }
            ConstraintType::RightToRight => {
                // Align widget right edge to target RIGHT edge
                let right_edge = target_rect.x.saturating_add_unsigned(target_rect.width);
                widget_rect.x =
                    right_edge.saturating_sub_unsigned(widget_rect.width) + constraint.offset;
            }
            ConstraintType::TopToTop => {
                widget_rect.y = target_rect.y + constraint.offset;
            }
            ConstraintType::TopToBottom => {
                widget_rect.y =
                    target_rect.y.saturating_add_unsigned(target_rect.height) + constraint.offset;
            }
            ConstraintType::BottomToTop => {
                // Align widget bottom edge to target TOP edge
                let top_edge = target_rect.y;
                widget_rect.y =
                    top_edge.saturating_sub_unsigned(widget_rect.height) + constraint.offset;
            }
            ConstraintType::BottomToBottom => {
                // Align widget bottom edge to target BOTTOM edge
                let bottom_edge = target_rect.y.saturating_add_unsigned(target_rect.height);
                widget_rect.y =
                    bottom_edge.saturating_sub_unsigned(widget_rect.height) + constraint.offset;
            }
            ConstraintType::CenterX => {
                let target_center_x =
                    target_rect.x.saturating_add((target_rect.width as i32).saturating_div(2));
                widget_rect.x = target_center_x
                    .saturating_sub((widget_rect.width as i32).saturating_div(2))
                    .saturating_add(constraint.offset);
            }
            ConstraintType::CenterY => {
                let target_center_y =
                    target_rect.y.saturating_add((target_rect.height as i32).saturating_div(2));
                widget_rect.y = target_center_y
                    .saturating_sub((widget_rect.height as i32).saturating_div(2))
                    .saturating_add(constraint.offset);
            }
            ConstraintType::Width => {
                // D09-LAYOUT-03: a non-finite multiplier cannot describe a scale, so it is
                // ignored rather than silently rounded to a degenerate size. Without this guard
                // `NaN.round() as i32` becomes `0` and `±∞`/over-range products saturate to the
                // `i32` bounds, either of which changes the widget geometry. This mirrors the
                // `AspectRatio` branch below, which already rejects a non-finite factor
                // (principle #50: degenerate input is ignored, not coerced).
                if !constraint.multiplier.is_finite() {
                    return;
                }
                let w = (target_rect.width as f32 * constraint.multiplier).round() as i32;
                widget_rect.width = w.max(0) as u32;
            }
            ConstraintType::Height => {
                // D09-LAYOUT-03: same finite-multiplier policy as the `Width` branch above.
                if !constraint.multiplier.is_finite() {
                    return;
                }
                let h = (target_rect.height as f32 * constraint.multiplier).round() as i32;
                widget_rect.height = h.max(0) as u32;
            }
            ConstraintType::AspectRatio(ratio) => {
                // A zero/non-finite ratio cannot describe an aspect, so it leaves the
                // rect unchanged (principle #50: degenerate input is ignored).
                if !(ratio.is_finite() && ratio > 0.0) {
                    return;
                }
                // Width = height * ratio, constrained to fit within widget rect.
                let w = (widget_rect.height as f32 * ratio).round() as i32;
                if (widget_rect.width as i32 - w).abs() > 2 {
                    widget_rect.width = w.max(0) as u32;
                } else {
                    let h = (widget_rect.width as f32 / ratio).round() as i32;
                    widget_rect.height = h.max(0) as u32;
                }
            }
        }
    }
}

crate::impl_default_via_new!(ConstraintLayout);

impl Layout for ConstraintLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// Drops the cached resolution order, so the next `update` recomputes it from the
    /// current constraint graph.
    fn invalidate(&mut self) {
        self.invalidate_order();
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        if !self.children.contains(&widget_id) {
            self.children.push(widget_id);
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|id| *id != widget_id);
        self.remove_constraints(widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.clone()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.contains(&id)
    }

    fn widget_count(&self) -> usize {
        self.children.len()
    }

    fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.children.iter().position(|child| *child == id)
    }

    fn clear(&mut self) {
        self.children.clear();
        self.constraints.clear();
        self.invalidate_order();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        if self.constraints.is_empty() {
            // No constraints: just lay out children filling the parent.
            for &child_id in &self.children {
                widgets(child_id, rect);
            }
            return;
        }

        // Resolve constraints sequentially. Start with a default rect for each child.
        // The map is keyed by id so each constraint's target lookup is a hash lookup rather
        // than a linear scan of the partially resolved vector.
        let mut resolved: HashMap<ObjectId, Rect> = HashMap::new();

        // First pass: seed resolved with parent rect for each child.
        for &child_id in &self.children {
            resolved.insert(child_id, rect);
        }

        // Apply constraints in dependency order, lowest priority first, so a stronger
        // constraint is the one that survives on a shared edge. A cycle is broken by
        // falling back to declaration order; the layout still terminates.
        let plan = self.resolve_order();
        let (ranked, constrained): (Vec<usize>, HashMap<ObjectId, ()>) = match plan {
            Some(plan) => {
                // `ranked` borrows the cached vectors but is consumed below; cloning it is
                // cheaper than re-building the dependency graph and re-sorting (G-157).
                (plan.ranked.clone(), plan.constrained.clone())
            }
            None => {
                log::warn!(
                    "constraint layout has a dependency cycle; resolving in declaration order"
                );
                let mut ranked: Vec<usize> = (0..self.constraints.len()).collect();
                ranked.sort_by_key(|index| self.constraints[*index].1.priority);
                let constrained = self.constraints.iter().map(|(id, _)| (*id, ())).collect();
                (ranked, constrained)
            }
        };
        for index in ranked {
            let (subject_id, constraint_ref) = &self.constraints[index];
            if let Some(mut widget_rect) = resolved.get(subject_id).copied() {
                Self::resolve_rect(&mut widget_rect, constraint_ref, rect, &resolved);
                resolved.insert(*subject_id, widget_rect);
            }
        }

        // Second pass: for children that were referenced but have no constraint
        // defined, still emit them at the parent rect position.
        for &child_id in &self.children {
            if let Some(widget_rect) = resolved.get(&child_id).copied() {
                // If the child has no constraints, its rect will still be the parent rect.
                // Only emit if it has at least one constraint (otherwise we'd double-emit rect-sized widgets).
                if constrained.contains_key(&child_id) {
                    widgets(child_id, widget_rect);
                } else {
                    // No constraints for this child: fill parent.
                    widgets(child_id, rect);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    #[test]
    fn test_left_to_left_alignment() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        // Widget 2 left edge aligns with widget 1 left edge, offset +10
        layout.add_constraint(2, 1, ConstraintType::LeftToLeft, 10, 1.0);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Widget 2 x should be widget 1 x + 10 = 0 + 10 = 10
        assert_eq!(rects.get(&2).map(|r| r.x), Some(10));
    }

    #[test]
    fn test_left_to_right_places_beside() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_constraint(1, 2, ConstraintType::LeftToLeft, 0, 1.0);

        // Child 1 at x=0, width=200 (parent rect). Child 2 at x=child1.right + 10
        // Remove constraint on 1 and add a proper one
        layout.remove_constraints(1);
        layout.add_constraint(1, 2, ConstraintType::LeftToRight, 10, 1.0);

        let mut rects = HashMap::new();
        layout.update(Rect::new(5, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Widget 1 left = widget 2.right + 10. Since widget 2 has no constraint,
        // widget 2 fills parent at x=5, width=200. So widget 2 right = 205.
        // Widget 1 x = 205 + 10 = 215.
        // But widget 2 without constraint gets parent rect (5, 0, 200, 100), so right = 205.
        assert_eq!(rects.get(&1).map(|r| r.x), Some(215));
    }

    #[test]
    fn test_center_x_alignment() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        // Widget 2 center aligns with widget 1 center.
        // Widget 1 is at parent rect (0,0,200,100), center x = 100.
        // Widget 2 has width = 200 (parent), center x = 200/2 = 100, so x stays 0.
        layout.add_constraint(2, 1, ConstraintType::CenterX, 0, 1.0);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Widget 2 center = widget 1 center = 100. Widget 2 x = 100 - 100 = 0.
        assert_eq!(rects.get(&2).map(|r| r.x), Some(0));
    }

    #[test]
    fn test_top_to_bottom() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        // Widget 2 top aligns with widget 1 bottom.
        layout.add_constraint(2, 1, ConstraintType::TopToBottom, 5, 1.0);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Widget 1 is at (0,0,200,100) (parent rect). Widget 2 y = 100 + 5 = 105.
        assert_eq!(rects.get(&2).map(|r| r.y), Some(105));
    }

    #[test]
    fn test_remove_constraints_clears_relationship() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_constraint(2, 1, ConstraintType::LeftToRight, 0, 1.0);
        layout.remove_constraints(2);

        assert!(layout.all_constraints().is_empty());
    }

    #[test]
    fn test_width_aspect_ratio_constraint() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        // Widget 1 has aspect ratio 2.0 (width = height * 2).
        // In parent rect (0,0,200,100), width=200, height=100.
        // Aspect ratio = 2.0, current ratio = 200/100 = 2.0, no change.
        layout.add_constraint(1, 1, ConstraintType::AspectRatio(2.0), 0, 1.0);

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        // width = height * 2 = 100 * 2 = 200, height stays 100.
        assert_eq!(rects.get(&1).map(|r| r.width), Some(200));
        assert_eq!(rects.get(&1).map(|r| r.height), Some(100));
    }

    #[test]
    fn test_clear_removes_all_children_and_constraints() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_constraint(2, 1, ConstraintType::LeftToRight, 0, 1.0);
        layout.clear();

        assert!(layout.child_ids().is_empty());
        assert!(layout.all_constraints().is_empty());
    }

    #[test]
    fn test_has_child_and_remove_widget() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(42, 0);
        assert!(layout.has_child(42));
        layout.remove_widget(42);
        assert!(!layout.has_child(42));
    }

    // ── D09-LAYOUT-03: Width/Height multiplier must be finite ──

    /// Runs a `Width` constraint with the given multiplier and returns the resolved width for
    /// widget `1`, whose target shares the same parent rect. The parent is 200x100, so a valid
    /// multiplier of `1.0` leaves the width at 200.
    fn width_for(multiplier: f32) -> u32 {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_constraint(1, 1, ConstraintType::Width, 0, multiplier);
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });
        rects.get(&1).map(|r| r.width).expect("widget placed")
    }

    /// Runs a `Height` constraint with the given multiplier; a valid `1.0` leaves the height at
    /// the parent's 100.
    fn height_for(multiplier: f32) -> u32 {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_constraint(1, 1, ConstraintType::Height, 0, multiplier);
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 200, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });
        rects.get(&1).map(|r| r.height).expect("widget placed")
    }

    /// A non-finite multiplier must not silently shrink the widget to zero (NaN) or saturate it
    /// to an extreme size (∞); the constraint is ignored and the rect keeps the parent size.
    #[test]
    fn width_non_finite_multiplier_is_ignored() {
        assert_eq!(width_for(f32::NAN), 200, "NaN must be ignored, not rounded to 0");
        assert_eq!(width_for(f32::INFINITY), 200, "+∞ must be ignored, not saturated");
        assert_eq!(width_for(f32::NEG_INFINITY), 200, "-∞ must be ignored, not saturated");
    }

    /// The `Height` branch shares the finite-multiplier policy (D09-LAYOUT-03).
    #[test]
    fn height_non_finite_multiplier_is_ignored() {
        assert_eq!(height_for(f32::NAN), 100, "NaN must be ignored, not rounded to 0");
        assert_eq!(height_for(f32::INFINITY), 100, "+∞ must be ignored, not saturated");
        assert_eq!(height_for(f32::NEG_INFINITY), 100, "-∞ must be ignored, not saturated");
    }

    /// Normal and huge-but-finite multipliers still scale the dimension, clamped at the `i32`
    /// boundary rather than wrapping to a negative width (D09-LAYOUT-03).
    #[test]
    fn width_finite_multipliers_scale_within_bounds() {
        assert_eq!(width_for(1.0), 200, "1.0 leaves the width unchanged");
        assert_eq!(width_for(0.5), 100, "0.5 halves the width");
        assert_eq!(width_for(0.0), 0, "0.0 collapses the width to zero, not negative");
        // A huge finite multiplier overflows `i32` when rounded; the non-negative clamp keeps it
        // from wrapping into a negative width that casts back to an enormous `u32`.
        let huge = width_for(f32::MAX);
        assert!(huge <= i32::MAX as u32, "a huge multiplier must saturate, not wrap: {huge}");
    }

    /// The `Height` branch scales finite multipliers identically.
    #[test]
    fn height_finite_multipliers_scale_within_bounds() {
        assert_eq!(height_for(1.0), 100);
        assert_eq!(height_for(0.5), 50);
        assert_eq!(height_for(0.0), 0);
        let huge = height_for(f32::MAX);
        assert!(huge <= i32::MAX as u32, "a huge multiplier must saturate, not wrap: {huge}");
    }

    /// A cycle is detected instead of recursing, and priority decides which constraint
    /// wins when two rules address the same edge.
    #[test]
    fn cycles_are_detected_and_priority_arbitrates() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        layout.add_constraint(1, 2, ConstraintType::LeftToRight, 0, 1.0);
        layout.add_constraint(2, 1, ConstraintType::LeftToRight, 0, 1.0);
        assert!(layout.has_constraint_cycle());
        let mut placed = Vec::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| placed.push((id, rect)));
        assert_eq!(placed.len(), 2, "a cycle must not hang or drop children");

        let mut ranked = ConstraintLayout::new();
        ranked.add_widget(1, 0);
        ranked.add_constraint_with_priority(1, 0, ConstraintType::LeftToLeft, 10, 1.0, 0);
        ranked.add_constraint_with_priority(1, 0, ConstraintType::LeftToLeft, 40, 1.0, 5);
        let mut placed = Vec::new();
        ranked.update(Rect::new(0, 0, 100, 100), &mut |id, rect| placed.push((id, rect)));
        assert_eq!(placed[0].1.x, 40, "the higher-priority rule wins");
    }
}
