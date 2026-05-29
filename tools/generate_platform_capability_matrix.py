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
    "WebView": ("WebView", [STATE_BACKED] * 7),
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
    "ToolBox": ("ToolBox", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    # === Special ===
    "FreeformShape": ("FreeformShape", [STATE_BACKED] * 7),
    "TabBar": ("TabBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
    "PieMenu": ("PieMenu", [STATE_BACKED] * 7),
    "RibbonBar": ("RibbonBar", [NATIVE] * 4 + [NATIVE] * 2 + [STATE_BACKED]),
}

# Sort widgets alphabetically by display name
SORTED_KEYS = sorted(WIDGETS.keys(), key=lambda k: WIDGETS[k][0])


def generate_matrix() -> str:
    """Generate the full markdown document."""
    lines = []
    lines.append("# Platform Capability Matrix — R6")
    lines.append("")
    lines.append("> **Auto-generated** by `tools/generate_platform_capability_matrix.py`")
    lines.append(
        "> **Legend:** ✅ Native · 🔶 StateBacked · ⬜ Placeholder · ➖ NotApplicable"
    )
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
