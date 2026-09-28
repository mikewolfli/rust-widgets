#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_icon_data_is_opt_in.sh — BLUE25 ICON-8 (the feature gate)
# ============================================================================
# The rule this guards:
#
#   **The Material Symbols icon data ships in the capable device profiles
#   (`desktop`/`tablet`/`mobile`), and never in `default` or the sized profiles
#   (`embedded`/`mini`).**
#
# `desktop`/`tablet`/`mobile` list `icons`, so a host that targets one of them gets the real
# outlines without asking. `default` names `desktop` and nothing else, so a plain `cargo build`
# still gets them *through the profile*. No sized profile (`embedded`/`mini`) lists `icons`,
# because those answer "what kind of machine is this" for a machine that cannot afford a payload
# the caller did not ask for.
#
# # The two halves are different questions, which is why both are here
#
# "Is it in the capable profiles?"  and  "is it out of `default` and the sized profiles?" are not
# the same assertion, and only holding both keeps the split above true. The polarity of this gate
# has been flipped before (the feature was opt-in, then in `default`, now in the profiles); each
# time the assertions were *inverted* rather than deleted, so the contract stays checkable.
#
# # Why the profile carries it now
#
# A device profile answers "what kind of machine is this". `desktop`/`tablet`/`mobile` are hosts
# with the bytes and a reason to want real outlines; `mini`/`embedded` are *sized*. Carrying the
# data in the profiles rather than in `default` is what makes `--features desktop` — the combo this
# crate's own snapshot exporter and the CI test job use — render the same icons a plain
# `cargo build` does. Before this they disagreed: the snapshots were exported with `--features
# desktop`, which carried no icon data, so the icon controls drew the hand-drawn fallback shapes
# while a runtime built from `default` drew the real outlines.
#
# What this checks (four assertions)
# ----------------------------------
#   1. `icons` is declared in `[features]`.
#   2. `icons` is in the `desktop`, `tablet` and `mobile` lists.
#   3. `icons` is absent from `default` and from the sized profiles (`embedded`, `mini`).
#   4. Both states build: `--features desktop` (on) and a sized profile with and without the
#      feature, so the data is *reachable* and still *removable*.
#
# Reverse injection
# -----------------
# Removing `"icons"` from `desktop`, or adding it to `default`, must make this fail naming the
# list it is missing from or appeared in. See the round's report for the exact output.
#
# Usage: tools/check_icon_data_is_opt_in.sh
# Exit 0 = the data is in every capable profile, out of `default` and the sized ones, and both
# build states work.
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

echo '[2/4] `icons` is in the capable device profiles'
for profile in desktop tablet mobile; do
    list="$(feature_list "$profile")"
    if ! printf '%s' "$list" | grep -qE '"icons"'; then
        echo "  FAIL  \`icons\` is missing from the \`$profile\` feature list"
        echo '        A capable host (desktop/tablet/mobile) must get the icon outlines without'
        echo '        asking; the sized profiles (embedded/mini) are where the payload is dropped.'
        exit 1
    fi
    echo "        $profile: present"
done
echo '  PASS  present in every capable profile'

echo '[3/4] `icons` is out of `default` and the sized profiles'
for profile in default embedded mini; do
    list="$(feature_list "$profile")"
    if [ -z "$list" ]; then
        continue
    fi
    if printf '%s' "$list" | grep -qE '"icons"'; then
        echo "  FAIL  \`icons\` is in the \`$profile\` feature list"
        echo '        The icon payload belongs in the capable profiles, not in `default` and not in'
        echo '        a sized profile. `default` reaches it through `desktop`; a `desktop` build'
        echo '        drops it only by choosing a sized profile instead.'
        exit 1
    fi
    echo "        $profile: absent"
done
echo '  PASS  absent from default and the sized profiles'

echo '[4/4] both build states compile'
# The first command is a capable profile, which now carries `icons`; the second is a sized profile,
# which does not — so this asserts the data is *reachable* and still *removable*.
rw_run_bounded 900 cargo check --no-default-features --features desktop --lib
rw_run_bounded 900 cargo check --no-default-features --features mini --lib
echo '  PASS  builds with the feature on (desktop) and off (mini)'

echo
echo "check_icon_data_is_opt_in: OK"
