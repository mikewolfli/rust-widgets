// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unified error system for rust_widgets.
//!
//! Provides `ErrorId` for FFI-safe error codes and `RwError` for
//! rich Rust-side error reporting.  `ErrorId` is **only** used at the
//! C/C++ FFI boundary.
//!
//! # Design
//!
//! - **C ABI functions** convert `RwError` → `ErrorId` (i32) so that
//!   C/C++ callers receive a stable numeric error code.
//! - **Rust internal APIs** carry the cause through [`RwError::source`]
//!   when they need to, and otherwise report the failure with the crate's
//!   own return conventions. `RwResult<T>` is the alias for that purpose; it is
//!   exported for FFI wrappers, and it is used inside this module by
//!   [`catch_panic`]. It is **not** the return type of every internal function —
//!   an earlier revision of this note claimed it was, which was never true.
//! - **`catch_panic`** must be used at every `extern "C" fn` entry
//!   point to prevent unwinding across the FFI boundary (UB).
//!
//! # Reachability
//!
//! **State:** Production callers: `src/bindings/binding_impl.rs:2300` and
//! `:2310` (`rw_error_code` / `rw_error_message` read this module's
//! [`ffi`] slot).

use crate::compat::{fmt, format, Box, MiniToString, String};

// ---------------------------------------------------------------------------
// ErrorId — stable integer error codes (for C/C++ FFI only)
// ---------------------------------------------------------------------------

/// FFI‑safe error identifier.
///
/// `ErrorId` values are **stable** – they must never be renumbered or
/// deleted once published in a C header.  New IDs are appended.
///
/// # Usage
/// - **Produced at runtime today**: `INVALID_ARGUMENT` (every capability and style
///   refusal records it — see [`ffi::record_capability_error`]) and `GENERAL`
///   (a caught panic). [`ffi`]'s reader reports `SUCCESS` as the literal `0`
///   [`ErrorId::SUCCESS`] holds when no failure was recorded.
/// - **Reserved for future use** (stable API, not yet wired):
///   `NOT_IMPLEMENTED`, `UNSUPPORTED_OPERATION`, `NULL_POINTER`, `OUT_OF_MEMORY`,
///   `LOCK_POISONED`, `WIDGET_BASE_NOT_IMPL`, `WIDGET_NOT_FOUND`,
///   `WIDGET_INVALID_STATE`, `WIDGET_DEPRECATED`, `PLATFORM_UNSUPPORTED`,
///   `PLATFORM_INIT_FAILED`, `CLIPBOARD_FAILED`, `DRAG_DROP_FAILED`,
///   `RENDER_CONTEXT_INVALID`, `RENDER_PIPELINE_FAILED`, `I18N_LOAD_FAILED`
///
/// `NOT_IMPLEMENTED`, `UNSUPPORTED_OPERATION` and `FILE_NOT_FOUND` are reachable
/// only through [`From<crate::core::CoreError>`] — `FILE_NOT_FOUND` additionally
/// through the FFI file helpers' own call sites. An earlier revision of this list
/// named `NOT_IMPLEMENTED`/`UNSUPPORTED_OPERATION`/`FILE_NOT_FOUND` as
/// "used in production", which no production path constructed.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ErrorId(pub i32);

impl ErrorId {
    // --- General (1‑99) ---
    /// Operation completed successfully. This is the only `ErrorId` that
    /// callers of an FFI function should treat as "no error"; all others are
    /// nonzero failures.
    pub const SUCCESS: Self = Self(0);
    /// The requested operation exists in the API surface but has no working
    /// implementation yet. Callers should surface this as a hard failure and
    /// not retry.
    pub const NOT_IMPLEMENTED: Self = Self(1);
    /// The current backend or platform cannot perform the operation at all.
    /// Unlike [`ErrorId::NOT_IMPLEMENTED`], this is a permanent capability gap
    /// rather than missing code.
    pub const UNSUPPORTED_OPERATION: Self = Self(2);
    /// An argument was rejected — wrong type, out of range, or inconsistent
    /// with the current state. Indicates a caller bug, not an environment
    /// problem.
    pub const INVALID_ARGUMENT: Self = Self(3);
    /// Catch-all failure with no more specific code available. Used by
    /// [`RwError::msg`] and by panics crossing [`catch_panic`].
    pub const GENERAL: Self = Self(999);
    /// Reserved — not yet wired into any code path.
    pub const NULL_POINTER: Self = Self(4);
    /// Reserved — not yet wired into any code path.
    pub const OUT_OF_MEMORY: Self = Self(5);
    /// Reserved — not yet wired into any code path.
    pub const LOCK_POISONED: Self = Self(6);

    // --- Widget (100–199) ---
    /// Reserved — not yet wired into any code path.
    pub const WIDGET_BASE_NOT_IMPL: Self = Self(100);
    /// Reserved — not yet wired into any code path.
    pub const WIDGET_NOT_FOUND: Self = Self(101);
    /// Reserved — not yet wired into any code path.
    pub const WIDGET_INVALID_STATE: Self = Self(102);
    /// Reserved — not yet wired into any code path.
    pub const WIDGET_DEPRECATED: Self = Self(103);

    // --- Platform (200–299) ---
    /// Reserved — not yet wired into any code path.
    pub const PLATFORM_UNSUPPORTED: Self = Self(200);
    /// Reserved — not yet wired into any code path.
    pub const PLATFORM_INIT_FAILED: Self = Self(201);
    /// Reserved — not yet wired into any code path.
    pub const CLIPBOARD_FAILED: Self = Self(202);
    /// Reserved — not yet wired into any code path.
    pub const DRAG_DROP_FAILED: Self = Self(203);

    // --- Render (300–399) ---
    /// Reserved — not yet wired into any code path.
    pub const RENDER_CONTEXT_INVALID: Self = Self(300);
    /// Reserved — not yet wired into any code path.
    pub const RENDER_PIPELINE_FAILED: Self = Self(301);

    // --- I/O (400–499) ---
    /// Reserved — not yet wired into any code path.
    pub const I18N_LOAD_FAILED: Self = Self(400);
    /// A path or resource referenced by the caller could not be located on
    /// disk or in the active bundle.
    pub const FILE_NOT_FOUND: Self = Self(401);

    // --- EW alias constants (compatibility) ---
    /// Legacy alias for [`ErrorId::SUCCESS`]; same numeric value. Kept for
    /// source compatibility with earlier `EW_*`-prefixed API users.
    pub const EW_SUCCESS: Self = Self(0);
    /// Legacy alias for [`ErrorId::NOT_IMPLEMENTED`]; same numeric value.
    pub const EW_NOT_IMPLEMENTED: Self = Self(1);
    /// Legacy alias for [`ErrorId::UNSUPPORTED_OPERATION`]; same numeric value.
    pub const EW_UNSUPPORTED_OPERATION: Self = Self(2);
    /// Legacy alias for [`ErrorId::INVALID_ARGUMENT`]; same numeric value.
    pub const EW_INVALID_ARGUMENT: Self = Self(3);
    /// Legacy alias for [`ErrorId::GENERAL`]; same numeric value.
    pub const EW_GENERAL: Self = Self(999);
    /// Reserved — not yet wired into any code path.
    pub const EW_NULL_POINTER: Self = Self(4);
    /// Reserved — not yet wired into any code path.
    pub const EW_OUT_OF_MEMORY: Self = Self(5);
    /// Reserved — not yet wired into any code path.
    pub const EW_LOCK_POISONED: Self = Self(6);
    /// Reserved — not yet wired into any code path.
    pub const EW_WIDGET_BASE_NOT_IMPL: Self = Self(100);
    /// Reserved — not yet wired into any code path.
    pub const EW_WIDGET_NOT_FOUND: Self = Self(101);
    /// Reserved — not yet wired into any code path.
    pub const EW_WIDGET_INVALID_STATE: Self = Self(102);
    /// Reserved — not yet wired into any code path.
    pub const EW_WIDGET_DEPRECATED: Self = Self(103);
    /// Reserved — not yet wired into any code path.
    pub const EW_PLATFORM_UNSUPPORTED: Self = Self(200);
    /// Reserved — not yet wired into any code path.
    pub const EW_PLATFORM_INIT_FAILED: Self = Self(201);
    /// Reserved — not yet wired into any code path.
    pub const EW_CLIPBOARD_FAILED: Self = Self(202);
    /// Reserved — not yet wired into any code path.
    pub const EW_DRAG_DROP_FAILED: Self = Self(203);
    /// Reserved — not yet wired into any code path.
    pub const EW_RENDER_CONTEXT_INVALID: Self = Self(300);
    /// Reserved — not yet wired into any code path.
    pub const EW_RENDER_PIPELINE_FAILED: Self = Self(301);
    /// Reserved — not yet wired into any code path.
    pub const EW_I18N_LOAD_FAILED: Self = Self(400);
    /// Legacy alias for [`ErrorId::FILE_NOT_FOUND`]; same numeric value.
    pub const EW_FILE_NOT_FOUND: Self = Self(401);
}

// ---------------------------------------------------------------------------
// RwError — rich Rust error type
// ---------------------------------------------------------------------------

/// Rich error carrying an `ErrorId` and a human‑readable message.
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
// `Clone` is implemented by hand rather than derived: the cause is a trait object, which
// is not `Clone`, and there is no way to clone a boxed error without knowing its original
// type. A clone keeps the classified id and message (the parts a caller acts on) and drops
// the cause, rather than refusing to clone at all. Dropping it is visible on
// `Error::source`, which returns `None` for the copy — but the message still names the
// original failure, so nothing a log prints is lost.
#[derive(Debug)]
pub struct RwError {
    /// Stable machine-readable error code. Treat as the authoritative
    /// classifier; `message` is for humans only and may be reworded.
    pub id: ErrorId,
    /// Human-readable description of what went wrong. Never parsed by code,
    /// never guaranteed to be localised, and may embed untrusted input taken
    /// from the caller's arguments.
    pub message: String,
    /// The underlying error this one was converted from, when there was one.
    ///
    /// # Why the field exists
    ///
    /// Without it, `?`-converting a real cause (an `io::Error`, a `CoreError`)
    /// into an `RwError` flattened it to a formatted string, and
    /// [`Error::source`](core::error::Error::source) could only ever answer `None`.
    /// A caller that wanted to branch on the cause had no way to, and a log that
    /// printed the chain showed only the outermost message.
    ///
    /// It stays `None` for errors constructed directly from an id and message,
    /// which is most of them, so the common case carries no allocation.
    pub cause: Option<Box<dyn core::error::Error + Send + Sync>>,
}

impl Clone for RwError {
    /// Copies the classified id and message; see the note on [`RwError::cause`].
    fn clone(&self) -> Self {
        Self { id: self.id, message: self.message.clone(), cause: None }
    }
}

impl RwError {
    /// Create a new error from an ID and message.
    pub fn new(id: ErrorId, message: impl Into<String>) -> Self {
        Self { id, message: message.into(), cause: None }
    }

    /// Creates an error that carries `cause` as its source.
    ///
    /// This is the form to use at a `?`-conversion site: it preserves both the
    /// classified id/message a caller reads and the original error a developer
    /// follows through [`Error::source`](core::error::Error::source).
    pub fn with_cause(
        id: ErrorId,
        message: impl Into<String>,
        cause: impl core::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self { id, message: message.into(), cause: Some(Box::new(cause)) }
    }

    /// Shorthand for a "not implemented" error.
    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::new(ErrorId::NOT_IMPLEMENTED, format!("not implemented: {}", feature.into()))
    }

    /// Create a new error from a message string.
    pub fn msg(message: impl Into<String>) -> Self {
        Self::new(ErrorId::GENERAL, message)
    }

    /// Convert panic info (from `catch_unwind`) into an `RwError`.
    pub fn from_panic(panic_info: &dyn crate::compat::Any) -> Self {
        let msg = panic_info
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic_info.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| String::from("unknown panic"));
        Self::new(ErrorId::GENERAL, msg)
    }
}

impl fmt::Display for RwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[EW-{:03}] {}", self.id.0, self.message)
    }
}

impl core::error::Error for RwError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.cause.as_deref().map(|e| e as &(dyn core::error::Error + 'static))
    }
}

// ---------------------------------------------------------------------------
// From impls — bridge between core and error domains
// ---------------------------------------------------------------------------

impl From<crate::core::CoreError> for RwError {
    /// Maps each [`CoreError`](crate::core::CoreError) variant onto the `ErrorId`
    /// that names the same failure, keeping the original error as the new one's
    /// [`source`](core::error::Error::source).
    ///
    /// `CoreError::Internal` maps to [`ErrorId::GENERAL`], not `NOT_IMPLEMENTED`:
    /// it means "something went wrong inside", which is exactly what `GENERAL`
    /// is for. It used to map to `NOT_IMPLEMENTED`, which is a *different* claim
    /// ("this exists but has no implementation yet") and which the reverse
    /// conversion does not produce — so a `GENERAL` error that made the round trip
    /// silently came back reclassified as "not implemented".
    fn from(err: crate::core::CoreError) -> Self {
        let (id, message) = match &err {
            crate::core::CoreError::InvalidArgument(msg) => {
                (ErrorId::INVALID_ARGUMENT, msg.clone())
            }
            crate::core::CoreError::NotSupported(msg) => {
                (ErrorId::UNSUPPORTED_OPERATION, msg.clone())
            }
            crate::core::CoreError::NotFound(msg) => (ErrorId::FILE_NOT_FOUND, msg.clone()),
            crate::core::CoreError::Internal(msg) => (ErrorId::GENERAL, msg.clone()),
        };
        RwError::with_cause(id, message, err)
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
///
/// # Under `mini`
///
/// `catch_unwind` has no `core` equivalent on the supported toolchain: it lives
/// in `std` because catching a panic needs the std unwinder and the panic-payload
/// machinery. `mini` targets `panic = "abort"` (see the `release-mini` profile in
/// `Cargo.toml`), where unwinding cannot be caught by *any* API, so the mini arm
/// calls `f` directly. That is not a silent behaviour change — aborting is what a
/// panic does under that profile with or without this function.
#[cfg(not(alloc_frugal))]
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where
    F: FnOnce() -> T + core::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(e) => Err(RwError::from_panic(&*e)),
    }
}

/// Execute a closure, converting any panic into an `RwResult::Err`.
///
/// **Must** be used at every `extern "C" fn` entry point to prevent
/// unwinding across the C ABI boundary.
///
/// # Under `mini`
///
/// `catch_unwind` lives in `std`: catching a panic needs the std unwinder and the panic-payload
/// machinery, neither of which `core` provides. The `mini` arm therefore calls `f` directly and
/// relies on the build aborting instead of unwinding.
///
/// That reliance is a property of the **profile**, not of this feature. Only
/// `[profile.release-mini]` and `[profile.release-embedded]` set `panic = "abort"`; a `mini` build
/// using `dev` or the plain `release` profile unwinds like any other, and there this function
/// does **not** catch — the panic propagates as if `catch_panic` had not been called. Verified on a
/// default `mini` build: an enclosing `std::panic::catch_unwind` observed the panic, so the value
/// never reached the `Err` arm.
///
/// Callers that need a guarantee rather than a convention must set `panic = "abort"` in their own
/// profile, which is what the shipped release profiles do. The alternative — forwarding to
/// `std::panic::catch_unwind` — is not available here, because `mini` is `#![no_std]` and the
/// crate links `std` only through the `extern crate std` macro shim in `lib.rs`.
#[cfg(alloc_frugal)]
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where
    F: FnOnce() -> T + core::panic::UnwindSafe,
{
    Ok(f())
}

// ---------------------------------------------------------------------------
// FFI safety — c_try! macro and helpers
// ---------------------------------------------------------------------------
// FFI boundary helpers
// ---------------------------------------------------------------------------
pub mod ffi;
pub use ffi::{c_try_fallback, CAbiSafe};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rw_error_display() {
        let e = RwError::msg("test error");
        assert!(e.to_string().contains("test"));
    }
    #[test]
    fn rw_error_not_implemented() {
        let e = RwError::not_implemented("feature");
        assert!(e.to_string().contains("not implemented"));
        assert!(e.to_string().contains("feature"));
    }
    #[test]
    fn rw_error_msg_not_success() {
        let e = RwError::msg("test");
        assert_ne!(e.id, ErrorId::SUCCESS);
    }

    #[test]
    fn ew_error_alias_values_match() {
        assert_eq!(ErrorId::EW_SUCCESS.0, ErrorId::SUCCESS.0);
        assert_eq!(ErrorId::EW_INVALID_ARGUMENT.0, ErrorId::INVALID_ARGUMENT.0);
        assert_eq!(ErrorId::EW_FILE_NOT_FOUND.0, ErrorId::FILE_NOT_FOUND.0);
    }

    #[test]
    fn rw_error_display_prefix_uses_ew() {
        let err = RwError::new(ErrorId::INVALID_ARGUMENT, "bad input");
        assert!(err.to_string().starts_with("[EW-"));
    }

    /// A converted cause is reachable through `Error::source`.
    ///
    /// `RwError` had no field to hold a cause, so the bridge from `CoreError`
    /// flattened the original error to a string and `source()` could only ever
    /// answer `None`. A caller that wanted to branch on the cause had nothing to
    /// branch on.
    #[test]
    fn a_converted_core_error_keeps_its_cause_in_the_source_chain() {
        use core::error::Error;
        let err = RwError::from(crate::core::CoreError::NotSupported("no GPU".to_string()));
        // The classified view is unchanged…
        assert_eq!(err.id, ErrorId::UNSUPPORTED_OPERATION);
        // …and the original error is still there for a developer.
        let cause = err.source().expect("the converted CoreError must be the source");
        assert!(cause.to_string().contains("no GPU"));
    }

    /// An error built from an id and message has no cause, and says so.
    #[test]
    fn an_error_built_from_an_id_has_no_source() {
        use core::error::Error;
        assert!(RwError::new(ErrorId::INVALID_ARGUMENT, "x").source().is_none());
    }

    /// `CoreError::Internal` maps to `GENERAL`, not `NOT_IMPLEMENTED`.
    ///
    /// The reverse conversion turns `GENERAL` back into `Internal`, so the mapping
    /// has to be that way round for the two to round-trip. Mapping `Internal` onto
    /// `NOT_IMPLEMENTED` reclassified an internal failure as "no implementation yet"
    /// on the way back, a claim about the API rather than about what happened.
    #[test]
    fn an_internal_core_error_round_trips_as_internal() {
        let err = RwError::from(crate::core::CoreError::Internal("boom".to_string()));
        assert_eq!(err.id, ErrorId::GENERAL, "internal is not \"not implemented\"");
        assert!(matches!(crate::core::CoreError::from(err), crate::core::CoreError::Internal(_)));
    }
}
