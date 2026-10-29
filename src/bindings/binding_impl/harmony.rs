// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HarmonyOS/ArkTS native bridge: node mapping, trigger callbacks, and XComponent.
use super::*;
use crate::c_try;
use crate::c_try_void;
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::compat::OnceLock;
use crate::control_backend::get_control_backend;
use core::ffi::c_char;
use core::ffi::c_uint;

/// Global node-handle registry used by Harmony native bridge callbacks.
fn harmony_node_registry() -> &'static Mutex<HashMap<u64, u64>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn harmony_lookup_widget(node_handle: u64) -> Option<u64> {
    if node_handle == 0 {
        return None;
    }
    harmony_node_registry().lock().unwrap_or_else(|e| e.into_inner()).get(&node_handle).copied()
}

/// Harmony callback alias: direct menu item trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_menu_item(menu_item_id: u64) -> CBool {
    c_try!({ get_control_backend().inject_menu_trigger(menu_item_id) })
}

/// Harmony callback alias: direct click trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_click(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
    })
}

/// Harmony callback alias: direct value-changed trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_value_changed(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend().inject_widget_trigger_event(
            widget_id,
            crate::platform::WidgetTriggerKind::ValueChanged,
        )
    })
}

/// Harmony callback alias: direct typed trigger by widget id and kind code.
#[no_mangle]
pub extern "C" fn rw_harmony_on_widget_event(widget_id: u64, kind_code: c_uint) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}

/// Register a Harmony node handle to logical widget id mapping.
#[no_mangle]
pub extern "C" fn rw_harmony_bind_node(node_handle: u64, widget_id: u64) -> CBool {
    c_try!({
        if node_handle == 0 || widget_id == 0 {
            return false;
        }
        harmony_node_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(node_handle, widget_id);
        true
    })
}

/// Remove a single Harmony node-handle mapping.
#[no_mangle]
pub extern "C" fn rw_harmony_unbind_node(node_handle: u64) -> CBool {
    c_try!({
        if node_handle == 0 {
            return false;
        }
        harmony_node_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&node_handle)
            .is_some()
    })
}

/// Resolve mapped widget id from Harmony node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_lookup_widget_id(node_handle: u64) -> u64 {
    c_try!({ harmony_lookup_widget(node_handle).unwrap_or(0) })
}

/// Clear all Harmony node-handle mappings.
#[no_mangle]
pub extern "C" fn rw_harmony_clear_node_bindings() {
    c_try_void!({
        harmony_node_registry().lock().unwrap_or_else(|e| e.into_inner()).clear();
    })
}

/// Runs `f` with the widget id a Harmony node handle maps to.
///
/// The four `rw_harmony_on_node_*` entry points had the same "look up or return false" preamble;
/// one helper keeps the mapping rule (an unmapped handle reports `false`, never `0`) in one place.
fn with_harmony_widget(node_handle: u64, f: impl FnOnce(u64) -> CBool) -> CBool {
    let Some(widget_id) = harmony_lookup_widget(node_handle) else {
        return false;
    };
    f(widget_id)
}

/// Harmony callback alias: menu trigger by node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_menu_item(node_handle: u64) -> CBool {
    c_try!({
        with_harmony_widget(node_handle, |widget_id| {
            get_control_backend().inject_menu_trigger(widget_id)
        })
    })
}

/// Harmony callback alias: click trigger by node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_click(node_handle: u64) -> CBool {
    c_try!({
        with_harmony_widget(node_handle, |widget_id| {
            get_control_backend()
                .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
        })
    })
}

/// Harmony callback alias: value-changed trigger by node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_value_changed(node_handle: u64) -> CBool {
    c_try!({
        with_harmony_widget(node_handle, |widget_id| {
            get_control_backend().inject_widget_trigger_event(
                widget_id,
                crate::platform::WidgetTriggerKind::ValueChanged,
            )
        })
    })
}

/// Harmony callback alias: typed trigger by node handle and kind code.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_widget_event(node_handle: u64, kind_code: c_uint) -> CBool {
    c_try!({
        with_harmony_widget(node_handle, |widget_id| {
            get_control_backend()
                .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
        })
    })
}

/// Bind the ArkUI **XComponent** this library should draw into.
///
/// # Why this is the first call an ArkTS host makes
///
/// Everything else on this page assumes the library has somewhere to put pixels and some way
/// to receive input. On OpenHarmony both arrive through an `XComponent`: the ArkTS side creates
/// one and its `onLoad` receives the `OH_NativeXComponent*` that this function takes.
///
/// # Contract
///
/// Call it from the `XComponent`'s `onLoad`, passing the component pointer ArkUI supplied.
/// After it returns `true`, `rw_mount_surface` becomes usable and input callbacks are live.
///
/// # Why it is not part of `rw_init`
///
/// The pointer does not exist until ArkUI has built the component tree, and `rw_init` runs
/// before that. Folding the two would either delay initialisation past where a host calls it or
/// register against a component ArkUI has not created.
///
/// # Build configuration
///
/// Returns `false` in every build without `feature = "xcomponent"`, which is the default.
/// That is the honest answer rather than a link error: a host that has not enabled the bridge
/// learns it cannot display anything, instead of the library pretending to have a surface.
///
/// # Safety
///
/// `component` must be the pointer ArkUI passed to the ArkTS `XComponent`'s native `onLoad`,
/// and must stay valid for the component's lifetime — the bridge stores it and ArkUI calls back
/// into the registered function pointers until the component is destroyed.
#[no_mangle]
pub unsafe extern "C" fn rw_harmony_bind_xcomponent(component: u64) -> CBool {
    c_try!({
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            // SAFETY: forwarded from this function's own contract — the caller passes the
            // pointer ArkUI gave it, valid for the component's lifetime.
            unsafe {
                crate::platform::harmony::xcomponent::bind(component as *mut core::ffi::c_void)
            }
        }
        #[cfg(not(all(feature = "xcomponent", not(alloc_frugal))))]
        {
            if component != 0 {
                log::error!(
                    "rw_harmony_bind_xcomponent called, but this build has no XComponent bridge \
                     (feature 'xcomponent' is off); the library has no surface to draw into"
                );
            }
            false
        }
    })
}

/// The ArkUI id of the bound `XComponent`, or `0` when none is bound.
///
/// The ArkTS side uses this to confirm which component the library is drawing into — an
/// application with more than one `XComponent` can otherwise not tell which one it handed over.
/// A fresh string is returned and must be released with `rw_free_string`.
#[no_mangle]
pub extern "C" fn rw_harmony_xcomponent_id() -> *mut c_char {
    c_try!({
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            match crate::platform::harmony::xcomponent::component_id() {
                Some(id) => c_string_or_empty(id).into_raw(),
                None => std::ptr::null_mut(),
            }
        }
        #[cfg(not(all(feature = "xcomponent", not(alloc_frugal))))]
        {
            std::ptr::null_mut()
        }
    })
}
