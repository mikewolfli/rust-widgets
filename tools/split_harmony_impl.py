#!/usr/bin/env python3
"""
Split src/platform/harmony.rs (507 lines) into platform/harmony/ sub-modules.
"""

import os

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = os.path.join(WORKSPACE, "src/platform/harmony.rs")
DIR = os.path.join(WORKSPACE, "src/platform/harmony")

os.makedirs(DIR, exist_ok=True)

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# 0-indexed:
# lines 0-89: types + helper impls
# lines 89-507: impl Platform for HarmonyPlatform (89-90 overlap just the blank line)
types_lines = lines[0:89]
platform_impl_lines = lines[89:507]

def count_braces(lines):
    ob = sum(l.count('{') for l in lines)
    cb = sum(l.count('}') for l in lines)
    return ob, cb

to, tc = count_braces(types_lines)
print(f"types.rs: {len(types_lines)} lines, braces {{ {to} }} {tc}, diff={to-tc}")
po, pc = count_braces(platform_impl_lines)
print(f"platform_impl.rs: {len(platform_impl_lines)} lines, braces {{ {po} }} {pc}, diff={po-pc}")

with open(os.path.join(DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(types_lines)
with open(os.path.join(DIR, "platform_impl.rs"), "w", encoding="utf-8") as f:
    f.writelines(platform_impl_lines)

with open(os.path.join(DIR, "mod.rs"), "w", encoding="utf-8") as f:
    f.write("""//! Harmony desktop backend shell (sub-module split).
pub mod types;
pub mod platform_impl;

pub use types::*;
""")

print("Done!")
