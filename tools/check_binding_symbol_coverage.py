#!/usr/bin/env python3
"""Verify that every language binding reaches the whole published C ABI.

The published header (``include/rw_generated.h``) is the contract. A binding that
silently omits a function leaves a capability unreachable for that language --
which is exactly the class of defect rule #71 ("capability reachability before
capability addition") exists to prevent.

Two bindings classes are distinguished, because they reach the C ABI differently:

* **Direct FFI bindings** (python / nodejs / cpp) declare or configure the
  ``rw_*`` symbols by name. Their declared symbol set must be a superset of the
  published header.
* **JNI bindings** (java / android) never name ``rw_*`` functions: Java calls
  ``native*`` methods that Rust maps onto ``rw_*``. Their coverage is verified by
  ``tools/check_jni_signatures.sh`` instead, so this script only checks that the
  JNI sources exist and then skips symbol comparison for them.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

SYMBOL_RE = re.compile(r"\brw_[a-z0-9_]+\b")

# Binding sources that name rw_* symbols directly, with the path used in the
# failure message. Kept explicit (not globbed) so adding a binding is a
# deliberate act: a new binding that nobody checks would defeat the gate.
DIRECT_FFI_BINDINGS: dict[str, str] = {
    "python": "bindings/python/rust_widgets/__init__.py",
    "nodejs": "bindings/nodejs/index.js",
    "cpp": "bindings/cpp/rust_widgets.hpp",
}

# JNI bindings reach the ABI through native methods; covered by the JNI
# signature gate. Listed here so their absence is a hard failure too.
JNI_BINDINGS: dict[str, str] = {
    "java": "bindings/java/RustWidgetsJNI.java",
    "android": "bindings/android/java/rust_widgets/RustWidgets.java",
}


def published_symbols(header: pathlib.Path) -> set[str]:
    """Return every rw_* function name declared in the published C header."""
    text = header.read_text(encoding="utf-8")
    symbols: set[str] = set()
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(("#", "/", "*")):
            continue
        if not stripped.endswith(";"):
            continue
        symbols.update(SYMBOL_RE.findall(stripped))
    return symbols


def declared_symbols(source: pathlib.Path, root: pathlib.Path) -> set[str]:
    """Returns every rw_* symbol a direct FFI binding reaches.

    A binding either names the symbol or gets it from a header it includes — the
    C++ wrapper deliberately declares nothing the generated header already
    declares, because a second copy is how it previously disagreed with the ABI.
    Following `#include` is therefore the correct reading: a symbol reachable
    through an included declaration IS available to that binding's users.
    """
    text = source.read_text(encoding="utf-8")
    symbols = set(SYMBOL_RE.findall(text))
    for include in re.findall(r'^\s*#\s*include\s*<([^>]+)>', text, re.MULTILINE):
        for candidate in (root / "include" / include, root / include):
            if candidate.exists() and candidate != source:
                symbols.update(SYMBOL_RE.findall(candidate.read_text(encoding="utf-8")))
                break
    return symbols


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--header",
        default="include/rw_generated.h",
        help="Published C header (the contract).",
    )
    args = parser.parse_args()

    root = pathlib.Path.cwd()
    header = root / args.header
    if not header.exists():
        print(f"Missing published header: {header}", file=sys.stderr)
        return 1

    expected = published_symbols(header)
    if not expected:
        print(f"No rw_* declarations parsed from {header}", file=sys.stderr)
        return 1

    failures: list[str] = []

    for name, relative in DIRECT_FFI_BINDINGS.items():
        path = root / relative
        if not path.exists():
            failures.append(f"{name}: missing binding source {relative}")
            continue
        actual = declared_symbols(path, root)
        missing = sorted(expected - actual)
        if missing:
            failures.append(
                f"{name} ({relative}) is missing {len(missing)}/{len(expected)} "
                f"published symbols: {', '.join(missing)}"
            )
        else:
            print(f"OK  {name:<8} {relative}: covers all {len(expected)} symbols")

    for name, relative in JNI_BINDINGS.items():
        path = root / relative
        if not path.exists():
            failures.append(f"{name}: missing JNI binding source {relative}")
        else:
            print(f"OK  {name:<8} {relative}: JNI binding (checked by check_jni_signatures.sh)")

    if failures:
        print("", file=sys.stderr)
        for failure in failures:
            print(f"FAIL {failure}", file=sys.stderr)
        return 1

    print(f"\nBinding symbol coverage OK ({len(expected)} published rw_* functions).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
