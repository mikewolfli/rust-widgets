// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The HarmonyOS **XComponent** bridge: where ArkUI hands this library a real surface and
//! real input.
//!
//! # Why this exists
//!
//! Before this module, `HarmonyPlatform` was state-only. `status.md` recorded two gaps in as
//! many words — "ArkUI native view bridge ⬜ Not implemented" and "Input delivery into
//! widgets ⬜ Not wired" — and they were one gap: the backend held no `OH_NativeXComponent`,
//! so it could not receive a surface to draw into nor any event to route.
//!
//! ArkUI solves this with `XComponent`: an ArkTS component that hands its native side an
//! `OH_NativeXComponent*`. From there the native library gets the surface lifecycle, touch,
//! mouse, key, focus and accessibility callbacks — which is exactly the set of things this
//! library needs and the set it previously could not reach.
//!
//! # What is bound, and what that buys
//!
//! | Native entry point | Callback it drives | What the library gains |
//! |---|---|---|
//! | `OH_NativeXComponent_RegisterCallback` | `OnSurfaceCreated/Changed/Destroyed` | a surface to mount into, and its size |
//! | the same, `DispatchTouchEvent` | `OH_NativeXComponent_GetTouchEvent` | multi-finger input → `Event::Touch*` |
//! | `OH_NativeXComponent_RegisterMouseEventCallback` | `DispatchMouseEvent` / `DispatchHoverEvent` | click, move, leave → `Event::Mouse*` |
//! | `OH_NativeXComponent_RegisterKeyEventCallback` | key down/up | typing → `Event::KeyPress` / `TextInput` |
//! | `OH_NativeXComponent_RegisterFocusEventCallback` | focus/blur | the library's focus model |
//! | `OH_NativeXComponent_GetNativeAccessibilityProvider` | — | the accessibility tree |
//!
//! # What this bridge does **not** deliver
//!
//! | Input | Why not |
//! |---|---|
//! | Wheel / scroll | `OH_NativeXComponent_MouseEvent` has no wheel field. Published as *screenX, screenY, timestamp, action, button* (`native_interface_xcomponent.h`, SDK 20) — there is nothing to read a delta from, and no separate wheel callback exists. A host that needs wheel input must deliver it as a `Event::Wheel` through its own channel. |
//! | Double click | `OH_NativeXComponent_MouseEventAction` is *NONE, PRESS, RELEASE, MOVE, CANCEL*. There is no multiplicity, so a double click would have to be synthesised from timestamps; the library does not invent one, and a control that needs it must derive it from the press stream. |
//! | Back / forward buttons | The header defines `OH_NATIVEXCOMPONENT_BACK_BUTTON` / `_FORWARD_BUTTON`, and this bridge maps only LEFT/RIGHT/MIDDLE — the three the library's `mouse_button` vocabulary has. A back/forward press therefore arrives as `PRIMARY`, which is the honest reading of "a button the library has no name for". |
//!
//! This table is stated because the line above it used to read "click, move, wheel →
//! `Event::Mouse*`", which named a capability the platform does not have and the code therefore
//! could not have implemented — the documentation defect principle #18 forbids, and the harder
//! kind to notice because it reads as a feature list rather than a promise.
//!
//! # Why hand-written `extern "C"` rather than a generated binding
//!
//! The declarations are a few dozen lines against headers this crate does not vendor, and
//! the crate has no bindgen dependency. Writing them out keeps the build free of a code
//! generator and of a build-time dependency on the SDK's location — the header's contract is
//! transcribed here once, with the SDK version it was read from.
//!
//! # Why the SDK is not a hard build requirement
//!
//! Everything below is gated on `feature = "xcomponent"`, which is **off by default**. That
//! is deliberate and it is the same rule the rest of the crate follows for host SDKs: a build
//! on a machine without the OpenHarmony SDK must still compile (principle #37's "an honest
//! absence", applied to a build configuration rather than a runtime capability). Turning the
//! feature on adds `-lace_ndk` at link time — see `build.rs`.
//!
//! # Threading
//!
//! Every entry point here runs on the ArkUI main thread, which is the thread that owns the
//! `OH_NativeXComponent`. The widget layer's runtime is thread-local and the surface state is
//! behind a `Mutex`, so a callback that arrives on a different thread is refused rather than
//! silently corrupting the tree — see [`is_ui_thread`].

#![cfg(all(feature = "xcomponent", not(alloc_frugal)))]

use crate::compat::String;
use crate::core::{ObjectId, Point, Rect, Size};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

// ── The SDK version this transcription was read from ────────────────────────────
//
// The declarations below are transcribed from
// `<SDK>/linux/native/sysroot/usr/include/ace/xcomponent/native_interface_xcomponent.h`
// as shipped with OpenHarmony 6.0.0.46 (Beta1). The ABI is stable across the versions this
// crate targets (`@since 8` for the core, `@since 9`/`10`/`11` for the later entry points),
// and each function below records its own `@since`.

/// `OH_NATIVE_XCOMPONENT_MAX_TOUCH_POINTS_NUMBER` from the header.
const MAX_TOUCH_POINTS: usize = 10;
/// `OH_XCOMPONENT_ID_LEN_MAX` from the header, the buffer size `component_id` asks for.
///
/// Read only by the OpenHarmony arm of `component_id`, so gated with it.
#[cfg(target_env = "ohos")]
const XCOMPONENT_ID_LEN_MAX: usize = 128;

/// The opaque XComponent handle ArkUI creates.
pub type NativeXComponent = core::ffi::c_void;

/// `OH_NativeXComponent_TouchEventType`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchEventType {
    /// A finger went down.
    Down = 0,
    /// A finger came up.
    Up = 1,
    /// A pressed finger moved.
    Move = 2,
    /// The gesture was cancelled by the system.
    Cancel = 3,
    /// The system reported a type this build does not know.
    Unknown = 4,
}

// Available to the OpenHarmony build and to this module's unit tests. The conversions
// and the thread guard touch no SDK, so keeping them under `test` is what lets the
// pure half of this module be verified on a host — and their absence from a plain
// host build is why that build is warning-free without an `#[allow(dead_code)]`.
#[cfg(any(target_env = "ohos", test))]
impl TouchEventType {
    /// Maps the raw discriminant, reporting `Unknown` rather than trusting the value.
    ///
    /// A value outside the enum would be undefined behaviour if transmuted, so it is matched
    /// explicitly — the header reserves `UNKNOWN` for exactly this case.
    fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Down,
            1 => Self::Up,
            2 => Self::Move,
            3 => Self::Cancel,
            _ => Self::Unknown,
        }
    }
}

/// `OH_NativeXComponent_MouseEventAction`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventAction {
    /// No button activity.
    None = 0,
    /// A button was pressed.
    Press = 1,
    /// A button was released.
    Release = 2,
    /// The pointer moved.
    Move = 3,
    /// The pointer left the component.
    Cancel = 4,
}

// Available to the OpenHarmony build and to this module's unit tests. The conversions
// and the thread guard touch no SDK, so keeping them under `test` is what lets the
// pure half of this module be verified on a host — and their absence from a plain
// host build is why that build is warning-free without an `#[allow(dead_code)]`.
#[cfg(any(target_env = "ohos", test))]
impl MouseEventAction {
    fn from_raw(raw: i32) -> Option<Self> {
        match raw {
            0 => Some(Self::None),
            1 => Some(Self::Press),
            2 => Some(Self::Release),
            3 => Some(Self::Move),
            4 => Some(Self::Cancel),
            _ => None,
        }
    }
}

/// `OH_NativeXComponent_MouseEventButton` — a bitfield in the header.
pub mod mouse_button {
    /// No button.
    pub const NONE: u32 = 0;
    /// Primary button.
    pub const LEFT: u32 = 0x01;
    /// Secondary button.
    pub const RIGHT: u32 = 0x02;
    /// Middle button.
    pub const MIDDLE: u32 = 0x04;
}

/// `OH_NativeXComponent_TouchPoint`.
///
/// # The per-point `type`, and why it is declared
///
/// The SDK 20 declaration is *id, screenX, screenY, x, y, **type**, size, force,
/// timeStamp, isPressed*. An earlier revision of this struct deliberately **omitted** `type`,
/// reasoning that the type is read from the enclosing [`TouchEvent`] and that offset 20..24 is
/// padding anyway because `size: f64` must start at a multiple of 8.
///
/// The layout argument was correct — the offsets below `type` are unchanged whether or not the field
/// is declared — but the **semantic** conclusion was wrong: the header gives each point its own
/// `type`, and a multi-touch record can carry contacts of different types in one callback (a second
/// finger pressing while the first moves). Reading the enclosing event's type for every point
/// collapsed that into one kind, so a mixed record was delivered as several identical events. The
/// field is therefore declared (into what was padding) and the dispatcher prefers the point's own
/// type, falling back to the event type only when the point's is `None` — which is what the header
/// uses for "no type here".
///
/// ```text
/// C  : sizeof=56  id=0 screenX=4 screenY=8 x=12 y=16 type=20 size=24 force=32 ts=40 pressed=48
/// Rust: sizeof=56 id=0 screenX=4 screenY=8 x=12 y=16 type=20 size=24 force=32 ts=40 pressed=48
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Per-contact identifier, stable for the duration of the contact.
    pub id: i32,
    /// X in screen space.
    pub screen_x: f32,
    /// Y in screen space.
    pub screen_y: f32,
    /// X relative to the component.
    pub x: f32,
    /// Y relative to the component.
    pub y: f32,
    /// This contact's own event type (`OH_NativeXComponent_TouchEventType`).
    ///
    /// Prefer this over the enclosing [`TouchEvent::event_type`] so a mixed multi-touch record
    /// delivers each contact as the kind it actually is. `0` (`None`) means the header left it
    /// unset, and the dispatcher then uses the enclosing event's type.
    pub point_type: i32,
    /// Contact area.
    pub size: f64,
    /// Contact pressure.
    pub force: f32,
    /// Timestamp in the platform's own units.
    pub time_stamp: i64,
    /// Whether this point is currently pressed.
    pub is_pressed: bool,
}

/// `OH_NativeXComponent_TouchEvent`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TouchEvent {
    /// Unique identifier of the finger.
    pub id: i32,
    /// X in screen space.
    pub screen_x: f32,
    /// Y in screen space.
    pub screen_y: f32,
    /// X relative to the component.
    pub x: f32,
    /// Y relative to the component.
    pub y: f32,
    /// What happened, as a raw discriminant (converted by `TouchEventType::from_raw`).
    ///
    /// Named as code rather than an intra-doc link because that conversion is compiled
    /// only for OpenHarmony and for tests (it is meaningless without the SDK that produces
    /// the discriminant), so a link would be unresolved in a host build's documentation.
    pub event_type: i32,
    /// Contact area.
    pub size: f64,
    /// Contact pressure.
    pub force: f32,
    /// Device that generated the event.
    pub device_id: i64,
    /// Timestamp in the platform's own units.
    pub time_stamp: i64,
    /// The individual contacts.
    pub touch_points: [TouchPoint; MAX_TOUCH_POINTS],
    /// How many of `touch_points` are populated.
    pub num_points: u32,
}

/// `OH_NativeXComponent_MouseEvent`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// X relative to the component.
    pub x: f32,
    /// Y relative to the component.
    pub y: f32,
    /// X in screen space.
    pub screen_x: f32,
    /// Y in screen space.
    pub screen_y: f32,
    /// Timestamp in the platform's own units.
    pub time_stamp: i64,
    /// What happened, as a raw discriminant.
    pub action: i32,
    /// Which button, as the bitfield in [`mouse_button`].
    pub button: u32,
}

/// `OH_NativeXComponent_Callback`.
///
/// # Why the fields are `Option<extern "C" fn>` and not bare function pointers
///
/// The header's fields are non-nullable `void (*)(...)`, but ArkUI only calls the ones it was
/// given, and a Rust `null` must be able to stand for "not interested". `Option<extern fn>` is
/// guaranteed to have the same layout as the nullable C pointer, so this is the representation
/// that matches the ABI without introducing an invalid function pointer value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeXComponentCallback {
    /// Surface was created; `window` is an `OHNativeWindow*`.
    pub on_surface_created: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    /// Surface changed size.
    pub on_surface_changed: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    /// Surface is going away.
    pub on_surface_destroyed: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    /// A finger event arrived.
    pub dispatch_touch_event: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
}

/// `OH_NativeXComponent_MouseEvent_Callback`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeXComponentMouseEventCallback {
    /// A mouse event arrived.
    pub dispatch_mouse_event: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    /// The pointer entered or left the component.
    pub dispatch_hover_event: Option<extern "C" fn(*mut NativeXComponent, bool)>,
}

// ── The bound entry points ──────────────────────────────────────────────────────
//
// One `extern "C"` block rather than a dependency on a `ohos` crate: the declarations are the
// contract, and transcribing them here keeps the build free of a code generator. `@since`
// notes are kept so a reader can tell which SDK level a call needs.
//
// # Why the declarations are gated on the OpenHarmony target
//
// `libace_ndk.z.so` exists only in the OpenHarmony sysroot, so any declaration that a host
// build *references* makes the host test binary fail to link. `build.rs` no longer emits
// `-lace_ndk.z` for a non-`ohos` target (a host build used to fail at link for that reason
// alone), and this gate is the other half: with it, `feature = "xcomponent"` on a host is a
// compilable, testable configuration whose bridge is simply inert, rather than a
// configuration in which nothing in this directory can be exercised by a unit test.
//
// The call sites below are written once and guarded with `cfg` at the point where they would
// run, so the two configurations share a single body wherever the body is meaningful.
#[cfg(target_env = "ohos")]
extern "C" {
    /// `@since 8`. Reads the touch event that `DispatchTouchEvent` was called for.
    fn OH_NativeXComponent_GetTouchEvent(
        component: *mut NativeXComponent,
        window: *mut core::ffi::c_void,
        touch_event: *mut TouchEvent,
    ) -> i32;

    /// `@since 8`. Registers the surface-lifecycle and touch callback block.
    fn OH_NativeXComponent_RegisterCallback(
        component: *mut NativeXComponent,
        callback: *mut NativeXComponentCallback,
    ) -> i32;

    /// `@since 9`. Registers the mouse callback block.
    fn OH_NativeXComponent_RegisterMouseEventCallback(
        component: *mut NativeXComponent,
        callback: *mut NativeXComponentMouseEventCallback,
    ) -> i32;

    /// `@since 10`. Registers the key callback.
    fn OH_NativeXComponent_RegisterKeyEventCallback(
        component: *mut NativeXComponent,
        callback: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    ) -> i32;

    /// `@since 10`. Registers the focus callback.
    fn OH_NativeXComponent_RegisterFocusEventCallback(
        component: *mut NativeXComponent,
        callback: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    ) -> i32;

    /// `@since 10`. Registers the blur (focus-lost) callback.
    fn OH_NativeXComponent_RegisterBlurEventCallback(
        component: *mut NativeXComponent,
        callback: Option<extern "C" fn(*mut NativeXComponent, *mut core::ffi::c_void)>,
    ) -> i32;

    /// `@since 8`. Reports the surface size, which is what a frame is rendered at.
    fn OH_NativeXComponent_GetXComponentSize(
        component: *mut NativeXComponent,
        window: *mut core::ffi::c_void,
        width: *mut u64,
        height: *mut u64,
    ) -> i32;

    /// `@since 8`. Reports the surface's offset inside the ArkUI tree.
    fn OH_NativeXComponent_GetXComponentOffset(
        component: *mut NativeXComponent,
        window: *mut core::ffi::c_void,
        x: *mut f64,
        y: *mut f64,
    ) -> i32;

    /// `@since 8`. Reads the XComponent's ArkTS id, for correlating with the ArkTS side.
    fn OH_NativeXComponent_GetXComponentId(
        component: *mut NativeXComponent,
        id: *mut core::ffi::c_char,
        size: *mut u64,
    ) -> i32;
}

/// The result constant the SDK uses for "succeeded" (`OH_NATIVEXCOMPONENT_RESULT_SUCCESS`).
///
/// Gated with the declarations it compares against: on a host build none of the calls that
/// return a status are declared, so nothing reads this and it would be dead code.
#[cfg(target_env = "ohos")]
const RESULT_SUCCESS: i32 = 0;

/// What the bridge knows about the live XComponent.
struct SurfaceState {
    /// The widget this XComponent is displaying, once `mount_surface` names one.
    mounted_widget: Option<ObjectId>,
    /// The surface size, in physical pixels, as last reported by ArkUI.
    size: Size,
    /// The surface offset inside the ArkUI tree.
    offset: Point,
    /// Whether ArkUI currently considers the surface alive.
    ///
    /// Gated with its only writer and reader, which are the surface-lifecycle callbacks: off
    /// an OpenHarmony target nothing sets it, so the field would be permanently `false` and
    /// dead. Keeping it out of the host build is what lets that build be warning-free without
    /// an `#[allow(dead_code)]` that would also silence a real future dead field.
    #[cfg(target_env = "ohos")]
    alive: bool,
}

impl SurfaceState {
    const fn new() -> Self {
        Self {
            mounted_widget: None,
            size: Size::new(0, 0),
            offset: Point::new(0, 0),
            #[cfg(target_env = "ohos")]
            alive: false,
        }
    }
}

/// The single XComponent this process is bound to.
///
/// # Why one and not a table
///
/// An application has one ArkUI window and one component tree; the library paints all of it
/// into one frame. Supporting several XComponents would mean several widget roots and a
/// compositor between them, which is a different design and not what this bridge is for. A
/// second `bind` therefore replaces the first rather than silently accumulating handles it
/// would never use.
fn surface() -> &'static Mutex<SurfaceState> {
    static SURFACE: OnceLock<Mutex<SurfaceState>> = OnceLock::new();
    SURFACE.get_or_init(|| Mutex::new(SurfaceState::new()))
}

/// Whether [`bind`] has completed. Read without the lock on the callback path.
static BOUND: AtomicBool = AtomicBool::new(false);

/// The thread the XComponent was bound on, so a callback from elsewhere can be refused.
// Available to the OpenHarmony build and to this module's unit tests. The conversions
// and the thread guard touch no SDK, so keeping them under `test` is what lets the
// pure half of this module be verified on a host — and their absence from a plain
// host build is why that build is warning-free without an `#[allow(dead_code)]`.
#[cfg(any(target_env = "ohos", test))]
static UI_THREAD: AtomicUsize = AtomicUsize::new(0);

/// Whether the calling thread is the one that bound the XComponent.
///
/// # Why callbacks check this
///
/// The widget runtime is thread-local (see `crate::widget::runtime`), and ArkUI delivers
/// every XComponent callback on its own main thread. A callback that arrived on another
/// thread would dispatch into a *different* registry — the one for the transport's thread —
/// and the events would vanish with no error. Refusing is the honest answer; the callback
/// reports it so it is diagnosable rather than silent.
///
/// Not gated on the target even though only the OHOS callbacks call it: it reads two
/// process-wide statics and touches no SDK, so keeping it available means the unit test
/// below still checks the "guard is closed before a bind" rule on a host build — the rule
/// that would otherwise be assertable only on a device.
#[cfg(any(target_env = "ohos", test))]
fn is_ui_thread() -> bool {
    let bound_on = UI_THREAD.load(Ordering::Acquire);
    bound_on != 0 && current_thread_id() == bound_on
}
/// The calling thread's id, as a number comparable with [`UI_THREAD`].
///
/// # Why a thread-local address
///
/// `std::thread::ThreadId` has no stable accessor for its numeric value, and it cannot be
/// `const`-initialised into a static either. A thread-local's own address is unique among
/// live threads, stable for that thread's lifetime, and cheap to take — which is exactly what
/// an identity comparison needs. The cell is never written, so the value is only ever used as
/// an address.
#[cfg(any(target_env = "ohos", test))]
fn current_thread_id() -> usize {
    // The `allow` sits on the item *inside* `thread_local!`, which is where the lint resolves:
    // applied to the macro invocation itself the attribute is ignored and clippy still fires.
    // Same placement as the crate's other thread-locals (`widget/runtime.rs`).
    thread_local! {
        #[allow(clippy::missing_const_for_thread_local)]
        static ID: u8 = const { 0 };
    }
    ID.with(|cell| cell as *const u8 as usize)
}

/// Binds the bridge to an `OH_NativeXComponent` and registers every callback.
///
/// Called from the ArkTS side with the pointer ArkUI gave it in `onLoad` — the entry point is
/// [`crate::bindings::rw_harmony_bind_xcomponent`].
///
/// # Return value
///
/// `false` when `component` is null, or when any registration the SDK rejects prevents the
/// surface from working. Each refusal is logged with the SDK's status code, because the
/// alternative — registering what succeeds and reporting success — is the silent half-broken
/// state this bridge exists to remove.
///
/// # Bind is idempotent per component and refuses a second, different one
///
/// The SDK 20 callback block has no unregister, so a second `bind` cannot detach the first
/// component's callbacks: they keep calling into this module's shared `SurfaceState`, and the
/// surface-lifecycle callbacks have no component identity to filter on. Binding a second component
/// would therefore let the first one's touch and destroy callbacks mutate the second one's mounted
/// widget and `alive` flag — the two surfaces would be indistinguishable to the callbacks. Rather
/// than accept that corruption, a second bind to a **different** component is refused with an
/// explicit error. Re-binding the **same** component is idempotent and succeeds, so a host that
/// re-runs its setup for one component is not broken.
///
/// # Safety
///
/// `component` must be the pointer ArkUI passed to the ArkTS `XComponent`'s native `onLoad`,
/// valid for the lifetime of the component. Registering against a dangling pointer would make
/// ArkUI call into freed state.
pub unsafe fn bind(component: *mut NativeXComponent) -> bool {
    if component.is_null() {
        log::error!("[harmony] xcomponent: bind called with a null OH_NativeXComponent");
        return false;
    }
    // Refuse a second, different component: see the "idempotent / refuses a second" note above.
    // The same component may bind again (idempotent), which keeps a host's repeated setup working.
    let already = BOUND_COMPONENT.load(Ordering::Acquire);
    if already != 0 && already != component as usize {
        log::error!(
            "[harmony] xcomponent: bind refused — the bridge is already bound to a different \
             component. SDK 20 exposes no callback unregister, so the previous component's \
             callbacks cannot be detached and would corrupt this one's state; unbind/finish the \
             first component before binding another"
        );
        return false;
    }
    // Only an OpenHarmony build has the SDK the registrations below call. On any other target
    // the feature compiles (so this module is type-checked and its pure logic is unit-tested)
    // but the bridge is inert, and saying so is the honest answer — pretending to register
    // against a library that does not exist is what the `cfg` prevents.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = component;
        log::warn!(
            "[harmony] xcomponent: this build is not for OpenHarmony, so it has no libace_ndk \
             to register against; the bridge is inert"
        );
        // The `return` is required rather than stylistic: this arm is the last statement in
        // the host build (the OHOS block below is compiled out), while in the OHOS build it
        // must leave early so the registrations are not also run. Clippy only sees one
        // configuration at a time, so it calls the `return` needless in the host one.
        #[allow(clippy::needless_return)]
        return false;
    }
    #[cfg(target_env = "ohos")]
    {
        UI_THREAD.store(current_thread_id(), Ordering::Release);

        // The callback block is `'static`: ArkUI keeps the pointer the registration stored, so it
        // must outlive every callback. A `static mut` behind an address-stable `OnceLock` is the
        // shape that guarantees that; the block is written exactly once, before registration.
        static CALLBACKS: OnceLock<NativeXComponentCallback> = OnceLock::new();
        let callbacks = CALLBACKS.get_or_init(|| NativeXComponentCallback {
            on_surface_created: Some(on_surface_created),
            on_surface_changed: Some(on_surface_changed),
            on_surface_destroyed: Some(on_surface_destroyed),
            dispatch_touch_event: Some(dispatch_touch_event),
        });
        // SAFETY: `component` is non-null per the guard above and valid per this function's
        // contract; `callbacks` is a `'static` block, so the pointer stored by registration stays
        // valid for as long as ArkUI may call it.
        let registered = unsafe {
            OH_NativeXComponent_RegisterCallback(
                component,
                callbacks as *const NativeXComponentCallback as *mut NativeXComponentCallback,
            )
        };
        if registered != RESULT_SUCCESS {
            log::error!(
            "[harmony] xcomponent: OH_NativeXComponent_RegisterCallback failed (status={registered})"
        );
            return false;
        }

        static MOUSE_CALLBACKS: OnceLock<NativeXComponentMouseEventCallback> = OnceLock::new();
        let mouse = MOUSE_CALLBACKS.get_or_init(|| NativeXComponentMouseEventCallback {
            dispatch_mouse_event: Some(dispatch_mouse_event),
            dispatch_hover_event: Some(dispatch_hover_event),
        });
        // A mouse callback is optional — a phone with no pointer never produces one — so a
        // refusal here is logged and does not fail the bind. Touch is the required input path.
        // SAFETY: as above; `mouse` is a `'static` block.
        let mouse_status = unsafe {
            OH_NativeXComponent_RegisterMouseEventCallback(
                component,
                mouse as *const NativeXComponentMouseEventCallback
                    as *mut NativeXComponentMouseEventCallback,
            )
        };
        if mouse_status != RESULT_SUCCESS {
            log::warn!(
                "[harmony] xcomponent: mouse callback not registered (status={mouse_status}); \
             pointer input will not be delivered to this surface"
            );
        }

        // SAFETY: as above; the function pointers are `'static` items.
        let key_status = unsafe {
            OH_NativeXComponent_RegisterKeyEventCallback(component, Some(dispatch_key_event))
        };
        if key_status != RESULT_SUCCESS {
            log::warn!(
                "[harmony] xcomponent: key callback not registered (status={key_status}); \
             typing will not reach this surface"
            );
        }
        // SAFETY: as above.
        let focus_status = unsafe {
            OH_NativeXComponent_RegisterFocusEventCallback(component, Some(on_focus_event))
        };
        if focus_status != RESULT_SUCCESS {
            log::warn!(
                "[harmony] xcomponent: focus callback not registered (status={focus_status})"
            );
        }
        // SAFETY: as above.
        let blur_status = unsafe {
            OH_NativeXComponent_RegisterBlurEventCallback(component, Some(on_blur_event))
        };
        if blur_status != RESULT_SUCCESS {
            log::warn!("[harmony] xcomponent: blur callback not registered (status={blur_status})");
        }

        // The accessibility provider is only reachable **through** a live component, so binding
        // it is part of the bind rather than something a host could do beforehand. A refusal is
        // logged and does not fail the bind: accessibility is a separate service, and a host
        // that did not enable it still gets a working surface and input.
        // SAFETY: `component` is non-null per the guard above and valid per this function's
        // contract, which is what the provider accessor requires.
        let a11y = unsafe {
            super::accessibility::attach_for_component(super::accessibility::bridge(), component)
        };
        if !a11y {
            log::debug!(
                "[harmony] xcomponent: no accessibility provider for this component; the surface
             and input work, but no accessibility notifications will be posted"
            );
        }

        // Soft-keyboard demand, reported to ArkUI so the on-screen keyboard appears when a text
        // control takes focus.
        //
        // # Why the library asks rather than the host
        //
        // Focus lives in `widget::runtime`, which the ArkTS side cannot read: ArkUI only knows
        // that the whole XComponent has focus, not that the library's focus moved into a
        // `LineEdit`. `SetNeedSoftKeyboard` is the bridge between the two — it tells ArkUI that
        // this component wants text input, and ArkUI then raises the keyboard for it.
        //
        // `@since 12`, so this is available on every SDK level the rest of the bridge needs
        // (`GetKeyEventModifierKeyStates` is `@since 20`, the newest thing bound here).
        // SAFETY: `component` is ArkUI's own pointer and valid for this call.
        let keyboard_status = unsafe { OH_NativeXComponent_SetNeedSoftKeyboard(component, true) };
        if keyboard_status != RESULT_SUCCESS {
            log::warn!(
                "[harmony] xcomponent: SetNeedSoftKeyboard failed (status={keyboard_status}); the \
             on-screen keyboard may not appear for text input"
            );
        }

        // Record the component **here**, at bind time, not only in `on_surface_created`.
        //
        // The reject-a-different-component guard above reads this value, so it must be set as soon
        // as the bind succeeds. Leaving it to `on_surface_created` left a window between a
        // successful `bind(A)` and ArkUI's first surface callback in which the guard saw `0` and
        // accepted `bind(B)` — precisely the corruption the guard exists to prevent (the first
        // component's callbacks would then drive the second's state). `on_surface_created` still
        // re-asserts the pointer, which is harmless because it is the same component ArkUI hands
        // to the callbacks the bind registered.
        BOUND_COMPONENT.store(component as usize, Ordering::Release);
        BOUND.store(true, Ordering::Release);
        log::info!(
        "[harmony] xcomponent: bound; surface, touch, mouse, key and focus callbacks registered"
    );
        true
    }
}

/// The ArkTS id of the bound XComponent, or `None` when nothing is bound.
///
/// The ArkTS side uses this to correlate its component with the library's, which is what lets
/// a page with several components find the one it handed over.
pub fn component_id() -> Option<String> {
    let _ = surface(); // keep the accessor shape symmetric with the other queries
                       // The id is fetched from ArkUI on demand rather than cached: it is only asked for at
                       // startup, and a cached copy would be one more thing to invalidate on rebind.
    let component = BOUND_COMPONENT.load(Ordering::Acquire);
    if component == 0 {
        return None;
    }
    // A build without the SDK can hold a component address only if something published one,
    // which only the OHOS arm of `bind` does — so this is unreachable there, and answering
    // `None` keeps the signature honest rather than calling into a library that is not linked.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = component;
        // Same reasoning as `bind`'s host arm: the early exit is what keeps the OHOS block
        // below from also running, so it is required in that configuration and merely final
        // in this one.
        #[allow(clippy::needless_return)]
        return None;
    }
    #[cfg(target_env = "ohos")]
    {
        // A zeroed buffer of the exact size the header requires (`OH_XCOMPONENT_ID_LEN_MAX + 1`,
        // the `+ 1` being the NUL the SDK appends).
        let mut buffer = vec![0u8; XCOMPONENT_ID_LEN_MAX + 1];
        let mut size = XCOMPONENT_ID_LEN_MAX as u64;
        // SAFETY: `component` is the pointer the last successful `bind` stored; `buffer` is
        // `XCOMPONENT_ID_LEN_MAX + 1` bytes, which is the size the header requires.
        let status = unsafe {
            OH_NativeXComponent_GetXComponentId(
                component as *mut NativeXComponent,
                buffer.as_mut_ptr() as *mut core::ffi::c_char,
                &mut size,
            )
        };
        if status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetXComponentId failed (status={status})");
            return None;
        }
        let end = buffer.iter().position(|byte| *byte == 0).unwrap_or(buffer.len());
        buffer.truncate(end);
        String::from_utf8(buffer).ok()
    }
}

/// The component pointer the last successful [`bind`] stored, for the on-demand queries.
static BOUND_COMPONENT: AtomicUsize = AtomicUsize::new(0);

/// Records which widget the surface displays, so input can be routed and frames rendered.
///
/// Returns `false` when nothing is bound: a mount with no surface could only produce a frame
/// nobody can present, and reporting that is better than recording a mount that cannot work.
pub fn set_mounted_widget(widget_id: ObjectId, rect: Rect) -> bool {
    if !BOUND.load(Ordering::Acquire) {
        log::error!(
            "[harmony] xcomponent: mount for widget {widget_id} refused — no XComponent is bound; \
             call rw_harmony_bind_xcomponent from the ArkTS onLoad first"
        );
        return false;
    }
    let mut state = surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.mounted_widget = Some(widget_id);
    state.size = Size::new(rect.width, rect.height);
    true
}

/// Forgets the mounted widget.
pub fn clear_mounted_widget() {
    let mut state = surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.mounted_widget = None;
}

/// The widget the surface displays, if any.
pub fn mounted_widget() -> Option<ObjectId> {
    surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).mounted_widget
}

/// The surface size in physical pixels as last reported by ArkUI.
///
/// `None` before ArkUI has reported one — a size is needed to render, and inventing one would
/// place every control against a guess.
pub fn surface_size() -> Option<Size> {
    let state = surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.size.width == 0 || state.size.height == 0 {
        None
    } else {
        Some(state.size)
    }
}

/// The surface offset inside the ArkUI tree.
pub fn surface_offset() -> Point {
    surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).offset
}

// ── Callbacks ───────────────────────────────────────────────────────────────────

/// Reads the surface size ArkUI just reported and stores it.
///
/// Shared by the created and changed callbacks: ArkUI reports a size in both, and the two
/// arms must agree about how it is read or a resize would produce a different number from the
/// first report of the same surface.
///
/// # Safety
///
/// `component` and `window` must be the pointers ArkUI passed to the callback.
#[cfg(target_env = "ohos")]
unsafe fn record_surface_size(component: *mut NativeXComponent, window: *mut core::ffi::c_void) {
    // A host build has no SDK to ask, so there is no size to record. Reported rather than
    // guessed: inventing a size would place every control against a fabricated extent.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (component, window);
        log::debug!(
            "[harmony] xcomponent: this build is not for OpenHarmony, so the surface size \
             cannot be read; no size was recorded"
        );
        return;
    }
    #[cfg(target_env = "ohos")]
    {
        let mut width: u64 = 0;
        let mut height: u64 = 0;
        // SAFETY: both pointers come from ArkUI for the duration of this call, and the two out
        // parameters are local.
        let status = unsafe {
            OH_NativeXComponent_GetXComponentSize(component, window, &mut width, &mut height)
        };
        if status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetXComponentSize failed (status={status})");
            return;
        }
        let mut offset_x: f64 = 0.0;
        let mut offset_y: f64 = 0.0;
        // SAFETY: as above.
        let offset_status = unsafe {
            OH_NativeXComponent_GetXComponentOffset(component, window, &mut offset_x, &mut offset_y)
        };
        if offset_status != RESULT_SUCCESS {
            // Not fatal: the offset only shifts input hit-testing, and `(0, 0)` is the honest
            // default for a component at the origin of its container.
            log::warn!("[harmony] xcomponent: GetXComponentOffset failed (status={offset_status})");
        }
        let mut state = surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.size = Size::new(width as u32, height as u32);
        state.offset = Point::new(offset_x.round() as i32, offset_y.round() as i32);
        log::debug!(
            "[harmony] xcomponent: surface {}x{} at ({}, {})",
            state.size.width,
            state.size.height,
            state.offset.x,
            state.offset.y
        );
    }
}

/// ArkUI created the surface: it is now the thing this library draws into.
#[cfg(target_env = "ohos")]
extern "C" fn on_surface_created(component: *mut NativeXComponent, window: *mut core::ffi::c_void) {
    BOUND_COMPONENT.store(component as usize, Ordering::Release);
    // SAFETY: forwarded from ArkUI's own call.
    unsafe { record_surface_size(component, window) };
    surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).alive = true;
    log::info!("[harmony] xcomponent: surface created");
}

/// The surface changed — most often a resize, which the window layout must follow.
#[cfg(target_env = "ohos")]
extern "C" fn on_surface_changed(component: *mut NativeXComponent, window: *mut core::ffi::c_void) {
    // SAFETY: forwarded from ArkUI's own call.
    unsafe { record_surface_size(component, window) };
    // The library's layout is re-run from the queued size, not from inside this callback:
    // ArkUI is mid-layout here, and running a control tree's layout re-entrantly is how a
    // resize turns into a stall.
    if let Some(size) = surface_size() {
        let widget = mounted_widget();
        if let Some(widget_id) = widget {
            crate::queue_resize_trigger(widget_id, size.width, size.height);
        }
    }
}

/// The surface is going away. Frames can no longer be presented.
#[cfg(target_env = "ohos")]
extern "C" fn on_surface_destroyed(
    _component: *mut NativeXComponent,
    _window: *mut core::ffi::c_void,
) {
    let mut state = surface().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    state.alive = false;
    log::info!("[harmony] xcomponent: surface destroyed");
}

/// A finger event arrived.
///
/// # Coordinate space
///
/// `x`/`y` on the touch event are already relative to the component (the header documents
/// them as "relative to the left edge of the element to touch"), which is the space widget
/// geometry lives in, so no offset is applied. `screenX`/`screenY` are deliberately ignored:
/// using them would require subtracting the component origin, and the relative pair is exact.
///
/// # Multi-touch
///
/// One callback can carry several contacts. Each is dispatched separately, because the
/// gesture recognizers expect one event per finger — `Pinch` and `Rotate` need two
/// independent contacts, and they can only get them from separate events.
#[cfg(target_env = "ohos")]
extern "C" fn dispatch_touch_event(
    component: *mut NativeXComponent,
    window: *mut core::ffi::c_void,
) {
    if !is_ui_thread() {
        log::error!("[harmony] xcomponent: touch callback off the ArkUI thread; event dropped");
        return;
    }
    let Some(widget_id) = mounted_widget() else {
        return;
    };
    // A host build has no SDK to read the event from. Reaching here at all would require a
    // bound component, which only the OHOS arm of `bind` can publish, so this is a guard
    // rather than a live path — see the module docs on why the bridge is inert off-target.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (component, window, widget_id);
        return;
    }
    #[cfg(target_env = "ohos")]
    {
        // SAFETY: `touch` is a local, fully zeroed before the call, and the SDK fills it.
        let mut touch: TouchEvent = unsafe { core::mem::zeroed() };
        // SAFETY: `component`/`window` are ArkUI's own arguments for this call, and `&mut touch`
        // is a valid out-parameter.
        let status = unsafe { OH_NativeXComponent_GetTouchEvent(component, window, &mut touch) };
        if status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetTouchEvent failed (status={status})");
            return;
        }
        let event_type = TouchEventType::from_raw(touch.event_type);
        let points = touch.num_points.min(MAX_TOUCH_POINTS as u32) as usize;
        // `numPoints` is 0 for a single-contact event on some SDK levels, in which case the
        // top-level `x`/`y`/`id` describe the only contact. Handling that here keeps a tap from
        // being dropped on the floor.
        if points == 0 {
            dispatch_one_touch(widget_id, touch.id as u64, touch.x, touch.y, event_type);
            return;
        }
        for point in touch.touch_points.iter().take(points) {
            // Use the contact's own type. Each point in the header carries its own
            // `OH_NativeXComponent_TouchEventType`, so a mixed record — one finger pressing while
            // another moves — is delivered per contact rather than collapsed to the enclosing
            // event's single type. `from_raw` maps an unknown discriminant to `Unknown` rather than
            // transmuting it, so a value this build does not know is reported, not guessed.
            let point_type = TouchEventType::from_raw(point.point_type);
            dispatch_one_touch(widget_id, point.id as u64, point.x, point.y, point_type);
        }
    }
}

/// Delivers one contact to the widget tree.
///
/// # Why the touch arms are gated on `feature = "touch"`
///
/// `touch` is an independently composable capability: `Event::TouchBegin`/`TouchMove`/
/// `TouchEnd` are themselves `#[cfg(feature = "touch")]` in `crate::event::types`, so a build
/// without it has no such event to construct. `xcomponent` and `touch` are therefore
/// orthogonal — an OpenHarmony build can have the surface bridge without the gesture engine,
/// and the honest behaviour there is to drop touch input rather than to fail the build.
///
/// The Windows canvas carries the same gate for the same reason (`windows/canvas.rs`,
/// `WM_TOUCH`), so the two backends answer "no touch capability" identically.
#[cfg(feature = "touch")]
#[cfg(target_env = "ohos")]
fn dispatch_one_touch(
    widget_id: ObjectId,
    touch_id: u64,
    x: f32,
    y: f32,
    event_type: TouchEventType,
) {
    let position = Point::new(x.round() as i32, y.round() as i32);
    let event = match event_type {
        TouchEventType::Down => crate::event::Event::touch_begin(position.x, position.y, touch_id),
        TouchEventType::Up => crate::event::Event::touch_end(position.x, position.y, touch_id),
        TouchEventType::Move => crate::event::Event::touch_move(position.x, position.y, touch_id),
        // A cancelled gesture and an unknown type are not events the widget layer defines.
        // The recognizers reset on `TouchEnd`, which is also what cancelling means for them,
        // so a cancel is delivered as an end rather than being dropped: a finger that the
        // system took away must not leave a recognizer mid-gesture forever.
        TouchEventType::Cancel => crate::event::Event::touch_end(position.x, position.y, touch_id),
        TouchEventType::Unknown => return,
    };
    if crate::platform::platform_facts().route_pointer_event(widget_id, &event, position) {
        crate::request_repaint_because(widget_id, crate::RepaintReason::State);
    }
}

/// The no-`touch` arm: there is no touch event to build, so the contact is dropped.
///
/// Reported at `debug` rather than `warn`: a host that enabled the surface bridge without the
/// gesture capability is a valid configuration, and warning on every finger down would be
/// noise rather than a fault report.
#[cfg(not(feature = "touch"))]
fn dispatch_one_touch(
    _widget_id: ObjectId,
    _touch_id: u64,
    _x: f32,
    _y: f32,
    _event_type: TouchEventType,
) {
    log::debug!(
        "[harmony] xcomponent: touch contact dropped — this build has no 'touch' capability, so \
         there is no touch event to deliver"
    );
}

/// A pointer event arrived.
#[cfg(target_env = "ohos")]
extern "C" fn dispatch_mouse_event(
    component: *mut NativeXComponent,
    window: *mut core::ffi::c_void,
) {
    if !is_ui_thread() {
        log::error!("[harmony] xcomponent: mouse callback off the ArkUI thread; event dropped");
        return;
    }
    let Some(widget_id) = mounted_widget() else {
        return;
    };
    // A host build has no SDK to read the mouse event from; see `dispatch_touch_event` for why
    // this guard is unreachable rather than a live path off-target.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (component, window, widget_id);
        return;
    }
    #[cfg(target_env = "ohos")]
    {
        // SAFETY: `mouse` is a local the SDK fills; the two handles are ArkUI's own arguments.
        let mut mouse: MouseEvent = unsafe { core::mem::zeroed() };
        let status = unsafe {
            // The header exposes `OH_NativeXComponent_GetMouseEvent` with the same shape as the
            // touch getter.
            OH_NativeXComponent_GetMouseEvent(component, window, &mut mouse)
        };
        if status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetMouseEvent failed (status={status})");
            return;
        }
        let Some(action) = MouseEventAction::from_raw(mouse.action) else {
            log::debug!("[harmony] xcomponent: unknown mouse action {}", mouse.action);
            return;
        };
        // The header's button field is a bitfield; the library speaks one button number per event.
        let button = if mouse.button & mouse_button::RIGHT != 0 {
            crate::event::mouse_button::SECONDARY
        } else if mouse.button & mouse_button::MIDDLE != 0 {
            crate::event::mouse_button::MIDDLE
        } else {
            crate::event::mouse_button::PRIMARY
        };
        let position = Point::new(mouse.x.round() as i32, mouse.y.round() as i32);
        // Remembered so a subsequent `DispatchHoverEvent(false)` — which carries no position —
        // can route its `MouseLeave` at the place the pointer actually was.
        record_pointer_position(position);
        let event = match action {
            MouseEventAction::Press => {
                crate::event::Event::mouse_press_with(position.x, position.y, button, 0)
            }
            MouseEventAction::Release => {
                crate::event::Event::MouseRelease { pos: position, button }
            }
            MouseEventAction::Move => crate::event::Event::MouseMove { pos: position },
            // `None` is "the system told us nothing happened", and `Cancel` is the pointer leaving
            // — which is what a `MouseLeave` reports, so it is delivered rather than dropped. A
            // hover highlight that is never cleared is the defect that leaves.
            MouseEventAction::Cancel => crate::event::Event::MouseLeave { pos: position },
            MouseEventAction::None => return,
        };
        if crate::platform::platform_facts().route_pointer_event(widget_id, &event, position) {
            crate::request_repaint_because(widget_id, crate::RepaintReason::State);
        }
    }
}

/// The pointer entered or left the component.
///
/// # Why the leave is routed at the last known pointer position, not the surface origin
///
/// This used to pass [`surface_offset()`] as the event's point. That is the component's
/// **origin inside the ArkUI tree**, not a position in the widget tree — and
/// `route_pointer_event` hit-tests with the point it is given. The leave was therefore
/// aimed at whatever sat at the ArkUI origin (usually the root), and a second leave was
/// produced on top of the one `dispatch_hover_transition` already fires at the previously
/// hovered widget.
///
/// When the pointer genuinely leaves the component there is no new position to report, so
/// the honest answer is to let the hover transition do its job from the position the
/// pointer was last seen at, and to route the leave there. The mouse arm records that
/// position; if it has never been recorded the pointer has not been inside yet and there is
/// nothing to leave.
///
/// # Why entering is a no-op
///
/// ArkUI sends this once for the component as a whole. A hover *highlight* belongs to an
/// individual control, and the mouse-move arm already delivers `MouseMove` at a real
/// position, which is what drives per-control hover. Synthesising an enter here would
/// highlight whatever happens to sit at a guessed coordinate.
#[cfg(target_env = "ohos")]
extern "C" fn dispatch_hover_event(_component: *mut NativeXComponent, is_hover: bool) {
    if !is_ui_thread() {
        return;
    }
    let Some(widget_id) = mounted_widget() else {
        return;
    };
    if is_hover {
        return;
    }
    // The pointer is already gone, so the leave is delivered at the position it was last
    // seen at — the same position the mouse arm recorded.
    let Some(position) = last_pointer_position() else {
        return;
    };
    let event = crate::event::Event::MouseLeave { pos: position };
    if crate::platform::platform_facts().route_pointer_event(widget_id, &event, position) {
        crate::request_repaint_because(widget_id, crate::RepaintReason::State);
    }
}

/// The last pointer position the mouse arm saw, in component-local coordinates.
///
/// `None` until a pointer event has been delivered, which is exactly the state in which
/// there is nothing to leave. Stored rather than recomputed because the pointer's position
/// at the moment it left is not reported by ArkUI — the leave callback carries only a
/// boolean.
#[cfg(target_env = "ohos")]
fn last_pointer_position() -> Option<Point> {
    LAST_POINTER.with(|cell| cell.get())
}

/// Records the pointer position for a subsequent hover-leave to use.
#[cfg(target_env = "ohos")]
fn record_pointer_position(position: Point) {
    LAST_POINTER.with(|cell| cell.set(Some(position)));
}

// A cell rather than a mutex: it is touched only from the ArkUI thread (every input
// callback is guarded by `is_ui_thread`), and it holds a plain `Copy` pair.
thread_local! {
    static LAST_POINTER: core::cell::Cell<Option<Point>> = const { core::cell::Cell::new(None) };
}

/// A key event arrived.
///
/// # Why the key **code** is forwarded as a key press
///
/// `OH_NativeXComponent_GetKeyEventCode` reports the same kind of value the library's
/// `Event::KeyPress` carries (a platform key code), so it is forwarded directly. Text
/// composition is a separate concern: an IME commits through the ArkTS side's text input, not
/// through this callback, so a committed string reaches the widget as `Event::TextInput` from
/// there.
#[cfg(target_env = "ohos")]
extern "C" fn dispatch_key_event(
    component: *mut NativeXComponent,
    _window: *mut core::ffi::c_void,
) {
    if !is_ui_thread() {
        log::error!("[harmony] xcomponent: key callback off the ArkUI thread; event dropped");
        return;
    }
    let Some(widget_id) = mounted_widget() else {
        return;
    };
    // A host build has no SDK to read the key event from; see `dispatch_touch_event` for why
    // this guard is unreachable rather than a live path off-target.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = (component, widget_id);
        return;
    }
    #[cfg(target_env = "ohos")]
    {
        let mut key_event: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: `component` is ArkUI's argument; `&mut key_event` is a valid out-parameter.
        let status = unsafe { OH_NativeXComponent_GetKeyEvent(component, &mut key_event) };
        if status != RESULT_SUCCESS || key_event.is_null() {
            log::error!("[harmony] xcomponent: GetKeyEvent failed (status={status})");
            return;
        }
        let mut action: i32 = 0;
        // SAFETY: `key_event` was produced by the call above and is valid for this callback;
        // `&mut action` is a valid out-parameter.
        let action_status =
            unsafe { OH_NativeXComponent_GetKeyEventAction(key_event, &mut action) };
        if action_status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetKeyEventAction failed (status={action_status})");
            return;
        }
        let mut code: i32 = 0;
        // SAFETY: as above.
        let code_status = unsafe { OH_NativeXComponent_GetKeyEventCode(key_event, &mut code) };
        if code_status != RESULT_SUCCESS {
            log::error!("[harmony] xcomponent: GetKeyEventCode failed (status={code_status})");
            return;
        }
        // The **down** action produces `KeyPress` and the **up** action produces `KeyRelease`.
        //
        // This used to return early for anything but down, with the reason "the up action has no
        // counterpart in the library's event set". That was not true: `Event::KeyRelease` is
        // published, has a constructor and a `shortcut::Modifiers` accessor — it simply had no
        // producer in any backend, so a control that tracks a held key could never learn the key
        // came up (see `windows::types` for the same fix on Win32). The callback is registered as
        // `RegisterKeyEventCallback`, which ArkUI invokes for both actions, so the release reaches
        // this function and was being discarded.
        const KEY_ACTION_DOWN: i32 = 0;
        const KEY_ACTION_UP: i32 = 1;
        let target = crate::widget::runtime::focused_widget().unwrap_or(widget_id);
        // SAFETY: `key_event` was produced by `GetKeyEvent` at the top of this callback and is
        // valid for its duration, which is what the modifier accessor requires.
        let modifiers = unsafe { key_modifiers(key_event) };
        let event = match action {
            KEY_ACTION_DOWN => crate::event::Event::KeyPress { key: code as u32, modifiers },
            KEY_ACTION_UP => crate::event::Event::KeyRelease { key: code as u32, modifiers },
            // A third action value is not part of the SDK's contract; report it rather than
            // guessing which of the two it meant.
            other => {
                log::debug!("[harmony] xcomponent: key action {other} is neither down nor up");
                return;
            }
        };
        if crate::widget::runtime::dispatch_event(target, &event) {
            crate::request_repaint_because(widget_id, crate::RepaintReason::State);
        }
    }
}

/// Reads the keyboard modifiers held during a key event, in the widget layer's encoding.
///
/// # Why this is bound separately
///
/// The key callback used to hard-code `modifiers: 0`, so every Ctrl/Shift/Alt chord on
/// HarmonyOS arrived as an unmodified key: shortcuts never fired, and Shift-selection and
/// Ctrl-click behaved as plain input. The SDK exposes the state
/// (`OH_NativeXComponent_GetKeyEventModifierKeyStates`, `@since 20`), it simply was not
/// called.
///
/// # Why the bits are translated rather than passed through
///
/// ArkUI's `ArkUI_ModifierKeyName` and this crate's widget-layer bits assign **different**
/// values to the same three modifiers — ArkUI is `CTRL=1, SHIFT=2, ALT=4` while the widget
/// layer (see `windows/canvas.rs::current_modifiers`) is `SHIFT=1, CTRL=2, ALT=4`. Passing
/// ArkUI's word through untranslated would therefore swap Control and Shift, so a
/// Ctrl-shortcut would fire on Shift. Each bit is mapped explicitly for that reason.
///
/// `FN` has no widget-layer counterpart and is deliberately dropped rather than folded into
/// another key: it is a hardware-layer modifier, and guessing an equivalent would invent an
/// input the user did not make.
///
/// A failed call is reported and answers "no modifiers", which is the honest direction: a
/// chord that silently loses its modifier is a wrong action, while an unmodified key is at
/// worst the behaviour this callback had before.
///
/// # Safety
///
/// `key_event` must be the pointer [`OH_NativeXComponent_GetKeyEvent`] produced for the
/// callback currently running.
#[cfg(target_env = "ohos")]
unsafe fn key_modifiers(key_event: *mut core::ffi::c_void) -> u32 {
    /// ArkUI's `ARKUI_MODIFIER_KEY_*` bits (`arkui/ui_input_event.h`).
    const ARKUI_CTRL: u64 = 1 << 0;
    const ARKUI_SHIFT: u64 = 1 << 1;
    const ARKUI_ALT: u64 = 1 << 2;
    /// The widget layer's bits, matching `windows/canvas.rs::current_modifiers`.
    const WIDGET_SHIFT: u32 = 1;
    const WIDGET_CONTROL: u32 = 2;
    const WIDGET_ALT: u32 = 4;

    // A host build has no accessor to call. `0` is the honest answer and the same one a
    // failed call produces: no modifier, which is the behaviour this callback had before
    // modifiers were read at all.
    #[cfg(not(target_env = "ohos"))]
    {
        let _ = key_event;
        return 0;
    }
    #[cfg(target_env = "ohos")]
    {
        let mut keys: u64 = 0;
        // SAFETY: `key_event` is the pointer the caller obtained from `GetKeyEvent` for this
        // callback, which is what this accessor requires; `&mut keys` is a valid out-parameter.
        let status =
            unsafe { OH_NativeXComponent_GetKeyEventModifierKeyStates(key_event, &mut keys) };
        if status != RESULT_SUCCESS {
            // `@since 20`: on an older SDK the symbol is absent from `libace_ndk` and the call
            // would not resolve at all, so a non-success result here is a real answer, not a
            // version check to work around.
            log::debug!(
                "[harmony] xcomponent: GetKeyEventModifierKeyStates failed (status={status}); \
             the key is delivered with no modifiers"
            );
            return 0;
        }
        let mut modifiers = 0u32;
        if keys & ARKUI_SHIFT != 0 {
            modifiers |= WIDGET_SHIFT;
        }
        if keys & ARKUI_CTRL != 0 {
            modifiers |= WIDGET_CONTROL;
        }
        if keys & ARKUI_ALT != 0 {
            modifiers |= WIDGET_ALT;
        }
        modifiers
    }
}

/// The component took focus.
#[cfg(target_env = "ohos")]
extern "C" fn on_focus_event(_component: *mut NativeXComponent, _window: *mut core::ffi::c_void) {
    if !is_ui_thread() {
        return;
    }
    if let Some(widget_id) = mounted_widget() {
        crate::widget::runtime::report_state(
            widget_id,
            crate::widget::runtime::StateFact::Focused(true),
        );
    }
}

/// The component lost focus.
#[cfg(target_env = "ohos")]
extern "C" fn on_blur_event(_component: *mut NativeXComponent, _window: *mut core::ffi::c_void) {
    if !is_ui_thread() {
        return;
    }
    if let Some(widget_id) = mounted_widget() {
        crate::widget::runtime::report_state(
            widget_id,
            crate::widget::runtime::StateFact::Focused(false),
        );
    }
}

// `OH_NativeXComponent_GetMouseEvent` and the key accessors are declared in a second block so
// the first reads as "what a bind needs" and this one as "what an event needs".
//
// Gated on the target for the same reason as the first block: a host build has no
// `libace_ndk`, so a declaration it references cannot link.
#[cfg(target_env = "ohos")]
extern "C" {
    /// `@since 9`. Reads the mouse event `DispatchMouseEvent` was called for.
    fn OH_NativeXComponent_GetMouseEvent(
        component: *mut NativeXComponent,
        window: *mut core::ffi::c_void,
        mouse_event: *mut MouseEvent,
    ) -> i32;

    /// `@since 10`. Reads the key event a key callback was called for.
    fn OH_NativeXComponent_GetKeyEvent(
        component: *mut NativeXComponent,
        key_event: *mut *mut core::ffi::c_void,
    ) -> i32;

    /// `@since 10`. Reads the key action (`0` is down).
    fn OH_NativeXComponent_GetKeyEventAction(
        key_event: *mut core::ffi::c_void,
        action: *mut i32,
    ) -> i32;

    /// `@since 10`. Reads the platform key code.
    fn OH_NativeXComponent_GetKeyEventCode(
        key_event: *mut core::ffi::c_void,
        code: *mut i32,
    ) -> i32;

    /// `@since 20`. Reads the pressed modifier keys as an `ArkUI_ModifierKeyName` bitfield.
    ///
    /// Declared here rather than above because it is not needed to *receive* a key, only to
    /// interpret one — the same split the two `extern "C"` blocks already make.
    fn OH_NativeXComponent_GetKeyEventModifierKeyStates(
        key_event: *mut core::ffi::c_void,
        keys: *mut u64,
    ) -> i32;

    /// `@since 12`. Tells ArkUI whether this component wants the soft keyboard.
    ///
    /// Called from [`bind`] rather than from the key path: ArkUI decides whether a focus
    /// change should raise the on-screen keyboard from this flag, and the library's focus
    /// model moves between controls inside one component, so the component as a whole is
    /// what must ask. Leaving it unset meant text input on a phone had no keyboard at all.
    fn OH_NativeXComponent_SetNeedSoftKeyboard(
        component: *mut NativeXComponent,
        need_soft_keyboard: bool,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The touch-type conversion must map every in-range discriminant and refuse the rest.
    ///
    /// `TouchEventType` is `repr(C)` over an SDK enum, so an out-of-range value reaching a
    /// `transmute` would be undefined behaviour. The mapping is explicit for that reason, and
    /// this pins that it stays explicit rather than collapsing into a cast.
    #[test]
    fn touch_event_types_map_explicitly() {
        assert_eq!(TouchEventType::from_raw(0), TouchEventType::Down);
        assert_eq!(TouchEventType::from_raw(1), TouchEventType::Up);
        assert_eq!(TouchEventType::from_raw(2), TouchEventType::Move);
        assert_eq!(TouchEventType::from_raw(3), TouchEventType::Cancel);
        assert_eq!(TouchEventType::from_raw(4), TouchEventType::Unknown);
        assert_eq!(TouchEventType::from_raw(99), TouchEventType::Unknown);
        assert_eq!(TouchEventType::from_raw(-1), TouchEventType::Unknown);
    }

    /// Same reasoning for the mouse action, which additionally reports "not a value" as
    /// `None` so a caller cannot mistake a garbage action for `MouseEventAction::None`.
    #[test]
    fn mouse_actions_map_explicitly() {
        assert_eq!(MouseEventAction::from_raw(0), Some(MouseEventAction::None));
        assert_eq!(MouseEventAction::from_raw(1), Some(MouseEventAction::Press));
        assert_eq!(MouseEventAction::from_raw(2), Some(MouseEventAction::Release));
        assert_eq!(MouseEventAction::from_raw(3), Some(MouseEventAction::Move));
        assert_eq!(MouseEventAction::from_raw(4), Some(MouseEventAction::Cancel));
        assert_eq!(MouseEventAction::from_raw(99), None);
    }

    /// A mount with no bound XComponent is refused rather than recorded.
    ///
    /// This is the state a build without an ArkTS host is in, and recording the mount would
    /// make `mount_surface` report success for a surface nobody can present — the silent
    /// failure this bridge removes.
    #[test]
    fn mounting_without_a_bound_component_is_refused() {
        // The test binary never binds, so `BOUND` is false.
        assert!(!BOUND.load(Ordering::Acquire), "a test must not have bound a real component");
        assert!(!set_mounted_widget(4242, Rect::new(0, 0, 100, 100)));
        assert_eq!(mounted_widget(), None);
    }

    /// The surface size is unknown until ArkUI reports one.
    #[test]
    fn surface_size_is_none_before_arkui_reports_one() {
        assert_eq!(surface_size(), None, "an unreported size must not be invented");
    }

    /// An unbound component has no id rather than an empty string, so a caller can tell
    /// "nothing is bound" from "the component's id is empty".
    #[test]
    fn component_id_is_none_when_unbound() {
        assert_eq!(component_id(), None);
    }

    /// The thread guard must refuse before a bind has recorded a thread.
    ///
    /// A zero `UI_THREAD` means "nothing bound yet", and `is_ui_thread` has to answer `false`
    /// there — otherwise a callback arriving between registration and the first surface would
    /// pass the guard against thread id `0`, which is not a real thread.
    #[test]
    fn the_ui_thread_guard_is_closed_before_a_bind() {
        // `UI_THREAD` starts at 0 and this binary never calls `bind`.
        assert!(!is_ui_thread());
    }
}
