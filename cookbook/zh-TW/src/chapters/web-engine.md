# Web 引擎

rust-widgets 包含一個完整的嵌入式網頁瀏覽子系統——網頁檢視（WebView）、
Web 引擎（WebEngine）、JavaScript 引擎、Cookie 管理、隱私保護、追蹤
防護、外掛系統，以及瀏覽記錄持久化——全部以純 Rust 實作。

## 架構概覽

```
┌────────────────────────────────────────────────────┐
│  WebViewEnhanced  │  WebEngineViewEnhanced          │  ← 公開 widget API
├────────────────────────────────────────────────────┤
│                WebViewCore (shared)                 │  ← 共用實作
├──────────┬──────────┬──────────┬───────────────────┤
│ Session  │ Browser  │ CookieJar│ TrackingProtection│
│ History  │ History  │          │                   │
├──────────┼──────────┼──────────┼───────────────────┤
│ Plugin   │ SimpleJs │ Privacy  │ SecuritySettings  │
│ Manager  │ Engine   │ Settings │                   │
├──────────┴──────────┴──────────┴───────────────────┤
│  delegate_widget!  │  8 個信號                      │
└────────────────────────────────────────────────────┘
```

**兩種公開 widget 型別共用同一個 `WebViewCore`（約 95% 共用程式碼）：**

| 功能 | `WebViewEnhanced` | `WebEngineViewEnhanced` |
|------|-------------------|------------------------|
| `WidgetKind` | `WebView` | `WebEngineView` |
| 初始 URL | `"about:blank"` | `""`（空字串） |
| 在 about:blank 上呼叫 `reload()` | 無作用 | 完整重新載入 |
| `set_plugins_enabled` | ❌ | ✅ |
| `set_private_browsing` | ❌ | ✅ |
| `certificate_error` 信號 | ❌ | ✅ |
| `download_requested` 信號 | ❌ | ✅ |

## WebView——嵌入式網頁內容 Widget

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

// 建立一個網頁檢視 widget
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 導航至某個 URL
view.load_url("https://example.com");
assert_eq!(view.url(), "https://example.com");

// 直接設定 URL
view.set_url("https://rust-lang.org".to_string());
assert_eq!(view.url(), "https://rust-lang.org");

// 直接載入 HTML
view.load_html("<h1>Hello, World!</h1>", Some("https://myapp.local"));
assert_eq!(view.title(), "HTML Content");
assert_eq!(view.html(), "<h1>Hello, World!</h1>");

// 載入原始資料
view.load_data(b"binary content", "text/plain", "https://data.url");
assert_eq!(view.url(), "https://data.url");
assert_eq!(view.content(), "binary content");

// 頁面元資料
println!("Loading: {}", view.is_loading());
println!("Progress: {}%", view.load_progress());
println!("Title: {}", view.title());
```

**標題與狀態管理：**

```rust
view.set_title("My Custom Page Title".to_string());
assert_eq!(view.title(), "My Custom Page Title");

// 獨特行為：reload() 會跳過 about:blank
let new_view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
assert_eq!(new_view.url(), "about:blank");
new_view.reload();  // 無作用——不會重新載入 about:blank
assert!(!new_view.is_loading());
```

## WebEngine——完整瀏覽器引擎

`WebEngineViewEnhanced` 提供完整的瀏覽器引擎，並附帶額外的信號
與設定：

```rust
use rust_widgets::web::WebEngineViewEnhanced;
use rust_widgets::core::Rect;

let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));

// URL 以空字串開始（不同於 WebViewEnhanced 的 "about:blank"）
assert_eq!(engine.url(), "");

// 導航
engine.load_url("https://example.com");
assert_eq!(engine.url(), "https://example.com");
assert!(!engine.is_loading());  // 在模擬模式中，載入會立即完成
assert_eq!(engine.load_progress(), 100);

engine.load_html("<h1>Hello</h1>", Some("https://base.url"));
assert_eq!(engine.url(), "https://base.url");

engine.load_data(b"binary", "application/octet-stream", "https://data.url");
assert_eq!(engine.title(), "Data: application/octet-stream");

// WebEngineViewEnhanced 獨有的功能：
engine.set_plugins_enabled(true);
assert!(engine.settings().plugins_enabled);

engine.set_private_browsing(true);
assert!(engine.settings().private_browsing);
// 啟用私密瀏覽會自動切換至嚴格隱私設定
```

## 共用 WebViewCore

兩種 widget 型別皆委派給共用的 `WebViewCore`，由其管理：

- URL、標題、載入狀態，以及載入進度
- 工作階段歷史記錄（上一頁／下一頁堆疊）
- 瀏覽記錄（持久化訪問記錄）
- JavaScript 引擎與上下文
- Cookie jar
- 隱私設定／追蹤防護
- 外掛管理員
- 網頁設定與安全設定
- **8 個信號**用於狀態觀察

## 導航——上一頁／下一頁堆疊

### SessionHistory

```rust
use rust_widgets::web::SessionHistory;

let mut history = SessionHistory::new(50);  // 最多 50 筆記錄

// 導航會建立上一頁堆疊
history.navigate("https://page1.com".to_string());
assert_eq!(history.current().unwrap(), "https://page1.com");
assert!(!history.can_go_back());

history.navigate("https://page2.com".to_string());
assert!(history.can_go_back());  // 現在上一頁堆疊中有 page1
assert!(!history.can_go_forward());

// 回到上一頁
let back = history.go_back();
assert_eq!(back.as_deref(), Some("https://page1.com"));
assert!(history.can_go_forward());  // page2 在下一頁堆疊中

// 前往下一頁
let fwd = history.go_forward();
assert_eq!(fwd.as_deref(), Some("https://page2.com"));

// 新的導航會清除下一頁堆疊
history.go_back();
history.navigate("https://page3.com".to_string());
assert!(!history.can_go_forward());  // 下一頁堆疊已被清除

// 檢視堆疊內容
for url in history.back_entries() {
    println!("Back: {}", url);
}
for url in history.forward_entries() {
    println!("Forward: {}", url);
}

history.clear();
```

### NavigationHistory

另一種工作階段歷史記錄實作，附帶時間戳記的記錄：

```rust
use rust_widgets::web::{NavigationHistory, NavigationEntry};

let mut history = NavigationHistory::new(100);

// 推入附帶元資料的記錄
history.push(NavigationEntry {
    url: "https://example.com".to_string(),
    title: "Example Site".to_string(),
    timestamp: 1718000000,
});

let current = history.current().unwrap();
assert_eq!(current.url, "https://example.com");

// 多筆記錄，支援上一頁／下一頁
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

// 在 go_back() 之後推入新記錄會截斷下一頁記錄
history.go_back();
history.push(NavigationEntry {
    url: "https://divergent.com".to_string(),
    title: "Divergent".to_string(),
    timestamp: 1718000005,
});
assert!(!history.can_go_forward());  // 下一頁記錄已被截斷
```

### 透過按鍵事件的導航控制

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 在獲得焦點時都會自動處理鍵盤快捷鍵：

| 按鍵組合 | 動作 |
|---------|------|
| `Alt + Left` | 回到上一頁 |
| `Alt + Right` | 前往下一頁 |
| `F5` 或 `Ctrl + R` | 重新載入 |

## 瀏覽記錄持久化

```rust
use rust_widgets::web::{BrowserHistory, HistoryEntry};

let mut history = BrowserHistory::new();  // 預設：100 筆記錄
// 或：BrowserHistory::with_capacity(500)

// 新增記錄（重複項目不會新增，而是增加 visit_count）
history.add_entry("https://example.com".to_string(), "Example".to_string());
history.add_entry("https://example.com".to_string(), "Example".to_string());
assert_eq!(history.len(), 1);  // 重複——visit_count 現為 2

history.add_entry("https://rust-lang.org".to_string(), "Rust".to_string());
assert_eq!(history.len(), 2);

// 依 URL 或標題搜尋（不分大小寫）
let results = history.search("rust");
assert_eq!(results.len(), 1);
assert_eq!(results[0].url, "https://rust-lang.org");

let results = history.search("example");
assert_eq!(results.len(), 1);  // 不分大小寫比對

// 最多訪問的記錄
let top = history.most_visited(5);  // 依 visit_count 排名前 5

// 最近的記錄
let recent = history.recent(10);  // 依 last_visit 排序最近 10 筆

// 移除特定記錄
assert!(history.remove_entry("https://example.com"));
assert_eq!(history.len(), 1);

// 超出容量時，最舊的記錄會被淘汰
let mut small = BrowserHistory::with_capacity(2);
small.add_entry("https://a.com".to_string(), "A".to_string());
small.add_entry("https://b.com".to_string(), "B".to_string());
small.add_entry("https://c.com".to_string(), "C".to_string());
assert_eq!(small.len(), 2);
assert_eq!(small.entries().front().unwrap().url, "https://b.com");  // A 被淘汰

// 迭代所有記錄
for entry in history.entries() {
    println!("{} (visited {}×)", entry.url, entry.visit_count);
}

history.clear();
assert!(history.is_empty());
```

## JavaScript 引擎

### SimpleJsEngine

一個純 Rust 的 JavaScript 解譯器，支援變數、函式、條件判斷、
迴圈、陣列，以及主控台日誌：

```rust
use rust_widgets::web::{SimpleJsEngine, JsValue, JsResult, JsContext, JsEngine};

let mut engine = SimpleJsEngine::new();
let mut ctx = JsContext::new();

// 求值表達式
let result = engine.evaluate("42", &mut ctx).unwrap();
assert_eq!(result, JsValue::Number(42.0));

// 變數賦值與取值
engine.evaluate("var name = 'Rust';", &mut ctx).unwrap();
let name = engine.evaluate("name", &mut ctx).unwrap();
assert_eq!(name, JsValue::String("Rust".to_string()));

// 算術運算
let calc = engine.evaluate("10 + 5 * 3", &mut ctx).unwrap();
// SimpleJsEngine 會直接求值字面表達式

// 字串串接
let greeting = engine.evaluate("'Hello, ' + 'World!'", &mut ctx).unwrap();

// 布林表達式
let bool_val = engine.evaluate("true", &mut ctx).unwrap();
assert_eq!(bool_val, JsValue::Boolean(true));

// 函式定義
engine.evaluate("function add(a, b) { return a + b; }", &mut ctx).unwrap();
// 函式會儲存供後續呼叫使用

// 主控台日誌
engine.evaluate("console.log('Debug message');", &mut ctx).unwrap();

// 讀取主控台輸出
for msg in ctx.console_messages() {
    println!("[{}] {} (line {})", msg.level, msg.message, msg.line);
}
```

### JsValue

JavaScript 值型別支援多種變體：

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

// 轉換方法
assert_eq!(JsValue::Number(42.0).to_string(), "42");
assert_eq!(JsValue::Boolean(true).to_boolean(), true);
assert!(JsValue::Number(42.0).is_truthy());
assert!(!JsValue::Boolean(false).is_truthy());
assert!(!JsValue::Null.is_truthy());
assert!(!JsValue::Undefined.is_truthy());
```

### 與 WebView 整合

```rust
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 在已載入頁面的上下文中執行 JavaScript
view.load_html("<div id='app'></div>", None);

let result = view.evaluate_javascript("42").unwrap();
assert_eq!(result, JsValue::Number(42.0));

// 變數賦值
let _ = view.evaluate_javascript("var x = 10;");

// 停用 JavaScript
view.set_javascript_enabled(false);
let result = view.evaluate_javascript("1 + 1");
assert!(result.is_err());
assert!(result.unwrap_err().message.contains("JavaScript is disabled"));

// 重新啟用
view.set_javascript_enabled(true);
```

## Cookie——CookieJar

```rust
use rust_widgets::web::{CookieJar, Cookie, SameSite};

let mut jar = CookieJar::new();

// 建立並新增一個 cookie
let cookie = Cookie::new(
    "session_id".to_string(),
    "abc123def456".to_string(),
    "example.com".to_string(),
);
assert!(!cookie.is_expired());  // 無過期時間 = 工作階段 cookie

jar.add(cookie);
assert_eq!(jar.len(), 1);

// 依名稱取得
let session = jar.get("session_id", "example.com");
assert!(session.is_some());

// 依網域範圍的 cookie
jar.add(Cookie::new(
    "theme".to_string(),
    "dark".to_string(),
    "sub.example.com".to_string(),
));

// 取得特定網域的 cookie
let domain_cookies = jar.cookies_for_domain("example.com");
println!("{} cookies for example.com", domain_cookies.len());

// 第三方 cookie 偵測
let tp_cookie = Cookie::new(
    "tracker".to_string(),
    "data".to_string(),
    "ad-network.com".to_string(),
);
assert!(tp_cookie.is_third_party("mysite.com"));

// 清除已過期的 cookie
jar.clear_expired();

// 清除特定網域的 cookie
jar.clear_for_domain("sub.example.com");

// 列出所有 cookie
for cookie in jar.all_cookies() {
    println!("{}={} (domain: {})", cookie.name, cookie.value, cookie.domain);
}

jar.clear();
assert!(jar.is_empty());
```

## 追蹤防護

### TrackingType——10 種追蹤機制

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
// 從嚴格隱私設定開始
let mut protection = TrackingProtection::new(PrivacySettings::strict());

// 檢查是否應阻擋追蹤
let blocked = protection.check_tracking(
    TrackingType::Fingerprinting,
    "tracker.com",
    "https://tracker.com/beacon",
);
assert!(blocked);  // 在嚴格模式下，指紋辨識會被阻擋

// 被記錄的嘗試次數
println!("Blocked: {}", protection.blocked_count());

for attempt in protection.attempts() {
    println!(
        "{:?} from {} — {}",
        attempt.tracking_type,
        attempt.domain,
        if attempt.blocked { "BLOCKED" } else { "ALLOWED" }
    );
}

// 將網域加入允許清單
protection.settings_mut().allow_domain("trusted-analytics.com");
let allowed = protection.check_tracking(
    TrackingType::Cookies,
    "trusted-analytics.com",
    "https://trusted-analytics.com/pixel",
);
// 受信任的網域可繞過追蹤防護

// 清除統計資料
protection.clear_stats();
assert_eq!(protection.blocked_count(), 0);
```

## 隱私——網域允許／封鎖清單

```rust
use rust_widgets::web::PrivacySettings;

// 三種預設等級：

// 1. 嚴格——封鎖所有項目
let strict = PrivacySettings::strict();
assert!(strict.do_not_track);
assert!(strict.block_tracking_cookies);
assert!(strict.block_third_party_cookies);
assert!(strict.clear_cookies_on_exit);

// 2. 平衡——預設中度保護
let balanced = PrivacySettings::balanced();
// 封鎖追蹤 cookie 和第三方 cookie
// 允許第一方工作階段 cookie

// 3. 寬鬆——最小程度封鎖
let permissive = PrivacySettings::permissive();
// 允許大多數 cookie，不發送 DNT 標頭

// 自訂網域允許／封鎖清單
let mut settings = PrivacySettings::new();
settings.allow_domain("my-trusted-site.com");
assert!(settings.is_domain_allowed("my-trusted-site.com"));

settings.block_domain("known-tracker.net");
assert!(!settings.is_domain_allowed("known-tracker.net"));

// 檢查特定追蹤類型
assert!(settings.should_block_tracking_type(TrackingType::Fingerprinting));
```

## 安全設定

```rust
use rust_widgets::web::SecuritySettings;

// 預設：預設為安全
let security = SecuritySettings::default();
assert!(!security.allow_insecure_content);   // 在 HTTPS 頁面上封鎖 HTTP
assert!(!security.allow_mixed_content);      // 封鎖混合 HTTP/HTTPS
assert!(security.block_popups);              // 封鎖彈出視窗
assert!(security.block_tracking);            // 封鎖追蹤
assert!(security.block_malware);             // 封鎖惡意軟體

// 為受信任的內部網路應用程式自訂
let intranet = SecuritySettings {
    allow_insecure_content: true,   // 允許 HTTP 內容
    allow_mixed_content: true,      // 允許混合內容
    block_popups: false,            // 允許彈出視窗
    ..SecuritySettings::default()
};
```

從網頁檢視存取安全設定：

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
    plugins_enabled: false,         // WebView：永遠為 false；Engine：可設定
    private_browsing: false,        // WebView：永遠為 false；Engine：可設定
    images_enabled: true,
    cookies_enabled: true,
    webgl_enabled: true,
    developer_extras_enabled: false,
    user_agent: "MyApp/1.0 RustWidgets/0.9".to_string(),
    default_encoding: "UTF-8".to_string(),
};

// 套用至檢視
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.settings_mut().webgl_enabled = false;
view.settings_mut().images_enabled = false;
```

## 外掛系統

### Plugin 特徵

```rust
use rust_widgets::web::{
    Plugin, PluginInfo, PluginState, PluginPermission, PluginError,
    PluginManager, ContentPlugin,
};

// 實作 Plugin 特徵
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

// 註冊一個外掛
let id = manager.register(Box::new(MyPlugin {
    info: PluginInfo {
        id: 0,  // 由 manager 指派
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

// 啟用外掛
manager.enable(id).unwrap();

// 檢查外掛是否擁有特定權限
if manager.has_permission(id, PluginPermission::NetworkAccess) {
    println!("Network access granted");
}

// 授予額外權限
manager.grant_permission(id, PluginPermission::ClipboardAccess).unwrap();
manager.revoke_permission(id, PluginPermission::ClipboardAccess);

// 傳送訊息至特定外掛
manager.send_message(id, "refresh_data");

// 廣播至所有已啟用的外掛
manager.broadcast("app_about_to_exit");

// 列出所有外掛
for plugin in manager.list() {
    println!("  {} v{}", plugin.info.name, plugin.info.version);
}

// 僅列出已啟用的外掛
let enabled = manager.list_enabled();
println!("{} plugins enabled", enabled.len());

// 停用並取消註冊
manager.disable(id).unwrap();
manager.unregister(id).unwrap();

manager.clear();
```

### ContentPlugin——內建內容處理器

```rust
use rust_widgets::web::ContentPlugin;

let mut plugin = ContentPlugin::new("PDF Viewer", "2.0.0");

// 註冊內容類型處理器
plugin.register_handler("application/pdf", Box::new(|data: &[u8]| {
    println!("Processing {} bytes of PDF data", data.len());
    // 渲染 PDF 內容...
}));

plugin.register_handler("application/json", Box::new(|data: &[u8]| {
    println!("Processing JSON data");
}));

// 處理內容
plugin.process("application/pdf", b"%PDF-1.4...");

// 生命週期
plugin.on_load();
plugin.on_enable();
```

## 瀏覽資料清除

```rust
use rust_widgets::web::{BrowsingData, WebViewEnhanced};
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.load_url("https://example.com");
assert!(!view.browser_history().is_empty());

// 清除特定資料類型
view.clear_browsing_data(BrowsingData {
    cookies: false,          // 保留 cookie
    history: true,           // 清除瀏覽記錄
    ..Default::default()
});
assert!(view.browser_history().is_empty());

// 清除所有資料
view.clear_browsing_data(BrowsingData::all());
// 所有項目：history, cookies, cache, localStorage, sessionStorage,
//           IndexedDB, WebSQL, service workers, plugin data,
//           downloads, passwords, form data——全部設為 true

// 不清除任何資料（元資料查詢）
let none = BrowsingData::none();
// 全部欄位設為 false
```

## 8 個用於狀態觀察的信號

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 都公開了定義在
`WebViewCore` 上的信號：

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 1. loading_started——頁面開始載入時觸發
view.base().loading_started.connect(|| {
    println!("Page loading started");
});

// 2. loading_finished——頁面載入完成時觸發
view.base().loading_finished.connect(|| {
    println!("Page loading finished");
});

// 3. loading_progress——傳送當前進度（0–100）時觸發
view.base().loading_progress.connect(|progress: Arc<u8>| {
    println!("Loading: {}%", progress);
});

// 4. title_changed——頁面標題變更時觸發
view.base().title_changed.connect(|| {
    println!("Title changed to: {}", view.title());
});

// 5. url_changed——URL 變更時觸發（導航或重新導向）
view.base().url_changed.connect(|| {
    println!("URL changed to: {}", view.url());
});

// 6. error_occurred——載入錯誤時觸發
// （私有的 WebViewCore 欄位 _error_occurred）

// 7. navigation_state_changed——上一頁／下一頁狀態變更時觸發
view.base().navigation_state_changed.connect(|| {
    println!(
        "Nav state: back={}, forward={}",
        view.can_go_back(),
        view.can_go_forward()
    );
});

// 8. console_message——JavaScript console.log/warn/error 時觸發
view.base().console_message.connect(|msg: Arc<String>| {
    println!("JS Console: {}", msg);
});

// WebEngineViewEnhanced 額外增加兩個信號：
let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
engine.certificate_error.connect(|domain: Arc<String>| {
    eprintln!("Certificate error for: {}", domain);
});
engine.download_requested.connect(|url: Arc<String>| {
    println!("Download requested: {}", url);
});
```

## `delegate_widget!` 巨集

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 都使用 `delegate_widget!`
巨集來實作 `Widget` 特徵，透過委派給共用的 `WebViewCore`：

```rust
// 內部實作（僅供理解參考）：
//
// delegate_widget!(WebViewEnhanced);
//
// 展開後相當於：
//
// impl Widget for WebViewEnhanced {
//     fn base(&self) -> &BaseWidget { &self.core.base }
//     fn base_mut(&mut self) -> &mut BaseWidget { &mut self.core.base }
//     fn kind(&self) -> WidgetKind { self.core.base.kind() }
//     fn geometry(&self) -> Rect { self.core.base.geometry() }
//     fn set_geometry(&mut self, g: Rect) { self.core.base.set_geometry(g); }
//     // ... 所有其他 Widget 特徵方法皆委派給 core.base ...
// }
```

## 完整網頁瀏覽器整合

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

        // 設定隱私保護
        view.privacy_mut().settings_mut().block_domain("ad-tracker.com");
        view.security_mut().block_popups = true;
        view.settings_mut().user_agent = "MyBrowser/1.0".to_string();

        // 註冊外掛
        view.plugins_mut().register(Box::new(
            ContentPlugin::new("Image Viewer", "1.0")
        )).unwrap();

        // 連接信號
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

        // 導航前檢查追蹤
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
        // 退出時清除所有資料（私密瀏覽）
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

## 總結

| 元件 | 用途 |
|------|------|
| `WebViewEnhanced` | 嵌入式網頁內容 widget（WidgetKind::WebView） |
| `WebEngineViewEnhanced` | 完整瀏覽器引擎 widget，附帶額外信號 |
| `WebViewCore` | 共用實作（URL、標題、載入狀態、進度） |
| `SessionHistory` | 上一頁／下一頁導航堆疊 |
| `NavigationHistory` | 附帶時間戳記的導航記錄，支援截斷 |
| `BrowserHistory` | 持久化訪問記錄，支援搜尋與排名 |
| `SimpleJsEngine` | 純 Rust JavaScript 解譯器 |
| `JsValue` | JavaScript 值型別（8 種變體） |
| `JsContext` | JS 上下文（全域變數、主控台訊息） |
| `CookieJar` | Cookie 儲存，支援網域範圍與過期 |
| `TrackingProtection` | 10 種追蹤類型，網域允許／封鎖 |
| `PrivacySettings` | 嚴格／平衡／寬鬆 三種預設等級 |
| `SecuritySettings` | 混合內容、彈出視窗、惡意軟體封鎖 |
| `WebSettings` | JS、外掛、圖片、WebGL、使用者代理 |
| `PluginManager` | 外掛註冊、生命週期、訊息傳遞 |
| `ContentPlugin` | 內建 MIME 類型內容處理器 |
| `BrowsingData` | 選擇性資料清除（12 種類別） |
| `delegate_widget!` | 將 Widget 特徵委派給 WebViewCore 的巨集 |
| 8 個信號 | 透過 Signal/GenericSignal 進行狀態觀察 |
