//! Frame and repaint signals re-exported at the crate root.

#[cfg(not(alloc_frugal))]
use crate::widget;

// The drain `drive_frame`'s documentation pairs itself with; the definition
// lives in `control`.
pub use super::control::drain_triggers;

/// Drives every library-side task of one frame, in the order they must happen.
///
/// # Why this is re-exported here rather than reached through `widget::runtime`
///
/// It is the counterpart of [`drain_triggers`]: both are "the library owns the logic,
/// the platform owns the clock" entry points that a backend's event loop calls every
/// frame. `drain_triggers` sits at the crate root because backends call it from
/// `mini` too, where `widget::runtime` is not compiled -- so the frame entry point
/// lives beside it, and the two are read together by every platform loop.
///
/// See [`widget::runtime::drive_frame`] for the ordering contract and the reason it is
/// the crate's single frame driver.
#[cfg(not(alloc_frugal))]
pub use widget::runtime::{
    animation_suspended, drive_frame, last_frame_stats, notify_native_redraw, report_state,
    request_repaint_because, set_animation_suspended, FrameOutcome, FrameStats, RepaintReason,
    StateFact,
};
