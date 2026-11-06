#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_macos_backends_clippy.sh — both macOS backends are linted, not just compiled
# ============================================================================
# The rule this guards:
#
#   **A configuration that nothing types-checks with `-D warnings` rots, and a macOS backend is
#   exactly such a configuration on a Linux CI host.**
#
# The crate has *two* macOS backends (`macos` = objc2, `cocoa-legacy` = the AppKit/cocoa one),
# selected by mutually exclusive `cfg`s so a single build never sees both. Neither was ever linted
# on the Apple target: `cargo clippy --target aarch64-apple-darwin --features macos` was not run by
# any gate. Two findings had accumulated there and were only found by hand:
#
#   * three dead items in `platform/display_link.rs` (`OBSERVERS_INSTALLED` and its two accessors),
#     which only the cocoa backend asks for — the objc2-only build carried them, and a
#     `cfg_attr(allow(dead_code))` kept the warning quiet instead of removing the items;
#   * `clippy::slow_vector_initialization` in `render/text/bidi.rs`, on the
#     `not(feature = "text-bidi")` branch that the objc2 profile selects.
#
# Both are fixed, and this gate is what keeps the next one from accumulating: the pairs it checks
# are the ones the crate actually ships (`desktop` carries cocoa, `macos` carries objc2), plus the
# combination of both, which no other target builds.
#
# # Why `--lib` and not `--all-targets`
#
# Measured on a Linux host: `--all-targets` for the Apple target fails in a **dependency's** build
# script (`alloca v0.4.0` wants a Darwin C toolchain), so the only cross-checkable unit is the
# library — which is the whole subject here (the backends live in `src/platform`). The library is
# also what a downstream host links.
#
# # Host requirement
#
# The `aarch64-apple-darwin` target from `rustup`. A host without it reports `unsupported host …`
# and exits non-zero, which the runner turns into `SKIP(host-limited)` rather than a pass — a check
# that quietly does nothing is worse than no check, and the message says how to stop skipping.
#
# Usage: tools/check_macos_backends_clippy.sh
# Exit 0 = all three macOS configurations lint clean.
# Exit 1 = a finding (the failing configuration is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

TARGET="${MACOS_CLIPPY_TARGET:-aarch64-apple-darwin}"
STEP_BUDGET="${STEP_BUDGET:-600}"

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "unsupported host: the $TARGET target is not installed." >&2
  echo "  Install it with: rustup target add $TARGET" >&2
  exit 1
fi

failures=0
ok() { printf '  ✅ %s\n' "$*"; }
bad() { printf '  ❌ %s\n' "$*" >&2; failures=$((failures + 1)); }

# `name|features` — one line per configuration the crate actually ships.
#
#   `macos`        the objc2 backend (`build.rs` selects it by target; the feature is what a host
#                  asks for when it wants that backend over the cocoa one).
#   `desktop`      the full desktop profile, which carries `cocoa-legacy` — the backend a plain
#                  `cargo build` compiles on macOS.
#   both           a combination nothing else builds: it is the only way to see that the two
#                  backends' shared modules (`display_link`, `macos_heartbeat`) compile when both
#                  sides of their `cfg`s are live at once.
CONFIGS=(
  "objc2|macos"
  "cocoa|desktop"
  "both|macos,macos-legacy"
)

echo "[1/2] each macOS backend lints clean with -D warnings"
for config in "${CONFIGS[@]}"; do
  name="${config%%|*}"
  features="${config##*|}"
  log="$(mktemp)"
  if rw_cargo_cached "$STEP_BUDGET" clippy \
      --target "$TARGET" \
      --no-default-features \
      --features "$features" \
      --lib \
      -- -D warnings >"$log" 2>&1; then
    ok "$name ($features) lints clean"
  else
    bad "$name ($features) has a clippy finding; reproduce with:"
    printf '       cargo clippy --target %s --no-default-features --features %s --lib -- -D warnings\n' \
      "$TARGET" "$features" >&2
    # The finding itself, so the log the runner keeps is enough to act on. Tail rather than the
    # whole log: a cold clippy run prints a compile line per crate, and the verdict is at the end.
    tail -40 "$log" >&2
  fi
  rm -f "$log"
done

echo "[2/2] both backends build together, so their shared modules cannot drift apart"
# The `both` row above already compiles that combination; this is the assertion that the row was
# not silently dropped from the list when somebody edits CONFIGS.
if printf '%s\n' "${CONFIGS[@]}" | grep -q '^both|'; then
  ok "the two-backend combination is covered"
else
  bad "CONFIGS no longer contains the two-backend combination"
fi

if [ "$failures" -ne 0 ]; then
  echo "check_macos_backends_clippy: $failures finding(s)" >&2
  exit 1
fi
echo "macOS backends clippy: OK (objc2, cocoa and the two-backend combination are warning-free)"
