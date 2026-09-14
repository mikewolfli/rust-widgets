// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

    /// The three controls that used to be state-only surrogates routed through
    /// native creation helpers. Those helpers are gone now that every kind is
    /// painted by `src/widget/`, so what remains to assert is the contract the
    /// routing layer relies on: the backend no longer allocates a native handle
    /// for any control kind, and `widget::runtime` is the only surface that does.
    #[test]
    fn control_kinds_no_longer_allocate_native_handles() {
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        let window = platform.create_window("w", 0, 0, 400, 300);
        assert!(window > 0, "window should be created");

        // The window itself still has a bound HWND: it is the host for mounted
        // self-drawn surfaces.
        assert!(
            platform.get_native_handle(window).is_some(),
            "a window must keep its native handle"
        );
    }

    /// Self-drawn widgets are hosted by `windows/canvas.rs`, so the backend must
    /// advertise the surface and refuse to mount an unregistered widget id.
    #[test]
    fn widget_surface_is_advertised_and_validates_ids() {
        use crate::platform::Platform;

        let platform = WindowsPlatform::new();
        platform.init();
        assert!(platform.supports_surfaces());

        // A widget id that was never registered in `widget::runtime` must be
        // refused rather than producing an empty canvas.
        assert!(!platform.mount_surface(1, 4242, crate::core::Rect::new(0, 0, 10, 10)));
        assert!(!platform.invalidate_surface(4242));
        assert!(!platform.unmount_surface(4242));
    }
}
