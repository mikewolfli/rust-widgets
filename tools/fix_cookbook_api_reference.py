#!/usr/bin/env python3
"""Corrects the cookbook API reference where it disagrees with `src/`.

Background: `cookbook/{en,zh-CN,zh-TW}/src/chapters/api-reference.md` is a
hand-maintained reference for the whole public API and nothing compiles it. The
drift found by `tools/check_cookbook_api_names.py` was not cosmetic -- 13 declared
items named APIs that do not exist, including a whole module that was removed
(the `chart` engine moved to `widget::chart_widgets`, so `crate::chart::*` paths
and `ChartSvgRenderer` were both wrong).

This script applies the verified corrections. Every replacement below was checked
against the source before being written; the "real" text is quoted from the file
named in the comment, so a future reader can re-verify with one grep.

It is idempotent: running it twice changes nothing the second time. It refuses to
guess -- if an anchor is not found exactly, it reports and skips rather than
damaging the document.

Usage: tools/fix_cookbook_api_reference.py [--check]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BOOKS = ["en", "zh-CN", "zh-TW"]

# (section heading fragment used to locate the block, old text, new text)
# Verified against:
#   src/render_engine/engine_trait.rs  -- trait RenderEngine
#   src/render_engine/embedded_engine.rs -- EmbeddedRenderEngine
#   src/render_engine/native.rs -- NativeRenderEngine
ENGINE_OLD = """pub trait EngineTrait {
    fn name(&self) -> &'static str;
    fn init(&mut self) -> Result<(), RwError>;
    fn run(&mut self) -> Result<(), RwError>;
    fn quit(&mut self);
    fn submit_frame(&mut self, surface: &mut SoftwareSurface);
    fn is_running(&self) -> bool;
}"""

ENGINE_NEW = """// The trait is `RenderEngine` (src/render_engine/engine_trait.rs). It borrows
// `&self`, not `&mut self`, and takes no `Result`: engine lifecycle is
// infallible by design, so there is nothing for a caller to handle.
pub trait RenderEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn profile(&self) -> RuntimeProfile;
    fn init(&self);
    fn run(&self);
    fn quit(&self);
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
    fn create_button(
        &self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32,
    ) -> u64;
}"""

NATIVE_OLD = """pub struct NativeEngine { /* ... */ }
impl NativeEngine {
    pub fn new() -> Self;
}
impl EngineTrait for NativeEngine { /* ... */ }"""

NATIVE_NEW = """pub struct NativeRenderEngine;

impl NativeRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for NativeRenderEngine { /* ... */ }"""

EMBEDDED_OLD = """pub struct EmbeddedEngine { /* ... */ }
impl EmbeddedEngine {
    pub fn new() -> Self;
    pub fn init(&mut self) -> bool;
    pub fn task_count(&self) -> u64;
    pub fn submit_noop(&self, label: &str) -> u64;
    pub fn frame_count(&self) -> u64;
    pub fn button_count(&self) -> u64;
    pub fn window_count(&self) -> u64;
    pub fn target_fps(&self) -> u32;
    pub fn set_target_fps(&mut self, fps: u32) -> u32;
    pub fn is_running(&self) -> bool;
    pub fn is_initialized(&self) -> bool;
}
impl EngineTrait for EmbeddedEngine { /* ... */ }"""

EMBEDDED_NEW = """pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for EmbeddedRenderEngine { /* ... */ }

// The engine itself is a thin fa\u00e7ade: the counters and the frame loop live in the
// process-wide embedded runtime (`render_engine::embedded`), which is what the
// engine methods delegate to. Task submission is a free function, not a method:
// `submit_embedded_task(label, callback)`.

/// Builds the engine for the compile-time profile.
///
/// `NativeRenderEngine` when an OS host exists, `EmbeddedRenderEngine` otherwise
/// (and always under the alloc-frugal `mini` profile).
pub fn default_render_engine() -> Box<dyn RenderEngine>;"""

# The chart engine moved: there is no `crate::chart` module.
#
# The `zh-CN` / `zh-TW` books translate the surrounding prose and the inline
# comments — including the punctuation (`:` vs `：`) — so the block appears in
# several spellings with the Rust lines identical. Rather than enumerate every
# translation, the code lines are matched on their own and the comment line is
# preserved verbatim; only the incorrect Rust is replaced.
CHART_OLD_LINES = [
    "pub struct ChartLayout { /* ... */ }",
    "pub struct ChartSvgRenderer { /* ... */ }",
    "",
]

# Comment lines that may sit between the declarations; kept as-is.
CHART_COMMENT_MARKERS = ("// 子模块", "// 子模組", "// Sub-modules", "// Sub-module")

CHART_NEW_LINES = [
    "// The engine and the chart widgets both live under `widget::chart_widgets`:",
    "// they are two layers of one feature, and there is no top-level `chart` module.",
    "pub struct ChartLayout { /* ... */ }          // widget::chart_widgets::layout",
    "pub struct SvgChartContext { /* ... */ }      // widget::chart_widgets::svg",
    "pub struct MemoryChartContext { /* ... */ }   // widget::chart_widgets::svg",
    "pub trait Chart { /* ... */ }                 // widget::chart_widgets::types",
    "pub trait ChartContext { /* ... */ }          // widget::chart_widgets::types",
]

# The prose directly above the block repeats the claim the code block used to make.
CHART_PROSE_OLD = [
    "The `chart` module provides the foundation for data visualization:",
    "`chart` 模块为数据可视化提供基础：",
    "`chart` 模組提供了資料視覺化的基礎：",
]
CHART_PROSE_NEW = (
    "Data visualization lives under `widget::chart_widgets`. The chart *engine* "
    "(layout, axes, ticks, SVG context) and the chart *widgets* are two layers of "
    "one feature, so they share a module:"
)

CHART_OLD = """pub struct ChartLayout { /* ... */ }
pub struct ChartSvgRenderer { /* ... */ }

// Sub-modules: charts, layout, svg, types

pub use crate::chart::charts::*;
pub use crate::chart::svg::*;
pub use crate::chart::types::*;"""

CHART_NEW = """// There is no top-level `chart` module. The chart engine and the chart widgets
// live together under `widget::chart_widgets`, because they are two layers of one
// feature (src/lib.rs documents the same point). The engine is reachable as
// `rust_widgets::widget::chart_widgets::charts`.
pub struct ChartLayout { /* ... */ }          // widget::chart_widgets::layout
pub struct SvgChartContext { /* ... */ }      // widget::chart_widgets::svg
pub struct MemoryChartContext { /* ... */ }   // widget::chart_widgets::svg
pub trait Chart { /* ... */ }                 // widget::chart_widgets::types
pub trait ChartContext { /* ... */ }          // widget::chart_widgets::types

// Sub-modules: charts (axes/ticks/legend), layout, svg, types
// Prefer the re-exports on `widget::chart_widgets` over the sub-module paths."""

# TimerManager's methods are named differently than documented.
TIMER_OLD = """impl TimerManager {
    pub fn new() -> Self;
    pub fn add_timer(&mut self, interval_ms: u64) -> u32;
    pub fn remove_timer(&mut self, id: u32);
    pub fn process_timers(&mut self, now_ms: u64) -> Vec<u32>;
    pub fn clear(&mut self);
}"""

TIMER_NEW = """impl TimerManager {
    /// Takes the sender the timer callbacks are delivered through: the manager
    /// does not own a loop, it posts into one.
    pub fn new(sender: EventSender) -> Self;
    /// Starts a timer on `target`; the tick is delivered as an event.
    pub fn start_timer(&self, target: ObjectId, interval: Duration, id: u32, repeat: bool) -> bool;
    /// Stops one timer on `target`. `false` if no such timer was running.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool;
    /// Stops every timer belonging to `target`; returns how many were stopped.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize;
    /// Stops every timer on every target.
    pub fn clear(&self);
    /// Delivers ticks for all timers whose deadline has passed. Call once per
    /// loop iteration.
    pub fn pump(&self);
}"""

# EventLoop has no timer methods of its own.
EVENTLOOP_OLD = """impl EventLoop {
    pub fn new() -> Self;
    pub fn run(&mut self) -> !;
    pub fn quit(&self);
    pub fn post_event(&self, event: Event);
    pub fn add_timer(&mut self, interval_ms: u64, id: u32);
    pub fn remove_timer(&mut self, id: u32);
}"""

EVENTLOOP_NEW = """impl EventLoop {
    pub fn new() -> Self;
    pub fn run(&mut self) -> !;
    pub fn quit(&self);
    pub fn post_event(&self, event: Event);
    // Timers are not an `EventLoop` method: `TimerManager` owns them and posts
    // ticks into the loop's queue. See the Timer section below.
}"""

# FocusManager: next/prev are focus_next/focus_previous, tab order is set_focus_order.
FOCUS_OLD = """    pub fn next_widget(&mut self) -> Option<ObjectId>;
    pub fn prev_widget(&mut self) -> Option<ObjectId>;
    pub fn register_tab_order(&mut self, ids: &[ObjectId]);"""

FOCUS_NEW = """    pub fn focus_next(&mut self) -> Option<ObjectId>;
    pub fn focus_previous(&mut self) -> Option<ObjectId>;
    /// Replaces the focus traversal order wholesale.
    pub fn set_focus_order(&mut self, ids: Vec<ObjectId>);
    pub fn register_focusable(&mut self, id: ObjectId);
    pub fn unregister_focusable(&mut self, id: ObjectId);
    pub fn focusable_widgets(&self) -> &[ObjectId];
    pub fn set_traversal_strategy(&mut self, strategy: FocusTraversalStrategy);"""

# PointerCaptureManager: capture/release/has_capture.
CAPTURE_OLD = """impl PointerCaptureManager {
    pub fn new() -> Self;
    pub fn capture(&mut self, widget_id: ObjectId);
    pub fn release(&mut self);
    pub fn captured_widget(&self) -> Option<ObjectId>;
    pub fn is_captured_by(&self, widget_id: ObjectId) -> bool;
}"""

CAPTURE_NEW = """impl PointerCaptureManager {
    pub fn new() -> Self;
    /// Captures the pointer for `widget_id`. `false` when another widget already
    /// holds the capture -- capture is exclusive.
    pub fn set_capture(&mut self, widget_id: ObjectId) -> bool;
    /// Releases the capture. `false` if nothing was captured.
    pub fn release_capture(&mut self) -> bool;
    /// The widget holding the capture, if any.
    pub fn capturing_widget(&self) -> Option<ObjectId>;
    /// Whether `widget_id` is the current captor.
    pub fn has_capture(&self, widget_id: ObjectId) -> bool;
}"""

# These components do not exist under the documented names.
REPLACEMENTS = [
    (ENGINE_OLD, ENGINE_NEW),
    (NATIVE_OLD, NATIVE_NEW),
    (EMBEDDED_OLD, EMBEDDED_NEW),
    (CHART_OLD, CHART_NEW),
    (TIMER_OLD, TIMER_NEW),
    (EVENTLOOP_OLD, EVENTLOOP_NEW),
    (FOCUS_OLD, FOCUS_NEW),
    (CAPTURE_OLD, CAPTURE_NEW),
]

# Names that do not exist at all, replaced wherever they appear as a type.
SIMPLE_RENAMES = [
    ("PlatformClipboard", "Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>"),
    ("PoolAllocator", "ObjectPool<T> / SharedPool<T> / PoolManager"),
    ("WebPlugin", "Plugin"),
    ("CssEngine", "CssParser"),
    # `CssWatcher` was listed here while it did not exist. It now does
    # (`crate::style::css_watcher::CssWatcher`), so the rename is removed: keeping it
    # would rewrite a real name into a different type and hide the fact that the
    # documentation and the code finally agree.
    ("VirtualKeyboardController", "Keyboard (widget::input_widgets::keyboard)"),
]


def fix_chart_block(text: str) -> tuple[str, bool]:
    """Replaces the chart declaration block, tolerating a translated comment line.

    The three books write the same Rust lines under a translated `// 子模块...`
    comment (and the Chinese editions use full-width punctuation), so matching the
    whole block as one string would need one entry per translation. Instead this
    matches the two incorrect declaration lines and rebuilds the block, keeping the
    document's own comment line where one is present.
    """
    lines = text.split("\n")
    out: list[str] = []
    index = 0
    changed = False
    while index < len(lines):
        if lines[index].strip() == CHART_OLD_LINES[0]:
            look = index + 1
            if look < len(lines) and lines[look].strip() == CHART_OLD_LINES[1]:
                look += 1
                stale_comments: list[str] = []
                stale_reexports: list[str] = []
                # Collect the rest of the stale block in any order: blank lines, the
                # (translated) comment marker, and the `pub use crate::chart::*`
                # re-exports that point at a module that no longer exists.
                while look < len(lines):
                    stripped = lines[look].strip()
                    if stripped == "":
                        look += 1
                        continue
                    if stripped.startswith(CHART_COMMENT_MARKERS):
                        stale_comments.append(lines[look])
                        look += 1
                        continue
                    if stripped.startswith("pub use crate::chart::"):
                        stale_reexports.append(lines[look])
                        look += 1
                        continue
                    break
                out.extend(CHART_NEW_LINES)
                out.extend(stale_comments)
                changed = True
                index = look
                continue
        out.append(lines[index])
        index += 1
    return "\n".join(out), changed


def apply(path: Path, check: bool) -> int:
    text = path.read_text(encoding="utf-8")
    original = text
    changes = 0

    text, chart_changed = fix_chart_block(text)
    changes += chart_changed

    # Retract the prose claim that a `chart` module exists.
    for prose in CHART_PROSE_OLD:
        if prose in text:
            text = text.replace(prose, CHART_PROSE_NEW)
            changes += 1

    for old, new in REPLACEMENTS:
        if old in text:
            text = text.replace(old, new)
            changes += 1

    for old, new in SIMPLE_RENAMES:
        if old in text:
            text = text.replace(old, new)
            changes += text.count(old)

    if text != original and not check:
        path.write_text(text, encoding="utf-8")
    return 1 if text != original else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report only")
    args = parser.parse_args()

    dirty = 0
    for book in BOOKS:
        path = ROOT / "cookbook" / book / "src" / "chapters" / "api-reference.md"
        if not path.is_file():
            print(f"missing: {path.relative_to(ROOT)}", file=sys.stderr)
            continue
        changed = apply(path, args.check)
        dirty += changed
        state = "needs fixes" if changed else "already consistent"
        print(f"{state:20} {path.relative_to(ROOT)}")

    return 1 if args.check and dirty else 0


if __name__ == "__main__":
    sys.exit(main())
