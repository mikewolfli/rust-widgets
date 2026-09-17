"""Every gesture `Event` variant must have a real producer outside `#[cfg(test)]`.

# Why this gate exists

The gesture recognizer chain looked complete and was not. `GestureEngine::process`
returned on the first recognizer that produced anything, so recognizers whose input is
*another recognizer's output* were never reached: `Event::DoubleTap` could not be
produced at all, because `TapGesture` (earlier in the chain) satisfied every second tap
first. The recognizer itself was correct and its own tests passed, because those tests
drove it directly and never went through the engine.

That defect is invisible to the gates that existed. `check_behavior_matrix.sh` runs
selected contracts; `check_control_has_tests.sh` asks whether each *control* is built by
a test. Neither asks whether an event a recognizer is supposed to emit can actually reach
a consumer through the real path.

This is principle #75: an event type needs both a producer and a consumer. A recognizer
that can never fire is worse than dead code — it has tests, it has a doc comment
promising the event, and it looks like a working feature.

# What counts as a producer

A construction site — `Event::<Variant> {` that is not a match arm (`=>`) and not inside
a `#[cfg(test)]` module — in the code that is *supposed* to produce it. Test-only
construction is explicitly not a producer: the tests for `DoubleTapGesture` constructed
`Event::DoubleTap` indirectly by driving the recognizer, and that is exactly the evidence
that misled everyone.

# Why the check is per-variant rather than "any construction anywhere"

`Event::Tap` is constructed in several unrelated places. Accepting any of them would let a
variant look produced because some *other* layer happens to build one. The gate therefore
requires a producer in the variant's own producing module, declared in
`GESTURE_PRODUCERS`, so a variant cannot inherit a producer it does not own.

Run from the repo root.
"""
from __future__ import annotations

import pathlib
import re
import sys

GESTURE_DIR = pathlib.Path("src/gesture")
EVENT_FILE = pathlib.Path("src/event/types.rs")

# Where each gesture event is expected to be produced, and why that module.
# A variant with no entry here is reported, so adding a recognizer forces a decision:
# either it produces its event (and gets an entry) or it is a consumer-only type.
GESTURE_PRODUCERS: dict[str, str] = {
    "Tap": "src/gesture/tap.rs",
    "DoubleTap": "src/gesture/tap.rs",
    "TwoFingerTap": "src/gesture/tap.rs",
    "LongPress": "src/gesture/press.rs",
    "Drag": "src/gesture/press.rs",
    "Swipe": "src/gesture/swipe.rs",
    "TwoFingerSwipe": "src/gesture/swipe.rs",
    "Fling": "src/gesture/swipe.rs",
    "Pinch": "src/gesture/pinch.rs",
    "Rotate": "src/gesture/rotate.rs",
}

# The raw touch events every input backend must produce for the recognizers to be fed.
#
# # Why this is the other half of the gesture gate
#
# The recognizer half above proves each gesture *can* be produced. It says nothing about
# whether the engine is ever *called* with real input: `event/loop.rs` gates that on
# `event.is_touch()`, which accepts only `Touch*` and gesture variants — never mouse
# events. So with no backend emitting `TouchBegin`, all eleven recognizers were reachable
# only from unit tests while every gesture test passed.
#
# This maps each raw touch event to the backend files that must construct it. A backend
# that stops emitting touch (a regression in `WM_TOUCH` / `NSTouch` / GDK forwarding) fails
# here rather than silently falling back to "gestures work, they just never run".
TOUCH_BACKENDS: dict[str, list[str]] = {
    "TouchBegin": [
        "src/platform/linux/canvas.rs",
        "src/platform/windows/canvas.rs",
        "src/platform/macos/canvas.rs",
    ],
}


def gesture_event_variants() -> list[str]:
    """Every `Event` variant the gesture layer is responsible for producing.

    Derived from the recognizer modules rather than from the whole `Event` enum: the
    enum also holds mouse, keyboard and window events whose producers are platform
    backends, and asserting those here would report defects that are not this gate's
    subject.
    """
    return sorted(GESTURE_PRODUCERS)


def strip_test_modules(text: str) -> str:
    """`text` with every `#[cfg(test)]` module removed.

    A producer must exist in production code. The cut runs from the attribute to the end
    of the file, matching the convention in this crate that a test module is last.
    """
    match = re.search(r"#\[cfg\(test\)\]", text)
    return text[: match.start()] if match else text


def strip_comments(text: str) -> str:
    """`text` with line comments replaced by spaces, preserving offsets.

    # Why comments must not count

    Documentation mentions variants by name: `pinch.rs` has a doc comment reading
    `Recognizes a two-finger pinch gesture and emits `Event::Pinch { scale }`.`` A
    regex over the raw source matched that sentence and reported `Pinch` as produced even
    with the real construction deleted — the gate answered PASS in exactly the case it
    exists to catch. A comment is a *claim* about a producer, not a producer.

    Length-preserving so that offsets from a match still address the original text.
    """
    out: list[str] = []
    for line in text.splitlines(keepends=True):
        stripped = line.lstrip()
        if stripped.startswith("//"):
            out.append(" " * (len(line) - 1) + ("\n" if line.endswith("\n") else ""))
        else:
            out.append(line)
    return "".join(out)


def production_construction_sites(path: pathlib.Path) -> set[str]:
    """Gesture variants *constructed* in `path`'s production code.

    # Why "is not a match arm" is not enough

    Recognizers both consume and produce events, and a recognizer consumes far more than
    it produces. Every one of these mentions a variant without building one:

    - a match arm: `Event::TouchMove { pos, touch_id } => {`
    - a guarded match arm, whose `=>` is on a later line
    - a test inside `matches!`: `matches!(event, Event::TouchEnd { .. })`
    - a binding: `if let Event::TouchMove { pos, .. } = event`

    An early version only rejected arms, so `TouchMove`, `TouchEnd` and `Timer` were
    reported as produced by `press.rs` — inverting the finding, since those are inputs.

    # What a construction actually looks like

    A construction builds the whole value, so it lists every field it sets and is not
    nested in a pattern-matching macro. The two signals used here:

    1. the fields are concrete (`Event::Drag { pos: *pos, touch_id: .. }`), not the
       placeholder `..` that only ever appears in a pattern;
    2. it is not the second argument of a `matches!` nor the pattern of an `if let`.
    """
    if not path.exists():
        return set()
    text = strip_comments(strip_test_modules(path.read_text(encoding="utf-8")))
    found: set[str] = set()
    for match in re.finditer(r"Event::(\w+)\s*\{", text):
        if _is_match_arm_pattern(text, match):
            continue
        if _is_pattern_only_use(text, match):
            continue
        found.add(match.group(1))
    return found


def _brace_span(text: str, open_index: int) -> tuple[int, int]:
    """The `(start, end)` offsets of the brace group opening at `open_index`."""
    depth = 0
    index = open_index
    while index < len(text):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return open_index, index
        index += 1
    return open_index, len(text) - 1


def _arm_fields(text: str, match: re.Match[str]) -> str:
    """The text of the `Event::X { .. }` pattern starting at `match`."""
    start, end = _brace_span(text, match.end() - 1)
    return text[start + 1 : end]


def _is_match_arm_pattern(text: str, match: re.Match[str]) -> bool:
    """Whether this `Event::X {` opens a match arm rather than a construction.

    Walks to the matching close brace, then reads on to the next non-space characters,
    so a pattern whose guard spans lines is still recognised as a pattern.
    """
    _, end = _brace_span(text, match.end() - 1)
    tail = text[end + 1 : end + 200].lstrip()
    return tail.startswith("=>")


def _is_pattern_only_use(text: str, match: re.Match[str]) -> bool:
    """Whether this mention is a pattern being matched against, not a value built.

    Recognizers consume far more variants than they produce, so "not an arm" is not
    enough — every one of these names a variant without building one:

    - `matches!(subject, Event::X { .. })`
    - `if let Event::X { .. } = ..`
    - a guarded arm, `Event::X { pos } if cond =>`

    A construction, by contrast, sets concrete fields and is followed by the end of the
    expression rather than by `if`, `=>` or a pattern binder.
    """
    fields = _arm_fields(text, match).strip()
    if ".." in fields:
        # `..` only ever appears while matching; a construction names every field.
        return True

    line_start = text.rfind("\n", 0, match.start()) + 1
    prefix = text[line_start : match.start()]
    if "matches!(" in prefix and "," in prefix:
        # `matches!(subject, Event::X {` — the variant is the pattern argument.
        return True
    if "if let" in prefix:
        return True

    # A guarded arm: the fields are concrete, but the arm continues with a guard and
    # then `=>`. Read to the end of the arm rather than of the line, because the guard
    # may wrap (`if self.active && Some(*touch_id) == self.touch_id =>`).
    _, end = _brace_span(text, match.end() - 1)
    tail = text[end + 1 : end + 300].lstrip()
    if tail.startswith("if"):
        return True
    return False


def event_enum_variants() -> set[str]:
    """Every variant declared on `Event`, to catch a name no longer in the enum."""
    text = EVENT_FILE.read_text(encoding="utf-8")
    # Variants are declared at one indent level inside `pub enum Event`.
    return set(re.findall(r"^\s{4}([A-Z][A-Za-z0-9]*)\s*\{", text, re.M))


def main() -> int:
    if not EVENT_FILE.exists():
        print(f"error: {EVENT_FILE} not found; run from the repo root", file=sys.stderr)
        return 2

    declared = event_enum_variants()
    failures: list[str] = []

    for variant in gesture_event_variants():
        if variant not in declared:
            failures.append(
                f"{variant}: listed in GESTURE_PRODUCERS but not a variant of `Event`; "
                f"the recognizer and the enum have drifted apart"
            )
            continue

        producer = pathlib.Path(GESTURE_PRODUCERS[variant])
        sites = production_construction_sites(producer)
        if variant not in sites:
            failures.append(
                f"{variant}: no production construction site in {producer}. "
                f"A recognizer that cannot emit its event is unreachable, not merely "
                f"untested — see principle #75"
            )

    # The reverse direction (principle #77): a recognizer that builds a gesture event
    # nobody registered here would be produced but never checked.
    known = set(gesture_event_variants())
    for path in sorted(GESTURE_DIR.rglob("*.rs")):
        for variant in sorted(production_construction_sites(path)):
            if variant in declared and variant not in known and variant != "TouchBegin":
                failures.append(
                    f"{variant}: produced in {path} but absent from GESTURE_PRODUCERS, "
                    f"so nothing verifies it has a reachable producer"
                )

    # The input half: every backend must actually feed the gesture engine. Without a
    # `TouchBegin` producer the engine is never called with real input, and every
    # recognizer is reachable only from tests — the state this gate was written for.
    for variant, backends in sorted(TOUCH_BACKENDS.items()):
        for backend in backends:
            sites = production_construction_sites(pathlib.Path(backend))
            if variant not in sites:
                failures.append(
                    f"{variant}: {backend} does not construct it, so the gesture engine "
                    f"is never fed real touch input on that backend; every recognizer is "
                    f"then reachable only from tests (principle #75)"
                )

    if failures:
        print("event producer audit FAILED:")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(f"gesture events with a reachable producer: {len(known)} / {len(known)}")
    for variant in gesture_event_variants():
        print(f"  {variant:16} <- {GESTURE_PRODUCERS[variant]}")
    for variant, backends in sorted(TOUCH_BACKENDS.items()):
        print(f"  {variant:16} <- {', '.join(backends)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
