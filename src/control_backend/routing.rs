use crate::control_backend::types::ControlRoutePreference;
use crate::widget::WidgetKind;
/// Returns the policy preference for one widget kind.
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference {
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    {
        // Windows-specific overrides. SpinBox/ListView/ScrollArea are no longer
        // listed here: they now have real native implementations (up-down,
        // SysListView32, a scrollable child window) and route natively above.
        // The dialogs still lack a dedicated native path in the default build, so
        // they keep routing to the custom backend on Windows.
        #[cfg(target_os = "windows")]
        if matches!(
            kind,
            WidgetKind::MessageBox
                | WidgetKind::FileDialog
                | WidgetKind::ColorDialog
                | WidgetKind::FontDialog
        ) {
            return ControlRoutePreference::CustomRequired;
        }

        match kind {
            // NOTE: these kinds are routed to the custom backend because the
            // native path has no dedicated primitive and silently degrades to a
            // *different* control (losing the widget's identity). Examples:
            // `create_date_picker` -> `create_panel`, `create_dial` ->
            // `create_slider`, `create_lcd_number` -> `create_label`. The custom
            // backend has a dedicated `create_*` for each, so routing there makes
            // them fully functional instead of downgraded.
            // Per-platform native implementations can be added later and will
            // move kinds back to `NativePreferred` once they exist.
            WidgetKind::DatePicker
            | WidgetKind::TimePicker
            | WidgetKind::DateTimePicker
            | WidgetKind::Calendar
            | WidgetKind::ActivityIndicator
            | WidgetKind::Dial
            | WidgetKind::LCDNumber
            | WidgetKind::FontComboBox
            | WidgetKind::DoubleSpinBox
            | WidgetKind::ToggleButton
            | WidgetKind::ScrollBar
            | WidgetKind::ScrollArea
            | WidgetKind::TabWidget
            | WidgetKind::Splitter
            | WidgetKind::GroupBox
            | WidgetKind::Frame
            | WidgetKind::ContextMenu
            | WidgetKind::MenuItem
            | WidgetKind::DirectoryDialog
            | WidgetKind::Dialog
            | WidgetKind::InputDialog
            | WidgetKind::ProgressDialog
            | WidgetKind::PopupWindow => ControlRoutePreference::CustomRequired,
            WidgetKind::Window
            | WidgetKind::MessageBox
            | WidgetKind::FileDialog
            | WidgetKind::ColorDialog
            | WidgetKind::FontDialog
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
            | WidgetKind::Panel
            | WidgetKind::MenuBar
            | WidgetKind::Menu
            | WidgetKind::ToolBar
            | WidgetKind::StatusBar => ControlRoutePreference::NativePreferred,
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
            | WidgetKind::TabBar
            | WidgetKind::PieMenu
            | WidgetKind::RibbonBar
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
            | WidgetKind::BarcodeScanner
            | WidgetKind::GridTable
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
            | WidgetKind::MiniCanvas => ControlRoutePreference::CustomRequired,
        }
    }
    #[cfg(any(feature = "mini", feature = "embedded"))]
    {
        let _ = kind;
        ControlRoutePreference::CustomRequired
    }
}

#[cfg(all(test, not(any(feature = "mini", feature = "embedded"))))]
mod tests {
    use super::*;
    use crate::widget::WidgetKind;

    #[cfg(all(not(any(feature = "mini", feature = "embedded")), not(target_os = "windows")))]
    #[test]
    fn native_preferred_widget_kinds() {
        // Widgets expected to prefer a native backend: every one of these has a
        // dedicated native create path on the platform backends.
        let native_preferred = [
            WidgetKind::Window,
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
            WidgetKind::Panel,
            WidgetKind::MenuBar,
            WidgetKind::Menu,
            WidgetKind::ToolBar,
            WidgetKind::StatusBar,
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

    /// Kinds whose native path has no dedicated primitive and silently degrades
    /// to a *different* control must route to the custom backend so they stay
    /// fully functional instead of losing their identity.
    ///
    /// This pins the 2026-09-11 change: `create_date_picker` -> `create_panel`,
    /// `create_dial` -> `create_slider`, `create_lcd_number` -> `create_label`,
    /// and similar. Routing them natively would return the wrong widget.
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    #[test]
    fn native_path_degraded_kinds_use_custom_backend() {
        let degraded = [
            WidgetKind::DatePicker,
            WidgetKind::TimePicker,
            WidgetKind::DateTimePicker,
            WidgetKind::Calendar,
            WidgetKind::ActivityIndicator,
            WidgetKind::Dial,
            WidgetKind::LCDNumber,
            WidgetKind::FontComboBox,
            WidgetKind::DoubleSpinBox,
            WidgetKind::ToggleButton,
            WidgetKind::ScrollBar,
            WidgetKind::ScrollArea,
            WidgetKind::TabWidget,
            WidgetKind::Splitter,
            WidgetKind::GroupBox,
            WidgetKind::Frame,
            WidgetKind::ContextMenu,
            WidgetKind::MenuItem,
            WidgetKind::DirectoryDialog,
            WidgetKind::Dialog,
            WidgetKind::InputDialog,
            WidgetKind::ProgressDialog,
            WidgetKind::PopupWindow,
        ];
        for kind in degraded {
            assert_eq!(
                route_preference_for_widget_kind(kind),
                ControlRoutePreference::CustomRequired,
                "WidgetKind::{kind:?} degrades on the native path and must use \
                 the custom backend",
            );
        }
    }

    /// On Windows the dialogs still lack a dedicated native path in the default
    /// build, so they route to the custom backend. SpinBox/ListView/ScrollArea
    /// are deliberately absent: they gained real native implementations
    /// (up-down / SysListView32 / scrollable child window) on 2026-09-11 and now
    /// route natively.
    #[cfg(all(not(any(feature = "mini", feature = "embedded")), target_os = "windows"))]
    #[test]
    fn windows_surrogate_widget_kinds_use_custom_backend() {
        let custom_required = [
            WidgetKind::MessageBox,
            WidgetKind::FileDialog,
            WidgetKind::ColorDialog,
            WidgetKind::FontDialog,
        ];

        for kind in custom_required {
            assert_eq!(
                route_preference_for_widget_kind(kind),
                ControlRoutePreference::CustomRequired,
                "WidgetKind::{kind:?} should use the custom backend on Windows",
            );
        }
    }

    /// The Windows native controls must not be re-routed to the custom backend:
    /// that would discard the real Win32 implementations.
    #[cfg(all(not(any(feature = "mini", feature = "embedded")), target_os = "windows"))]
    #[test]
    fn windows_native_controls_route_natively() {
        for kind in [WidgetKind::SpinBox, WidgetKind::ListView, WidgetKind::ScrollArea] {
            assert_eq!(
                route_preference_for_widget_kind(kind),
                ControlRoutePreference::NativePreferred,
                "WidgetKind::{kind:?} has a native Win32 implementation and must \
                 route natively",
            );
        }
    }

    #[cfg(not(any(feature = "mini", feature = "embedded")))]
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
            WidgetKind::CollapsiblePane,
            WidgetKind::DockWidget,
            WidgetKind::ColumnView,
            WidgetKind::UndoView,
            WidgetKind::CommandLink,
            WidgetKind::FreeformShape,
            WidgetKind::TabBar,
            WidgetKind::PieMenu,
            WidgetKind::RibbonBar,
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
            // Media/animation widgets (custom painted)
            WidgetKind::AnimatedImage,
            WidgetKind::HeroAnimation,
            WidgetKind::BezierCurveEditor,
            WidgetKind::LottieWidget,
            WidgetKind::RiveWidget,
            WidgetKind::VideoPlayer,
            WidgetKind::ImageGallery,
            WidgetKind::AudioVisualizer,
            WidgetKind::CameraPreview,
            WidgetKind::BarcodeScanner,
            // Data table widgets
            WidgetKind::GridTable,
            // Self-drawn BLUE13 widgets (custom paint backend owns these)
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
            WidgetKind::Frame,
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
            // Media/animation widgets
            WidgetKind::AnimatedImage,
            WidgetKind::HeroAnimation,
            WidgetKind::BezierCurveEditor,
            WidgetKind::LottieWidget,
            WidgetKind::RiveWidget,
            WidgetKind::VideoPlayer,
            WidgetKind::ImageGallery,
            WidgetKind::AudioVisualizer,
            WidgetKind::CameraPreview,
            WidgetKind::BarcodeScanner,
            // Data table widgets
            WidgetKind::GridTable,
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
