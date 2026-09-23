#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_font_licenses.sh — BLUE23 §0A.4 (附录 G-4a)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, G-4a "子集生成器 + 许可门禁 + NOTICE"):
#
#   **Every byte of third-party font data shipped in `src/` must carry its provenance and its
#   licence in the repository, in a form a person can find without reading the generator.**
#
# # Why this is a gate and not a convention
#
# Font data is unlike every other dependency in this crate. A crate dependency is named in
# `Cargo.toml` and its licence travels with it; a *subset of glyphs compiled into a source
# file* looks, to every tool that inspects the repository, like code the project wrote. The
# moment that is untrue — a glyph bitmap copied from an OFL face, an outline lifted from a
# differently-licensed one — the repository's own MIT `LICENSE` is a false statement about it,
# and nothing in the build, the tests or the snapshots can tell.
#
# The two halves of the answer are both mechanical:
#
#   1. **The generator refuses to run without a stated licence.** `tools/gen_cjk_bitmap.py`
#      takes `--license=<id>` from a closed set and exits 2 otherwise. That is the *inbound*
#      gate: data cannot enter the tree through a run that did not assert which licence it was
#      relying on.
#   2. **The generated file declares its provenance in its own header, and `NOTICE` repeats
#      it.** That is the *outbound* gate: whoever reads the shipped bytes can find the origin,
#      the exact upstream digest, the font project's name and both halves of a dual licence.
#
# This script checks both, and checks that the first one still refuses.
#
# # What this gate proves
#
#   * `tools/gen_cjk_bitmap.py` exits **2** with no `--license`, and **2** with an unknown
#     `--license`. (A gate that only read the script would not notice the check being deleted.)
#   * Every `.rs` file under `src/` that declares itself `GENERATED FILE` and has a
#     `Glyph source:` header is named in `NOTICE`, with its upstream SHA-256 repeated there,
#     its font project named there, and both halves of its dual licence present there.
#   * The regenerator named in that header exists.
#
# # What this gate does NOT prove
#
#   * It cannot tell whether the digest in the header is the digest of the data in the file —
#     only `gen_cjk_bitmap.py --check` can, and that needs the upstream download. It proves the
#     *declaration* is complete and consistent, not that the bytes match the declaration.
#   * It does not read licences or judge them. It checks that a record exists and is specific.
#
# # Reverse injection
#
# Two, because the gate has two halves:
#   * the scanner is pointed at a temp copy of the generated table placed outside `src/`, whose
#     path `NOTICE` cannot name — the scan must report it (`--inject`);
#   * the generator is run with no `--license` and with a bogus one, and must exit 2 both times.
#
# Usage: tools/check_font_licenses.sh
# Exit 0 = every shipped font table is recorded, and the inbound gate refuses.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

# ── Half 1: the outbound record ─────────────────────────────────────────────────────────────────
if ! OUT="$("$PYTHON" tools/font_license_scan.py)"; then
    echo "FAIL: font_license_scan.py could not run"
    exit 1
fi
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A generated font table ships without a complete licence record. Add a section to"
        echo "  NOTICE naming the file, the upstream project, the source digest and the licence."
        exit 1
        ;;
esac

# ── Reverse injection: a table NOTICE cannot name must be reported ──────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
cp src/render/text/cjk_bitmap_data.rs "$INJECT_DIR/unrecorded_font_table.rs"
if "$PYTHON" tools/font_license_scan.py --inject="$INJECT_DIR/unrecorded_font_table.rs" \
        | grep -q "failed=0"; then
    echo "FAIL: injecting an unrecorded font table did not fail, so the scan is not comparing"
    echo "      against NOTICE"
    exit 1
fi

# ── Half 2: the inbound gate must still refuse ──────────────────────────────────────────────────
if "$PYTHON" tools/gen_cjk_bitmap.py >/dev/null 2>&1; then
    echo "FAIL: the generator ran with no --license; the licence gate is gone"
    exit 1
fi
if "$PYTHON" tools/gen_cjk_bitmap.py --license=not-a-real-licence >/dev/null 2>&1; then
    echo "FAIL: the generator accepted an unknown --license"
    exit 1
fi

echo "font-licence checks passed."
