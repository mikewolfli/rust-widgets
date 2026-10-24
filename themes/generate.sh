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
# The macOS presets name the host's system UI face only when the runtime face registry can
# actually register it (P2-3). Generating with `runtime-fonts` is what lets a macOS host write
# `macos-light.json` / `macos-dark.json` that carry that family; on a host without the face the
# same presets honestly fall back to the compiled family. The flag changes no other preset field.
GENERATOR_FEATURES="$PROFILE,runtime-fonts"

mkdir -p themes

# Both temporary files are created with `mktemp` rather than at a fixed `/tmp/rw_generate_themes.rs`
# and `tests/tmp_generate_themes.rs`. The fixed paths were racy (a concurrent run clobbers the file
# the other is compiling) and the `tests/` one left a dirty file behind on an abnormal exit. The
# generated test must still live under `tests/` so cargo discovers it. Create a unique
# extensionless file first: BSD and GNU `mktemp` both require the X template at the end. Then add
# `.rs` by renaming the reserved path; the trap removes either name on every exit path.
TMP_RS_BASE="$(mktemp tests/tmp_generate_themes_XXXXXX)"
TMP_RS="${TMP_RS_BASE}.rs"
trap 'rm -f "$TMP_RS_BASE" "$TMP_RS"' EXIT
mv "$TMP_RS_BASE" "$TMP_RS"
# Cargo derives the test target name from the file stem.
TEST_NAME="$(basename "$TMP_RS" .rs)"

cat > "$TMP_RS" << 'RUST'
//! Fixture generator. Writes each built-in preset through the same
//! `save_theme` the library exposes, so the output is exactly what the loader
//! will accept.
#[test]
fn generate() {
    use rust_widgets::theme::{Theme, ThemeManager};

    let mut manager = ThemeManager::new();
    manager.register_theme(Theme::dark());
    // The macOS presets are registered here, from their constructors, rather than checked in as
    // hand-written JSON: the files then cannot disagree with the structs they describe. The two
    // presets record the host facts they read (accent, metrics profile, system UI family), so the
    // generated files describe the machine the generator ran on.
    manager.register_theme(Theme::macos_light());
    manager.register_theme(Theme::macos_dark());

    for name in ["default", "dark", "macos-light", "macos-dark"] {
        assert!(manager.set_theme(name), "preset {name} must be registered");
        let path = format!("themes/{name}.json");
        manager.save_theme(&path).unwrap_or_else(|e| panic!("saving {path}: {e}"));
        println!("wrote {path}");
    }
}
RUST

cargo test --no-default-features --features "$GENERATOR_FEATURES" --test "$TEST_NAME" -- --nocapture
