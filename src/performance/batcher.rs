//! Update batching for coallescing frame updates.
use super::region::DirtyRegionTracker;
use crate::core::Rect;
/// Coalesces update rects into batches based on timeout thresholds.
pub struct UpdateBatcher {
    pending_updates: Vec<Rect>,
    batch_timeout_ms: u64,
    last_batch: Option<std::time::Instant>,
}
impl UpdateBatcher {
    pub fn new(batch_timeout_ms: u64) -> Self {
        Self { pending_updates: Vec::new(), batch_timeout_ms, last_batch: None }
    }
    pub fn add(&mut self, rect: Rect) {
        self.pending_updates.push(rect);
    }
    pub fn should_flush(&self) -> bool {
        if self.pending_updates.is_empty() {
            return false;
        }
        if let Some(last) = self.last_batch {
            if last.elapsed().as_millis() as u64 >= self.batch_timeout_ms {
                return true;
            }
        }
        self.pending_updates.len() >= 10
    }
    pub fn flush(&mut self) -> Vec<Rect> {
        if self.pending_updates.is_empty() {
            return Vec::new();
        }
        let mut tracker = DirtyRegionTracker::new();
        for rect in self.pending_updates.drain(..) {
            tracker.add(rect);
        }
        tracker.merge();
        self.last_batch = Some(std::time::Instant::now());
        tracker.regions.into_iter().map(|r| r.rect).collect()
    }
    pub fn clear(&mut self) {
        self.pending_updates.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.pending_updates.is_empty()
    }
    pub fn len(&self) -> usize {
        self.pending_updates.len()
    }
}
impl Default for UpdateBatcher {
    fn default() -> Self {
        Self::new(16)
    }
}
