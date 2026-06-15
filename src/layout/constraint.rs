//! Constraint layout manager — positions children using anchor relationships.
//!
//! A simplified constraint-based layout that resolves spatial relationships
//! between widgets sequentially (not a full Cassowary solver). Each constraint
//! references a target widget and describes how this widget relates to it.
use super::Layout;
use crate::core::{ObjectId, Rect};

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
    pub multiplier: f32,
}

/// A constraint-based layout that resolves anchor relationships between widgets.
#[derive(Debug)]
pub struct ConstraintLayout {
    /// All defined constraints stored as (subject_id, ConstraintRef).
    constraints: Vec<(ObjectId, ConstraintRef)>,
    /// The set of child widget IDs tracked by this layout.
    children: Vec<ObjectId>,
}

impl ConstraintLayout {
    /// Create a new empty constraint layout.
    pub fn new() -> Self {
        Self { constraints: Vec::new(), children: Vec::new() }
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
        self.constraints
            .push((widget_id, ConstraintRef { target_id, constraint, offset, multiplier }));
    }

    /// Remove all constraints associated with the given widget.
    pub fn remove_constraints(&mut self, widget_id: ObjectId) {
        self.constraints.retain(|(id, _)| *id != widget_id);
    }

    /// Remove all constraints.
    pub fn clear_constraints(&mut self) {
        self.constraints.clear();
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
        resolved: &[(ObjectId, Rect)],
    ) {
        let target_rect = resolved
            .iter()
            .find(|(id, _)| *id == constraint.target_id)
            .map(|(_, r)| *r)
            .unwrap_or(parent_rect);

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
                let target_center_x = target_rect.x + (target_rect.width as i32) / 2;
                widget_rect.x =
                    target_center_x - (widget_rect.width as i32) / 2 + constraint.offset;
            }
            ConstraintType::CenterY => {
                let target_center_y = target_rect.y + (target_rect.height as i32) / 2;
                widget_rect.y =
                    target_center_y - (widget_rect.height as i32) / 2 + constraint.offset;
            }
            ConstraintType::Width => {
                let w = (target_rect.width as f32 * constraint.multiplier) as i32;
                widget_rect.width = w.max(0) as u32;
            }
            ConstraintType::Height => {
                let h = (target_rect.height as f32 * constraint.multiplier) as i32;
                widget_rect.height = h.max(0) as u32;
            }
            ConstraintType::AspectRatio(ratio) => {
                // Width = height * ratio, constrained to fit within widget rect.
                let w = (widget_rect.height as f32 * ratio) as i32;
                if (widget_rect.width as i32 - w).abs() > 2 {
                    widget_rect.width = w.max(0) as u32;
                } else {
                    let h = (widget_rect.width as f32 / ratio) as i32;
                    widget_rect.height = h.max(0) as u32;
                }
            }
        }
    }
}

impl Default for ConstraintLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout for ConstraintLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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

    fn clear(&mut self) {
        self.children.clear();
        self.constraints.clear();
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
        // Collect only widget IDs that have constraints.
        let mut resolved: Vec<(ObjectId, Rect)> = Vec::new();

        // First pass: seed resolved with parent rect for each constrained child.
        for &child_id in &self.children {
            resolved.push((child_id, rect));
        }

        // Apply constraints in order. Each constraint references a target that may
        // have been positioned earlier in the constraints list.
        for (subject_id, constraint_ref) in &self.constraints {
            if let Some(pos) = resolved.iter().position(|(id, _)| id == subject_id) {
                let mut widget_rect = resolved[pos].1;
                Self::resolve_rect(&mut widget_rect, constraint_ref, rect, &resolved);
                resolved[pos].1 = widget_rect;
            }
        }

        // Second pass: for children that were referenced but have no constraint
        // defined, still emit them at the parent rect position.
        for &child_id in &self.children {
            if let Some(pos) = resolved.iter().position(|(id, _)| *id == child_id) {
                // If the child has no constraints, its rect will still be the parent rect.
                // Only emit if it has at least one constraint (otherwise we'd double-emit rect-sized widgets).
                let has_constraint = self.constraints.iter().any(|(id, _)| *id == child_id);
                if has_constraint {
                    widgets(child_id, resolved[pos].1);
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

    #[test]
    fn test_left_to_left_alignment() {
        let mut layout = ConstraintLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        // Widget 2 left edge aligns with widget 1 left edge, offset +10
        layout.add_constraint(2, 1, ConstraintType::LeftToLeft, 10, 1.0);

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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

        let mut rects = std::collections::HashMap::new();
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
}
