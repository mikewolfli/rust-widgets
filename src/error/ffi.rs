//! C FFI safety macro and helpers.
//!
//! Provides the `c_try!` family of macros that wrap `extern "C" fn`
//! bodies so that any internal panic is caught and converted to a
//! safe fallback value instead of unwinding across the C ABI boundary.

/// Wrap a C‑exported function body with panic protection.
///
/// Evaluates the expression and returns `T` on success or the
/// appropriate fallback value (0, false, null, etc.) on panic.
///
/// # Usage
///
/// ```rust,ignore
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
/// ```rust,ignore
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
        }
    }};
}
