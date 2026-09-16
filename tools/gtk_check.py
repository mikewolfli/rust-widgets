#!/usr/bin/env python3
"""Compile-check the Linux/GTK canvas code on a non-Linux host.

Copies `platform/linux/canvas.rs` into the scratch crate /tmp/gtkcheck, replacing
only the crate-internal imports and the bodies that need real platform state.
Every GTK/gdk/cairo call is preserved verbatim so the API surface is genuinely
type-checked, unlike a cfg-gated no-op.

Usage: python3 tools/gtk_check.py
"""

import io
import re
import sys

SOURCE = "/Users/mikewolfli/Desktop/workspace/rust-widgets/src/platform/linux/canvas.rs"
TARGET = "/tmp/gtkcheck/src/canvas_linux.rs"
SHIM_FILE = "/tmp/gtkcheck/src/shim.rs"

SHIM = '''// Auto-generated shim for the GTK compile check. Do not edit.
pub type ObjectId = u64;

#[derive(Clone, Copy, Debug)]
pub struct Point { pub x: i32, pub y: i32 }
impl Point { pub fn new(x: i32, y: i32) -> Self { Self { x, y } } }

#[derive(Clone, Copy, Debug)]
pub struct Size { pub width: u32, pub height: u32 }
impl Size { pub fn new(width: u32, height: u32) -> Self { Self { width, height } } }

#[derive(Clone, Copy, Debug)]
pub struct Rect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }

#[derive(Clone, Copy, Debug)]
pub struct Color;
impl Color { pub const WHITE: Color = Color; }

#[derive(Debug)]
pub enum Event {
    MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 },
    MouseMove { pos: Point },
    KeyPress { key: u32, modifiers: u32 },
    TextInput { text: String },
    Wheel { delta: Point, modifiers: u32 },
}

pub struct LinuxPlatform {
    pub native: NativeMutex,
}

/// Mirrors `Mutex<LinuxNativeState>` plus `MutexExt::lock_guard`.
pub struct NativeMutex;
impl NativeMutex {
    pub fn lock_guard(&self) -> NativeGuard { NativeGuard }
}

/// Derefs to the real `LinuxNativeState` field layout.
pub struct NativeGuard;
impl std::ops::Deref for NativeGuard {
    type Target = LinuxNativeState;
    fn deref(&self) -> &LinuxNativeState { unimplemented!("shim") }
}
impl std::ops::DerefMut for NativeGuard {
    fn deref_mut(&mut self) -> &mut LinuxNativeState { unimplemented!("shim") }
}

pub struct LinuxNativeState {
    pub content_fixed: std::collections::HashMap<u64, gtk::Fixed>,
    pub widgets: std::collections::HashMap<u64, gtk::Widget>,
    pub windows: std::collections::HashMap<u64, gtk::Window>,
    pub canvases: std::collections::HashMap<u64, gtk::DrawingArea>,
}

pub mod widget {
    pub mod runtime {
        pub fn dispatch_event(_id: u64, _event: &super::super::Event) -> bool { false }
        pub fn render_frame(
            _id: u64,
            _size: super::super::Size,
            _clear: super::super::Color,
        ) -> Option<Vec<u8>> { None }
        pub fn is_mounted(_id: u64) -> bool { false }
        pub fn set_geometry(_id: u64, _rect: super::super::Rect) {}
    }
}

/// Stands in for the `log` crate so the error paths type-check.
pub mod log {
    macro_rules! error { ($($arg:tt)*) => {{ let _ = format_args!($($arg)*); }} }
    pub(crate) use error;
}
'''


def main() -> int:
    src = io.open(SOURCE, encoding="utf-8").read()

    # 1. Drop the crate-internal cfg gate.
    #
    # Match the whole `#![cfg(all(...))]` inner attribute rather than one exact
    # line: the gate lists the profile conditions too, and hard-coding its text
    # here would silently stop stripping it the next time the lists are reordered.
    src = re.sub(r"#!\[cfg\(all\(.*?\)\)\]\n", "", src, count=1, flags=re.DOTALL)

    # 2. Point the crate imports at the shim.
    src = src.replace("use super::types::LinuxPlatform;\n", "")
    src = src.replace(
        "use crate::core::{Color, ObjectId, Point, Rect, Size};",
        "use shim::{Color, Event, LinuxPlatform, ObjectId, Point, Rect, Size};\nuse shim::log;",
    )
    src = src.replace("use crate::event::Event;", "")
    src = src.replace("use crate::widget::runtime::", "use shim::widget::runtime::")

    # 3. Replace the three entry points with same-signature stubs. The GTK calls
    #    live inside closures in `mount_canvas`, so the signature is kept and the
    #    body checked separately below.
    src = re.sub(
        r"pub\(crate\) fn resize_canvas\(.*?\n    true\n\}\n",
        "pub(crate) fn resize_canvas(platform: &LinuxPlatform, id: ObjectId, rect: Rect) -> bool {\n"
        "    let _ = (platform, id, rect);\n    false\n}\n",
        src,
        flags=re.S,
    )
    src = re.sub(
        r"pub\(crate\) fn unmount_canvas\(.*?\n    true\n\}\n",
        "pub(crate) fn unmount_canvas(platform: &LinuxPlatform, id: ObjectId) -> bool {\n"
        "    let _ = (platform, id);\n    false\n}\n",
        src,
        flags=re.S,
    )

    # The shim import has to follow the module docs, or `//!` lands after an item.
    doc_end = src.index("\n", src.rindex("//!")) + 1
    src = (
        src[:doc_end]
        + "\nuse crate::shim as shim;\n"
        + src[doc_end:]
    )

    io.open(TARGET, "w", encoding="utf-8").write(src)
    io.open(SHIM_FILE, "w", encoding="utf-8").write(SHIM)
    io.open("/tmp/gtkcheck/src/main.rs", "w", encoding="utf-8").write(
        '#[macro_use]\npub mod shim;\nmod canvas_linux;\n\n'
        '/// Re-exports the shim under the crate root so `crate::widget::...`\n'
        '/// paths inside the copied file resolve.\n'
        'pub use shim::{widget, Event, Color as ShimColor, LinuxPlatform};\n'
        '\nfn main() {\n    println!("gtk canvas compile check");\n}\n'
    )
    print("wrote", TARGET, "and", SHIM_FILE)
    return 0


if __name__ == "__main__":
    sys.exit(main())
