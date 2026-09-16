#!/usr/bin/env python3
"""
Split src/platform/windows.rs (2181 lines) into a windows/ directory.
Strategy:
  - windows/mod.rs          → module declarations + re-exports
  - windows/types.rs         → WindowsHandleKind, WindowsPlatform, Win32MenuState,
                               PlatformDowncast, WindowsPlatformExtSlider + impls
  - windows/helpers.rs       → try_create_label, try_create_slider,
                               try_create_progress_bar, try_create_combo_box
  - windows/notify.rs        → ensure_window_class_registered, active_windows_platform,
                               control_notify_kind_for_widget, enqueue_control_notify_event,
                               notify_kind_for_widget
  - windows/platform_impl.rs → impl Platform for WindowsPlatform (big block)
  - windows/tests.rs         → mod tests
"""

import os
import shutil

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC_FILE = os.path.join(WORKSPACE, "src/platform/windows.rs")
BACKUP = os.path.join(WORKSPACE, "src/platform/windows.rs.bak")
WIN_DIR = os.path.join(WORKSPACE, "src/platform/windows")

# Read original
with open(SRC_FILE, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Backup
shutil.copy2(SRC_FILE, BACKUP)
print(f"Backed up to {BACKUP}")

# Define line ranges (0-based)
# Lines 1-2: initial use imports
# Lines 3-81: try_create_label, try_create_slider
# Lines 82-86: use imports
# Lines 87-1578: impl Platform for WindowsPlatform
# Lines 1579-1677: WindowsHandleKind enum
# Lines 1678-1701: ensure_window_class_registered
# Lines 1702-1710: active_windows_platform
# Lines 1711-1756: control_notify_kind_for_widget
# Lines 1757-1783: enqueue_control_notify_event
# Lines 1784-1793: notify_kind_for_widget
# Lines 1794-1846: impl WindowsPlatform
# Lines 1847-1858: PlatformDowncast trait + impl
# Lines 1859-1874: use imports for structs
# Lines 1875-1884: pub struct WindowsPlatform
# Lines 1885-1895: pub struct Win32MenuState
# Lines 1896-1911: impl Win32MenuState
# Lines 1912-1973: try_create_progress_bar
# Lines 1974-2038: try_create_combo_box
# Lines 2039-2049: impl WindowsPlatform
# Lines 2050-2054: impl Default for WindowsPlatform
# Lines 2055-2129: WindowsPlatformExtSlider trait + impl
# Lines 2130-2181: mod tests

os.makedirs(WIN_DIR, exist_ok=True)

# ============================================================
# 1. helpers.rs - try_create_label, try_create_slider,
#                 try_create_progress_bar, try_create_combo_box
# ============================================================
helpers_lines = []
# Add necessary use for helpers
helpers_lines.append("//! Win32 helper functions for native control creation.\n")
helpers_lines.append("\n")
# Copy lines 1-2 (initial use)
helpers_lines.extend(lines[0:2])  # use crate::platform::{state::BackendState, DropEvent};
# Copy try_create_label (lines 2-62, 0-indexed 1-61)
helpers_lines.extend(lines[1:62])  # includes the doc comment
# Copy try_create_slider (lines 63-81, 0-indexed 62-80)
helpers_lines.extend(lines[62:81])
# Copy try_create_progress_bar (lines 1912-1973, 0-indexed 1911-1972)
helpers_lines.extend(lines[1911:1973])
# Copy try_create_combo_box (lines 1974-2038, 0-indexed 1973-2037)
helpers_lines.extend(lines[1973:2038])
# End with newline
helpers_lines.append("\n")

with open(os.path.join(WIN_DIR, "helpers.rs"), "w", encoding="utf-8") as f:
    f.writelines(helpers_lines)
print(f"helpers.rs: {len(helpers_lines)} lines")

# ============================================================
# 2. types.rs - WindowsHandleKind, WindowsPlatform, Win32MenuState,
#               PlatformDowncast, WindowsPlatformExtSlider + impls
# ============================================================
types_lines = []
types_lines.append("//! Windows platform types, structs, enums, and traits.\n")
types_lines.append("\n")
# WindowsHandleKind (lines 1579-1677, 0-indexed 1578-1676)
types_lines.extend(lines[1578:1677])
# Blank line
types_lines.append("\n")
# impl WindowsPlatform (lines 1794-1846, 0-indexed 1793-1845)
types_lines.extend(lines[1793:1846])
# Blank line
types_lines.append("\n")
# PlatformDowncast (lines 1847-1858, 0-indexed 1846-1857)
types_lines.extend(lines[1846:1858])
# Blank line
types_lines.append("\n")
# use + structs (lines 1859-1911, 0-indexed 1858-1910)
types_lines.extend(lines[1858:1911])
# Blank line
types_lines.append("\n")
# impl WindowsPlatform (lines 2039-2049, 0-indexed 2038-2048)
types_lines.extend(lines[2038:2049])
# Blank line
types_lines.append("\n")
# impl Default (lines 2050-2054, 0-indexed 2049-2053)
types_lines.extend(lines[2049:2054])
# Blank line
types_lines.append("\n")
# WindowsPlatformExtSlider trait + impl (lines 2055-2129, 0-indexed 2054-2128)
types_lines.extend(lines[2054:2129])
types_lines.append("\n")

with open(os.path.join(WIN_DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(types_lines)
print(f"types.rs: {len(types_lines)} lines")

# ============================================================
# 3. notify.rs - helper functions for event notification
# ============================================================
notify_lines = []
notify_lines.append("//! Win32 event notification helpers.\n")
notify_lines.append("\n")
# ensure_window_class_registered (lines 1678-1701, 0-indexed 1677-1700)
notify_lines.extend(lines[1677:1701])
notify_lines.append("\n")
# active_windows_platform (lines 1702-1710, 0-indexed 1701-1709)
notify_lines.extend(lines[1701:1710])
notify_lines.append("\n")
# control_notify_kind_for_widget (lines 1711-1756, 0-indexed 1710-1755)
notify_lines.extend(lines[1710:1756])
notify_lines.append("\n")
# enqueue_control_notify_event (lines 1757-1783, 0-indexed 1756-1782)
notify_lines.extend(lines[1756:1783])
notify_lines.append("\n")
# notify_kind_for_widget (lines 1784-1793, 0-indexed 1783-1792)
notify_lines.extend(lines[1783:1793])
notify_lines.append("\n")

with open(os.path.join(WIN_DIR, "notify.rs"), "w", encoding="utf-8") as f:
    f.writelines(notify_lines)
print(f"notify.rs: {len(notify_lines)} lines")

# ============================================================
# 4. platform_impl.rs - impl Platform for WindowsPlatform
# ============================================================
impl_lines = []
impl_lines.append("//! `impl Platform for WindowsPlatform` — the main trait implementation.\n")
impl_lines.append("\n")
# use imports (lines 82-86, 0-indexed 81-85)
impl_lines.extend(lines[81:86])
impl_lines.append("\n")
# Use types from our new module
impl_lines.append("use crate::platform::windows::types::*;\n")
impl_lines.append("\n")
# impl Platform block (lines 87-1578, 0-indexed 86-1577)
impl_lines.extend(lines[86:1578])
impl_lines.append("\n")

with open(os.path.join(WIN_DIR, "platform_impl.rs"), "w", encoding="utf-8") as f:
    f.writelines(impl_lines)
print(f"platform_impl.rs: {len(impl_lines)} lines")

# ============================================================
# 5. tests.rs - test module
# ============================================================
tests_lines = []
tests_lines.append("//! Windows platform tests.\n")
tests_lines.append("\n")
# mod tests (lines 2130-2181, 0-indexed 2129-2180)
tests_lines.extend(lines[2129:2181])

with open(os.path.join(WIN_DIR, "tests.rs"), "w", encoding="utf-8") as f:
    f.writelines(tests_lines)
print(f"tests.rs: {len(tests_lines)} lines")

# ============================================================
# 6. mod.rs - module declarations + re-exports
# ============================================================
mod_lines = []
mod_lines.append("//! Windows platform backend implementation.\n")
mod_lines.append("\n")
mod_lines.append("pub mod helpers;\n")
mod_lines.append("pub mod types;\n")
mod_lines.append("mod notify;\n")
mod_lines.append("mod platform_impl;\n")
mod_lines.append("\n")
mod_lines.append("pub use crate::platform::windows::helpers::*;\n")
mod_lines.append("pub use crate::platform::windows::types::*;\n")
mod_lines.append("\n")
mod_lines.append("#[cfg(test)]\n")
mod_lines.append("mod tests;\n")

with open(os.path.join(WIN_DIR, "mod.rs"), "w", encoding="utf-8") as f:
    f.writelines(mod_lines)
print(f"mod.rs: {len(mod_lines)} lines")

# ============================================================
# 7. Remove old windows.rs
# ============================================================
os.remove(SRC_FILE)
print(f"Removed {SRC_FILE}")

# ============================================================
# 8. Verify src/platform/mod.rs still has `pub mod windows;`
# ============================================================
mod_rs_path = os.path.join(WORKSPACE, "src/platform/mod.rs")
with open(mod_rs_path, "r", encoding="utf-8") as f:
    mod_content = f.read()

if "pub mod windows;" in mod_content:
    print("platform/mod.rs already declares `pub mod windows;` — no change needed.")
else:
    print("WARNING: platform/mod.rs does NOT declare `pub mod windows;`. Check it.")

print("\n✅ Split complete!")
print(f"   helpers.rs       — control creation helpers")
print(f"   types.rs         — types, structs, traits")
print(f"   notify.rs        — event notification helpers")
print(f"   platform_impl.rs — impl Platform (the big block)")
print(f"   tests.rs         — tests")
print(f"   mod.rs           — module declarations")
