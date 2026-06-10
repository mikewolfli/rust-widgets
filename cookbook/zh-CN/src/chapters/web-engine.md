# Web引擎

rust-widgets 包含一个完整的嵌入式网页浏览子系统——网页视图(WebView)、
Web引擎、JavaScript引擎、Cookie管理、隐私控制、跟踪
保护、插件系统和浏览器历史持久化——全部用纯 Rust 实现。

## 架构概览

```
┌────────────────────────────────────────────────────┐
│  WebViewEnhanced  │  WebEngineViewEnhanced          │  ← 公共 widget API
├────────────────────────────────────────────────────┤
│                WebViewCore (共享)                    │  ← 通用实现
├──────────┬──────────┬──────────┬───────────────────┤
│ Session  │ Browser  │ CookieJar│ TrackingProtection│
│ History  │ History  │          │                   │
├──────────┼──────────┼──────────┼───────────────────┤
│ Plugin   │ SimpleJs │ Privacy  │ SecuritySettings  │
│ Manager  │ Engine   │ Settings │                   │
├──────────┴──────────┴──────────┴───────────────────┤
│  delegate_widget!  │  8 个信号                     │
└────────────────────────────────────────────────────┘
```

**两种公共 widget 类型共享同一个 `WebViewCore`（约 95% 代码共享）：**

| 特性 | `WebViewEnhanced` | `WebEngineViewEnhanced` |
|---------|-------------------|------------------------|
| `WidgetKind` | `WebView` | `WebEngineView` |
| 初始 URL | `"about:blank"` | `""`（空字符串） |
| 在 about:blank 上调用 `reload()` | 无操作 | 完全重新加载 |
| `set_plugins_enabled` | ❌ | ✅ |
| `set_private_browsing` | ❌ | ✅ |
| `certificate_error` 信号 | ❌ | ✅ |
| `download_requested` 信号 | ❌ | ✅ |

## 网页视图(WebView) — 嵌入式网页内容 Widget

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

// 创建一个网页视图 widget
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 导航到 URL
view.load_url("https://example.com");
assert_eq!(view.url(), "https://example.com");

// 直接设置 URL
view.set_url("https://rust-lang.org".to_string());
assert_eq!(view.url(), "https://rust-lang.org");

// 直接加载 HTML
view.load_html("<h1>Hello, World!</h1>", Some("https://myapp.local"));
assert_eq!(view.title(), "HTML Content");
assert_eq!(view.html(), "<h1>Hello, World!</h1>");

// 加载原始数据
view.load_data(b"binary content", "text/plain", "https://data.url");
assert_eq!(view.url(), "https://data.url");
assert_eq!(view.content(), "binary content");

// 页面元数据
println!("Loading: {}", view.is_loading());
println!("Progress: {}%", view.load_progress());
println!("Title: {}", view.title());
```

**标题与状态管理：**

```rust
view.set_title("My Custom Page Title".to_string());
assert_eq!(view.title(), "My Custom Page Title");

// 特殊行为：reload() 跳过 about:blank
let new_view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
assert_eq!(new_view.url(), "about:blank");
new_view.reload();  // 无操作——不会重新加载 about:blank
assert!(!new_view.is_loading());
```

## Web引擎 — 完整浏览器引擎

`WebEngineViewEnhanced` 提供完整的浏览器引擎，包含额外的信号
和设置：

```rust
use rust_widgets::web::WebEngineViewEnhanced;
use rust_widgets::core::Rect;

let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 1024, 768));

// URL 初始为空（不同于 WebViewEnhanced 的 "about:blank"）
assert_eq!(engine.url(), "");

// 导航
engine.load_url("https://example.com");
assert_eq!(engine.url(), "https://example.com");
assert!(!engine.is_loading());  // 在模拟模式下，加载立即完成
assert_eq!(engine.load_progress(), 100);

engine.load_html("<h1>Hello</h1>", Some("https://base.url"));
assert_eq!(engine.url(), "https://base.url");

engine.load_data(b"binary", "application/octet-stream", "https://data.url");
assert_eq!(engine.title(), "Data: application/octet-stream");

// WebEngineViewEnhanced 独有的功能：
engine.set_plugins_enabled(true);
assert!(engine.settings().plugins_enabled);

engine.set_private_browsing(true);
assert!(engine.settings().private_browsing);
// 启用隐私浏览会自动切换到严格隐私设置
```

## 共享的 WebViewCore

两种 widget 类型都委托给共享的 `WebViewCore`，它管理：

- URL、标题、加载状态和加载进度
- 会话历史（后退/前进栈）
- 浏览器历史（持久化访问记录）
- JavaScript 引擎和上下文
- Cookie 容器 (Cookie jar)
- 隐私 / 跟踪保护
- 插件管理器
- Web 设置和安全设置
- **8 个信号**用于状态观察

## 导航 — 后退/前进栈

### SessionHistory（会话历史）

```rust
use rust_widgets::web::SessionHistory;

let mut history = SessionHistory::new(50);  // 最多 50 条记录

// 导航会构建后退栈
history.navigate("https://page1.com".to_string());
assert_eq!(history.current().unwrap(), "https://page1.com");
assert!(!history.can_go_back());

history.navigate("https://page2.com".to_string());
assert!(history.can_go_back());  // 后退栈中有 page1
assert!(!history.can_go_forward());

// 后退
let back = history.go_back();
assert_eq!(back.as_deref(), Some("https://page1.com"));
assert!(history.can_go_forward());  // 前进栈中有 page2

// 前进
let fwd = history.go_forward();
assert_eq!(fwd.as_deref(), Some("https://page2.com"));

// 新导航会清空前进栈
history.go_back();
history.navigate("https://page3.com".to_string());
assert!(!history.can_go_forward());  // 前进栈已被清空

// 查看栈内容
for url in history.back_entries() {
    println!("Back: {}", url);
}
for url in history.forward_entries() {
    println!("Forward: {}", url);
}

history.clear();
```

### NavigationHistory（带时间戳的导航历史）

另一种带时间戳的会话历史实现：

```rust
use rust_widgets::web::{NavigationHistory, NavigationEntry};

let mut history = NavigationHistory::new(100);

// 添加带元数据的记录
history.push(NavigationEntry {
    url: "https://example.com".to_string(),
    title: "Example Site".to_string(),
    timestamp: 1718000000,
});

let current = history.current().unwrap();
assert_eq!(current.url, "https://example.com");

// 多条记录，支持后退/前进
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

// 在 go_back() 之后添加新记录会截断前进记录
history.go_back();
history.push(NavigationEntry {
    url: "https://divergent.com".to_string(),
    title: "Divergent".to_string(),
    timestamp: 1718000005,
});
assert!(!history.can_go_forward());  // 前进记录已被截断
```

### 通过键盘事件的导航控制

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 在获得焦点时都会自动处理键盘快捷键：

| 按键组合 | 操作 |
|----------------|--------|
| `Alt + Left` | 后退 |
| `Alt + Right` | 前进 |
| `F5` 或 `Ctrl + R` | 重新加载 |

## 浏览器历史持久化

```rust
use rust_widgets::web::{BrowserHistory, HistoryEntry};

let mut history = BrowserHistory::new();  // 默认：100 条记录
// 或者：BrowserHistory::with_capacity(500)

// 添加记录（重复项会递增 visit_count 而不是新加）
history.add_entry("https://example.com".to_string(), "Example".to_string());
history.add_entry("https://example.com".to_string(), "Example".to_string());
assert_eq!(history.len(), 1);  // 重复——visit_count 现在为 2

history.add_entry("https://rust-lang.org".to_string(), "Rust".to_string());
assert_eq!(history.len(), 2);

// 按 URL 或标题搜索（不区分大小写）
let results = history.search("rust");
assert_eq!(results.len(), 1);
assert_eq!(results[0].url, "https://rust-lang.org");

let results = history.search("example");
assert_eq!(results.len(), 1);  // 不区分大小写的匹配

// 访问最多的记录
let top = history.most_visited(5);  // 按 visit_count 排名前 5

// 最近的记录
let recent = history.recent(10);  // 按 last_visit 最近 10 条

// 删除指定记录
assert!(history.remove_entry("https://example.com"));
assert_eq!(history.len(), 1);

// 超过容量时会淘汰最旧的记录
let mut small = BrowserHistory::with_capacity(2);
small.add_entry("https://a.com".to_string(), "A".to_string());
small.add_entry("https://b.com".to_string(), "B".to_string());
small.add_entry("https://c.com".to_string(), "C".to_string());
assert_eq!(small.len(), 2);
assert_eq!(small.entries().front().unwrap().url, "https://b.com");  // A 已被淘汰

// 遍历记录
for entry in history.entries() {
    println!("{} (visited {}×)", entry.url, entry.visit_count);
}

history.clear();
assert!(history.is_empty());
```

## JavaScript引擎

### SimpleJsEngine（简单 JavaScript 引擎）

一个纯 Rust 的 JavaScript 解释器，支持变量、函数、条件
语句、循环、数组和控制台日志：

```rust
use rust_widgets::web::{SimpleJsEngine, JsValue, JsResult, JsContext, JsEngine};

let mut engine = SimpleJsEngine::new();
let mut ctx = JsContext::new();

// 计算表达式
let result = engine.evaluate("42", &mut ctx).unwrap();
assert_eq!(result, JsValue::Number(42.0));

// 变量赋值和获取
engine.evaluate("var name = 'Rust';", &mut ctx).unwrap();
let name = engine.evaluate("name", &mut ctx).unwrap();
assert_eq!(name, JsValue::String("Rust".to_string()));

// 算术运算
let calc = engine.evaluate("10 + 5 * 3", &mut ctx).unwrap();
// SimpleJsEngine 直接计算字面量表达式

// 字符串拼接
let greeting = engine.evaluate("'Hello, ' + 'World!'", &mut ctx).unwrap();

// 布尔表达式
let bool_val = engine.evaluate("true", &mut ctx).unwrap();
assert_eq!(bool_val, JsValue::Boolean(true));

// 函数定义
engine.evaluate("function add(a, b) { return a + b; }", &mut ctx).unwrap();
// 函数被保存以供后续调用

// 控制台日志
engine.evaluate("console.log('Debug message');", &mut ctx).unwrap();

// 读取控制台输出
for msg in ctx.console_messages() {
    println!("[{}] {} (line {})", msg.level, msg.message, msg.line);
}
```

### JsValue（JavaScript 值类型）

JavaScript 值类型支持多种变体：

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

// 转换方法
assert_eq!(JsValue::Number(42.0).to_string(), "42");
assert_eq!(JsValue::Boolean(true).to_boolean(), true);
assert!(JsValue::Number(42.0).is_truthy());
assert!(!JsValue::Boolean(false).is_truthy());
assert!(!JsValue::Null.is_truthy());
assert!(!JsValue::Undefined.is_truthy());
```

### 与 WebView 集成

```rust
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 在已加载页面的上下文中执行 JavaScript
view.load_html("<div id='app'></div>", None);

let result = view.evaluate_javascript("42").unwrap();
assert_eq!(result, JsValue::Number(42.0));

// 变量赋值
let _ = view.evaluate_javascript("var x = 10;");

// 禁用 JavaScript
view.set_javascript_enabled(false);
let result = view.evaluate_javascript("1 + 1");
assert!(result.is_err());
assert!(result.unwrap_err().message.contains("JavaScript is disabled"));

// 重新启用
view.set_javascript_enabled(true);
```

## Cookie — CookieJar（Cookie 容器）

```rust
use rust_widgets::web::{CookieJar, Cookie, SameSite};

let mut jar = CookieJar::new();

// 创建并添加 Cookie
let cookie = Cookie::new(
    "session_id".to_string(),
    "abc123def456".to_string(),
    "example.com".to_string(),
);
assert!(!cookie.is_expired());  // 无过期时间 = 会话 Cookie

jar.add(cookie);
assert_eq!(jar.len(), 1);

// 按名称获取
let session = jar.get("session_id", "example.com");
assert!(session.is_some());

// 域名范围内的 Cookie
jar.add(Cookie::new(
    "theme".to_string(),
    "dark".to_string(),
    "sub.example.com".to_string(),
));

// 获取特定域名的 Cookie
let domain_cookies = jar.cookies_for_domain("example.com");
println!("{} cookies for example.com", domain_cookies.len());

// 第三方 Cookie 检测
let tp_cookie = Cookie::new(
    "tracker".to_string(),
    "data".to_string(),
    "ad-network.com".to_string(),
);
assert!(tp_cookie.is_third_party("mysite.com"));

// 清除已过期的 Cookie
jar.clear_expired();

// 清除特定域名的 Cookie
jar.clear_for_domain("sub.example.com");

// 列出所有 Cookie
for cookie in jar.all_cookies() {
    println!("{}={} (domain: {})", cookie.name, cookie.value, cookie.domain);
}

jar.clear();
assert!(jar.is_empty());
```

## 跟踪保护

### TrackingType — 10 种跟踪机制

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

### TrackingProtection（跟踪保护）

```rust
// 从严格隐私设置开始
let mut protection = TrackingProtection::new(PrivacySettings::strict());

// 检查是否应该阻止跟踪
let blocked = protection.check_tracking(
    TrackingType::Fingerprinting,
    "tracker.com",
    "https://tracker.com/beacon",
);
assert!(blocked);  // 严格模式下指纹识别被阻止

// 被阻止的尝试会被记录
println!("Blocked: {}", protection.blocked_count());

for attempt in protection.attempts() {
    println!(
        "{:?} from {} — {}",
        attempt.tracking_type,
        attempt.domain,
        if attempt.blocked { "BLOCKED" } else { "ALLOWED" }
    );
}

// 将域名加入允许列表
protection.settings_mut().allow_domain("trusted-analytics.com");
let allowed = protection.check_tracking(
    TrackingType::Cookies,
    "trusted-analytics.com",
    "https://trusted-analytics.com/pixel",
);
// 受信任的域名绕过跟踪保护

// 清空统计
protection.clear_stats();
assert_eq!(protection.blocked_count(), 0);
```

## 隐私 — 域名允许/阻止列表

```rust
use rust_widgets::web::PrivacySettings;

// 三种预设级别：

// 1. 严格——阻止一切
let strict = PrivacySettings::strict();
assert!(strict.do_not_track);
assert!(strict.block_tracking_cookies);
assert!(strict.block_third_party_cookies);
assert!(strict.clear_cookies_on_exit);

// 2. 平衡——默认的中等保护
let balanced = PrivacySettings::balanced();
// 阻止跟踪 Cookie 和第三方 Cookie
// 允许第一方会话 Cookie

// 3. 宽松——最小化阻止
let permissive = PrivacySettings::permissive();
// 允许大多数 Cookie，不发送 DNT 请求头

// 自定义域名允许/阻止列表
let mut settings = PrivacySettings::new();
settings.allow_domain("my-trusted-site.com");
assert!(settings.is_domain_allowed("my-trusted-site.com"));

settings.block_domain("known-tracker.net");
assert!(!settings.is_domain_allowed("known-tracker.net"));

// 检查特定的跟踪类型
assert!(settings.should_block_tracking_type(TrackingType::Fingerprinting));
```

## 安全设置

```rust
use rust_widgets::web::SecuritySettings;

// 默认：安全优先
let security = SecuritySettings::default();
assert!(!security.allow_insecure_content);   // 在 HTTPS 页面上阻止 HTTP
assert!(!security.allow_mixed_content);      // 阻止混合 HTTP/HTTPS
assert!(security.block_popups);              // 阻止弹窗
assert!(security.block_tracking);            // 阻止跟踪
assert!(security.block_malware);             // 阻止恶意软件

// 为可信的内网应用自定义
let intranet = SecuritySettings {
    allow_insecure_content: true,   // 允许 HTTP 内容
    allow_mixed_content: true,      // 允许混合内容
    block_popups: false,            // 允许弹窗
    ..SecuritySettings::default()
};
```

从网页视图访问安全设置：

```rust
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.security_mut().block_popups = false;
view.security_mut().allow_insecure_content = true;

let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
engine.security_mut().block_malware = false;
```

## WebSettings（Web 设置）

```rust
use rust_widgets::web::WebSettings;

let settings = WebSettings {
    javascript_enabled: true,
    plugins_enabled: false,         // WebView：始终为 false；Engine：可配置
    private_browsing: false,        // WebView：始终为 false；Engine：可配置
    images_enabled: true,
    cookies_enabled: true,
    webgl_enabled: true,
    developer_extras_enabled: false,
    user_agent: "MyApp/1.0 RustWidgets/0.9".to_string(),
    default_encoding: "UTF-8".to_string(),
};

// 应用到视图
let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.settings_mut().webgl_enabled = false;
view.settings_mut().images_enabled = false;
```

## 插件系统

### Plugin Trait（插件特质）

```rust
use rust_widgets::web::{
    Plugin, PluginInfo, PluginState, PluginPermission, PluginError,
    PluginManager, ContentPlugin,
};

// 实现 Plugin trait
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

### PluginManager（插件管理器）

```rust
use rust_widgets::web::PluginManager;

let mut manager = PluginManager::new();

// 注册一个插件
let id = manager.register(Box::new(MyPlugin {
    info: PluginInfo {
        id: 0,  // 由管理器分配
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

// 启用插件
manager.enable(id).unwrap();

// 检查插件是否有权限
if manager.has_permission(id, PluginPermission::NetworkAccess) {
    println!("Network access granted");
}

// 授予额外权限
manager.grant_permission(id, PluginPermission::ClipboardAccess).unwrap();
manager.revoke_permission(id, PluginPermission::ClipboardAccess);

// 向指定插件发送消息
manager.send_message(id, "refresh_data");

// 广播给所有已启用的插件
manager.broadcast("app_about_to_exit");

// 列出所有插件
for plugin in manager.list() {
    println!("  {} v{}", plugin.info.name, plugin.info.version);
}

// 仅列出已启用的插件
let enabled = manager.list_enabled();
println!("{} plugins enabled", enabled.len());

// 禁用和注销
manager.disable(id).unwrap();
manager.unregister(id).unwrap();

manager.clear();
```

### ContentPlugin — 内置内容处理器

```rust
use rust_widgets::web::ContentPlugin;

let mut plugin = ContentPlugin::new("PDF Viewer", "2.0.0");

// 注册内容类型处理器
plugin.register_handler("application/pdf", Box::new(|data: &[u8]| {
    println!("Processing {} bytes of PDF data", data.len());
    // 渲染 PDF 内容...
}));

plugin.register_handler("application/json", Box::new(|data: &[u8]| {
    println!("Processing JSON data");
}));

// 处理内容
plugin.process("application/pdf", b"%PDF-1.4...");

// 生命周期
plugin.on_load();
plugin.on_enable();
```

## 浏览数据清理

```rust
use rust_widgets::web::{BrowsingData, WebViewEnhanced};
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));
view.load_url("https://example.com");
assert!(!view.browser_history().is_empty());

// 清理指定的数据类型
view.clear_browsing_data(BrowsingData {
    cookies: false,          // 保留 Cookie
    history: true,           // 清理浏览历史
    ..Default::default()
});
assert!(view.browser_history().is_empty());

// 清理所有数据
view.clear_browsing_data(BrowsingData::all());
// 全部：历史、Cookie、缓存、localStorage、sessionStorage、
//       IndexedDB、WebSQL、Service Worker、插件数据、
//       下载记录、密码、表单数据——全部设为 true

// 不清理任何数据（元数据查询）
let none = BrowsingData::none();
// 所有字段设为 false
```

## 8 个状态观察信号

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 都暴露在 `WebViewCore` 上定义的信号：

```rust
use rust_widgets::web::WebViewEnhanced;
use rust_widgets::core::Rect;

let mut view = WebViewEnhanced::new(Rect::new(0, 0, 800, 600));

// 1. loading_started — 页面开始加载时触发
view.base().loading_started.connect(|| {
    println!("Page loading started");
});

// 2. loading_finished — 页面加载完成时触发
view.base().loading_finished.connect(|| {
    println!("Page loading finished");
});

// 3. loading_progress — 触发当前加载进度（0–100）
view.base().loading_progress.connect(|progress: Arc<u8>| {
    println!("Loading: {}%", progress);
});

// 4. title_changed — 页面标题变化时触发
view.base().title_changed.connect(|| {
    println!("Title changed to: {}", view.title());
});

// 5. url_changed — URL 变化时触发（导航或重定向）
view.base().url_changed.connect(|| {
    println!("URL changed to: {}", view.url());
});

// 6. error_occurred — 加载错误时触发
// (WebViewCore 私有字段 _error_occurred)

// 7. navigation_state_changed — 后退/前进状态变化时触发
view.base().navigation_state_changed.connect(|| {
    println!(
        "Nav state: back={}, forward={}",
        view.can_go_back(),
        view.can_go_forward()
    );
});

// 8. console_message — JavaScript console.log/warn/error 时触发
view.base().console_message.connect(|msg: Arc<String>| {
    println!("JS Console: {}", msg);
});

// WebEngineViewEnhanced 额外还有两个信号：
let mut engine = WebEngineViewEnhanced::new(Rect::new(0, 0, 800, 600));
engine.certificate_error.connect(|domain: Arc<String>| {
    eprintln!("Certificate error for: {}", domain);
});
engine.download_requested.connect(|url: Arc<String>| {
    println!("Download requested: {}", url);
});
```

## `delegate_widget!` 宏

`WebViewEnhanced` 和 `WebEngineViewEnhanced` 都使用 `delegate_widget!`
宏来实现 `Widget` trait，通过委托到共享的 `WebViewCore`：

```rust
// 内部实现（展示供理解）：
//
// delegate_widget!(WebViewEnhanced);
//
// 展开后等价于：
//
// impl Widget for WebViewEnhanced {
//     fn base(&self) -> &BaseWidget { &self.core.base }
//     fn base_mut(&mut self) -> &mut BaseWidget { &mut self.core.base }
//     fn kind(&self) -> WidgetKind { self.core.base.kind() }
//     fn geometry(&self) -> Rect { self.core.base.geometry() }
//     fn set_geometry(&mut self, g: Rect) { self.core.base.set_geometry(g); }
//     // ... 所有其他 Widget trait 方法委托给 core.base ...
// }
```

## 完整浏览器集成

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

        // 配置隐私设置
        view.privacy_mut().settings_mut().block_domain("ad-tracker.com");
        view.security_mut().block_popups = true;
        view.settings_mut().user_agent = "MyBrowser/1.0".to_string();

        // 注册一个插件
        view.plugins_mut().register(Box::new(
            ContentPlugin::new("Image Viewer", "1.0")
        )).unwrap();

        // 连接信号
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

        // 导航前检查跟踪
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
        // 退出时清理所有数据（隐私浏览）
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

## 总结

| 组件 | 用途 |
|-----------|---------|
| `WebViewEnhanced` | 嵌入式网页内容 widget（WidgetKind::WebView） |
| `WebEngineViewEnhanced` | 完整浏览器引擎 widget，带额外信号 |
| `WebViewCore` | 共享实现（URL、标题、加载状态、进度） |
| `SessionHistory` | 后退/前进导航栈 |
| `NavigationHistory` | 带时间戳的导航记录，支持截断 |
| `BrowserHistory` | 持久化访问历史，支持搜索和排名 |
| `SimpleJsEngine` | 纯 Rust JavaScript 解释器 |
| `JsValue` | JavaScript 值类型（8 种变体） |
| `JsContext` | JS 上下文（全局变量、控制台消息） |
| `CookieJar` | Cookie 存储，支持域名范围、过期 |
| `TrackingProtection` | 10 种跟踪类型，域名允许/阻止 |
| `PrivacySettings` | 严格 / 平衡 / 宽松三种预设 |
| `SecuritySettings` | 混合内容、弹窗、恶意软件拦截 |
| `WebSettings` | JS、插件、图片、WebGL、用户代理 |
| `PluginManager` | 插件注册、生命周期、消息通信 |
| `ContentPlugin` | 针对 MIME 类型的内置内容处理器 |
| `BrowsingData` | 选择性数据清理（12 个类别） |
| `delegate_widget!` | 将 Widget trait 委托给 WebViewCore 的宏 |
| 8 个信号 | 通过 Signal/GenericSignal 进行状态观察 |
