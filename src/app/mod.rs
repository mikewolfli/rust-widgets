// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Application lifecycle wrapper and type-safe widget handles.
//!
//! This is the **primary entry-point** for end-user applications.
//! Prefer using `App` + `AppConfig` + `WidgetHandle` over the
//! low-level crate-level functions.
//!
//! # Reachability
//!
//! **State:** Production callers: `examples/control_property_uniform.rs:46`, `examples/menu_shortcut_runtime.rs:12`, `examples/control_creation_is_single_mechanism.rs:42`, `examples/macos_window_state_async_probe.rs:36`. The documented primary entry point for applications, and it has real external consumers.

mod app_core;
mod handle;
pub mod lifecycle;

pub use app_core::{App, AppConfig};
/// Test-only seam onto the value-changed router, re-exported so a frame-driver test can
/// observe the dispatch path without a window handle. See the function's own docs.
#[cfg(test)]
pub(crate) use handle::set_widget_value_callback as handle_set_widget_value_callback;
pub use handle::{
    dispatch_trigger, drain_triggers, ButtonHandle, CheckBoxHandle, CheckState, ComboBoxHandle,
    CustomWidgetHandle, CustomWidgetMountError, DialogHandle, EchoMode, FrameHandle,
    GridWidgetHandle, LabelHandle, LineEditHandle, ListBoxHandle, ListModel, ListViewHandle,
    MenuBarHandle, MenuHandle, MenuItemHandle, MessageBoxHandle, PanelHandle, ProgressBarHandle,
    RadioButtonHandle, ScrollAreaHandle, ScrollBarHandle, SelectionMode, SliderHandle,
    SpinBoxHandle, StatusBarHandle, SurfaceHandle, SurfaceMountError, TabWidgetHandle,
    TextEditHandle, ToolBarHandle, WebViewHandle, WidgetHandle, WindowHandle,
};
pub use lifecycle::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_core_default() {
        let config = crate::app::app_core::AppConfig::default();
        assert_eq!(config.app_name, "");
        assert_eq!(config.version, "");
        assert!(config.enable_i18n);
        assert!(config.enable_accessibility);
    }
    #[test]
    fn app_handle_set_get() {
        // Test basic handle operations
        let handle = WindowHandle::from_raw(42);
        assert_eq!(handle.raw_id(), 42);
    }
}
