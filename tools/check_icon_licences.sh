#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_icon_licences.sh — BLUE25 ICON-8 (the licence chain)
# ============================================================================
# The rule this guards (blue25.md §6.7.3, Apache-2.0 §4 / §6):
#
#   **Every byte of third-party icon data the crate ships carries its provenance and its
#   licence in the repository, and the whole chain is checkable without a human reading it.**
#
# # Why this is a gate
#
# Icon data is unlike a crate dependency. A dependency is named in `Cargo.toml` and its licence
# travels with it; a set of SVG path strings compiled into a source file looks, to every tool that
# inspects the repository, like code the project wrote. The moment that is untrue — an outline
# copied from an Apache-2.0 set — the repository's own MIT `LICENSE` is a false statement about it,
# and nothing in the build, the tests or the snapshots can tell.
#
# Material Symbols was chosen over Lucide (blue25.md §6.1.4) precisely because its obligations are
# *mechanical*: a licence copy, a NOTICE section, a generated header, and a "does upstream ship a
# NOTICE" answer. All four are asserted below, so "the licence record is complete" is a fact the
# suite maintains rather than a claim in a document.
#
# # What this proves
#
#   1. `tools/material_symbols/LICENSE` exists and its digest equals the pinned upstream digest
#      (Apache-2.0 §4(a) — the required copy of the licence).
#   2. The vendored outline count matches the icon tokens the crate declares, so no icon ships
#      without its own vendor file.
#   3. `src/widget/icon_data.rs` carries a GENERATED header naming the upstream project, the
#      pinned revision and the licence (§4(b) — modifications marked), and is **current**: its
#      contents still equal what the vendored tree generates.
#   4. `NOTICE` has a section naming the generated file, repeating the upstream revision and
#      digest, and stating the licence (§4(c) — attribution kept).
#   5. Upstream still ships **no** `NOTICE` at the pinned revision, recorded in
#      `tools/material_symbols/UPSTREAM_HAS_NO_NOTICE`; if that file disappears, upstream added
#      one and its text must be vendored (§4(d)).
#   6. `tools/gen_icon_data.py` still **refuses** to run without `--license`, and rejects an
#      unknown one with exit 2 — the inbound half of the gate (the same shape
#      `tools/check_font_licenses.sh` checks for the font generators).
#
# # What this does NOT prove
#
#   * It does not fetch upstream. The pinned revision and its digest are literals in
#     `tools/vendor_material_symbols.py`; `--check` (there) verifies the vendored copy against
#     them only when the network is available. This gate is fully offline.
#   * It does not read licences or judge them. It checks that a record exists and is specific.
#
# # Reverse injection
#
# Two, because the gate has two halves:
#   * deleting the Material Symbols section from a copy of `NOTICE` must be reported (the scan is
#     pointed at the copy with `--notice`);
#   * `gen_icon_data.py` with no `--license` and with a bogus one must both exit 2.
#
# Usage: tools/check_icon_licences.sh
# Exit 0 = the icon licence chain is complete and the inbound gate refuses.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/icon_license_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  The icon licence chain is incomplete. Fix the finding above: an icon ships without a"
        echo "  licence copy, a generated header, or a NOTICE section. See blue25.md §6.7.2."
        exit 1
        ;;
esac

# ── Reverse injection: a NOTICE that lost its Material Symbols section must be reported ────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
"$PYTHON" - "$INJECT_DIR/NOTICE" <<'PY'
import pathlib, sys
text = pathlib.Path("NOTICE").read_text(encoding="utf-8")
marker = "Material Symbols — SVG path subsets for `icons`"
# Every section in NOTICE is prefixed by a rule line; cutting from the rule that precedes the
# heading removes the icon record and leaves every other section intact, so the scan must name
# the icon record and nothing else.
start = text.index(marker)
rule = text.rindex("=" * 80, 0, start)
pathlib.Path(sys.argv[1]).write_text(text[:rule], encoding="utf-8")
PY
if "$PYTHON" tools/icon_license_scan.py --notice="$INJECT_DIR/NOTICE" | grep -q "failed=0"; then
    echo "FAIL: removing the Material Symbols section from NOTICE did not fail the scan,"
    echo "      so the scan is not reading the record it claims to check"
    exit 1
fi

# ── Reverse injection: the inbound licence gate must still refuse ───────────────────────────────
if "$PYTHON" tools/gen_icon_data.py >/dev/null 2>&1; then
    echo "FAIL: tools/gen_icon_data.py ran with no --license; the licence gate is gone"
    exit 1
fi
if "$PYTHON" tools/gen_icon_data.py --license=not-a-real-licence >/dev/null 2>&1; then
    echo "FAIL: tools/gen_icon_data.py accepted an unknown --license"
    exit 1
fi

# ── The generated table must be **current** (not only correctly headed) ─────────────────────────
#
# The scan above reads the header and the entry count, which catches a table that never got a
# licence record. It does not catch a table whose *contents* drifted from the vendored SVGs while
# the header stayed right — the `d` values could be edited by hand, or a vendor refresh could leave
# the table behind. `--check` recomputes and compares, which is the only way to see that. Offline,
# so it reads the vendor tree rather than the network (see the generator's `--check`).
if ! "$PYTHON" tools/gen_icon_data.py --license=apache-2.0 --check >/dev/null 2>&1; then
    echo "FAIL: src/widget/icon_data.rs is stale; regenerate with"
    echo "      python3 tools/gen_icon_data.py --license=apache-2.0"
    exit 1
fi

# ── The vendored tree must match the pinned revision's digests ──────────────────────────────────
#
# `vendor_material_symbols.py --check` verifies the licence copy against the pinned digest and that
# every token has a vendored SVG. It needs no network: the digests are literals. A network-using
# refresh (`--refresh`) is a separate, deliberate act.
if ! "$PYTHON" tools/vendor_material_symbols.py --check >/dev/null 2>&1; then
    echo "FAIL: the vendored icon tree does not match its pinned revision; see"
    echo "      python3 tools/vendor_material_symbols.py --check"
    exit 1
fi

echo "icon-licence checks passed."
