#!/usr/bin/env python3
"""
Split src/platform/macos.rs (1546 lines) into a macos/ directory.
Strategy:
  - macos/mod.rs           → module declarations + re-exports
  - macos/types.rs          → MacOSPlatform struct + HandleKind + CocoaHandle +
                              helper static fns + class registration fns +
                              impl MacOSPlatform + Default impl
  - macos/platform_impl.rs  → impl Platform for MacOSPlatform (the big block)
  - macos/tests.rs          → mod tests
"""

import os
import shutil

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC_FILE = os.path.join(WORKSPACE, "src/platform/macos.rs")
BACKUP = os.path.join(WORKSPACE, "src/platform/macos.rs.bak")
MACOS_DIR = os.path.join(WORKSPACE, "src/platform/macos")

# Read original
with open(SRC_FILE, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Backup
shutil.copy2(SRC_FILE, BACKUP)
print(f"Backed up to {BACKUP}")

os.makedirs(MACOS_DIR, exist_ok=True)

# Define line ranges (0-based):
# Lines 1-2:     //! header + #![allow(deprecated)]
# Lines 3-17:    use imports
# Lines 18-51:   HandleKind enum + CocoaHandle struct
# Lines 53-67:   pub struct MacOSPlatform
# Lines 68-79:   static fns (menu_events, widget_events)
# Lines 80-148:  window_delegate_class()
# Lines 149-166: application_delegate_class()
# Lines 167-174: menu_delegate_class()
# Lines 175-200: menu_target_class()
# Lines 201-219: shared_menu_target()
# Lines 220-224: button_target_class()
# Lines 225-249: shared_button_target() + parse_shortcut()
# Lines 250-262: impl MacOSPlatform (setup_shortcut)
# Lines 263-267: impl Default for MacOSPlatform
# Lines 268-358: impl MacOSPlatform (various)
# Lines 359-1496: impl Platform for MacOSPlatform (1138 lines)
# Lines 1498-1546: mod tests

# ============================================================
# 1. types.rs — struct, enum, static helpers, class registration,
#               impl MacOSPlatform blocks, Default impl
# ============================================================
types_lines = []
types_lines.append("//! macOS platform types, structs, enums, and helper functions.\n")
types_lines.append("\n")
types_lines.append("#![allow(deprecated)]\n")
types_lines.append("\n")
# use imports (lines 3-17, 0-indexed 2-16)
types_lines.extend(lines[2:17])
types_lines.append("\n")
# HandleKind enum + CocoaHandle (lines 18-52, 0-indexed 17-51)
types_lines.extend(lines[17:52])
types_lines.append("\n")
# MacOSPlatform struct (lines 53-67, 0-indexed 52-66)
types_lines.extend(lines[52:67])
types_lines.append("\n")
# Static fns (lines 68-79, 0-indexed 67-78)
types_lines.extend(lines[67:79])
types_lines.append("\n")
# Class registration fns (lines 80-249, 0-indexed 79-248)
types_lines.extend(lines[79:249])
types_lines.append("\n")
# impl MacOSPlatform (lines 250-262, 0-indexed 249-261)
types_lines.extend(lines[249:262])
types_lines.append("\n")
# impl Default (lines 263-267, 0-indexed 262-266)
types_lines.extend(lines[262:267])
types_lines.append("\n")
# impl MacOSPlatform (lines 268-358, 0-indexed 267-357)
types_lines.extend(lines[267:358])
types_lines.append("\n")

with open(os.path.join(MACOS_DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(types_lines)
print(f"types.rs: {len(types_lines)} lines")

# ============================================================
# 2. platform_impl.rs — impl Platform for MacOSPlatform
# ============================================================
impl_lines = []
impl_lines.append("//! `impl Platform for MacOSPlatform` — the main trait implementation.\n")
impl_lines.append("\n")
impl_lines.append("#![allow(deprecated)]\n")
impl_lines.append("\n")
# Import types from our new module
impl_lines.append("use crate::platform::macos::types::*;\n")
impl_lines.append("use cocoa::appkit::{\n")
impl_lines.append("    NSApp, NSApplication, NSApplicationActivationPolicyRegular,\n")
impl_lines.append("    NSBackingStoreBuffered, NSBezelStyle, NSButton, NSControl,\n")
impl_lines.append("    NSTextField, NSView, NSWindow, NSWindowStyleMask,\n")
impl_lines.append("};\n")
impl_lines.append("use cocoa::base::{id, nil, NO, YES};\n")
impl_lines.append("use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};\n")
impl_lines.append("use objc::{class, msg_send, sel, sel_impl};\n")
impl_lines.append("use crate::core::{ObjectId, PlatformFamily};\n")
impl_lines.append("use super::state::BackendState;\n")
impl_lines.append("use super::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};\n")
impl_lines.append("\n")
# impl Platform block (lines 359-1496, 0-indexed 358-1495)
impl_lines.extend(lines[358:1496])
impl_lines.append("\n")

with open(os.path.join(MACOS_DIR, "platform_impl.rs"), "w", encoding="utf-8") as f:
    f.writelines(impl_lines)
print(f"platform_impl.rs: {len(impl_lines)} lines")

# ============================================================
# 3. tests.rs — test module
# ============================================================
tests_lines = []
tests_lines.append("//! macOS platform tests.\n")
tests_lines.append("\n")
tests_lines.append("#![allow(deprecated)]\n")
tests_lines.append("\n")
# mod tests (lines 1498-1546, 0-indexed 1497-1545)
tests_lines.extend(lines[1497:1546])

with open(os.path.join(MACOS_DIR, "tests.rs"), "w", encoding="utf-8") as f:
    f.writelines(tests_lines)
print(f"tests.rs: {len(tests_lines)} lines")

# ============================================================
# 4. mod.rs — module declarations + re-exports
# ============================================================
mod_lines = []
mod_lines.append("//! macOS platform backend implementation using Cocoa.\n")
mod_lines.append("\n")
mod_lines.append("pub mod types;\n")
mod_lines.append("mod platform_impl;\n")
mod_lines.append("\n")
mod_lines.append("pub use crate::platform::macos::types::*;\n")
mod_lines.append("\n")
mod_lines.append("#[cfg(test)]\n")
mod_lines.append("mod tests;\n")

with open(os.path.join(MACOS_DIR, "mod.rs"), "w", encoding="utf-8") as f:
    f.writelines(mod_lines)
print(f"mod.rs: {len(mod_lines)} lines")

# ============================================================
# 5. Remove old macos.rs
# ============================================================
os.remove(SRC_FILE)
print(f"Removed {SRC_FILE}")

print("\n✅ Split complete!")
print(f"   types.rs         — types, structs, class registrations, impl blocks ({len(types_lines)} lines)")
print(f"   platform_impl.rs — impl Platform ({len(impl_lines)} lines)")
print(f"   tests.rs         — tests ({len(tests_lines)} lines)")
print(f"   mod.rs           — module declarations ({len(mod_lines)} lines)")
