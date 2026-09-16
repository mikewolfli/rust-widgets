// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// The kinds of tracking a page can attempt, as classified by the privacy layer.
///
/// Each variant is a storage or network channel that a site can use to recognise
/// a returning visitor. [`PrivacySettings::block_tracking_types`] holds the set
/// the caller wants refused; [`TrackingProtection::check_tracking`] is where a
/// runtime request is matched against that set.
pub enum TrackingType {
    /// HTTP cookies.
    Cookies,
    /// The `localStorage` web storage area, which persists until explicitly cleared.
    LocalStorage,
    /// The `sessionStorage` web storage area, which is scoped to one tab session.
    SessionStorage,
    /// IndexedDB databases.
    IndexedDB,
    /// Web SQL databases (deprecated web standard).
    WebSQL,
    /// The Cache Storage API used by service workers.
    CacheStorage,
    /// Service worker registration and its background execution.
    ServiceWorker,
    /// `navigator.sendBeacon` / `ping` requests that report a visit on unload.
    WebBeacon,
    /// Browser or device fingerprinting built from passively collected attributes.
    Fingerprinting,
    /// Scripts loaded from a domain other than the page's own origin.
    ThirdPartyScripts,
}
#[derive(Debug, Clone, Default)]
/// Policy knobs consulted when the web layer decides whether to allow a cookie or
/// a tracking channel.
///
/// This type is a plain data holder: nothing here enforces itself. Construct one
/// of the presets ([`PrivacySettings::new`], [`PrivacySettings::strict`],
/// [`PrivacySettings::balanced`], [`PrivacySettings::permissive`]), hand it to
/// [`TrackingProtection::new`], and let that type apply the policy.
pub struct PrivacySettings {
    /// Refuse cookies whose domain does not match the page's own domain.
    pub block_third_party_cookies: bool,
    /// Refuse cookies whose domain appears in the tracker lists.
    pub block_tracking_cookies: bool,
    /// Refuse every cookie, first-party included. Implies the two flags above are moot.
    pub block_all_cookies: bool,
    /// Drop the cookie store when the session ends rather than persisting it.
    pub clear_cookies_on_exit: bool,
    /// Ask sites not to track the user (the `DNT: 1` request header).
    ///
    /// This is a request, not an enforcement mechanism: sites are free to ignore it.
    pub do_not_track: bool,
    /// The tracking channels that must be refused outright.
    ///
    /// Matched by exact equality in [`PrivacySettings::should_block_tracking_type`].
    pub block_tracking_types: HashSet<TrackingType>,
    /// Domains explicitly permitted, checked by exact string match.
    pub allowed_domains: HashSet<String>,
    /// Domains explicitly refused, checked by exact string match.
    ///
    /// A domain present in both sets is treated as blocked.
    pub blocked_domains: HashSet<String>,
    /// Maximum age a cookie may declare; `None` removes the cap.
    ///
    /// Cookies declaring a longer `Max-Age` are expected to be clamped to this
    /// value. The presets use 30 days (`86400 * 30` seconds) or no limit.
    pub cookie_duration_limit: Option<Duration>,
}
impl PrivacySettings {
    /// The default "balanced" policy.
    ///
    /// Blocks third-party and tracking cookies, requests `DNT`, refuses
    /// [`TrackingType::WebBeacon`] and [`TrackingType::Fingerprinting`], and caps
    /// cookie lifetime at 30 days. First-party cookies, local storage and
    /// session storage stay available, and the cookie store survives exit.
    pub fn new() -> Self {
        let mut block_tracking_types = HashSet::new();
        block_tracking_types.insert(TrackingType::WebBeacon);
        block_tracking_types.insert(TrackingType::Fingerprinting);
        Self {
            block_third_party_cookies: true,
            block_tracking_cookies: true,
            block_all_cookies: false,
            clear_cookies_on_exit: false,
            do_not_track: true,
            block_tracking_types,
            allowed_domains: HashSet::new(),
            blocked_domains: HashSet::new(),
            cookie_duration_limit: Some(Duration::from_secs(86400 * 30)),
        }
    }
    /// The most restrictive preset: everything [`PrivacySettings::new`] sets,
    /// plus refusal of all cookies, clearing the cookie store on exit, and
    /// blocking [`TrackingType::Cookies`], [`TrackingType::LocalStorage`],
    /// [`TrackingType::SessionStorage`] and [`TrackingType::ThirdPartyScripts`].
    pub fn strict() -> Self {
        let mut settings = Self::new();
        settings.block_all_cookies = true;
        settings.clear_cookies_on_exit = true;
        settings.block_tracking_types.insert(TrackingType::Cookies);
        settings.block_tracking_types.insert(TrackingType::LocalStorage);
        settings.block_tracking_types.insert(TrackingType::SessionStorage);
        settings.block_tracking_types.insert(TrackingType::ThirdPartyScripts);
        settings
    }
    /// The middle-of-the-road preset; identical to [`PrivacySettings::new`].
    pub fn balanced() -> Self {
        Self::new()
    }
    /// The least restrictive preset: no cookie blocking, no `DNT` request, no
    /// blocked tracking channels and no cookie lifetime cap.
    ///
    /// Note that `allowed_domains` and `blocked_domains` still start empty, so
    /// this is a policy preset rather than an empty value.
    pub fn permissive() -> Self {
        Self {
            block_third_party_cookies: false,
            block_tracking_cookies: false,
            block_all_cookies: false,
            clear_cookies_on_exit: false,
            do_not_track: false,
            block_tracking_types: HashSet::new(),
            allowed_domains: HashSet::new(),
            blocked_domains: HashSet::new(),
            cookie_duration_limit: None,
        }
    }
    /// Adds `domain` to the allow-list, removing it from the block-list if it was
    /// there. The two lists are kept disjoint.
    pub fn allow_domain(&mut self, domain: String) {
        self.blocked_domains.remove(domain.as_str());
        self.allowed_domains.insert(domain);
    }
    /// Adds `domain` to the block-list, removing it from the allow-list if it was
    /// there. The two lists are kept disjoint.
    pub fn block_domain(&mut self, domain: String) {
        self.allowed_domains.remove(domain.as_str());
        self.blocked_domains.insert(domain);
    }
    /// Reports whether `domain` may be used.
    ///
    /// Only the block-list can refuse a domain — an unknown domain is allowed,
    /// because trusting everything except explicit exclusions is the intended
    /// default. Note that the comparison is an exact string match, so neither
    /// subdomains nor a `www.` prefix are covered by an entry.
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        if self.blocked_domains.contains(domain) {
            return false;
        }
        if self.allowed_domains.contains(domain) {
            return true;
        }
        true
    }
    /// Reports whether `tracking_type` is in [`PrivacySettings::block_tracking_types`].
    pub fn should_block_tracking_type(&self, tracking_type: TrackingType) -> bool {
        self.block_tracking_types.contains(&tracking_type)
    }
}
#[derive(Debug, Clone)]
/// A single HTTP cookie as held by [`CookieJar`].
///
/// These fields mirror the cookie attributes a browser deals with; the jar does
/// not interpret them beyond comparing `domain` and `name` for identity and
/// reading `expires` in [`Cookie::is_expired`].
///
/// `path`, `max_age`, `secure`, `http_only` and `same_site` are carried for the
/// caller and are not enforced by this module.
pub struct Cookie {
    /// The cookie name.
    pub name: String,
    /// The cookie value.
    pub value: String,
    /// The domain the cookie is scoped to.
    pub domain: String,
    /// The path prefix the cookie is scoped to, e.g. `/`.
    pub path: String,
    /// Expiry as a Unix timestamp in **seconds** since the epoch; `None` means a
    /// session cookie, which [`Cookie::is_expired`] never treats as expired.
    pub expires: Option<u64>,
    /// The `Max-Age` attribute in **seconds**.
    ///
    /// Stored for the caller; [`Cookie::is_expired`] reads `expires` instead.
    pub max_age: Option<u64>,
    /// Whether the cookie is restricted to HTTPS transport.
    pub secure: bool,
    /// Whether the cookie is hidden from scripts running on the page.
    pub http_only: bool,
    /// The cross-site request policy for the cookie.
    pub same_site: SameSite,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The `SameSite` cookie attribute.
///
pub enum SameSite {
    /// No restriction: the cookie is sent with cross-site requests too.
    None,
    /// Sent with cross-site requests only for top-level navigations using safe
    /// methods. This is the default for [`Cookie::new`].
    Lax,
    /// Never sent with cross-site requests.
    Strict,
}
impl Cookie {
    /// Creates a cookie with the given identity and permissive defaults.
    ///
    /// `path` is `/`, `expires` and `max_age` are `None` (a session cookie),
    /// `secure` and `http_only` are `false`, and `same_site` is [`SameSite::Lax`].
    pub fn new(name: String, value: String, domain: String) -> Self {
        Self {
            name,
            value,
            domain,
            path: "/".to_string(),
            expires: None,
            max_age: None,
            secure: false,
            http_only: false,
            same_site: SameSite::Lax,
        }
    }
    /// Reports whether the cookie is past its expiry time.
    ///
    /// A cookie with `expires == None` is a session cookie and is never expired.
    /// Otherwise the comparison is `now > expires` in whole seconds since the
    /// Unix epoch, so a cookie is still valid during the exact second it expires.
    /// If the system clock reads before the epoch, `now` falls back to `0`.
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            return now > expires;
        }
        false
    }
    /// Reports whether the cookie is third-party relative to `request_domain`.
    ///
    /// The check is a domain **suffix** test (`cookie.domain.ends_with(request_domain)`),
    /// not an equality test and not an origin check: a cookie for `example.com`
    /// is first-party for a request to `example.com` or `sub.example.com`. A
    /// request domain that the cookie's domain merely ends with (e.g. cookie
    /// `ample.com`, request `example.com`) also counts as first-party, so callers
    /// should pass real registrable domains.
    pub fn is_third_party(&self, request_domain: &str) -> bool {
        !self.domain.ends_with(request_domain)
    }
}
#[derive(Debug, Clone, Default)]
/// An in-memory cookie store keyed by `domain:name`.
///
/// The jar performs no domain, path or expiry reordering on lookup: a cookie is
/// replaced when another cookie with the same domain and name is added.
/// Expired cookies are kept until [`CookieJar::clear_expired`] or
/// [`CookieJar::clear_for_domain`] is called; only
/// [`CookieJar::cookies_for_domain`] filters them out.
pub struct CookieJar {
    /// The stored cookies, keyed by `"{domain}:{name}"`.
    cookies: HashMap<String, Cookie>,
}
impl CookieJar {
    /// Creates an empty jar.
    pub fn new() -> Self {
        Self { cookies: HashMap::new() }
    }
    /// Stores `cookie`, overwriting any existing cookie with the same domain and name.
    pub fn add(&mut self, cookie: Cookie) {
        let key = format!("{}:{}", cookie.domain, cookie.name);
        self.cookies.insert(key, cookie);
    }
    /// Looks up the cookie for `domain` and `name`, or `None` if absent.
    ///
    /// Expiry is not considered; use [`Cookie::is_expired`] on the result if the
    /// caller needs a live cookie.
    pub fn get(&self, domain: &str, name: &str) -> Option<&Cookie> {
        let key = format!("{domain}:{name}");
        self.cookies.get(&key)
    }
    /// Removes and returns the cookie for `domain` and `name`, or `None` if absent.
    pub fn remove(&mut self, domain: &str, name: &str) -> Option<Cookie> {
        let key = format!("{domain}:{name}");
        self.cookies.remove(&key)
    }
    /// Discards every cookie, expired ones included.
    pub fn clear(&mut self) {
        self.cookies.clear();
    }
    /// Discards only the cookies for which [`Cookie::is_expired`] is true.
    pub fn clear_expired(&mut self) {
        self.cookies.retain(|_, cookie| !cookie.is_expired());
    }
    /// Discards every cookie whose `domain:name` key starts with `domain`.
    ///
    /// This is a key **prefix** match, so it also removes cookies of any domain
    /// that merely starts with the same text (passing `example.com` would also
    /// drop an `example.com.evil` cookie). Cookie subdomains are *not* removed by
    /// their parent's name, since the stored domains differ.
    pub fn clear_for_domain(&mut self, domain: &str) {
        self.cookies.retain(|key, _| !key.starts_with(domain));
    }
    /// Returns the unexpired cookies associated with `domain`.
    ///
    /// Both directions of the suffix relation are accepted — the requested
    /// `domain` may extend the cookie's domain or the cookie's domain may extend
    /// `domain` — so a parent domain sees its subdomains' cookies and vice versa.
    /// Ordering is the map's, which is unspecified.
    pub fn cookies_for_domain(&self, domain: &str) -> Vec<&Cookie> {
        self.cookies
            .values()
            .filter(|c| domain.ends_with(&c.domain) || c.domain.ends_with(domain))
            .filter(|c| !c.is_expired())
            .collect()
    }
    /// Borrows the whole store, expired cookies included, keyed by `"{domain}:{name}"`.
    pub fn all_cookies(&self) -> &HashMap<String, Cookie> {
        &self.cookies
    }
    /// The number of stored cookies, expired ones included.
    pub fn len(&self) -> usize {
        self.cookies.len()
    }
    /// Reports whether the jar holds no cookies at all.
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
}
#[derive(Debug, Clone)]
/// A record of one tracking check, kept by [`TrackingProtection`] for auditing.
pub struct TrackingAttempt {
    /// The channel that was requested.
    pub tracking_type: TrackingType,
    /// The domain the request was made on behalf of.
    pub domain: String,
    /// The full URL of the request.
    pub url: String,
    /// When the check ran, as Unix seconds since the epoch.
    pub timestamp: u64,
    /// The verdict the policy returned: `true` if the request was refused.
    pub blocked: bool,
}
#[derive(Debug, Clone, Default)]
/// Evaluates tracking requests against a [`PrivacySettings`] policy and keeps a
/// running log of the verdicts.
///
/// The log is unbounded: every call to [`TrackingProtection::check_tracking`]
/// appends an entry, so a long-lived instance accumulates one record per check
/// until [`TrackingProtection::clear_attempts`] or
/// [`TrackingProtection::clear_stats`] is called.
pub struct TrackingProtection {
    /// The policy applied to every check.
    settings: PrivacySettings,
    /// Every check made so far, oldest first.
    attempts: Vec<TrackingAttempt>,
    /// How many of the recorded checks were blocked.
    blocked_count: u64,
}
impl TrackingProtection {
    /// Creates a tracker that enforces `settings` and has an empty log.
    pub fn new(settings: PrivacySettings) -> Self {
        Self { settings, attempts: Vec::new(), blocked_count: 0 }
    }
    /// Borrows the active policy.
    pub fn settings(&self) -> &PrivacySettings {
        &self.settings
    }
    /// Mutably borrows the active policy, for changing it in place.
    pub fn settings_mut(&mut self) -> &mut PrivacySettings {
        &mut self.settings
    }
    /// Decides whether a `tracking_type` request on `domain` for `url` may proceed.
    ///
    /// The request is blocked when its type is in
    /// [`PrivacySettings::block_tracking_types`] **or** `domain` is not allowed
    /// by [`PrivacySettings::is_domain_allowed`]. Either way the outcome is
    /// recorded in the attempt log — this method is the log's only writer — and
    /// the count of blocked attempts is incremented when the verdict is `true`.
    ///
    /// The `url` is stored verbatim for auditing; it takes no part in the decision.
    pub fn check_tracking(&mut self, tracking_type: TrackingType, domain: &str, url: &str) -> bool {
        let blocked = self.settings.should_block_tracking_type(tracking_type)
            || !self.settings.is_domain_allowed(domain);
        let attempt = TrackingAttempt {
            tracking_type,
            domain: domain.to_string(),
            url: url.to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            blocked,
        };
        if blocked {
            self.blocked_count += 1;
        }
        self.attempts.push(attempt);
        blocked
    }
    /// The full request log, oldest first.
    pub fn attempts(&self) -> &[TrackingAttempt] {
        &self.attempts
    }
    /// How many requests have been blocked since the counter was last reset.
    ///
    /// This counts blocked verdicts, not log entries; it is unaffected by
    /// [`TrackingProtection::clear_attempts`], which empties the log but leaves
    /// the counter alone.
    pub fn blocked_count(&self) -> u64 {
        self.blocked_count
    }
    /// Empties the request log, leaving [`TrackingProtection::blocked_count`] as it is.
    pub fn clear_attempts(&mut self) {
        self.attempts.clear();
    }
    /// Empties the request log and resets [`TrackingProtection::blocked_count`] to zero.
    pub fn clear_stats(&mut self) {
        self.attempts.clear();
        self.blocked_count = 0;
    }
}
#[derive(Debug, Clone)]
/// A selection of browsing-data categories, used as a set of checkboxes when
/// clearing site data.
///
/// Every field is an independent flag; the type imposes no relationship between
/// them, so any combination is representable. [`BrowsingData::default`] enables
/// the four low-risk categories and [`BrowsingData::all`] enables everything.
pub struct BrowsingData {
    /// Browsing history entries.
    pub history: bool,
    /// HTTP cookies.
    pub cookies: bool,
    /// Cached resources.
    pub cache: bool,
    /// `localStorage` contents.
    pub local_storage: bool,
    /// `sessionStorage` contents.
    pub session_storage: bool,
    /// IndexedDB databases.
    pub indexed_db: bool,
    /// Web SQL databases (deprecated web standard).
    pub web_sql: bool,
    /// Service worker registrations and their caches.
    pub service_workers: bool,
    /// Data held by browser plugins.
    pub plugin_data: bool,
    /// The download history.
    pub downloads: bool,
    /// Saved passwords.
    pub passwords: bool,
    /// Saved form entries and autofill data.
    pub form_data: bool,
}
impl Default for BrowsingData {
    /// Enables `history`, `cookies`, `cache`, `local_storage` and
    /// `session_storage`. The remaining categories are destructive or security
    /// relevant and stay disabled until the caller opts in.
    fn default() -> Self {
        Self {
            history: true,
            cookies: true,
            cache: true,
            local_storage: true,
            session_storage: true,
            indexed_db: false,
            web_sql: false,
            service_workers: false,
            plugin_data: false,
            downloads: false,
            passwords: false,
            form_data: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── PrivacySettings tests ──

    #[test]
    fn test_privacy_settings_new() {
        let settings = PrivacySettings::new();
        assert!(settings.block_third_party_cookies);
        assert!(settings.block_tracking_cookies);
        assert!(!settings.block_all_cookies);
        assert!(!settings.clear_cookies_on_exit);
        assert!(settings.do_not_track);
        assert!(settings.should_block_tracking_type(TrackingType::WebBeacon));
        assert!(settings.should_block_tracking_type(TrackingType::Fingerprinting));
        assert!(!settings.should_block_tracking_type(TrackingType::Cookies));
        assert_eq!(settings.cookie_duration_limit, Some(Duration::from_secs(86400 * 30)));
    }

    #[test]
    fn test_privacy_settings_strict() {
        let settings = PrivacySettings::strict();
        assert!(settings.block_all_cookies);
        assert!(settings.clear_cookies_on_exit);
        assert!(settings.should_block_tracking_type(TrackingType::Cookies));
        assert!(settings.should_block_tracking_type(TrackingType::LocalStorage));
        assert!(settings.should_block_tracking_type(TrackingType::SessionStorage));
        assert!(settings.should_block_tracking_type(TrackingType::ThirdPartyScripts));
        assert!(settings.should_block_tracking_type(TrackingType::WebBeacon));
        assert!(settings.should_block_tracking_type(TrackingType::Fingerprinting));
    }

    #[test]
    fn test_privacy_settings_balanced() {
        let balanced = PrivacySettings::balanced();
        let default = PrivacySettings::new();
        assert_eq!(balanced.block_third_party_cookies, default.block_third_party_cookies);
        assert_eq!(balanced.block_tracking_cookies, default.block_tracking_cookies);
        assert_eq!(balanced.block_all_cookies, default.block_all_cookies);
        assert_eq!(balanced.do_not_track, default.do_not_track);
    }

    #[test]
    fn test_privacy_settings_permissive() {
        let settings = PrivacySettings::permissive();
        assert!(!settings.block_third_party_cookies);
        assert!(!settings.block_tracking_cookies);
        assert!(!settings.block_all_cookies);
        assert!(!settings.clear_cookies_on_exit);
        assert!(!settings.do_not_track);
        assert!(settings.block_tracking_types.is_empty());
        assert!(settings.cookie_duration_limit.is_none());
    }

    #[test]
    fn test_privacy_settings_allow_domain() {
        let mut settings = PrivacySettings::new();
        settings.block_domain("bad-site.com".to_string());
        assert!(!settings.is_domain_allowed("bad-site.com"));
        settings.allow_domain("bad-site.com".to_string());
        assert!(settings.is_domain_allowed("bad-site.com"));
    }

    #[test]
    fn test_privacy_settings_block_domain() {
        let mut settings = PrivacySettings::new();
        assert!(settings.is_domain_allowed("unknown.com"));
        settings.block_domain("evil.com".to_string());
        assert!(!settings.is_domain_allowed("evil.com"));
    }

    #[test]
    fn test_privacy_settings_block_domain_removes_from_allowed() {
        let mut settings = PrivacySettings::new();
        settings.allow_domain("trusted.com".to_string());
        assert!(settings.is_domain_allowed("trusted.com"));
        settings.block_domain("trusted.com".to_string());
        assert!(!settings.is_domain_allowed("trusted.com"));
    }

    #[test]
    fn test_privacy_settings_default_implemented() {
        let settings = PrivacySettings::default();
        assert!(!settings.block_all_cookies);
    }

    // ── Cookie tests ──

    #[test]
    fn test_cookie_new() {
        let cookie =
            Cookie::new("session".to_string(), "abc123".to_string(), "example.com".to_string());
        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.domain, "example.com");
        assert_eq!(cookie.path, "/");
        assert!(!cookie.secure);
        assert!(!cookie.http_only);
        assert_eq!(cookie.same_site, SameSite::Lax);
    }

    #[test]
    fn test_cookie_is_expired_when_no_expiry() {
        let cookie = Cookie::new("test".to_string(), "val".to_string(), "example.com".to_string());
        assert!(!cookie.is_expired());
    }

    #[test]
    fn test_cookie_is_expired() {
        let cookie = Cookie {
            name: "test".to_string(),
            value: "val".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: Some(100), // year 1970 — definitely expired
            max_age: None,
            secure: false,
            http_only: false,
            same_site: SameSite::Lax,
        };
        assert!(cookie.is_expired());
    }

    #[test]
    fn test_cookie_is_third_party() {
        let cookie = Cookie::new("test".to_string(), "val".to_string(), "other.com".to_string());
        assert!(cookie.is_third_party("example.com"));
    }

    #[test]
    fn test_cookie_is_not_third_party_same_domain() {
        let cookie = Cookie::new("test".to_string(), "val".to_string(), "example.com".to_string());
        assert!(!cookie.is_third_party("example.com"));
    }

    // ── CookieJar tests ──

    #[test]
    fn test_cookie_jar_new() {
        let jar = CookieJar::new();
        assert!(jar.is_empty());
        assert_eq!(jar.len(), 0);
    }

    #[test]
    fn test_cookie_jar_add_and_get() {
        let mut jar = CookieJar::new();
        let cookie =
            Cookie::new("session".to_string(), "abc".to_string(), "example.com".to_string());
        jar.add(cookie);
        assert_eq!(jar.len(), 1);
        let retrieved = jar.get("example.com", "session");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, "abc");
    }

    #[test]
    fn test_cookie_jar_remove() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new("a".to_string(), "1".to_string(), "example.com".to_string()));
        let removed = jar.remove("example.com", "a");
        assert!(removed.is_some());
        assert!(jar.is_empty());
    }

    #[test]
    fn test_cookie_jar_remove_nonexistent() {
        let mut jar = CookieJar::new();
        assert!(jar.remove("example.com", "nonexistent").is_none());
    }

    #[test]
    fn test_cookie_jar_clear() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new("a".to_string(), "1".to_string(), "a.com".to_string()));
        jar.add(Cookie::new("b".to_string(), "2".to_string(), "b.com".to_string()));
        jar.clear();
        assert!(jar.is_empty());
    }

    #[test]
    fn test_cookie_jar_clear_expired() {
        let mut jar = CookieJar::new();
        jar.add(Cookie {
            name: "expired".to_string(),
            value: "old".to_string(),
            domain: "example.com".to_string(),
            path: "/".to_string(),
            expires: Some(1),
            max_age: None,
            secure: false,
            http_only: false,
            same_site: SameSite::Lax,
        });
        jar.add(Cookie::new("fresh".to_string(), "new".to_string(), "example.com".to_string()));
        jar.clear_expired();
        assert_eq!(jar.len(), 1);
        assert!(jar.get("example.com", "fresh").is_some());
    }

    #[test]
    fn test_cookie_jar_clear_for_domain() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new("a".to_string(), "1".to_string(), "example.com".to_string()));
        jar.add(Cookie::new("b".to_string(), "2".to_string(), "other.com".to_string()));
        jar.clear_for_domain("example.com");
        assert_eq!(jar.len(), 1);
        assert!(jar.get("other.com", "b").is_some());
    }

    #[test]
    fn test_cookie_jar_cookies_for_domain() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new("a".to_string(), "1".to_string(), "example.com".to_string()));
        jar.add(Cookie::new("b".to_string(), "2".to_string(), "api.example.com".to_string()));
        let cookies = jar.cookies_for_domain("example.com");
        assert_eq!(cookies.len(), 2);
    }

    #[test]
    fn test_cookie_jar_all_cookies() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new("a".to_string(), "1".to_string(), "a.com".to_string()));
        jar.add(Cookie::new("b".to_string(), "2".to_string(), "b.com".to_string()));
        assert_eq!(jar.all_cookies().len(), 2);
    }

    // ── TrackingProtection tests ──

    #[test]
    fn test_tracking_protection_new() {
        let tp = TrackingProtection::new(PrivacySettings::balanced());
        assert_eq!(tp.blocked_count(), 0);
        assert!(tp.attempts().is_empty());
    }

    #[test]
    fn test_tracking_protection_check_tracking_blocks() {
        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        let blocked = tp.check_tracking(
            TrackingType::Fingerprinting,
            "tracker.com",
            "https://tracker.com/pixel",
        );
        assert!(blocked);
        assert_eq!(tp.blocked_count(), 1);
    }

    #[test]
    fn test_tracking_protection_check_tracking_allows() {
        let mut tp = TrackingProtection::new(PrivacySettings::permissive());
        let blocked = tp.check_tracking(
            TrackingType::Fingerprinting,
            "tracker.com",
            "https://tracker.com/pixel",
        );
        assert!(!blocked);
        assert_eq!(tp.blocked_count(), 0);
    }

    #[test]
    fn test_tracking_protection_attempts_logged() {
        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        tp.check_tracking(TrackingType::Cookies, "ad.com", "https://ad.com/tracker");
        assert_eq!(tp.attempts().len(), 1);
        assert_eq!(tp.attempts()[0].domain, "ad.com");
    }

    #[test]
    fn test_tracking_protection_clear_attempts() {
        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        tp.check_tracking(TrackingType::WebBeacon, "beacon.com", "https://beacon.com/pixel");
        assert_eq!(tp.attempts().len(), 1);
        tp.clear_attempts();
        assert!(tp.attempts().is_empty());
        // blocked_count should still be 1
        assert_eq!(tp.blocked_count(), 1);
    }

    #[test]
    fn test_tracking_protection_clear_stats() {
        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        tp.check_tracking(TrackingType::Fingerprinting, "tracker.com", "https://tracker.com");
        assert_eq!(tp.blocked_count(), 1);
        tp.clear_stats();
        assert_eq!(tp.blocked_count(), 0);
        assert!(tp.attempts().is_empty());
    }

    #[test]
    fn test_tracking_protection_settings_access() {
        let mut tp = TrackingProtection::new(PrivacySettings::permissive());
        assert!(!tp.settings().do_not_track);
        tp.settings_mut().do_not_track = true;
        assert!(tp.settings().do_not_track);
    }

    // ── BrowsingData tests ──

    #[test]
    fn test_browsing_data_default() {
        let data = BrowsingData::default();
        assert!(data.history);
        assert!(data.cookies);
        assert!(data.cache);
        assert!(data.local_storage);
        assert!(data.session_storage);
        assert!(!data.indexed_db);
        assert!(!data.web_sql);
        assert!(!data.service_workers);
        assert!(!data.plugin_data);
        assert!(!data.downloads);
        assert!(!data.passwords);
        assert!(!data.form_data);
    }

    #[test]
    fn test_browsing_data_all() {
        let data = BrowsingData::all();
        assert!(data.history);
        assert!(data.cookies);
        assert!(data.cache);
        assert!(data.local_storage);
        assert!(data.session_storage);
        assert!(data.indexed_db);
        assert!(data.web_sql);
        assert!(data.service_workers);
        assert!(data.plugin_data);
        assert!(data.downloads);
        assert!(data.passwords);
        assert!(data.form_data);
    }

    #[test]
    fn test_browsing_data_none() {
        let data = BrowsingData::none();
        assert!(!data.history);
        assert!(!data.cookies);
        assert!(!data.cache);
        assert!(!data.local_storage);
        assert!(!data.session_storage);
        assert!(!data.indexed_db);
        assert!(!data.web_sql);
        assert!(!data.service_workers);
        assert!(!data.plugin_data);
        assert!(!data.downloads);
        assert!(!data.passwords);
        assert!(!data.form_data);
    }
}
impl BrowsingData {
    /// Selects every category.
    pub fn all() -> Self {
        Self {
            history: true,
            cookies: true,
            cache: true,
            local_storage: true,
            session_storage: true,
            indexed_db: true,
            web_sql: true,
            service_workers: true,
            plugin_data: true,
            downloads: true,
            passwords: true,
            form_data: true,
        }
    }
    /// Selects no category; the inverse of [`BrowsingData::all`].
    pub fn none() -> Self {
        Self {
            history: false,
            cookies: false,
            cache: false,
            local_storage: false,
            session_storage: false,
            indexed_db: false,
            web_sql: false,
            service_workers: false,
            plugin_data: false,
            downloads: false,
            passwords: false,
            form_data: false,
        }
    }
}
