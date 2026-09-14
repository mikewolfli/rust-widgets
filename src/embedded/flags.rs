// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Embedded mode flag management and memory configuration helpers.
//!
//! # Where the numbers come from
//!
//! The four budget functions below used to derive their answers from two unrelated
//! run-time flags (`EMBEDDED_MODE` / `LOW_MEMORY_MODE`) with their own `if` chains,
//! so `mini`'s and `embedded`'s budgets were described in two places that had to
//! agree by hand. They now read [`crate::platform::profile::surface_policy`], which
//! is the single table for "what does this profile's host provide" (BLUE15 Phase E).
//!
//! The run-time flags survive because they are a *caller* choice: an application can
//! put a desktop build into low-memory mode at run time, and that narrows the budget
//! further. They can only tighten the policy, never widen it — a build cannot gain
//! resources its profile does not have.

use crate::core::Size;
use crate::platform::profile::surface_policy;
use std::sync::atomic::{AtomicBool, Ordering};

static EMBEDDED_MODE: AtomicBool = AtomicBool::new(false);
static LOW_MEMORY_MODE: AtomicBool = AtomicBool::new(false);

/// Check if running in embedded mode
pub fn is_embedded_mode() -> bool {
    EMBEDDED_MODE.load(Ordering::Relaxed)
}

/// Set embedded mode
pub fn set_embedded_mode(enabled: bool) {
    EMBEDDED_MODE.store(enabled, Ordering::Relaxed);
}

/// Check if low memory mode is enabled
pub fn is_low_memory_mode() -> bool {
    LOW_MEMORY_MODE.load(Ordering::Relaxed)
}

/// Set low memory mode
pub fn set_low_memory_mode(enabled: bool) {
    LOW_MEMORY_MODE.store(enabled, Ordering::Relaxed);
}

/// `true` when the caller asked for the reduced budget on top of the profile's own.
///
/// Deliberately reads **only** the two run-time flags. The profile's own budget is
/// applied by the policy table, not here: folding `!os_window` into this predicate
/// made `embedded` behave as if a caller had requested low-memory mode, so
/// `recommended_buffer_size()` could never return the full size on any non-desktop
/// profile — the caller's flag stopped meaning "reduce it further" and started
/// meaning "is this embedded", which is a question the table already answers.
fn caller_requested_reduction() -> bool {
    is_embedded_mode() || is_low_memory_mode()
}

/// Get recommended buffer size for current mode
pub fn recommended_buffer_size() -> Size {
    let full = surface_policy();
    if caller_requested_reduction() {
        Size::new(800, 600)
    } else {
        Size::new(full.recommended_window.0, full.recommended_window.1)
    }
}

/// Get maximum recommended texture size
pub fn max_texture_size() -> u32 {
    surface_policy().max_texture
}

/// Get recommended font cache size
pub fn font_cache_size() -> usize {
    if is_low_memory_mode() {
        // A caller-requested reduction must be able to go below the profile cap.
        (surface_policy().font_cache_bytes / 4).max(64 * 1024)
    } else {
        surface_policy().font_cache_bytes
    }
}

/// Get recommended event queue size
pub fn event_queue_size() -> usize {
    surface_policy().event_queue
}

/// Maximum number of simultaneously mounted controls this profile budgets for.
///
/// Exposed here so a pool or widget allocator can size itself from the same table
/// the rest of the runtime uses, rather than carrying another constant.
pub fn max_widgets() -> usize {
    surface_policy().max_widgets
}

/// Initialize embedded environment with optimal settings
pub fn init_embedded(config: crate::embedded::config::EmbeddedConfig) {
    set_embedded_mode(true);
    set_low_memory_mode(config.low_memory_mode);
    if let Some(dpi) = config.fixed_dpi {
        crate::embedded::dpi::set_fixed_dpi(dpi);
    }
}

/// Restore desktop environment settings
pub fn init_desktop() {
    set_embedded_mode(false);
    set_low_memory_mode(false);
    crate::embedded::dpi::clear_fixed_dpi();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_mode() {
        assert!(!is_embedded_mode());
        set_embedded_mode(true);
        assert!(is_embedded_mode());
        set_embedded_mode(false);
        assert!(!is_embedded_mode());
    }

    #[test]
    fn test_low_memory_mode() {
        assert!(!is_low_memory_mode());
        set_low_memory_mode(true);
        assert!(is_low_memory_mode());
        set_low_memory_mode(false);
        assert!(!is_low_memory_mode());
    }

    /// The buffer size must follow the profile's policy, not a desktop constant.
    ///
    /// The old assertion hard-coded `1920` for the un-reduced case, which is only
    /// correct on a device profile: `embedded` and `mini` budget for smaller
    /// surfaces, so the test passed on desktop and failed everywhere else. It now
    /// compares against the policy row the running profile actually selects, which
    /// is the fact the function is supposed to express.
    #[test]
    fn test_buffer_size() {
        use crate::platform::profile::surface_policy;

        set_low_memory_mode(true);
        assert_eq!(recommended_buffer_size().width, 800, "a reduced request wins");
        set_low_memory_mode(false);

        let policy = surface_policy();
        let size = recommended_buffer_size();
        assert_eq!(size.width, policy.recommended_window.0);
        assert_eq!(size.height, policy.recommended_window.1);
    }

    /// A caller-requested reduction must go below the profile's own cap, and the
    /// cap must still hold when no reduction was requested.
    #[test]
    fn budgets_follow_the_policy_and_can_be_narrowed() {
        use crate::platform::profile::surface_policy;

        let policy = surface_policy();
        set_embedded_mode(false);
        set_low_memory_mode(false);
        assert_eq!(max_texture_size(), policy.max_texture);
        assert_eq!(event_queue_size(), policy.event_queue);
        assert_eq!(font_cache_size(), policy.font_cache_bytes);
        assert_eq!(max_widgets(), policy.max_widgets);

        set_low_memory_mode(true);
        assert!(
            font_cache_size() <= policy.font_cache_bytes,
            "a caller request may only narrow the profile's budget",
        );
        set_low_memory_mode(false);
    }
}
