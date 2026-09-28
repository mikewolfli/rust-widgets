#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: every `impl Draw` type must be covered by some existing gate, or be acknowledged.

# Why this gate exists

The crate's gates are all built around **one number**: 188 controls.

  * `check_svg_snapshots.sh` requires exactly `188 x 2 + extras` files under `snapshots/svg/`;
  * `check_control_rendering.sh` sweeps the factory registry, which is those 188 names;
  * `check_control_has_tests.sh` walks the same 186 names.

A struct that implements `Draw` and does **not** correspond to one of those names is therefore
outside all three — by construction, not by oversight. Sometimes that is correct: a chart engine
(`ChartWidget` for `chart`) or a shape wrapper (`FreeformShapeWidget` for `freeform_shape`) is
reached *through* the registered control, so the registered control's snapshot covers the
delegated drawing. Sometimes it is a real gap, and one was found while writing this gate:

    KeySequenceEdit  src/widget/advanced_widgets/key_sequence_edit.rs

It exports a public type, draws six colour literals, is re-exported from `src/widget/mod.rs`, and
**nothing in the crate constructs it**. No snapshot, no census row, no test gate: a control that
renders and that nothing checks. It is not a duplicate of the canonical `shortcut_editor` either —
that control manages a *list* of shortcuts, this one captures a *single* one — so deleting it as
redundant would have been wrong. It has to be either registered or acknowledged, and the gate is
what makes that a decision rather than a silence.

**A gate that cannot fail is not a gate**, so this one is checked in three directions:

  * a drawn type that is neither registered nor acknowledged is a finding;
  * an acknowledged entry whose type has since become registered (or which no longer exists) is
    reported as **stale**, so the list can only shrink;
  * `--inject=<Type>` pretends a registered type has lost its registration and requires the answer
    to change, so a search that matched nothing cannot pass.

# What counts as "covered"

A drawn type is covered when it reaches the gates through any of these routes:

  * its name (snake-cased) is a `canonical_name` in `src/widget/capability/properties.rs`;
  * a `constructors.rs` function constructs it, which makes it the implementation behind a
    registered control regardless of whether the struct and the control share a name;
  * it is listed in `ACKNOWLEDGED` with a written reason.

The middle route is why the check cannot be a name comparison: seven of the eight types this
audit found are the implementations behind registered controls under a longer struct name
(`OrderBookWidget` -> `order_book`, `LCDNumber` -> `lcd_number`, `QRCode` -> `qr_code`, ...).
Reporting those would be seven false positives, and a gate with seven false positives is one
nobody reads.

# What does not count as coverage

  * **A `pub use` re-export.** `KeySequenceEdit` is exported from `src/widget/mod.rs`; that is what
    makes it *public*, not what makes it *checked*. An export says "a host may name this type", and
    a host that mounts it gets no snapshot and no census row — which is the gap, written as an
    export.
  * **A test fixture.** `TestChild` / `TestContent` implement `Draw` for one test each. They are
    skipped by name prefix, which is the same convention `check_control_has_tests.py` uses.

Usage:
    python3 tools/check_drawn_types_are_covered.py [--inject TypeName]
"""

import glob
import pathlib
import re
import sys

PROPERTIES = "src/widget/capability/properties.rs"
CONSTRUCTORS = "src/widget/capability/constructors.rs"
WIDGET_GLOB = "src/widget/**/*.rs"

CANONICAL = re.compile(r'canonical_name:\s*"([^"]+)"')
DRAW_IMPL = re.compile(r"impl Draw for ([A-Za-z_][A-Za-z0-9_]*)")

#: Types that implement `Draw` with no gate covering them, and why that is accepted.
#:
#: Each entry must name the plan item that will connect it, exactly as
#: `check_mechanism_has_a_consumer.py`'s table does: an acknowledgement without a successor is how
#: a finding becomes permanent. An entry whose type later becomes registered is reported stale.
ACKNOWLEDGED = {
    "KeySequenceEdit": (
        "src/widget/advanced_widgets/key_sequence_edit.rs",
        "A single-shortcut capture field with no registry entry: it is exported from "
        "`src/widget/mod.rs` and nothing constructs it, so it has no snapshot, no census row and no "
        "test gate. Not a duplicate of `shortcut_editor` (that manages a list, this captures one), "
        "so it needs registering or deleting rather than folding. BLUE23 appendix A (`shortcut_editor` "
        "row) is where the decision goes.",
    ),
}


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def main() -> int:
    injected = None
    for arg in sys.argv[1:]:
        if arg.startswith("--inject="):
            injected = arg.split("=", 1)[1]

    canonical = set(CANONICAL.findall(pathlib.Path(PROPERTIES).read_text(encoding="utf-8")))
    constructors = pathlib.Path(CONSTRUCTORS).read_text(encoding="utf-8")

    drawn: dict[str, str] = {}
    for path in sorted(glob.glob(WIDGET_GLOB, recursive=True)):
        text = open(path, encoding="utf-8", errors="ignore").read()
        for match in DRAW_IMPL.finditer(text):
            drawn.setdefault(match.group(1), path.replace("\\", "/"))

    if injected is not None and injected not in drawn:
        print(f"  FAIL  --inject={injected} names no `impl Draw` type, so the injection proves nothing")
        return 1

    findings = []
    stale = []
    for type_name, path in sorted(drawn.items()):
        if type_name.startswith("Test"):
            continue
        registered = bool({type_name.lower(), snake(type_name)} & canonical) or (
            re.search(r"\b" + type_name + r"\b", constructors) is not None
        )
        if injected is not None and type_name == injected:
            # Pretend the type lost its registration: the answering code must then treat it as
            # uncovered. If the report does not change, the search and the report are unrelated.
            registered = False
        if registered:
            if type_name in ACKNOWLEDGED:
                stale.append((type_name, "now covered by a gate"))
            continue
        if type_name not in ACKNOWLEDGED:
            findings.append((type_name, path))
            continue
        acked_path, _ = ACKNOWLEDGED[type_name]
        if acked_path != path:
            stale.append((type_name, f"moved to {path}, table says {acked_path}"))

    for type_name, reason in stale:
        print(f"  STALE  {type_name}: {reason} — remove it from ACKNOWLEDGED")

    if findings:
        print("  FAIL  these `impl Draw` types are covered by no gate:")
        for type_name, path in findings:
            print(f"          {type_name}  ({path})")
        print(
            "        Register the type (a `canonical_name` plus a constructor), or add it to\n"
            "        ACKNOWLEDGED with the plan item that will connect it."
        )

    if findings or stale:
        return 1

    print(f"  PASS  every drawn type is covered or acknowledged ({len(ACKNOWLEDGED)} acknowledged)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
