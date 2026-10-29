// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Per-language binding status and ABI version probes.
use crate::c_try;
use core::ffi::c_uint;

#[no_mangle]
/// Return the C ABI binding contract version.
///
/// Independent of the crate semantic version: bumped only when the exported
/// `rw_*` symbol set or its calling conventions change. `8` marks the stable
/// 1.0 ABI line.
pub extern "C" fn rw_bindings_api_version() -> c_uint {
    c_try!({ 8 })
}

/// Return Node.js binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Node.js adapter/example available
#[no_mangle]
pub extern "C" fn rw_nodejs_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) })
}

/// Return Python binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Python adapter/example available
/// - bit2: profile-aware capability query available
#[no_mangle]
pub extern "C" fn rw_python_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}

/// Return C++ wrapper status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: C++ wrapper skeleton/example available
#[no_mangle]
pub extern "C" fn rw_cpp_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) })
}

/// Return Java/JNI binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Java native-method skeleton available
/// - bit2: JNI bridge skeleton available
#[no_mangle]
pub extern "C" fn rw_java_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}

/// Return Java/JNI skeleton ABI version.
#[no_mangle]
pub extern "C" fn rw_java_jni_skeleton_version() -> c_uint {
    c_try!({ 1 })
}

/// Reserved C++ binding marker — returns the current C++ wrapper ABI version.
///
/// Kept as a stable, never-changing symbol so that language bindings can
/// probe for wrapper support without linking against a moving target.
#[no_mangle]
pub extern "C" fn rw_cpp_reserved() -> c_uint {
    c_try!({ 1 })
}

/// Reserved Java binding marker — returns the current JNI wrapper ABI version.
#[no_mangle]
pub extern "C" fn rw_java_reserved() -> c_uint {
    c_try!({ 1 })
}

/// Reserved Python binding marker — returns the current Python wrapper ABI version.
#[no_mangle]
pub extern "C" fn rw_python_reserved() -> c_uint {
    c_try!({ 1 })
}
