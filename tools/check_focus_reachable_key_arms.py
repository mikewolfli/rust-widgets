#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule, its reverse injection and its limits are stated in
# tools/check_focus_reachable_key_arms.sh, which is the only caller. This file is the scanner:
# it prints one finding per control whose focus-gated keyboard arm can never run, then a
# `checked=N failed=M` summary. Exit status is always 0; the shell wrapper judges.

import pathlib
import re

# A keyboard arm that is gated on the control actually holding focus. That gate is the right
# shape -- it stops a control from swallowing keys meant for the focused one -- but it is only
# meaningful if the control can *become* the focus owner.
FOCUS_GATED_KEY_ARM = re.compile(r"focus_reason\(\)\.is_some\(\)")
KEY_PRESS = re.compile(r"Event::KeyPress")

# How a control states its own kind. Every control builds its `BaseWidget` with one.
BASE_KIND = re.compile(r"BaseWidget::new\(\s*WidgetKind::(\w+)")

# An explicit opt-in. `Widget::is_focusable` defaults to the kind predicate, and a control whose
# kind is shared with non-interactive siblings overrides it instead of widening the table.
FOCUSABLE_OVERRIDE = re.compile(r"\bfn\s+is_focusable\s*\(")

# The kind predicate's positive list. Variants inside the `#[cfg(widgets_unstripped)]` block are
# still `true` for the profiles that compile them, so a textual read is the right granularity.
KIND_TABLE = re.compile(
    r"pub const fn wants_keyboard_focus\(self\) -> bool \{(.*?)\n    \}\n", re.DOTALL
)
SELF_VARIANT = re.compile(r"Self::(\w+)")

# Shared infrastructure that carries the gate for the controls it dispatches to, not for itself.
NOT_A_CONTROL = {
    "src/widget/base.rs",
    "src/widget/runtime/dispatch.rs",
}


def production_lines(path):
    """[(line_number, text)] outside a `mod tests` block (tests are evidence, not draw sites)."""
    text = path.read_text(encoding="utf-8").splitlines()
    out = []
    i = 0
    while i < len(text):
        if re.match(r"\s*(pub\s+)?mod\s+tests\b", text[i]):
            depth = 0
            started = False
            while i < len(text):
                depth += text[i].count("{") - text[i].count("}")
                if "{" in text[i]:
                    started = True
                i += 1
                if started and depth <= 0:
                    break
            continue
        out.append((i + 1, text[i]))
        i += 1
    return out


def kinds_in_text(text):
    return set(BASE_KIND.findall(text))


def declared_kinds(path):
    """The kinds a control declares, following a split `event`/`draw` file to its siblings.

    A control is often several files (`widget_impl.rs`, `events.rs`, `paint.rs`). The kind is
    stated once, at the `BaseWidget::new` call, so a file without one is resolved from the files
    that share its directory -- and, one level up, from the directory that owns the control.
    """
    kinds = kinds_in_text(path.read_text(encoding="utf-8"))
    if kinds:
        return kinds
    for directory in (path.parent, path.parent.parent):
        if directory is None:
            continue
        for sibling in sorted(directory.glob("*.rs")):
            if sibling == path:
                continue
            kinds |= kinds_in_text(sibling.read_text(encoding="utf-8"))
    return kinds


def has_key_arm(lines, index):
    """Whether `focus_reason()` at `lines[index]` gates a `KeyPress` arm.

    The predicate is written on the arm's own line or on one of the lines an `if` guard can
    span, so the check looks back a few lines for `Event::KeyPress`. `ime_preedit` asks the same
    question while drawing its caret; that is a focus *read*, not a dead keyboard arm.
    """
    for _, text in lines[max(0, index - 4) : index + 1]:
        if KEY_PRESS.search(text):
            return True
    return False


def focusable_kinds(root):
    text = (root / "src/widget/kind.rs").read_text(encoding="utf-8")
    match = KIND_TABLE.search(text)
    if not match:
        return None
    return set(SELF_VARIANT.findall(match.group(1)))


def main():
    root = pathlib.Path(".")
    listed = focusable_kinds(root)
    if listed is None:
        # A gate that cannot read its table must say so rather than pass vacuously.
        print("the `wants_keyboard_focus` table could not be parsed")
        print("checked=0 failed=1")
        return

    checked = 0
    findings = []
    for path in sorted((root / "src/widget").rglob("*.rs")):
        rel = path.as_posix()
        if rel in NOT_A_CONTROL:
            continue
        lines = production_lines(path)
        arm_lines = [
            index for index, (_, text) in enumerate(lines) if FOCUS_GATED_KEY_ARM.search(text)
        ]
        if not any(has_key_arm(lines, index) for index in arm_lines):
            continue
        checked += 1
        if any(FOCUSABLE_OVERRIDE.search(text) for _, text in lines):
            continue
        kinds = declared_kinds(path)
        if not kinds:
            findings.append(f"{rel}: keyboard arm is focus-gated but no BaseWidget kind is stated")
            continue
        if kinds & listed:
            continue
        findings.append(
            f"{rel}: kind(s) {', '.join(sorted(kinds))} answer `wants_keyboard_focus() == false` "
            f"and the control does not override `is_focusable`, so the pointer router can never "
            f"focus it and its keyboard arm is dead code"
        )

    for finding in findings:
        print(finding)
    print(f"checked={checked} failed={len(findings)}")


if __name__ == "__main__":
    main()
