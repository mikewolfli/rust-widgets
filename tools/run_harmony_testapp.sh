#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
# ============================================================================
# run_harmony_testapp.sh — run the crate on a live OpenHarmony target
# ============================================================================
# The OpenHarmony analogue of `tools/run_ios_testapp.sh` and
# `tools/run_android_testapp.sh`:
#
#   1. build `examples/embedded_host.rs` for `aarch64-unknown-linux-ohos`
#   2. copy the produced ELF to the target with `hdc`
#   3. run it there and assert the facts it prints
#
# # Why a bare ELF and not a `.hap`
#
# Until this script existed, HarmonyOS had no *runtime* evidence at all: the only
# tool was `tools/check_harmony_cross.sh`, which compiles and links. The reason
# recorded in `docs/platform/real-device-verification.md` was that the repository
# has no `.hap` build/sign/install path — true, but it made a run look like it
# needed an ArkTS host, a signing profile and a UI framework project.
#
# It does not. An OpenHarmony target is a Linux/aarch64 userland whose `/bin/sh`
# (via `hdc shell`) executes any ELF placed in a writable, executable directory —
# verified on a DevEco emulator instance by pushing a musl-linked aarch64 binary
# and running it. A `.hap` is required to reach the **ArkUI** surface (a window, an
# `XComponent`, NAPI), not to execute the library.
#
# # Why `embedded`, not `desktop`
#
# The `desktop` build for this triple links `libace_napi.z.so` (the ArkUI NAPI
# bridge, pulled in by the Harmony backend). That library's own dependency
# `libark_jsruntime.so` is not resolvable from an ordinary `hdc shell` process —
# there is no ArkTS VM outside an application sandbox — so the binary dies in the
# loader with `rc=127` before `main` runs. That is a property of the *execution
# context*, not of the code, and it is exactly the boundary a `.hap` exists to
# cross.
#
# `embedded` links nothing but `libc.so`, so it runs, and it is the profile that
# exercises the portable surface this probe is about: build identity, the frame
# budget, the task queue, surface allocation with an explicit stride, the row
# copier, the engine registry and the capability contract.
#
# # Why the target must already be running
#
# Booting is deliberately *not* this script's job:
#
#   * the DevEco emulator is a Qt GUI application whose lifecycle belongs to
#     DevEco Studio's Device Manager;
#   * its `emulator-crash-service` watchdog kills a cold-booting instance roughly
#     21 s after launch ("Emulator snapshot boot timeout, try to restart"), which
#     is *before* the guest finishes booting, so a supervised launch from here is
#     not reliable.
#
# So the contract is the same as the Android runner's `ANDROID_SERIAL` path: a
# target is already connected and this script consumes it. With no target it
# reports `unsupported host` and exits 2 (principle #103) rather than waiting for
# a timeout (principle #59.4).
#
# Environment overrides:
#   HARMONY_TARGET        hdc target string, e.g. `127.0.0.1:5555`. Unset uses the
#                         only connected target.
#   OHOS_SDK_NATIVE       the SDK's `native` directory (`cargo ohos` reads this).
#   HARMONY_BUILD_TIMEOUT / HARMONY_PUSH_TIMEOUT / HARMONY_RUN_TIMEOUT
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Every step below is bounded: a stuck cross-linker or an `hdc` blocked on a
# half-dead target must not take the whole run with it (principle #58).
. "$ROOT_DIR/tools/lib_timeout.sh"

HARMONY_BUILD_TIMEOUT="${HARMONY_BUILD_TIMEOUT:-1800}"
HARMONY_PUSH_TIMEOUT="${HARMONY_PUSH_TIMEOUT:-300}"
HARMONY_RUN_TIMEOUT="${HARMONY_RUN_TIMEOUT:-120}"

TRIPLE="aarch64-unknown-linux-ohos"
EXAMPLE="embedded_host"
REMOTE_BIN="/data/local/tmp/rw_${EXAMPLE}"
LOCAL_BIN="$ROOT_DIR/target/$TRIPLE/debug/examples/$EXAMPLE"

# ---------------------------------------------------------------------------
# [0/4] Preflight: the three things this verification needs, each reported by
# name when absent. A gate that cannot run must say *what* is missing and *how*
# to install it, not fail later with a confusing error from a nested tool.
# ---------------------------------------------------------------------------

HDC_BIN=""
# DevEco ships a newer SDK than the standalone command-line one, and the target
# is a DevEco emulator, so prefer it when both are installed.
for candidate in \
  "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc" \
  "$HOME"/Library/OpenHarmony/Sdk/*/toolchains/hdc \
  "$HOME"/OpenHarmony/Sdk/*/toolchains/hdc; do
  if [[ -x "$candidate" ]]; then
    HDC_BIN="$candidate"
    break
  fi
done
if [[ -z "$HDC_BIN" ]]; then
  HDC_BIN="$(command -v hdc 2>/dev/null || true)"
fi
if [[ -z "$HDC_BIN" ]]; then
  echo "unsupported host (no hdc found)" >&2
  echo "  install the OpenHarmony SDK, or use DevEco Studio's bundled toolchain" >&2
  exit 2
fi

if ! cargo ohos --version >/dev/null 2>&1; then
  echo "unsupported host (cargo-ohos is not installed)" >&2
  echo "  install with: cargo install cargo-ohos" >&2
  exit 2
fi

if ! rustup target list --installed 2>/dev/null | grep -qx "$TRIPLE"; then
  echo "unsupported host (rust target $TRIPLE is not installed)" >&2
  echo "  install with: rustup target add $TRIPLE" >&2
  exit 2
fi

# `cargo ohos` reads OHOS_SDK_NATIVE; accept the older OHOS_SDK spelling too so an
# existing environment keeps working (same derivation as check_harmony_cross.sh).
if [[ -z "${OHOS_SDK_NATIVE:-}" && -n "${OHOS_SDK:-}" && -d "${OHOS_SDK}/native" ]]; then
  export OHOS_SDK_NATIVE="${OHOS_SDK}/native"
fi
if [[ -z "${OHOS_SDK_NATIVE:-}" ]]; then
  for root in "${HOS_SDK_HOME:-}" "$HOME/Library/OpenHarmony/Sdk" "$HOME/OpenHarmony/Sdk" \
              /Applications/DevEco-Studio.app/Contents/sdk/default/openharmony; do
    [[ -n "$root" && -d "$root" ]] || continue
    if [[ -d "$root/native/llvm" ]]; then
      OHOS_SDK_NATIVE="$root/native"
      break
    fi
    candidate="$(find "$root" -maxdepth 2 -type d -name native 2>/dev/null | sort -V | tail -1)"
    if [[ -n "$candidate" && -d "$candidate/llvm" ]]; then
      OHOS_SDK_NATIVE="$candidate"
      break
    fi
  done
fi
export OHOS_SDK_NATIVE="${OHOS_SDK_NATIVE:-}"
if [[ -z "$OHOS_SDK_NATIVE" || ! -d "$OHOS_SDK_NATIVE/llvm/bin" ]]; then
  echo "unsupported host (OHOS_SDK_NATIVE is unset or is not an OpenHarmony SDK)" >&2
  echo "  expected the SDK's 'native' directory, containing llvm/bin" >&2
  echo "  (got '${OHOS_SDK_NATIVE:-<unset>}')" >&2
  exit 2
fi

# Target selection. `hdc list targets` prints `[Empty]` when nothing is attached.
TARGET="${HARMONY_TARGET:-}"
if [[ -z "$TARGET" ]]; then
  TARGETS="$("$HDC_BIN" list targets 2>/dev/null | tr -d '\r' | grep -v '^\[Empty\]$' | grep -v '^$' || true)"
  count="$(printf '%s\n' "$TARGETS" | grep -c . || true)"
  if [[ "$count" -eq 0 ]]; then
    echo "unsupported host (no OpenHarmony target connected; 'hdc list targets' is empty)" >&2
    echo "  start the emulator from DevEco Studio's Device Manager, or:" >&2
    echo "    /Applications/DevEco-Studio.app/Contents/tools/emulator/Emulator -start '<instance>'" >&2
    echo "  (a cold boot needs its crash-service watchdog stopped, or it is killed after ~21s)" >&2
    echo "  or attach a device over USB, then re-run this script" >&2
    exit 2
  fi
  if [[ "$count" -gt 1 ]]; then
    echo "error: multiple targets connected; set HARMONY_TARGET to choose one:" >&2
    printf '  %s\n' $TARGETS >&2
    exit 2
  fi
  TARGET="$TARGETS"
fi

# Record the identity of what is being verified: a runtime claim without the
# device's own version strings is not reproducible evidence.
echo "[0/4] target: $TARGET"
for prop in const.ohos.fullname const.ohos.apiversion const.product.model; do
  value="$("$HDC_BIN" -t "$TARGET" shell "param get $prop" 2>/dev/null | tr -d '\r' | tr -s ' ' || true)"
  echo "      $prop = $value"
done
echo "      hdc = $HDC_BIN"

echo "[1/4] Building $EXAMPLE for $TRIPLE"
rw_run_bounded "$HARMONY_BUILD_TIMEOUT" cargo ohos build -t aarch64 \
  --no-default-features --features embedded --example "$EXAMPLE"

if [[ ! -f "$LOCAL_BIN" ]]; then
  echo "error: expected binary not produced: $LOCAL_BIN" >&2
  exit 1
fi

echo "[2/4] Copying to $TARGET:$REMOTE_BIN"
rw_run_bounded "$HARMONY_PUSH_TIMEOUT" "$HDC_BIN" -t "$TARGET" file send "$LOCAL_BIN" "$REMOTE_BIN"
rw_run_bounded "$HARMONY_PUSH_TIMEOUT" "$HDC_BIN" -t "$TARGET" shell "chmod 755 $REMOTE_BIN"

echo "[3/4] Running on $TARGET"
OUT_FILE="$(mktemp)"
status=0
rw_run_bounded "$HARMONY_RUN_TIMEOUT" "$HDC_BIN" -t "$TARGET" shell "$REMOTE_BIN" >"$OUT_FILE" 2>&1 || status=$?
cat "$OUT_FILE"

echo "[4/4] Checking the run"
fail=0
check() {
  if grep -qxF "$1" "$OUT_FILE"; then
    echo "      ok   $1"
  else
    echo "      FAIL missing line: $1" >&2
    fail=1
  fi
}
if [[ "$status" -ne 0 ]]; then
  # 124 is this project's timeout marker (see lib_timeout.sh); 127 is the dynamic
  # loader reporting an unresolvable dependency. Neither is a pass or a fail of
  # the library, so they are named rather than folded into "did not report PASS".
  echo "      FAIL the run exited $status" >&2
  case "$status" in
    124) echo "           (the bound fired; the target did not answer in ${HARMONY_RUN_TIMEOUT}s)" >&2 ;;
    127) echo "           (loader could not resolve a dependency — a 'desktop' build on this" >&2
         echo "            host needs the ArkUI chain; only 'embedded' runs from the shell)" >&2 ;;
  esac
  fail=1
fi

check "DEMO_PROFILE=embedded"
check "SUPPORTS_SURFACES=true"
check "DONE"

rm -f "$OUT_FILE"

if [[ "$fail" -eq 0 ]]; then
  echo "✅ OpenHarmony runtime verification PASSED ($EXAMPLE, target=$TARGET)"
  exit 0
fi

echo "❌ OpenHarmony runtime verification did not report PASS" >&2
exit 1
