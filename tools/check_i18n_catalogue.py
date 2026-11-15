#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Catalogue reconciliation gate for `language/*.json` (W7-04).

# The defect this stops

The catalogue carried a 175-entry `widget.<name>.label` section that nothing in
`src/`, `crates/`, `demo/` or `examples/` read, and 61 of those names were not
registered controls at all (`checkbox`, `combobox`, `lineedit`, …). Both halves
were invisible: the three language files agreed with each other, so no diff ever
showed the rot, and no gate compared the key names with the registry.

# What it asserts

  1. every catalogue declares the same key set — a key added to one language and
     forgotten in the others is the drift a translator cannot see;
  2. every message is a non-empty string, and `{placeholder}` sets agree across
     languages (a translated sentence that drops a placeholder renders as text
     with the value silently missing);
  3. every `widget.<name>.label` names a **canonical** control (the names
     `WidgetFactory` registers, read through `generate_control_index.py`); a
     legacy spelling fails with the canonical spelling as the suggestion.

Missing labels are printed but are not a failure by default: a control without a
label is announced by `Widget::accessible_name`'s kind-name fallback, so absence
is a degradation rather than a lie. `--strict` turns the list into a failure and
is what a translation batch should run.

# Why the canonical name list is imported, not copied

`generate_control_index.py` already derives canonical names from the capability
tables. A second list here would drift from the registry in exactly the way this
gate exists to catch (principle #101).

Usage: python3 tools/check_i18n_catalogue.py [--strict]
Exit 0 = catalogues agree and every label names a real control.
Exit 1 = a finding.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import re
import sys

LANGUAGE_DIR = pathlib.Path("language")
EXPECTED_LANGUAGES = ("en", "zh-cn", "zh-tw")
PLACEHOLDER_RE = re.compile(r"\{[A-Za-z0-9_]+\}")


def load_catalogues() -> dict[str, dict[str, str]]:
    catalogues: dict[str, dict[str, str]] = {}
    for language in EXPECTED_LANGUAGES:
        path = LANGUAGE_DIR / f"{language}.json"
        if not path.is_file():
            raise SystemExit(f"❌ {path} is missing")
        document = json.loads(path.read_text(encoding="utf-8"))
        translations = document.get("translations")
        if not isinstance(translations, dict):
            raise SystemExit(f"❌ {path} has no `translations` object")
        messages: dict[str, str] = {}
        for key, entry in translations.items():
            if not isinstance(entry, dict) or not isinstance(entry.get("message"), str):
                raise SystemExit(f"❌ {path}: `{key}` has no string `message`")
            message = entry["message"]
            if not message.strip():
                raise SystemExit(f"❌ {path}: `{key}` has an empty message")
            messages[key] = message
        catalogues[language] = messages
    return catalogues


def registry_names() -> tuple[set[str], dict[str, str]]:
    """Returns (canonical names, normalised spelling → canonical name)."""
    path = pathlib.Path(__file__).with_name("generate_control_index.py")
    spec = importlib.util.spec_from_file_location("generate_control_index", path)
    if spec is None or spec.loader is None:  # pragma: no cover - broken checkout
        raise SystemExit(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    aliases = module.alias_names()
    canonical = {name for name, target in aliases.items() if name == target}
    normalised = {module.normalise(name): name for name in canonical}
    return canonical, normalised


def label_name(key: str) -> str | None:
    if key.startswith("widget.") and key.endswith(".label"):
        return key[len("widget.") : -len(".label")]
    return None


def main() -> int:
    strict = "--strict" in sys.argv[1:]
    catalogues = load_catalogues()
    reference = catalogues["en"]
    problems: list[str] = []

    # 1. The three catalogues must carry the same keys.
    reference_keys = set(reference)
    for language, messages in catalogues.items():
        keys = set(messages)
        only_here = sorted(keys - reference_keys)
        missing = sorted(reference_keys - keys)
        if only_here:
            problems.append(f"{language}: {len(only_here)} key(s) not in en.json: {only_here[:5]}")
        if missing:
            problems.append(f"{language}: {len(missing)} key(s) missing vs en.json: {missing[:5]}")

    # 2. Placeholders must survive translation.
    for key in sorted(reference_keys):
        expected = set(PLACEHOLDER_RE.findall(reference[key]))
        for language, messages in catalogues.items():
            if key not in messages:
                continue
            found = set(PLACEHOLDER_RE.findall(messages[key]))
            if found != expected:
                problems.append(
                    f"{language}: `{key}` placeholders {sorted(found)} != en {sorted(expected)}"
                )

    # 3. Every control label must name a canonical control.
    canonical, normalised = registry_names()
    labelled: set[str] = set()
    for key in sorted(reference_keys):
        name = label_name(key)
        if name is None:
            continue
        labelled.add(name)
        if name in canonical:
            continue
        suggestion = normalised.get(name.replace("_", "").replace("-", "").lower())
        hint = f" (did you mean `{suggestion}`?)" if suggestion else ""
        problems.append(f"`{key}` names no registered control{hint}")

    missing_labels = sorted(canonical - labelled)
    print(
        f"catalogue: {len(reference_keys)} keys × {len(catalogues)} languages; "
        f"{len(labelled & canonical)}/{len(canonical)} controls have a label"
    )
    if missing_labels:
        print(
            f"  note: {len(missing_labels)} canonical control(s) have no `widget.*.label`; "
            "absence falls back to the kind name in `accessible_name`"
        )
        print("        " + ", ".join(missing_labels[:20]) + (" …" if len(missing_labels) > 20 else ""))
        if strict:
            problems.append(f"{len(missing_labels)} canonical control(s) have no label (--strict)")

    if problems:
        print("i18n catalogue findings (W7-04):", file=sys.stderr)
        for problem in problems:
            print(f"  ❌ {problem}", file=sys.stderr)
        return 1

    print("✅ i18n catalogue: languages agree and every control label is canonical")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
