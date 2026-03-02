//! Stable C ABI for foreign language bindings.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uint};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type CBool = bool;

/// Global node-handle registry used by Harmony native bridge callbacks.
fn harmony_node_registry() -> &'static Mutex<HashMap<u64, u64>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn harmony_lookup_widget(node_handle: u64) -> Option<u64> {
    if node_handle == 0 {
        return None;
    }
    harmony_node_registry()
        .lock()
        .expect("harmony node registry lock poisoned")
        .get(&node_handle)
        .copied()
}

/// Convert stable C ABI trigger code to internal typed trigger enum.
fn trigger_kind_from_code(code: c_uint) -> crate::platform::WidgetTriggerKind {
    match code {
        1 => crate::platform::WidgetTriggerKind::Clicked,
        2 => crate::platform::WidgetTriggerKind::ValueChanged,
        3 => crate::platform::WidgetTriggerKind::SelectionChanged,
        4 => crate::platform::WidgetTriggerKind::Closed,
        _ => crate::platform::WidgetTriggerKind::Unknown,
    }
}

fn capability_contract_mask(contract: crate::platform::CapabilityContract) -> c_uint {
    match contract {
        crate::platform::CapabilityContract::Native(native) => {
            let mut mask: c_uint = 0;
            mask |= 1 << 0;
            if native.dpi_scaling {
                mask |= 1 << 1;
            }
            if native.ime {
                mask |= 1 << 2;
            }
            if native.accessibility {
                mask |= 1 << 3;
            }
            if native.native_menu {
                mask |= 1 << 4;
            }
            if native.typed_widget_trigger {
                mask |= 1 << 5;
            }
            mask
        }
        crate::platform::CapabilityContract::Embedded(embedded) => {
            let mut mask: c_uint = 0;
            if embedded.fixed_dpi {
                mask |= 1 << 1;
            }
            if embedded.low_memory_mode {
                mask |= 1 << 2;
            }
            if embedded.typed_widget_trigger {
                mask |= 1 << 3;
            }
            mask
        }
    }
}

/// Convert nullable C string pointer to owned Rust `String`.
fn c_str_or_default(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

#[no_mangle]
pub extern "C" fn rust_widgets_init() {
    crate::init();
}

#[no_mangle]
pub extern "C" fn rust_widgets_run() {
    crate::run();
}

#[no_mangle]
pub extern "C" fn rust_widgets_quit() {
    crate::quit();
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_window(&c_str_or_default(title), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_button(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_button(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_checkbox(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_checkbox(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_line_edit(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_line_edit(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_label(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_label(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_radio_button(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_radio_button(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_slider(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_slider(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_progress_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_progress_bar(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_combo_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_combo_box(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_list_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_list_box(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_panel(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_panel(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_menu_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_menu_bar(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_menu(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_menu(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_attach_menu_bar_to_window(window: u64, menu_bar: u64) -> CBool {
    crate::platform::get_platform().attach_menu_bar_to_window(window, menu_bar)
}

#[no_mangle]
pub extern "C" fn rust_widgets_menu_add_item(
    parent_menu: u64,
    text: *const c_char,
    shortcut: *const c_char,
) -> u64 {
    let shortcut_text = if shortcut.is_null() {
        None
    } else {
        Some(c_str_or_default(shortcut))
    };
    crate::platform::get_platform().menu_add_item(
        parent_menu,
        &c_str_or_default(text),
        shortcut_text.as_deref(),
    )
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_menu_triggered() -> u64 {
    crate::platform::get_platform()
        .poll_menu_triggered()
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_widget_triggered() -> u64 {
    crate::platform::get_platform()
        .poll_widget_triggered()
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_widget_trigger_event(widget_id_out: *mut u64) -> c_uint {
    let Some(event) = crate::platform::get_platform().poll_widget_trigger_event() else {
        return 0;
    };
    if !widget_id_out.is_null() {
        unsafe {
            *widget_id_out = event.widget_id;
        }
    }
    event.kind as c_uint
}

/// Generic menu trigger injection entrypoint for native hosts.
#[no_mangle]
pub extern "C" fn rust_widgets_inject_menu_trigger(menu_item_id: u64) -> CBool {
    crate::platform::get_platform().inject_menu_trigger(menu_item_id)
}

/// Generic typed widget trigger injection entrypoint for native hosts.
#[no_mangle]
pub extern "C" fn rust_widgets_inject_widget_trigger_event(widget_id: u64, kind_code: c_uint) -> CBool {
    crate::platform::get_platform().inject_widget_trigger_event(
        widget_id,
        trigger_kind_from_code(kind_code),
    )
}

/// Harmony callback alias: direct menu item trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_menu_item(menu_item_id: u64) -> CBool {
    crate::platform::get_platform().inject_menu_trigger(menu_item_id)
}

/// Harmony callback alias: direct click trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_click(widget_id: u64) -> CBool {
    crate::platform::get_platform()
        .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
}

/// Harmony callback alias: direct value-changed trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_value_changed(widget_id: u64) -> CBool {
    crate::platform::get_platform()
        .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::ValueChanged)
}

/// Harmony callback alias: direct typed trigger by widget id and kind code.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_widget_event(widget_id: u64, kind_code: c_uint) -> CBool {
    crate::platform::get_platform().inject_widget_trigger_event(
        widget_id,
        trigger_kind_from_code(kind_code),
    )
}

/// Register a Harmony node handle to logical widget id mapping.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_bind_node(node_handle: u64, widget_id: u64) -> CBool {
    if node_handle == 0 || widget_id == 0 {
        return false;
    }
    harmony_node_registry()
        .lock()
        .expect("harmony node registry lock poisoned")
        .insert(node_handle, widget_id);
    true
}

/// Remove a single Harmony node-handle mapping.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_unbind_node(node_handle: u64) -> CBool {
    if node_handle == 0 {
        return false;
    }
    harmony_node_registry()
        .lock()
        .expect("harmony node registry lock poisoned")
        .remove(&node_handle)
        .is_some()
}

/// Resolve mapped widget id from Harmony node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_lookup_widget_id(node_handle: u64) -> u64 {
    harmony_lookup_widget(node_handle).unwrap_or(0)
}

/// Clear all Harmony node-handle mappings.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_clear_node_bindings() {
    harmony_node_registry()
        .lock()
        .expect("harmony node registry lock poisoned")
        .clear();
}

/// Harmony callback alias: menu trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_menu_item(node_handle: u64) -> CBool {
    let Some(widget_id) = harmony_lookup_widget(node_handle) else {
        return false;
    };
    crate::platform::get_platform().inject_menu_trigger(widget_id)
}

/// Harmony callback alias: click trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_click(node_handle: u64) -> CBool {
    let Some(widget_id) = harmony_lookup_widget(node_handle) else {
        return false;
    };
    crate::platform::get_platform()
        .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
}

/// Harmony callback alias: value-changed trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_value_changed(node_handle: u64) -> CBool {
    let Some(widget_id) = harmony_lookup_widget(node_handle) else {
        return false;
    };
    crate::platform::get_platform()
        .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::ValueChanged)
}

/// Harmony callback alias: typed trigger by node handle and kind code.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_widget_event(node_handle: u64, kind_code: c_uint) -> CBool {
    let Some(widget_id) = harmony_lookup_widget(node_handle) else {
        return false;
    };
    crate::platform::get_platform().inject_widget_trigger_event(
        widget_id,
        trigger_kind_from_code(kind_code),
    )
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_tool_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_tool_bar(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_status_bar(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_status_bar(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_show_widget(widget_id: u64) {
    crate::platform::get_platform().show_widget(widget_id);
}

#[no_mangle]
pub extern "C" fn rust_widgets_hide_widget(widget_id: u64) {
    crate::platform::get_platform().hide_widget(widget_id);
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_text(widget_id: u64, text: *const c_char) {
    crate::platform::get_platform().set_widget_text(widget_id, &c_str_or_default(text));
}

#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_text(widget_id: u64) -> *const c_char {
    let text = crate::platform::get_platform().get_widget_text(widget_id);
    match CString::new(text) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new("").expect("static string is valid").into_raw(),
    }
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_enabled(widget_id: u64, enabled: CBool) {
    crate::platform::get_platform().set_widget_enabled(widget_id, enabled);
}

#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_enabled(widget_id: u64) -> CBool {
    crate::platform::get_platform().is_widget_enabled(widget_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_visible(widget_id: u64, visible: CBool) {
    crate::platform::get_platform().set_widget_visible(widget_id, visible);
}

#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_visible(widget_id: u64) -> CBool {
    crate::platform::get_platform().is_widget_visible(widget_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_ime_enabled(widget_id: u64, enabled: CBool) -> CBool {
    crate::platform::get_platform().set_widget_ime_enabled(widget_id, enabled)
}

#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_ime_enabled(widget_id: u64) -> CBool {
    crate::platform::get_platform().is_widget_ime_enabled(widget_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_accessibility_name(widget_id: u64, name: *const c_char) -> CBool {
    crate::platform::get_platform().set_widget_accessibility_name(widget_id, &c_str_or_default(name))
}

#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_accessibility_name(widget_id: u64) -> *const c_char {
    let name = crate::platform::get_platform().get_widget_accessibility_name(widget_id);
    match CString::new(name) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new("").expect("static string is valid").into_raw(),
    }
}

#[no_mangle]
pub extern "C" fn rust_widgets_backend_name() -> *const c_char {
    CString::new(crate::platform::get_platform().backend_name())
        .expect("backend string is valid")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn rust_widgets_platform_capabilities() -> c_uint {
    let caps = crate::platform::capabilities();
    let mut mask: c_uint = 0;
    if caps.dpi_scaling {
        mask |= 1 << 0;
    }
    if caps.ime {
        mask |= 1 << 1;
    }
    if caps.accessibility {
        mask |= 1 << 2;
    }
    if caps.native_menu {
        mask |= 1 << 3;
    }
    if caps.typed_widget_trigger {
        mask |= 1 << 4;
    }
    mask
}

#[no_mangle]
pub extern "C" fn rust_widgets_platform_dpi_scale_factor() -> c_float {
    crate::platform::dpi_scale_factor()
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_render_aa_samples_per_axis(samples: c_uint) -> c_uint {
    let config = crate::render::SoftwareRenderConfig {
        aa_samples_per_axis: samples as u8,
    }
    .normalized();
    crate::render::set_default_software_render_config(config);
    crate::render::default_software_render_config().aa_samples_per_axis as c_uint
}

#[no_mangle]
pub extern "C" fn rust_widgets_get_render_aa_samples_per_axis() -> c_uint {
    crate::render::default_software_render_config().aa_samples_per_axis as c_uint
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_embedded_target_fps(fps: c_uint) -> c_uint {
    crate::render_engine::set_embedded_target_fps(fps) as c_uint
}

#[no_mangle]
pub extern "C" fn rust_widgets_get_embedded_target_fps() -> c_uint {
    crate::render_engine::embedded_target_fps() as c_uint
}

#[no_mangle]
pub extern "C" fn rust_widgets_submit_embedded_noop_task(label: *const c_char) -> u64 {
    crate::render_engine::submit_embedded_task(c_str_or_default(label), |_| {})
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_is_initialized() -> CBool {
    crate::render_engine::embedded_engine_stats().initialized
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_is_running() -> CBool {
    crate::render_engine::embedded_engine_stats().running
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_frame_count() -> u64 {
    crate::render_engine::embedded_engine_stats().frame_count
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_pending_task_count() -> u64 {
    crate::render_engine::embedded_engine_stats().pending_task_count as u64
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_window_count() -> u64 {
    crate::render_engine::embedded_engine_stats().window_count as u64
}

#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_button_count() -> u64 {
    crate::render_engine::embedded_engine_stats().button_count as u64
}

#[no_mangle]
pub extern "C" fn rust_widgets_platform_capability_contract(profile_code: c_uint) -> c_uint {
    let profile = if profile_code == 1 {
        crate::core::RuntimeProfile::Embedded
    } else {
        crate::core::RuntimeProfile::Full
    };
    let contract = crate::platform::negotiate_capability_contract(profile);
    capability_contract_mask(contract)
}

#[no_mangle]
pub extern "C" fn rust_widgets_mobile_backend_name() -> *const c_char {
    #[cfg(feature = "mobile-api")]
    {
        return CString::new(crate::platform::mobile_backend_name())
            .expect("backend string is valid")
            .into_raw();
    }
    #[cfg(not(feature = "mobile-api"))]
    {
        CString::new("").expect("static string is valid").into_raw()
    }
}

#[no_mangle]
pub extern "C" fn rust_widgets_mobile_attach_native_view(native_handle: u64) -> CBool {
    #[cfg(feature = "mobile-api")]
    {
        return crate::platform::mobile_attach_to_native_view(native_handle as usize);
    }
    #[cfg(not(feature = "mobile-api"))]
    {
        let _ = native_handle;
        false
    }
}

#[no_mangle]
pub extern "C" fn rust_widgets_bindings_api_version() -> c_uint {
    7
}

/// Return Python binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Python adapter/example available
/// - bit2: profile-aware capability query available
#[no_mangle]
pub extern "C" fn rust_widgets_python_binding_status() -> c_uint {
    (1 << 0) | (1 << 1) | (1 << 2)
}

/// Return C++ wrapper status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: C++ wrapper skeleton/example available
#[no_mangle]
pub extern "C" fn rust_widgets_cpp_binding_status() -> c_uint {
    (1 << 0) | (1 << 1)
}

/// Return Java/JNI binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Java native-method skeleton available
/// - bit2: JNI bridge skeleton available
#[no_mangle]
pub extern "C" fn rust_widgets_java_binding_status() -> c_uint {
    (1 << 0) | (1 << 1) | (1 << 2)
}

/// Return Java/JNI skeleton ABI version.
#[no_mangle]
pub extern "C" fn rust_widgets_java_jni_skeleton_version() -> c_uint {
    1
}

#[no_mangle]
pub extern "C" fn rust_widgets_python_reserved() -> c_uint {
    rust_widgets_python_binding_status()
}

#[no_mangle]
pub extern "C" fn rust_widgets_cpp_reserved() -> c_uint {
    rust_widgets_cpp_binding_status()
}

#[no_mangle]
pub extern "C" fn rust_widgets_java_reserved() -> c_uint {
    rust_widgets_java_binding_status()
}

#[no_mangle]
pub extern "C" fn rust_widgets_free_string(ptr: *const c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr as *mut c_char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_aa_sample_abi_roundtrip_clamps_values() {
        let original = rust_widgets_get_render_aa_samples_per_axis();

        let low = rust_widgets_set_render_aa_samples_per_axis(0);
        assert_eq!(low, 1);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), 1);

        let high = rust_widgets_set_render_aa_samples_per_axis(100);
        assert_eq!(high, 8);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), 8);

        rust_widgets_set_render_aa_samples_per_axis(original);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), original.clamp(1, 8));
    }

    #[test]
    fn embedded_target_fps_abi_roundtrip_clamps_values() {
        let original = rust_widgets_get_embedded_target_fps();

        let low = rust_widgets_set_embedded_target_fps(0);
        assert_eq!(low, 1);
        assert_eq!(rust_widgets_get_embedded_target_fps(), 1);

        let high = rust_widgets_set_embedded_target_fps(1000);
        assert_eq!(high, 240);
        assert_eq!(rust_widgets_get_embedded_target_fps(), 240);

        rust_widgets_set_embedded_target_fps(original);
        assert_eq!(rust_widgets_get_embedded_target_fps(), original.clamp(1, 240));
    }
}
