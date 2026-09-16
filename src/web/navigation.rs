// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Web navigation types: entries, history, resources, settings.
use alloc::collections::VecDeque;
const MAX_HISTORY_SIZE: usize = 100;
/// An entry in the navigation history (session-based back/forward).
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    /// The address that was navigated to.
    pub url: String,
    /// The document title as known at navigation time, or `"Blank Page"` for the
    /// [`Default`] entry.
    pub title: String,
    /// When the entry was recorded. This type never reads or fills it in, so the
    /// unit is whatever the caller chooses (epoch seconds and milliseconds are
    /// both in use); `0` means "unset" and is what [`Default`] produces.
    pub timestamp: u64,
}
impl Default for NavigationEntry {
    fn default() -> Self {
        Self { url: "about:blank".to_string(), title: "Blank Page".to_string(), timestamp: 0 }
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
    /// Creates an empty history holding at most `max_size` entries.
    ///
    /// `max_size` is a hard cap on retained entries; there is no clamping of the
    /// argument, so a value of `0` produces a history that discards every entry
    /// as soon as it is pushed. [`NavigationHistory::default`] uses 100.
    pub fn new(max_size: usize) -> Self {
        Self { entries: VecDeque::with_capacity(max_size), current_index: None, max_size }
    }
    /// Records a visit and makes it the current entry.
    ///
    /// # What it does to the history
    ///
    /// * Any entries *after* the current one are discarded first — the usual
    ///   "browsing forward then following a new link" truncation.
    /// * The oldest entry is evicted when the history is at `max_size`; the
    ///   cursor moves with it so it keeps pointing at the same entry.
    /// * The new entry becomes current, so [`NavigationHistory::can_go_forward`]
    ///   is `false` afterwards and [`NavigationHistory::can_go_back`] is `true`
    ///   unless this is the only entry.
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
    /// The entry the cursor is on, or `None` when the history is empty (or has
    /// been cleared).
    pub fn current(&self) -> Option<&NavigationEntry> {
        self.current_index.and_then(|idx| self.entries.get(idx))
    }
    /// Whether there is an entry behind the current one.
    pub fn can_go_back(&self) -> bool {
        self.current_index.is_some_and(|idx| idx > 0)
    }
    /// Whether there is an entry ahead of the current one — that is, whether a
    /// previous [`NavigationHistory::go_back`] left somewhere to return to.
    /// Always `false` on an empty history.
    pub fn can_go_forward(&self) -> bool {
        self.current_index.is_some_and(|idx| idx < self.entries.len() - 1)
    }
    /// Steps the cursor one entry towards the oldest, returning the entry now
    /// current, or `None` (and no movement) when [`NavigationHistory::can_go_back`]
    /// is `false`.
    ///
    /// This only moves the cursor: nothing is removed, so a later
    /// [`NavigationHistory::go_forward`] returns to where it left.
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
    /// Steps the cursor one entry towards the newest, returning the entry now
    /// current, or `None` when [`NavigationHistory::can_go_forward`] is `false`.
    ///
    /// The forward entries it walks over survive, so the cursor can be moved
    /// back again. Note that [`NavigationHistory::push`] is what discards them.
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
    /// All recorded entries, oldest first.
    ///
    /// Returns only the first contiguous slice of the backing deque. The deque is
    /// only split when a push wraps around inside it, so this is normally the
    /// whole history, but after a wrapped push the oldest entries become
    /// unreachable through this method. [`NavigationHistory::len`] reports the
    /// true count, so it can exceed `entries().len()`.
    pub fn entries(&self) -> &[NavigationEntry] {
        self.entries.as_slices().0
    }
    /// Discards every entry and resets the cursor, leaving
    /// [`NavigationHistory::can_go_back`] and
    /// [`NavigationHistory::can_go_forward`] both `false`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_index = None;
    }
    /// How many entries are currently held, capped by the `max_size` passed to
    /// [`NavigationHistory::new`].
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether no entries are held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
/// Load state of a web page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    /// No navigation has been started yet.
    NotStarted,
    /// A navigation is in flight; the bytes are not ready to display.
    Loading,
    /// The navigation finished and the document is available.
    Loaded,
    /// The navigation was attempted and did not complete.
    Failed,
}
/// A web resource with url, mime type and raw data.
#[derive(Debug, Clone)]
pub struct WebResource {
    /// The address the resource was retrieved from, used as its identity when
    /// cached or resolved.
    pub url: String,
    /// The resource's MIME type, e.g. `"text/html"`. The requestor decides
    /// whether it can handle the resource from this alone.
    pub mime_type: String,
    /// The resource body, verbatim and still encoded as declared by
    /// `mime_type` — this type performs no decoding or charset conversion.
    pub data: Vec<u8>,
}
impl WebResource {
    /// Builds a resource from its three parts, taking ownership of all of them.
    /// Nothing is validated: `mime_type` is stored exactly as given, and `data`
    /// is not checked against it.
    pub fn new(url: String, mime_type: String, data: Vec<u8>) -> Self {
        Self { url, mime_type, data }
    }
    /// Builds a `text/plain` resource from `text`, encoded as UTF-8 bytes.
    ///
    /// Use this for text the caller already holds rather than to fetch `url`;
    /// no I/O happens here.
    pub fn from_text(url: &str, text: &str) -> Self {
        Self {
            url: url.to_string(),
            mime_type: "text/plain".to_string(),
            data: text.as_bytes().to_vec(),
        }
    }
    /// Builds a `text/html` resource from `html`, encoded as UTF-8 bytes.
    ///
    /// Nothing is parsed or sanitised — the markup is stored as written.
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
    /// Whether page scripts may run. When `false`,
    /// `WebViewCore::evaluate_javascript` fails with a `"JavaScript is disabled"`
    /// error rather than silently returning nothing.
    pub javascript_enabled: bool,
    /// Whether browser plugins are permitted to load. Off by default.
    pub plugins_enabled: bool,
    /// Whether this session avoids persisting cookies and history. Off by
    /// default a normal browsing session; turning it on is what callers do to
    /// enter a private session.
    pub private_browsing: bool,
    /// Whether images are fetched and painted. On by default.
    pub images_enabled: bool,
    /// Whether cookies are stored and sent. On by default.
    pub cookies_enabled: bool,
    /// Whether WebGL canvases are permitted. On by default.
    pub webgl_enabled: bool,
    /// Whether developer tooling (inspector, console bridge) is exposed. Off by
    /// default; it is not needed by end users and should not be left on for a
    /// shipped application without a reason.
    pub developer_extras_enabled: bool,
    /// The `User-Agent` header sent with requests. Defaults to
    /// `"RustWidgets/0.1"`, so it intentionally does not impersonate a browser
    /// unless the caller replaces it.
    pub user_agent: String,
    /// The charset assumed when a response does not declare one. Defaults to
    /// `"UTF-8"`.
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
    /// Whether content served over plain `http://` is permitted at all. `false`
    /// by default.
    pub allow_insecure_content: bool,
    /// Whether an `https://` page may pull in `http://` sub-resources
    /// ("mixed content"). `false` by default.
    pub allow_mixed_content: bool,
    /// Whether script-opened windows are suppressed. `true` by default — an
    /// unexpected popup is treated as hostile until the caller opts in.
    pub block_popups: bool,
    /// Whether cross-site tracking is blocked. `true` by default.
    pub block_tracking: bool,
    /// Whether known-malware URLs are blocked. `true` by default.
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
