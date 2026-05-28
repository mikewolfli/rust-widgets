//! Web navigation types: entries, history, resources, settings.
use std::collections::VecDeque;
const MAX_HISTORY_SIZE: usize = 100;
/// An entry in the navigation history (session-based back/forward).
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub url: String,
    pub title: String,
    pub timestamp: u64,
}
impl Default for NavigationEntry {
    fn default() -> Self {
        Self {
            url: "about:blank".to_string(),
            title: "Blank Page".to_string(),
            timestamp: 0,
        }
    }
}
/// Session-based navigation history (back/forward stack).
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    entries: VecDeque<NavigationEntry>,
    current_index: Option<usize>,
    max_size: usize,
}
impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new(MAX_HISTORY_SIZE)
    }
}
impl NavigationHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            current_index: None,
            max_size,
        }
    }
    pub fn push(&mut self, entry: NavigationEntry) {
        if let Some(idx) = self.current_index {
            if idx < self.entries.len() - 1 {
                self.entries.truncate(idx + 1);
            }
        }
        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
            if let Some(ref mut idx) = self.current_index {
                *idx = idx.saturating_sub(1);
            }
        }
        self.entries.push_back(entry);
        self.current_index = Some(self.entries.len().saturating_sub(1));
    }
    pub fn current(&self) -> Option<&NavigationEntry> {
        self.current_index.and_then(|idx| self.entries.get(idx))
    }
    pub fn can_go_back(&self) -> bool {
        self.current_index.is_some_and(|idx| idx > 0)
    }
    pub fn can_go_forward(&self) -> bool {
        self.current_index
            .is_some_and(|idx| idx < self.entries.len() - 1)
    }
    pub fn go_back(&mut self) -> Option<&NavigationEntry> {
        if self.can_go_back() {
            if let Some(ref mut idx) = self.current_index {
                *idx -= 1;
            }
            self.current()
        } else {
            None
        }
    }
    pub fn go_forward(&mut self) -> Option<&NavigationEntry> {
        if self.can_go_forward() {
            if let Some(ref mut idx) = self.current_index {
                *idx += 1;
            }
            self.current()
        } else {
            None
        }
    }
    pub fn entries(&self) -> &[NavigationEntry] {
        self.entries.as_slices().0
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
/// Load state of a web page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    NotStarted,
    Loading,
    Loaded,
    Failed,
}
/// A web resource with url, mime type and raw data.
#[derive(Debug, Clone)]
pub struct WebResource {
    pub url: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}
impl WebResource {
    pub fn new(url: String, mime_type: String, data: Vec<u8>) -> Self {
        Self {
            url,
            mime_type,
            data,
        }
    }
    pub fn from_text(url: &str, text: &str) -> Self {
        Self {
            url: url.to_string(),
            mime_type: "text/plain".to_string(),
            data: text.as_bytes().to_vec(),
        }
    }
    pub fn from_html(url: &str, html: &str) -> Self {
        Self {
            url: url.to_string(),
            mime_type: "text/html".to_string(),
            data: html.as_bytes().to_vec(),
        }
    }
}
/// Configurable web engine preferences.
#[derive(Debug, Clone)]
pub struct WebSettings {
    pub javascript_enabled: bool,
    pub plugins_enabled: bool,
    pub private_browsing: bool,
    pub images_enabled: bool,
    pub cookies_enabled: bool,
    pub webgl_enabled: bool,
    pub developer_extras_enabled: bool,
    pub user_agent: String,
    pub default_encoding: String,
}
impl Default for WebSettings {
    fn default() -> Self {
        Self {
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            images_enabled: true,
            cookies_enabled: true,
            webgl_enabled: true,
            developer_extras_enabled: false,
            user_agent: "RustWidgets/0.1".to_string(),
            default_encoding: "UTF-8".to_string(),
        }
    }
}
/// Security preferences for web content.
#[derive(Debug, Clone)]
pub struct SecuritySettings {
    pub allow_insecure_content: bool,
    pub allow_mixed_content: bool,
    pub block_popups: bool,
    pub block_tracking: bool,
    pub block_malware: bool,
}
impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            allow_insecure_content: false,
            allow_mixed_content: false,
            block_popups: true,
            block_tracking: true,
            block_malware: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_entry_default() {
        let entry = NavigationEntry::default();
        assert_eq!(entry.url, "about:blank");
        assert_eq!(entry.title, "Blank Page");
        assert_eq!(entry.timestamp, 0);
    }

    #[test]
    fn test_navigation_history_new() {
        let history = NavigationHistory::new(10);
        assert_eq!(history.max_size, 10);
        assert!(history.current().is_none());
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
        assert!(history.is_empty());
    }

    #[test]
    fn test_navigation_history_push_and_current() {
        let mut history = NavigationHistory::new(10);
        history.push(NavigationEntry {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            timestamp: 100,
        });
        let current = history.current();
        assert!(current.is_some());
        assert_eq!(current.unwrap().url, "https://example.com");
        assert_eq!(current.unwrap().title, "Example");
        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
    }

    #[test]
    fn test_navigation_history_go_back_and_forward() {
        let mut history = NavigationHistory::new(10);
        history.push(NavigationEntry {
            url: "https://page1.com".to_string(),
            title: "Page 1".to_string(),
            timestamp: 1,
        });
        history.push(NavigationEntry {
            url: "https://page2.com".to_string(),
            title: "Page 2".to_string(),
            timestamp: 2,
        });
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());
        assert_eq!(history.current().unwrap().url, "https://page2.com");

        let back = history.go_back();
        assert!(back.is_some());
        assert_eq!(back.unwrap().url, "https://page1.com");
        assert!(!history.can_go_back());
        assert!(history.can_go_forward());

        let fwd = history.go_forward();
        assert!(fwd.is_some());
        assert_eq!(fwd.unwrap().url, "https://page2.com");
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn test_navigation_history_go_back_at_start() {
        let mut history = NavigationHistory::new(10);
        assert!(history.go_back().is_none());
        assert!(history.go_forward().is_none());
    }

    #[test]
    fn test_navigation_history_clear() {
        let mut history = NavigationHistory::new(10);
        history.push(NavigationEntry {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            timestamp: 0,
        });
        assert!(!history.is_empty());
        history.clear();
        assert!(history.is_empty());
        assert!(history.current().is_none());
        assert!(!history.can_go_back());
    }

    #[test]
    fn test_navigation_history_truncates_forward_on_new_push() {
        let mut history = NavigationHistory::new(10);
        history.push(NavigationEntry {
            url: "https://a.com".to_string(),
            title: "A".to_string(),
            timestamp: 1,
        });
        history.push(NavigationEntry {
            url: "https://b.com".to_string(),
            title: "B".to_string(),
            timestamp: 2,
        });
        history.push(NavigationEntry {
            url: "https://c.com".to_string(),
            title: "C".to_string(),
            timestamp: 3,
        });
        history.go_back();
        history.go_back();
        assert_eq!(history.current().unwrap().url, "https://a.com");
        // Pushing a new URL after going back truncates forward stack
        history.push(NavigationEntry {
            url: "https://d.com".to_string(),
            title: "D".to_string(),
            timestamp: 4,
        });
        assert!(!history.can_go_forward());
        assert_eq!(history.current().unwrap().url, "https://d.com");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_navigation_history_max_size_eviction() {
        let mut history = NavigationHistory::new(3);
        history.push(NavigationEntry {
            url: "https://a.com".to_string(),
            title: "A".to_string(),
            timestamp: 1,
        });
        history.push(NavigationEntry {
            url: "https://b.com".to_string(),
            title: "B".to_string(),
            timestamp: 2,
        });
        history.push(NavigationEntry {
            url: "https://c.com".to_string(),
            title: "C".to_string(),
            timestamp: 3,
        });
        history.push(NavigationEntry {
            url: "https://d.com".to_string(),
            title: "D".to_string(),
            timestamp: 4,
        });
        // The oldest entry should be evicted
        assert_eq!(history.len(), 3);
        assert_eq!(history.current().unwrap().url, "https://d.com");
    }

    #[test]
    fn test_navigation_history_entries_slice() {
        let mut history = NavigationHistory::new(10);
        history.push(NavigationEntry {
            url: "https://a.com".to_string(),
            title: "A".to_string(),
            timestamp: 1,
        });
        history.push(NavigationEntry {
            url: "https://b.com".to_string(),
            title: "B".to_string(),
            timestamp: 2,
        });
        let entries = history.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://a.com");
        assert_eq!(entries[1].url, "https://b.com");
    }

    #[test]
    fn test_load_status_variants() {
        assert_eq!(LoadStatus::NotStarted as u8, 0);
        assert_eq!(LoadStatus::Loading as u8, 1);
        assert_eq!(LoadStatus::Loaded as u8, 2);
        assert_eq!(LoadStatus::Failed as u8, 3);
        assert_ne!(LoadStatus::NotStarted, LoadStatus::Loading);
        assert_ne!(LoadStatus::Loaded, LoadStatus::Failed);
    }

    #[test]
    fn test_web_resource_new() {
        let resource = WebResource::new(
            "https://example.com/data".to_string(),
            "application/json".to_string(),
            vec![1, 2, 3],
        );
        assert_eq!(resource.url, "https://example.com/data");
        assert_eq!(resource.mime_type, "application/json");
        assert_eq!(resource.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_web_resource_from_text() {
        let resource = WebResource::from_text("https://example.com/hello", "Hello, World!");
        assert_eq!(resource.url, "https://example.com/hello");
        assert_eq!(resource.mime_type, "text/plain");
        assert_eq!(resource.data, b"Hello, World!");
    }

    #[test]
    fn test_web_resource_from_html() {
        let resource = WebResource::from_html("https://example.com/page", "<h1>Title</h1>");
        assert_eq!(resource.url, "https://example.com/page");
        assert_eq!(resource.mime_type, "text/html");
        assert_eq!(resource.data, b"<h1>Title</h1>");
    }

    #[test]
    fn test_web_settings_default() {
        let settings = WebSettings::default();
        assert!(settings.javascript_enabled);
        assert!(!settings.plugins_enabled);
        assert!(!settings.private_browsing);
        assert!(settings.images_enabled);
        assert!(settings.cookies_enabled);
        assert!(settings.webgl_enabled);
        assert!(!settings.developer_extras_enabled);
        assert_eq!(settings.user_agent, "RustWidgets/0.1");
        assert_eq!(settings.default_encoding, "UTF-8");
    }

    #[test]
    fn test_web_settings_mutate() {
        let settings = WebSettings {
            javascript_enabled: false,
            plugins_enabled: true,
            private_browsing: true,
            ..WebSettings::default()
        };
        assert!(!settings.javascript_enabled);
        assert!(settings.plugins_enabled);
        assert!(settings.private_browsing);
    }

    #[test]
    fn test_security_settings_default() {
        let security = SecuritySettings::default();
        assert!(!security.allow_insecure_content);
        assert!(!security.allow_mixed_content);
        assert!(security.block_popups);
        assert!(security.block_tracking);
        assert!(security.block_malware);
    }

    #[test]
    fn test_security_settings_mutate() {
        let security = SecuritySettings {
            allow_insecure_content: true,
            block_popups: false,
            ..SecuritySettings::default()
        };
        assert!(security.allow_insecure_content);
        assert!(!security.block_popups);
    }
}
