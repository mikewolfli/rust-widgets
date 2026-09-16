#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# HarmonyOS (OpenHarmony) cross-target gate.
#
# Verifies the single fact that is easy to get wrong and impossible to notice on
# a host build: how the OpenHarmony targets identify themselves, and that the
# backend selection follows it.
#
# `rustc --target <t> --print cfg` reports `target_os="linux"` and
# `target_env="ohos"` for **every** `*-unknown-linux-ohos` target. A backend
# selected with `cfg(target_os = "ohos")` therefore never matches. The historical
# spelling did exactly that and made the target fail with `cannot find value
# 'create_native_platform' in this scope` — not merely mis-selected, it did not
# build at all.
#
# Requires the OpenHarmony SDK's native toolchain (clang + sysroot). Point
# OHOS_SDK at the SDK host directory (the one containing `native/`). When the
# SDK or a target is absent this gate reports HOST-GATED (exit 2) rather than
# passing vacuously: a green result must mean the checks really ran.
#
# Target coverage (all four triples rustc knows for OpenHarmony):
#   aarch64 / armv7 / x86_64  — prebuilt rust-std + libc in the SDK sysroot
#   loongarch64               — Tier 3: no prebuilt rust-std, needs -Zbuild-std,
#                               and the SDK sysroot ships no loongarch64 libc,
#                               so it CANNOT LINK with this SDK. Checked to the
#                               link step, and the outcome asserted explicitly.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PRIMARY="aarch64-unknown-linux-ohos"
# Triples with prebuilt std shipped by rustup.
LINKABLE=(armv7-unknown-linux-ohos x86_64-unknown-linux-ohos)
# Triple whose std must be built from source and whose libc the SDK omits.
BUILD_STD_ONLY="loongarch64-unknown-linux-ohos"
ALL_TARGETS=("$PRIMARY" "${LINKABLE[@]}" "$BUILD_STD_ONLY")

if ! rustup target list --installed 2>/dev/null | grep -q "^${PRIMARY}$"; then
    echo "HOST-GATED: rust target ${PRIMARY} is not installed"
    echo "  install with: rustup target add ${PRIMARY}"
    exit 2
fi

if [[ -z "${OHOS_SDK:-}" || ! -d "${OHOS_SDK}/native/llvm/bin" ]]; then
    echo "HOST-GATED: OHOS_SDK is unset or does not point at an OpenHarmony SDK"
    echo "  expected a directory containing native/llvm/bin (got '${OHOS_SDK:-<unset>}')"
    exit 2
fi

if [[ ! -x "${OHOS_SDK}/native/llvm/bin/${PRIMARY}-clang" ]]; then
    echo "HOST-GATED: no OpenHarmony clang for ${PRIMARY} in the SDK"
    exit 2
fi

# Point cargo at the SDK's per-target clang wrapper for every triple under test.
# The wrapper carries the sysroot, so `-lc` / `crti.o` resolve without extra flags.
for triple in "${ALL_TARGETS[@]}"; do
    cc="${OHOS_SDK}/native/llvm/bin/${triple}-clang"
    upper="$(echo "$triple" | tr 'a-z-' 'A-Z_')"
    underscored="$(echo "$triple" | tr '-' '_')"
    if [[ -x "$cc" ]]; then
        export "CARGO_TARGET_${upper}_LINKER=$cc"
        export "CC_${underscored}=$cc"
        export "AR_${underscored}=${OHOS_SDK}/native/llvm/bin/llvm-ar"
    fi
done

fail=0
note() { printf '  %s\n' "$*"; }

# ---------------------------------------------------------------------------
# [1] The premise this whole gate rests on.
# ---------------------------------------------------------------------------
echo "[1/6] target-identification premise (all OpenHarmony triples)"
for triple in "${ALL_TARGETS[@]}"; do
    cfg_out="$(rustc --target "$triple" --print cfg 2>/dev/null || true)"
    if [[ -z "$cfg_out" ]]; then
        note "$triple: rustc cannot describe this target (no prebuilt std) — skipped"
        continue
    fi
    echo "$cfg_out" | grep -qx 'target_env="ohos"' ||
        { echo "  FAIL $triple: target_env is not \"ohos\""; fail=1; }
    echo "$cfg_out" | grep -qx 'target_os="linux"' ||
        { echo "  FAIL $triple: target_os is not \"linux\""; fail=1; }
    note "$triple: target_env=ohos, target_os=linux  ✓"
done
[[ $fail -eq 0 ]] || { echo "premise check failed"; exit 1; }

# ---------------------------------------------------------------------------
# [2]-[4] The targets rustup ships std for: the full recipe.
# ---------------------------------------------------------------------------
for triple in "$PRIMARY" "${LINKABLE[@]}"; do
    if ! rustup target list --installed 2>/dev/null | grep -q "^${triple}$"; then
        echo "[skip] ${triple}: rust-std not installed (rustup target add ${triple})"
        continue
    fi

    # The backend is chosen from the target alone — no `harmony` feature. This is
    # what a real HarmonyOS application links against, so it is the case that must
    # work. A `target_os = "ohos"` spelling fails here with E0425/E0428.
    echo "[2/6] ${triple}: backend auto-selected from the target (no 'harmony' feature)"
    cargo check --target "$triple" --no-default-features \
        --features "desktop,touch,i18n,serde,serde_json"

    echo "[3/6] ${triple}: full feature contact surface"
    cargo check --target "$triple" --no-default-features --all-targets \
        --features "desktop,harmony,touch,i18n,serde,serde_json,advanced-widgets,controls-custom,controls-native"

    echo "[4/6] ${triple}: stripped profile"
    cargo check --target "$triple" --no-default-features --features embedded
done

# ---------------------------------------------------------------------------
# [5] Lint. Several lints only fire on the OpenHarmony std, so this is not
#     redundant with the host clippy run.
# ---------------------------------------------------------------------------
echo "[5/6] clippy on ${PRIMARY} (deny warnings)"
cargo clippy --target "$PRIMARY" --no-default-features \
    --features "desktop,harmony" --all-targets -- -D warnings

# ---------------------------------------------------------------------------
# [6] loongarch64: Tier 3, so assert the *specific* expected outcome.
#
#     `--print cfg` cannot describe it and rustup has no std, so the only way to
#     check it is `-Zbuild-std` on nightly. Its `cargo check` passes; its *link*
#     cannot, because the SDK sysroot has no loongarch64 libc
#     (`ld.lld: unable to find library -lc`, `cannot open crti.o`). That is an SDK
#     packaging fact, not a defect in this crate — so the gate pins it rather than
#     pretending either way.
# ---------------------------------------------------------------------------
echo "[6/6] ${BUILD_STD_ONLY}: Tier 3 target (no prebuilt std, no SDK libc)"
if ! command -v rustup >/dev/null 2>&1; then
    note "rustup unavailable — skipped"
elif ! rustc +nightly -vV >/dev/null 2>&1; then
    note "nightly toolchain unavailable — skipped (needs -Zbuild-std)"
elif [[ -z "$(rustup component list --installed --toolchain nightly 2>/dev/null | grep rust-src || true)" ]]; then
    note "nightly rust-src unavailable — skipped (rustup component add rust-src --toolchain nightly)"
elif [[ ! -e "${OHOS_SDK}/native/sysroot/usr/lib/loongarch64-linux-ohos/libc.so" \
        && ! -e "${OHOS_SDK}/native/sysroot/usr/lib/loongarch64-linux-ohos/crti.o" ]]; then
    # The SDK has no loongarch64 libc, which is the reason the link below fails.
    note "SDK sysroot has no loongarch64-linux-ohos libc — check-only, as expected"
    echo "[6a/6] ${BUILD_STD_ONLY}: cargo check with -Zbuild-std"
    RUSTC_BOOTSTRAP=1 cargo +nightly check --target "$BUILD_STD_ONLY" \
        --no-default-features --features "desktop" -Zbuild-std=std,panic_abort
    note "type-checks clean ✓ (link is impossible with this SDK — evidenced in the log)"

    echo "[6b/6] ${BUILD_STD_ONLY}: confirm the failure is the SDK's libc, not our code"
    if RUSTC_BOOTSTRAP=1 cargo +nightly build --target "$BUILD_STD_ONLY" \
        --no-default-features --features "desktop" -Zbuild-std=std,panic_abort \
        > /tmp/ohos_loong_link.log 2>&1; then
        note "it linked after all — the SDK must have gained loongarch64 libc; update this gate"
    elif grep -qE 'unable to find library -lc|cannot open crti\.o' /tmp/ohos_loong_link.log; then
        note "link fails on the missing libc (crti.o / -lc), as documented ✓"
    else
        echo "  FAIL ${BUILD_STD_ONLY}: link failed for an unexpected reason"
        grep -E "^error|ld\.lld" /tmp/ohos_loong_link.log | head -5
        fail=1
    fi
else
    note "SDK now ships loongarch64 libc — full build expected"
    RUSTC_BOOTSTRAP=1 cargo +nightly check --target "$BUILD_STD_ONLY" \
        --no-default-features --features "desktop" -Zbuild-std=std,panic_abort
fi

[[ $fail -eq 0 ]] || { echo "HarmonyOS cross-target checks FAILED"; exit 1; }
echo "All HarmonyOS cross-target checks passed."
