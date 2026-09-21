#!/usr/bin/env python3
"""Resolve every platform `create_*` to the capability that answers for it.

# Why this exists

`src/platform/types.rs` declares a `create_*` per control shape the platform trait knows.
The capability registry is smaller, because several platform entries name a *type alias*
(`ContextMenu` is `Menu`, `DoubleSpinBox` is `SpinBox`) rather than a distinct control.

A platform `create_*` with **no** capability behind it is the failure this reports: the
name is spellable and the trait method exists, but nothing can be constructed, so the call
returns zero and the control is silently absent from the designer's list.

# Why the comparison is by *normalised* name, not by literal spelling

The first version of this script compared the platform name to the capability names with
`==`. That made `create_checkbox` report as `UNRESOLVED (1)` — a **false positive**, and
BLUE20 §1.7 recorded it as the layer's one real gap. It is not a gap: `WidgetFactory`
stores every name under `normalize_key`, which strips `_`, `-` and spaces and lowercases,
so `"checkbox"` and `"check_box"` are the *same key* and both already construct the
control. Measured:

    factory.create("checkbox", ..).is_some() == true   -> canonical_name == "check_box"
    factory.create("checkBox", ..).is_some() == true

The mistake was in the criterion, not in the code, so the fix is here (principle #110): a
literal comparison under-reports reachability, and "fixing" it by adding an alias would
have been a no-op that also violates `capability_alias_hygiene_test`. The alias tables live
in `src/widget/capability.rs` and the normalisation rule in
`src/widget/capability/coercion.rs::normalize_key`; this script reads them rather than
re-stating them.
"""

import re
import sys

CAPABILITY = "src/widget/capability.rs"
PROPERTIES = "src/widget/capability/properties.rs"
PLATFORM = "src/platform/types.rs"


def normalize_key(value: str) -> str:
    """The factory's own name normalisation, mirrored from `coercion.rs::normalize_key`.

    Duplicated rather than imported because this is a source-text scanner with no Rust
toolchain in the loop, and because the *rule* (strip `_`, `-`, space; lowercase) is what
    this script depends on. A divergence would show up as a spurious `UNRESOLVED`, which is
    the finding this script exists to produce, so it fails loudly rather than silently.
    """
    return "".join(ch for ch in value.lower() if ch not in "_- ")


def main() -> int:
    capability = open(CAPABILITY, encoding="utf-8").read()
    properties = open(PROPERTIES, encoding="utf-8").read()
    platform = open(PLATFORM, encoding="utf-8").read()

    canonical = set(re.findall(r'canonical_name:\s*"([^"]+)"', properties))

    # `aliases: &["pushbutton", "btn"]` — extra names each capability accepts.
    alias_names = set()
    for group in re.findall(r"aliases:\s*&\[([^\]]*)\]", properties):
        alias_names |= set(re.findall(r'"([^"]+)"', group))

    # Keyed by the *normalised* name, which is what the factory actually looks up.
    reachable: dict[str, str] = {normalize_key(name): name for name in canonical}
    for name in alias_names:
        reachable.setdefault(normalize_key(name), name)

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
        key = normalize_key(name)
        if key in reachable:
            # Name the *canonical* spelling when the two differ, so a reader can see that
            # `checkbox` resolved to `check_box` rather than being silently accepted.
            canonical_hit = reachable[key]
            status = (
                "capability"
                if canonical_hit == name
                else f"normalised name -> {canonical_hit}"
            )
        elif name in name_alias:
            status = f"name alias -> {name_alias[name]}"
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
