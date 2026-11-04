//! Host-facing platform re-exports.

// Re-exports from platform module for convenience
use crate::platform;

#[allow(deprecated)]
pub use platform::NativeCapabilityContract;
#[cfg(not(alloc_frugal))]
pub use platform::{
    backend_name, capabilities, dpi_scale_factor, get_platform, init as platform_init,
    quit as platform_quit, run as platform_run, runtime_gui_mode, runtime_gui_mode_for,
};
pub use platform::{
    CapabilityContract, DropEvent, EmbeddedCapabilityContract, HostedCapabilityContract,
    MobileBackend, OpenError, PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent,
    WidgetTriggerKind, WindowStateFlag,
};
