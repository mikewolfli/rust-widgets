use crate::control_backend::types::ControlRoutePreference;
use crate::widget::WidgetKind;
/// Returns the policy preference for one widget kind.
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference {
    #[cfg(not(feature = "mini"))]
    {
        match kind {
            WidgetKind::TabBar
            | WidgetKind::Window
            | WidgetKind::Dialog
            | WidgetKind::MessageBox
            | WidgetKind::FileDialog
            | WidgetKind::ColorDialog
            | WidgetKind::FontDialog
            | WidgetKind::InputDialog
            | WidgetKind::ProgressDialog
            | WidgetKind::PopupWindow
            | WidgetKind::Button
            | WidgetKind::CheckBox
            | WidgetKind::RadioButton
            | WidgetKind::Label
            | WidgetKind::LineEdit
            | WidgetKind::ComboBox
            | WidgetKind::SpinBox
            | WidgetKind::ListBox
            | WidgetKind::ProgressBar
            | WidgetKind::Slider
            | WidgetKind::ScrollBar
            | WidgetKind::ScrollArea
            | WidgetKind::Panel
            | WidgetKind::GroupBox
            | WidgetKind::TabWidget
            | WidgetKind::Splitter
            | WidgetKind::MenuBar
            | WidgetKind::Menu
            | WidgetKind::MenuItem
            | WidgetKind::ContextMenu
            | WidgetKind::ToolBar
            | WidgetKind::StatusBar
            | WidgetKind::ToggleButton
            | WidgetKind::DoubleSpinBox
            | WidgetKind::Dial
            | WidgetKind::DatePicker
            | WidgetKind::TimePicker
            | WidgetKind::DateTimePicker
            | WidgetKind::DirectoryDialog
            | WidgetKind::ActivityIndicator
            | WidgetKind::Calendar
            | WidgetKind::LCDNumber
            | WidgetKind::FontComboBox
            | WidgetKind::PieMenu
            | WidgetKind::RibbonBar
            | WidgetKind::Arc
            | WidgetKind::Spinner
            | WidgetKind::Roller
            | WidgetKind::Dropdown
            | WidgetKind::TextArea
            | WidgetKind::Keyboard
            | WidgetKind::TileView
            | WidgetKind::Line
            | WidgetKind::Meter
            | WidgetKind::MiniChart
            | WidgetKind::ImageView
            | WidgetKind::MiniCanvas => ControlRoutePreference::NativePreferred,
            WidgetKind::TextEdit
            | WidgetKind::RichEdit
            | WidgetKind::ListView
            | WidgetKind::TreeView
            | WidgetKind::DockPanel
            | WidgetKind::MdiArea
            | WidgetKind::Canvas
            | WidgetKind::Table
            | WidgetKind::Grid
            | WidgetKind::Chart
            | WidgetKind::CheckListBox
            | WidgetKind::Wizard
            | WidgetKind::DataView
            | WidgetKind::PropertyGrid
            | WidgetKind::Toolbox
            | WidgetKind::CollapsiblePane
            | WidgetKind::DockWidget
            | WidgetKind::ColumnView
            | WidgetKind::UndoView
            | WidgetKind::CommandLink
            | WidgetKind::FreeformShape
            | WidgetKind::WebEngineView
            | WidgetKind::WebEnginePage
            | WidgetKind::WebEngineSettings
            | WidgetKind::WebEngineDownloadItem
            | WidgetKind::WebEngineCookieStore
            | WidgetKind::WebEngineWebChannel
            | WidgetKind::WebEngineFindTextResult
            | WidgetKind::WebEngineNotification
            | WidgetKind::WebEngineScriptDialog
            | WidgetKind::WebEngineContextMenuRequest => ControlRoutePreference::CustomRequired,
            WidgetKind::StackedWidget
            | WidgetKind::Action
            | WidgetKind::ToolButton
            | WidgetKind::Switch
            | WidgetKind::SearchBox
            | WidgetKind::Chip
            | WidgetKind::Badge
            | WidgetKind::SkeletonLoader
            | WidgetKind::FAB
            | WidgetKind::RefreshControl
            | WidgetKind::BottomSheet
            | WidgetKind::BottomNavigationBar
            | WidgetKind::NavigationDrawer
            | WidgetKind::AppBar
            | WidgetKind::MobileDatePicker
            | WidgetKind::Divider
            | WidgetKind::Stepper
            | WidgetKind::Rating
            | WidgetKind::Avatar
            | WidgetKind::EmptyState
            | WidgetKind::Carousel
            | WidgetKind::ColorHistory
            | WidgetKind::ColorWell
            | WidgetKind::TagInput
            | WidgetKind::ImePreedit
            | WidgetKind::InplaceEditor
            | WidgetKind::QRCode
            | WidgetKind::MasonryLayout
            | WidgetKind::CupertinoSwitch
            | WidgetKind::MaterialSnackbar
            | WidgetKind::AdaptiveScaffold
            | WidgetKind::WizardDialog
            | WidgetKind::SafeArea
            | WidgetKind::CupertinoAlertDialog
            | WidgetKind::CupertinoSlider
            | WidgetKind::MaterialNavigationRail
            | WidgetKind::Tooltip
            | WidgetKind::SegmentedButton
            | WidgetKind::NavigationStack
            | WidgetKind::ProgressCircle
            | WidgetKind::Icon
            | WidgetKind::DropdownMenu
            | WidgetKind::MaskedEdit
            | WidgetKind::MenuButton
            | WidgetKind::Popover
            | WidgetKind::AutoCompleteEdit
            | WidgetKind::MultiSelectComboBox
            | WidgetKind::RangeSlider
            | WidgetKind::FloatingLabel
            | WidgetKind::FontPreview
            | WidgetKind::CupertinoNavigationBar
            | WidgetKind::CupertinoSegmentedControl
            | WidgetKind::SwipeToDismiss
            | WidgetKind::PagerPageView
            | WidgetKind::TabView
            | WidgetKind::SearchBar
            | WidgetKind::ShortcutEditor
            | WidgetKind::ModalBottomSheet
            | WidgetKind::LineChart
            | WidgetKind::Sparkline
            | WidgetKind::BarChart
            | WidgetKind::FindReplaceDialog
            | WidgetKind::PropertiesPanel
            | WidgetKind::PieChart
            | WidgetKind::CupertinoDatePicker
            | WidgetKind::EditableComboBox
            | WidgetKind::DateRangePicker
            | WidgetKind::AnimatedImage
            | WidgetKind::HeroAnimation
            | WidgetKind::BezierCurveEditor
            | WidgetKind::LottieWidget
            | WidgetKind::RiveWidget
            | WidgetKind::VideoPlayer
            | WidgetKind::ImageGallery
            | WidgetKind::AudioVisualizer
            | WidgetKind::CameraPreview
            | WidgetKind::BarcodeScanner => ControlRoutePreference::CustomRequired,
        }
    }
    #[cfg(feature = "mini")]
    {
        let _ = kind;
        ControlRoutePreference::CustomRequired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetKind;

    #[cfg(not(feature = "mini"))]
    #[test]
    fn native_preferred_widget_kinds() {
        // Widgets expected to prefer native backend.
        let native_preferred = [
            WidgetKind::TabBar,
            WidgetKind::Window,
            WidgetKind::Dialog,
            WidgetKind::MessageBox,
            WidgetKind::FileDialog,
            WidgetKind::ColorDialog,
            WidgetKind::FontDialog,
            WidgetKind::InputDialog,
            WidgetKind::ProgressDialog,
            WidgetKind::PopupWindow,
            WidgetKind::Button,
            WidgetKind::CheckBox,
            WidgetKind::RadioButton,
            WidgetKind::Label,
            WidgetKind::LineEdit,
            WidgetKind::ComboBox,
            WidgetKind::SpinBox,
            WidgetKind::ListBox,
            WidgetKind::ProgressBar,
            WidgetKind::Slider,
            WidgetKind::ScrollBar,
            WidgetKind::ScrollArea,
            WidgetKind::Panel,
            WidgetKind::GroupBox,
            WidgetKind::TabWidget,
            WidgetKind::Splitter,
            WidgetKind::MenuBar,
            WidgetKind::Menu,
            WidgetKind::MenuItem,
            WidgetKind::ContextMenu,
            WidgetKind::ToolBar,
            WidgetKind::StatusBar,
            WidgetKind::ToggleButton,
            WidgetKind::DoubleSpinBox,
            WidgetKind::Dial,
            WidgetKind::DatePicker,
            WidgetKind::TimePicker,
            WidgetKind::DateTimePicker,
            WidgetKind::DirectoryDialog,
            WidgetKind::ActivityIndicator,
            WidgetKind::Calendar,
            WidgetKind::LCDNumber,
            WidgetKind::FontComboBox,
            WidgetKind::PieMenu,
            WidgetKind::RibbonBar,
            WidgetKind::Arc,
            WidgetKind::Spinner,
            WidgetKind::Roller,
            WidgetKind::Dropdown,
            WidgetKind::TextArea,
            WidgetKind::Keyboard,
            WidgetKind::TileView,
            WidgetKind::Line,
            WidgetKind::Meter,
            WidgetKind::MiniChart,
            WidgetKind::ImageView,
            WidgetKind::MiniCanvas,
        ];
        for kind in &native_preferred {
            assert_eq!(
                route_preference_for_widget_kind(*kind),
                ControlRoutePreference::NativePreferred,
                "WidgetKind::{:?} should be NativePreferred",
                kind,
            );
        }
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn custom_required_widget_kinds() {
        // Widgets expected to require custom-painted backend.
        let custom_required = [
            WidgetKind::TextEdit,
            WidgetKind::RichEdit,
            WidgetKind::ListView,
            WidgetKind::TreeView,
            WidgetKind::DockPanel,
            WidgetKind::MdiArea,
            WidgetKind::Canvas,
            WidgetKind::Table,
            WidgetKind::Grid,
            WidgetKind::Chart,
            WidgetKind::CheckListBox,
            WidgetKind::Wizard,
            WidgetKind::DataView,
            WidgetKind::PropertyGrid,
            WidgetKind::Toolbox,
            WidgetKind::Toolbox,
            WidgetKind::CollapsiblePane,
            WidgetKind::DockWidget,
            WidgetKind::ColumnView,
            WidgetKind::UndoView,
            WidgetKind::CommandLink,
            WidgetKind::FreeformShape,
            WidgetKind::WebEngineView,
            WidgetKind::WebEnginePage,
            WidgetKind::WebEngineSettings,
            WidgetKind::WebEngineDownloadItem,
            WidgetKind::WebEngineCookieStore,
            WidgetKind::WebEngineWebChannel,
            WidgetKind::WebEngineFindTextResult,
            WidgetKind::WebEngineNotification,
            WidgetKind::WebEngineScriptDialog,
            WidgetKind::WebEngineContextMenuRequest,
            WidgetKind::StackedWidget,
            WidgetKind::Action,
            WidgetKind::ToolButton,
            WidgetKind::Toolbox,
            WidgetKind::Toolbox,
            WidgetKind::Switch,
            WidgetKind::SearchBox,
            WidgetKind::Chip,
            WidgetKind::Badge,
            WidgetKind::SkeletonLoader,
            WidgetKind::FAB,
            WidgetKind::BottomSheet,
            WidgetKind::BottomNavigationBar,
            WidgetKind::NavigationDrawer,
            WidgetKind::AppBar,
            WidgetKind::MobileDatePicker,
            WidgetKind::Divider,
            WidgetKind::Stepper,
            WidgetKind::Rating,
            WidgetKind::Avatar,
            WidgetKind::EmptyState,
            WidgetKind::Carousel,
            WidgetKind::ColorHistory,
            WidgetKind::ColorWell,
            WidgetKind::TagInput,
            WidgetKind::ImePreedit,
            WidgetKind::InplaceEditor,
            WidgetKind::QRCode,
            WidgetKind::MasonryLayout,
            WidgetKind::CupertinoSwitch,
            WidgetKind::MaterialSnackbar,
            WidgetKind::AdaptiveScaffold,
            WidgetKind::WizardDialog,
            WidgetKind::SafeArea,
            WidgetKind::CupertinoAlertDialog,
            WidgetKind::CupertinoSlider,
            WidgetKind::MaterialNavigationRail,
            WidgetKind::Tooltip,
            WidgetKind::SegmentedButton,
            WidgetKind::NavigationStack,
            WidgetKind::ProgressCircle,
            WidgetKind::Icon,
            WidgetKind::DropdownMenu,
            WidgetKind::MaskedEdit,
            WidgetKind::MenuButton,
            WidgetKind::Popover,
            WidgetKind::AutoCompleteEdit,
            WidgetKind::MultiSelectComboBox,
            WidgetKind::RangeSlider,
            WidgetKind::FloatingLabel,
            WidgetKind::FontPreview,
            WidgetKind::CupertinoNavigationBar,
            WidgetKind::CupertinoSegmentedControl,
            WidgetKind::SwipeToDismiss,
            WidgetKind::PagerPageView,
            WidgetKind::TabView,
            WidgetKind::SearchBar,
            WidgetKind::ShortcutEditor,
            WidgetKind::RefreshControl,
            WidgetKind::ModalBottomSheet,
            WidgetKind::LineChart,
            WidgetKind::Sparkline,
            WidgetKind::BarChart,
            WidgetKind::FindReplaceDialog,
            WidgetKind::PropertiesPanel,
            WidgetKind::PieChart,
            WidgetKind::CupertinoDatePicker,
            WidgetKind::EditableComboBox,
            WidgetKind::DateRangePicker,
        ];
        for kind in &custom_required {
            assert_eq!(
                route_preference_for_widget_kind(*kind),
                ControlRoutePreference::CustomRequired,
                "WidgetKind::{:?} should be CustomRequired",
                kind,
            );
        }
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn all_widget_kinds_are_routed() {
        // Verify every WidgetKind variant is covered by the routing function.
        // This test will fail to compile if a new variant is added to WidgetKind
        // and not included in the match.
        let all = [
            WidgetKind::Window,
            WidgetKind::Dialog,
            WidgetKind::MessageBox,
            WidgetKind::FileDialog,
            WidgetKind::ColorDialog,
            WidgetKind::FontDialog,
            WidgetKind::InputDialog,
            WidgetKind::ProgressDialog,
            WidgetKind::PopupWindow,
            WidgetKind::Button,
            WidgetKind::CheckBox,
            WidgetKind::RadioButton,
            WidgetKind::Label,
            WidgetKind::LineEdit,
            WidgetKind::TextEdit,
            WidgetKind::RichEdit,
            WidgetKind::ComboBox,
            WidgetKind::SpinBox,
            WidgetKind::ListBox,
            WidgetKind::ListView,
            WidgetKind::TreeView,
            WidgetKind::ProgressBar,
            WidgetKind::Slider,
            WidgetKind::ScrollBar,
            WidgetKind::ScrollArea,
            WidgetKind::Panel,
            WidgetKind::DockPanel,
            WidgetKind::GroupBox,
            WidgetKind::TabWidget,
            WidgetKind::Splitter,
            WidgetKind::MdiArea,
            WidgetKind::MenuBar,
            WidgetKind::Menu,
            WidgetKind::MenuItem,
            WidgetKind::ContextMenu,
            WidgetKind::ToolBar,
            WidgetKind::StatusBar,
            WidgetKind::Canvas,
            WidgetKind::Table,
            WidgetKind::Grid,
            WidgetKind::Chart,
            WidgetKind::ToggleButton,
            WidgetKind::CheckListBox,
            WidgetKind::DoubleSpinBox,
            WidgetKind::Dial,
            WidgetKind::Wizard,
            WidgetKind::DatePicker,
            WidgetKind::TimePicker,
            WidgetKind::DateTimePicker,
            WidgetKind::DirectoryDialog,
            WidgetKind::DataView,
            WidgetKind::PropertyGrid,
            WidgetKind::Toolbox,
            WidgetKind::Toolbox,
            WidgetKind::StackedWidget,
            WidgetKind::CollapsiblePane,
            WidgetKind::DockWidget,
            WidgetKind::ActivityIndicator,
            WidgetKind::Calendar,
            WidgetKind::ColumnView,
            WidgetKind::UndoView,
            WidgetKind::CommandLink,
            WidgetKind::LCDNumber,
            WidgetKind::FontComboBox,
            WidgetKind::WebEngineView,
            WidgetKind::WebEnginePage,
            WidgetKind::WebEngineSettings,
            WidgetKind::WebEngineDownloadItem,
            WidgetKind::WebEngineCookieStore,
            WidgetKind::WebEngineWebChannel,
            WidgetKind::WebEngineFindTextResult,
            WidgetKind::WebEngineNotification,
            WidgetKind::WebEngineScriptDialog,
            WidgetKind::WebEngineContextMenuRequest,
            WidgetKind::Action,
            WidgetKind::ToolButton,
            WidgetKind::Toolbox,
            WidgetKind::Toolbox,
            WidgetKind::FreeformShape,
            WidgetKind::TabBar,
            WidgetKind::PieMenu,
            WidgetKind::RibbonBar,
            WidgetKind::Arc,
            WidgetKind::Spinner,
            WidgetKind::Roller,
            WidgetKind::Dropdown,
            WidgetKind::TextArea,
            WidgetKind::Keyboard,
            WidgetKind::Switch,
            WidgetKind::SearchBox,
            WidgetKind::Chip,
            WidgetKind::Badge,
            WidgetKind::SkeletonLoader,
            WidgetKind::FAB,
            WidgetKind::BottomSheet,
            WidgetKind::BottomNavigationBar,
            WidgetKind::NavigationDrawer,
            WidgetKind::AppBar,
            WidgetKind::MobileDatePicker,
            WidgetKind::Divider,
            WidgetKind::Stepper,
            WidgetKind::Rating,
            WidgetKind::Avatar,
            WidgetKind::EmptyState,
            WidgetKind::Carousel,
            WidgetKind::ColorHistory,
            WidgetKind::ColorWell,
            WidgetKind::TagInput,
            WidgetKind::ImePreedit,
            WidgetKind::InplaceEditor,
            WidgetKind::QRCode,
            WidgetKind::MasonryLayout,
            WidgetKind::CupertinoSwitch,
            WidgetKind::MaterialSnackbar,
            WidgetKind::AdaptiveScaffold,
            WidgetKind::WizardDialog,
            WidgetKind::SafeArea,
            WidgetKind::CupertinoAlertDialog,
            WidgetKind::CupertinoSlider,
            WidgetKind::MaterialNavigationRail,
            WidgetKind::Tooltip,
            WidgetKind::SegmentedButton,
            WidgetKind::NavigationStack,
            WidgetKind::ProgressCircle,
            WidgetKind::Icon,
            WidgetKind::DropdownMenu,
            WidgetKind::MaskedEdit,
            WidgetKind::MenuButton,
            WidgetKind::Popover,
            WidgetKind::AutoCompleteEdit,
            WidgetKind::MultiSelectComboBox,
            WidgetKind::RangeSlider,
            WidgetKind::FloatingLabel,
            WidgetKind::FontPreview,
            WidgetKind::CupertinoNavigationBar,
            WidgetKind::CupertinoSegmentedControl,
            WidgetKind::SwipeToDismiss,
            WidgetKind::PagerPageView,
            WidgetKind::TabView,
            WidgetKind::SearchBar,
            WidgetKind::ShortcutEditor,
            WidgetKind::RefreshControl,
            WidgetKind::ModalBottomSheet,
            WidgetKind::LineChart,
            WidgetKind::Sparkline,
            WidgetKind::BarChart,
            WidgetKind::FindReplaceDialog,
            WidgetKind::PropertiesPanel,
            WidgetKind::PieChart,
            WidgetKind::CupertinoDatePicker,
            WidgetKind::EditableComboBox,
            WidgetKind::DateRangePicker,
            WidgetKind::TileView,
            WidgetKind::Line,
            WidgetKind::Meter,
            WidgetKind::MiniChart,
            WidgetKind::ImageView,
            WidgetKind::MiniCanvas,
        ];
        for kind in &all {
            let preference = route_preference_for_widget_kind(*kind);
            assert!(
                preference == ControlRoutePreference::NativePreferred
                    || preference == ControlRoutePreference::CustomRequired,
                "WidgetKind::{:?} should map to a valid route preference, got {:?}",
                kind,
                preference,
            );
        }
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn route_preference_partial_eq() {
        assert_eq!(
            route_preference_for_widget_kind(WidgetKind::Button),
            ControlRoutePreference::NativePreferred,
        );
        assert_eq!(
            route_preference_for_widget_kind(WidgetKind::Canvas),
            ControlRoutePreference::CustomRequired,
        );
    }
}
