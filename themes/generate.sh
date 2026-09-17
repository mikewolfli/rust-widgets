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

cat > /tmp/rw_generate_themes.rs << 'RUST'
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

cp /tmp/rw_generate_themes.rs tests/tmp_generate_themes.rs
trap 'rm -f tests/tmp_generate_themes.rs' EXIT

cargo test --no-default-features --features "$PROFILE" --test tmp_generate_themes -- --nocapture
