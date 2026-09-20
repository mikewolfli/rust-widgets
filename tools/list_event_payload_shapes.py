#!/usr/bin/env python3
"""List the Rust payload type of every signal in the crate, grouped by shape.

# Why this exists

BLUE19 step 1 turns the capability table's `events: &'static [&'static str]` into a
type-carrying `EventSchema`. That work is sized by *how many distinct payload shapes* the
signals actually use, not by how many events there are — a plan that assumes "just add an
enum" collapses the first time it meets `Signal1<(String, u32, String)>`.

This script is the evidence for that sizing. It reads the declarations from source (there
is no runtime registry of payload types) and reports:

  * every distinct payload spelling, with a count and an example signal name;
  * the non-scalar shapes specifically — tuples, `Option<..>`, `Vec<..>` — because those
    are the ones `PropertyValueKind` cannot express today and which therefore need a
    decision (BLUE19 D1/D2/D3) before any implementation starts.

It is a *listing* tool, not a gate: nothing here fails. The gate that will enforce
"declared payload kind matches the signal's real type" belongs to step 1 itself.

Run from the repo root. Usage: python3 tools/list_event_payload_shapes.py
"""

from __future__ import annotations

import collections
import pathlib
import re

SRC = pathlib.Path("src")

# `pub name: SignalN<..>` / `pub name: GenericSignal`. The generic argument is captured
# non-greedily up to the terminating `>` that precedes the field separator, because the
# payload may itself contain `>` (`Signal1<Vec<String>>`).
DECL_RE = re.compile(r"pub\s+(\w+)\s*:\s*(Signal\d+)\s*<([^;]*?)>\s*[,;]", re.S)
GENERIC_RE = re.compile(r"pub\s+(\w+)\s*:\s*GenericSignal\s*[,;]")


def normalise(text: str) -> str:
    return " ".join(text.split())


def main() -> int:
    by_shape: collections.Counter[str] = collections.Counter()
    example: dict[str, str] = {}
    non_scalar: collections.Counter[str] = collections.Counter()
    non_scalar_examples: dict[str, list[str]] = collections.defaultdict(list)

    for path in sorted(SRC.rglob("*.rs")):
        text = path.read_text()
        for name in GENERIC_RE.findall(text):
            by_shape["()  # GenericSignal"] += 1
            example.setdefault("()  # GenericSignal", name)
        for match in DECL_RE.finditer(text):
            name, payload = match.group(1), normalise(match.group(3))
            by_shape[payload] += 1
            example.setdefault(payload, name)
            # The shapes `PropertyValueKind` cannot express.
            if "(" in payload:
                non_scalar["tuple"] += 1
                non_scalar_examples["tuple"].append(name)
            if "Option<" in payload:
                non_scalar["option"] += 1
                non_scalar_examples["option"].append(name)
            if "Vec<" in payload:
                non_scalar["vec"] += 1
                non_scalar_examples["vec"].append(name)

    total = sum(by_shape.values())
    print(f"signal declarations: {total}")
    print(f"distinct payload spellings: {len(by_shape)}")
    print()
    print("=== payload spellings, most common first ===")
    for shape, count in by_shape.most_common():
        print(f"  {shape:46s} x{count:<4} e.g. {example[shape]}")

    print()
    print("=== shapes `PropertyValueKind` cannot express today (BLUE19 D1/D2/D3) ===")
    unsupported = 0
    for kind in ("tuple", "option", "vec"):
        n = non_scalar[kind]
        unsupported += n
        sample = ", ".join(non_scalar_examples[kind][:4])
        print(f"  {kind:8s} {n:3d}   e.g. {sample}")
    print()
    print(
        f"  total needing a decision: {unsupported} "
        f"({unsupported * 100 // max(total, 1)}% of all signal declarations)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
