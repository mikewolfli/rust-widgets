#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: a schema row flagged `readable: false` must not be answered by its control's `get`.

# Why this gate exists (BLUE21 P3-1d / C3)

`PropertySchema::readable` is not advisory. `WidgetFactory::read_property` looks the name up in
the capability's schema and, when the row says `readable: false`, returns
`CapabilityAccessError::UnsupportedOnWidget` **without ever asking the control**:

    if !property.readable {
        return Err(CapabilityAccessError::UnsupportedOnWidget);
    }

The write path checks `writable` the same way, and that is what makes the two flags dangerous when
they disagree with reality: `write_property` and `read_property` consult *different* flags, so a row
declared `readable: false, writable: true` is settable and unreadable. The caller sets a value, the
control stores it, the control's own `get` would return it — and the reflection layer refuses to
hand it back. "Wrote it, cannot read it" is the one-directional contract the schema exists to
prevent, and it is invisible to every other gate: the Rust compiles, the control is correct, and the
only wrong thing is a boolean in a table.

A real row sat in exactly this state: `candlestick_chart.overlay_count` was declared `false, true`
while `CandlestickChart::get` returns `self.overlays.len()`. It is fixed (now `true, true`) and this
gate is what keeps it that way.

# Why a source-level gate rather than only the sibling Rust test

`a_readable_false_row_must_not_be_answered_by_its_control` in `properties_tests.rs` asserts the same
invariant at runtime, by constructing every registered control and asking its own `get`. That test is
the authority on *behaviour*. This gate answers the narrower question the plan asks for — "is there a
`readable: false` row whose control has a `get` arm for that name?" — and it does so **at the source**,
so it can name the schema line to edit and needs no build. The two are deliberately kept: the test
catches a flag that is wrong about a behaviour the schema cannot see, and this gate catches the flag
even in a profile where the control's test module is not compiled.

# What it asserts

For every property schema row with `readable: false`:

  * the control that owns the declaring `*_PROPERTIES` table is resolved through the registration
    chain (`registration.rs` -> `properties.rs` -> `constructors.rs` -> the type's
    `impl WidgetProperties`);
  * the control's own `get` must not contain a `"<name>" =>` arm.

`geometry` (and the rest of the shared four) are exempt by name: their answer comes from
`base_property_get`, the fallback every control reaches from its `_` arm, so their flags are policy
about a shared value rather than a claim about whether some arm exists. A control-owned property has
no such fallback.

**A row whose owning control cannot be resolved is a finding, not a skip.** A gate that silently
drops the rows it cannot map is a gate that stops checking without saying so, which is the failure
mode this crate treats as worse than a false positive.

# Reverse injection

`--inject=<control>.<property>` requires that the named pair be reported as a violation, so a run
that found nothing because it compared nothing cannot pass.

Usage: tools/check_readable_flag_is_true_when_get_answers.py   (exit 1 on a violation)
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"
CAPABILITY = SRC / "widget" / "capability"

# The shared four, answered by `base_property_get` for every control. Their `readable` flag is
# policy about a value the base owns, not a statement about a control's own `get` arm, so a
# `readable: false` row for one of them is legal and is not this gate's business.
SHARED_BASE_PROPERTIES = frozenset({"enabled", "visible", "tooltip", "geometry"})

# `PropertySchema::new("name", PropertyValueKind::X, readable, writable)` and
# `PropertySchema::enumerated("name", readable, writable, &[...])`. The value-kind argument is
# optional because the enum constructor omits it.
SCHEMA_ROW = re.compile(
    r"PropertySchema::(?:new|enumerated)\(\s*"
    r'"(?P<name>[a-z0-9_]+)"\s*,\s*'
    r"(?:PropertyValueKind::\w+\s*,\s*)?"
    r"(?P<readable>true|false)\s*,\s*"
    r"(?P<writable>true|false)\s*[,)]"
)

# `pub(crate) const FOO_PROPERTIES: &[PropertySchema] = &[ ... ];`
PROPERTY_TABLE = re.compile(
    r"const (?P<const>[A-Z0-9_]+)_PROPERTIES: &\[PropertySchema\]\s*=\s*&\[(?P<body>.*?)\n\s*\];",
    re.S,
)

# `pub(crate) fn button_capability() -> WidgetCapability { ... canonical_name: "button" ... }`
CAPABILITY_FN = re.compile(
    r"pub\(crate\) fn (?P<fn>\w+)_capability\(\)\s*->\s*WidgetCapability\s*\{(?P<body>.*?)\n    \}",
    re.S,
)

# `self.register(button_capability(), create_button);`
REGISTRATION = re.compile(r"self\.register\((?P<capfn>\w+)_capability\(\),\s*(?P<ctor>create_\w+)\)")

# `impl WidgetProperties for Button {`
WIDGET_PROPERTIES_IMPL = re.compile(r"impl\s+WidgetProperties\s+for\s+(?P<type>[A-Za-z0-9_]+)")

# `Box::new(` that is really the box constructor, not the tail of `CheckBox::new(`.
BOX_NEW = r"(?<![A-Za-z0-9_:])Box::new\(\s*"

LINE_COMMENT = re.compile(r"//[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def strip_comments(text: str) -> str:
    """Blanks comments while preserving offsets, so a doc example cannot be read as code.

    The doc comments in `properties_trait.rs` contain `impl WidgetProperties for Button {` as
    *prose*, and a scan that read them would believe a third `Button` impl exists.
    """
    text = BLOCK_COMMENT.sub(lambda m: " " * len(m.group(0)), text)
    return LINE_COMMENT.sub(lambda m: " " * len(m.group(0)), text)


def brace_body(text: str, open_index: int) -> str:
    """The text between the brace at `open_index` and its matching close.

    A regex cannot match a Rust function body, and an over-long fixed window silently reads into
    the next function — which is how an early draft assigned `create_label`'s type to
    `create_button`.
    """
    depth = 0
    for index in range(open_index, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[open_index + 1 : index]
    return ""


def read_sources() -> dict[str, str]:
    """Every `src/**/*.rs`, keyed by the path relative to the repository root.

    Keys are relative so a lookup is identical on every host: an absolute key would make this
    gate's constants depend on where the checkout happens to live.
    """
    return {
        path.relative_to(REPO).as_posix(): strip_comments(path.read_text(encoding="utf-8"))
        for path in sorted(SRC.rglob("*.rs"))
    }


def property_tables(sources: dict[str, str]) -> dict[str, list[tuple[str, bool, bool]]]:
    """`CONST` -> [(name, readable, writable)] for every `*_PROPERTIES` table."""
    tables: dict[str, list[tuple[str, bool, bool]]] = {}
    for relative, text in sources.items():
        if not relative.startswith("src/widget/capability/") or not relative.endswith(".in.rs"):
            continue
        for match in PROPERTY_TABLE.finditer(text):
            rows = [
                (row.group("name"), row.group("readable") == "true", row.group("writable") == "true")
                for row in SCHEMA_ROW.finditer(match.group("body"))
            ]
            tables[match.group("const")] = rows
    return tables


def canonical_to_table(sources: dict[str, str]) -> dict[str, tuple[str, str]]:
    """capability fn name -> (canonical name, `*_PROPERTIES` const)."""
    mapping: dict[str, tuple[str, str]] = {}
    text = sources["src/widget/capability/properties.rs"]
    for match in CAPABILITY_FN.finditer(text):
        body = match.group("body")
        canonical = re.search(r'canonical_name:\s*"(?P<name>[a-z0-9_]+)"', body)
        table = re.search(r"properties:\s*(?P<const>[A-Z0-9_]+)_PROPERTIES", body)
        if canonical and table:
            mapping[match.group("fn")] = (canonical.group("name"), table.group("const"))
    return mapping


def constructor_bodies(sources: dict[str, str]) -> dict[str, str]:
    text = sources["src/widget/capability/constructors.rs"]
    pattern = re.compile(r"pub fn (?P<fn>create_[a-z0-9_]+)\([^)]*\)\s*->\s*Box<dyn Widget>\s*\{")
    return {m.group("fn"): brace_body(text, m.end() - 1) for m in pattern.finditer(text)}


def concrete_type(
    body: str, bodies: dict[str, str], depth: int = 0
) -> str | None:
    """The concrete widget type a `create_*` constructor binds into its returned `Box`.

    Constructor bodies come in four shapes and all four are real in the tree:
    `Box::new(Button::new(..))`, `let mut x = CheckBox::new(..); .. Box::new(x)`,
    `create_virtual_list(geometry, text)` (a delegation to another constructor) and
    `crate::path::CandlestickChart::new(..)` inside a `let`. Recursion is bounded because a
    constructor that delegates to itself would otherwise hang a gate — a hang reports nothing.
    """
    if depth > 4:
        return None
    direct = re.search(BOX_NEW + r"(?P<path>[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)\s*::\w+\s*\(", body)
    if direct:
        return direct.group("path").split("::")[-1]

    boxed = re.search(BOX_NEW + r"(?P<expr>[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)", body)
    if boxed:
        expression = boxed.group("expr")
        if "::" in expression:
            return expression.split("::")[-1]
        # `Box::new(x)` where `x` is a local: find what `x` was bound to.
        binding = re.search(
            r"\blet\s+(?:mut\s+)?" + re.escape(expression) + r"\s*(?::[^=]+)?=\s*"
            r"(?P<path>[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)\s*::",
            body,
        )
        if binding:
            return binding.group("path").split("::")[-1]
        delegated = re.search(
            r"\blet\s+(?:mut\s+)?" + re.escape(expression) + r"\s*(?::[^=]+)?=\s*"
            r"(?P<call>create_[a-z0-9_]+)\s*\(",
            body,
        )
        if delegated:
            return concrete_type(bodies.get(delegated.group("call"), ""), bodies, depth + 1)

    delegation = re.search(r"^\s*(?P<call>create_[a-z0-9_]+)\s*\(", body, re.M)
    if delegation:
        return concrete_type(bodies.get(delegation.group("call"), ""), bodies, depth + 1)
    return None


def widget_properties_files(sources: dict[str, str]) -> dict[str, list[str]]:
    """type name -> the files implementing `WidgetProperties` for it (comments excluded)."""
    files: dict[str, list[str]] = {}
    for relative, text in sources.items():
        for match in WIDGET_PROPERTIES_IMPL.finditer(text):
            files.setdefault(match.group("type"), []).append(relative)
    return files


def get_arms(source: str) -> set[str]:
    """The property-name literals a control's `get` matches on.

    Scoped to the `get` body: a control matches the same names in `set` and `command`, and a
    `set`-only arm is exactly what a read-only property legitimately has. Counting `set` arms
    would report every read-only property as a violation.
    """
    start = source.find("fn get(&self, name: &str)")
    if start < 0:
        return set()
    return set(re.findall(r'"([a-z0-9_]+)"\s*=>', brace_body(source, source.find("{", start))))


def scan(inject: str | None) -> tuple[int, list[str], list[str]]:
    """Returns (rows checked, violations, unresolved mappings)."""
    sources = read_sources()
    tables = property_tables(sources)
    canonical = canonical_to_table(sources)
    bodies = constructor_bodies(sources)
    implementations = widget_properties_files(sources)

    violations: list[str] = []
    unresolved: list[str] = []
    checked = 0

    for capfn, ctor in REGISTRATION.findall(sources["src/widget/capability/registration.rs"]):
        if capfn not in canonical:
            unresolved.append(f"capability function `{capfn}_capability` has no canonical_name")
            continue
        control, table = canonical[capfn]
        if table not in tables:
            unresolved.append(f"{control}: `{table}_PROPERTIES` is not declared in any *.in.rs")
            continue

        typ = concrete_type(bodies.get(ctor, ""), bodies)
        if typ is None:
            unresolved.append(f"{control}: cannot resolve the type {ctor} builds")
            continue
        files = implementations.get(typ, [])
        if len(files) != 1:
            unresolved.append(
                f"{control}: `impl WidgetProperties for {typ}` resolves to {files!r}, "
                "so this gate cannot tell which file owns the control"
            )
            continue
        arms = get_arms(sources[files[0]])
        inert_control = ("", "")
        for name, readable, writable in tables[table]:
            # Reverse injection. `--inject=<control>.<property>` requires the named row to be
            # reported, whatever its current flags are: an injection that only fires when the row
            # already violates the rule would prove nothing about a run that found nothing. So the
            # injection is checked *before* the flags, and it asserts the row is one this gate
            # would compare — the name must exist in this table.
            if inject == f"{control}.{name}":
                if name in SHARED_BASE_PROPERTIES:
                    violations.append(
                        f"{control}: --inject={inject} names a shared base property, which this "
                        "gate exempts by design and therefore never compares"
                    )
                else:
                    violations.append(
                        f"{control}: `{name}` is flagged `readable: false` while its control's "
                        f'`get` in {files[0]} has a "{name}" => arm '
                        f"(reverse injection; declared readable={readable}, writable={writable})"
                    )
                inert_control = (control, name)
                checked += 1
                continue
            if readable:
                continue
            if name in SHARED_BASE_PROPERTIES:
                continue
            checked += 1
            if name in arms:
                shape = (
                    "`readable: false, writable: true`, so a caller can `set` it and `read_property` "
                    "will refuse to read it back"
                    if writable
                    else "`readable: false`, so `read_property` answers UnsupportedOnWidget"
                )
                violations.append(
                    f"{control}: `{name}` is flagged {shape} while `get` in {files[0]} has a "
                    f'"{name}" => arm'
                )

        # The injection named this control but no row in its table, so nothing was compared.
        if inject is not None and inject.startswith(f"{control}.") and inert_control != (
            control,
            inject.split(".", 1)[1],
        ):
            violations.append(
                f"{control}: --inject={inject} names no property in `{table}_PROPERTIES`, so the "
                "injection compared nothing"
            )

    return checked, violations, unresolved


def main() -> int:
    inject = None
    for argument in sys.argv[1:]:
        if argument.startswith("--inject="):
            inject = argument.split("=", 1)[1]

    checked, violations, unresolved = scan(inject)

    print(f"non-readable non-shared schema rows checked: {checked}")
    print(f"unresolved control mappings: {len(unresolved)}")

    if violations or unresolved:
        print(f"failed: {len(violations) + len(unresolved)}")
        print()
        if violations:
            print("`readable: false` is enforced by `WidgetFactory::read_property`, which returns")
            print("`UnsupportedOnWidget` before it asks the control anything. A row whose control")
            print("has a `get` arm therefore hides a value the control is holding — and when the")
            print("same row is `writable: true` the value can be written and never read back.")
            print()
            for violation in violations:
                print(f"  {violation}")
        if unresolved:
            print()
            print("These rows could not be mapped to a control, so this gate did not check them.")
            print("An unmapped row is reported rather than skipped: a gate that quietly drops the")
            print("inputs it cannot read stops checking without saying so.")
            print()
            for finding in unresolved:
                print(f"  {finding}")
        print()
        print("Set `readable: true`, or remove the `get` arm that answers the name.")
        return 1

    if inject is not None:
        print(f"❌ --inject={inject} did not produce a failure, so this gate is not checking")
        return 1

    print("failed: 0  (no `readable: false` row is answered by its control's `get`)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
