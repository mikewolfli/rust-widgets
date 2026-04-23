#!/usr/bin/env python3
"""
Split src/render/pipeline/mod.rs (3347 lines) into categorized sub-files.

Sections (0-indexed):
  0-530:   imports + helpers + basic widgets (window..scroll_bar) -> controls.rs
  531-898: menu/toolbar (menu_bar..status_bar) -> menu_toolbar.rs
  899-2339: containers/advanced (tab_widget..scroll_area) -> containers.rs
  2339-2665: pixel ops + text helpers -> pixel_ops.rs
  2666-2925: dialogs -> dialogs.rs
  2926-3286: misc (activity_indicator..wizard) -> misc.rs
  3287-3347: routing functions -> mod.rs
"""

import os

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = os.path.join(WORKSPACE, "src/render/pipeline/mod.rs")
DIR = os.path.join(WORKSPACE, "src/render/pipeline")

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

sections = {
    "controls.rs": (0, 530),
    "menu_toolbar.rs": (530, 897),
    "containers.rs": (897, 2338),
    "pixel_ops.rs": (2338, 2668),
    "dialogs.rs": (2668, 2925),
    "misc.rs": (2925, 3287),
}

# mod.rs will contain only routing functions + re-exports
routing_lines = lines[3287:3347]

for filename, (start, end) in sections.items():
    content = lines[start:end]
    ob = sum(l.count('{') for l in content)
    cb = sum(l.count('}') for l in content)
    diff = ob - cb
    print(f"{filename}: {len(content)} lines, braces {{ {ob} }} {cb}, diff={diff}")
    with open(os.path.join(DIR, filename), "w", encoding="utf-8") as f:
        f.writelines(content)

# Write routing section
ob = sum(l.count('{') for l in routing_lines)
cb = sum(l.count('}') for l in routing_lines)
print(f"routing (to mod.rs): {len(routing_lines)} lines, braces {{ {ob} }} {cb}, diff={ob-cb}")

print("\nAll sections written. Check brace balance — all diffs should be 0.")
