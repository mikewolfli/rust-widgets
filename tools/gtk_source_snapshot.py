#!/usr/bin/env python3
"""Snapshot the Linux/GTK canvas source into a standalone, compilable-in-isolation form.

**This is a source generator, not a compile check.** It copies
`platform/linux/canvas.rs` into a throwaway scratch directory, replacing only the
crate-internal imports and the bodies that need real platform state, so a
developer can open the GTK call surface on a non-Linux host. It does **not**
invoke a compiler — the real compile check for this file is the `linux-gtk` CI job
in `.github/workflows/ci.yml`, which runs `cargo check --features desktop,gtk-native`
on a machine with GTK 3 installed. `exit 0` therefore means *the snapshot was
written*, nothing more.

The name used to be `gtk_check.py` and its README line claimed it "inspects a live
GTK widget tree", both of which overstated what it does: it never compiled and
never touched a live tree (D08-G-04). The honest name and contract are the fix.

Usage: python3 tools/gtk_source_snapshot.py [--out DIR]
Exit 0 = snapshot written; non-zero = generation failed.
"""

import argparse
import io
import os
import re
import sys
import tempfile

# Resolve the source relative to this file's repository root rather than a
# developer-specific absolute path (D08-G-04). `tools/` sits directly under it.
REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCE = os.path.join(REPO_ROOT, "src", "platform", "linux", "canvas.rs")

SHIM = '''// Auto-generated shim for the GTK source snapshot. Do not edit.
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        help="Directory to write the snapshot into (default: a fresh temp dir).",
    )
    args = parser.parse_args()

    if not os.path.isfile(SOURCE):
        print("source not found: %s" % SOURCE, file=sys.stderr)
        return 2

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

    # Write the snapshot. A unique, caller-supplied or freshly created directory
    # keeps two runs from clobbering each other; the old fixed `/tmp/gtkcheck` was
    # shared by every checkout (D08-G-04).
    out_dir = args.out or tempfile.mkdtemp(prefix="rw_gtk_snapshot_")
    os.makedirs(out_dir, exist_ok=True)
    canvas_path = os.path.join(out_dir, "canvas_linux.rs")
    shim_path = os.path.join(out_dir, "shim.rs")
    with io.open(canvas_path, "w", encoding="utf-8") as handle:
        handle.write(src)
    with io.open(shim_path, "w", encoding="utf-8") as handle:
        handle.write(SHIM)
    print("wrote", canvas_path, "and", shim_path)
    print(
        "NOTE: this is a source snapshot only; the real GTK compile check is the "
        "`linux-gtk` CI job (cargo check --features desktop,gtk-native)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
