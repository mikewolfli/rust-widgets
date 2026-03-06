mod history;
mod js_engine;
mod privacy;
mod plugins;
mod web_view;
mod web_engine;

pub use history::*;
pub use js_engine::*;
pub use privacy::*;
pub use plugins::*;
pub use web_view::*;
pub use web_engine::*;

use std::collections::VecDeque;

const MAX_HISTORY_SIZE: usize = 100;

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
        self.current_index = Some(self.entries.len() - 1);
    }

    pub fn current(&self) -> Option<&NavigationEntry> {
        self.current_index.and_then(|idx| self.entries.get(idx))
    }

    pub fn can_go_back(&self) -> bool {
        self.current_index.map_or(false, |idx| idx > 0)
    }

    pub fn can_go_forward(&self) -> bool {
        self.current_index.map_or(false, |idx| idx < self.entries.len() - 1)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    NotStarted,
    Loading,
    Loaded,
    Failed,
}

#[derive(Debug, Clone)]
pub struct WebResource {
    pub url: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl WebResource {
    pub fn new(url: String, mime_type: String, data: Vec<u8>) -> Self {
        Self { url, mime_type, data }
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
