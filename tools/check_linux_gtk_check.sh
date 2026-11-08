#!/usr/bin/env bash
# Linux/GTK type-check closure (GV4-02).
#
# # Why this gate exists
#
# `src/platform/linux/**` is gated on `all(unix_desktop, feature = "gtk-native")`, so no host build
# ever type-checks it, and the issue list carried it as "un-covered" for several rounds. The
# cross-compilation shim (`tools/pkgconfig_linux_cross_shim.sh`) already existed but had **no
# consumer**: the tool was ready and nothing ran it. This gate is that consumer.
#
# # What it asserts
#
# `cargo check --target x86_64-unknown-linux-gnu --no-default-features --features "tablet,gtk-native"
# --lib` finishes with no warnings. It is a **compile** check, not a link check: the shim is only
# valid for `check`/`clippy` because it drops the host's framework flags.
#
# # Host requirements
#
# gtk+3 development files, pkg-config, and `rustup target add x86_64-unknown-linux-gnu`. When any
# is missing this gate prints `unsupported host (…)` and exits 2, which the runner records as
# SKIP(host-limited) — a missing host capability must be detected, not expressed as a timeout
# (principle #103).
#
# Usage: tools/check_linux_gtk_check.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

# One `cargo check`; bounded per rule #58.
GATE_TIMEOUT="${GATE_TIMEOUT:-900}"
TARGET="x86_64-unknown-linux-gnu"

unsupported() {
  echo "unsupported host ($1)"
  exit 2
}

# [1] The Rust target must be installed; a missing target is a host fact, not a code defect.
if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  unsupported "rust target $TARGET is not installed; run: rustup target add $TARGET"
fi

# [2] pkg-config must exist and answer for gtk+-3.0. On a macOS host the shim points at Homebrew's
# pkg-config; on a Linux host the one on PATH is the right one.
HOST_OS="$(uname -s)"
if [[ "$HOST_OS" == "Darwin" ]]; then
  PKG_CONFIG_BIN="/opt/homebrew/bin/pkg-config"
  [[ -x "$PKG_CONFIG_BIN" ]] || PKG_CONFIG_BIN="$(command -v pkg-config || true)"
  [[ -n "$PKG_CONFIG_BIN" ]] || unsupported "pkg-config is not installed (brew install pkg-config)"
  PKG_CONFIG_ALLOW_CROSS=1 "$PKG_CONFIG_BIN" --exists gtk+-3.0 \
    || unsupported "gtk+3 development files are not installed (brew install gtk+3)"
  export PKG_CONFIG_ALLOW_CROSS=1
  export PKG_CONFIG="$ROOT_DIR/tools/pkgconfig_linux_cross_shim.sh"
else
  command -v pkg-config >/dev/null 2>&1 \
    || unsupported "pkg-config is not installed (install pkg-config for your distribution)"
  pkg-config --exists gtk+-3.0 \
    || unsupported "gtk+3 development files are not installed (libgtk-3-dev or gtk3-devel)"
fi

echo "[1/1] cargo check --target $TARGET --no-default-features --features \"tablet,gtk-native\" --lib"
if ! rw_cargo_cached "$GATE_TIMEOUT" check --target "$TARGET" --no-default-features \
  --features "tablet,gtk-native" --lib; then
  echo "❌ Linux/GTK type-check failed"
  exit 1
fi

echo "✅ Linux/GTK type-check passed (compile only; no link)"
