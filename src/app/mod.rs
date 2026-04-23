//! Application lifecycle wrapper and type-safe widget handles.

mod app_core;
mod handle;

pub use app_core::App;
pub use handle::{
    ButtonHandle, CheckBoxHandle, ComboBoxHandle, LabelHandle, LineEditHandle,
    ListBoxHandle, ListViewHandle, MessageBoxHandle, PanelHandle, ProgressBarHandle,
    RadioButtonHandle, ScrollAreaHandle, SliderHandle, SpinBoxHandle, WindowHandle,
};
