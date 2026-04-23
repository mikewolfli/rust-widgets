#!/usr/bin/env python3
"""
Split src/platform/macos_objc2.rs (896 lines) into platform/macos_objc2/ sub-modules.
Structure:
  types.rs: structs, enums, types, helper impls
  platform_impl.rs: impl Platform for MacOSObjc2Platform  
  tests.rs: mod tests { ... }
  mod.rs: re-exports
"""

import os

WORKSPACE = "/Users/mikewolfli/Desktop/workspace/rust-widgets"
SRC = os.path.join(WORKSPACE, "src/platform/macos_objc2.rs")
DIR = os.path.join(WORKSPACE, "src/platform/macos_objc2")

# Create directory
os.makedirs(DIR, exist_ok=True)

with open(SRC, "r", encoding="utf-8") as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Line ranges (0-indexed):
# 0-121:    types + impl MacOSObjc2Platform + helper impls
# 121-619:  impl Platform for MacOSObjc2Platform
# 619-896:  mod tests

types_lines = lines[0:121]
platform_impl_lines = lines[121:619]
tests_lines = lines[619:896]

# Check braces balance
def count_braces(lines):
    open_br = sum(l.count('{') for l in lines)
    close_br = sum(l.count('}') for l in lines)
    return open_br, close_br

to, tc = count_braces(types_lines)
print(f"types.rs: {len(types_lines)} lines, braces {{ {to} }} {tc}, diff={to-tc}")
po, pc = count_braces(platform_impl_lines)
print(f"platform_impl.rs: {len(platform_impl_lines)} lines, braces {{ {po} }} {pc}, diff={po-pc}")
tto, ttc = count_braces(tests_lines)
print(f"tests.rs: {len(tests_lines)} lines, braces {{ {tto} }} {ttc}, diff={tto-ttc}")

# Write files
with open(os.path.join(DIR, "types.rs"), "w", encoding="utf-8") as f:
    f.writelines(types_lines)

with open(os.path.join(DIR, "platform_impl.rs"), "w", encoding="utf-8") as f:
    f.writelines(platform_impl_lines)

with open(os.path.join(DIR, "tests.rs"), "w", encoding="utf-8") as f:
    f.writelines(tests_lines)

# Write mod.rs
with open(os.path.join(DIR, "mod.rs"), "w", encoding="utf-8") as f:
    f.write("""//! macOS objc2 migration preview backend (sub-module split).
pub mod types;
pub mod platform_impl;
#[cfg(test)]
pub mod tests;

pub use types::*;
""")

print("Done!")
