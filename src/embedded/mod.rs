//! Embedded system optimizations and support.

pub mod config;
pub mod dpi;
pub mod input;
pub mod lightweight;

pub use config::*;
pub use dpi::*;
pub use input::*;
pub use lightweight::*;

use crate::core::Size;
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

/// Get recommended buffer size for current mode
pub fn recommended_buffer_size() -> Size {
    if is_low_memory_mode() {
        Size::new(800, 600)
    } else {
        Size::new(1920, 1080)
    }
}

/// Get maximum recommended texture size
pub fn max_texture_size() -> u32 {
    if is_embedded_mode() {
        1024
    } else {
        4096
    }
}

/// Get recommended font cache size
pub fn font_cache_size() -> usize {
    if is_low_memory_mode() {
        256 * 1024
    } else {
        2 * 1024 * 1024
    }
}

/// Get recommended event queue size
pub fn event_queue_size() -> usize {
    if is_embedded_mode() {
        64
    } else {
        256
    }
}

/// Initialize embedded environment with optimal settings
pub fn init_embedded(config: EmbeddedConfig) {
    set_embedded_mode(true);
    set_low_memory_mode(config.low_memory_mode);

    if let Some(dpi) = config.fixed_dpi {
        set_fixed_dpi(dpi);
    }
}

/// Restore desktop environment settings
pub fn init_desktop() {
    set_embedded_mode(false);
    set_low_memory_mode(false);
    clear_fixed_dpi();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_mode() {
        set_embedded_mode(true);
        assert!(is_embedded_mode());

        set_embedded_mode(false);
        assert!(!is_embedded_mode());
    }

    #[test]
    fn test_low_memory_mode() {
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
