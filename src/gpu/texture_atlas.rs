//! Texture atlas for GPU rendering — packs small textures into a larger atlas (BLUE11 R5.8).
use crate::core::Size;
use std::collections::HashMap;

/// A rectangle within the texture atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A single entry in the texture atlas.
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub rect: AtlasRect,
    pub texture_id: u64,
}

/// Simple texture atlas that packs textures into rows.
pub struct TextureAtlas {
    max_size: Size,
    entries: HashMap<u64, AtlasEntry>,
    next_id: u64,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl TextureAtlas {
    pub fn new(max_size: Size) -> Self {
        Self {
            max_size,
            entries: HashMap::new(),
            next_id: 1,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// Allocate a slot in the atlas. Returns the texture ID and rect.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u64, AtlasRect)> {
        // Simple row-based packing
        if self.cursor_x + width > self.max_size.width {
            // Start new row
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        if self.cursor_y + height > self.max_size.height {
            return None; // Atlas full
        }
        let id = self.next_id;
        self.next_id += 1;
        let rect = AtlasRect { x: self.cursor_x, y: self.cursor_y, width, height };
        self.entries.insert(id, AtlasEntry { rect, texture_id: id });
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);
        Some((id, rect))
    }

    pub fn get(&self, id: u64) -> Option<&AtlasEntry> {
        self.entries.get(&id)
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.remove(&id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn utilization(&self) -> f32 {
        if self.max_size.width == 0 || self.max_size.height == 0 {
            return 0.0;
        }
        let total_pixels = self.max_size.width as u64 * self.max_size.height as u64;
        let used_pixels: u64 =
            self.entries.values().map(|e| e.rect.width as u64 * e.rect.height as u64).sum();
        used_pixels as f32 / total_pixels as f32
    }
}
