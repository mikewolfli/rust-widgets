//! macOS platform tests.

#![allow(deprecated)]

mod tests {
    use crate::core::ObjectId;
    use crate::platform::macos::{HandleKind, MacOSPlatform};
    use crate::platform::{DropEvent, Platform};
    fn insert_dummy_widget(platform: &MacOSPlatform) -> ObjectId {
        platform.state.create_widget(HandleKind::Button, "dummy", 0, 0, 10, 10)
    }
    #[test]
    fn macos_backend_ime_and_accessibility_state_roundtrip() {
        let platform = MacOSPlatform::new();
        let widget_id = insert_dummy_widget(&platform);
        assert!(Platform::set_widget_ime_enabled(&platform, widget_id, true));
        assert!(Platform::is_widget_ime_enabled(&platform, widget_id));
        assert!(Platform::set_widget_accessibility_name(&platform, widget_id, "Accessible"));
        assert_eq!(
            Platform::get_widget_accessibility_name(&platform, widget_id),
            "Accessible".to_string()
        );
    }
    #[test]
    fn macos_backend_clipboard_and_drag_drop_roundtrip() {
        let platform = MacOSPlatform::new();
        let widget_id = insert_dummy_widget(&platform);
        assert!(Platform::set_clipboard_text(&platform, "hello"));
        assert_eq!(Platform::get_clipboard_text(&platform), "hello".to_string());
        assert!(Platform::begin_drag(&platform, widget_id, "text/plain", b"abc"));
        let event = Platform::poll_drop_event(&platform).expect("drop event should exist");
        assert_eq!(event.source_widget_id, widget_id);
        assert_eq!(event.mime, "text/plain");
        assert_eq!(event.payload, b"abc".to_vec());
        let injected = DropEvent {
            source_widget_id: widget_id,
            target_widget_id: widget_id,
            mime: "application/octet-stream".to_string(),
            payload: vec![1, 2, 3],
        };
        assert!(Platform::inject_drop_event(&platform, injected.clone()));
        assert_eq!(Platform::poll_drop_event(&platform), Some(injected));
    }
}
