#!/usr/bin/env bash
# ============================================================================
# check_source_structure.sh — principles #8 (mod.rs is a façade) and #9 (2000 lines)
# ============================================================================
# Two structural rules were recorded as issues HX-02 and HX-03 and then deferred,
# because the *work* is a series of module splits. Deferring the work is a decision;
# letting the numbers grow while it waits is not. This gate makes both rules
# enforceable today and turns the remaining work into a list that can only shrink.
#
# # Check 1 — `mod.rs` holds declarations, not implementations
#
# Principle #8: a `mod.rs` exists to declare and re-export. An `impl`, a `struct` or a
# free function defined there is invisible to a reader scanning the module for its
# children, and it grows without anyone noticing because the file is never opened.
#
# # Check 2 — no production source file exceeds 2000 lines
#
# Principle #9, as clarified on 2026-10-22: **test modules are exempt**. The measure is
# therefore the file's *production* lines — everything before its inline
# `#[cfg(test)] mod NAME { … }` block — and a separate `<dir>/tests.rs` is exempt entirely.
# The reason is that a test module's length tracks *coverage*, not coupling: splitting a flat
# list of independent cases by line count scatters cases that belong together, and moving
# them into a sibling file only to make a total-line count smaller is pure churn
# (`src/view/engine.rs` was put back after exactly that). Production code has no such excuse.
#
# # Why the allowlists are bidirectional
#
# An allowlist that only excuses is a licence to stay broken: the entry keeps passing
# after the defect is fixed, so the gate silently stops covering that file. Both lists
# therefore also fail when an entry is no longer needed. The list can only shrink.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" - <<'PY'
import pathlib
import re
import sys

LIMIT = 2000
SRC = pathlib.Path("src")

# ── Check 1 ─────────────────────────────────────────────────────────────────────
ITEM = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:fn|struct|impl|enum|trait|const|static|macro_rules!)\b"
)

# `mod.rs` files that still define items, with the extraction each one needs. Keyed by
# path; the value is the seam. Removing an entry from this table is the fix.
MOD_RS_PENDING: dict[str, str] = {
    "src/action/mod.rs": "the action data types → `action/types.rs` (mod.rs keeps the declarations and re-exports)",
    "src/data_binding/mod.rs": "the `PropertySink` trait and its adapters → `data_binding/sink.rs`",
    "src/menu_config/mod.rs": "the JSON converters → `menu_config/json.rs`",
    "src/widget/base_widgets/mod.rs": "`AutoRepeat` and its helpers → `base_widgets/auto_repeat.rs`",
    "src/widget/chart_widgets/finance/mod.rs": "`sync_view_range` → `finance/view_range.rs`",
    # Recorded 2026-10-28 (round 11): the two `mod.rs` files that still define items.
    "src/designer/mod.rs": "`DocumentEditCommand` / `DesignerHistory` → `designer/history.rs` (mod.rs keeps the declarations and re-exports)",
    "src/json/mod.rs": "`MAX_DEPTH` → `json/limits.rs`",
}

# ── Check 2 ─────────────────────────────────────────────────────────────────────
# Production files still over the limit. Each entry names the split it needs; the entry
# must disappear as the split lands, and this gate will say so.
TOO_LONG_PENDING = {
    "src/app/handle.rs": "event dispatch for five event families → `app/handle/{keys,pointer,window,menu,scroll}.rs`",
    "src/render/text/cjk_bitmap_data.rs": "generated glyph table → emit `cjk_bitmap.bin` + `include_bytes!`",
    "src/widget/runtime.rs": "registry / widget ledger / render tree / animation sweep / resize dispatch",
    "src/lib.rs": "crate façade: group the `pub use` surface into `api/{widgets,platform,signals}.rs`",
    "src/bindings/binding_impl.rs": "one module per binding family (python/java/c/node)",
    "src/widget/capability/properties.rs": "the per-kind property table → `properties/table.rs`",
    "src/control_backend/trait_def/trait_def.rs": "trait + its ~40 defaulted methods → `trait_def/{core,defaults}.rs`",
    "src/platform/types/traits.rs": "`Platform` (2270-line trait) → split by capability group",
    "src/platform/android_jni.rs": "JNI entry points → `android_jni/{lifecycle,input}.rs`",
    "src/image/decoder.rs": "one module per codec (png/jpeg/webp/bmp)",
    "src/designer/generator.rs": "wire-rule emission → `designer/generator/{emit,validate}.rs`",
    "src/print/print_impl.rs": "layout vs drawing → `print_impl/{layout,draw}.rs`",
    "src/platform/wayland/platform_impl.rs": "window vs input vs surface → `wayland/impl/*.rs`",
    "src/style/animation.rs": "easing curves vs drivers → `style/animation/{curves,drivers}.rs`",
    "src/wgpu_backend/renderer.rs": "resource caches vs 2D/mesh encoders → `wgpu_backend/renderer/{resources,encode}.rs`",
    "src/widget/base_widgets/button.rs": "state/API vs drawing → `button/{api,paint}.rs`",
    "src/widget/capability/access.rs": "per-kind defaults → `capability/access/{input,view,other}.rs`",
    "src/widget/container_widgets/layout_family.rs": "the six layout wrappers → one module each",
    "src/widget/container_widgets/tabwidget.rs": "strip vs pages vs drag → `tabwidget/{strip,pages,drag}.rs`",
    "src/widget/input_widgets/lineedit.rs": "text model vs field chrome → `lineedit/{model,paint}.rs`",
    "src/render/backend/surface.rs": "software surface: pixel primitives + layer/back-buffer caches vs damage/present accounting → `surface/{raster,present}.rs`",
    "src/widget/advanced_widgets/tab_bar.rs": "tab model/API vs strip geometry vs paint vs events → `tab_bar/{model,paint,events}.rs`",
    "src/widget/input_widgets/textarea.rs": "document model vs viewport paint → `textarea/{model,paint}.rs`",
    "src/widget/menu_toolbar/menu.rs": "menu model vs popup paint/input → `menu/{model,popup}.rs`",
    "src/widget/metrics.rs": "dimension tables vs focus/profile metrics → `metrics/{dimensions,profiles}.rs`",
    # Measured at the 2.9.2 review round. The two entries marked (new) crossed the limit in that
    # round; the rest were already over and are recorded here with the split each needs so the
    # list can only shrink.
    "src/json/loader.rs": "node/dialogue vs event-wire binding → `json/loader/{nodes,wiring}.rs`",
    "src/layout/flex.rs": "solver vs constraint table vs API → `flex/{solver,constraints}.rs`",
    "src/platform/macos_objc2/native.rs": "(new) view/controller types vs menu/dialog bridges → `macos_objc2/native/{views,bridges}.rs`",
    "src/render/pipeline/primitives.rs": "(new) shapes vs text/image drawing → `pipeline/{shapes,text_images}.rs`",
    "src/theme/manager.rs": "resolution vs caches vs presets → `theme/manager/{resolve,cache,presets}.rs`",
    "src/widget/advanced_widgets/calendar.rs": "model vs grid paint vs navigation → `calendar/{model,paint}.rs`",
    "src/widget/advanced_widgets/ribbon_bar.rs": "model vs tab/panel paint vs overflow → `ribbon_bar/{model,paint}.rs`",
    "src/widget/capability/constructors.rs": "one module per widget family (input/view/other)",
    "src/widget/container_widgets/dockwidget.rs": "layout model vs drag/float vs persistence → `dockwidget/{layout,drag,state}.rs`",
    "src/widget/container_widgets/scrollarea.rs": "chrome derivation vs sticky bands vs content → `scrollarea/{chrome,sticky}.rs`",
    "src/widget/dialog/alert_dialogs.rs": "card state vs button row vs paint → `alert_dialogs/{card,paint}.rs`",
    "src/widget/dialog/file_dialog.rs": "navigation vs listing/model vs paint → `file_dialog/{model,list,paint}.rs`",
    "src/widget/input_widgets/combobox.rs": "model vs editor vs popup paint → `combobox/{model,editor,popup}.rs`",
    "src/widget/input_widgets/spinbox.rs": "model vs buttons vs paint → `spinbox/{model,paint}.rs`",
    "src/widget/input_widgets/textedit.rs": "document/wrap model vs paint → `textedit/{model,paint}.rs`",
    "src/widget/media_widgets/lottie_widget.rs": "composition model vs player vs paint → `lottie/{model,paint}.rs`",
    "src/widget/media_widgets/rive_widget.rs": "composition bounds vs playback vs paint → `rive/{model,paint}.rs`",
    "src/widget/menu_toolbar/tool_bar.rs": "item model vs band layout vs paint → `tool_bar/{model,layout,paint}.rs`",
    "src/widget/chart_widgets/chart.rs": "series model vs axes/plot paint vs legend → `chart/{model,paint}.rs`",
    "src/widget/special_widgets/diff_viewer.rs": "diff model vs row projection vs paint → `diff_viewer/{model,rows,paint}.rs`",
    "src/widget/chart_widgets/kanban_board.rs": "board model vs lane layout vs drag/paint → `kanban/{model,layout,drag}.rs`",
    "src/widget/special_widgets/markdown_editor.rs": "document/line index vs find vs preview vs paint → `markdown_editor/{document,find,preview,paint}.rs`",
    "src/widget/special_widgets/terminal_view.rs": "screen buffer/ANSI model vs row paint → `terminal_view/{model,paint}.rs`",
    "src/widget/view_widgets/data_grid.rs": "projection window vs column layout vs paint → `data_grid/{projection,columns,paint}.rs`",
    "src/widget/view_widgets/grid_table.rs": "span layout vs selection vs paint → `grid_table/{layout,paint}.rs`",
    "src/widget/view_widgets/properties_panel.rs": "property rows vs editors vs paint → `properties_panel/{rows,paint}.rs`",
    "src/widget/view_widgets/tree_view.rs": "projection/index vs selection vs paint → `tree_view/{model,paint}.rs`",
    # Recorded 2026-10-28 (round 11): the files the gate named that had no row. Each seam is the
    # split that removes the finding; the row must disappear with it.
    "src/platform/linux/platform_impl.rs": "window/menu/dialog creation vs input vs surface/monitor → `linux/impl/{create,input,display}.rs`",
    "src/platform/windows/platform_impl.rs": "window/menu/dialog creation vs input vs display → `windows/impl/{create,input,display}.rs`",
    "src/style/css.rs": "selector model vs declaration parse vs cascade/lookup caches → `style/css/{selector,parse,cascade}.rs`",
    "src/wgpu_backend/raster.rs": "the command walk vs the CPU raster kernels → `raster/{walk,fills,text}.rs`",
    "src/widget/capability/properties_other.in.rs": "an `include!`d table: one file per widget family, mirroring `properties_base.in.rs` / `properties_menu.in.rs`",
    "src/widget/chart_widgets/bar_chart.rs": "series model vs axes/plot paint vs legend/hover → `bar_chart/{model,paint}.rs`",
    "src/widget/chart_widgets/gantt_widget.rs": "task model vs axis/row paint vs interaction → `gantt/{model,paint,events}.rs`",
    "src/widget/chart_widgets/line_chart.rs": "series model vs plot paint vs hover → `line_chart/{model,paint}.rs`",
    "src/widget/container_widgets/mdiarea.rs": "child-window model vs tile/cascade layout vs paint → `mdiarea/{model,layout,paint}.rs`",
    "src/widget/input_widgets/listbox.rs": "item model vs selection vs row paint → `listbox/{model,selection,paint}.rs`",
    "src/widget/input_widgets/rich_edit.rs": "document model vs format spans vs paint → `rich_edit/{model,formats,paint}.rs`",
    "src/widget/media_widgets/video_player.rs": "playback state vs transport/controls chrome vs paint → `video_player/{model,controls,paint}.rs`",
    "src/widget/view_widgets/tree_table.rs": "tree index vs column layout vs paint → `tree_table/{model,columns,paint}.rs`",
}


def is_test_module(path: pathlib.Path) -> bool:
    """A child test module: `<dir>/tests.rs` or `<dir>/<name>_tests.rs`."""
    return path.name == "tests.rs" or path.name.endswith("_tests.rs")


CFG_TEST = re.compile(r"^#\[cfg\([^\]]*\btest\b[^\]]*\)\]\s*$")
TEST_MOD = re.compile(r"^#{0,1}mod [A-Za-z0-9_]+ \{\s*$")


def production_lines(lines: list[str]) -> int:
    """Lines before a trailing inline `#[cfg(test)] mod NAME { … }` block.

    Returns the line index of the attribute, which is the number of production lines.
    A file with no inline test module measures its whole length.
    """
    starts = [
        index
        for index in range(len(lines) - 1)
        if CFG_TEST.match(lines[index]) and TEST_MOD.match(lines[index + 1])
    ]
    # The *first* inline test block ends the production region. A file may declare more than
    # one (`style/animation.rs` has `property_driver_tests` then `tests`; `lineedit.rs` has
    # `tests` then `composition_tests`), and taking the last one would count the earlier test
    # module as production code.
    return starts[0] if starts else len(lines)


errors: list[str] = []

# ── Check 1 ─────────────────────────────────────────────────────────────────────
for path in sorted(SRC.rglob("mod.rs")):
    lines = path.read_text(encoding="utf-8").splitlines()
    items = [line for line in lines if ITEM.match(line)]
    key = str(path)
    if items and key not in MOD_RS_PENDING:
        errors.append(
            f"{key}: defines {len(items)} item(s) in a `mod.rs` "
            f"(first: `{items[0].strip()}`). Principle #8: declare and re-export here, "
            "define in a sibling file."
        )
    elif not items and key in MOD_RS_PENDING:
        errors.append(
            f"{key}: no longer defines items — remove it from MOD_RS_PENDING "
            "(a stale allowlist entry excuses nothing and hides the next regression)."
        )

# ── Check 2 ─────────────────────────────────────────────────────────────────────
for path in sorted(SRC.rglob("*.rs")):
    if is_test_module(path):
        continue
    count = production_lines(path.read_text(encoding="utf-8").splitlines())
    key = str(path)
    if count > LIMIT and key not in TOO_LONG_PENDING:
        errors.append(
            f"{key}: {count} lines exceeds the {LIMIT}-line limit (principle #9). "
            "Split it, or record the seam in TOO_LONG_PENDING with the split it needs."
        )
    elif count <= LIMIT and key in TOO_LONG_PENDING:
        errors.append(
            f"{key}: now {count} lines, within the limit — remove it from "
            "TOO_LONG_PENDING so the gate covers it as ordinary code."
        )

if errors:
    for error in errors:
        print(f"❌ {error}", file=sys.stderr)
    print(f"\ncheck_source_structure: FAILED ({len(errors)} finding(s))", file=sys.stderr)
    sys.exit(1)

print(
    f"check_source_structure: mod.rs files carry no implementations outside "
    f"{len(MOD_RS_PENDING)} named extraction(s); no production file exceeds {LIMIT} "
    f"lines outside {len(TOO_LONG_PENDING)} named split(s)"
)
PY

echo "✅ check_source_structure: principles #8 and #9 hold, with the remaining work enumerated"
