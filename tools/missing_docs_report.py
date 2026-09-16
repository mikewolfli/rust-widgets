#!/usr/bin/env python3
"""Lists `missing_docs` locations per file for a given cargo feature set.

Why this exists: `--message-format=short` drops the file path when rustc renders a
lint that has no primary span, and `missing_docs` on a module or a macro is often
reported that way. That makes the plain-text output useless for splitting the
remaining work by file. The JSON diagnostic stream always carries the span, so this
reads that instead.

Usage:
  tools/missing_docs_report.py <feature> [<feature> ...]
  tools/missing_docs_report.py desktop
  tools/missing_docs_report.py --all-features
"""

from __future__ import annotations

import argparse
import collections
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("features", nargs="*", help="cargo features, or --all-features")
    parser.add_argument("--all-features", action="store_true", help="check every feature")
    args = parser.parse_args()

    if not args.all_features and not args.features:
        parser.error("give at least one feature, or --all-features")

    cmd = ["cargo", "check", "--message-format=json"]
    if args.all_features:
        cmd.append("--all-features")
    else:
        cmd += ["--no-default-features", "--features", ",".join(args.features)]

    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)

    by_file: collections.Counter[str] = collections.Counter()
    by_kind: collections.Counter[str] = collections.Counter()
    no_span = 0

    for line in proc.stdout.splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get("reason") != "compiler-message":
            continue
        message = record["message"]
        text = message.get("message", "")
        if message.get("level") != "warning" or "missing documentation" not in text:
            continue

        kind = text.replace("missing documentation for ", "").replace("missing documentation", "item")
        by_kind[kind] += 1

        located = False
        for span in message.get("spans") or []:
            name = span.get("file_name")
            if name and not name.startswith("target/"):
                by_file[name] += 1
                located = True
                break
        if not located:
            no_span += 1

    total = sum(by_kind.values())
    label = "all-features" if args.all_features else " ".join(args.features)
    print(f"features: {label}")
    print(f"total missing docs: {total}")
    if no_span:
        print(f"  ({no_span} without a usable span, usually inside an include!-ed file)")
    print()
    for kind, count in by_kind.most_common():
        print(f"  {count:5d}  {kind}")
    print()
    if by_file:
        print(f"files: {len(by_file)}")
        for name, count in by_file.most_common():
            print(f"  {count:5d}  {name}")
    return 0 if total == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
