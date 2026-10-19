#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Redox OS (Orbital) cross-target check.
#
# # Why this gate is different from the other cross gates
#
# Windows, macOS, iOS, Android, HarmonyOS and wasm all have prebuilt `std` from rustup, so their
# gates are ordinary `cargo check --target <triple>` runs. Redox targets are **Tier 3**: rustc knows
# the triple, but rustup ships no `std`, so the only way to type-check the backend is to build the
# standard library from source (`-Z build-std=std,panic_abort`) on nightly.
#
# # The feature set is part of the judgement
#
# The backend itself only needs `desktop-runtime` + `controls-custom`; the full `desktop` profile
# additionally pulls `minimp3_fixed -> slice-ring-buffer`, which uses SysV IPC constants that
# Redox's libc does not define. That is a dependency incompatibility, not a defect in this crate, so
# the gate checks the surface the backend actually uses and says which set it checked. A future
# audio backend on Redox will need a different dependency, and this line is where that shows up.
#
# # Host limitation is a SKIP, not a timeout
#
# No nightly, no `rust-src`, or no `build-std` support means the gate cannot answer the question at
# all. It prints `unsupported host (...)` and exits 2 so the runner records `SKIP(host-limited)`
# (principle #103). Only a failure of the crate under test exits 1.
#
# rw-gate-budget: 2400

set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR" || exit 1

# shellcheck source=tools/lib_timeout.sh
. "$ROOT_DIR/tools/lib_timeout.sh"

TARGETS=(x86_64-unknown-redox aarch64-unknown-redox)
PROFILE_FEATURES="desktop-runtime,controls-custom"
BUDGET=2400

if ! command -v rustup >/dev/null 2>&1; then
  echo "unsupported host (rustup is not installed; a Redox target has no prebuilt std)"
  exit 2
fi

if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "unsupported host (no nightly toolchain; -Z build-std is required for a Tier 3 Redox target)"
  exit 2
fi

if ! rustup component list --toolchain nightly --installed 2>/dev/null | grep -q '^rust-src'; then
  echo "unsupported host (nightly has no rust-src; build-std needs the standard-library sources)"
  exit 2
fi

echo "=== Redox/Orbital targets: ${#TARGETS[@]} triple(s), -Z build-std, features: $PROFILE_FEATURES ==="

failures=0
for target in "${TARGETS[@]}"; do
  echo "--- $target"
  if rw_run_bounded "$BUDGET" cargo +nightly check \
    --target "$target" \
    -Z build-std=std,panic_abort \
    --no-default-features \
    --features "$PROFILE_FEATURES" \
    --lib; then
    :
  else
    status=$?
    if [ "$status" -eq 124 ]; then
      echo "   TIMEOUT after ${BUDGET}s: the check did not finish; this is not a pass and not a code failure"
      echo "   (re-run with a larger budget before drawing a conclusion — principle #104)"
      exit 2
    fi
    failures=$((failures + 1))
  fi
done

if [ "$failures" -ne 0 ]; then
  echo
  echo "redox cross-target checks failed: $failures target(s)"
  exit 1
fi

echo
echo "  note: type-checking is the most this host can do. Running the backend needs a Redox"
echo "        installation with an Orbital session; the protocol calls are compiled but not executed."
echo
echo "redox cross-target checks passed."
