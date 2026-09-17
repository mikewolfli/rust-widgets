#!/usr/bin/env python3
"""Module reachability gate (principle #72).

Every `pub mod` must be in exactly one of three states, and must say so in its
own module documentation under a `# Reachability` heading:

* **Production callers** — list `path:line` references. Each is verified to exist
  and to mention the module.
* **Exposed over the C ABI** — a `rw_*` entry point must exist in
  `src/bindings/binding_impl.rs`.
* **Reserved** — an experiment or deliberately kept module. Requires a non-empty
  reason; no caller evidence is demanded.

# Why a gate and not a review habit

Roughly 30,000 lines in this crate had no production consumer and nothing said
so. The absence of a caller is invisible in a diff, so a module can stay
unreachable for many rounds while every other gate reports green. Requiring an
explicit answer makes "nobody uses this" a fact someone had to write down, and
therefore a fact that can be acted on.

# Why the statement lives in the module, not in a list here

A list in one file drifts the moment a module is added, and the reviewer reading
a module sees nothing. Keeping the statement next to the code means the answer
travels with the thing it describes.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

LIB_FILE = "src/lib.rs"
BINDING_FILE = "src/bindings/binding_impl.rs"

# `pub mod foo;` / `pub mod foo {` — the declaration form, not a use statement.
MOD_RE = re.compile(r"^pub mod (?P<name>[a-z_0-9]+)", re.MULTILINE)

REACHABILITY_HEADING_RE = re.compile(
    r"^(?:///|//!)\s*#\s*Reachability\s*$", re.MULTILINE
)

# `//! **State:** ...` in a module file, or `/// **State:** ...` on an inline
# `pub mod` declared in `lib.rs`. Both are module documentation; which prefix is
# legal depends only on whether the module has its own file.
STATE_RE = re.compile(r"^\s*(?:///|//!)\s*\*\*State:\*\*\s*(?P<body>.+)$", re.MULTILINE)

PRODUCTION_RE = re.compile(r"Production callers?\b", re.IGNORECASE)
ABI_RE = re.compile(r"Exposed over the C ABI", re.IGNORECASE)
RESERVED_RE = re.compile(r"\bReserved\b", re.IGNORECASE)

# `src/app/handle.rs:88` or `app/handle.rs:88` inside the State line.
CALLER_RE = re.compile(r"([A-Za-z0-9_./-]+\.rs):(\d+)")

# `rw_foo` named in the State line of an ABI-exposed module.
SYMBOL_RE = re.compile(r"\b(rw_[a-z0-9_]+)\b")


def module_source(root: pathlib.Path, name: str) -> pathlib.Path | None:
    """Returns the file whose module docs describe `name`, or None.

    An inline `pub mod x { ... }` has no separate file: its documentation lives
    in `src/lib.rs`, so that is what the checker reads. Treating it as missing
    would make the gate fail on a module that is perfectly well documented.
    """
    for candidate in (root / "src" / f"{name}.rs", root / "src" / name / "mod.rs"):
        if candidate.exists():
            return candidate
    lib_text = (root / LIB_FILE).read_text(encoding="utf-8")
    if re.search(rf"^pub mod {re.escape(name)} \{{", lib_text, re.MULTILINE):
        return root / LIB_FILE
    return None


def state_bodies(docs: str) -> list[str]:
    """Returns the `**State:**` bodies declared for one module.

    A module may have more than one declaration when different configurations
    reach it differently; all of them are checked.
    """
    return [match.group("body").strip() for match in STATE_RE.finditer(docs)]


def check_module(root: pathlib.Path, name: str, abi_symbols: set[str]) -> list[str]:
    """Returns a list of human-readable violations for one module."""
    problems: list[str] = []
    source = module_source(root, name)
    if source is None:
        return [f"{name}: declared in lib.rs but no source file was found"]
    docs = source.read_text(encoding="utf-8")

    if not REACHABILITY_HEADING_RE.search(docs):
        return [
            f"{source}: declares no `# Reachability` section, so its consumer state "
            "is unstated"
        ]

    bodies = state_bodies(docs)
    if not bodies:
        return [f"{source}: has a `# Reachability` heading but no `**State:**` line"]

    for body in bodies:
        if PRODUCTION_RE.search(body):
            callers = CALLER_RE.findall(body)
            if not callers:
                problems.append(
                    f"{source}: claims production callers but names no `path:line`"
                )
                continue
            for relative, line_text in callers:
                referenced = root / relative
                if not referenced.exists():
                    problems.append(
                        f"{source}: claims caller {relative}:{line_text}, which does not exist"
                    )
                    continue
                # The reference must really mention the module: a stale line number
                # after unrelated edits is otherwise invisible.
                if name not in referenced.read_text(encoding="utf-8"):
                    problems.append(
                        f"{source}: claims caller {relative}:{line_text}, which never "
                        f"mentions `{name}`"
                    )
        elif ABI_RE.search(body):
            symbols = SYMBOL_RE.findall(body)
            if not symbols:
                problems.append(
                    f"{source}: claims C ABI exposure but names no `rw_*` symbol"
                )
                continue
            for symbol in symbols:
                if symbol not in abi_symbols:
                    problems.append(
                        f"{source}: claims C ABI exposure via `{symbol}`, which "
                        f"{BINDING_FILE} does not define"
                    )
        elif RESERVED_RE.search(body):
            # A reason must follow the word, not just the label.
            residual = RESERVED_RE.split(body, maxsplit=1)[-1]
            residual = residual.strip(" .:-")
            if len(residual) < 15:
                problems.append(
                    f"{source}: reserves the module but gives no usable reason "
                    f"(got {residual!r})"
                )
        else:
            problems.append(
                f"{source}: `**State:**` names none of the three states "
                "(Production callers / Exposed over the C ABI / Reserved)"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--report", action="store_true", help="List every module and its state."
    )
    args = parser.parse_args()

    root = pathlib.Path.cwd()
    lib_text = (root / LIB_FILE).read_text(encoding="utf-8")
    modules = sorted(set(MOD_RE.findall(lib_text)))

    binding_text = (root / BINDING_FILE).read_text(encoding="utf-8")
    abi_symbols = set(re.findall(r"fn (rw_[a-z0-9_]+)", binding_text))

    problems: list[str] = []
    for name in modules:
        found = check_module(root, name, abi_symbols)
        if args.report:
            status = "OK" if not found else "PROBLEM"
            print(f"{name:<16} {status}")
        problems.extend(found)

    if problems:
        print("Module reachability violations (principle #72):", file=sys.stderr)
        for problem in problems:
            print(f"  ❌ {problem}", file=sys.stderr)
        print(
            "\nAdd a `# Reachability` section with one `**State:**` line: "
            "`Production callers: path:line[, ...]`, "
            "`Exposed over the C ABI (rw_name)`, or "
            "`Reserved: <reason>`.",
            file=sys.stderr,
        )
        return 1

    print(f"✅ module reachability: {len(modules)} `pub mod` declarations all state their state")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
