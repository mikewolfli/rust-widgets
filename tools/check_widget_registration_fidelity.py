#!/usr/bin/env python3
"""Registration fidelity gate: every `WidgetKind` must be reachable, aliased or
explicitly declared a base/child kind.

The defect class this blocks
----------------------------

`TimelineWidget`, `CommandPalette`, `NotificationCenter`, `DiffViewer`,
`MarkdownEditor` and `ToastStack` were fully implemented and publicly exported,
but no `self.register(...)` row existed for them. A Rust caller could write
`TimelineWidget::new(rect)`, yet `factory.create("timeline_widget", ..)`
returned `None` — so the control could not appear in a declarative JSON tree and
could not be targeted by a CSS selector. Nothing failed; the controls simply
were not there for anyone using the library's name-based surface.

Why the criterion is the enum, not a name suffix
-----------------------------------------------

An earlier attempt matched `pub struct <Name>` against suffixes
(`Widget`/`View`/`Editor`/...). That heuristic reports every data type that
happens to end in a control-ish word (`ToastItem`, `ChartSeries`,
`WebEngineSettings`) and would need ever-growing exclusions. The `WidgetKind`
enum is the library's own, authoritative statement of "this is a control", so it
is the only list that cannot drift.

The three states (principle #72, made machine-checkable)
--------------------------------------------------------

Every kind must hit exactly one of:

* **Registered**   — a factory `canonical_name` or `alias` carries it. Proved by
  the compiled registry, not by grepping source, so a name spelled in a comment
  cannot satisfy it. A companion Rust test cross-checks this from inside.
* **AliasOf**      — a `pub type X = Y;` in `src/widget/mod.rs` declares it, and
  `Y` is itself Registered. A type alias is the same control under a second name
  (`Panel = GroupBox`), so registering it would create a duplicate entry.
* **BaseOrChild**  — annotated with a `// kind-role: base` or `// kind-role: child`
  marker next to its variant in `src/widget/kind.rs`. These are structural roles
  (`Frame` supplies its children's drawing, `MenuItem` is built by its `Menu`),
  never standalone controls. The marker exists because "this one is a base class"
  otherwise lives only in a developer's head, which is indistinguishable from
  "somebody forgot to register it" — precisely how this defect arose.

Usage
-----

    tools/check_widget_registration_fidelity.sh            # gate
    tools/check_widget_registration_fidelity.sh --report   # print the full table
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

KIND_FILE = "src/widget/kind.rs"
ALIAS_FILE = "src/widget/mod.rs"

# Matches a `// kind-role: base` / `// kind-role: child` marker. The marker is
# allowed to carry trailing prose, and to sit on the line above or the same line
# as the variant, because either reads naturally in Rust.
ROLE_RE = re.compile(r"//\s*kind-role:\s*(base|child)\b")

# `pub type Panel = GroupBox;` — the alias half of the reachability model.
ALIAS_RE = re.compile(r"^pub type (?P<alias>[A-Za-z0-9_]+)\s*=\s*(?P<target>[A-Za-z0-9_]+)\s*;")


def pascal_to_snake(name: str) -> str:
    """`CommandPalette` -> `command_palette`, `LCDNumber` -> `lcd_number`.

    The registry stores canonical names in snake_case, so comparing a kind name
    requires the same spelling. Acronym runs are kept together (`LCDNumber` is
    `lcd_number`, not `l_c_d_number`), matching the spellings the registry uses.
    """
    first = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", first).lower()


def load_reachability() -> dict:
    """Ask the compiled library which names the factory actually resolves.

    Shelling out to `cargo test` with a filter would be slow and would tie this
    gate to a build; instead the registry is exported through a tiny example that
    the gate compiles once. That keeps the source of truth the same code path the
    library uses at runtime.
    """
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--no-default-features",
            "--features",
            "desktop",
            "--example",
            "widget_reachability",
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print("Failed to run the widget_reachability example:", file=sys.stderr)
        print(result.stdout, file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        raise SystemExit(2)

    # The example prints one JSON object; anything before it is build noise.
    for line in reversed(result.stdout.splitlines()):
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)
    print("widget_reachability produced no JSON report", file=sys.stderr)
    raise SystemExit(2)


def parse_kind_roles() -> dict[str, str]:
    """Returns kind name -> role for every `// kind-role:` annotation."""
    text = pathlib.Path(KIND_FILE).read_text(encoding="utf-8")
    roles: dict[str, str] = {}
    pending: str | None = None
    for line in text.splitlines():
        marker = ROLE_RE.search(line)
        if marker:
            # Same-line marker: the variant name is on this line too.
            variant = re.search(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,", line)
            if variant:
                roles[variant.group(1)] = marker.group(1)
                pending = None
                continue
            pending = marker.group(1)
            continue
        variant = re.search(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,", line)
        if variant and pending:
            roles[variant.group(1)] = pending
            pending = None
    return roles


def parse_aliases() -> dict[str, str]:
    """Returns alias kind name -> target kind name from `src/widget/mod.rs`."""
    aliases: dict[str, str] = {}
    for path in (ALIAS_FILE, KIND_FILE):
        candidate = pathlib.Path(path)
        if not candidate.exists():
            continue
        for line in candidate.read_text(encoding="utf-8").splitlines():
            match = ALIAS_RE.match(line.strip())
            if match:
                aliases[match.group("alias")] = match.group("target")
    return aliases


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--report", action="store_true", help="Print every kind with its state."
    )
    args = parser.parse_args()

    reachability = load_reachability()
    kinds: list[str] = reachability["kinds"]
    registered: set[str] = set(reachability["registered"])
    aliases = parse_aliases()
    roles = parse_kind_roles()

    unresolved: list[str] = []
    rows: list[tuple[str, str]] = []

    for kind in kinds:
        snake = pascal_to_snake(kind)
        if snake in registered:
            rows.append((kind, "Registered"))
            continue
        # An alias declared as `pub type X = Y;` is the same control under a second
        # name; registering it would create a duplicate entry (`Panel = GroupBox`).
        target = aliases.get(kind)
        if target is not None:
            if pascal_to_snake(target) in registered:
                rows.append((kind, f"AliasOf({target})"))
                continue
            # An alias whose target is not registered is *not* reachable: the
            # chain terminates nowhere, which is a real defect rather than a
            # passing alias.
            unresolved.append(f"{kind} (alias of {target}, which is not registered)")
            continue
        role = roles.get(kind)
        if role is not None:
            rows.append((kind, f"BaseOrChild({role})"))
            continue
        unresolved.append(f"{kind} (no registration, alias or kind-role marker)")

    if args.report:
        width = max((len(kind) for kind, _ in rows), default=0)
        for kind, state in sorted(rows):
            print(f"{kind:<{width}}  {state}")
        print(f"\n{len(rows)} classified, {len(unresolved)} unresolved")

    # Reverse assertion (principle #77): the classification must be total. A
    # one-way check that "everything registered is a kind" would pass while kinds
    # silently fell off the list, which is the failure being guarded.
    if unresolved:
        print("Unreachable WidgetKind variants:", file=sys.stderr)
        for entry in unresolved:
            print(f"  ❌ {entry}", file=sys.stderr)
        print(
            "\nEvery kind must be registered in the factory, declared a `pub type` alias "
            "of a registered kind, or carry a `// kind-role: base|child` marker in "
            f"{KIND_FILE}.",
            file=sys.stderr,
        )
        return 1

    print(
        f"✅ widget registration fidelity: {len(kinds)} kinds classified "
        f"({len(registered)} registered names, {len(aliases)} aliases, {len(roles)} role markers)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
