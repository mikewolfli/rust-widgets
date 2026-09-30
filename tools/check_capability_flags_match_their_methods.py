#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Every `PlatformCapabilities` flag a backend declares must be backed by the method it promises.

# Why this gate exists

`PlatformCapabilities` is not documentation — it is a **promise that escapes the process**.
`rw_platform_capabilities()` (`src/bindings/binding_impl.rs`) packs the five flags into a C ABI
bitmask, and a host application reads that mask to decide whether to offer an input method, a
screen reader, or a native menu. So a flag set to `true` without a method behind it is not a
cosmetic inaccuracy: it is a host-visible false claim.

The fields map one-to-one onto trait methods:

| flag              | promised method                        |
|-------------------|----------------------------------------|
| `dpi_scaling`     | `Platform::dpi_scale_factor`           |
| `ime`             | `Platform::ime_bridge`                 |
| `accessibility`   | `Platform::accessibility_bridge`       |
| `native_menu`     | `Platform::create_menu_bar` and friends|
| `typed_widget_trigger` | `Platform::poll_widget_trigger_event` (library-implemented) |

# The defect class this catches

Two rounds of review found the same defect six times, each time by hand and each time by
accident: a backend's `capabilities()` claimed a host integration while the backend overrode
none of the methods behind it. The trait default is an honest all-`false`, so the *fix* is a
deletion — which means the defect has no failing test, no compile error, and no runtime symptom
until a host acts on the bit.

Concretely, the history this gate pins down:

* `macos_objc2` claimed `ime`, `accessibility`, `native_menu` and `dpi_scaling` with none of the
  four methods overridden — and its own test asserted `create_menu_bar(..) == 0`.
* `windows` and `macos` claimed `native_menu` with no `create_menu_bar` anywhere in the backend.
* `wasm` claimed `supports_surfaces()` with no `mount_surface`.
* `ios` claimed three flags whose methods do not exist.
* `wayland` claimed `ime` and `accessibility` while overriding neither bridge — the last one to
  fall, and the reason a gate was written rather than another fix.

The common shape is "a string/computed answer with nothing behind it", which is principle #37's
"never hardcode a pretend value". A reviewer catches this only by cross-checking two lists by
hand; the gate does it for every backend on every run.

# What is checked, and how

For each backend that defines `capabilities()`, the flag's literal is read **from the source
text** and compared against whether the same backend defines the promised method. The method
search is deliberately textual and per-file, because that is exactly the granularity at which
the defect occurs: the question is "does *this* backend implement it", not "does some type in
the crate".

`dpi_scaling` is the one flag whose value is legitimately computed (Linux reports
`self.dpi_scale_factor() != 1.0 || cfg!(gtk-native)`), so it is checked for the **presence of an
override** rather than for the literal `true`.

# What this deliberately does not attempt
#
# * **It does not check the reverse direction.** A backend that implements a method but reports
#   `false` is *under*-claiming, which the trait's own documentation calls the direction a default
#   must err. Under-claiming is a missing feature, not a false claim, so it is not this gate's
#   business (and flagging it would make the honest-default idiom unrepresentable).
# * **It does not require a *computed* `dpi_scaling`.** The first draft of this gate reported a
#   literal `dpi_scaling: true` as a fabricated value, on the history that the trait default is the
#   constant `1.0`. That was wrong: `windows` answers `GetDeviceCaps(LOGPIXELSX) / 96.0` and
#   `wayland` reads `GDK_SCALE` / `QT_SCALE_FACTOR`, so a literal `true` next to either is a true
#   claim about a real measurement. The rule would have demanded that correct code be rewritten to
#   match the gate — the failure mode where a gate reports the *spelling* of the fix instead of the
#   fix. What makes `dpi_scaling` honest is the `dpi_scale_factor` override, which is what is
#   checked.
# * **It does not verify the method's body is real.** `fn ime_bridge(&self) -> Option<..>` that
#   returns `None` unconditionally would pass. That is a deeper question with its own answer —
#   `check_capability_matrix_truthfulness.py` grades `create_*` bodies for the widget path — and
#   a lexical gate claiming to answer it would be asserting a spelling rather than the property.

Usage: python3 tools/check_capability_flags_match_their_methods.py
"""

from __future__ import annotations

import pathlib
import re
import sys

# The `supports_*` claims. Same shape as the flags above — a boolean that promises a method —
# and the same defect happened here once already: `wasm` reported `supports_surfaces() == true`
# with no `mount_surface`, so a host built a widget tree and the first mount came back
# `SurfaceMountError::RejectedByBackend`. The trait documents the flag as reportable "only once
# `mount_surface` is actually implemented", which is a promise nothing was checking.
SUPPORTS_CLAIMS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("supports_surfaces", ("mount_surface",)),
    # A backend that advertises a web engine must be able to construct one. The engine itself is
    # feature-gated, so the method may legitimately be absent from a build that has no engine —
    # which is why `check_supports_claims` treats a *missing* method as a finding only when the
    # flag is a non-`false` literal in the same file. See the module doc.
    ("supports_web_engine", ("web_engine",)),
)

# The flag -> (promised method(s), one of which must be defined by the backend).
#
# A tuple because `native_menu` is served by any of several creation methods: a backend that
# implements `create_menu_bar` may reasonably not implement `create_menu`, and requiring a
# specific one would report a working backend as broken.
PROMISED: dict[str, tuple[str, ...]] = {
    "ime": ("ime_bridge",),
    "accessibility": ("accessibility_bridge",),
    "native_menu": (
        "create_menu_bar",
        "create_menu",
        "menu_add_item",
        "attach_menu_bar_to_window",
        "poll_menu_triggered",
    ),
    # Implemented by the library over the shared queue rather than by the host, so a backend is
    # expected to have it and this is a real (if easily satisfied) check.
    "typed_widget_trigger": ("poll_widget_trigger_event", "inject_widget_trigger_event"),
    # Computed or literal; checked for an override, not for a value. See the module doc.
    "dpi_scaling": ("dpi_scale_factor",),
}

# The backends whose `capabilities()` is read, as (name, source path). Explicit rather than a
# glob so the coverage is a statement: a backend added to `src/platform/` without being added
# here is a gap this list makes visible, and `check_every_backend_is_listed` below fails loudly
# when one appears.
BACKENDS: tuple[tuple[str, str], ...] = (
    ("linux", "src/platform/linux/platform_impl.rs"),
    ("wayland", "src/platform/wayland/platform_impl.rs"),
    ("windows", "src/platform/windows/platform_impl.rs"),
    ("macos", "src/platform/macos/platform_impl.rs"),
    ("macos_objc2", "src/platform/macos_objc2/platform_impl.rs"),
    ("android", "src/platform/android/platform_impl.rs"),
    ("ios", "src/platform/ios/platform_impl.rs"),
    ("harmony", "src/platform/harmony/platform_impl.rs"),
    ("wasm", "src/platform/wasm/platform_impl.rs"),
    ("mobile", "src/platform/mobile.rs"),
    # # Why `stub.rs` is a backend and not an exemption
    #
    # It was in `NON_BACKEND_CAPABILITY_FILES` below, described as "the shared state backend the
    # mobile/portable hosts delegate to" — which is what it is, and also exactly why it must be
    # checked. It is the `Platform` implementation for the **portable host**: the selected backend
    # for `mini`, for `embedded` on a host with no backend, for a target with no backend module, and
    # for the two macOS fallbacks. Its claims are read by real hosts.
    #
    # Treating it as not-a-backend is what let it inherit `supports_surfaces() == false` with no
    # surface methods at all, while being the one host whose entire purpose is supplying a surface.
    # The exemption was load-bearing for the bug.
    ("stub", "src/platform/stub.rs"),
)

# Files that define a `capabilities()` but are not backends in the table above.
NON_BACKEND_CAPABILITY_FILES = (
    # The trait's own default: `false` for everything by construction, so there is nothing to
    # promise and nothing to check.
    "src/platform/types.rs",
    # A test-only recorder in the runtime module.
    "src/platform/runtime.rs",
    # The shared state machine the state-backed backends build on; it defines no
    # `capabilities()` of its own.
    "src/platform/state.rs",
)

METHOD_RE = re.compile(r"\bfn\s+([a-z0-9_]+)\s*\(")
# `ime: true,` / `dpi_scaling: false,` inside a `PlatformCapabilities` literal.
FLAG_RE_TEMPLATE = r"\b{flag}\s*:\s*(true|false|[A-Za-z_][A-Za-z0-9_.()!\s|&]*?)\s*,"


def production_part(text: str) -> str:
    """`text` with every `#[cfg(test)]` module removed.

    A backend's own test module legitimately constructs a `PlatformCapabilities` with values that
    are *not* the backend's declarations (a fixture for a consumer, or a comparison against the
    trait default). Reading those as the backend's own answer would produce findings about test
    code, which is the noise this split exists to remove.
    """
    for marker in ("\n#[cfg(test)]\n", "\n    #[cfg(test)]\n"):
        if marker in text:
            return text.split(marker, 1)[0]
    return text


def capabilities_flag_values(text: str) -> dict[str, str]:
    """The raw source spelling of each flag inside this backend's `capabilities()`.

    Returns only the flags actually written there. A flag the backend does not mention is not
    reported, because it inherits the trait default (`false`) — which is the honest answer.
    """
    start = text.find("fn capabilities(")
    if start < 0:
        return {}
    # The body is the next braced block; a lexical scan is enough because the literal is flat.
    open_idx = text.find("{", start)
    if open_idx < 0:
        return {}
    depth = 1
    j = open_idx + 1
    while j < len(text) and depth > 0:
        if text[j] == "{":
            depth += 1
        elif text[j] == "}":
            depth -= 1
        j += 1
    body = text[open_idx:j]

    values: dict[str, str] = {}
    for flag in PROMISED:
        m = re.search(FLAG_RE_TEMPLATE.format(flag=re.escape(flag)), body)
        if m:
            values[flag] = m.group(1).strip()
    return values


def methods_defined(text: str) -> set[str]:
    """Every `fn` name defined in this file."""
    return set(METHOD_RE.findall(text))


def check_backend(name: str, path_str: str) -> list[str]:
    findings: list[str] = []
    path = pathlib.Path(path_str)
    if not path.exists():
        return [f"{name}: {path_str} does not exist; the file moved and this gate is blind"]
    text = production_part(path.read_text(encoding="utf-8"))
    values = capabilities_flag_values(text)
    if not values:
        # A backend with no `capabilities()` inherits the honest all-`false` default, which
        # promises nothing. Nothing to check.
        return findings
    defined = methods_defined(text)

    for flag, spelling in values.items():
        promised = PROMISED[flag]
        # A flag is a claim only when it is not literally `false`. The computed forms
        # (`self.dpi_scale_factor() != 1.0 || ...`) are claims too, so anything that is not
        # exactly `false` counts as one.
        claims = spelling != "false"
        has_method = any(method in defined for method in promised)
        if claims and not has_method:
            findings.append(
                f"{name}: {path_str} declares `{flag}: {spelling}` but defines none of "
                f"{promised} — the trait default would answer for it, so the flag promises a "
                f"method that cannot answer. The flag escapes the process through "
                f"`rw_platform_capabilities()`."
            )
    return findings


def check_supports_claims(name: str, path_str: str) -> list[str]:
    """A `supports_*()` that returns `true` must have the method it advertises.

    # Why this is a separate function from `check_backend`

    The `PlatformCapabilities` flags are read out of one flat struct literal; these are individual
    methods with their own bodies. Both are the same defect, and keeping the two passes separate
    means the `PlatformCapabilities` parse cannot be disturbed by a method body it does not model.

    # What is checked

    A `supports_x()` whose body is a bare `true` must be accompanied by a definition of the method
    `x` promises. A body of `false` promises nothing and is skipped — that is the honest answer, and
    a backend that answers `false` is not making a claim to verify.

    A **computed** body (`self.engine.is_some()`, `cfg!(...)`) is skipped rather than guessed at:
    a lexical gate cannot evaluate it, and reporting a backend for a condition it does compute
    would be asserting the spelling of an answer instead of the answer.
    """
    findings: list[str] = []
    path = pathlib.Path(path_str)
    if not path.exists():
        return findings
    text = production_part(path.read_text(encoding="utf-8"))
    defined = methods_defined(text)

    for claim, promised in SUPPORTS_CLAIMS:
        # Find the method and take its body by brace matching, so a `true` elsewhere in the file
        # cannot be mistaken for this method's answer.
        start = text.find(f"fn {claim}(")
        if start < 0:
            continue
        open_idx = text.find("{", start)
        if open_idx < 0:
            continue
        depth = 1
        j = open_idx + 1
        while j < len(text) and depth > 0:
            if text[j] == "{":
                depth += 1
            elif text[j] == "}":
                depth -= 1
            j += 1
        body = text[open_idx:j]
        # Normalise to the non-comment tokens so a prose `true` in a doc comment cannot match,
        # and so a `#[cfg]`-gated pair of arms is visible as more than a bare literal.
        code = re.sub(r"//[^\n]*", "", body)
        arms = [a for a in re.findall(r"\b(true|false)\b", code)]
        if not arms:
            # A computed body: honest by construction, and not this gate's question.
            continue
        if any(a == "false" for a in arms):
            # Any `false` arm means the backend does not claim it unconditionally. A backend that
            # answers `true` on one host and `false` on another is making a conditional claim the
            # source cannot resolve; skipping is the conservative reading.
            continue
        if not any(method in defined for method in promised):
            findings.append(
                f"{name}: {path_str} answers `{claim}()` with `true` but defines none of "
                f"{promised} — the trait documents the flag as reportable only once the method "
                f"is implemented, and a host that trusts it builds a UI it cannot display"
            )
    return findings


def check_every_backend_is_listed() -> list[str]:
    """A `platform_impl` file that defines `capabilities()` must be in `BACKENDS`.

    Without this, adding a backend silently opts it out of the check — the failure mode a
    hand-maintained list always has.
    """
    findings: list[str] = []
    listed = {pathlib.Path(path).resolve() for _, path in BACKENDS}
    known_exempt = {pathlib.Path(p).resolve() for p in NON_BACKEND_CAPABILITY_FILES}
    for path in sorted(pathlib.Path("src/platform").rglob("*.rs")):
        resolved = path.resolve()
        if resolved in listed or resolved in known_exempt:
            continue
        text = production_part(path.read_text(encoding="utf-8"))
        if "fn capabilities(" in text:
            findings.append(
                f"{path.as_posix()} defines `capabilities()` but is neither in `BACKENDS` nor in "
                f"`NON_BACKEND_CAPABILITY_FILES`. Add it to one, or its flags go unchecked"
            )
    return findings


def inject() -> int:
    """Break one flag and require the gate to notice."""
    name, path_str = "wayland", "src/platform/wayland/platform_impl.rs"
    path = pathlib.Path(path_str)
    original = path.read_text(encoding="utf-8")
    broken = original.replace("            ime: false,", "            ime: true,", 1)
    if broken == original:
        print(f"❌ injection point not found in {path_str}")
        return 1
    try:
        path.write_text(broken, encoding="utf-8")
        found = check_backend(name, path_str)
    finally:
        path.write_text(original, encoding="utf-8")
    if found:
        print("✅ reverse injection: a capability flag with no method behind it is detected")
        return 0
    print("❌ reverse injection: a capability flag with no method behind it was NOT detected")
    return 1


def main() -> int:
    if "--inject" in sys.argv:
        return inject()

    findings: list[str] = []
    findings += check_every_backend_is_listed()
    checked = 0
    declared = 0
    for name, path_str in BACKENDS:
        values = capabilities_flag_values(
            production_part(pathlib.Path(path_str).read_text(encoding="utf-8"))
        ) if pathlib.Path(path_str).exists() else {}
        if values:
            checked += 1
            declared += len(values)
        findings += check_backend(name, path_str)
        findings += check_supports_claims(name, path_str)

    print(
        f"capability flags: {declared} declared flag(s) across {checked} of {len(BACKENDS)} "
        f"listed backend(s)"
    )
    print()
    if findings:
        print(f"❌ a capability flag promises a method the backend does not define ({len(findings)}):")
        for finding in findings:
            print(f"   {finding}")
        return 1
    print("✅ capability flags: every claim has the method that answers it")
    return 0


if __name__ == "__main__":
    sys.exit(main())
