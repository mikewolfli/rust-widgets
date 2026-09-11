#!/usr/bin/env python3
"""
generate_platform_capability_matrix.py — R6 Platform Capability Matrix Generator

Generates a markdown capability matrix from a structured data dictionary.
Designed to be the single source of truth for the matrix.

Usage:
    python3 tools/generate_platform_capability_matrix.py [--output FILE]
"""

import argparse
import os
import re
import sys
from typing import Dict, List, Tuple

# Repository root, derived from this script's location (tools/).
REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# The native/FFI control backend whose `create_*` delegations are parsed to build
# the "Degradation notes" fallback table.
NATIVE_RS = os.path.join(REPO_ROOT, "src", "control_backend", "native.rs")

# ---------------------------------------------------------------------------
# Capability symbols
#
# These three states must not be conflated: a widget that is fully implemented by
# the custom/self-drawn backend is NOT the same as one that degrades to a
# different primitive on the native path.
#
#   NATIVE       — the backend creates a real platform primitive for this widget
#                  (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`).
#   CUSTOM       — no platform primitive exists, but the custom/self-drawn backend
#                  has a dedicated `create_*` implementation. The widget is fully
#                  functional; it is simply drawn by this library rather than by
#                  the OS. This is the project's "原生优先，自绘兜底" policy.
#   STATE_BACKED — the native path answers with a *different* primitive (a silent
#                  downgrade, e.g. `create_chart` -> `create_panel`), so the widget
#                  loses its identity. This is the genuinely limited case.
#
# The legend in the generated document is derived from these names.
# ---------------------------------------------------------------------------
NATIVE = "✅"
CUSTOM = "🟦"
STATE_BACKED = "🔶"
PLACEHOLDER = "⬜"
NOT_APPLICABLE = "➖"

# Human-readable meaning of each symbol, used to render the legend.
SYMBOL_MEANING = {
    NATIVE: "Native implementation — the backend creates a real platform primitive for this widget. "
    "该平台创建真实原生原语。",
    CUSTOM: "Self-drawn implementation (fully functional) — no platform primitive exists, so this "
    "library's custom backend draws it; the widget behaves normally. "
    "自绘实现（功能完整）：平台无此原语，由本库自绘后端实现，行为正常。",
    STATE_BACKED: "Limited — the native path degrades to a *different* primitive and the widget "
    "loses its identity. 受限：原生路径降级为其它原语，丢失自身身份。",
    PLACEHOLDER: "Placeholder — declared but not implemented yet. 已声明但尚未实现。",
    NOT_APPLICABLE: "Not applicable on this platform. 该平台不适用。",
}

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
    "Window": ("Window", [NATIVE] * 4 + [CUSTOM] * 3),
    "Dialog": ("Dialog", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "MessageBox": ("MessageBox", [CUSTOM, CUSTOM, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "FileDialog": ("FileDialog", [CUSTOM, CUSTOM, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "ColorDialog": ("ColorDialog", [CUSTOM, CUSTOM, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "FontDialog": ("FontDialog", [CUSTOM, CUSTOM, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "InputDialog": ("InputDialog", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "ProgressDialog": ("ProgressDialog", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "PopupWindow": ("PopupWindow", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    # === Base controls (native create_* on all desktop + mobile) ===
    "Button": ("Button", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "CheckBox": ("CheckBox", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "RadioButton": ("RadioButton", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "Label": ("Label", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "LineEdit": ("LineEdit", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "TextEdit": ("TextEdit", [CUSTOM] * 7),
    "RichEdit": ("RichEdit", [NATIVE] * 3 + [CUSTOM] * 4),
    "ComboBox": ("ComboBox", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "SpinBox": ("SpinBox", [NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "ListBox": ("ListBox", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "ListView": ("ListView", [NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "TreeView": ("TreeView", [CUSTOM] * 7),
    "ProgressBar": ("ProgressBar", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "Slider": ("Slider", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "ScrollBar": ("ScrollBar", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    # === Containers ===
    "ScrollArea": ("ScrollArea", [NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "Panel": ("Panel", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "DockPanel": ("DockPanel", [CUSTOM] * 7),
    # GroupBox/TabWidget/Splitter: real native objects exist only where the
    # backend constructs one — Linux under `gtk-native` (gtk::Frame /
    # gtk::Notebook / gtk::Paned) and Windows (BS_GROUPBOX button frame /
    # SysTabControl32 / client-edge host). macOS, Wayland, Harmony, iOS and
    # Android only record a distinguished state handle, which is not a native
    # object, so those cells are 🟦 (self-drawn/functional) rather than ✅.
    "GroupBox": ("GroupBox", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "TabWidget": ("TabWidget", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "Splitter": ("Splitter", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "MdiArea": ("MdiArea", [NATIVE] * 4 + [CUSTOM] * 3),
    # === Menus & Toolbars ===
    "MenuBar": ("MenuBar", [NATIVE] * 4 + [CUSTOM] * 3),
    "Menu": ("Menu", [NATIVE] * 4 + [CUSTOM] * 3),
    "MenuItem": ("MenuItem", [NATIVE] * 4 + [CUSTOM] * 3),
    "ContextMenu": ("ContextMenu", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "ToolBar": ("ToolBar", [NATIVE] * 4 + [CUSTOM] * 3),
    "StatusBar": ("StatusBar", [NATIVE] * 4 + [CUSTOM] * 3),
    # === Display / Canvas ===
    "Canvas": ("Canvas", [CUSTOM] * 7),
    "Table": ("Table", [CUSTOM] * 7),
    "Grid": ("Grid", [CUSTOM] * 7),
    "Chart": ("Chart", [CUSTOM] * 7),
    # === Input variants ===
    "ToggleButton": ("ToggleButton", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "CheckListBox": ("CheckListBox", [CUSTOM] * 7),
    "DoubleSpinBox": ("DoubleSpinBox", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "Dial": ("Dial", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "Wizard": ("Wizard", [CUSTOM] * 7),
    "DatePicker": ("DatePicker", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "TimePicker": ("TimePicker", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "DateTimePicker": ("DateTimePicker", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "DirectoryDialog": ("DirectoryDialog", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    # === Data / Property ===
    "DataView": ("DataView", [CUSTOM] * 7),
    "PropertyGrid": ("PropertyGrid", [CUSTOM] * 7),
    "Toolbox": ("Toolbox", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "StackedWidget": ("StackedWidget", [CUSTOM] * 7),
    "CollapsiblePane": ("CollapsiblePane", [CUSTOM] * 7),
    "DockWidget": ("DockWidget", [NATIVE] * 4 + [CUSTOM] * 3),
    # === Web ===
    # NOTE: no "WebView" row — WidgetKind has no WebView variant; the
    # WebView/WebViewEnhanced handle- and render-layer aliases map onto
    # WidgetKind::WebEngineView, which has its own rows below.
    "ActivityIndicator": ("ActivityIndicator", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    # === Advanced ===
    "Calendar": ("Calendar", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    "ColumnView": ("ColumnView", [CUSTOM] * 7),
    "UndoView": ("UndoView", [CUSTOM] * 7),
    "CommandLink": ("CommandLink", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "LCDNumber": ("LCDNumber", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "FontComboBox": ("FontComboBox", [NATIVE, NATIVE, CUSTOM, CUSTOM, CUSTOM, CUSTOM, CUSTOM]),
    # === WebEngine ===
    "WebEngineView": ("WebEngineView", [CUSTOM] * 7),
    "WebEnginePage": ("WebEnginePage", [CUSTOM] * 7),
    "WebEngineSettings": ("WebEngineSettings", [CUSTOM] * 7),
    "WebEngineDownloadItem": ("WebEngineDownloadItem", [CUSTOM] * 7),
    "WebEngineCookieStore": ("WebEngineCookieStore", [CUSTOM] * 7),
    "WebEngineWebChannel": ("WebEngineWebChannel", [CUSTOM] * 7),
    "WebEngineFindTextResult": ("WebEngineFindTextResult", [CUSTOM] * 7),
    "WebEngineNotification": ("WebEngineNotification", [CUSTOM] * 7),
    "WebEngineScriptDialog": ("WebEngineScriptDialog", [CUSTOM] * 7),
    "WebEngineContextMenuRequest": ("WebEngineContextMenuRequest", [CUSTOM] * 7),
    # === Actions ===
    "Action": ("Action", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "ToolButton": ("ToolButton", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    # NOTE: only "Toolbox" exists in WidgetKind (no "ToolBox" variant), so
    # the duplicate "ToolBox" entry was removed.
    # === Special ===
    "FreeformShape": ("FreeformShape", [CUSTOM] * 7),
    "TabBar": ("TabBar", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    "PieMenu": ("PieMenu", [CUSTOM] * 7),
    "RibbonBar": ("RibbonBar", [NATIVE] * 4 + [NATIVE] * 2 + [CUSTOM]),
    # === BLUE13 / modern widget-model widgets (state-backed everywhere) ===
    "AdaptiveScaffold": ("AdaptiveScaffold", [CUSTOM] * 7),
    "AnimatedImage": ("AnimatedImage", [CUSTOM] * 7),
    "AppBar": ("AppBar", [CUSTOM] * 7),
    "Arc": ("Arc", [CUSTOM] * 7),
    "AudioVisualizer": ("AudioVisualizer", [CUSTOM] * 7),
    "AutoCompleteEdit": ("AutoCompleteEdit", [CUSTOM] * 7),
    "Avatar": ("Avatar", [CUSTOM] * 7),
    "Badge": ("Badge", [CUSTOM] * 7),
    "BarChart": ("BarChart", [CUSTOM] * 7),
    "BarcodeScanner": ("BarcodeScanner", [CUSTOM] * 7),
    "BezierCurveEditor": ("BezierCurveEditor", [CUSTOM] * 7),
    "BottomNavigationBar": ("BottomNavigationBar", [CUSTOM] * 7),
    "BottomSheet": ("BottomSheet", [CUSTOM] * 7),
    "CameraPreview": ("CameraPreview", [CUSTOM] * 7),
    "Carousel": ("Carousel", [CUSTOM] * 7),
    "Chip": ("Chip", [CUSTOM] * 7),
    "ColorHistory": ("ColorHistory", [CUSTOM] * 7),
    "ColorWell": ("ColorWell", [CUSTOM] * 7),
    "CupertinoAlertDialog": ("CupertinoAlertDialog", [CUSTOM] * 7),
    "CupertinoDatePicker": ("CupertinoDatePicker", [CUSTOM] * 7),
    "CupertinoNavigationBar": ("CupertinoNavigationBar", [CUSTOM] * 7),
    "CupertinoSegmentedControl": ("CupertinoSegmentedControl", [CUSTOM] * 7),
    "CupertinoSlider": ("CupertinoSlider", [CUSTOM] * 7),
    "CupertinoSwitch": ("CupertinoSwitch", [CUSTOM] * 7),
    "DateRangePicker": ("DateRangePicker", [CUSTOM] * 7),
    "Divider": ("Divider", [CUSTOM] * 7),
    "Dropdown": ("Dropdown", [CUSTOM] * 7),
    "DropdownMenu": ("DropdownMenu", [CUSTOM] * 7),
    "EditableComboBox": ("EditableComboBox", [CUSTOM] * 7),
    "EmptyState": ("EmptyState", [CUSTOM] * 7),
    "FAB": ("FAB", [CUSTOM] * 7),
    "FindReplaceDialog": ("FindReplaceDialog", [CUSTOM] * 7),
    "FloatingLabel": ("FloatingLabel", [CUSTOM] * 7),
    "FontPreview": ("FontPreview", [CUSTOM] * 7),
    "Frame": ("Frame", [CUSTOM] * 7),
    "GridTable": ("GridTable", [CUSTOM] * 7),
    "HeroAnimation": ("HeroAnimation", [CUSTOM] * 7),
    "Icon": ("Icon", [CUSTOM] * 7),
    "ImageGallery": ("ImageGallery", [CUSTOM] * 7),
    "ImageView": ("ImageView", [CUSTOM] * 7),
    "ImePreedit": ("ImePreedit", [CUSTOM] * 7),
    "InplaceEditor": ("InplaceEditor", [CUSTOM] * 7),
    "Keyboard": ("Keyboard", [CUSTOM] * 7),
    "Line": ("Line", [CUSTOM] * 7),
    "LineChart": ("LineChart", [CUSTOM] * 7),
    "LottieWidget": ("LottieWidget", [CUSTOM] * 7),
    "MaskedEdit": ("MaskedEdit", [CUSTOM] * 7),
    "MasonryLayout": ("MasonryLayout", [CUSTOM] * 7),
    "MaterialNavigationRail": ("MaterialNavigationRail", [CUSTOM] * 7),
    "MaterialSnackbar": ("MaterialSnackbar", [CUSTOM] * 7),
    "MenuButton": ("MenuButton", [CUSTOM] * 7),
    "Meter": ("Meter", [CUSTOM] * 7),
    "MiniCanvas": ("MiniCanvas", [CUSTOM] * 7),
    "MiniChart": ("MiniChart", [CUSTOM] * 7),
    "MobileDatePicker": ("MobileDatePicker", [CUSTOM] * 7),
    "ModalBottomSheet": ("ModalBottomSheet", [CUSTOM] * 7),
    "MultiSelectComboBox": ("MultiSelectComboBox", [CUSTOM] * 7),
    "NavigationDrawer": ("NavigationDrawer", [CUSTOM] * 7),
    "NavigationStack": ("NavigationStack", [CUSTOM] * 7),
    "PagerPageView": ("PagerPageView", [CUSTOM] * 7),
    "PieChart": ("PieChart", [CUSTOM] * 7),
    "Popover": ("Popover", [CUSTOM] * 7),
    "ProgressCircle": ("ProgressCircle", [CUSTOM] * 7),
    "PropertiesPanel": ("PropertiesPanel", [CUSTOM] * 7),
    "QRCode": ("QRCode", [CUSTOM] * 7),
    "RangeSlider": ("RangeSlider", [CUSTOM] * 7),
    "Rating": ("Rating", [CUSTOM] * 7),
    "RefreshControl": ("RefreshControl", [CUSTOM] * 7),
    "RiveWidget": ("RiveWidget", [CUSTOM] * 7),
    "Roller": ("Roller", [CUSTOM] * 7),
    "SafeArea": ("SafeArea", [CUSTOM] * 7),
    "SearchBar": ("SearchBar", [CUSTOM] * 7),
    "SearchBox": ("SearchBox", [CUSTOM] * 7),
    "SegmentedButton": ("SegmentedButton", [CUSTOM] * 7),
    "ShortcutEditor": ("ShortcutEditor", [CUSTOM] * 7),
    "SkeletonLoader": ("SkeletonLoader", [CUSTOM] * 7),
    "Sparkline": ("Sparkline", [CUSTOM] * 7),
    "Spinner": ("Spinner", [CUSTOM] * 7),
    "Stepper": ("Stepper", [CUSTOM] * 7),
    "SwipeToDismiss": ("SwipeToDismiss", [CUSTOM] * 7),
    "Switch": ("Switch", [CUSTOM] * 7),
    "TabView": ("TabView", [CUSTOM] * 7),
    "TagInput": ("TagInput", [CUSTOM] * 7),
    "TextArea": ("TextArea", [CUSTOM] * 7),
    "TileView": ("TileView", [CUSTOM] * 7),
    "Tooltip": ("Tooltip", [CUSTOM] * 7),
    "VideoPlayer": ("VideoPlayer", [CUSTOM] * 7),
    "WizardDialog": ("WizardDialog", [CUSTOM] * 7),
}

# Sort widgets alphabetically by display name
SORTED_KEYS = sorted(WIDGETS.keys(), key=lambda k: WIDGETS[k][0])

SYMBOL_SEMANTICS = """
| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Native implementation — the backend creates a real platform primitive for this widget (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`). 原生实现：后端创建真实平台原语。 |
| 🟦 | Self-drawn implementation (**fully functional**) — the platform ships no primitive for this widget, so this library's custom backend draws it. The widget behaves normally; it is simply not hosted by the OS. 自绘实现（功能完整）：平台无此原语，由本库自绘后端实现，行为正常，只是不由操作系统承载。 |
| 🔶 | Limited — the native path degrades to a *different* primitive, so the widget loses its identity (e.g. `create_chart` returns a panel). 受限：原生路径降级为其它原语，丢失自身身份。 |
| ⬜ | Placeholder — declared but not implemented yet. 已声明但尚未实现。 |
| ➖ | Not applicable on this platform. 该平台不适用。 |

> **How to read this（如何阅读）**
>
> - ✅ means a real platform primitive exists for the widget on that platform.
> - 🟦 means the widget is **implemented and usable**, just self-drawn. It is *not*
>   a defect: the project policy is native-first with a self-drawn fallback
>   （原生优先，自绘兜底）. Every 🟦 widget has a dedicated `create_*`
>   implementation in `src/control_backend/custom/`, not a delegation.
> - 🔶 is reserved for genuine downgrades where the *native* path silently
>   substitutes a different primitive. See "Degradation notes" for the exact map.
>
> 注意：🟦 表示控件**已实现且可用**，只是由本库自绘，并非缺陷；每个 🟦 控件在
> `src/control_backend/custom/` 都有专用 `create_*` 实现（非委托）。🔶 仅用于
> *原生路径*静默替换为其它原语的真实降级情形。
"""

def _snake_to_widget_row(fn_name: str) -> str:
    """Convert a `create_*` method name to the matrix's Widget row name.

    The result must match a `WidgetKind` variant in `src/widget/kind.rs`, since
    the matrix rows are keyed by those names. Examples:
    `create_web_engine_view` -> `WebEngineView`, `create_lcd_number` -> `LCDNumber`,
    `create_mdi_area` -> `MdiArea`, `create_stack_widget` -> `StackedWidget`.
    """
    stem = fn_name[len("create_") :]
    row = "".join(part.capitalize() for part in stem.split("_"))
    # Match the exact `WidgetKind` spellings where naive capitalization differs.
    row = row.replace("Lcd", "LCD")
    row = row.replace("Mdi", "Mdi")
    row = row.replace("StackWidget", "StackedWidget")
    row = row.replace("ToolBox", "Toolbox")
    row = row.replace("Checkbox", "CheckBox")
    return row


def derive_degradation_map(native_rs_path: str) -> Dict[str, List[str]]:
    """Parse `src/control_backend/native.rs` for `create_*` delegations.

    Returns a mapping of `fallback_fn -> [delegating_fn, ...]` for every method
    whose body forwards to a *different* `get_platform().create_*` call. Methods
    that forward to themselves are dedicated implementations, not degradations,
    and are excluded.

    Deriving this mechanically keeps the generated "Degradation notes" in sync
    with the code; a hand-maintained mirror previously drifted (widgets that had
    gained dedicated implementations were still listed as degraded).
    """
    with open(native_rs_path, "r", encoding="utf-8") as handle:
        source = handle.read()

    method_re = re.compile(
        r"fn (create_\w+)\s*\([^)]*\)\s*->\s*ObjectId\s*\{(.*?)\n    \}",
        re.DOTALL,
    )
    call_re = re.compile(r"get_platform\(\)\.(create_\w+)\(")

    delegations: Dict[str, List[str]] = {}
    for match in method_re.finditer(source):
        delegating, body = match.group(1), match.group(2)
        calls = call_re.findall(body)
        if not calls:
            continue
        fallback = calls[0]
        if fallback == delegating:
            # Dedicated implementation — the matrix marks these as ✅/🟦,
            # never as a degradation.
            continue
        delegations.setdefault(fallback, []).append(delegating)
    return delegations


def render_degradation_table(delegations: Dict[str, List[str]]) -> str:
    """Render the derived delegation map as the markdown fallback table.

    Row names are deduplicated (`create_toolbox`/`create_tool_box` both map to
    `Toolbox`) and filtered to names that are real `WidgetKind` variants, because
    the matrix is keyed by those.
    """
    known_kinds = _widget_kind_variants()
    rows = []
    for fallback in sorted(delegations):
        names = sorted({_snake_to_widget_row(name) for name in delegations[fallback]})
        known = [name for name in names if name in known_kinds]
        if known:
            rows.append(f"| `{fallback}` | {', '.join(known)} |")
    return "\n".join(rows)


_KIND_CACHE: Dict[str, List[str]] = {}


def _widget_kind_variants() -> List[str]:
    """Return the `WidgetKind` variant names declared in `src/widget/kind.rs`.

    Filters the derived row names down to real kinds, so handle/render-layer
    aliases such as `WebView` (which maps onto `WidgetKind::WebEngineView`) do not
    leak into the fallback table.
    """
    if "variants" in _KIND_CACHE:
        return _KIND_CACHE["variants"]
    kind_rs = os.path.join(REPO_ROOT, "src", "widget", "kind.rs")
    variants: List[str] = []
    with open(kind_rs, "r", encoding="utf-8") as handle:
        in_enum = False
        for line in handle:
            stripped = line.strip()
            if stripped.startswith("pub enum WidgetKind"):
                in_enum = True
                continue
            if in_enum and stripped.startswith("}"):
                break
            if in_enum:
                match = re.match(r"([A-Z][A-Za-z0-9_]*)\s*[,=]", stripped)
                if match:
                    variants.append(match.group(1))
    _KIND_CACHE["variants"] = variants
    return variants


# Degradation notes — the fallback table is derived from
# src/control_backend/native.rs at generation time so it cannot drift from the
# code. The surrounding prose is static.
DEGRADATION_NOTES_HEADER = """
## Degradation notes（降级说明）

On the **native/FFI path** (`src/control_backend/native.rs`), the following widget
families are not created as dedicated native controls. Each `create_*` listed
below delegates to a fallback primitive, silently in most cases (`log::warn!` is
emitted only for `data_view`, `property_grid`, `collapsible_pane`, `column_view`,
`undo_view`):

| Fallback created | Widgets (WidgetKind / matrix row names) |
| --- | --- |
"""

DEGRADATION_NOTES_FOOTER = """

Additional facts to keep the matrix consistent with `src/widget/kind.rs`:
- `ToolBox` is not a `WidgetKind` variant (only `Toolbox` is); the duplicate row was removed.
- `WebView` is not a `WidgetKind` variant either — the `WebView`/`WebViewEnhanced`
  aliases live at the handle/render layer and map onto `WidgetKind::WebEngineView`.
  The matrix therefore lists only the WebEngine rows.
- `MessageBox`/`FileDialog`/`ColorDialog`/`FontDialog` are 🟦 (self-drawn) on
  Windows/Linux/Wayland because the **default** runtime of those platform impls
  creates a state/surrogate handle (Windows: `Panel` surrogate; Linux/Wayland:
  state-only) rather than a dedicated native dialog; only macOS (objc2 +
  cocoa-legacy) creates real `NSAlert`/`NSOpenPanel`/`NSColorPanel`/`NSFontPanel` (✅).
  The widgets themselves are fully usable — the custom backend provides a dedicated
  dialog implementation — they are simply not OS-hosted. Under the `gtk-native`
  feature the Linux backend additionally constructs real
  `gtk::MessageDialog`/`FileChooserDialog`/`ColorChooserDialog`/`FontChooserDialog`
  objects, so a `gtk-native` build upgrades those cells to ✅ in practice.
- `SpinBox`/`ListView`/`ScrollArea` are ✅ on Windows as of 2026-09-11: the backend now
  creates real Win32 objects (`msctls_updown32` with `UDS_SETBUDDYINT`;`SysListView32`
  in report view with an inserted column and `LVS_EX_FULLROWSELECT`; a
  `WS_HSCROLL | WS_VSCROLL` child window with an initial scroll range). These are
  **compile-verified** for `x86_64-pc-windows-msvc`/`gnullvm` and clippy-clean via
  the `windows-cross-check` CI job, but have not yet been observed on a running
  Windows machine.
- `SpinBox` is 🟦 (self-drawn) on Linux/X11, Wayland, Mobile and Harmony: the macOS
  objc2 backend creates a native `NSStepper` under the `macos` feature, but the
  default cocoa-legacy path is not native. Under `gtk-native` the Linux backend
  creates a real `gtk::SpinButton`.
- `ListView`/`ScrollArea` are 🟦 (self-drawn) on Linux/X11, macOS, Wayland, Mobile
  and Harmony in the default build.
- `DatePicker`/`TimePicker`/`DateTimePicker` gained dedicated `create_*` native
  implementations on 2026-09-11 (GTK composites over `gtk::Calendar`/`SpinButton`;
  Win32 `SysDateTimePick32`), so they no longer appear in the fallback table above.
- Mobile (Android) creation is state-backed by default; when `android-jni` is
  enabled **and** an Activity `Context` is stored, `platform_impl` constructs real
  Android `View` objects for Button/TextView/EditText/CheckBox/RadioButton/
  SeekBar/ProgressBar/Spinner/ListView/ScrollView/NumberPicker/FrameLayout.
  `MessageBox` also becomes a real `android.app.AlertDialog` (create / show /
  dismiss / `setMessage`) on that path — verified on an Android emulator, and on a
  physical arm64 device (Xiaomi M2102J2SC, Android 13).
  `FileDialog` becomes a real system `ACTION_OPEN_DOCUMENT` picker on that path
  (also verified on the physical device). `ColorDialog`/`FontDialog` (no platform
  picker on Android) and menus/toolbars remain logical-only because they need an
  Activity callback rather than a standalone View; the backend logs an explicit
  diagnostic on that path instead of silently degrading."""


DEGRADATION_NOTES = (
    DEGRADATION_NOTES_HEADER
    + render_degradation_table(derive_degradation_map(NATIVE_RS))
    + DEGRADATION_NOTES_FOOTER
)



def generate_matrix() -> str:
    """Generate the full markdown document."""
    lines = []
    lines.append("# Platform Capability Matrix — R6")
    lines.append("")
    lines.append("> **Auto-generated** by `tools/generate_platform_capability_matrix.py`")
    lines.append(
        "> **Legend:** ✅ Native · 🟦 Self-drawn (functional) · 🔶 Limited · ⬜ Placeholder · ➖ N/A"
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
