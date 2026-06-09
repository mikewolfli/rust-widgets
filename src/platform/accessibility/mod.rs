//! Platform accessibility bridges (macOS NSAccessibility, Windows UIAutomation, Linux AT-SPI).
//!
//! This module provides the foundation for OS-level accessibility integration.
//! Each platform backend can implement the `AccessibilityBridge` trait to expose
//! widget information to screen readers and other assistive technologies.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

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

/// ARIA property mapping for accessibility (BLUE11 R7.5).
#[derive(Debug, Clone, Default)]
pub struct AriaProperties {
    /// aria-label — overrides the accessible name.
    pub label: Option<String>,
    /// aria-describedby reference.
    pub described_by: Option<String>,
    /// aria-live region (polite, assertive, off).
    pub live_region: Option<String>,
    /// aria-atomic for live regions.
    pub atomic: bool,
    /// aria-busy state.
    pub busy: bool,
    /// Custom key-value ARIA attributes.
    pub custom: Vec<(String, String)>,
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
            // BLUE11 new widgets
            WidgetKind::Switch => AccessibleRole::Button,
            WidgetKind::SearchBox => AccessibleRole::TextField,
            WidgetKind::Chip => AccessibleRole::Button,
            WidgetKind::Badge => AccessibleRole::Label,
            WidgetKind::SkeletonLoader => AccessibleRole::Label,
            WidgetKind::FAB => AccessibleRole::Button,
            WidgetKind::PullToRefresh => AccessibleRole::Group,
            WidgetKind::BottomSheet => AccessibleRole::Group,
            WidgetKind::BottomNavigationBar => AccessibleRole::TabGroup,
            WidgetKind::NavigationDrawer => AccessibleRole::Group,
            WidgetKind::AppBar => AccessibleRole::Group,
            WidgetKind::MobileDatePicker => AccessibleRole::SpinButton,
            WidgetKind::Divider => AccessibleRole::Group,
            WidgetKind::Stepper => AccessibleRole::SpinButton,
            WidgetKind::Rating => AccessibleRole::Slider,
            WidgetKind::Avatar => AccessibleRole::Image,
            WidgetKind::EmptyState => AccessibleRole::Group,
            WidgetKind::Carousel => AccessibleRole::TabGroup,
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
    /// Set ARIA properties on a widget.
    fn set_aria_properties(&self, _id: ObjectId, _props: &AriaProperties) {}
}
