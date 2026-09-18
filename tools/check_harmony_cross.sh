#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# HarmonyOS (OpenHarmony) cross-target gate.
#
# Verifies two things that are easy to get wrong and impossible to notice on a
# host build:
#   1. how the OpenHarmony targets identify themselves, and that the backend
#      selection follows it;
#   2. that they actually **link**, using the toolchain's own sysroot.
#
# On identification: `rustc --target <t> --print cfg` reports `target_os="linux"`
# and `target_env="ohos"` for **every** `*-unknown-linux-ohos` target. A backend
# selected with `cfg(target_os = "ohos")` therefore never matches. The historical
# spelling did exactly that and made the target fail with `cannot find value
# 'create_native_platform' in this scope` — not merely mis-selected, it did not
# build at all.
#
# On linking: `cargo check` does not link, and a plain `cargo build` with only
# `CARGO_TARGET_*_LINKER` set compiles C dependencies against the *host* headers.
# The SDK's sysroot must be supplied as explicit `--target`/`--sysroot` flags
# plus `-D__MUSL__`; `cargo ohos` computes exactly that set, which is why this
# gate drives it instead of hand-rolling the environment.
#
# Requires: the OpenHarmony SDK and `cargo-ohos`.
# Set OHOS_SDK_NATIVE to the SDK's `native` directory (that is the variable
# `cargo-ohos` itself reads). When the prerequisites are absent this gate reports
# HOST-GATED (exit 2) rather than passing vacuously: a green result must mean the
# checks really ran.
#
# Target coverage (all four triples rustc knows for OpenHarmony):
#   aarch64 / armv7 / x86_64  — built AND linked, artifacts arch-verified
#   loongarch64               — Tier 3 (no prebuilt rust-std) and the SDK ships
#                               no libc for it, so it cannot build; asserted.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# Cross-target work is the slowest thing here: three triples, each checked under
# several feature sets and then *built and linked* against the SDK sysroot. A
# wedged `cargo ohos` (a stuck NDK linker is the realistic case) would otherwise
# block the gate indefinitely with no output after its banner.
OHOS_CHECK_TIMEOUT=1800
OHOS_BUILD_TIMEOUT=2400

PRIMARY="aarch64-unknown-linux-ohos"
LINKABLE=(armv7-unknown-linux-ohos x86_64-unknown-linux-ohos)
BUILD_STD_ONLY="loongarch64-unknown-linux-ohos"
ALL_TARGETS=("$PRIMARY" "${LINKABLE[@]}" "$BUILD_STD_ONLY")

# `cargo ohos` reads OHOS_SDK_NATIVE; accept the older OHOS_SDK spelling too by
# deriving it, so an existing environment keeps working.
#
# Exported, not merely assigned. `cargo ohos` is a *child process*, so a shell-local
# variable is invisible to it: with only `OHOS_SDK` set, this script's own preflight
# check passed (it reads the variable in-process) and then step [2] failed with
# "Could not find the OpenHarmony native SDK" — the gate reporting an SDK problem that
# was really a missing `export` in the gate itself.
if [[ -z "${OHOS_SDK_NATIVE:-}" && -n "${OHOS_SDK:-}" && -d "${OHOS_SDK}/native" ]]; then
    OHOS_SDK_NATIVE="${OHOS_SDK}/native"
fi

# # Why `HOS_SDK_HOME` and the DevEco path are also accepted
#
# DevEco Studio exports `HOS_SDK_HOME` pointing at the SDK **root**, not
# `OHOS_SDK`/`OHOS_SDK_NATIVE` — so on a machine with a complete, working
# OpenHarmony toolchain this gate reported "OHOS_SDK_NATIVE is unset" and exited 2.
# That reads as "no SDK" when the SDK is present and the cross target links, which
# is the misreport the `export` fix above already had to correct once for a
# different variable name. The SDK root may also hold several API levels
# (`Sdk/18`, `Sdk/20`, …), so the newest one containing a `native/llvm` is chosen
# rather than assuming a single version.
#
# Only consulted when `OHOS_SDK_NATIVE` is genuinely unset, so an explicit setting
# always wins.
if [[ -z "${OHOS_SDK_NATIVE:-}" ]]; then
    for root in "${HOS_SDK_HOME:-}" "$HOME/Library/OpenHarmony/Sdk" \
                "$HOME/OpenHarmony/Sdk" /opt/OpenHarmony/Sdk; do
        [[ -n "$root" && -d "$root" ]] || continue
        # A root that *is* the native dir, or the newest API level inside it.
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
export OHOS_SDK_NATIVE

if ! rustup target list --installed 2>/dev/null | grep -q "^${PRIMARY}$"; then
    echo "HOST-GATED: rust target ${PRIMARY} is not installed"
    echo "  install with: rustup target add ${PRIMARY}"
    exit 2
fi

if ! cargo ohos --version >/dev/null 2>&1; then
    echo "HOST-GATED: cargo-ohos is not installed"
    echo "  install with: cargo install cargo-ohos"
    exit 2
fi

if [[ -z "${OHOS_SDK_NATIVE:-}" || ! -d "${OHOS_SDK_NATIVE}/llvm/bin" ]]; then
    echo "HOST-GATED: OHOS_SDK_NATIVE is unset or does not point at an OpenHarmony SDK"
    echo "  expected the SDK's 'native' directory, containing llvm/bin"
    echo "  (got '${OHOS_SDK_NATIVE:-<unset>}')"
    exit 2
fi

fail=0
note() { printf '  %s\n' "$*"; }

# ---------------------------------------------------------------------------
# [1] The premise this whole gate rests on.
# ---------------------------------------------------------------------------
echo "[1/6] target-identification premise (all OpenHarmony triples)"
for triple in "${ALL_TARGETS[@]}"; do
    cfg_out="$(rustc --target "$triple" --print cfg 2>/dev/null || true)"
    if [[ -z "$cfg_out" ]]; then
        note "$triple: rustc cannot describe this target — skipped"
        continue
    fi
    echo "$cfg_out" | grep -qx 'target_env="ohos"' ||
        { echo "  FAIL $triple: target_env is not \"ohos\""; fail=1; }
    echo "$cfg_out" | grep -qx 'target_os="linux"' ||
        { echo "  FAIL $triple: target_os is not \"linux\""; fail=1; }
    note "$triple: target_env=ohos, target_os=linux  ✓"
    # `cfg(target_os = "harmony")` is the spelling this gate exists to prevent; it
    # never matches, so a `[target.'cfg(target_os="harmony")']` section in the
    # manifest would be dead weight.
    if echo "$cfg_out" | grep -qx 'target_os="harmony"'; then
        echo "  FAIL $triple: target_os is suddenly 'harmony' — the manifest note is stale"
        fail=1
    fi
done
[[ $fail -eq 0 ]] || { echo "premise check failed"; exit 1; }

# ---------------------------------------------------------------------------
# [2]-[4] The targets rustup ships std for: build, and then LINK.
# ---------------------------------------------------------------------------
for triple in "$PRIMARY" "${LINKABLE[@]}"; do
    if ! rustup target list --installed 2>/dev/null | grep -q "^${triple}$"; then
        echo "[skip] ${triple}: rust-std not installed (rustup target add ${triple})"
        continue
    fi
    short="${triple%%-*}"

    # The backend is chosen from the target alone — no `harmony` feature. This is
    # what a real HarmonyOS application links against, so it is the case that must
    # work. A `target_os = "ohos"` spelling fails here with E0425/E0428.
    echo "[2/6] ${triple}: backend auto-selected from the target (no 'harmony' feature)"
    rw_run_bounded "$OHOS_CHECK_TIMEOUT" cargo ohos check -t "$short" --no-default-features \
        --features "desktop,touch,i18n,serde,serde_json"

    echo "[3/6] ${triple}: full feature contact surface"
    rw_run_bounded "$OHOS_CHECK_TIMEOUT" cargo ohos check -t "$short" --no-default-features --all-targets \
        --features "desktop,harmony,touch,i18n,serde,serde_json,advanced-widgets,controls-custom,controls-native"

    echo "[4/6] ${triple}: stripped profile"
    rw_run_bounded "$OHOS_CHECK_TIMEOUT" cargo ohos check -t "$short" --no-default-features --features embedded

    # `check` never links, so on its own it cannot catch a missing sysroot: the C
    # dependencies in the graph (minimp3-sys, ...) only fail at link/build time.
    # This step produces a real shared object and verifies its machine type, so a
    # silently wrong toolchain cannot pass.
    echo "[4b/6] ${triple}: build + verify the linked artifact is the right machine"
    rw_run_bounded "$OHOS_BUILD_TIMEOUT" cargo ohos build -t "$short" --no-default-features \
        --features "desktop,touch,i18n,serde,serde_json"

    so="target/${triple}/debug/librust_widgets.so"
    if [[ ! -f "$so" ]]; then
        echo "  FAIL ${triple}: no shared object at ${so}"
        fail=1
        continue
    fi
    case "$triple" in
        aarch64-*) want="AArch64" ;;
        armv7-*)   want="ARM"     ;;
        x86_64-*)  want="X86-64"  ;;
        *)         want=""        ;;
    esac
    machine="$("${OHOS_SDK_NATIVE}/llvm/bin/llvm-readelf" -h "$so" | awk -F: '/Machine/ {print $2}' | xargs)"
    if [[ "$machine" != *"$want"* ]]; then
        echo "  FAIL ${triple}: artifact machine is '${machine}', expected '${want}'"
        fail=1
    else
        note "$(basename "$so"): Machine=${machine}, $(du -h "$so" | cut -f1)  ✓"
    fi
done

# ---------------------------------------------------------------------------
# [5] Lint. Several lints only fire on the OpenHarmony std, so this is not
#     redundant with the host clippy run.
# ---------------------------------------------------------------------------
echo "[5/6] clippy on ${PRIMARY} (deny warnings)"
rw_run_bounded "$OHOS_CHECK_TIMEOUT" cargo ohos clippy -t aarch64 --no-default-features \
    --features "desktop,harmony" --all-targets -- -D warnings

# ---------------------------------------------------------------------------
# [6] loongarch64: Tier 3, so assert the *specific* expected outcome.
#
#     rustup ships no std for it (Tier 3 — "official builds are not available"),
#     and the SDK sysroot ships no loongarch64 libc: its C headers are incomplete
#     (`bits/alltypes.h` missing), so even a dependency's C compilation stops
#     before Rust is reached. `cargo-ohos` documents the same limitation
#     independently ("`loongarch64` is not supported in the latest SDK (CMake)").
#
#     The gate pins this rather than pretending either way, and flips its
#     expectation if the SDK ever ships the libc.
# ---------------------------------------------------------------------------
echo "[6/6] ${BUILD_STD_ONLY}: Tier 3 target (no prebuilt std, SDK libc incomplete)"
LOONG_LIB_DIR="${OHOS_SDK_NATIVE}/sysroot/usr/lib/loongarch64-linux-ohos"
if [[ ! -d "$LOONG_LIB_DIR" ]]; then
    note "SDK has no ${LOONG_LIB_DIR} — cannot compile C, let alone link"
    if rustc --target "$BUILD_STD_ONLY" --print cfg >/dev/null 2>&1; then
        note "rustc knows the triple — Tier 3, so no prebuilt std ✓"
    else
        echo "  FAIL: rustc no longer recognises ${BUILD_STD_ONLY}"
        fail=1
    fi
    if rustup target list --installed 2>/dev/null | grep -q "^${BUILD_STD_ONLY}$"; then
        echo "  FAIL: rust-std is now installed for ${BUILD_STD_ONLY} — strengthen step [2] to cover it"
        fail=1
    else
        note "rustup has no std for it (Tier 3: 'official builds are not available') ✓"
    fi
elif ! rustc +nightly -vV >/dev/null 2>&1; then
    note "nightly toolchain unavailable — skipped (needs -Zbuild-std)"
elif [[ -z "$(rustup component list --installed --toolchain nightly 2>/dev/null | grep rust-src || true)" ]]; then
    note "nightly rust-src unavailable — skipped (rustup component add rust-src --toolchain nightly)"
else
    note "SDK now ships a loongarch64 lib dir — full build expected"
    RUSTC_BOOTSTRAP=1 rw_run_bounded "$OHOS_BUILD_TIMEOUT" cargo +nightly check --target "$BUILD_STD_ONLY" \
        --no-default-features --features "desktop" -Zbuild-std=std,panic_abort
fi

[[ $fail -eq 0 ]] || { echo "HarmonyOS cross-target checks FAILED"; exit 1; }
echo "All HarmonyOS cross-target checks passed."
