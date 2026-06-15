use crate::compat::{HashMap, Instant};
use crate::core::{Color, Rect, Size};
use core::hash::{Hash, Hasher};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    pub(crate) access_order: u64,
    pub created_at: Instant,
}
impl CachedText {
    pub fn new(key: TextKey, size: Size, bounds: Rect) -> Self {
        Self { key, size, bounds, data: Vec::new(), access_order: 0, created_at: Instant::now() }
    }
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }
    pub fn with_access_order(mut self, order: u64) -> Self {
        self.access_order = order;
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
        Self { max_entries: 1000, max_memory_bytes: 10 * 1024 * 1024, ttl_seconds: 300 }
    }
}
pub struct TextCache {
    cache: HashMap<TextKey, CachedText>,
    config: CacheConfig,
    current_memory: usize,
    access_counter: u64,
    hits: u64,
    misses: u64,
}
impl TextCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: HashMap::new(),
            config,
            current_memory: 0,
            access_counter: 0,
            hits: 0,
            misses: 0,
        }
    }
    pub fn get(&mut self, key: &TextKey) -> Option<&CachedText> {
        self.access_counter += 1;
        // Check expiry with an immutable lookup first
        let needs_removal = self.cache.get(key).map_or(false, |entry| self.is_expired(entry));
        if needs_removal {
            self.cache.remove(key);
            self.misses += 1;
            return None;
        }
        // Single mutable lookup: update LRU order and return
        match self.cache.get_mut(key) {
            Some(entry) => {
                entry.access_order = self.access_counter;
                self.hits += 1;
                // Reborrow &mut → & for the shared return type
                Some(entry)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }
    pub fn get_mut(&mut self, key: &TextKey) -> Option<&mut CachedText> {
        self.access_counter += 1;
        // Check expiry with an immutable lookup first
        let needs_removal = self.cache.get(key).map_or(false, |entry| self.is_expired(entry));
        if needs_removal {
            self.cache.remove(key);
            self.misses += 1;
            return None;
        }
        match self.cache.get_mut(key) {
            Some(entry) => {
                entry.access_order = self.access_counter;
                self.hits += 1;
                Some(entry)
            }
            None => {
                self.misses += 1;
                None
            }
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
        let cached = cached.with_access_order(self.access_counter);
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
        cached.created_at.elapsed() > std::time::Duration::from_secs(self.config.ttl_seconds)
    }
    fn evict_lru(&mut self) -> bool {
        if self.cache.is_empty() {
            return false;
        }
        let oldest_key =
            self.cache.iter().min_by_key(|(_, v)| v.access_order).map(|(k, _)| k.clone());
        if let Some(key) = oldest_key {
            self.remove(&key);
            return true;
        }
        false
    }
    pub fn prune_expired(&mut self) {
        let expired: Vec<TextKey> =
            self.cache.iter().filter(|(_, v)| self.is_expired(v)).map(|(k, _)| k.clone()).collect();
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
        Self { glyphs: HashMap::new(), max_entries }
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
