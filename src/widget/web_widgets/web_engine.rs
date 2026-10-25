// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Simulated web-engine view widget (self-drawn chrome).
//!
//! `WebEngineView` draws a browser-style chrome (URL bar, content hint) and
//! models navigation state (loading/back/forward). It does **not** contain a
//! DOM layout or network stack, so page content itself is never rendered.
//!
//! JavaScript evaluation is real: with the `js-engine` feature the widget
//! evaluates scripts in a genuine embedded engine (boa_engine via
//! `crate::web::SimpleJsEngine`) and returns the actual result or error.
//! Without that feature, evaluation reports `Err` instead of faking success.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::vec::Vec;

/// First page id [`WebEngineView::create_page`] issues.
///
/// Deliberately not `0`: that is the "no widget" sentinel the runtime and the C ABI
/// use, so issuing it as a page id would make an absent page indistinguishable from
/// a real one.
pub const FIRST_PAGE_ID: ObjectId = 1;

/// User agent used when the host has not set one.
///
/// The view performs no network I/O, so this string is reported to the host rather than sent
/// anywhere; it exists so a settings consumer always has a concrete value to display and pass on.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (compatible; rust-widgets WebEngineView) AppleWebKit/605.1.15";

/// The `SameSite` policy a cookie was stored with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CookieSameSite {
    /// The host declared no policy.
    #[default]
    Unspecified,
    /// Sent on same-site requests and top-level navigations.
    Lax,
    /// Sent on same-site requests only.
    Strict,
    /// Sent on every request; legal only with `Secure`.
    None,
}

/// A cookie with the fields a browser store needs.
///
/// The store is in memory; persistence is the host's file I/O. [`WebEngineView::persist_cookies`]
/// serialises the list to JSON and [`WebEngineView::load_cookies`] restores it, so a host that
/// wants disk persistence writes and reads that document itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie belongs to.
    pub domain: String,
    /// Path scope; defaults to `/`.
    pub path: String,
    /// Expiry in Unix milliseconds, or `None` for a session cookie.
    pub expires_ms: Option<i64>,
    /// Whether the cookie is HTTPS-only.
    pub secure: bool,
    /// Whether script can read the cookie.
    pub http_only: bool,
    /// Cross-site policy.
    pub same_site: CookieSameSite,
}

impl Cookie {
    /// A session cookie with the historical `(name, value, domain)` defaults.
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: "/".to_string(),
            expires_ms: None,
            secure: false,
            http_only: false,
            same_site: CookieSameSite::Unspecified,
        }
    }
}

/// One history entry, with the state a back/forward navigation restores.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// The page URL.
    pub url: String,
    /// The title the page reported when it was current.
    pub title: String,
    /// The content scroll position in logical pixels.
    pub scroll_pos: f32,
}

/// Every non-overlapping occurrence of `needle` in `haystack`, found in linear time.
///
/// The KMP prefix table makes the scan O(haystack + needle); the previous byte-by-byte
/// `haystack[offset..offset + needle.len()]` comparison was O(haystack × needle) in the worst case
/// (a long run of a repeated prefix). Case folding is ASCII-only, matching the published rule.
fn find_all_offsets(haystack: &[u8], needle: &[u8], case_sensitive: bool) -> Vec<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return Vec::new();
    }
    let fold = |byte: u8| if case_sensitive { byte } else { byte.to_ascii_lowercase() };
    let mut prefix = vec![0usize; needle.len()];
    let mut matched = 0usize;
    for index in 1..needle.len() {
        while matched > 0 && fold(needle[index]) != fold(needle[matched]) {
            matched = prefix[matched - 1];
        }
        if fold(needle[index]) == fold(needle[matched]) {
            matched += 1;
        }
        prefix[index] = matched;
    }

    let mut matches = Vec::new();
    matched = 0;
    for (index, byte) in haystack.iter().enumerate() {
        let byte = fold(*byte);
        while matched > 0 && byte != fold(needle[matched]) {
            matched = prefix[matched - 1];
        }
        if byte == fold(needle[matched]) {
            matched += 1;
        }
        if matched == needle.len() {
            matches.push(index + 1 - needle.len());
            // Non-overlapping matches: restart the scan at the next byte.
            matched = 0;
        }
    }
    matches
}

/// Lifecycle state of the most recent download the host has reported.
///
/// The view performs no network I/O; the host supplies progress through
/// [`WebEngineView::report_download_progress`], and this enum records what that
/// reporting implies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebEngineDownloadState {
    /// No download has been requested or reported.
    Idle,
    /// A download was requested, but no progress has been reported yet.
    Requested,
    /// Progress has been reported and the transfer has not reached its total.
    InProgress,
    /// The host reported a received byte count that reached the known total.
    Completed,
}

/// One rendered block of the lightweight HTML page painter.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlBlock {
    text: String,
    kind: HtmlBlockKind,
}

/// How a parsed block is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HtmlBlockKind {
    Text,
    Heading,
    Link,
    Image,
}

/// Splits an HTML document into paintable text blocks.
///
/// This is deliberately a *reader*, not a browser engine: it strips tags, keeps the text of
/// block-level elements as one block each, marks anchors and headings, and turns `<img>` into a
/// placeholder. That is enough for a web view to show document content instead of a blank
/// surface, which is the MW-10 gap this closes without claiming a DOM implementation.
fn parse_html_blocks(html: &str) -> Vec<HtmlBlock> {
    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut kind = HtmlBlockKind::Text;
    let mut in_anchor = false;
    let bytes = html.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            let Some(end) = html[index..].find('>').map(|offset| index + offset) else {
                break;
            };
            let tag = html[index + 1..end].trim().to_ascii_lowercase();
            let closing = tag.starts_with('/');
            let name = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("");
            match name {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    if closing && !text.is_empty() {
                        blocks.push(HtmlBlock {
                            text: text.trim().to_string(),
                            kind: HtmlBlockKind::Heading,
                        });
                        text.clear();
                    }
                }
                "a" => {
                    if closing {
                        if !text.trim().is_empty() {
                            blocks.push(HtmlBlock {
                                text: text.trim().to_string(),
                                kind: HtmlBlockKind::Link,
                            });
                            text.clear();
                        }
                        in_anchor = false;
                    } else {
                        in_anchor = true;
                    }
                }
                "img" => {
                    let alt = tag
                        .split("alt=\"")
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .unwrap_or("[image]")
                        .to_string();
                    blocks.push(HtmlBlock { text: alt, kind: HtmlBlockKind::Image });
                }
                "p" | "div" | "li" | "br" | "tr" | "section" | "article" | "header" | "footer" => {
                    if !text.trim().is_empty() {
                        blocks.push(HtmlBlock {
                            text: text.trim().to_string(),
                            kind: if in_anchor { HtmlBlockKind::Link } else { kind },
                        });
                        text.clear();
                    }
                    kind = HtmlBlockKind::Text;
                }
                _ => {}
            }
            index = end + 1;
            continue;
        }
        let ch = html[index..].chars().next().unwrap_or(' ');
        text.push(ch);
        index += ch.len_utf8();
    }
    if !text.trim().is_empty() {
        blocks.push(HtmlBlock {
            text: text.trim().to_string(),
            kind: if in_anchor { HtmlBlockKind::Link } else { kind },
        });
    }
    blocks
}

/// Web engine view widget for web content rendering.
pub struct WebEngineView {
    base: BaseWidget,
    url: String,
    loading: bool,
    pending_load: bool,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    javascript_enabled: bool,
    plugins_enabled: bool,
    private_browsing: bool,
    /// In-memory cookie store.
    cookies: Vec<Cookie>,
    /// Whether a host may persist cookies; the store itself never touches the filesystem.
    cookie_persistence_enabled: bool,
    /// Bytes received for the most recent host-reported download.
    download_received: u64,
    /// Total bytes for that download when the host knows it.
    download_total: Option<u64>,
    /// Lifecycle state of that download.
    download_state: WebEngineDownloadState,
    /// Visited pages, oldest first.
    history: Vec<HistoryEntry>,
    /// Index of the current entry in [`Self::history`].
    history_index: usize,
    /// Last reported load progress, `0..=100`.
    load_progress: u32,
    /// Registered web-channel names.
    channels: Vec<String>,
    /// Page zoom factor, `1.0` being 100%.
    zoom_factor: f32,
    /// Emitted when the page starts loading.
    pub loading_started: Signal1<String>,
    /// Emitted when the page finishes loading.
    pub loading_finished: Signal1<String>,
    /// Emitted when the title changes.
    pub title_changed: Signal1<String>,
    /// Emitted when the URL changes.
    pub url_changed: Signal1<String>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
    /// Emitted when the navigation state changes.
    pub navigation_state_changed: Signal1<(bool, bool)>,
    /// Emitted when a certificate error occurs.
    pub certificate_error: Signal1<String>,
    /// Emitted when a JavaScript console message is received.
    pub console_message: Signal1<(String, u32, String)>,
    /// Emitted when a download is requested.
    pub download_requested: Signal1<String>,
    /// Emitted when the page is created.
    pub page_created: Signal1<ObjectId>,
    /// Emitted when the page is destroyed.
    pub page_destroyed: Signal1<ObjectId>,
    /// Emitted with the load percentage, `0..=100`.
    pub load_progress_changed: Signal1<u32>,
    /// Emitted with `(channel, payload)` when the page posts to a web channel.
    pub channel_message: Signal1<(String, String)>,
    /// Emitted with the target URL when the pointer enters a link, and with an empty string when
    /// it leaves (the standard wiring point for a status-bar URL readout).
    pub link_hovered: Signal1<String>,
    /// Emitted with a host-supplied reason when the render process exits abnormally.
    pub render_process_crashed: Signal1<String>,
    /// Emitted when a deferred JavaScript evaluation finishes, with `(ok, payload)`.
    pub js_evaluated: Signal1<(bool, String)>,
    /// The user agent the host declared, or [`DEFAULT_USER_AGENT`].
    user_agent: String,
    /// Whether the blank page surface is painted at all.
    background_transparent: bool,
    /// HTML source the current find was computed over; a new document invalidates the matches.
    find_needle: String,
    /// Whether the active find is case sensitive.
    find_case_sensitive: bool,
    /// Byte offsets of the current matches in [`Self::html_content`].
    find_matches: Vec<usize>,
    /// Index into [`Self::find_matches`] of the match a find-next/previous action selected.
    find_current: Option<usize>,
    /// Script queued by [`Self::evaluate_javascript_async`], drained on the next tick.
    pending_javascript: Option<String>,
    /// Most recently loaded HTML source (kept for inspection; no DOM layout).
    html_content: String,
    /// The paintable blocks parsed from [`Self::html_content`].
    html_blocks: Vec<HtmlBlock>,
    /// Vertical page scroll offset in logical pixels (MW-10).
    scroll_offset: i32,
    /// The page's painted content height, measured by the last draw.
    page_content_height: i32,
    /// Where a host stores downloads, when it declared a directory.
    download_dir: Option<String>,
    /// Emitted as `(file name, fraction)` as a download progresses.
    pub download_progress_changed: Signal1<(String, f32)>,
    /// The zoom-out button's rectangle from the last paint.
    zoom_out_rect: Option<Rect>,
    /// The zoom-in button's rectangle from the last paint.
    zoom_in_rect: Option<Rect>,
    /// Next id [`WebEngineView::create_page`] will issue.
    ///
    /// Starts at [`FIRST_PAGE_ID`] so a page id is never `0`: `0` is the "no widget"
    /// sentinel the C ABI and the runtime use, and handing one out as a page id
    /// would make an absent page indistinguishable from a real one.
    next_page_id: ObjectId,
    /// Embedded real JavaScript engine (boa_engine) for script evaluation.
    #[cfg(feature = "js-engine")]
    js_engine: Option<crate::web::BoaJsEngine>,
}
// Newtype structs for render pipeline symbol imports, wrapping WebEngineView.
/// placeholder facade: forwards WebEngineView only, no independent semantics.
///
/// The render pipeline resolves the "page" symbol name to a type; this wrapper
/// exists for that registration and carries no page model of its own.
#[doc(hidden)]
pub struct WebEnginePage(pub WebEngineView);
impl WebEnginePage {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Symbol-name facade for the render pipeline's "engine" name.
///
/// Forwards [`WebEngineView`] only; it has no independent semantics.
pub struct WebEngine(pub WebEngineView);
impl WebEngine {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Settings facade that exposes the wrapped view's real feature flags.
///
/// The wrapped [`WebEngineView`] owns `javascript_enabled`, `plugins_enabled`
/// and `private_browsing`; this type forwards their getters and setters so the
/// settings symbol name addresses those flags rather than a second copy.
pub struct WebEngineSettings(pub WebEngineView);
impl WebEngineSettings {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
    /// Enables or disables JavaScript evaluation for the wrapped view.
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.0.set_javascript_enabled(enabled);
    }
    /// The user agent the wrapped view reports.
    pub fn user_agent(&self) -> &str {
        self.0.user_agent()
    }
    /// Sets the user agent the wrapped view reports.
    pub fn set_user_agent(&mut self, user_agent: impl Into<String>) {
        self.0.set_user_agent(user_agent);
    }
    /// Whether the wrapped view paints a page surface.
    pub fn background_transparent(&self) -> bool {
        self.0.background_transparent()
    }
    /// Skips the page surface fill for the wrapped view.
    pub fn set_background_transparent(&mut self, transparent: bool) {
        self.0.set_background_transparent(transparent);
    }
    /// Returns whether JavaScript evaluation is enabled for the wrapped view.
    pub fn is_javascript_enabled(&self) -> bool {
        self.0.is_javascript_enabled()
    }
    /// Records whether plugin content is permitted for the wrapped view.
    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.0.set_plugins_enabled(enabled);
    }
    /// Returns whether plugin content is enabled for the wrapped view.
    pub fn is_plugins_enabled(&self) -> bool {
        self.0.is_plugins_enabled()
    }
    /// Records whether private browsing is in effect for the wrapped view.
    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.0.set_private_browsing(enabled);
    }
    /// Returns whether the wrapped view is in private browsing mode.
    pub fn is_private_browsing(&self) -> bool {
        self.0.is_private_browsing()
    }
}
/// Download-item facade over the wrapped view's host-reported download state.
///
/// This widget performs no network I/O. The host reports byte progress with
/// [`WebEngineDownloadItem::report_progress`], and the item tracks whether that
/// progress has completed.
pub struct WebEngineDownloadItem(pub WebEngineView);
impl WebEngineDownloadItem {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
    /// Reports download progress supplied by the host.
    ///
    /// `total` is `None` while the host does not know the final size. Reporting a
    /// `received` count that reaches a known positive `total` moves the state to
    /// [`WebEngineDownloadState::Completed`].
    pub fn report_progress(&mut self, received: u64, total: Option<u64>) {
        self.0.report_download_progress(received, total);
    }
    /// The last reported progress as `(received, total)`.
    pub fn progress(&self) -> (u64, Option<u64>) {
        self.0.download_progress()
    }
    /// The lifecycle state of the most recent download.
    pub fn state(&self) -> WebEngineDownloadState {
        self.0.download_state()
    }
}
/// Cookie-store facade over the wrapped view's in-memory cookie list.
///
/// Cookies are [`Cookie`] values held only in memory and discarded with the view. While the
/// wrapped view is in private browsing, [`WebEngineCookieStore::set_cookie`] refuses to store
/// anything. Persistence is the host's file I/O: [`WebEngineCookieStore::persist_cookies`] and
/// [`WebEngineCookieStore::load_cookies`] convert the jar to and from the JSON a host saves.
pub struct WebEngineCookieStore(pub WebEngineView);
impl WebEngineCookieStore {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
    /// Stores a cookie, returning `false` when private browsing refused it.
    pub fn set_cookie(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        domain: impl Into<String>,
    ) -> bool {
        self.0.set_cookie(name, value, domain)
    }
    /// Returns the cookie stored under `name`, if any.
    pub fn get_cookie(&self, name: &str) -> Option<&Cookie> {
        self.0.get_cookie(name)
    }
    /// Alias for [`Self::get_cookie`].
    pub fn get(&self, name: &str) -> Option<&Cookie> {
        self.get_cookie(name)
    }
    /// Stores a fully specified cookie.
    pub fn set_cookie_entry(&mut self, cookie: &Cookie) -> bool {
        self.0.set_cookie_entry(cookie)
    }
    /// Returns every stored cookie.
    pub fn cookies(&self) -> &[Cookie] {
        self.0.cookies()
    }
    /// Serialises the jar to JSON (empty while persistence is disabled).
    pub fn persist_cookies(&self) -> String {
        self.0.persist_cookies()
    }
    /// Restores a jar from JSON.
    pub fn load_cookies(&mut self, json: &str) -> Result<(), String> {
        self.0.load_cookies(json)
    }
    /// Removes every stored cookie.
    pub fn clear_cookies(&mut self) {
        self.0.clear_cookies();
    }
    /// Alias for [`Self::clear_cookies`].
    pub fn clear(&mut self) {
        self.clear_cookies();
    }
}
/// placeholder facade: forwards WebEngineView only, no independent semantics.
///
/// The render pipeline resolves the "web channel" symbol name to a type; this
/// wrapper exists for that registration and carries no channel model.
#[doc(hidden)]
pub struct WebEngineWebChannel(pub WebEngineView);
impl WebEngineWebChannel {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// Find-text facade that searches the wrapped view's retained HTML source.
///
/// [`WebEngineFindTextResult::find_text`] counts matches in
/// [`WebEngineView::html_source`]; there is no DOM or rendered text to search.
pub struct WebEngineFindTextResult(pub WebEngineView);
impl WebEngineFindTextResult {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
    /// Counts `needle` in the retained HTML source.
    ///
    /// Returns `(match_count, selected_offset)`, where `backwards` selects the
    /// last match's offset instead of the first. See
    /// [`WebEngineView::find_text`] for the exact matching rules.
    pub fn find_text(
        &mut self,
        needle: &str,
        case_sensitive: bool,
        backwards: bool,
    ) -> (usize, usize) {
        self.0.find_text(needle, case_sensitive, backwards)
    }
}
/// placeholder facade: forwards WebEngineView only, no independent semantics.
///
/// The render pipeline resolves the "notification" symbol name to a type; this
/// wrapper exists for that registration and carries no notification model.
#[doc(hidden)]
pub struct WebEngineNotification(pub WebEngineView);
impl WebEngineNotification {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// placeholder facade: forwards WebEngineView only, no independent semantics.
///
/// The render pipeline resolves the "script dialog" symbol name to a type; this
/// wrapper exists for that registration and carries no dialog model.
#[doc(hidden)]
pub struct WebEngineScriptDialog(pub WebEngineView);
impl WebEngineScriptDialog {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}
/// placeholder facade: forwards WebEngineView only, no independent semantics.
///
/// The render pipeline resolves the "context menu request" symbol name to a type;
/// this wrapper exists for that registration and carries no menu model.
#[doc(hidden)]
pub struct WebEngineContextMenuRequest(pub WebEngineView);
impl WebEngineContextMenuRequest {
    /// Wraps a fresh view occupying `geometry` (in logical pixels).
    pub fn new(geometry: Rect) -> Self {
        Self(WebEngineView::new(geometry))
    }
    /// Borrows the wrapped view.
    pub fn inner(&self) -> &WebEngineView {
        &self.0
    }
    /// Mutably borrows the wrapped view.
    pub fn inner_mut(&mut self) -> &mut WebEngineView {
        &mut self.0
    }
}

macro_rules! impl_web_engine_wrapper_traits {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Widget for $ty {
                fn base(&self) -> &BaseWidget {
                    self.0.base()
                }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

                fn base_mut(&mut self) -> &mut BaseWidget {
                    self.0.base_mut()
                }

                /// Forwards to the wrapped view.
                ///
                /// All ten wrappers report `WebEngineView` as their kind (see
                /// `WidgetKind::WebEngineView`), so the capability lookup resolves them to
                /// `web_engine_view` and its eleven published names. Without this forward each
                /// name was accepted by `connect_event` and wired to nothing: the wrapper
                /// delegates its behaviour, so it must delegate its signal resolution too, or the
 /// forwarding promise stops at the event side.
                fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
                    self.0.event_signal_dyn(name)
                }
            }

            impl EventHandler for $ty {
                fn handle_event(&mut self, event: &Event) {
                    self.0.handle_event(event);
                }
            }

            impl Draw for $ty {
                fn draw(&mut self, ctx: &mut RenderContext) {
                    self.0.draw(ctx);
                }

                fn uses_custom_drawing(&self) -> bool {
                    self.0.uses_custom_drawing()
                }
            }
        )+
    };
}

impl_web_engine_wrapper_traits!(
    WebEnginePage,
    WebEngine,
    WebEngineSettings,
    WebEngineDownloadItem,
    WebEngineCookieStore,
    WebEngineWebChannel,
    WebEngineFindTextResult,
    WebEngineNotification,
    WebEngineScriptDialog,
    WebEngineContextMenuRequest,
);

impl WebEngineView {
    const LOAD_TIMER_ID: u32 = 1;

    /// Creates a view with empty URL, title and HTML source, not loading, with
    /// JavaScript enabled and plugins, private browsing and navigation history
    /// all off.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "WebEngineView"),
            url: "".to_string(),
            loading: false,
            pending_load: false,
            title: "".to_string(),
            can_go_back: false,
            can_go_forward: false,
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            cookies: Vec::new(),
            download_received: 0,
            download_total: None,
            download_state: WebEngineDownloadState::Idle,
            history: Vec::new(),
            history_index: 0,
            load_progress: 0,
            channels: Vec::new(),
            zoom_factor: 1.0,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            certificate_error: Signal1::new(),
            console_message: Signal1::new(),
            download_requested: Signal1::new(),
            page_created: Signal1::new(),
            page_destroyed: Signal1::new(),
            load_progress_changed: Signal1::new(),
            channel_message: Signal1::new(),
            link_hovered: Signal1::new(),
            render_process_crashed: Signal1::new(),
            js_evaluated: Signal1::new(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            background_transparent: false,
            find_needle: String::new(),
            find_case_sensitive: false,
            find_matches: Vec::new(),
            find_current: None,
            pending_javascript: None,
            cookie_persistence_enabled: false,
            html_content: String::new(),
            html_blocks: Vec::new(),
            scroll_offset: 0,
            page_content_height: 0,
            download_dir: None,
            download_progress_changed: Signal1::new(),
            zoom_out_rect: None,
            zoom_in_rect: None,
            next_page_id: FIRST_PAGE_ID,
            #[cfg(feature = "js-engine")]
            js_engine: None,
        }
    }
    /// Returns the current URL, or an empty string when nothing has been loaded.
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Returns whether a load is in progress. Because a load completes only when
    /// the load timer tick arrives, this can remain `true` until then.
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    /// Returns the page title, or an empty string when none has been set.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets the page title, emitting [`WebEngineView::title_changed`] and
    /// requesting a redraw only when the value actually differs.
    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title.clone();
            // The current history entry restores the title the page had; keeping it in step here is
            // what makes back/forward restore a title rather than a stale one.
            if let Some(entry) = self.history.get_mut(self.history_index) {
                entry.title = title.clone();
            }
            self.title_changed.emit(title);
            self.base.request_redraw();
        }
    }
    /// Returns whether going back is currently available. The flag reflects the
    /// simulated navigation state, not a real history stack.
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    /// Returns whether going forward is currently available.
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
    /// Returns whether JavaScript evaluation is permitted for this view.
    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    /// Returns whether plugin content is enabled for this view.
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    /// Returns whether the view is in private browsing mode.
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }

    /// Returns the timer id the widget registers to complete a pending load;
    /// callers driving the event loop treat that timer's tick as the signal to
    /// finish loading.
    pub fn load_timer_id() -> u32 {
        Self::LOAD_TIMER_ID
    }

    fn begin_loading(&mut self) {
        self.loading = true;
        self.pending_load = true;
        self.load_progress = 0;
        self.loading_started.emit(self.url.clone());
        self.base.request_redraw();
    }

    fn finish_loading(&mut self) {
        if self.loading {
            self.loading = false;
            self.pending_load = false;
            self.report_load_progress(100);
            self.loading_finished.emit(self.url.clone());
            self.base.request_redraw();
        }
    }

    /// Replaces the URL and starts a simulated load, emitting
    /// [`WebEngineView::url_changed`] then [`WebEngineView::loading_started`] and
    /// refreshing the navigation flags. Does nothing if `url` is already current.
    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url.clone();
            self.push_history(&url);
            self.sync_history_flags();
            self.url_changed.emit(url.clone());
            self.begin_loading();
            self.update_navigation_state();
            self.base.request_redraw();
        }
    }

    /// Returns the load progress reported so far, `0..=100`.
    pub fn load_progress(&self) -> u32 {
        self.load_progress
    }

    /// Reports host-supplied load progress, in percent.
    ///
    /// The view performs no network I/O, so the host owns the measurement. Values above 100 are
    /// clamped; a change is announced through [`Self::load_progress_changed`].
    pub fn report_load_progress(&mut self, percent: u32) {
        let percent = percent.min(100);
        if self.load_progress == percent {
            return;
        }
        self.load_progress = percent;
        self.load_progress_changed.emit(percent);
        self.base.request_redraw();
    }

    /// Appends a URL to the history, discarding any forward entries.
    fn push_history(&mut self, url: &str) {
        self.history.truncate(self.history_index + 1);
        self.history.push(HistoryEntry {
            url: url.to_string(),
            title: self.title.clone(),
            scroll_pos: 0.0,
        });
        self.history_index = self.history.len() - 1;
    }

    /// Returns the visited URL list, oldest first.
    pub fn history(&self) -> Vec<String> {
        self.history.iter().map(|entry| entry.url.clone()).collect()
    }

    /// Returns the full history entries, including the title and scroll position each restores.
    pub fn history_entries(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Records the current page's scroll position on its history entry.
    ///
    /// A navigation that leaves the page calls this so going back restores where the reader was,
    /// not just which document was open.
    pub fn set_history_scroll(&mut self, scroll_pos: f32) {
        if !scroll_pos.is_finite() {
            return;
        }
        if let Some(entry) = self.history.get_mut(self.history_index) {
            entry.scroll_pos = scroll_pos.max(0.0);
        }
    }

    /// Returns the index of the current history entry.
    pub fn history_index(&self) -> usize {
        self.history_index
    }

    /// Registers a web-channel name the page may post to.
    pub fn register_channel(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !self.channels.iter().any(|existing| existing == &name) {
            self.channels.push(name);
        }
    }

    /// Posts a payload to a registered channel.
    ///
    /// Returns `false` when the channel is not registered. The message is delivered through
    /// [`Self::channel_message`] because this widget owns no DOM for the host's script to read.
    pub fn send_to_web_channel(&mut self, channel: &str, payload: &str) -> bool {
        if !self.channels.iter().any(|name| name == channel) {
            return false;
        }
        self.channel_message.emit((channel.to_string(), payload.to_string()));
        true
    }

    /// Reports a payload posted by the page, echoing it to [`Self::channel_message`].
    pub fn post_from_web_channel(&mut self, channel: &str, payload: &str) {
        self.channel_message.emit((channel.to_string(), payload.to_string()));
    }

    /// Returns the registered web-channel names.
    pub fn channels(&self) -> &[String] {
        &self.channels
    }

    /// Returns the page zoom factor (`1.0` is 100%).
    pub fn zoom_factor(&self) -> f32 {
        self.zoom_factor
    }

    /// The page's vertical scroll offset in logical pixels.
    pub fn scroll_offset(&self) -> i32 {
        self.scroll_offset
    }

    /// Scrolls the page, clamped to the content the last frame measured.
    pub fn set_scroll_offset(&mut self, offset: i32) {
        let max = (self.page_content_height
            - crate::widget::metrics::dimensions::WEB_URL_BAR_HEIGHT as i32)
            .max(0);
        let clamped = offset.clamp(0, max);
        if clamped == self.scroll_offset {
            return;
        }
        self.scroll_offset = clamped;
        self.base.request_redraw();
    }

    /// The directory the host stores downloads in, when one was declared.
    pub fn download_dir(&self) -> Option<&str> {
        self.download_dir.as_deref()
    }

    /// Declares where downloads are written.
    ///
    /// The view itself performs no file I/O; the directory is carried so the host's download
    /// handler and the progress readout name the same destination (MW-10).
    pub fn set_download_dir(&mut self, dir: impl Into<String>) {
        let dir = dir.into();
        if self.download_dir.as_deref() == Some(dir.as_str()) {
            return;
        }
        self.download_dir = if dir.is_empty() { None } else { Some(dir) };
        self.base.request_redraw();
    }

    /// Sets the page zoom factor, clamped to `0.25..=5.0`.
    pub fn set_zoom_factor(&mut self, factor: f32) {
        if !factor.is_finite() {
            return;
        }
        let factor = factor.clamp(0.25, 5.0);
        if self.zoom_factor == factor {
            return;
        }
        self.zoom_factor = factor;
        self.base.request_redraw();
    }
    /// "Loads" the supplied HTML synchronously: stores the source for
    /// [`WebEngineView::html_source`], sets the URL to `data:text/html` and the
    /// title to `HTML Content`, then emits the load, title, URL and navigation
    /// signals in that order. No DOM is built and none of the HTML is rendered.
    pub fn load_html(&mut self, html: &str) {
        // Navigation and loading state are simulated (no DOM layout), but the
        // HTML source is retained so script evaluation and introspection work.
        self.html_content = html.to_string();
        self.html_blocks = parse_html_blocks(html);
        self.scroll_offset = 0;
        self.clear_find();
        self.url = "data:text/html".to_string();
        self.push_history(&self.url.clone());
        self.sync_history_flags();
        self.title = "HTML Content".to_string();
        self.begin_loading();
        self.finish_loading();
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    /// Return the most recently loaded HTML source.
    pub fn html_source(&self) -> &str {
        &self.html_content
    }
    /// Loads an in-memory payload, storing `data` decoded as lossy UTF-8, then
    /// setting the URL to `base_url` and the title to `Data Content`.
    ///
    /// The MIME type and encoding arguments are currently ignored because no
    /// decoding or layout is performed. Emits the same signals as `load_html`.
    pub fn load_data(&mut self, data: &[u8], _mime_type: &str, _encoding: &str, base_url: &str) {
        // Data payloads are retained for introspection; no binary media decode
        // or layout happens in this simulated view.
        self.html_content = String::from_utf8_lossy(data).into_owned();
        self.html_blocks = parse_html_blocks(&self.html_content);
        self.clear_find();
        self.url = base_url.to_string();
        self.push_history(&self.url.clone());
        self.sync_history_flags();
        self.title = "Data Content".to_string();
        self.begin_loading();
        self.finish_loading();
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
        self.base.request_redraw();
    }
    /// Models a back navigation by flipping the availability flags: `can_go_back`
    /// becomes false and `can_go_forward` true. No history is kept and the URL is
    /// left unchanged. No-op when going back is unavailable.
    pub fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let entry = self.history[self.history_index].clone();
            self.restore_history_entry(entry);
            self.sync_history_flags();
            self.url_changed.emit(self.url.clone());
            self.begin_loading();
            self.update_navigation_state();
        }
    }
    /// Moves forward through the recorded history, restoring that entry's URL.
    pub fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let entry = self.history[self.history_index].clone();
            self.restore_history_entry(entry);
            self.sync_history_flags();
            self.url_changed.emit(self.url.clone());
            self.begin_loading();
            self.update_navigation_state();
        }
    }

    /// Restores the URL and title an entry records, emitting `title_changed` when the title differs.
    fn restore_history_entry(&mut self, entry: HistoryEntry) {
        self.url = entry.url;
        if self.title != entry.title {
            self.title = entry.title.clone();
            self.title_changed.emit(entry.title);
        }
    }

    /// Re-derives the back/forward flags from the history index.
    fn sync_history_flags(&mut self) {
        self.can_go_back = self.history_index > 0;
        self.can_go_forward = self.history_index + 1 < self.history.len();
        self.base.request_redraw();
    }
    /// Restarts the load for the current URL without fetching anything.
    ///
    /// Emits [`WebEngineView::loading_started`] again and marks a load pending, so
    /// `is_loading()` becomes `true`. It is completed the same way any other load is:
    /// the timer returned by [`WebEngineView::load_timer_id`] must be delivered, which
    /// emits [`WebEngineView::loading_finished`] and clears the flag. Call
    /// [`WebEngineView::stop`] to finish early.
    ///
    /// Nothing is fetched either way — this widget models the load lifecycle rather
    /// than performing network I/O.
    ///
    /// No-op when the URL is empty, because there is nothing to reload.
    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            self.begin_loading();
        }
    }
    /// Completes the load in progress, emitting [`WebEngineView::loading_finished`].
    /// No-op when nothing is loading.
    pub fn stop(&mut self) {
        self.finish_loading();
    }
    /// Evaluates `script` and returns its result rendered as a string.
    ///
    /// **Refused while JavaScript is disabled.** When
    /// [`WebEngineView::is_javascript_enabled`] is `false` this returns an `Err`
    /// without touching the engine, so the flag is enforced rather than advisory.
    /// Previously the flag was stored and never consulted, which meant a host that
    /// turned scripting off still executed any script it was handed — a security
    /// setting that silently did nothing.
    ///
    /// With the `js-engine` feature the script runs in a real embedded engine, created
    /// lazily on first use, and the engine's error text is returned on failure.
    /// Without that feature it always returns `Err`.
    pub fn evaluate_javascript(&mut self, script: &str) -> Result<String, String> {
        if !self.javascript_enabled {
            // Return before the engine is even created: a disabled page must not
            // have script of any kind evaluated for it.
            return Err("JavaScript is disabled for this view; call set_javascript_enabled(true) \
                 to permit evaluation"
                .to_string());
        }

        #[cfg(feature = "js-engine")]
        {
            let engine = self.js_engine.get_or_insert_with(crate::web::BoaJsEngine::new);
            engine.evaluate_to_string(script)
        }
        #[cfg(not(feature = "js-engine"))]
        {
            let _ = script;
            Err("JavaScript evaluation requires the `js-engine` feature".to_string())
        }
    }
    /// Sets whether JavaScript may be evaluated for this view.
    ///
    /// Enforced by [`WebEngineView::evaluate_javascript`], which refuses to run
    /// anything while this is `false`. Defaults to `true` at construction. It gates
    /// only this widget's own evaluation entry point; it is not a substitute for a
    /// sandbox in whatever engine the host may run elsewhere.
    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.javascript_enabled = enabled;
    }

    /// Queues a script for evaluation and returns immediately.
    ///
    /// The script is drained by [`Self::poll_async_javascript`] (which [`Widget::tick`] calls), and
    /// the result is announced through [`Self::js_evaluated`] as `(ok, payload)`. This is the
    /// asynchronous variant of [`Self::evaluate_javascript`]: a host that drives a frame loop does
    /// not block the caller while the document runs. The work is deferred, not moved to another
    /// thread — this crate has no scheduler to hand it to.
    pub fn evaluate_javascript_async(&mut self, script: &str) {
        self.pending_javascript = Some(script.to_string());
        self.base.request_redraw();
    }

    /// Runs the queued script, if any; returns whether one was waiting.
    pub fn poll_async_javascript(&mut self) -> bool {
        let Some(script) = self.pending_javascript.take() else {
            return false;
        };
        match self.evaluate_javascript(&script) {
            Ok(value) => self.js_evaluated.emit((true, value)),
            Err(error) => {
                self.js_evaluated.emit((false, error.clone()));
                self.error_occurred.emit(error);
            }
        }
        true
    }

    /// Whether a script is waiting for the next tick.
    pub fn has_pending_javascript(&self) -> bool {
        self.pending_javascript.is_some()
    }
    /// Records whether plugin content is permitted. This is a state flag only and
    /// gates no rendering.
    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.plugins_enabled = enabled;
    }
    /// Records whether private browsing is in effect. While enabled,
    /// [`Self::set_cookie`] refuses to store cookies, so a private session leaves
    /// no cookie state behind. Existing cookies are not cleared by the flag; use
    /// [`Self::clear_cookies`] for that.
    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.private_browsing = enabled;
    }

    /// Returns the cookies currently held in this view's in-memory store.
    ///
    /// Each entry is a [`Cookie`] with its path, expiry, security flags and SameSite policy, not
    /// the old `(name, value, domain)` triple. Nothing is persisted to disk automatically: the list
    /// lives with the view, and [`Self::persist_cookies`] is the serialisation a host writes itself.
    pub fn cookies(&self) -> &[Cookie] {
        &self.cookies
    }

    /// Stores a cookie, replacing the value of an existing `(name, domain)` pair.
    ///
    /// Returns `false` and stores nothing while [`Self::is_private_browsing`] is
    /// true: a private session must not leave cookie state behind. This convenience form creates
    /// a session cookie at path `/`; use [`Self::set_cookie_entry`] for the full field set.
    pub fn set_cookie(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        domain: impl Into<String>,
    ) -> bool {
        self.set_cookie_entry(&Cookie::new(name, value, domain))
    }

    /// Stores a fully specified cookie.
    ///
    /// Replacement is by `(name, domain, path)`, which is the identity a browser's cookie jar uses:
    /// two cookies with the same name on different paths are distinct.
    pub fn set_cookie_entry(&mut self, cookie: &Cookie) -> bool {
        if self.private_browsing {
            return false;
        }
        if let Some(existing) = self.cookies.iter_mut().find(|stored| {
            stored.name == cookie.name
                && stored.domain == cookie.domain
                && stored.path == cookie.path
        }) {
            *existing = cookie.clone();
        } else {
            self.cookies.push(cookie.clone());
        }
        true
    }

    /// Returns the cookie stored under `name`, if any.
    pub fn get_cookie(&self, name: &str) -> Option<&Cookie> {
        self.cookies.iter().find(|cookie| cookie.name == name)
    }

    /// Alias for [`Self::get_cookie`].
    pub fn get(&self, name: &str) -> Option<&Cookie> {
        self.get_cookie(name)
    }

    /// Removes every cookie from the in-memory store.
    pub fn clear_cookies(&mut self) {
        self.cookies.clear();
    }

    /// Alias for [`Self::clear_cookies`].
    pub fn clear(&mut self) {
        self.clear_cookies();
    }

    /// Whether the host may persist cookies. The control never writes a file itself.
    pub fn cookie_persistence_enabled(&self) -> bool {
        self.cookie_persistence_enabled
    }

    /// Enables or disables cookie persistence for this view.
    pub fn set_cookie_persistence_enabled(&mut self, enabled: bool) {
        self.cookie_persistence_enabled = enabled;
    }

    /// Serialises the cookie jar to JSON, so a host can write it to its own storage.
    ///
    /// Returns an empty string while [`Self::cookie_persistence_enabled`] is false: a view told not
    /// to persist must not hand out a document a host could save.
    pub fn persist_cookies(&self) -> String {
        if !self.cookie_persistence_enabled {
            return String::new();
        }
        let entries: Vec<serde_json::Value> = self
            .cookies
            .iter()
            .map(|cookie| {
                serde_json::json!({
                    "name": cookie.name,
                    "value": cookie.value,
                    "domain": cookie.domain,
                    "path": cookie.path,
                    "expires_ms": cookie.expires_ms,
                    "secure": cookie.secure,
                    "http_only": cookie.http_only,
                    "same_site": match cookie.same_site {
                        CookieSameSite::Unspecified => "unspecified",
                        CookieSameSite::Lax => "lax",
                        CookieSameSite::Strict => "strict",
                        CookieSameSite::None => "none",
                    },
                })
            })
            .collect();
        serde_json::Value::Array(entries).to_string()
    }

    /// Restores a cookie jar from the JSON written by [`Self::persist_cookies`].
    ///
    /// The whole document is validated before the store is replaced, so a malformed entry cannot
    /// leave the jar half-loaded.
    pub fn load_cookies(&mut self, json: &str) -> Result<(), String> {
        if self.private_browsing {
            return Err("private browsing refuses to restore cookies".to_string());
        }
        let parsed: serde_json::Value =
            serde_json::from_str(json).map_err(|error| format!("invalid cookie JSON: {error}"))?;
        let entries = parsed
            .as_array()
            .ok_or_else(|| "cookie JSON must be an array of objects".to_string())?;
        let mut restored = Vec::with_capacity(entries.len());
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "a cookie needs a name".to_string())?;
            let value = entry
                .get("value")
                .and_then(|value| value.as_str())
                .ok_or_else(|| format!("cookie {name} needs a value"))?;
            let domain = entry
                .get("domain")
                .and_then(|value| value.as_str())
                .ok_or_else(|| format!("cookie {name} needs a domain"))?;
            restored.push(Cookie {
                name: name.to_string(),
                value: value.to_string(),
                domain: domain.to_string(),
                path: entry.get("path").and_then(|value| value.as_str()).unwrap_or("/").to_string(),
                expires_ms: entry.get("expires_ms").and_then(|value| value.as_i64()),
                secure: entry.get("secure").and_then(|value| value.as_bool()).unwrap_or(false),
                http_only: entry
                    .get("http_only")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                same_site: match entry.get("same_site").and_then(|value| value.as_str()) {
                    Some("lax") => CookieSameSite::Lax,
                    Some("strict") => CookieSameSite::Strict,
                    Some("none") => CookieSameSite::None,
                    _ => CookieSameSite::Unspecified,
                },
            });
        }
        self.cookies = restored;
        Ok(())
    }

    /// The user agent the host declared, or [`DEFAULT_USER_AGENT`].
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Sets the user agent a host would send. An empty string restores the default.
    pub fn set_user_agent(&mut self, user_agent: impl Into<String>) {
        let user_agent = user_agent.into();
        self.user_agent =
            if user_agent.is_empty() { DEFAULT_USER_AGENT.to_string() } else { user_agent };
    }

    /// Whether the blank page surface is painted.
    pub fn background_transparent(&self) -> bool {
        self.background_transparent
    }

    /// Skips the page surface fill, so a rounded container or layered window shows through.
    pub fn set_background_transparent(&mut self, transparent: bool) {
        if self.background_transparent == transparent {
            return;
        }
        self.background_transparent = transparent;
        self.base.request_redraw();
    }

    /// Counts occurrences of `needle` in the retained HTML source and returns
    /// `(match_count, selected_offset)`.
    ///
    /// The search covers [`Self::html_source`] only — this widget has no DOM, so
    /// there is no rendered text to search. Matches are non-overlapping and found with a linear
    /// two-way (KMP) scan, so the worst case is O(haystack + needle) rather than the old
    /// O(haystack × needle) byte-by-byte comparison.
    /// `case_sensitive` false folds ASCII case; the returned offset is always a byte offset into
    /// the original source. `backwards` selects the last match instead of the first. An empty
    /// needle, or no match, returns `(0, 0)`. The match list is retained for
    /// [`Self::find_next`] / [`Self::find_previous`].
    pub fn find_text(
        &mut self,
        needle: &str,
        case_sensitive: bool,
        backwards: bool,
    ) -> (usize, usize) {
        self.find_needle = needle.to_string();
        self.find_case_sensitive = case_sensitive;
        if needle.is_empty() {
            self.find_matches.clear();
            self.find_current = None;
            return (0, 0);
        }
        self.find_matches =
            find_all_offsets(self.html_content.as_bytes(), needle.as_bytes(), case_sensitive);
        let selected_index = if self.find_matches.is_empty() {
            None
        } else if backwards {
            Some(self.find_matches.len() - 1)
        } else {
            Some(0)
        };
        self.find_current = selected_index;
        let selected = selected_index.map(|index| self.find_matches[index]).unwrap_or(0);
        self.base.request_redraw();
        (self.find_matches.len(), selected)
    }

    /// Selects the next match, wrapping to the first at the end.
    ///
    /// Returns the selected `(index, total)` (zero-based index), or `None` when there is no active
    /// find. The view repaints so the highlight moves with the selection.
    pub fn find_next(&mut self) -> Option<(usize, usize)> {
        if self.find_matches.is_empty() {
            return None;
        }
        let next = match self.find_current {
            Some(current) => (current + 1) % self.find_matches.len(),
            None => 0,
        };
        self.find_current = Some(next);
        self.base.request_redraw();
        Some((next, self.find_matches.len()))
    }

    /// Selects the previous match, wrapping to the last at the start.
    pub fn find_previous(&mut self) -> Option<(usize, usize)> {
        if self.find_matches.is_empty() {
            return None;
        }
        let previous = match self.find_current {
            Some(0) | None => self.find_matches.len() - 1,
            Some(current) => current - 1,
        };
        self.find_current = Some(previous);
        self.base.request_redraw();
        Some((previous, self.find_matches.len()))
    }

    /// Clears the active find, including every stored match offset.
    pub fn clear_find(&mut self) {
        if self.find_matches.is_empty() && self.find_needle.is_empty() {
            return;
        }
        self.find_needle.clear();
        self.find_matches.clear();
        self.find_current = None;
        self.base.request_redraw();
    }

    /// The `(one-based current, total)` find progress, or `(0, 0)` when nothing is selected.
    pub fn find_progress(&self) -> (usize, usize) {
        match self.find_current {
            Some(current) if !self.find_matches.is_empty() => {
                (current + 1, self.find_matches.len())
            }
            _ => (0, 0),
        }
    }

    /// The byte range of the currently selected match in the HTML source.
    pub fn current_match_range(&self) -> Option<(usize, usize)> {
        let index = self.find_current?;
        let start = *self.find_matches.get(index)?;
        Some((start, start + self.find_needle.len()))
    }

    /// Reports host-supplied progress for the most recent download.
    ///
    /// The host owns the network I/O, so the byte counts are reported rather than
    /// observed here. A known positive total that `received` has reached moves
    /// the state to [`WebEngineDownloadState::Completed`]; otherwise it is
    /// [`WebEngineDownloadState::InProgress`].
    pub fn report_download_progress(&mut self, received: u64, total: Option<u64>) {
        self.download_received = received;
        self.download_total = total;
        self.download_state = match total {
            Some(total) if total > 0 && received >= total => WebEngineDownloadState::Completed,
            _ => WebEngineDownloadState::InProgress,
        };
        // The progress readout names the file the host is writing: the last path segment of
        // the current URL is the only name this control has (MW-10).
        let name = self
            .url
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or("download")
            .to_string();
        let fraction = match total {
            Some(total) if total > 0 => (received as f32 / total as f32).clamp(0.0, 1.0),
            _ => 0.0,
        };
        self.download_progress_changed.emit((name, fraction));
        self.base.request_redraw();
    }

    /// The last host-reported download progress as `(received, total)`.
    pub fn download_progress(&self) -> (u64, Option<u64>) {
        (self.download_received, self.download_total)
    }

    /// The lifecycle state of the most recent download.
    pub fn download_state(&self) -> WebEngineDownloadState {
        self.download_state
    }
    fn update_navigation_state(&self) {
        self.navigation_state_changed.emit((self.can_go_back, self.can_go_forward));
    }

    /// Reports a load failure, emitting [`WebEngineView::error_occurred`] and ending
    /// any load in progress.
    ///
    /// This widget performs no network I/O, so the failure is supplied by the host
    /// rather than discovered here. That is deliberate: a control that invented its
    /// own errors would report them at times unrelated to what the host actually
    /// observed. The load is ended first so a caller that reacts by retrying sees
    /// `loading == false` rather than a view that claims to still be loading.
    pub fn report_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.loading {
            self.loading = false;
            self.pending_load = false;
        }
        self.base.request_redraw();
        self.error_occurred.emit(message);
    }

    /// Reports a TLS certificate problem, emitting
    /// [`WebEngineView::certificate_error`].
    ///
    /// Kept distinct from [`Self::report_error`] because the two call for different
    /// responses: a certificate failure is something a user may be asked to override
    /// once, while a load error is not.
    pub fn report_certificate_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.base.request_redraw();
        self.certificate_error.emit(message);
    }

    /// Records a JavaScript console message, emitting
    /// [`WebEngineView::console_message`] with `(message, line, source)`.
    pub fn report_console_message(
        &mut self,
        message: impl Into<String>,
        line: u32,
        source: impl Into<String>,
    ) {
        self.console_message.emit((message.into(), line, source.into()));
    }

    /// Reports a download request, emitting [`WebEngineView::download_requested`]
    /// with the URL.
    ///
    /// No file is fetched or written: the signal tells the host that a download was
    /// requested so it can decide what to do, which is the only part this widget can
    /// honestly do without an I/O stack.
    pub fn request_download(&mut self, url: impl Into<String>) {
        self.download_received = 0;
        self.download_total = None;
        self.download_state = WebEngineDownloadState::Requested;
        self.download_requested.emit(url.into());
    }

    /// Records that a child page was created, emitting
    /// [`WebEngineView::page_created`] with its id.
    ///
    /// The returned id is issued here rather than by the platform, because this
    /// widget models pages as bookkeeping and has no platform page objects to ask.
    pub fn create_page(&mut self) -> ObjectId {
        let id = self.next_page_id;
        self.next_page_id = self.next_page_id.saturating_add(1);
        self.page_created.emit(id);
        id
    }

    /// Records that the page `id` was destroyed, emitting
    /// [`WebEngineView::page_destroyed`].
    ///
    /// Returns `false` for an id this view never issued, so a host cannot make the
    /// view announce the destruction of a page it does not own.
    pub fn destroy_page(&mut self, id: ObjectId) -> bool {
        if id < FIRST_PAGE_ID || id >= self.next_page_id {
            return false;
        }
        self.page_destroyed.emit(id);
        true
    }

    /// Reloads the page `id`, returning `false` for an id this view never issued.
    ///
    /// A page id is validated against the same range `destroy_page` uses, so a host cannot ask a
    /// view to reload a page it does not own.
    pub fn reload_page(&mut self, id: ObjectId) -> bool {
        if id < FIRST_PAGE_ID || id >= self.next_page_id {
            return false;
        }
        self.reload();
        true
    }

    /// Reports the URL under the pointer, or an empty string when it leaves a link.
    ///
    /// This widget cannot hit-test a DOM, so the hover is host-reported; the signal is the
    /// standard wiring point for a status bar's URL readout.
    pub fn report_link_hover(&mut self, url: impl Into<String>) {
        self.link_hovered.emit(url.into());
    }

    /// Reports that the render process exited abnormally, emitting
    /// [`Self::render_process_crashed`].
    ///
    /// A crash is kept distinct from a load error: a host shows a "page died" recovery UI and may
    /// call [`Self::reload_page`], whereas a load error is retried or reported inline.
    pub fn report_render_process_crash(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        self.loading = false;
        self.pending_load = false;
        self.base.request_redraw();
        self.render_process_crashed.emit(reason);
    }
}
impl Widget for WebEngineView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 300)
    }

    /// Drains a script queued by [`Self::evaluate_javascript_async`].
    fn tick(&mut self, _delta_ms: u32) -> bool {
        self.poll_async_javascript()
    }

    /// A queued script is an owed result, so the frame bus keeps ticking until it is drained.
    fn is_animating(&self) -> bool {
        self.has_pending_javascript()
    }

    /// Resolves a published event name to this view's own signal.
    ///
    /// `web_engine_view` publishes eleven names and the view emits all eleven, so the mapping is
    /// one-to-one. Without an arm each name was accepted by `connect_event` and never reached: the
    /// load pipeline emitted `loading_started`, the hub held a slot nobody called.
    ///
    /// The payload spellings follow the same rule as the rest of the crate — a string travels as
    /// `String`, an identifier as `Int`, and the two composite payloads (`navigation_state_changed`,
    /// `console_message`) as their debug spelling, which is the honest lossy form for a value the
    /// hub cannot carry field by field.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "loading_started" => {
                Some(EventSignalRef::mapped("loading_started", &self.loading_started, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "loading_finished" => {
                Some(EventSignalRef::mapped("loading_finished", &self.loading_finished, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "title_changed" => {
                Some(EventSignalRef::mapped("title_changed", &self.title_changed, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "url_changed" => Some(EventSignalRef::mapped("url_changed", &self.url_changed, |v| {
                crate::widget::capability::CapabilityValue::String(v.clone())
            })),
            "error_occurred" => {
                Some(EventSignalRef::mapped("error_occurred", &self.error_occurred, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "navigation_state_changed" => Some(EventSignalRef::mapped(
                "navigation_state_changed",
                &self.navigation_state_changed,
                // The declared shape is `Tuple2` of `Bool`, so the payload arrives as the
                // (can_go_back, can_go_forward) pair rather than its `Debug` spelling.
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Bool(v.0),
                        crate::widget::capability::CapabilityValue::Bool(v.1),
                    ]))
                },
            )),
            "page_created" => {
                Some(EventSignalRef::mapped("page_created", &self.page_created, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            "page_destroyed" => {
                Some(EventSignalRef::mapped("page_destroyed", &self.page_destroyed, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v)
                }))
            }
            "console_message" => {
                Some(EventSignalRef::mapped("console_message", &self.console_message, |v| {
                    // The declared shape is `Mixed`, so the payload arrives as the
                    // (message, line, source) triple: the message and source are strings and the
                    // line is an unsigned count, rather than a `Debug` dump a subscriber cannot
                    // split back apart.
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::String(v.0.clone()),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                        crate::widget::capability::CapabilityValue::String(v.2.clone()),
                    ]))
                }))
            }
            "download_requested" => {
                Some(EventSignalRef::mapped("download_requested", &self.download_requested, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "certificate_error" => {
                Some(EventSignalRef::mapped("certificate_error", &self.certificate_error, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "channel_message" => {
                Some(EventSignalRef::mapped("channel_message", &self.channel_message, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::String(v.0.clone()),
                        crate::widget::capability::CapabilityValue::String(v.1.clone()),
                    ]))
                }))
            }
            "load_progress_changed" => Some(EventSignalRef::mapped(
                "load_progress_changed",
                &self.load_progress_changed,
                |v| crate::widget::capability::CapabilityValue::UInt(*v as u64),
            )),
            "link_hovered" => {
                Some(EventSignalRef::mapped("link_hovered", &self.link_hovered, |v| {
                    crate::widget::capability::CapabilityValue::String((*v).clone())
                }))
            }
            "render_process_crashed" => Some(EventSignalRef::mapped(
                "render_process_crashed",
                &self.render_process_crashed,
                |v| crate::widget::capability::CapabilityValue::String((*v).clone()),
            )),
            "js_evaluated" => {
                Some(EventSignalRef::mapped("js_evaluated", &self.js_evaluated, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Bool(v.0),
                        crate::widget::capability::CapabilityValue::String(v.1.clone()),
                    ]))
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `WebEngineView`'s property contract.
///
/// `WebEngineView` is a concrete type, not an alias: `WebView` is the alias
/// (`web_widgets/web_view.rs` re-exports `WebEngineView as WebView`), so this one
/// impl answers for the `WebEngineView` and `WebView` kinds alike.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: `url`, `loading`
/// and `title` are readable, and none of them is writable through this contract.
/// (The old `WidgetKind::WebEngineView` write arm downcast to `MediaPlayer`, which
/// is why it answered `source` / `playing` / `position_ms`; those names belong to
/// `MediaPlayer` and are answered there.)
impl WidgetProperties for WebEngineView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "url" => Ok(CapabilityValue::String(self.url().to_string())),
            "loading" => Ok(CapabilityValue::Bool(self.is_loading())),
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "user_agent" => Ok(CapabilityValue::String(self.user_agent().to_string())),
            "background_transparent" => Ok(CapabilityValue::Bool(self.background_transparent())),
            "cookie_persistence_enabled" => {
                Ok(CapabilityValue::Bool(self.cookie_persistence_enabled()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "url" => {
                self.set_url(expect_string(value)?);
                Ok(())
            }
            // Loading is a state the load pipeline drives, not a value a caller
            // assigns; navigation is the way to change it (`set_url` / `reload`).
            "loading" => Err(CapabilityAccessError::ReadOnlyProperty),
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "user_agent" => {
                self.set_user_agent(expect_string(value)?);
                Ok(())
            }
            "background_transparent" => {
                let transparent = match value {
                    CapabilityValue::Bool(value) => value,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_background_transparent(transparent);
                Ok(())
            }
            "cookie_persistence_enabled" => {
                let enabled = match value {
                    CapabilityValue::Bool(value) => value,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_cookie_persistence_enabled(enabled);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "url",
            "loading",
            "title",
            "user_agent",
            "background_transparent",
            "cookie_persistence_enabled",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `web_engine_view` publishes.
    ///
    /// `reload`, `go_back`, `go_forward` and `stop` are the browser's own
    /// payload-free navigation controls — the same four a toolbar's buttons drive —
    /// so a bare invocation performs them. Each is already guarded by the view's own
    /// state: going back with no history is a no-op, stopping when nothing is loading
    /// is a no-op, and reloading a view with no URL does nothing. Those are the right
    /// outcomes for a control and not errors, so they answer `Ok(())`: the command
    /// *ran*, it simply had nothing to change. Refusing them instead would make the
    /// four buttons a toolbar is built from unusable through the command layer on a
    /// freshly mounted view, which is the one state a toolbar always starts in.
    /// `load_url` needs the URL to load, which is the one thing the command name
    /// cannot supply, and `set_url` is the same write under its property name.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "reload" => {
                self.reload();
                Ok(())
            }
            "go_back" => {
                self.go_back();
                Ok(())
            }
            "go_forward" => {
                self.go_forward();
                Ok(())
            }
            "stop" => {
                self.stop();
                Ok(())
            }
            "load_url" => Err(CapabilityAccessError::OutOfRange),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for WebEngineView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::Timer { id } = event {
            if *id == Self::LOAD_TIMER_ID && self.pending_load {
                self.finish_loading();
            }
        }
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // A press on the view is reported through the shared `clicked` signal; the
            // host decides what it navigates to. Two defects were stacked here: the
            // position was discarded (so any click anywhere in the window navigated),
            // and the URL was the hard-coded literal `https://example.com/12345`,
            // identical on every press. A control cannot know which link was clicked
            // without a DOM hit-test, so inventing a URL was the wrong answer in both
            // respects — it is removed, and the click is reported instead.
            Event::MousePress { pos, button, .. }
                if *button == 1 && self.geometry().contains_point(*pos) =>
            {
                // The zoom controls are part of the page chrome, so they take the click before
                // it is reported as a page activation (MW-10).
                if self.zoom_in_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.set_zoom_factor(self.zoom_factor + 0.1);
                    return;
                }
                if self.zoom_out_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.set_zoom_factor(self.zoom_factor - 0.1);
                    return;
                }
                self.base.clicked.emit();
            }
            Event::Wheel { delta, .. } => {
                // One notch scrolls roughly three text lines; the page clamps the offset to
                // the height its last frame measured.
                self.set_scroll_offset(self.scroll_offset - delta.y * 48);
            }
            Event::KeyPress { key, modifiers } => {
                match *key {
                    37 => {
                        // Left arrow — go back
                        self.go_back();
                    }
                    39 => {
                        // Right arrow — go forward
                        self.go_forward();
                    }
                    116 => {
                        // F5 — reload
                        self.reload();
                    }
                    82
                        // Ctrl+R — reload
                        if *modifiers == 1 => {
                            self.reload();
                        }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for WebEngineView {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();

        // The view's chrome and its blank page surface resolve the explicit style first, then the
        // theme's resolved style for this control, and only then a literal. The page area used to be
        // a fixed white and the URL bar a fixed grey, so a light/dark switch left the whole view —
        // including its dominant colour — unchanged and the rendering census reported the control
        // as theme-blind. Once a real document is loaded its own pixels replace this surface.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("web_engine_view");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, accent) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `web_engine_view` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`. The
        // blank page is the window fill eased toward the text colour, so it reads as a distinct
        // document on a light theme and on a dark one; a colour the caller set still wins.
        let page = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.12),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != page)
            .unwrap_or_else(|| page.blend(&secondary, 0.45));

        // A transparent-background view is a layered surface over whatever its container paints, so
        // it still draws its URL bar and border but leaves the page area un-filled.
        if !self.background_transparent {
            ctx.fill_rect(g, page);
        }
        ctx.draw_rect(g, border);
        // Draw URL bar. The label is centred inside the bar's own height: the origin is the
        // glyph's **top** edge, so `+ 20` in a 28 px bar left a 14 px URL spanning 20..34 —
        // six pixels past the bar and over the content it frames.
        //
        // The bar's height is `dimensions::WEB_URL_BAR_HEIGHT` rather than the literal `28`,
        // so the strip the browser chrome occupies and the height a layout would be told
        // cannot disagree. The bar can also be taller than the view is: `top_band` clamps it,
        // which is what stops a 20 px view from painting a 28 px bar over its own content.
        let bar = ControlMetrics::top_band(g, dimensions::WEB_URL_BAR_HEIGHT);
        ctx.fill_rect(bar, page.blend(&ink, 0.10));
        let url_font = Font::default_ui();
        let url_h = ctx.measure_text("M", &url_font).height;
        // An empty URL is *nothing to say*, not an empty text element: `draw_text_fitted` on an
        // empty string emitted `<text x="7" y="7" ...></text>` into `web_engine_view.svg` — a
        // zero-content element that a downstream bounds reader still has to handle. The guard
        // is the same one every other labelled control in the crate uses.
        if !self.url().is_empty() {
            ctx.draw_text_fitted_cow(
                Rect::new(
                    bar.x + 4,
                    bar.y + (bar.height as i32 - url_h as i32) / 2,
                    bar.width.saturating_sub(8),
                    url_h,
                ),
                self.url(),
                &url_font,
                ink.blend(&page, 0.35),
                HorizontalAlignment::Left,
            );
        }
        // A load in progress paints a quiet hint at the top of the page area; it never hides the
        // content already parsed.
        if self.is_loading() {
            let hint_font = Font::default_ui();
            let hint_h = ctx.measure_text("M", &hint_font).height;
            let content = ControlMetrics::content_below_top_band(g, dimensions::WEB_URL_BAR_HEIGHT);
            if content.height > 0 {
                ctx.draw_text_fitted_cow(
                    Rect::new(
                        content.x + 4,
                        content.y + (content.height as i32 - hint_h as i32) / 2,
                        content.width.saturating_sub(8),
                        hint_h,
                    ),
                    "Loading...",
                    &hint_font,
                    secondary,
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── Page content ──
        //
        // A lightweight block renderer: headings, paragraphs, links and image placeholders at
        // the page zoom factor, scrolled by `scroll_offset`. It is not a DOM layout engine, but
        // it makes the loaded document visible, which is what a web view is for (MW-10).
        let content = ControlMetrics::content_below_top_band(g, dimensions::WEB_URL_BAR_HEIGHT);
        if content.height > 0 && !self.html_blocks.is_empty() {
            let page_font = role_font(self.base.style(), 14.0 * self.zoom_factor);
            let heading_font =
                role_font(self.base.style(), 18.0 * self.zoom_factor).with_bold(true);
            let line_h = ctx.measure_text("M", &page_font).height.max(1) as i32 + 2;
            let text_width = content.width.saturating_sub(24);
            let mut y = content.y + 6 - self.scroll_offset;
            ctx.push_clip(content.x, content.y, content.width, content.height);
            for block in &self.html_blocks {
                if block.kind == HtmlBlockKind::Image {
                    let image_rect =
                        Rect::new(content.x + 8, y, text_width.min(180), (line_h * 3) as u32);
                    ctx.fill_rect(image_rect, secondary.with_alpha(40));
                    ctx.draw_rect_stroke(image_rect, secondary.with_alpha(120), 1);
                    ctx.draw_text_fitted_cow(
                        image_rect,
                        &block.text,
                        &page_font,
                        ink.blend(&page, 0.45),
                        HorizontalAlignment::Center,
                    );
                    y += line_h * 3 + 6;
                    continue;
                }
                let font =
                    if block.kind == HtmlBlockKind::Heading { &heading_font } else { &page_font };
                let ranges = crate::widget::text_utils::wrap_to_width(
                    &block.text,
                    text_width,
                    &mut |text: &str| ctx.measure_text(text, font).width,
                );
                let color = match block.kind {
                    HtmlBlockKind::Link => accent,
                    HtmlBlockKind::Heading => ink,
                    _ => ink.blend(&page, 0.25),
                };
                for range in ranges {
                    if y + line_h >= content.y && y < content.y + content.height as i32 {
                        if let Some(text) = block.text.get(range) {
                            ctx.draw_text_fitted_cow(
                                Rect::new(content.x + 8, y, text_width, line_h as u32),
                                text,
                                font,
                                color,
                                HorizontalAlignment::Left,
                            );
                        }
                    }
                    y += line_h;
                }
                y += 6;
            }
            ctx.pop_clip();
            self.page_content_height = (y + self.scroll_offset - content.y).max(0) + 6;

            // Vertical scrollbar, drawn only when the page overflows.
            let max_scroll = (self.page_content_height - content.height as i32).max(0);
            if max_scroll > 0 {
                let track = Rect::new(content.right() - 8, content.y, 8, content.height);
                ctx.fill_rect(track, secondary.with_alpha(50));
                let visible = content.height as i64;
                let total = self.page_content_height.max(1) as i64;
                let thumb_h = ((visible * visible / total) as u32).max(24).min(content.height);
                let travel = content.height.saturating_sub(thumb_h) as i32;
                let thumb_y = content.y
                    + (travel as i64 * self.scroll_offset as i64 / max_scroll.max(1) as i64) as i32;
                ctx.fill_rounded_rect(
                    Rect::new(track.x + 1, thumb_y, 6, thumb_h),
                    3,
                    secondary.with_alpha(160),
                );
            }

            // Zoom controls in the content's trailing bottom corner.
            let button = 18u32;
            let button_y = content.y + content.height as i32 - button as i32 - 6;
            let zoom_out =
                Rect::new(content.right() - 2 * button as i32 - 8, button_y, button, button);
            let zoom_in = Rect::new(content.right() - button as i32 - 4, button_y, button, button);
            self.zoom_out_rect = Some(zoom_out);
            self.zoom_in_rect = Some(zoom_in);
            for (rect, plus) in [(zoom_out, false), (zoom_in, true)] {
                ctx.fill_rounded_rect(rect, 4, page.blend(&ink, 0.18));
                let center = rect.center();
                ctx.draw_line(
                    Point::new(center.x - 4, center.y),
                    Point::new(center.x + 4, center.y),
                    ink,
                );
                if plus {
                    ctx.draw_line(
                        Point::new(center.x, center.y - 4),
                        Point::new(center.x, center.y + 4),
                        ink,
                    );
                }
            }

            // Download progress, along the page's bottom edge.
            if self.download_state == WebEngineDownloadState::InProgress {
                if let Some(total) = self.download_total.filter(|total| *total > 0) {
                    let fraction = (self.download_received as f32 / total as f32).clamp(0.0, 1.0);
                    let bar = Rect::new(content.x, content.bottom() - 4, content.width, 4);
                    ctx.fill_rect(bar, secondary.with_alpha(60));
                    let filled = (content.width as f32 * fraction) as u32;
                    if filled > 0 {
                        ctx.fill_rect(Rect::new(bar.x, bar.y, filled, bar.height), accent);
                    }
                }
            }
        }

        // ── Find highlight ──
        //
        // There is no DOM layout, so the match has no painted glyph box to outline. The band is
        // placed at the match's *proportional* position in the source instead: it moves as
        // find-next advances and is honest about being an indicator of where in the document the
        // match sits, rather than pretending to be a text selection.
        if let Some((start, _)) = self.current_match_range() {
            let content = ControlMetrics::content_below_top_band(g, dimensions::WEB_URL_BAR_HEIGHT);
            if content.height > 0 && !self.html_content.is_empty() {
                let line_h = ctx.measure_text("M", &Font::default_ui()).height.max(1);
                let fraction = (start as f32 / self.html_content.len() as f32).clamp(0.0, 1.0);
                let travel = content.height.saturating_sub(line_h);
                let top = content.y + (travel as f32 * fraction) as i32;
                let highlight = Rect::new(
                    content.x + 2,
                    top,
                    content.width.saturating_sub(4).clamp(8, 160),
                    line_h,
                );
                ctx.fill_rect(highlight, accent.with_alpha(90));
            }
        }
    }
    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::WidgetKind;
    use std::sync::{Arc, Mutex};

    #[test]
    fn web_engine_view_draw_produces_svg() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let svg = crate::widget::svg::render_to_svg(&mut wv);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn web_engine_view_url_and_title() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        assert!(wv.url().is_empty());
        assert!(wv.title().is_empty());
        wv.set_url("https://example.com".to_string());
        assert_eq!(wv.url(), "https://example.com");
    }

    #[test]
    fn web_engine_set_url_starts_then_finishes_on_timer() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let started = Arc::new(Mutex::new(0usize));
        let finished = Arc::new(Mutex::new(0usize));

        let started_sink = started.clone();
        wv.loading_started.connect(move |_| {
            if let Ok(mut count) = started_sink.lock() {
                *count += 1;
            }
        });

        let finished_sink = finished.clone();
        wv.loading_finished.connect(move |_| {
            if let Ok(mut count) = finished_sink.lock() {
                *count += 1;
            }
        });

        wv.set_url("https://example.com".to_string());
        assert!(wv.is_loading());
        assert_eq!(*started.lock().expect("started lock poisoned"), 1);
        assert_eq!(*finished.lock().expect("finished lock poisoned"), 0);

        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading());
        assert_eq!(*finished.lock().expect("finished lock poisoned"), 1);
    }

    #[test]
    fn web_engine_stop_finishes_pending_load() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        wv.set_url("https://rust-lang.org".to_string());
        assert!(wv.is_loading());

        wv.stop();
        assert!(!wv.is_loading());
    }

    /// Script evaluation must be refused while the flag is off.
    ///
    /// The defect this pins: `set_javascript_enabled(false)` stored a flag that
    /// `evaluate_javascript` never read, so a host that turned scripting off still
    /// executed whatever script it was handed — a security setting that silently did
    /// nothing. The assertion is on the refusal, which holds with or without the
    /// `js-engine` feature, so the test is meaningful in every build.
    #[test]
    fn evaluate_javascript_is_refused_while_disabled() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        assert!(wv.is_javascript_enabled(), "scripting is on by default");

        wv.set_javascript_enabled(false);
        let error =
            wv.evaluate_javascript("1 + 1").expect_err("a disabled view must not evaluate script");
        assert!(error.contains("disabled"), "the refusal must say why, got: {error:?}");

        // Re-enabling restores evaluation. With `js-engine` on it actually returns a
        // result; without it the failure must be the missing-feature one, never the
        // policy check — hence the two branches rather than an assumption.
        wv.set_javascript_enabled(true);
        match wv.evaluate_javascript("1 + 1") {
            Ok(value) => assert_eq!(value, "2", "the script must actually run once enabled"),
            Err(error) => assert!(
                error.contains("js-engine"),
                "without the feature the failure must name it, got: {error:?}"
            ),
        }
    }

    /// The engine must not even be created while scripting is disabled.
    ///
    /// `evaluate_javascript` returns before `get_or_insert_with`, so a disabled view
    /// never allocates an engine. Checked through behaviour rather than by reaching
    /// into the field: with the flag off, a script that would trap the engine is
    /// still refused by policy.
    #[test]
    fn a_disabled_view_refuses_script_before_looking_at_it() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
        wv.set_javascript_enabled(false);

        // Malformed script: if the policy check were reached *after* evaluation, the
        // error would be a parse error instead of the policy refusal.
        let error = wv.evaluate_javascript("this is not valid javascript(((").expect_err("refused");
        assert!(error.contains("disabled"), "the policy check must run first, got: {error:?}");
    }

    /// `reload()` must go through the normal load lifecycle, not wedge the widget.
    ///
    /// It marks a load pending, so `is_loading()` is true until the load timer (or
    /// `stop`) completes it — the same contract `set_url` follows. The doc previously
    /// implied the load was never finished, which read as a stuck state.
    #[test]
    fn reload_runs_the_normal_load_lifecycle() {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));

        // Nothing to reload before a URL exists.
        wv.reload();
        assert!(!wv.is_loading(), "reload with no URL is a no-op");

        wv.set_url("https://example.com/".to_string());
        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading());

        wv.reload();
        assert!(wv.is_loading(), "reload starts a load again");
        wv.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wv.is_loading(), "the load timer completes a reloaded page");
    }

    #[test]
    fn web_engine_wrappers_delegate_widget_draw_and_event_handler() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        fn assert_wrapper_behaves(mut wrapper: impl Widget + Draw) {
            assert_eq!(wrapper.base().kind(), WidgetKind::WebEngineView);
            let svg = crate::widget::svg::render_to_svg(&mut wrapper);
            assert!(svg.starts_with("<svg"));
            wrapper.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        }

        let geometry = Rect::new(0, 0, 240, 140);
        assert_wrapper_behaves(WebEnginePage::new(geometry));
        assert_wrapper_behaves(WebEngine::new(geometry));
        assert_wrapper_behaves(WebEngineSettings::new(geometry));
        assert_wrapper_behaves(WebEngineDownloadItem::new(geometry));
        assert_wrapper_behaves(WebEngineCookieStore::new(geometry));
        assert_wrapper_behaves(WebEngineWebChannel::new(geometry));
        assert_wrapper_behaves(WebEngineFindTextResult::new(geometry));
        assert_wrapper_behaves(WebEngineNotification::new(geometry));
        assert_wrapper_behaves(WebEngineScriptDialog::new(geometry));
        assert_wrapper_behaves(WebEngineContextMenuRequest::new(geometry));
    }

    #[test]
    fn web_engine_wrappers_forward_timer_completion() {
        let mut wrapper = WebEnginePage::new(Rect::new(0, 0, 320, 200));
        wrapper.inner_mut().set_url("https://example.com/path".to_string());
        assert!(wrapper.inner().is_loading());

        wrapper.handle_event(&Event::timer(WebEngineView::load_timer_id()));
        assert!(!wrapper.inner().is_loading());
    }

    /// `find_text` counts matches in the HTML source the host loaded.
    #[test]
    fn find_text_counts_matches_in_the_loaded_html() {
        let mut result = WebEngineFindTextResult::new(Rect::new(0, 0, 300, 200));
        result.inner_mut().load_html("<p>Rust rust</p>");

        let (insensitive_count, first) = result.find_text("rust", false, false);
        assert_eq!(insensitive_count, 2, "case-insensitive matching finds both spellings");

        let (sensitive_count, _) = result.find_text("rust", true, false);
        assert_eq!(sensitive_count, 1, "case-sensitive matching finds only the lowercase one");

        let (_, last) = result.find_text("rust", false, true);
        assert!(last > first, "backwards selects the last match's offset");

        assert_eq!(result.find_text("missing", false, false), (0, 0));
        assert_eq!(result.find_text("", false, false), (0, 0), "an empty needle matches nothing");
    }

    /// Private browsing refuses every cookie, so the store stays empty.
    #[test]
    fn private_browsing_refuses_to_store_cookies() {
        let mut store = WebEngineCookieStore::new(Rect::new(0, 0, 300, 200));
        store.inner_mut().set_private_browsing(true);

        assert!(!store.set_cookie("session", "abc", "example.com"));
        assert!(store.cookies().is_empty(), "private browsing must not persist a cookie");
        assert!(store.get_cookie("session").is_none());
    }

    /// The cookie store really stores, replaces and clears entries.
    #[test]
    fn cookies_round_trip_and_clear() {
        let mut store = WebEngineCookieStore::new(Rect::new(0, 0, 300, 200));
        assert!(store.set_cookie("session", "abc", "example.com"));

        let cookie = store.get_cookie("session").expect("the cookie was stored");
        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc");
        assert_eq!(cookie.domain, "example.com");
        assert_eq!(cookie.path, "/", "the convenience setter stores a root-path session cookie");

        assert!(store.set_cookie("session", "def", "example.com"));
        assert_eq!(store.cookies().len(), 1, "the same name and domain replaces the value");
        assert_eq!(store.get_cookie("session").expect("still stored").value, "def");

        store.clear_cookies();
        assert!(store.cookies().is_empty());
    }

    /// A full cookie survives a JSON round trip, and persistence is opt-in.
    #[test]
    fn cookies_persist_only_when_enabled_and_keep_every_field() {
        let mut store = WebEngineCookieStore::new(Rect::new(0, 0, 300, 200));
        let mut cookie = Cookie::new("token", "abc", "example.com");
        cookie.path = "/app".to_string();
        cookie.expires_ms = Some(1_700_000_000_000);
        cookie.secure = true;
        cookie.http_only = true;
        cookie.same_site = CookieSameSite::Strict;
        assert!(store.set_cookie_entry(&cookie));
        assert_eq!(store.persist_cookies(), "", "persistence must be opt-in");

        store.inner_mut().set_cookie_persistence_enabled(true);
        let json = store.persist_cookies();
        let mut restored = WebEngineCookieStore::new(Rect::new(0, 0, 300, 200));
        restored.inner_mut().set_cookie_persistence_enabled(true);
        restored.load_cookies(&json).expect("the persisted jar must load");
        assert_eq!(restored.cookies(), &[cookie]);
    }

    /// Find-next/previous walk the stored matches, and the highlight range follows.
    #[test]
    fn find_navigation_reports_one_based_progress_and_a_match_range() {
        let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
        view.load_html("<p>Rust rust rust</p>");
        assert_eq!(view.find_text("rust", false, false), (3, 3));
        assert_eq!(view.find_progress(), (1, 3));
        assert_eq!(view.find_next(), Some((1, 3)));
        assert_eq!(view.find_progress(), (2, 3));
        assert_eq!(view.find_previous(), Some((0, 3)));
        assert_eq!(view.find_previous(), Some((2, 3)), "previous wraps to the last match");
        let (start, end) = view.current_match_range().expect("a match is selected");
        assert_eq!(end - start, 4);
        view.clear_find();
        assert_eq!(view.find_progress(), (0, 0));
        assert!(view.current_match_range().is_none());
    }

    /// The async evaluation variant queued a script and announced the result on tick.
    #[test]
    fn deferred_javascript_reports_its_result() {
        let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let s = seen.clone();
        view.js_evaluated.connect(move |value: std::sync::Arc<(bool, String)>| {
            *s.lock().unwrap() = Some((*value).clone());
        });
        view.evaluate_javascript_async("1 + 1");
        assert!(view.has_pending_javascript());
        assert!(view.tick(16), "draining a queued script is a frame-worthy event");
        assert!(!view.has_pending_javascript());
        let (ok, payload) = seen.lock().unwrap().clone().expect("a result was announced");
        #[cfg(feature = "js-engine")]
        assert!(ok && payload == "2", "a real engine returns the value: {payload}");
        #[cfg(not(feature = "js-engine"))]
        assert!(!ok, "without an engine the honest answer is an error");
    }

    /// History entries restore a title, and the crash/hover signals resolve.
    #[test]
    fn history_titles_links_crashes_and_user_agent_are_reported() {
        let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
        let titles = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let t = titles.clone();
        view.title_changed.connect(move |value: std::sync::Arc<String>| {
            t.lock().unwrap().push((*value).clone());
        });
        view.set_url("https://example.com/a".to_string());
        view.set_title("A".to_string());
        view.set_url("https://example.com/b".to_string());
        view.set_title("B".to_string());
        view.go_back();
        assert_eq!(view.title(), "A", "back restores the title, not just the URL");
        assert!(titles.lock().unwrap().iter().any(|title| title == "A"));
        assert_eq!(view.history_entries()[1].title, "B");

        let hovered = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let h = hovered.clone();
        view.link_hovered.connect(move |value: std::sync::Arc<String>| {
            *h.lock().unwrap() = (*value).clone();
        });
        view.report_link_hover("https://example.com/link");
        assert_eq!(*hovered.lock().unwrap(), "https://example.com/link");
        view.report_link_hover("");
        assert_eq!(*hovered.lock().unwrap(), "", "leaving a link reports an empty target");

        let crashed = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let c = crashed.clone();
        view.render_process_crashed.connect(move |value: std::sync::Arc<String>| {
            *c.lock().unwrap() = (*value).clone();
        });
        view.report_render_process_crash("SIGSEGV");
        assert_eq!(*crashed.lock().unwrap(), "SIGSEGV");
        for name in ["link_hovered", "render_process_crashed", "js_evaluated"] {
            assert!(view.event_signal_dyn(name).is_some(), "{name} must resolve");
        }

        assert_eq!(view.user_agent(), DEFAULT_USER_AGENT);
        view.set_user_agent("custom/1.0");
        assert_eq!(view.user_agent(), "custom/1.0");
        view.set_user_agent("");
        assert_eq!(view.user_agent(), DEFAULT_USER_AGENT, "empty restores the default");
    }

    /// The settings facade forwards the view's real feature flags.
    #[test]
    fn settings_forward_the_views_feature_flags() {
        let mut settings = WebEngineSettings::new(Rect::new(0, 0, 300, 200));
        assert!(settings.is_javascript_enabled(), "scripting is on by default");

        settings.set_javascript_enabled(false);
        assert!(!settings.is_javascript_enabled());
        assert!(!settings.inner().is_javascript_enabled(), "the flag lives on the wrapped view");

        settings.set_plugins_enabled(true);
        assert!(settings.is_plugins_enabled());
        settings.set_private_browsing(true);
        assert!(settings.is_private_browsing());
    }

    /// Download progress is host-reported and drives a real state machine.
    #[test]
    fn download_item_tracks_host_reported_progress() {
        let mut item = WebEngineDownloadItem::new(Rect::new(0, 0, 300, 200));
        assert_eq!(item.state(), WebEngineDownloadState::Idle);

        item.inner_mut().request_download("https://example.com/file.zip");
        assert_eq!(item.state(), WebEngineDownloadState::Requested);

        item.report_progress(40, Some(100));
        assert_eq!(item.progress(), (40, Some(100)));
        assert_eq!(item.state(), WebEngineDownloadState::InProgress);

        item.report_progress(100, Some(100));
        assert_eq!(item.state(), WebEngineDownloadState::Completed);
    }

    /// Every semantic-free wrapper must declare that it is only a facade.
    #[test]
    fn placeholder_facades_declare_no_independent_semantics() {
        let source = include_str!("web_engine.rs").to_ascii_lowercase();
        let declaration =
            "placeholder facade: forwards webengineview only, no independent semantics";
        for name in [
            "WebEnginePage",
            "WebEngineWebChannel",
            "WebEngineNotification",
            "WebEngineScriptDialog",
            "WebEngineContextMenuRequest",
        ] {
            let marker = alloc::format!("pub struct {name}(").to_ascii_lowercase();
            let start =
                source.find(&marker).unwrap_or_else(|| panic!("{name} must still be defined"));
            let doc = &source[start.saturating_sub(400)..start];
            assert!(
                doc.contains(declaration),
                "{name} must document that it is a placeholder facade, got:\n{doc}"
            );
        }
    }
}

#[test]
fn web_engine_evaluate_javascript_is_real_when_feature_enabled() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
    #[cfg(feature = "js-engine")]
    {
        // Real boa_engine evaluation: arithmetic and string results.
        assert_eq!(wv.evaluate_javascript("1 + 2 * 3").as_deref(), Ok("7"));
        assert_eq!(wv.evaluate_javascript("'a' + 'b'").as_deref(), Ok("ab"));
        // Syntax/runtime errors surface instead of a fake success value.
        assert!(wv.evaluate_javascript("this is not valid js {{").is_err());
    }
    #[cfg(not(feature = "js-engine"))]
    {
        // Honest failure instead of a placeholder "Result".
        assert!(wv.evaluate_javascript("1").is_err());
    }
}

#[test]
fn web_engine_load_html_retains_source() {
    let mut wv = WebEngineView::new(Rect::new(0, 0, 300, 200));
    wv.load_html("<html><body>Hello</body></html>");
    assert_eq!(wv.html_source(), "<html><body>Hello</body></html>");
    assert_eq!(wv.url(), "data:text/html");
}

/// An unloaded view emits no text element for its empty URL.
///
/// The defect this pins: `draw` passed the (empty) URL straight to `draw_text_fitted`, so
/// `web_engine_view.svg` carried `<text x="7" y="7" ...></text>` — a zero-content element
/// that a bounds reader still has to handle and that says nothing to a user. Empty text is
/// nothing to say, not something to draw.
#[test]
fn an_empty_url_paints_no_text_element() {
    let mut wv = WebEngineView::new(Rect::new(0, 0, 240, 120));
    assert!(wv.url().is_empty(), "a fresh view has no URL");
    let svg = crate::widget::svg::render_to_svg(&mut wv);
    assert!(
        !svg.contains("></text>"),
        "an empty URL must not emit an empty text element, got:\n{svg}"
    );
}

/// The URL bar is a fixed strip, whatever height the view has.
///
/// The defect this pins: the bar was a literal `28` tall while the view's own chrome spanned
/// the whole rectangle, so the strip and the size the control reports were two unrelated
/// numbers.
#[test]
fn the_url_bar_keeps_its_own_height() {
    use crate::widget::metrics::{dimensions, ControlMetrics};
    for height in [28u32, 60, 120, 400] {
        let mut wv = WebEngineView::new(Rect::new(0, 0, 240, height));
        let svg = crate::widget::svg::render_to_svg(&mut wv);
        let bar =
            ControlMetrics::top_band(Rect::new(0, 0, 240, height), dimensions::WEB_URL_BAR_HEIGHT);
        let fill = format!("x=\"0\" y=\"0\" width=\"240\" height=\"{}\"", bar.height);
        assert!(svg.contains(&fill), "at view height {height} the bar is {bar:?}:\n{svg}");
    }
}

/// Back and forward move through a real URL history rather than flipping two flags.
#[test]
fn back_and_forward_restore_the_previous_url() {
    let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
    view.set_url("https://example.com/a".to_string());
    view.set_url("https://example.com/b".to_string());
    assert!(view.can_go_back());
    view.go_back();
    assert_eq!(view.url(), "https://example.com/a");
    assert!(view.can_go_forward());
    view.go_forward();
    assert_eq!(view.url(), "https://example.com/b");
}

/// The load percentage is host-reported and announced on every change.
#[test]
fn reported_load_progress_is_clamped_and_announced() {
    let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let s = seen.clone();
    view.load_progress_changed.connect(move |value: std::sync::Arc<u32>| {
        s.store(*value, std::sync::atomic::Ordering::SeqCst);
    });
    view.report_load_progress(250);
    assert_eq!(view.load_progress(), 100);
    assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 100);
}

/// A channel payload reaches the host only through a registered channel.
#[test]
fn a_web_channel_echoes_only_registered_names() {
    let mut view = WebEngineView::new(Rect::new(0, 0, 300, 200));
    assert!(!view.send_to_web_channel("bridge", "ping"));
    view.register_channel("bridge");
    assert!(view.send_to_web_channel("bridge", "ping"));
    assert_eq!(view.zoom_factor(), 1.0);
    view.set_zoom_factor(2.0);
    assert_eq!(view.zoom_factor(), 2.0);
}
