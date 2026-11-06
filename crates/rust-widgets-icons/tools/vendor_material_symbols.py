#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Vendor the Material Symbols SVG paths this crate ships, at a **pinned commit SHA**.

# Why a separate script from `gen_icon_data.py`

Two different jobs, two different failure modes:

  * **this script** talks to the network. Its output is the `tools/material_symbols/*.svg`
    vendor tree, the upstream codepoint list and the upstream `LICENSE`. It runs rarely, when the
    icon set is refreshed.
  * **`gen_icon_data.py`** is offline. It reads the vendor tree and writes
    `crates/rust-widgets-icons/src/data.rs`. It runs on every change and in every gate, so it must never
    touch the network.

Splitting them means a build or a gate can never be affected by an upstream outage, and a
refresh is an explicit, reviewable act rather than a side effect of generating.

# Why the SHA is pinned and this script refuses to run without one

`google/material-design-icons` is updated continuously (measured: a commit on 2026-09-25).
Following `master` would make `tools/icon_census.txt` drift on an upstream schedule, and a
drift in a generated file that a gate compares against reads as "a mysterious failure". The
SHA is therefore a literal in this file, and `--refresh` is what a human runs to move it.

# The three tables, and why each one is a literal

The catalogue is upstream's, so most tokens are their own upstream name and need no entry here:
for those, `ICONS[token] == token`. Three groups are *not* a naming convention and are stated
explicitly, because a convention cannot express them and a derived answer would be a guess:

  * `TRANSLATIONS` — the 75 hand-curated tokens whose spelling differs from upstream
    (`user` -> `person`, `cross` -> `cancel`). Each is a reviewed judgement.
  * `PINNED_ALIASES` — the 136 catalogue names that upstream publishes **no standalone file
    for** at the pinned revision. Their outline is a *different* name's file, and the pairing was
    measured, not assumed: the release tree's `<name>.svg` and the pinned `<canonical>.svg` carry
    byte-identical `d` (all 136 pairs verified). Vendoring the pinned file keeps one revision.
  * `RELEASE_ONLY` — the 73 catalogue names absent from the pinned tree *and* from any pinned
    outline (they are the retired Material Icons spellings: `place`, `people`, `help_outline`, …).
    Their geometry comes from the Material Symbols release tree, because dropping 73 names a host
    may well pass to `Icon::set_icon` is worse than a recorded revision exception. `data.rs`'s
    header and `NOTICE` state the exception.
  * `LEGACY_GRID` — the 73 names the pinned tree carries only on its legacy 24-unit, y-down grid
    (an SVG with no `viewBox`, or one declaring `0 96 960 960`). This crate stores one
    coordinate convention, so those files come from the release tree's 960-unit build instead.
    See the table's own comment; `gen_icon_data.py` refuses an off-grid vendor file by name.

# Licence

Upstream is Apache-2.0 (`LICENSE`, copyright Google LLC) with **no** `NOTICE` file — verified
against the pinned tree. This script fetches `LICENSE` so the repository carries the required
copy (Apache-2.0 §4(a)); `NOTICE` records the derivation; `check_icon_licences.sh` asserts all
of it, including that upstream still ships no `NOTICE`. The codepoint list is upstream's own
file, vendored verbatim and pinned by digest for the same reason.

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

sys.path.insert(0, str(Path(__file__).resolve().parent))
from icon_sources import (  # noqa: E402
    REPO_ROOT,
    VENDOR_DIR,
    catalog_declarations,
    curated_declarations,
    upstream_codepoints,
)

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

# The upstream codepoint list: the **name set** the catalogue is generated from. Vendored
# verbatim (no header added) so its digest is upstream's file, and pinned by digest for the same
# reason the licence is.
UPSTREAM_CODEPOINTS_PATH = "variablefont/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].codepoints"
UPSTREAM_CODEPOINTS_SHA256 = "225bd09137103cb7746bc93dc08d08764c9f0c3bd04f4b958d4a3c3c19432dd6"

# The codepoint list's filename is the one part of the URL that is percent-encoded; jsDelivr and
# raw.githubusercontent disagree about `[`/`,`, so the path is encoded once here.
UPSTREAM_CODEPOINTS_ENCODED = UPSTREAM_CODEPOINTS_PATH.replace("[", "%5B").replace("]", "%5D")

# The chosen style. Material Symbols ships outlined / rounded / sharp; `outlined` is the one
# whose strokes read at 16-24 px, which is where this crate's icons are used.
STYLE = "materialsymbolsoutlined"
SIZE = "24px"

# The release tree, for the `RELEASE_ONLY` names. It is not a pinned revision — see the module
# docstring — so only those names are ever fetched from it.
RELEASE_TREE_BASE = "https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined"

# Local icon token -> upstream icon name, for the **curated** spellings only. The left side is
# this crate's `IconName::as_str` spelling; the right side is the Material Symbols directory name.
# They mostly agree, and the few that differ are the whole reason this table exists rather than a
# naming convention.
TRANSLATIONS: dict[str, str] = {
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
    # names are shipped as translations: two tokens resolving to one outline is now *recorded*
    # (`UPSTREAM_ALIASES`) rather than refused, because the catalogue is upstream's own name set
    # and upstream itself publishes one picture under several names.
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
    # `check`: one is a bare tick, the other a filled circle containing it. The duplicate-outline
    # relation is computed and published as `UPSTREAM_ALIASES`, so this pairing stays visible
    # rather than assumed.
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
    # ── Designer vocabulary (RW-36) ──
    "debug_bug": "bug_report",
    "design_component": "widgets",
    "design_event": "bolt",
    "test_flask": "science",
    "tools_terminal": "terminal",
    "tools_translate": "translate",
}

# `cross` and `close` are two *distinct* declared icons, so they must not share one upstream
# shape — that would reproduce the `Close == Cross` defect (`icon.rs` drew both with one
# method) in the data, where it would be much harder to see. Upstream `close` is the rounded
# dismiss stroke and upstream `cancel` is a circle with the same X inside it: two different
# pictures, which is what the two local tokens mean.
ICON_OVERRIDES: dict[str, str] = {"cross": "cancel"}

# The 136 catalogue names with no standalone pinned file whose pinned outline lives under
# another name. Derived by measurement (release-tree `d` == pinned `d`, all 136 byte-identical),
# then frozen here so a refresh cannot silently re-derive a different answer.
PINNED_ALIASES: dict[str, str] = {
    "access_time": "schedule",
    "access_time_filled": "schedule",
    "add_circle_outline": "add_circle",
    "add_ic_call": "add_call",
    "addchart": "add_chart",
    "announcement": "feedback",
    "assessment": "insert_chart",
    "assistant_photo": "emoji_flags",
    "battery_std": "battery_full",
    "bluetooth_audio": "bluetooth_searching",
    "bookmark_border": "bookmark",
    "browser_not_supported": "web_asset_off",
    "business": "domain",
    "call_end_alt": "call_end",
    "camera_alt": "photo_camera",
    "chat_bubble_outline": "chat_bubble",
    "check_circle_filled": "check_circle",
    "check_circle_outline": "check_circle",
    "class": "book",
    "clear": "close",
    "closed_caption_off": "closed_caption",
    "collections": "photo_library",
    "control_point": "add_circle",
    "create": "edit",
    "crop_din": "crop_square",
    "crop_original": "image",
    "data_saver_off": "data_usage",
    "delete_outline": "delete",
    "directions_car_filled": "directions_car",
    "do_disturb_off": "do_not_disturb_off",
    "do_disturb_on": "do_not_disturb_on",
    "drive_eta": "directions_car",
    "email": "mail",
    "expension_panels": "expansion_panels",
    "favorite_border": "favorite",
    "filter_list_alt": "filter_alt",
    "flag_filled": "emoji_flags",
    "free_breakfast": "local_cafe",
    "games": "gamepad",
    "get_app": "download",
    "gpp_good": "verified_user",
    "gps_off": "location_disabled",
    "headset": "headphones",
    "highlight_alt": "ink_selection",
    "highlight_off": "cancel",
    "home_filled": "home",
    "import_export": "swap_vert",
    "insert_chart_filled": "insert_chart",
    "insert_chart_outlined": "insert_chart",
    "insert_comment": "comment",
    "insert_drive_file": "draft",
    "insert_link": "link",
    "insert_photo": "image",
    "iso": "exposure",
    "keep_pin": "keep",
    "laptop": "computer",
    "launch": "open_in_new",
    "local_movies": "theaters",
    "local_offer": "sell",
    "local_phone": "call",
    "local_play": "local_activity",
    "local_printshop": "print",
    "locator_tag": "nest_tag",
    "loop": "autorenew",
    "mail_outline": "mail",
    "maps_home_work": "home_work",
    "markunread": "mail",
    "message": "chat",
    "missed_video_call_filled": "missed_video_call",
    "mode": "edit",
    "mode_edit": "edit",
    "mode_edit_outline": "edit",
    "money_off_csred": "money_off",
    "movie_creation": "movie",
    "no_encryption_gmailerrorred": "no_encryption",
    "notifications_none": "notifications",
    "ondemand_video": "live_tv",
    "outlined_flag": "emoji_flags",
    "pause_circle_filled": "pause_circle",
    "pause_circle_outline": "pause_circle",
    "payment": "credit_card",
    "personal_video": "tv",
    "phone": "call",
    "phone_alt": "call",
    "photo_size_select_actual": "image",
    "pie_chart_filled": "pie_chart",
    "pie_chart_outline": "pie_chart",
    "pie_chart_outlined": "pie_chart",
    "plus_one": "exposure_plus_1",
    "poll": "insert_chart",
    "query_builder": "schedule",
    "queue": "add_to_photos",
    "rate_review_rtl": "rate_review",
    "reminders_alt": "reminder",
    "remove_circle": "do_not_disturb_on",
    "remove_circle_outline": "do_not_disturb_on",
    "report_gmailerrorred": "report",
    "restore": "history",
    "ring_volume_filled": "ring_volume",
    "save_alt": "download",
    "sd_storage": "sd_card",
    "security_update": "mobile_arrow_down",
    "settings_input_composite": "settings_input_component",
    "shop_2": "shop_two",
    "shortcut": "forward",
    "signal_cellular_no_sim": "no_sim",
    "signal_wifi_4_bar_lock": "wifi_lock",
    "signal_wifi_connected_no_internet_4": "signal_wifi_bad",
    "signal_wifi_statusbar_4_bar": "signal_wifi_4_bar",
    "sim_card_alert": "sd_card_alert",
    "source": "topic",
    "star_border": "grade",
    "star_border_purple500": "grade",
    "star_outline": "grade",
    "star_purple500": "grade",
    "store_mall_directory": "store",
    "system_security_update": "mobile_arrow_down",
    "system_security_update_good": "mobile_check",
    "terrain": "landscape",
    "thumb_down_alt": "thumb_down",
    "thumb_down_filled": "thumb_down",
    "thumb_down_off_alt": "thumb_down",
    "thumb_up_alt": "thumb_up",
    "thumb_up_filled": "thumb_up",
    "thumb_up_off_alt": "thumb_up",
    "time_to_leave": "directions_car",
    "try": "reviews",
    "tungsten": "fluorescent",
    "turned_in": "bookmark",
    "turned_in_not": "bookmark",
    "unpin": "keep_off",
    "watch_later": "schedule",
    "wb_cloudy": "cloud",
    "work_off": "enterprise_off",
    "work_outline": "work",
    "workflow": "arrow_split",
}

# Names the pinned tree does not carry at all. Their geometry is the release tree's, which is
# the one place in this crate where an outline is not the pinned revision's. See the module
# docstring; `data.rs`'s header and `NOTICE` repeat the exception.
RELEASE_ONLY: frozenset[str] = frozenset({
    "access_alarm",
    "access_alarms",
    "account_circle_filled",
    "add_alarm",
    "app_settings_alt",
    "audiotrack",
    "auto_draw_solid",
    "card_giftcard",
    "color_lens",
    "communities_filled",
    "contact_phone_filled",
    "delivery_dining",
    "directions_boat_filled",
    "directions_bus_filled",
    "directions_railway_filled",
    "directions_subway_filled",
    "directions_transit",
    "directions_transit_filled",
    "do_disturb",
    "do_disturb_alt",
    "do_not_disturb",
    "do_not_disturb_alt",
    "drawing_recognition",
    "emoji_emotions",
    "error_outline",
    "face_unlock",
    "fmd_good",
    "gps_fixed",
    "gps_not_fixed",
    "handwriting_recognition",
    "help_outline",
    "https",
    "insert_emoticon",
    "insert_invitation",
    "label_important_outline",
    "label_outline",
    "lightbulb_outline",
    "local_airport",
    "local_grocery_store",
    "local_hotel",
    "location_pin",
    "lock_outline",
    "mic_none",
    "modeling",
    "motion_photos_pause",
    "nightlight_round",
    "not_interested",
    "order_play",
    "outpatient_med",
    "oxygen_saturation",
    "people",
    "people_alt",
    "people_outline",
    "perm_identity",
    "person_add_alt",
    "person_outline",
    "phonelink",
    "place",
    "play_music",
    "portrait",
    "question_answer",
    "remove_red_eye",
    "report_problem",
    "room",
    "sentiment_satisfied_alt",
    "settings_panorama",
    "shadow_minus",
    "shape_recognition",
    "system_security_update_warning",
    "tablet_android",
    "tag_faces",
    "textsms",
    "warning_amber",
})


# 73 names upstream publishes at the pinned revision only in its **legacy encoding**: an SVG with
# no `viewBox` (66 of them) or one declaring `0 96 960 960` (7), both authored on a 24-unit,
# y-down grid. This crate stores one convention for its geometry — `grid: 960`, y up, which is
# what `IconData`'s coordinates mean — so re-encoding those numbers here would make the generated
# header's "copied verbatim" claim false. They are vendored from the release tree instead, whose
# build emits the 960-unit form for every name. `gen_icon_data.py` refuses any vendor file that
# is not on the 960 grid, so a refresh cannot quietly reintroduce the legacy form and collapse
# the icon into a two-pixel sliver.
LEGACY_GRID: frozenset[str] = frozenset({
    "ambient_screen",
    "assistant",
    "astrophotography_auto",
    "astrophotography_off",
    "auto_activity_zone",
    "auto_awesome",
    "auto_detect_voice",
    "auto_fix",
    "auto_fix_high",
    "auto_fix_normal",
    "auto_fix_off",
    "auto_graph",
    "auto_label",
    "auto_meeting_room",
    "auto_mode",
    "auto_schedule",
    "auto_videocam",
    "autopay",
    "battery_20",
    "battery_30",
    "battery_50",
    "battery_60",
    "battery_80",
    "battery_90",
    "battery_vert_005",
    "battery_vert_020",
    "battery_vert_050",
    "camera_enhance",
    "cloud_queue",
    "cloudy",
    "cloudy_filled",
    "drive_file_move_outline",
    "drive_file_move_rtl",
    "drive_file_rename_outline",
    "error_circle_rounded",
    "face_retouching_natural",
    "file_download",
    "file_upload",
    "flourescent",
    "generating_tokens",
    "google_plus_reshare",
    "hourglass_full",
    "image_not_supported",
    "insights",
    "join_full",
    "keyboard_voice",
    "location_automation",
    "magic_button",
    "magic_exchange",
    "magic_tether",
    "movie_filter",
    "nest_locator_tag",
    "nest_wifi_gale",
    "nest_wifi_mistral",
    "nest_wifi_point_vento",
    "note",
    "person_filled",
    "photo_filter",
    "power_rounded",
    "push_pin",
    "replay_circle_filled",
    "settings_suggest",
    "sleep",
    "smart_button",
    "sms_failed",
    "temp_preferences_custom",
    "thumb_down_off",
    "thumb_up_off",
    "tips_and_updates",
    "tools_wrench",
    "view_in_ar_new",
    "vitals",
    "workspaces_outline",
})

# Every name whose file does **not** come from the pinned revision: absent there entirely
# (`RELEASE_ONLY`), or present only in the legacy encoding above (`LEGACY_GRID`). The fetch path
# reads this set, and `data.rs`'s header and `NOTICE` state the count.
RELEASE_TREE_SOURCES: frozenset[str] = RELEASE_ONLY | LEGACY_GRID


def curated_tokens() -> list[str]:
    """The curated token column of `tools/icon_tokens.txt`, in declaration order.

    These are the tokens whose spelling and doc line a human wrote (`user`, `cross`, …); the
    parsing lives in `icon_sources` so all six generators read one format.
    """
    return [token for token, _variant, _doc in curated_declarations()]


def catalog_tokens() -> list[str]:
    """The generated token column of `tools/icon_catalog.txt`, in declaration order."""
    return [token for token, _variant, _doc in catalog_declarations()]


def upstream_names() -> list[str]:
    """Every icon name in the vendored upstream codepoint list, in upstream order."""
    return [name for name, _codepoint in upstream_codepoints()]


def _build_icons() -> dict[str, str]:
    """The full token -> upstream name map, derived from the three tables above.

    Curated tokens take `TRANSLATIONS`; catalogue tokens are their own upstream name unless
    `PINNED_ALIASES` says the pinned file lives elsewhere. `RELEASE_ONLY` names are their own.
    """
    icons: dict[str, str] = {}
    for token in curated_tokens():
        if token not in TRANSLATIONS:
            raise SystemExit(
                f"REFUSED: curated token {token!r} has no TRANSLATIONS entry, so no SVG is vendored "
                "for it. Add it (and run with --refresh), or remove it from tools/icon_tokens.txt."
            )
        icons[token] = TRANSLATIONS[token]
    for token in catalog_tokens():
        icons[token] = PINNED_ALIASES.get(token, token)
    unknown = sorted(set(PINNED_ALIASES) - set(icons))
    if unknown:
        raise SystemExit(
            "REFUSED: these PINNED_ALIASES keys are not catalogued tokens, so nothing can reach "
            f"them: {unknown}"
        )
    stray = sorted(RELEASE_ONLY - set(icons))
    if stray:
        raise SystemExit(f"REFUSED: these RELEASE_ONLY names are not catalogued tokens: {stray}")
    stray = sorted(LEGACY_GRID - set(icons))
    if stray:
        raise SystemExit(f"REFUSED: these LEGACY_GRID names are not catalogued tokens: {stray}")
    return icons


# The single map every generator reads: token -> upstream name, for every token this crate ships.
ICONS: dict[str, str] = _build_icons()


def svg_url(upstream_name: str) -> str:
    return f"{RAW_BASE}/symbols/web/{upstream_name}/{STYLE}/{upstream_name}_{SIZE}.svg"


def release_svg_url(name: str) -> str:
    return f"{RELEASE_TREE_BASE}/{name}/default/{SIZE}.svg"


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


def looks_like_icon_svg(data: bytes) -> bool:
    """Whether `data` is an SVG with at least one `<path d="…">`.

    A fetch that answers 200 with an HTML error page, or with an SVG whose path was emptied,
    must not be written into the vendor tree: the generator would then ship an icon that draws
    nothing, which is indistinguishable from a missing icon on screen.
    """
    text = data.decode("utf-8", errors="replace")
    return "<svg" in text and '<path' in text and 'd="' in text


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

    # ── The upstream codepoint list (the catalogue's name set) ──
    codepoints_path = VENDOR_DIR / "upstream_codepoints.txt"
    if args.refresh or not codepoints_path.exists():
        try:
            data = fetch(f"{RAW_BASE}/{UPSTREAM_CODEPOINTS_ENCODED}")
        except Exception as error:  # noqa: BLE001 - reported, not swallowed
            failures.append(f"upstream codepoints: {error}")
        else:
            digest = sha256(data)
            if digest != UPSTREAM_CODEPOINTS_SHA256:
                failures.append(
                    f"upstream codepoints digest {digest} != pinned {UPSTREAM_CODEPOINTS_SHA256}"
                )
            else:
                codepoints_path.write_bytes(data)
                print(f"wrote {codepoints_path.relative_to(REPO_ROOT)}")
    else:
        print(f"kept  {codepoints_path.relative_to(REPO_ROOT)}")

    # ── The per-icon SVG paths ──
    for token, upstream in sorted(ICONS.items()):
        target = VENDOR_DIR / f"{token}.svg"
        if args.refresh or not target.exists():
            url = release_svg_url(token) if token in RELEASE_TREE_SOURCES else svg_url(upstream)
            try:
                data = fetch(url)
            except Exception as error:  # noqa: BLE001 - reported, not swallowed
                failures.append(f"{token} ({upstream}): {error}")
                continue
            if not looks_like_icon_svg(data):
                failures.append(f"{token} ({upstream}): the response carries no <path d=…>")
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
    """Verify every vendored file exists and, for the pinned digests, matches."""
    problems: list[str] = []

    license_path = VENDOR_DIR / "LICENSE"
    if not license_path.exists():
        problems.append("tools/material_symbols/LICENSE is missing (Apache-2.0 §4(a))")
    elif sha256(license_path.read_bytes()) != UPSTREAM_LICENSE_SHA256:
        problems.append("tools/material_symbols/LICENSE does not match the pinned upstream digest")

    codepoints_path = VENDOR_DIR / "upstream_codepoints.txt"
    if not codepoints_path.exists():
        problems.append("tools/material_symbols/upstream_codepoints.txt is missing")
    elif sha256(codepoints_path.read_bytes()) != UPSTREAM_CODEPOINTS_SHA256:
        problems.append(
            "tools/material_symbols/upstream_codepoints.txt does not match the pinned upstream digest"
        )
    else:
        # The catalogue must cover exactly the upstream names: a name added upstream and not
        # catalogued is an icon the crate claims not to ship, and the reverse is an icon it
        # cannot draw at this revision.
        upstream = set(upstream_names())
        curated = set(curated_tokens())
        catalogued = set(catalog_tokens())
        missing = sorted(upstream - catalogued - curated)
        extra = sorted(catalogued - upstream)
        if missing:
            problems.append(
                f"{len(missing)} upstream name(s) are not catalogued (run gen_icon_catalog.py): "
                f"{missing[:8]}"
            )
        if extra:
            problems.append(
                f"{len(extra)} catalogue token(s) are not in the pinned codepoint list: {extra[:8]}"
            )

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
    print(
        f"vendor tree OK: {len(ICONS)} icons + LICENSE + codepoints, upstream {UPSTREAM_SHA[:12]}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
