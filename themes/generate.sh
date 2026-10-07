#!/usr/bin/env bash
# Regenerate the theme fixtures in `themes/` from the library's built-in presets.
#
# The fixtures exist so the theme JSON format has a checked-in example and a test
# that keeps it honest (see `tests/theme_fixture_test.rs`). They are generated
# rather than hand-written: a hand-written file is a second description of the
# schema, and it drifts the moment a field changes. Generating from
# `Theme::default()` / `Theme::dark()` means the file cannot disagree with the
# structs by construction.
#
# Requires the `save_theme` path, which is gated on a real device profile.
#
# Usage: themes/generate.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="desktop"

mkdir -p themes

# Both temporary files are created with `mktemp` rather than at a fixed `/tmp/rw_generate_themes.rs`
# and `tests/tmp_generate_themes.rs`. The fixed paths were racy (a concurrent run clobbers the file
# the other is compiling) and the `tests/` one left a dirty file behind on an abnormal exit. The
# generated test must still live under `tests/` so cargo discovers it, so the `mktemp` file is
# written into `tests/` (its name is unique), and the trap removes it on every exit path.
TMP_RS="$(mktemp tests/tmp_generate_themes_XXXXXX.rs)"
# cargo derives the test target name from the file stem; strip the leading `tmp_` and the `.rs`.
TEST_NAME="$(basename "$TMP_RS" .rs)"
trap 'rm -f "$TMP_RS"' EXIT

cat > "$TMP_RS" << 'RUST'
//! Fixture generator. Writes each built-in preset through the same
//! `save_theme` the library exposes, so the output is exactly what the loader
//! will accept.
#[test]
fn generate() {
    use rust_widgets::theme::{Theme, ThemeManager};

    let mut manager = ThemeManager::new();
    manager.register_theme(Theme::dark());

    for name in ["default", "dark"] {
        assert!(manager.set_theme(name), "preset {name} must be registered");
        let path = format!("themes/{name}.json");
        manager.save_theme(&path).unwrap_or_else(|e| panic!("saving {path}: {e}"));
        println!("wrote {path}");
    }
}
RUST

cargo test --no-default-features --features "$PROFILE" --test "$TEST_NAME" -- --nocapture
