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
. "$ROOT_DIR/tools/lib_timeout.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"

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
if ! rw_cargo_cached "${RW_GATE_TIMEOUT:-900}" test --no-default-features --features "$PROFILE" --test theme_fixture_test; then
    echo "FAIL: a theme fixture does not round-trip."
    fail=1
fi

echo
echo "[3/3] fixtures are in sync with the built-in presets"
# Snapshot, regenerate, diff. `themes/generate.sh` writes through the library's own
# `save_theme`, so an in-sync fixture is byte-identical by construction.
#
# # Why every theme file is snapshotted, while only two are compared
#
# The generator writes **all** presets, and two of them (`macos-*.json`) carry the host's system UI
# family — `SF Pro` on macOS, a fallback elsewhere — so comparing them would make this gate fail on
# one host and pass on another. They are therefore not compared. They *are* snapshotted and
# restored: without that, running the gate on macOS rewrote them (`Arial` → `SF Pro`) and left the
# working tree dirty, which is a check that changes what it checks. Restoring every file the
# generator touches is what lets the gate be read-only from the caller's point of view.
snapshot_dir="$(mktemp -d)"
trap 'rm -rf "$snapshot_dir"' EXIT
for file in themes/*.json; do
    cp "$file" "${snapshot_dir}/$(basename "$file")"
done

if ! bash themes/generate.sh > /dev/null 2>&1; then
    echo "FAIL: themes/generate.sh failed; cannot verify the fixtures are in sync."
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

# Put back every file the generator wrote, so the gate leaves the tree exactly as it found it —
# both on success and on the failure paths above.
for file in themes/*.json; do
    cp "${snapshot_dir}/$(basename "$file")" "$file"
done

echo
if [[ "$fail" -ne 0 ]]; then
    echo "Theme fixture checks FAILED."
    echo "  Regenerate with: themes/generate.sh"
    exit 1
fi
echo "Theme fixture checks passed."
