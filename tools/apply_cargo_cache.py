#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Point the heavy gates at the shared cargo cache.

# Why a script rather than twenty edits

Every heavy gate already routes its `cargo` steps through one uniform shape:

    rw_run_bounded "$PROFILE_TIMEOUT" cargo check --no-default-features --features mini …

so the change is mechanical, and doing it by hand across twenty files is how a call site gets
missed. This tool rewrites the shape, sources `lib_cargo_cache.sh` next to the `lib_timeout.sh` the
gates already source, and reports per-file counts so a zero is visible rather than silent.

# What it does not touch

`cargo test` steps. A test run's verdict depends on the *test binary*, and while the cache would key
that correctly, a test step's value is the assertion output a human reads when it fails — replaying
a stale-looking log for a passing test is worse than re-running it. The gate's own `run_test_case`
wrapper already guards the "zero tests matched" failure mode; leaving tests uncached keeps that
guard's evidence live.
"""

import pathlib
import re

GATES = [
    "check_profiles.sh",
    "check_behavior_matrix.sh",
    "check_view_platform_gate.sh",
    "check_generator_output_compiles.sh",
    "check_designer_feature_gate.sh",
    "check_declared_targets_ship.sh",
    "check_generated_sources.sh",
    "check_web_engine_honest.sh",
    "check_visual_regression.sh",
    "check_control_rendering.sh",
    "check_svg_snapshots.sh",
    "check_mode_consistency.sh",
    "check_theme_fixtures.sh",
    "check_jni_signatures.sh",
    "check_designer_manifest_roundtrip.sh",
    "check_declaration_implementation_alignment.sh",
    "check_binding_symbol_coverage.sh",
    "check_apple_native.sh",
    "check_ios_cross.sh",
    "check_harmony_cross.sh",
]

# The uniform shape these gates use for a bounded `cargo check`/`cargo build` step.
BOUNDED_CARGO = re.compile(r'rw_run_bounded\s+"\$([A-Z_]+)"\s+cargo\b')
# …and the bare form, for a gate whose budget is the helper's own default.
BOUNDED_CARGO_BARE = re.compile(r"rw_run_bounded\s+cargo\b")

SOURCE_MARKER = '. "$ROOT_DIR/tools/lib_timeout.sh"'
SOURCE_WITH_CACHE = (
    '. "$ROOT_DIR/tools/lib_timeout.sh"\n. "$ROOT_DIR/tools/lib_cargo_cache.sh"'
)


def main() -> None:
    for name in GATES:
        path = pathlib.Path("tools") / name
        if not path.exists():
            print(f"  ABSENT  {name}")
            continue
        original = path.read_text(encoding="utf-8")
        text = original

        text, rewritten = BOUNDED_CARGO.subn(lambda m: f'rw_cargo_cached "${m.group(1)}" cargo', text)
        text, bare = BOUNDED_CARGO_BARE.subn('rw_cargo_cached "$RW_TIMEOUT_DEFAULT" cargo', text)
        rewritten += bare

        if rewritten and "lib_cargo_cache.sh" not in text:
            if SOURCE_MARKER not in text:
                print(f"  NO ANCHOR  {name} (could not find lib_timeout.sh to source beside)")
                continue
            text = text.replace(SOURCE_MARKER, SOURCE_WITH_CACHE, 1)

        if text != original:
            path.write_text(text, encoding="utf-8")
        flag = "" if rewritten else "   <-- nothing rewritten, check manually"
        print(f"  {rewritten:3d} step(s)  {name}{flag}")


if __name__ == "__main__":
    main()
