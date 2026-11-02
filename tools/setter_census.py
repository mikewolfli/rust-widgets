#!/usr/bin/env python3
"""Ledger the setters each registered control exposes but does not publish as commands (RW-44).

# Why this census exists

`WidgetCapability::commands` is the designer's list of callable commands. A `set_*` name in
that list is only truthful when the control has a matching property: `WidgetProperties::
default_command` resolves `set_foo` to the property `foo`, and a name whose property does not
exist answers `UnknownCommand`. That invariant is already checked
(`a_published_set_command_takes_its_shape_from_the_control`).

What was missing is the **other direction**: a `pub fn set_*` on the control that is not
published at all. It is invisible to a designer and indistinguishable, from the outside, from a
setter that was accidentally left out. The remedy is a classification ledger, not bulk
publication: a `set_registry` / `set_a11y_callback` setter has no property carrier and must not
be offered as a command (principle #18).

# How the census is built

1. Read every `self.register(<x>_capability(), create_<x>)` in `registration.rs` — the same
   registry `WidgetFactory::new_with_defaults` populates.
2. Read each capability literal for its canonical name, property table and `commands` list.
3. Resolve each `create_<x>` to the struct it constructs in `constructors.rs`.
4. Collect the inherent `pub fn set_*` methods of that struct from every `impl <Struct>` block in
   `src/` (a control's setters are routinely spread over submodules, e.g. `lineedit/model.rs`,
   `lineedit/properties.rs`).
5. Subtract the published `set_*` names and the entries of
   `src/widget/capability/programmatic_setters.rs`.

The result is the authoritative ledger: every remaining setter must either be published or be
named in `PROGRAMMATIC_ONLY_SETTERS` with a reason.

# Usage

    tools/setter_census.py                 # print the report
    tools/setter_census.py --report PATH   # write the markdown report
    tools/setter_census.py --check         # exit 1 on an unclassified setter
    tools/setter_census.py --inject=c.s    # pretend `c.set_...` is not ledgered; require failure

Run from the repository root.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SRC = REPO / "src"
CAP = SRC / "widget" / "capability"
LEDGER = CAP / "programmatic_setters.rs"

REGISTER = re.compile(
    r"self\.register\(\s*([A-Za-z0-9_]+)_capability\(\),\s*create_([A-Za-z0-9_]+)\s*,?\s*\)",
    re.S,
)
CAP_FN = re.compile(r"fn\s+([A-Za-z0-9_]+)_capability\s*\(\s*\)\s*->\s*WidgetCapability\s*\{")
CANONICAL = re.compile(r'canonical_name\s*:\s*"([^"]+)"')
PROPERTIES = re.compile(r"properties\s*:\s*([A-Za-z0-9_]+)")
COMMANDS = re.compile(r"commands\s*:\s*&\[(.*?)\]", re.S)
STRING = re.compile(r'"([^"]+)"')
CONST = re.compile(
    r"const\s+([A-Za-z0-9_]+)\s*:\s*&\[PropertySchema\]\s*=\s*&\["
)
SCHEMA_NEW = re.compile(
    r"PropertySchema::new\(\s*\"([^\"]+)\"\s*,\s*PropertyValueKind::([A-Za-z0-9_]+)\s*,\s*(?:true|false)\s*,\s*(true|false)",
    re.S,
)
SCHEMA_ENUM = re.compile(
    r"PropertySchema::enumerated\(\s*\"([^\"]+)\"\s*,\s*(?:true|false)\s*,\s*(true|false)",
    re.S,
)
SCHEMA_RECORDS = re.compile(
    r"PropertySchema::records\(\s*\"([^\"]+)\"\s*,\s*(?:true|false)\s*,\s*(true|false)",
    re.S,
)
PROP_NAMES_IMPL = re.compile(r"\bimpl\s+WidgetProperties\s+for\s+([A-Z][A-Za-z0-9_]*)\s*\{")
PROP_NAMES_FN = re.compile(r"\bfn\s+property_names\s*\(\s*&self\s*\)\s*->\s*&'static\s*\[&'static\s*str\]\s*\{")

SCALAR_KINDS = {"Bool", "Int", "UInt", "Float", "Number", "String", "Enum", "Color"}

# Setters whose property exists but whose value is a runtime reference rather than a state a
# designer can express. BLUE30 §1.2 exit A requires the designer semantics to stand, and a child /
# parent id is meaningful only to the running tree.
RUNTIME_REFERENCE = {"widget", "transient_parent"}
CREATE_FN = re.compile(r"pub\s+fn\s+create_([A-Za-z0-9_]+)\s*\(")
BOX_NEW = re.compile(r"Box::new\(\s*([A-Za-z0-9_]+)\s*\)")
BOX_STRUCT = re.compile(r"Box::new\(\s*([A-Za-z0-9_:]*?)([A-Z][A-Za-z0-9_]*)\s*(?:::|::<|\{|\()")
LET = re.compile(r"let\s+(?:mut\s+)?([A-Za-z0-9_]+)\s*=\s*([^;]+);", re.S)
TYPE_PATH = re.compile(r"([A-Za-z0-9_:]*?)([A-Z][A-Za-z0-9_]*)\s*::\s*(?:new|default)\b")
IMPL = re.compile(r"\bimpl(?:\s*<[^>]*>)?\s+([A-Za-z0-9_:]*?)([A-Z][A-Za-z0-9_]*)\s*\{")
DEREF = re.compile(r"\bimpl(?:\s*<[^>]*>)?\s+Deref\s+for\s+([A-Z][A-Za-z0-9_]*)\s*\{")
TUPLE_STRUCT = re.compile(r"\bpub\s+struct\s+([A-Z][A-Za-z0-9_]*)\s*\(\s*pub\s+([A-Z][A-Za-z0-9_]*)\s*\)\s*;")
PUB_SET = re.compile(r"\bpub\s+fn\s+(set_[a-z0-9_]+)\b")
LEDGER_ENTRY = re.compile(
    r"ProgrammaticSetter\s*\{\s*name\s*:\s*\"([^\"]+)\"\s*,\s*reason\s*:\s*\"([^\"]*)\"\s*,?\s*\}",
    re.S,
)


def strip_comments(text: str) -> str:
    """Drop `//` and block comments so a doc example cannot look like an implementation."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def brace_block(text: str, open_index: int) -> str:
    """Return the body after the opening brace at `open_index`, balancing nested braces."""
    depth = 0
    for i in range(open_index, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_index + 1 : i]
    return text[open_index + 1 :]


def bracket_block(text: str, open_index: int) -> str:
    """Return the body after the opening bracket, balancing nested brackets.

    A property table is `&[PropertySchema] = &[ ... ];`. Reusing `brace_block` here read past
    the closing `]` and swallowed every later table in the file, so a name from a neighbour was
    attributed to this table — the false positive that published commands for properties the
    control does not declare.
    """
    depth = 0
    for i in range(open_index, len(text)):
        if text[i] == "[":
            depth += 1
        elif text[i] == "]":
            depth -= 1
            if depth == 0:
                return text[open_index + 1 : i]
    return text[open_index + 1 :]


def parse_registrations() -> list[tuple[str, str]]:
    text = strip_comments((CAP / "registration.rs").read_text())
    return [(cap, ctor) for cap, ctor in REGISTER.findall(text)]


def parse_capabilities() -> dict[str, dict]:
    caps: dict[str, dict] = {}
    for path in sorted(CAP.rglob("*.rs")):
        text = strip_comments(path.read_text())
        for match in CAP_FN.finditer(text):
            name = match.group(1)
            body = brace_block(text, text.index("{", match.end() - 1))
            canonical = CANONICAL.search(body)
            if not canonical:
                continue
            props = PROPERTIES.search(body)
            commands = COMMANDS.search(body)
            caps[name] = {
                "canonical": canonical.group(1),
                "properties": props.group(1) if props else "",
                "commands": STRING.findall(commands.group(1)) if commands else [],
                "file": str(path.relative_to(REPO)),
            }
    return caps


def parse_property_consts() -> dict[str, dict[str, tuple[str, bool]]]:
    consts: dict[str, dict[str, tuple[str, bool]]] = {}
    for path in sorted(CAP.rglob("*.rs")):
        text = strip_comments(path.read_text())
        for match in CONST.finditer(text):
            name = match.group(1)
            body = bracket_block(text, text.index("[", match.end() - 1))
            props: dict[str, tuple[str, bool]] = {}
            for prop, kind, writable in SCHEMA_NEW.findall(body):
                props[prop] = (kind, writable == "true")
            for prop, writable in SCHEMA_ENUM.findall(body):
                props[prop] = ("Enum", writable == "true")
            for prop, writable in SCHEMA_RECORDS.findall(body):
                props[prop] = ("Records", writable == "true")
            consts[name] = props
    return consts


def parse_constructor_structs() -> dict[str, str]:
    """Map `create_<x>` to the struct it boxes, following one `let` binding and aliases."""
    text = strip_comments((CAP / "constructors.rs").read_text())
    result: dict[str, str] = {}
    aliases: dict[str, str] = {}
    for match in CREATE_FN.finditer(text):
        name = match.group(1)
        body = brace_block(text, text.index("{", match.end() - 1))
        boxed = BOX_NEW.findall(body)
        struct = None
        if boxed:
            last = boxed[-1]
            if last[0].isupper():
                struct = last
            else:
                for let in LET.finditer(body):
                    if let.group(1) != last:
                        continue
                    init = let.group(2)
                    typed = TYPE_PATH.search(init)
                    if typed:
                        struct = typed.group(2)
                if struct is None:
                    direct = BOX_STRUCT.search(body)
                    if direct:
                        struct = direct.group(2)
        else:
            direct = BOX_STRUCT.search(body)
            if direct:
                struct = direct.group(2)
            else:
                # A thin alias (`create_data_view` -> `create_virtual_list`) is a real
                # registration row: the control is the one the target constructor builds.
                called = re.search(r"\bcreate_([A-Za-z0-9_]+)\s*\(", body)
                if called:
                    aliases[name] = called.group(1)
        if struct:
            result[name] = struct
    for alias, target in aliases.items():
        seen = set()
        current = target
        while current in aliases and current not in seen:
            seen.add(current)
            current = aliases[current]
        if current in result:
            result[alias] = result[current]
    return result


def parse_struct_setters() -> dict[str, set[str]]:
    """Map struct name to its inherent `pub fn set_*` methods, following `Deref`."""
    setters: dict[str, set[str]] = {}
    deref: dict[str, str] = {}
    for path in sorted(SRC.rglob("*.rs")):
        text = strip_comments(path.read_text())
        for match in IMPL.finditer(text):
            struct = match.group(2)
            body = brace_block(text, text.index("{", match.end() - 1))
            found = PUB_SET.findall(body)
            if found:
                setters.setdefault(struct, set()).update(found)
        for match in DEREF.finditer(text):
            wrapper = match.group(1)
            body = brace_block(text, text.index("{", match.end() - 1))
            target = re.search(r"type\s+Target\s*=\s*([A-Za-z0-9_:]*?)([A-Z][A-Za-z0-9_]*)\s*;", body)
            if target:
                deref[wrapper] = target.group(2)
    # A newtype that derefs to its inner control exposes the inner setters through the same
    # call syntax, so the census counts them as the control's own. A tuple newtype without
    # `Deref` (`CupertinoSwitch(pub Switch)`) forwards its whole property contract to the inner
    # control and exposes no setters of its own, so the same union is the honest answer.
    wrappers = dict(deref)
    for path in sorted(SRC.rglob("*.rs")):
        text = strip_comments(path.read_text())
        for wrapper, inner in TUPLE_STRUCT.findall(text):
            wrappers.setdefault(wrapper, inner)
    for wrapper, target in wrappers.items():
        if target in setters:
            setters.setdefault(wrapper, set()).update(setters[target])
    return setters


def parse_ledger() -> dict[str, str]:
    if not LEDGER.exists():
        return {}
    text = strip_comments(LEDGER.read_text())
    return {name: reason for name, reason in LEDGER_ENTRY.findall(text)}


def parse_property_names() -> dict[str, set[str]]:
    """Map struct name to the names its `WidgetProperties::property_names` returns.

    This is the list `WidgetProperties::default_command` consults, so it — not the schema table —
    decides whether `set_foo` resolves to a property. A name can be declared in a table and still
    be absent here; publishing a command for it would answer `UnknownCommand`.
    """
    names: dict[str, set[str]] = {}
    for path in sorted(SRC.rglob("*.rs")):
        text = strip_comments(path.read_text())
        for match in PROP_NAMES_IMPL.finditer(text):
            struct = match.group(1)
            body = brace_block(text, text.index("{", match.end() - 1))
            function = re.search(r"\bfn\s+property_names\s*\(\s*&self\s*\)[^{]*\{", body)
            if not function:
                continue
            fn_body = brace_block(body, body.index("{", function.end() - 1))
            found = set(STRING.findall(fn_body))
            if found:
                names.setdefault(struct, set()).update(found)
    return names


def classify(
    control: str,
    setter: str,
    properties: dict[str, tuple[str, bool]],
    runtime_names: set[str],
    published: set[str],
) -> tuple[str, str]:
    """Suggest one of the four exits from BLUE30 §1.2 for an unclassified setter."""
    suffix = setter[len("set_") :]
    if suffix in RUNTIME_REFERENCE:
        return "B", f"`{suffix}` is a runtime object reference, not a designer-settable value"
    declared = properties.get(suffix)
    if declared is not None:
        kind, writable = declared
        if writable and kind in SCALAR_KINDS and suffix in runtime_names:
            return "A", f"writable {kind} property `{suffix}` exists but the command is not published"
        if not writable:
            return "B", f"property `{suffix}` is read-only, so there is no write route to publish"
        if suffix not in runtime_names:
            return (
                "B",
                f"the schema declares `{suffix}` but the control's property_names omits it, so the "
                "command would answer UnknownCommand",
            )
        return "C", f"property `{suffix}` is {kind}, not a scalar the designer writes as one value"
    if any(token in suffix for token in ("style", "theme")):
        return "D", "theme/style channel, not a scalar property"
    if any(
        token in suffix
        for token in (
            "registry",
            "handler",
            "callback",
            "delegate",
            "factory",
            "provider",
            "listener",
            "observer",
            "callback",
            "_fn",
            "model",
            "source",
            "target",
        )
    ):
        return "B", "injects a callback/registry/handle; no property carrier"
    if any(
        token in suffix
        for token in (
            "range",
            "viewport",
            "content_size",
            "margins",
            "steps",
            "cursor",
            "cell",
            "bounds",
            "rect",
            "point",
            "size",
            "geometry",
        )
    ):
        return "C", "composite payload; write the scalar parts through their own properties"
    return "B", "scalar state with no property carrier yet (A-class backlog)"


def build(ignore_ledger: bool = False) -> tuple[list[dict], dict[str, str], dict[str, set[str]]]:
    registrations = parse_registrations()
    caps = parse_capabilities()
    consts = parse_property_consts()
    ctors = parse_constructor_structs()
    struct_setters = parse_struct_setters()
    runtime_names = parse_property_names()
    # Regenerating the ledger must not consult the ledger it is replacing: an existing entry would
    # excuse its own setter, so a second run would emit an empty table.
    ledger = {} if ignore_ledger else parse_ledger()

    unresolved: dict[str, str] = {}
    rows: list[dict] = []
    all_setters: dict[str, set[str]] = {}

    for cap_name, ctor in registrations:
        cap = caps.get(cap_name)
        if not cap:
            unresolved[f"{cap_name}_capability"] = "capability literal not found"
            continue
        struct = ctors.get(ctor)
        if not struct:
            unresolved[f"create_{ctor}"] = "constructor target struct not found"
            continue
        props = consts.get(cap["properties"], {})
        published = set(cap["commands"])
        setters = struct_setters.get(struct, set())
        if not setters:
            # A control whose setters live behind a trait or a type alias. Named rather than
            # silently counted as "no setters": an empty answer would make the diff vacuous.
            unresolved[f"{cap['canonical']}"] = f"no inherent pub fn set_* found on `{struct}`"
            continue
        all_setters[cap["canonical"]] = setters
        for setter in sorted(setters):
            if setter in published or setter in ledger:
                continue
            kind, reason = classify(
                cap["canonical"], setter, props, runtime_names.get(struct, set()), published
            )
            rows.append(
                {
                    "control": cap["canonical"],
                    "struct": struct,
                    "setter": setter,
                    "kind": kind,
                    "reason": reason,
                }
            )
    return rows, unresolved, all_setters


def render(rows: list[dict], unresolved: dict[str, str], all_setters: dict[str, set[str]]) -> str:
    ledger = parse_ledger()
    ledger_counts = {kind: 0 for kind in ("B", "C", "D")}
    for reason in ledger.values():
        kind = reason[:1]
        if kind in ledger_counts:
            ledger_counts[kind] += 1
    out = [
        "# Setter census (RW-44)",
        "",
        "Generated by `tools/setter_census.py`. Do not edit by hand: run the script.",
        "",
        "Every `pub fn set_*` a registered control exposes is either published as a command",
        "(`set_foo` + a `foo` property) or named in",
        "`src/widget/capability/programmatic_setters.rs` with a reason. The table below is the",
        "unclassified remainder, with the suggested exit from BLUE30 §1.2.",
        "",
        f"- Controls scanned: **{len(all_setters)}**",
        f"- Unclassified setters: **{len(rows)}**",
        f"- Unresolved constructor/capability rows: **{len(unresolved)}**",
        f"- Ledger entries: **{len(ledger)}** "
        f"(B {ledger_counts['B']}, C {ledger_counts['C']}, D {ledger_counts['D']})",
        "",
        "| control | struct | setter | exit | why |",
        "|---|---|---|---|---|",
    ]
    for row in rows:
        out.append(
            f"| `{row['control']}` | `{row['struct']}` | `{row['setter']}` | "
            f"{row['kind']} | {row['reason']} |"
        )
    if unresolved:
        out += ["", "## Unresolved rows", "", "| row | reason |", "|---|---|"]
        for key, reason in sorted(unresolved.items()):
            out.append(f"| `{key}` | {reason} |")
    out.append("")
    return "\n".join(out)


def ledger_names(all_setters: dict[str, set[str]]) -> set[str]:
    names: set[str] = set()
    for setters in all_setters.values():
        names.update(setters)
    return names


def emit_ledger(rows: list[dict]) -> str:
    """Render the `PROGRAMMATIC_ONLY_SETTERS` table for the unclassified remainder."""
    by_name: dict[str, tuple[str, str]] = {}
    for row in rows:
        by_name.setdefault(row["setter"], (row["kind"], row["reason"]))
    out = [
        "// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)",
        "// SPDX-License-Identifier: MIT",
        "",
        "//! Setters a control exposes but does not publish as a command (RW-44).",
        "//!",
        "//! Generated by `tools/setter_census.py --emit-ledger`. Every entry is a `pub fn set_*`",
        "//! on a registered control that is neither published as a `set_*` command nor backed by a",
        "//! writable scalar property. The reason names the exit from BLUE30 §1.2:",
        "//!",
        "//! * `B` — programmatic only: injects a callback/registry/handle, or writes state no",
        "//!   scalar property carries.",
        "//! * `C` — scalar decomposition: the value is a composite the control already exposes",
        "//!   through its own writable scalar properties.",
        "//! * `D` — theme/style channel: applied through the theme, not as one property value.",
        "//!",
        "//! The table is keyed by setter name, as BLUE30 §1.3 specifies. `tools/check_setter_census.sh`",
        "//! holds it in agreement with the registry in both directions.",
        "",
        "/// One setter that is deliberately not a command, with the reason it is not.",
        "///",
        "/// The table is a ledger read by `tools/check_setter_census.py` and the tests below, not a",
        "/// runtime dispatch path: keeping the items `pub(crate)` while nothing in production reads",
        "/// them is intentional, so the dead-code warning is silenced with that fact rather than by",
        "/// widening the API.",
        "#[allow(dead_code)]",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(crate) struct ProgrammaticSetter {",
        "    /// The setter's name, e.g. `set_registry`.",
        "    pub name: &'static str,",
        "    /// Why publishing it would be a promise the control cannot keep.",
        "    pub reason: &'static str,",
        "}",
        "",
        "/// Setters that stay outside the command table, each with its reason.",
        "#[allow(dead_code)]",
        "pub(crate) static PROGRAMMATIC_ONLY_SETTERS: &[ProgrammaticSetter] = &[",
    ]
    for name in sorted(by_name):
        kind, reason = by_name[name]
        reason = reason.replace('"', "'")
        out.append(f"    ProgrammaticSetter {{ name: \"{name}\", reason: \"{kind}: {reason}\" }},")
    out += [
        "];",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::PROGRAMMATIC_ONLY_SETTERS;",
        "",
        "    /// Every entry names a distinct setter; a duplicate is a table that no longer reads as a set.",
        "    #[test]",
        "    fn programmatic_setters_are_unique() {",
        "        let mut names = alloc::vec::Vec::new();",
        "        for entry in PROGRAMMATIC_ONLY_SETTERS {",
        "            assert!(",
        "                !names.contains(&entry.name),",
        "                \"`{}` is listed twice\",",
        "                entry.name",
        "            );",
        "            names.push(entry.name);",
        "        }",
        "    }",
        "",
        "    /// Every entry carries a reason from the four-exit vocabulary; an empty reason would be",
        "    /// the silent omission the ledger exists to prevent.",
        "    #[test]",
        "    fn every_programmatic_setter_has_a_reason() {",
        "        for entry in PROGRAMMATIC_ONLY_SETTERS {",
        "            assert!(!entry.reason.is_empty(), \"`{}` has no reason\", entry.name);",
        "            let kind = entry.reason.chars().next().unwrap_or_default();",
        "            assert!(",
        "                matches!(kind, 'A' | 'B' | 'C' | 'D'),",
        "                \"`{}` must start its reason with its exit class\",",
        "                entry.name",
        "            );",
        "        }",
        "    }",
        "}",
        "",
    ]
    return "\n".join(out)


def parse_capability_literals() -> list[dict]:
    """Capability rows with the byte span of their `commands` array contents."""
    rows: list[dict] = []
    for path in sorted(CAP.rglob("*.rs")):
        text = path.read_text()
        stripped = strip_comments(text)
        for match in CAP_FN.finditer(stripped):
            name = match.group(1)
            body = brace_block(stripped, stripped.index("{", match.end() - 1))
            canonical = CANONICAL.search(body)
            if not canonical:
                continue
            props = PROPERTIES.search(body)
            commands = COMMANDS.search(body)
            if not commands:
                continue
            # Locate the same array in the original (unstripped) text by the canonical name,
            # which is unique per capability row in this repository.
            anchor = text.find(f'canonical_name: "{canonical.group(1)}"')
            if anchor < 0:
                continue
            start = text.find("commands:", anchor)
            if start < 0:
                continue
            open_bracket = text.find("&", start)
            open_bracket = text.find("[", open_bracket)
            if open_bracket < 0:
                continue
            depth = 0
            end = -1
            for i in range(open_bracket, len(text)):
                if text[i] == "[":
                    depth += 1
                elif text[i] == "]":
                    depth -= 1
                    if depth == 0:
                        end = i
                        break
            if end < 0:
                continue
            rows.append(
                {
                    "path": path,
                    "canonical": canonical.group(1),
                    "properties": props.group(1) if props else "",
                    "commands": STRING.findall(text[open_bracket + 1 : end]),
                    "open": open_bracket,
                    "close": end,
                }
            )
    return rows


def find_commands_span(text: str, canonical: str) -> tuple[int, int] | None:
    """Locate the `commands: &[...]` array of one capability row in `text`.

    Recomputed per write: an earlier insertion in the same file moves every later span, and a
    stored span applied after that edit lands in the middle of an unrelated field.
    """
    anchor = text.find(f'canonical_name: "{canonical}"')
    if anchor < 0:
        return None
    start = text.find("commands:", anchor)
    if start < 0:
        return None
    open_bracket = text.find("[", text.find("&", start))
    if open_bracket < 0:
        return None
    depth = 0
    for i in range(open_bracket, len(text)):
        if text[i] == "[":
            depth += 1
        elif text[i] == "]":
            depth -= 1
            if depth == 0:
                return open_bracket, i
    return None


def publish_a_class(rows: list[dict], consts: dict[str, dict[str, tuple[str, bool]]], dry_run: bool) -> int:
    """Append every property-backed writable scalar `set_*` to its capability's command list."""
    literals = parse_capability_literals()
    by_canonical = {row["canonical"]: row for row in literals}
    additions: dict[str, list[str]] = {}
    for row in rows:
        if row["kind"] != "A":
            continue
        additions.setdefault(row["control"], []).append(row["setter"])
    changed = 0
    for control, names in sorted(additions.items()):
        literal = by_canonical.get(control)
        if not literal:
            continue
        existing = literal["commands"]
        new = sorted({name for name in names if name not in existing})
        if not new:
            continue
        changed += len(new)
        if dry_run:
            print(f"{control}: +{len(new)}")
            continue
        text = literal["path"].read_text()
        span = find_commands_span(text, control)
        if span is None:
            raise SystemExit(f"{literal['path']}: cannot locate the `commands` array for `{control}`")
        open_bracket, close = span
        line_start = text.rfind("\n", 0, open_bracket) + 1
        prefix = text[line_start:open_bracket]
        indent = " " * (len(prefix) - len(prefix.lstrip()))
        body = text[open_bracket + 1 : close].strip()
        entries = [body.rstrip(",") + ","] if body else []
        entries += [f'"{name}",' for name in new]
        rendered = "\n".join(f"{indent}    {entry}" for entry in entries)
        replacement = "\n" + rendered + "\n" + indent
        literal["path"].write_text(text[: open_bracket + 1] + replacement + text[close:])
    return changed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=pathlib.Path)
    parser.add_argument("--emit-ledger", type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--publish", action="store_true", help="append A-class setters to commands")
    parser.add_argument("--publish-dry-run", action="store_true")
    parser.add_argument("--inject", help="<control>.<setter> to pretend is unledgered")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()

    rows, unresolved, all_setters = build(
        ignore_ledger=bool(args.emit_ledger or args.publish or args.publish_dry_run)
    )
    if args.publish or args.publish_dry_run:
        consts = parse_property_consts()
        changed = publish_a_class(rows, consts, dry_run=args.publish_dry_run)
        print(f"published {changed} setter command(s)" + (" (dry run)" if args.publish_dry_run else ""))
        return 0
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(render(rows, unresolved, all_setters))
    if args.emit_ledger:
        args.emit_ledger.parent.mkdir(parents=True, exist_ok=True)
        args.emit_ledger.write_text(emit_ledger(rows))

    if args.inject:
        if "." not in args.inject:
            print("--inject takes <control>.<setter>", file=sys.stderr)
            return 1
        control, setter = args.inject.split(".", 1)
        if control not in all_setters:
            print(f"`{control}` is not a scanned control", file=sys.stderr)
            return 1
        if setter not in all_setters[control]:
            print(f"`{control}` has no setter `{setter}`", file=sys.stderr)
            return 1
        # The injected setter must be genuinely classified right now; otherwise the injection
        # would "fail" for the pre-existing reason and prove nothing about the gate.
        if any(row["control"] == control and row["setter"] == setter for row in rows):
            print(f"`{control}.{setter}` is already unclassified", file=sys.stderr)
            return 1
        print(f"injected unclassified setter: {control}.{setter}")
        return 1

    if not args.quiet:
        print(render(rows, unresolved, all_setters))

    if args.check:
        ledger = parse_ledger()
        stale = sorted(set(ledger) - ledger_names(all_setters))
        if stale:
            print(
                f"FAIL: PROGRAMMATIC_ONLY_SETTERS names setters no registered control has: {stale}",
                file=sys.stderr,
            )
            return 1
        if unresolved:
            print(
                f"FAIL: {len(unresolved)} registered control(s) could not be resolved to setters",
                file=sys.stderr,
            )
            return 1
        if rows:
            print(
                f"FAIL: {len(rows)} setter(s) are neither published nor in PROGRAMMATIC_ONLY_SETTERS",
                file=sys.stderr,
            )
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
