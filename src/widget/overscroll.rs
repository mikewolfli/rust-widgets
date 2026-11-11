// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Overscroll: what a control does when a gesture runs past its own content (UX6-08).
//!
//! # The three languages this module replaces
//!
//! A pointer drag that reaches the end of a scrollable surface can be answered in exactly three
//! ways, and before this module each control answered it in its own:
//!
//! * **Hard clamp** — the offset stops at the content edge and nothing moves. `ScrollArea` clamps
//!   in `set_scroll_position` and that is the right default for a dense table or a form: the ink
//!   and the hit test stay in the same place.
//! * **Rubber band** — the offset follows the pointer at a damped rate, so the content visibly
//!   resists and springs back on release. `Carousel` implemented the damping inline (`raw / 3`)
//!   and the spring-back inline (`offset *= 1.0 - delta / 200`), which is where the number lived
//!   for exactly one control.
//! * **Hand-written spring-back** — the release animation. Same shape, different numbers, per
//!   control.
//!
//! The first is a policy and stays a policy: [`clamp_to_range`] is here so a control that wants it
//! says so through this module rather than through a bare `clamp`. The other two are **maths**, and
//! maths that exists in three places drifts. [`damp`] and [`settle_step`] are that maths.
//!
//! # Why the numbers are constants rather than parameters
//!
//! A damping divisor is a feel, not a policy: two controls that damp by different amounts read as
//! two different platforms. Callers that genuinely need a different feel (a pitch-locked track, a
//! photo viewer at a fitted zoom) can pass their own fraction to [`damp_with`]; everything else
//! uses [`damp`], which is the value `Carousel` shipped and the one the crate documents.

/// The divisor applied to pointer travel past a boundary.
///
/// `3.0` means the content follows a third of the overscrolled distance: it moves, so the user
/// knows the gesture arrived, and it visibly resists, so the gesture does not read as a scroll
/// that silently did nothing.
pub const DAMPING_DIVISOR: f32 = 3.0;

/// Below this many logical pixels an offset counts as at rest.
///
/// A settle animation that stepped forever would keep the frame clock awake; below half a pixel
/// the offset cannot change the painted picture, so it is the honest stopping point.
pub const SETTLE_THRESHOLD: f32 = 0.5;

/// The default duration of a spring-back from a released overscroll.
pub const SETTLE_DURATION_MS: f32 = 200.0;

/// Damps pointer travel that has run past the content boundary.
///
/// # Why this is not an easing curve
///
/// The offset is applied *while the pointer is down*, so it has to be a position, not a fraction
/// of an animation: the user can reverse the drag mid-gesture and the content has to follow back
/// out of the overscroll by the same rule. Dividing the travel is exactly that — a linear map
/// with a slope under one.
pub fn damp(delta: f32) -> f32 {
    damp_with(delta, DAMPING_DIVISOR)
}

/// [`damp`] with an explicit divisor, for a control whose travel needs a different resistance.
///
/// A divisor at or below zero would multiply the travel (or invert it), which is not a damping at
/// all, so it falls back to [`DAMPING_DIVISOR`].
pub fn damp_with(delta: f32, divisor: f32) -> f32 {
    let divisor = if divisor > 0.0 { divisor } else { DAMPING_DIVISOR };
    delta / divisor
}

/// One frame of a spring-back from `offset`, over a [`SETTLE_DURATION_MS`] animation.
///
/// Returns the remaining offset. The decay is linear in the frame time rather than a fixed
/// per-frame fraction, so the animation takes the same wall-clock time at 30 fps and at 120 fps;
/// and it snaps to exactly zero at [`SETTLE_THRESHOLD`] so a caller can test `offset == 0.0`
/// to learn that the animation is over.
pub fn settle_step(offset: f32, delta_ms: u32) -> f32 {
    settle_step_over(offset, delta_ms, SETTLE_DURATION_MS)
}

/// [`settle_step`] with an explicit duration, for a control that settles faster or slower.
pub fn settle_step_over(offset: f32, delta_ms: u32, duration_ms: f32) -> f32 {
    if offset.abs() <= SETTLE_THRESHOLD {
        return 0.0;
    }
    let duration = if duration_ms > 0.0 { duration_ms } else { SETTLE_DURATION_MS };
    let fraction = (delta_ms as f32 / duration).clamp(0.0, 1.0);
    let remaining = offset * (1.0 - fraction);
    if remaining.abs() <= SETTLE_THRESHOLD {
        0.0
    } else {
        remaining
    }
}

/// Whether an offset still needs frames to settle.
pub fn is_settling(offset: f32) -> bool {
    offset.abs() > SETTLE_THRESHOLD
}

/// Clamps a scroll offset to `0..=(content - viewport)`, the **no-feedback** policy.
///
/// This is `ScrollArea`'s rule, stated once: a negative maximum (content narrower than the
/// viewport) clamps to zero rather than to a negative bound, so the offset can never invert the
/// content.
pub fn clamp_to_range(offset: i32, content: i32, viewport: i32) -> i32 {
    offset.clamp(0, (content - viewport).max(0))
}

/// Whether a scroll indicator has anything to report: the content is larger than the viewport.
///
/// # Why "overflow only" is the default (UX-04)
///
/// Six containers answered "should I paint a scroll bar?" differently: a fade in `ScrollArea`, an
/// opt-in flag in `TreeView` / `DataGrid` / `GridTable` / `VirtualTable` / `VirtualList` (off, so
/// a scrolling grid showed no indication at all), a static bar in `TextArea`, and **nothing** in
/// `ListView`. A control that can scroll and shows nothing is the "silently refuses input" shape
/// in chrome: the reader has no way to know more rows exist.
///
/// The default is therefore *derived*, not configured: paint the thumb exactly when there is
/// something off-screen. The explicit flag on the flag-bearing containers stays as a **force-on**
/// override (a host that wants a permanent rail keeps it), never as a way to hide an overflow.
///
/// This is deliberately *not* the fade policy: fading needs a per-frame clock and a tick, which is
/// what `ScrollArea`'s mounted `ScrollBar` already owns. A container that repaints only when its
/// data moves keeps a static thumb, which is the cheaper and equally honest answer.
pub fn content_overflows(total: usize, visible: usize) -> bool {
    total > visible
}

/// The thumb rectangle for a vertical scroll indicator, or `None` when nothing is off-screen.
///
/// `track` is the band the bar occupies and whose **right edge** is where the thumb's right edge
/// sits (a caller that keeps a 2 px inset hands in a track two pixels narrower); `total`,
/// `visible` and `offset` are in the control's own scroll unit — rows for the tables, lines for a
/// grid-laid-out list — so no caller has to convert to pixels first.
///
/// `force` is the container's own flag: when the content fits, `force = false` draws nothing and
/// `force = true` draws a full-height rail (the "permanent rail" a host can ask for). When the
/// content overflows the flag makes no difference — an overflowing container always reports it.
///
/// # Why this is shared
///
/// Six controls computed this by hand (`GridTable`, `TreeView`, `DataGrid`, `VirtualTable`,
/// `VirtualList`; `ListView` had none at all). The arithmetic is the same for every one of them,
/// and a thumb whose length or travel disagrees with the offset it reports is worse than no thumb:
/// it tells the reader they are further along than they are. The 24 px floor is the smallest
/// target that still reads as a position rather than a speck.
pub fn scroll_thumb_rect(
    track: crate::core::Rect,
    total: usize,
    visible: usize,
    offset: usize,
    width: u32,
    force: bool,
) -> Option<crate::core::Rect> {
    if track.width == 0 || track.height == 0 {
        return None;
    }
    let total = total.max(1);
    let visible = visible.max(1).min(total);
    if !force && !content_overflows(total, visible) {
        return None;
    }
    let height = track.height;
    let thumb = ((height as u64 * visible as u64) / total as u64).max(24).min(height as u64) as u32;
    let travel = height.saturating_sub(thumb);
    let max_scroll = total.saturating_sub(visible);
    let along = if max_scroll == 0 {
        0
    } else {
        (travel as u64 * offset.min(max_scroll) as u64 / max_scroll as u64) as u32
    };
    Some(crate::core::Rect::new(
        track.x + track.width as i32 - width as i32,
        track.y + along as i32,
        width,
        thumb,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_damped_drag_follows_a_fraction_of_the_travel() {
        assert_eq!(damp(30.0), 10.0);
        assert_eq!(damp(-30.0), -10.0);
        assert_eq!(damp(0.0), 0.0);
    }

    #[test]
    fn a_non_positive_divisor_cannot_amplify_the_travel() {
        // A zero or negative divisor would make the content run *away* from the pointer, so the
        // documented default is used instead of dividing by zero.
        assert_eq!(damp_with(30.0, 0.0), 10.0);
        assert_eq!(damp_with(30.0, -2.0), 10.0);
        assert_eq!(damp_with(30.0, 6.0), 5.0);
    }

    #[test]
    fn a_settle_animation_snaps_to_rest_and_finishes() {
        let mut offset = 90.0f32;
        // Half the duration leaves half the offset — the decay is in time, not in frames, which
        // is what keeps a 30 fps host and a 120 fps host settling over the same wall-clock span.
        offset = settle_step(offset, 100);
        assert!((offset - 45.0).abs() < 0.01, "half the time is half the travel: {offset}");
        offset = settle_step(offset, 100);
        assert!((offset - 22.5).abs() < 0.01, "the same fraction applies again: {offset}");
        // A frame as long as the whole duration lands on zero, not on a fraction of a pixel.
        assert_eq!(settle_step(offset, 200), 0.0);
        // And a tiny offset is already at rest.
        assert_eq!(settle_step(0.4, 16), 0.0);
        assert!(!is_settling(0.4));
        assert!(is_settling(0.6));
    }

    #[test]
    fn the_clamp_never_inverts_a_short_content() {
        assert_eq!(clamp_to_range(50, 400, 100), 50);
        assert_eq!(clamp_to_range(-5, 400, 100), 0);
        assert_eq!(clamp_to_range(500, 400, 100), 300);
        // Content narrower than the viewport has no scrollable range at all.
        assert_eq!(clamp_to_range(20, 40, 100), 0);
    }

    /// UX-04: the indicator is derived from the content, not configured.
    #[test]
    fn an_indicator_is_needed_only_when_something_is_off_screen() {
        assert!(content_overflows(20, 5), "ten rows in a five-row viewport scroll");
        assert!(!content_overflows(5, 5), "exactly one page fits: nothing is hidden");
        assert!(!content_overflows(2, 5), "a short list has no bar to draw");
        assert!(!content_overflows(0, 0));
    }

    /// The thumb's length, travel and placement are one derivation for every container.
    #[test]
    fn the_thumb_is_proportional_and_stays_inside_its_track() {
        use crate::core::Rect;
        let track = Rect::new(10, 20, 100, 200);

        // No overflow: no thumb at all, rather than a full-height one that says nothing.
        assert_eq!(scroll_thumb_rect(track, 5, 5, 0, 6, false), None);

        // At rest: a quarter of the track (200 * 5/20), at the top, on the trailing edge.
        let top = scroll_thumb_rect(track, 20, 5, 0, 6, false).expect("twenty rows overflow five");
        assert_eq!(top, Rect::new(104, 20, 6, 50));

        // At the end: the same length, flush with the track's bottom edge.
        let bottom = scroll_thumb_rect(track, 20, 5, 15, 6, false).expect("still overflowing");
        assert_eq!(bottom, Rect::new(104, 170, 6, 50));

        // Halfway: the travel is split proportionally.
        let middle = scroll_thumb_rect(track, 20, 5, 7, 6, false).expect("still overflowing");
        assert_eq!(middle.y, 20 + ((150u64 * 7) / 15) as i32);

        // A tiny fraction of a huge model still gets a legible thumb, never a speck.
        let tiny = scroll_thumb_rect(track, 10_000, 1, 0, 6, false)
            .expect("ten thousand rows overflow one");
        assert_eq!(tiny.height, 24);
    }

    /// The flag is a force-on *rail*: it adds the indicator when the content fits, and it can
    /// never hide an overflow.
    #[test]
    fn the_flag_forces_a_rail_when_the_content_fits() {
        use crate::core::Rect;
        let track = Rect::new(10, 20, 100, 200);

        // Fitting content with the flag off: nothing to say, so nothing drawn.
        assert_eq!(scroll_thumb_rect(track, 5, 5, 0, 6, false), None);
        // Fitting content with the flag on: the whole track is the thumb — a permanent rail.
        assert_eq!(scroll_thumb_rect(track, 5, 5, 0, 6, true), Some(Rect::new(104, 20, 6, 200)));
        // Overflowing content looks identical either way: the flag is never a way to hide it.
        assert_eq!(
            scroll_thumb_rect(track, 20, 5, 0, 6, false),
            scroll_thumb_rect(track, 20, 5, 0, 6, true),
        );
    }
}
