#!/usr/bin/env python3
"""Every `Event` variant must have either a real producer or a declared host producer.

# Why this gate exists

`tools/check_event_producers.py` covers the *gesture* events, and it caught a real
defect (`Event::DoubleTap` could not be produced). But it deliberately derives its list
from the recognizer modules, so the rest of the `Event` enum was never audited — and the
same defect class was sitting there:

* `GamepadPress` / `GamepadRelease` / `GamepadAxis` / `GamepadConnected` /
  `GamepadDisconnected` — five variants, five constructors, five constructor tests,
  **zero producers and zero consumers** anywhere in the crate. No backend read a gamepad
  API; no widget matched on one. A gate that iterated *all* variants would have named
  them the day they were added.
* `ImeCommit` / `ImePreedit` — matched by the code editor, `tag_input` and `search_bar`,
  and produced by nobody. The platform `ImeBridge` drove the OS composition, but nothing
  joined it to the widget event layer, so those `match` arms were reachable only from a
  test. (`src/platform/ime.rs` now has `deliver_composition` / `deliver_commit`.)
* `OrientationChanged` — no producer and no consumer. A host posts it after reporting the
  new client size, which is the same division `queue_resize_trigger` documents.

The general statement is principle #75: an event type needs **both** a producer and a
consumer, and a variant with neither is not "future work" — it is a contract the library
does not honour, with a doc comment claiming otherwise.

# What counts as a producer

The same definition `check_event_producers.py` uses, imported from it so the two gates
cannot drift: a construction site (`Event::X { .. }` with concrete fields) in production
code — not a match arm, not a pattern, not inside `#[cfg(test)]`, and not inside the file
that declares the enum. Constructing a value only to hand it to `matches!` is not
producing it.

# The two honest exemptions

Some variants genuinely have no in-crate producer because the *host* is the producer — the
library has no window delegate on that platform and says so in the variant's own doc. Those
are listed in `HOST_PRODUCED` with the reason, so adding a variant still forces a decision
("who emits this?") instead of silently passing.

Run from the repo root.
"""
from __future__ import annotations

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from check_event_producers import (  # noqa: E402
    EVENT_FILE,
    production_construction_sites,
)

# Variants the **host** is expected to produce, with the reason. Each entry is a deliberate
# decision that the library cannot emit this event itself; the reason must say why.
HOST_PRODUCED: dict[str, str] = {
    "OrientationChanged": (
        "the host owns the screen-orientation observer (iOS "
        "`UIDevice.orientationDidChangeNotification`, Android `onConfigurationChanged`); the "
        "library stores no UIKit/Activity handle, exactly as `queue_resize_trigger` documents"
    ),
    "Custom": (
        "the free-form escape hatch by definition: the sender and the receiver agree on the "
        "name, and the framework does not interpret the payload"
    ),
    "Quit": (
        "the platform backends post it from their own loop (macOS/Windows/Linux); on a host "
        "that owns its loop the host posts it, and there is no library-owned loop to close"
    ),
}


def declared_variants() -> list[str]:
    """Every variant declared on `Event`, in declaration order."""
    import re

    text = EVENT_FILE.read_text(encoding="utf-8")
    enum_start = text.index("pub enum Event")
    body = text[enum_start:]
    return re.findall(r"^\s{4}([A-Z][A-Za-z0-9]*)\s*\{", body, re.M)


def constructor_helpers() -> dict[str, str]:
    """Map each `Event::<variant>` to the `Event::<helper>` constructor that builds it.

    # Why a helper counts as a construction site

    Much of this crate builds events through their named constructors (`Event::timer(id)`,
    `Event::ime_commit(text)`) rather than with a struct literal, and that is the *preferred*
    spelling — the constructor is where the fields are defaulted consistently. A gate that only
    accepted literals would report `Timer` as unproduced even though `event/timer.rs` posts it
    from two places.

    The helper is only registered when its **body** builds the variant, so a helper that
    forwards to another helper (or does not build the variant at all) is not credited: the
    chain is walked to an actual literal.

    The constructors inside `impl Event` spell the variant `Self::X { .. }`, so both that and
    the fully-qualified `Event::X { .. }` are accepted.
    """
    import re

    text = EVENT_FILE.read_text(encoding="utf-8")
    helpers: dict[str, str] = {}
    for match in re.finditer(
        r"pub fn (\w+)\([^)]*\)\s*->\s*Self\b[^{]*\{([^}]*)\}", text, re.S
    ):
        helper, body = match.group(1), match.group(2)
        built = re.search(r"(?:Event|Self)::([A-Z]\w+)\s*\{", body)
        if built:
            helpers.setdefault(built.group(1), helper)
    return helpers


def helper_is_called(variant: str) -> bool:
    """Whether some production module calls the helper that builds `variant`."""
    import re

    helper = constructor_helpers().get(variant)
    if helper is None:
        return False
    call = re.compile(rf"Event::{helper}\s*\(")
    for path in pathlib.Path("src").rglob("*.rs"):
        if path == EVENT_FILE:
            continue
        text = path.read_text(encoding="utf-8")
        if re.search(r"#\[cfg\(test\)\]", text):
            text = text[: text.index("#[cfg(test)]")]
        if call.search(text):
            return True
    return False


def main() -> int:
    if not EVENT_FILE.exists():
        print(f"error: {EVENT_FILE} not found; run from the repo root", file=sys.stderr)
        return 2

    produced: set[str] = set()
    for path in pathlib.Path("src").rglob("*.rs"):
        if path == EVENT_FILE:
            continue
        produced |= production_construction_sites(path)

    declared = declared_variants()
    orphans = [
        v
        for v in declared
        if v not in produced and v not in HOST_PRODUCED and not helper_is_called(v)
    ]

    if orphans:
        print("event variant audit FAILED: these variants have no producer (principle #75):")
        for variant in orphans:
            print(f"  - Event::{variant}")
        print()
        print("  Either add a real production construction site, or — if the *host* is the")
        print("  producer because the library has no handle on that platform — list it in")
        print("  HOST_PRODUCED with the reason, so the decision is written down rather than")
        print("  assumed.")
        return 1

    print(
        f"event variants: {len(declared)} declared, "
        f"{len(produced & set(declared))} produced in-crate, "
        f"{len([v for v in declared if helper_is_called(v)])} produced via their constructor, "
        f"{len([v for v in declared if v in HOST_PRODUCED])} host-produced"
    )
    for variant in declared:
        if variant in HOST_PRODUCED:
            print(f"  {variant:20} <- host ({HOST_PRODUCED[variant]})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
