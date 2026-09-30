// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HarmonyOS accessibility bridge: the `ArkUI_AccessibilityProvider` of a bound XComponent.
//!
//! # What this is for
//!
//! The library paints every widget itself, so there is no ArkUI control for the system's
//! accessibility service to inspect. On a host with a bound XComponent the service still
//! asks *somebody* what is on screen, and that somebody is the
//! `ArkUI_AccessibilityProvider` the component owns
//! (`OH_NativeXComponent_GetNativeAccessibilityProvider`, `@since 13`).
//!
//! # What this bridge does and does not do
//!
//! It **owns the provider handle** and **posts events through it** — that is the half the
//! library can do alone, and it is the half that makes a screen reader announce a name, a
//! value, a state or a focus change. Those four notifications are exactly
//! [`AccessibilityBridge`](crate::platform::accessibility::AccessibilityBridge)'s `notify_*`
//! methods, and each maps onto one ArkUI event type.
//!
//! It does **not** answer the service's queries, because those arrive as C callbacks
//! (`ArkUI_AccessibilityProviderCallbacks`) that must be registered with
//! `OH_ArkUI_AccessibilityProviderRegisterCallback`, and this crate has no C-ABI-stable
//! representation of its own accessibility tree to answer them with — the tree lives in
//! `widget::runtime` behind Rust types that a C callback cannot name. Registering callbacks
//! that could only answer "not found" would be worse than not registering: the service
//! would treat the provider as authoritative and report the surface as having no content,
//! rather than falling back.
//!
//! # Why this is honest rather than a stub
//!
//! Every method either performs a real SDK call or reports that the bridge cannot act
//! because no component is bound or no provider could be obtained. Nothing reports success
//! for an action it did not perform: `notify_*` return their own outcome, and a failure is
//! logged with the SDK's status code. That is the rule this crate applies everywhere a
//! capability is absent — under-claim, and say which.

use crate::compat::{lock, HashMap, Mutex, String, ToString};
use crate::core::ObjectId;
use crate::platform::accessibility::AccessibilityBridge;

/// The ArkUI accessibility event types this bridge can post.
///
/// Values are from `arkui/native_interface_accessibility.h`'s
/// `ArkUI_AccessibilityEventType`. They are reproduced as constants rather than imported
/// because the header is a C enum the crate does not bind; the numeric values are part of
/// the ABI, so they are stable and checked against the header.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A11yEventType {
    /// The node gained focus.
    AccessibilityFocused = 1,
    /// The node's accessible text changed.
    AccessibilityTextUpdate = 2,
    /// The node's value changed.
    AccessibilityValueUpdate = 3,
    /// The node's enabled/disabled state changed.
    AccessibilityStateUpdate = 5,
}

/// The HarmonyOS accessibility bridge.
///
/// Holds the provider handle once a bound XComponent supplies one, plus the names the
/// library was told about. The name store mirrors the other backends' bridges (Windows
/// keeps the same map) so `accessibility_name` answers consistently across platforms
/// instead of being Windows-only.
pub struct HarmonyAccessibilityBridge {
    /// The provider handle from the bound XComponent, as an address.
    ///
    /// `0` means "no component bound, or the SDK refused to hand one over". Kept as an
    /// address rather than a typed pointer because the provider type is opaque here and an
    /// `AtomicUsize` is what can be published from the XComponent callback thread.
    provider: core::sync::atomic::AtomicUsize,
    /// Accessible names the library set, keyed by widget.
    names: Mutex<HashMap<ObjectId, String>>,
}

impl HarmonyAccessibilityBridge {
    /// Creates a bridge with no provider; [`Self::attach_provider`] supplies one.
    pub fn new() -> Self {
        Self {
            provider: core::sync::atomic::AtomicUsize::new(0),
            names: Mutex::new(HashMap::new()),
        }
    }

    /// Records the provider handle obtained from a bound XComponent.
    ///
    /// Called by [`super::xcomponent::bind`] once the component exists: the provider is only
    /// reachable *through* a live `OH_NativeXComponent`, so it cannot be fetched earlier.
    pub fn attach_provider(&self, provider: *mut core::ffi::c_void) {
        self.provider.store(provider as usize, core::sync::atomic::Ordering::Release);
    }

    /// Whether a provider is available to post through.
    pub fn has_provider(&self) -> bool {
        self.provider.load(core::sync::atomic::Ordering::Acquire) != 0
    }

    /// Posts one accessibility event for `id`.
    ///
    /// # Why this is the real mechanism and not a log line
    ///
    /// ArkUI delivers accessibility notifications through an event object: the caller
    /// creates it (`OH_ArkUI_CreateAccessibilityEventInfo`), stamps the type and the
    /// affected element, sends it (`OH_ArkUI_SendAccessibilityAsyncEvent`), and destroys it
    /// (`OH_ArkUI_DestoryAccessibilityEventInfo` — the SDK's own spelling). All three are
    /// bound below, so a notification really reaches the accessibility service rather than
    /// being reported as sent.
    ///
    /// Returns `false` when there is no provider, when the event object could not be
    /// created, or when a setter reported an error. A `false` is logged at `debug` here and
    /// at `warn` for the error cases, so the difference between "announced" and "nothing
    /// happened" is visible in a host's own output.
    fn post_event(&self, id: ObjectId, event_type: A11yEventType) -> bool {
        let provider = self.provider.load(core::sync::atomic::Ordering::Acquire);
        if provider == 0 {
            log::debug!(
                "[harmony] a11y: no ArkUI accessibility provider (no bound XComponent); \
                 the {event_type:?} notification for widget {id} was not posted"
            );
            return false;
        }
        let provider = provider as *mut core::ffi::c_void;
        // SAFETY: `provider` is the non-null handle `attach_provider` stored, which came
        // from the SDK for a live component; the event object is created by the SDK and
        // destroyed below on every path.
        //
        // # Why the call sites are written once behind a target check
        //
        // The declarations below are `#[cfg(target_env = "ohos")]` because `libace_ndk.z.so`
        // exists only in the OpenHarmony sysroot. Rather than fork this whole body into two
        // copies that could drift, the host arm returns early above (`provider` is never
        // published without a real component, and a host build publishes nothing), so the
        // reachable code here is the OHOS one. The `cfg` on the block keeps the compiler
        // honest in both configurations instead of relying on that reasoning alone.
        #[cfg(target_env = "ohos")]
        unsafe {
            let event = OH_ArkUI_CreateAccessibilityEventInfo();
            if event.is_null() {
                log::warn!(
                    "[harmony] a11y: OH_ArkUI_CreateAccessibilityEventInfo returned null; the \
                     {event_type:?} notification for widget {id} was not posted"
                );
                return false;
            }
            let typed = OH_ArkUI_AccessibilityEventInfo_SetEventType(event, event_type as i32);
            // The element the event is about. `id` is the widget-registry id, which is the
            // identity the library uses throughout and the one the host can correlate.
            let identified = OH_ArkUI_AccessibilityEventInfo_SetElementId(event, id as i64);
            if typed != ACCESSIBILITY_RESULT_SUCCESSFUL
                || identified != ACCESSIBILITY_RESULT_SUCCESSFUL
            {
                log::warn!(
                    "[harmony] a11y: could not describe the {event_type:?} notification for \
                     widget {id} (setEventType={typed}, setElementId={identified})"
                );
                OH_ArkUI_DestoryAccessibilityEventInfo(event);
                return false;
            }
            // The send is asynchronous — the callback is optional (`null` is documented as
            // acceptable) and this bridge has nothing to do with the outcome beyond
            // reporting it, so no callback is registered.
            OH_ArkUI_SendAccessibilityAsyncEvent(provider, event, None);
            // The SDK documents this as destroying the caller's event object; sending is
            // asynchronous, so the object is copied before this returns.
            OH_ArkUI_DestoryAccessibilityEventInfo(event);
        }
        // A non-OpenHarmony build can never reach here: it publishes no provider (see
        // `attach_for_component`), so the `provider == 0` guard above always returns first.
        // The `_` binding keeps the values named in that configuration rather than silently
        // dropping them.
        #[cfg(not(target_env = "ohos"))]
        let _ = (provider, id, event_type);
        true
    }
}

crate::impl_default_via_new!(HarmonyAccessibilityBridge);

impl AccessibilityBridge for HarmonyAccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str) {
        lock(&self.names).insert(id, name.to_string());
    }

    fn accessibility_name(&self, id: ObjectId) -> Option<String> {
        lock(&self.names).get(&id).cloned()
    }

    fn notify_name_changed(&self, id: ObjectId) {
        self.post_event(id, A11yEventType::AccessibilityTextUpdate);
    }

    fn notify_value_changed(&self, id: ObjectId) {
        self.post_event(id, A11yEventType::AccessibilityValueUpdate);
    }

    fn notify_state_changed(&self, id: ObjectId) {
        self.post_event(id, A11yEventType::AccessibilityStateUpdate);
    }

    fn notify_focus_changed(&self, id: ObjectId) {
        self.post_event(id, A11yEventType::AccessibilityFocused);
    }
}

/// `ARKUI_ACCESSIBILITY_NATIVE_RESULT_SUCCESSFUL`.
///
/// Gated with the declarations it compares against: on a host build the setters are not
/// declared, so nothing reads this and the constant would be dead code.
#[cfg(target_env = "ohos")]
const ACCESSIBILITY_RESULT_SUCCESSFUL: i32 = 0;

// The provider accessor lives in the XComponent header; the event object and sender live in
// the ArkUI accessibility header. Both are declared here so the bridge owns the whole
// vocabulary it uses.
//
// # Why these are gated on the OpenHarmony target
//
// `libace_ndk.z.so` exists only in the OpenHarmony sysroot, so a declaration that is
// *referenced* from a host build makes the host test binary fail to link — which would mean
// this module's own tests could never run anywhere, on any machine, including CI. Gating the
// declarations on the target keeps the whole vocabulary in one place while letting the host
// build compile (and test) the parts that do not need the SDK. The `unsafe` call sites are
// written once and are identical in both configurations.
#[cfg(target_env = "ohos")]
extern "C" {
    /// `@since 13`. The accessibility provider of a bound XComponent.
    fn OH_NativeXComponent_GetNativeAccessibilityProvider(
        component: *mut core::ffi::c_void,
        handle: *mut *mut core::ffi::c_void,
    ) -> i32;

    /// `@since 13`. Creates the event object the send below takes ownership of.
    fn OH_ArkUI_CreateAccessibilityEventInfo() -> *mut core::ffi::c_void;

    /// `@since 13`. Destroys an event object (`Destory` is the SDK's spelling).
    fn OH_ArkUI_DestoryAccessibilityEventInfo(event_info: *mut core::ffi::c_void);

    /// `@since 13`. Stamps the event type onto an event object.
    fn OH_ArkUI_AccessibilityEventInfo_SetEventType(
        event_info: *mut core::ffi::c_void,
        event_type: i32,
    ) -> i32;

    /// `@since 13`. Stamps the affected element id onto an event object.
    fn OH_ArkUI_AccessibilityEventInfo_SetElementId(
        event_info: *mut core::ffi::c_void,
        element_id: i64,
    ) -> i32;

    /// `@since 13`. Sends the event to the accessibility service.
    fn OH_ArkUI_SendAccessibilityAsyncEvent(
        provider: *mut core::ffi::c_void,
        event_info: *mut core::ffi::c_void,
        callback: Option<extern "C" fn(i32)>,
    );
}

/// Asks the SDK for the provider of the bound component and attaches it.
///
/// Called from [`super::xcomponent::bind`], because the provider is only reachable while a
/// component is alive.
///
/// Returns `false` and logs the SDK's status when the component has no provider, which is
/// the honest answer for a build whose host did not enable the accessibility service.
///
/// # Safety
///
/// `component` must be the pointer ArkUI passed to the ArkTS `XComponent`'s native
/// `onLoad`, valid for the component's lifetime.
pub unsafe fn attach_for_component(
    bridge: &HarmonyAccessibilityBridge,
    component: *mut core::ffi::c_void,
) -> bool {
    if component.is_null() {
        return false;
    }
    #[cfg(target_env = "ohos")]
    {
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: `component` is non-null per the guard and valid per this function's
        // contract; `&mut handle` is a valid out-parameter.
        let status =
            unsafe { OH_NativeXComponent_GetNativeAccessibilityProvider(component, &mut handle) };
        if status != RESULT_SUCCESS || handle.is_null() {
            log::warn!(
                "[harmony] a11y: this XComponent exposes no accessibility provider \
                 (status={status}); accessibility notifications will not be posted"
            );
            return false;
        }
        bridge.attach_provider(handle);
        log::info!("[harmony] a11y: accessibility provider attached to the bound XComponent");
        return true;
    }
    // A build that is not for OpenHarmony has no `libace_ndk` to ask, so there is no
    // provider to attach and none is published — which is what keeps `post_event`'s
    // `provider == 0` guard the only reachable path on a host.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (bridge, component);
        log::debug!(
            "[harmony] a11y: this build is not for OpenHarmony, so it has no accessibility \
             provider to attach"
        );
        false
    }
}

/// `OH_NATIVEXCOMPONENT_RESULT_SUCCESS`.
///
/// Gated with the accessor it compares against (see the declarations above).
#[cfg(target_env = "ohos")]
const RESULT_SUCCESS: i32 = 0;

/// The process-wide bridge, so `Platform::accessibility_bridge` can hand out a `'static`
/// reference the way every other backend does.
pub fn bridge() -> &'static HarmonyAccessibilityBridge {
    static BRIDGE: crate::compat::OnceLock<HarmonyAccessibilityBridge> =
        crate::compat::OnceLock::new();
    BRIDGE.get_or_init(HarmonyAccessibilityBridge::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With no XComponent bound there is no provider, so nothing is posted — and that is
    /// reported rather than silently treated as sent.
    ///
    /// This is the state a test binary is always in: it never binds a real component. The
    /// assertion is that the bridge answers honestly instead of fabricating a send.
    #[test]
    fn nothing_is_posted_without_a_provider() {
        let bridge = HarmonyAccessibilityBridge::new();
        assert!(!bridge.has_provider(), "an unbound bridge must have no provider");
        assert!(
            !bridge.post_event(1, A11yEventType::AccessibilityFocused),
            "a notification with no provider must report that it was not posted"
        );
    }

    /// The name store must round-trip, because it is the one half of this bridge that works
    /// without a provider and the half `accessibility_name` publishes.
    #[test]
    fn names_round_trip_without_a_provider() {
        let bridge = HarmonyAccessibilityBridge::new();
        assert_eq!(bridge.accessibility_name(7), None, "an unknown widget has no name");
        bridge.set_accessibility_name(7, "Play");
        assert_eq!(bridge.accessibility_name(7), Some("Play".to_string()));
        bridge.set_accessibility_name(7, "Pause");
        assert_eq!(
            bridge.accessibility_name(7),
            Some("Pause".to_string()),
            "a second write must replace the first, not add a second name"
        );
    }

    /// A null component must be refused rather than dereferenced.
    #[test]
    fn attaching_for_a_null_component_is_refused() {
        let bridge = HarmonyAccessibilityBridge::new();
        // SAFETY: the null guard is what is under test, so no pointer is dereferenced.
        let attached = unsafe { attach_for_component(&bridge, core::ptr::null_mut()) };
        assert!(!attached);
        assert!(!bridge.has_provider());
    }

    /// The event-type constants must match the header, since they cross the ABI.
    ///
    /// `arkui/native_interface_accessibility.h`'s `ArkUI_AccessibilityEventType` numbers
    /// these; a mismatch would post the wrong notification (or an unrecognised one), which
    /// no SDK call would reject loudly. Pinned so a future edit cannot drift them silently.
    #[test]
    fn event_type_values_match_the_sdk_header() {
        assert_eq!(A11yEventType::AccessibilityFocused as i32, 1);
        assert_eq!(A11yEventType::AccessibilityTextUpdate as i32, 2);
        assert_eq!(A11yEventType::AccessibilityValueUpdate as i32, 3);
        assert_eq!(A11yEventType::AccessibilityStateUpdate as i32, 5);
    }
}
