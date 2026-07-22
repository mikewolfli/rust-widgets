//! Space allocation preference used by layout items.

use crate::core::{ObjectId, Point, Rect, Size};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePolicy {
    /// Use fixed size defined by constraints.
    Fixed,
    /// Prefer natural size while allowing negotiation.
    Preferred,
    /// Expand to consume remaining space.
    Expanding,
}
/// Min/max limits applied during layout calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConstraints {
    /// Minimum major-axis size.
    pub min: u32,
    /// Optional maximum major-axis size.
    pub max: Option<u32>,
}
impl LayoutConstraints {
    /// Creates new layout constraints.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }
}
/// Layout context carrying device adaptation parameters.
///
/// Passed to [`Layout::update_with_context`] to enable DPI-aware
/// spacing, margins, and minimum touch-target sizes.
#[derive(Debug, Clone, Copy)]
pub struct LayoutContext {
    /// Scale factor applied to spacing, margins, and padding.
    /// Derived from device DPI and font scale factors.
    pub layout_scale: f32,
    /// Scale factor applied to font/metric sizes.
    pub font_scale: f32,
    /// Minimum touch-target size in logical pixels.
    /// Defaults to 32×32 (recommended minimum for touch).
    pub min_touch_size: Size,
}
impl Default for LayoutContext {
    fn default() -> Self {
        Self { layout_scale: 1.0, font_scale: 1.0, min_touch_size: Size::new(32, 32) }
    }
}

/// Common interface implemented by all layout managers.
pub trait Layout {
    /// Add widget into layout with optional stretch factor.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    /// Remove widget from layout.
    fn remove_widget(&mut self, widget_id: ObjectId);
    /// Recompute child geometries within given rect.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    /// Recompute child geometries from explicit position/size primitives.
    fn update_from_position_size(
        &self,
        position: Point,
        size: Size,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(Rect::from_position_size(position, size), widgets);
    }
    /// Returns all child widget IDs managed by this layout.
    fn child_ids(&self) -> Vec<ObjectId> {
        vec![]
    }

    /// Returns true if the given widget ID is a child of this layout.
    fn has_child(&self, _id: ObjectId) -> bool {
        false
    }

    /// Removes all children from this layout.
    /// Default implementation does nothing (layouts without children).
    fn clear(&mut self) {
        // Default: layouts that don't track children externally are no-ops.
    }

    /// Update child geometries with device-aware scaling context.
    ///
    /// The default implementation ignores the context and delegates
    /// to [`update`](Layout::update). Override to apply
    /// [`LayoutContext::layout_scale`] to spacing, margins, etc.
    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let _ = context;
        self.update(rect, widgets);
    }

    /// Enables downcasting from `dyn Layout` to concrete types.
    /// Required by the layout inspector for introspection.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Enables mutable downcasting from `dyn Layout` to concrete types.
    /// Required for mutation access to concrete layout implementations
    /// through the trait object.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
