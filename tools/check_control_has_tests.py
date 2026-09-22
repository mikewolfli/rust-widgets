"""Every control must have at least one test that names it.

# Why this gate exists

`Toast` shipped with zero tests of its own. It was registered in the factory, had a
property contract, was reachable from JSON, CSS and the C ABI, and appeared in the
capability matrix — every gate passed. None of them asked the one question that
matters for a control: *is there a test that exercises this control*.

The existing gates cover the opposite direction. `check_behavior_matrix.sh` runs
selected cross-configuration contracts; `check_capability_matrix_truthfulness.sh`
checks that declared properties are answered; `check_widget_registration_fidelity.sh`
checks that every kind resolves. A control with a contract and no test satisfies all
three, because "the contract answers" is provable by reading the contract.

# What counts as a test for a control

A `#[test]` function whose body mentions the control's type name. That is a
deliberately weak-looking bar, and it is the right one: the gate is here to catch a
control that nobody ever exercised, not to judge how well someone tested one. A test
that merely constructs the type still fails if the type stops compiling, which is the
class of failure this gate is a floor for.

Run from the repo root.
"""
from __future__ import annotations

import pathlib
import re
import sys

WIDGET_DIR = pathlib.Path("src/widget")
KIND_FILE = WIDGET_DIR / "kind.rs"

# Controls whose behaviour legitimately lives outside a unit test. Each entry needs a
# reason, because the point of the list is to be argued with.
ALLOWLIST: dict[str, str] = {}


def widget_kinds() -> list[str]:
    """Every `WidgetKind` variant name."""
    text = KIND_FILE.read_text(encoding="utf-8")
    return sorted(set(re.findall(r"^\s{4}([A-Z][A-Za-z0-9]*),", text, re.M)))


def pascal_to_snake(name: str) -> str:
    first = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", first).lower()


def test_bodies() -> dict[str, str]:
    """Every `#[cfg(test)]` module in the crate, as `path -> module text`.

    # Why the whole module rather than each `#[test]` function

    A test rarely builds its subject inline. `splash_screen.rs` has a `fn screen(..)`
    helper that calls `SplashScreen::new`, and its ten tests call that helper — so
    reading only `#[test]` bodies reported `SplashScreen` as untested while it had ten
    tests. A control built through a helper is exercised exactly as much as one built
    inline.

    The module boundary is the right unit because it is the boundary the language
    already draws: anything reachable from a test in that module can be built by it,
    and nothing outside a `#[cfg(test)]` module runs under `cargo test` by accident.

    # Modules excluded from the search

    `routing.rs`'s test module lists every `WidgetKind` to assert the routing table is
    total, and `examples/widget_reachability.rs` does the same for reachability. Both
    mention every control by construction, so counting them would make this gate answer
    "every control is tested" regardless of the truth — which is precisely the vacuity
    the gate exists to avoid. They verify the *tables*, not any control's behaviour.
    """
    EXCLUDED = {
        "src/control_backend/routing.rs":
            "lists every WidgetKind to assert the routing table is total",
        "src/control_backend/custom/mod.rs":
            "enumerates kinds through the factory rather than exercising one control",
    }

    bodies: dict[str, str] = {}
    for path in pathlib.Path("src").rglob("*.rs"):
        if str(path) in EXCLUDED:
            continue
        text = path.read_text(encoding="utf-8")
        # A test module may carry extra profile gates alongside `test`, because some
        # tests need a module that only exists in a build with a device profile —
        # `crate::theme` and `crate::widget::census` are both `cfg(device_profile)` /
        # `cfg(full_widgets)`, so a test that drives the theme cannot compile under
        # `mini`. Matching the literal `#[cfg(test)]` would read such a module as
        # absent and report a tested control as untested, which is the vacuous answer
        # in the other direction. So the attribute is matched together with any extra
        # conjuncts, and only `test` is required to be among them.
        for match in re.finditer(r"#\[cfg\(([^\]]*\btest\b[^\]]*)\)\]", text):
            # Take from the attribute to the end of the file: a test module is the last
            # thing in every file in this crate, and taking a fixed window would cut
            # long modules short while a brace count would need to know whether the
            # attribute lands on `mod tests {` or on a bare `mod tests;`.
            bodies[str(path)] = text[match.start() :]
            break

    # `#[cfg(test)] mod tests;` puts the module in a sibling file, so its tests are not
    # in the file that declares them. Following that one hop is what lets a split
    # control (`toast/tests.rs`, `freeform_shape/tests.rs`) count as tested — without
    # it the gate would push controls back towards one file each.
    for path in list(pathlib.Path("src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for declared in re.findall(
            r"#\[cfg\([^\]]*\btest\b[^\]]*\)\]\s*mod\s+(\w+)\s*;", text
        ):
            sibling = path.parent / f"{declared}.rs"
            if sibling.exists():
                bodies[str(path)] = bodies.get(str(path), "") + sibling.read_text(
                    encoding="utf-8"
                )
    return bodies


def control_types() -> dict[str, str]:
    """Map a control's public type name to the file that declares it.

    A control is a `pub struct` with its *own* `impl Widget`. The `for` clause must be
    followed by ` {` (or a newline), because a plain substring search for
    `impl Widget for Tab` also matches `impl Widget for TabWidget` — which made the
    data item `Tab` look like a control and reported a control with no test that was
    never a control to begin with.
    """
    controls: dict[str, str] = {}
    for path in WIDGET_DIR.rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        for name in re.findall(r"^pub struct (\w+)", text, re.M):
            if re.search(rf"^impl Widget for {re.escape(name)}\s*\{{", text, re.M):
                controls[name] = str(path)
    return controls


def construction_pattern(name: str) -> re.Pattern[str]:
    """Matches a test that *builds* the control.

    # Why naming the type is not enough

    The first version of this gate accepted any test whose body mentioned the type
    name. That made it vacuous: `routing.rs::all_widget_kinds_are_routed` lists every
    `WidgetKind`, including `WidgetKind::Toast`, so every control counted as covered
    by a test that only enumerates kinds. The gate reported 179/179 while `Toast` had
    no test at all, and deleting those tests did not change the answer.

    # Why `WidgetKind::X` is also excluded

    An enum-listing test (`all_widget_kinds_are_routed`, the reachability example)
    mentions every kind by construction, so accepting `WidgetKind::X` reproduces the
    same vacuity one step later. Those tests are valuable, but they verify the *table*
    rather than any control's behaviour.

    Construction is the discriminating bar: a control that is never built is never
    exercised, and building one is what forces its fields, its constructor signature
    and its base wiring to be correct. Two patterns remain, covering how a test in
    this crate builds a control — `Type::new(..)`, and the factory by canonical name
    (`create("toast", ..)`), which is how a factory-level test reaches one.
    """
    snake = pascal_to_snake(name)
    alternatives = [
        # `Type::new(..)` and the other constructors this crate uses (`with_dimensions`,
        # `new_with_defaults`). Anchoring on `::new` alone reported `GridWidget` as
        # untested while it had tests, because they build it through
        # `GridWidget::with_dimensions`.
        rf"\b{re.escape(name)}\s*::\s*(?:new|with_\w+)\b",
        rf'"{re.escape(snake)}"',
    ]
    return re.compile("|".join(alternatives))


def main() -> int:
    bodies = test_bodies()
    controls = control_types()

    # The kinds whose name is not spelled as the Rust type (`CheckListBox` is a
    # `pub type` alias for `ListBox`, `FAB` implements as `FAB`, ...). The check is
    # on the *type* name, because that is what a test would mention.
    kinds = {pascal_to_snake(k) for k in widget_kinds()}

    untested: list[tuple[str, str]] = []
    tested = 0
    skipped = 0
    for name, path in sorted(controls.items()):
        if name in ALLOWLIST:
            skipped += 1
            continue
        pattern = construction_pattern(name)
        found = any(pattern.search(body) for body in bodies.values())
        if found:
            tested += 1
        else:
            untested.append((name, path))

    print(f"controls with a test that names them: {tested} / {len(controls)}")
    if skipped:
        print(f"allowlisted (with a stated reason): {skipped}")
        for name, reason in sorted(ALLOWLIST.items()):
            print(f"  - {name}: {reason}")

    if untested:
        print()
        print("controls with no test mentioning them:", file=sys.stderr)
        for name, path in untested:
            print(f"  {name}  ({path})", file=sys.stderr)
        print(
            "\nA control must have at least one test that exercises it. The "
            "registration, capability and matrix gates all pass for a control whose "
            "contract is correct but which nothing ever mounts, which is how `Toast` "
            "shipped untested.",
            file=sys.stderr,
        )
        return 1

    # The check must not be vacuous: an empty control set would pass while covering
    # nothing, which is the failure mode the repo's rules call out.
    if not controls:
        print("FAIL no controls were discovered; the gate would pass vacuously", file=sys.stderr)
        return 1
    if not kinds:
        print("FAIL no WidgetKind variants were parsed", file=sys.stderr)
        return 1

    print("✅ every control has at least one test that mentions it")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
