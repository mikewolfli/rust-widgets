// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Text-input decoration slots — the five non-value strings a text field shows.
//!
//! # What this is, and what it deliberately is not
//!
//! A text field's chrome is more than its value and its placeholder. The shared table
//! models the same five extra strings, and this crate had none of them on its text entries:
//!
//! | Slot | What it is | Where it goes |
//! |---|---|---|
//! | `prefix` | a unit marker written *before* the value — `$`, `https://` | inside the field, before the text |
//! | `suffix` | a unit marker written *after* the value — `%`, `kg` | inside the field, after the text |
//! | `helper` | a quiet line explaining what the field wants | a row below the field |
//! | `error` | why the current value is refused | a row below the field, in the error colour |
//! | `counter` | how much of the limit is used — `12/40` | the row below, at the trailing edge |
//!
//! # Why `prefix`/`suffix` are *not* concatenated into the value
//!
//! The obvious shortcut is to make the display text `prefix + value + suffix`, which is what
//! `spin_box` did. That is wrong for three separate reasons, and each is a defect rather than a style
//! preference:
//!
//! 1. **The caret is placed by measuring the text.** With a prefix folded in, `cursor_position == 0`
//!    measures the *prefix* and draws the caret after the `$` — so the user cannot put the caret at the
//!    start of what they are editing.
//! 2. **A selection would include the decoration.** Select-all plus copy would yield `$12` instead of
//!    `12`, and what was drawn would disagree with the field's own `text`.
//! 3. **The slots are not always text.** A `suffix` is frequently a unit badge or an icon in the
//!    trailing edge, which is why these are modelled as *content items* rather than as string
//!    concatenation.
//!
//! So this module keeps the slots **separate** and derives their *boxes*. `spin_box` keeps its
//! concatenating `display_text` for the *announcement* string — reading out `$12` is correct there —
//! while drawing the `$` in its own box, so the value's origin no longer includes it.
//!
//! # What is measured and what is argued
//!
//! Only the renderer knows its font metrics, so every string's width arrives here as a number the
//! caller measured ([`DecorationMetrics`]), and the box arithmetic below is pure. A caller that guessed
//! a width would place the value at a different x than the glyphs it draws — the divergence `GroupBox`
//! and `TabBar` both had to fix by measuring their own titles.

use crate::compat::{format, String};
use crate::core::Rect;

/// The space a decoration slot keeps from the text beside it.
///
/// # Why this is shared and not a per-control constant
///
/// A `$` in a `line_edit` and a `$` in a `spin_box` are the same mark in the same relationship to the
/// same value, so they have to sit the same distance away. Two constants is two chances for a form to
/// look subtly misaligned, and the misalignment is only visible when both controls are on screen at
/// once — which is exactly the configuration a unit test does not build.
pub const DECORATION_GAP: u32 = 4;

/// The five decoration strings a text field may show.
///
/// Every field defaults to empty, so a control that never sets one draws exactly as it did before this
/// type existed — which is what makes adding the slots a non-change for existing callers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecorationSlots {
    /// Written inside the field, before the value.
    pub prefix: String,
    /// Written inside the field, after the value.
    pub suffix: String,
    /// The quiet hint under the field, shown only while there is no error.
    pub helper: String,
    /// The refusal message under the field.
    pub error: String,
    /// The usage count under the field's trailing edge, e.g. `12/40`.
    pub counter: String,
}

impl DecorationSlots {
    /// An empty set of slots: the state a field that never sets one is in.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the field is currently showing a refusal.
    ///
    /// # Why this is `error`'s presence rather than a separate flag
    ///
    /// A flag is a second way to say whether an error is up and the two can disagree: a field that
    /// cleared its message but not its flag would paint an empty error row, and a field that set its
    /// flag without a message would paint nothing while claiming to be in error. Deriving it from the
    /// only thing the state actually is makes both unrepresentable.
    pub fn has_error(&self) -> bool {
        !self.error.is_empty()
    }

    /// Returns whether any slot is set, i.e. whether the decoration surface is in use at all.
    pub fn is_empty(&self) -> bool {
        self.prefix.is_empty()
            && self.suffix.is_empty()
            && self.helper.is_empty()
            && self.error.is_empty()
            && self.counter.is_empty()
    }

    /// The message the field shows *below* itself, if any.
    ///
    /// An error displaces the helper rather than being shown beside it: the two occupy one row, and
    /// showing both would make the refusal and the hint compete for the same space. The counter is
    /// **not** displaced, because it is a different kind of statement — how much of the budget is used,
    /// not what the field wants.
    pub fn support_message(&self) -> &str {
        if self.has_error() {
            &self.error
        } else {
            &self.helper
        }
    }

    /// Returns whether the field must reserve a row below itself.
    pub fn needs_support_row(&self) -> bool {
        !self.support_message().is_empty() || !self.counter.is_empty()
    }
}

/// The measured widths a decorated field's layout needs.
///
/// # Why widths are carried rather than arguments
///
/// [`DecorationLayout::compute`] has to stay pure to be testable, and only the renderer can measure
/// text. Splitting the two — measure once, then lay out from numbers — is what keeps the arithmetic
/// checkable and the metrics real.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecorationMetrics {
    /// The measured advance of [`DecorationSlots::prefix`], or 0 when there is none.
    pub prefix_width: u32,
    /// The measured advance of [`DecorationSlots::suffix`], or 0 when there is none.
    pub suffix_width: u32,
}

impl DecorationMetrics {
    /// Measures both slots with `measure`, which the caller supplies from its render context.
    ///
    /// A slot that is not set measures to **zero**, so an unset slot costs exactly nothing — a field
    /// with no `$` must not be inset by a phantom one. This is the same rule
    /// [`crate::widget::metrics`] applies to an absent control metric.
    pub fn measure(slots: &DecorationSlots, measure: impl Fn(&str) -> u32) -> Self {
        Self {
            prefix_width: if slots.prefix.is_empty() { 0 } else { measure(&slots.prefix) },
            suffix_width: if slots.suffix.is_empty() { 0 } else { measure(&slots.suffix) },
        }
    }
}

/// The boxes a decorated text field is laid out from.
///
/// Derived together from one field box, one set of metrics and one slot set, so the pieces cannot
/// disagree. The defect this replaces was a value drawn from the field's leading padding while the
/// caret was measured from a string that included the prefix — two different origins for the same
/// glyph run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorationLayout {
    /// Where the prefix is drawn, or `None` when the field has no prefix.
    pub prefix: Option<Rect>,
    /// Where the **value** is drawn. Always present, and never overlapping the slots.
    pub value: Rect,
    /// Where the suffix is drawn, or `None` when the field has no suffix.
    pub suffix: Option<Rect>,
    /// The box each character of [`DecorationSlots::counter`] is drawn into, or `None` when the field
    /// has no counter or no support row.
    ///
    /// Returned box-wise rather than as one width because the caller draws the digits itself: the
    /// counter is per-character so the `counter_position` (the digit the caret would be at) picks the
    /// right cell, exactly as the value's caret is placed by measuring a prefix of the value.
    pub counter: Option<Rect>,
    /// The row below the field holding the support message and the counter, if either is set.
    pub support: Option<Rect>,
    /// The line box the support message is drawn in, when there is a support row.
    pub support_text: Option<Rect>,
}

impl DecorationLayout {
    /// Lays out the field's regions.
    ///
    /// # The parameters, and why each is the caller's to decide
    ///
    /// * `field` — the field's **own** box, already inset by the caller from whatever control rectangle
    ///   it was given. Decoration slots belong to the field, not to the control, so a spin box can keep
    ///   its step buttons out of every box here.
    /// * `padding_h` — the field's horizontal text padding, the same number the caller uses to inset
    ///   the value when there are no slots at all.
    /// * `line_height` — the height of one line of the field's font, for the support row.
    /// * `gap` — the space between a slot and the text beside it **and** between the field and its
    ///   support row. One number, because they are the same visual break.
    /// * `metrics` — the measured slot widths.
    /// * `counter_width` — the measured width of the counter string, or 0 when there is no counter.
    ///
    /// # Why the support row is outside the field box
    ///
    /// The row sits *below* `field`, not inside it. Drawing a helper inside the field would let it
    /// overlap the value on a short control, so the control's own rectangle has to bound the **pair** —
    /// see [`Self::total_height`], which is the number a layout must reserve.
    pub fn compute(
        field: Rect,
        padding_h: u32,
        line_height: u32,
        gap: u32,
        metrics: DecorationMetrics,
        counter_width: u32,
        slots: &DecorationSlots,
    ) -> Self {
        let padding = padding_h as i32;
        let gap = gap as i32;
        let inner_left = field.x + padding;
        let inner_right = field.x + field.width as i32 - padding;

        // The prefix occupies the leading edge, then its gap.
        let mut cursor = inner_left;
        let prefix = if metrics.prefix_width > 0 {
            let width = metrics.prefix_width as i32;
            let box_ = Rect::new(cursor, field.y, metrics.prefix_width, field.height);
            cursor += width + gap;
            Some(box_)
        } else {
            None
        };

        // The suffix is anchored to the **trailing** edge, so it does not move when the value's length
        // changes. A value that grows must not push the unit away from the edge it marks.
        let suffix = if metrics.suffix_width > 0 {
            let width = metrics.suffix_width as i32;
            let x = inner_right - width;
            // The value's own box must stop before the suffix *and* its gap, so a long value is bounded
            // by the slot instead of running under it.
            let box_ = Rect::new(x, field.y, metrics.suffix_width, field.height);
            cursor += 0; // the cursor stays at the value's start
            Some((box_, x - gap))
        } else {
            None
        };
        let (suffix_box, value_right) = match suffix {
            Some((box_, limit)) => (Some(box_), limit),
            None => (None, inner_right),
        };

        // The value owns everything between the prefix and the suffix's limit, and is clamped at zero
        // rather than allowed to go negative — a field narrower than its own slots is a squeezed field,
        // not an underflow.
        let value_width = (value_right - cursor).max(0) as u32;
        let value = Rect::new(cursor, field.y, value_width, field.height);

        // The support row, below the field.
        let has_counter = !slots.counter.is_empty() && counter_width > 0;
        let support = if slots.needs_support_row() {
            Some(Rect::new(field.x, field.y + field.height as i32 + gap, field.width, line_height))
        } else {
            None
        };
        let (support_text, counter_box) = match support {
            None => (None, None),
            Some(row) => {
                // The message takes the leading edge and the counter the trailing one. When there is
                // **only** a counter it still goes to the trailing edge, and the message box keeps the
                // row's full width so a caller can draw either without a second branch.
                let counter_box = if has_counter {
                    Some(Rect::new(
                        row.x + row.width as i32 - counter_width as i32,
                        row.y,
                        counter_width,
                        row.height,
                    ))
                } else {
                    None
                };
                (Some(row), counter_box)
            }
        };

        Self { prefix, value, suffix: suffix_box, counter: counter_box, support, support_text }
    }

    /// The total height the field and its support row occupy, including the gap between them.
    ///
    /// The number a layout must reserve, returned here rather than computed by the caller so the "is
    /// there a support row" question is answered once. A caller that guessed would either clip the row
    /// or leave a phantom band under a field with nothing to say.
    pub fn total_height(field_height: u32, line_height: u32, gap: u32, has_support: bool) -> u32 {
        if has_support {
            field_height.saturating_add(gap).saturating_add(line_height)
        } else {
            field_height
        }
    }

    /// The counter text for a value of `length` against an optional `max_length`.
    ///
    /// # Why the counter is derived and not stored
    ///
    /// A counter the caller has to keep in step with the text is a counter that goes stale — the field
    /// knows its own length, so asking it is the only way the two cannot disagree. `None` when there is
    /// no limit, because a counter with no budget to report is not a counter.
    pub fn counter_text(length: usize, max_length: Option<usize>) -> Option<String> {
        max_length.map(|max| format!("{length}/{max}"))
    }

    /// Whether a value of `length` is over `max_length`, and by how much.
    ///
    /// The field uses this to decide `has_error` on its own, so an over-long value is refused visibly
    /// rather than being silently truncated. Returns `0` when there is no limit or the value fits.
    pub fn over_limit(length: usize, max_length: Option<usize>) -> usize {
        match max_length {
            Some(max) => length.saturating_sub(max),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the renderer: each character advances 8 px, so a width is a function of the
    /// string rather than a number a test has to keep in step.
    fn measure(text: &str) -> u32 {
        text.chars().count() as u32 * 8
    }

    #[test]
    fn an_empty_decoration_set_costs_nothing() {
        let slots = DecorationSlots::new();
        assert!(slots.is_empty());
        assert!(!slots.needs_support_row());
        assert!(!slots.has_error());
        assert_eq!(slots.support_message(), "");

        let metrics = DecorationMetrics::measure(&slots, |_| 999);
        assert_eq!(metrics.prefix_width, 0, "an unset prefix must not be measured");
        assert_eq!(metrics.suffix_width, 0, "an unset suffix must not be measured");

        let field = Rect::new(10, 20, 200, 24);
        let layout = DecorationLayout::compute(field, 6, 16, 2, metrics, 0, &slots);
        assert_eq!(layout.prefix, None);
        assert_eq!(layout.suffix, None);
        assert_eq!(layout.support, None);
        assert_eq!(layout.counter, None);
        // With no slots the value owns the whole inner box, which is what makes adding the slots a
        // no-op for a caller that never sets one.
        assert_eq!(layout.value, Rect::new(16, 20, 188, 24));
    }

    /// The value starts *after* the prefix, and the caret origin follows it.
    ///
    /// # The defect this pins
    ///
    /// `spin_box` built its display text as `format!("{}{}{}", prefix, value, suffix)`. Placing the
    /// caret by measuring that string puts `cursor_position == 0` after the `$`, so the user cannot put
    /// the caret at the start of what they are editing, and a select-all copies the `$`.
    #[test]
    fn the_prefix_shifts_the_value_and_not_the_other_way_round() {
        let slots = DecorationSlots { prefix: "$".into(), ..Default::default() };
        let metrics = DecorationMetrics::measure(&slots, measure);
        assert_eq!(metrics.prefix_width, 8);
        let field = Rect::new(10, 20, 200, 24);
        let layout = DecorationLayout::compute(field, 6, 16, 2, metrics, 0, &slots);

        assert_eq!(layout.prefix, Some(Rect::new(16, 20, 8, 24)));
        assert_eq!(layout.value.x, 16 + 8 + 2, "the value begins after the prefix and its gap");
        assert_eq!(layout.suffix, None);
        assert!(layout.prefix.unwrap().right() <= layout.value.x, "the boxes must not overlap");
        // The value's box is narrower by exactly the prefix's share plus the gap.
        assert_eq!(layout.value.width, 188 - 8 - 2);
    }

    /// The suffix is anchored to the field's **trailing** edge, so it does not move when the value's
    /// length changes — a value that grows must not push the unit away from the edge it marks.
    #[test]
    fn the_suffix_is_anchored_to_the_trailing_edge() {
        let slots = DecorationSlots { suffix: "%".into(), ..Default::default() };
        let metrics = DecorationMetrics::measure(&slots, measure);
        assert_eq!(metrics.suffix_width, 8);
        let field = Rect::new(10, 20, 200, 24);
        let layout = DecorationLayout::compute(field, 6, 16, 2, metrics, 0, &slots);

        let suffix = layout.suffix.expect("a suffix is set");
        assert_eq!(suffix, Rect::new(10 + 200 - 6 - 8, 20, 8, 24));
        assert!(
            layout.value.right() <= suffix.x,
            "the value {:?} runs under the suffix {suffix:?}",
            layout.value
        );
        // The value keeps the leading padding when there is only a suffix.
        assert_eq!(layout.value.x, 16);
    }

    /// With both slots the value is squeezed from both sides and still ends between them.
    #[test]
    fn both_slots_bracket_the_value_without_overlapping_it() {
        let slots =
            DecorationSlots { prefix: "$".into(), suffix: "%".into(), ..Default::default() };
        let metrics = DecorationMetrics::measure(&slots, measure);
        let layout =
            DecorationLayout::compute(Rect::new(0, 0, 120, 24), 6, 16, 2, metrics, 0, &slots);

        let prefix = layout.prefix.expect("a prefix is set");
        let suffix = layout.suffix.expect("a suffix is set");
        assert!(prefix.right() <= layout.value.x, "{prefix:?} overlaps {:?}", layout.value);
        assert!(layout.value.right() <= suffix.x, "{:?} overlaps {suffix:?}", layout.value);
        assert!(layout.value.width > 0, "the value must keep room in a 120px field");
    }

    /// A field narrower than its slots gives the value nothing rather than a negative width.
    #[test]
    fn a_field_narrower_than_its_slots_clamps_the_value_to_zero() {
        let slots =
            DecorationSlots { prefix: "aa".into(), suffix: "bb".into(), ..Default::default() };
        let metrics = DecorationMetrics::measure(&slots, measure);
        assert_eq!(metrics.prefix_width, 16);
        // A 40 px field with 6 px padding each side leaves 28; the two 16 px slots plus gaps exceed it.
        let layout =
            DecorationLayout::compute(Rect::new(0, 0, 40, 24), 6, 16, 2, metrics, 0, &slots);
        assert_eq!(layout.value.width, 0, "a negative width must clamp, not underflow");
    }

    /// An error displaces the helper; a counter does not, and the two share the row.
    #[test]
    fn an_error_displaces_the_helper_but_not_the_counter() {
        let slots = DecorationSlots {
            helper: "Enter a URL".into(),
            error: "Not a URL".into(),
            counter: "12/40".into(),
            ..Default::default()
        };
        assert!(slots.has_error());
        assert_eq!(slots.support_message(), "Not a URL", "the error wins the row");

        let counter_width = measure("12/40");
        let layout = DecorationLayout::compute(
            Rect::new(0, 0, 200, 24),
            6,
            16,
            2,
            DecorationMetrics::default(),
            counter_width,
            &slots,
        );
        let row = layout.support.expect("there is a message and a counter");
        assert_eq!(row.y, 24 + 2, "the row sits below the field, past the gap");
        assert_eq!(row.height, 16);
        assert_eq!(row.x, 0, "the row starts at the field's own leading edge");
        assert_eq!(row.width, 200);

        let counter = layout.counter.expect("the counter shares the row, not displaced");
        assert_eq!(counter.width, counter_width);
        assert_eq!(counter.right(), row.right(), "the counter is anchored to the trailing edge");
        // The message box keeps the row's full width, so the caller's draw call needs no special case.
        assert_eq!(layout.support_text.expect("a message box"), row);
    }

    /// A counter with no message still needs the row, and still anchors to the trailing edge.
    #[test]
    fn a_counter_alone_still_occupies_the_row() {
        let slots = DecorationSlots { counter: "3/10".into(), ..Default::default() };
        assert!(slots.needs_support_row());
        assert_eq!(slots.support_message(), "", "there is no message to show");
        let layout = DecorationLayout::compute(
            Rect::new(0, 0, 100, 20),
            4,
            12,
            3,
            DecorationMetrics::default(),
            measure("3/10"),
            &slots,
        );
        assert!(layout.support.is_some(), "the counter alone reserves the row");
        assert!(layout.counter.is_some());
    }

    /// `needs_support_row` follows both the message and the counter, and clearing the error restores
    /// the helper.
    #[test]
    fn the_support_row_follows_what_is_actually_set() {
        let mut slots = DecorationSlots { helper: "Hint".into(), ..Default::default() };
        assert!(slots.needs_support_row());
        assert_eq!(slots.support_message(), "Hint");

        slots.error = "Bad".into();
        assert_eq!(slots.support_message(), "Bad");
        slots.error.clear();
        assert_eq!(slots.support_message(), "Hint", "clearing the error restores the helper");

        slots.helper.clear();
        assert!(!slots.needs_support_row(), "no message and no counter means no row");
    }

    /// The reserved height includes the support row only when there is one.
    #[test]
    fn the_reserved_height_follows_the_support_row() {
        assert_eq!(DecorationLayout::total_height(24, 16, 2, false), 24);
        assert_eq!(DecorationLayout::total_height(24, 16, 2, true), 42, "field + gap + line");
    }

    /// The support row sits outside the field box, so a short control draws it below rather than over
    /// the value.
    #[test]
    fn the_support_row_never_overlaps_the_field() {
        let slots = DecorationSlots { helper: "Hint".into(), ..Default::default() };
        let field = Rect::new(5, 5, 100, 20);
        let layout =
            DecorationLayout::compute(field, 4, 12, 3, DecorationMetrics::default(), 0, &slots);
        let row = layout.support.unwrap();
        assert!(
            row.y >= field.y + field.height as i32,
            "the row at {row:?} overlaps the field {field:?}"
        );
    }

    /// The counter text is derived from the field's own length and limit, so the two cannot go stale.
    #[test]
    fn the_counter_text_comes_from_the_length_and_the_limit() {
        assert_eq!(DecorationLayout::counter_text(0, None), None, "no limit means no counter");
        assert_eq!(DecorationLayout::counter_text(12, Some(40)).as_deref(), Some("12/40"));
        assert_eq!(DecorationLayout::counter_text(0, Some(5)).as_deref(), Some("0/5"));
    }

    /// An over-long value is reported rather than silently truncated.
    #[test]
    fn an_over_long_value_reports_how_far_over_it_is() {
        assert_eq!(DecorationLayout::over_limit(3, Some(5)), 0, "within the limit");
        assert_eq!(DecorationLayout::over_limit(5, Some(5)), 0, "exactly at the limit is not over");
        assert_eq!(DecorationLayout::over_limit(8, Some(5)), 3, "three characters too long");
        assert_eq!(DecorationLayout::over_limit(8, None), 0, "no limit can be exceeded");
    }
}
