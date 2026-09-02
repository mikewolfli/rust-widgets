//! C FFI safety macro and helpers.
//!
//! Provides the `c_try!` family of macros that wrap `extern "C" fn`
//! bodies so that any internal panic is caught and converted to a
//! safe fallback value instead of unwinding across the C ABI boundary.

/// Wrap a C‑exported function body with panic protection.
///
/// Evaluates the expression and returns `T` on success or the
/// appropriate fallback value (0, false, null, etc.) on panic.
/// On failure the error is logged AND recorded as the last FFI error,
/// retrievable by C callers via `rw_error_code` / `rw_error_message`.
///
/// # Usage
///
/// ```text
/// #[no_mangle]
/// pub extern "C" fn rw_create_button(...) -> u64 {
///     c_try!({ get_control_backend().create_button(...) })
/// }
/// ```
#[macro_export]
macro_rules! c_try {
    ($body:expr) => {{
        let __result: $crate::error::RwResult<_> = $crate::error::catch_panic(|| $body);
        match __result {
            Ok(val) => val,
            Err(e) => {
                log::error!("[rust_widgets] C ABI error: {e}");
                $crate::error::ffi::record_last_ffi_error(e.clone());
                $crate::error::c_try_fallback(e)
            }
        }
    }};
}

/// Fallback value dispatcher for `c_try!`.
///
/// The compiler picks the correct overload based on the expected
/// return type of the enclosing `extern "C" fn`.
pub fn c_try_fallback<T>(_e: super::RwError) -> T
where
    T: CAbiSafe,
{
    T::c_abi_fallback()
}

/// Last FFI error recorded by `c_try!`, readable by C callers.
///
/// A `Mutex`-protected slot (not thread-local) so that any thread's failed
/// C ABI call is observable by the caller thread that queries the error.
static LAST_FFI_ERROR: std::sync::Mutex<Option<super::RwError>> = std::sync::Mutex::new(None);

/// Record the most recent FFI error for `rw_error_code` / `rw_error_message`.
pub fn record_last_ffi_error(error: super::RwError) {
    if let Ok(mut slot) = LAST_FFI_ERROR.lock() {
        *slot = Some(error);
    }
}

/// Returns the last FFI error, if any.
///
/// Only the desktop FFI bindings (`rw_error_code` / `rw_error_message`) read it.
#[cfg(all(feature = "desktop", not(feature = "mini")))]
pub(crate) fn last_ffi_error() -> Option<super::RwError> {
    LAST_FFI_ERROR.lock().ok().and_then(|slot| slot.clone())
}

/// Clear the recorded last FFI error.
pub fn clear_last_ffi_error() {
    if let Ok(mut slot) = LAST_FFI_ERROR.lock() {
        *slot = None;
    }
}

/// Trait for C‑ABI‑safe types that provide a safe fallback value.
pub trait CAbiSafe {
    fn c_abi_fallback() -> Self;
}

impl CAbiSafe for u64 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for i64 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for u32 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for i32 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for u16 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for i16 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for u8 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for i8 {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for f32 {
    fn c_abi_fallback() -> Self {
        0.0
    }
}
impl CAbiSafe for f64 {
    fn c_abi_fallback() -> Self {
        0.0
    }
}

// ⚠️ `bool` is **not** C‑ABI‑safe (its memory representation is not guaranteed by the ABI).
// Prefer `u8` (0/1) or `u32` (0/1) for `extern "C" fn` return types. This impl exists only
// to support existing call sites.
//
// DEPRECATED: Do not use `CAbiSafe for bool` in new code. The Rust bool type has no
// guaranteed C‑ABI representation — use `u8` (0/1) or `u32` (0/1) as the return type
// for `extern "C" fn` signatures instead.
impl CAbiSafe for bool {
    fn c_abi_fallback() -> Self {
        false
    }
}
impl CAbiSafe for *const std::ffi::c_char {
    fn c_abi_fallback() -> Self {
        std::ptr::null()
    }
}
impl CAbiSafe for *mut std::ffi::c_char {
    fn c_abi_fallback() -> Self {
        std::ptr::null_mut()
    }
}
impl CAbiSafe for *const u64 {
    fn c_abi_fallback() -> Self {
        std::ptr::null()
    }
}
impl CAbiSafe for *mut u64 {
    fn c_abi_fallback() -> Self {
        std::ptr::null_mut()
    }
}
impl CAbiSafe for usize {
    fn c_abi_fallback() -> Self {
        0
    }
}
impl CAbiSafe for isize {
    fn c_abi_fallback() -> Self {
        0
    }
}

/// Wrap a void (no-return) C‑exported function body with panic protection.
///
/// # Usage
///
/// ```text
/// #[no_mangle]
/// pub extern "C" fn rw_something(id: u64) {
///     c_try_void!({
///         do_something(id);
///     })
/// }
/// ```
#[macro_export]
macro_rules! c_try_void {
    ($body:expr) => {{
        let __result: $crate::error::RwResult<_> = $crate::error::catch_panic(|| $body);
        if let Err(e) = __result {
            log::error!("[rust_widgets] C ABI error: {e}");
            $crate::error::ffi::record_last_ffi_error(e.clone());
        }
    }};
}

#[cfg(all(test, feature = "desktop", not(feature = "mini")))]
mod tests {
    use super::*;
    use crate::error::{ErrorId, RwError};

    #[test]
    fn last_ffi_error_roundtrip() {
        clear_last_ffi_error();
        assert!(last_ffi_error().is_none());

        record_last_ffi_error(RwError::new(ErrorId::INVALID_ARGUMENT, "bad widget"));
        let recorded = last_ffi_error().expect("error should be recorded");
        assert_eq!(recorded.id, ErrorId::INVALID_ARGUMENT);
        assert!(recorded.message.contains("bad widget"));

        clear_last_ffi_error();
        assert!(last_ffi_error().is_none());
    }
}
