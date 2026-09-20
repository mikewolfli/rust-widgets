#!/usr/bin/env python3
"""A control that wires its events dynamically must resolve every name its capability publishes.

# Why this gate exists

`Widget::event_signal_dyn` is how a name written in a designer's project becomes a live
subscription. `EventSignalBinder::forward_all` walks a capability's published events and asks the
control to resolve each one, so the two must agree exactly:

  * a name the capability publishes that `event_signal_dyn` does **not** resolve is an event the
    designer offers and the wiring silently drops — `forward_all` counts it as unwired, which is
    honest but still means a panel entry that does nothing;
  * a name `event_signal_dyn` resolves that the capability does **not** publish is worse: nothing
    can ever subscribe to it through `connect_event`, so the arm is dead code that looks like
    support.

# Scope: converted controls only

Resolution is opt-in. A control that has not been converted falls back to the trait default, which
returns `None` for every name — correct and honest, but indistinguishable from a converted control
with a missing arm. So the check is scoped to the controls this tool is told are **converted**: the
list below is the declaration of which controls wire themselves dynamically, and for each one every
published name must resolve. Adding a control to the list is a commitment, not a formality, and
removing one is a visible regression rather than a silent one.

# Reverse injection

`--inject=<control>.<event>` pretends one arm is missing and requires a failure, so a check that
quietly found nothing cannot pass.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"

# Controls whose `event_signal_dyn` is implemented, with the file that implements it. The file is
# stated rather than searched for so that moving an arm to a different control cannot pass by
# accident — a copied match block in the wrong file would resolve names for a control that does not
# publish them.
CONVERTED: dict[str, str] = {
    "button": "src/widget/base_widgets/button.rs",
    "check_box": "src/widget/base_widgets/checkbox.rs",
    "slider": "src/widget/display_widgets/slider.rs",
}

CENSUS = REPO / "tools/event_published_census.txt"


def read_census() -> dict[str, list[str]]:
    """control -> published event names, from the same file the payload table is derived from."""
    published: dict[str, list[str]] = {}
    for line in CENSUS.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        control, _, rest = line.partition(":")
        published[control.strip()] = [name.strip() for name in rest.split(",") if name.strip()]
    return published


def resolved_names(source: str) -> set[str]:
    """The event-name literals a control's `event_signal_dyn` matches on.

    Only string literals in the `match` arms are counted, which is what distinguishes a delivered
    arm from a comment or a doc example mentioning the same name.
    """
    body = re.search(r"fn event_signal_dyn\(&self, name: &str\).*?\n    \}", source, re.S)
    if body is None:
        return set()
    return set(re.findall(r'"([a-z0-9_]+)"\s*=>', body.group(0)))


def main() -> int:
    inject = None
    for argument in sys.argv[1:]:
        if argument.startswith("--inject="):
            inject = argument.split("=", 1)[1]

    published = read_census()
    failures: list[str] = []
    checked = 0

    for control, relative in sorted(CONVERTED.items()):
        path = REPO / relative
        if not path.exists():
            failures.append(f"{control}: {relative} does not exist")
            continue
        declared = set(published.get(control, []))
        if not declared:
            failures.append(f"{control}: the census publishes no events for it")
            continue
        arm_source = path.read_text()

        if inject is not None and inject.startswith(f"{control}."):
            injected = inject.split(".", 1)[1]
            # Pretend the arm is absent by removing its literal from the text this reads.
            arm_source = arm_source.replace(f'"{injected}" =>', '"__injected__" =>', 1)

        resolved = resolved_names(arm_source)
        checked += 1

        for name in sorted(declared - resolved):
            failures.append(
                f"{control}: `{name}` is published but `event_signal_dyn` does not resolve it"
            )
        for name in sorted(resolved - declared):
            failures.append(
                f"{control}: `event_signal_dyn` resolves `{name}`, which the capability does not "
                "publish — nothing can subscribe to it"
            )

    print(f"converted controls checked: {checked}")
    for control in sorted(CONVERTED):
        print(f"  {control}: {len(published.get(control, []))} published names")

    if failures:
        print()
        print(f"❌ {len(failures)} name(s) disagree between the capability table and the control:")
        for failure in failures:
            print(f"   {failure}")
        print()
        print("   Add the missing arm, or withdraw the name from the capability.")
        return 1

    if inject is not None:
        # An injection that produced no failure means this check is not comparing. Failing here is
        # the point of the mode.
        print(f"❌ --inject={inject} did not produce a failure, so this gate is not checking")
        return 1

    print()
    print("✅ every converted control resolves every event name its capability publishes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
