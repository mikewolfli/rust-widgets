use crate::core::{Color, Rect, Size};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
#[derive(Debug, Clone)]
pub struct TextKey {
    pub text: String,
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub color: Color,
}
impl TextKey {
    pub fn new(text: &str, font_family: &str, font_size: u16, color: Color) -> Self {
        Self {
            text: text.to_string(),
            font_family: font_family.to_string(),
            font_size,
            font_weight: 400,
            color,
        }
    }
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.font_weight = weight;
        self
    }
}
impl PartialEq for TextKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_family == other.font_family
            && self.font_size == other.font_size
            && self.font_weight == other.font_weight
            && self.color == other.color
    }
}
impl Eq for TextKey {}
impl Hash for TextKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_family.hash(state);
        self.font_size.hash(state);
        self.font_weight.hash(state);
        self.color.r.hash(state);
        self.color.g.hash(state);
        self.color.b.hash(state);
        self.color.a.hash(state);
    }
}
#[derive(Debug, Clone)]
pub struct CachedText {
    pub key: TextKey,
    pub size: Size,
    pub bounds: Rect,
    pub data: Vec<u8>,
    pub timestamp: u64,
}
impl CachedText {
    pub fn new(key: TextKey, size: Size, bounds: Rect) -> Self {
        Self {
            key,
            size,
            bounds,
            data: Vec::new(),
            timestamp: 0,
        }
    }
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }
    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub max_memory_bytes: usize,
    pub ttl_seconds: u64,
}
impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_bytes: 10 * 1024 * 1024,
            ttl_seconds: 300,
        }
    }
}
pub struct TextCache {
    cache: HashMap<TextKey, CachedText>,
    config: CacheConfig,
    current_memory: usize,
    current_timestamp: u64,
    hits: u64,
    misses: u64,
}
impl TextCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: HashMap::new(),
            config,
            current_memory: 0,
            current_timestamp: 0,
            hits: 0,
            misses: 0,
        }
    }
    pub fn get(&mut self, key: &TextKey) -> Option<&CachedText> {
        self.current_timestamp += 1;
        if let Some(cached) = self.cache.get(key) {
            if self.is_expired(cached) {
                self.cache.remove(key);
                self.misses += 1;
                return None;
            }
            self.hits += 1;
            Some(self.cache.get(key).unwrap())
        } else {
            self.misses += 1;
            None
        }
    }
    pub fn get_mut(&mut self, key: &TextKey) -> Option<&mut CachedText> {
        self.current_timestamp += 1;
        if let Some(cached) = self.cache.get(key) {
            if self.is_expired(cached) {
                self.cache.remove(key);
                self.misses += 1;
                return None;
            }
            self.hits += 1;
            Some(self.cache.get_mut(key).unwrap())
        } else {
            self.misses += 1;
            None
        }
    }
    pub fn insert(&mut self, cached: CachedText) {
        let size = cached.data.len();
        let key = cached.key.clone();
        if size > self.config.max_memory_bytes {
            return;
        }
        while self.cache.len() >= self.config.max_entries
            || self.current_memory + size > self.config.max_memory_bytes
        {
            if !self.evict_lru() {
                break;
            }
        }
        self.current_memory += size;
        let cached = cached.with_timestamp(self.current_timestamp);
        self.cache.insert(key, cached);
    }
    pub fn remove(&mut self, key: &TextKey) -> Option<CachedText> {
        if let Some(cached) = self.cache.remove(key) {
            self.current_memory -= cached.data.len();
            Some(cached)
        } else {
            None
        }
    }
    pub fn contains(&self, key: &TextKey) -> bool {
        self.cache.contains_key(key)
    }
    pub fn clear(&mut self) {
        self.cache.clear();
        self.current_memory = 0;
    }
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
    pub fn memory_usage(&self) -> usize {
        self.current_memory
    }
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            memory_bytes: self.current_memory,
            hits: self.hits,
            misses: self.misses,
            hit_rate: self.hit_rate(),
        }
    }
    fn is_expired(&self, cached: &CachedText) -> bool {
        if self.config.ttl_seconds == 0 {
            return false;
        }
        let age = self.current_timestamp.saturating_sub(cached.timestamp);
        age > self.config.ttl_seconds * 60
    }
    fn evict_lru(&mut self) -> bool {
        if self.cache.is_empty() {
            return false;
        }
        let oldest_key = self
            .cache
            .iter()
            .min_by_key(|(_, v)| v.timestamp)
            .map(|(k, _)| k.clone());
        if let Some(key) = oldest_key {
            self.remove(&key);
            return true;
        }
        false
    }
    pub fn prune_expired(&mut self) {
        let expired: Vec<TextKey> = self
            .cache
            .iter()
            .filter(|(_, v)| self.is_expired(v))
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            self.remove(&key);
        }
    }
}
impl Default for TextCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub entries: usize,
    pub memory_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f32,
}
pub struct GlyphCache {
    glyphs: HashMap<(char, u16, String), GlyphInfo>,
    max_entries: usize,
}
#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub char: char,
    pub size: u16,
    pub font_family: String,
    pub width: u32,
    pub height: u32,
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub data: Vec<u8>,
}
impl GlyphCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            glyphs: HashMap::new(),
            max_entries,
        }
    }
    pub fn get(&self, c: char, size: u16, font_family: &str) -> Option<&GlyphInfo> {
        self.glyphs.get(&(c, size, font_family.to_string()))
    }
    pub fn insert(&mut self, glyph: GlyphInfo) {
        while self.glyphs.len() >= self.max_entries {
            if let Some(key) = self.glyphs.keys().next().cloned() {
                self.glyphs.remove(&key);
            } else {
                break;
            }
        }
        let key = (glyph.char, glyph.size, glyph.font_family.clone());
        self.glyphs.insert(key, glyph);
    }
    pub fn clear(&mut self) {
        self.glyphs.clear();
    }
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}
impl Default for GlyphCache {
    fn default() -> Self {
        Self::new(10000)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_text_cache() {
        let mut cache = TextCache::new(CacheConfig {
            max_entries: 10,
            max_memory_bytes: 1024 * 1024,
            ttl_seconds: 0,
        });
        let key = TextKey::new("Hello", "Arial", 12, Color::BLACK);
        let cached = CachedText::new(key.clone(), Size::new(50, 20), Rect::new(0, 0, 50, 20))
            .with_data(vec![0u8; 100]);
        cache.insert(cached);
        assert!(cache.contains(&key));
        assert_eq!(cache.len(), 1);
    }
    #[test]
    fn test_glyph_cache() {
        let mut cache = GlyphCache::new(100);
        let glyph = GlyphInfo {
            char: 'A',
            size: 12,
            font_family: "Arial".to_string(),
            width: 10,
            height: 12,
            advance: 10.0,
            bearing_x: 0.0,
            bearing_y: 10.0,
            data: vec![0u8; 120],
        };
        cache.insert(glyph);
        assert!(cache.get('A', 12, "Arial").is_some());
    }
}
