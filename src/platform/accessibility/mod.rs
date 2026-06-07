//! Platform accessibility bridges (macOS NSAccessibility, Windows UIAutomation, Linux AT-SPI).
//!
//! This module provides the foundation for OS-level accessibility integration.
//! Each platform backend can implement the `AccessibilityBridge` trait to expose
//! widget information to screen readers and other assistive technologies.

pub mod macos;
// pub mod windows;  // TODO: R7.3
// pub mod linux;    // TODO: R7.4

use crate::core::ObjectId;
use crate::widget::WidgetKind;

/// Accessibility role types corresponding to platform-specific roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibleRole {
    Button,
    CheckBox,
    ComboBox,
    Dialog,
    Group,
    Image,
    Label,
    Link,
    List,
    ListItem,
    Menu,
    MenuBar,
    MenuItem,
    ProgressBar,
    RadioButton,
    ScrollBar,
    Slider,
    SpinButton,
    Splitter,
    StaticText,
    Tab,
    TabGroup,
    Table,
    TextField,
    ToolBar,
    Tree,
    TreeItem,
    Window,
    Unknown,
}

impl From<WidgetKind> for AccessibleRole {
    fn from(kind: WidgetKind) -> Self {
        match kind {
            WidgetKind::Button | WidgetKind::ToggleButton | WidgetKind::ToolButton => {
                AccessibleRole::Button
            }
            WidgetKind::CheckBox | WidgetKind::CheckListBox => AccessibleRole::CheckBox,
            WidgetKind::ComboBox | WidgetKind::FontComboBox => AccessibleRole::ComboBox,
            WidgetKind::Dialog
            | WidgetKind::FileDialog
            | WidgetKind::ColorDialog
            | WidgetKind::FontDialog
            | WidgetKind::InputDialog
            | WidgetKind::ProgressDialog
            | WidgetKind::DirectoryDialog => AccessibleRole::Dialog,
            WidgetKind::Label => AccessibleRole::StaticText,
            WidgetKind::LineEdit | WidgetKind::TextEdit | WidgetKind::RichEdit => {
                AccessibleRole::TextField
            }
            WidgetKind::ListBox | WidgetKind::ListView => AccessibleRole::List,
            WidgetKind::MenuBar => AccessibleRole::MenuBar,
            WidgetKind::Menu | WidgetKind::ContextMenu => AccessibleRole::Menu,
            WidgetKind::MenuItem => AccessibleRole::MenuItem,
            WidgetKind::ProgressBar | WidgetKind::ActivityIndicator => AccessibleRole::ProgressBar,
            WidgetKind::RadioButton => AccessibleRole::RadioButton,
            WidgetKind::ScrollBar => AccessibleRole::ScrollBar,
            WidgetKind::Slider | WidgetKind::Dial => AccessibleRole::Slider,
            WidgetKind::SpinBox | WidgetKind::DoubleSpinBox => AccessibleRole::SpinButton,
            WidgetKind::TabWidget | WidgetKind::TabBar => AccessibleRole::TabGroup,
            WidgetKind::Table | WidgetKind::DataView => AccessibleRole::Table,
            WidgetKind::ToolBar => AccessibleRole::ToolBar,
            WidgetKind::TreeView => AccessibleRole::Tree,
            WidgetKind::Splitter => AccessibleRole::Splitter,
            WidgetKind::Window => AccessibleRole::Window,
            _ => AccessibleRole::Unknown,
        }
    }
}

/// Trait for platform-specific accessibility integration.
pub trait AccessibilityBridge: Send + Sync {
    /// Set the accessible name (label) for a widget.
    fn set_accessibility_name(&self, id: ObjectId, name: &str);
    /// Get the accessible name for a widget.
    fn accessibility_name(&self, id: ObjectId) -> Option<String>;
    /// Post a notification that a widget's accessible name changed.
    fn notify_name_changed(&self, id: ObjectId);
    /// Post a notification that a widget's value changed.
    fn notify_value_changed(&self, id: ObjectId);
    /// Post a notification that a widget's state changed (e.g., enabled/disabled).
    fn notify_state_changed(&self, id: ObjectId);
    /// Post a notification that focus moved to a widget.
    fn notify_focus_changed(&self, id: ObjectId);
}
