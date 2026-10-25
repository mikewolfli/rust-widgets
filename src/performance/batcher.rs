// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Update batching for coallescing frame updates.
use super::region::DirtyRegionTracker;
use crate::compat::{Instant, Vec};
use crate::core::Rect;
/// Coalesces update rects into batches based on timeout thresholds.
///
/// Callers accumulate damaged rectangles with [`UpdateBatcher::add`] and
/// periodically ask [`UpdateBatcher::should_flush`]; flushing merges the rects
/// into a minimal set of regions. Batching reduces the number of repaint passes
/// at the cost of latency, so the timeout is the latency budget.
///
pub struct UpdateBatcher {
    pending_updates: Vec<Rect>,
    batch_timeout_ms: u64,
    last_batch: Option<Instant>,
    /// When the current pending batch's first rect was added.
    ///
    /// The timeout is a latency budget, so it must start from the **first pending update**, not
    /// from the first flush: a fresh batcher that receives a single rect and nothing else used to
    /// never reach the count threshold and never start its timer, so it could wait forever.
    first_pending: Option<Instant>,
}
impl UpdateBatcher {
    /// Creates an empty batcher that flushes at most every `batch_timeout_ms`
    /// milliseconds.
    pub fn new(batch_timeout_ms: u64) -> Self {
        Self {
            pending_updates: Vec::new(),
            batch_timeout_ms,
            last_batch: None,
            first_pending: None,
        }
    }
    /// Queues a damaged rectangle for the next flush.
    ///
    /// Rectangles are appended, not merged, so adding the same area repeatedly
    /// enlarges the pending list and can bring the count threshold forward.
    pub fn add(&mut self, rect: Rect) {
        if self.pending_updates.is_empty() {
            // The batch's timeout budget starts when its first rect arrives.
            self.first_pending = Some(Instant::now());
        }
        self.pending_updates.push(rect);
    }
    /// Returns whether the pending updates should be flushed now.
    ///
    /// True when there is something pending **and** either the timeout has elapsed since the
    /// first pending update, the timeout has elapsed since the last flush, or at least 10
    /// rectangles have accumulated. Always `false` with nothing pending. This only reports; it
    /// does not flush.
    pub fn should_flush(&self) -> bool {
        if self.pending_updates.is_empty() {
            return false;
        }
        // Latency budget: a small first batch is delivered within the timeout of its first rect,
        // even when the count threshold is never reached.
        if let Some(first) = self.first_pending {
            if first.elapsed().as_millis() as u64 >= self.batch_timeout_ms {
                return true;
            }
        }
        // Periodic cadence: never flush faster than the timeout after a previous flush.
        if let Some(last) = self.last_batch {
            if last.elapsed().as_millis() as u64 >= self.batch_timeout_ms {
                return true;
            }
        }
        self.pending_updates.len() >= 10
    }
    /// Drains the pending rectangles and returns them merged into a minimal set
    /// of non-overlapping regions.
    ///
    /// Also restarts the batch timer. Returns an empty vector when nothing is
    /// pending, in which case the timer is **not** restarted.
    pub fn flush(&mut self) -> Vec<Rect> {
        self.take_merged_tracker()
            .map(|tracker| tracker.regions.into_iter().map(|region| region.rect).collect())
            .unwrap_or_default()
    }

    /// Drains the batch into a tracker that has already been merged once.
    ///
    /// `flush_clipped` used to call `flush` and then rebuild a second tracker from its output and
    /// merge again — the same clip information computed twice per frame (G-31). The tracker is
    /// handed over already merged, so `render_dirty_regions`' own `merge()` is a no-op.
    fn take_merged_tracker(&mut self) -> Option<DirtyRegionTracker> {
        if self.pending_updates.is_empty() {
            return None;
        }
        let mut tracker = DirtyRegionTracker::new();
        for rect in self.pending_updates.drain(..) {
            tracker.add(rect);
        }
        tracker.merge();
        self.last_batch = Some(Instant::now());
        self.first_pending = None;
        Some(tracker)
    }
    /// Flush pending updates and render only dirty regions.
    pub fn flush_clipped(
        &mut self,
        ctx: &mut crate::render::RenderContext,
        render_all: impl FnMut(&mut crate::render::RenderContext),
    ) {
        let Some(mut tracker) = self.take_merged_tracker() else {
            return;
        };
        super::render_dirty_regions(&mut tracker, ctx, render_all);
    }

    /// Discards queued updates without flushing them.
    ///
    /// The batch is now empty, so the first-pending timer restarts on the next [`add`]. The
    /// last-flush timer is left alone: a clear immediately before the timeout does not buy
    /// extra time, so the next `add` may trigger a flush at once.
    pub fn clear(&mut self) {
        self.pending_updates.clear();
        self.first_pending = None;
    }
    /// Returns `true` when no update is waiting to be flushed.
    pub fn is_empty(&self) -> bool {
        self.pending_updates.is_empty()
    }
    /// Returns how many raw rectangles are queued, before merging.
    ///
    /// This counts each [`UpdateBatcher::add`] call, so it does not shrink when
    /// overlapping rects are added.
    pub fn len(&self) -> usize {
        self.pending_updates.len()
    }
}
impl Default for UpdateBatcher {
    fn default() -> Self {
        Self::new(16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend as _, RenderContext, SoftwarePaintBackend};

    /// Nothing queued means nothing to flush — and no timer may start.
    #[test]
    fn an_empty_batch_never_flushes() {
        let mut batcher = UpdateBatcher::new(0);
        assert!(!batcher.should_flush(), "an empty batch is never ready");
        assert!(batcher.is_empty());
        assert_eq!(batcher.flush(), Vec::new(), "and flushing it yields nothing");
        // A flush of nothing must not start the periodic timer either: the next `add`
        // still owns the first rect's latency budget.
        batcher.add(Rect::new(0, 0, 10, 10));
        assert!(batcher.should_flush(), "the zero-timeout budget starts at the first rect");
    }

    /// The count threshold is what flushes a burst without waiting for a clock.
    #[test]
    fn ten_rects_flush_a_batch_whose_timeout_has_not_elapsed() {
        let mut batcher = UpdateBatcher::new(60_000);
        for i in 0..9 {
            batcher.add(Rect::new(i * 20, 0, 10, 10));
            assert!(!batcher.should_flush(), "{} rects is below the threshold", i + 1);
        }
        batcher.add(Rect::new(200, 0, 10, 10));
        assert!(batcher.should_flush(), "ten rects flush regardless of the timeout");
        assert_eq!(batcher.len(), 10, "`len` counts raw adds, before merging");
    }

    /// A flush merges through the shared tracker, so the batch and the damage path agree.
    #[test]
    fn flush_merges_through_the_dirty_region_tracker() {
        let mut batcher = UpdateBatcher::new(60_000);
        batcher.add(Rect::new(0, 0, 100, 100));
        batcher.add(Rect::new(50, 50, 100, 100));
        batcher.add(Rect::new(400, 400, 10, 10));

        // What the tracker alone answers for the same three rects.
        let mut reference = DirtyRegionTracker::new();
        for rect in
            [Rect::new(0, 0, 100, 100), Rect::new(50, 50, 100, 100), Rect::new(400, 400, 10, 10)]
        {
            reference.add(rect);
        }
        reference.merge();
        let expected: Vec<Rect> = reference.regions().iter().map(|r| r.rect).collect();

        let flushed = batcher.flush();
        assert_eq!(flushed, expected, "the batch merges exactly as the damage tracker does");
        assert_eq!(flushed.len(), 2, "the two overlapping rects became one");
        assert!(batcher.is_empty(), "a flush drains the batch");
        assert!(batcher.flush().is_empty(), "and a second flush is empty");
    }

    /// The flush restarts the cadence, and `clear` drops the batch without buying time.
    #[test]
    fn flush_and_clear_treat_their_timers_differently() {
        let mut batcher = UpdateBatcher::new(60_000);
        for i in 0..10 {
            batcher.add(Rect::new(i * 20, 0, 10, 10));
        }
        assert!(batcher.should_flush());
        let _ = batcher.flush();

        // The last flush was just now, so a single new rect must not flush immediately.
        batcher.add(Rect::new(0, 0, 10, 10));
        assert!(!batcher.should_flush(), "the periodic cadence holds after a flush");

        // `clear` empties without touching the flush cadence: the next add is still
        // subject to the timer, which is what the documented contract says.
        batcher.clear();
        assert!(batcher.is_empty());
        assert!(!batcher.should_flush(), "nothing pending, so nothing to flush");
        batcher.add(Rect::new(0, 0, 10, 10));
        assert_eq!(batcher.len(), 1, "clear only drops what was queued");
    }

    /// `flush_clipped` paints exactly once for a merged batch, and not at all when empty.
    #[test]
    fn flush_clipped_paints_only_what_was_queued() {
        let frame = crate::core::Size::new(400, 300);
        let mut backend = SoftwarePaintBackend::new(frame, 1.0);
        backend.begin_frame(crate::core::Color::rgb(0, 0, 0));

        let mut batcher = UpdateBatcher::new(0);
        let mut paints = 0usize;
        {
            let mut ctx = RenderContext::new(&mut backend);
            // Nothing queued: the renderer must not be asked to paint.
            batcher.flush_clipped(&mut ctx, |_| paints += 1);
            assert_eq!(paints, 0, "an empty batch paints nothing");

            // Two overlapping rects are one merged region, so one clipped pass.
            batcher.add(Rect::new(10, 10, 40, 40));
            batcher.add(Rect::new(30, 30, 40, 40));
            batcher.flush_clipped(&mut ctx, |_| paints += 1);
        }
        assert_eq!(paints, 1, "a merged batch is one clipped paint");
    }
}
