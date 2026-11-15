#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate (or verify) `docs/plans/widget_classification.md` (issue §6.4, plan A).

# Why this is generated rather than written

`docs/issues/issue-20261115.md` §6 recorded a control classification and a scenario matrix, and
its own §6.5 admitted the weakness: the family counts were a scan-time estimate ("成员以目录 `ls`
为权威") that no longer matched the registry by the time the page was proposed. A hand-typed
census of 243 names is a number nobody re-measures — the exact arrangement `check_widget_kind_count`
and `gen_codemap_counts` were written to end.

So this page keeps **two kinds of information apart**:

  * **Facts** — the kind total, the control total, the alias count, each family's membership and
    each family's module gate — are *derived* here, from `src/widget/kind.rs`, the capability
    tables, and `src/widget/mod.rs`. None of them is typed by a person.
  * **Advice** — which family suits which device shape, and what an OS or input model costs — is
    curated below in `CURATED`. It is opinion, and the only mechanical claim made about it is that
    **every family in the tree has a row**: adding a family without answering "where does this
    belong" fails instead of silently defaulting.

`--check` compares the committed page byte for byte, so the page cannot drift from the derivation.

Usage:
    python3 tools/gen_widget_classification.py           # rewrite the page
    python3 tools/gen_widget_classification.py --check   # exit 1 when it is stale
"""

from __future__ import annotations

import importlib.util
import pathlib
import re
import sys

PAGE = pathlib.Path("docs/plans/widget_classification.md")
WIDGET_MOD = pathlib.Path("src/widget/mod.rs")

# Advice, not fact: `(mini_note, desktop, mobile, device_shape, os_note)`.
#
# `mini_note` is only used for a family whose module is compiled in the stripped profiles; a
# family gated on `full_widgets` gets a derived "does not compile" cell instead, so a profile
# change rewrites that column without anyone editing this table.
CURATED: dict[str, tuple[str, str, str, str, str]] = {
    "base_widgets": (
        "✅ 仅 `toggle_button` 随 `widgets_unstripped`",
        "✅ 全量",
        "✅ 全部可用",
        "全形态",
        "`radio_button` / `check_box` / `toggle_button` 有 touch 分支",
    ),
    "input_widgets": (
        "⚠️ 核心子集（`masked_edit` / `otp_input` / `mention` / `cascader` 等不编译）",
        "✅ 全量",
        "✅ 触控友好子集",
        "桌面全量；移动取 picker/field 子集",
        "`shortcut_editor` 强键盘语境；`keyboard` / `ime_preedit` 面向移动软键盘",
    ),
    "display_widgets": (
        "✅ 大头可用；`image_view` 的 kind 常驻但 mini 无 `image` feature → 图像源不可用",
        "✅ 全量",
        "✅ 全部可用",
        "全形态",
        "`rating` / `roller` / `switch` 有 touch 分支；`lcd_number` / `meter` 适合仪表语境",
    ),
    "container_widgets": (
        "⚠️ `scroll_area` / `panel` 可用；tab/splitter/mdi 与布局族需 `full_widgets`",
        "✅ 全量",
        "✅ 移动取 stack/scroll/safe_area",
        "桌面=dock/mdi；移动=stack/scroll",
        "`safe_area` 是移动安全区；布局族 12 个 kind 在 mini 全部不可用",
    ),
    "nav_widgets": (
        "—",
        "✅ 全量",
        "✅ 移动/平板优先",
        "移动优先",
        "`drawer` / `bottom_navigation_bar` 按触控设计；`app_bar` 同时服务桌面",
    ),
    "menu_toolbar": (
        "—",
        "✅ 全量",
        "⚠️ 建议精简（长按菜单 + 少量 action）",
        "桌面优先",
        "`menu_bar` / `ribbon_bar` 是桌面习惯；移动端更宜用 `bottom_navigation_bar`",
    ),
    "dialog": (
        "—",
        "✅ 全量（`file_dialog` / `font_dialog` 强 OS 语境）",
        "✅ 移动优先用 `bottom_sheet` / `modal_bottom_sheet`",
        "全形态",
        "`color_dialog` / `file_dialog` 桌面语境；`wizard` 适合任意形态的分步流程",
    ),
    "overlay_widgets": (
        "—",
        "✅ 全量",
        "✅ 触控件为主",
        "移动/平板",
        "`fab` / `swipe_to_dismiss` / `refresh_control` 依赖 `touch` feature",
    ),
    "view_widgets": (
        "—",
        "✅ 主力（大表强桌面语境）",
        "⚠️ 仅浏览场景",
        "桌面优先",
        "键盘 + 滚轮 + hover 是交互前提；`virtual_*` 面向大数据集",
    ),
    "chart_widgets": (
        "—",
        "✅ 含引擎路径（`chart` feature 仅 desktop）",
        "⚠️ fallback 路径",
        "桌面优先",
        "金融深度图随 suites；移动端图表走 `chart_widgets/mod.rs` 的 fallback",
    ),
    "special_widgets": (
        "—",
        "✅ 全量（两个编辑器各自独立 feature，默认开）",
        "✅ `canvas` / `signature` / `segmented` 适用",
        "桌面=编辑器；移动=canvas/signature",
        "`signature_pad` 依赖 `touch`；`code_editor` / `markdown_editor` 需要键盘",
    ),
    "advanced_widgets": (
        "—",
        "✅ 全量（`dial` / `pie_menu` / `ribbon_bar` 桌面习惯）",
        "✅ picker 系（`calendar` / `date_edit` / `time_edit`）触控友好",
        "桌面=dial/pie/ribbon；移动=picker",
        "`pie_menu` 是指针语境控件，触控端建议 `action_sheet` 类替代",
    ),
    "media_widgets": (
        "—",
        "✅ `animated_image` / `hero_animation`",
        "✅ 同左",
        "全形态",
        "重媒体（video/audio）在 suites 且显式 opt-in，不在任何 device profile",
    ),
    "misc_widgets": (
        "—",
        "✅ 全量",
        "✅ `mobile_date_picker` / `drum_column` 移动最佳",
        "移动=picker/qr；桌面=avatar/qr",
        "`avatar` / `qr_code` 依赖 `image`；`barcode_scanner` 是相机语境",
    ),
    "cupertino": (
        "—",
        "⚠️ 可编译但语境错位",
        "✅ iOS 语境最佳",
        "mobile/tablet",
        "纯风格族，无 OS cfg → 全平台编译；iOS/Android 只是使用建议",
    ),
    "macos": (
        "—",
        "✅ 仅 macOS 语境推荐",
        "⚠️ 语境错位",
        "macOS 桌面",
        "纯风格族，无 OS cfg；`mac_*` 用桌面度量 profile 绘制",
    ),
    "companions": (
        "—",
        "✅ 随宿主控件使用",
        "✅ 同左",
        "随宿主",
        "地图/画布伴生件面向 suites；`step_strip` / `zoom_strip` 等零依赖，可直接挂在任意页",
    ),
    "root": (
        "⚠️ `window` 各 profile 均编译；`tool_box` 仅 `full_widgets`",
        "✅ 全量",
        "✅ `window` 全形态",
        "全形态",
        "`window` 是顶层窗口容器；`tool_box` 是桌面工具面板",
    ),
}


def load(name: str):
    """Loads a sibling tool as a module so its derivation is reused, not re-implemented."""
    path = pathlib.Path(__file__).with_name(name)
    spec = importlib.util.spec_from_file_location(name.removesuffix(".py"), path)
    if spec is None or spec.loader is None:  # pragma: no cover - a broken checkout
        raise SystemExit(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def family_gates() -> dict[str, str]:
    """Maps each widget family module to the `cfg` gate `src/widget/mod.rs` puts on it.

    `"every profile"` when the `pub mod` line carries no `cfg`, `"full_widgets"` when the line
    above it is `#[cfg(full_widgets)]`. This is what makes the profile column a derived fact:
    ungating a family rewrites the page instead of leaving a stale claim behind.
    """
    lines = WIDGET_MOD.read_text(encoding="utf-8").splitlines()
    gates: dict[str, str] = {}
    for index, line in enumerate(lines):
        match = re.match(r"^pub mod ([a-z_]+);", line)
        if not match:
            continue
        name = match.group(1)
        if name not in family_names():
            continue
        previous = lines[index - 1].strip() if index else ""
        gates[name] = "full_widgets" if previous.startswith("#[cfg(full_widgets)]") else "every profile"
    return gates


def family_names() -> set[str]:
    return {path.name for path in pathlib.Path("src/widget").iterdir() if path.is_dir()}


def render() -> str:
    codemap = load("gen_codemap_counts.py")
    index = load("generate_control_index.py")

    grouped = codemap.family_members()
    kinds = codemap.variants()
    controls = sum(len(names) for names in grouped.values())
    mapping = index.alias_names()
    aliases = sum(1 for name, canonical in mapping.items() if name != canonical)
    gates = family_gates()

    missing = sorted(set(grouped) - set(CURATED))
    if missing:
        raise SystemExit(
            "no curated scenario row for: "
            + ", ".join(missing)
            + "\n  Add one to CURATED in tools/gen_widget_classification.py — a new family must"
            + " answer 'which device shape' rather than inherit a default."
        )

    out: list[str] = [
        "<!-- GENERATED by tools/gen_widget_classification.py — do not edit by hand. -->",
        "",
        "# Control classification and scenario guidance",
        "",
        "> This page is plan **A** of `docs/issues/issue-20261115.md` §6.4: a reader-facing map of",
        "> the widget families, with the mechanical half generated instead of typed. Regenerate with",
        "> `python3 tools/gen_widget_classification.py`; `bash tools/check_widget_classification.sh`",
        "> fails when the committed page is stale.",
        "",
        "## 1. What the numbers mean (all derived)",
        "",
        "| Item | Value | Derivation |",
        "|---|---|---|",
        f"| `WidgetKind` variants | {kinds} | `pub enum WidgetKind` in `src/widget/kind.rs` |",
        f"| Registered controls (canonical names) | {controls} | `canonical_name:` rows in"
        " `src/widget/capability/properties/` |",
        f"| Alias names | {aliases} | `aliases: &[...]` rows in the same tables |",
        f"| Families | {len(grouped)} | directories under `src/widget/` that own a control |",
        "",
        "The two totals are **different measurements and neither is wrong**: a kind is an enum",
        "variant (several controls may borrow one kind — the table family, the panels, "
        "`stepper`/`mac_stepper`/`step_strip`), while a control is a row in the factory registry.",
        "This page indexes by **control**, because that is what a host mounts.",
        "",
        "Suites controls (`crates/rust-widgets-suites`: form, webview, finance, maps, project,",
        "devtools, media playback) are not in this table: they are optional packages installed into",
        "the same factory, and `check_suite_control_count.sh` owns their census.",
        "",
        "## 2. Families (membership derived from the registry)",
        "",
        "| Family | Controls | Registered controls (canonical names) |",
        "|---|---|---|",
    ]
    for family in codemap.FAMILY_ORDER:
        if family not in grouped:
            continue
        names = grouped[family]
        out.append(f"| `{family}/` | {len(names)} | {', '.join(names)} |")
    out += [
        "",
        "## 3. Scenario guidance",
        "",
        "The **Module gate** column is derived from `src/widget/mod.rs`; the rest is curated advice",
        "(§6.4 plan A). A family gated on `full_widgets` is absent from `mini`/`embedded` wholesale,",
        "so its stripped-profile cell says so instead of repeating a per-control guess.",
        "",
        "| Family | Module gate | mini / embedded | desktop | tablet / mobile | Device shape | OS / input notes |",
        "|---|---|---|---|---|---|---|",
    ]
    for family in codemap.FAMILY_ORDER:
        if family not in grouped:
            continue
        mini_note, desktop, mobile, shape, os_note = CURATED[family]
        gate = gates.get(family, "every profile")
        stripped = mini_note if gate != "full_widgets" else "❌ whole family is not compiled"
        out.append(
            f"| `{family}/` | `{gate}` | {stripped} | {desktop} | {mobile} | {shape} | {os_note} |"
        )
    out += [
        "",
        "## 4. Adoption record",
        "",
        "- **A. This page** — adopted. Cost: generated artefact + one gate; it introduces no second",
        "  source of truth because every number here is recomputed.",
        "- **B. Recommended-profile fields in the capability schema** — deferred (issue §6.4). The",
        "  schema is a runtime export locked by several gates, every row currently means \"any\",",
        "  and curated advice cannot be machine-verified; there is no machine-readable consumer yet",
        "  (designer/CLI filtering). If one appears, this page stays the only hand-curated source",
        "  and a script derives the schema fields from it.",
        "- **C. A `mini` demo gallery** — optional, deferred. One extra gallery is real maintenance;",
        "  it becomes worthwhile only when someone needs a shippable mini reference.",
        "",
        "Per-family `mod.rs` header comments are deliberately **not** part of this round: they would",
        "be a second copy of §3 with no gate behind them.",
        "",
    ]
    return "\n".join(out)


def main(argv: list[str]) -> int:
    rendered = render()
    check = "--check" in argv
    if check:
        if not PAGE.is_file():
            print(f"FAIL {PAGE} is missing; run python3 tools/gen_widget_classification.py")
            return 1
        committed = PAGE.read_text(encoding="utf-8")
        if committed != rendered:
            print(f"FAIL {PAGE} is stale; run python3 tools/gen_widget_classification.py")
            # Name the first differing line so the fix is obvious.
            for number, (old, new) in enumerate(
                zip(committed.splitlines(), rendered.splitlines()), start=1
            ):
                if old != new:
                    print(f"     first difference at line {number}:")
                    print(f"       committed: {old}")
                    print(f"       derived:   {new}")
                    break
            else:
                print("     the files differ in length")
            return 1
        print(f"✅ {PAGE} matches its derivation")
        return 0
    PAGE.write_text(rendered, encoding="utf-8")
    print(f"wrote {PAGE}: {len(rendered.splitlines())} lines")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
