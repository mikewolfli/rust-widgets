//! Unified error system for rust_widgets.
//!
//! Provides [`ErrorId`] for FFI-safe error codes and [`RwError`] for
//! rich Rust-side error reporting.  `ErrorId` is **only** used at the
//! C/C++ FFI boundary; internal Rust code uses `RwResult<T>`.
//!
//! # Design
//!
//! - **Rust internal APIs** return `RwResult<T>` (or `Option<T>`).
//! - **C ABI functions** convert `RwError` → `ErrorId` (i32) so that
//!   C/C++ callers receive a stable numeric error code.
//! - **`catch_panic`** must be used at every `extern "C" fn` entry
//!   point to prevent unwinding across the FFI boundary (UB).

use std::fmt;

// ---------------------------------------------------------------------------
// ErrorId — stable integer error codes (for C/C++ FFI only)
// ---------------------------------------------------------------------------

/// FFI‑safe error identifier.
///
/// `ErrorId` values are **stable** – they must never be renumbered or
/// deleted once published in a C header.  New IDs are appended.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ErrorId(pub i32);

impl ErrorId {
    // --- General (1‑99) ---
    pub const SUCCESS: Self = Self(0);
    pub const NOT_IMPLEMENTED: Self = Self(1);
    pub const UNSUPPORTED_OPERATION: Self = Self(2);
    pub const INVALID_ARGUMENT: Self = Self(3);
    pub const NULL_POINTER: Self = Self(4);
    pub const OUT_OF_MEMORY: Self = Self(5);
    pub const LOCK_POISONED: Self = Self(6);

    // --- Widget (100‑199) ---
    pub const WIDGET_BASE_NOT_IMPL: Self = Self(100);
    pub const WIDGET_NOT_FOUND: Self = Self(101);
    pub const WIDGET_INVALID_STATE: Self = Self(102);
    pub const WIDGET_DEPRECATED: Self = Self(103);

    // --- Platform (200‑299) ---
    pub const PLATFORM_UNSUPPORTED: Self = Self(200);
    pub const PLATFORM_INIT_FAILED: Self = Self(201);
    pub const CLIPBOARD_FAILED: Self = Self(202);
    pub const DRAG_DROP_FAILED: Self = Self(203);

    // --- Render (300‑399) ---
    pub const RENDER_CONTEXT_INVALID: Self = Self(300);
    pub const RENDER_PIPELINE_FAILED: Self = Self(301);

    // --- I/O (400‑499) ---
    pub const I18N_LOAD_FAILED: Self = Self(400);
    pub const FILE_NOT_FOUND: Self = Self(401);
}

// ---------------------------------------------------------------------------
// RwError — rich Rust error type
// ---------------------------------------------------------------------------

/// Rich error carrying an [`ErrorId`] and a human‑readable message.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::error::{RwError, ErrorId};
///
/// let err = RwError::new(ErrorId::INVALID_ARGUMENT, "bad input");
/// assert_eq!(err.id, ErrorId::INVALID_ARGUMENT);
/// assert!(err.message.contains("bad input"));
///
/// let not_impl = RwError::not_implemented("my_feature");
/// assert_eq!(not_impl.id, ErrorId::NOT_IMPLEMENTED);
/// ```
#[derive(Debug, Clone)]
pub struct RwError {
    pub id: ErrorId,
    pub message: String,
}

impl RwError {
    /// Create a new error from an ID and message.
    pub fn new(id: ErrorId, message: impl Into<String>) -> Self {
        Self {
            id,
            message: message.into(),
        }
    }

    /// Shorthand for a "not implemented" error.
    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::new(
            ErrorId::NOT_IMPLEMENTED,
            format!("not implemented: {}", feature.into()),
        )
    }

    /// Convert panic info (from `catch_unwind`) into an `RwError`.
    pub fn from_panic(panic_info: &dyn std::any::Any) -> Self {
        let msg = panic_info
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic_info.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| String::from("unknown panic"));
        Self::new(ErrorId::NOT_IMPLEMENTED, msg)
    }
}

impl fmt::Display for RwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[RW-{:03}] {}", self.id.0, self.message)
    }
}

impl std::error::Error for RwError {}

// ---------------------------------------------------------------------------
// From impls — bridge between core and error domains
// ---------------------------------------------------------------------------

impl From<crate::core::CoreError> for RwError {
    fn from(err: crate::core::CoreError) -> Self {
        match err {
            crate::core::CoreError::InvalidArgument(msg) => {
                RwError::new(ErrorId::INVALID_ARGUMENT, msg)
            }
            crate::core::CoreError::NotSupported(msg) => {
                RwError::new(ErrorId::UNSUPPORTED_OPERATION, msg)
            }
            crate::core::CoreError::NotFound(msg) => RwError::new(ErrorId::FILE_NOT_FOUND, msg),
            crate::core::CoreError::Internal(msg) => RwError::new(ErrorId::NOT_IMPLEMENTED, msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Result alias
// ---------------------------------------------------------------------------

/// Convenience alias for fallible operations inside rust_widgets.
pub type RwResult<T> = Result<T, RwError>;

// ---------------------------------------------------------------------------
// Panic‑safety helpers
// ---------------------------------------------------------------------------

/// Execute a closure, converting any panic into an `RwResult::Err`.
///
/// **Must** be used at every `extern "C" fn` entry point to prevent
/// unwinding across the C ABI boundary.
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(e) => Err(RwError::from_panic(&*e)),
    }
}

/// Convert an `RwResult<T>` into an `ErrorId` (i32) for C callers.
///
/// Logs the error via `log::error!` for debugging.
// ---------------------------------------------------------------------------
// FFI safety — c_try! macro and helpers
// ---------------------------------------------------------------------------
pub mod ffi;
pub use ffi::{c_try_fallback, CAbiSafe};

pub fn to_error_id(result: RwResult<()>) -> i32 {
    match result {
        Ok(()) => ErrorId::SUCCESS.0,
        Err(e) => {
            log::error!("[rust_widgets] {}", e);
            e.id.0
        }
    }
}
