// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mock/test implementation of the `ControlBackend` trait (BLUE11 R9.1, BLUE15 Phase C).

use super::trait_def::ControlBackend;
use crate::compat::String;
use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;

/// A test double that implements only the required surface of `ControlBackend`.
///
/// # Why it is tiny
///
/// This type used to override ~90 optional methods with the value `100 + n` so
/// that a test could read a hard-coded id back out. Those overrides described a
/// backend that creates controls, which no backend does any more: the library
/// paints every `WidgetKind`, and the host only supplies a window and a drawing
/// surface (BLUE15 #55/#56). A double that still pretended to build widgets would
/// be exactly the kind of "still looks native from the outside" surface the
/// refactor removed.
///
/// What a test double must actually prove is that the *contract* is complete and
/// that the defaults are honest — that is, that an unimplemented member reports
/// absence (`0` / `false` / `None`) rather than a plausible-looking value. Both
/// are asserted below.
struct TestBackend;

/// The id this double reports for the one member it really implements.
const WINDOW_ID: ObjectId = 100;

impl ControlBackend for TestBackend {
    fn backend_name(&self) -> &'static str {
        "test-backend"
    }

    fn kind(&self) -> ControlBackendKind {
        ControlBackendKind::Native
    }

    fn create_window(&self, _title: &str, _x: i32, _y: i32, _width: u32, _height: u32) -> ObjectId {
        WINDOW_ID
    }

    // The creation members the trait still declares as required. They are no-ops on
    // purpose: this double models a host that supplies a window only, which is what
    // every real backend now does (BLUE15 #56). Reporting `0` keeps a caller's
    // `assert_ne!(id, 0)` meaningful instead of handing it an id that addresses
    // nothing.
    fn create_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_checkbox(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_line_edit(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_label(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_radio_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_slider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_progress_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_list_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_panel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_group_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    fn create_toggle_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    // Control state members. This double holds no state, so a mutation reports
    // failure and a read reports the empty/absent value. Returning an invented
    // value here would make a test pass against a backend that cannot happen.
    fn set_widget_text(&self, _widget_id: ObjectId, _text: &str) {}

    fn get_widget_text(&self, _widget_id: ObjectId) -> String {
        String::new()
    }

    fn set_widget_enabled(&self, _widget_id: ObjectId, _enabled: bool) {}

    fn is_widget_enabled(&self, _widget_id: ObjectId) -> bool {
        false
    }

    fn set_widget_visible(&self, _widget_id: ObjectId, _visible: bool) {}

    fn is_widget_visible(&self, _widget_id: ObjectId) -> bool {
        false
    }

    fn set_widget_geometry(
        &self,
        _widget_id: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) {
    }

    fn set_widget_ime_enabled(&self, _widget_id: ObjectId, _enabled: bool) -> bool {
        false
    }

    fn is_widget_ime_enabled(&self, _widget_id: ObjectId) -> bool {
        false
    }

    fn set_widget_accessibility_name(&self, _widget_id: ObjectId, _name: &str) -> bool {
        false
    }

    fn get_widget_accessibility_name(&self, _widget_id: ObjectId) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_can_be_constructed() {
        let backend = TestBackend;
        assert_eq!(backend.backend_name(), "test-backend");
        assert_eq!(backend.kind(), ControlBackendKind::Native);
    }

    #[test]
    fn test_backend_creates_window() {
        let backend = TestBackend;
        assert_eq!(backend.create_window("Test", 0, 0, 800, 600), WINDOW_ID);
    }

    /// A member this double does not implement must report absence.
    ///
    /// This is the meaningful assertion now: the trait's defaults have to be
    /// honest. A default that returned a non-zero id would let a caller treat a
    /// control that was never created as a live one.
    #[test]
    fn unimplemented_creation_reports_absence() {
        let backend = TestBackend;
        assert_eq!(backend.create_button(100, "Click", 10, 20, 100, 30), 0);
        assert_eq!(backend.create_label(100, "Hello", 0, 0, 200, 50), 0);
        assert_eq!(
            backend.create_window("W", 0, 0, 1, 1),
            WINDOW_ID,
            "the one override still answers"
        );
    }

    /// The name-based entry must answer `0` unless a backend resolves the name.
    #[test]
    fn create_widget_by_name_default_is_absent() {
        let backend = TestBackend;
        assert_eq!(
            ControlBackend::create_widget(&backend, "button", 0, "x", 0, 0, 10, 10),
            0,
            "an unimplemented name lookup must report absence"
        );
    }

    /// Typed trigger polling has no queue in this double, so it reports `None`.
    #[test]
    fn typed_trigger_poll_reports_none() {
        let backend = TestBackend;
        assert!(backend.poll_widget_trigger_event().is_none());
        assert!(backend.poll_widget_triggered().is_none());
    }

    /// Mutation of a control that does not exist must not claim success.
    #[test]
    fn unimplemented_mutation_reports_failure() {
        let backend = TestBackend;
        assert!(!backend.set_widget_accessibility_name(7, "name"));
        assert!(
            !backend.inject_widget_trigger_event(7, crate::platform::WidgetTriggerKind::Clicked)
        );
        assert!(!backend.destroy_widget(7));
    }
}
