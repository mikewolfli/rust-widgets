//! Clipboard and drag-drop managers.
mod clipboard_manager;
mod drag_drop_manager;
pub use clipboard_manager::ClipboardManager;
pub use drag_drop_manager::DragDropManager;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PlatformFamily;
    use crate::platform::{DropEvent, Platform, StubPlatform};
    #[test]
    fn clipboard_roundtrip() {
        let stub = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
        assert!(ClipboardManager::set_text_with(&stub, "hello"));
        assert_eq!(ClipboardManager::text_with(&stub), "hello".to_string());
    }
    #[test]
    fn drop_event_queue_roundtrip() {
        let stub = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
        let source = stub.create_window("source", 0, 0, 10, 10);
        let target = stub.create_window("target", 10, 10, 10, 10);
        let event = DropEvent {
            source_widget_id: source,
            target_widget_id: target,
            mime: "text/plain".to_string(),
            payload: b"payload".to_vec(),
        };
        assert!(DragDropManager::inject_drop_event_with(
            &stub,
            event.clone()
        ));
        let queued = DragDropManager::poll_drop_event_with(&stub);
        assert!(queued.is_some());
        assert_eq!(queued.unwrap(), event);
        assert!(DragDropManager::poll_drop_event_with(&stub).is_none());
    }
}
