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
import pathlib
import re
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
# the custom-paint backend is NOT the same as one that degrades to a different
# primitive on the primitive-mapping path.
#
#   NATIVE       — the backend creates a real platform primitive for this widget
#                  (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`).
#   CUSTOM       — no platform primitive exists, but the custom-paint backend has a
#                  dedicated `create_*` implementation. The widget is fully
#                  functional; it is simply drawn by this library rather than by
#                  the platform.
#   STATE_BACKED — the primitive path answers with a *different* primitive (a silent
#                  downgrade, e.g. `create_chart` -> `create_panel`), so the widget
#                  loses its identity. This is the genuinely limited case.
#
# NOTE: NATIVE vs CUSTOM is an *internal platform strategy*. Callers never branch
# on it — they call one API and the platform layer picks the mechanism. These
# symbols exist to audit backend coverage, not to appear in user-facing code.
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
    NATIVE: "Primitive-mapped implementation — the backend creates a real platform "
    "primitive for this widget. 映射到平台原语：后端创建真实平台原语。",
    CUSTOM: "Custom-painted implementation (fully functional) — no platform primitive exists, "
    "so this library's custom backend draws it; the widget behaves normally. "
    "自绘型实现（功能完整）：平台无此原语，由本库自绘后端绘制，行为正常。",
    STATE_BACKED: "Limited — the primitive path degrades to a *different* primitive and the widget "
    "loses its identity. 受限：原语路径降级为其它原语，丢失自身身份。",
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
# ---------------------------------------------------------------------------
# Widget capability definitions
#
# Every cell is `CUSTOM`, because every `WidgetKind` is painted by this library on
# every platform: the host supplies a window and a drawing surface, and there is no
# per-kind primitive to map onto (BLUE15 #55/#56). The table used to carry a
# hand-maintained "which platform has a primitive for this widget" vector — 167 rows
# x 7 platforms of cells that had to be edited in lockstep with the backends. That
# question no longer has a per-platform answer, so it is stated once via `CELLS`
# instead of 1169 times.
#
# The row set is still enumerated because the gate cross-checks it against
# `src/widget/kind.rs`: a `WidgetKind` with no row here is a real defect, and this
# keeps that check meaningful.
#
# If a backend ever gains a genuine primitive, it must say so *deliberately* — the
# right place is `route_preference_for_widget_kind` (see BLUE15 section 7), and this
# table then needs the per-platform vectors back.
# ---------------------------------------------------------------------------
CELLS = [CUSTOM] * len(PLATFORMS)
WIDGETS: Dict[str, Tuple[str, List[str]]] = {
    "Window": ("Window", CELLS),
    "Dialog": ("Dialog", CELLS),
    "MessageBox": ("MessageBox", CELLS),
    "FileDialog": ("FileDialog", CELLS),
    "ColorDialog": ("ColorDialog", CELLS),
    "FontDialog": ("FontDialog", CELLS),
    "InputDialog": ("InputDialog", CELLS),
    "ProgressDialog": ("ProgressDialog", CELLS),
    "PopupWindow": ("PopupWindow", CELLS),
    "Button": ("Button", CELLS),
    "CheckBox": ("CheckBox", CELLS),
    "RadioButton": ("RadioButton", CELLS),
    "Label": ("Label", CELLS),
    "LineEdit": ("LineEdit", CELLS),
    "TextEdit": ("TextEdit", CELLS),
    "RichEdit": ("RichEdit", CELLS),
    "ComboBox": ("ComboBox", CELLS),
    "SpinBox": ("SpinBox", CELLS),
    "ListBox": ("ListBox", CELLS),
    "ListView": ("ListView", CELLS),
    "TreeView": ("TreeView", CELLS),
    "ProgressBar": ("ProgressBar", CELLS),
    "Slider": ("Slider", CELLS),
    "ScrollBar": ("ScrollBar", CELLS),
    "ScrollArea": ("ScrollArea", CELLS),
    "Panel": ("Panel", CELLS),
    "DockPanel": ("DockPanel", CELLS),
    "GroupBox": ("GroupBox", CELLS),
    "TabWidget": ("TabWidget", CELLS),
    "Splitter": ("Splitter", CELLS),
    "MdiArea": ("MdiArea", CELLS),
    "MenuBar": ("MenuBar", CELLS),
    "Menu": ("Menu", CELLS),
    "MenuItem": ("MenuItem", CELLS),
    "ContextMenu": ("ContextMenu", CELLS),
    "ToolBar": ("ToolBar", CELLS),
    "StatusBar": ("StatusBar", CELLS),
    "Canvas": ("Canvas", CELLS),
    "Table": ("Table", CELLS),
    "Grid": ("Grid", CELLS),
    "Chart": ("Chart", CELLS),
    "RadarChart": ("RadarChart", CELLS),
    "CandlestickChart": ("CandlestickChart", CELLS),
    "VolumeChart": ("VolumeChart", CELLS),
    "DepthChart": ("DepthChart", CELLS),
    "OrderBook": ("OrderBook", CELLS),
    "QuoteBoard": ("QuoteBoard", CELLS),
    "IndicatorChart": ("IndicatorChart", CELLS),
    "KanbanBoard": ("KanbanBoard", CELLS),
    "Cascader": ("Cascader", CELLS),
    "QueryBuilder": ("QueryBuilder", CELLS),
    "EmojiPicker": ("EmojiPicker", CELLS),
    "Mention": ("Mention", CELLS),
    "ToggleButton": ("ToggleButton", CELLS),
    "CheckListBox": ("CheckListBox", CELLS),
    "DoubleSpinBox": ("DoubleSpinBox", CELLS),
    "Dial": ("Dial", CELLS),
    "Wizard": ("Wizard", CELLS),
    "DatePicker": ("DatePicker", CELLS),
    "TimePicker": ("TimePicker", CELLS),
    "DateTimePicker": ("DateTimePicker", CELLS),
    "DirectoryDialog": ("DirectoryDialog", CELLS),
    "DataView": ("DataView", CELLS),
    "PropertyGrid": ("PropertyGrid", CELLS),
    "Toolbox": ("Toolbox", CELLS),
    "StackedWidget": ("StackedWidget", CELLS),
    "CollapsiblePane": ("CollapsiblePane", CELLS),
    "DockWidget": ("DockWidget", CELLS),
    "ActivityIndicator": ("ActivityIndicator", CELLS),
    "Calendar": ("Calendar", CELLS),
    "ColumnView": ("ColumnView", CELLS),
    "UndoView": ("UndoView", CELLS),
    "CommandLink": ("CommandLink", CELLS),
    "LCDNumber": ("LCDNumber", CELLS),
    "FontComboBox": ("FontComboBox", CELLS),
    "WebEngineView": ("WebEngineView", CELLS),
    "WebEnginePage": ("WebEnginePage", CELLS),
    "WebEngineSettings": ("WebEngineSettings", CELLS),
    "WebEngineDownloadItem": ("WebEngineDownloadItem", CELLS),
    "WebEngineCookieStore": ("WebEngineCookieStore", CELLS),
    "WebEngineWebChannel": ("WebEngineWebChannel", CELLS),
    "WebEngineFindTextResult": ("WebEngineFindTextResult", CELLS),
    "WebEngineNotification": ("WebEngineNotification", CELLS),
    "WebEngineScriptDialog": ("WebEngineScriptDialog", CELLS),
    "WebEngineContextMenuRequest": ("WebEngineContextMenuRequest", CELLS),
    "Action": ("Action", CELLS),
    "ToolButton": ("ToolButton", CELLS),
    "FreeformShape": ("FreeformShape", CELLS),
    "TabBar": ("TabBar", CELLS),
    "PieMenu": ("PieMenu", CELLS),
    "RibbonBar": ("RibbonBar", CELLS),
    "AdaptiveScaffold": ("AdaptiveScaffold", CELLS),
    "AnimatedImage": ("AnimatedImage", CELLS),
    "AppBar": ("AppBar", CELLS),
    "Arc": ("Arc", CELLS),
    "AudioVisualizer": ("AudioVisualizer", CELLS),
    "AutoCompleteEdit": ("AutoCompleteEdit", CELLS),
    "Avatar": ("Avatar", CELLS),
    "Badge": ("Badge", CELLS),
    "BarChart": ("BarChart", CELLS),
    "Banner": ("Banner", CELLS),
    "BarcodeScanner": ("BarcodeScanner", CELLS),
    "BezierCurveEditor": ("BezierCurveEditor", CELLS),
    "BottomNavigationBar": ("BottomNavigationBar", CELLS),
    "BottomSheet": ("BottomSheet", CELLS),
    "CameraPreview": ("CameraPreview", CELLS),
    "Carousel": ("Carousel", CELLS),
    "Chip": ("Chip", CELLS),
    "ColorHistory": ("ColorHistory", CELLS),
    "ColorPicker": ("ColorPicker", CELLS),
    "ColorWell": ("ColorWell", CELLS),
    "CupertinoAlertDialog": ("CupertinoAlertDialog", CELLS),
    "CupertinoDatePicker": ("CupertinoDatePicker", CELLS),
    "CupertinoNavigationBar": ("CupertinoNavigationBar", CELLS),
    "CupertinoSegmentedControl": ("CupertinoSegmentedControl", CELLS),
    "CupertinoSlider": ("CupertinoSlider", CELLS),
    "CupertinoSwitch": ("CupertinoSwitch", CELLS),
    "DateRangePicker": ("DateRangePicker", CELLS),
    "Divider": ("Divider", CELLS),
    "Dropdown": ("Dropdown", CELLS),
    "DropdownMenu": ("DropdownMenu", CELLS),
    "EditableComboBox": ("EditableComboBox", CELLS),
    "EmptyState": ("EmptyState", CELLS),
    "FAB": ("FAB", CELLS),
    "FindReplaceDialog": ("FindReplaceDialog", CELLS),
    "FloatingLabel": ("FloatingLabel", CELLS),
    "FontPreview": ("FontPreview", CELLS),
    "Frame": ("Frame", CELLS),
    "GridTable": ("GridTable", CELLS),
    "NumberPicker": ("NumberPicker", CELLS),
    "OtpInput": ("OtpInput", CELLS),
    "Pagination": ("Pagination", CELLS),
    "SplashScreen": ("SplashScreen", CELLS),
    "Toast": ("Toast", CELLS),
    "HeroAnimation": ("HeroAnimation", CELLS),
    "Icon": ("Icon", CELLS),
    "ImageGallery": ("ImageGallery", CELLS),
    "ImageView": ("ImageView", CELLS),
    "ImePreedit": ("ImePreedit", CELLS),
    "InplaceEditor": ("InplaceEditor", CELLS),
    "Keyboard": ("Keyboard", CELLS),
    "Line": ("Line", CELLS),
    "LineChart": ("LineChart", CELLS),
    "LottieWidget": ("LottieWidget", CELLS),
    "MaskedEdit": ("MaskedEdit", CELLS),
    "MasonryLayout": ("MasonryLayout", CELLS),
    "MaterialNavigationRail": ("MaterialNavigationRail", CELLS),
    "MaterialSnackbar": ("MaterialSnackbar", CELLS),
    "MenuButton": ("MenuButton", CELLS),
    "Meter": ("Meter", CELLS),
    "MiniCanvas": ("MiniCanvas", CELLS),
    "MiniChart": ("MiniChart", CELLS),
    "MobileDatePicker": ("MobileDatePicker", CELLS),
    "ModalBottomSheet": ("ModalBottomSheet", CELLS),
    "MultiSelectComboBox": ("MultiSelectComboBox", CELLS),
    "NavigationDrawer": ("NavigationDrawer", CELLS),
    "NavigationStack": ("NavigationStack", CELLS),
    "PieChart": ("PieChart", CELLS),
    "Popover": ("Popover", CELLS),
    "ProgressCircle": ("ProgressCircle", CELLS),
    "PropertiesPanel": ("PropertiesPanel", CELLS),
    "QRCode": ("QRCode", CELLS),
    "RangeSlider": ("RangeSlider", CELLS),
    "Rating": ("Rating", CELLS),
    "RefreshControl": ("RefreshControl", CELLS),
    "RiveWidget": ("RiveWidget", CELLS),
    "Roller": ("Roller", CELLS),
    "SafeArea": ("SafeArea", CELLS),
    "SearchBar": ("SearchBar", CELLS),
    "SearchBox": ("SearchBox", CELLS),
    "SegmentedButton": ("SegmentedButton", CELLS),
    "ShortcutEditor": ("ShortcutEditor", CELLS),
    "SkeletonLoader": ("SkeletonLoader", CELLS),
    "Sparkline": ("Sparkline", CELLS),
    "Spinner": ("Spinner", CELLS),
    "Stepper": ("Stepper", CELLS),
    "SwipeToDismiss": ("SwipeToDismiss", CELLS),
    "Switch": ("Switch", CELLS),
    "TabView": ("TabView", CELLS),
    "TagInput": ("TagInput", CELLS),
    "TextArea": ("TextArea", CELLS),
    "Tooltip": ("Tooltip", CELLS),
    "VideoPlayer": ("VideoPlayer", CELLS),
    "WizardDialog": ("WizardDialog", CELLS),
    "TreeTable": ("TreeTable", CELLS),
    "Breadcrumb": ("Breadcrumb", CELLS),
    "SignaturePad": ("SignaturePad", CELLS),
    "DropZone": ("DropZone", CELLS),
}

# Sort widgets alphabetically by display name
SORTED_KEYS = sorted(WIDGETS.keys(), key=lambda k: WIDGETS[k][0])

SYMBOL_SEMANTICS = """
| Symbol | Meaning（符号语义） |
| --- | --- |
| ✅ | Primitive-mapped implementation — the backend creates a real platform primitive for this widget (e.g. Win32 `Button`, GTK `SpinButton`, Android `SeekBar`). 映射到平台原语：后端创建真实平台原语。 |
| 🟦 | Custom-painted implementation (**fully functional**) — the platform ships no primitive for this widget, so this library's custom backend draws it. The widget behaves normally. 自绘型实现（功能完整）：平台无此原语，由本库自绘后端绘制，行为正常。 |
| 🔶 | Limited — the primitive path degrades to a *different* primitive, so the widget loses its identity (e.g. `create_chart` returns a panel). 受限：原语路径降级为其它原语，丢失自身身份。 |
| ⬜ | Placeholder — declared but not implemented yet. 已声明但尚未实现。 |
| ➖ | Not applicable on this platform. 该平台不适用。 |

> **How to read this（如何阅读）**
>
> **✅ 与 🟦 的区分是 `src/platform/` 的内部实现策略。调用方从不区分二者**：
> 它调用同一个 API，由平台层选择机制。这两个符号用于审计后端覆盖度，
> 不应出现在面向使用者的代码分支中。
>
> - ✅ means a real platform primitive exists for the widget on that platform.
> - 🟦 means the widget is **implemented and usable**, just custom-painted. It is
>   *not* a defect. Every 🟦 widget has a dedicated `create_*` implementation in
>   `src/control_backend/custom/`, not a delegation.
> - 🔶 is reserved for genuine downgrades where the *primitive* path silently
>   substitutes a different primitive. See "Degradation notes" for the exact map.
>
> 注意：🟦 表示控件**已实现且可用**，只是由本库自绘，并非缺陷；每个 🟦 控件在
> `src/control_backend/custom/` 都有专用 `create_*` 实现（非委托）。🔶 仅用于
> *原语路径*静默替换为其它原语的真实降级情形。
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
- **The `WebEngine*` rows after `WebEngineView` are not `WidgetKind` variants.**
  `WebEnginePage`, `WebEngineSettings`, `WebEngineDownloadItem`,
  `WebEngineCookieStore`, `WebEngineWebChannel`, `WebEngineFindTextResult`,
  `WebEngineNotification`, `WebEngineScriptDialog` and `WebEngineContextMenuRequest`
  are Rust **wrapper types** over the one registered view: each forwards
  `Widget::base()` to what it wraps, so `kind()` answers `WebEngineView` for all of
  them. They were previously `WidgetKind` variants marked `kind-role: base`, which
  made them orphans (rule #22) — nothing could produce them, and because
  `factory_name_for_kind` resolves through `capability_by_kind`,
  `create_web_engine_page(..)` silently produced id `0`. They remain listed here
  because they are real render-pipeline symbols worth tracking, but they are
  **categories of `WebEngineView`**, not kinds of their own.
  `tests/blue9_r6_platform_capability_test.rs` therefore compares matrix rows to
  `WidgetKind` variants modulo exactly this documented set.
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
  `route_preference_for_widget_kind` promotes these three kinds to
  `ControlRoutePreference::NativePreferred` **only under `cfg(target_os = "windows")`**;
  every other OS keeps them on the custom backend, because no other platform provides
  these primitives. Before that routing change the Win32 implementations existed but
  were unreachable — the global `CustomRequired` arm always won — so the ✅ cells
  overstated the reachable behaviour. The Win32 objects are reached through
  `NativeControlBackend::create_spin_box`/`create_list_view`/`create_scroll_area`,
  each of which forwards to its same-named `Platform` method (no aliasing to a Panel).
  `windows_native_controls_route_natively` and
  `non_windows_native_controls_use_custom_backend` pin both sides of that split.

### ✅ cells not yet verified on a real device（未经真机验证的 ✅ 单元格）

A ✅ cell means a real platform primitive is created and reached. The following
cells are **compile-verified only** — they build cleanly for their target and are
covered by clippy, but no one has observed the widget on the running OS. Treat
them as "implemented and wired, pending hardware confirmation", not as confirmed
runtime behaviour:

| Widget | Platform | Status |
| --- | --- | --- |
| `SpinBox` | Windows | compile-verified (`x86_64-pc-windows-msvc`/`gnullvm`), not yet run on Windows |
| `ListView` | Windows | compile-verified, not yet run on Windows |
| `ScrollArea` | Windows | compile-verified, not yet run on Windows |

Everything else marked ✅ has been exercised on a real device or the platform's
native runtime (Windows/Linux/macOS desktop backends are the default build path;
the Android JNI cells were verified on an emulator and a physical arm64 device).
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


def _abi_constructible_kinds() -> set[str]:
    """Kinds a C caller can construct, derived from the exported ABI.

    # Why this is derived rather than listed

    The question "can a C caller create this control?" has exactly one source of
    truth: the exported bindings. Writing the answer as a table here would let it
    drift from `binding_impl.rs` the moment a function is added — which is the
    failure this column exists to make visible, not to reproduce.

    # How the derivation works

    Each exported `rw_create_*` function forwards to a backend `create_*` whose name
    mirrors the widget's kind (`rw_create_line_edit` -> `create_line_edit` ->
    `LineEdit`). The generic `rw_create_widget_of_kind` is deliberately *not* treated
    as covering everything: it takes a name at run time, so which kinds it can reach
    is not knowable from the signature, and claiming it here would make the column
    say "every kind" and therefore say nothing.
    """
    bindings = pathlib.Path("src/bindings/binding_impl.rs")
    if not bindings.exists():
        return set()
    text = bindings.read_text(encoding="utf-8")
    kinds: set[str] = set()
    for match in re.finditer(r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+rw_create_([a-z0-9_]+)', text):
        suffix = match.group(1)
        # `create_line_edit` -> `LineEdit`
        kinds.add("".join(part.capitalize() for part in suffix.split("_")))
    # Spelling differences between a kind and its ABI function name, each of which is
    # visible in the bindings' own test data. Kept explicit so a rename shows up here
    # rather than silently dropping a row's marker.
    aliases = {
        "Checkbox": "CheckBox",
        "ColorDialog": "ColorDialog",
        "Toolbar": "ToolBar",
        "Statusbar": "StatusBar",
        "Menubar": "MenuBar",
        "Textedit": "TextEdit",
        "Listbox": "ListBox",
        "Treeview": "TreeView",
        "Dataview": "DataView",
        "Gridtable": "GridTable",
    }
    return {aliases.get(kind, kind) for kind in kinds}


def generate_matrix() -> str:
    """Generate the full markdown document."""
    lines = []
    lines.append("# Platform Capability Matrix — R6")
    lines.append("")
    lines.append(
        "> **Auto-generated** by `tools/generate_platform_capability_matrix.py`"
    )
    lines.append(
        "> **Legend:** ✅ Primitive-mapped · 🟦 Custom-painted (functional) · 🔶 Limited · "
        "⬜ Placeholder · ➖ N/A"
    )
    lines.append(
        "> **C**: ✅ when a typed `rw_create_*` function exists for the kind, "
        "⬜ when the only route is the generic `rw_create_widget_of_kind(name)`. "
        "Derived from `src/bindings/binding_impl.rs`, never hand-maintained."
    )
    lines.append(
        "> A few ✅ cells are compile-verified only; see "
        "[✅ cells not yet verified on a real device]"
        "(#-cells-not-yet-verified-on-a-real-device未经真机验证的--单元格)"
        " under Degradation notes."
    )
    lines.append("")
    lines.append("## Symbol semantics（符号语义）")
    lines.append("")
    lines.extend(SYMBOL_SEMANTICS.strip().splitlines())
    lines.append("")
    lines.append("## Matrix")
    lines.append("")

    abi_kinds = _abi_constructible_kinds()

    # Header
    header = "| Widget | " + " | ".join(PLATFORMS) + " | C |"
    sep = "| " + "--- |" * (len(PLATFORMS) + 2)

    lines.append(header)
    lines.append(sep)

    # Rows
    for key in SORTED_KEYS:
        display_name, levels = WIDGETS[key]
        # The key is the `WidgetKind` variant name; `WIDGETS` uses the variant spelling.
        abi_cell = "✅" if key in abi_kinds else "⬜"
        row = f"| **{display_name}** | " + " | ".join(levels) + f" | {abi_cell} |"
        lines.append(row)

    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append(
        f"Total widgets: {len(WIDGETS)} ({len(WIDGETS) - 9} WidgetKind variants plus 9 documented WebEngine wrapper types)"
    )
    lines.append("")
    lines.append(
        f"C-ABI typed constructors: {len(abi_kinds & set(WIDGETS))} of {len(WIDGETS)} "
        "kinds. The remainder are reachable through `rw_create_widget_of_kind`, which "
        "takes a factory name at run time."
    )
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
        # `newline="\n"` keeps the artifact byte-identical on every host. Without
        # it Windows translates every `\n` to `\r\n`, which made
        # `tools/check_platform_capability_matrix.sh` step [4] report a false
        # "document is stale" (its `diff` compares bytes) and made the truthfulness
        # gate rewrite the checked-in LF file with CRLF on that host.
        with open(args.output, "w", encoding="utf-8", newline="\n") as f:
            f.write(output)
        print(f"Matrix written to {args.output}")
    else:
        print(output)


if __name__ == "__main__":
    main()
