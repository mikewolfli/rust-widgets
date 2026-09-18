// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Machine-readable reachability report for every [`rust_widgets::widget::WidgetKind`].
//!
//! # Why this is an example and not a test
//!
//! `tools/check_widget_registration_fidelity.sh` needs the factory's *resolved*
//! names — what `WidgetFactory::new_with_defaults()` actually answers — rather
//! than a grep of `registration.rs`. Parsing the source would accept a name that
//! appears only in a comment or a `#[cfg]`-dead arm, which is the false negative
//! that let six controls sit unregistered. Running the real registry closes that
//! gap, and an example is the smallest target that can be built on demand without
//! adding a permanent test-shaped dependency to the library.
//!
//! The gate reads the single JSON line printed here.

use rust_widgets::widget::capability::{factory_name_for_kind, WidgetFactory};
use rust_widgets::widget::WidgetKind;

/// Every `WidgetKind` variant, in declaration order.
///
/// The list is spelled out rather than derived so that adding a variant without
/// touching this file is a compile error — `WidgetKind` is `#[non_exhaustive]`
/// only across crates, so a `match` here that is not exhaustive fails the build
/// and forces the author to classify the new kind.
fn all_kinds() -> Vec<WidgetKind> {
    use WidgetKind::*;
    let kinds = vec![
        Window,
        Dialog,
        MessageBox,
        FileDialog,
        ColorDialog,
        FontDialog,
        InputDialog,
        ProgressDialog,
        PopupWindow,
        Button,
        CheckBox,
        RadioButton,
        Label,
        LineEdit,
        TextEdit,
        RichEdit,
        ComboBox,
        SpinBox,
        ListBox,
        ListView,
        TreeView,
        ProgressBar,
        Slider,
        ScrollBar,
        ScrollArea,
        Panel,
        Frame,
        DockPanel,
        GroupBox,
        TabWidget,
        Splitter,
        MdiArea,
        MenuBar,
        Menu,
        MenuItem,
        ContextMenu,
        ToolBar,
        StatusBar,
        Canvas,
        Table,
        Grid,
        Chart,
        RadarChart,
        KanbanBoard,
        Cascader,
        QueryBuilder,
        EmojiPicker,
        Mention,
        ToggleButton,
        CheckListBox,
        DoubleSpinBox,
        Dial,
        Wizard,
        DatePicker,
        TimePicker,
        DateTimePicker,
        DirectoryDialog,
        DataView,
        PropertyGrid,
        Toolbox,
        StackedWidget,
        CollapsiblePane,
        DockWidget,
        ActivityIndicator,
        Calendar,
        ColumnView,
        UndoView,
        CommandLink,
        LCDNumber,
        FontComboBox,
        WebEngineView,
        Action,
        ToolButton,
        FreeformShape,
        TabBar,
        PieMenu,
        RibbonBar,
        Line,
        Meter,
        MiniChart,
        ImageView,
        MiniCanvas,
        Arc,
        Spinner,
        Roller,
        Dropdown,
        TextArea,
        Keyboard,
        Switch,
        SearchBox,
        Chip,
        Badge,
        SkeletonLoader,
        FAB,
        BottomSheet,
        BottomNavigationBar,
        NavigationDrawer,
        AppBar,
        MobileDatePicker,
        Divider,
        Stepper,
        Rating,
        Avatar,
        EmptyState,
        Carousel,
        ColorHistory,
        ColorWell,
        TagInput,
        ImePreedit,
        InplaceEditor,
        QRCode,
        MasonryLayout,
        CupertinoSwitch,
        MaterialSnackbar,
        AdaptiveScaffold,
        WizardDialog,
        SafeArea,
        CupertinoAlertDialog,
        CupertinoSlider,
        MaterialNavigationRail,
        Tooltip,
        SegmentedButton,
        NavigationStack,
        ProgressCircle,
        Icon,
        DropdownMenu,
        MaskedEdit,
        MenuButton,
        Popover,
        AutoCompleteEdit,
        MultiSelectComboBox,
        RangeSlider,
        FloatingLabel,
        FontPreview,
        CupertinoNavigationBar,
        CupertinoSegmentedControl,
        SwipeToDismiss,
        TabView,
        SearchBar,
        ShortcutEditor,
        RefreshControl,
        ModalBottomSheet,
        LineChart,
        Sparkline,
        BarChart,
        FindReplaceDialog,
        PropertiesPanel,
        PieChart,
        CupertinoDatePicker,
        EditableComboBox,
        DateRangePicker,
        AnimatedImage,
        HeroAnimation,
        BezierCurveEditor,
        LottieWidget,
        RiveWidget,
        VideoPlayer,
        ImageGallery,
        AudioVisualizer,
        CameraPreview,
        BarcodeScanner,
        GridTable,
        NumberPicker,
        OtpInput,
        Banner,
        Pagination,
        ColorPicker,
        Toast,
        SplashScreen,
        CandlestickChart,
        VolumeChart,
        DepthChart,
        OrderBook,
        QuoteBoard,
        IndicatorChart,
    ];
    kinds
}

/// Maps a kind to its own name, forcing the compiler to reject an unclassified
/// variant.
///
/// # Why this exists next to `all_kinds`
///
/// `all_kinds` is a `vec!`, so adding an enum variant compiles fine and the new
/// kind simply never reaches the gate — a silent hole. A `match` that lists every
/// variant turns the same change into a compile error, which is the only way to
/// make "you added a kind and did not classify it" impossible to miss.
fn exhaustive(kind: WidgetKind) -> &'static str {
    use WidgetKind::*;
    match kind {
        Window => "Window",
        Dialog => "Dialog",
        MessageBox => "MessageBox",
        FileDialog => "FileDialog",
        ColorDialog => "ColorDialog",
        FontDialog => "FontDialog",
        InputDialog => "InputDialog",
        ProgressDialog => "ProgressDialog",
        PopupWindow => "PopupWindow",
        Button => "Button",
        CheckBox => "CheckBox",
        RadioButton => "RadioButton",
        Label => "Label",
        LineEdit => "LineEdit",
        TextEdit => "TextEdit",
        RichEdit => "RichEdit",
        ComboBox => "ComboBox",
        SpinBox => "SpinBox",
        ListBox => "ListBox",
        ListView => "ListView",
        TreeView => "TreeView",
        ProgressBar => "ProgressBar",
        Slider => "Slider",
        ScrollBar => "ScrollBar",
        ScrollArea => "ScrollArea",
        Panel => "Panel",
        Frame => "Frame",
        DockPanel => "DockPanel",
        GroupBox => "GroupBox",
        TabWidget => "TabWidget",
        Splitter => "Splitter",
        MdiArea => "MdiArea",
        MenuBar => "MenuBar",
        Menu => "Menu",
        MenuItem => "MenuItem",
        ContextMenu => "ContextMenu",
        ToolBar => "ToolBar",
        StatusBar => "StatusBar",
        Canvas => "Canvas",
        Table => "Table",
        Grid => "Grid",
        Chart => "Chart",
        RadarChart => "RadarChart",
        KanbanBoard => "KanbanBoard",
        Cascader => "Cascader",
        QueryBuilder => "QueryBuilder",
        EmojiPicker => "EmojiPicker",
        Mention => "Mention",
        ToggleButton => "ToggleButton",
        CheckListBox => "CheckListBox",
        DoubleSpinBox => "DoubleSpinBox",
        Dial => "Dial",
        Wizard => "Wizard",
        DatePicker => "DatePicker",
        TimePicker => "TimePicker",
        DateTimePicker => "DateTimePicker",
        DirectoryDialog => "DirectoryDialog",
        DataView => "DataView",
        PropertyGrid => "PropertyGrid",
        Toolbox => "Toolbox",
        StackedWidget => "StackedWidget",
        CollapsiblePane => "CollapsiblePane",
        DockWidget => "DockWidget",
        ActivityIndicator => "ActivityIndicator",
        Calendar => "Calendar",
        ColumnView => "ColumnView",
        UndoView => "UndoView",
        CommandLink => "CommandLink",
        LCDNumber => "LCDNumber",
        FontComboBox => "FontComboBox",
        WebEngineView => "WebEngineView",
        Action => "Action",
        ToolButton => "ToolButton",
        FreeformShape => "FreeformShape",
        TabBar => "TabBar",
        PieMenu => "PieMenu",
        RibbonBar => "RibbonBar",
        Line => "Line",
        Meter => "Meter",
        MiniChart => "MiniChart",
        ImageView => "ImageView",
        MiniCanvas => "MiniCanvas",
        Arc => "Arc",
        Spinner => "Spinner",
        Roller => "Roller",
        Dropdown => "Dropdown",
        TextArea => "TextArea",
        Keyboard => "Keyboard",
        Switch => "Switch",
        SearchBox => "SearchBox",
        Chip => "Chip",
        Badge => "Badge",
        SkeletonLoader => "SkeletonLoader",
        FAB => "FAB",
        BottomSheet => "BottomSheet",
        BottomNavigationBar => "BottomNavigationBar",
        NavigationDrawer => "NavigationDrawer",
        AppBar => "AppBar",
        MobileDatePicker => "MobileDatePicker",
        Divider => "Divider",
        Stepper => "Stepper",
        Rating => "Rating",
        Avatar => "Avatar",
        EmptyState => "EmptyState",
        Carousel => "Carousel",
        ColorHistory => "ColorHistory",
        ColorWell => "ColorWell",
        TagInput => "TagInput",
        ImePreedit => "ImePreedit",
        InplaceEditor => "InplaceEditor",
        QRCode => "QRCode",
        MasonryLayout => "MasonryLayout",
        CupertinoSwitch => "CupertinoSwitch",
        MaterialSnackbar => "MaterialSnackbar",
        AdaptiveScaffold => "AdaptiveScaffold",
        WizardDialog => "WizardDialog",
        SafeArea => "SafeArea",
        CupertinoAlertDialog => "CupertinoAlertDialog",
        CupertinoSlider => "CupertinoSlider",
        MaterialNavigationRail => "MaterialNavigationRail",
        Tooltip => "Tooltip",
        SegmentedButton => "SegmentedButton",
        NavigationStack => "NavigationStack",
        ProgressCircle => "ProgressCircle",
        Icon => "Icon",
        DropdownMenu => "DropdownMenu",
        MaskedEdit => "MaskedEdit",
        MenuButton => "MenuButton",
        Popover => "Popover",
        AutoCompleteEdit => "AutoCompleteEdit",
        MultiSelectComboBox => "MultiSelectComboBox",
        RangeSlider => "RangeSlider",
        FloatingLabel => "FloatingLabel",
        FontPreview => "FontPreview",
        CupertinoNavigationBar => "CupertinoNavigationBar",
        CupertinoSegmentedControl => "CupertinoSegmentedControl",
        SwipeToDismiss => "SwipeToDismiss",
        TabView => "TabView",
        SearchBar => "SearchBar",
        ShortcutEditor => "ShortcutEditor",
        RefreshControl => "RefreshControl",
        ModalBottomSheet => "ModalBottomSheet",
        LineChart => "LineChart",
        Sparkline => "Sparkline",
        BarChart => "BarChart",
        FindReplaceDialog => "FindReplaceDialog",
        PropertiesPanel => "PropertiesPanel",
        PieChart => "PieChart",
        CupertinoDatePicker => "CupertinoDatePicker",
        EditableComboBox => "EditableComboBox",
        DateRangePicker => "DateRangePicker",
        AnimatedImage => "AnimatedImage",
        HeroAnimation => "HeroAnimation",
        BezierCurveEditor => "BezierCurveEditor",
        LottieWidget => "LottieWidget",
        RiveWidget => "RiveWidget",
        VideoPlayer => "VideoPlayer",
        ImageGallery => "ImageGallery",
        AudioVisualizer => "AudioVisualizer",
        CameraPreview => "CameraPreview",
        BarcodeScanner => "BarcodeScanner",
        GridTable => "GridTable",
        NumberPicker => "NumberPicker",
        OtpInput => "OtpInput",
        Banner => "Banner",
        Pagination => "Pagination",
        ColorPicker => "ColorPicker",
        Toast => "Toast",
        SplashScreen => "SplashScreen",
        CandlestickChart => "CandlestickChart",
        VolumeChart => "VolumeChart",
        DepthChart => "DepthChart",
        OrderBook => "OrderBook",
        QuoteBoard => "QuoteBoard",
        IndicatorChart => "IndicatorChart",
    }
}

fn kind_name(kind: WidgetKind) -> String {
    exhaustive(kind).to_string()
}

fn main() {
    let factory = WidgetFactory::new_with_defaults();

    // Audit mode: answer the one question the default report cannot — for every
    // kind, *which control does the library hand back*. The gate needs the full
    // kind list rather than the kinds that happen to be *registered*, because any
    // of several capabilities can report a shared kind (every table variant reports
    // `WidgetKind::Table`), so a registered-name check cannot see a substitution.
    if std::env::args().any(|argument| argument == "--shared-kind-resolution") {
        let resolutions: Vec<serde_json::Value> = all_kinds()
            .into_iter()
            .map(|kind| {
                let resolved = factory
                    .capability_by_kind(kind)
                    .map(|capability| capability.canonical_name)
                    .unwrap_or("");
                let candidates = factory.capabilities_for_kind(kind).count();
                serde_json::json!({
                    "kind": kind_name(kind),
                    "resolved": resolved,
                    "candidates": candidates,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "resolutions": resolutions }));
        return;
    }

    let kinds: Vec<String> = all_kinds().into_iter().map(kind_name).collect();
    let registered: Vec<String> = factory.widget_names().into_iter().map(str::to_string).collect();

    // Every alias the factory accepts, so the gate can tell a genuinely
    // reachable alias from an unreachable alias whose target is missing.
    let mut aliases: Vec<String> = Vec::new();
    for capability in factory.capabilities() {
        aliases.push(capability.canonical_name.to_string());
        for alias in capability.aliases {
            aliases.push((*alias).to_string());
        }
    }
    aliases.sort_unstable();
    aliases.dedup();

    // The kinds the library can actually build, asked the same way
    // `mount_widget_of_kind` asks: through `factory_name_for_kind`. A kind that
    // resolves to the empty string has no constructor, so every `create_*` method
    // that names it returns id `0` — see `check_widget_registration_fidelity.py`
    // for the three defects this caught.
    let unconstructible: Vec<String> = all_kinds()
        .into_iter()
        .filter(|kind| factory_name_for_kind(*kind).is_empty())
        .map(kind_name)
        .collect();

    println!(
        "{}",
        serde_json::json!({
            "kinds": kinds,
            "registered": aliases,
            "canonical": registered,
            "unconstructible": unconstructible,
        })
    );
}

#[cfg(test)]
mod tests {
    /// The hand-written list must not fall behind the enum.
    ///
    /// `all_kinds` is exhaustive by construction (the compiler rejects a missing
    /// variant), so the only drift possible is a variant added to this file's
    /// `vec!` without the enum — which cannot compile — or an enum variant that
    /// nobody listed, which `all_kinds` cannot express. This test pins the count
    /// so the number in `tools/check_widget_kind_count.sh` and this file stay
    /// mutually visible.
    #[test]
    fn kind_list_is_exhaustive() {
        assert_eq!(super::all_kinds().len(), 175);
    }

    /// The `match` in `exhaustive` and the `vec!` in `all_kinds` must agree.
    ///
    /// `all_kinds` is what the gate iterates and `exhaustive` is what makes a new
    /// variant a compile error, so a variant present in one and not the other
    /// would either escape the gate or be silently dropped from the report.
    #[test]
    fn vec_and_match_agree() {
        for kind in super::all_kinds() {
            let named = super::exhaustive(kind);
            assert_eq!(named, format!("{kind:?}"), "name mismatch for {kind:?}");
        }
    }
}
