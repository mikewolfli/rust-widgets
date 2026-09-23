#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generates `control.md` — every control's SVG snapshots, grouped by family.

# Why a generator rather than a hand-written page

The 188 controls and their two snapshots each are produced by
`examples/export_control_svgs.rs`, and the set of controls is derived from the widget
registry. A hand-maintained index would drift the moment a control was added or renamed,
and the drift is silent: the page would simply stop mentioning a control. Generating it
makes the index a *view* of the registry rather than a second copy of it (rule #101).

# How a control's family is decided

By the module that implements it, read from the source tree: `src/widget/<family>/<file>.rs`
for a control in a subdirectory, or `src/widget/<file>.rs` for one of the files directly
under `src/widget/`. The family is therefore the same grouping the code itself uses, and a
control that moves gets reclassified by regenerating this page rather than by editing a
table.

Usage:
    python3 tools/generate_control_index.py
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SNAPSHOTS = ROOT / "snapshots" / "svg"
WIDGET_SRC = ROOT / "src" / "widget"
DEFAULT_OUTPUT = ROOT / "control.md"

# The families, in the order a reader wants them: the controls a form is built from first,
# then the chrome around them, then the specialised families. Anything not listed keeps its
# directory name and sorts after these, so a new family cannot be silently dropped.
FAMILY_ORDER = [
    "base_widgets",
    "input_widgets",
    "display_widgets",
    "container_widgets",
    "nav_widgets",
    "menu_toolbar",
    "dialog",
    "overlay_widgets",
    "view_widgets",
    "chart_widgets",
    "special_widgets",
    "advanced_widgets",
    "media_widgets",
    "misc_widgets",
    "cupertino",
    "web_widgets",
]

FAMILY_TITLES = {
    "base_widgets": "Base controls",
    "input_widgets": "Input controls",
    "display_widgets": "Display controls",
    "container_widgets": "Containers",
    "nav_widgets": "Navigation",
    "menu_toolbar": "Menus and toolbars",
    "dialog": "Dialogs",
    "overlay_widgets": "Overlays",
    "view_widgets": "Views",
    "chart_widgets": "Charts",
    "special_widgets": "Specialised controls",
    "advanced_widgets": "Advanced controls",
    "media_widgets": "Media and web",
    "misc_widgets": "Miscellaneous",
    "cupertino": "Cupertino (iOS-style)",
    "web_widgets": "Web",
    "root": "Core controls",
}


def alias_names() -> dict[str, str]:
    """Maps every factory name (canonical and alias) to its canonical name.

    Read out of the capability tables in `src/widget/capability/properties.rs`, whose
    `canonical_name: "..."` / `aliases: &["..."]` entries are the same strings
    `WidgetFactory` resolves. Reading them here rather than maintaining a second list is
    what keeps this page's names in step with the registry.
    """
    source = (ROOT / "src" / "widget" / "capability" / "properties.rs").read_text(
        encoding="utf-8"
    )
    mapping: dict[str, str] = {}
    for block in source.split("canonical_name:")[1:]:
        canonical = re.match(r'\s*"([a-z0-9_]+)"', block)
        if not canonical:
            continue
        name = canonical.group(1)
        mapping[name] = name
        aliases = re.search(r"aliases:\s*&\[([^\]]*)\]", block)
        if aliases:
            for alias in re.findall(r'"([a-z0-9_]+)"', aliases.group(1)):
                mapping.setdefault(alias, name)
    return mapping


def control_families() -> dict[str, str]:
    """Maps every factory name to the family directory that implements it.

    A control's canonical name and its module name are often different — `spin_box` lives in
    `spinbox.rs`, `tab_widget` in `tabwidget.rs`, `code_editor` in `code_editor/` — so the
    match is made on a **normalised** form of both (lower case, separators removed) and then
    widened with the capability table's aliases. A name that still matches nothing is
    reported by `main`, because silently dropping it is exactly the kind of gap this page
    exists to make visible.
    """
    files = source_families()
    types = declared_types()
    targets = constructor_targets()
    built = constructed_types()
    families: dict[str, str] = {}
    for name, canonical in alias_names().items():
        # Most specific first: the declared type of whatever the constructor builds, then the
        # module named after the control, then the control's own name.
        constructor = targets.get(canonical)
        constructed = resolve_constructor(built, constructor) if constructor else None
        family = types.get(normalise(constructed)) if constructed else None
        if family is None:
            family = files.get(normalise(canonical))
        if family is None:
            family = files.get(normalise(name))
        if family:
            families[name] = family
            families.setdefault(canonical, family)
    return families


def normalise(name: str) -> str:
    """Lower case with separators removed, so `spin_box` and `spinbox` agree."""
    return name.lower().replace("_", "").replace("-", "")


def constructor_targets() -> dict[str, str]:
    """Maps a factory name to the constructor function that builds it.

    `src/widget/capability/registration.rs` is the one place a canonical name is bound to a
    creation function, so reading it here means this page follows the registry rather than a
    copy of it.
    """
    source = (ROOT / "src" / "widget" / "capability" / "registration.rs").read_text(
        encoding="utf-8"
    )
    targets: dict[str, str] = {}
    # `self.register(<name>_capability(), create_<name>);` — the two arguments may be on
    # separate lines, which is why the whitespace class spans newlines.
    for capability, constructor in re.findall(
        r"self\.register\(\s*([a-z0-9_]+)_capability\(\)\s*,\s*([a-z0-9_]+)\s*,?\s*\)",
        source,
        re.S,
    ):
        targets.setdefault(capability, constructor)
    return targets


def constructed_types() -> dict[str, str]:
    """Maps a constructor function to the widget type it constructs.

    Several factory names are **aliases of another control** (`panel` builds a `GroupBox`,
    `table` builds a `TableWidget`, `data_view` builds a virtual list). Their family is the
    family of the type actually constructed, which is the only answer that tells a reader
    where the drawing code lives. The first `Xxx::new(` or `Xxx::with_` in the function body
    is that type.
    """
    source = (ROOT / "src" / "widget" / "capability" / "constructors.rs").read_text(
        encoding="utf-8"
    )
    bodies = re.split(r"\npub fn ", source)
    types: dict[str, str] = {}
    for body in bodies:
        name = re.match(r"([a-z0-9_]+)", body)
        if not name:
            continue
        # `Box::new(Xxx::new(..))` names the constructed type inside the box; the plain
        # `Xxx::new` form is the fallback for a constructor that returns the widget directly.
        boxed = re.search(r"Box::new\(\s*([A-Z][A-Za-z0-9_]*)\s*::", body)
        direct = re.search(r"([A-Z][A-Za-z0-9_]*)\s*::\s*(?:new|with_)", body)
        constructed = boxed or direct
        if constructed:
            types[name.group(1)] = constructed.group(1)
        else:
            # A constructor that delegates to another one (`data_view` builds a virtual
            # list) states its type one hop away; follow that hop. The function's own name is
            # excluded, or a recursive-looking match would resolve it to itself.
            own = name.group(1)
            delegate = next(
                (
                    candidate
                    for candidate in re.findall(r"\b(create_[a-z0-9_]+)\(", body)
                    if candidate != own
                ),
                None,
            )
            if delegate:
                types[own] = delegate
    return types


def declared_types() -> dict[str, str]:
    """Maps a declared widget type to the family directory of its source file.

    A control's type name and its file name need not agree — `CupertinoSlider`,
    `MaterialSnackbar` and `CupertinoAlertDialog` are all declared in
    `cupertino/core.rs`. Indexing the tree by the **declared type** is therefore the mapping
    that answers the question this page asks ("where does this control's drawing code
    live?") for every control, not only the ones whose file happens to be named after them.
    """
    families: dict[str, str] = {}
    for path in WIDGET_SRC.rglob("*.rs"):
        relative = path.relative_to(WIDGET_SRC)
        family = relative.parts[0] if len(relative.parts) > 1 else "root"
        text = path.read_text(encoding="utf-8", errors="ignore")
        for declared in re.findall(r"pub struct ([A-Z][A-Za-z0-9_]*)", text):
            families.setdefault(normalise(declared), family)
        for alias in re.findall(r"pub type ([A-Z][A-Za-z0-9_]*)", text):
            families.setdefault(normalise(alias), family)
    return families


def source_families() -> dict[str, str]:
    """Every module name mapped to its family, for the fallback match and the report.

    A control may be a single file (`button.rs`) or a directory of files
    (`code_editor/`, `toast/`), and both spellings have to resolve. The key is the
    **normalised** stem so `spin_box`, `spinbox` and `SpinBox` all agree.
    """
    families: dict[str, str] = {}
    for path in WIDGET_SRC.rglob("*"):
        if path.is_dir():
            if path == WIDGET_SRC:
                continue
            stem = path.name
        elif path.suffix == ".rs":
            if path.name in {"mod.rs", "registry.rs", "census.rs"}:
                continue
            stem = path.stem
        else:
            continue
        relative = path.relative_to(WIDGET_SRC)
        family = relative.parts[0] if len(relative.parts) > 1 else "root"
        # A directory's *own* family is the directory; its files belong to the parent family.
        if path.is_dir():
            family = relative.parts[0]
        families.setdefault(normalise(stem), family)
    return families


def resolve_constructor(types: dict[str, str], entry: str, depth: int = 0) -> str | None:
    """Follows a constructor chain to the type it ultimately builds.

    `data_view` → `create_virtual_list` → `VirtualList` is two hops, and a page that stopped
    at the first would report the control as having no source. The depth bound keeps a
    cycle (which a caller could introduce by accident) from hanging the generator.
    """
    if depth > 8:
        return None
    target = types.get(entry)
    if target is None:
        return None
    if target.startswith("create_"):
        return resolve_constructor(types, target, depth + 1)
    return target


def registry_names() -> list[str]:
    """The control names the snapshot exporter writes, taken from the files themselves.

    The snapshot set *is* the registry's published set (`export_control_svgs.rs` iterates
    `factory.widget_names()`), so reading the directory keeps this page in step with the
    registry without a second list to maintain.
    """
    names = sorted(
        path.name[: -len(".svg")]
        for path in SNAPSHOTS.glob("*.svg")
        if not path.name.endswith(".light.svg")
    )
    return names


def classify(name: str, families: dict[str, str], sources: dict[str, str]) -> str:
    """The family a control belongs to, or `unclassified` when nothing matches.

    An unclassified control is not hidden: it is listed under its own heading so the gap is
    visible, which is the opposite of the silent drift a hand-written table produces.
    """
    if name in families:
        return families[name]
    # A normalised stem match catches a control whose module name happens to equal it.
    if normalise(name) in sources:
        return sources[normalise(name)]
    return "unclassified"


def main() -> int:
    if not SNAPSHOTS.is_dir():
        print(f"error: {SNAPSHOTS} does not exist", file=sys.stderr)
        return 1
    families = control_families()
    sources = source_families()
    names = registry_names()
    if not names:
        print(f"error: no snapshots found in {SNAPSHOTS}", file=sys.stderr)
        return 1

    grouped: dict[str, list[str]] = {}
    unclassified: list[str] = []
    for name in names:
        family = classify(name, families, sources)
        if family == "unclassified":
            unclassified.append(name)
        grouped.setdefault(family, []).append(name)

    ordered = [family for family in FAMILY_ORDER if family in grouped]
    ordered += sorted(family for family in grouped if family not in FAMILY_ORDER)

    out: list[str] = []
    out.append("<!-- Generated by tools/generate_control_index.py — do not hand-edit. -->")
    out.append("")
    out.append("# 控件图像总览 / Control gallery")
    out.append("")
    out.append(
        f"本仓一共有 **{len(names)}** 个控件，每个控件有两个外观（深色 / 浅色），"
        f"共 **{len(names) * 2}** 张 SVG。"
    )
    out.append("")
    out.append(
        "这些图片由 `examples/export_control_svgs.rs` 从**控件注册表**导出，"
        "并由 `tools/check_svg_snapshots.sh` 门禁保证「重新生成的结果与提交的字节完全一致」。"
        "因此它们是**产物**而不是手绘插图：任何控件的绘制改动都会在这里以 diff 的形式出现。"
    )
    out.append("")
    out.append("| | |")
    out.append("|---|---|")
    out.append("| 画布 | 240 x 120（与渲染普查的 `CENSUS_RECT` 相同，因此控件之间可横向对比） |")
    out.append("| 深色 | `snapshots/svg/<name>.svg` |")
    out.append("| 浅色 | `snapshots/svg/<name>.light.svg` |")
    out.append("| 重新生成 | `cargo run --no-default-features --features desktop --example export_control_svgs` |")
    out.append(
        "| 分组依据 | 控件实现所在的模块目录（`src/widget/<family>/`），由本脚本自动推导 |"
    )
    out.append("")
    out.append("## 分组统计 / Families")
    out.append("")
    out.append("| 分组 | 控件数 | 本节 |")
    out.append("|---|---|---|")
    for family in ordered:
        title = FAMILY_TITLES.get(family, family)
        anchor = anchor_for(title)
        out.append(f"| {title} (`{family}`) | {len(grouped[family])} | [跳转](#{anchor}) |")
    out.append(f"| **合计** | **{len(names)}** | |")
    out.append("")

    for family in ordered:
        title = FAMILY_TITLES.get(family, family)
        controls = grouped[family]
        out.append(f"## {title}")
        out.append("")
        out.append(f"`{family}` — {len(controls)} 个控件。")
        out.append("")
        for name in controls:
            out.append(f"### `{name}`")
            out.append("")
            out.append(f"![{name} (dark)](snapshots/svg/{name}.svg)")
            out.append("")
            out.append(f"![{name} (light)](snapshots/svg/{name}.light.svg)")
            out.append("")

    output = DEFAULT_OUTPUT
    output.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"wrote {output.relative_to(ROOT)}: {len(names)} controls in {len(ordered)} families")
    for family in ordered:
        print(f"  {family}: {len(grouped[family])}")
    if unclassified:
        print(
            f"\n{len(unclassified)} control(s) matched no source module — they are listed under "
            "their own heading, and a reader should treat that as a gap to close:",
            file=sys.stderr,
        )
        for name in unclassified:
            print(f"  {name}", file=sys.stderr)
    return 0


def anchor_for(title: str) -> str:
    """The GitHub anchor for a heading, so the summary links actually land."""
    slug = title.strip().lower()
    slug = re.sub(r"[^\w\s\u4e00-\u9fff-]", "", slug)
    return slug.replace(" ", "-")


if __name__ == "__main__":
    raise SystemExit(main())
