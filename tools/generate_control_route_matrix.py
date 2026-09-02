#!/usr/bin/env python3
"""Generate control-route and implementation-grade matrix from source code.

This report is used by BLUE9 R6 as a verifiable baseline for backend routing
coverage, backend implementation quality, and profile-grade visibility.
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Optional, Set, Tuple


enum_re = re.compile(r"pub\s+enum\s+WidgetKind\s*\{(?P<body>.*?)\n\}", re.DOTALL)
variant_re = re.compile(r"^\s*([A-Za-z][A-Za-z0-9_]*)\s*,\s*$")
kind_use_re = re.compile(r"WidgetKind::([A-Za-z][A-Za-z0-9_]*)")
fn_create_re = re.compile(r"\bfn\s+(create_[a-z0-9_]+)\s*\(")
platform_delegate_re = re.compile(r"get_platform\(\)\.(create_[a-z0-9_]+)\s*\(")


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
    "WebEngineView": "create_web_engine_view",
    "WebEnginePage": "create_web_engine_page",
    "WebEngineSettings": "create_web_engine_settings",
    "WebEngineDownloadItem": "create_web_engine_download_item",
    "WebEngineCookieStore": "create_web_engine_cookie_store",
    "WebEngineWebChannel": "create_web_engine_web_channel",
    "WebEngineFindTextResult": "create_web_engine_find_text_result",
    "WebEngineNotification": "create_web_engine_notification",
    "WebEngineScriptDialog": "create_web_engine_script_dialog",
    "WebEngineContextMenuRequest": "create_web_engine_context_menu_request",
    "ToolButton": "create_tool_button",
    "ToolBox": "create_tool_box",
    "MenuItem": "create_action",
    "FreeformShape": "create_canvas",
    "PopupWindow": "create_popup_window",
    "MessageBox": "create_message_box",
    "FileDialog": "create_file_dialog",
    "ColorDialog": "create_color_dialog",
    "FontDialog": "create_font_dialog",
    # Frame is a type alias for GroupBox — native creates it via create_group_box.
    "Frame": "create_group_box",
    "InputDialog": "create_dialog",
    "ProgressDialog": "create_dialog",
    "PieMenu": "create_menu",
    "RibbonBar": "create_panel",
    "TabBar": "create_tab_widget",
}


MANUAL_PLATFORM_DELEGATE_ACCEPT: Dict[str, Set[str]] = {
    # Explicit alias/fallback delegates considered state-backed but valid.
    "create_dialog": {"create_message_box"},
    "create_popup_window": {"create_window"},
    "create_text_edit": {"create_line_edit"},
    "create_rich_edit": {"create_line_edit"},
    "create_tree_view": {"create_list_box"},
    "create_scroll_bar": {"create_slider"},
    "create_scroll_area": {"create_panel"},
    "create_dock_panel": {"create_panel"},
}


@dataclass(frozen=True)
class MatrixRow:
    kind: str
    expected_create_method: str
    trait_has_method: bool
    route_preference: str
    native_delegate: str
    native_grade: str
    custom_grade: str
    hybrid_grade: str
    native_strict_grade: str
    custom_full_grade: str


def parse_widget_kinds(kind_rs: pathlib.Path) -> List[str]:
    text = kind_rs.read_text(encoding="utf-8")
    m = enum_re.search(text)
    if not m:
        raise ValueError("Could not find WidgetKind enum in kind.rs")

    variants: List[str] = []
    for raw_line in m.group("body").splitlines():
        line = raw_line.split("//", 1)[0].rstrip()
        vm = variant_re.match(line)
        if vm:
            variants.append(vm.group(1))

    if not variants:
        raise ValueError("Parsed 0 WidgetKind variants")
    return variants


def parse_route_preferences(routing_rs: pathlib.Path) -> Dict[str, str]:
    text = routing_rs.read_text(encoding="utf-8")
    fn_start = text.find("pub fn route_preference_for_widget_kind")
    # The tests module may be gated (e.g. `#[cfg(all(test, not(feature = "mini")))]`)
    # or plain (`#[cfg(test)]`), so anchor on the `mod tests` declaration itself.
    tests_mod = text.find("mod tests", fn_start)
    if fn_start < 0 or tests_mod < 0 or tests_mod <= fn_start:
        raise ValueError("Could not isolate route_preference_for_widget_kind body")

    body = text[fn_start:tests_mod]

    result: Dict[str, str] = {}
    segment_re = re.compile(
        r"(?P<arms>(?:.|\n)*?)=>\s*ControlRoutePreference::(?P<pref>NativePreferred|CustomRequired)",
        re.DOTALL,
    )

    for seg in segment_re.finditer(body):
        pref = seg.group("pref")
        arms = seg.group("arms")
        for kind in kind_use_re.findall(arms):
            if kind in result and result[kind] != pref:
                raise ValueError(
                    f"WidgetKind::{kind} mapped to conflicting preferences: "
                    f"{result[kind]} vs {pref}"
                )
            result[kind] = pref

    if not result:
        raise ValueError("Parsed 0 routed WidgetKind preferences")

    return result


def camel_to_snake(name: str) -> str:
    # Convert CamelCase to snake_case, preserving known acronyms in overrides.
    parts: List[str] = []
    start = 0
    for idx in range(1, len(name)):
        if name[idx].isupper() and (not name[idx - 1].isupper()):
            parts.append(name[start:idx].lower())
            start = idx
    parts.append(name[start:].lower())
    return "_".join(parts)


def kind_to_create_method(kind: str) -> str:
    override = KIND_METHOD_OVERRIDES.get(kind)
    if override:
        return override
    return f"create_{camel_to_snake(kind)}"


def extract_method_bodies(source: str) -> Dict[str, str]:
    methods: Dict[str, str] = {}
    i = 0
    while i < len(source):
        m = fn_create_re.search(source, i)
        if not m:
            break

        method = m.group(1)
        start = m.start()

        open_idx = source.find("{", m.end())
        if open_idx < 0:
            i = m.end()
            continue

        brace_count = 1
        j = open_idx + 1
        while j < len(source) and brace_count > 0:
            ch = source[j]
            if ch == "{":
                brace_count += 1
            elif ch == "}":
                brace_count -= 1
            j += 1

        methods[method] = source[start:j]
        i = j

    return methods


def parse_native_delegates(native_rs: pathlib.Path) -> Tuple[Set[str], Dict[str, str]]:
    text = native_rs.read_text(encoding="utf-8")
    bodies = extract_method_bodies(text)
    methods = set(bodies.keys())
    delegates: Dict[str, str] = {}

    for method, body in bodies.items():
        pm = platform_delegate_re.search(body)
        if pm:
            delegates[method] = pm.group(1)

    return methods, delegates


def parse_custom_methods(custom_rs: pathlib.Path) -> Set[str]:
    # The custom backend's method bodies live in per-category include files
    # (create_widgets_*.in.rs) that are expanded via `macro_rules!` inside
    # `create_widgets.rs`. Scan the whole directory so grades reflect the
    # real implementations instead of the empty macro-invocation shell.
    methods: Set[str] = set()
    files = [custom_rs, *sorted(custom_rs.parent.glob("create_widgets_*.in.rs"))]
    for source in files:
        if not source.exists():
            continue
        text = source.read_text(encoding="utf-8")
        methods.update(extract_method_bodies(text).keys())
    return methods


def parse_trait_methods(trait_rs: pathlib.Path) -> Set[str]:
    text = trait_rs.read_text(encoding="utf-8")
    return set(fn_create_re.findall(text))


def classify_native_grade(method: str, delegate: Optional[str]) -> str:
    if not method:
        return "Placeholder"
    if not delegate:
        return "Placeholder"
    expected_platform = method
    if delegate == expected_platform:
        return "Native"
    if delegate in MANUAL_PLATFORM_DELEGATE_ACCEPT.get(method, set()):
        return "StateBacked"
    return "StateBacked"


def classify_custom_grade(method_present: bool) -> str:
    if method_present:
        return "StateBacked"
    return "Placeholder"


def render_markdown(
    *,
    rows: List[MatrixRow],
    missing: List[str],
    source_kind: pathlib.Path,
    source_route: pathlib.Path,
    source_native: pathlib.Path,
    source_custom: pathlib.Path,
    source_trait: pathlib.Path,
) -> str:
    now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    native_pref = sum(1 for row in rows if row.route_preference == "NativePreferred")
    custom_req = sum(1 for row in rows if row.route_preference == "CustomRequired")
    native_grade_counts: Dict[str, int] = {}
    custom_grade_counts: Dict[str, int] = {}
    hybrid_grade_counts: Dict[str, int] = {}
    for row in rows:
        native_grade_counts[row.native_grade] = native_grade_counts.get(row.native_grade, 0) + 1
        custom_grade_counts[row.custom_grade] = custom_grade_counts.get(row.custom_grade, 0) + 1
        hybrid_grade_counts[row.hybrid_grade] = hybrid_grade_counts.get(row.hybrid_grade, 0) + 1
    contract_missing = [row for row in rows if not row.trait_has_method]

    lines: List[str] = []
    lines.append("# rust_widgets control-route matrix")
    lines.append("")
    lines.append(f"Generated at: {now}")
    lines.append(f"Source enum: `{source_kind}`")
    lines.append(f"Source routing: `{source_route}`")
    lines.append(f"Source native backend: `{source_native}`")
    lines.append(f"Source custom backend: `{source_custom}`")
    lines.append(f"Source trait contract: `{source_trait}`")
    lines.append("")
    lines.append("## Summary")
    lines.append(f"- Total WidgetKind variants: {len(rows)}")
    lines.append(f"- NativePreferred: {native_pref}")
    lines.append(f"- CustomRequired: {custom_req}")
    lines.append(f"- Missing route mappings: {len(missing)}")
    lines.append(
        f"- Native backend grades: Native={native_grade_counts.get('Native', 0)}, "
        f"StateBacked={native_grade_counts.get('StateBacked', 0)}, "
        f"Placeholder={native_grade_counts.get('Placeholder', 0)}"
    )
    lines.append(
        f"- Custom backend grades: StateBacked={custom_grade_counts.get('StateBacked', 0)}, "
        f"Placeholder={custom_grade_counts.get('Placeholder', 0)}"
    )
    lines.append(
        f"- Hybrid (desktop) grades: Native={hybrid_grade_counts.get('Native', 0)}, "
        f"StateBacked={hybrid_grade_counts.get('StateBacked', 0)}, "
        f"Placeholder={hybrid_grade_counts.get('Placeholder', 0)}"
    )
    lines.append(f"- Missing trait create-method contracts: {len(contract_missing)}")
    lines.append("")
    lines.append("## Policy-to-grade rules")
    lines.append("- `hybrid-native-first`: NativePreferred 取 native grade；CustomRequired 取 custom grade。")
    lines.append("- `native-strict`: 全部使用 native grade。")
    lines.append("- `custom-full`: 全部使用 custom grade。")
    lines.append("- `Placeholder`: 对应后端缺失 create 方法或无法解析有效委托。")
    lines.append("")

    if missing:
        lines.append("## Unmapped WidgetKind variants")
        for kind in missing:
            lines.append(f"- `{kind}`")
        lines.append("")

    if contract_missing:
        lines.append("## Missing trait contract hotspots")
        lines.append("| WidgetKind | Expected Create Method |")
        lines.append("|---|---|")
        for row in contract_missing:
            lines.append(f"| {row.kind} | {row.expected_create_method} |")
        lines.append("")

    placeholder_rows = [
        row
        for row in rows
        if row.native_grade == "Placeholder"
        or row.custom_grade == "Placeholder"
        or row.hybrid_grade == "Placeholder"
    ]
    if placeholder_rows:
        lines.append("## Placeholder risk hotspots")
        lines.append("| WidgetKind | Expected Create Method | Native Grade | Custom Grade | Hybrid Grade |")
        lines.append("|---|---|---|---|---|")
        for row in placeholder_rows:
            lines.append(
                f"| {row.kind} | {row.expected_create_method} | {row.native_grade} | "
                f"{row.custom_grade} | {row.hybrid_grade} |"
            )
        lines.append("")

    lines.append("## Widget matrix")
    lines.append(
        "| WidgetKind | Expected Create Method | In Trait Contract | Route Preference | Native Delegate | "
        "Native Grade | Custom Grade | Hybrid (desktop) | Native Strict | Custom Full |"
    )
    lines.append("|---|---|---|---|---|---|---|---|---|---|")
    for row in rows:
        lines.append(
            f"| {row.kind} | {row.expected_create_method} | {'yes' if row.trait_has_method else 'no'} | {row.route_preference} | "
            f"{row.native_delegate} | {row.native_grade} | {row.custom_grade} | {row.hybrid_grade} | "
            f"{row.native_strict_grade} | {row.custom_full_grade} |"
        )

    lines.append("")
    lines.append("## Notes")
    lines.append("- This report is generated from source-of-truth enums, routing match arms, and backend impls.")
    lines.append("- Native grade is inferred from `create_*` delegate target equivalence analysis.")
    lines.append("- Use together with `feature_completeness_matrix.md` for remediation prioritization.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate control route matrix report")
    parser.add_argument("--kind", default="src/widget/kind.rs", help="Path to WidgetKind enum file")
    parser.add_argument(
        "--routing",
        default="src/control_backend/routing.rs",
        help="Path to route_preference_for_widget_kind source file",
    )
    parser.add_argument(
        "--output",
        default="target/qa/control_route_matrix.md",
        help="Output markdown path",
    )
    parser.add_argument(
        "--native",
        default="src/control_backend/native.rs",
        help="Path to native backend source file",
    )
    parser.add_argument(
        "--custom",
        default="src/control_backend/custom/create_widgets.rs",
        help="Path to custom backend source file",
    )
    parser.add_argument(
        "--trait",
        default="src/control_backend/trait_def/trait_def.rs",
        help="Path to ControlBackend trait source file",
    )
    parser.add_argument(
        "--fail-on-placeholder",
        action="store_true",
        help="Return non-zero when any hybrid grade is Placeholder",
    )
    parser.add_argument(
        "--fail-on-contract-miss",
        action="store_true",
        help="Return non-zero when expected create method is not defined in trait contract",
    )
    args = parser.parse_args()

    kind_file = pathlib.Path(args.kind)
    routing_file = pathlib.Path(args.routing)
    native_file = pathlib.Path(args.native)
    custom_file = pathlib.Path(args.custom)
    trait_file = pathlib.Path(args.trait)
    output_file = pathlib.Path(args.output)

    kinds = parse_widget_kinds(kind_file)
    preference_map = parse_route_preferences(routing_file)
    native_methods, native_delegates = parse_native_delegates(native_file)
    custom_methods = parse_custom_methods(custom_file)
    trait_methods = parse_trait_methods(trait_file)

    missing = sorted(kind for kind in kinds if kind not in preference_map)

    rows: List[MatrixRow] = []
    for kind in sorted(kinds):
        expected_method = kind_to_create_method(kind)
        pref = preference_map.get(kind, "UNMAPPED")

        method_in_native = expected_method in native_methods
        native_delegate = native_delegates.get(expected_method)
        native_grade = classify_native_grade(
            expected_method if method_in_native else "",
            native_delegate,
        )

        method_in_custom = expected_method in custom_methods
        custom_grade = classify_custom_grade(method_in_custom)

        if pref == "UNMAPPED":
            rows.append(
                MatrixRow(
                    kind=kind,
                    expected_create_method=expected_method,
                    trait_has_method=(expected_method in trait_methods),
                    route_preference=pref,
                    native_delegate="(unmapped)",
                    native_grade="Placeholder",
                    custom_grade="Placeholder",
                    hybrid_grade="Placeholder",
                    native_strict_grade="Placeholder",
                    custom_full_grade="Placeholder",
                )
            )
            continue

        if pref == "NativePreferred":
            hybrid_grade = native_grade
        else:
            hybrid_grade = custom_grade

        rows.append(
            MatrixRow(
                kind=kind,
                expected_create_method=expected_method,
                trait_has_method=(expected_method in trait_methods),
                route_preference=pref,
                native_delegate=native_delegate if native_delegate else "(none)",
                native_grade=native_grade,
                custom_grade=custom_grade,
                hybrid_grade=hybrid_grade,
                native_strict_grade=native_grade,
                custom_full_grade=custom_grade,
            )
        )

    report = render_markdown(
        rows=rows,
        missing=missing,
        source_kind=kind_file,
        source_route=routing_file,
        source_native=native_file,
        source_custom=custom_file,
        source_trait=trait_file,
    )

    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(report, encoding="utf-8")

    print(f"control route matrix report written to {output_file.resolve()}")
    if missing:
        print("unmapped WidgetKind variants detected:", ", ".join(missing))
        return 2

    hybrid_placeholder = [row.kind for row in rows if row.hybrid_grade == "Placeholder"]
    if args.fail_on_placeholder and hybrid_placeholder:
        print(
            "hybrid placeholder widgets detected:",
            ", ".join(sorted(hybrid_placeholder)),
        )
        return 3

    contract_miss = sorted(
        row.kind for row in rows if not row.trait_has_method
    )
    if args.fail_on_contract_miss and contract_miss:
        print(
            "trait contract missing expected create methods for:",
            ", ".join(contract_miss),
        )
        return 4

    return 0


if __name__ == "__main__":
    sys.exit(main())
