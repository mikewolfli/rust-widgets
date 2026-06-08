//! Application lifecycle wrapper and type-safe widget handles.
//!
//! This is the **primary entry-point** for end-user applications.
//! Prefer using `App` + `AppConfig` + `WidgetHandle` over the
//! low-level crate-level functions.

mod app_core;
mod handle;

pub use app_core::{App, AppConfig};
pub use handle::{
    dispatch_trigger, ButtonHandle, CheckBoxHandle, CheckState, ComboBoxHandle, DialogHandle,
    EchoMode, FrameHandle, GridWidgetHandle, LabelHandle, LineEditHandle, ListBoxHandle, ListModel,
    ListViewHandle, MessageBoxHandle, PanelHandle, ProgressBarHandle, RadioButtonHandle,
    ScrollAreaHandle, ScrollBarHandle, SelectionMode, SliderHandle, SpinBoxHandle, TabWidgetHandle,
    TextEditHandle, WebViewHandle, WidgetHandle, WindowHandle,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_core_default() {
        let config = crate::app::app_core::AppConfig::default();
        assert_eq!(config.app_name, "");
        assert!(config.enable_i18n);
    }
    #[test]
    fn app_handle_set_get() {
        // Test basic handle operations
        let handle = WindowHandle::from_raw(42);
        assert_eq!(handle.raw_id(), 42);
    }
}
