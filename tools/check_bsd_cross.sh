#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_bsd_cross.sh — the BSD half of `unix_desktop`
# ============================================================================
# # Why this gate exists
#
# `build.rs` emits `cfg(unix_desktop)` for the Unix hosts that run the X11/GTK or
# Wayland backend: Linux **and the BSDs**, but not Apple, Android, Redox,
# OpenHarmony or wasm. That predicate replaced 115 hand-written
# `target_os = "linux"` gates, and a typo in it has no compile-time symptom on
# the host: the BSD target would simply fall back to the portable stub, which
# compiles fine.
#
# The only way to keep the two halves honest is to type-check a BSD target. This
# gate does that for FreeBSD and NetBSD (the two BSDs rustup ships std for),
# with the GTK and Wayland paths enabled when the host has GTK headers to
# type-check against.
#
# # Host requirement
#
#   * `rustup target add x86_64-unknown-freebsd x86_64-unknown-netbsd`
#   * `pkg-config` with `gtk+-3.0` (e.g. `libgtk-3-dev`) for the GTK path;
#     without it the gate still checks the state backend and says so.
#   * `clang` for NetBSD's C dependency (`minimp3-sys`); NetBSD is skipped when
#     the compiler cannot see a libc to compile against.
#
# A host with neither BSD target prints `unsupported host` and exits 0: that is
# a limitation of the machine, not a defect in the code.
#
# Usage: tools/check_bsd_cross.sh
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FEATURES="desktop,wayland-native"
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gtk+-3.0 2>/dev/null; then
  FEATURES="desktop,gtk-native,wayland-native"
else
  echo "note: gtk+-3.0 not found via pkg-config; checking the state backend + Wayland only"
fi

installed_targets() {
  rustup target list --installed 2>/dev/null
}

checked=0
skipped=()

check_freebsd() {
  local triple="x86_64-unknown-freebsd"
  if ! installed_targets | grep -qx "$triple"; then
    skipped+=("$triple (std not installed)")
    return
  fi
  echo "--- $triple"
  # `PKG_CONFIG_ALLOW_CROSS` is required for `cargo check` only: the gtk-sys
  # build script otherwise refuses to emit flags for a target it does not
  # recognise. No library is linked by `cargo check`, so probing the host's GTK
  # headers is exactly the type-check this gate wants.
  PKG_CONFIG_ALLOW_CROSS=1 \
    cargo check --lib --target "$triple" --no-default-features --features "$FEATURES"
  checked=$((checked + 1))
}

check_netbsd() {
  local triple="x86_64-unknown-netbsd"
  if ! installed_targets | grep -qx "$triple"; then
    skipped+=("$triple (std not installed)")
    return
  fi
  # `minimp3-sys` (via the `desktop` profile's audio feature) compiles C, and
  # cc-rs looks for `x86_64--netbsd-gcc` by default. clang can compile the file
  # when it can see a libc header set; the include path below is the same trick
  # the hand-run command uses on a Debian/Ubuntu host. On a host without those
  # headers (macOS), this target is skipped rather than reported as a code
  # failure.
  if ! command -v clang >/dev/null 2>&1 \
    || [[ ! -d /usr/include/x86_64-linux-gnu ]]; then
    skipped+=("$triple (no clang with a usable libc header set)")
    return
  fi
  echo "--- $triple"
  CC_x86_64_unknown_netbsd=clang \
    AR_x86_64_unknown_netbsd=llvm-ar \
    C_INCLUDE_PATH=/usr/include/x86_64-linux-gnu:/usr/include \
    PKG_CONFIG_ALLOW_CROSS=1 \
    cargo check --lib --target "$triple" --no-default-features --features "$FEATURES"
  checked=$((checked + 1))
}

check_freebsd
check_netbsd

for entry in "${skipped[@]:-}"; do
  [[ -n "$entry" ]] && echo "  skipped: $entry"
done

if [[ "$checked" -eq 0 ]]; then
  echo "unsupported host: no BSD target with std is installed"
  exit 0
fi

echo
echo "bsd cross-target checks passed ($checked target(s); features=$FEATURES)."
