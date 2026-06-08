//! Radio button widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Radio button widget.
pub struct RadioButton {
    base: BaseWidget,
    checked: bool,
    group_id: Option<String>,
    text: String,
    pub selected: GenericSignal,
    pub checked_changed: Signal1<bool>,
}
impl RadioButton {
    /// Creates an unchecked radio button with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RadioButton, geometry, "RadioButton"),
            checked: false,
            group_id: None,
            text: String::new(),
            selected: GenericSignal::new(),
            checked_changed: Signal1::new(),
        }
    }
    /// Returns current checked state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Returns the text label displayed next to the radio button.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets the text label displayed next to the radio button and requests a redraw.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    /// Sets optional group identifier.
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
    }
    /// Returns optional group identifier.
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets checked state and emits deterministic signals.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.checked_changed.emit(checked);
        if checked {
            self.selected.emit();
        }
    }
    /// Selects one radio button within a peer group.
    pub fn select_in_group(peers: &mut [&mut RadioButton], selected_index: usize) -> bool {
        if selected_index >= peers.len() {
            return false;
        }
        let selected_group = peers[selected_index].group_id.clone();
        for (index, peer) in peers.iter_mut().enumerate() {
            if selected_group.is_some() && peer.group_id != selected_group {
                continue;
            }
            peer.set_checked(index == selected_index);
        }
        true
    }
}
// Implement Widget trait
impl Widget for RadioButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for RadioButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } if *button == 1 => {
                self.set_checked(true);
                self.base.clicked.emit();
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { .. } | Event::Tap { .. } => {
                self.set_checked(true);
                self.base.clicked.emit();
            }
            Event::KeyPress { key, .. } if *key == 32 || *key == 13 => {
                // Space or Enter
                self.set_checked(true);
                self.base.clicked.emit();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for RadioButton {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw radio button
        let rect = self.geometry();
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
        let radius = rect.height.min(rect.width) / 4;
        // Draw outer circle
        context.draw_circle(center, radius, Color::from_rgb(100u8, 100, 100));
        // Draw inner circle if checked
        if self.checked {
            let inner_radius = radius / 2;
            context.fill_circle(center, inner_radius, Color::from_rgb(0u8, 120, 215));
        }
        // Draw text label
        let text_pos = Point::new(rect.x + rect.width as i32 / 2 + radius as i32 + 4, center.y);
        context.draw_text(text_pos, &self.text, &Font::default(), Color::from_rgb(60u8, 60, 60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::core::Size;
    use crate::style::WidgetStyle;

    // -----------------------------------------------------------------------
    // 1. Creation: default unchecked, empty text, no group_id
    // -----------------------------------------------------------------------
    #[test]
    fn test_creation_defaults() {
        let rect = Rect::new(10, 20, 100, 30);
        let rb = RadioButton::new(rect);
        assert!(!rb.is_checked(), "new radio button should be unchecked");
        assert_eq!(rb.text(), "", "new radio button text should be empty");
        assert_eq!(rb.group_id(), None, "new radio button should have no group_id");
        assert_eq!(rb.geometry(), rect, "geometry should match");
    }

    // -----------------------------------------------------------------------
    // 2. set_checked(true/false) state changes
    // -----------------------------------------------------------------------
    #[test]
    fn test_set_checked_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    #[test]
    fn test_set_checked_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
    }

    #[test]
    fn test_set_checked_toggle() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true);
        assert!(rb.is_checked());
        rb.set_checked(false);
        assert!(!rb.is_checked());
        rb.set_checked(true);
        assert!(rb.is_checked());
    }

    // -----------------------------------------------------------------------
    // 3. checked_changed signal (on true, on false, not on noop)
    // -----------------------------------------------------------------------
    #[test]
    fn test_checked_changed_emitted_on_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to true"
        );
        assert!(
            fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be true"
        );
    }

    #[test]
    fn test_checked_changed_emitted_on_false() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let f = std::sync::Arc::clone(&fired);
            let fv = std::sync::Arc::clone(&fired_value);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |val| {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
                fv.store(*val, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed should fire when set to false"
        );
        assert!(
            !fired_value.load(std::sync::atomic::Ordering::SeqCst),
            "checked_changed value should be false"
        );
    }

    #[test]
    fn test_checked_changed_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.checked_changed.connect_scoped(scope, move |_val| {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // First set to true -> should fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should fire once on true");
        // Set to true again (noop) -> should NOT fire
        rb.set_checked(true);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1, "should NOT fire on noop");
    }

    // -----------------------------------------------------------------------
    // 4. selected signal (on becoming true, not on uncheck)
    // -----------------------------------------------------------------------
    #[test]
    fn test_selected_emitted_on_becoming_true() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(true);
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "selected signal should fire when checked becomes true"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_uncheck() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_checked(true); // start checked
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false);
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on uncheck"
        );
    }

    #[test]
    fn test_selected_not_emitted_on_noop() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        {
            let c = std::sync::Arc::clone(&count);
            let scope = rb.connection_scope();
            rb.selected.connect_scoped(scope, move || {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        rb.set_checked(false); // already false -> noop
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "selected signal should NOT fire on noop"
        );
        rb.set_checked(true); // becomes true -> fires
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "selected signal should fire once on becoming true"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Text set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_text_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.text(), "");
        rb.set_text("Option A".to_string());
        assert_eq!(rb.text(), "Option A");
    }

    #[test]
    fn test_text_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("First".to_string());
        assert_eq!(rb.text(), "First");
        rb.set_text("Second".to_string());
        assert_eq!(rb.text(), "Second");
    }

    #[test]
    fn test_text_empty_after_set() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_text("Something".to_string());
        rb.set_text(String::new());
        assert_eq!(rb.text(), "");
    }

    // -----------------------------------------------------------------------
    // 6. Group ID set/get
    // -----------------------------------------------------------------------
    #[test]
    fn test_group_id_set_get() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.group_id(), None);
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
    }

    #[test]
    fn test_group_id_overwrite() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group_a".to_string()));
        assert_eq!(rb.group_id(), Some("group_a"));
        rb.set_group_id(Some("group_b".to_string()));
        assert_eq!(rb.group_id(), Some("group_b"));
    }

    #[test]
    fn test_group_id_clear() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb.set_group_id(Some("group1".to_string()));
        assert_eq!(rb.group_id(), Some("group1"));
        rb.set_group_id(None);
        assert_eq!(rb.group_id(), None);
    }

    // -----------------------------------------------------------------------
    // 7. select_in_group static method
    // -----------------------------------------------------------------------
    #[test]
    fn test_select_in_group_selects_correct_peer() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb2 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));
        rb2.set_group_id(Some("g".to_string()));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1, &mut rb2];
        let result = RadioButton::select_in_group(&mut peers, 1);
        assert!(result, "select_in_group should return true on success");
        drop(peers);
        assert!(!rb0.is_checked(), "peer 0 should be unchecked");
        assert!(rb1.is_checked(), "peer 1 should be checked");
        assert!(!rb2.is_checked(), "peer 2 should be unchecked");
    }

    #[test]
    fn test_select_in_group_deselects_others() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        rb0.set_group_id(Some("g".to_string()));
        rb1.set_group_id(Some("g".to_string()));

        rb0.set_checked(true);
        assert!(rb0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);
        assert!(!rb0.is_checked(), "previously checked peer 0 should be deselected");
        assert!(rb1.is_checked(), "peer 1 should be selected");
    }

    #[test]
    fn test_select_in_group_leaves_other_groups() {
        let mut rb_a0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_a1 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb_b1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        rb_a0.set_group_id(Some("A".to_string()));
        rb_a1.set_group_id(Some("A".to_string()));
        rb_b0.set_group_id(Some("B".to_string()));
        rb_b1.set_group_id(Some("B".to_string()));

        rb_a0.set_checked(true);
        rb_b0.set_checked(true);
        assert!(rb_a0.is_checked());
        assert!(rb_b0.is_checked());

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb_a0, &mut rb_a1, &mut rb_b0, &mut rb_b1];
        RadioButton::select_in_group(&mut peers, 1);
        drop(peers);

        assert!(!rb_a0.is_checked(), "group A peer 0 should be deselected");
        assert!(rb_a1.is_checked(), "group A peer 1 should be selected");
        assert!(rb_b0.is_checked(), "group B peer 0 should remain checked");
        assert!(!rb_b1.is_checked(), "group B peer 1 should remain unchecked");
    }

    #[test]
    fn test_select_in_group_out_of_bounds_returns_false() {
        let mut rb0 = RadioButton::new(Rect::new(0, 0, 50, 20));
        let mut rb1 = RadioButton::new(Rect::new(0, 0, 50, 20));

        let mut peers: Vec<&mut RadioButton> = vec![&mut rb0, &mut rb1];
        let result = RadioButton::select_in_group(&mut peers, 5);
        assert!(!result, "out-of-bounds index should return false");
        drop(peers);
        assert!(!rb0.is_checked());
        assert!(!rb1.is_checked());
    }

    #[test]
    fn test_select_in_group_empty_slice_returns_false() {
        let mut empty_peers: Vec<&mut RadioButton> = vec![];
        let result = RadioButton::select_in_group(&mut empty_peers, 0);
        assert!(!result, "empty slice should return false");
    }

    // -----------------------------------------------------------------------
    // 8. Widget trait delegation
    // -----------------------------------------------------------------------
    #[test]
    fn test_widget_id_kind() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.id(), rb.base.id());
        assert_eq!(rb.kind(), WidgetKind::RadioButton);
    }

    #[test]
    fn test_widget_geometry() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.geometry(), Rect::new(0, 0, 50, 20));
        rb.set_geometry(Rect::new(10, 10, 80, 30));
        assert_eq!(rb.geometry(), Rect::new(10, 10, 80, 30));
    }

    #[test]
    fn test_widget_min_max_size() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.min_size().is_none());
        assert!(rb.max_size().is_none());
        rb.set_min_size(Some(Size::new(20, 10)));
        rb.set_max_size(Some(Size::new(200, 100)));
        assert_eq!(rb.min_size(), Some(Size::new(20, 10)));
        assert_eq!(rb.max_size(), Some(Size::new(200, 100)));
    }

    #[test]
    fn test_widget_parent_children() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let pid = 42u64;
        assert!(rb.parent().is_none());
        rb.set_parent(Some(pid));
        assert_eq!(rb.parent(), Some(pid));
        rb.set_parent(None);
        assert!(rb.parent().is_none());

        let cid = 100u64;
        assert!(rb.children().is_empty());
        rb.add_child(cid);
        assert_eq!(rb.children(), &[cid]);
        rb.remove_child(cid);
        assert!(rb.children().is_empty());
    }

    #[test]
    fn test_widget_visibility() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_visible());
        rb.hide();
        assert!(!rb.is_visible());
        rb.show();
        assert!(rb.is_visible());
    }

    #[test]
    fn test_widget_enabled() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert!(rb.is_enabled());
        rb.set_enabled(false);
        assert!(!rb.is_enabled());
        rb.set_enabled(true);
        assert!(rb.is_enabled());
    }

    #[test]
    fn test_widget_tooltip() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        assert_eq!(rb.tooltip(), "");
        rb.set_tooltip("Click me".to_string());
        assert_eq!(rb.tooltip(), "Click me");
    }

    #[test]
    fn test_widget_style() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let style = WidgetStyle::default();
        rb.set_style(style.clone());
        let _ = rb.style();
    }

    #[test]
    fn test_widget_signals_exist() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.hover_signal();
        let _ = rb.mouse_down_signal();
        let _ = rb.mouse_up_signal();
        let _ = rb.key_down_signal();
        let _ = rb.key_up_signal();
        let _ = rb.focus_gained_signal();
        let _ = rb.focus_lost_signal();
        let _ = rb.redraw_requested_signal();
        let _ = rb.layout_requested_signal();
    }

    #[test]
    fn test_widget_connection_scope() {
        let rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let _ = rb.connection_scope();
    }

    #[test]
    fn test_widget_request_redraw_signal() {
        let mut rb = RadioButton::new(Rect::new(0, 0, 50, 20));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = rb.connection_scope();
            rb.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
        // Setting text triggers request_redraw, which should emit the signal
        rb.set_text("Test".to_string());
        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "redraw_requested_signal should fire when text is set"
        );
    }
}
