// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard-aware layout manager — shifts content upward when the mobile keyboard appears,
//! preventing the focused input from being obscured.
use super::{Layout, LayoutContext};
use crate::compat::{fmt, Any, Box, Vec};
use crate::core::{ObjectId, Rect};
use crate::style::animation::EasingFunction;

/// A layout wrapper that shifts its children upward by the keyboard offset.
///
/// This is useful on mobile platforms where the virtual keyboard
/// occludes the bottom portion of the screen. When `set_keyboard_offset`
/// is called with the keyboard height, all child positions are adjusted
/// so that the content remains visible.
///
/// # Why the offset is animated here
///
/// `animation_duration` used to be stored and never read: the offset was applied in one
/// step, so the documented animation did not exist. The slide is now computed by
/// [`KeyboardAwareLayout::tick`], which a host drives with the frame delta the same way it
/// drives every other animated control (`tick(delta_ms) -> bool`, see
/// [`crate::style::animation::CursordBlink`](crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS))
/// — a duration of zero keeps the old instantaneous behaviour, which is what a layout built
/// with `new(inner, 0)` still gets.
pub struct KeyboardAwareLayout {
    /// Inner layout that performs the actual child positioning.
    inner: Box<dyn Layout>,
    /// Target keyboard height in pixels. 0 = keyboard hidden.
    keyboard_offset: i32,
    /// Duration of the offset animation in milliseconds.
    animation_duration: u64,
    /// Offset actually applied to the children, interpolated toward
    /// [`Self::keyboard_offset`] by [`Self::tick`].
    current_offset: i32,
    /// Milliseconds of the in-flight transition, or `None` when settled.
    elapsed_ms: Option<u64>,
}

impl fmt::Debug for KeyboardAwareLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyboardAwareLayout")
            .field("keyboard_offset", &self.keyboard_offset)
            .field("current_offset", &self.current_offset)
            .field("animation_duration", &self.animation_duration)
            .field("inner", &"<dyn Layout>")
            .finish()
    }
}

impl KeyboardAwareLayout {
    /// Create a keyboard-aware layout wrapping an inner layout.
    ///
    /// * `inner` – The layout that manages child positions.
    /// * `animation_duration` – Animation duration in milliseconds. `0` applies the offset
    ///   immediately, which is the behaviour a host with no frame loop wants.
    pub fn new(inner: Box<dyn Layout>, animation_duration: u64) -> Self {
        Self { inner, keyboard_offset: 0, animation_duration, current_offset: 0, elapsed_ms: None }
    }

    /// Set the target keyboard offset (height of the visible keyboard) and begin the
    /// transition toward it.
    ///
    /// Pass `0` to indicate the keyboard is hidden. With a zero
    /// [`animation_duration`](Self::animation_duration) the offset takes effect immediately;
    /// otherwise it is interpolated by [`Self::tick`] until it arrives.
    pub fn set_keyboard_offset(&mut self, offset: i32) {
        if offset == self.keyboard_offset {
            return;
        }
        self.keyboard_offset = offset;
        if self.animation_duration == 0 {
            self.current_offset = offset;
            self.elapsed_ms = None;
        } else {
            // Restart from *wherever we are now*, not from the previous target: a keyboard
            // that is dismissed mid-slide must reverse smoothly rather than jump back.
            self.elapsed_ms = Some(0);
        }
    }

    /// Returns the **target** keyboard offset.
    pub fn keyboard_offset(&self) -> i32 {
        self.keyboard_offset
    }

    /// Returns the offset currently applied to the children.
    ///
    /// This is the target once the transition has settled, and something between the two
    /// while it runs.
    pub fn current_offset(&self) -> i32 {
        self.current_offset
    }

    /// Advances the keyboard slide by `delta_ms`.
    ///
    /// Returns `true` while another frame is still needed, matching the
    /// `tick(delta_ms) -> bool` contract every animated control in this crate follows. With a
    /// zero duration there is nothing to advance and this is always `false`.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let Some(elapsed) = self.elapsed_ms else {
            return false;
        };
        if self.animation_duration == 0 {
            self.current_offset = self.keyboard_offset;
            self.elapsed_ms = None;
            return false;
        }
        let elapsed = elapsed.saturating_add(u64::from(delta_ms));
        let progress = (elapsed as f32 / self.animation_duration as f32).min(1.0);
        let eased = EasingFunction::EaseInOut.apply(progress);
        let from = self.current_offset as f32;
        let to = self.keyboard_offset as f32;
        self.current_offset = (from + (to - from) * eased).round() as i32;
        if progress >= 1.0 {
            self.current_offset = self.keyboard_offset;
            self.elapsed_ms = None;
            false
        } else {
            self.elapsed_ms = Some(elapsed);
            true
        }
    }

    /// Returns the animation duration in milliseconds.
    pub fn animation_duration(&self) -> u64 {
        self.animation_duration
    }

    /// Sets the animation duration.
    ///
    /// Setting it to `0` snaps any in-flight transition to its target, so a host that turns
    /// animation off does not leave the content mid-slide.
    pub fn set_animation_duration(&mut self, duration: u64) {
        self.animation_duration = duration;
        if duration == 0 {
            self.current_offset = self.keyboard_offset;
            self.elapsed_ms = None;
        }
    }

    /// Whether a transition is still in flight.
    pub fn is_animating(&self) -> bool {
        self.elapsed_ms.is_some()
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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
        // Shift the entire rect upward by the *animated* offset so content
        // remains visible above the keyboard.
        let adjusted = Rect::new(rect.x, rect.y - self.current_offset, rect.width, rect.height);
        self.inner.update(adjusted, widgets);
    }

    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        let adjusted = Rect::new(rect.x, rect.y - self.current_offset, rect.width, rect.height);
        self.inner.update_with_context(adjusted, context, widgets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;
    use crate::layout::stack::StackLayout;

    /// Helper to collect update results into a HashMap.
    fn collect_update(layout: &dyn Layout, rect: Rect) -> HashMap<ObjectId, Rect> {
        let mut rects = HashMap::new();
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
        // A zero duration is the host that wants the shift immediately.
        let mut layout = KeyboardAwareLayout::new(inner, 0);
        layout.add_widget(42, 0);
        layout.set_keyboard_offset(150);

        let rects = collect_update(&layout, Rect::new(0, 0, 100, 400));
        // Keyboard is 150px high, so content shifts up by 150:
        // the child receives rect at (0, -150, 100, 400)
        assert_eq!(rects.get(&42), Some(&Rect::new(0, -150, 100, 400)));
    }

    /// With a duration the offset is **interpolated**: the first frame is still at the old
    /// position, the midpoint is halfway, and the transition reports "more work" until it
    /// arrives. `animation_duration` used to be stored and never read — the slide it
    /// documented did not exist.
    #[test]
    fn a_non_zero_duration_slides_the_offset_instead_of_snapping() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 100);
        layout.add_widget(42, 0);
        layout.set_keyboard_offset(100);

        assert!(layout.is_animating(), "setting a target starts a transition");
        assert_eq!(layout.current_offset(), 0, "it starts where it was");

        // Halfway through: the eased value is 50 for `EaseInOut`, so the content has moved
        // half the distance but is not yet at the target.
        assert!(layout.tick(50), "the transition is unfinished");
        assert_eq!(layout.current_offset(), 50);
        let rects = collect_update(&layout, Rect::new(0, 0, 100, 400));
        assert_eq!(rects.get(&42), Some(&Rect::new(0, -50, 100, 400)));

        // Arriving settles it: no further frames are owed, and the offset is exact.
        assert!(!layout.tick(50), "the transition is finished");
        assert_eq!(layout.current_offset(), 100);
        assert!(!layout.is_animating());
        let rects = collect_update(&layout, Rect::new(0, 0, 100, 400));
        assert_eq!(rects.get(&42), Some(&Rect::new(0, -100, 100, 400)));
    }

    /// A keyboard dismissed mid-slide reverses from **wherever the content is now**, so the
    /// motion stays continuous instead of jumping back to the top and starting again.
    #[test]
    fn a_reversed_slide_resumes_from_the_current_position() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 100);
        layout.set_keyboard_offset(100);
        layout.tick(50);
        assert_eq!(layout.current_offset(), 50);

        layout.set_keyboard_offset(0);
        assert_eq!(layout.current_offset(), 50, "no jump on reversal");
        layout.tick(50);
        assert_eq!(layout.current_offset(), 25, "it walks back from where it was");
        layout.tick(50);
        assert_eq!(layout.current_offset(), 0);
    }

    /// Turning animation off snaps an in-flight transition, so a host that disables motion
    /// does not leave the content parked mid-slide.
    #[test]
    fn disabling_the_duration_settles_an_in_flight_slide() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 100);
        layout.set_keyboard_offset(80);
        layout.tick(10);
        assert!(layout.is_animating());

        layout.set_animation_duration(0);
        assert!(!layout.is_animating(), "the transition was resolved, not dropped");
        assert_eq!(layout.current_offset(), 80);
    }

    /// Setting the same offset again must not restart the transition.
    #[test]
    fn setting_the_same_offset_does_not_restart_the_slide() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 100);
        layout.set_keyboard_offset(60);
        layout.tick(99);
        let nearly_there = layout.current_offset();
        layout.set_keyboard_offset(60);
        assert_eq!(layout.current_offset(), nearly_there, "the progress was preserved");
        assert!(!layout.tick(1), "it still arrives on the same frame");
    }

    #[test]
    fn keyboard_aware_zero_offset_after_set() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 0);
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
        assert_eq!(layout.current_offset(), 0);
        assert!(!layout.is_animating());
    }

    #[test]
    fn keyboard_aware_update_with_context_applies_offset() {
        let inner: Box<dyn Layout> = Box::new(StackLayout::new());
        let mut layout = KeyboardAwareLayout::new(inner, 0);
        layout.add_widget(7, 0);
        layout.set_keyboard_offset(100);

        let context = LayoutContext::default();
        let mut rects = HashMap::new();
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
