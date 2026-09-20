#!/usr/bin/env python3
"""Every control that handles user input must consult its `enabled` state.

# Why this gate exists

`Widget::set_enabled(false)` is a host's way to take a control out of play. That promise is only
kept if the control's event handler checks it. Nothing enforced the check, and the failures were
concentrated in controls nobody had looked at:

  * `MiniCanvas` emitted `mouse_pressed`, `mouse_released` and `clicked` while disabled — and for
    a press anywhere in the window, since it also had no hit test.
  * `ImePreedit` appended keystrokes to its buffer while disabled.
  * `MaterialSnackbar`, `CupertinoAlertDialog`, `CupertinoSlider` and `MaterialNavigationRail` all
    handled pointer input with no `enabled` check at all.
  * `CandlestickChart` and `VolumeChart` did the same.

# What this gate asks

For every `impl EventHandler for X`, does the handler body reference `is_enabled()`? A handler
that does not is reported unless it is in `NON_INTERACTIVE` with a reason.

# Why an allowlist rather than a blanket rule

Most controls legitimately need no check: `Line`, `Divider`, `Badge`, `QrCode`, `Avatar`, `Arc`,
`Meter`, `SafeArea` and `MasonryLayout` are decoration or pure containers, and their handlers
either do nothing or only forward. Requiring the check everywhere would force meaningless guards
and make the gate noise. The allowlist is therefore explicit, and each entry states *why* the
control has no interaction to disable — so a genuinely interactive control cannot be added to it
without the reason being visibly wrong.

This mirrors `tools/check_control_has_tests.py`'s design: the interesting output is a new file
that is neither guarded nor listed.

Run from the repo root.
"""

from __future__ import annotations

import pathlib
import re
import sys

WIDGET_DIR = pathlib.Path("src/widget")

# Handler files that legitimately need no `enabled` check, with the reason.
NON_INTERACTIVE: dict[str, str] = {
    "src/widget/display_widgets/line.rs": "pure decoration: draws a rule, handles no input",
    "src/widget/display_widgets/divider.rs": "pure decoration: draws a separator",
    "src/widget/display_widgets/badge.rs": "passive overlay: shows a count, handles no input",
    "src/widget/display_widgets/arc.rs": "pure decoration: draws an arc",
    "src/widget/display_widgets/meter.rs": "pure readout: draws a level, handles no input",
    "src/widget/display_widgets/image_view.rs": "passive: displays an image, handles no input",
    "src/widget/display_widgets/font_preview.rs": "passive: renders sample text for a font",
    "src/widget/display_widgets/mini_chart.rs": "passive: draws a sparkline, handles no input",
    "src/widget/misc_widgets/avatar.rs": "passive: displays a picture or initials",
    "src/widget/misc_widgets/qr_code.rs": "passive: draws a code, handles no input",
    "src/widget/container_widgets/safe_area.rs":
        "container: only forwards, the children it holds are what interact",
    "src/widget/container_widgets/masonry_layout.rs":
        "container: only forwards, the children it holds are what interact",
    "src/widget/menu_toolbar/status_bar.rs":
        "passive: status text with no interaction; `show_message` is programmatic",
    "src/widget/dialog/tooltip.rs":
        "shown and hidden by its owner, not by user input; `MouseEnter`/`MouseLeave` only "
        "redraw",
    "src/widget/window.rs": "the window shell itself; its children hold the interactions",
    "src/widget/draw_bridge.rs": "not a control: the `impl_draw_bridge!` macro body",
    "src/widget/capability/properties_tests.rs": "test support, not a control",
    "src/widget/capability/access.rs": "property dispatch, not a control",
    "src/widget/base.rs":
        "`BaseWidget` is the shared primitive-signal router every control delegates to; it "
        "deliberately has no gate because the concrete control owns the decision (see its own "
        "doc: it emits only `hover`/`mouse_down`/`mouse_up`/`key_down`/`key_up`/focus, never the "
        "semantic signals)",
    "src/widget/cupertino/core.rs":
        "`CupertinoSwitch` and the other wrappers here forward to an inner control that applies "
        "its own `enabled` check",
    "src/widget/chart_widgets/bar_chart.rs": "passive: its handler only delegates to the base",
    "src/widget/chart_widgets/line_chart.rs": "passive: its handler only delegates to the base",
    "src/widget/chart_widgets/pie_chart.rs": "passive: its handler only delegates to the base",
    "src/widget/chart_widgets/sparkline.rs": "passive: its handler only delegates to the base",
    "src/widget/container_widgets/stackedwidget.rs":
        "passive: its handler only delegates to the base",
    "src/widget/display_widgets/icon.rs": "passive: its handler only delegates to the base",
    "src/widget/display_widgets/lcd_number.rs":
        "passive readout: its handler only delegates to the base",
    "src/widget/display_widgets/progress_circle.rs":
        "passive readout: its handler only delegates to the base",
    "src/widget/display_widgets/progressbar.rs":
        "passive readout: its handler only delegates to the base",
}

IMPL_RE = re.compile(r"impl(?:\s+[\w:]+)?\s+EventHandler\s+for\s+(\w+)")

HANDLER_OPEN_RE = re.compile(r"fn\s+handle_event\s*\([^)]*\)\s*\{")


def handler_body(text: str) -> str | None:
    """The body of the first `handle_event`, by brace matching."""
    open_match = HANDLER_OPEN_RE.search(text)
    if not open_match:
        return None
    depth = 1
    index = open_match.end()
    while index < len(text) and depth > 0:
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        index += 1
    return text[open_match.end() : index]


def main() -> int:
    if not WIDGET_DIR.is_dir():
        print("❌ src/widget not found (run from the repo root)")
        return 1

    unguarded: list[tuple[str, str]] = []
    guarded = 0
    allowed = 0

    for path in sorted(WIDGET_DIR.rglob("*.rs")):
        # Skip test modules: a test file's handler is not production behaviour.
        text = path.read_text()
        for marker in ("\n#[cfg(test)]\nmod tests {", "\nmod tests {"):
            cut = text.find(marker)
            if cut != -1:
                text = text[:cut]
                break

        matches = list(IMPL_RE.finditer(text))
        if not matches:
            continue

        rel = path.as_posix()
        for match in matches:
            control = match.group(1)
            # Take the region from this impl header to the next one (or EOF).
            end = len(text)
            for later in matches:
                if later.start() > match.start():
                    end = later.start()
                    break
            region = text[match.start() : end]

            per_handler = region.count("fn handle_event")
            body = handler_body(region)
            if body is None:
                continue

            if "is_enabled()" in region:
                guarded += per_handler or 1
            elif rel in NON_INTERACTIVE:
                allowed += per_handler or 1
            else:
                unguarded.append((rel, control))

    print(f"EventHandler impls found: {guarded + allowed + len(unguarded)}")
    print(f"  guarded by `is_enabled()`: {guarded}")
    print(f"  allowlisted as non-interactive: {allowed}")
    print()

    if unguarded:
        print(f"❌ {len(unguarded)} input-handling control(s) never consult `enabled`:")
        for rel, control in unguarded:
            print(f"   {control}  ({rel})")
        print()
        print("   A host that calls `set_enabled(false)` still receives this control's events.")
        print("   Either add `if !self.base.is_enabled() { return; }` to the handler, or add the")
        print("   file to NON_INTERACTIVE in this script with the reason it has no interaction.")
        return 1

    print("✅ enabled contract: every input-handling control consults `enabled`, or is")
    print("   allowlisted with a stated reason")
    return 0


if __name__ == "__main__":
    sys.exit(main())
