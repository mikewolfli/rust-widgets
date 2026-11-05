// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Solver-level telemetry: what an arrange pass had to clamp (RW-48).
//!
//! # Why this exists
//!
//! `Layout::arrange` writes child rectangles and returns nothing. When a solver has less room than
//! its children's hints declare, it silently gives a child a size outside the range that child
//! said it needed — the child is drawn squeezed or clipped, and nothing in the program says which
//! one or by how much. A designer's canvas cannot warn "this control is clipped here" without that
//! fact, which is why T10.03's `overflow` row was the one item with no mapping.
//!
//! This module is the report, not a second solver: [`ArrangeReport::from_placements`] compares the
//! rectangles a solver *did* emit against the [`ChildInfo`] hints the caller supplied. Every
//! solver therefore produces a report with no change to its own arithmetic, and the existing
//! `arrange` signatures stay as they were (principle #21: additive API only).
//!
//! # What is detected, and what is deliberately not
//!
//! * **Hint-range violations** — a child narrower than its `min`, wider than its `max`, or the
//!   same on the vertical axis. These are unambiguous: the solver gave the child a size the child
//!   itself said it cannot use.
//! * **Parent overflow** — a rectangle extending past the parent's box. This is reported
//!   separately because it is *not* always a defect: the absolute solver (test-only now, API2-01)
//!   documents negative offsets as legitimate, and a scrolling canvas intentionally places content
//!   outside the viewport. A caller that knows its layout semantics can ignore `overflowing`
//!   while still reading `clamped`.
//!
//! Nothing here allocates when the caller uses the plain `arrange` path: the report is built only
//! by the `*_reported` entries, and each of those records the placements it already had to
//! collect.

use crate::compat::{format, String, Vec};
use crate::core::{ObjectId, Rect};
use crate::layout::hints::ChildInfo;

/// Which bound a placement violated.
///
/// The bound is carried with the kind because "narrower than its minimum" is actionable only if
/// the reader knows *what* that minimum was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClampKind {
    /// The child is narrower than the `min` it declared.
    BelowMinimumWidth {
        /// The declared minimum width.
        bound: u32,
    },
    /// The child is wider than the `max` it declared.
    AboveMaximumWidth {
        /// The declared maximum width.
        bound: u32,
    },
    /// The child is shorter than the `min` it declared.
    BelowMinimumHeight {
        /// The declared minimum height.
        bound: u32,
    },
    /// The child is taller than the `max` it declared.
    AboveMaximumHeight {
        /// The declared maximum height.
        bound: u32,
    },
    /// The rectangle extends past the parent's box.
    OutsideParent,
}

impl ClampKind {
    /// A stable token for a report line or a gate.
    pub const fn token(self) -> &'static str {
        match self {
            Self::BelowMinimumWidth { .. } => "below-min-width",
            Self::AboveMaximumWidth { .. } => "above-max-width",
            Self::BelowMinimumHeight { .. } => "below-min-height",
            Self::AboveMaximumHeight { .. } => "above-max-height",
            Self::OutsideParent => "outside-parent",
        }
    }

    /// A one-line explanation, including the violated bound.
    pub fn describe(self) -> String {
        match self {
            Self::BelowMinimumWidth { bound } => {
                format!("narrower than its declared minimum width {bound}")
            }
            Self::AboveMaximumWidth { bound } => {
                format!("wider than its declared maximum width {bound}")
            }
            Self::BelowMinimumHeight { bound } => {
                format!("shorter than its declared minimum height {bound}")
            }
            Self::AboveMaximumHeight { bound } => {
                format!("taller than its declared maximum height {bound}")
            }
            Self::OutsideParent => String::from("outside the parent rectangle"),
        }
    }
}

/// One placement the solver clamped away from what the child asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClampRecord {
    /// The child whose rectangle violated a bound.
    pub child: ObjectId,
    /// Which bound, and the value it declared.
    pub kind: ClampKind,
    /// The rectangle the solver emitted.
    pub applied: Rect,
}

impl ClampRecord {
    /// A one-line report entry.
    pub fn describe(&self) -> String {
        format!(
            "child {} is {} (placed at {}x{} at {},{}): {}",
            self.child,
            self.kind.describe(),
            self.applied.width,
            self.applied.height,
            self.applied.x,
            self.applied.y,
            self.kind.token()
        )
    }
}

/// What one arrange pass did to the children's declared bounds (RW-48).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArrangeReport {
    /// The parent rectangle the pass was given.
    pub parent: Rect,
    /// How many child rectangles the pass emitted (spacers included, if the layout emits them).
    pub placements: usize,
    /// Children the solver sized outside their declared hint range.
    pub clamped: Vec<ClampRecord>,
    /// Children placed wholly or partly outside the parent rectangle.
    ///
    /// Separate because this is layout semantics, not necessarily a defect: an absolute layout's
    /// negative offsets and a scrolling canvas's content are both intentional.
    pub overflowing: Vec<ClampRecord>,
}

impl ArrangeReport {
    /// Builds a report by comparing emitted rectangles against the children's hints.
    ///
    /// A rectangle whose child is not in `children` still participates in the parent-overflow
    /// check (the layout may emit a spacer or an id the caller did not describe), but it cannot
    /// be judged against a hint because none was supplied.
    pub fn from_placements(
        parent: Rect,
        children: &[ChildInfo],
        placements: &[(ObjectId, Rect)],
    ) -> Self {
        let mut report = Self {
            parent,
            placements: placements.len(),
            clamped: Vec::new(),
            overflowing: Vec::new(),
        };
        let parent_left = i64::from(parent.x);
        let parent_top = i64::from(parent.y);
        let parent_right = parent_left + i64::from(parent.width);
        let parent_bottom = parent_top + i64::from(parent.height);

        for (child, rect) in placements {
            let right = i64::from(rect.x) + i64::from(rect.width);
            let bottom = i64::from(rect.y) + i64::from(rect.height);
            if i64::from(rect.x) < parent_left
                || i64::from(rect.y) < parent_top
                || right > parent_right
                || bottom > parent_bottom
            {
                report.overflowing.push(ClampRecord {
                    child: *child,
                    kind: ClampKind::OutsideParent,
                    applied: *rect,
                });
            }

            let Some(info) = children.iter().find(|info| info.id == *child) else { continue };
            let mut record = |kind: ClampKind| {
                report.clamped.push(ClampRecord { child: *child, kind, applied: *rect });
            };
            if rect.width < info.hints.width.min {
                record(ClampKind::BelowMinimumWidth { bound: info.hints.width.min });
            }
            if rect.width > info.hints.width.max {
                record(ClampKind::AboveMaximumWidth { bound: info.hints.width.max });
            }
            if rect.height < info.hints.height.min {
                record(ClampKind::BelowMinimumHeight { bound: info.hints.height.min });
            }
            if rect.height > info.hints.height.max {
                record(ClampKind::AboveMaximumHeight { bound: info.hints.height.max });
            }
        }
        report
    }

    /// Whether the pass honoured every declared bound and stayed inside the parent.
    pub fn is_clean(&self) -> bool {
        self.clamped.is_empty() && self.overflowing.is_empty()
    }

    /// How many distinct children were clamped (one child may violate two axes).
    pub fn clamped_children(&self) -> usize {
        let mut ids: Vec<ObjectId> = Vec::new();
        for record in &self.clamped {
            if !ids.contains(&record.child) {
                ids.push(record.child);
            }
        }
        ids.len()
    }

    /// A one-line summary for a log line or a designer status bar.
    pub fn summary(&self) -> String {
        let children = self.clamped_children();
        let clamp_word = if children == 1 { "clamp" } else { "clamps" };
        format!(
            "{} placements, {} {} on {} children, {} outside the parent",
            self.placements,
            children,
            clamp_word,
            children,
            self.overflowing.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::vec;
    use crate::layout::hints::{Hints, LayoutParams};

    fn child(id: ObjectId, min: u32, max: u32) -> ChildInfo {
        ChildInfo::new(id, Hints::fixed(min, max)).with_params(LayoutParams::new())
    }

    /// A placement inside its hints and inside the parent reports nothing.
    #[test]
    fn an_honoured_arrangement_is_clean() {
        let parent = Rect::new(0, 0, 100, 40);
        let report = ArrangeReport::from_placements(
            parent,
            &[child(1, 20, 20)],
            &[(1, Rect::new(10, 10, 20, 20))],
        );
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(report.placements, 1);
    }

    /// A child squeezed below its minimum is reported with the bound it declared.
    #[test]
    fn a_narrow_child_is_reported_as_clamped() {
        let parent = Rect::new(0, 0, 100, 40);
        let report = ArrangeReport::from_placements(
            parent,
            &[child(1, 80, 80)],
            &[(1, Rect::new(0, 0, 40, 40))],
        );
        assert!(!report.is_clean());
        assert!(report
            .clamped
            .iter()
            .any(|record| matches!(record.kind, ClampKind::BelowMinimumWidth { bound: 80 })));
        assert_eq!(report.clamped_children(), 1);
        assert!(report.summary().contains("1 clamp"), "{}", report.summary());
    }

    /// A child placed outside the parent is reported separately from a hint clamp, because
    /// absolute layouts legitimately do that.
    #[test]
    fn parent_overflow_is_reported_separately() {
        let parent = Rect::new(100, 100, 50, 50);
        let report = ArrangeReport::from_placements(parent, &[], &[(7, Rect::new(0, 0, 20, 20))]);
        assert!(report.clamped.is_empty());
        assert_eq!(report.overflowing.len(), 1);
        assert_eq!(report.overflowing[0].kind, ClampKind::OutsideParent);
        assert!(report.summary().contains("outside the parent"));
    }

    /// A hint violation and an overflow can both be true of one child; both are recorded.
    #[test]
    fn one_child_can_violate_a_hint_and_the_parent() {
        let parent = Rect::new(0, 0, 30, 30);
        let report = ArrangeReport::from_placements(
            parent,
            &[child(3, 10, 10)],
            &[(3, Rect::new(20, 0, 40, 10))],
        );
        assert_eq!(report.clamped.len(), 1);
        assert_eq!(report.overflowing.len(), 1);
        assert_eq!(vec![report.clamped[0].child], vec![3]);
    }
}
