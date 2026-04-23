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
