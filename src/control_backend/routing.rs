// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::control_backend::types::ControlRoutePreference;
use crate::widget::WidgetKind;
/// Returns the policy preference for one widget kind.
///
/// # Why this is a constant
///
/// Every `WidgetKind` is painted by the library: the platform layer was reduced to
/// host capabilities (`create_window` / `mount_surface` / …) and no longer offers
/// controls to map onto, so there is no second mechanism left to choose between.
/// The function is kept — rather than deleted along with
/// `ControlRoutePreference` — because `ControlBackend` selection still reads it,
/// and because the day a backend gains a real primitive that must be used, that
/// has to be a *deliberate* edit here rather than an implicit fallback. See
/// BLUE15 rules #55 and #61.
///
/// The original table routed roughly twenty kinds to `NativePreferred` and
/// resolved the rest through nine category arms plus a runtime query against
/// `Platform::native_widget_kinds`. All of that was answering a question — "which
/// mechanism paints this?" — that no longer has two answers.
pub fn route_preference_for_widget_kind(_kind: WidgetKind) -> ControlRoutePreference {
    ControlRoutePreference::CustomRequired
}

#[cfg(all(test, widgets_unstripped))]
mod tests {
    use super::*;
    use crate::widget::WidgetKind;

    /// Number of `WidgetKind` variants, as pinned by `tests::widget_kind_count`.
    ///
    /// Restated here so the routing table cannot silently fall behind the enum:
    /// the array below is the routing table's coverage list, and comparing its
    /// length to this constant fails the build's tests when a variant is added
    /// without a routing decision.
    ///
    /// # Why this is a hand-maintained copy
    ///
    /// The copy is deliberate — it is what makes "a variant was added without a
    /// routing decision" a test failure rather than a silent default — but it
    /// means the constant and the array must be updated *together*, and it drifted:
    /// `NumberPicker`, `OtpInput`, `Pagination` and `Banner` were added to the enum
    /// (167 → 171) and to the factory, while this constant and array stayed at 167.
    /// The test below could not catch that, because it compares the array to the
    /// constant rather than to `kind.rs`; `check_widget_kind_count.sh` could not
    /// either, because it compares prose numbers to each other. See BLUE16 §十二
    /// E-2 and the `check_kind_reachability.sh` gate it adds.
    /// `PagerPageView` and `TileView` were then deleted outright (171 → 169), so the
    /// constant and the array were updated together in that change.
    const EXPECTED_WIDGET_KIND_COUNT: usize = 169;

    /// Every widget kind must be routed to the library.
    ///
    /// This replaces two weaker tests: the original only asserted "one of two
    /// valid values", which would have accepted a regression back to the
    /// platform-held path, and four others branched on `cfg(target_os)` to encode
    /// per-platform expectations. All of that described a question that no longer
    /// has two answers (BLUE15 rules #55/#68).
    #[test]
    fn all_widget_kinds_are_routed() {
        // The complete variant list, kept explicit so adding a `WidgetKind`
        // without deciding its route is a compile-time reminder rather than a
        // silent default.
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
            WidgetKind::RadarChart,
            WidgetKind::KanbanBoard,
            WidgetKind::Cascader,
            WidgetKind::QueryBuilder,
            WidgetKind::EmojiPicker,
            WidgetKind::Mention,
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
            // Input and navigation widgets added in BLUE16 phase E-2
            WidgetKind::NumberPicker,
            WidgetKind::OtpInput,
            WidgetKind::Pagination,
            WidgetKind::Banner,
            // Controls split out of an overloaded kind, or added in BLUE16 phase E-6
            WidgetKind::ColorPicker,
            WidgetKind::Toast,
            WidgetKind::SplashScreen,
        ];

        assert_eq!(
            all.len(),
            EXPECTED_WIDGET_KIND_COUNT,
            "the routing table must cover every WidgetKind"
        );

        for kind in &all {
            assert_eq!(
                route_preference_for_widget_kind(*kind),
                ControlRoutePreference::CustomRequired,
                "WidgetKind::{kind:?} must be painted by the library; a NativePreferred route would mean a second, platform-held mechanism came back",
            );
        }
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn route_preference_is_single_valued_for_every_kind() {
        // `ControlRoutePreference` keeps both variants (BLUE15 §七 registers its
        // removal as a BLUE16 candidate), but the policy must only ever produce one
        // of them. A `NativePreferred` answer here would mean a second,
        // platform-held mechanism came back.
        for kind in [WidgetKind::Button, WidgetKind::Canvas, WidgetKind::Window] {
            assert_eq!(
                route_preference_for_widget_kind(kind),
                ControlRoutePreference::CustomRequired,
                "WidgetKind::{kind:?} must be painted by the library",
            );
        }
    }
}
