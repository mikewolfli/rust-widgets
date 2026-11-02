#!/usr/bin/env python3
"""TEMPORARY one-off migration helper (deleted after the 3.3.0 migration).

Lifts a widget group's capability rows, property schemas, events and constructors out of the
parent crate into `crates/rust-widgets-suites/src/<group>/capability.rs`, deleting them from the
parent as it goes. Run from the repo root: `python3 tools/_mig_tmp.py <group>`.
"""

from __future__ import annotations

import pathlib
import re
import sys
import textwrap

SRC = pathlib.Path("src")
CAP = SRC / "widget/capability"

GROUPS = {
    "maps": {
        "controls": [
            ("map_view", "Canvas"),
            ("map_overview", "Canvas"),
            ("map_attribution", "Canvas"),
            ("tile_loader", "TileLoader"),
            ("zoom_control", "ZoomControl"),
            ("scale_bar", "ScaleBar"),
            ("compass", "Compass"),
        ],
        # (parent module path, local module path)
        "local": [
            ("widget::special_widgets::map_view", "map_view"),
            ("widget::special_widgets::map_overview", "map_overview"),
            ("widget::special_widgets::map_attribution", "map_attribution"),
        ],
        "items": [
            ("MapOverviewPanel", "map_overview"),
            ("CenterMarkerStyle", "map_overview"),
            ("MapAttributionOverlay", "map_attribution"),
            ("AttributionEntry", "map_attribution"),
            ("Corner", "map_attribution"),
        ],
    },
    "project": {
        "controls": [
            ("gantt_widget", "Chart"),
            ("gantt_task_grid", "Chart"),
            ("timeline_axis", "TimelineAxis"),
            ("dependency_editor", "TimelineAxis"),
            ("kanban_board", "KanbanBoard"),
            ("kanban_card_view", "KanbanCardView"),
            ("kanban_column_header", "KanbanColumnHeader"),
        ],
        "local": [
            ("widget::chart_widgets::gantt", "gantt"),
            ("widget::chart_widgets::kanban_board", "kanban_board"),
            ("widget::special_widgets::timeline_widget", "timeline_widget"),
        ],
    },
    "devtools": {
        "controls": [
            ("terminal_view", "TerminalView"),
            ("diff_viewer", "DiffViewer"),
            ("diff_file_list", "DiffFileList"),
            ("terminal_find_bar", "SearchBar"),
        ],
        "local": [
            ("widget::special_widgets::terminal", "terminal"),
            ("widget::special_widgets::diff_viewer", "diff_viewer"),
        ],
    },
    "media_playback": {
        "controls": [
            ("video_player", "VideoPlayer"),
            ("lottie_widget", "LottieWidget"),
            ("rive_widget", "RiveWidget"),
            ("audio_visualizer", "AudioVisualizer"),
            ("camera_preview", "CameraPreview"),
            ("chapter_list_panel", "ChapterListPanel"),
            ("media_player", "WebEngineView"),
            ("media_transport_bar", "WebEngineView"),
            ("media_scrubber", "WebEngineView"),
            ("subtitle_bar", "WebEngineView"),
        ],
        "local": [
            ("widget::media_widgets::video_player", "video_player"),
            ("widget::media_widgets::lottie_widget", "lottie_widget"),
            ("widget::media_widgets::rive_widget", "rive_widget"),
            ("widget::media_widgets::audio_visualizer", "audio_visualizer"),
            ("widget::media_widgets::camera_preview", "camera_preview"),
            ("widget::media_widgets::chapter_list_panel", "chapter_list_panel"),
            ("widget::special_widgets::media_player", "media_player"),
        ],
    },
}


def brace_end(text: str, open_index: int) -> int:
    depth = 0
    i = open_index
    while i < len(text):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError("unbalanced braces")


def back_up_to_attrs(text: str, start: int) -> int:
    """Walk backwards over the attribute/doc lines directly above the line holding `start`."""
    line_start = text.rfind("\n", 0, start) + 1
    lines = text[:line_start].splitlines(keepends=True)
    k = len(lines)
    while k > 0:
        prev = lines[k - 1].strip()
        if prev.startswith("#[") or prev.startswith("///"):
            k -= 1
        else:
            break
    return sum(len(line) for line in lines[:k])


def extract_fn(text: str, sig: str) -> str:
    i = text.index(sig)
    start = back_up_to_attrs(text, i)
    end = brace_end(text, text.index("{", i))
    while end < len(text) and text[end] == "\n":
        end += 1
    return text[start:end]


def extract_const(text: str, decl: str) -> str:
    i = text.index(decl)
    start = back_up_to_attrs(text, i)
    end = text.index("];\n", text.index("= &[", i)) + 3
    while end < len(text) and text[end] == "\n":
        end += 1
    return text[start:end]


def upper(name: str) -> str:
    return name.upper()


def event_rows(name: str) -> list[tuple[str, str, str]]:
    src = (CAP / "event_payloads.rs").read_text()
    rows = re.findall(r"EventSchema \{[^}]*\}", src[src.index("static EVENT_SCHEMAS") :])
    starts = re.findall(
        r'\("([a-z_]+)", (\d+), (\d+)\)', src[src.index("static CONTROL_STARTS") :]
    )
    table = {n: (int(s), int(l)) for n, s, l in starts}
    if name not in table:
        # A control that publishes no events has no row in the derived table.
        return []
    out = []
    for row in rows[table[name][0] : table[name][0] + table[name][1]]:
        event = re.search(r'name: "([^"]+)"', row).group(1)
        payload = re.search(r"payload: (None|Some\(K::[A-Za-z]+\))", row).group(1)
        shape = re.search(r"shape: (None|Some\(S::[A-Za-z0-9]+\))", row).group(1)
        out.append((event, payload, shape))
    return out


def render_events(name: str, rows) -> str:
    if not rows:
        return ""
    body = []
    for event, payload, shape in rows:
        if payload == "None":
            body.append(f'    EventSchema::unit("{event}"),')
        elif shape == "None":
            kind = re.search(r"K::(\w+)", payload).group(1)
            body.append(f'    EventSchema::scalar("{event}", K::{kind}),')
        else:
            kind = re.search(r"K::(\w+)", payload).group(1)
            sh = re.search(r"S::(\w+)", shape).group(1)
            body.append(
                f'    EventSchema {{ name: "{event}", payload: Some(K::{kind}), '
                f"shape: Some(S::{sh}) }},"
            )
    return (
        f"/// `{name}`'s published events.\n"
        f"const {upper(name)}_EVENTS: &[EventSchema] = &[\n" + "\n".join(body) + "\n];\n"
    )


HEADER = """// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of this suite group.
//!
//! A capability is the designer's entire view of a control: the name it is created by, the
//! property contract with a type and an accessibility flag per row, and the published events
//! with their payload kinds. All of those are declared here, next to the constructors, so the
//! registration and the documentation cannot describe different controls.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::apply_control_label;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind as K,
    RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};
"""


def head_text(path) -> str:
    """The file's committed content, for blocks an earlier (interrupted) run already removed."""
    import subprocess

    return subprocess.run(
        ["git", "show", f"HEAD:{path}"], capture_output=True, text=True, check=True
    ).stdout


def main() -> int:
    group = sys.argv[1]
    cfg = GROUPS[group]
    controls = cfg["controls"]
    local = cfg["local"]

    texts: dict[str, str] = {}

    def read(path):
        key = str(path)
        if key not in texts:
            texts[key] = pathlib.Path(path).read_text()
        return texts[key]

    pending: dict[str, list[str]] = {}

    def stage_delete(path, block):
        pending.setdefault(str(path), []).append(block)

    def localize(block: str) -> str:
        for parent, name in local:
            block = block.replace(f"crate::{parent}::", f"crate::{name}::")
            block = block.replace(f"crate::{parent};", f"crate::{name};")
        block = block.replace("crate::widget::", "rust_widgets::widget::")
        block = block.replace("crate::core::", "rust_widgets::core::")
        block = block.replace("crate::compat::", "rust_widgets::compat::")
        return block

    pieces = [HEADER]
    schema_files = sorted(CAP.glob("properties_*.in.rs"))

    for canonical, _kind in controls:
        const = upper(canonical) + "_PROPERTIES"
        decl = f"const {const}: &[PropertySchema]"
        for f in schema_files:
            if decl in read(f):
                block = extract_const(read(f), decl)
                stage_delete(f, block)
                block = textwrap.dedent(block)
                block = block.replace("pub(crate) const", "pub const")
                for item, module in cfg.get("items", []):
                    block = block.replace(
                        f"rust_widgets::widget::special_widgets::{item}", f"crate::{module}::{item}"
                    )
                block = re.sub(r"^#\[cfg\([^)]*\)\]\n", "", block, flags=re.M)
                pieces.append("\n" + localize(block))
                break
            if decl in head_text(f):
                block = extract_const(head_text(f), decl)
                block = textwrap.dedent(block)
                block = block.replace("pub(crate) const", "pub const")
                for item, module in cfg.get("items", []):
                    block = block.replace(
                        f"rust_widgets::widget::special_widgets::{item}", f"crate::{module}::{item}"
                    )
                block = re.sub(r"^#\[cfg\([^)]*\)\]\n", "", block, flags=re.M)
                pieces.append("\n" + localize(block))
                print(f"  (schema {const} recovered from HEAD)")
                break
        else:
            print(f"!! schema {const} not found")

    for canonical, _kind in controls:
        pieces.append("\n" + render_events(canonical, event_rows(canonical)))

    for canonical, _kind in controls:
        sig = f"fn {canonical}_capability() -> WidgetCapability {{"
        for f in sorted((CAP / "properties").glob("*.rs")):
            if sig in read(f) or sig in head_text(f):
                source = read(f) if sig in read(f) else head_text(f)
                if source is not read(f):
                    print(f"  (capability {canonical} recovered from HEAD)")
                block = extract_fn(source, sig)
                stage_delete(f, block)
                block = block.replace(sig, f"pub fn {canonical}_capability() -> WidgetCapability {{")
                block = block.replace(f'events_of!("{canonical}")', f"{upper(canonical)}_EVENTS")
                block = re.sub(r"^#\[cfg\([^)]*\)\]\n", "", block, flags=re.M)
                pieces.append("\n" + block)
                break
        else:
            print(f"!! capability {canonical} not found")

    ctor_file = CAP / "constructors.rs"
    for canonical, _kind in controls:
        pattern = re.compile(
            rf"pub fn create_{canonical}\(geometry: Rect, _?text: &str\) -> Box<dyn Widget> \{{"
        )
        source = read(ctor_file)
        match = pattern.search(source)
        if match is None:
            source = head_text(ctor_file)
            match = pattern.search(source)
            if match is None:
                print(f"!! ctor create_{canonical} not found")
                continue
            print(f"  (ctor create_{canonical} recovered from HEAD)")
        sig = match.group(0)
        block = extract_fn(source, sig)
        stage_delete(ctor_file, block)
        block = re.sub(r"^#\[cfg\([^)]*\)\]\n", "", block, flags=re.M)
        block = localize(block)
        for item, module in cfg.get("items", []):
            block = block.replace(
                f"rust_widgets::widget::special_widgets::{item}", f"crate::{module}::{item}"
            )
        block = re.sub(
            r"label\(\s*geometry,\s*text,\s*(Box::new\((?:[^()]|\([^()]*\))*\))\s*\)",
            r"apply_control_label(\1, text)",
            block,
        )
        pieces.append("\n" + block)

    for path, blocks in pending.items():
        text = pathlib.Path(path).read_text()
        for block in blocks:
            if block in text:
                text = text.replace(block, "")
        pathlib.Path(path).write_text(text)

    out = "".join(pieces)
    target = pathlib.Path("crates/rust-widgets-suites/src") / group.replace("-", "_") / "capability.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(out)
    print(f"wrote {target} ({len(out)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
