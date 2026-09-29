#!/usr/bin/env python3
"""Classify the 30 candidates by *why* they might not need a direct `request_redraw`.

Four shapes account for almost all of them, and each is legitimate:

  A. **delegated** -- the body calls something that redraws (often a signal handler, or a helper
     whose name says so). The redraw happens one call away.
  B. **animation-driven** -- the value only takes effect on a `tick`, so the frame loop is already
     running and a redraw is owed by the animation rather than by the setter.
  C. **theme-reapplied** -- `reapply_active_theme_state` recomputes the style, which is the thing
     that actually paints.
  D. **nothing paints it yet** -- the field is read by `draw` only behind a condition this setter
     cannot satisfy, or the read is of a *different* same-named field.

Anything left is a real finding.
"""
from __future__ import annotations

import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
REDRAW_HELPERS = (
    "request_redraw",
    "reapply_active_theme_state",
    "touch_activity",
    "announce_after",
    "scroll_to",
    "ensure_visible",
    "update_geometry",
    "invalidate",
    "mark_dirty",
)


def strip_comments(line: str) -> str:
    out, in_str, i = [], False, 0
    while i < len(line):
        ch = line[i]
        if ch == '"' and (i == 0 or line[i - 1] != "\\"):
            in_str = not in_str
        if not in_str and line.startswith("//", i):
            break
        out.append(ch)
        i += 1
    return "".join(out)


def body_of(text: str, method: str) -> str | None:
    lines = text.split("\n")
    for i, line in enumerate(lines):
        if re.match(r"^\s{4}(?:pub(?:\([^)]*\))?\s+)?fn\s+" + re.escape(method) + r"\s*[(<]", line):
            depth, seen, body, j = 0, False, [], i
            while j < len(lines):
                raw = strip_comments(lines[j])
                depth += raw.count("{") - raw.count("}")
                if "{" in raw:
                    seen = True
                body.append(raw)
                if seen and depth <= 0:
                    break
                j += 1
            return "\n".join(body)
    return None


def main() -> None:
    pairs = [
        ("src/widget/advanced_widgets/pie_menu.rs", "set_hover_color"),
        ("src/widget/advanced_widgets/pie_menu.rs", "set_text_color"),
        ("src/widget/container_widgets/groupbox.rs", "set_registry"),
        ("src/widget/container_widgets/safe_area.rs", "set_margin_color"),
        ("src/widget/container_widgets/scrollarea.rs", "set_registry"),
        ("src/widget/container_widgets/scrollarea.rs", "set_widget_resizable"),
        ("src/widget/container_widgets/scrollarea.rs", "set_viewport"),
        ("src/widget/container_widgets/scrollarea.rs", "set_content_size"),
        ("src/widget/container_widgets/stackedwidget.rs", "set_registry"),
        ("src/widget/dialog/color_dialog.rs", "set_modal"),
        ("src/widget/dialog/dialog_widget.rs", "set_modal"),
        ("src/widget/display_widgets/image_view.rs", "set_scaled"),
        ("src/widget/display_widgets/scrollbar.rs", "set_value"),
        ("src/widget/display_widgets/slider.rs", "set_minimum"),
        ("src/widget/display_widgets/slider.rs", "set_maximum"),
        ("src/widget/display_widgets/slider.rs", "set_range"),
        ("src/widget/display_widgets/switch.rs", "set_checked"),
        ("src/widget/input_widgets/font_combo_box.rs", "set_max_visible_items"),
        ("src/widget/input_widgets/otp_input.rs", "set_value"),
        ("src/widget/input_widgets/otp_input.rs", "set_focused_index"),
        ("src/widget/media_widgets/camera_preview.rs", "set_camera_id"),
        ("src/widget/media_widgets/lottie_widget.rs", "set_frame_rate"),
        ("src/widget/misc_widgets/barcode_scanner.rs", "set_scan_interval"),
        ("src/widget/nav_widgets/tab_view.rs", "set_registry"),
        ("src/widget/overlay_widgets/refresh_control.rs", "set_threshold"),
        ("src/widget/special_widgets/color_picker.rs", "set_hsva"),
        ("src/widget/special_widgets/color_picker.rs", "set_from_hue_point"),
    ]
    for rel, method in pairs:
        text = (ROOT / rel).read_text(encoding="utf-8")
        body = body_of(text, method)
        if body is None:
            print(f"?? {rel}::{method} (body not found)")
            continue
        reasons = [h for h in REDRAW_HELPERS if h in body]
        signal = bool(re.search(r"\.emit\s*\(", body))
        verdict = ";".join(reasons) if reasons else ("emits-a-signal" if signal else "NEEDS A RULING")
        first = " ".join(body.split("\n")[0:4])[:150]
        print(f"[{verdict:28}] {rel.split('/')[-1]:22} {method}\n      {first}")


if __name__ == "__main__":
    main()
