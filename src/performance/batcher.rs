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
        if self.pending_updates.is_empty() {
            return Vec::new();
        }
        let mut tracker = DirtyRegionTracker::new();
        for rect in self.pending_updates.drain(..) {
            tracker.add(rect);
        }
        tracker.merge();
        self.last_batch = Some(Instant::now());
        self.first_pending = None;
        tracker.regions.into_iter().map(|r| r.rect).collect()
    }
    /// Flush pending updates and render only dirty regions.
    pub fn flush_clipped(
        &mut self,
        ctx: &mut crate::render::RenderContext,
        render_all: impl FnMut(&mut crate::render::RenderContext),
    ) {
        let rects = self.flush();
        if rects.is_empty() {
            return;
        }
        let mut tracker = DirtyRegionTracker::new();
        for rect in rects {
            tracker.add(rect);
        }
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
