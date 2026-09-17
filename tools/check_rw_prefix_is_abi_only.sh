#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_rw_prefix_is_abi_only.sh — the `rw_` prefix belongs to the ABI boundary
# ============================================================================
# # The rule
#
# `rw_` names the **C ABI** (`src/bindings/`), where the global namespace makes a
# prefix necessary: a C caller gets flat symbols, and `create_button` would collide
# with anything else in the process. Rust does not need it — a module path
# (`rust_widgets::view::Node`) already provides the namespace, and a type prefix in
# Rust is the C idiom, not the Rust one.
#
# So the prefix should be *confined*, not *spread*. This gate keeps it that way.
#
# # What it checks
#
#   [1] No `rw_*` in **definition position** outside `src/bindings/`.
#   [2] No `#[no_mangle]`/`#[export_name]` export outside the two legitimate ABI
#       modules, so a new C symbol cannot appear somewhere the header does not
#       track.
#
# "Definition position" is deliberately narrow: `fn rw_foo`, `struct rw_foo`,
# `const rw_foo`, or `export_name = "rw_foo"`. A *mention* inside a doc comment or
# a string is not a definition — documentation legitimately names ABI functions
# ("reachable through `rw_widget_property_names`"), and flagging that would push
# people to stop naming the ABI in the docs, which is worse than the leak.
#
# # Why definition position rather than every occurrence
#
# An earlier draft of this rule was "no `rw_` token anywhere outside bindings".
# That produced 38 hits, of which the overwhelming majority were **correct**: doc
# comments pointing at ABI entry points, and test-only temp-path prefixes
# (`/tmp/rw_spool_probe_*`) whose whole purpose is to be unlikely to collide.
# A gate with a 34-item allowlist nobody reads is a gate that gets allowlisted
# into uselessness, so the check is narrowed to the thing that actually matters:
# a *new name* taking the prefix in Rust code.
#
# # Accepted exceptions, each with a reason
#
#   * `src/error/mod.rs` — `RwError` and its `RwResult` alias. This is **settled
#     public API**: it appears in `api-reference.md` as a documented declaration
#     and is used at 35 call sites. Renaming it is a breaking change for a naming
#     preference (principle #21), and the C ABI never names the type — the prefix
#     is inherited style, not an export. Recorded here rather than silently
#     skipped, so a reader can argue with the decision.
#   * Test function names in `src/error/mod.rs` (`fn rw_error_display()`) mirror
#     the type they test. Also listed, for the same reason.
#
# Exit 0 = the prefix is confined. Exit 1 = a new leak was found.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0

# Files allowed to define an `rw_*` name, with the reason. Keyed by path.
#   * `src/bindings/` is the C ABI itself.
ACCEPTED_FILES=$(cat <<'FILES'
src/error/mod.rs
FILES
)

echo "=== [1] No new 'rw_*' definition outside the C ABI boundary ==="

# Definition positions only. `-E` for the alternation; the patterns are anchored on
# the keyword so a doc mention cannot match.
PATTERN='^[[:space:]]*(pub(\(crate\))?[[:space:]]+)?(unsafe[[:space:]]+)?(extern[[:space:]]+"[^"]*"[[:space:]]+)?(fn|struct|enum|union|const|static)[[:space:]]+rw_[a-z0-9_]+'

violations=0
while IFS= read -r hit; do
  [[ -z "$hit" ]] && continue
  file="${hit%%:*}"
  case "$file" in
    src/bindings/*) continue ;;
  esac
  if printf '%s\n' "$ACCEPTED_FILES" | grep -qx "$file"; then
    continue
  fi
  echo "  ❌ $hit"
  echo "       the 'rw_' prefix belongs to the C ABI; a Rust-level name should"
  echo "       rely on its module path instead (see this script's header)"
  violations=$((violations + 1))
done < <(grep -rEn "$PATTERN" src/ --include="*.rs" || true)

if [[ "$violations" -eq 0 ]]; then
  echo "  ✅ no 'rw_*' definition outside src/bindings/"
fi
ERRORS=$((ERRORS + violations))

echo ""
echo "=== [2] No ABI export outside the modules the ABI is generated from ==="
# `#[no_mangle]` creates a global symbol. Outside the C ABI and the JNI bridge that
# would mean an entry point the generated header does not know about, which is the
# defect `check_binding_symbol_coverage.sh` was written to catch on the other side.
export_violations=0
while IFS= read -r hit; do
  [[ -z "$hit" ]] && continue
  file="${hit%%:*}"
  case "$file" in
    src/bindings/*) continue ;;
    # The JNI bridge is the process boundary for Android and uses the JVM's own
    # `Java_*` naming, not `rw_`. It is an ABI module by the same argument as
    # `src/bindings/` (principle #40).
    src/platform/android_jni.rs) continue ;;
  esac
  # A mention inside a doc comment is documentation, not an export.
  case "$hit" in
    *"///"*|*"//!"*|*"// "*) continue ;;
  esac
  echo "  ❌ $hit"
  export_violations=$((export_violations + 1))
done < <(grep -rEn '^[[:space:]]*#\[(no_mangle|export_name)' src/ --include="*.rs" || true)

if [[ "$export_violations" -eq 0 ]]; then
  echo "  ✅ every ABI export lives in src/bindings/ or the JNI bridge"
fi
ERRORS=$((ERRORS + export_violations))

echo ""
echo "=== [3] The accepted exceptions must still be real ==="
# If `RwError` is ever renamed, this list should shrink; a stale exception is how an
# allowlist turns into a place leaks hide.
if grep -q 'pub struct RwError' src/error/mod.rs; then
  echo "  ✅ src/error/mod.rs still defines RwError (documented exception)"
else
  echo "  ❌ src/error/mod.rs no longer defines RwError, so the exception in this"
  echo "     script is stale — remove the entry rather than leaving it to hide a"
  echo "     future leak"
  ERRORS=$((ERRORS + 1))
fi

echo ""
if [[ "$ERRORS" -gt 0 ]]; then
  echo "rw-prefix gate: FAILED ($ERRORS finding(s))" >&2
  exit 1
fi
echo "✅ rw-prefix gate passed: 'rw_' is confined to the ABI boundary."
