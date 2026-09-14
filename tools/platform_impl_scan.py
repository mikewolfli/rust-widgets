#!/usr/bin/env python3
"""platform_impl_scan.py — Per-OS platform_impl implementation-grade scanner.

P1 honesty tooling: cross-checks that QA reports about "native" controls are
grounded in each OS `platform_impl` actually creating a native control, instead
of only trusting `src/control_backend/native.rs` delegation.

For every `create_*` method defined in each `src/platform/<os>/**` backend this
tool classifies the method as one of:

  Native       — body (after following `self.<create_*>(...)` delegate chains)
                 contains a platform-native object/alloc/registry call.
  StateBacked  — body only inserts into the logical widget state
                 (`state.create_widget` / `insert_widget`) with no native object.
  Placeholder  — empty body or only `return 0;`.
  Missing      — method not defined by this OS backend at all.
  Unclassifiable — body has code but no recognized native or state signal; a
                 maintainer must classify it (the QA gate fails on these so a
                 silent "no signal" can never be mistaken for native).

Grade tokens are intentionally conservative: when unsure the tool reports
StateBacked/Unclassifiable rather than guessing Native, so reports never
overclaim native capability.
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Set, Tuple

NATIVE = "Native"
STATE_BACKED = "StateBacked"
PLACEHOLDER = "Placeholder"
MISSING = "Missing"
UNCLASSIFIABLE = "Unclassifiable"

fn_re = re.compile(r"\bfn\s+(create_[a-z0-9_]+)\s*\(")
any_fn_re = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
delegate_re = re.compile(r"\bself\.([A-Za-z_][A-Za-z0-9_]*)\s*\(")
free_call_re = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(self\b")
state_insert_re = re.compile(r"(?:state\.)?create_widget\(|insert_widget\(")

# Per-OS native-creation signals (tokens inside a method body or a followed
# delegate chain indicate a real native/FFI object is created). Intentionally
# conservative: preview/state-only backends keep an empty list so they are never
# misreported as native.
OS_NATIVE_TOKENS: Dict[str, List[str]] = {
    "windows": [
        "CreateWindowExW",
        "CreateWindowEx",
        "bind_native_handle",
        "try_create_",
    ],
    "linux": [
        "gtk::",
        "upcast::<gtk::",
        "gtk::Window",
        "gtk::Button",
    ],
    "macos": [
        "NSWindow",
        "NSButton",
        "NSTextField",
        "NSSlider",
        "NSAlert",
        "NSOpenPanel",
        "NSColorPanel",
        "NSFontPanel",
        "create_native_dialog",
        "initWithFrame",
        "addSubview",
        "class!(",
        "shared_button_target",
        "create_ns_",
    ],
    "macos_objc2": [
        "store_native_view",
        "MainThreadMarker",
        "create_ns_",
        "NSWindow",
    ],
    # Preview / mobile / embedded backends are state-backed by design; they get
    # an empty native-token list so they are reported StateBacked, never Native.
    "ios": [],
    "android": [],
    "harmony": [],
    "wasm": [],
    "wayland": [],
    "mobile": [],
    "stub": [],
}

# Grade order per OS: some backends always insert logical state and only add a
# native object under a feature flag. For those we prefer StateBacked so the
# default runtime is not overclaimed; a method with no state insert still falls
# through to the native check (e.g. linux create_window).
STATE_FIRST_OS = {"linux"}

# OS backend -> (primary impl file, companion files for delegate resolution).
OS_SOURCES: Dict[str, List[str]] = {
    "windows": [
        "src/platform/windows/platform_impl.rs",
        "src/platform/windows/helpers.rs",
    ],
    "linux": [
        "src/platform/linux/platform_impl.rs",
    ],
    "macos": ["src/platform/macos/platform_impl.rs"],
    "macos_objc2": ["src/platform/macos_objc2/platform_impl.rs"],
    "ios": ["src/platform/ios/platform_impl.rs"],
    "android": ["src/platform/android/platform_impl.rs"],
    "harmony": ["src/platform/harmony/platform_impl.rs"],
    "wasm": ["src/platform/wasm/platform_impl.rs"],
    "wayland": ["src/platform/wayland/platform_impl.rs"],
    "mobile": ["src/platform/mobile.rs"],
    "stub": ["src/platform/stub.rs"],
}

# File listing canonical control `create_*` methods (ControlBackend contract);
# used to filter stray non-control helpers out of the reported rows.
TRAIT_SOURCE = "src/control_backend/trait_def/trait_def.rs"

# Widgets whose matrix row maps to a create method (reused to align reports).
KIND_METHOD_OVERRIDES: Dict[str, str] = {
    "CheckBox": "create_checkbox",
    "LineEdit": "create_line_edit",
    "TextEdit": "create_text_edit",
    "RichEdit": "create_rich_edit",
    "ComboBox": "create_combo_box",
    "ListBox": "create_list_box",
    "ListView": "create_list_view",
    "TreeView": "create_tree_view",
    "ScrollBar": "create_scroll_bar",
    "ScrollArea": "create_scroll_area",
    "DockPanel": "create_dock_panel",
    "GroupBox": "create_group_box",
    "TabWidget": "create_tab_widget",
    "MdiArea": "create_mdi_area",
    "MenuBar": "create_menu_bar",
    "ToolBar": "create_tool_bar",
    "StatusBar": "create_status_bar",
    "ToggleButton": "create_toggle_button",
    "CheckListBox": "create_check_list_box",
    "DoubleSpinBox": "create_double_spin_box",
    "DatePicker": "create_date_picker",
    "TimePicker": "create_time_picker",
    "DateTimePicker": "create_date_time_picker",
    "DirectoryDialog": "create_directory_dialog",
    "DataView": "create_data_view",
    "PropertyGrid": "create_property_grid",
    "StackedWidget": "create_stack_widget",
    "DockWidget": "create_dock_widget",
    "ActivityIndicator": "create_activity_indicator",
    "ColumnView": "create_column_view",
    "UndoView": "create_undo_view",
    "CommandLink": "create_command_link",
    "LCDNumber": "create_lcd_number",
    "FontComboBox": "create_font_combo_box",
    "ToolButton": "create_tool_button",
    "ToolBox": "create_tool_box",
    "MenuItem": "create_action",
    "FreeformShape": "create_canvas",
    "PopupWindow": "create_popup_window",
    "MessageBox": "create_message_box",
    "FileDialog": "create_file_dialog",
    "ColorDialog": "create_color_dialog",
    "FontDialog": "create_font_dialog",
    "Frame": "create_group_box",
    "InputDialog": "create_dialog",
    "ProgressDialog": "create_dialog",
    "PieMenu": "create_menu",
    "RibbonBar": "create_panel",
    "TabBar": "create_tab_widget",
}


@dataclass(frozen=True)
class OsBackend:
    key: str
    files: List[str]
    methods: Dict[str, str]  # method name -> grade
    details: Dict[str, str]  # method name -> short signal note


def extract_method_bodies(source: str, regex=fn_re) -> Dict[str, str]:
    methods: Dict[str, str] = {}
    i = 0
    while i < len(source):
        m = regex.search(source, i)
        if not m:
            break
        method = m.group(1)
        open_idx = source.find("{", m.end())
        if open_idx < 0:
            i = m.end()
            continue
        brace_count = 1
        j = open_idx + 1
        while j < len(source) and brace_count > 0:
            if source[j] == "{":
                brace_count += 1
            elif source[j] == "}":
                brace_count -= 1
            j += 1
        methods[method] = source[m.start():j]
        i = j
    return methods


def _collect_chain(
    method: str,
    bodies: Dict[str, str],
    os_key: str,
    _seen: Optional[Set[str]] = None,
) -> Tuple[List[str], Set[str]]:
    """Collect code reachable through `self.<name>(...)` and free `<name>(self`."""
    if _seen is None:
        _seen = set()
    if method in _seen:
        return [], _seen
    _seen.add(method)
    body = bodies.get(method, "")
    snippets = [body]
    for target in delegate_re.findall(body):
        if target in bodies and target not in _seen:
            sub_snippets, _seen = _collect_chain(target, bodies, os_key, _seen)
            snippets.extend(sub_snippets)
    for target in free_call_re.findall(body):
        if target in bodies and target not in _seen:
            sub_snippets, _seen = _collect_chain(target, bodies, os_key, _seen)
            snippets.extend(sub_snippets)
    return snippets, _seen


def classify_method(method: str, bodies: Dict[str, str], os_key: str) -> Tuple[str, str]:
    if method not in bodies:
        return MISSING, "method not defined by this OS backend"

    snippets, _ = _collect_chain(method, bodies, os_key)
    joined = "\n".join(snippets)

    # Empty / pure return-0 shell.
    stripped = re.sub(r"//[^\n]*", "", joined)
    stripped = re.sub(r"/\*.*?\*/", "", stripped, flags=re.DOTALL)
    if not stripped.strip():
        return PLACEHOLDER, "empty body"
    if re.fullmatch(r"\s*return\s+0\s*;\s*", stripped):
        return PLACEHOLDER, "returns 0 only"

    has_state = bool(state_insert_re.search(joined))
    has_native = any(tok in joined for tok in OS_NATIVE_TOKENS.get(os_key, []))

    # Feature-gated native backends always insert logical state first; grade the
    # default (state) runtime rather than the cfg-gated native path.
    if os_key in STATE_FIRST_OS:
        if has_state:
            note = "logical state insert (gtk-native path is cfg-gated)"
            if has_native:
                return STATE_BACKED, note + "; native path exists under feature"
            return STATE_BACKED, "logical state insert only"
        if has_native:
            return NATIVE, f"native signal `{next(t for t in OS_NATIVE_TOKENS[os_key] if t in joined)}`"
        return UNCLASSIFIABLE, "code present but no native/state signal recognized"

    if has_native:
        token = next(t for t in OS_NATIVE_TOKENS.get(os_key, []) if t in joined)
        return NATIVE, f"native signal `{token}`"
    if has_state:
        return STATE_BACKED, "logical state insert only"
    return UNCLASSIFIABLE, "code present but no native/state signal recognized"


def scan_os(os_key: str, files: List[str], canonical: Set[str]) -> OsBackend:
    # Full chain bodies include every `fn` in primary + companion files so
    # `_impl` / `try_create_*` delegate targets can be followed.
    chain_bodies: Dict[str, str] = {}
    primary_bodies: Dict[str, str] = {}
    for idx, rel in enumerate(files):
        path = pathlib.Path(rel)
        if not path.exists():
            continue
        source = path.read_text(encoding="utf-8")
        chain_bodies.update(extract_method_bodies(source, any_fn_re))
        if idx == 0:
            primary_bodies = extract_method_bodies(source, fn_re)

    # Only report control create methods (canonical contract) that this OS
    # actually defines; stray non-control `create_*` helpers are ignored.
    reported = sorted(set(primary_bodies.keys()) & canonical)
    methods: Dict[str, str] = {}
    details: Dict[str, str] = {}
    for method in reported:
        grade, note = classify_method(method, chain_bodies, os_key)
        methods[method] = grade
        details[method] = note
    return OsBackend(
        key=os_key,
        files=[f for f in files if pathlib.Path(f).exists()],
        methods=methods,
        details=details,
    )


def render(backends: List[OsBackend], output: Optional[pathlib.Path]) -> Tuple[str, int]:
    now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines: List[str] = []
    lines.append("# Per-OS platform_impl implementation-grade matrix")
    lines.append("")
    lines.append(f"Generated at: {now}")
    lines.append(
        "> Source-of-truth scan of each `src/platform/<os>/**` `platform_impl` "
        "`create_*` methods (P1 honesty gate)."
    )
    lines.append(
        "> Grades: Native (creates a real platform object) · StateBacked (logical "
        "state only) · Placeholder · Missing · Unclassifiable (needs maintainer)."
    )
    lines.append("")

    summary_counts: Dict[str, Dict[str, int]] = {}
    all_methods: Set[str] = set()
    for b in backends:
        all_methods.update(b.methods.keys())
        counts: Dict[str, int] = {}
        for g in b.methods.values():
            counts[g] = counts.get(g, 0) + 1
        summary_counts[b.key] = counts

    lines.append("## Per-OS summary")
    lines.append("| OS backend | Native | StateBacked | Placeholder | Missing* | Unclassifiable |")
    lines.append("|---|---|---|---|---|---|")
    for b in backends:
        c = summary_counts[b.key]
        lines.append(
            f"| {b.key} | {c.get(NATIVE, 0)} | {c.get(STATE_BACKED, 0)} | "
            f"{c.get(PLACEHOLDER, 0)} | {c.get(MISSING, 0)} | {c.get(UNCLASSIFIABLE, 0)} |"
        )
    lines.append("")
    lines.append(
        "* Missing = the backend does not define this `create_*`; the `Platform` "
        "trait default (`0`/state) is used for those rows."
    )
    lines.append("")

    # Widget-kind-aligned detail table (row per method defined by any OS, so
    # reports line up with the capability matrix).
    lines.append("## Widget-kind alignment (per-OS create method)")
    method_rows: Dict[str, Dict[str, str]] = {}
    for b in backends:
        for method, grade in b.methods.items():
            method_rows.setdefault(method, {})[b.key] = grade

    header = "| Create method | " + " | ".join(b.key for b in backends) + " |"
    sep = "|" + "---|" * (len(backends) + 1)
    lines.append(header)
    lines.append(sep)
    for method in sorted(method_rows.keys()):
        cells = [method_rows[method].get(b.key, MISSING) for b in backends]
        lines.append(f"| `{method}` | " + " | ".join(cells) + " |")
    lines.append("")

    lines.append("## Unclassifiable hotspots (gate)")
    unclass = []
    for b in backends:
        for method, grade in b.methods.items():
            if grade == UNCLASSIFIABLE:
                unclass.append(f"{b.key}:{method} ({b.details.get(method, '')})")
    if unclass:
        lines.append("The following implemented methods have no recognized native/state signal:")
        for u in sorted(unclass):
            lines.append(f"- {u}")
        lines.append("")
    else:
        lines.append("None — every implemented create method is classified.")
        lines.append("")

    text = "\n".join(lines)
    if output:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text, encoding="utf-8")
        print(f"per-OS platform_impl matrix written to {output.resolve()}")
    else:
        print(text)
    return text, len(unclass)


def camel_to_snake(name: str) -> str:
    parts: List[str] = []
    start = 0
    for idx in range(1, len(name)):
        if name[idx].isupper() and (not name[idx - 1].isupper()):
            parts.append(name[start:idx].lower())
            start = idx
    parts.append(name[start:].lower())
    return "_".join(parts)


def kind_to_method(kind: str) -> str:
    return KIND_METHOD_OVERRIDES.get(kind, "create_" + camel_to_snake(kind))


def parse_canonical_methods() -> Set[str]:
    path = pathlib.Path(TRAIT_SOURCE)
    if not path.exists():
        return set()
    # Trait declarations have no `{...}` body, so use a simple findall over
    # `fn create_*(...)` signatures instead of brace matching.
    text = path.read_text(encoding="utf-8")
    return set(fn_re.findall(text))


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan per-OS platform_impl grades")
    parser.add_argument("--output", default="target/qa/platform_impl_matrix.md")
    parser.add_argument(
        "--fail-on-unclassifiable",
        action="store_true",
        help="Return non-zero when any implemented control create method is unclassifiable",
    )
    args = parser.parse_args()

    canonical = parse_canonical_methods()
    backends = [scan_os(key, files, canonical) for key, files in OS_SOURCES.items()]
    _, unclass_count = render(backends, pathlib.Path(args.output))
    if args.fail_on_unclassifiable and unclass_count:
        print(f"error: {unclass_count} unclassifiable implemented create methods", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
