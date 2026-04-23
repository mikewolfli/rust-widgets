use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
const MAX_HISTORY_ENTRIES: usize = 100;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visit: u64,
}
impl HistoryEntry {
    pub fn new(url: String, title: String) -> Self {
        Self {
            url,
            title,
            visit_count: 1,
            last_visit: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    pub fn touch(&mut self) {
        self.visit_count += 1;
        self.last_visit = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}
#[derive(Debug, Clone, Default)]
pub struct BrowserHistory {
    entries: VecDeque<HistoryEntry>,
    max_entries: usize,
}
impl BrowserHistory {
    pub fn new() -> Self {
        Self::with_capacity(MAX_HISTORY_ENTRIES)
    }
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }
    pub fn add_entry(&mut self, url: String, title: String) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.url == url) {
            existing.touch();
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(HistoryEntry::new(url, title));
    }
    pub fn remove_entry(&mut self, url: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.url == url) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }
    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.url.to_lowercase().contains(&query_lower)
                    || e.title.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
    pub fn most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));
        entries.into_iter().take(limit).collect()
    }
    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));
        entries.into_iter().take(limit).collect()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
#[derive(Debug, Clone)]
pub struct SessionHistory {
    back_stack: VecDeque<String>,
    forward_stack: VecDeque<String>,
    current: Option<String>,
    max_size: usize,
}
impl Default for SessionHistory {
    fn default() -> Self {
        Self::new(50)
    }
}
impl SessionHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            back_stack: VecDeque::with_capacity(max_size),
            forward_stack: VecDeque::with_capacity(max_size),
            current: None,
            max_size,
        }
    }
    pub fn navigate(&mut self, url: String) {
        if let Some(current) = self.current.take() {
            if self.back_stack.len() >= self.max_size {
                self.back_stack.pop_front();
            }
            self.back_stack.push_back(current);
        }
        self.forward_stack.clear();
        self.current = Some(url);
    }
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }
    pub fn go_back(&mut self) -> Option<String> {
        if !self.can_go_back() {
            return None;
        }
        if let Some(current) = self.current.take() {
            if self.forward_stack.len() >= self.max_size {
                self.forward_stack.pop_front();
            }
            self.forward_stack.push_back(current);
        }
        self.current = self.back_stack.pop_back();
        self.current.clone()
    }
    pub fn go_forward(&mut self) -> Option<String> {
        if !self.can_go_forward() {
            return None;
        }
        if let Some(current) = self.current.take() {
            if self.back_stack.len() >= self.max_size {
                self.back_stack.pop_front();
            }
            self.back_stack.push_back(current);
        }
        self.current = self.forward_stack.pop_front();
        self.current.clone()
    }
    pub fn current(&self) -> Option<&String> {
        self.current.as_ref()
    }
    pub fn back_entries(&self) -> &VecDeque<String> {
        &self.back_stack
    }
    pub fn forward_entries(&self) -> &VecDeque<String> {
        &self.forward_stack
    }
    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current = None;
    }
}
