#!/usr/bin/env python3
"""Check for controls a widget library is normally expected to ship but this one may not."""
import re

s = open('src/widget/capability/properties.rs').read()
names = set(re.findall(r'canonical_name:\s*"([^"]+)"', s))

# Names a reviewer would reasonably look for. Grouped by the family they belong to, so a
# miss reads as "this family has a hole" rather than as an arbitrary list.
EXPECTED = {
    "core inputs": [
        "button", "label", "check_box", "radio_button", "combo_box", "line_edit",
        "spin_box", "slider", "progress_bar", "list_box", "scroll_bar", "scroll_area",
    ],
    "text": [
        "text_edit", "rich_edit", "code_editor", "markdown_editor", "text_area",
        "password_edit", "search_bar", "search_box",
    ],
    "selection": [
        "list_view", "tree_view", "table", "data_grid", "grid_table", "tree_table",
        "segmented_button", "segmented_control", "toggle_button", "switch", "rating",
        "stepper", "number_picker", "range_slider",
    ],
    "containers": [
        "panel", "group_box", "frame", "tab_widget", "tab_bar", "tab_view",
        "splitter", "dock_widget", "collapsible_pane", "accordion", "carousel",
        "stacked_widget", "mdi_area", "tool_box",
    ],
    "menus & chrome": [
        "menu_bar", "menu", "menu_button", "tool_bar", "status_bar", "ribbon_bar",
        "dropdown", "dropdown_menu", "context_menu", "app_bar",
    ],
    "dialogs": [
        "dialog", "message_box", "input_dialog", "file_dialog", "color_dialog",
        "font_dialog", "find_replace_dialog", "wizard_dialog", "progress_dialog",
        "alert_dialog", "confirm_dialog", "about_dialog",
    ],
    "date & time": [
        "calendar", "date_edit", "time_edit", "date_time_edit", "date_range_picker",
        "time_picker", "clock",
    ],
    "display": [
        "image_view", "icon", "avatar", "badge", "divider", "tooltip", "popover",
        "chip", "tag", "skeleton_loader", "spinner", "empty_state", "hero_animation",
    ],
    "layout": [
        "grid", "flex", "flow_layout", "wrap_layout", "masonry_layout", "safe_area",
        "split_layout", "aspect_ratio", "spacer", "center", "align", "sized_box",
        "padding", "expanded", "flexible", "intrinsic_width",
    ],
    "navigation": [
        "navigation_drawer", "navigation_stack", "bottom_navigation_bar",
        "material_navigation_rail", "pagination", "breadcrumb", "tab_view",
    ],
    "charts": [
        "chart", "line_chart", "bar_chart", "pie_chart", "radar_chart",
        "candlestick_chart", "sparkline", "mini_chart", "gauge", "heatmap",
    ],
    "media": [
        "video_player", "audio_visualizer", "media_player", "camera_preview",
        "lottie_widget", "rive_widget", "animated_image",
    ],
    "advanced input": [
        "otp_input", "mention", "tag_input", "masked_edit", "phone_input",
        "email_input", "signature_pad", "color_picker", "qr_code", "barcode_scanner",
    ],
}

missing_total = []
for family, items in EXPECTED.items():
    missing = [i for i in items if i not in names]
    if missing:
        print(f"{family}:")
        for m in missing:
            print(f"    missing? {m}")
        missing_total.extend(missing)

print()
print(f"candidate gaps: {len(missing_total)}")
