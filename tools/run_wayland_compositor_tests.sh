#!/usr/bin/env bash
# ============================================================================
# run_wayland_compositor_tests.sh — verify the Wayland backend against a real
# compositor.
# ============================================================================
# The project's Wayland backend only takes its native protocol path when a
# compositor is reachable on WAYLAND_DISPLAY. Development machines here run an
# X11 session with no compositor installed, so the native path was previously
# never exercised — this script removes that environment block.
#
# Two acquisition modes are supported, chosen automatically:
#
#   system  (CI / any host with sudo or a preinstalled weston)
#       Use the weston already on PATH, or install it via apt when root is
#       available. No patching is required because the modules live at the
#       compiled-in path.
#
#   rootless (this project's default dev host: no sudo, no compositor)
#       1. `apt-get download` weston + libweston (download needs no root) and
#          extract them into ~/.local/opt/weston-extract.
#       2. weston's modules are dlopen()ed by an absolute, compiled-in path
#          (/usr/lib/...), which is not writable without root. The library's
#          copy of that path string is patched in place to point at
#          ~/.local/lib/wlmods (a strictly shorter path, so the binary layout
#          and all offsets are preserved).
#
# Either way a headless compositor is started (headless-backend + kiosk-shell,
# which needs no client processes) on a private socket and the Wayland test
# suite runs against it, in both directions:
#   - with a compositor   → the backend must bind real globals
#   - without a compositor → the backend must fall back to state-only
#
# Requirements: python3, cargo, and the dav1d prerequisites for any feature set
# that pulls in `image` (see docs/plans/blue14.md §五).
#
# Environment overrides:
#   WESTON_MODE          auto (default) | system | rootless
#   WESTON_BIN           explicit weston binary path (skips mode detection)
#   WESTON_SOCKET        compositor socket name (default: wayland-test)
#   WESTON_SRC           dir for downloaded .debs (rootless; /tmp/weston-debs)
#   WESTON_RUNTIME_DIR   private XDG_RUNTIME_DIR for the compositor
#                        (default: /tmp/xdg-weston-$WESTON_SOCKET; never the
#                        caller's live session directory)
#   WESTON_FEATURES      cargo feature set to test (default: wayland-native)
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WESTON_SOCKET="${WESTON_SOCKET:-wayland-test}"
WESTON_SRC="${WESTON_SRC:-/tmp/weston-debs}"
WESTON_MODE="${WESTON_MODE:-auto}"
WESTON_FEATURES="${WESTON_FEATURES:-wayland-native}"

PREFIX="$HOME/.local/opt/weston-extract"
MODDIR="$HOME/.local/lib/wlmods"

# --- Resolve the weston binary and its module environment --------------------

WESTON_BIN="${WESTON_BIN:-}"
EXTRA_LD_PATH=""
EXTRA_APT_PACKAGES=""

resolve_system() {
  if [[ -n "$WESTON_BIN" && -x "$WESTON_BIN" ]]; then
    return 0
  fi
  local found
  found="$(command -v weston || true)"
  if [[ -n "$found" ]]; then
    WESTON_BIN="$found"
    echo "      using system weston: $WESTON_BIN"
    return 0
  fi
  # Try to install it when we have root (CI runners do).
  if command -v apt-get >/dev/null 2>&1 && [[ "$(id -u)" -eq 0 || -n "${CI:-}" ]]; then
    if sudo -n true 2>/dev/null || [[ "$(id -u)" -eq 0 ]]; then
      local SUDO="sudo"
      [[ "$(id -u)" -eq 0 ]] && SUDO=""
      echo "      installing weston via apt (root available)"
      $SUDO apt-get update -qq
      # shellcheck disable=SC2086
      $SUDO apt-get install -y -qq weston
      WESTON_BIN="$(command -v weston || true)"
      if [[ -n "$WESTON_BIN" ]]; then
        echo "      installed: $WESTON_BIN"
        return 0
      fi
    fi
  fi
  return 1
}

stage_rootless() {
  echo "      rootless mode: apt-get download + in-place module-path patch"
  mkdir -p "$WESTON_SRC"
  if ! compgen -G "$WESTON_SRC/weston_*.deb" >/dev/null; then
    (cd "$WESTON_SRC" && apt-get download weston)
  fi
  if ! compgen -G "$WESTON_SRC/libweston-13-0_*.deb" >/dev/null; then
    (cd "$WESTON_SRC" && apt-get download libweston-13-0)
  fi

  mkdir -p "$PREFIX"
  for deb in "$WESTON_SRC"/weston_*.deb "$WESTON_SRC"/libweston-13-0_*.deb; do
    dpkg-deb -x "$deb" "$PREFIX"
  done

  if [[ ! -x "$PREFIX/usr/bin/weston" ]]; then
    echo "error: weston binary not found after extraction" >&2
    return 1
  fi

  # Stage every module into one directory (dereferencing symlinks, since the
  # modules are then loaded by bare name from that directory).
  mkdir -p "$MODDIR"
  local f
  for f in "$PREFIX/usr/lib/x86_64-linux-gnu/libweston-13/"*.so \
           "$PREFIX/usr/lib/x86_64-linux-gnu/weston/"*.so; do
    [[ -e "$f" ]] && cp -Lf "$f" "$MODDIR/"
  done

  python3 - "$PREFIX/usr/lib/x86_64-linux-gnu/libweston-13.so.0.0.0" \
            "$PREFIX/usr/lib/x86_64-linux-gnu/weston/libexec_weston.so.0.0.0" \
            "$MODDIR" \
            "/usr/lib/x86_64-linux-gnu/libweston-13" \
            "/usr/lib/x86_64-linux-gnu/weston" <<'PY'
import sys, os

lib, libexec, moddir, orig_moddir, orig_execdir = sys.argv[1:6]
new = moddir.encode()

def patch(path, old_s):
    old = old_s.encode()
    if not os.path.exists(path):
        print(f"  skip (missing): {path}")
        return
    with open(path, "r+b") as f:
        data = f.read()
        n = data.count(old)
        if n == 0:
            print(f"  already patched or not present: {os.path.basename(path)}")
            return
        assert len(new) <= len(old), (
            f"replacement path too long: {len(new)} > {len(old)}; "
            f"choose a shorter prefix"
        )
        data = data.replace(old, new + b"\x00" * (len(old) - len(new)))
        f.seek(0)
        f.write(data)
    print(f"  patched {n} occurrence(s) in {os.path.basename(path)}")

print(f"  module dir: {moddir}")
patch(lib, orig_moddir)      # backend modules (headless-backend.so, ...)
patch(libexec, orig_execdir) # shell/client modules (kiosk-shell.so, ...)
PY

  WESTON_BIN="$PREFIX/usr/bin/weston"
  EXTRA_LD_PATH="$PREFIX/usr/lib/x86_64-linux-gnu/weston:$PREFIX/usr/lib/x86_64-linux-gnu"
  return 0
}

echo "[1/5] Selecting weston ($WESTON_MODE mode)"
case "$WESTON_MODE" in
  system)
    resolve_system || { echo "error: no system weston available" >&2; exit 2; }
    ;;
  rootless)
    stage_rootless || { echo "error: rootless weston staging failed" >&2; exit 2; }
    ;;
  auto)
    if resolve_system; then
      : # system weston is fine
    else
      stage_rootless || { echo "error: could not obtain weston (system or rootless)" >&2; exit 2; }
    fi
    ;;
  *)
    echo "error: WESTON_MODE must be auto|system|rootless" >&2
    exit 2
    ;;
esac

echo "[2/5] Sampling weston version"
# Set the module/library search path before invoking weston. In rootless mode
# the extracted libexec/libweston live under the prefix, so they must be
# reachable for the version probe just as much as for the compositor run.
if [[ -n "$EXTRA_LD_PATH" ]]; then
  export LD_LIBRARY_PATH="$EXTRA_LD_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
"$WESTON_BIN" --version

echo "[3/5] Preparing private runtime dir (socket=$WESTON_SOCKET)"
# Always use a private runtime dir so a live $XDG_RUNTIME_DIR (e.g. a real
# desktop session) is never touched or removed.
XDG_RUNTIME_DIR="${WESTON_RUNTIME_DIR:-/tmp/xdg-weston-$WESTON_SOCKET}"
rm -rf "$XDG_RUNTIME_DIR"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
export XDG_RUNTIME_DIR

echo "[4/5] Starting headless compositor"
WESTON_LOG="$(mktemp /tmp/weston-compositor-XXXXXX.log)"
nohup "$WESTON_BIN" \
  --backend=headless-backend.so \
  --shell=kiosk-shell.so \
  --socket="$WESTON_SOCKET" \
  --idle-time=0 >"$WESTON_LOG" 2>&1 &
WESTON_PID=$!

cleanup() {
  kill "$WESTON_PID" 2>/dev/null || true
  wait "$WESTON_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for the socket to appear, then confirm the compositor is still alive.
for _ in $(seq 1 50); do
  [[ -S "$XDG_RUNTIME_DIR/$WESTON_SOCKET" ]] && break
  if ! kill -0 "$WESTON_PID" 2>/dev/null; then
    echo "error: compositor exited during startup:" >&2
    cat "$WESTON_LOG" >&2
    exit 1
  fi
  sleep 0.1
done

if [[ ! -S "$XDG_RUNTIME_DIR/$WESTON_SOCKET" ]]; then
  echo "error: compositor socket never appeared; log:" >&2
  cat "$WESTON_LOG" >&2
  exit 1
fi
echo "      compositor up (pid $WESTON_PID), socket $XDG_RUNTIME_DIR/$WESTON_SOCKET"

echo "[5/5] Running Wayland tests against the live compositor"
export WAYLAND_DISPLAY="$WESTON_SOCKET"

# Positive path: the compositor is reachable, so the backend must bind it.
cargo test --lib --features "$WESTON_FEATURES" platform::wayland -- --nocapture

echo
# Negative control: with no compositor the backend must degrade to state-only.
# Clear the display variable so the fallback branch is genuinely exercised.
echo "[5b/5] Negative control (no compositor → state-only fallback)"
env -u WAYLAND_DISPLAY cargo test --lib --features "$WESTON_FEATURES" \
  platform::wayland::tests::native_session_stays_empty_without_compositor -- --nocapture

echo
echo "✅ Wayland backend verified against a real compositor (pid $WESTON_PID)"
