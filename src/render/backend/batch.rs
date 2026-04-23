use crate::core::{Color, Point, Rect};
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchId(u64);
impl BatchId {
    pub fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
impl Default for BatchId {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderCommand {
    FillRect,
    StrokeRect,
    FillRoundedRect,
    StrokeRoundedRect,
    DrawLine,
    DrawText,
    DrawImage,
    DrawPath,
    ClipRect,
    Transform,
}
#[derive(Debug, Clone)]
pub struct RenderItem {
    pub command: RenderCommand,
    pub rect: Rect,
    pub color: Color,
    pub layer: u32,
    pub z_index: i32,
    pub extra_data: Option<Vec<u8>>,
}
impl RenderItem {
    pub fn new(command: RenderCommand, rect: Rect, color: Color) -> Self {
        Self {
            command,
            rect,
            color,
            layer: 0,
            z_index: 0,
            extra_data: None,
        }
    }
    pub fn with_layer(mut self, layer: u32) -> Self {
        self.layer = layer;
        self
    }
    pub fn with_z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }
}
#[derive(Debug, Clone)]
pub struct RenderBatch {
    pub id: BatchId,
    pub items: Vec<RenderItem>,
    pub clip_rect: Option<Rect>,
    pub layer: u32,
}
impl RenderBatch {
    pub fn new(layer: u32) -> Self {
        Self {
            id: BatchId::new(),
            items: Vec::new(),
            clip_rect: None,
            layer,
        }
    }
    pub fn add(&mut self, item: RenderItem) {
        self.items.push(item);
    }
    pub fn add_rect(&mut self, rect: Rect, color: Color) {
        self.items
            .push(RenderItem::new(RenderCommand::FillRect, rect, color));
    }
    pub fn add_stroke_rect(&mut self, rect: Rect, color: Color) {
        self.items
            .push(RenderItem::new(RenderCommand::StrokeRect, rect, color));
    }
    pub fn add_line(&mut self, start: Point, end: Point, color: Color) {
        let rect = Rect::new(
            start.x.min(end.x),
            start.y.min(end.y),
            (end.x - start.x).abs() as u32 + 1,
            (end.y - start.y).abs() as u32 + 1,
        );
        self.items
            .push(RenderItem::new(RenderCommand::DrawLine, rect, color));
    }
    pub fn set_clip(&mut self, rect: Rect) {
        self.clip_rect = Some(rect);
    }
    pub fn clear_clip(&mut self) {
        self.clip_rect = None;
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn sort_by_z(&mut self) {
        self.items.sort_by_key(|item| item.z_index);
    }
    pub fn optimize(&mut self) {
        self.merge_adjacent_rects();
        self.remove_duplicates();
    }
    fn merge_adjacent_rects(&mut self) {
        if self.items.len() < 2 {
            return;
        }
        let mut merged = Vec::new();
        let mut current: Option<RenderItem> = None;
        for item in self.items.drain(..) {
            if let Some(ref mut curr) = current {
                if curr.command == item.command
                    && curr.color == item.color
                    && curr.layer == item.layer
                    && curr.z_index == item.z_index
                    && curr.command == RenderCommand::FillRect
                    && curr.rect.intersects(&item.rect)
                {
                    curr.rect = curr.rect.union(&item.rect);
                } else {
                    merged.push(curr.clone());
                    *curr = item;
                }
            } else {
                current = Some(item);
            }
        }
        if let Some(item) = current {
            merged.push(item);
        }
        self.items = merged;
    }
    fn remove_duplicates(&mut self) {
        let mut seen = HashMap::new();
        self.items.retain(|item| {
            let key = (
                item.command,
                item.rect.x,
                item.rect.y,
                item.rect.width,
                item.rect.height,
                item.color,
            );
            if seen.contains_key(&key) {
                false
            } else {
                seen.insert(key, true);
                true
            }
        });
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.clip_rect = None;
    }
}
pub struct BatchBuilder {
    batches: Vec<RenderBatch>,
    current_batch: Option<RenderBatch>,
    max_batch_size: usize,
}
impl BatchBuilder {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            current_batch: None,
            max_batch_size: 1000,
        }
    }
    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }
    pub fn begin_batch(&mut self, layer: u32) {
        if let Some(batch) = self.current_batch.take() {
            if !batch.is_empty() {
                self.batches.push(batch);
            }
        }
        self.current_batch = Some(RenderBatch::new(layer));
    }
    pub fn end_batch(&mut self) {
        if let Some(batch) = self.current_batch.take() {
            if !batch.is_empty() {
                self.batches.push(batch);
            }
        }
    }
    pub fn add(&mut self, item: RenderItem) {
        let layer = item.layer;
        if self.current_batch.is_none() {
            self.begin_batch(layer);
        }
        if let Some(ref mut batch) = self.current_batch {
            if batch.layer != layer || batch.len() >= self.max_batch_size {
                let old_batch = std::mem::replace(batch, RenderBatch::new(layer));
                if !old_batch.is_empty() {
                    self.batches.push(old_batch);
                }
            }
            batch.add(item);
        }
    }
    pub fn add_rect(&mut self, rect: Rect, color: Color, layer: u32) {
        self.add(RenderItem::new(RenderCommand::FillRect, rect, color).with_layer(layer));
    }
    pub fn add_stroke_rect(&mut self, rect: Rect, color: Color, layer: u32) {
        self.add(RenderItem::new(RenderCommand::StrokeRect, rect, color).with_layer(layer));
    }
    pub fn build(mut self) -> Vec<RenderBatch> {
        self.end_batch();
        for batch in &mut self.batches {
            batch.optimize();
        }
        self.batches.sort_by_key(|b| b.layer);
        self.batches
    }
    pub fn clear(&mut self) {
        self.batches.clear();
        self.current_batch = None;
    }
    pub fn batch_count(&self) -> usize {
        self.batches.len() + if self.current_batch.is_some() { 1 } else { 0 }
    }
    pub fn item_count(&self) -> usize {
        let mut count: usize = self.batches.iter().map(|b| b.len()).sum();
        if let Some(ref batch) = self.current_batch {
            count += batch.len();
        }
        count
    }
}
impl Default for BatchBuilder {
    fn default() -> Self {
        Self::new()
    }
}
pub struct RenderQueue {
    batches: Vec<RenderBatch>,
    current_index: usize,
}
impl RenderQueue {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            current_index: 0,
        }
    }
    pub fn from_batches(batches: Vec<RenderBatch>) -> Self {
        Self {
            batches,
            current_index: 0,
        }
    }
    pub fn submit(&mut self, batch: RenderBatch) {
        self.batches.push(batch);
    }
    pub fn next(&mut self) -> Option<&RenderBatch> {
        if self.current_index < self.batches.len() {
            let batch = &self.batches[self.current_index];
            self.current_index += 1;
            Some(batch)
        } else {
            None
        }
    }
    pub fn reset(&mut self) {
        self.current_index = 0;
    }
    pub fn clear(&mut self) {
        self.batches.clear();
        self.current_index = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }
    pub fn len(&self) -> usize {
        self.batches.len()
    }
    pub fn total_items(&self) -> usize {
        self.batches.iter().map(|b| b.len()).sum()
    }
    pub fn sort_by_layer(&mut self) {
        self.batches.sort_by_key(|b| b.layer);
    }
    pub fn clip_to(&mut self, rect: &Rect) {
        for batch in &mut self.batches {
            batch.set_clip(*rect);
        }
    }
}
impl Default for RenderQueue {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_batch_builder() {
        let mut builder = BatchBuilder::new();
        builder.add_rect(Rect::new(0, 0, 100, 100), Color::RED, 0);
        builder.add_rect(Rect::new(50, 50, 100, 100), Color::BLUE, 0);
        let batches = builder.build();
        assert!(!batches.is_empty());
    }
    #[test]
    fn test_render_batch() {
        let mut batch = RenderBatch::new(0);
        batch.add_rect(Rect::new(0, 0, 50, 50), Color::RED);
        batch.add_rect(Rect::new(60, 60, 50, 50), Color::BLUE);
        assert_eq!(batch.len(), 2);
        batch.optimize();
        assert_eq!(batch.len(), 2);
    }
}
