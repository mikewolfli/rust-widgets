#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_macos_cross.sh — the macOS backend must type-check, as far as this host can take it
# ============================================================================
#
# # What this can and cannot do, measured rather than assumed
#
# The macOS backend (`src/platform/macos/`, the cocoa-legacy one) is gated on `target_os = "macos"`
# plus the `cocoa-legacy` feature. A Linux host cannot **run** it and cannot **link** anything for a
# Mac, but it *can* type-check it:
#
#   $ cargo check --lib --no-default-features --features cocoa-legacy --target x86_64-apple-darwin
#   Finished `dev` profile … in 1.81s          ← succeeds on Linux
#
# So this gate does what is possible here and states the boundary rather than claiming more:
#
#   * **possible**: `cargo check` of the lib for `x86_64-apple-darwin` and `aarch64-apple-darwin`,
#     which compiles every `cfg(target_os = "macos")` module behind `cocoa-legacy` — the AppKit
#     message sends in `platform_impl.rs`, `file_picker.rs`, `canvas.rs` (when the profile is
#     unstripped) and `displays.rs`. A wrong type, a missing gate, or a `Display` construction that
#     does not match the trait lands here. (A wrong *selector* does not: `msg_send!` takes the
#     selector as an identifier, so nothing validates it at compile time.)
#   * **not possible**: `--all-targets`, because a transitive build script (`alloca`, via `objc2`)
#     compiles C for the target and needs a cross `CC` for Apple. Verified: the failure is in
#     `alloca`'s build script, with zero errors in `rust_widgets`' own code.
#   * **not possible**: the full `desktop` feature set, because `minimp3-sys` (audio) also compiles C
#     for the target. Verified the same way. The `cocoa-legacy`-only set is the largest that stays
#     free of a cross C toolchain, and it is what covers the backend modules.
#   * **not possible**: running the backend, which needs a Mac.
#
# # Why `--lib` and not `--all-targets`
#
# Because `--all-targets` fails for a reason unrelated to this crate, and a gate that reports a
# dependency's host limitation as a defect is noise that trains people to ignore it. The limitation
# is recorded here so the next reader does not have to rediscover it.
#
# # Why both darwin targets
#
# `x86_64-apple-darwin` and `aarch64-apple-darwin` differ in `target_arch`, which is enough to hide
# a break in one (`cfg(target_arch)` paths, pointer-size assumptions). Checking both is what caught
# the equivalent split on iOS.
#
# Usage: tools/check_macos_cross.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

TARGETS=(
  x86_64-apple-darwin
  aarch64-apple-darwin
)

# The largest feature set that compiles the macOS backends without a cross C toolchain. See the
# header: `desktop` (audio) and `--all-targets` (objc2's `alloca`) both pull a C build script.
# `cocoa-legacy` selects the legacy cocoa backend, `macos` the objc2 preview one, so both are
# type-checked; `macos` alone is not enough on this target, because `src/platform/macos/` is
# declared whenever `target_os = "macos"` and its contents need `cocoa-legacy`.
FEATURES="cocoa-legacy,macos"

MISSING=()
for target in "${TARGETS[@]}"; do
  if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
    MISSING+=("$target")
  fi
done
if [[ "${#MISSING[@]}" -gt 0 ]]; then
  # `unsupported host`: the runner classifies on this marker, so without it a host with no Apple
  # toolchain is reported as a defect in the code under test (principle #59.4/#103).
  echo "unsupported host: these macOS targets are not installed, so this gate cannot check them:" >&2
  for target in "${MISSING[@]}"; do
    echo "  - $target" >&2
  done
  echo "" >&2
  echo "Install them with:" >&2
  echo "  rustup target add ${MISSING[*]}" >&2
  exit 1
fi

echo "=== macOS targets: ${#TARGETS[@]} target(s), lib type-check only ==="
for target in "${TARGETS[@]}"; do
  echo "--- $target"
  if ! rw_cargo_cached "$GATE_TIMEOUT" check --lib \
      --target "$target" --no-default-features --features "$FEATURES"; then
    echo "" >&2
    echo "FAIL: \`$target\` does not type-check with the macOS backend feature set." >&2
    echo "      This compiles every \`cfg(target_os = \"macos\")\` + \`cocoa-legacy\` module, so a" >&2
    echo "      type error or a missing gate in the macOS backend lands here." >&2
    exit 1
  fi
done

echo ""
echo "  note: running the backend and anything that links still needs a Mac. \`--all-targets\` and"
echo "        the full \`desktop\` set both additionally need a cross \`CC\` for Apple (\`alloca\` and"
echo "        \`minimp3-sys\` compile C for the target). Neither is a defect here."
echo ""
echo "macos cross-target checks passed."
