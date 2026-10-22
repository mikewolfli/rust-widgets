#!/usr/bin/env bash
# rw-gate-budget: 600
# ============================================================================
# run_wayland_clipboard_paste.sh — PLAT-12 end-to-end: external copy -> first paste
# ============================================================================
# Starts a kiosk compositor (cage) whose single client is the library test, then
# has an **external** process (`wl-copy`) take the selection and asserts that the
# library's very first paste returns it.
#
# Requirements:
#   cage        — a kiosk Wayland compositor that focuses its single client
#   wl-copy     — wl-clipboard's external selection source
#   cargo, DISPLAY (for the x11 backend) or WLR_BACKENDS=headless
#
# Both tools are distro packages (`apt install cage wl-clipboard`). A host that
# does not have them exits 2 with `unsupported host`, as principle #103 requires;
# it does not fail the run and does not pretend the verification passed.
#
# Environment overrides:
#   CAGE_BIN             cage binary (default: cage on PATH)
#   WL_COPY              wl-copy binary (default: wl-copy on PATH)
#   WLR_BACKENDS         compositor backend (default: x11 when DISPLAY is set,
#                        headless otherwise)
#   WLR_RENDERER         renderer (default: pixman)
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_timeout.sh"

CAGE_BIN="${CAGE_BIN:-$(command -v cage || true)}"
WL_COPY="${WL_COPY:-$(command -v wl-copy || true)}"

if [[ -z "$CAGE_BIN" || -z "$WL_COPY" ]]; then
  echo "unsupported host: PLAT-12 needs a Wayland kiosk compositor and wl-clipboard" >&2
  echo "  missing:${CAGE_BIN:+} ${CAGE_BIN:-cage}${WL_COPY:+ }${WL_COPY:-wl-copy}" >&2
  echo "  install: sudo apt-get install cage wl-clipboard" >&2
  echo "  (or provide CAGE_BIN=/path/to/cage WL_COPY=/path/to/wl-copy)" >&2
  exit 2
fi

if [[ -z "${DISPLAY:-}" ]]; then
  echo "unsupported host: PLAT-12 needs an X11 display for cage's x11 backend" >&2
  echo "  run under: xvfb-run -a bash tools/run_wayland_clipboard_paste.sh" >&2
  exit 2
fi

WLR_BACKENDS="${WLR_BACKENDS:-x11}"
export WLR_BACKENDS
export WLR_RENDERER="${WLR_RENDERER:-pixman}"

RUNTIME="$(mktemp -d /tmp/rw-wl-paste.XXXXXX)"
chmod 700 "$RUNTIME"
cleanup() { rm -rf "$RUNTIME"; }
trap cleanup EXIT

export XDG_RUNTIME_DIR="$RUNTIME"
# cage owns the compositor socket; the child inherits WAYLAND_DISPLAY automatically.

echo "PLAT-12: backend=$WLR_BACKENDS cage=$CAGE_BIN wl-copy=$WL_COPY"
rw_run_bounded "${RW_GATE_TIMEOUT:-600}" env RW_WL_COPY="$WL_COPY" "$CAGE_BIN" -- cargo test --lib --features wayland-native \
  platform::wayland::tests::first_paste_reads_an_external_source -- --nocapture
