//! Keyboard-aware layout manager — shifts content upward when the mobile keyboard appears,
//! preventing the focused input from being obscured.
use super::{Layout, LayoutContext};
use crate::core::{ObjectId, Rect};

/// A layout wrapper that shifts its children upward by `keyboard_offset` pixels.
///
/// This is useful on mobile platforms where the virtual keyboard
/// occludes the bottom portion of the screen. When `set_keyboard_offset`
/// is called with the keyboard height, all child positions are adjusted
/// so that the content remains visible.
pub struct KeyboardAwareLayout {
    /// Inner layout that performs the actual child positioning.
    inner: Box<dyn Layout>,
    /// Current keyboard height in pixels. 0 = keyboard hidden.
    keyboard_offset: i32,
    /// Duration of the offset animation in milliseconds.
    animation_duration: u64,
}

impl std::fmt::Debug for KeyboardAwareLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardAwareLayout")
            .field("keyboard_offset", &self.keyboard_offset)
            .field("animation_duration", &self.animation_duration)
            .field("inner", &"<dyn Layout>")
            .finish()
    }
}

impl KeyboardAwareLayout {
    /// Create a keyboard-aware layout wrapping an inner layout.
    ///
    /// * `inner` – The layout that manages child positions.
    /// * `animation_duration` – Animation duration in milliseconds.
    pub fn new(inner: Box<dyn Layout>, animation_duration: u64) -> Self {
        Self { inner, keyboard_offset: 0, animation_duration }
    }

    /// Set the current keyboard offset (height of the visible keyboard).
    ///
    /// Pass `0` to indicate the keyboard is hidden.
    pub fn set_keyboard_offset(&mut self, offset: i32) {
        self.keyboard_offset = offset;
    }

    /// Returns the current keyboard offset.
    pub fn keyboard_offset(&self) -> i32 {
        self.keyboard_offset
    }

    /// Returns the animation duration in milliseconds.
    pub fn animation_duration(&self) -> u64 {
        self.animation_duration
    }

    /// Sets the animation duration.
    pub fn set_animation_duration(&mut self, duration: u64) {
        self.animation_duration = duration;
    }

    /// Returns a shared reference to the inner layout.
    pub fn inner_layout(&self) -> &dyn Layout {
        self.inner.as_ref()
    }

    /// Returns a mutable reference to the inner layout.
    pub fn inner_layout_mut(&mut self) -> &mut dyn Layout {
        self.inner.as_mut()
    }

    /// Consumes this layout and returns the inner layout.
    pub fn into_inner(self) -> Box<dyn Layout> {
        self.inner
    }
}

impl Layout for KeyboardAwareLayout {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.inner.add_widget(widget_id, stretch);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.inner.remove_widget(widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.inner.child_ids()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.inner.has_child(id)
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        // Shift the entire rect upward by keyboard_offset so content
        // remains visible above the keyboard.
        let adjusted = Rect::new(rect.x, rect.y - self.keyboard_offset, rect.width, rect.height);
        self.inner.update(adjusted, widgets);
    }

    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let adjusted = Rect::new(rect.x, rect.y - self.keyboard_offset, rect.width, rect.height);
        self.inner.update_with_context(adjusted, context, widgets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::stack::StackLayout;

    /// Helper to collect update results into a HashMap.
    fn collect_update(
        layout: &dyn Layout,
        rect: Rect,
    ) -> std::collections::HashMap<ObjectId, Rect> {
        let mut rects = std::collections::HashMap::new();
        layout.update(rect, &mut |id, r| {
            rects.insert(id, r);
        });
        rects
    }

    #[test]
    fn keyboard_aware_passes_through_add_remove() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);

        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        assert!(layout.has_child(1));
        assert!(layout.has_child(2));
        assert_eq!(layout.child_ids().len(), 2);

        layout.remove_widget(1);
        assert!(!layout.has_child(1));
        assert!(layout.has_child(2));
    }

    #[test]
    fn keyboard_aware_clear() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);

        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        assert_eq!(layout.child_ids().len(), 2);

        layout.clear();
        assert_eq!(layout.child_ids().len(), 0);
    }

    #[test]
    fn keyboard_aware_no_offset_passthrough() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);
        layout.add_widget(42, 0);

        let rects = collect_update(&layout, Rect::new(0, 0, 100, 200));
        // No offset: rect passed through unchanged.
        assert_eq!(rects.get(&42), Some(&Rect::new(0, 0, 100, 200)));
    }

    #[test]
    fn keyboard_aware_offset_shifts_content_up() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);
        layout.add_widget(42, 0);
        layout.set_keyboard_offset(150);

        let rects = collect_update(&layout, Rect::new(0, 0, 100, 400));
        // Keyboard is 150px high, so content shifts up by 150:
        // the child receives rect at (0, -150, 100, 400)
        assert_eq!(rects.get(&42), Some(&Rect::new(0, -150, 100, 400)));
    }

    #[test]
    fn keyboard_aware_zero_offset_after_set() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);
        layout.add_widget(42, 0);
        layout.set_keyboard_offset(200);
        layout.set_keyboard_offset(0); // keyboard hidden again

        let rects = collect_update(&layout, Rect::new(0, 0, 100, 200));
        assert_eq!(rects.get(&42), Some(&Rect::new(0, 0, 100, 200)));
    }

    #[test]
    fn keyboard_aware_getters() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let layout = KeyboardAwareLayout::new(inner, 300);

        assert_eq!(layout.animation_duration(), 300);
        assert_eq!(layout.keyboard_offset(), 0);
    }

    #[test]
    fn keyboard_aware_update_with_context_applies_offset() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);
        layout.add_widget(7, 0);
        layout.set_keyboard_offset(100);

        let context = LayoutContext::default();
        let mut rects = std::collections::HashMap::new();
        layout.update_with_context(Rect::new(0, 0, 200, 300), &context, &mut |id, rect| {
            rects.insert(id, rect);
        });

        // Offset applied: y becomes -100
        assert_eq!(rects.get(&7), Some(&Rect::new(0, -100, 200, 300)));
    }

    #[test]
    fn keyboard_aware_inner_layout_access() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let layout = KeyboardAwareLayout::new(inner, 200);

        let inner = layout.inner_layout();
        assert!(inner.as_any().is::<StackLayout>());
    }

    #[test]
    fn keyboard_aware_inner_layout_mut_access() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 200);

        let inner_mut = layout.inner_layout_mut();
        inner_mut.add_widget(99, 0);
        assert!(layout.has_child(99));
    }

    #[test]
    fn keyboard_aware_into_inner() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let layout = KeyboardAwareLayout::new(inner, 200);
        let _inner = layout.into_inner();
    }
}
