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
//! - **Rust internal APIs** carry the cause through `RwError::source`
//!   when they need to, and otherwise report the failure with the crate's
//!   own return conventions. `RwResult<T>` is the alias for that purpose; it is
//!   exported for FFI wrappers, and it is used inside this module by
//!   `catch_panic`. It is **not** the return type of every internal function —
//!   an earlier revision of this note claimed it was, which was never true.
//! - **`catch_panic`** must be used at every `extern "C" fn` entry
//!   point to prevent unwinding across the FFI boundary (UB).
//!
//! # Reachability
//!
//! **State:** Production callers: `src/bindings/binding_impl.rs:2300` and
//! `:2310` (`rw_error_code` / `rw_error_message` read this module's
//! `ffi` slot).

// ---------------------------------------------------------------------------
// ErrorId — stable integer error codes (for C/C++ FFI only)
// ---------------------------------------------------------------------------

mod types;
pub use types::{catch_panic, ErrorId, RwError, RwResult};

pub mod ffi;
pub use ffi::{c_try_fallback, CAbiSafe};

#[cfg(test)]
mod tests {
    use super::*;
    // `to_string` on `RwError` comes from this trait in the `no_std` compat layer; the
    // type definitions moved to `types.rs`, so the trait is imported here rather than
    // leaking an unused import into the module root.
    use crate::compat::MiniToString;
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
