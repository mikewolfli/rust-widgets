// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The one control backend, and the two entry points that reach it.
//!
//! # Why the `cfg` tower is gone
//!
//! This module used to select a backend through a matrix of feature
//! combinations: `controls-native` × `controls-custom`, with a separate
//! `stripped_widgets` arm — five copies each of [`get_control_backend`],
//! [`get_control_backend_for_widget`] and [`active_control_policy`], which had to
//! keep agreeing by hand. They could not: the mutation test above found two arms
//! answering with different values for the same build.
//!
//! There is one mechanism now (the library paints every `WidgetKind`; the host
//! supplies a window and a surface), so the selection collapses to the two real
//! cases: a build that can paint, and a build without the painting backend at all.
//! `controls-native` no longer selects anything — it is retained only where the
//! `native` backend wrapper itself is compiled.

#[cfg(feature = "controls-custom")]
use crate::compat::OnceLock;
#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
use crate::control_backend::trait_def::ControlBackend;
#[cfg(not(feature = "controls-custom"))]
use crate::core::ObjectId;
use crate::widget::WidgetKind;

// A `native_control_backend()` factory used to live here, gated
// `all(feature = "controls-native", widgets_unstripped)` and marked
// `#[allow(dead_code)]`. Its only reference was its own definition — nothing selected
// it, and the module doc above explains why: `controls-native` no longer selects
// anything, so the native wrapper is reachable only as the type `NativeControlBackend`
// re-exported from `control_backend::mod`. Deleted rather than kept behind the allow,
// matching the precedent in `src/lib.rs` where a registry-backed arm that cannot be
// called was removed with the same reasoning (principle #4: an unreachable factory is
// not a fallback). The type itself is still built and exported, so the naming place a
// genuine platform primitive would report from has not been lost.

#[cfg(feature = "controls-custom")]
fn custom_control_backend() -> &'static CustomPaintControlBackend {
    static BACKEND: OnceLock<CustomPaintControlBackend> = OnceLock::new();
    BACKEND.get_or_init(CustomPaintControlBackend::new)
}
#[cfg(not(feature = "controls-custom"))]
struct NoControlBackend;
#[cfg(not(feature = "controls-custom"))]
impl crate::control_backend::trait_def::ControlBackend for NoControlBackend {
    fn backend_name(&self) -> &'static str {
        "no-control-backend"
    }
    fn kind(&self) -> crate::control_backend::types::ControlBackendKind {
        crate::control_backend::types::ControlBackendKind::Custom
    }
    fn create_window(&self, _title: &str, _x: i32, _y: i32, _width: u32, _height: u32) -> ObjectId {
        0
    }
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
#[cfg(not(feature = "controls-custom"))]
fn no_control_backend() -> &'static NoControlBackend {
    static BACKEND: NoControlBackend = NoControlBackend;
    &BACKEND
}
/// Returns control backend resolved by compile-time policy for one widget kind.
///
/// With one mechanism there is nothing left to resolve, so this is
/// [`get_control_backend`]. It is kept as a named entry point because "which
/// backend creates this kind" remains a real question, and the day a backend
/// gains a genuine primitive it must answer **here**, deliberately.
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    get_control_backend()
}
/// Returns the control backend every caller should use.
///
/// # Why this is the custom-painting backend even when `controls-native` is on
///
/// `controls-native` used to mean "hand widget creation to the host, which owns a
/// real control for this kind". That is no longer a thing: the host supplies a
/// window and a drawing surface, and the library paints every `WidgetKind`
/// (BLUE15 #55/#56). The `native` backend's control methods therefore forward to
/// `Platform` methods whose defaults report "no such control", so selecting it
/// here made every `rw_create_*` return `0`.
///
/// Choosing the custom backend unconditionally is the honest expression of "there
/// is one creation mechanism". It also closes the second creation path (BLUE15
/// §2.5/G-4): previously the C ABI and `lib.rs::create_*` resolved the backend
/// separately from `create_widget_of_kind`, so they could disagree about which
/// mechanism built a widget.
///
/// # When there is no custom backend
///
/// `controls-custom` is the backend that *is* the painting mechanism, so a build
/// without it has no control mechanism at all. Such a build (it is not part of any
/// shipped profile) gets `NoControlBackend`, whose members report absence — the
/// same rule as everywhere else: an unimplemented capability says so rather than
/// answering with a value that would look live.
pub fn get_control_backend() -> &'static dyn ControlBackend {
    #[cfg(feature = "controls-custom")]
    {
        custom_control_backend()
    }
    #[cfg(not(feature = "controls-custom"))]
    {
        no_control_backend()
    }
}
/// Return compile-time control policy label used by diagnostics and docs.
///
/// Under self-drawing there is exactly one policy, so the label is constant. The
/// function survives because the label is part of the diagnostics contract: a
/// reader of a runtime trace should be able to see *which* mechanism a build
/// selected, and that answer must not require reading `#[cfg]` attributes.
pub fn active_control_policy() -> &'static str {
    "self-drawn"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(widgets_unstripped)]
    use crate::widget::WidgetKind;

    #[test]
    fn get_control_backend_returns_valid_backend() {
        let backend = get_control_backend();
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
        let kind = backend.kind();
        let _ = format!("{:?}", kind);
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn get_control_backend_for_widget_returns_non_null() {
        let backend = get_control_backend_for_widget(WidgetKind::Button);
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty for Button");
        let backend2 = get_control_backend_for_widget(WidgetKind::Canvas);
        let name2 = backend2.backend_name();
        assert!(!name2.is_empty(), "backend_name must not be empty for Canvas");
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn get_control_backend_for_widget_various_kinds() {
        let kinds = [
            WidgetKind::Window,
            WidgetKind::Button,
            WidgetKind::Label,
            WidgetKind::Canvas,
            WidgetKind::Table,
            WidgetKind::TextEdit,
            WidgetKind::Slider,
            WidgetKind::MenuBar,
        ];
        for kind in &kinds {
            let backend = get_control_backend_for_widget(*kind);
            let name = backend.backend_name();
            assert!(!name.is_empty(), "backend_name must not be empty for {:?}", kind);
            let _ = backend.kind();
        }
    }

    #[test]
    fn active_control_policy_is_the_single_mechanism() {
        assert_eq!(
            active_control_policy(),
            "self-drawn",
            "the policy label must name the one mechanism that exists",
        );
    }

    #[test]
    fn control_backend_is_send_sync() {
        let backend = get_control_backend();
        let _: &(dyn ControlBackend + Send + Sync) = backend;
    }

    /// Supports the "one mechanism" claim: the per-kind entry point and the
    /// general one must be the *same value*, for every sampled kind. Two different
    /// backends here is exactly how the second creation path used to exist.
    ///
    /// Gated on the full widget set because the sample spans categories that
    /// reduced profiles compile out; the claim itself is profile-independent, since
    /// [`get_control_backend_for_widget`] ignores its argument.
    #[cfg(all(feature = "controls-custom", full_widgets))]
    #[test]
    fn every_kind_resolves_to_the_same_backend() {
        let expected = get_control_backend();
        // Kinds chosen to span the categories the old two-tier table used to
        // separate: base, view, container, dialog and self-drawn kinds.
        let sample = [
            WidgetKind::Button,
            WidgetKind::Label,
            WidgetKind::Slider,
            WidgetKind::Canvas,
            WidgetKind::Table,
            WidgetKind::DatePicker,
            WidgetKind::PopupWindow,
            WidgetKind::Arc,
            WidgetKind::MiniChart,
            WidgetKind::MenuBar,
        ];
        for kind in &sample {
            let resolved = get_control_backend_for_widget(*kind);
            assert!(
                std::ptr::eq(resolved, expected),
                "WidgetKind::{kind:?} resolved to a different backend than the general entry",
            );
        }
    }
}
