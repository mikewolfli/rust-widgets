#!/usr/bin/env bash
# Theme fixture gate.
#
# `themes/*.json` is the checked-in description of the theme file format — the
# public API of a theme file. Nothing else in the build reads it, so without this
# gate a field rename or a serde change could break every user's theme file while
# every test stayed green (principle #18: the docs and the code must agree; a
# fixture is documentation that claims to be loadable).
#
#   [1] PRESENT     every built-in preset has a checked-in fixture.
#   [2] ROUND-TRIP  each fixture loads back into a `Theme` and resolves a style,
#                   and a token round-trips through every field (including the
#                   three-way shadow override, which is where a nested `Option`
#                   silently lost information before).
#   [3] IN SYNC     regenerating from the presets produces byte-identical files.
#                   This is what stops a fixture from drifting: it is generated,
#                   so "regenerate and diff" is the exact definition of "in sync".
#
# Usage: tools/check_theme_fixtures.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="${PROFILE:-desktop}"
fail=0

echo "[1/3] preset fixtures are present"
for preset in default dark; do
    if [[ -f "themes/${preset}.json" ]]; then
        echo "  themes/${preset}.json: present"
    else
        echo "  themes/${preset}.json: MISSING"
        fail=1
    fi
done

echo
echo "[2/3] fixtures round-trip (test/theme_fixture_test.rs)"
if ! cargo test --no-default-features --features "$PROFILE" --test theme_fixture_test; then
    echo "FAIL: a theme fixture does not round-trip."
    fail=1
fi

echo
echo "[3/3] fixtures are in sync with the built-in presets"
# Snapshot, regenerate, diff. `themes/generate.sh` writes through the library's own
# `save_theme`, so an in-sync fixture is byte-identical by construction.
snapshot_dir="$(mktemp -d)"
trap 'rm -rf "$snapshot_dir"' EXIT
for preset in default dark; do
    cp "themes/${preset}.json" "${snapshot_dir}/${preset}.json"
done

if ! bash themes/generate.sh > /dev/null 2>&1; then
    echo "FAIL: themes/generate.sh failed; cannot verify the fixtures are in sync."
    # Restore before failing so the working tree is left as it was found.
    for preset in default dark; do
        cp "${snapshot_dir}/${preset}.json" "themes/${preset}.json"
    done
    fail=1
else
    for preset in default dark; do
        if ! diff -q "${snapshot_dir}/${preset}.json" "themes/${preset}.json" > /dev/null; then
            echo "  themes/${preset}.json: OUT OF DATE (regenerate with themes/generate.sh)"
            diff -u "${snapshot_dir}/${preset}.json" "themes/${preset}.json" || true
            fail=1
        else
            echo "  themes/${preset}.json: in sync"
        fi
    done
fi

echo
if [[ "$fail" -ne 0 ]]; then
    echo "Theme fixture checks FAILED."
    echo "  Regenerate with: themes/generate.sh"
    exit 1
fi
echo "Theme fixture checks passed."
