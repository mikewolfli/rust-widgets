use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackingType {
    Cookies,
    LocalStorage,
    SessionStorage,
    IndexedDB,
    WebSQL,
    CacheStorage,
    ServiceWorker,
    WebBeacon,
    Fingerprinting,
    ThirdPartyScripts,
}
#[derive(Debug, Clone, Default)]
pub struct PrivacySettings {
    pub block_third_party_cookies: bool,
    pub block_tracking_cookies: bool,
    pub block_all_cookies: bool,
    pub clear_cookies_on_exit: bool,
    pub do_not_track: bool,
    pub block_tracking_types: HashSet<TrackingType>,
    pub allowed_domains: HashSet<String>,
    pub blocked_domains: HashSet<String>,
    pub cookie_duration_limit: Option<Duration>,
}
impl PrivacySettings {
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
    pub fn strict() -> Self {
        let mut settings = Self::new();
        settings.block_all_cookies = true;
        settings.clear_cookies_on_exit = true;
        settings.block_tracking_types.insert(TrackingType::Cookies);
        settings
            .block_tracking_types
            .insert(TrackingType::LocalStorage);
        settings
            .block_tracking_types
            .insert(TrackingType::SessionStorage);
        settings
            .block_tracking_types
            .insert(TrackingType::ThirdPartyScripts);
        settings
    }
    pub fn balanced() -> Self {
        Self::new()
    }
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
    pub fn allow_domain(&mut self, domain: String) {
        self.blocked_domains.remove(domain.as_str());
        self.allowed_domains.insert(domain);
    }
    pub fn block_domain(&mut self, domain: String) {
        self.allowed_domains.remove(domain.as_str());
        self.blocked_domains.insert(domain);
    }
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        if self.blocked_domains.contains(domain) {
            return false;
        }
        if self.allowed_domains.contains(domain) {
            return true;
        }
        true
    }
    pub fn should_block_tracking_type(&self, tracking_type: TrackingType) -> bool {
        self.block_tracking_types.contains(&tracking_type)
    }
}
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<u64>,
    pub max_age: Option<u64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    None,
    Lax,
    Strict,
}
impl Cookie {
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
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return now > expires;
        }
        false
    }
    pub fn is_third_party(&self, request_domain: &str) -> bool {
        !self.domain.ends_with(request_domain)
    }
}
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    cookies: HashMap<String, Cookie>,
}
impl CookieJar {
    pub fn new() -> Self {
        Self {
            cookies: HashMap::new(),
        }
    }
    pub fn add(&mut self, cookie: Cookie) {
        let key = format!("{}:{}", cookie.domain, cookie.name);
        self.cookies.insert(key, cookie);
    }
    pub fn get(&self, domain: &str, name: &str) -> Option<&Cookie> {
        let key = format!("{}:{}", domain, name);
        self.cookies.get(&key)
    }
    pub fn remove(&mut self, domain: &str, name: &str) -> Option<Cookie> {
        let key = format!("{}:{}", domain, name);
        self.cookies.remove(&key)
    }
    pub fn clear(&mut self) {
        self.cookies.clear();
    }
    pub fn clear_expired(&mut self) {
        self.cookies.retain(|_, cookie| !cookie.is_expired());
    }
    pub fn clear_for_domain(&mut self, domain: &str) {
        self.cookies.retain(|key, _| !key.starts_with(domain));
    }
    pub fn cookies_for_domain(&self, domain: &str) -> Vec<&Cookie> {
        self.cookies
            .values()
            .filter(|c| domain.ends_with(&c.domain) || c.domain.ends_with(domain))
            .filter(|c| !c.is_expired())
            .collect()
    }
    pub fn all_cookies(&self) -> &HashMap<String, Cookie> {
        &self.cookies
    }
    pub fn len(&self) -> usize {
        self.cookies.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
}
#[derive(Debug, Clone)]
pub struct TrackingAttempt {
    pub tracking_type: TrackingType,
    pub domain: String,
    pub url: String,
    pub timestamp: u64,
    pub blocked: bool,
}
#[derive(Debug, Clone, Default)]
pub struct TrackingProtection {
    settings: PrivacySettings,
    attempts: Vec<TrackingAttempt>,
    blocked_count: u64,
}
impl TrackingProtection {
    pub fn new(settings: PrivacySettings) -> Self {
        Self {
            settings,
            attempts: Vec::new(),
            blocked_count: 0,
        }
    }
    pub fn settings(&self) -> &PrivacySettings {
        &self.settings
    }
    pub fn settings_mut(&mut self) -> &mut PrivacySettings {
        &mut self.settings
    }
    pub fn check_tracking(&mut self, tracking_type: TrackingType, domain: &str, url: &str) -> bool {
        let blocked = self.settings.should_block_tracking_type(tracking_type)
            || !self.settings.is_domain_allowed(domain);
        let attempt = TrackingAttempt {
            tracking_type,
            domain: domain.to_string(),
            url: url.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            blocked,
        };
        if blocked {
            self.blocked_count += 1;
        }
        self.attempts.push(attempt);
        blocked
    }
    pub fn attempts(&self) -> &[TrackingAttempt] {
        &self.attempts
    }
    pub fn blocked_count(&self) -> u64 {
        self.blocked_count
    }
    pub fn clear_attempts(&mut self) {
        self.attempts.clear();
    }
    pub fn clear_stats(&mut self) {
        self.attempts.clear();
        self.blocked_count = 0;
    }
}
#[derive(Debug, Clone)]
pub struct BrowsingData {
    pub history: bool,
    pub cookies: bool,
    pub cache: bool,
    pub local_storage: bool,
    pub session_storage: bool,
    pub indexed_db: bool,
    pub web_sql: bool,
    pub service_workers: bool,
    pub plugin_data: bool,
    pub downloads: bool,
    pub passwords: bool,
    pub form_data: bool,
}
impl Default for BrowsingData {
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
        assert_eq!(
            settings.cookie_duration_limit,
            Some(Duration::from_secs(86400 * 30))
        );
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
        assert_eq!(
            balanced.block_third_party_cookies,
            default.block_third_party_cookies
        );
        assert_eq!(
            balanced.block_tracking_cookies,
            default.block_tracking_cookies
        );
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
        let cookie = Cookie::new(
            "session".to_string(),
            "abc123".to_string(),
            "example.com".to_string(),
        );
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
        let cookie = Cookie::new(
            "test".to_string(),
            "val".to_string(),
            "example.com".to_string(),
        );
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
        let cookie = Cookie::new(
            "test".to_string(),
            "val".to_string(),
            "other.com".to_string(),
        );
        assert!(cookie.is_third_party("example.com"));
    }

    #[test]
    fn test_cookie_is_not_third_party_same_domain() {
        let cookie = Cookie::new(
            "test".to_string(),
            "val".to_string(),
            "example.com".to_string(),
        );
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
        let cookie = Cookie::new(
            "session".to_string(),
            "abc".to_string(),
            "example.com".to_string(),
        );
        jar.add(cookie);
        assert_eq!(jar.len(), 1);
        let retrieved = jar.get("example.com", "session");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, "abc");
    }

    #[test]
    fn test_cookie_jar_remove() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new(
            "a".to_string(),
            "1".to_string(),
            "example.com".to_string(),
        ));
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
        jar.add(Cookie::new(
            "a".to_string(),
            "1".to_string(),
            "a.com".to_string(),
        ));
        jar.add(Cookie::new(
            "b".to_string(),
            "2".to_string(),
            "b.com".to_string(),
        ));
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
        jar.add(Cookie::new(
            "fresh".to_string(),
            "new".to_string(),
            "example.com".to_string(),
        ));
        jar.clear_expired();
        assert_eq!(jar.len(), 1);
        assert!(jar.get("example.com", "fresh").is_some());
    }

    #[test]
    fn test_cookie_jar_clear_for_domain() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new(
            "a".to_string(),
            "1".to_string(),
            "example.com".to_string(),
        ));
        jar.add(Cookie::new(
            "b".to_string(),
            "2".to_string(),
            "other.com".to_string(),
        ));
        jar.clear_for_domain("example.com");
        assert_eq!(jar.len(), 1);
        assert!(jar.get("other.com", "b").is_some());
    }

    #[test]
    fn test_cookie_jar_cookies_for_domain() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new(
            "a".to_string(),
            "1".to_string(),
            "example.com".to_string(),
        ));
        jar.add(Cookie::new(
            "b".to_string(),
            "2".to_string(),
            "api.example.com".to_string(),
        ));
        let cookies = jar.cookies_for_domain("example.com");
        assert_eq!(cookies.len(), 2);
    }

    #[test]
    fn test_cookie_jar_all_cookies() {
        let mut jar = CookieJar::new();
        jar.add(Cookie::new(
            "a".to_string(),
            "1".to_string(),
            "a.com".to_string(),
        ));
        jar.add(Cookie::new(
            "b".to_string(),
            "2".to_string(),
            "b.com".to_string(),
        ));
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
        tp.check_tracking(
            TrackingType::WebBeacon,
            "beacon.com",
            "https://beacon.com/pixel",
        );
        assert_eq!(tp.attempts().len(), 1);
        tp.clear_attempts();
        assert!(tp.attempts().is_empty());
        // blocked_count should still be 1
        assert_eq!(tp.blocked_count(), 1);
    }

    #[test]
    fn test_tracking_protection_clear_stats() {
        let mut tp = TrackingProtection::new(PrivacySettings::strict());
        tp.check_tracking(
            TrackingType::Fingerprinting,
            "tracker.com",
            "https://tracker.com",
        );
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
