#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_text_model_is_single_sourced.sh,
# which is the only caller. That file is the documentation of record.
#
# Four decisions about text — how it clusters, what one cluster advances, which scalars are wide,
# and where the glyph geometry comes from — must each have exactly one definition, because each
# consumer that keeps its own copy drifts silently. This scan counts definitions.
#
# Prints one `finding: ...` line per problem, then a `singletons=N failed=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# Each entry: a decision, the single function that owns it, and the file it lives in. The owner
# file is named so that moving the definition is a deliberate edit here rather than a silent
# relocation the scan would follow.
SINGLETONS = {
    "fn is_wide_scalar": "the wide-scalar table",
    "fn estimate_cluster_advance": "the advance model",
    "fn for_each_cluster": "the grapheme clustering rule",
}

# Clustering is the step a second implementation would most plausibly appear in, because it looks
# like a two-line loop. Any *construction* of the cluster type outside the text layer is that —
# the type's own definition is not a construction, so it is excluded by name.
CLUSTER_CONSTRUCTION = re.compile(r"\bTextCluster\s*\{")
CLUSTER_DEFINITION = re.compile(r"\bstruct\s+TextCluster\b")

# The two renderers must read the text layer's derivation rather than build clusters themselves.
SHAPING_CONSUMERS = ("src/render/pipeline/containers.rs", "src/render/svg/backend.rs")
SHAPING_ENTRY = "shape_line("


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src", default=str(REPO_ROOT / "src"))
    parser.add_argument("--inject", action="append", default=[],
                        help="an extra file to scan, to prove the scan looks (repeatable)")
    args = parser.parse_args(argv)

    files = sorted(pathlib.Path(args.src).rglob("*.rs"))
    files.extend(pathlib.Path(p) for p in args.inject if pathlib.Path(p).exists())

    findings = []
    counts = {name: [] for name in SINGLETONS}
    cluster_sites = []

    for path in files:
        text = path.read_text(encoding="utf-8", errors="replace")
        try:
            relative = path.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            relative = path.as_posix()
        for number, line in enumerate(text.splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith(("//", "*", "/*")):
                continue
            for name in SINGLETONS:
                if name in line:
                    counts[name].append(f"{relative}:{number}")
            if (
                CLUSTER_CONSTRUCTION.search(line)
                and not CLUSTER_DEFINITION.search(line)
                and not relative.startswith("src/render/text/")
            ):
                cluster_sites.append(f"{relative}:{number}")

    for name, where in counts.items():
        if len(where) != 1:
            findings.append(
                f"{SINGLETONS[name]} is defined {len(where)} times "
                f"({', '.join(where) or 'nowhere'}); exactly one definition is required"
            )
    for site in cluster_sites:
        findings.append(f"{site}: builds a text cluster outside the text layer")

    for consumer in SHAPING_CONSUMERS:
        path = REPO_ROOT / consumer
        if not path.exists() or SHAPING_ENTRY not in path.read_text(encoding="utf-8"):
            findings.append(f"{consumer}: does not call the text layer's `{SHAPING_ENTRY[:-1]}`")

    for finding in findings:
        print(f"finding: {finding}")
    print(f"singletons={len(counts)} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
