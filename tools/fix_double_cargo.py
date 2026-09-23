#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Fix the double-`cargo` the cache rewrite introduced.

`rw_cargo_cached` invokes `cargo` itself, so the argument list must start at the subcommand. The
mechanical rewrite `rw_run_bounded "$BUDGET" cargo check …` -> `rw_cargo_cached "$BUDGET" cargo
check …` left the word `cargo` in the args, which ran `cargo cargo check` and failed with a
cargo-install help message rather than a compile error — caught by running the gate, not by reading
the diff, which is why every rewritten gate is executed at least once afterwards.
"""

import pathlib
import re

BAD = re.compile(r'rw_cargo_cached(\s+"\$[A-Z_]+")\s+cargo\s+')
# The same mistake in the bare form added for a compile-only probe.
BAD_BARE = re.compile(r"rw_cargo_cached(\s+)\s*cargo\s+")


def main() -> None:
    total = 0
    for path in sorted(pathlib.Path("tools").glob("check_*.sh")):
        original = path.read_text(encoding="utf-8")
        text, count = BAD.subn(lambda m: f"rw_cargo_cached{m.group(1)} ", original)
        text, extra = BAD_BARE.subn(lambda m: f"rw_cargo_cached{m.group(1)}", text)
        count += extra
        if text != original:
            path.write_text(text, encoding="utf-8")
            total += count
            print(f"  {count:3d}  {path.name}")
    print(f"fixed: {total}")


if __name__ == "__main__":
    main()
