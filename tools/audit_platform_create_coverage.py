#!/usr/bin/env python3
"""Resolve every platform `create_*` to the capability that answers for it.

# Why this exists

`src/platform/types.rs` declares a `create_*` per control shape the platform trait knows.
The capability registry is smaller, because several platform entries name a *type alias*
(`ContextMenu` is `Menu`, `DoubleSpinBox` is `SpinBox`) rather than a distinct control.

A platform `create_*` with **no** capability behind it is the failure this reports: the
name is spellable and the trait method exists, but nothing can be constructed, so the call
returns zero and the control is silently absent from the designer's list.

The alias tables live in `src/widget/capability.rs`; this script reads them rather than
re-stating them, so a new alias is picked up without editing it.
"""

import re
import sys

CAPABILITY = "src/widget/capability.rs"
PROPERTIES = "src/widget/capability/properties.rs"
PLATFORM = "src/platform/types.rs"


def main() -> int:
    capability = open(CAPABILITY, encoding="utf-8").read()
    properties = open(PROPERTIES, encoding="utf-8").read()
    platform = open(PLATFORM, encoding="utf-8").read()

    canonical = set(re.findall(r'canonical_name:\s*"([^"]+)"', properties))

    # `aliases: &["pushbutton", "btn"]` — extra names each capability accepts.
    alias_names = set()
    for group in re.findall(r"aliases:\s*&\[([^\]]*)\]", properties):
        alias_names |= set(re.findall(r'"([^"]+)"', group))

    # `WidgetKind::ContextMenu => "menu"` — a variant that is a type alias.
    kind_alias = dict(re.findall(r'WidgetKind::(\w+) => "([^"]+)"', capability))
    # `"context_menu" => "menu"` — the name-level alias table.
    name_alias = dict(re.findall(r'"([a-z_][a-z0-9_]*)"\s*=>\s*"([a-z_][a-z0-9_]*)"', capability))

    # A control factory accepts a rectangle and returns an `ObjectId`. A platform method
    # named `create_*` that does not is not a control factory at all —
    # `create_web_engine(&self) -> Option<Box<dyn NativeWebEngine>>` hands back a native
    # engine handle for `src/web/` to drive, and reporting it as "a control with no
    # capability" is a false positive that sends a reader looking for a missing widget
    # that was never meant to exist.
    #
    # `width`/`height` rather than `parent` is what separates the two cases, because a
    # window is a root and therefore has no parent to take:
    # `create_window(title, x, y, width, height) -> ObjectId` is a factory like the rest.
    factory_signature = re.compile(
        r"fn create_([a-z_][a-z0-9_]*)\s*\(\s*&self,\s*"
        r"[^)]*?\bwidth\b[^)]*?\bheight\b[^)]*?\)\s*->\s*ObjectId"
    )
    creates = sorted(set(factory_signature.findall(platform)))

    non_factories = sorted(
        set(re.findall(r"fn create_([a-z_][a-z0-9_]*)\s*\(", platform)) - set(creates)
    )

    print(f"platform control factories: {len(creates)}")
    print(f"non-factory create_* skipped: {len(non_factories)} {non_factories}")
    print(f"capability records:        {len(canonical)}")
    print(f"alias names:               {len(alias_names)}")
    print()
    print(f"{'platform create_*':28} status")
    print("-" * 78)

    unresolved = []
    for name in creates:
        if name in canonical:
            status = "capability"
        elif name in name_alias:
            status = f"name alias -> {name_alias[name]}"
        elif name in alias_names:
            status = "in a capability's aliases"
        else:
            camel = "".join(word.title() for word in name.split("_"))
            if camel in kind_alias:
                status = f"kind alias {camel} -> {kind_alias[camel]}"
            else:
                status = "UNRESOLVED"
                unresolved.append(name)
        print(f"{name:28} {status}")

    print()
    if unresolved:
        print(f"UNRESOLVED ({len(unresolved)}): {unresolved}", file=sys.stderr)
        return 1
    print("every platform create_* resolves to a capability or a declared alias")
    return 0


if __name__ == "__main__":
    sys.exit(main())
