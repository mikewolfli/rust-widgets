#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_icon_data_is_opt_in.sh — BLUE25 ICON-8 (the feature gate)
# ============================================================================
# The rule this guards:
#
#   **The Material Symbols icon data is on by default, and never inherited by a device profile.**
#
# `default = ["desktop", "icons"]`, so a plain `cargo build` gets the real outlines. No profile
# (`desktop` / `tablet` / `mobile` / `embedded` / `mini`) lists `icons`, because a profile answers
# "what kind of machine is this" and `mini`/`embedded` are *sized* — a payload the caller did not
# ask for is exactly wrong there. A profile build that wants icons asks for them.
#
# # The two halves are different questions, which is why both are here
#
# "Is it in `default`?"  and  "is it in a profile?" are not the same assertion, and only holding
# both keeps the split above true. An earlier revision of this gate asserted the *opposite* first
# half ("absent from `default`") when the feature was opt-in; the assertion is inverted here
# rather than deleted, so the contract stays checkable either way round.
#
# # Why the polarity of this feature changed
#
# It was opt-in while the data was unproven: the outlines were newly vendored, the two renderers
# had just been reconciled, and a default nobody had looked at was the wrong risk. Once the census,
# the distinctness check and the licence chain were in place the data was no longer the unknown —
# so the useful default is "on", and a caller who wants the hand-drawn shapes back still has one
# (a build without the feature), because `Icon::draw` keeps that path for exactly this reason.
#
# What this checks (four assertions)
# ----------------------------------
#   1. `icons` is declared in `[features]`.
#   2. `icons` **is** in the `default` list.
#   3. `icons` is absent from every device-profile list.
#   4. Both states build: `--features desktop` (off) and `--features desktop,icons` (on).
#      The second is what proves the data is *reachable*; the first is what proves it is still
#      *removable*, which is the property a caller who does not want the payload depends on.
#
# Reverse injection
# -----------------
# Removing `"icons"` from `default`, or adding it to `desktop`, must make this fail naming the
# list it is missing from or appeared in. See the round's report for the exact output.
#
# Usage: tools/check_icon_data_is_opt_in.sh
# Exit 0 = the data is on by default, absent from every profile, and both build states work.
# Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_timeout.sh"

MANIFEST="Cargo.toml"

echo "[1/4] \`icons\` is a declared feature"
if ! grep -qE '^icons = \[\]' "$MANIFEST"; then
    echo "  FAIL  Cargo.toml no longer declares \`icons = []\`"
    echo "        The assertions below can only prove a feature that exists."
    exit 1
fi
echo "  PASS  declared"

# Read a feature's value list from the manifest: everything between `name = [` and the first `]`.
feature_list() {
    local name="$1"
    awk -v name="$name" '
        $0 ~ "^" name " = \\[" { capture = 1 }
        capture { printf "%s", $0; if (index($0, "]")) { exit } }
    ' "$MANIFEST"
}

echo '[2/4] `icons` is in `default`'
default_list="$(feature_list default)"
if ! printf '%s' "$default_list" | grep -qE '"icons"'; then
    echo '  FAIL  `icons` is missing from the `default` feature list'
    echo '        A plain `cargo build` must get the icon outlines; a caller who does not want'
    echo '        the payload builds without the feature instead.'
    exit 1
fi
echo "  PASS  present in default"

echo '[3/4] `icons` is not in any device profile'
for profile in desktop tablet mobile embedded mini; do
    list="$(feature_list "$profile")"
    if [ -z "$list" ]; then
        continue
    fi
    if printf '%s' "$list" | grep -qE '"icons"'; then
        echo "  FAIL  \`icons\` is in the \`$profile\` feature list"
        echo "        \`$profile\` is a device profile; the icon payload must be asked for, not"
        echo "        inherited. Use \`--features $profile,icons\`."
        exit 1
    fi
    echo "        $profile: absent"
done
echo "  PASS  absent from every device profile"

echo "[4/4] both build states compile"
# The first command is the important one now: with `icons` in `default`, `--no-default-features`
# is how a caller *rejects* the payload, so this asserts that path still exists. The second
# asserts the data is reachable when asked for by name.
rw_run_bounded 900 cargo check --no-default-features --features desktop --lib
rw_run_bounded 900 cargo check --no-default-features --features "desktop,icons" --lib
echo "  PASS  builds with the feature off and on"

echo
echo "check_icon_data_is_opt_in: OK"
