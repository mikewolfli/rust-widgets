#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Cache the remaining *build* steps that the uniform rewrite could not reach.

Three gates invoke cargo in a shape the general rewriter did not match: a bare `cargo build` whose
only product is an artifact, a `cargo package --list`, and a `cargo test --no-run` (which compiles
a probe but runs no test — its verdict is "does it compile", so caching it is safe and its output
carries no assertion evidence).

`cargo test` steps that *run* tests are deliberately left alone: see `apply_cargo_cache.py`.
"""

import pathlib

EDITS = {
    "check_binding_symbol_coverage.sh": [
        (
            "cargo build --lib --no-default-features --features desktop >/dev/null 2>&1",
            'rw_cargo_cached "$RW_TIMEOUT_DEFAULT" build --lib '
            "--no-default-features --features desktop >/dev/null 2>&1",
        )
    ],
    "check_declared_targets_ship.sh": [
        (
            "rw_run_bounded 300 cargo package --list --allow-dirty",
            "rw_cargo_cached 300 package --list --allow-dirty",
        )
    ],
    "check_view_platform_gate.sh": [
        (
            'out="$(cargo test --test view_platform_gate_probe --no-run "$@" 2>&1)"',
            'out="$(rw_cargo_cached "$RW_TIMEOUT_DEFAULT" test '
            '--test view_platform_gate_probe --no-run "$@" 2>&1)"',
        )
    ],
}

SOURCE_MARKER = '. "$ROOT_DIR/tools/lib_timeout.sh"'
SOURCE_WITH_CACHE = (
    '. "$ROOT_DIR/tools/lib_timeout.sh"\n. "$ROOT_DIR/tools/lib_cargo_cache.sh"'
)


def main() -> None:
    for name, pairs in EDITS.items():
        path = pathlib.Path("tools") / name
        if not path.exists():
            print(f"  ABSENT  {name}")
            continue
        text = path.read_text(encoding="utf-8")
        hits = 0
        for old, new in pairs:
            if old in text:
                text = text.replace(old, new)
                hits += 1
            else:
                print(f"  MISS    {name}: {old[:64]}")
        if hits and "lib_cargo_cache.sh" not in text:
            if SOURCE_MARKER in text:
                text = text.replace(SOURCE_MARKER, SOURCE_WITH_CACHE, 1)
            else:
                print(f"  NO ANCHOR  {name}")
                continue
        path.write_text(text, encoding="utf-8")
        print(f"  {hits} rewritten  {name}")


if __name__ == "__main__":
    main()
