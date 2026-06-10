use alloc::collections::VecDeque;
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
            last_visit: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }
    pub fn touch(&mut self) {
        self.visit_count += 1;
        self.last_visit =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
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
        Self { entries: VecDeque::with_capacity(max_entries), max_entries }
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
        entries.sort_by_key(|b| std::cmp::Reverse(b.visit_count));
        entries.into_iter().take(limit).collect()
    }
    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.last_visit));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── HistoryEntry tests ──

    #[test]
    fn test_history_entry_new() {
        let entry = HistoryEntry::new("https://example.com".to_string(), "Example".to_string());
        assert_eq!(entry.url, "https://example.com");
        assert_eq!(entry.title, "Example");
        assert_eq!(entry.visit_count, 1);
        assert!(entry.last_visit > 1_600_000_000);
    }

    #[test]
    fn test_history_entry_touch() {
        let mut entry = HistoryEntry::new("https://example.com".to_string(), "Example".to_string());
        let first_visit = entry.last_visit;
        std::thread::sleep(Duration::from_millis(2));
        entry.touch();
        assert_eq!(entry.visit_count, 2);
        assert!(entry.last_visit >= first_visit);
    }

    // ── BrowserHistory tests ──

    #[test]
    fn test_browser_history_new() {
        let history = BrowserHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_browser_history_with_capacity() {
        let history = BrowserHistory::with_capacity(5);
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_browser_history_add_entry() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://example.com".to_string(), "Example".to_string());
        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_browser_history_add_duplicate_touches() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://example.com".to_string(), "Example".to_string());
        history.add_entry("https://example.com".to_string(), "Example".to_string());
        // Duplicate URL should increment visit_count, not add another entry
        assert_eq!(history.len(), 1);
        let entries = history.entries();
        let entry = entries.front().unwrap();
        assert_eq!(entry.visit_count, 2);
    }

    #[test]
    fn test_browser_history_add_multiple() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        history.add_entry("https://c.com".to_string(), "C".to_string());
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_browser_history_remove_entry() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://example.com".to_string(), "Example".to_string());
        assert!(history.remove_entry("https://example.com"));
        assert!(history.is_empty());
        // Removing a non-existent entry returns false
        assert!(!history.remove_entry("https://nonexistent.com"));
    }

    #[test]
    fn test_browser_history_clear() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        history.clear();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_browser_history_entries() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        let entries = history.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://a.com");
        assert_eq!(entries[1].url, "https://b.com");
    }

    #[test]
    fn test_browser_history_search_by_url() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://example.com".to_string(), "Example".to_string());
        history.add_entry("https://rust-lang.org".to_string(), "Rust".to_string());
        let results = history.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://rust-lang.org");
    }

    #[test]
    fn test_browser_history_search_by_title() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://example.com".to_string(), "Example Page".to_string());
        history.add_entry("https://other.com".to_string(), "Other Site".to_string());
        let results = history.search("page");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com");
    }

    #[test]
    fn test_browser_history_search_case_insensitive() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://EXAMPLE.COM".to_string(), "Example".to_string());
        let results = history.search("example");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_browser_history_most_visited() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        history.add_entry("https://a.com".to_string(), "A".to_string()); // visit_count = 2
        let top = history.most_visited(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].url, "https://a.com");
    }

    #[test]
    fn test_browser_history_most_visited_limit() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        history.add_entry("https://c.com".to_string(), "C".to_string());
        let top = history.most_visited(2);
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn test_browser_history_recent() {
        let mut history = BrowserHistory::new();
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        let recent = history.recent(3);
        // Both entries should be returned when limit is high enough
        assert_eq!(recent.len(), 2);
        let urls: Vec<&str> = recent.iter().map(|e| e.url.as_str()).collect();
        assert!(urls.contains(&"https://a.com"));
        assert!(urls.contains(&"https://b.com"));
    }

    #[test]
    fn test_browser_history_max_entries_eviction() {
        let mut history = BrowserHistory::with_capacity(2);
        history.add_entry("https://a.com".to_string(), "A".to_string());
        history.add_entry("https://b.com".to_string(), "B".to_string());
        history.add_entry("https://c.com".to_string(), "C".to_string());
        assert_eq!(history.len(), 2);
        assert_eq!(history.entries().front().unwrap().url, "https://b.com");
    }

    // ── SessionHistory tests ──

    #[test]
    fn test_session_history_new() {
        let history = SessionHistory::new(10);
        assert!(history.current().is_none());
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_session_history_default() {
        let history = SessionHistory::default();
        assert!(history.current().is_none());
    }

    #[test]
    fn test_session_history_navigate() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://example.com".to_string());
        assert_eq!(history.current().unwrap(), "https://example.com");
        assert!(!history.can_go_back());
    }

    #[test]
    fn test_session_history_navigate_multiple() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://page1.com".to_string());
        history.navigate("https://page2.com".to_string());
        assert!(history.can_go_back());
        assert_eq!(history.current().unwrap(), "https://page2.com");
    }

    #[test]
    fn test_session_history_go_back() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://page1.com".to_string());
        history.navigate("https://page2.com".to_string());
        let back = history.go_back();
        assert_eq!(back.as_deref(), Some("https://page1.com"));
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());
    }

    #[test]
    fn test_session_history_go_back_none_when_empty() {
        let mut history = SessionHistory::new(10);
        assert!(history.go_back().is_none());
    }

    #[test]
    fn test_session_history_go_forward() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://page1.com".to_string());
        history.navigate("https://page2.com".to_string());
        history.go_back();
        let fwd = history.go_forward();
        assert_eq!(fwd.as_deref(), Some("https://page2.com"));
        assert!(!history.can_go_forward());
        assert!(history.can_go_back());
    }

    #[test]
    fn test_session_history_go_forward_none_when_empty() {
        let mut history = SessionHistory::new(10);
        assert!(history.go_forward().is_none());
    }

    #[test]
    fn test_session_history_navigate_clears_forward() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://page1.com".to_string());
        history.navigate("https://page2.com".to_string());
        history.go_back();
        assert!(history.can_go_forward());
        // Navigating to a new URL should clear the forward stack
        history.navigate("https://page3.com".to_string());
        assert!(!history.can_go_forward());
        assert!(history.can_go_back());
        assert_eq!(history.current().unwrap(), "https://page3.com");
    }

    #[test]
    fn test_session_history_back_entries() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://a.com".to_string());
        history.navigate("https://b.com".to_string());
        history.navigate("https://c.com".to_string());
        let back = history.back_entries();
        assert_eq!(back.len(), 2);
    }

    #[test]
    fn test_session_history_forward_entries() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://a.com".to_string());
        history.navigate("https://b.com".to_string());
        history.go_back();
        let fwd = history.forward_entries();
        assert_eq!(fwd.len(), 1);
    }

    #[test]
    fn test_session_history_clear() {
        let mut history = SessionHistory::new(10);
        history.navigate("https://a.com".to_string());
        history.navigate("https://b.com".to_string());
        history.clear();
        assert!(history.current().is_none());
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_session_history_max_size_back_stack() {
        let mut history = SessionHistory::new(2);
        history.navigate("https://a.com".to_string());
        history.navigate("https://b.com".to_string());
        history.navigate("https://c.com".to_string());
        // Back stack should be capped at 2
        assert_eq!(history.back_entries().len(), 2);
    }
}
