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
/// The policy's frugal rows already imply the reduced budget, so an embedded or
/// `mini` build is frugal whether or not this flag is set — which is why the
/// accessors below test the policy first and this flag only as a further narrowing.
fn budget_is_reduced() -> bool {
    is_embedded_mode() || is_low_memory_mode() || !surface_policy().os_window
}

/// Get recommended buffer size for current mode
pub fn recommended_buffer_size() -> Size {
    if budget_is_reduced() {
        Size::new(800, 600)
    } else {
        Size::new(1920, 1080)
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

    #[test]
    fn test_buffer_size() {
        set_low_memory_mode(true);
        let low_mem_size = recommended_buffer_size();
        assert_eq!(low_mem_size.width, 800);
        set_low_memory_mode(false);
        let normal_size = recommended_buffer_size();
        assert_eq!(normal_size.width, 1920);
    }
}
