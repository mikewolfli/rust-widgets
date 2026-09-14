// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Control backend abstraction for the library-painted control path.
use crate::core::ObjectId;
use crate::platform::WidgetTriggerEvent;
use alloc::collections::VecDeque;
/// Control backend family, used to report which implementation is active.
///
/// Both variants are library-painted now; the distinction that remains is whether
/// a backend wraps platform primitives or owns its surface outright, which
/// diagnostics and tests report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBackendKind {
    /// Backend that wraps platform-provided primitives where they exist.
    Native,
    /// Backend that paints every control on a surface it owns.
    Custom,
}
/// Policy answer for a widget kind's creation route.
///
/// Kept as an enum rather than deleted: [`crate::control_backend::routing`] returns
/// a single value today, but a backend that gains a real primitive must be able to
/// say so **deliberately** rather than through an implicit fallback (BLUE15 §七).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRoutePreference {
    /// Prefer a platform-provided primitive where one exists.
    NativePreferred,
    /// The library paints this kind on the host surface.
    CustomRequired,
}

/// State the backend owns that does **not** belong to a widget.
///
/// # What was removed and why
///
/// This type used to hold six maps mirroring widget state — `texts`, `enabled`,
/// `visible`, `ime_enabled`, `accessibility_names` and per-widget geometry — plus a
/// `widget_properties` entry for every control. That was a second copy of data the
/// widget already owned, so the two could disagree, and it meant `create_*` could
/// return an id with no widget behind it (BLUE15 §10.3).
///
/// Control state now lives in the widget, reached through
/// [`crate::widget::runtime`]. What remains here is only what a widget cannot hold:
///
/// - `ime_enabled` — describes how the *host* routes composition events to a
///   control, which is a backend policy rather than a control property.
/// - `accessibility_names` — an override supplied by the host for assistive
///   technology; a control's own name is derived from its properties.
/// - the two trigger queues — pending events produced by the backend's input
///   handling and consumed by [`crate::ControlBackend::poll_widget_trigger_event`].
#[derive(Default)]
pub(crate) struct CustomControlState {
    /// Host policy: does this control accept composition input?
    pub(crate) ime_enabled: crate::compat::HashMap<ObjectId, bool>,
    /// Host-supplied accessible name, overriding the control's own.
    pub(crate) accessibility_names: crate::compat::HashMap<ObjectId, String>,
    /// Menu activations awaiting delivery.
    #[cfg(not(embedded_surface))]
    pub(crate) menu_trigger_queue: VecDeque<ObjectId>,
    /// Widget activations awaiting delivery.
    pub(crate) widget_trigger_queue: VecDeque<WidgetTriggerEvent>,
}

/// Unit tests for the remaining backend-owned state.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_backend_kind_debug_and_eq() {
        assert_eq!(ControlBackendKind::Native, ControlBackendKind::Native);
        assert_eq!(ControlBackendKind::Custom, ControlBackendKind::Custom);
        assert_ne!(ControlBackendKind::Native, ControlBackendKind::Custom);
        let _ = format!("{:?}", ControlBackendKind::Native);
        let _ = format!("{:?}", ControlBackendKind::Custom);
    }

    #[test]
    fn control_route_preference_debug_and_eq() {
        assert_eq!(
            ControlRoutePreference::NativePreferred,
            ControlRoutePreference::NativePreferred
        );
        assert_eq!(ControlRoutePreference::CustomRequired, ControlRoutePreference::CustomRequired);
        assert_ne!(ControlRoutePreference::NativePreferred, ControlRoutePreference::CustomRequired);
        let _ = format!("{:?}", ControlRoutePreference::NativePreferred);
        let _ = format!("{:?}", ControlRoutePreference::CustomRequired);
    }

    /// The remaining state must start empty: anything pre-populated would be a
    /// hidden default that a control could inherit without asking.
    #[test]
    fn custom_control_state_starts_empty() {
        let state = CustomControlState::default();
        assert!(state.ime_enabled.is_empty());
        assert!(state.accessibility_names.is_empty());
        #[cfg(not(embedded_surface))]
        assert!(state.menu_trigger_queue.is_empty());
        assert!(state.widget_trigger_queue.is_empty());
    }
}
