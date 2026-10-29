// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! C ABI platform surface: themes, fonts, diagnostics, the embedded engine, and mobile hooks.
use super::*;
use crate::c_try;
use crate::c_try_void;
use crate::control_backend::get_control_backend;
use alloc::ffi::CString;
use core::ffi::c_char;
use core::ffi::c_float;
use core::ffi::c_int;
use core::ffi::c_uint;

fn capability_contract_mask(contract: crate::platform::CapabilityContract) -> c_uint {
    match contract {
        crate::platform::CapabilityContract::Hosted(native) => {
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
            // Host services, appended after the original five. Appending (rather than
            // renumbering) is what keeps every published bit meaning the same thing it did.
            if native.clipboard {
                mask |= 1 << 6;
            }
            if native.open_external {
                mask |= 1 << 7;
            }
            // The host's own path picker; appended, for the same reason as the two above.
            if native.file_dialog {
                mask |= 1 << 8;
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

#[no_mangle]
/// Selects the active theme by name.
///
/// Returns `false` when no theme is registered under `name`, leaving the current
/// theme in place. See [`rw_theme_names`] for the registered names.
///
/// # Profiles without a theme module
///
/// `crate::theme` is gated `device_profile`. A build that compiles this ABI
/// without one (`android-jni` alone, for instance) therefore has no theme to
/// select, and the honest answer is `false` — the same "this build cannot do
/// that" the per-kind `create_*` functions give in a stripped profile. The
/// branch is in the **body**, not around the function, so the symbol always
/// exists in the library and a Java/C caller can link against it (principle
/// #41: capability is a runtime fact, not a compile-time API fork).
///
/// Before this gate the reference to `crate::theme` was unconditional, so
/// `cargo check --features "android-jni jni mobile-api …"` failed with
/// `cannot find theme in crate` — the real Android build script's exact feature
/// set.
pub extern "C" fn rw_set_theme(name: *const c_char) -> CBool {
    c_try!({
        let requested = c_str_or_default(name);
        #[cfg(device_profile)]
        {
            let activated = crate::theme::global_theme_manager().set_theme(&requested);
            if activated {
                // The switch emitted `theme_changed`, which every mounted control's registration
                // subscription recorded (G-404). This drain re-resolves each notified control's
                // style against the new theme synchronously — the switch's completion step — so
                // a host that reads styles or renders before its next frame still sees the new
                // palette. Paths that do not complete here are covered by the frame step.
                crate::reapply_active_theme();
            }
            activated
        }
        #[cfg(not(device_profile))]
        {
            let _ = requested;
            false
        }
    })
}

#[no_mangle]
/// Names of the registered themes, space-separated.
///
/// Same convention as [`rw_widget_kind_names`]: the required size is returned and
/// a null `out` with `cap == 0` queries it.
///
/// A build without a theme module registers no themes, so it truthfully reports
/// an empty list (`0`) instead of naming themes that do not exist.
pub extern "C" fn rw_theme_names(out: *mut c_char, cap: c_uint) -> c_uint {
    c_try!({
        #[cfg(device_profile)]
        {
            let manager = crate::theme::global_theme_manager();
            let names = manager.theme_names();
            write_space_separated(&names, out, cap)
        }
        #[cfg(not(device_profile))]
        {
            write_space_separated(&[], out, cap)
        }
    })
}

#[no_mangle]
/// Number of entries in the host's installed-font snapshot.
///
/// The snapshot is process-wide. The first call returns `0` and starts a background query; a later
/// call reads the installed list. A target whose backend reports no fonts truthfully returns `0`
/// rather than inventing a list. This never blocks the calling thread on `fc-list` or the platform
/// font store — see [`crate::platform::fonts::prewarm_system_fonts`].
pub extern "C" fn rw_system_font_count() -> c_uint {
    c_try!({ crate::platform::fonts::system_font_count_nonblocking() as c_uint })
}

#[no_mangle]
/// The family name at `index`, or an empty string when `index` is out of range.
///
/// The result is freshly allocated and must be released with [`rw_free_string`]. Call
/// [`rw_system_font_count`] first: an index past the end is not an error, it is answered with
/// the empty string.
pub extern "C" fn rw_system_font_family(index: c_uint) -> *const c_char {
    c_try!({
        let family = crate::platform::fonts::system_fonts()
            .get(index as usize)
            .map(|font| font.family.clone())
            .unwrap_or_default();
        to_c_string_or_empty(family)
    })
}

#[no_mangle]
/// The style name at `index` (for example `"Regular"`), or an empty string when the backend
/// does not report one or `index` is out of range.
///
/// The result is freshly allocated and must be released with [`rw_free_string`].
pub extern "C" fn rw_system_font_style(index: c_uint) -> *const c_char {
    c_try!({
        let style = crate::platform::fonts::system_fonts()
            .get(index as usize)
            .map(|font| font.style.clone())
            .unwrap_or_default();
        to_c_string_or_empty(style)
    })
}

#[no_mangle]
/// Re-queries the host's installed fonts and returns the new count.
///
/// For a host that has just installed a font. A reader still holding a slice from the previous
/// query keeps a valid (stale) value, which is the safe outcome; see
/// [`crate::platform::fonts::refresh_system_fonts`].
pub extern "C" fn rw_refresh_system_fonts() -> c_uint {
    c_try!({ crate::platform::fonts::refresh_system_fonts().len() as c_uint })
}

#[no_mangle]
/// Sets the high-contrast override: `0` disables it, any non-zero value enables it.
///
/// Applies to every control the library creates from here on, and re-applies to
/// the live ones so a change takes effect without a rebuild.
///
/// A build without a theme module has no high-contrast override to set, so this
/// is a no-op there rather than a link error — see [`rw_set_theme`] for why the
/// gate is in the body.
pub extern "C" fn rw_set_high_contrast(mode: c_int) {
    c_try_void!({
        let enabled = mode != 0;
        let mode = if enabled {
            crate::style::HighContrastMode::WhiteOnBlack
        } else {
            crate::style::HighContrastMode::None
        };
        #[cfg(device_profile)]
        {
            // The override emits `theme_changed`, which every mounted control's registration
            // subscription recorded (G-404); the drain re-resolves each notified control here so
            // the change takes effect without a rebuild, exactly as the switch paths do.
            crate::theme::set_global_high_contrast(mode);
            crate::reapply_active_theme();
        }
        #[cfg(not(device_profile))]
        {
            let _ = mode;
        }
    })
}

#[no_mangle]
/// Runs the unified platform diagnostic probe and returns the mask of passed facts.
///
/// Bit order is published in `platform::diagnostics` and is stable across platforms. A fact the
/// backend honestly does not have is skipped, not failed; the mask therefore answers "which facts
/// are measured here", while [`rw_platform_diagnostics_json`] names every skip.
///
/// This is the C ABI half of the probe. Hosts that cannot call C but can read a string use the
/// JSON entry point instead; both run the same probe.
pub extern "C" fn rw_platform_diagnostics() -> c_uint {
    c_try!({
        // Gate in the body, not around the function: the generated header declares this symbol
        // unconditionally, and an `alloc_frugal` build that dropped it would fail to link for a
        // host that compiled against the header. A mask of 0 — "no fact measured" — is the
        // honest answer from a profile that carries no diagnostic probe.
        #[cfg(alloc_frugal)]
        {
            0
        }
        #[cfg(not(alloc_frugal))]
        {
            crate::platform::diagnostics::run().mask()
        }
    })
}

#[no_mangle]
/// Runs the unified platform diagnostic probe and returns its JSON form.
///
/// The returned string is freshly allocated and must be released with [`rw_free_string`]. The
/// shape is documented on `platform::diagnostics::PlatformDiagnostics::to_json`.
pub extern "C" fn rw_platform_diagnostics_json() -> *const c_char {
    c_try!({
        // Same reasoning as `rw_platform_diagnostics`: the header declares the symbol, so a
        // profile without the probe returns an empty JSON object rather than a missing symbol.
        #[cfg(alloc_frugal)]
        {
            to_c_string_or_empty("{}")
        }
        #[cfg(not(alloc_frugal))]
        {
            to_c_string_or_empty(crate::platform::diagnostics::run().to_json())
        }
    })
}

#[no_mangle]
/// The name of the control backend currently serving widget creation, such as
/// `"native"` or `"custom"`.
///
/// Useful for diagnostics, since behaviour differs between backends. The result
/// is a freshly allocated C string and must be released with `rw_free_string`.
pub extern "C" fn rw_backend_name() -> *const c_char {
    c_try!({ to_c_string_or_empty(get_control_backend().backend_name()) })
}

#[no_mangle]
/// The platform's supported-capability bitmask, negotiated at compile time.
///
/// Bit layout:
/// - bit0: DPI scaling
/// - bit1: IME
/// - bit2: accessibility
/// - bit3: native menu bar
/// - bit4: typed widget trigger events
/// - bit5: host clipboard
/// - bit6: external open (hand a URL/path to the host's default handler)
///
/// The last two are appended, so a host compiled against an earlier layout still reads bits 0-4
/// exactly as before. See `PlatformCapabilities::clipboard` for what each one means.
///
/// This reflects the platform's *general* capabilities; use
/// [`rw_platform_capability_contract`] for the contract of a specific runtime
/// profile.
pub extern "C" fn rw_platform_capabilities() -> c_uint {
    c_try!({
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
        // Host services, appended after the original five so bits 0-4 keep their meaning.
        if caps.clipboard {
            mask |= 1 << 5;
        }
        if caps.open_external {
            mask |= 1 << 6;
        }
        // The host's own path picker, appended after the services above for the same reason: the
        // bits already published keep their meaning.
        if caps.file_dialog {
            mask |= 1 << 7;
        }
        mask
    })
}

#[no_mangle]
/// The factor that converts logical pixels to physical device pixels.
///
/// `1.0` means no scaling. Returns nothing about failure: a platform without DPI
/// support reports `1.0`.
pub extern "C" fn rw_platform_dpi_scale_factor() -> c_float {
    c_try!({ crate::platform::dpi_scale_factor() })
}

#[no_mangle]
/// Sets the software renderer's anti-aliasing quality, in samples per axis.
///
/// The value is clamped to `1..=8`, and the clamped value actually in effect is
/// returned — the caller does not need a follow-up read to learn the outcome.
/// The setting is process-wide and applies to canvases created afterwards.
pub extern "C" fn rw_set_render_aa_samples_per_axis(samples: c_uint) -> c_uint {
    c_try!({
        // Clamp to 1..=8 in `c_uint` space *before* narrowing to u8, so a value like
        // 256 wraps to 8 rather than truncating to 0 and then being promoted to 1.
        let samples = samples.clamp(1, 8) as u8;
        let config =
            crate::render::SoftwareRenderConfig { aa_samples_per_axis: samples }.normalized();
        crate::render::set_default_software_render_config(config);
        crate::render::default_software_render_config().aa_samples_per_axis as c_uint
    })
}

#[no_mangle]
/// The software renderer's current anti-aliasing quality, in samples per axis.
///
/// Always within `1..=8`.
pub extern "C" fn rw_get_render_aa_samples_per_axis() -> c_uint {
    c_try!({ crate::render::default_software_render_config().aa_samples_per_axis as c_uint })
}

#[no_mangle]
/// Sets the embedded render engine's target frame rate, in hertz (frames per
/// second).
///
/// The value is clamped to `1..=240`, and the clamped value actually in effect is
/// returned.
pub extern "C" fn rw_set_embedded_target_fps(fps: c_uint) -> c_uint {
    c_try!({ crate::render_engine::set_embedded_target_fps(fps) as c_uint })
}

#[no_mangle]
/// The embedded render engine's current target frame rate, in hertz.
///
/// Always within `1..=240`.
pub extern "C" fn rw_get_embedded_target_fps() -> c_uint {
    c_try!({ crate::render_engine::embedded_target_fps() as c_uint })
}

#[no_mangle]
/// Queues a no-op task on the embedded render engine and returns its task id.
///
/// `label` may be null for an empty label; if it is non-null it is copied, so the
/// caller keeps ownership of the original. The task body does nothing — this
/// exists so native hosts can exercise the scheduling path from C.
pub extern "C" fn rw_submit_embedded_noop_task(label: *const c_char) -> u64 {
    c_try!({ crate::render_engine::submit_embedded_task(c_str_or_default(label), |_| {}) })
}

#[no_mangle]
/// Reports whether the embedded render engine has been initialized.
pub extern "C" fn rw_embedded_engine_is_initialized() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().initialized })
}

#[no_mangle]
/// Reports whether the embedded render engine's loop is currently running.
pub extern "C" fn rw_embedded_engine_is_running() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().running })
}

#[no_mangle]
/// The number of frames the embedded render engine has rendered since start.
pub extern "C" fn rw_embedded_engine_frame_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().frame_count })
}

#[no_mangle]
/// The number of tasks queued on the embedded render engine but not yet run.
pub extern "C" fn rw_embedded_engine_pending_task_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().pending_task_count as u64 })
}

#[no_mangle]
/// The number of windows the embedded render engine is tracking.
pub extern "C" fn rw_embedded_engine_window_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().window_count as u64 })
}

#[no_mangle]
/// The number of buttons the embedded render engine is tracking.
pub extern "C" fn rw_embedded_engine_button_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().button_count as u64 })
}

#[no_mangle]
/// The capability contract negotiated for a runtime profile, as a bitmask.
///
/// `profile_code` is `1` for the embedded profile and any other value for the
/// full profile — note that an unrecognised code therefore silently means
/// "full" rather than being rejected.
///
/// Bit meanings are not shared between the two contract kinds. For native
/// contracts: bit0 is always set, bit1 DPI scaling, bit2 IME, bit3 accessibility,
/// bit4 native menu, bit5 typed widget triggers, bit6 clipboard,
/// bit7 external open. For embedded contracts: bit0 is never set, bit1 fixed DPI,
/// bit2 low-memory mode, bit3 typed widget triggers.
pub extern "C" fn rw_platform_capability_contract(profile_code: c_uint) -> c_uint {
    c_try!({
        let profile = if profile_code == 1 {
            crate::core::RuntimeProfile::Embedded
        } else {
            crate::core::RuntimeProfile::Full
        };
        let contract = crate::platform::negotiate_capability_contract(profile);
        capability_contract_mask(contract)
    })
}

#[no_mangle]
/// The mobile backend's name, or an empty string when the `mobile-api` feature is
/// not compiled in.
///
/// The result is a freshly allocated C string and must be released with
/// `rw_free_string`.
pub extern "C" fn rw_mobile_backend_name() -> *const c_char {
    c_try!({
        #[cfg(feature = "mobile-api")]
        {
            to_c_string_or_empty(crate::platform::mobile_backend_name())
        }
        #[cfg(not(feature = "mobile-api"))]
        {
            // Empty string never contains interior NUL bytes.
            CString::new("").unwrap().into_raw()
        }
    })
}

#[no_mangle]
/// Binds the widget layer to an existing native view.
///
/// `native_handle` is the platform's own identifier for the view being taken
/// over, interpreted by the mobile backend. Returns `true` when the view was
/// attached; always `false` when the `mobile-api` feature is not compiled in.
pub extern "C" fn rw_mobile_attach_native_view(native_handle: u64) -> CBool {
    c_try!({
        #[cfg(feature = "mobile-api")]
        {
            crate::platform::mobile_attach_to_native_view(native_handle as usize)
        }
        #[cfg(not(feature = "mobile-api"))]
        {
            let _ = native_handle;
            false
        }
    })
}
