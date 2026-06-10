# Web Engine

rust-widgets includes a complete embedded web browsing subsystem — WebView,
WebEngine, JavaScript engine, cookie management, privacy controls, tracking
protection, plugin system, and browser history persistence — all implemented
in pure Rust.

## Architecture Overview

```
┌────────────────────────────────────────────────────┐
│  WebViewEnhanced  │  WebEngineViewEnhanced          │  ← public widget APIs
├────────────────────────────────────────────────────┤
│                WebViewCore (shared)                 │  ← common implementation
├──────────┬──────────┬──────────┬───────────────────┤
│ Session  │ Browser  │ CookieJar│ TrackingProtection│
│ History  │ History  │          │                   │
├──────────┼──────────┼──────────┼───────────────────┤
│ Plugin   │ SimpleJs │ Privacy  │ SecuritySettings  │
│ Manager  │ Engine   │ Settings │                   │
├──────────┴──────────┴──────────┴───────────────────┤
│  delegate_widget!  │  8 signals                    │
└────────────────────────────────────────────────────┘
```

**Two public widget types share a common `WebViewCore` (~95% shared code):**

| Feature | `WebViewEnhanced` | `WebEngineViewEnhanced` |
|---------|-------------------|------------------------|
| `WidgetKind` | `WebView` | `WebEngineView` |
| Initial URL | `"about:blank"` | `""` (empty) |
| `reload()` on about:blank | No-op | Full reload |
| `set_plugins_enabled` | ❌ | ✅ |
| `set_private_browsing` | ❌ | ✅ |
| `certificate_error` signal | ❌ | ✅ |
| `download_requested` signal | ❌ | ✅ |

## WebView — Embedded Web Content Widget

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

// Create a web view widget
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// Navigate to a URL
view.load_url("https://example.com");
assert_eq!(view.url(), "https://example.com");

// Set URL directly
view.set_url("https://rust-lang.org".to_string());
assert_eq!(view.url(), "https://rust-lang.org");

// Load HTML directly
view.load_html("<h1>Hello, World!</h1>", Some("https://myapp.local"));
assert_eq!(view.title(), "HTML Content");
assert_eq!(view.html(), "<h1>Hello, World!</h1>");

// Load raw data
view.load_data(b"binary content", "text/plain", "https://data.url");
assert_eq!(view.url(), "https://data.url");
assert_eq!(view.content(), "binary content");

// Page metadata
println!("Loading: {}", view.is_loading());
println!("Progress: {}%", view.load_progress());
println!("Title: {}", view.title());
```

**Title and state management:**

```rust
view.set_title("My Custom Page Title".to_string());
assert_eq!(view.title(), "My Custom Page Title");

// Unique behavior: reload() skips about:blank
let new_view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
assert_eq!(new_view.url(), "about:blank");
new_view.reload();  // No-op — won't reload about:blank
assert!(!new_view.is_loading());
```

## WebEngine — Full Browser Engine

`WebEngineViewEnhanced` provides the full browser engine with additional signals
and settings:

```rust
use rust_widgets::web::WebEngineViewEnhanced;
use rust_widgets::core::Rect;

let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));

// URL starts empty (unlike WebViewEnhanced's "about:blank")
assert_eq!(engine.url(), "");

// Navigate
engine.load_url("https://example.com");
assert_eq!(engine.url(), "https://example.com");
assert!(!engine.is_loading());  // In simulated mode, load completes instantly
assert_eq!(engine.load_progress(), 100);

engine.load_html("<h1>Hello</h1>", Some("https://base.url"));
assert_eq!(engine.url(), "https://base.url");

engine.load_data(b"binary", "application/octet-stream", "https://data.url");
assert_eq!(engine.title(), "Data: application/octet-stream");

// Unique to WebEngineViewEnhanced:
engine.set_plugins_enabled(true);
assert!(engine.settings().plugins_enabled);

engine.set_private_browsing(true);
assert!(engine.settings().private_browsing);
// Enabling private browsing automatically switches to strict privacy settings
```

## Shared WebViewCore

Both widget types delegate to a shared `WebViewCore` which manages:

- URL, title, loading state, and load progress
- Session history (back/forward stacks)
- Browser history (persistent visits)
- JavaScript engine and context
- Cookie jar
- Privacy / tracking protection
- Plugin manager
- Web settings and security settings
- **8 signals** for state observation

## Navigation — Back/Forward Stacks

### SessionHistory

```rust
use rust_widgets::web::SessionHistory;

let mut history = SessionHistory::new(50);  // Max 50 entries

// Navigate builds the back stack
history.navigate("https://page1.com".to_string());
assert_eq!(history.current().unwrap(), "https://page1.com");
assert!(!history.can_go_back());

history.navigate("https://page2.com".to_string());
assert!(history.can_go_back());  // Now has page1 in back stack
assert!(!history.can_go_forward());

// Go back
let back = history.go_back();
assert_eq!(back.as_deref(), Some("https://page1.com"));
assert!(history.can_go_forward());  // page2 is in forward stack

// Go forward
let fwd = history.go_forward();
assert_eq!(fwd.as_deref(), Some("https://page2.com"));

// New navigation clears the forward stack
history.go_back();
history.navigate("https://page3.com".to_string());
assert!(!history.can_go_forward());  // Forward stack cleared

// Inspect stacks
for url in history.back_entries() {
    println!("Back: {}", url);
}
for url in history.forward_entries() {
    println!("Forward: {}", url);
}

history.clear();
```

### NavigationHistory

An alternative session history implementation with timestamped entries:

```rust
use rust_widgets::web::{NavigationHistory, NavigationEntry};

let mut history = NavigationHistory::new(100);

// Push entries with metadata
history.push(NavigationEntry {
    url: "https://example.com".to_string(),
    title: "Example Site".to_string(),
    timestamp: 1718000000,
});

let current = history.current().unwrap();
assert_eq!(current.url, "https://example.com");

// Multiple entries with back/forward
history.push(NavigationEntry {
    url: "https://page2.com".to_string(),
    title: "Page 2".to_string(),
    timestamp: 1718000001,
});

assert!(history.can_go_back());
let back = history.go_back().unwrap();
assert_eq!(back.url, "https://example.com");

let fwd = history.go_forward().unwrap();
assert_eq!(fwd.url, "https://page2.com");

// New push after go_back() truncates forward entries
history.go_back();
history.push(NavigationEntry {
    url: "https://divergent.com".to_string(),
    title: "Divergent".to_string(),
    timestamp: 1718000005,
});
assert!(!history.can_go_forward());  // Forward truncated
```

### Navigation Controls via Key Events

Both `WebViewEnhanced` and `WebEngineViewEnhanced` handle keyboard shortcuts
automatically when focused:

| Key Combination | Action |
|----------------|--------|
| `Alt + Left` | Go back |
| `Alt + Right` | Go forward |
| `F5` or `Ctrl + R` | Reload |

## Browser History Persistence

```rust
use rust_widgets::web::{BrowserHistory, HistoryEntry};

let mut history = BrowserHistory::new();  // Default: 100 entries
// Or: BrowserHistory::with_capacity(500)

// Add entries (duplicates increment visit_count instead of adding)
history.add_entry("https://example.com".to_string(), "Example".to_string());
history.add_entry("https://example.com".to_string(), "Example".to_string());
assert_eq!(history.len(), 1);  // Duplicate — visit_count is now 2

history.add_entry("https://rust-lang.org".to_string(), "Rust".to_string());
assert_eq!(history.len(), 2);

// Search by URL or title (case-insensitive)
let results = history.search("rust");
assert_eq!(results.len(), 1);
assert_eq!(results[0].url, "https://rust-lang.org");

let results = history.search("example");
assert_eq!(results.len(), 1);  // Case-insensitive match

// Most visited entries
let top = history.most_visited(5);  // Top 5 by visit_count

// Most recent entries
let recent = history.recent(10);  // Last 10 by last_visit

// Remove a specific entry
assert!(history.remove_entry("https://example.com"));
assert_eq!(history.len(), 1);

// The oldest entry is evicted when capacity is exceeded
let mut small = BrowserHistory::with_capacity(2);
small.add_entry("https://a.com".to_string(), "A".to_string());
small.add_entry("https://b.com".to_string(), "B".to_string());
small.add_entry("https://c.com".to_string(), "C".to_string());
assert_eq!(small.len(), 2);
assert_eq!(small.entries().front().unwrap().url, "https://b.com");  // A was evicted

// Iterate entries
for entry in history.entries() {
    println!("{} (visited {}×)", entry.url, entry.visit_count);
}

history.clear();
assert!(history.is_empty());
```

## JavaScript Engine

### SimpleJsEngine

A pure-Rust JavaScript interpreter supporting variables, functions, conditionals,
loops, arrays, and console logging:

```rust
use rust_widgets::web::{SimpleJsEngine, JsValue, JsResult, JsContext, JsEngine};

let mut engine = SimpleJsEngine::new();
let mut ctx = JsContext::new();

// Evaluate expressions
let result = engine.evaluate("42", &mut ctx).unwrap();
assert_eq!(result, JsValue::Number(42.0));

// Variable assignment and retrieval
engine.evaluate("var name = 'Rust';", &mut ctx).unwrap();
let name = engine.evaluate("name", &mut ctx).unwrap();
assert_eq!(name, JsValue::String("Rust".to_string()));

// Arithmetic
let calc = engine.evaluate("10 + 5 * 3", &mut ctx).unwrap();
// SimpleJsEngine evaluates literal expressions directly

// String concatenation
let greeting = engine.evaluate("'Hello, ' + 'World!'", &mut ctx).unwrap();

// Boolean expressions
let bool_val = engine.evaluate("true", &mut ctx).unwrap();
assert_eq!(bool_val, JsValue::Boolean(true));

// Function definition
engine.evaluate("function add(a, b) { return a + b; }", &mut ctx).unwrap();
// Functions are stored for later invocation

// Console logging
engine.evaluate("console.log('Debug message');", &mut ctx).unwrap();

// Read console output
for msg in ctx.console_messages() {
    println!("[{}] {} (line {})", msg.level, msg.message, msg.line);
}
```

### JsValue

The JavaScript value type supports multiple variants:

```rust
use rust_widgets::web::JsValue;

let vals = [
    JsValue::Undefined,
    JsValue::Null,
    JsValue::Boolean(true),
    JsValue::Number(42.0),
    JsValue::String("hello".to_string()),
    JsValue::Array(vec![]),
    JsValue::Object(std::collections::HashMap::new()),
    JsValue::Function,
    JsValue::Ident("myVar".to_string()),
];

// Conversion methods
assert_eq!(JsValue::Number(42.0).to_string(), "42");
assert_eq!(JsValue::Boolean(true).to_boolean(), true);
assert!(JsValue::Number(42.0).is_truthy());
assert!(!JsValue::Boolean(false).is_truthy());
assert!(!JsValue::Null.is_truthy());
assert!(!JsValue::Undefined.is_truthy());
```

### Integration with WebView

```rust
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// Evaluate JavaScript in the context of the loaded page
view.load_html("<div id='app'></div>", None);

let result = view.evaluate_javascript("42").unwrap();
assert_eq!(result, JsValue::Number(42.0));

// Variable assignment
let _ = view.evaluate_javascript("var x = 10;");

// Disabling JavaScript
view.set_javascript_enabled(false);
let result = view.evaluate_javascript("1 + 1");
assert!(result.is_err());
assert!(result.unwrap_err().message.contains("JavaScript is disabled"));

// Re-enable
view.set_javascript_enabled(true);
```

## Cookies — CookieJar

```rust
use rust_widgets::web::{CookieJar, Cookie, SameSite};

let mut jar = CookieJar::new();

// Create and add a cookie
let cookie = Cookie::new(
    "session_id".to_string(),
    "abc123def456".to_string(),
    "example.com".to_string(),
);
assert!(!cookie.is_expired());  // No expiry = session cookie

jar.add(cookie);
assert_eq!(jar.len(), 1);

// Retrieve by name
let session = jar.get("session_id", "example.com");
assert!(session.is_some());

// Domain-scoped cookies
jar.add(Cookie::new(
    "theme".to_string(),
    "dark".to_string(),
    "sub.example.com".to_string(),
));

// Get cookies for a specific domain
let domain_cookies = jar.cookies_for_domain("example.com");
println!("{} cookies for example.com", domain_cookies.len());

// Third-party cookie detection
let tp_cookie = Cookie::new(
    "tracker".to_string(),
    "data".to_string(),
    "ad-network.com".to_string(),
);
assert!(tp_cookie.is_third_party("mysite.com"));

// Clear expired cookies
jar.clear_expired();

// Clear for specific domain
jar.clear_for_domain("sub.example.com");

// List all cookies
for cookie in jar.all_cookies() {
    println!("{}={} (domain: {})", cookie.name, cookie.value, cookie.domain);
}

jar.clear();
assert!(jar.is_empty());
```

## Tracking Protection

### TrackingType — 10 Tracking Mechanisms

```rust
use rust_widgets::web::{TrackingType, TrackingProtection, PrivacySettings};

let tracking_types = [
    TrackingType::Cookies,
    TrackingType::LocalStorage,
    TrackingType::SessionStorage,
    TrackingType::IndexedDB,
    TrackingType::WebSQL,
    TrackingType::CacheStorage,
    TrackingType::ServiceWorker,
    TrackingType::WebBeacon,
    TrackingType::Fingerprinting,
    TrackingType::ThirdPartyScripts,
];
```

### TrackingProtection

```rust
// Start with strict privacy settings
let mut protection = TrackingProtection::new(PrivacySettings::strict());

// Check if tracking should be blocked
let blocked = protection.check_tracking(
    TrackingType::Fingerprinting,
    "tracker.com",
    "https://tracker.com/beacon",
);
assert!(blocked);  // Fingerprinting is blocked in strict mode

// Tracked attempts are logged
println!("Blocked: {}", protection.blocked_count());

for attempt in protection.attempts() {
    println!(
        "{:?} from {} — {}",
        attempt.tracking_type,
        attempt.domain,
        if attempt.blocked { "BLOCKED" } else { "ALLOWED" }
    );
}

// Allow-list a domain
protection.settings_mut().allow_domain("trusted-analytics.com");
let allowed = protection.check_tracking(
    TrackingType::Cookies,
    "trusted-analytics.com",
    "https://trusted-analytics.com/pixel",
);
// Trusted domain bypasses tracking protection

// Clear stats
protection.clear_stats();
assert_eq!(protection.blocked_count(), 0);
```

## Privacy — Domain Allow/Block Lists

```rust
use rust_widgets::web::PrivacySettings;

// Three preset levels:

// 1. Strict — block everything
let strict = PrivacySettings::strict();
assert!(strict.do_not_track);
assert!(strict.block_tracking_cookies);
assert!(strict.block_third_party_cookies);
assert!(strict.clear_cookies_on_exit);

// 2. Balanced — default moderate protection
let balanced = PrivacySettings::balanced();
// Blocks tracking cookies and third-party cookies
// Allows first-party session cookies

// 3. Permissive — minimal blocking
let permissive = PrivacySettings::permissive();
// Allows most cookies, no DNT header

// Custom domain allow/block lists
let mut settings = PrivacySettings::new();
settings.allow_domain("my-trusted-site.com");
assert!(settings.is_domain_allowed("my-trusted-site.com"));

settings.block_domain("known-tracker.net");
assert!(!settings.is_domain_allowed("known-tracker.net"));

// Check specific tracking type
assert!(settings.should_block_tracking_type(TrackingType::Fingerprinting));
```

## Security Settings

```rust
use rust_widgets::web::SecuritySettings;

// Default: secure by default
let security = SecuritySettings::default();
assert!(!security.allow_insecure_content);   // Block HTTP on HTTPS pages
assert!(!security.allow_mixed_content);      // Block mixed HTTP/HTTPS
assert!(security.block_popups);              // Block popups
assert!(security.block_tracking);            // Block tracking
assert!(security.block_malware);             // Block malware

// Customize for a trusted intranet app
let intranet = SecuritySettings {
    allow_insecure_content: true,   // Allow HTTP content
    allow_mixed_content: true,      // Allow mixed content
    block_popups: false,            // Allow popups
    ..SecuritySettings::default()
};
```

Access security settings from web views:

```rust
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.security_mut().block_popups = false;
view.security_mut().allow_insecure_content = true;

let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
engine.security_mut().block_malware = false;
```

## WebSettings

```rust
use rust_widgets::web::WebSettings;

let settings = WebSettings {
    javascript_enabled: true,
    plugins_enabled: false,         // WebView: always false; Engine: configurable
    private_browsing: false,        // WebView: always false; Engine: configurable
    images_enabled: true,
    cookies_enabled: true,
    webgl_enabled: true,
    developer_extras_enabled: false,
    user_agent: "MyApp/1.0 RustWidgets/0.9".to_string(),
    default_encoding: "UTF-8".to_string(),
};

// Apply to view
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.settings_mut().webgl_enabled = false;
view.settings_mut().images_enabled = false;
```

## Plugin System

### The Plugin Trait

```rust
use rust_widgets::web::{
    Plugin, PluginInfo, PluginState, PluginPermission, PluginError,
    PluginManager, ContentPlugin,
};

// Implement the Plugin trait
struct MyPlugin {
    info: PluginInfo,
}

impl Plugin for MyPlugin {
    fn info(&self) -> &PluginInfo { &self.info }
    fn info_mut(&mut self) -> &mut PluginInfo { &mut self.info }
    fn on_load(&mut self) { println!("Plugin loaded: {}", self.info.name); }
    fn on_unload(&mut self) { println!("Plugin unloaded"); }
    fn on_enable(&mut self) { self.info.state = PluginState::Enabled; }
    fn on_disable(&mut self) { self.info.state = PluginState::Disabled; }
    fn handle_message(&mut self, message: &str) -> Option<String> {
        println!("Message received: {}", message);
        None
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}
```

### PluginManager

```rust
use rust_widgets::web::PluginManager;

let mut manager = PluginManager::new();

// Register a plugin
let id = manager.register(Box::new(MyPlugin {
    info: PluginInfo {
        id: 0,  // assigned by manager
        name: "MyPlugin".to_string(),
        version: "1.0.0".to_string(),
        description: "A custom plugin".to_string(),
        author: "Developer".to_string(),
        homepage: None,
        permissions: vec![
            PluginPermission::NetworkAccess,
            PluginPermission::Storage,
        ],
        state: PluginState::Installed,
    },
})).unwrap();

println!("Plugin registered with ID: {}", id);

// Enable the plugin
manager.enable(id).unwrap();

// Check if a plugin has permission
if manager.has_permission(id, PluginPermission::NetworkAccess) {
    println!("Network access granted");
}

// Grant additional permissions
manager.grant_permission(id, PluginPermission::ClipboardAccess).unwrap();
manager.revoke_permission(id, PluginPermission::ClipboardAccess);

// Send a message to a specific plugin
manager.send_message(id, "refresh_data");

// Broadcast to all enabled plugins
manager.broadcast("app_about_to_exit");

// List all plugins
for plugin in manager.list() {
    println!("  {} v{}", plugin.info.name, plugin.info.version);
}

// List only enabled plugins
let enabled = manager.list_enabled();
println!("{} plugins enabled", enabled.len());

// Disable and unregister
manager.disable(id).unwrap();
manager.unregister(id).unwrap();

manager.clear();
```

### ContentPlugin — Built-in Content Handler

```rust
use rust_widgets::web::ContentPlugin;

let mut plugin = ContentPlugin::new("PDF Viewer", "2.0.0");

// Register content type handlers
plugin.register_handler("application/pdf", Box::new(|data: &[u8]| {
    println!("Processing {} bytes of PDF data", data.len());
    // Render PDF content...
}));

plugin.register_handler("application/json", Box::new(|data: &[u8]| {
    println!("Processing JSON data");
}));

// Process content
plugin.process("application/pdf", b"%PDF-1.4...");

// Lifecycle
plugin.on_load();
plugin.on_enable();
```

## Browsing Data Clearing

```rust
use rust_widgets::web::{BrowsingData, WebViewEnhanced};
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.load_url("https://example.com");
assert!(!view.browser_history().is_empty());

// Clear specific data types
view.clear_browsing_data(BrowsingData {
    cookies: false,          // Preserve cookies
    history: true,           // Clear browsing history
    ..Default::default()
});
assert!(view.browser_history().is_empty());

// Clear everything
view.clear_browsing_data(BrowsingData::all());
// All: history, cookies, cache, localStorage, sessionStorage,
//      IndexedDB, WebSQL, service workers, plugin data,
//      downloads, passwords, form data — all set to true

// Clear nothing (metadata query)
let none = BrowsingData::none();
// All fields set to false
```

## 8 Signals for State Observation

Both `WebViewEnhanced` and `WebEngineViewEnhanced` expose signals defined on
`WebViewCore`:

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 1. loading_started — emitted when page load begins
view.base().loading_started.connect(|| {
    println!("Page loading started");
});

// 2. loading_finished — emitted when page load completes
view.base().loading_finished.connect(|| {
    println!("Page loading finished");
});

// 3. loading_progress — emitted with current progress (0–100)
view.base().loading_progress.connect(|progress: Arc<u8>| {
    println!("Loading: {}%", progress);
});

// 4. title_changed — emitted when page title changes
view.base().title_changed.connect(|| {
    println!("Title changed to: {}", view.title());
});

// 5. url_changed — emitted when URL changes (navigation or redirect)
view.base().url_changed.connect(|| {
    println!("URL changed to: {}", view.url());
});

// 6. error_occurred — emitted on load errors
// (private WebViewCore field _error_occurred)

// 7. navigation_state_changed — emitted when back/forward state changes
view.base().navigation_state_changed.connect(|| {
    println!(
        "Nav state: back={}, forward={}",
        view.can_go_back(),
        view.can_go_forward()
    );
});

// 8. console_message — emitted on JavaScript console.log/warn/error
view.base().console_message.connect(|msg: Arc<String>| {
    println!("JS Console: {}", msg);
});

// WebEngineViewEnhanced adds two more:
let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
engine.certificate_error.connect(|domain: Arc<String>| {
    eprintln!("Certificate error for: {}", domain);
});
engine.download_requested.connect(|url: Arc<String>| {
    println!("Download requested: {}", url);
});
```

## The `delegate_widget!` Macro

Both `WebViewEnhanced` and `WebEngineViewEnhanced` use the `delegate_widget!`
macro to implement the `Widget` trait by delegating to the shared `WebViewCore`:

```rust
// Internal implementation (shown for understanding):
//
// delegate_widget!(WebViewEnhanced);
//
// This expands to:
//
// impl Widget for WebViewEnhanced {
//     fn base(&self) -> &BaseWidget { &self.core.base }
//     fn base_mut(&mut self) -> &mut BaseWidget { &mut self.core.base }
//     fn kind(&self) -> WidgetKind { self.core.base.kind() }
//     fn geometry(&self) -> Rect { self.core.base.geometry() }
//     fn set_geometry(&mut self, g: Rect) { self.core.base.set_geometry(g); }
//     // ... all other Widget trait methods delegated to core.base ...
// }
```

## Complete Web Browser Integration

```rust
use rust_widgets::web::{
    WebEngineViewEnhanced, SessionHistory, BrowserHistory,
    CookieJar, PrivacySettings, TrackingProtection,
    SecuritySettings, WebSettings, PluginManager,
};
use rust_widgets::core::Rect;

struct Browser {
    view: WebEngineViewEnhanced,
}

impl Browser {
    fn new() -> Self {
        let mut view = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));

        // Configure for privacy
        view.privacy_mut().settings_mut().block_domain("ad-tracker.com");
        view.security_mut().block_popups = true;
        view.settings_mut().user_agent = "MyBrowser/1.0".to_string();

        // Register a plugin
        view.plugins_mut().register(Box::new(
            ContentPlugin::new("Image Viewer", "1.0")
        )).unwrap();

        // Connect signals
        view.base().url_changed.connect(|| {
            println!("Navigated to: {}", view.url());
        });

        view.base().loading_progress.connect(|p: Arc<u8>| {
            println!("Loading: {}%", p);
        });

        Self { view }
    }

    fn navigate(&mut self, url: &str) {
        let clean_url = if !url.starts_with("http") {
            format!("https://{}", url)
        } else {
            url.to_string()
        };

        // Check tracking before navigation
        let blocked = self.view.privacy().check_tracking(
            TrackingType::Cookies,
            &clean_url,
            &clean_url,
        );

        if !blocked {
            self.view.load_url(&clean_url);
        } else {
            eprintln!("Navigation blocked by privacy settings");
        }
    }

    fn go_back(&mut self) {
        if self.view.can_go_back() {
            self.view.go_back();
        }
    }

    fn go_forward(&mut self) {
        if self.view.can_go_forward() {
            self.view.go_forward();
        }
    }

    fn clear_data(&mut self) {
        use rust_widgets::web::BrowsingData;
        // Clear everything on exit (private browsing)
        self.view.clear_browsing_data(BrowsingData::all());
    }

    fn show_history(&self) {
        for entry in self.view.browser_history().entries() {
            println!(
                "{} — {} (visited {}×)",
                entry.title,
                entry.url,
                entry.visit_count
            );
        }
    }
}
```

## Summary

| Component | Purpose |
|-----------|---------|
| `WebViewEnhanced` | Embedded web content widget (WidgetKind::WebView) |
| `WebEngineViewEnhanced` | Full browser engine widget with extra signals |
| `WebViewCore` | Shared implementation (URL, title, loading state, progress) |
| `SessionHistory` | Back/forward navigation stacks |
| `NavigationHistory` | Timestamped navigation entries with truncation |
| `BrowserHistory` | Persistent visit history with search and ranking |
| `SimpleJsEngine` | Pure-Rust JavaScript interpreter |
| `JsValue` | JavaScript value type (8 variants) |
| `JsContext` | JS context (globals, console messages) |
| `CookieJar` | Cookie storage with domain scoping, expiry |
| `TrackingProtection` | 10 tracking types, domain allow/block |
| `PrivacySettings` | Strict / Balanced / Permissive presets |
| `SecuritySettings` | Mixed content, popups, malware blocking |
| `WebSettings` | JS, plugins, images, WebGL, user agent |
| `PluginManager` | Plugin registration, lifecycle, messaging |
| `ContentPlugin` | Built-in content handler for MIME types |
| `BrowsingData` | Selective data clearing (12 categories) |
| `delegate_widget!` | Macro to delegate Widget trait to WebViewCore |
| 8 signals | State observation via Signal/GenericSignal |
