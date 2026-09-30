// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The uniform widget-property `Platform` methods, implemented once over
//! [`BackendState`](crate::platform::state::BackendState).
//!
//! # The defect this removes
//!
//! `Platform` publishes a uniform property API — `set_widget_value` / `widget_value`,
//! `set_widget_checked` / `is_widget_checked`, `set_widget_placeholder` /
//! `widget_placeholder`, and about thirty more. It exists so application code can drive any
//! control on any host without asking which backend it is talking to.
//!
//! Measured before this module existed: **one** backend of eleven implemented it
//! (`StubPlatform`), and the other ten inherited the trait defaults — `false` for every
//! setter, `None`/empty for every getter. So `slider.set_value(42)` worked on the portable
//! host and silently did nothing on Windows, macOS, Linux, Android, iOS, Harmony, Wayland
//! and wasm. Nothing reported the failure: a setter returns `bool` and the callers that
//! ignore it, ignore it.
//!
//! Ten copies of forty methods is not the fix — that is the "76 mechanical migrations" shape
//! this crate has already paid for. Every one of those ten backends stores its widget state
//! in the *same* `BackendState`, so the methods have exactly one implementation, and it is
//! here.
//!
//! # Which backends use it
//!
//! Any backend whose `Platform` impl has a `state: BackendState<K>` field. That is the
//! state tier — `stub`, `mobile`, `harmony`, `ios`, `android`, `wasm`, `macos_objc2`,
//! `wayland` — plus the three desktop backends for the properties they do **not** answer
//! natively.
//!
//! # Why a macro rather than a trait with a blanket impl
//!
//! A blanket `impl<T: HasState> Platform for T` would be the cleaner shape, but Rust has no
//! negative reasoning: it would overlap with every hand-written `impl Platform for X` and
//! neither could be selected. A macro expands *inside* the existing `impl Platform` block, so
//! each backend keeps its own implementation and adds these methods to it — and a backend
//! that wants a real native answer for one property simply writes it and omits that line.
//!
//! # What each method answers, and what it deliberately does not
//!
//! These are **state** implementations: they read and write the per-widget record. That is
//! the correct answer for a property the host does not own — "is this check box checked" is
//! the control's own fact, and the library paints every control itself, so there is no OS
//! object that could disagree. A backend that *does* own a native control for a kind (none
//! does today; every `create_*` is a library concern since BLUE15) overrides the specific
//! method rather than using this expansion for it.
//!
//! The `false` an unknown id gets is therefore honest, not a stub: it says "no such widget
//! here", which is exactly the question `contains_widget` answers.

use crate::core::ObjectId;

/// Whether `widget_id` names a **checkable** control in the widget layer.
///
/// # Why this lives here rather than in the widget layer's caller
///
/// Tri-state mode and the checked property only exist for a control that can be checked, and
/// the widget layer is the authority on which kinds those are — a second list kept in the
/// platform layer would eventually disagree with it. Both this module's macro and the
/// hand-written `stub` implementation read the same widget-layer fact through this one
/// function.
///
/// # Why a registry-free profile answers `false`
///
/// `mini` compiles no widget objects at all, so no control exists that could carry the
/// property. `false` is the truthful answer there, and reaching for `widget::runtime` would
/// not compile.
#[cfg(not(alloc_frugal))]
pub(crate) fn widget_is_checkable(widget_id: ObjectId) -> bool {
    crate::widget::runtime::widget_is_checkable(widget_id)
}

/// See the `not(alloc_frugal)` definition: a build with no widget layer has no checkable
/// control.
#[cfg(alloc_frugal)]
pub(crate) fn widget_is_checkable(_widget_id: ObjectId) -> bool {
    false
}

/// Implements the uniform widget-property `Platform` methods over `self.state`.
///
/// Expands to method definitions that belong inside an existing
/// `impl Platform for YourBackend` block; it is not a standalone `impl`.
///
/// The backend must have a field named `state` holding a
/// [`BackendState`](crate::platform::state::BackendState). Every method reads it directly,
/// so no accessor is needed on the backend itself.
///
/// # Placement
///
/// Call it anywhere inside the `impl Platform` block. The expansion is order-independent —
/// Rust resolves methods regardless of source order — but placing it near the other property
/// methods keeps the block readable.
#[macro_export]
macro_rules! impl_platform_state_properties {
    () => {
        // ── Text, geometry, enablement, visibility ──────────────────────────────
        //
        // These four already have hand-written implementations on the desktop backends (which
        // route them to real toolkit objects where one exists) and on the state tier. The macro
        // supplies them for any backend that has not written its own; a backend that has one
        // keeps it, because a duplicate method in the same `impl` block is a compile error and
        // that is the signal to drop the macro's line.

        fn set_widget_text(&self, widget_id: $crate::core::ObjectId, text: &str) {
            // `BackendState::set_text` reports whether the id exists; the trait's signature has
            // no channel for that, so the answer is dropped rather than invented. A caller that
            // needs to know whether the write landed reads [`Self::get_widget_text`] back.
            let _ = self.state.set_text(widget_id, text);
        }

        fn get_widget_text(&self, widget_id: $crate::core::ObjectId) -> $crate::compat::String {
            self.state.text(widget_id)
        }

        fn set_widget_geometry(
            &self,
            widget_id: $crate::core::ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) {
            self.state.set_geometry(widget_id, x, y, width, height);
        }

        fn set_widget_enabled(&self, widget_id: $crate::core::ObjectId, enabled: bool) {
            self.state.set_enabled(widget_id, enabled);
        }

        fn is_widget_enabled(&self, widget_id: $crate::core::ObjectId) -> bool {
            self.state.enabled(widget_id)
        }

        fn set_widget_visible(&self, widget_id: $crate::core::ObjectId, visible: bool) {
            self.state.set_visible(widget_id, visible);
        }

        /// Shows a control.
        ///
        /// # Why this is in the macro and not left to the trait default
        ///
        /// The default is an empty body, so `show_widget` was a silent no-op on every
        /// backend that did not write one — measured before this: `android`, `wasm` and
        /// `macos_objc2`. On the hosts where the library paints into a frame the host pulls
        /// (which is exactly those three), the *visible* flag is the whole story of whether
        /// the control is drawn, so the no-op meant `show_widget` silently did nothing.
        ///
        /// # This is the state answer; a native surface overrides it
        ///
        /// On `windows`, `macos` and `linux/gtk` a top-level window has a real toolkit object
        /// and those backends implement this themselves (show/hide the `HWND`/`NSWindow`/
        /// `gtk::Widget`). A backend that does the same must delete this line and write its
        /// own — a duplicate method in one `impl` block is a compile error, which is the
        /// signal, not a silent preference.
        fn show_widget(&self, widget_id: $crate::core::ObjectId) {
            self.state.set_visible(widget_id, true);
        }

        /// Hides a control. See [`show_widget`](Self::show_widget) for why this is here and
        /// what a native-surface backend must do instead.
        fn hide_widget(&self, widget_id: $crate::core::ObjectId) {
            self.state.set_visible(widget_id, false);
        }

        fn is_widget_visible(&self, widget_id: $crate::core::ObjectId) -> bool {
            self.state.visible(widget_id)
        }

        // ── Numeric value ───────────────────────────────────────────────────────
        //
        // A record holds a numeric value only if its creator seeded one, and each control
        // seeds exactly the properties its kind has. That is the per-control answer — a slider
        // accepts a value, a button does not — without a global classification table here.

        fn set_widget_value(&self, widget_id: $crate::core::ObjectId, value: f64) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_value(widget_id, value)
        }

        fn widget_value(&self, widget_id: $crate::core::ObjectId) -> Option<f64> {
            self.state.value(widget_id)
        }

        fn set_widget_range(&self, widget_id: $crate::core::ObjectId, min: f64, max: f64) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_range(widget_id, min, max)
        }

        fn widget_range(&self, widget_id: $crate::core::ObjectId) -> Option<(f64, f64)> {
            self.state.range(widget_id)
        }

        fn set_widget_step(&self, widget_id: $crate::core::ObjectId, step: f64) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_step(widget_id, step)
        }

        fn widget_step(&self, widget_id: $crate::core::ObjectId) -> Option<f64> {
            self.state.step(widget_id)
        }

        // ── Selection ───────────────────────────────────────────────────────────

        fn set_widget_selected_index(
            &self,
            widget_id: $crate::core::ObjectId,
            index: Option<usize>,
        ) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_selected_index(widget_id, index)
        }

        fn widget_selected_index(&self, widget_id: $crate::core::ObjectId) -> Option<usize> {
            self.state.selected_index(widget_id)
        }

        fn set_widget_selection(
            &self,
            widget_id: $crate::core::ObjectId,
            start: u32,
            end: u32,
        ) -> bool {
            self.state.set_selection(widget_id, start, end)
        }

        fn widget_selection(&self, widget_id: $crate::core::ObjectId) -> Option<(u32, u32)> {
            self.state.selection(widget_id)
        }

        // ── Checkable controls ──────────────────────────────────────────────────

        fn set_widget_checked(&self, widget_id: $crate::core::ObjectId, checked: bool) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_checked(widget_id, checked)
        }

        fn is_widget_checked(&self, widget_id: $crate::core::ObjectId) -> Option<bool> {
            self.state.checked(widget_id)
        }

        /// Enables tri-state mode on a *checkable* control.
        ///
        /// Whether a control can be tri-state is a property of the **widget**, not the host,
        /// so the answer comes from the widget layer's own property table rather than a second
        /// list of kinds kept here — two places having to agree on which controls are
        /// checkable is the duplication that eventually disagrees.
        ///
        /// A registry-free profile (`mini`) has no widget objects at all, so no control could
        /// carry tri-state and the request is refused without reaching for a module that is
        /// not compiled in.
        fn set_widget_tristate(&self, widget_id: $crate::core::ObjectId, enabled: bool) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            if !$crate::platform::state_impl::widget_is_checkable(widget_id) {
                return false;
            }
            self.state.set_tristate(widget_id, enabled)
        }

        /// Reads tri-state mode, agreeing with the write gate above.
        ///
        /// A label never has tri-state mode, so asking answers `None` rather than a stored
        /// `false` that would imply the question was meaningful.
        fn is_widget_tristate(&self, widget_id: $crate::core::ObjectId) -> Option<bool> {
            if !$crate::platform::state_impl::widget_is_checkable(widget_id) {
                return None;
            }
            self.state.tristate(widget_id)
        }

        /// Puts a radio button into a named mutually-exclusive group.
        fn set_widget_group(&self, widget_id: $crate::core::ObjectId, group: &str) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_group(widget_id, group)
        }

        fn widget_group(
            &self,
            widget_id: $crate::core::ObjectId,
        ) -> Option<$crate::compat::String> {
            self.state.group(widget_id)
        }

        // ── Text-entry presentation ─────────────────────────────────────────────

        fn set_widget_placeholder(&self, widget_id: $crate::core::ObjectId, text: &str) -> bool {
            self.state.set_placeholder(widget_id, text)
        }

        fn widget_placeholder(
            &self,
            widget_id: $crate::core::ObjectId,
        ) -> Option<$crate::compat::String> {
            self.state.placeholder(widget_id)
        }

        fn set_widget_echo_mode(
            &self,
            widget_id: $crate::core::ObjectId,
            mode: $crate::platform::EchoMode,
        ) -> bool {
            self.state.set_echo_mode(widget_id, mode)
        }

        fn widget_echo_mode(
            &self,
            widget_id: $crate::core::ObjectId,
        ) -> Option<$crate::platform::EchoMode> {
            self.state.echo_mode(widget_id)
        }

        fn set_widget_read_only(&self, widget_id: $crate::core::ObjectId, read_only: bool) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_read_only(widget_id, read_only)
        }

        fn is_widget_read_only(&self, widget_id: $crate::core::ObjectId) -> Option<bool> {
            self.state.read_only(widget_id)
        }

        fn set_widget_max_length(
            &self,
            widget_id: $crate::core::ObjectId,
            max_length: u32,
        ) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_max_length(widget_id, max_length)
        }

        fn widget_max_length(&self, widget_id: $crate::core::ObjectId) -> Option<u32> {
            self.state.max_length(widget_id)
        }

        // ── Progress / slider / scroll ──────────────────────────────────────────

        fn set_widget_indeterminate(
            &self,
            widget_id: $crate::core::ObjectId,
            indeterminate: bool,
        ) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_indeterminate(widget_id, indeterminate)
        }

        fn is_widget_indeterminate(&self, widget_id: $crate::core::ObjectId) -> Option<bool> {
            self.state.indeterminate(widget_id)
        }

        fn set_slider_orientation(
            &self,
            widget_id: $crate::core::ObjectId,
            orientation: $crate::core::Orientation,
        ) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_orientation(widget_id, orientation)
        }

        fn slider_orientation(
            &self,
            widget_id: $crate::core::ObjectId,
        ) -> Option<$crate::core::Orientation> {
            self.state.orientation(widget_id)
        }

        fn set_widget_scroll_position(
            &self,
            widget_id: $crate::core::ObjectId,
            x: i32,
            y: i32,
        ) -> bool {
            if !self.state.contains_widget(widget_id) {
                return false;
            }
            self.state.set_scroll(widget_id, x, y)
        }

        fn widget_scroll_position(&self, widget_id: $crate::core::ObjectId) -> Option<(i32, i32)> {
            self.state.scroll(widget_id)
        }

        // ── Window state ────────────────────────────────────────────────────────
        //
        // Only a window has window state: the record is `None` for anything else, so an
        // ordinary control honestly reports refusal here rather than a fabricated answer.

        fn set_window_state(
            &self,
            widget_id: $crate::core::ObjectId,
            flag: $crate::platform::WindowStateFlag,
            on: bool,
        ) -> bool {
            self.state.set_window_state(widget_id, flag, on)
        }

        fn is_window_in_state(
            &self,
            widget_id: $crate::core::ObjectId,
            flag: $crate::platform::WindowStateFlag,
        ) -> Option<bool> {
            self.state.window_state(widget_id, flag)
        }

        fn set_window_min_size(
            &self,
            widget_id: $crate::core::ObjectId,
            width: u32,
            height: u32,
        ) -> bool {
            self.state.set_window_min_size(widget_id, width, height)
        }

        fn window_min_size(&self, widget_id: $crate::core::ObjectId) -> Option<(u32, u32)> {
            self.state.window_min_size(widget_id)
        }

        fn set_window_icon(&self, widget_id: $crate::core::ObjectId, path: &str) -> bool {
            self.state.set_window_icon(widget_id, path)
        }

        fn window_icon(&self, widget_id: $crate::core::ObjectId) -> Option<$crate::compat::String> {
            self.state.window_icon(widget_id)
        }

        // ── IME / accessibility metadata, clipboard, drag-and-drop ──────────────
        //
        // Metadata, not bridges: a backend with a real IME or accessibility integration
        // overrides `ime_bridge`/`accessibility_bridge` separately, and these store the
        // per-widget flags an upper layer reads back.

        fn set_widget_ime_enabled(&self, widget_id: $crate::core::ObjectId, enabled: bool) -> bool {
            self.state.set_ime_enabled(widget_id, enabled)
        }

        fn is_widget_ime_enabled(&self, widget_id: $crate::core::ObjectId) -> bool {
            self.state.ime_enabled(widget_id)
        }

        fn set_widget_accessibility_name(
            &self,
            widget_id: $crate::core::ObjectId,
            name: &str,
        ) -> bool {
            self.state.set_accessibility_name(widget_id, name)
        }

        fn get_widget_accessibility_name(
            &self,
            widget_id: $crate::core::ObjectId,
        ) -> $crate::compat::String {
            self.state.accessibility_name(widget_id)
        }

        fn begin_drag(
            &self,
            source_widget_id: $crate::core::ObjectId,
            mime: &str,
            payload: &[u8],
        ) -> bool {
            self.state.begin_drag(source_widget_id, mime, payload)
        }

        fn poll_drop_event(&self) -> Option<$crate::platform::DropEvent> {
            self.state.pop_drop_event()
        }

        fn inject_drop_event(&self, event: $crate::platform::DropEvent) -> bool {
            self.state.inject_drop_event(event)
        }
    };
}
