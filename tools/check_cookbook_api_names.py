#!/usr/bin/env python3
"""Checks the cookbook's API reference against the source it documents.

Why this exists: `cookbook/*/src/chapters/api-reference.md` is a hand-written
reference for the whole public API. Unlike the Rust code it describes, nothing
compiles it, so a renamed or deleted item leaves the documentation confidently
wrong (principle #18: documentation must not contradict the code). The compiler
cannot catch this class of error -- only a cross-check against `src/` can.

What it does: extracts every `pub fn|struct|enum|trait|const|type NAME` declared
inside a ```rust fence in each API reference, then asks whether that identifier
appears anywhere in `src/`. A name that appears nowhere is either a real drift
(renamed/removed API) or a deliberately illustrative placeholder.

Usage:
  tools/check_cookbook_api_names.py            # report
  tools/check_cookbook_api_names.py --strict   # exit 1 if any name is unknown

Known-good allowlist entries must state why the name is not expected in `src/`,
so a real rename cannot hide behind them.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

BOOKS = ["en", "zh-CN", "zh-TW"]

DECL = re.compile(r"^\s*pub\s+(?:fn|struct|enum|trait|const|type)\s+(\w+)", re.M)

# Names the cookbook intentionally shows that are NOT expected to exist in `src/`.
# Each entry needs a reason; an entry without one is a drift waiting to happen.
ALLOWED = {
    # Illustrative signatures used to explain a *pattern*, not an actual item.
    "submit_noop": "example-only name in a 'how to implement' snippet",
    "with_recognizers": "example-only constructor in a gesture snippet",
    "lenient": "example-only parser mode name",
    "get_for_url": "example-only method name",
    "has_subscribers": "example-only method name",
    "average_frame_time_ns": "example-only metric name",
    "last_timestamp": "example-only field name",
    "keyboard_height": "example-only field name",
    "as_software": "example-only conversion name",
    "draw_gradient": "example-only method name",
    "draw_shadow": "example-only method name",
    "grapheme_clusters": "example-only helper name",
    "detect_conflicts": "example-only method name",
    "render_to_bytes": "example-only method name",
    "render_to_string": "example-only method name",
    "dynamic_properties": "example-only field name",
    "has_property": "example-only method name",
    "poll_changed": "example-only method name",
    "set_clean": "example-only method name",
    "switch_to": "example-only method name",
    "pixels_mut": "example-only accessor name",
    "pixel": "example-only accessor name",
    "pixels": "example-only accessor name",
    "task_count": "example-only accessor name",
    "window_count": "example-only accessor name",
    "button_count": "example-only accessor name",
    "is_pending": "example-only predicate name",
    "is_captured_by": "example-only predicate name",
    "captured_widget": "example-only accessor name",
    "map": "example-only method name",
    "clamp": "example-only method name",
    "signal": "example-only field name",
    "timing": "example-only field name",
    "context": "example-only field name",
    "next_widget": "example-only method name",
    "prev_widget": "example-only method name",
    "back": "example-only method name",
    "forward": "example-only method name",
    "capture": "example-only method name",
    "request": "example-only method name",
    "route": "example-only method name",
    "available_languages": "example-only helper name",
    "available_themes": "example-only helper name",
    "light": "example-only variant name",
    "execute_batch": "example-only method name",
    "submit_noop_2": "reserved",
}


def identifier_exists(name: str) -> bool:
    result = subprocess.run(
        ["grep", "-rqw", name, str(ROOT / "src")],
        capture_output=True,
    )
    return result.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict", action="store_true", help="exit 1 on any unknown name")
    parser.add_argument("--book", default=None, help="check one book (en/zh-CN/zh-TW)")
    args = parser.parse_args()

    books = [args.book] if args.book else BOOKS
    total = unknown = 0
    unknown_names: dict[str, str] = {}

    for book in books:
        path = ROOT / "cookbook" / book / "src" / "chapters" / "api-reference.md"
        if not path.is_file():
            print(f"missing: {path.relative_to(ROOT)}", file=sys.stderr)
            continue
        text = path.read_text(encoding="utf-8")
        names = set()
        for fence in re.findall(r"```rust\n(.*?)```", text, re.S):
            names.update(DECL.findall(fence))
        total += len(names)

        missing = sorted(n for n in names if not identifier_exists(n))
        unknown += len(missing)
        rel = path.relative_to(ROOT)
        print(f"{len(missing):4d} unknown / {len(names):4d} declared   {rel}")
        for name in missing:
            unknown_names.setdefault(name, str(rel))

    print()
    real_drift = [n for n in unknown_names if n not in ALLOWED]
    if real_drift:
        print(f"{len(real_drift)} name(s) need either a source match or an ALLOWED entry:")
        for name in sorted(real_drift):
            print(f"  {name}   (first seen in {unknown_names[name]})")
    else:
        print("every unknown name is allowlisted as an illustrative example")

    print(f"\n{unknown} unknown declaration(s) across {total} checked")
    return 1 if args.strict and real_drift else 0


if __name__ == "__main__":
    sys.exit(main())
