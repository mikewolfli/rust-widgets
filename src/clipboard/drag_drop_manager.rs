use crate::core::ObjectId;
#[cfg(not(feature = "mini"))]
use crate::platform::get_platform;
use crate::platform::{DropEvent, Platform};
/// High-level drag-and-drop access facade.
///
/// This manager exposes minimal cross-platform drag and drop entry points.
pub struct DragDropManager;
impl DragDropManager {
    /// Start a drag operation from a source widget.
    ///
    /// `mime` identifies payload type and `payload` carries raw bytes.
    /// Returns `true` if the backend accepts the drag request.
    #[cfg(not(feature = "mini"))]
    pub fn begin_drag(source_widget_id: ObjectId, mime: impl AsRef<str>, payload: &[u8]) -> bool {
        Self::begin_drag_with(get_platform(), source_widget_id, mime.as_ref(), payload)
    }
    /// Drag and drop not available in mini mode.
    #[cfg(feature = "mini")]
    pub fn begin_drag(
        _source_widget_id: ObjectId,
        _mime: impl AsRef<str>,
        _payload: &[u8],
    ) -> bool {
        false
    }
    /// Inject a drop event into the backend queue.
    ///
    /// Useful for tests, bridges, and host-driven event forwarding.
    #[cfg(not(feature = "mini"))]
    pub fn inject_drop_event(event: DropEvent) -> bool {
        Self::inject_drop_event_with(get_platform(), event)
    }
    /// Drag and drop not available in mini mode.
    #[cfg(feature = "mini")]
    pub fn inject_drop_event(_event: DropEvent) -> bool {
        false
    }
    /// Poll the next pending drop event, if any.
    #[cfg(not(feature = "mini"))]
    pub fn poll_drop_event() -> Option<DropEvent> {
        Self::poll_drop_event_with(get_platform())
    }
    /// Drag and drop not available in mini mode.
    #[cfg(feature = "mini")]
    pub fn poll_drop_event() -> Option<DropEvent> {
        None
    }
    /// Start a drag operation from a source widget via an explicit platform.
    /// Kept under mini so tests (and any `&dyn Platform` caller) can use it.
    #[cfg_attr(feature = "mini", allow(dead_code))]
    pub(crate) fn begin_drag_with(
        platform: &dyn Platform,
        source_widget_id: ObjectId,
        mime: &str,
        payload: &[u8],
    ) -> bool {
        platform.begin_drag(source_widget_id, mime, payload)
    }
    /// Inject a drop event into the backend queue via an explicit platform.
    /// Kept under mini so tests (and any `&dyn Platform` caller) can use it.
    #[cfg_attr(feature = "mini", allow(dead_code))]
    pub(crate) fn inject_drop_event_with(platform: &dyn Platform, event: DropEvent) -> bool {
        platform.inject_drop_event(event)
    }
    /// Poll the next pending drop event via an explicit platform.
    /// Kept under mini so tests (and any `&dyn Platform` caller) can use it.
    #[cfg_attr(feature = "mini", allow(dead_code))]
    pub(crate) fn poll_drop_event_with(platform: &dyn Platform) -> Option<DropEvent> {
        platform.poll_drop_event()
    }
}
