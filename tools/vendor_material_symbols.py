#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Vendor the Material Symbols SVG paths this crate ships, at a **pinned commit SHA**.

# Why a separate script from `gen_icon_data.py`

Two different jobs, two different failure modes:

  * **this script** talks to the network. Its output is the `tools/material_symbols/*.svg`
    vendor tree plus the upstream `LICENSE`. It runs rarely, when the icon set is refreshed.
  * **`gen_icon_data.py`** is offline. It reads the vendor tree and writes
    `src/widget/icon_data.rs`. It runs on every change and in every gate, so it must never
    touch the network.

Splitting them means a build or a gate can never be affected by an upstream outage, and a
refresh is an explicit, reviewable act rather than a side effect of generating.

# Why the SHA is pinned and this script refuses to run without one

`google/material-design-icons` is updated continuously (measured: a commit on 2026-09-25).
Following `master` would make `tools/icon_census.txt` drift on an upstream schedule, and a
drift in a generated file that a gate compares against reads as "a mysterious failure". The
SHA is therefore a literal in this file, and `--refresh` is what a human runs to move it.

# Licence

Upstream is Apache-2.0 (`LICENSE`, copyright Google LLC) with **no** `NOTICE` file — verified
against the pinned tree. This script fetches `LICENSE` so the repository carries the required
copy (Apache-2.0 §4(a)); `NOTICE` records the derivation; `check_icon_licences.sh` asserts all
of it, including that upstream still ships no `NOTICE`.

Usage:  python3 tools/vendor_material_symbols.py            # fetch anything missing
        python3 tools/vendor_material_symbols.py --refresh  # re-fetch everything
        python3 tools/vendor_material_symbols.py --check     # verify digests, write nothing
"""

from __future__ import annotations

import argparse
import hashlib
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VENDOR_DIR = REPO_ROOT / "tools" / "material_symbols"

# --------------------------------------------------------------------------------------
# The pinned upstream revision. Moving this is a deliberate, reviewable act.
# --------------------------------------------------------------------------------------
UPSTREAM_REPO = "google/material-design-icons"
UPSTREAM_SHA = "bd8cb85bd4bad964fe6918f79665bb40c3a8efef"
UPSTREAM_REF_DESCRIPTION = "master @ 2026-09-25"

RAW_BASE = f"https://raw.githubusercontent.com/{UPSTREAM_REPO}/{UPSTREAM_SHA}"

# SHA-256 of upstream `LICENSE` at the pinned revision, so the vendored copy is verifiably
# that revision's licence text and not whatever a later fetch happens to return.
UPSTREAM_LICENSE_SHA256 = "58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd"

# The chosen style. Material Symbols ships outlined / rounded / sharp; `outlined` is the one
# whose strokes read at 16-24 px, which is where this crate's icons are used.
STYLE = "materialsymbolsoutlined"
SIZE = "24px"

# Local icon token -> upstream icon name. The left side is this crate's `IconName::as_str`
# spelling; the right side is the Material Symbols directory name. They mostly agree, and the
# few that differ are the whole reason this table exists rather than a naming convention.
ICONS: dict[str, str] = {
    "check": "check",
    "cross": "cancel",
    "arrow_left": "arrow_left",
    "arrow_right": "arrow_right",
    "arrow_up": "arrow_upward",
    "arrow_down": "arrow_downward",
    "star": "star",
    "heart": "favorite",
    "settings": "settings",
    "home": "home",
    "search": "search",
    "menu": "menu",
    "close": "close",
    "plus": "add",
    "minus": "remove",
    "info": "info",
    "warning": "warning",
    "error": "error",
    "user": "person",
    "mail": "mail",
    "bell": "notifications",
    "edit": "edit",
    "trash": "delete",
    "share": "share",
    "refresh": "refresh",
    "more": "more_horiz",
    "filter": "filter",
    "lock": "lock",
    "unlock": "lock_open",
    "download": "download",
    "upload": "upload",
    # ── Navigation ──
    # The `chevron_*` family is the single most-used icon in any UI (a disclosure triangle
    # on every menu, combo box, tree node and accordion). Upstream's `expand_more`/`expand_less`
    # are the same shapes as `chevron_down`/`chevron_up` in this style, so only the chevron
    # names are shipped: two tokens resolving to one outline is the `Close == Cross` defect, and
    # `gen_icon_data.py --check` refuses it, so a second name for the same picture is not an
    # option.
    "chevron_left": "chevron_left",
    "chevron_right": "chevron_right",
    "chevron_up": "expand_less",
    "chevron_down": "expand_more",
    "first_page": "first_page",
    "last_page": "last_page",
    # ── Files ──
    "folder": "folder",
    "folder_open": "folder_open",
    "file": "description",
    "save": "save",
    "copy": "content_copy",
    "print": "print",
    # ── Editing ──
    "undo": "undo",
    "redo": "redo",
    "cut": "content_cut",
    "paste": "content_paste",
    "attachment": "attach_file",
    "link": "link",
    # ── Status ──
    # `success` is `check_circle` upstream, and it is deliberately *not* the same outline as
    # `check`: one is a bare tick, the other a filled circle containing it. `gen_icon_data.py
    # --check` rejects two tokens resolving to one outline, so this pairing is guarded, not
    # assumed.
    "success": "check_circle",
    "help": "help",
    "block": "block",
    "schedule": "schedule",
    "hourglass": "hourglass_empty",
    # ── Media ──
    "play": "play_arrow",
    "pause": "pause",
    "stop": "stop",
    "skip_next": "skip_next",
    "volume_up": "volume_up",
    "volume_off": "volume_off",
    # ── Data ──
    "sort": "sort",
    "bar_chart": "bar_chart",
    "calendar": "calendar_month",
    "table": "table_chart",
    # ── Communication ──
    "chat": "chat",
    "call": "call",
    "send": "send",
    "notifications_off": "notifications_off",
    # ── View ──
    # A four-corner expand mark, for fullscreen. It exists because a control needs it: the
    # video player used to draw the Unicode `⛶` (U+26F6) through `draw_text`, a codepoint no
    # bundled face covers, so it fell back to an 8x8 bitmap block. Every symbol a control
    # draws as an "icon" belongs here, where it is a real outline rather than a font gamble.
    "fullscreen": "fullscreen",
}

# `cross` and `close` are two *distinct* declared icons, so they must not share one upstream
# shape — that would reproduce the `Close == Cross` defect (`icon.rs` drew both with one
# method) in the new data, where it would be much harder to see. Upstream `close` is the rounded
# dismiss stroke and upstream `cancel` is a circle with the same X inside it: two different
# pictures, which is what the two local tokens mean. `gen_icon_data.py --check` independently
# rejects any two tokens that resolve to byte-identical outlines, so this pairing is guarded as
# well as chosen.
ICON_OVERRIDES: dict[str, str] = {"cross": "cancel"}


def svg_url(upstream_name: str) -> str:
    return f"{RAW_BASE}/symbols/web/{upstream_name}/{STYLE}/{upstream_name}_{SIZE}.svg"


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "rust-widgets/vendor"})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def fetch_optional(url: str) -> bytes | None:
    """Fetch `url`, answering `None` on 404 rather than raising.

    Used for the upstream `NOTICE` probe: a 404 is the expected answer and must not abort the
    run, but a 200 must be visible so the licence obligation can be reviewed.
    """
    try:
        return fetch(url)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--refresh", action="store_true", help="re-fetch every file")
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the vendored tree matches the pinned revision and upstream; write nothing",
    )
    args = parser.parse_args()

    VENDOR_DIR.mkdir(parents=True, exist_ok=True)

    if args.check:
        return check()

    failures: list[str] = []

    # ── The upstream licence copy (Apache-2.0 §4(a)) ──
    license_path = VENDOR_DIR / "LICENSE"
    if args.refresh or not license_path.exists():
        data = fetch(f"{RAW_BASE}/LICENSE")
        digest = sha256(data)
        if digest != UPSTREAM_LICENSE_SHA256:
            failures.append(
                f"upstream LICENSE digest {digest} != pinned {UPSTREAM_LICENSE_SHA256}"
            )
        else:
            license_path.write_bytes(data)
            print(f"wrote {license_path.relative_to(REPO_ROOT)}")
    else:
        print(f"kept  {license_path.relative_to(REPO_ROOT)}")

    # ── The per-icon SVG paths ──
    for token, upstream in sorted(ICONS.items()):
        target = VENDOR_DIR / f"{token}.svg"
        if args.refresh or not target.exists():
            try:
                data = fetch(svg_url(upstream))
            except Exception as error:  # noqa: BLE001 - reported, not swallowed
                failures.append(f"{token} ({upstream}): {error}")
                continue
            target.write_bytes(data)
            print(f"wrote {target.relative_to(REPO_ROOT)}")

    # ── The upstream NOTICE probe (Apache-2.0 §4(d)) ──
    # Upstream ships no NOTICE at the pinned revision, so the obligation is not triggered.
    # The probe is still run so a *future* upstream that adds one is caught here rather than
    # silently dropped, and so `check_icon_licences.sh` has a fact to assert.
    notice = fetch_optional(f"{RAW_BASE}/NOTICE")
    marker = VENDOR_DIR / "UPSTREAM_HAS_NO_NOTICE"
    if notice is None:
        marker.write_text(
            "Upstream google/material-design-icons at the pinned revision ships no NOTICE\n"
            "file, so Apache-2.0 §4(d) is not triggered. Verified by\n"
            "tools/vendor_material_symbols.py (probe) and tools/check_icon_licences.sh (gate).\n"
            "If this file disappears, upstream added one and its text must be vendored.\n",
            encoding="utf-8",
        )
        print(f"wrote {marker.relative_to(REPO_ROOT)} (upstream has no NOTICE)")
    else:
        marker.unlink(missing_ok=True)
        (VENDOR_DIR / "UPSTREAM_NOTICE").write_bytes(notice)
        print("NOTE: upstream now ships a NOTICE; vendored as UPSTREAM_NOTICE")

    if failures:
        print("\nvendoring failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    return 0


def check() -> int:
    """Verify every vendored file exists and, for the licence, matches the pinned digest."""
    problems: list[str] = []

    license_path = VENDOR_DIR / "LICENSE"
    if not license_path.exists():
        problems.append("tools/material_symbols/LICENSE is missing (Apache-2.0 §4(a))")
    elif sha256(license_path.read_bytes()) != UPSTREAM_LICENSE_SHA256:
        problems.append("tools/material_symbols/LICENSE does not match the pinned upstream digest")

    for token in sorted(ICONS):
        if not (VENDOR_DIR / f"{token}.svg").exists():
            problems.append(f"tools/material_symbols/{token}.svg is missing")

    if not (VENDOR_DIR / "UPSTREAM_HAS_NO_NOTICE").exists():
        problems.append(
            "tools/material_symbols/UPSTREAM_HAS_NO_NOTICE is missing; run without --check to "
            "refresh the upstream NOTICE probe"
        )

    if problems:
        for problem in problems:
            print(f"FAIL  {problem}", file=sys.stderr)
        return 1
    print(f"vendor tree OK: {len(ICONS)} icons + LICENSE, upstream {UPSTREAM_SHA[:12]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
