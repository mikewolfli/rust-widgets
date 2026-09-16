#!/usr/bin/env python3
"""
Split src/platform/linux.rs (1010 lines) into platform/linux/ sub-modules.
Structure:
  types.rs: structs, enums, types (LinuxHandleKind, LinuxMenuState, LinuxRuntimeState, LinuxNativeState)
  platform_impl.rs: impl Platform for LinuxPlatform
  mod.rs: re-exports
"""

import os, shutil

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = os.path.join(WORKSPACE, "src/platform/linux.rs")
DIR = os.path.join(WORKSPACE, "src/platform/linux")
OLD_MOD = os.path.join(DIR, "mod.rs")

# Create linux/ directory
os.makedirs(DIR, exist_ok=True)

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Line ranges (0-indexed):
# 0-8:    module doc + imports
# 9-21:   enum LinuxHandleKind
# 22-36:  struct LinuxMenuState
# 37-53:  struct LinuxRuntimeState + impl LinuxRuntimeState  
# 54-70:  struct LinuxPlatform + struct LinuxNativeState (cfg)
# 71-101: impl LinuxPlatform (new, Default) + insert_widget, kind_of
# 102-1010: impl Platform for LinuxPlatform

# types.rs: lines 0-112 (all structs, enums, impl LinuxRuntimeState, impl LinuxPlatform)
types_lines = lines[0:112]

# platform_impl.rs: lines 112-1010 (impl Platform for LinuxPlatform)
platform_impl_lines = lines[112:1010]

# Check braces balance
def count_braces(lines):
    open_br = sum(l.count('{') for l in lines)
    close_br = sum(l.count('}') for l in lines)
    return open_br, close_br

to, tc = count_braces(types_lines)
print(f"types.rs: {len(types_lines)} lines, braces {{ {to} }} {tc}, diff={to-tc}")
po, pc = count_braces(platform_impl_lines)
print(f"platform_impl.rs: {len(platform_impl_lines)} lines, braces {{ {po} }} {pc}, diff={po-pc}")

# Write types.rs
with open(os.path.join(DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(types_lines)

# Write platform_impl.rs
with open(os.path.join(DIR, "platform_impl.rs"), "w", encoding="utf-8") as f:
    f.writelines(platform_impl_lines)

# Write mod.rs
with open(os.path.join(DIR, "mod.rs"), "w", encoding="utf-8") as f:
    f.write("""//! Linux backend platform (sub-module split).
pub mod types;
pub mod platform_impl;

pub use types::*;
pub use platform_impl::*;
""")

print("Done! Files written to:", DIR)
print("types.rs:", len(types_lines), "lines")
print("platform_impl.rs:", len(platform_impl_lines), "lines")

# Check total
to2, tc2 = count_brakes(platform_impl_lines)
