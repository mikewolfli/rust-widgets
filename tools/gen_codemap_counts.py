#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate (or verify) the codemap §9 file-count table (issue §8 direction 5).

# Why this exists

The table drifted twice: "Layout managers 14" when the derived count was 15, and "Benchmark
files 5" when `Cargo.toml` had 9. Each row was a hand-typed number with a derivation written
next to it, which is the arrangement that guarantees the next change updates one and not the
other. This script *is* the derivation: `--check` compares the committed table, and a plain run
rewrites it.

# Definitions (the table's own header says what each row measures)

  * Rust source files     — `src/**/*.rs`
  * Widget kinds          — `WidgetKind` enum variants in `src/widget/kind.rs`
  * Layout managers       — `impl Layout for` targets in `src/layout`, excluding the internal
                            `ScaleProbe` adapter and `Wrapper` specialisation
  * Platform backends     — directories under `src/platform`, excluding `types`
  * IME implementations   — `src/platform/ime_*.rs`
  * Clipboard backends    — `impl RichClipboardBackend for` targets, excluding the test mock
  * Gesture recognisers   — `pub struct *Gesture` in `src/gesture`
  * Integration tests     — `tests/*.rs`
  * Benchmark files       — `[[bench]]` entries in `Cargo.toml`

Usage: python3 tools/gen_codemap_counts.py [--check]
Exit 0 = written / table matches. Exit 1 = --check found drift.
"""

from __future__ import annotations

import pathlib
import re
import sys

CODEMAP = pathlib.Path("docs/plans/codemap.md")
SECTION = "## 9. File Count Summary"
FAMILY_SECTION = "#### Widget Subfolders"
ROW = re.compile(r"^\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|$")

# The order a reader expects (the file lists them in dependency order, not alphabetically).
# Any family the tree has but this list does not is appended alphabetically, so a new
# subdirectory appears in the table instead of being silently omitted (W8-15).
FAMILY_ORDER = [
    "base_widgets",
    "input_widgets",
    "container_widgets",
    "display_widgets",
    "nav_widgets",
    "dialog",
    "chart_widgets",
    "media_widgets",
    "menu_toolbar",
    "overlay_widgets",
    "view_widgets",
    "special_widgets",
    "advanced_widgets",
    "cupertino",
    "macos",
    "misc_widgets",
    "companions",
    "root",
]


def read_text(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="ignore")


def variants() -> int:
    text = read_text(pathlib.Path("src/widget/kind.rs"))
    body = text[text.index("pub enum WidgetKind") :]
    body = body[: body.index("\n}")]
    return len(re.findall(r"(?m)^\s{4}([A-Z][A-Za-z0-9_]*)\s*,", body))


def layout_managers() -> int:
    names: set[str] = set()
    for path in pathlib.Path("src/layout").rglob("*.rs"):
        names.update(re.findall(r"impl\s+Layout\s+for\s+(\w+)", read_text(path)))
    return len(names - {"ScaleProbe", "Wrapper"})


def platform_backends() -> int:
    return len([p for p in pathlib.Path("src/platform").iterdir() if p.is_dir() and p.name != "types"])


def ime_implementations() -> int:
    return len(list(pathlib.Path("src/platform").glob("ime_*.rs")))


def clipboard_backends() -> int:
    names: set[str] = set()
    for path in pathlib.Path("src/platform").rglob("*.rs"):
        names.update(re.findall(r"impl\s+RichClipboardBackend\s+for\s+(\w+)", read_text(path)))
    return len(names - {"MockClipboard"})


def gestures() -> int:
    names: set[str] = set()
    for path in pathlib.Path("src/gesture").rglob("*.rs"):
        names.update(re.findall(r"pub struct\s+(\w+Gesture)\b", read_text(path)))
    return len(names)


def benchmark_files() -> int:
    return len(re.findall(r"(?m)^\[\[bench\]\]", read_text(pathlib.Path("Cargo.toml"))))


def counts() -> list[tuple[str, int]]:
    return [
        ("Rust source files", len(list(pathlib.Path("src").rglob("*.rs")))),
        ("Widget kinds (`WidgetKind` variants)", variants()),
        ("Layout managers", layout_managers()),
        ("Platform backends", platform_backends()),
        ("IME implementations", ime_implementations()),
        ("Clipboard backends", clipboard_backends()),
        ("Gesture recognizers", gestures()),
        ("Integration test files", len(list(pathlib.Path("tests").glob("*.rs")))),
        ("Benchmark files", benchmark_files()),
    ]


def render(rows: list[tuple[str, int]]) -> str:
    out = ["| Category | Count |", "|----------|-------|"]
    out += [f"| {name} | {value:,} |" for name, value in rows]
    return "\n".join(out)


def family_members() -> dict[str, list[str]]:
    """Maps each `src/widget/<family>/` directory to its registered canonical names.

    The derivation is `generate_control_index.py`'s own: the capability tables bind a
    canonical name to a constructor, the constructor's built type names a module, and the
    module's directory is the family. Reusing that derivation is what makes the codemap
    table a *view* of the registry rather than a third hand-typed census (W8-15).
    """
    import importlib.util

    path = pathlib.Path(__file__).with_name("generate_control_index.py")
    spec = importlib.util.spec_from_file_location("generate_control_index", path)
    if spec is None or spec.loader is None:  # pragma: no cover - a broken checkout
        raise SystemExit(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    aliases = module.alias_names()
    families = module.control_families()
    grouped: dict[str, list[str]] = {}
    for name, canonical in sorted(aliases.items()):
        # The mapping contains aliases too; only a canonical name is a control of its own.
        if name != canonical:
            continue
        family = families.get(name)
        if family is None:  # pragma: no cover - `generate_control_index` reports these
            raise SystemExit(f"{name} is registered but belongs to no family")
        grouped.setdefault(family, []).append(name)
    return grouped


def render_family_table(grouped: dict[str, list[str]]) -> str:
    ordered = [family for family in FAMILY_ORDER if family in grouped]
    ordered += sorted(family for family in grouped if family not in FAMILY_ORDER)
    out = [
        "| Subfolder | Count | Registered controls (canonical names, derived) |",
        "|-----------|-------|--------------------------------------------------|",
    ]
    for family in ordered:
        names = grouped[family]
        out.append(f"| `{family}/` | {len(names)} | {', '.join(names)} |")
    return "\n".join(out)


def table_bounds(text: str, start: int) -> tuple[int, int]:
    """The contiguous `|` table block after the §9 heading."""
    first = text.find("\n|", start)
    if first == -1:
        raise SystemExit("codemap §9 has no table")
    first += 1
    end = first
    while True:
        line_end = text.find("\n", end)
        if line_end == -1:
            return first, len(text)
        if not text[end:line_end].lstrip().startswith("|"):
            return first, end
        end = line_end + 1


def main() -> int:
    check = "--check" in sys.argv[1:]
    text = read_text(CODEMAP)
    start = text.find(SECTION)
    if start == -1:
        print(f"FAIL: {CODEMAP} has no `{SECTION}`")
        return 1
    family_start = text.find(FAMILY_SECTION)
    if family_start == -1:
        print(f"FAIL: {CODEMAP} has no `{FAMILY_SECTION}`")
        return 1

    first, end = table_bounds(text, start)
    committed = text[first:end]
    expected = render(counts())
    family_first, family_end = table_bounds(text, family_start)
    committed_families = text[family_first:family_end]
    expected_families = render_family_table(family_members())

    if check:
        failed = False
        if committed.rstrip() != expected.rstrip():
            print("FAIL: codemap §9 counts drifted; regenerate with")
            print("      python3 tools/gen_codemap_counts.py")
            for (name, value), line in zip(counts(), committed.splitlines()):
                if line.startswith("|") and str(value) not in line:
                    print(f"  {name}: derived {value}")
            failed = True
        if committed_families.rstrip() != expected_families.rstrip():
            print("FAIL: codemap widget-family counts drifted; regenerate with")
            print("      python3 tools/gen_codemap_counts.py")
            for line in expected_families.splitlines():
                if line.startswith("|") and line not in committed_families:
                    print(f"  derived: {line}")
            failed = True
        if failed:
            return 1
        print("✅ codemap §9 and widget-family counts match their derivations")
        return 0

    # Replace the family table first: it is earlier in the file, so the §9 offsets stay valid.
    remainder = text[family_end:].lstrip("\n")
    text = text[:family_first] + expected_families + "\n\n" + remainder
    start = text.find(SECTION)
    first, end = table_bounds(text, start)
    remainder = text[end:].lstrip("\n")
    text = text[:first] + expected + "\n\n" + remainder
    CODEMAP.write_text(text, encoding="utf-8")
    print(f"wrote {CODEMAP}: {len(counts())} file counts + {len(family_members())} families")
    return 0


if __name__ == "__main__":
    sys.exit(main())
