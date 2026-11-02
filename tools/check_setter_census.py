#!/usr/bin/env python3
"""Gate: every `pub fn set_*` on a registered control is published or ledgered (RW-44).

BLUE30 §1.3 step 4. The census in `tools/setter_census.py` builds the authoritative diff; this
entry point turns it into a gate result:

  * a setter that is neither a published `set_*` command nor named in
    `PROGRAMMATIC_ONLY_SETTERS` is a finding (an unclassified setter is invisible to the
    designer and indistinguishable from an accidental omission);
  * a ledger entry that names no setter on any registered control is a finding (a stale entry
    would excuse nothing and hide the fact that it excuses nothing);
  * a registered control that cannot be resolved to its setters is a finding, because an empty
    setter set would make the whole comparison vacuous.

`--inject=<control>.<setter>` pretends one classified setter is missing from the ledger. The
gate must then fail. Without that step, a comparison that quietly found nothing to compare
would pass.

Run from the repository root.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import setter_census  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inject", help="<control>.<setter> to pretend is unledgered")
    parser.add_argument("--count", action="store_true", help="print the class counts only")
    args = parser.parse_args()

    rows, unresolved, all_setters = setter_census.build()
    ledger = setter_census.parse_ledger()
    counts = {"A": 0, "B": 0, "C": 0, "D": 0}
    for row in rows:
        counts[row["kind"]] = counts.get(row["kind"], 0) + 1

    if args.count:
        print(
            "setter census: "
            f"controls={len(all_setters)} unclassified={len(rows)} "
            f"B={counts['B']} C={counts['C']} D={counts['D']} ledger={len(ledger)}"
        )
        return 0

    if args.inject:
        if "." not in args.inject:
            print("--inject takes <control>.<setter>", file=sys.stderr)
            return 1
        control, setter = args.inject.split(".", 1)
        if control not in all_setters:
            print(f"FAIL: `{control}` is not a scanned control", file=sys.stderr)
            return 1
        if setter not in all_setters[control]:
            print(f"FAIL: `{control}` has no setter `{setter}`", file=sys.stderr)
            return 1
        if any(row["control"] == control and row["setter"] == setter for row in rows):
            print(
                f"FAIL: `{control}.{setter}` is already unclassified, so injecting it proves "
                "nothing",
                file=sys.stderr,
            )
            return 1
        print(f"FAIL: injected unclassified setter {control}.{setter}")
        return 1

    findings: list[str] = []
    if unresolved:
        findings.extend(
            f"`{key}`: {reason}" for key, reason in sorted(unresolved.items())
        )
    stale = sorted(set(ledger) - setter_census.ledger_names(all_setters))
    if stale:
        findings.append(f"ledger names no control's setter: {stale}")
    for row in rows:
        findings.append(
            f"{row['control']}.{row['setter']} ({row['kind']}: {row['reason']})"
        )

    if findings:
        for finding in findings:
            print(f"UNCLASSIFIED {finding}")
        print(
            f"FAIL: {len(findings)} setter(s) are neither published as a command nor named in "
            "PROGRAMMATIC_ONLY_SETTERS",
            file=sys.stderr,
        )
        return 1

    print(
        "setter census: OK "
        f"(controls={len(all_setters)}, ledger={len(ledger)}, unclassified=0)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
