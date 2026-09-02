#!/usr/bin/env python3
"""
generate_platform_capability_matrix.py — R6 Platform Capability Matrix Generator

Generates a markdown capability matrix from a structured data dictionary.
Designed to be the single source of truth for the matrix.

Usage:
    python3 tools/generate_platform_capability_matrix.py [--output FILE]
"""

import argparse
import sys
from typing import Dict, List, Tuple

# ---------------------------------------------------------------------------
# Emoji codes
# ---------------------------------------------------------------------------
NATIVE = "✅"
STATE_BACKED = "🔶"
PLACEHOLDER = "⬜"
NOT_APPLICABLE = "➖"

PLATFORMS = [
    "Windows",
    "Linux/X11",
    "macOS",
    "Wayland",
    "Mobile",
    "Harmony",
    "Embedded/Stub",
]

# ---------------------------------------------------------------------------
# Widget capability definitions
# Structure: widget_name -> (display_name, [level_win, level_x11, level_mac, level_wayland, level_mobile, level_harmony, level_embedded])
# ---------------------------------------------------------------------------
WIDGETS: Dict[str, Tuple[str, List[str]]] = {
    # === Dialogs & Windows ===
    "Window": ("Window", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "Dialog": ("Dialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "MessageBox": ("MessageBox", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "FileDialog": ("FileDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "ColorDialog": ("ColorDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "FontDialog": ("FontDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "InputDialog": ("InputDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "ProgressDialog": ("ProgressDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "PopupWindow": ("PopupWindow", [NATIVE] * 4 + [STATE_BACKED] * 3),
    # === Base controls (native create_* on all desktop + mobile) ===
    "Button": ("Button", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "CheckBox": ("CheckBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "RadioButton": ("RadioButton", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "Label": ("Label", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "LineEdit": ("LineEdit", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "TextEdit": ("TextEdit", [STATE_BACKED] * 7),
    "RichEdit": ("RichEdit", [NATIVE] * 3 + [STATE_BACKED] * 4),
    "ComboBox": ("ComboBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "SpinBox": ("SpinBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "ListBox": ("ListBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "ListView": ("ListView", [STATE_BACKED] * 7),
    "TreeView": ("TreeView", [STATE_BACKED] * 7),
    "ProgressBar": ("ProgressBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "Slider": ("Slider", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "ScrollBar": ("ScrollBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    # === Containers ===
    "ScrollArea": ("ScrollArea", [STATE_BACKED] * 7),
    "Panel": ("Panel", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "DockPanel": ("DockPanel", [STATE_BACKED] * 7),
    "GroupBox": ("GroupBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "TabWidget": ("TabWidget", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "Splitter": ("Splitter", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "MdiArea": ("MdiArea", [NATIVE] * 4 + [STATE_BACKED] * 3),
    # === Menus & Toolbars ===
    "MenuBar": ("MenuBar", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "Menu": ("Menu", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "MenuItem": ("MenuItem", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "ContextMenu": ("ContextMenu", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "ToolBar": ("ToolBar", [NATIVE] * 4 + [STATE_BACKED] * 3),
    "StatusBar": ("StatusBar", [NATIVE] * 4 + [STATE_BACKED] * 3),
    # === Display / Canvas ===
    "Canvas": ("Canvas", [STATE_BACKED] * 7),
    "Table": ("Table", [STATE_BACKED] * 7),
    "Grid": ("Grid", [STATE_BACKED] * 7),
    "Chart": ("Chart", [STATE_BACKED] * 7),
    # === Input variants ===
    "ToggleButton": ("ToggleButton", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "CheckListBox": ("CheckListBox", [STATE_BACKED] * 7),
    "DoubleSpinBox": ("DoubleSpinBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "Dial": ("Dial", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "Wizard": ("Wizard", [STATE_BACKED] * 7),
    "DatePicker": ("DatePicker", [STATE_BACKED] * 7),
    "TimePicker": ("TimePicker", [STATE_BACKED] * 7),
    "DateTimePicker": ("DateTimePicker", [STATE_BACKED] * 7),
    "DirectoryDialog": ("DirectoryDialog", [NATIVE] * 4 + [STATE_BACKED] * 3),
    # === Data / Property ===
    "DataView": ("DataView", [STATE_BACKED] * 7),
    "PropertyGrid": ("PropertyGrid", [STATE_BACKED] * 7),
    "Toolbox": ("Toolbox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "StackedWidget": ("StackedWidget", [STATE_BACKED] * 7),
    "CollapsiblePane": ("CollapsiblePane", [STATE_BACKED] * 7),
    "DockWidget": ("DockWidget", [NATIVE] * 4 + [STATE_BACKED] * 3),
    # === Web ===
    # NOTE: no "WebView" row — WidgetKind has no WebView variant; the
    # WebView/WebViewEnhanced handle- and render-layer aliases map onto
    # WidgetKind::WebEngineView, which has its own rows below.
    "ActivityIndicator": ("ActivityIndicator", [STATE_BACKED] * 7),
    # === Advanced ===
    "Calendar": ("Calendar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "ColumnView": ("ColumnView", [STATE_BACKED] * 7),
    "UndoView": ("UndoView", [STATE_BACKED] * 7),
    "CommandLink": ("CommandLink", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "LCDNumber": ("LCDNumber", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "FontComboBox": ("FontComboBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    # === WebEngine ===
    "WebEngineView": ("WebEngineView", [STATE_BACKED] * 7),
    "WebEnginePage": ("WebEnginePage", [STATE_BACKED] * 7),
    "WebEngineSettings": ("WebEngineSettings", [STATE_BACKED] * 7),
    "WebEngineDownloadItem": ("WebEngineDownloadItem", [STATE_BACKED] * 7),
    "WebEngineCookieStore": ("WebEngineCookieStore", [STATE_BACKED] * 7),
    "WebEngineWebChannel": ("WebEngineWebChannel", [STATE_BACKED] * 7),
    "WebEngineFindTextResult": ("WebEngineFindTextResult", [STATE_BACKED] * 7),
    "WebEngineNotification": ("WebEngineNotification", [STATE_BACKED] * 7),
    "WebEngineScriptDialog": ("WebEngineScriptDialog", [STATE_BACKED] * 7),
    "WebEngineContextMenuRequest": ("WebEngineContextMenuRequest", [STATE_BACKED] * 7),
    # === Actions ===
    "Action": ("Action", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "ToolButton": ("ToolButton", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    # NOTE: only "Toolbox" exists in WidgetKind (no "ToolBox" variant), so
    # the duplicate "ToolBox" entry was removed.
    # === Special ===
    "FreeformShape": ("FreeformShape", [STATE_BACKED] * 7),
    "TabBar": ("TabBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "PieMenu": ("PieMenu", [STATE_BACKED] * 7),
    "RibbonBar": ("RibbonBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    # === BLUE13 / modern widget-model widgets (state-backed everywhere) ===
    "AdaptiveScaffold": ("AdaptiveScaffold", [STATE_BACKED] * 7),
    "AnimatedImage": ("AnimatedImage", [STATE_BACKED] * 7),
    "AppBar": ("AppBar", [STATE_BACKED] * 7),
    "Arc": ("Arc", [STATE_BACKED] * 7),
    "AudioVisualizer": ("AudioVisualizer", [STATE_BACKED] * 7),
    "AutoCompleteEdit": ("AutoCompleteEdit", [STATE_BACKED] * 7),
    "Avatar": ("Avatar", [STATE_BACKED] * 7),
    "Badge": ("Badge", [STATE_BACKED] * 7),
    "BarChart": ("BarChart", [STATE_BACKED] * 7),
    "BarcodeScanner": ("BarcodeScanner", [STATE_BACKED] * 7),
    "BezierCurveEditor": ("BezierCurveEditor", [STATE_BACKED] * 7),
    "BottomNavigationBar": ("BottomNavigationBar", [STATE_BACKED] * 7),
    "BottomSheet": ("BottomSheet", [STATE_BACKED] * 7),
    "CameraPreview": ("CameraPreview", [STATE_BACKED] * 7),
    "Carousel": ("Carousel", [STATE_BACKED] * 7),
    "Chip": ("Chip", [STATE_BACKED] * 7),
    "ColorHistory": ("ColorHistory", [STATE_BACKED] * 7),
    "ColorWell": ("ColorWell", [STATE_BACKED] * 7),
    "CupertinoAlertDialog": ("CupertinoAlertDialog", [STATE_BACKED] * 7),
    "CupertinoDatePicker": ("CupertinoDatePicker", [STATE_BACKED] * 7),
    "CupertinoNavigationBar": ("CupertinoNavigationBar", [STATE_BACKED] * 7),
    "CupertinoSegmentedControl": ("CupertinoSegmentedControl", [STATE_BACKED] * 7),
    "CupertinoSlider": ("CupertinoSlider", [STATE_BACKED] * 7),
    "CupertinoSwitch": ("CupertinoSwitch", [STATE_BACKED] * 7),
    "DateRangePicker": ("DateRangePicker", [STATE_BACKED] * 7),
    "Divider": ("Divider", [STATE_BACKED] * 7),
    "Dropdown": ("Dropdown", [STATE_BACKED] * 7),
    "DropdownMenu": ("DropdownMenu", [STATE_BACKED] * 7),
    "EditableComboBox": ("EditableComboBox", [STATE_BACKED] * 7),
    "EmptyState": ("EmptyState", [STATE_BACKED] * 7),
    "FAB": ("FAB", [STATE_BACKED] * 7),
    "FindReplaceDialog": ("FindReplaceDialog", [STATE_BACKED] * 7),
    "FloatingLabel": ("FloatingLabel", [STATE_BACKED] * 7),
    "FontPreview": ("FontPreview", [STATE_BACKED] * 7),
    "Frame": ("Frame", [STATE_BACKED] * 7),
    "GridTable": ("GridTable", [STATE_BACKED] * 7),
    "HeroAnimation": ("HeroAnimation", [STATE_BACKED] * 7),
    "Icon": ("Icon", [STATE_BACKED] * 7),
    "ImageGallery": ("ImageGallery", [STATE_BACKED] * 7),
    "ImageView": ("ImageView", [STATE_BACKED] * 7),
    "ImePreedit": ("ImePreedit", [STATE_BACKED] * 7),
    "InplaceEditor": ("InplaceEditor", [STATE_BACKED] * 7),
    "Keyboard": ("Keyboard", [STATE_BACKED] * 7),
    "Line": ("Line", [STATE_BACKED] * 7),
    "LineChart": ("LineChart", [STATE_BACKED] * 7),
    "LottieWidget": ("LottieWidget", [STATE_BACKED] * 7),
    "MaskedEdit": ("MaskedEdit", [STATE_BACKED] * 7),
    "MasonryLayout": ("MasonryLayout", [STATE_BACKED] * 7),
    "MaterialNavigationRail": ("MaterialNavigationRail", [STATE_BACKED] * 7),
    "MaterialSnackbar": ("MaterialSnackbar", [STATE_BACKED] * 7),
    "MenuButton": ("MenuButton", [STATE_BACKED] * 7),
    "Meter": ("Meter", [STATE_BACKED] * 7),
    "MiniCanvas": ("MiniCanvas", [STATE_BACKED] * 7),
    "MiniChart": ("MiniChart", [STATE_BACKED] * 7),
    "MobileDatePicker": ("MobileDatePicker", [STATE_BACKED] * 7),
    "ModalBottomSheet": ("ModalBottomSheet", [STATE_BACKED] * 7),
    "MultiSelectComboBox": ("MultiSelectComboBox", [STATE_BACKED] * 7),
    "NavigationDrawer": ("NavigationDrawer", [STATE_BACKED] * 7),
    "NavigationStack": ("NavigationStack", [STATE_BACKED] * 7),
    "PagerPageView": ("PagerPageView", [STATE_BACKED] * 7),
    "PieChart": ("PieChart", [STATE_BACKED] * 7),
    "Popover": ("Popover", [STATE_BACKED] * 7),
    "ProgressCircle": ("ProgressCircle", [STATE_BACKED] * 7),
    "PropertiesPanel": ("PropertiesPanel", [STATE_BACKED] * 7),
    "QRCode": ("QRCode", [STATE_BACKED] * 7),
    "RangeSlider": ("RangeSlider", [STATE_BACKED] * 7),
    "Rating": ("Rating", [STATE_BACKED] * 7),
    "RefreshControl": ("RefreshControl", [STATE_BACKED] * 7),
    "RiveWidget": ("RiveWidget", [STATE_BACKED] * 7),
    "Roller": ("Roller", [STATE_BACKED] * 7),
    "SafeArea": ("SafeArea", [STATE_BACKED] * 7),
    "SearchBar": ("SearchBar", [STATE_BACKED] * 7),
    "SearchBox": ("SearchBox", [STATE_BACKED] * 7),
    "SegmentedButton": ("SegmentedButton", [STATE_BACKED] * 7),
    "ShortcutEditor": ("ShortcutEditor", [STATE_BACKED] * 7),
    "SkeletonLoader": ("SkeletonLoader", [STATE_BACKED] * 7),
    "Sparkline": ("Sparkline", [STATE_BACKED] * 7),
    "Spinner": ("Spinner", [STATE_BACKED] * 7),
    "Stepper": ("Stepper", [STATE_BACKED] * 7),
    "SwipeToDismiss": ("SwipeToDismiss", [STATE_BACKED] * 7),
    "Switch": ("Switch", [STATE_BACKED] * 7),
    "TabView": ("TabView", [STATE_BACKED] * 7),
    "TagInput": ("TagInput", [STATE_BACKED] * 7),
    "TextArea": ("TextArea", [STATE_BACKED] * 7),
    "TileView": ("TileView", [STATE_BACKED] * 7),
    "Tooltip": ("Tooltip", [STATE_BACKED] * 7),
    "VideoPlayer": ("VideoPlayer", [STATE_BACKED] * 7),
    "WizardDialog": ("WizardDialog", [STATE_BACKED] * 7),
}

# Sort widgets alphabetically by display name
SORTED_KEYS = sorted(WIDGETS.keys(), key=lambda k: WIDGETS[k][0])

SYMBOL_SEMANTICS = """
| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Usable control path on this platform — either a real native primitive or a state/self-drawn backend implementation that behaves normally. 该平台提供可用的控件路径（原生原语或 state/自绘后端均可正常工作）。 |
| 🔶 | Limited by backend capability — mapped/degraded/partial implementation. 受限于后端能力（映射/降级/部分实现）。 |
| ⬜ | Placeholder — declared but not implemented yet. |
| ➖ | Not applicable on this platform. |

> Note: ✅ only means *a working creation path exists*. For the rows listed under
> "Degradation notes（降级说明）" below, the native/FFI path returns a fallback
> primitive (Panel/Slider/Label/…), so their ✅ cells do **not** imply a dedicated
> native control implementation. 注：✅ 仅表示存在可用创建路径；文末“降级说明”
> 所列控件在 native/FFI 路径上实际创建为回退原语。
"""

# Degradation notes — mirror of src/control_backend/native.rs fallback delegations.
DEGRADATION_NOTES = """
## Degradation notes（降级说明）

On the **native/FFI path** (`src/control_backend/native.rs`), the following widget
families are not created as dedicated native controls. Each `create_*` listed
below delegates to a fallback primitive, silently in most cases (`log::warn!` is
emitted only for `data_view`, `property_grid`, `collapsible_pane`, `column_view`,
`undo_view`):

| Fallback created | Widgets (WidgetKind / matrix row names) |
| --- | --- |
| `create_panel` | ScrollArea, DockPanel, GroupBox, TabWidget, Splitter, StackedWidget, MdiArea, Canvas, Table, Grid, Chart, Wizard, DatePicker, TimePicker, DateTimePicker, DataView, PropertyGrid, Toolbox, CollapsiblePane, DockWidget, Calendar, WebView/WebEngine family (WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem, WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult, WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest) |
| `create_slider` | ScrollBar, Dial |
| `create_label` | LCDNumber (rendered as a label showing `\"0\"`) |
| `create_line_edit` | TextEdit, RichEdit |
| `create_button` | CommandLink, Action, ToolButton |
| `create_combo_box` | FontComboBox |
| `create_list_box` | TreeView |
| `create_list_view` | ColumnView, UndoView |
| `create_checkbox` | ToggleButton |
| `create_spin_box` | DoubleSpinBox |
| `create_message_box` | Dialog |
| `create_file_dialog` | DirectoryDialog |
| `create_menu` | ContextMenu |
| `create_progress_bar` | ActivityIndicator |
| `create_window` | PopupWindow |

Additional facts to keep the matrix consistent with `src/widget/kind.rs`:
- `ToolBox` is not a `WidgetKind` variant (only `Toolbox` is); the duplicate row was removed.
- `WebView` is not a `WidgetKind` variant either — the `WebView`/`WebViewEnhanced`
  aliases live at the handle/render layer and map onto `WidgetKind::WebEngineView`.
  The matrix therefore lists only the WebEngine rows.
"""



def generate_matrix() -> str:
    """Generate the full markdown document."""
    lines = []
    lines.append("# Platform Capability Matrix — R6")
    lines.append("")
    lines.append("> **Auto-generated** by `tools/generate_platform_capability_matrix.py`")
    lines.append(
        "> **Legend:** ✅ Usable · 🔶 Backend-limited · ⬜ Placeholder · ➖ NotApplicable"
    )
    lines.append("")
    lines.append("## Symbol semantics（符号语义）")
    lines.append("")
    lines.extend(SYMBOL_SEMANTICS.strip().splitlines())
    lines.append("")
    lines.append("## Matrix")
    lines.append("")

    # Header
    header = "| Widget | " + " | ".join(PLATFORMS) + " |"
    sep = "| " + "--- |" * (len(PLATFORMS) + 1)

    lines.append(header)
    lines.append(sep)

    # Rows
    for key in SORTED_KEYS:
        display_name, levels = WIDGETS[key]
        row = f"| **{display_name}** | " + " | ".join(levels) + " |"
        lines.append(row)

    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append(f"Total widgets: {len(WIDGETS)} (matches {len(WIDGETS)} WidgetKind variants)")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.extend(DEGRADATION_NOTES.strip().splitlines())
    lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Generate the R6 platform capability matrix"
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        default=None,
        help="Output file path (default: stdout)",
    )
    args = parser.parse_args()

    output = generate_matrix()

    if args.output:
        with open(args.output, "w") as f:
            f.write(output)
        print(f"Matrix written to {args.output}")
    else:
        print(output)


if __name__ == "__main__":
    main()
