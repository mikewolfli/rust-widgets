//! Deprecated crate-root aliases kept for API compatibility (principle #21).

// Old crate-root paths keep resolving, so a rename is a warning rather than a
// break. Exported here rather than relying on callers reaching into `deprecated`.
#[allow(deprecated)]
pub use crate::deprecated::{
    mount_custom_widget, request_custom_repaint, resize_custom_widget, supports_custom_widgets,
    unmount_custom_widget,
};
// Gated with `backend_handle`, which the alloc-frugal profile does not compile (it has
// no platform singleton to ask), so the alias must not be re-exported there.
#[cfg(not(alloc_frugal))]
#[allow(deprecated)]
pub use crate::deprecated::native_handle;
