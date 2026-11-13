// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The cocoa canvas' half of the presentation path: offer a finished frame, then decide who draws.
//!
//! Split out of `canvas.rs` at the seam principle #9 points at: everything here is about *delivery*
//! (the intent, the presentation target, and the Metal sublayer `wgpu` leaves behind), while
//! `canvas.rs` is about AppKit plumbing — the view, its dirty rectangle and CoreGraphics. Keeping
//! them apart also keeps `canvas.rs` under the file-length limit without merging unrelated code.
//!
//! Both entry points run from `drawRect:`, i.e. on the AppKit main thread, with a live `NSView`.

// The `cocoa` 0.24 bindings are deprecated in favour of `objc2`, and every deprecated item this
// module names is one of them. This is the **legacy** macOS presentation path, kept beside
// `platform/macos_objc2` by design (see `docs/plans/platform_optional_backends.md`); rewriting
// these calls against objc2 would duplicate that backend (principle #101). Scoped to this module.
#![allow(deprecated)]

use cocoa::base::{id, nil, NO, YES};
use objc::runtime::Class;
use objc::{msg_send, sel, sel_impl};

use crate::core::ObjectId;

/// Shows or hides the Metal sublayer the presentation path added to `view`.
///
/// # Why this is needed
///
/// `wgpu` makes the view layer-backed and inserts a `CAMetalLayer` sublayer; its `Drop` only
/// unregisters key-value observers, it does **not** detach the sublayer. So a window that returns
/// to the default delivery path would keep showing the last presented frame on top of every frame
/// the canvas blits afterwards. Hiding the sublayer is what makes `Default` mean what it says, and
/// showing it again is what lets a returned-to `Exact` window present into it.
///
/// # Safety
///
/// `view` must be a live `NSView` on the AppKit main thread, which is where `drawRect:` runs.
pub(super) unsafe fn set_presented_layer_visible(view: id, visible: bool) {
    if view == nil {
        return;
    }
    let root: id = msg_send![view, layer];
    if root == nil {
        return;
    }
    let sublayers: id = msg_send![root, sublayers];
    if sublayers == nil {
        return;
    }
    // The class is looked up at runtime rather than linked: `CAMetalLayer` lives in QuartzCore,
    // which this backend does not link for its own drawing (CoreGraphics is enough), and a class
    // lookup keeps the dependency out of the link line.
    let Some(metal_class) = Class::get("CAMetalLayer") else {
        return;
    };
    let count: usize = msg_send![sublayers, count];
    for index in 0..count {
        let layer: id = msg_send![sublayers, objectAtIndex: index];
        if layer == nil {
            continue;
        }
        let is_metal: cocoa::base::BOOL = msg_send![layer, isKindOfClass: metal_class];
        if is_metal == NO {
            continue;
        }
        let _: () = msg_send![layer, setHidden: if visible { YES } else { NO }];
    }
}

/// Offers `frame` to the presentation path when this window asked for one.
///
/// Returns `true` when the frame is on the target and the caller must **not** blit it. `false`
/// — the default path — means the window never asked, or asked and this frame was refused
/// (occluded, timed out, no target), in which case the canvas paints the frame itself exactly as
/// it always did.
///
/// # A frame the target already shows is not an offer
///
/// `frame` is the handle the frame cache returned, and identity is the damage signal the canvas
/// cannot otherwise see: AppKit calls `drawRect:` for exposures, occlusion changes and
/// backing-store evictions, none of which repaint a pixel. Such a frame is skipped **before** a
/// surface is acquired (B29.12) — measured: 4 presents during a 60 s idle window without it — and
/// the answer is `true`, because the sublayer still shows exactly these pixels.
///
/// # `Smooth`
///
/// When the window asked for `Smooth`, the tree is recorded into a
/// [`crate::render::present::GpuPaintBackend`] and delivered as **draw commands**
/// ([`crate::render::present::present_recorded_commands`]). Any command the pipelines cannot draw
/// faithfully drops the whole frame back to the byte-exact path below — the picture is never a mix
/// of two rasterisers, and the reason is logged once per window.
///
/// # The two ids
///
/// `intent_key` is the id the host used when it called `WindowHandle::set_frame_presentation`:
/// the widget id for a window canvas, the mounted surface's own id otherwise.
/// `handle_key` is the id this backend's handle table knows the view by (the host id for a
/// window). They differ for a window, and using one in place of the other is how the capability
/// and the handle stopped agreeing once already.
///
/// `is_window_canvas` says which of the two painters applies: a window paints its tree, a mounted
/// surface paints only itself.
///
/// # Size and scale
///
/// The target is configured at the **frame's own** pixel size with a scale of `1.0`. That matches
/// what the existing path does: the canvas blits a `width x height` `CGImage` into a possibly
/// scaled context, so the compositor scales the same image either way. Rendering the widget tree
/// at the backing resolution is a separate change, and pretending this one does it would be a
/// claim the pixels do not support.
#[allow(clippy::too_many_arguments)]
pub(super) fn present_through_target(
    intent_key: ObjectId,
    handle_key: ObjectId,
    width: u32,
    height: u32,
    frame: &crate::compat::Rc<Vec<u8>>,
    dirty_rows: Option<(usize, usize)>,
    is_window_canvas: bool,
) -> bool {
    use crate::render::present::{self, FramePresentation, WindowPresentOutcome};

    let intent = present::window_intent(intent_key);
    if intent == FramePresentation::Default {
        return false;
    }
    let quality = match intent {
        FramePresentation::Smooth => crate::wgpu_backend::RenderQuality::Antialiased,
        _ => crate::wgpu_backend::RenderQuality::ByteExact,
    };
    // Nothing was repainted since the target last took this frame: there is no damage to upload
    // and no present to make. Asking before touching the surface is also what keeps an idle
    // window at zero submissions (B29.12).
    if present::frame_is_shown(intent_key, frame, quality) {
        return true;
    }
    let platform = crate::platform::get_platform();
    let Some(handle) = platform.raw_window_handle(handle_key) else {
        return false;
    };
    let display = platform.raw_display_handle();

    // `Smooth`: the same tree, delivered as commands. The recording is the tree's own drawing code
    // run against a backend that keeps the stream instead of the pixels, so the two paths cannot
    // disagree about what the frame is.
    if intent == FramePresentation::Smooth {
        let size = crate::core::Size::new(width, height);
        // One recorder per thread, reused across frames (RD-02). `begin_frame` inside the painter
        // clears the previous stream, so nothing of the old frame reaches this one.
        let presented = present::with_gpu_paint_backend(size, 1.0, |recorder| {
            let painted = if is_window_canvas {
                crate::widget::runtime::paint_tree_into(
                    intent_key,
                    size,
                    crate::core::Color::WHITE,
                    recorder,
                )
            } else {
                crate::widget::runtime::paint_widget_into(
                    intent_key,
                    size,
                    crate::core::Color::WHITE,
                    recorder,
                )
            };
            painted
                && present::present_recorded_commands(
                    intent_key,
                    handle,
                    display,
                    recorder.commands(),
                    (width, height),
                    1.0,
                    recorder.clear_color(),
                ) == WindowPresentOutcome::Presented
        });
        if presented {
            present::note_frame_shown(intent_key, frame, quality);
            return true;
        }
        // Falls through: a stream the pipelines cannot draw, or a surface that refused this frame.
        // The pixels the canvas already holds are the reference picture, so nothing is lost.
    }

    // The paint's own damage rectangles are preferred: they carry the horizontal extent, which
    // the AppKit row band throws away (W6-43). The band stays as the fallback for a frame the
    // paint recorded no damage for (a cached frame, or a paint path that reports none).
    let damage_regions = crate::widget::runtime::last_frame_damage(intent_key);
    let band;
    let damage = if !damage_regions.is_empty() {
        Some(damage_regions.as_slice())
    } else {
        match dirty_rows {
            Some((first_row, last_row)) if last_row > first_row => {
                band = [(0i32, first_row as i32, width, (last_row - first_row) as u32)];
                Some(&band[..])
            }
            _ => None,
        }
    };
    let outcome = present::present_window_frame(
        intent_key,
        handle,
        display,
        frame.as_slice(),
        (width, height),
        1.0,
        quality,
        damage,
    );
    if outcome == WindowPresentOutcome::Presented {
        present::note_frame_shown(intent_key, frame, quality);
        return true;
    }
    // Refused for this frame: say why once per window, so a host that opted in can see that the
    // canvas fell back instead of silently keeping the default picture.
    log::debug!(
        "[macos] canvas: presentation path answered '{}' for window {intent_key}; blitting",
        outcome.label()
    );
    false
}
