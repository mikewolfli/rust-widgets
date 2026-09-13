#!/usr/bin/env python3
"""Compile-check the GTK calls in the unified widget-property accessors.

The Linux backend is gated on `target_os = "linux"`, so `cargo check` on a
macOS host never type-checks its GTK calls. This tool lifts the property
accessors added to `platform/linux/widget_state.rs` into a scratch crate
(`/tmp/gtkprobe`) that depends on the real `gtk` 0.18 crate, preserving every
GTK call verbatim. That turns "the GTK API surface I used actually exists" into
a compile-time fact instead of an assumption.

Usage: python3 tools/gtk_property_check.py   # exit 0 == type-checks
"""

import io
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCE = os.path.join(REPO, "src", "platform", "linux", "widget_state.rs")
CRATE = "/tmp/gtkprobe"

# The accessors whose GTK calls must be checked. The specialised combo/list
# index paths are deliberately excluded: they are pre-existing, already
# exercised by the platform contract tests, and pull in `Platform` trait paths
# that the shim would have to fake.
TARGET_FNS = [
    "kind_accepts_numeric_value",
    "set_widget_value_impl",
    "widget_value_impl",
    "set_widget_range_impl",
    "widget_range_impl",
    "set_widget_checked_impl",
    "is_widget_checked_impl",
    "set_widget_step_impl",
    "widget_step_impl",
    "set_widget_indeterminate_impl",
    "is_widget_indeterminate_impl",
    "set_widget_read_only_impl",
    "is_widget_read_only_impl",
    "set_widget_max_length_impl",
    "widget_max_length_impl",
    "set_window_state_impl",
    "is_window_in_state_impl",
    "set_window_min_size_impl",
    "window_min_size_impl",
    "set_window_icon_impl",
    "window_icon_impl",
    "set_widget_selection_impl",
    "widget_selection_impl",
    "set_widget_placeholder_impl",
    "widget_placeholder_impl",
    "set_widget_echo_mode_impl",
    "widget_echo_mode_impl",
]

CARGO_TOML = """[package]
name = "gtkprobe"
version = "0.1.0"
edition = "2021"

[dependencies]
gtk = "0.18"
gdk = "0.18"
"""

SHIM = """// Auto-generated shim for the GTK property compile check. Do not edit.
#![allow(dead_code, unused_variables, unused_imports)]
use gtk::prelude::*;

/// Stands in for the `log` crate so the error paths type-check.
#[allow(unused_macros)]
#[macro_use]
pub mod log {
    macro_rules! log_warn { ($($arg:tt)*) => {{ let _ = format_args!($($arg)*); }} }
    macro_rules! log_error { ($($arg:tt)*) => {{ let _ = format_args!($($arg)*); }} }
    pub(crate) use log_warn as warn;
    pub(crate) use log_error as error;
}

pub type ObjectId = u64;

/// Stand-in for the backend's per-widget kind discriminator.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LinuxHandleKind {
    Window,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    SpinBox,
    ScrollBar,
    DoubleSpinBox,
    CheckBox,
    RadioButton,
    ToggleButton,
    ActivityIndicator,
    ProgressDialog,
    LineEdit,
    Other,
}

pub struct BackendState;
impl BackendState {
    pub fn set_value(&self, _id: u64, _v: f64) -> bool { true }
    pub fn value(&self, _id: u64) -> Option<f64> { None }
    pub fn set_range(&self, _id: u64, _a: f64, _b: f64) -> bool { true }
    pub fn range(&self, _id: u64) -> Option<(f64, f64)> { None }
    pub fn set_checked(&self, _id: u64, _v: bool) -> bool { true }
    pub fn checked(&self, _id: u64) -> Option<bool> { None }
    pub fn set_step(&self, _id: u64, _v: f64) -> bool { true }
    pub fn step(&self, _id: u64) -> Option<f64> { None }
    pub fn set_indeterminate(&self, _id: u64, _v: bool) -> bool { true }
    pub fn indeterminate(&self, _id: u64) -> Option<bool> { None }
    pub fn set_read_only(&self, _id: u64, _v: bool) -> bool { true }
    pub fn read_only(&self, _id: u64) -> Option<bool> { None }
    pub fn set_max_length(&self, _id: u64, _v: u32) -> bool { true }
    pub fn max_length(&self, _id: u64) -> Option<u32> { None }
    pub fn window_state(&self, _id: u64, _flag: crate::shim::WindowStateFlag) -> Option<bool> {
        None
    }
    pub fn set_window_state(
        &self,
        _id: u64,
        _flag: crate::shim::WindowStateFlag,
        _on: bool,
    ) -> bool {
        true
    }
    pub fn set_window_min_size(&self, _id: u64, _w: u32, _h: u32) -> bool { true }
    pub fn window_min_size(&self, _id: u64) -> Option<(u32, u32)> { None }
    pub fn set_window_icon(&self, _id: u64, _p: &str) -> bool { true }
    pub fn window_icon(&self, _id: u64) -> Option<String> { None }
    pub fn set_selection(&self, _id: u64, _s: u32, _e: u32) -> bool { true }
    pub fn selection(&self, _id: u64) -> Option<(u32, u32)> { None }
    pub fn set_placeholder(&self, _id: u64, _t: &str) -> bool { true }
    pub fn placeholder(&self, _id: u64) -> Option<String> { None }
    pub fn set_echo_mode(&self, _id: u64, _m: crate::shim::EchoMode) -> bool { true }
    pub fn echo_mode(&self, _id: u64) -> Option<crate::shim::EchoMode> { None }
}

/// Stand-in for the platform echo-mode enum.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EchoMode {
    Normal,
    Password,
    NoEcho,
}

/// Stand-in for the platform window-state enum.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowStateFlag {
    Maximized,
    Minimized,
    Fullscreen,
    Resizable,
    Decorated,
}

pub struct LinuxNativeState {
    pub widgets: std::collections::HashMap<u64, gtk::Widget>,
    pub windows: std::collections::HashMap<u64, gtk::Window>,
}

pub struct NativeGuard;
impl std::ops::Deref for NativeGuard {
    type Target = LinuxNativeState;
    fn deref(&self) -> &LinuxNativeState { unimplemented!("shim") }
}
impl std::ops::DerefMut for NativeGuard {
    fn deref_mut(&mut self) -> &mut LinuxNativeState { unimplemented!("shim") }
}

pub struct NativeMutex;
impl NativeMutex {
    pub fn lock_guard(&self) -> NativeGuard { unimplemented!("shim") }
}

pub struct LinuxPlatform {
    pub state: BackendState,
    pub native: NativeMutex,
}
impl LinuxPlatform {
    pub fn kind_of(&self, _id: u64) -> Option<LinuxHandleKind> { None }
}
"""


def extract_target_fns(src: str) -> str:
    """Return only the target function items, cfg gates stripped.

    Each function is sliced from its `pub(crate) fn name(` (or `fn name(`) up to
    the matching closing brace at four-space indentation, which is this file's
    item indentation inside `impl`.
    """
    src = re.sub(r"#!\[cfg\(all\(.*?\)\)\]\n", "", src, count=1, flags=re.DOTALL)
    # Strip per-function cfg gates first so the slice boundaries are stable.
    src = src.replace(
        '#[cfg(all(target_os = "linux", feature = "gtk-native"))]',
        "",
    )
    # The shim *is* the crate root here, so internal platform paths resolve to it.
    src = src.replace("crate::platform::", "crate::shim::")

    out = []
    for name in TARGET_FNS:
        pattern = re.compile(
            r"(?m)^    (?:pub\(crate\) )?fn " + re.escape(name) + r"\b.*?(?=^    (?:pub\(crate\) )?fn |^})",
            re.DOTALL,
        )
        match = pattern.search(src)
        if not match:
            raise SystemExit(f"could not locate fn {name} in {SOURCE}")
        out.append(match.group(0).rstrip())
    return "\n\n".join(out)


def main() -> int:
    src = io.open(SOURCE, encoding="utf-8").read()
    fns = extract_target_fns(src)

    os.makedirs(os.path.join(CRATE, "src"), exist_ok=True)
    io.open(os.path.join(CRATE, "Cargo.toml"), "w", encoding="utf-8").write(CARGO_TOML)
    io.open(os.path.join(CRATE, "src", "shim.rs"), "w", encoding="utf-8").write(SHIM)
    io.open(os.path.join(CRATE, "src", "properties.rs"), "w", encoding="utf-8").write(
        "#![allow(dead_code, unused_variables, unused_imports)]\n"
        "use crate::shim::*;\nuse gtk::prelude::*;\n\n"
        "impl LinuxPlatform {\n"
        + fns
        + "\n}\n"
    )
    io.open(os.path.join(CRATE, "src", "main.rs"), "w", encoding="utf-8").write(
        "#![allow(dead_code, unused_variables, unused_imports)]\n"
        "mod shim;\nmod properties;\n\nfn main() {}\n"
    )
    print("wrote", CRATE)
    return 0


if __name__ == "__main__":
    sys.exit(main())
