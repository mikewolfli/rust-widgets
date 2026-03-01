//! Clipboard and drag-drop managers.

use crate::core::ObjectId;
use crate::platform::{get_platform, DropEvent, Platform};

/// High-level clipboard access facade.
///
/// This manager forwards clipboard operations to the active platform backend.
pub struct ClipboardManager;

impl ClipboardManager {
    /// Set plain text into the system/process clipboard.
    ///
    /// Returns `true` when the backend accepts the update.
    pub fn set_text(text: impl AsRef<str>) -> bool {
        Self::set_text_with(get_platform(), text.as_ref())
    }

    /// Read plain text from the clipboard.
    ///
    /// Returns an empty string when no text is available.
    pub fn text() -> String {
        Self::text_with(get_platform())
    }

    fn set_text_with(platform: &dyn Platform, text: &str) -> bool {
        platform.set_clipboard_text(text)
    }

    fn text_with(platform: &dyn Platform) -> String {
        platform.get_clipboard_text()
    }
}

/// High-level drag-and-drop access facade.
///
/// This manager exposes minimal cross-platform drag and drop entry points.
pub struct DragDropManager;

impl DragDropManager {
    /// Start a drag operation from a source widget.
    ///
    /// `mime` identifies payload type and `payload` carries raw bytes.
    /// Returns `true` if the backend accepts the drag request.
    pub fn begin_drag(source_widget_id: ObjectId, mime: impl AsRef<str>, payload: &[u8]) -> bool {
        Self::begin_drag_with(get_platform(), source_widget_id, mime.as_ref(), payload)
    }

    /// Inject a drop event into the backend queue.
    ///
    /// Useful for tests, bridges, and host-driven event forwarding.
    pub fn inject_drop_event(event: DropEvent) -> bool {
        Self::inject_drop_event_with(get_platform(), event)
    }

    /// Poll the next pending drop event, if any.
    pub fn poll_drop_event() -> Option<DropEvent> {
        Self::poll_drop_event_with(get_platform())
    }

    fn begin_drag_with(platform: &dyn Platform, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        platform.begin_drag(source_widget_id, mime, payload)
    }

    fn inject_drop_event_with(platform: &dyn Platform, event: DropEvent) -> bool {
        platform.inject_drop_event(event)
    }

    fn poll_drop_event_with(platform: &dyn Platform) -> Option<DropEvent> {
        platform.poll_drop_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PlatformFamily;
    use crate::platform::{Platform, StubPlatform};

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

        assert!(DragDropManager::inject_drop_event_with(&stub, event.clone()));
        let queued = DragDropManager::poll_drop_event_with(&stub);
        assert!(queued.is_some());
        assert_eq!(queued.unwrap(), event);
        assert!(DragDropManager::poll_drop_event_with(&stub).is_none());
    }
}
