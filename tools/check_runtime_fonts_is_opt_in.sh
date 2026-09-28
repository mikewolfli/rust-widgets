#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_runtime_fonts_is_opt_in — the runtime face registry ships no data and stays removable.
#
#     **`runtime-fonts` adds no font payload, and the crate builds with and without it.**
#
# # Why this gate exists
#
# `runtime-fonts` is the one font feature that is not about *data*: it adds an entry point
# (`register_face`) and a four-slot registry, and the bytes are the host's. That means it must never
# grow into a data feature by accident (a compiled default face, a bundled fallback), and it must
# stay genuinely optional — a build without it must not gain the registry, the mutex, or the
# coverage-cache invalidation that only the registry can trigger.
#
# # What this checks (three assertions)
#
#   1. `runtime-fonts` is a declared feature and implies `text-shaping`.
#   2. The crate builds with it **on** and with it **off**, both with a vector face so the shaper's
#      face list is non-empty.
#   3. The registry is absent from a build without the feature — the public entry point does not
#      resolve, which is the property "opt-in" is supposed to mean.
#
# # Reverse injection
#
# Removing `text-shaping` from `runtime-fonts` in `Cargo.toml` must make step 1 fail. See the round's
# report for the exact output.
#
# Principle #58: every command runs under a timeout, via `rw_run_bounded`.

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

# shellcheck source=tools/lib_timeout.sh
source tools/lib_timeout.sh

MANIFEST="Cargo.toml"

echo "[1/3] \`runtime-fonts\` is declared and implies \`text-shaping\`"
if ! grep -qE '^runtime-fonts = \["text-shaping"\]' "$MANIFEST"; then
    echo "  FAIL  Cargo.toml does not declare \`runtime-fonts = [\"text-shaping\"]\`"
    echo "        The registry reads a face's outlines and layout tables, so it needs the shaper;"
    echo "        a runtime face on a build with no shaper has no consumer."
    exit 1
fi
echo "  PASS"

echo "[2/3] the crate builds with the feature on and off"
rw_run_bounded 900 cargo check --no-default-features \
    --features "desktop,fonts-vector-latin,runtime-fonts" --lib || {
    echo "  FAIL  the build with \`runtime-fonts\` does not compile"
    exit 1
}
rw_run_bounded 900 cargo check --no-default-features \
    --features "desktop,fonts-vector-latin" --lib || {
    echo "  FAIL  the build without \`runtime-fonts\` does not compile"
    exit 1
}
echo "  PASS"

echo "[3/3] the registry is absent when the feature is off"
# A probe crate would be the thorough way to do this; compiling the *library* with the feature off
# and asserting the symbol is not in its metadata is the cheap version. `rustdoc` grepping exports
# is not available for a private registry, so the assertion is on the source gate: the public
# re-export must be `#[cfg]`-gated on the feature.
if ! grep -qE '#\[cfg\(all\(feature = "runtime-fonts", feature = "text-shaping"\)\)\]' \
        src/render/text/mod.rs; then
    echo "  FAIL  the \`register_face\` re-export is not gated on \`runtime-fonts\`"
    echo "        A build without the feature must not expose the registry."
    exit 1
fi
echo "  PASS"

echo
echo "check_runtime_fonts_is_opt_in: OK"
