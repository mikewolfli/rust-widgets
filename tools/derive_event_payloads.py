#!/usr/bin/env python3
"""Derive the real Rust payload type of every *published* capability event, from source.

# Why this exists

BLUE19 rule #95 requires the event side of `WidgetCapability` to carry a payload kind, the way the
property side carries `PropertyValueKind`. Filling that in by hand for 326 pairs invites guesses,
and a guess is worse than a gap: a wrong `payload` makes a designer offer a wire that cannot be
made, and makes the JSON round-trip assert a type the signal does not have.

So the payload kind is *derived*, not typed in:

  1. this script attributes an emitting struct to a capability (`canonical_name` -> struct
     convention, plus the same alias/`PRODUCER_ELSEWHERE` corrections the emit gate uses);
  2. it reads the signal the event name resolves to in that struct — the field
     (`pub page_changed: Signal1<usize>`) or an inherited base accessor
     (`pub fn clicked_signal(&self) -> &GenericSignal`) — and takes the generic argument;
  3. it maps that Rust type to a `PropertyValueKind` plus an `EventPayloadShape`;
  4. it writes the answer into `src/widget/capability/event_payloads.rs`, which
     `properties.rs` includes and `events_of!` indexes.

The gate that checks the result (`check_event_payload_types.py`) re-derives the same way, so an
edit that disagrees with the signal is a failure rather than a silent divergence.

# Where the *name list* comes from

From the generated table, and from the capability constructors while they still spell the list out
literally. There is no third source: `events:` either lists names (before
`tools/migrate_capability_events.py` rewrites them) or points at this table (after). Reading the
table back is therefore not a shortcut around the derivation — the payloads are still taken from
the signals every run, and the gate asks the reverse question independently. What the table
preserves is the **set of names**, so a derivation that stops resolving a name is a reported error
instead of a name that quietly disappears from the published list.

Usage:  python3 tools/derive_event_payloads.py            # write the table
        python3 tools/derive_event_payloads.py --check     # exit 1 if the table is stale
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"
PROPERTIES = SRC / "widget/capability/properties.rs"
OUT = SRC / "widget/capability/event_payloads.rs"

# The published-name census: control -> the names its capability publishes.
#
# # Why the census is a separate file from the generated table
#
# The table holds names *and* payloads, and is rewritten wholesale by `migrate_capability_events.py`
# (which replaces the literal lists with `events_of!` lookups). So it cannot be the thing the
# derivation reads the names from: a run that overwrote it would leave the next run nothing to
# derive from, and "nothing to derive" and "this control publishes nothing" would look the same.
#
# The census is written once, from the literal lists in `properties.rs`, and is then the fixed
# point both scripts agree on. It is a build-time source like any other: a capability that gains an
# event adds it here (or re-runs the census from the pre-migration source), and the gate compares
# the table against it.
CENSUS = REPO / "tools/event_published_census.txt"

# ── Capability -> emitting struct ───────────────────────────────────────────────────
# The cases the `canonical_name` -> `PascalCase` convention does not reach. The emit gate
# (`check_capability_events_are_emitted.py`) keeps its own copy of the same corrections, so a
# change to one is visible against the other.
PRODUCER_ELSEWHERE: dict[str, str] = {
    "cupertino_switch": "Switch",
    "chart": "ChartWidget",
    "table": "TableWidget",
    "order_book": "OrderBookWidget",
    "freeform_shape": "FreeformShapeWidget",
    "fab": "FAB",
    "lcd_number": "LCDNumber",
}

# Capabilities served by one struct whose name the convention does not reach.
SHARED_PRODUCERS: dict[str, str] = {
    "grid": "GridWidget",
    "grid_table": "GridTableWidget",
    "data_view": "VirtualList",
}

# Signals declared by a *base or parent* struct that a control inherits rather than owns. A
# capability may publish such a name (`button` publishes `clicked`, which every widget carries),
# and the payload is known from the declaring struct.
INHERITED_SIGNALS: dict[str, str] = {
    "clicked": "GenericSignal",
    "changed": "GenericSignal",
}

# A control that publishes an event the derivation cannot resolve is a failure, not a licence to
# omit it: leaving the name out would make `events_of!` publish less than the capability says it
# publishes. This is empty, and it must stay empty — it exists so that the next unresolvable name
# has an obvious home with the reason attached.
MANUAL: dict[tuple[str, str], tuple[str, str]] = {}

# ── Rust payload type -> declared kind ──────────────────────────────────────────────
# The tokens are Rust path segments: `K::Int` is `PropertyValueKind::Int` and `S::Tuple2` is
# `EventPayloadShape::Tuple2`, both brought into scope by the generated file's `use` lines.
SCALARS: dict[str, str] = {
    "bool": "K::Bool",
    "i8": "K::Int",
    "i16": "K::Int",
    "i32": "K::Int",
    "i64": "K::Int",
    "u8": "K::UInt",
    "u16": "K::UInt",
    "u32": "K::UInt",
    "u64": "K::UInt",
    "usize": "K::UInt",
    "f32": "K::Float",
    "f64": "K::Float",
    "String": "K::String",
}

# Types that travel as their token spelling. Listed explicitly rather than pattern-matched: a
# payload that silently fell through to `String` would be a claim nobody made.
DOMAIN_AS_STRING: frozenset[str] = frozenset(
    {
        "Font",
        "Date",
        "Time",
        "DateTime",
        "chrono::NaiveDate",
        "Shortcut",
        "KeySequence",
        "Orientation",
        "DockWidgetArea",
        "DockWidgetFeatures",
        "StandardButton",
        "ButtonState",
        "CheckState",
        "ToggleButtonState",
        "NavigationEvent",
        "DragPayload",
        "BarcodeResult",
        "FilterExpr",
        "SignatureStroke",
        "DateRange",
        "CardPosition",
        "PropertyValue",
        # A finance enum with a handful of variants and no data. Its name is the whole value, which
        # is the same contract `PropertyValueKind::Enum` gives the property side.
        "BookSide",
    }
)

# ── Capability constructors in `properties.rs` ──────────────────────────────────────
CAPABILITY_RE = re.compile(r"pub\(crate\) fn \w+\(\) -> WidgetCapability \{(.*?)\n\}", re.S)
NAME_RE = re.compile(r'canonical_name:\s*"([^"]+)"')
# The two spellings of an event list, tried in order. They are separate patterns rather than one
# alternation because `&\[(.*?)\]` would also match an *empty* list and a rustfmt-multi-line one,
# and a match that yields no names is indistinguishable from "the constructors stopped spelling
# names out" — which is the failure this file must never mistake for success.
EMPTY_EVENTS_RE = re.compile(r"events:\s*&\[\s*\]")
LOOKUP_EVENTS_RE = re.compile(r'events:\s*events_of!\("([a-z0-9_]+)"\)')
LITERAL_EVENTS_RE = re.compile(r"events:\s*&\[(.*?)\s*\]", re.S)
QUOTED_RE = re.compile(r'"([^"]+)"')


class Unsupported(Exception):
    """A payload spelling BLUE19 has not agreed a representation for."""


def pascal_case(name: str) -> str:
    return "".join(part.capitalize() for part in re.split(r"[_\s]+", name) if part)


def normalise(text: str) -> str:
    return " ".join(text.split())


def strip_test_modules(text: str) -> str:
    for marker in ("\nmod tests {", "\n#[cfg(test)]\nmod tests {"):
        cut = text.find(marker)
        if cut != -1:
            return text[:cut]
    return text


def brace_body(text: str, start: int) -> str:
    depth = 1
    index = start
    while index < len(text) and depth > 0:
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        index += 1
    return text[start:index]


def collect_type_aliases() -> dict[str, str]:
    """`pub type Panel = GroupBox;` -> {"Panel": "GroupBox"}, so an alias resolves to its target."""
    aliases: dict[str, str] = {}
    for path in sorted(SRC.rglob("*.rs")):
        for match in re.finditer(r"\npub type (\w+)\s*=\s*(\w+)\s*;", path.read_text()):
            aliases[match.group(1)] = match.group(2)
    return aliases


def collect_signals() -> tuple[dict[str, str], dict[str, str]]:
    """Two flat maps, `struct::name` -> declaration, for fields and for accessors.

    A flat key rather than a nested dict because both lookups are by the same pair, and flattening
    makes "this control does not declare this name" a single `get` instead of two.
    """
    fields: dict[str, str] = {}
    accessors: dict[str, str] = {}
    for path in sorted(SRC.rglob("*.rs")):
        production = strip_test_modules(path.read_text())
        for match in re.finditer(r"\npub struct (\w+)[^{]*\{", production):
            struct = match.group(1)
            body = brace_body(production, match.end())
            for field in re.finditer(r"pub\s+(\w+)\s*:\s*([^;=\n]*Signal[^;=\n]*)", body):
                fields.setdefault(f"{struct}::{field.group(1)}", normalise(field.group(2)))
        # `impl Trait for Type` first (longer, more specific), then the inherent `impl Type`.
        for match in re.finditer(
            r"\nimpl(?:<[^>]*>)?\s+(?:(?:[A-Za-z_]\w*::)*[A-Za-z_]\w*\s+for\s+)?([A-Za-z_]\w*)\b[^{;]*\{",
            production,
        ):
            struct = match.group(1)
            body = brace_body(production, match.end())
            for accessor in re.finditer(
                r"pub fn (\w+)\(&self\)\s*->\s*&([^;{\n]*Signal[^;{\n]*)", body
            ):
                accessors.setdefault(
                    f"{struct}::{accessor.group(1)}", normalise(accessor.group(2))
                )
    return fields, accessors


def payload_of(declaration: str) -> str | None:
    """The generic argument of a signal declaration: `GenericSignal` -> `()`, else the `T`."""
    if "GenericSignal" in declaration:
        return "()"
    open_angle = declaration.find("<")
    close_angle = declaration.rfind(">")
    if open_angle == -1 or close_angle <= open_angle:
        return None
    return normalise(declaration[open_angle + 1 : close_angle])


def split_top_level(text: str) -> list[str]:
    """Split a tuple body on the commas that are not nested inside `<>` or `()`."""
    parts: list[str] = []
    depth = 0
    current: list[str] = []
    for char in text:
        if char in "<([":
            depth += 1
        elif char in ">)]":
            depth -= 1
        if char == "," and depth == 0:
            parts.append(normalise("".join(current)))
            current = []
            continue
        current.append(char)
    tail = normalise("".join(current))
    if tail:
        parts.append(tail)
    return parts


def classify(payload: str) -> tuple[str, str]:
    """(payload kind, payload shape) for one Rust payload spelling.

    Raises `Unsupported` with a reason when the type has no agreed representation — the caller
    reports that rather than inventing one (BLUE19: "do not invent payload semantics").
    """
    payload = normalise(payload)
    if payload == "()":
        return "None", "-"
    if payload in SCALARS:
        return SCALARS[payload], "Scalar"
    if payload in ("Color", "crate::core::Color"):
        return "K::Color", "Scalar"
    if payload in ("Rect", "crate::core::Rect"):
        return "K::Rect", "Scalar"
    if payload in ("Point", "crate::core::Point"):
        # A point is property-shaped but has no `PropertyValueKind` of its own. It is carried as
        # its `"x,y"` spelling; claiming `Rect` would assert a width and height it does not have.
        return "K::String", "Scalar"
    if payload == "ObjectId":
        # An id is an index into the object table, and the property side publishes the same thing
        # as `UInt` — one representation, so a designer can compare an id from a property with an
        # id from an event.
        return "K::UInt", "Scalar"
    if payload.startswith("Option<") and payload.endswith(">"):
        inner_kind, inner_shape = classify(payload[len("Option<") : -1])
        if inner_kind == "None":
            raise Unsupported("Option<()> has no representation")
        return inner_kind, f"Optional{inner_shape}"
    if payload.startswith("Vec<") and payload.endswith(">"):
        inner_kind, inner_shape = classify(payload[len("Vec<") : -1])
        if inner_shape != "Scalar":
            raise Unsupported(f"Vec<{inner_shape}> has no representation")
        return inner_kind, "ListScalar"
    if payload.startswith("(") and payload.endswith(")"):
        parts = split_top_level(payload[1:-1])
        if not parts:
            return "None", "-"
        kinds: list[str] = []
        shapes: list[str] = []
        for part in parts:
            kind, shape = classify(part)
            kinds.append(kind)
            shapes.append(shape)
        if len(set(kinds)) == 1 and set(shapes) <= {"Scalar", "OptionalScalar"}:
            return kinds[0], f"Tuple{len(parts)}"
        # A mixed tuple carries more than one kind, so no single `PropertyValueKind` describes it.
        # `String` is how it travels (BLUE19 D1: no nested representation is claimed for a payload
        # no published wire targets) and `Mixed` is the honest statement that it is richer than a
        # scalar, so a designer will not offer it as a plain string value.
        return "K::String", "Mixed"
    if payload in DOMAIN_AS_STRING:
        return "K::String", "Scalar"
    raise Unsupported(f"no representation agreed for `{payload}`")


def resolve_owner(capability: str, aliases: dict[str, str]) -> str:
    """The struct whose declarations define this capability's events.

    Three sources, in order: an explicit correction, the shared-producer table, and finally the
    crate's own `canonical_name` -> `PascalCase` convention with type aliases followed. Kept as a
    module-level function because the gate must resolve owners **the same way** the generator does:
    two copies of this rule would eventually disagree about which struct an event belongs to, and
    the disagreement would show up as a payload failure rather than as the lookup bug it is.
    """
    if capability in PRODUCER_ELSEWHERE:
        return PRODUCER_ELSEWHERE[capability]
    if capability in SHARED_PRODUCERS:
        return SHARED_PRODUCERS[capability]
    candidate = pascal_case(capability)
    seen: set[str] = set()
    while candidate in aliases and candidate not in seen:
        seen.add(candidate)
        candidate = aliases[candidate]
    return candidate


def resolve_declaration(
    struct: str,
    event: str,
    fields: dict[str, str],
    accessors: dict[str, str],
    fallback_struct: str,
) -> str | None:
    """The signal declaration an event name resolves to on `struct`, if any.

    The four lookups are the two naming conventions (`name` and `name_signal`) crossed with the two
    ways a control exposes a signal (a `pub` field and an accessor). A miss falls back to
    `fallback_struct` — the convention-derived owner — because an override that points at a
    *forwarding* type would otherwise turn a resolvable event into an unresolved one.
    """
    for owner in (struct, fallback_struct):
        for name in (event, f"{event}_signal"):
            if declaration := fields.get(f"{owner}::{name}"):
                return declaration
            if declaration := accessors.get(f"{owner}::{name}"):
                return declaration
    return INHERITED_SIGNALS.get(event)


def collect_published() -> dict[str, list[str]]:
    """control -> event names, from the census file, falling back to the literal lists.

    The census is authoritative once it exists. Falling back to `properties.rs` is what makes the
    first run (and a deliberate re-census) work, and it is only reachable while the constructors
    still spell the names out.
    """
    if CENSUS.exists():
        return read_census()
    return census_from_source()


def census_from_source() -> dict[str, list[str]]:
    """The names as `properties.rs` spells them literally, or via a lookup into the table."""
    source = PROPERTIES.read_text()
    generated = read_generated_names()
    published: dict[str, list[str]] = {}
    for match in CAPABILITY_RE.finditer(source):
        body = match.group(1)
        name_match = NAME_RE.search(body)
        if not name_match:
            continue
        capability = name_match.group(1)
        names = read_sites(body, generated, capability)
        if names is None:
            raise SystemExit(self_check_message(capability, body))
        published[capability] = names
    return published


def write_census(published: dict[str, list[str]]) -> None:
    """Writes the census: one `control: name, name, ...` line per control, sorted.

    A plain text file rather than JSON because its job is to be read and diffed by a human as much
    as by these scripts — an event added or removed shows up as one changed line in review.
    """
    lines = [
        "# Published capability events, one control per line: control: name, name, ...",
        "# Generated from src/widget/capability/properties.rs by tools/derive_event_payloads.py.",
        "# This file is the census the event payload table is derived from; edit it to publish or",
        "# withdraw a name, and re-run the tool.",
    ]
    for capability in sorted(published):
        names = published[capability]
        lines.append(f"{capability}:" + ("" if not names else " " + ", ".join(names)))
    CENSUS.write_text("\n".join(lines) + "\n")


def read_census() -> dict[str, list[str]]:
    """Parse [`CENSUS`] back into control -> names."""
    published: dict[str, list[str]] = {}
    for line in CENSUS.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        capability, _, rest = line.partition(":")
        published[capability.strip()] = [name.strip() for name in rest.split(",") if name.strip()]
    return published


def read_sites(body: str, generated: dict[str, list[str]], capability: str) -> list[str] | None:
    """The names one capability publishes, or `None` when its site is neither spelling."""
    if EMPTY_EVENTS_RE.search(body) is not None:
        return []
    lookup_site = LOOKUP_EVENTS_RE.search(body)
    if lookup_site is not None:
        if lookup_site.group(1) != capability:
            return None
        return generated.get(capability, [])
    literal_site = LITERAL_EVENTS_RE.search(body)
    if literal_site is None:
        # No `events:` key at all: the capability publishes nothing.
        return []
    return QUOTED_RE.findall(literal_site.group(1) or "")


def self_check_message(capability: str, body: str) -> str:
    return (
        f"❌ {capability}: its `events:` site is neither a literal list nor an `events_of!` "
        f"lookup naming itself — the two spellings of its identity disagree.\n   site: {body!r}"
    )


def read_generated_names() -> dict[str, list[str]]:
    """The name columns of [`OUT`], for the controls whose sites are `events_of!` lookups."""
    if not OUT.exists():
        return {}
    table = OUT.read_text()
    section = table.partition("EVENT_SCHEMAS")[2].partition("CONTROL_STARTS")[0]
    names: dict[str, list[str]] = {}
    for capability, event in re.findall(r'\n    \("([a-z0-9_]+)", "([a-z0-9_]+)"', section):
        names.setdefault(capability, []).append(event)
    return names


def main() -> int:
    check_only = "--check" in sys.argv
    if not PROPERTIES.exists():
        print(f"❌ {PROPERTIES} not found (run from the repo root)")
        return 1

    fields, accessors = collect_signals()
    aliases = collect_type_aliases()
    if CENSUS.exists():
        published = read_census()
    else:
        # First run: the constructors still spell the names out, so the census can be taken from
        # them. It is written before the table so the table always has a stable source.
        published = census_from_source()
        write_census(published)
        print(f"wrote {CENSUS}: {sum(len(v) for v in published.values())} published names")

    rows: list[tuple[str, str, str, str]] = []
    unresolved: list[tuple[str, str, str]] = []

    for capability, events in sorted(published.items()):
        if not events:
            continue
        fallback = pascal_case(capability)
        struct = resolve_owner(capability, aliases)
        for event in events:
            declaration = resolve_declaration(struct, event, fields, accessors, fallback)
            if declaration is None:
                unresolved.append((capability, event, f"no signal named `{event}` on `{struct}`"))
                continue
            payload = payload_of(declaration)
            if payload is None:
                unresolved.append((capability, event, f"`{declaration}` has no payload argument"))
                continue
            try:
                kind, shape = classify(payload)
            except Unsupported as reason:
                unresolved.append((capability, event, str(reason)))
                continue
            kind_expr = "None" if kind == "None" else f"Some({kind})"
            shape_expr = "None" if shape == "-" else f"Some(S::{shape})"
            rows.append((capability, event, kind_expr, shape_expr))

    for (capability, event), (kind_expr, shape_expr) in sorted(MANUAL.items()):
        if event in published.get(capability, []):
            rows.append((capability, event, kind_expr, shape_expr))
    rows.sort()

    # A control that publishes events must appear in the table. Losing one is how the published
    # list would silently shrink, which is the failure this whole file exists to prevent.
    expected = {capability for capability, events in published.items() if events}
    written = {capability for capability, *_ in rows}
    if expected - written:
        print(f"❌ {len(expected - written)} control(s) lost their events: {sorted(expected - written)}")
        return 1

    body = render(rows)

    if check_only:
        current = OUT.read_text() if OUT.exists() else ""
        if current != body:
            print(f"❌ {OUT} is stale — run `python3 tools/derive_event_payloads.py`")
            return 1
        print(f"✅ event payload table is current ({len(rows)} pairs)")
        return 0

    OUT.write_text(body)
    print(f"wrote {OUT}: {len(rows)} pairs")

    if unresolved:
        print(f"❌ {len(unresolved)} published event(s) have no derived payload:")
        for capability, event, detail in unresolved:
            print(f"   {capability}: {event}  ({detail})")
        print()
        print("   Each one is a name the capability publishes that the derivation cannot type.")
        print("   Resolve it in MANUAL with the kind and shape the signal really carries, or fix")
        print("   the signal so it is derivable — do not drop the name from the table.")
        return 1
    return 0


HEADER = '''// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// The declared payload of every published capability event — **generated**.
//
// # Why this is generated and not hand-written
//
// BLUE19 rule #95 makes the event side of a capability carry a type, the way the property side
// carries `PropertyValueKind`. There are 326 `(control, event)` pairs; a hand-written table that
// size would be a second source of truth for 186 names, and would drift from the signals on the
// first rename. This table is derived from the real signal declarations by
// `tools/derive_event_payloads.py` and re-derived, independently, by
// `tools/check_event_payload_types.py` — so an edit that disagrees with the signal fails a gate
// instead of quietly misinforming a designer.
//
// Ordinary comments rather than `//!` doc comments because this file is `include!`-ed into
// `properties.rs` mid-module, where an inner doc comment is a compile error.
//
// Regenerate with: `python3 tools/derive_event_payloads.py`.

use super::types::{EventPayloadShape as S, EventSchema};
use super::PropertyValueKind as K;

/// The published events of one control, with their payload types.
///
/// # Why a macro and not 187 hand-written slices
///
/// The names and the payloads are one table because they are one fact: a name without its type is
/// exactly what a designer cannot use. A capability therefore looks its events up instead of
/// re-spelling them:
///
/// ```ignore
/// events: events_of!("button"),
/// ```
///
/// Two things that could drift apart — the list of names a capability publishes and the payload
/// each name carries — are then one row, and the row is derived from the signal.
///
/// # How the lookup stays a compile-time check
///
/// The expansion indexes [`EVENT_SCHEMAS`] with the span [`CONTROL_STARTS`] records for this
/// control, so it is a bounds-checked slice of a `static`: a control whose span is wrong, or that
/// is absent from [`CONTROL_STARTS`], fails to compile rather than publishing less than it says.
macro_rules! events_of {
    ($control:literal) => {{
        const ROW: usize = {
            let mut i = 0usize;
            let mut found = usize::MAX;
            while i < CONTROL_STARTS.len() {
                if str_eq(CONTROL_STARTS[i].0, $control) {
                    found = i;
                    break;
                }
                i += 1;
            }
            if found == usize::MAX {
                panic!("no events derived for this control — run tools/derive_event_payloads.py");
            }
            found
        };
        let (_control, start, len) = CONTROL_STARTS[ROW];
        &EVENT_SCHEMAS[ROW_ROWS[start]..ROW_ROWS[start + len]]
    }};
}

/// Compile-time string equality, for the macro's control lookup.
///
/// Rust has no `const` `str::eq`, and the alternatives are worse: `match` on string literals is
/// not const either, and emitting 162 named constants would make the table unreadable. Walking the
/// bytes is a handful of instructions, all resolved at compile time.
const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Every published event of every control, in `(control, event)` order.
///
/// The rows are [`EventSchema`] values rather than the raw kind and shape, so `events_of!` returns
/// the slice a capability can use directly and the macro does no conversion at the call site.
pub(crate) static EVENT_SCHEMAS: &[EventSchema] = &[
'''

FOOTER = '''];

/// `(control, schema start index, event count)` for every control that publishes an event.
///
/// The span into [`EVENT_SCHEMAS`] for one control, so `events_of!` costs one lookup and no
/// per-row filtering. `EVENT_SCHEMAS` is grouped by control (the generator sorts it), which is what
/// makes a span a contiguous range rather than a scatter.
pub(crate) static CONTROL_STARTS: &[(&str, usize, usize)] = &[
'''

MIDDLE = '''];

/// Index into [`EVENT_SCHEMAS`] for each control's start and end, mirroring [`CONTROL_STARTS`].
///
/// Plain integers, and one entry longer than [`CONTROL_STARTS`] so the last span's end has a home.
/// They exist because a macro expansion cannot do arithmetic on a `static`, and a table of integers
/// is the smallest thing that can be indexed from a `const` context.
pub(crate) static ROW_ROWS: &[usize] = &[
'''


def render(rows: list[tuple[str, str, str, str]]) -> str:
    """The generated file: three tables that `events_of!` indexes.

    Rows are written out one per line rather than produced by a macro so the table stays
    greppable — `("slider", "value_changed", Some(K::Int), Some(S::Scalar))` says what it means
    with no indirection to expand.
    """
    by_control: dict[str, list[tuple[str, str, str]]] = {}
    for capability, event, kind, shape in rows:
        by_control.setdefault(capability, []).append((event, kind, shape))

    events: list[str] = []
    starts: list[str] = []
    offsets: list[str] = []
    for capability in sorted(by_control):
        control_rows = by_control[capability]
        starts.append(f'    ("{capability}", {len(events)}, {len(control_rows)}),\n')
        for event, kind, shape in control_rows:
            offsets.append(f"    {len(events)},\n")
            events.append(f'    EventSchema {{ name: "{event}", payload: {kind}, shape: {shape} }},\n')
    offsets.append(f"    {len(events)},\n")

    return (
        HEADER + "".join(events) + FOOTER + "".join(starts) + MIDDLE + "".join(offsets) + "];\n"
    )


if __name__ == "__main__":
    raise SystemExit(main())
