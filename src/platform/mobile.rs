//! Mobile phase-1 platform slice (Android baseline).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use crate::core::{ObjectId, PlatformFamily};

use super::state::BackendState;
use super::{MobileBackend, MobilePlatformExtension, Platform, WidgetTriggerEvent, WidgetTriggerKind};

/// Logical handle kinds used by mobile baseline state model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MobileHandleKind {
    Window,
    Button,
    LineEdit,
    Label,
    CheckBox,
    Slider,
}

/// Baseline Android mobile platform adapter.
pub struct AndroidMobilePlatform {
    state: BackendState<MobileHandleKind>,
    attached_native_view: AtomicUsize,
}

impl AndroidMobilePlatform {
    /// Creates a new Android mobile platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            attached_native_view: AtomicUsize::new(0),
        }
    }

    /// Insert one widget into the mobile state table.
    fn insert_widget(
        &self,
        kind: MobileHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.state.create_widget(kind, text, x, y, width, height)
    }

    fn create_child_widget(
        &self,
        parent: ObjectId,
        kind: MobileHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if !self.state.contains_widget(parent) {
            return 0;
        }
        self.insert_widget(kind, text, x, y, width, height)
    }

    /// Returns currently attached native view handle when present.
    pub fn attached_native_view(&self) -> Option<usize> {
        let handle = self.attached_native_view.load(Ordering::SeqCst);
        if handle == 0 {
            None
        } else {
            Some(handle)
        }
    }
}

impl Platform for AndroidMobilePlatform {
    fn backend_name(&self) -> &'static str { "android-mobile" }
    fn family(&self) -> PlatformFamily { PlatformFamily::Mobile }
    fn init(&self) {}
    fn run(&self) {}
    fn quit(&self) {}

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.insert_widget(MobileHandleKind::Window, title, x, y, width, height)
    }

    fn create_button(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(parent, MobileHandleKind::Button, text, x, y, width, height)
    }

    fn create_line_edit(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(parent, MobileHandleKind::LineEdit, text, x, y, width, height)
    }

    fn create_label(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(parent, MobileHandleKind::Label, text, x, y, width, height)
    }

    fn create_checkbox(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(parent, MobileHandleKind::CheckBox, text, x, y, width, height)
    }

    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_child_widget(parent, MobileHandleKind::Slider, "Slider", x, y, width, height)
    }

    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        let _ = self.state.set_text(widget_id, text);
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state.pop_menu_event()
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        self.state.push_menu_event(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }
}

impl MobilePlatformExtension for AndroidMobilePlatform {
    fn mobile_backend(&self) -> MobileBackend {
        MobileBackend::Android
    }

    fn attach_to_native_view(&self, native_handle: usize) -> bool {
        if native_handle == 0 {
            return false;
        }
        self.attached_native_view.store(native_handle, Ordering::SeqCst);
        true
    }
}

static MOBILE_PLATFORM: OnceLock<AndroidMobilePlatform> = OnceLock::new();

/// Returns process-global mobile platform singleton.
pub fn get_mobile_platform() -> &'static AndroidMobilePlatform {
    MOBILE_PLATFORM.get_or_init(AndroidMobilePlatform::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_backend_creates_extended_controls() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        assert_ne!(window, 0);

        let line_edit = platform.create_line_edit(window, "name", 10, 10, 120, 24);
        let label = platform.create_label(window, "label", 10, 40, 120, 24);
        let checkbox = platform.create_checkbox(window, "check", 10, 70, 120, 24);
        let slider = platform.create_slider(window, 10, 100, 160, 24);

        assert_ne!(line_edit, 0);
        assert_ne!(label, 0);
        assert_ne!(checkbox, 0);
        assert_ne!(slider, 0);

        assert_eq!(platform.state.kind_of(line_edit), Some(MobileHandleKind::LineEdit));
        assert_eq!(platform.state.kind_of(label), Some(MobileHandleKind::Label));
        assert_eq!(platform.state.kind_of(checkbox), Some(MobileHandleKind::CheckBox));
        assert_eq!(platform.state.kind_of(slider), Some(MobileHandleKind::Slider));
    }

    #[test]
    fn mobile_backend_routes_trigger_events_for_extended_controls() {
        let platform = AndroidMobilePlatform::new();
        let window = platform.create_window("mobile", 0, 0, 320, 480);
        let line_edit = platform.create_line_edit(window, "", 10, 10, 120, 24);
        let checkbox = platform.create_checkbox(window, "", 10, 40, 120, 24);

        assert!(platform.inject_widget_trigger_event(line_edit, WidgetTriggerKind::ValueChanged));
        assert!(platform.inject_widget_trigger_event(checkbox, WidgetTriggerKind::Clicked));

        let first = platform
            .poll_widget_trigger_event()
            .expect("first event should exist");
        let second = platform
            .poll_widget_trigger_event()
            .expect("second event should exist");

        assert_eq!(first.widget_id, line_edit);
        assert_eq!(first.kind, WidgetTriggerKind::ValueChanged);
        assert_eq!(second.widget_id, checkbox);
        assert_eq!(second.kind, WidgetTriggerKind::Clicked);
    }
}
