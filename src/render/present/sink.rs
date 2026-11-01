// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The per-window presentation targets, and the one call a platform canvas makes.
//!
//! # Why a table keyed by window
//!
//! A presentation target owns a `wgpu` surface, a device and the frame mirror — all of which must
//! live as long as the window does, and must be dropped before it. The canvas paints on the
//! thread that owns the window, so a thread-local table keyed by the public window id gives each
//! window exactly one target, created the first time that window asks for one.
//!
//! # The three answers a canvas can get
//!
//! * [`WindowPresentOutcome::Presented`] — the frame is on the target; the canvas must **not** also
//!   blit it (that would draw the same frame twice through two paths).
//! * [`WindowPresentOutcome::Skipped`] — the target exists but could not take this frame (occluded,
//!   timed out, stopped). The canvas blits, so the window keeps showing the software frame.
//! * [`WindowPresentOutcome::Unavailable`] — no target can exist for this window (no adapter, no
//!   supported format, a zero-sized window). The canvas blits, and the attempt is remembered so a
//!   host does not pay a failed adapter probe on every frame.
//!
//! # What this module deliberately does not do
//!
//! It does not decide *when* to present: the caller has a frame in hand and asks for it to be
//! shown. It does not create a loop, a timer or a thread, and it does not blit.

use super::{FramePresentation, FramePresenter, PresentationStatus};
use crate::compat::{HashMap, Rc, RefCell};
use crate::core::ObjectId;
use crate::wgpu_backend::RenderQuality;
use core::mem::ManuallyDrop;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

/// What happened to a frame a canvas offered to the presentation path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPresentOutcome {
    /// The frame was handed to the compositor.
    Presented,
    /// The target exists but this frame was not taken; the canvas should blit it.
    Skipped,
    /// No target can exist for this window; the canvas should blit it.
    Unavailable,
}

impl WindowPresentOutcome {
    /// Whether the canvas still has to blit the frame itself.
    pub const fn needs_blit(self) -> bool {
        !matches!(self, Self::Presented)
    }

    /// A stable name for logs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Presented => "presented",
            Self::Skipped => "skipped",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One window's entry in the table.
enum Entry {
    /// A live target.
    Ready(Box<FramePresenter>),
    /// The target could not be created. The reason was logged once, when it failed; the entry
    /// exists so the failure is not retried on every frame.
    Unavailable,
}

thread_local! {
    /// Window id -> its entry. See the module docs for why the key is the window.
    ///
    /// # Why this table is `ManuallyDrop`
    ///
    /// The entries own `wgpu` surfaces and devices. Dropping them from a thread-local destructor
    /// aborts the process on macOS: the destructor runs after AppKit has been torn down, the Metal
    /// release path calls into it, and a panic inside a TLS destructor is a non-unwinding abort
    /// (`panic in a destructor during cleanup`). Measured, not suspected — `present_smoke`
    /// aborted with exit 134 before this attribute was added.
    ///
    /// Presenters are therefore dropped by [`forget_window`], i.e. while the window is still alive
    /// and the native view is still valid, which is the only moment the contract allows. Anything
    /// still open at process exit is reclaimed by the OS, and no destructor touches it.
    static TARGETS: ManuallyDrop<RefCell<HashMap<ObjectId, Entry>>> =
        ManuallyDrop::new(RefCell::new(HashMap::new()));

    /// How many frames each window has handed to the compositor, for diagnostics and tests.
    static PRESENTED: RefCell<HashMap<ObjectId, u64>> = RefCell::new(HashMap::new());

    /// How many of those frames went out as draw commands rather than as pixels.
    ///
    /// Separate from `PRESENTED` because it answers a different question: `PRESENTED` says "the
    /// canvas reached the compositor", this says "the `Smooth` path was the one that did". A probe
    /// that wants to prove the command path is live reads this one (B29.10).
    static PRESENTED_COMMANDS: RefCell<HashMap<ObjectId, u64>> = RefCell::new(HashMap::new());

    /// The last reason a recorded frame was refused per window, so a repeated one is not re-logged.
    static SMOOTH_FALLBACKS: RefCell<HashMap<ObjectId, String>> = RefCell::new(HashMap::new());

    /// The last refusal reason per window, so a repeated one is not re-logged.
    static REFUSALS: RefCell<HashMap<ObjectId, String>> = RefCell::new(HashMap::new());

    /// The last outcome per window, for diagnostics and probes.
    static OUTCOMES: RefCell<HashMap<ObjectId, WindowPresentOutcome>> = RefCell::new(HashMap::new());

    /// The frame each window's target is currently showing, and the quality it was drawn at.
    ///
    /// # Why the frame is remembered as an `Rc` and not compared byte by byte
    ///
    /// The frame cache in `crate::widget::runtime` hands back **the same `Rc`** when it did not
    /// repaint (`render_frame_tree_cached_shared` / `render_frame_cached_shared` return the cached
    /// handle) and a fresh one whenever it painted. Pointer identity is therefore "not a single
    /// pixel changed" at the cost of a refcount increment, where comparing the pixels would cost a
    /// whole-frame `memcmp` on every repaint request — 33 MB for a 4K window.
    ///
    /// Identity cannot mistake two frames for each other: this table holds the handle it compares
    /// against, so the address it remembers stays allocated (and unique) for as long as the entry
    /// exists. A frame the cache replaces is a different allocation, because ours is still alive.
    static SHOWN: RefCell<HashMap<ObjectId, ShownFrame>> = RefCell::new(HashMap::new());
}

/// The frame a target is showing, with the quality it was drawn at.
#[derive(Clone)]
struct ShownFrame {
    frame: Rc<Vec<u8>>,
    quality: RenderQuality,
}

/// Whether `window`'s target is already showing `frame`, drawn at `quality`.
///
/// The canvas asks this **before** offering a frame. AppKit, Win32 and GTK all call their paint
/// callback for reasons that are not a content change (an expose after another window moved away,
/// an occlusion change, a backing-store eviction); each of those would otherwise upload a whole
/// frame and present it to show the pixels that are already on screen. Measured on macOS: without
/// this check a 60 s idle window was presented **4 times**.
///
/// `false` for an unknown window, for a frame the target is not showing, and for a frame the
/// target shows at a different quality — the last one because `Smooth` and `Exact` rasterise
/// differently, so a quality change is a frame change even when the source pixels agree.
pub fn frame_is_shown(window: ObjectId, frame: &Rc<Vec<u8>>, quality: RenderQuality) -> bool {
    SHOWN.with(|shown| {
        shown
            .borrow()
            .get(&window)
            .is_some_and(|shown| shown.quality == quality && Rc::ptr_eq(&shown.frame, frame))
    })
}

/// Records that `window`'s target now shows `frame`, drawn at `quality`.
///
/// Called only when a present **succeeded**: a refused frame (occluded, timed out) is not on the
/// target, so the next offer of the same pixels must be presented rather than skipped.
pub fn note_frame_shown(window: ObjectId, frame: &Rc<Vec<u8>>, quality: RenderQuality) {
    SHOWN.with(|shown| {
        shown.borrow_mut().insert(window, ShownFrame { frame: Rc::clone(frame), quality });
    });
}

/// Records a refusal, but only when the reason for `window` changes.
///
/// A canvas paints many frames per second; logging every refused one would drown the log, while
/// logging none leaves a host that opted in with no explanation for why the picture is still being
/// blitted. One line per distinct reason per window is the useful middle.
fn note_refusal(window: ObjectId, reason: &str) -> WindowPresentOutcome {
    let changed = REFUSALS.with(|reasons| {
        let mut reasons = reasons.borrow_mut();
        if reasons.get(&window).map(String::as_str) == Some(reason) {
            return false;
        }
        reasons.insert(window, reason.to_string());
        true
    });
    if changed {
        log::warn!(
            "[present] window {window}: the presentation path answered '{reason}'; the canvas \
             keeps blitting this window until that changes"
        );
    }
    record_outcome(window, WindowPresentOutcome::Skipped)
}

fn record_outcome(window: ObjectId, outcome: WindowPresentOutcome) -> WindowPresentOutcome {
    OUTCOMES.with(|outcomes| {
        outcomes.borrow_mut().insert(window, outcome);
    });
    outcome
}

/// The last outcome [`present_window_frame`] answered for `window`. Diagnostics only.
pub fn last_outcome(window: ObjectId) -> Option<WindowPresentOutcome> {
    OUTCOMES.with(|outcomes| outcomes.borrow().get(&window).copied())
}

fn clear_refusal(window: ObjectId) {
    REFUSALS.with(|reasons| {
        reasons.borrow_mut().remove(&window);
    });
    OUTCOMES.with(|outcomes| {
        outcomes.borrow_mut().remove(&window);
    });
}

/// Records why `window`'s recorded frame is being delivered as pixels, once per distinct reason.
///
/// A host that asked for `Smooth` and silently stayed on the reference rasteriser is the kind of
/// degradation principle #45 says must be visible. The reason is stable per frame in practice (the
/// same commands force the CPU), so a one-line-per-change log is both quiet and complete.
fn note_smooth_fallback(window: ObjectId, reason: &str) {
    let changed = SMOOTH_FALLBACKS.with(|reasons| {
        let mut reasons = reasons.borrow_mut();
        if reasons.get(&window).map(String::as_str) == Some(reason) {
            return false;
        }
        reasons.insert(window, reason.to_string());
        true
    });
    if changed {
        log::info!(
            "[present] window {window}: the recorded frame is not drawable on the GPU ({reason}); \
             the frame is delivered byte-exactly instead"
        );
    }
}

fn clear_smooth_fallback(window: ObjectId) {
    SMOOTH_FALLBACKS.with(|reasons| {
        reasons.borrow_mut().remove(&window);
    });
}

/// Presents `frame` for `window` through its target, creating the target on first use.
///
/// `damage` is the set of damaged rectangles since the previous presented frame, in the frame's
/// own physical pixels; `None` means "assume the whole frame changed" (the first frame, a resize,
/// a restored window). The tuples are the layout
/// [`crate::render::PaintBackend::damage_regions`] returns.
///
/// `size` is the frame's logical size and `scale` the window's scale factor; together they decide
/// the target's physical size, so a canvas that passes a frame whose length does not match is told
/// so by the presenter's own length check (and the frame is skipped, not presented wrong).
///
/// # The handle
///
/// `window_handle` must name the same window `window` does, and must stay valid until
/// [`forget_window`] drops the target — which is what `Platform::raw_window_handle` guarantees for
/// the lifetime of the window.
pub fn present_window_frame(
    window: ObjectId,
    window_handle: RawWindowHandle,
    display_handle: Option<RawDisplayHandle>,
    frame: &[u8],
    size: (u32, u32),
    scale: f32,
    quality: RenderQuality,
    damage: Option<&[(i32, i32, u32, u32)]>,
) -> WindowPresentOutcome {
    let outcome =
        with_presenter(window, window_handle, display_handle, size, scale, quality, |presenter| {
            let status = presenter.resize(size, scale);
            if status == PresentationStatus::Stopped {
                return WindowPresentOutcome::Skipped;
            }
            match damage {
                Some(regions) => presenter.mark_damage(regions),
                None => presenter.mark_full_damage(),
            }
            match presenter.present(frame) {
                Ok(PresentationStatus::Presented) => {
                    PRESENTED.with(|counts| {
                        *counts.borrow_mut().entry(window).or_insert(0) += 1;
                    });
                    clear_refusal(window);
                    WindowPresentOutcome::Presented
                }
                Ok(PresentationStatus::Idle) => {
                    // Nothing was damaged, so the target already shows the frame the caller holds.
                    // The canvas must not blit on top of it: the presented frame is the same one.
                    WindowPresentOutcome::Presented
                }
                Ok(status) => {
                    let reason = presenter
                        .last_failure()
                        .map(|failure| failure.label())
                        .unwrap_or_else(|| status.label());
                    note_refusal(window, reason)
                }
                Err(error) => {
                    log::warn!("[present] window {window}: {error}; the canvas paints this frame");
                    WindowPresentOutcome::Skipped
                }
            }
        });
    match outcome {
        Some(outcome) => record_outcome(window, outcome),
        None => record_outcome(window, WindowPresentOutcome::Unavailable),
    }
}

/// Why a recorded frame cannot be delivered as draw commands.
///
/// The reason is a sentence a log can print: it names either the command classes that force the
/// CPU (with the census' own names) or the first command the bridge cannot convert. Nothing is
/// approximated — the caller falls back to the byte-exact path (plan B29.10: "`Smooth` 帧内出现任一
/// `ForcesCpu` ⇒ 整帧走 `Exact`，不混用两条路径").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothGap {
    /// The sentence to log.
    pub reason: String,
}

impl core::fmt::Display for SmoothGap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// The GPU draw commands for a recorded frame, or why the frame must stay byte-exact.
///
/// This is the decision half of the `Smooth` path, split from the device half so it can be tested
/// without a GPU: the census classifies every command (plan B29.10's gate), and the bridge converts
/// the stream. A stream passes only when every command is mapped — one `ForcesCpu` command means
/// the whole frame goes back to `Exact` rather than mixing two rasterisers in one picture.
///
/// `target` is the frame's pixel size, which one family needs: a conic gradient command carries no
/// rectangle, so its bridged quad is the target itself.
pub fn smooth_commands(
    commands: &[crate::render::RenderCommand],
    text_scale: f32,
    target: (u32, u32),
) -> Result<Vec<crate::wgpu_backend::WgpuDrawCommand>, SmoothGap> {
    let audit = super::census(commands);
    if !audit.can_draw_on_gpu() {
        return Err(SmoothGap {
            reason: format!(
                "{} command(s) force the CPU ({})",
                audit.forces_cpu,
                audit.forces_cpu_names.join(", ")
            ),
        });
    }
    crate::wgpu_backend::bridge_render_commands(commands, text_scale, target)
        .map_err(|gap| SmoothGap { reason: gap.to_string() })
}

/// Presents a frame the canvas **recorded** instead of rasterised (the `Smooth` intent).
///
/// The stream is audited and bridged first ([`smooth_commands`]); a stream the pipelines cannot
/// draw faithfully answers [`WindowPresentOutcome::Skipped`] with the reason logged once per
/// window, and the canvas then offers the same frame through [`present_window_frame`] — the picture
/// is the reference rasteriser's, only the requested quality is unmet.
///
/// The recorded frame is always drawn whole (a command stream has no damage model), so the caller
/// is responsible for not offering an unchanged frame — `frame_is_shown` is what answers that.
pub fn present_recorded_commands(
    window: ObjectId,
    window_handle: RawWindowHandle,
    display_handle: Option<RawDisplayHandle>,
    commands: &[crate::render::RenderCommand],
    size: (u32, u32),
    scale: f32,
) -> WindowPresentOutcome {
    let drawn = match smooth_commands(commands, scale, size) {
        Ok(drawn) => drawn,
        Err(gap) => {
            note_smooth_fallback(window, &gap.reason);
            return record_outcome(window, WindowPresentOutcome::Skipped);
        }
    };
    let quality = RenderQuality::Antialiased;
    let outcome =
        with_presenter(window, window_handle, display_handle, size, scale, quality, |presenter| {
            if presenter.resize(size, scale) == PresentationStatus::Stopped {
                return WindowPresentOutcome::Skipped;
            }
            match presenter.present_commands(&drawn) {
                Ok(PresentationStatus::Presented) => {
                    PRESENTED_COMMANDS.with(|counts| {
                        *counts.borrow_mut().entry(window).or_insert(0) += 1;
                    });
                    clear_smooth_fallback(window);
                    clear_refusal(window);
                    WindowPresentOutcome::Presented
                }
                Ok(status) => {
                    let reason = presenter
                        .last_failure()
                        .map(|failure| failure.label())
                        .unwrap_or_else(|| status.label());
                    note_refusal(window, reason)
                }
                Err(error) => {
                    log::warn!(
                        "[present] window {window}: the command stream was rejected ({error}); \
                         the canvas paints this frame byte-exactly"
                    );
                    WindowPresentOutcome::Skipped
                }
            }
        });
    match outcome {
        Some(outcome) => record_outcome(window, outcome),
        None => record_outcome(window, WindowPresentOutcome::Unavailable),
    }
}

/// Runs `body` with `window`'s target, creating it on first use.
///
/// `None` means no target can exist for this window; the reason was logged when creation failed,
/// and the failure is remembered so a window that cannot present does not pay an adapter probe for
/// every frame. Both delivery paths (pixels and commands) run through here, so "create once",
/// "remember failure" and "resize first" have one implementation rather than two.
fn with_presenter<R>(
    window: ObjectId,
    window_handle: RawWindowHandle,
    display_handle: Option<RawDisplayHandle>,
    size: (u32, u32),
    scale: f32,
    quality: RenderQuality,
    body: impl FnOnce(&mut FramePresenter) -> R,
) -> Option<R> {
    TARGETS.with(|targets| {
        let mut targets = targets.borrow_mut();
        // One lookup, not `contains_key` followed by `insert`: the target is created at most once
        // per window and the table is type-agnostic (`HashMap` is `BTreeMap` without `std`), so
        // `or_insert_with` is the form both containers share.
        let entry = targets.entry(window).or_insert_with(|| {
            match FramePresenter::new(window_handle, display_handle, size, scale, quality) {
                Ok(presenter) => {
                    log::info!(
                        "[present] window {window}: target created at {}x{} ({:?})",
                        presenter.config().width,
                        presenter.config().height,
                        presenter.config().format
                    );
                    Entry::Ready(Box::new(presenter))
                }
                Err(error) => {
                    // Remembered, not retried: a window whose target cannot be created would
                    // otherwise pay a full adapter probe on every frame.
                    log::warn!(
                        "[present] window {window}: no presentation target ({error}); \
                         the canvas keeps painting this window itself"
                    );
                    Entry::Unavailable
                }
            }
        });
        match entry {
            Entry::Ready(presenter) => Some(body(presenter)),
            Entry::Unavailable => None,
        }
    })
}

/// Drops `window`'s target and its counters.
///
/// The window's owner calls this when the window is destroyed, **after** it can no longer paint
/// and **before** the native window goes away. Presenting after this call simply creates a new
/// target, so a forgotten call costs a re-creation rather than a dangling surface.
pub fn forget_window(window: ObjectId) {
    TARGETS.with(|targets| {
        targets.borrow_mut().remove(&window);
    });
    SHOWN.with(|shown| {
        shown.borrow_mut().remove(&window);
    });
    SMOOTH_FALLBACKS.with(|reasons| {
        reasons.borrow_mut().remove(&window);
    });
    PRESENTED_COMMANDS.with(|counts| {
        counts.borrow_mut().remove(&window);
    });
    PRESENTED.with(|counts| {
        counts.borrow_mut().remove(&window);
    });
    REFUSALS.with(|reasons| {
        reasons.borrow_mut().remove(&window);
    });
    OUTCOMES.with(|outcomes| {
        outcomes.borrow_mut().remove(&window);
    });
}

/// How many frames `window` has handed to the compositor through the canvas path.
///
/// This is diagnostics, and the probe's evidence that a repaint really reached the compositor
/// rather than silently falling back to a blit. It counts only the canvas path
/// ([`present_window_frame`]); a host driving [`FramePresenter`] directly reads
/// [`FramePresenter::stats`] instead.
pub fn presented_frames(window: ObjectId) -> u64 {
    PRESENTED.with(|counts| counts.borrow().get(&window).copied()).unwrap_or(0)
}

/// How many of `window`'s presented frames went out as **draw commands** (the `Smooth` path).
///
/// Diagnostics for a host and the acceptance probe of B29.10: the pixel counter says the canvas
/// reached the compositor, this one says the command stream did. A window whose tree contains a
/// command the pipelines cannot draw stays at zero and says why through [`smooth_fallback_reason`].
pub fn presented_command_frames(window: ObjectId) -> u64 {
    PRESENTED_COMMANDS.with(|counts| counts.borrow().get(&window).copied()).unwrap_or(0)
}

/// The reason `window`'s last recorded frame had to be delivered as pixels, if one did.
///
/// The counterpart of [`last_refusal`] for the `Smooth` path: `None` means "no command stream was
/// refused for this window since it last succeeded (or since it was created)".
pub fn smooth_fallback_reason(window: ObjectId) -> Option<String> {
    SMOOTH_FALLBACKS.with(|reasons| reasons.borrow().get(&window).cloned())
}

/// The reason the last frame was refused for `window`, if it was.
///
/// Diagnostics for a probe or a host log: `Skipped` says the canvas blitted, this says *why* —
/// `occluded` (the window server does not consider the window visible), `timeout`, and so on.
pub fn last_refusal(window: ObjectId) -> Option<String> {
    REFUSALS.with(|reasons| reasons.borrow().get(&window).cloned())
}

/// Whether `window` currently owns a presentation target. Diagnostics only.
pub fn has_target(window: ObjectId) -> bool {
    TARGETS.with(|targets| matches!(targets.borrow().get(&window), Some(Entry::Ready(_))))
}

/// The intent recorded for `window`, for a canvas that needs to ask in one place.
pub fn window_intent(window: ObjectId) -> FramePresentation {
    super::requested(window)
}

#[cfg(test)]
mod tests {
    use super::{
        forget_window, frame_is_shown, has_target, note_frame_shown, presented_frames,
        window_intent, WindowPresentOutcome,
    };
    use crate::compat::Rc;
    use crate::render::present::FramePresentation;
    use crate::wgpu_backend::RenderQuality;

    #[test]
    fn an_unknown_window_owes_a_blit_and_owns_nothing() {
        assert_eq!(presented_frames(8_001), 0);
        assert!(!has_target(8_001));
        assert_eq!(window_intent(8_001), FramePresentation::Default);
    }

    #[test]
    fn forgetting_an_unknown_window_is_a_no_op() {
        forget_window(8_002);
        assert_eq!(presented_frames(8_002), 0);
    }

    #[test]
    fn the_outcomes_say_who_draws_the_frame() {
        assert!(!WindowPresentOutcome::Presented.needs_blit());
        assert!(WindowPresentOutcome::Skipped.needs_blit());
        assert!(WindowPresentOutcome::Unavailable.needs_blit());
        assert_eq!(WindowPresentOutcome::Presented.label(), "presented");
        assert_eq!(WindowPresentOutcome::Skipped.label(), "skipped");
        assert_eq!(WindowPresentOutcome::Unavailable.label(), "unavailable");
    }

    #[test]
    fn a_recorded_frame_is_drawable_only_when_every_command_is_mapped() {
        use crate::core::{Color, Rect};
        use crate::render::RenderCommand;

        // The bridged families convert; the gate's answer is the commands, in order.
        let drawable = [
            RenderCommand::PushClip { x: 0, y: 0, width: 8, height: 8 },
            RenderCommand::FillRect { rect: Rect::new(0, 0, 4, 4), color: Color::BLACK },
            RenderCommand::PopClip,
        ];
        let commands = super::smooth_commands(&drawable, 1.0, (32, 24))
            .expect("a rect-and-clip stream converts");
        assert_eq!(commands.len(), 3, "every mapped command must survive the bridge");

        // A command with no faithful GPU form is enough: the whole frame goes back to the
        // reference rasteriser, and the reason names the class rather than saying "unsupported".
        let mut with_blur = drawable.to_vec();
        with_blur.push(RenderCommand::Blur { radius: 2 });
        let gap =
            super::smooth_commands(&with_blur, 1.0, (32, 24)).expect_err("blur forces the CPU");
        assert!(
            gap.reason.contains("1 command(s) force the CPU") && gap.reason.contains("Blur"),
            "the reason must count and name what stopped the frame: {}",
            gap.reason
        );
    }

    #[test]
    fn a_frame_the_target_shows_is_recognized_by_identity_only() {
        let window = 8_003;
        forget_window(window);
        let shown = Rc::new(vec![7u8; 16]);
        // Nothing recorded: the canvas must offer the frame.
        assert!(!frame_is_shown(window, &shown, RenderQuality::ByteExact));

        note_frame_shown(window, &shown, RenderQuality::ByteExact);
        assert!(frame_is_shown(window, &shown, RenderQuality::ByteExact));
        // A shared handle is the same frame: the cache answers a refcount increment, not a copy.
        assert!(frame_is_shown(window, &Rc::clone(&shown), RenderQuality::ByteExact));

        // A repaint allocates a new buffer even when the pixels happen to agree, and that is the
        // signal: the canvas offers it.
        let repainted = Rc::new(vec![7u8; 16]);
        assert!(!frame_is_shown(window, &repainted, RenderQuality::ByteExact));

        // A quality change rasterises the same source pixels differently, so it re-presents.
        assert!(!frame_is_shown(window, &shown, RenderQuality::Antialiased));

        // Destroying the window forgets which frame it showed.
        forget_window(window);
        assert!(!frame_is_shown(window, &shown, RenderQuality::ByteExact));
    }
}
