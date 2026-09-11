//! Windows platform tests.
//!
//! The pure notification-mapping tests now live in
//! [`crate::platform::windows_notify`] so they run on every host. What remains
//! here needs a real `WindowsPlatform` and therefore only compiles on Windows.

#[cfg(target_os = "windows")]
mod tests {
    use crate::platform::windows::{
        notify::{control_notify_kind_for_widget, enqueue_control_notify_event},
        types::{WindowsHandleKind, WindowsPlatform},
    };
    use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
    #[test]
    fn control_notify_mapping_button_click_routes_clicked() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::Button, 0);
        assert_eq!(kind, Some(WidgetTriggerKind::Clicked));
    }
    #[test]
    fn control_notify_mapping_line_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::LineEdit, 0x0300);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }
    #[test]
    fn control_notify_mapping_combo_selection_routes_selection_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 1);
        assert_eq!(kind, Some(WidgetTriggerKind::SelectionChanged));
    }
    #[test]
    fn control_notify_mapping_combo_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 5);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }
    #[test]
    fn combo_selection_notify_enqueues_selection_and_value_events() {
        let platform = WindowsPlatform::new();
        let combo =
            platform.state.create_widget(WindowsHandleKind::ComboBox, "ComboBox", 0, 0, 120, 24);
        assert!(enqueue_control_notify_event(&platform, combo, 1));
        let mut queue = platform
            .menu_state
            .pending_widget_events
            .lock()
            .expect("windows trigger queue lock poisoned");
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent {
                widget_id: combo,
                kind: WidgetTriggerKind::SelectionChanged,
            })
        );
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::ValueChanged })
        );
    }

    /// The three controls that used to be state-only surrogates must now go
    /// through a real native creation helper. On Windows the helper must return
    /// a bound handle.
    #[test]
    fn spin_box_list_view_scroll_area_route_through_native_helpers() {
        use crate::platform::windows::helpers::{
            try_create_list_view, try_create_scroll_area, try_create_spin_box,
        };
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        let window = platform.create_window("w", 0, 0, 400, 300);
        assert!(window > 0, "window should be created");

        let spin = try_create_spin_box(&platform, window, 0, 0, 80, 24);
        let list = try_create_list_view(&platform, window, 0, 30, 200, 150);
        let scroll = try_create_scroll_area(&platform, window, 0, 190, 200, 100);

        // A real `msctls_updown32` / `SysListView32` / scrollable child window
        // must have been created and bound to its widget id.
        for (label, created) in [("SpinBox", spin), ("ListView", list), ("ScrollArea", scroll)] {
            let id = created.unwrap_or_else(|| panic!("{label} native creation failed"));
            assert!(
                platform.get_native_handle(id).is_some(),
                "{label} must have a bound native handle"
            );
        }
    }

    /// An unknown parent must be rejected before any native call is attempted.
    #[test]
    fn native_control_helpers_reject_unknown_parent() {
        use crate::platform::windows::helpers::{
            try_create_list_view, try_create_scroll_area, try_create_spin_box,
        };

        let platform = WindowsPlatform::new();
        platform.init();
        let bogus = 4242;
        assert_eq!(try_create_spin_box(&platform, bogus, 0, 0, 80, 24), None);
        assert_eq!(try_create_list_view(&platform, bogus, 0, 0, 80, 24), None);
        assert_eq!(try_create_scroll_area(&platform, bogus, 0, 0, 80, 24), None);
    }
}
