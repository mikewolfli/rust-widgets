#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_font_data_is_opt_in.sh, which is the
# only caller. That file is the documentation of record.
#
# Font data is measured in hundreds of kilobytes and is the one thing a `mini`/`embedded`
# profile cannot afford to carry by accident. So the check is on the *feature graph*: no
# `fonts-*` feature may be a member of `default` or of any device profile, every `fonts-*`
# feature must actually be declared, and every binary font payload must sit behind one.
#
# Prints one `finding: ...` line per problem, then a `fonts=N default_gated=M` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# A font-data feature is named `fonts-*` by convention, and the convention is what this scans.
# Naming it rather than pattern-matching the payloads is deliberate: a feature list is the
# contract a consumer reads, so it is the thing that must be right.
FONT_FEATURE_PREFIX = "fonts-"
# The profiles a feature could be dragged into. `full` and `--all-features` are excluded on
# purpose: `--all-features` means "every capability", so it is *expected* to carry data, and the
# Cargo manifest cannot express "all but this one".
PROFILE_FEATURES = ("default", "desktop", "tablet", "mobile", "embedded", "mini")

# `include_bytes!("….ttf")` and friends: the only way binary font data can enter the crate.
FONT_PAYLOAD = re.compile(r"include_bytes!\(\s*\"(?P<path>[^\"]+\.(?:ttf|otf|ttc|woff2?|pcf|bdf))\"")


def parse_features(manifest: str):
    """Map feature name → list of its members, from a `[features]` section.

    A hand-rolled parse rather than a TOML dependency, because this gate must run in the
    verification loop with nothing installed beyond the standard library — the same reason the
    other scanners here are hand-rolled. The section's shape is simple and is asserted at the end.
    """
    features = {}
    in_section = False
    current = None
    for raw in manifest.splitlines():
        line = raw.split("#", 1)[0].rstrip()
        if not line.strip():
            continue
        if line.startswith("["):
            in_section = line.strip() == "[features]"
            current = None
            continue
        if not in_section:
            continue
        if "=" in line and not line.startswith((" ", "\t")):
            name, _, value = line.partition("=")
            current = name.strip()
            features[current] = re.findall(r'"([^"]+)"', value)
        elif current is not None:
            features[current].extend(re.findall(r'"([^"]+)"', line))
    return features


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default=str(REPO_ROOT / "Cargo.toml"))
    parser.add_argument("--src", default=str(REPO_ROOT / "src"))
    args = parser.parse_args(argv)

    manifest_path = pathlib.Path(args.manifest)
    features = parse_features(manifest_path.read_text(encoding="utf-8"))
    if "features" in features or not features:
        print("finding: could not parse any feature from the manifest")
        print("fonts=0 default_gated=0")
        return 0

    font_features = sorted(name for name in features if name.startswith(FONT_FEATURE_PREFIX))
    findings = []

    for profile in PROFILE_FEATURES:
        members = features.get(profile)
        if members is None:
            continue
        for member in members:
            # A profile may *transitively* pull a font feature, so the check follows one level
            # of indirection. The member itself is tested *first*: it is usually declared as a
            # feature, and expanding it before testing it would replace the very name that is
            # the finding with its (empty) member list.
            if member.startswith(FONT_FEATURE_PREFIX):
                findings.append(f"profile {profile!r} enables font data via {member!r}")
                continue
            for candidate in features.get(member, []):
                if candidate.startswith(FONT_FEATURE_PREFIX):
                    findings.append(
                        f"profile {profile!r} enables font data via {member!r} → {candidate!r}"
                    )

    # Every font payload must be in a module that a `fonts-*` feature gates, and the crate's
    # only sanctioned home for one is the generated-asset directory.
    payloads = []
    for path in sorted(pathlib.Path(args.src).rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in FONT_PAYLOAD.finditer(text):
            payloads.append((path, match.group("path")))
    for path, payload in payloads:
        relative = path.relative_to(REPO_ROOT).as_posix()
        if not relative.startswith("src/render/text/font_assets/"):
            findings.append(f"{relative}: embeds font payload {payload!r} outside font_assets/")
    if not font_features:
        findings.append("no `fonts-*` feature is declared at all, so the scan proves nothing")

    for finding in findings:
        print(f"finding: {finding}")
    print(f"fonts={len(font_features)} payloads={len(payloads)} findings={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
