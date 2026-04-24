//! Application lifecycle wrapper and type-safe widget handles.
//!
//! This is the **primary entry-point** for end-user applications.
//! Prefer using [`App`] + [`AppConfig`] + [`WidgetHandle`] over the
//! low-level crate-level functions.

mod app_core;
mod handle;

pub use app_core::{App, AppConfig};
pub use handle::{
    dispatch_trigger, ButtonHandle, CheckBoxHandle, ComboBoxHandle, LabelHandle,
    LineEditHandle, ListBoxHandle, ListViewHandle, MessageBoxHandle, PanelHandle,
    ProgressBarHandle, RadioButtonHandle, ScrollAreaHandle, SliderHandle, SpinBoxHandle,
    WidgetHandle, WindowHandle,
};
