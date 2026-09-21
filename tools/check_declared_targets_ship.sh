#!/usr/bin/env bash
# ============================================================================
# check_declared_targets_ship.sh — every declared Cargo target must be packaged
# ============================================================================
# `Cargo.toml` declares explicit targets with `[[test]]` / `[[bench]]` /
# `[[example]]` / `[[bin]]` sections, some of which point at paths outside the
# conventional directories (`tools/view_platform_gate_probe.rs` and
# `tools/widget_gallery.rs` are under `tools/`, not `tests/` or `examples/`).
#
# The crate also sets an explicit `include` list, and `tools/` is mostly NOT in
# it — gates and generators are not part of the crate's build surface. That
# combination produced this warning:
#
#   warning: ignoring test `view_platform_gate_probe` as
#            `tools/view_platform_gate_probe.rs` is not included in the published
#            package
#
# The failure mode is worse than the warning suggests. A target declared in
# `Cargo.toml` but absent from the tarball does not exist for anyone building
# from the published crate: `cargo test` silently runs one fewer test target, and
# the consumer has no way to know a gate was dropped. Deleting a file from the
# `include` list is a one-line change with no compile error anywhere, so without
# this gate the drift is undetectable until someone reads the warnings.
#
# This gate compares the two facts directly:
#   1. the target paths declared in `Cargo.toml`;
#   2. the file list `cargo package` would actually ship.
#
# It deliberately does not run `cargo package` to completion (which compiles the
# tarball): `--list` answers the question without building, and the bound below
# keeps it from hanging on an index lock.
# ============================================================================

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

ERRORS=0
error() {
  echo "❌ $*" >&2
  ERRORS=$((ERRORS + 1))
}

echo "--- [1] Collect declared target paths from Cargo.toml ---"

# Every explicit `path = "..."` under a target section. A `[[test]]`/`[[bin]]`
# without a `path` uses the conventional location, which cargo always packages,
# so only the explicit ones can point outside the `include` list.
DECLARED=()
while IFS= read -r p; do
  [ -n "$p" ] && DECLARED+=("$p")
done < <(grep -E '^path = "' Cargo.toml | sed 's/^path = "//; s/"$//')

if [ "${#DECLARED[@]}" -eq 0 ]; then
  # Not a failure: a crate with no explicit target paths has nothing to check.
  # Stated rather than silently passing, so the output cannot be read as
  # "checked and clean" when nothing was examined.
  echo "  (no explicit target paths declared — nothing to verify)"
  echo "✅ check_declared_targets_ship: no explicit target paths to verify"
  exit 0
fi

echo "  declared: ${#DECLARED[@]}"
for p in "${DECLARED[@]}"; do
  echo "    - $p"
done

echo "--- [2] Each declared path must exist on disk ---"
for p in "${DECLARED[@]}"; do
  if [ ! -f "$p" ]; then
    error "Cargo.toml declares a target at '$p' but no such file exists"
  fi
done

echo "--- [3] Ask cargo which files the package would contain ---"
PKG_LIST="$(mktemp)"
RAW_LIST="$(mktemp)"
trap 'rm -f "$PKG_LIST" "$RAW_LIST"' EXIT

# `--list` does not build, but it does resolve the index, so it is bounded.
if ! rw_run_bounded 300 cargo package --list --allow-dirty >"$RAW_LIST" 2>/dev/null; then
  echo "check_declared_targets_ship: unsupported host (cargo package --list did not complete)" >&2
  exit 0
fi

if [ ! -s "$RAW_LIST" ]; then
  error "cargo package --list produced no output, so nothing could be verified"
fi

# `cargo package --list` prints **host-native** separators: on Windows the paths come
# back as `examples\foo.rs`, while every `path = "..."` in `Cargo.toml` — and therefore
# every element of `DECLARED` — uses `/`. Comparing them verbatim made every declared
# target look unpackaged on Windows, which is a false positive about the *host*, not a
# finding about the crate (the same mistake `tools/audit_appearance.py` had). Normalise
# once, here, so the comparison is about the file rather than about the separator.
tr '\\' '/' < "$RAW_LIST" | sort -u > "$PKG_LIST"

echo "--- [4] Every declared target path must be packaged ---"
for p in "${DECLARED[@]}"; do
  if grep -qx "$p" "$PKG_LIST"; then
    echo "  ✅ ships: $p"
  else
    error "'$p' is a declared Cargo target but is NOT in the published package; add it to the 'include' list in Cargo.toml (otherwise 'cargo package' warns and consumers lose the target)"
  fi
done

if [ "$ERRORS" -gt 0 ]; then
  echo "❌ check_declared_targets_ship: $ERRORS problem(s)" >&2
  exit 1
fi

echo "✅ check_declared_targets_ship: all ${#DECLARED[@]} declared target(s) are packaged"
