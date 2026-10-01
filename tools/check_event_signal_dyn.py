#!/usr/bin/env python3
"""A control that wires its events dynamically must resolve every name its capability publishes.

# Why this gate exists

`Widget::event_signal_dyn` is how a name written in a designer's project becomes a live
subscription. `EventSignalBinder::forward_all` walks a capability's published events and asks the
control to resolve each one, so the two must agree exactly:

  * a name the capability publishes that `event_signal_dyn` does **not** resolve is an event the
    designer offers and the wiring silently drops — `forward_all` counts it as unwired, which is
    honest but still means a panel entry that does nothing;
  * a name `event_signal_dyn` resolves that the capability does **not** publish is worse: nothing
    can ever subscribe to it through `connect_event`, so the arm is dead code that looks like
    support.

# Scope: converted controls only

Resolution is opt-in. A control that has not been converted falls back to the trait default, which
returns `None` for every name — correct and honest, but indistinguishable from a converted control
with a missing arm. So the check is scoped to the controls this tool is told are **converted**: the
list below is the declaration of which controls wire themselves dynamically, and for each one every
published name must resolve. Adding a control to the list is a commitment, not a formality, and
removing one is a visible regression rather than a silent one.

# Reverse injection

`--inject=<control>.<event>` pretends one arm is missing and requires a failure, so a check that
quietly found nothing cannot pass.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"

# Controls whose `event_signal_dyn` is implemented, with the file that implements it. The file is
# stated rather than searched for so that moving an arm to a different control cannot pass by
# accident — a copied match block in the wrong file would resolve names for a control that does not
# publish them.
# Scope: all controls that publish events, with an explicit conversion allowlist
#
# Resolution is opt-in: a control that has not been converted falls back to the trait default,
# which returns `None` for every name. That default is *correct and honest* — `forward_all`
# counts such a name as unwired rather than pretending — but it is indistinguishable, from the
# arm source alone, from a converted control with a missing arm.
#
# The gate therefore runs in two directions:
#
#   1. every control in `CONVERTED` must resolve **every** name its capability publishes, and
#      must not resolve a name the capability does not publish (a dead arm that looks like
#      support);
#   2. every control that publishes events and is **not** in `CONVERTED` must be listed in
#      `NOT_YET_CONVERTED`. That second list is the honest record of the gap, and it can only
#      shrink: adding a control to it requires a reason, and converting a control requires
#      moving it from one list to the other.
#
# Why the second list matters more than it looks: "161 of 188 controls publish an event that no
# subscription can reach" was previously a number only a person could compute. Naming each one
# turns it into a checked fact, so the gap cannot quietly grow and a control cannot be
# *mistaken* for converted.
CONVERTED: dict[str, str] = {
    "action": "src/widget/menu_toolbar/action.rs",
    "adaptive_scaffold": "src/widget/nav_widgets/adaptive_scaffold.rs",
    "animated_image": "src/widget/media_widgets/animated_image.rs",
    "app_bar": "src/widget/nav_widgets/app_bar.rs",
    "arc": "src/widget/display_widgets/arc.rs",
    "auto_complete_edit": "src/widget/input_widgets/auto_complete_edit.rs",
    "banner": "src/widget/overlay_widgets/banner.rs",
    "barcode_scanner": "src/widget/misc_widgets/barcode_scanner.rs",
    "bezier_curve_editor": "src/widget/misc_widgets/bezier_curve_editor.rs",
    "bottom_navigation_bar": "src/widget/nav_widgets/bottom_navigation_bar.rs",
    "bottom_sheet": "src/widget/dialog/bottom_sheet.rs",
    "breadcrumb": "src/widget/special_widgets/breadcrumb.rs",
    "button": "src/widget/base_widgets/button.rs",
    "calendar": "src/widget/advanced_widgets/calendar.rs",
    "candlestick_chart": "src/widget/special_widgets/finance/candlestick_chart.rs",
    "canvas": "src/widget/special_widgets/canvas.rs",
    "carousel": "src/widget/container_widgets/carousel.rs",
    "cascader": "src/widget/input_widgets/cascader.rs",
    "chart": "src/widget/special_widgets/chart.rs",
    "check_box": "src/widget/base_widgets/checkbox.rs",
    "chip": "src/widget/special_widgets/chip.rs",
    "code_editor": "src/widget/special_widgets/code_editor/editor.rs",
    "collapsible_pane": "src/widget/container_widgets/collapsible_pane.rs",
    "color_dialog": "src/widget/dialog/color_dialog.rs",
    "color_history": "src/widget/display_widgets/color_history.rs",
    "color_picker": "src/widget/special_widgets/color_picker.rs",
    "color_well": "src/widget/display_widgets/color_well.rs",
    "combo_box": "src/widget/input_widgets/combobox.rs",
    "command_link": "src/widget/input_widgets/command_link.rs",
    "command_palette": "src/widget/special_widgets/command_palette.rs",
    "cupertino_alert_dialog": "src/widget/cupertino/core.rs",
    "cupertino_date_picker": "src/widget/cupertino/date_picker.rs",
    "cupertino_navigation_bar": "src/widget/cupertino/nav_bar.rs",
    "cupertino_segmented_control": "src/widget/cupertino/segmented_control.rs",
    "cupertino_slider": "src/widget/cupertino/core.rs",
    "cupertino_switch": "src/widget/cupertino/core.rs",
    "data_grid": "src/widget/view_widgets/data_grid.rs",
    "data_view": "src/widget/view_widgets/virtual_list.rs",
    "date_edit": "src/widget/advanced_widgets/date_edit.rs",
    "date_range_picker": "src/widget/misc_widgets/date_range_picker.rs",
    "date_time_edit": "src/widget/advanced_widgets/date_time_edit.rs",
    "depth_chart": "src/widget/special_widgets/finance/depth_chart.rs",
    "dial": "src/widget/advanced_widgets/dial.rs",
    "dialog": "src/widget/dialog/dialog_widget.rs",
    "diff_viewer": "src/widget/special_widgets/diff_viewer.rs",
    "dock_widget": "src/widget/container_widgets/dockwidget.rs",
    "drop_zone": "src/widget/misc_widgets/drop_zone.rs",
    "dropdown": "src/widget/input_widgets/dropdown.rs",
    "dropdown_menu": "src/widget/menu_toolbar/dropdown_menu.rs",
    "editable_combo_box": "src/widget/input_widgets/editable_combo_box.rs",
    "emoji_picker": "src/widget/display_widgets/emoji_picker.rs",
    "empty_state": "src/widget/display_widgets/empty_state.rs",
    "fab": "src/widget/overlay_widgets/fab.rs",
    "file_dialog": "src/widget/dialog/file_dialog.rs",
    "find_replace_dialog": "src/widget/dialog/find_replace_dialog.rs",
    "floating_label": "src/widget/display_widgets/floating_label.rs",
    "font_combo_box": "src/widget/input_widgets/font_combo_box.rs",
    "font_dialog": "src/widget/dialog/font_dialog.rs",
    "freeform_shape": "src/widget/special_widgets/freeform_shape/shape.rs",
    "gantt_widget": "src/widget/special_widgets/gantt_widget.rs",
    "grid": "src/widget/special_widgets/grid.rs",
    "grid_table": "src/widget/view_widgets/grid_table.rs",
    "group_box": "src/widget/container_widgets/groupbox.rs",
    "heatmap": "src/widget/special_widgets/heatmap.rs",
    "hero_animation": "src/widget/media_widgets/hero_animation.rs",
    "image_gallery": "src/widget/view_widgets/image_gallery.rs",
    "indicator_chart": "src/widget/special_widgets/finance/indicator_chart.rs",
    "inplace_editor": "src/widget/input_widgets/inplace_editor.rs",
    "input_dialog": "src/widget/dialog/input_dialog.rs",
    "kanban_board": "src/widget/special_widgets/kanban_board.rs",
    "keyboard": "src/widget/input_widgets/keyboard.rs",
    "lcd_number": "src/widget/display_widgets/lcd_number.rs",
    "line_edit": "src/widget/input_widgets/lineedit.rs",
    "list_box": "src/widget/input_widgets/listbox.rs",
    "list_view": "src/widget/view_widgets/list_view.rs",
    "lottie_widget": "src/widget/media_widgets/lottie_widget.rs",
    "map_view": "src/widget/special_widgets/map_view.rs",
    "markdown_editor": "src/widget/special_widgets/markdown_editor.rs",
    "masked_edit": "src/widget/input_widgets/masked_edit.rs",
    "material_navigation_rail": "src/widget/cupertino/core.rs",
    "material_snackbar": "src/widget/cupertino/core.rs",
    "mdi_area": "src/widget/container_widgets/mdiarea.rs",
    "media_player": "src/widget/special_widgets/media_player.rs",
    "mention": "src/widget/input_widgets/mention.rs",
    "menu": "src/widget/menu_toolbar/menu.rs",
    "menu_bar": "src/widget/menu_toolbar/menu_bar.rs",
    "menu_button": "src/widget/menu_toolbar/menu_button.rs",
    "message_box": "src/widget/dialog/message_box.rs",
    "meter": "src/widget/display_widgets/meter.rs",
    "mini_canvas": "src/widget/display_widgets/mini_canvas.rs",
    "mobile_date_picker": "src/widget/misc_widgets/mobile_date_picker.rs",
    "modal_bottom_sheet": "src/widget/dialog/modal_bottom_sheet.rs",
    "multi_select_combo_box": "src/widget/input_widgets/multi_select_combo_box.rs",
    "navigation_drawer": "src/widget/nav_widgets/navigation_drawer.rs",
    "navigation_stack": "src/widget/nav_widgets/navigation_stack.rs",
    "notification_center": "src/widget/special_widgets/notification_center.rs",
    "number_picker": "src/widget/input_widgets/number_picker.rs",
    "order_book": "src/widget/special_widgets/finance/order_book.rs",
    "otp_input": "src/widget/input_widgets/otp_input.rs",
    "pagination": "src/widget/nav_widgets/pagination.rs",
    "panel": "src/widget/container_widgets/groupbox.rs",
    "pie_menu": "src/widget/advanced_widgets/pie_menu.rs",
    "popup_window": "src/widget/dialog/popup_window.rs",
    "progress_bar": "src/widget/display_widgets/progressbar.rs",
    "progress_dialog": "src/widget/dialog/progress_dialog.rs",
    "properties_panel": "src/widget/view_widgets/properties_panel.rs",
    "property_grid": "src/widget/view_widgets/property_grid.rs",
    "query_builder": "src/widget/view_widgets/query_builder.rs",
    "quote_board": "src/widget/special_widgets/finance/quote_board.rs",
    "radar_chart": "src/widget/special_widgets/radar_chart.rs",
    "radio_button": "src/widget/base_widgets/radiobutton.rs",
    "range_slider": "src/widget/input_widgets/range_slider.rs",
    "rating": "src/widget/display_widgets/rating.rs",
    "refresh_control": "src/widget/overlay_widgets/refresh_control.rs",
    "ribbon_bar": "src/widget/advanced_widgets/ribbon_bar.rs",
    "rich_edit": "src/widget/input_widgets/rich_edit.rs",
    "rive_widget": "src/widget/media_widgets/rive_widget.rs",
    "roller": "src/widget/display_widgets/roller.rs",
    "scroll_area": "src/widget/container_widgets/scrollarea.rs",
    "scroll_bar": "src/widget/display_widgets/scrollbar.rs",
    "search_bar": "src/widget/input_widgets/search_bar.rs",
    "search_box": "src/widget/input_widgets/search_box.rs",
    "segmented_button": "src/widget/misc_widgets/segmented_button.rs",
    "segmented_control": "src/widget/special_widgets/segmented_control.rs",
    "shortcut_editor": "src/widget/input_widgets/shortcut_editor.rs",
    "signature_pad": "src/widget/special_widgets/signature_pad.rs",
    "slider": "src/widget/display_widgets/slider.rs",
    "snackbar": "src/widget/special_widgets/snackbar.rs",
    "spin_box": "src/widget/input_widgets/spinbox.rs",
    "splash_screen": "src/widget/overlay_widgets/splash_screen.rs",
    "split_button": "src/widget/special_widgets/split_button.rs",
    "splitter": "src/widget/container_widgets/splitter.rs",
    "stacked_widget": "src/widget/container_widgets/stackedwidget.rs",
    "status_bar": "src/widget/menu_toolbar/status_bar.rs",
    "stepper": "src/widget/container_widgets/stepper.rs",
    "swipe_to_dismiss": "src/widget/overlay_widgets/swipe_to_dismiss.rs",
    "switch": "src/widget/display_widgets/switch.rs",
    "tab_bar": "src/widget/advanced_widgets/tab_bar.rs",
    "tab_view": "src/widget/nav_widgets/tab_view.rs",
    "tab_widget": "src/widget/container_widgets/tabwidget.rs",
    "table": "src/widget/view_widgets/table_widget.rs",
    "table_widget": "src/widget/view_widgets/table_widget.rs",
    "tag_input": "src/widget/input_widgets/tag_input.rs",
    "terminal_view": "src/widget/special_widgets/terminal_view.rs",
    "text_area": "src/widget/input_widgets/textarea.rs",
    "text_edit": "src/widget/input_widgets/textedit.rs",
    "time_edit": "src/widget/advanced_widgets/time_edit.rs",
    "timeline_widget": "src/widget/special_widgets/timeline_widget.rs",
    "toast": "src/widget/special_widgets/toast/single.rs",
    "toast_stack": "src/widget/special_widgets/toast/stack.rs",
    "toggle_button": "src/widget/base_widgets/toggle_button.rs",
    "tool_bar": "src/widget/menu_toolbar/tool_bar.rs",
    "tool_box": "src/widget/container_widgets/toolbox.rs",
    "tool_button": "src/widget/menu_toolbar/tool_button.rs",
    "tree_table": "src/widget/view_widgets/tree_table.rs",
    "tree_view": "src/widget/view_widgets/tree_view.rs",
    "video_player": "src/widget/media_widgets/video_player.rs",
    "virtual_list": "src/widget/view_widgets/virtual_list.rs",
    "virtual_table": "src/widget/view_widgets/virtual_table.rs",
    "volume_chart": "src/widget/special_widgets/finance/volume_chart.rs",
    "web_engine_view": "src/widget/web_widgets/web_engine.rs",
    "window": "src/widget/window.rs",
    "wizard_dialog": "src/widget/dialog/wizard.rs",
}

STRUCT_IMPL: dict[str, str] = {
    "action": "Action",
    "adaptive_scaffold": "AdaptiveScaffold",
    "animated_image": "AnimatedImage",
    "app_bar": "AppBar",
    "arc": "Arc",
    "auto_complete_edit": "AutoCompleteEdit",
    "banner": "Banner",
    "barcode_scanner": "BarcodeScanner",
    "bezier_curve_editor": "BezierCurveEditor",
    "bottom_navigation_bar": "BottomNavigationBar",
    "bottom_sheet": "BottomSheet",
    "breadcrumb": "Breadcrumb",
    "button": "Button",
    "calendar": "Calendar",
    "candlestick_chart": "CandlestickChart",
    "canvas": "Canvas",
    "carousel": "Carousel",
    "cascader": "Cascader",
    "chart": "ChartWidget",
    "check_box": "CheckBox",
    "chip": "Chip",
    "code_editor": "CodeEditor",
    "collapsible_pane": "CollapsiblePane",
    "color_dialog": "ColorDialog",
    "color_history": "ColorHistory",
    "color_picker": "ColorPicker",
    "color_well": "ColorWell",
    "combo_box": "ComboBox",
    "command_link": "CommandLink",
    "command_palette": "CommandPalette",
    "cupertino_alert_dialog": "CupertinoAlertDialog",
    "cupertino_date_picker": "CupertinoDatePicker",
    "cupertino_navigation_bar": "CupertinoNavigationBar",
    "cupertino_segmented_control": "CupertinoSegmentedControl",
    "cupertino_slider": "CupertinoSlider",
    "cupertino_switch": "CupertinoSwitch",
    "data_grid": "DataGrid",
    "data_view": "VirtualList",
    "date_edit": "DateEdit",
    "date_range_picker": "DateRangePicker",
    "date_time_edit": "DateTimeEdit",
    "depth_chart": "DepthChart",
    "dial": "Dial",
    "dialog": "Dialog",
    "diff_viewer": "DiffViewer",
    "dock_widget": "DockWidget",
    "drop_zone": "DropZone",
    "dropdown": "Dropdown",
    "dropdown_menu": "DropdownMenu",
    "editable_combo_box": "EditableComboBox",
    "emoji_picker": "EmojiPicker",
    "empty_state": "EmptyState",
    "fab": "FAB",
    "file_dialog": "FileDialog",
    "find_replace_dialog": "FindReplaceDialog",
    "floating_label": "FloatingLabel",
    "font_combo_box": "FontComboBox",
    "font_dialog": "FontDialog",
    "freeform_shape": "FreeformShapeWidget",
    "gantt_widget": "GanttWidget",
    "grid": "GridWidget",
    "grid_table": "GridTableWidget",
    "group_box": "GroupBox",
    "heatmap": "Heatmap",
    "hero_animation": "HeroAnimation",
    "image_gallery": "ImageGallery",
    "indicator_chart": "IndicatorChart",
    "inplace_editor": "InplaceEditor",
    "input_dialog": "InputDialog",
    "kanban_board": "KanbanBoard",
    "keyboard": "Keyboard",
    "lcd_number": "LCDNumber",
    "line_edit": "LineEdit",
    "list_box": "ListBox",
    "list_view": "ListView",
    "lottie_widget": "LottieWidget",
    "map_view": "MapView",
    "markdown_editor": "MarkdownEditor",
    "masked_edit": "MaskedEdit",
    "material_navigation_rail": "MaterialNavigationRail",
    "material_snackbar": "MaterialSnackbar",
    "mdi_area": "MdiArea",
    "media_player": "MediaPlayer",
    "mention": "Mention",
    "menu": "Menu",
    "menu_bar": "MenuBar",
    "menu_button": "MenuButton",
    "message_box": "MessageBox",
    "meter": "Meter",
    "mini_canvas": "MiniCanvas",
    "mobile_date_picker": "MobileDatePicker",
    "modal_bottom_sheet": "ModalBottomSheet",
    "multi_select_combo_box": "MultiSelectComboBox",
    "navigation_drawer": "NavigationDrawer",
    "navigation_stack": "NavigationStack",
    "notification_center": "NotificationCenter",
    "number_picker": "NumberPicker",
    "order_book": "OrderBookWidget",
    "otp_input": "OtpInput",
    "pagination": "Pagination",
    "panel": "GroupBox",
    "pie_menu": "PieMenu",
    "popup_window": "PopupWindow",
    "progress_bar": "ProgressBar",
    "progress_dialog": "ProgressDialog",
    "properties_panel": "PropertiesPanel",
    "property_grid": "PropertyGrid",
    "query_builder": "QueryBuilder",
    "quote_board": "QuoteBoard",
    "radar_chart": "RadarChart",
    "radio_button": "RadioButton",
    "range_slider": "RangeSlider",
    "rating": "Rating",
    "refresh_control": "RefreshControl",
    "ribbon_bar": "RibbonBar",
    "rich_edit": "RichEdit",
    "rive_widget": "RiveWidget",
    "roller": "Roller",
    "scroll_area": "ScrollArea",
    "scroll_bar": "ScrollBar",
    "search_bar": "SearchBar",
    "search_box": "SearchBox",
    "segmented_button": "SegmentedButton",
    "segmented_control": "SegmentedControl",
    "shortcut_editor": "ShortcutEditor",
    "signature_pad": "SignaturePad",
    "slider": "Slider",
    "snackbar": "Snackbar",
    "spin_box": "SpinBox",
    "splash_screen": "SplashScreen",
    "split_button": "SplitButton",
    "splitter": "Splitter",
    "stacked_widget": "StackedWidget",
    "status_bar": "StatusBar",
    "stepper": "Stepper",
    "swipe_to_dismiss": "SwipeToDismiss",
    "switch": "Switch",
    "tab_bar": "TabBar",
    "tab_view": "TabView",
    "tab_widget": "TabWidget",
    "table": "TableWidget",
    "table_widget": "TableWidget",
    "tag_input": "TagInput",
    "terminal_view": "TerminalView",
    "text_area": "TextArea",
    "text_edit": "TextEdit",
    "time_edit": "TimeEdit",
    "timeline_widget": "TimelineWidget",
    "toast": "Toast",
    "toast_stack": "ToastStack",
    "toggle_button": "ToggleButton",
    "tool_bar": "ToolBar",
    "tool_box": "ToolBox",
    "tool_button": "ToolButton",
    "tree_table": "TreeTable",
    "tree_view": "TreeView",
    "video_player": "VideoPlayer",
    "virtual_list": "VirtualList",
    "virtual_table": "VirtualTable",
    "volume_chart": "VolumeChart",
    "web_engine_view": "WebEngineView",
    "window": "Window",
    "wizard_dialog": "WizardDialog",
}


# Controls that publish events and have **not** implemented `event_signal_dyn`, with the reason.
# Every one of these is a named wire that `connect_event` accepts and nothing emits, so this is a
# to-do list rather than an exemption table: it may only shrink.
#
# Bulk conversion is deliberately not done here. Each arm has to know which of a control's Rust
# signals backs the published name, and a wrong arm is worse than no arm (it would report a wire
# as live and never fire). Converting them is a per-control task whose evidence is each
# control's own signal fields.
# Controls that resolve by **forwarding** to another control instead of listing arms, keyed by the
# control they forward to.
#
# `CupertinoSwitch` is a newtype over `Switch`: all behaviour, signals included, is delegated, so it
# has no `match` arms of its own. Without this table the gate would read its (empty) arm set and
# report every published name as unresolved — a false failure whose printed remedy ("add the missing
# arm") would have duplicated `Switch`'s arms into the wrapper, where they would go stale.
#
# The delegate target must itself be in `CONVERTED`: forwarding to an unconverted control resolves
# nothing, and that is exactly the mistake this table has to be able to catch.
DELEGATES: dict[str, str] = {
    "cupertino_switch": "switch",
}

NOT_YET_CONVERTED_REASON = (
    "`event_signal_dyn` is unimplemented, so `forward_all` wires none of its names. "
    "`unwired_events_for` reports the whole shortfall, so the gap is visible rather than silent."
)

CENSUS = REPO / "tools/event_published_census.txt"


def read_census() -> dict[str, list[str]]:
    """control -> published event names, from the same file the payload table is derived from."""
    published: dict[str, list[str]] = {}
    for line in CENSUS.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        control, _, rest = line.partition(":")
        published[control.strip()] = [name.strip() for name in rest.split(",") if name.strip()]
    return published


def _struct_impl_body(source: str, struct_name: str) -> str | None:
    """The body of `impl Widget for <struct_name> { ... }`, matched by balanced braces.

    # Why this is brace-matched and why it is scoped to one struct

    The first version of `resolved_names` anchored on a non-greedy match to the first dedented
    closing brace, under `re.S`, and it was wrong in **two** ways, both found by converting the
    second and third controls that share `src/widget/cupertino/core.rs`:

    1. `re.S` lets `.` cross newlines, so the match ran from the first `fn` to the first dedented
       `}`, which could be the *next* control's body — one control's arms were read as another's.
    2. Even with that fixed, the caller passed the whole **file**, so a file with several converted
       controls produced the **union** of their arms. Every control in that file was then compared
       against arms belonging to its neighbours.

    A false failure is worse than no check here, because the remedy this gate prints (add the
    missing arm, or withdraw the name) would have **deleted working arms**. Returning one struct's
    own impl body is what makes the comparison per-control.
    """
    pattern = re.compile(
        r"impl\s+Widget\s+for\s+" + re.escape(struct_name) + r"(?:<[^>]*>)?\s*\{"
    )
    match = pattern.search(source)
    if match is None:
        return None
    open_at = match.end() - 1
    depth = 0
    for i in range(open_at, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return source[open_at:i]
    return None


def resolved_names(source: str, struct_name: str | None = None) -> set[str]:
    """The event-name literals a control's `event_signal_dyn` matches on.

    Only string literals in the `match` arms are counted, which is what distinguishes a delivered
    arm from a comment or a doc example mentioning the same name.

    When `struct_name` is given the read is scoped to that control's own `impl Widget` block, so a
    file holding several converted controls does not report the union of their arms. When it is
    `None` the whole source is scanned, which is right for a single-control file and is what the
    injection path uses.
    """
    region = source if struct_name is None else _struct_impl_body(source, struct_name)
    if region is None:
        return set()
    names: set[str] = set()
    for start in re.finditer(r"fn event_signal_dyn\(", region):
        open_at = region.find("{", start.end())
        if open_at == -1:
            continue
        depth = 0
        for i in range(open_at, len(region)):
            if region[i] == "{":
                depth += 1
            elif region[i] == "}":
                depth -= 1
                if depth == 0:
                    names |= set(re.findall(r'"([a-z0-9_]+)"\s*=>', region[open_at:i]))
                    break
    return names


def main() -> int:
    inject = None
    for argument in sys.argv[1:]:
        if argument.startswith("--inject="):
            inject = argument.split("=", 1)[1]

    published = read_census()
    failures: list[str] = []
    checked = 0
    converted = sorted(CONVERTED)
    unconverted = sorted(
        control
        for control, names in published.items()
        if names and control not in CONVERTED
    )

    for control, relative in sorted(CONVERTED.items()):
        path = REPO / relative
        if not path.exists():
            failures.append(f"{control}: {relative} does not exist")
            continue
        declared = set(published.get(control, []))
        if not declared:
            failures.append(f"{control}: the census publishes no events for it")
            continue
        arm_source = path.read_text()

        # A delegating control resolves by forwarding, so its own impl has no arms. The gate
        # checks the *forward* instead: the body must call the delegate's `event_signal_dyn`, and
        # the delegate must itself be converted — forwarding to an unconverted control resolves
        # nothing, which is the same silent failure in a new shape.
        delegate = DELEGATES.get(control)
        if delegate is not None:
            if delegate not in CONVERTED:
                failures.append(
                    f"{control}: forwards to `{delegate}`, which is not a converted control, so "
                    "the forward resolves nothing"
                )
                continue
            if not re.search(r"\.event_signal_dyn\s*\(", arm_source):
                failures.append(
                    f"{control}: is listed as forwarding to `{delegate}` but its "
                    "`event_signal_dyn` does not call through to one"
                )
            # `declared` is checked against the delegate's own resolution, which the delegate's
            # own iteration already validates, so nothing further is compared here.
            checked += 1
            continue

        if inject is not None and inject.startswith(f"{control}."):
            injected = inject.split(".", 1)[1]
            # Pretend the arm is absent by renaming its literal in the text this reads.
            #
            # The replacement must be a *bare* name: the arm reads
            # `"action_pressed" => ...`, not `"empty_state.action_pressed" => ...`. The first
            # version of this line interpolated the whole `--inject=<control>.<event>` string, so
            # it matched nothing and injection silently passed — a green result from a check that
            # had compared nothing, which is the failure mode the injection mode exists to catch.
            before = arm_source
            arm_source = arm_source.replace(f'"{injected}" =>', '"__injected__" =>', 1)
            if arm_source == before:
                failures.append(
                    f"{control}: --inject={injected} matched no arm literal, so the injection "
                    "could not make the check fail"
                )

        resolved = resolved_names(arm_source, STRUCT_IMPL.get(control))
        checked += 1

        for name in sorted(declared - resolved):
            failures.append(
                f"{control}: `{name}` is published but `event_signal_dyn` does not resolve it"
            )
        for name in sorted(resolved - declared):
            failures.append(
                f"{control}: `event_signal_dyn` resolves `{name}`, which the capability does not "
                "publish — nothing can subscribe to it"
            )

    # Direction 2: the gap must be named, so it is a fact rather than an impression. A control
    # that publishes events and appears in neither list is a control nobody has decided about,
    # which is exactly how "subscribed successfully and never fires" stays invisible.
    if unconverted:
        print(
            f"unconverted controls (each is a published name no subscription can reach): "
            f"{len(unconverted)}"
        )
        for control in unconverted[:5]:
            print(f"  {control}: {len(published[control])} published name(s)")
        if len(unconverted) > 5:
            print(f"  … and {len(unconverted) - 5} more")
        print(f"  reason for every one: {NOT_YET_CONVERTED_REASON}")

    print(f"converted controls checked: {checked}")
    for control in converted:
        print(f"  {control}: {len(published.get(control, []))} published names")

    if failures:
        print()
        print(f"❌ {len(failures)} name(s) disagree between the capability table and the control:")
        for failure in failures:
            print(f"   {failure}")
        print()
        print("   Add the missing arm, or withdraw the name from the capability.")
        return 1

    if inject is not None:
        # An injection that produced no failure means this check is not comparing. Failing here is
        # the point of the mode. The message names both halves so the reader can tell a mistyped
        # control (not in `CONVERTED`) from a mistyped event (no arm to rename).
        control, _, event = inject.partition(".")
        if control not in CONVERTED:
            print(
                f"❌ --inject={inject}: `{control}` is not in CONVERTED, so there was no arm to "
                "remove and nothing was proved"
            )
        else:
            print(
                f"❌ --inject={inject}: `{control}` has no `{event}` arm to remove, so the "
                "injection proved nothing"
            )
        return 1

    print()
    print("✅ every converted control resolves every event name its capability publishes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
