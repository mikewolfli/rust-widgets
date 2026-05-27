//! Windows platform tests.

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
            platform
                .state
                .create_widget(WindowsHandleKind::ComboBox, "ComboBox", 0, 0, 120, 24);
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
            Some(WidgetTriggerEvent {
                widget_id: combo,
                kind: WidgetTriggerKind::ValueChanged,
            })
        );
    }
}
