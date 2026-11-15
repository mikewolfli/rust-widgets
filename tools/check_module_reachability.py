#!/usr/bin/env python3
"""Module reachability gate (principle #72).

Every `pub mod` must be in exactly one of three states, and must say so in its
own module documentation under a `# Reachability` heading:

* **Production callers** — list `path:line` references. Each is verified to exist
  and to mention the module within +/-5 lines of the cited line. A citation whose
  line drifted is reported with the nearest real mention, because the pointer is
  what a reader (human or agent) follows.
* **Exposed over the C ABI** — a `rw_*` entry point must exist in
  `src/bindings/binding_impl/`.
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
BINDING_DIR = "src/bindings/binding_impl"

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

# `src/app/handle/types.rs:88` or `app/handle/types.rs:88` inside the State line. (The
# example used the old single-file `handle.rs`; the handles are now a directory.)
CALLER_RE = re.compile(r"([A-Za-z0-9_./-]+\.rs):(\d+)")

# How many lines on either side of the cited line may contain the module token.
# A `path:line` citation is a pointer for a reader; when unrelated edits move the
# anchor, the gate must see the drift instead of verifying only "the file still
# exists and mentions the module somewhere" (W7-03). Five lines is wide enough to
# survive a nearby rustdoc rewrite and narrow enough that a citation pointing at
# the wrong function still fails.
CALLER_WINDOW = 5

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


def _citation_problem(
    root: pathlib.Path, source: pathlib.Path, name: str, relative: str, line_text: str
) -> str | None:
    """Checks one `path:line` citation and returns a problem, or None when it holds.

    The check is a line window, not a whole-file substring: a citation is a pointer
    for a human (and for an agent's next `read_file`), so a stale line number sends
    the reader to unrelated code even though the file still mentions the module.
    """
    referenced = root / relative
    if not referenced.exists():
        return f"{source}: claims caller {relative}:{line_text}, which does not exist"
    try:
        cited = int(line_text)
    except ValueError:  # pragma: no cover - CALLER_RE only matches digits
        return f"{source}: claims caller {relative}:{line_text}, which is not a line number"

    lines = referenced.read_text(encoding="utf-8").splitlines()
    if cited < 1 or cited > len(lines):
        return (
            f"{source}: claims caller {relative}:{line_text}, which is past the end "
            f"(the file has {len(lines)} lines)"
        )

    first = max(0, cited - 1 - CALLER_WINDOW)
    last = min(len(lines), cited + CALLER_WINDOW)
    if name in "\n".join(lines[first:last]):
        return None

    # Distinguish "moved" from "never there": a suggestion is actionable, and
    # "never mentions" points at a citation for a different module entirely.
    nearest: int | None = None
    for index, line in enumerate(lines, start=1):
        if name in line:
            if nearest is None or abs(index - cited) < abs(nearest - cited):
                nearest = index
    if nearest is None:
        return (
            f"{source}: claims caller {relative}:{line_text}, which never mentions "
            f"`{name}`"
        )
    return (
        f"{source}: claims caller {relative}:{line_text}, but +/-{CALLER_WINDOW} lines "
        f"there never mention `{name}`; the nearest mention is {relative}:{nearest} "
        f"(line {abs(nearest - cited)} away)"
    )


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
                problem = _citation_problem(root, source, name, relative, line_text)
                if problem is not None:
                    problems.append(problem)
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
                        f"{BINDING_DIR}/ does not define"
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

    binding_text = "\n".join(
        p.read_text(encoding="utf-8") for p in sorted((root / BINDING_DIR).glob("*.rs"))
    )
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
