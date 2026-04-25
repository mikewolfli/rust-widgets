//! Performance monitoring and optimization utilities, including dirty region tracking, update batching, and profiling.
pub mod batcher;
pub mod dirty;
mod profiler;
pub mod region;
pub use batcher::*;
pub use dirty::*;
pub use profiler::*;
pub use region::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    #[test]
    fn test_dirty_region_tracker() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.add(Rect::new(0, 0, 100, 100));
        tracker.add(Rect::new(50, 50, 100, 100));
        assert_eq!(tracker.len(), 2);
        tracker.merge();
        assert_eq!(tracker.len(), 1);
        let bounding = tracker.get_bounding_rect().unwrap();
        assert_eq!(bounding.x, 0);
        assert_eq!(bounding.y, 0);
        assert_eq!(bounding.width, 150);
        assert_eq!(bounding.height, 150);
    }
    #[test]
    fn test_update_batcher() {
        let mut batcher = UpdateBatcher::new(100);
        batcher.add(Rect::new(0, 0, 10, 10));
        batcher.add(Rect::new(20, 20, 10, 10));
        assert_eq!(batcher.len(), 2);
        let rects = batcher.flush();
        assert!(!rects.is_empty());
        assert!(batcher.is_empty());
    }
}
