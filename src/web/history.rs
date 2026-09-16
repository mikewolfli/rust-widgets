// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Browser history: a visited-page log and a per-session navigation stack.
//!
//! [`BrowserHistory`] records which URLs have been visited, with a visit count
//! and a last-visit timestamp. [`SessionHistory`] models a single tab's
//! back/forward state.
//!
//! # Ordering and capacity
//!
//! * [`BrowserHistory`] stores entries **oldest first**: [`BrowserHistory::add_entry`]
//!   appends to the back, so [`BrowserHistory::entries`] reads from the oldest
//!   visit to the newest, and eviction drops from the front, i.e. the oldest
//!   entry. The default cap is [`MAX_HISTORY_ENTRIES`] (100);
//!   [`BrowserHistory::with_capacity`] changes it.
//! * [`SessionHistory`] keeps the *navigated* order rather than visit order:
//!   [`SessionHistory::back_entries`] is oldest-first with the most recently
//!   left page at the back, while [`SessionHistory::forward_entries`] is
//!   oldest-first with the page that `go_back` would reach first at the front.
//!   Both stacks are independently capped at the size passed to
//!   [`SessionHistory::new`], evicting from the front.
//! * Visiting a URL that is already present does **not** reorder
//!   [`BrowserHistory`]; it increments the visit count in place.
//!
//! Timestamps are whole seconds since the Unix epoch, so two visits within the
//! same second are indistinguishable by `last_visit`.

use alloc::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};
/// Default entry cap for [`BrowserHistory::new`].
const MAX_HISTORY_ENTRIES: usize = 100;
/// One visited page in a [`BrowserHistory`].
///
/// An entry is created by [`HistoryEntry::new`], which starts the visit count
/// at `1` and stamps the current time; repeat visits update it in place through
/// [`HistoryEntry::touch`] rather than creating a second entry for the same URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Page URL. Used as the identity of the entry when adding, removing, and
    /// searching, so URLs are compared exactly and case-sensitively.
    pub url: String,
    /// Display title captured on the *first* visit; later visits do not update
    /// it, so a retitled page keeps its original title.
    pub title: String,
    /// Number of times the URL has been visited; starts at `1`.
    pub visit_count: u32,
    /// Time of the most recent visit, in whole seconds since the Unix epoch.
    pub last_visit: u64,
}
impl HistoryEntry {
    /// Creates an entry for `url` with the given `title`.
    ///
    /// The visit count starts at `1` and `last_visit` is the current wall-clock
    /// time. If the system clock is before the Unix epoch, the timestamp falls
    /// back to `0` rather than failing.
    pub fn new(url: String, title: String) -> Self {
        Self {
            url,
            title,
            visit_count: 1,
            last_visit: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }
    /// Records another visit to this page.
    ///
    /// Increments [`HistoryEntry::visit_count`] and refreshes
    /// [`HistoryEntry::last_visit`]. The title is left unchanged.
    pub fn touch(&mut self) {
        self.visit_count += 1;
        self.last_visit =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    }
}
/// Visited-page log, ordered from the oldest entry to the newest.
///
/// Backed by a [`VecDeque`] with a fixed cap; once full, adding a new URL evicts
/// the oldest entry. Adding a URL that is already present refreshes that entry's
/// visit count and timestamp instead of reordering or duplicating it.
#[derive(Debug, Clone, Default)]
pub struct BrowserHistory {
    entries: VecDeque<HistoryEntry>,
    max_entries: usize,
}
impl BrowserHistory {
    /// Creates an empty history with the default cap of `100` entries.
    pub fn new() -> Self {
        Self::with_capacity(MAX_HISTORY_ENTRIES)
    }
    /// Creates an empty history that keeps at most `max_entries` entries.
    ///
    /// The value only pre-allocates; the cap is what matters. Note that a cap
    /// of `0` makes every [`BrowserHistory::add_entry`] evict itself, leaving
    /// the history permanently empty.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self { entries: VecDeque::with_capacity(max_entries), max_entries }
    }
    /// Records a visit to `url`, or refreshes it if already present.
    ///
    /// When the URL already exists its entry is [`HistoryEntry::touch`]ed — the
    /// count goes up and the timestamp is refreshed, but `title` and position
    /// are left alone. Otherwise a new entry is appended at the back, evicting
    /// the oldest entry first if the cap has been reached.
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
    /// Removes the entry with exactly this `url`.
    ///
    /// Returns `true` if an entry was removed, `false` when no entry has that
    /// URL. The match is exact and case-sensitive.
    pub fn remove_entry(&mut self, url: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.url == url) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }
    /// Removes every entry, leaving the history empty.
    ///
    /// The configured cap is unaffected, so the history can keep filling up to
    /// the same limit afterwards.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    /// Borrows all entries, oldest first.
    ///
    /// A read-only view; use [`BrowserHistory::add_entry`] and
    /// [`BrowserHistory::remove_entry`] to modify the history.
    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }
    /// Returns entries whose URL or title contains `query`, in stored order.
    ///
    /// Matching is case-insensitive on both fields, but a plain substring test:
    /// the query is not tokenised, split on spaces, or interpreted as a scheme
    /// or host pattern. An empty query therefore matches everything.
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
    /// Returns up to `limit` entries with the highest visit counts, most visited
    /// first.
    ///
    /// The sort is not stable, so entries with equal counts may come back in any
    /// order. Fewer than `limit` entries are returned when the history is
    /// smaller; `limit` of `0` yields an empty vector.
    pub fn most_visited(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.visit_count));
        entries.into_iter().take(limit).collect()
    }
    /// Returns up to `limit` entries with the most recent timestamps, newest
    /// first.
    ///
    /// Ties are broken arbitrarily, and since timestamps have one-second
    /// resolution, entries visited in the same second may appear in any order.
    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.last_visit));
        entries.into_iter().take(limit).collect()
    }
    /// Returns the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns `true` when nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
/// Back/forward navigation state for one browsing session.
///
/// Holds the URL currently being viewed plus the two stacks of URLs reachable
/// by going back and forward. Navigating to a new URL pushes the current one
/// onto the back stack and discards the forward stack, which is the standard
/// browser behaviour. Both stacks are capped at the size given to
/// [`SessionHistory::new`], dropping from the front (the oldest navigation) when
/// full.
///
/// [`SessionHistory::current`] is `Option` because a fresh history has no page
/// loaded: it is `None` until the first [`SessionHistory::navigate`], and
/// [`SessionHistory::clear`] returns it to `None`.
#[derive(Debug, Clone)]
pub struct SessionHistory {
    back_stack: VecDeque<String>,
    forward_stack: VecDeque<String>,
    current: Option<String>,
    max_size: usize,
}
/// Creates a history capped at `50` entries per stack.
impl Default for SessionHistory {
    fn default() -> Self {
        Self::new(50)
    }
}
impl SessionHistory {
    /// Creates an empty history with no current page.
    ///
    /// `max_size` caps the back and forward stacks *independently*; the oldest
    /// entry is evicted from a stack when it would exceed the cap. The cap is
    /// not validated, and a value of `0` makes every navigation forget the
    /// previous page.
    pub fn new(max_size: usize) -> Self {
        Self {
            back_stack: VecDeque::with_capacity(max_size),
            forward_stack: VecDeque::with_capacity(max_size),
            current: None,
            max_size,
        }
    }
    /// Navigates to `url`, as if the user followed a link.
    ///
    /// The previous current URL is pushed onto the back stack (if there was
    /// one), and the forward stack is **cleared**: after navigating, there is
    /// nothing to go forward to. Re-navigating to the URL already being viewed
    /// pushes a duplicate onto the back stack rather than being ignored.
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
    /// Returns `true` when there is a previous page to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }
    /// Returns `true` when there is a page to go forward to.
    ///
    /// The forward stack is only non-empty after a [`SessionHistory::go_back`].
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }
    /// Moves to the previous page and returns it.
    ///
    /// Returns `None` without changing anything when there is nothing to go back
    /// to. The page being left is pushed onto the forward stack, so a following
    /// [`SessionHistory::go_forward`] returns here.
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
    /// Moves to the next page and returns it.
    ///
    /// Returns `None` without changing anything when there is nothing to go
    /// forward to. The page being left is pushed onto the back stack.
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
    /// Borrows the URL currently being viewed, or `None` when no page is loaded.
    pub fn current(&self) -> Option<&String> {
        self.current.as_ref()
    }
    /// Borrows the back stack, oldest navigation first.
    ///
    /// The last element is the page [`SessionHistory::go_back`] would move to.
    pub fn back_entries(&self) -> &VecDeque<String> {
        &self.back_stack
    }
    /// Borrows the forward stack, in the order the pages were left.
    ///
    /// The first element is the page [`SessionHistory::go_forward`] would move
    /// to; the stack is empty unless a [`SessionHistory::go_back`] happened and
    /// no navigation has occurred since.
    pub fn forward_entries(&self) -> &VecDeque<String> {
        &self.forward_stack
    }
    /// Resets the history to its initial state.
    ///
    /// Drops both stacks and sets [`SessionHistory::current`] back to `None`,
    /// so the session behaves as if no page had been loaded.
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
