#!/usr/bin/env python3
"""Every published capability event must resolve to a signal its own widget really emits.

# Why this gate exists

`src/widget/capability/properties.rs` publishes, for each of the 187 capabilities, a list of
event *names*:

    events: &["clicked", "pressed", "released", "state_changed"],

Those names are a public contract. `WidgetFactory::connect_event` accepts a name by comparing it
against this list, and `EventSignalBinder` turns a name into a live subscription. Three separate
things therefore have to agree:

  1. the name published in the capability table,
  2. a signal field (or accessor) that the name resolves to,
  3. an `.emit(..)` call site in the widget's **own** code that fires it.

Nothing checked any of the three against each other. Two real defects were found by hand:

  * `button`/`toggle_button` published `pressed`/`released` while their fields are
    `pressed_signal`/`released_signal`, so `connect_event("button", "pressed", ..)` returned `Ok`
    and no code path ever emitted under that name.
  * `find_replace_dialog` published five names (`find_next`, `find_previous`, `replace`,
    `replace_all`, `close`) that its signals do not carry — they are `find_next_signal`, ... and
    `close_signal` — so a consumer subscribed to five names nothing emits.

# Why the emission check is scoped per struct

An earlier draft of this gate asked only "does *any* file emit this name?". That is too weak, and
it was measured to be too weak: deleting `Switch`'s only `self.toggled.emit(..)` still passed,
because `CheckBox`, `GroupBox`, `CollapsiblePane`, `Action` and `SplitButton` all emit `toggled`
too. A name emitted by *some other control* is not a producer for this one — the same lesson
`tools/check_event_producers.py` records for gesture variants.

So the emission side is attributed to the struct: the capability's `canonical_name` is mapped to
the crate's own struct-name convention (`switch` -> `Switch`, `find_replace_dialog` ->
`FindReplaceDialog`), and only `.emit(..)` calls inside that struct's `impl` blocks count. When
the convention does not resolve to a known struct the pair is reported as unverifiable rather
than silently accepted.

Two deliberate exemptions, each requiring a reason so the list cannot become a silent escape
hatch:

  * `DATA_SOURCE_SIGNALS` — signals owned by a *model/data-source* trait. `TreeView` and friends
    subscribe to `model.data_changed_signal()`; the model emits it, so correctly there is no
    `.emit()` in the widget layer.
  * `ACCESSOR_ALIASES` — an accessor returning a differently-named field
    (`on_theme_changed()` returns `theme_changed`), where the name mismatch is the only reason a
    textual match fails.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

PROPERTIES = pathlib.Path("src/widget/capability/properties.rs")
SRC = pathlib.Path("src")

# Signals owned by a data-source trait, emitted by the model rather than the widget layer.
DATA_SOURCE_SIGNALS: dict[str, str] = {
    "data_changed_signal": "trait method; the model owns and emits this signal",
}

# Accessors whose underlying field is named differently, with the field that is emitted.
ACCESSOR_ALIASES: dict[str, str] = {
    "on_theme_changed": "theme_changed",
    "hover_signal": "hover",
    "mouse_down_signal": "mouse_down",
    "mouse_up_signal": "mouse_up",
    "key_down_signal": "key_down",
    "key_up_signal": "key_up",
    "focus_gained_signal": "focus_gained",
    "focus_lost_signal": "focus_lost",
    "redraw_requested_signal": "redraw_requested",
    "layout_requested_signal": "layout_requested",
}

# Capabilities whose events are produced by a *helper* type rather than the widget struct itself,
# or whose struct name genuinely differs from the `canonical_name` convention. Aliases declared in
# the crate (`pub type Panel = GroupBox;`) are resolved automatically by `collect_type_aliases`, so
# only real mismatches belong here. The value is the emitting type and the reason.
PRODUCER_ELSEWHERE: dict[str, tuple[str, str]] = {
    "cupertino_switch": ("Switch", "the iOS wrapper forwards every event to its inner `Switch`"),
    "chart": ("ChartWidget", "`WidgetKind::Chart` is served by the `ChartWidget` struct"),
    "table": ("TableWidget", "the `table` capability is served by `TableWidget`"),
    "order_book": ("OrderBookWidget", "the `order_book` capability is served by `OrderBookWidget`"),
    "freeform_shape": (
        "FreeformShapeWidget",
        "the `freeform_shape` capability is served by `FreeformShapeWidget`",
    ),
    "fab": ("FAB", "`fab` is the short name for the `FAB` struct"),
    "lcd_number": ("LCDNumber", "the type is spelled `LCDNumber`, not `LcdNumber`"),
}

# Emitting type -> the capability whose events it owns, for types that serve two capabilities.
SHARED_PRODUCERS: dict[str, str] = {
    "GridWidget": "grid",
    "GridTableWidget": "grid_table",
    "VirtualList": "data_view",
}

CAPABILITY_RE = re.compile(r"pub\(crate\) fn \w+\(\) -> WidgetCapability \{(.*?)\n\}", re.S)
NAME_RE = re.compile(r'canonical_name:\s*"([^"]+)"')
EVENTS_RE = re.compile(r"events:\s*&\[(.*?)\]", re.S)
QUOTED_RE = re.compile(r'"([^"]+)"')

FIELD_RE = re.compile(r"pub\s+(\w+)\s*:\s*[^;=\n]*Signal[^;=\n]*")
ACCESSOR_RE = re.compile(r"pub fn (\w+)\(&self\)\s*->\s*&[^;{\n]*Signal")


def strip_test_modules(text: str) -> str:
    """Drop everything from the file's test module onward.

    A signal emitted only from a test is not emitted: the point is that the production path fires
    it. `ChartWidget`'s hover had tests and (before it was fixed) no `MouseLeave` arm.
    """
    for marker in ("\nmod tests {", "\n#[cfg(test)]\nmod tests {"):
        cut = text.find(marker)
        if cut != -1:
            return text[:cut]
    return text


def pascal_case(name: str) -> str:
    """`find_replace_dialog` -> `FindReplaceDialog`, the crate's own struct convention."""
    return "".join(part.capitalize() for part in re.split(r"[_\s]+", name) if part)


def collect_signals() -> set[str]:
    names: set[str] = set()
    for path in sorted(SRC.rglob("*.rs")):
        text = path.read_text()
        for match in FIELD_RE.finditer(text):
            names.add(match.group(1))
        for match in ACCESSOR_RE.finditer(text):
            names.add(match.group(1))
    return names


def collect_type_aliases() -> dict[str, str]:
    """`pub type Panel = GroupBox;` -> {"Panel": "GroupBox"}, so an alias resolves to its target.

    The crate declares several controls as aliases rather than distinct structs (`Panel` for
    `GroupBox`, `Grid` for `GridWidget`, `DataView` for `VirtualList`). Hardcoding those would make
    the gate stale the moment one is added; reading them keeps the convention honest.
    """
    aliases: dict[str, str] = {}
    for path in sorted(SRC.rglob("*.rs")):
        for match in re.finditer(r"\npub type (\w+)\s*=\s*(\w+)\s*;", path.read_text()):
            aliases[match.group(1)] = match.group(2)
    return aliases


def collect_struct_emits() -> dict[str, set[str]]:
    """Type name -> the signal names emitted inside its own `impl` blocks, tests excluded.

    `impl` headers are parsed in both spellings: `impl Type {` (inherent) and
    `impl Trait for Type {` (trait). Attributing on the *first* identifier after `impl` was wrong
    and was caught by the gate reporting `Canvas`/`AppBar`/`Menu` as never emitting their events —
    for a trait impl the first identifier is the **trait**, so `impl EventHandler for Canvas` was
    filed under `EventHandler` and the emits never reached `Canvas`.
    """
    emits: dict[str, set[str]] = {}
    # `impl Trait for Type` first (longer, more specific), then the inherent `impl Type`. The trait
    # may itself be path-qualified (`impl crate::event::EventHandler for Foo`), which an earlier
    # `[A-Za-z_]\w*` pattern did not match — that silently dropped every emit in files whose trait
    # impls are written with a path, reporting `OrderBookWidget`, `LCDNumber` and `FAB` as never
    # emitting the events they in fact emit.
    header_re = re.compile(
        r"\nimpl(?:<[^>]*>)?\s+(?:(?:[A-Za-z_]\w*::)*[A-Za-z_]\w*\s+for\s+)?([A-Za-z_]\w*)\b[^{;]*\{"
    )
    for path in sorted(SRC.rglob("*.rs")):
        production = strip_test_modules(path.read_text())
        for match in header_re.finditer(production):
            # With the optional trait part made non-capturing, group 1 is always the type.
            struct = match.group(1)
            depth = 1
            index = match.end()
            while index < len(production) and depth > 0:
                char = production[index]
                if char == "{":
                    depth += 1
                elif char == "}":
                    depth -= 1
                index += 1
            body = production[match.end() : index]
            for call in re.finditer(r"(\w+)\.emit\(", body):
                emits.setdefault(struct, set()).add(call.group(1))
    return emits


def collect_struct_declares() -> dict[str, set[str]]:
    """Type name -> the `pub` signal fields / accessors *it* declares.

    The reverse-direction counterpart of `collect_struct_emits`: a name this type both declares and
    emits is a public event of that control, so the capability table has to publish it.
    """
    declares: dict[str, set[str]] = {}
    for path in sorted(SRC.rglob("*.rs")):
        production = strip_test_modules(path.read_text())
        for match in re.finditer(r"\npub struct (\w+)[^{]*\{", production):
            struct = match.group(1)
            depth = 1
            index = match.end()
            while index < len(production) and depth > 0:
                char = production[index]
                if char == "{":
                    depth += 1
                elif char == "}":
                    depth -= 1
                index += 1
            body = production[match.end() : index]
            for field in re.finditer(r"pub\s+(\w+)\s*:\s*[^;=\n]*Signal", body):
                declares.setdefault(struct, set()).add(field.group(1))
    return declares


def main() -> int:
    if not PROPERTIES.exists():
        print(f"❌ {PROPERTIES} not found (run from the repo root)")
        return 1

    source = PROPERTIES.read_text()
    signals = collect_signals()
    struct_emits = collect_struct_emits()
    struct_declares = collect_struct_declares()
    aliases = collect_type_aliases()

    def resolve_owner(capability: str) -> tuple[str, bool]:
        """The struct that should carry this capability's emit sites, and whether it was named
        explicitly (a convention guess that resolves to nothing is reported, not accepted)."""
        if capability in PRODUCER_ELSEWHERE:
            return PRODUCER_ELSEWHERE[capability][0], True
        candidate = pascal_case(capability)
        # Follow crate type aliases (`Grid` -> `GridWidget`) however many hops.
        seen = set()
        while candidate in aliases and candidate not in seen:
            seen.add(candidate)
            candidate = aliases[candidate]
        return candidate, False

    unresolved: list[tuple[str, str]] = []
    unemitted: list[tuple[str, str, str, str]] = []
    unattributable: list[tuple[str, str, str]] = []
    unpublished: list[tuple[str, str, str]] = []
    pairs = 0
    distinct: set[str] = set()

    for match in CAPABILITY_RE.finditer(source):
        body = match.group(1)
        name_match = NAME_RE.search(body)
        events_match = EVENTS_RE.search(body)
        if not name_match or not events_match:
            continue
        capability = name_match.group(1)
        events = QUOTED_RE.findall(events_match.group(1))
        if not events:
            continue

        struct, explicit = resolve_owner(capability)
        if capability in SHARED_PRODUCERS:
            struct = SHARED_PRODUCERS[capability]
            explicit = True
        owned = struct_emits.get(struct)

        for event in events:
            pairs += 1
            distinct.add(event)

            resolved = {event, f"{event}_signal"} & signals
            if not resolved:
                alias = ACCESSOR_ALIASES.get(event)
                if alias and alias in signals:
                    resolved = {alias}
            if not resolved:
                unresolved.append((capability, event))
                continue

            if event in DATA_SOURCE_SIGNALS:
                continue

            if owned is None:
                # The struct-name convention did not find a type with impl blocks, so the
                # emission cannot be attributed. Reported rather than assumed correct.
                unattributable.append((capability, event, struct))
                continue

            if not (resolved & owned):
                unemitted.append((capability, event, ", ".join(sorted(resolved)), struct))

    # ── Reverse direction ─────────────────────────────────────────────────────
    #
    # The checks above ask whether every *published* name is backed by an emit. Nothing asked the
    # mirror question: is every name a control *emits* actually published? A signal that is
    # declared `pub`, emitted from production code, and documented — but absent from the table —
    # is rejected by `connect_event` with `UnknownCommand`, so the control's own documented event
    # cannot be subscribed to by name at all. 24 such names were found across 13 capabilities when
    # this direction was added; `Slider::slider_pressed`/`slider_released`, `ToolButton::triggered`,
    # `ChartWidget::data_point_unhovered` and `FileDialog::current_changed` are all emitted and
    # tested, so each was reachable by a Rust field accessor and by nothing else.
    #
    # Only names the struct **both declares and emits** count, which is what keeps framework noise
    # out: `BaseWidget`'s `hover`/`mouse_down`/`focus_gained` are declared there but emitted by
    # `BaseWidget` rather than by the control, so no capability owns them.
    for match in CAPABILITY_RE.finditer(source):
        body = match.group(1)
        name_match = NAME_RE.search(body)
        events_match = EVENTS_RE.search(body)
        if not name_match or not events_match:
            continue
        capability = name_match.group(1)
        published = set(QUOTED_RE.findall(events_match.group(1)))
        struct, _ = resolve_owner(capability)
        if capability in SHARED_PRODUCERS:
            struct = SHARED_PRODUCERS[capability]
        declared = struct_declares.get(struct, set())
        emitted = struct_emits.get(struct, set())
        for signal in sorted(declared & emitted):
            base = signal[: -len("_signal")] if signal.endswith("_signal") else signal
            if signal in published or base in published:
                continue
            if signal in DATA_SOURCE_SIGNALS:
                continue
            unpublished.append((capability, signal, struct))

    failures = 0
    print(f"Published events: {pairs} pairs across {len(distinct)} distinct names")
    print(f"Public signal names discovered: {len(signals)}")
    print(f"Structs with attributed emit sites: {len(struct_emits)}")
    print()

    if unresolved:
        failures += 1
        print(f"❌ {len(unresolved)} published event(s) resolve to NO signal:")
        for capability, event in unresolved:
            print(f"   {capability}: {event}")
        print()
        print("   Such a name is accepted by `connect_event` and never delivered.")

    if unemitted:
        failures += 1
        print(f"❌ {len(unemitted)} published event(s) have a signal their widget never emits:")
        for capability, event, signal, struct in unemitted:
            print(f"   {capability}: {event}  (signal `{signal}`, struct `{struct}`)")
        print()
        print("   The signal exists, so a caller can subscribe and will never be called.")

    if unattributable:
        failures += 1
        print(f"❌ {len(unattributable)} published event(s) could not be attributed to a struct:")
        for capability, event, struct in sorted(set(unattributable)):
            print(f"   {capability}: {event}  (expected struct `{struct}`)")
        print()
        print("   Add the capability to PRODUCER_ELSEWHERE with the type that emits its events,")
        print("   or fix the `canonical_name` so the convention resolves.")

    if unpublished:
        failures += 1
        print(f"❌ {len(unpublished)} emitted signal(s) are NOT published by their capability:")
        for capability, signal, struct in sorted(unpublished):
            print(f"   {capability}: {signal}  (struct `{struct}`)")
        print()
        print("   The control emits and documents it, but `connect_event(name, event)` answers")
        print("   `UnknownCommand`, so it can only be reached through the Rust field accessor.")
        print("   Add the name to the capability's `events:` list, or mark the signal internal.")

    if failures:
        return 1

    print("✅ capability events: every published name resolves to a signal its own widget emits")
    return 0


if __name__ == "__main__":
    sys.exit(main())
