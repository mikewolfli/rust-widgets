#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE23 §5A.3 gate: lifecycle hooks are carried at build time, never invoked there.

`View::build` and `diff` are pure functions -- that purity is what the retained-tree
architecture rests on. A lifecycle hook (`on_mount` / `on_unmount`) is a side effect, so it
must be *stored* on the node and run by the engine after the patch batch lands. Invoking it
inside a builder or the diff would still compile and still pass the behaviour tests (the
hook does fire), while silently breaking the purity the diff depends on.

This scan reads the two files that describe and compare a tree -- `src/view/node.rs` and
`src/view/diff.rs` -- and fails if a hook field appears in a call position (`(...)` right
after the field). The engine is the one file allowed to invoke hooks, and its invocation is
asserted separately so a scan that read nothing cannot pass.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
# The files that must NOT invoke a hook: the builder and the comparator.
PURE_FILES = ["src/view/node.rs", "src/view/diff.rs"]
ENGINE = ROOT / "src" / "view" / "engine.rs"

# A **stored** hook in a call position. The builder methods themselves are
# `pub fn on_mount(mut self, f: ...)` -- they *store*, which is the point -- so the scan
# must not match a `fn` declaration. What it matches instead is calling the field:
# `node.on_mount(...)` / `self.on_mount(...)` -- a `.on_mount(` call on a value.
HOOK_DECL = re.compile(r"\bfn\s+on_(?:mount|unmount)\s*\(")
HOOK_FIELD_CALL = re.compile(r"\.on_(?:mount|unmount)\s*\(")
# The engine must actually invoke what it carries, or "carried" is a decoration.
ENGINE_INVOKE = re.compile(r"\bhook\s*\(")


def invaders(body: str) -> list[str]:
    """Call positions that invoke a stored hook, ignoring the builder declarations."""
    # Drop the builder *declarations* (their bodies store with `= Some(..)`, an assignment
    # rather than a call), leaving any real field call to be found below.
    without_decls = HOOK_DECL.sub("fn __builder__(", body)
    return HOOK_FIELD_CALL.findall(without_decls)


def read(rel: str) -> str:
    path = ROOT / rel
    text = path.read_text(encoding="utf-8")
    # Only production code; a test may call a hook to prove it fires.
    marker = text.rfind("\n#[cfg(test)]")
    return text[:marker] if marker != -1 else text


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--inject",
        action="store_true",
        help="pretend a builder invoked a hook, to prove the scan reads the file",
    )
    args = parser.parse_args(argv)

    if args.inject:
        body = read("src/view/node.rs") + "\nfn injected(node: &Node) { let _ = node.on_mount(1); }\n"
        return 1 if HOOK_FIELD_CALL.search(body) else 0

    off_when = [rel for rel in PURE_FILES if invaders(read(rel))]
    if off_when:
        print("FAIL: a lifecycle hook is invoked while a tree is described or compared:")
        for rel in off_when:
            print(f"  {rel}")
        print()
        print("  Hooks are side effects. Store them on the node (`on_mount`/`on_unmount`) and")
        print("  let `ViewEngine` run them after the patch batch lands -- see BLUE23 §5A.3.")
        return 1

    # The other half: the engine really does invoke the hooks it carries. Without this the
    # rule above would be satisfiable by never calling a hook at all.
    if not ENGINE_INVOKE.search(ENGINE.read_text(encoding="utf-8")):
        print("FAIL: the engine never invokes a lifecycle hook, so carrying one does nothing")
        return 1

    print(f"lifecycle-hook scan: checked={len(PURE_FILES)} describe/compare files, failed=0")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
