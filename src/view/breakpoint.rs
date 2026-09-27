// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Choosing a **subtree** by available size, rather than computing coordinates by it.
//!
//! # Why a declared breakpoint and not an `if` in a draw path
//!
//! The tempting spelling of "a phone layout" is `if rect.width < 600 { .. } else { .. }`
//! inside a control's `draw`. It is wrong for two reasons that this module exists to remove:
//!
//! 1. **It cannot be reviewed.** The phone layout then lives in each control's paint code, so
//!    "what does this application look like narrow?" has as many answers as there are controls.
//! 2. **It cannot see the context a subtree has.** `Hints` propagate *upward*: a control's
//!    intrinsic size depends on where it sits (a button in a drawer versus a button in the main
//!    region). A branch taken in `draw` has already lost that context, whereas a branch taken
//!    while *building the tree* has it.
//!
//! So a breakpoint selects **which nodes exist**, exactly as [`Node::child_if`] selects whether
//! one does. Once the tree is chosen, every control's arithmetic is settled by `Layout` + `Hints`
//! as it always was — a breakpoint never changes a coordinate itself.
//!
//! # Why three tiers
//!
//! `Compact` (narrow / phone) · `Medium` (tablet) · `Expanded` (desktop). Not four or five: the
//! metric table and `Hints` already express "how wide", so an extra tier would add a **name**
//! without a **behaviour** — a number with no consequence, which the crate's rules call a knob
//! that is really a decoration. The boundary live in
//! [`crate::widget::metrics::dimensions`], where the rest of the sizes live, so they are data and
//! can be moved in one place.
//!
//! [`Node::child_if`]: crate::view::Node::child_if

use crate::core::Size;
use crate::widget::metrics::dimensions::{BREAKPOINT_COMPACT_MAX, BREAKPOINT_MEDIUM_MAX};

/// A size tier a subtree can be declared to live in.
///
/// See the module documentation for why the classification exists and why there are three tiers.
/// The tiers are ordered from narrowest to widest, so a comparison like "at least a tablet" is
/// `>= Breakpoint::Medium` and does not need a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint {
    /// Narrow: a phone's portrait window. The layout is usually one column.
    Compact,
    /// Medium: a tablet, or a desktop window narrowed enough that a sidebar is a compromise.
    Medium,
    /// Wide: a desktop window with room for more than one region side by side.
    Expanded,
}

impl Breakpoint {
    /// Classifies an available size.
    ///
    /// # Why the *width* decides
    ///
    /// A height alone never tells a layout "put a sidebar beside the content", and the failure the
    /// classification prevents — a two-column layout crushed into a phone's width — is a
    /// horizontal one. So a tall window is not mistaken for a wide one, which a diagonal or
    /// area-based rule would do.
    ///
    /// The thresholds are read from the metric table rather than written here, so the boundary is
    /// the theme's/device's fact and not a second copy of it (see the module docs).
    pub fn of(available: Size) -> Self {
        if available.width <= BREAKPOINT_COMPACT_MAX {
            Breakpoint::Compact
        } else if available.width <= BREAKPOINT_MEDIUM_MAX {
            Breakpoint::Medium
        } else {
            Breakpoint::Expanded
        }
    }

    /// The breakpoint in force for the build currently running, or [`Breakpoint::Expanded`] when
    /// no build has declared one.
    ///
    /// # Why `Expanded` is the default
    ///
    /// A tree built without a viewport fact should render **everything** it declares, and
    /// `Expanded` is the tier where a declaration is most likely to include every region — a
    /// narrow default would silently drop a subtree a caller wrote, which looks like a bug in the
    /// caller. So the answer with no information is the widest one, and a caller that wants the
    /// narrow layout supplies the size.
    ///
    /// # Why it is a thread-local rather than a parameter
    ///
    /// A `View::build` takes no viewport argument (its signature is fixed and its contract is
    /// purity), so a size cannot be threaded to it through `Node`. It is instead established for
    /// the duration of one build by [`with_breakpoint`], which is the same shape the environment
    /// snapshot uses: one value in force for one pass, read from wherever it is needed.
    pub fn current() -> Self {
        CURRENT.try_with(|cell| cell.get()).unwrap_or(Breakpoint::Expanded)
    }
}

impl Default for Breakpoint {
    /// The widest tier — see [`Breakpoint::current`] for why "no information" means `Expanded`.
    fn default() -> Self {
        Breakpoint::Expanded
    }
}

thread_local! {
    /// The breakpoint in force for this thread's current build.
    ///
    /// Thread-local for the same reason the widget registry and the environment snapshot are:
    /// builds run on the thread that owns the UI, and a test can set one without racing a build on
    /// another thread. [`with_breakpoint`] is the only writer.
    #[allow(clippy::missing_const_for_thread_local)]
    static CURRENT: core::cell::Cell<Breakpoint> = const { core::cell::Cell::new(Breakpoint::Expanded) };
}

/// Runs `f` with `breakpoint` in force, restoring the previous one afterwards.
///
/// # Why this restores rather than leaving the value set
///
/// A build is a **scoped** event: a caller that builds a tree at 320 px and then builds another at
/// 1200 px must get two different trees, and a leaked value would make the second build inherit
/// the first one's tier. Restoring on exit — including on unwind, because this is a scope guard —
/// is what makes "the breakpoint is a fact about this build" true rather than a hope.
///
/// ```
/// use rust_widgets::view::{with_breakpoint, Breakpoint};
/// use rust_widgets::core::Size;
/// let narrow = with_breakpoint(Breakpoint::of(Size::new(320, 640)), Breakpoint::current);
/// assert_eq!(narrow, Breakpoint::Compact);
/// // Restored: the default is in force again.
/// assert_eq!(Breakpoint::current(), Breakpoint::Expanded);
/// ```
pub fn with_breakpoint<R>(breakpoint: Breakpoint, f: impl FnOnce() -> R) -> R {
    /// Restores the previous breakpoint however the scope exits, including on a panic.
    struct Restore(Breakpoint);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = CURRENT.try_with(|cell| cell.set(self.0));
        }
    }
    let previous = CURRENT.try_with(|cell| cell.replace(breakpoint)).unwrap_or(breakpoint);
    let _restore = Restore(previous);
    f()
}

/// Whether `breakpoint` is the one in force for the build currently running.
///
/// The predicate [`crate::view::Node::breakpoint`] branches on. It is a function rather than a
/// comparison at the call site so the "which breakpoint is current" question has one answer, and
/// so a future change to how it is established does not touch the builder.
pub fn is_current(breakpoint: Breakpoint) -> bool {
    Breakpoint::current() == breakpoint
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three tiers are reached at their boundaries, and a boundary belongs to exactly one of
    /// them — the property a table with an off-by-one would violate.
    #[test]
    fn the_width_classifies_into_exactly_one_tier() {
        assert_eq!(Breakpoint::of(Size::new(0, 1000)), Breakpoint::Compact);
        assert_eq!(Breakpoint::of(Size::new(320, 640)), Breakpoint::Compact);
        assert_eq!(
            Breakpoint::of(Size::new(BREAKPOINT_COMPACT_MAX, 1000)),
            Breakpoint::Compact,
            "the compact bound is inclusive"
        );
        assert_eq!(
            Breakpoint::of(Size::new(BREAKPOINT_COMPACT_MAX + 1, 1000)),
            Breakpoint::Medium,
            "one pixel past it is medium, with no gap"
        );
        assert_eq!(
            Breakpoint::of(Size::new(BREAKPOINT_MEDIUM_MAX, 1000)),
            Breakpoint::Medium,
            "the medium bound is inclusive"
        );
        assert_eq!(
            Breakpoint::of(Size::new(BREAKPOINT_MEDIUM_MAX + 1, 1000)),
            Breakpoint::Expanded,
            "and one past it is expanded"
        );
    }

    /// Height alone never makes a window wide: a tall narrow viewport stays `Compact`.
    #[test]
    fn a_tall_window_is_not_mistaken_for_a_wide_one() {
        assert_eq!(Breakpoint::of(Size::new(320, 4000)), Breakpoint::Compact);
        assert_eq!(Breakpoint::of(Size::new(1200, 200)), Breakpoint::Expanded);
    }

    /// The tier in force is the one a scope declared, and it is restored on exit.
    #[test]
    fn a_build_scope_declares_the_tier_in_force() {
        assert_eq!(Breakpoint::current(), Breakpoint::Expanded, "the neutral default");
        let inside = with_breakpoint(Breakpoint::Compact, Breakpoint::current);
        assert_eq!(inside, Breakpoint::Compact, "the scope's tier is in force inside");
        assert!(is_current(Breakpoint::Expanded), "and the previous one is restored after");
        assert!(!is_current(Breakpoint::Compact));
    }

    /// A nested scope restores its parent's tier rather than the default.
    #[test]
    fn nested_scopes_restore_their_parent() {
        with_breakpoint(Breakpoint::Medium, || {
            assert_eq!(Breakpoint::current(), Breakpoint::Medium);
            with_breakpoint(Breakpoint::Compact, || {
                assert_eq!(Breakpoint::current(), Breakpoint::Compact);
            });
            assert_eq!(Breakpoint::current(), Breakpoint::Medium, "the parent is restored");
        });
        assert_eq!(Breakpoint::current(), Breakpoint::Expanded);
    }
}
