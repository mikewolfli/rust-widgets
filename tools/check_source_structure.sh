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
MOD_RS_PENDING: dict[str, str] = {}

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
