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

    /// One routing decision per widget kind, written as an **exhaustive `match`
    /// with no wildcard arm**.
    ///
    /// # Why an exhaustive match replaced an array plus a constant
    ///
    /// The previous version listed every kind in a `[WidgetKind; N]` array and
    /// asserted `array.len() == EXPECTED_WIDGET_KIND_COUNT`, a second hand-copied
    /// list. That compared the two halves of the *same* hand-maintained
    /// bookkeeping, so when `TreeTable`, `Breadcrumb`, `SignaturePad` and
    /// `DropZone` were added to `kind.rs` the array and the constant both stayed
    /// at 175 and the assertion still passed: a check that cannot fail is not a
    /// check. The same drift had already happened once before, from 167 to 171
    /// (see BLUE16 §十二 E-2), and the `check_kind_reachability.sh` gate that
    /// change promised was never written.
    ///
    /// A `match` over `WidgetKind` with every variant spelled out is checked for
    /// exhaustiveness by the compiler. Adding a variant without giving it a
    /// routing decision stops this test build from compiling, so there is no
    /// number left to keep in sync — the drift class is removed rather than
    /// merely detected.
    ///
    /// The arms are grouped with the same category comments the array carried.
    /// Every arm yields `CustomRequired` because the platform layer offers no
    /// second mechanism to route to (see [`route_preference_for_widget_kind`]),
    /// which is why `clippy::match_same_arms` is allowed deliberately here: the
    /// *set* of variants is the information, not the arm bodies.
    #[allow(clippy::match_same_arms)]
    fn route_is_library_painted(kind: WidgetKind) -> ControlRoutePreference {
        match kind {
            WidgetKind::Window
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
            | WidgetKind::TextEdit
            | WidgetKind::RichEdit
            | WidgetKind::ComboBox
            | WidgetKind::SpinBox
            | WidgetKind::ListBox
            | WidgetKind::ListView
            | WidgetKind::TreeView
            | WidgetKind::ProgressBar
            | WidgetKind::Slider
            | WidgetKind::ScrollBar
            | WidgetKind::ScrollArea
            | WidgetKind::Panel
            | WidgetKind::Frame
            | WidgetKind::DockPanel
            | WidgetKind::GroupBox
            | WidgetKind::TabWidget
            | WidgetKind::Splitter
            | WidgetKind::MdiArea
            | WidgetKind::MenuBar
            | WidgetKind::Menu
            | WidgetKind::MenuItem
            | WidgetKind::ContextMenu
            | WidgetKind::ToolBar
            | WidgetKind::StatusBar
            | WidgetKind::Canvas
            | WidgetKind::Table
            | WidgetKind::Grid
            | WidgetKind::Chart
            | WidgetKind::RadarChart
            | WidgetKind::KanbanBoard
            | WidgetKind::Cascader
            | WidgetKind::QueryBuilder
            | WidgetKind::EmojiPicker
            | WidgetKind::Mention
            | WidgetKind::ToggleButton
            | WidgetKind::CheckListBox
            | WidgetKind::DoubleSpinBox
            | WidgetKind::Dial
            | WidgetKind::Wizard
            | WidgetKind::DatePicker
            | WidgetKind::TimePicker
            | WidgetKind::DateTimePicker
            | WidgetKind::DirectoryDialog
            | WidgetKind::DataView
            | WidgetKind::PropertyGrid
            | WidgetKind::Toolbox
            | WidgetKind::StackedWidget
            | WidgetKind::CollapsiblePane
            | WidgetKind::DockWidget
            | WidgetKind::ActivityIndicator
            | WidgetKind::Calendar
            | WidgetKind::ColumnView
            | WidgetKind::UndoView
            | WidgetKind::CommandLink
            | WidgetKind::LCDNumber
            | WidgetKind::FontComboBox
            | WidgetKind::WebEngineView
            | WidgetKind::Action
            | WidgetKind::ToolButton
            | WidgetKind::FreeformShape
            | WidgetKind::TabBar
            | WidgetKind::PieMenu
            | WidgetKind::RibbonBar
            | WidgetKind::Arc
            | WidgetKind::Spinner
            | WidgetKind::Roller
            | WidgetKind::Dropdown
            | WidgetKind::TextArea
            | WidgetKind::Keyboard
            | WidgetKind::Switch
            | WidgetKind::SearchBox
            | WidgetKind::Chip
            | WidgetKind::Badge
            | WidgetKind::SkeletonLoader
            | WidgetKind::FAB
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
            | WidgetKind::TabView
            | WidgetKind::SearchBar
            | WidgetKind::ShortcutEditor
            | WidgetKind::RefreshControl
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
            | WidgetKind::Line
            | WidgetKind::Meter
            | WidgetKind::MiniChart
            | WidgetKind::ImageView
            | WidgetKind::MiniCanvas => ControlRoutePreference::CustomRequired,
            // Media/animation widgets
            WidgetKind::AnimatedImage
            | WidgetKind::HeroAnimation
            | WidgetKind::BezierCurveEditor
            | WidgetKind::LottieWidget
            | WidgetKind::RiveWidget
            | WidgetKind::VideoPlayer
            | WidgetKind::ImageGallery
            | WidgetKind::AudioVisualizer
            | WidgetKind::CameraPreview
            | WidgetKind::BarcodeScanner => ControlRoutePreference::CustomRequired,
            // Data table widgets
            WidgetKind::GridTable => ControlRoutePreference::CustomRequired,
            // Input and navigation widgets added in BLUE16 phase E-2
            WidgetKind::NumberPicker
            | WidgetKind::OtpInput
            | WidgetKind::Pagination
            | WidgetKind::Banner => ControlRoutePreference::CustomRequired,
            // Controls split out of an overloaded kind, or added in BLUE16 phase E-6
            WidgetKind::ColorPicker
            | WidgetKind::Toast
            | WidgetKind::SplashScreen
            | WidgetKind::CandlestickChart
            | WidgetKind::VolumeChart
            | WidgetKind::DepthChart
            | WidgetKind::OrderBook
            | WidgetKind::QuoteBoard
            | WidgetKind::IndicatorChart => ControlRoutePreference::CustomRequired,
            // Controls promoted to their own kind in the 2.4.0 audit.
            WidgetKind::TreeTable
            | WidgetKind::Breadcrumb
            | WidgetKind::SignaturePad
            | WidgetKind::DropZone => ControlRoutePreference::CustomRequired,
        }
    }

    /// Every widget kind must be routed to the library.
    ///
    /// The completeness guarantee is the `match` above compiling at all; this
    /// test states the policy its arms encode and pins one value so the wiring
    /// cannot be removed silently.
    #[test]
    fn all_widget_kinds_are_routed() {
        assert_eq!(
            route_is_library_painted(WidgetKind::Window),
            ControlRoutePreference::CustomRequired,
            "the routing policy must stay library-painted for every kind"
        );
        assert_eq!(
            route_preference_for_widget_kind(WidgetKind::Window),
            ControlRoutePreference::CustomRequired,
        );
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
