#!/usr/bin/env python3
"""
Split src/platform/mod.rs (1494 lines) into sub-modules.

Strategy: types.rs and stub.rs ALREADY EXIST as pre-split fragments but were never
activated in mod.rs. This script:
  1. Backs up mod.rs -> mod.rs.bak3
  2. Rewrites mod.rs to use existing types.rs and stub.rs via module declarations
  3. Extracts runtime fns -> runtime.rs (NEW)
  4. Extracts contract fns -> contract.rs (NEW)
  5. Extracts tests -> tests.rs (NEW)
  6. Fixes types.rs: makes WidgetState + MenuNodeState pub(crate)
"""

from pathlib import Path

SRC = Path("src/platform/mod.rs")
BAK = Path("src/platform/mod.rs.bak3")
TYPES = Path("src/platform/types.rs")
STUB = Path("src/platform/stub.rs")
STATE = Path("src/platform/state.rs")
RUNTIME = Path("src/platform/runtime.rs")
CONTRACT = Path("src/platform/contract.rs")
TESTS = Path("src/platform/tests.rs")


def backup():
    if SRC.exists():
        content = SRC.read_text()
        BAK.write_text(content)
        print(f"Backup: {BAK} ({len(content.splitlines())} lines)")
        return True
    print(f"Source not found: {SRC}")
    return False


def main():
    if not backup():
        return

    lines = SRC.read_text().splitlines()

    # === NEW mod.rs ===
    new_mod = """//! Platform abstraction for desktop/embedded/mobile families.

// Platform backend implementations (one per target)
pub mod harmony;
pub mod linux;
#[cfg(all(target_os = "macos", not(feature = "objc2-macos")))]
pub mod macos;
#[cfg(all(target_os = "macos", feature = "objc2-macos"))]
pub mod macos_objc2;
#[cfg(feature = "mobile-api")]
pub mod mobile;
#[cfg(target_os = "windows")]
pub mod windows;

// Internal sub-modules (split from monolithic mod.rs)
mod state;
mod stub;
pub mod types;
mod runtime;
mod contract;

// Re-exports: everything that was previously defined directly in mod.rs
pub use crate::platform::types::*;
pub use crate::platform::stub::StubPlatform;
pub use crate::platform::runtime::{get_platform, init, run, quit, capabilities};
pub use crate::platform::runtime::{runtime_gui_mode, runtime_gui_mode_for, dpi_scale_factor};
pub use crate::platform::runtime::{RuntimeGuiMode, mobile_backend_name, mobile_attach_to_native_view};
pub use crate::platform::contract::{negotiate_capability_contract, CapabilityContract};
pub use crate::platform::contract::{NativeCapabilityContract, EmbeddedCapabilityContract};

#[cfg(test)]
mod tests;
"""

    # === runtime.rs ===
    # Lines 1192-1301 (0-indexed)
    runtime_parts = lines[1192:1302]
    runtime_imports = """use std::sync::OnceLock;
use crate::core::{PlatformFamily, RuntimeProfile};
pub use crate::platform::types::*;
#[cfg(target_os = "windows")]
use crate::platform::windows::WindowsPlatform;
#[cfg(all(target_os = "macos", not(feature = "objc2-macos")))]
use crate::platform::macos::MacOSPlatform;
#[cfg(all(target_os = "linux", not(feature = "embedded")))]
use crate::platform::linux::LinuxPlatform;
#[cfg(feature = "mobile-api")]
use crate::platform::mobile;

"""
    runtime_content = runtime_imports + "".join(l + "\n" for l in runtime_parts)

    # === contract.rs ===
    # Lines 1301-1334 (0-indexed)
    contract_parts = lines[1301:1335]
    contract_imports = """use crate::core::RuntimeProfile;
pub use crate::platform::types::*;

"""
    contract_content = contract_imports + "".join(l + "\n" for l in contract_parts)

    # === tests.rs ===
    # Lines 1335-end (0-indexed)
    test_parts = lines[1335:]
    test_content = "".join(l + "\n" for l in test_parts)

    # === Fix types.rs ===
    types_content = TYPES.read_text()
    types_fixed = types_content.replace(
        "struct WidgetState {",
        "pub(crate) struct WidgetState {"
    )
    types_fixed = types_fixed.replace(
        "struct MenuNodeState {",
        "pub(crate) struct MenuNodeState {"
    )

    # === Write all files ===
    for path, content in [
        (SRC, new_mod),
        (RUNTIME, runtime_content),
        (CONTRACT, contract_content),
        (TESTS, test_content),
        (TYPES, types_fixed),
    ]:
        path.write_text(content)
        n = len(content.splitlines())
        print(f"Wrote {path} ({n} lines)")

    print(f"\nSplit complete!")
    print(f"  {SRC}: was {len(lines)} lines -> now {len(new_mod.splitlines())} lines")
    print(f"  {RUNTIME}: {len(runtime_content.splitlines())} lines (new)")
    print(f"  {CONTRACT}: {len(contract_content.splitlines())} lines (new)")
    print(f"  {TESTS}: {len(test_content.splitlines())} lines (new)")
    print(f"  {TYPES}: {len(types_fixed.splitlines())} lines (fixed visibility)")
    print(f"  {STUB}: unchanged ({len(STUB.read_text().splitlines())} lines)")
    print(f"  {STATE}: unchanged ({len(STATE.read_text().splitlines())} lines)")


if __name__ == "__main__":
    main()
