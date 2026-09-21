#!/usr/bin/env python3
"""The wire-compatibility rules must be data both consumers can read (BLUE19 T-3).

# Why this gate exists

`blue19.md` §3.3 requires the event→target compatibility rules to be **数据化** — readable by a
consumer — and not hard-coded in a `match`. The reason is not style. There are two consumers:

  1. the runtime, which validates a wire when a project loads;
  2. the code generator (T-23), which must decide at generation time whether to emit a direct
     assignment or a conversion, because a generated program cannot ask a question the generator
     already knows the answer to.

A `match` inside the runtime is invisible to the generator, so the two drift: the designer accepts
a wire, and the generated program rejects it. This gate is what makes the table's shape a
requirement rather than an intention.

# What it asserts

  [1] The rule set is a `pub const` array of `WireRule` values (`WIRE_RULES`), not a function.
  [2] Every rule carries a `reason`, so a `Rejected` verdict always has something to show a user.
  [3] `compatibility` looks the pair up in the array rather than matching on kind pairs itself.
      Checked by requiring the function body to iterate the table.
  [4] The verdict enum has a `Rejected` variant carrying a reason, so "not allowed" is
      distinguishable from "allowed".
  [5] The rules mention the case the plan names by hand: `String` → a numeric kind is rejected.

# Reverse injection

Step [3] is the load-bearing one, and step 2 of the shell wrapper removes the table walk and
requires a failure — a check that greps for a symbol it just introduced would pass regardless.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

RULES = pathlib.Path("src/widget/capability/wire_rules.rs")


def strip_comments(text: str) -> str:
    """Drop `//` line comments and `/* */` blocks, so prose cannot satisfy a code check."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def rule_entries(rules: str) -> list[str]:
    """The argument text of every `rule(..)` entry, by brace-matching the parentheses.

    A regex cannot do this: the arguments contain nested `Option(..)` calls, so a non-greedy
    `rule\((.*?)\)` stops at the first inner `)` and reports half an argument list. That is how
    the first version of this script reported twenty rules with "no reason" when every one of them
    has one.
    """
    entries: list[str] = []
    for match in re.finditer(r"\brule\s*\(", rules):
        depth = 1
        index = match.end()
        while index < len(rules) and depth > 0:
            char = rules[index]
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            index += 1
        entries.append(rules[match.end() : index - 1])
    return entries


def check_table_is_data(rules: str) -> list[str]:
    """[1] The rules must be a `const` array, not a function returning verdicts."""
    findings: list[str] = []
    if not re.search(r"pub\s+const\s+WIRE_RULES\s*:\s*&\[WireRule\]", rules):
        findings.append(
            "`WIRE_RULES` is not declared as `pub const WIRE_RULES: &[WireRule]`, so the rule "
            "set is not readable data"
        )
    # The table's own entries: `rule(...)` inside the `WIRE_RULES` array, excluding the `const fn`
    # declaration named `rule` itself.
    array = rules.split("pub const WIRE_RULES", 1)[-1].split("];", 1)[0]
    entries = len(rule_entries(array)) if "rule(" in array else 0
    if entries < 8:
        findings.append(
            f"the rule table has {entries} entries; a table that lists too few pairs cannot be "
            f"the source of truth the runtime and the generator share"
        )
    return findings


def check_every_rule_has_a_reason(rules: str) -> list[str]:
    """[2] Each entry needs a reason, so a refusal is explainable."""
    findings: list[str] = []
    array = rules.split("pub const WIRE_RULES", 1)[-1].split("];", 1)[0]
    entries = rule_entries(array)
    if not entries:
        return ["no `rule(..)` entries found; cannot check that each states a reason"]
    for entry in entries:
        if '"' not in entry:
            findings.append(f"a rule entry states no reason: {entry.strip()[:80]}")
    return findings


def check_lookup_walks_the_table(rules: str) -> list[str]:
    """[3] `compatibility` must consult the array rather than re-list the pairs.

    # Why a symbol-presence check is not enough

    The first version of this step asked whether `WIRE_RULES` appeared inside `compatibility`.
    Reverse injection defeated it immediately: `WIRE_RULES.iter().take(0)` mentions the name and
    reads none of it, so the check stayed green while the function had become a hard-coded answer.
    The step therefore requires an **iteration over the whole table** whose body compares the two
    fields — that is the smallest shape that is load-bearing rather than decorative.
    """
    findings: list[str] = []
    body = re.search(r"pub fn compatibility\(.*?\n\}", rules, re.S)
    if body is None:
        return ["`compatibility` is not defined in this module"]
    code = strip_comments(body.group(0))
    if "WIRE_RULES" not in code:
        findings.append(
            "`compatibility` does not read `WIRE_RULES`, so it must be re-stating the rule set "
            "inline — the duplication this gate exists to prevent"
        )
        return findings
    # A plain iteration over the table with no `.take(`/`.skip(`/index truncation.
    if not re.search(r"for\s+\w+\s+in\s+WIRE_RULES\s*\{", code):
        findings.append(
            "`compatibility` does not iterate `WIRE_RULES` directly; a truncating or filtering "
            "iteration reads only part of the table while still naming it"
        )
    if re.search(r"WIRE_RULES[^\n]*\.(take|skip|rev)\s*\(", code):
        findings.append(
            "`compatibility` truncates its walk over `WIRE_RULES`, so part of the rule set is "
            "unreachable"
        )
    # Both fields must take part in the comparison, or the walk cannot be finding the pair.
    for field in ("source", "target"):
        if not re.search(rf"\.{field}\s*==", code):
            findings.append(
                f"`compatibility` does not compare `.{field}` while walking the table, so it "
                f"cannot be matching the pair it was asked about"
            )
    return findings


def check_rejected_carries_a_reason(rules: str) -> list[str]:
    """[4] "Not allowed" must be distinguishable from "allowed"."""
    findings: list[str] = []
    if not re.search(r"Rejected\s*\(\s*&\s*'static\s+str\s*\)", rules):
        findings.append(
            "`WireCompatibility::Rejected` does not carry a `&'static str`, so a designer or a "
            "generator would have to refuse a wire without being able to say why"
        )
    if "pub fn is_offered" not in rules:
        findings.append(
            "there is no query telling a designer whether to offer a wire, so a caller has to "
            "match on the variants itself"
        )
    return findings


def check_the_named_rejection(rules: str) -> list[str]:
    """[5] `String` → a numeric kind must be rejected, and the reason must name the hazard."""
    findings: list[str] = []
    code = strip_comments(rules)
    # The rejection reason is produced by `rejection_reason`, which must call out the numeric pair.
    if not re.search(
        r"Kind::String\s*\)\s*,\s*Kind::Int\s*\|\s*Kind::UInt\s*\|\s*Kind::Float", code
    ):
        findings.append(
            "the rules do not single out `String` → a numeric kind, which is the rejection "
            "`blue19.md` §3.3 names by hand"
        )
    if "parsing can fail" not in rules:
        findings.append(
            "the `String` → number rejection does not name the hazard (a failed parse has no "
            "defined result here)"
        )
    return findings


def main() -> int:
    if not RULES.exists():
        print(f"❌ {RULES} not found (run from the repo root)")
        return 1

    rules = RULES.read_text()
    findings: list[str] = []
    findings += check_table_is_data(rules)
    findings += check_every_rule_has_a_reason(rules)
    findings += check_lookup_walks_the_table(rules)
    findings += check_rejected_carries_a_reason(rules)
    findings += check_the_named_rejection(rules)

    entries = len(rule_entries(rules.split("pub const WIRE_RULES", 1)[-1].split("];", 1)[0]))
    print(f"wire rules declared: {entries}")
    print()

    if findings:
        print(f"❌ the wire-compatibility rules are not the shared data source ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1

    print("✅ wire rules: the compatibility table is data, every verdict states a reason, and the")
    print("   lookup walks the table instead of restating it")
    return 0


if __name__ == "__main__":
    sys.exit(main())
