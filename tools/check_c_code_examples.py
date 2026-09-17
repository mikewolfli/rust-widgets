#!/usr/bin/env python3
"""Compile the cookbook's C code blocks and check them against the real ABI.

Why this exists
---------------

`check_cookbook.sh` verifies *Rust* declarations and builds the books, but it
explicitly does not compile prose code blocks. `check_abi.sh` checks the
published header against the Rust exports, but never reads the cookbook. So the
C a reader is told to write was, until now, checked by nothing at all -- which is
how `#include "rust_widgets.h"` and the `ObjectId` type alias both survived in the
prose while existing nowhere in the tree.

Two directions of checking
--------------------------

1. **Syntax** (`--compile`): every ```c block is compiled with `cc -fsyntax-only`
   as its own translation unit. A block that omits the standard includes (most of
   them do, for brevity) gets them from a prelude; nothing else is injected, so a
   block must declare what it uses or include the published header.

2. **Agreement** (`--agree`): every `rw_*` identifier a block mentions must exist
   in the published header. Syntax checking alone accepts a block that invents a
   function or misspells one, so this is what ties the prose to the contract.

Skipping a block
----------------

A block that is deliberately illustrative is skipped by putting
``<!-- c-example: skip -->`` on the line before its fence. Inline ``...`` is not
enough, because ellipses also appear in blocks that are meant to compile, and a
silent skip is indistinguishable from a block nobody checked. Every skip is
printed so the exemption list stays visible.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass

# Fences we treat as C. `cpp`/`c++` are excluded on purpose: those blocks are
# C++ and are the C++ binding chapter's business, not this gate's.
BLOCK = re.compile(r"^```c\n(?P<body>.*?)^```\s*$", re.M | re.S)

# Marker that exempts the *following* block from compilation.
SKIP_MARKER = "<!-- c-example: skip -->"

# Standard headers the blocks rely on implicitly.
PRELUDE = "#include <stdint.h>\n#include <stdbool.h>\n#include <stddef.h>\n#include <stdio.h>\n"

# A `rw_*` identifier, excluding ones followed by `.` -- `#include "rw_generated.h"`
# names a file, not a function, and treating it as a function would make the
# agreement check fail on correct prose.
RW_NAME = re.compile(r"\brw_[a-z0-9_]+\b(?!\.)")


@dataclass
class Block:
    """One ```c fenced block, with enough context to report it precisely."""

    path: pathlib.Path
    index: int
    line: int
    body: str
    skipped: bool

    def where(self) -> str:
        return f"{self.path}:{self.line} (block #{self.index})"


def collect_blocks(root: pathlib.Path) -> list[Block]:
    """Return every C block under `root`, in a stable order."""
    blocks: list[Block] = []
    for path in sorted(root.rglob("*.md")):
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        for index, match in enumerate(BLOCK.finditer(text)):
            # 1-based line number of the ```c fence.
            line = text.count("\n", 0, match.start()) + 1
            # The marker is honoured when it is the last non-empty line before the
            # fence, which survives re-wrapping of the surrounding prose.
            skipped = False
            for above in reversed(lines[: line - 1]):
                if not above.strip():
                    continue
                skipped = SKIP_MARKER in above
                break
            blocks.append(
                Block(
                    path=path,
                    index=index,
                    line=line,
                    body=match.group("body"),
                    skipped=skipped,
                )
            )
    return blocks


def header_names(header: pathlib.Path) -> set[str]:
    """Every `rw_*` name the published header declares."""
    return set(RW_NAME.findall(header.read_text(encoding="utf-8")))


def compile_block(block: Block, header_dir: pathlib.Path) -> str | None:
    """Syntax-check one block; return the first compiler error, or None."""
    compiler = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        return "no C compiler (cc/gcc/clang) found on PATH"

    with tempfile.TemporaryDirectory() as tmpdir:
        source = pathlib.Path(tmpdir) / "block.c"
        source.write_text(PRELUDE + block.body, encoding="utf-8")
        result = subprocess.run(
            [compiler, "-fsyntax-only", "-I", str(header_dir), str(source)],
            capture_output=True,
            text=True,
        )
    if result.returncode == 0:
        return None
    for line in result.stderr.splitlines():
        if "error:" in line:
            # Drop the temp path so the message points at the cookbook instead.
            return line.split("error:", 1)[1].strip()
    return (result.stderr.strip().splitlines() or ["compilation failed"])[0]


def check_compile(blocks: list[Block], header_dir: pathlib.Path) -> int:
    """Compile every non-skipped block. Returns the number of failures."""
    failures = 0
    skipped = 0
    compiled = 0
    for block in blocks:
        if block.skipped:
            skipped += 1
            print(f"SKIP {block.where()} (marked {SKIP_MARKER})")
            continue
        error = compile_block(block, header_dir)
        if error is None:
            compiled += 1
        else:
            failures += 1
            print(f"FAIL {block.where()}: {error}")
    print(f"     compiled {compiled}, skipped {skipped}, failed {failures}")
    return failures


def check_agreement(blocks: list[Block], header: pathlib.Path) -> int:
    """Every `rw_*` name in every block must be declared in the header."""
    declared = header_names(header)
    if not declared:
        print(f"FAIL {header} declares no rw_* functions; the check would be vacuous")
        return 1

    failures = 0
    for block in blocks:
        # Skipped blocks are still checked for agreement: a skip exempts a block
        # from compiling, not from naming things that exist.
        missing = sorted(
            {name for name in RW_NAME.findall(block.body) if name not in declared}
        )
        if missing:
            failures += len(missing)
            listed = ", ".join(missing)
            print(f"FAIL {block.where()}: not in the header: {listed}")
    if failures == 0:
        print(f"     checked {len(blocks)} blocks against {len(declared)} declared names")
    return failures


def check_header(header: pathlib.Path) -> int:
    """The published header must compile standalone as C."""
    compiler = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if compiler is None:
        print("FAIL no C compiler (cc/gcc/clang) found on PATH")
        return 1

    with tempfile.TemporaryDirectory() as tmpdir:
        source = pathlib.Path(tmpdir) / "header.c"
        source.write_text(f'#include "{header.name}"\nint main(void) {{ return 0; }}\n')
        result = subprocess.run(
            [compiler, "-fsyntax-only", "-I", str(header.parent), str(source)],
            capture_output=True,
            text=True,
        )
    if result.returncode == 0:
        return 0
    print(f"FAIL {header} does not compile standalone:")
    print(result.stderr.strip())
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cookbook", default="cookbook", help="Cookbook root")
    parser.add_argument(
        "--header",
        default="include/rw_generated.h",
        help="Published header the blocks are checked against",
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--compile", action="store_true")
    group.add_argument("--agree", action="store_true")
    group.add_argument("--header-check", action="store_true")
    args = parser.parse_args()

    blocks = collect_blocks(pathlib.Path(args.cookbook))
    if not blocks:
        print("FAIL no C blocks found; the gate would pass vacuously")
        return 1

    if args.compile:
        return 1 if check_compile(blocks, pathlib.Path(args.header).parent) else 0
    if args.agree:
        return 1 if check_agreement(blocks, pathlib.Path(args.header)) else 0
    return check_header(pathlib.Path(args.header))


if __name__ == "__main__":
    sys.exit(main())
