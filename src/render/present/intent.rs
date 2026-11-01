// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What the host asked the window's frame path to be.
//!
//! The values describe an **intent**, never a mechanism (principle #52): `Exact` means "the pixels
//! must match the software reference", `Smooth` means "a smoother edge is worth a difference",
//! and `Default` means "keep the delivery path the backend already had". Which pipeline that
//! turns into is decided in [`super::presenter`], not here.
//!
//! # Why the intent is stored here rather than on the window mirror
//!
//! The platform canvases read it at the point they paint (the macOS canvas asks before choosing
//! its frame path), and the presenter reads it again when it is created. A single registry in the
//! presentation layer means both readers agree by construction; a per-backend copy would drift
//! (principle #101). The registry is thread-local for the same reason the window mirror is: a
//! window belongs to the thread that created it.
//!
//! Only a **supported** request is recorded: `WindowHandle::set_frame_presentation` writes here
//! after the backend answered that it can honour the intent, so an unsupported host keeps
//! [`FramePresentation::Default`] and no record of a request it cannot satisfy exists.

use crate::compat::{HashMap, RefCell};
use crate::core::ObjectId;

/// How the host wants a window's finished frame delivered.
///
/// `Default` is the behaviour that existed before this enum: the platform canvas blits the
/// software frame itself. `Exact` and `Smooth` ask for the GPU presentation path, and differ only
/// in what may be sacrificed for a smoother edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FramePresentation {
    /// Keep the backend's existing frame delivery: the software frame is blitted by the canvas.
    #[default]
    Default,
    /// Deliver the same software frame through the presentation path, byte for byte.
    ///
    /// This changes *how* the frame reaches the window, not *what* it contains. A host that
    /// compares frames, records them, or feeds them to a downstream encoder asks for this.
    Exact,
    /// Draw the frame's command stream on the GPU where that is faithful.
    ///
    /// Edges of shapes whose pipelines feather their rim (circle, ring, arc, rounded outline) may
    /// differ from the software reference; everything else is expected to match. A host that
    /// prefers a smoother edge over byte equality asks for this.
    Smooth,
}

impl FramePresentation {
    /// Whether this intent keeps the backend's pre-existing delivery path.
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Default)
    }

    /// Whether this intent asks for the GPU presentation path at all.
    pub const fn needs_presenter(self) -> bool {
        !self.is_default()
    }

    /// A stable name for logs and status output.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Exact => "exact",
            Self::Smooth => "smooth",
        }
    }
}

thread_local! {
    /// Window id -> the intent the host asked for and the backend accepted.
    static INTENTS: RefCell<HashMap<ObjectId, FramePresentation>> =
        RefCell::new(HashMap::new());
}

/// Records `intent` for `window`.
///
/// Called by `WindowHandle::set_frame_presentation` **after** the backend confirmed it can honour
/// the request, and by the platform canvas when a window is torn down (`Default`).
pub fn request(window: ObjectId, intent: FramePresentation) {
    INTENTS.with(|map| {
        let mut map = map.borrow_mut();
        if intent.is_default() {
            map.remove(&window);
        } else {
            map.insert(window, intent);
        }
    });
}

/// The intent recorded for `window`, or [`FramePresentation::Default`] when none is.
pub fn requested(window: ObjectId) -> FramePresentation {
    INTENTS.with(|map| map.borrow().get(&window).copied()).unwrap_or_default()
}

/// Drops any recorded intent for `window`.
///
/// Called when the window is destroyed, so a reused id cannot inherit a previous window's intent.
pub fn forget(window: ObjectId) {
    INTENTS.with(|map| {
        map.borrow_mut().remove(&window);
    });
}

/// Drops every recorded intent. Test support only.
#[cfg(test)]
pub(crate) fn forget_all() {
    INTENTS.with(|map| map.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::{forget, forget_all, request, requested, FramePresentation};

    #[test]
    fn an_unrequested_window_reports_default() {
        forget_all();
        assert_eq!(requested(7_001), FramePresentation::Default);
    }

    #[test]
    fn a_requested_intent_is_read_back_and_default_removes_it() {
        forget_all();
        request(7_002, FramePresentation::Exact);
        assert_eq!(requested(7_002), FramePresentation::Exact);
        request(7_002, FramePresentation::Smooth);
        assert_eq!(requested(7_002), FramePresentation::Smooth);
        request(7_002, FramePresentation::Default);
        assert_eq!(requested(7_002), FramePresentation::Default);
    }

    #[test]
    fn forgetting_one_window_leaves_the_others_alone() {
        forget_all();
        request(7_003, FramePresentation::Exact);
        request(7_004, FramePresentation::Smooth);
        forget(7_003);
        assert_eq!(requested(7_003), FramePresentation::Default);
        assert_eq!(requested(7_004), FramePresentation::Smooth);
    }

    #[test]
    fn the_labels_are_stable_and_distinct() {
        assert_eq!(FramePresentation::Default.label(), "default");
        assert_eq!(FramePresentation::Exact.label(), "exact");
        assert_eq!(FramePresentation::Smooth.label(), "smooth");
        assert!(FramePresentation::Default.is_default());
        assert!(FramePresentation::Exact.needs_presenter());
    }
}
