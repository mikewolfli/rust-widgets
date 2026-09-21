#!/usr/bin/env python3
"""The generator must reuse the runtime's wire rules, not restate them (BLUE19 T-23 DoD).

# Why this gate exists

T-23's completion criteria include a line that is easy to satisfy on paper and easy to violate in
practice:

> 生成器复用 T-3 的类型兼容规则（**不是**另写一套）

The violation is not a missing call — it is a **second `match`** that happens to agree today. A
generator that decides conversions with its own arms will agree with `WIRE_RULES` until someone adds
a kind, at which point the designer accepts a wire the generated program rejects. Nothing about that
failure is visible in the generator's own tests.

# What this gate asserts

  [1] `src/designer/generator.rs` names `WIRE_RULES` (the shared table).
  [2] It does **not** contain a second compatibility verdict: no comparison of two
      `PropertyValueKind` values that yields a conversion decision.
  [3] The shared table is reachable from the generator without going through `crate::json`, so the
      reuse is a real dependency rather than a re-export that only exists on one profile.
  [4] `shared_wire_rule_count()` reports a non-zero count, so the seam is live.

# Reverse injection

Step 4 of the shell wrapper adds a local verdict table and requires the gate to fail on [2].
Without it, [1] alone would pass against a generator that named the table and then ignored it —
which is exactly the decorative-reuse failure this gate is for.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

GENERATOR = pathlib.Path("src/designer/generator.rs")
WIRE_RULES = pathlib.Path("src/widget/capability/wire_rules.rs")

# The verdict names, kept for the doc and for the reverse-injection description. The check itself uses
# `VERDICT_CONSTRUCTION_RE`, because a *reference* to a verdict is not a violation.
VERDICTS = ("WireCompatibility::Direct", "WireCompatibility::Converted", "WireCompatibility::Rejected")


def production_code(text: str) -> str:
    """
    The module without its test module, comments included or not.

    # Why the test module has to be cut

    The gate's first run reported the generator as producing its own verdicts — because the
    *tests* assert `matches!(.., WireCompatibility::Rejected(_))`. Asserting on a verdict is exactly
    what a correct consumer does, so a check that could not tell an assertion from a construction
    would push the tests out of the file. Cutting at the `#[cfg(test)]` module is what makes the
    check about production behaviour.
    """
    marker = "\n#[cfg(test)]\nmod tests {"
    cut = text.find(marker)
    return text if cut == -1 else text[:cut]


def strip_comments(text: str) -> str:
    """Drop `//` line comments and `/* */` blocks, so prose cannot satisfy a code check."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


# A verdict *constructed* in production code. `matches!`/`assert_eq!` on a verdict is a consumer;
# `Some(WireCompatibility::X(..))` or a bare `-> WireCompatibility::X` is a producer.
VERDICT_CONSTRUCTION_RE = re.compile(r"(?:Some|Box::new)?\(?WireCompatibility::(Direct|Converted|Rejected)")


def check_table_is_named(text: str) -> list[str]:
    """[1] The generator must name the shared table."""
    findings: list[str] = []
    if "WIRE_RULES" not in text and "shared_wire_rule_count" not in text:
        findings.append(
            "`src/designer/generator.rs` never names `WIRE_RULES`, so it is not reusing the "
            "runtime's compatibility rules (T-23 DoD)"
        )
    return findings


def check_no_second_verdict(text: str) -> list[str]:
    """[2] The generator must not construct its own verdict in production code."""
    findings: list[str] = []
    code = strip_comments(production_code(text))
    for match in VERDICT_CONSTRUCTION_RE.finditer(code):
        verdict = match.group(1)
        findings.append(
            f"the generator constructs `WireCompatibility::{verdict}` itself, so it has its own "
            f"compatibility verdict rather than reading the shared table — the second rule set the "
            f"T-23 DoD forbids"
        )
        break
    # A `match` over two property kinds is the other shape a second rule set takes.
    for match in re.finditer(r"match\s*\(?\s*([^)]{0,120}?)\)?\s*\{", code, re.S):
        scrutinee = match.group(1)
        if scrutinee.count("PropertyValueKind") >= 2:
            findings.append(
                "the generator matches on two property kinds directly, which is how a second "
                "compatibility table starts"
            )
            break
    return findings


def check_shared_table_is_profile_independent(text: str) -> list[str]:
    """[3] The reuse must not depend on a profile-gated module."""
    findings: list[str] = []
    if "WIRE_RULES" in text:
        import_block = re.search(r"^use .*WIRE_RULES.*$", text, re.M)
        if import_block is None:
            findings.append(
                "`WIRE_RULES` is used but not imported by name, so where it comes from cannot be "
                "checked"
            )
        elif "crate::json" in import_block.group(0):
            findings.append(
                "the generator reaches `WIRE_RULES` through `crate::json`, which is "
                "`full_widgets`-gated; the rules live in `capability`, which is available more "
                "widely, so routing through `json` narrows them for no reason"
            )
    if not WIRE_RULES.exists():
        findings.append(f"{WIRE_RULES} is missing; the shared table has no definition")
    return findings


def check_reported_count(text: str) -> list[str]:
    """[4] The live seam must report a non-zero count."""
    findings: list[str] = []
    body = re.search(r"pub fn shared_wire_rule_count\(\)[^{]*\{\s*([^}]*)\}", text)
    if body is None:
        findings.append("`shared_wire_rule_count` is missing, so the reuse has no live seam")
    elif "WIRE_RULES.len()" not in body.group(1):
        findings.append(
            "`shared_wire_rule_count` does not read `WIRE_RULES.len()`, so it would keep "
            "reporting a count after the table stopped being used"
        )
    return findings


def main() -> int:
    if not GENERATOR.exists():
        print(f"❌ {GENERATOR} not found (run from the repo root)")
        return 1

    text = GENERATOR.read_text()
    findings: list[str] = []
    findings += check_table_is_named(text)
    findings += check_no_second_verdict(text)
    findings += check_shared_table_is_profile_independent(text)
    findings += check_reported_count(text)

    print(f"generator size: {len(text.splitlines())} lines")
    print(f"shared table:   {WIRE_RULES}")
    print()

    if findings:
        print(f"❌ the generator does not reuse the shared wire rules ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1

    print("✅ the generator consumes the runtime's wire-rule table and states no verdict of its own")
    return 0


if __name__ == "__main__":
    sys.exit(main())
