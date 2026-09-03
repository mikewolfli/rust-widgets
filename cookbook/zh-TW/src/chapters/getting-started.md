# 開始使用

本章將引導你安裝 rust-widgets、設定你的第一個專案、撰寫完整的應用程式，並了解每個 rust-widgets 應用程式都會遵循的基本模式。

---

## 先決條件

在開始之前，請確保你的環境符合以下要求：

| 需求 | 最低版本 | 備註 |
|---|---|---|
| **Rust** | 1.87+ (MSRV) | 使用 `rustc --version` 檢查 |
| **作業系統** | Linux、macOS、Windows、Android、iOS、WASM、HarmonyOS | |
| **平台 SDK** | 請見下表 | 只需針對你要建構的目標平台安裝 |

### 平台相依套件

| 平台 | 所需套件 |
|---|---|
| **Linux (GTK)** | `libgtk-3-dev` |
| **Linux (Wayland)** | `libwayland-dev`, `wayland-protocols` |
| **macOS / iOS** | Xcode Command Line Tools |
| **Windows** | Visual Studio Build Tools (MSVC) |
| **Android** | Android NDK, `cargo-ndk` |
| **WASM** | `wasm-bindgen-cli`, `wasm-pack` |

如果你尚未安裝 Rust，請透過 [rustup](https://rustup.rs) 安裝：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

---

## 將 rust-widgets 加入你的專案

建立一個新的二進位 crate，並將 `rust_widgets` 加入相依套件：

```sh
cargo new my_rust_widgets_app
cd my_rust_widgets_app
```

編輯 `Cargo.toml` 並加入相依套件：

```toml
[package]
name = "my_rust_widgets_app"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_widgets = "1.0.0"
```

預設的功能集 (`desktop`) 會載入完整的桌面設定檔：透過 wgpu 的 GPU 渲染、作業系統原生平台後端、觸控支援、i18n、圖表、PDF/列印、無障礙存取以及進階 widgets。

---

## 功能選擇 — 三軸系統

rust-widgets 使用**三軸功能系統**，讓你可以組合出你真正需要的二進位檔案。請從每個軸線中選擇一個選項：

### 軸線 1：裝置設定檔（互斥 — 僅選一個）

| 設定檔 | 功能標記 | 說明 |
|---|---|---|
| **Desktop** | `desktop`（預設） | 完整原生平台、GPU、觸控、i18n、圖表、列印、PDF、無障礙 |
| **Tablet** | `tablet` | 觸控優先、GPU、原生控制項、無桌面額外功能 |
| **Mobile** | `mobile` | 觸控、GPU、行動 API、原生控制項 |
| **Embedded** | `embedded` | 軟體光柵化、無 GPU、低記憶體、相容 `no_std` |
| **Mini** | `mini` | LVGL 風格：約 15 個核心 widgets、軟體光柵化、無需大量記憶體配置的相依套件 |

### 軸線 2：作業系統後端（選擇一個或自動偵測）

| 後端 | 功能標記 | 目標平台 |
|---|---|---|
| **自動偵測** | `os-auto`（預設） | 根據 `target_os` 選擇後端 |
| **macOS (objc2)** | `macos` | 透過 objc2 繫結的 macOS |
| **macOS (legacy)** | `macos-legacy` | 透過 cocoa crate 的 macOS |
| **iOS** | `ios` | 透過 UIKit FFI 的 iOS |
| **Windows** | `windows` | 透過 Win32 API 的 Windows |
| **Linux GTK** | `linux-gtk` | 透過 GTK3 繫結的 Linux |
| **Linux Wayland** | `linux-wayland` | 透過 Wayland 協定的 Linux |
| **Android** | `android` | 透過 JNI 的 Android |
| **WASM** | `wasm` | 透過 wasm-bindgen 的網頁 |
| **HarmonyOS** | `harmony` | HarmonyOS 原生 |

### 軸線 3：功能（任意組合）

| 功能 | 功能標記 | 引入內容 |
|---|---|---|
| **GPU 渲染** | `wgpu` / `gpu` | `wgpu` crate |
| **軟體光柵化** | `software` | CPU 渲染器 |
| **觸控與手勢** | `touch` | 11 種手勢辨識器 |
| **i18n** | `i18n` | `tr!()` 巨集 + 翻譯基礎架構 |
| **圖表** | `chart` | LineChart、BarChart、PieChart、Sparkline |
| **PDF 輸出** | `pdf` | 文件生成管線 |
| **列印** | `print` | 系統列印服務 |
| **無障礙存取** | `a11y` | 透過 zbus 的 AT-SPI 橋接 |
| **全像投影** | `holographic` | 全像顯示支援 |
| **投影** | `projection` | 投影機/簡報顯示 |

### 範例 `Cargo.toml` 選擇

```toml
# 桌面 Linux 搭配 Wayland，保留所有功能：
[dependencies]
rust_widgets = { version = "1.0.0", features = ["desktop", "linux-wayland"] }

# 平板搭配自動偵測作業系統：
[dependencies]
rust_widgets = { version = "1.0.0", default-features = false, features = ["tablet"] }

# 最小內嵌（無 std）：
[dependencies]
rust_widgets = { version = "1.0.0", default-features = false, features = ["embedded"] }

# 行動 Android：
[dependencies]
rust_widgets = { version = "1.0.0", default-features = false, features = ["mobile", "android"] }

# WASM 網頁應用程式：
[dependencies]
rust_widgets = { version = "1.0.0", default-features = false, features = ["mobile", "wasm", "touch"] }
```

### 建構設定檔

針對資源受限的目標，提供了兩種額外的 release 設定檔：

```toml
# 在你的 Cargo.toml 中：
[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"

[profile.release-mini]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

建構指令：
```sh
cargo build --profile release-embedded --features embedded
```

---

## 你的第一個應用程式：完整運作範例

以下是一個完整、自足的 rust-widgets 應用程式。請建立 `src/main.rs`：

```rust
use rust_widgets::prelude::*;
use rust_widgets::app::{App, AppConfig};

fn main() {
    // ── 1. 建立應用程式並進行設定 ──
    let app = App::with_config(
        AppConfig::default()
            .with_app_name("HelloApp")
            .with_organization("Acme Corp"),
    )
    .on_startup(|| {
        println!("應用程式啟動中...");
    })
    .on_shutdown(|| {
        println!("應用程式正在關閉。");
    });

    // ── 2. 初始化執行階段（平台後端 + 渲染器）──
    app.init();

    // ── 3. 建立主視窗 ──
    let window = app.new_window("我的第一個 rust-widgets 視窗", 100, 100, 800, 600);

    // ── 4. 使用 handle 建立 widgets ──
    let button = window.new_button("點我！", 10, 10, 120, 32);
    let label = window.new_label("哈囉，rust-widgets！", 10, 60, 300, 24);

    // ── 5. 連接訊號 ──
    let mut counter = 0;
    button.on_click(move || {
        counter += 1;
        label.set_text(&format!("已點擊 {} 次", counter));
    });

    // ── 6. 執行事件迴圈 ──
    app.run();
}
```

> **注意**：`prelude` 模組會重新匯出最常用的型別：所有幾何型別（`Point`、`Size`、`Rect`）、顏色（`Color`）、字型（`Font`）以及 widget 建構函式。請在每個 rust-widgets 應用程式的頂端匯入它。

---

## 建構與執行

```sh
# 開發建構（編譯快速，無最佳化）：
cargo build

# 執行：
cargo run

# Release 建構（含 LTO、codegen-units=1）：
cargo build --release
cargo run --release
```

---

## Widget 建立模式

rust-widgets 提供兩種互補的 widget 建立 API：

### 1. 頂層 `create_*` 函式（低階）

這些是 `lib.rs` 中相容 C-ABI 的頂層函式，用於配置 widget 並回傳其 `ObjectId`：

```rust
use rust_widgets::{
    create_window, create_button, create_label,
    create_checkbox, create_line_edit, create_slider,
    create_progress_bar, create_combo_box, create_list_box,
    create_spin_box, create_list_view, create_scroll_area,
    create_panel, create_message_box, create_file_dialog,
    create_color_dialog, create_font_dialog,
    create_radio_button,
};

let window_id = create_window("我的視窗", 100, 100, 800, 600);
let button_id = create_button("提交", 10, 10, 120, 32);
let label_id = create_label("狀態", 10, 60, 300, 24);
```

可用的 `create_*` 函式：

| 函式 | 建立內容 |
|---|---|
| `create_window(name, x, y, w, h)` | 頂層視窗 |
| `create_button(text, x, y, w, h)` | 按鈕 |
| `create_checkbox(text, x, y, w, h)` | 核取方塊 |
| `create_radio_button(text, x, y, w, h)` | 選項按鈕 |
| `create_label(text, x, y, w, h)` | 文字標籤 |
| `create_line_edit(text, x, y, w, h)` | 單行文字輸入 |
| `create_slider(min, max, val, x, y, w, h)` | 水平滑桿 |
| `create_progress_bar(min, max, val, x, y, w, h)` | 進度指示器 |
| `create_combo_box(x, y, w, h)` | 下拉式選單 |
| `create_list_box(x, y, w, h)` | 滾動列表 |
| `create_spin_box(min, max, step, val, x, y, w, h)` | 數字旋轉盒 |
| `create_list_view(x, y, w, h)` | 多欄列表 |
| `create_scroll_area(x, y, w, h)` | 可滾動容器 |
| `create_panel(x, y, w, h)` | 面板/群組方塊 |
| `create_message_box(title, msg)` | 模態訊息對話框 |
| `create_file_dialog(title, dir, filter)` | 檔案選擇器 |
| `create_color_dialog(title)` | 色彩選取器對話框 |
| `create_font_dialog(title)` | 字型選擇對話框 |

### 2. App API 搭配型別化 Handle（建議）

`App` API 會回傳**型別化的 handle**，提供編譯期安全性和便利方法。這是新應用程式的建議做法：

```rust
use rust_widgets::app::{App, AppConfig};

let app = App::new();
app.init();

let window = app.new_window("標題", 100, 100, 640, 480);

// 每個 handle 會公開特定 widget 的方法：
let btn  = window.new_button("確定", 10, 10, 80, 24);
let chk  = window.new_checkbox("啟用", 10, 40, 120, 24);
let edit = window.new_line_edit("", 10, 70, 200, 24);
let lbl  = window.new_label("輸出", 10, 100, 300, 24);
let cb   = window.new_combo_box(10, 130, 200, 24);
let list = window.new_list_box(10, 160, 200, 100);
let prog = window.new_progress_bar(0, 100, 50, 10, 270, 200, 20);
let spin = window.new_spin_box(0, 100, 1, 50, 10, 300, 80, 24);
let grid = window.new_grid(10, 330, 400, 200);
let frame = window.new_frame("區段", 10, 540, 400, 50);
let radio = window.new_radio_button("選項 A", 420, 10, 120, 24);
let slider = window.new_slider(0, 100, 50, 420, 40, 200, 24);
let scroll = window.new_scroll_area(420, 70, 200, 150);
let tab = window.new_tab_widget(420, 230, 200, 150);
let web = window.new_web_view(420, 390, 200, 150);
```

> **設計原則**：rust-widgets 對 widget 型別使用**建構器模式**。每個 widget 結構體都公開一個 `new(...)` 建構函式，但 `App` API 透過 `WindowHandle` 提供了更符合人體工學的工廠方法。

---

## 事件迴圈

rust-widgets 管理自己的事件迴圈。三個關鍵的生命週期函式是：

```rust
// 在 lib.rs 中 — 頂層 API：
pub fn init();   // 初始化平台後端 + 渲染器
pub fn run();    // 進入事件迴圈（阻塞直到退出）
pub fn quit();   // 通知事件迴圈結束
```

### 使用 App API

`App` 結構體包裝了這些生命週期函式：

```rust
let app = App::with_config(AppConfig::default()
    .with_app_name("MyApp"))
    .on_startup(|| { /* 初始化資料 */ })
    .on_shutdown(|| { /* 清理 */ });

app.init();  // ← 必須在建立 widgets 之前呼叫
// ... 建立 widgets ...
app.run();   // ← 在此處阻塞，處理事件直到視窗關閉
```

### AppConfig 建構器

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_name: String,        // 用於視窗標題等
    pub organization: String,    // 廠商/組織名稱
    pub enable_i18n: bool,       // 初始化 i18n 子系統（預設：true）
}

impl AppConfig {
    pub fn with_app_name(mut self, name: &str) -> Self;
    pub fn with_organization(mut self, org: &str) -> Self;
    pub fn with_i18n(mut self, enable: bool) -> Self;
}
```

### 生命週期狀態機

`AppLifecycle` 狀態機追蹤應用程式的狀態轉換：

```rust
use rust_widgets::app::lifecycle::{AppLifecycle, AppLifecycleState, LifecycleEvent};

// 狀態：
// Starting → Foreground → Background → Suspended → Terminating
//
// 事件：
// WillEnterForeground, DidEnterForeground,
// WillEnterBackground, DidEnterBackground,
// WillTerminate, MemoryWarning, StateRestored

let mut lifecycle = AppLifecycle::new();
lifecycle.transition(AppLifecycleState::Foreground);

lifecycle.add_listener(Box::new(move |event| {
    match event {
        LifecycleEvent::WillEnterBackground => { /* 暫停工作 */ }
        LifecycleEvent::DidEnterForeground => { /* 恢復工作 */ }
        LifecycleEvent::MemoryWarning => { /* 釋放快取 */ }
        _ => {}
    }
}));
```

---

## 在執行階段載入 JSON 佈局

rust-widgets 支援以 JSON 定義 widget 樹，以實現動態 UI 載入：

```rust
use rust_widgets::json;

let json_str = r#"
{
    "widgets": [
        {
            "kind": "Window",
            "title": "JSON 視窗",
            "geometry": { "x": 100, "y": 100, "width": 800, "height": 600 },
            "children": [
                {
                    "kind": "Button",
                    "id": "btn_submit",
                    "text": "提交",
                    "geometry": { "x": 10, "y": 10, "width": 120, "height": 32 }
                },
                {
                    "kind": "Label",
                    "id": "lbl_status",
                    "text": "就緒",
                    "geometry": { "x": 10, "y": 60, "width": 300, "height": 24 }
                }
            ]
        }
    ]
}
"#;

// 從 JSON 解析並建立 widget 樹：
let result = json::build_from_json(json_str);
```

JSON 定義的 widgets 可以與程式建立的 widgets 混合使用。JSON 模組支援所有 `WidgetKind` 變體及其建構函式。

---

## 使用 `tr!()` 巨集進行 i18n

`tr!()` 巨集提供編譯期的金鑰式翻譯。翻譯金鑰會靜態提取，並在建構時透過覆蓋率稽核器進行檢查。

```rust
use rust_widgets::tr;

// 基本翻譯：
let greeting = tr!("hello_world");        // → "Hello, world!" (en)
                                          // → "你好，世界！" (zh-CN)

// 情境式翻譯：
let save = tr!("save");                   // 通用
let save_file = tr!("save", context: "file");  // 情境感知

// 複數形式：
let items = tr!("item_count", count: 1);  // → "1 item"
let items = tr!("item_count", count: 5);  // → "5 items"
```

### 在 Widget 上設定已翻譯的工具提示

```rust
// 在 widget trait 上：
widget.set_translated_tooltip("tooltip.save_button");

// 工具提示會顯示對應語系的翻譯。
```

i18n 子系統內建了**英文**（en）、**簡體中文**（zh-CN）和**繁體中文**（zh-TW）的翻譯。`audit_keys()` 函式會在建構時捕捉遺漏的翻譯，確保你的翻譯覆蓋率始終完整。

---

## Widget Handle 模式

Widget handles 是 `ObjectId` 的輕量包裝，為每種 widget 種類提供型別安全且符合人體工學的操作。

### `WidgetHandle` Trait

```rust
pub trait WidgetHandle: Sized {
    fn raw_id(&self) -> ObjectId;
    fn from_raw(id: ObjectId) -> Self;

    // 可見性
    fn show(&self);
    fn hide(&self);
    fn set_visible(&self, visible: bool);
    fn is_visible(&self) -> bool;

    // 幾何
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32);

    // 文字
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    // 啟用狀態
    fn enable(&self);
    fn disable(&self);
    fn is_enabled(&self) -> bool;

    // 事件回呼
    fn on_click<F: FnMut() + 'static>(&self, f: F);
    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F);
}
```

### 特殊化的 Handle 型別

| Handle 型別 | Widget | 額外方法 |
|---|---|---|
| `WindowHandle` | Window | `new_button()`、`new_label()`、`new_line_edit()` 等 |
| `ButtonHandle` | Button | 點擊回呼 |
| `CheckBoxHandle` | CheckBox | `set_checked()`、`is_checked()`、`CheckState` |
| `LabelHandle` | Label | 文字取得/設定 |
| `LineEditHandle` | LineEdit | `set_echo_mode()`、`EchoMode` |
| `ComboBoxHandle` | ComboBox | `add_item()`、`clear()`、`set_current_index()` |
| `ListBoxHandle` | ListBox | `add_item()`、`remove_item()`、`current_index()` |
| `SliderHandle` | Slider | `set_value()`、`value()`、`set_range()` |
| `ProgressBarHandle` | ProgressBar | `set_value()`、`set_range()` |
| `SpinBoxHandle` | SpinBox | `set_value()`、`value()`、`set_range()` |
| `ScrollBarHandle` | ScrollBar | `set_value()`、`set_range()` |
| `TabWidgetHandle` | TabWidget | `add_tab()`、`set_current_index()` |
| `ScrollAreaHandle` | ScrollArea | 其他 widget 的容器 |
| `ListViewHandle` | ListView | `ListModel` 整合 |
| `TextEditHandle` | TextEdit | 多行編輯 |
| `FrameHandle` | Frame/GroupBox | 容器框架 |
| `GridWidgetHandle` | Grid | 網格佈局 |
| `RadioButtonHandle` | RadioButton | 選取狀態 |
| `MessageBoxHandle` | MessageBox | 模態訊息顯示 |
| `WebViewHandle` | WebView | 網頁內容顯示 |
| `DialogHandle` | Dialog/PopupWindow | 模態對話框 |

### 回呼分派

回呼會以每個 `ObjectId` 為單位儲存在執行緒區域儲存空間中，並在平台後端觸發事件時進行分派：

```rust
// 內部分派函式：
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool;

// WidgetHandle::on_click 註冊一個回呼：
button.on_click(|| println!("已點擊！"));

// WidgetHandle::on_value_changed 註冊一個值變更回呼：
combo.on_value_changed(|text| println!("已選取：{}", text));
```

---

## 常見模式與最佳實務

### 1. 在容器 Handle 中對相關 Widgets 進行分組

```rust
// 好作法：透過視窗 handle 建立子元件
let button = window.new_button("確定", 10, 10, 80, 32);

// 視窗 handle 會在內部追蹤父子關係。
```

### 2. 使用限定範圍的訊號連接

當直接使用 `Signal` 系統（而非透過 handles）時，請使用 `ConnectionScope` 進行自動清理：

```rust
use rust_widgets::signal::ConnectionScope;

let scope = ConnectionScope::new();
my_signal.connect_scoped(&scope, |value| {
    println!("收到：{:?}", value);
});
// 當 `scope` 被釋放時，所有透過它建立的連接都會被中斷。
```

### 3. 偏好建構器風格的 AppConfig

```rust
let app = App::with_config(
    AppConfig::default()
        .with_app_name("ProductionApp")
        .with_organization("MyCompany")
        .with_i18n(true),
)
.on_startup(|| { setup_logging(); })
.on_shutdown(|| { save_state(); });
```

### 4. 將 UI 建立與商業邏輯分離

```rust
struct AppState {
    counter: i32,
}

fn build_ui(window: &WindowHandle, state: &mut AppState) {
    let label = window.new_label("計數：0", 10, 10, 200, 24);
    let button = window.new_button("增加", 10, 40, 100, 32);

    button.on_click(move || {
        state.counter += 1;
        label.set_text(&format!("計數：{}", state.counter));
    });
}
```

### 5. 使用 `#[cfg]` 檢查功能可用性

```rust
#[cfg(not(feature = "mini"))]
fn create_rich_editor(parent: &WindowHandle) {
    // 使用僅在非 mini 設定檔中可用的進階 widgets
    let _editor = parent.new_text_edit("", 10, 10, 400, 300);
}

#[cfg(feature = "mini")]
fn create_rich_editor(_parent: &WindowHandle) {
    // mini 設定檔的備用方案
}
```

### 6. 遵循座標系統

rust-widgets 使用**螢幕座標**，原點在**左上角**：
- X 向右增加
- Y 向下增加

Widget 定位遵循 `Rect::new(x, y, width, height)`，其中 `(x, y)` 是左上角（以像素為單位）。

### 7. 在銷毀 Widget 時清理回呼

```rust
use rust_widgets::app::handle::remove_callbacks;

// 當 widget 被銷毀時：
remove_callbacks(widget_id);
```

### 8. 在建立 Widget 之前先初始化

在建立任何 widgets 之前，請務必先呼叫 `app.init()`。這會初始化平台後端、渲染管線和 i18n 子系統。

---

## 後續步驟

現在你已經有一個運作中的 rust-widgets 應用程式，可以進一步深入探索：

- **架構概觀** — 了解分層架構、crate 階層以及編譯期 vs 執行期的設計決策
- **核心型別** — 掌握 `ObjectId`、`Color`、`Rect`、`Size`、`Point`、`Font` 以及所有基本建構區塊
- **Widget 系統** — 探索完整的 widget 階層、`Widget` trait，以及如何建立自訂 widgets
