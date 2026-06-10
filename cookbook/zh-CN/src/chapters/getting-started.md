# 快速入门

本章将指导你安装 `rust-widgets`、配置你的第一个项目、编写一个完整的应用程序，并理解所有 `rust-widgets` 应用所遵循的基本模式。

---

## 前置要求

在开始之前，请确保你的环境满足以下要求：

| 要求 | 最低版本 | 说明 |
|---|---|---|
| **Rust** | 1.87+ (MSRV) | 使用 `rustc --version` 检查 |
| **操作系统** | Linux, macOS, Windows, Android, iOS, WASM, HarmonyOS | |
| **平台 SDK** | 见下表 | 仅在你构建的目标平台上需要 |

### 平台依赖

| 平台 | 必需的软件包 |
|---|---|
| **Linux (GTK)** | `libgtk-3-dev` |
| **Linux (Wayland)** | `libwayland-dev`, `wayland-protocols` |
| **macOS / iOS** | Xcode Command Line Tools |
| **Windows** | Visual Studio Build Tools (MSVC) |
| **Android** | Android NDK, `cargo-ndk` |
| **WASM** | `wasm-bindgen-cli`, `wasm-pack` |

如果你尚未安装 Rust，请通过 [rustup](https://rustup.rs) 安装：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

---

## 将 rust-widgets 添加到你的项目

创建一个新的二进制 crate 并将 `rust_widgets` 添加为依赖：

```sh
cargo new my_rust_widgets_app
cd my_rust_widgets_app
```

编辑 `Cargo.toml` 并添加依赖：

```toml
[package]
name = "my_rust_widgets_app"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_widgets = "0.9.6"
```

默认功能集（`desktop`）会拉取完整的桌面配置文件：通过 wgpu 的 GPU 渲染、操作系统原生平台后端、触摸支持、i18n、图表、PDF/打印、辅助功能和高级控件。

---

## 功能选择——三轴系统

`rust-widgets` 使用**三轴功能系统**，让你能够精确组合所需的二进制产物。从每个轴中选择一个选项：

### 轴 1：设备配置文件（互斥——只选一个）

| 配置文件 | 功能标志 | 描述 |
|---|---|---|
| **桌面** | `desktop`（默认） | 完整的原生平台、GPU、触摸、i18n、图表、打印、PDF、辅助功能 |
| **平板** | `tablet` | 触摸优先、GPU、原生控件，不含桌面附加功能 |
| **移动端** | `mobile` | 触摸、GPU、移动端 API、原生控件 |
| **嵌入式** | `embedded` | 软件光栅化、无 GPU、低内存、兼容 `no_std` |
| **迷你** | `mini` | LVGL 风格：约 15 个核心控件、软件光栅化、无 alloc 密集型依赖 |

### 轴 2：操作系统后端（选一个或自动检测）

| 后端 | 功能标志 | 目标平台 |
|---|---|---|
| **自动检测** | `os-auto`（默认） | 根据 `target_os` 选择后端 |
| **macOS (objc2)** | `macos` | 通过 objc2 绑定的 macOS |
| **macOS (legacy)** | `macos-legacy` | 通过 cocoa crate 的 macOS |
| **iOS** | `ios` | 通过 UIKit FFI 的 iOS |
| **Windows** | `windows` | 通过 Win32 API 的 Windows |
| **Linux GTK** | `linux-gtk` | 通过 GTK3 绑定的 Linux |
| **Linux Wayland** | `linux-wayland` | 通过 Wayland 协议的 Linux |
| **Android** | `android` | 通过 JNI 的 Android |
| **WASM** | `wasm` | 通过 wasm-bindgen 的 Web |
| **HarmonyOS** | `harmony` | HarmonyOS 原生 |

### 轴 3：能力（任意组合）

| 能力 | 功能标志 | 引入的依赖 |
|---|---|---|
| **GPU 渲染** | `wgpu` / `gpu` | `wgpu` crate |
| **软件光栅化** | `software` | CPU 渲染器 |
| **触摸与手势** | `touch` | 11 个手势识别器 |
| **国际化** | `i18n` | `tr!()` 宏 + 翻译基础设施 |
| **图表** | `chart` | LineChart, BarChart, PieChart, Sparkline |
| **PDF 输出** | `pdf` | 文档生成管线 |
| **打印** | `print` | 系统打印服务 |
| **辅助功能** | `a11y` | 通过 zbus 的 AT-SPI 桥接 |
| **全息显示** | `holographic` | 全息显示支持 |
| **投影** | `projection` | 投影仪/演示显示 |

### 示例 `Cargo.toml` 选择

```toml
# 桌面 Linux 使用 Wayland，保留所有功能：
[dependencies]
rust_widgets = { version = "0.9.6", features = ["desktop", "linux-wayland"] }

# 平板使用自动检测操作系统：
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["tablet"] }

# 最小嵌入式（无 std）：
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["embedded"] }

# 移动端 Android：
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["mobile", "android"] }

# WASM Web 应用：
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["mobile", "wasm", "touch"] }
```

### 构建配置文件

为有限的目标平台提供了两个额外的发布配置文件：

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

使用以下命令构建：
```sh
cargo build --profile release-embedded --features embedded
```

---

## 你的第一个应用程序：完整的可运行示例

下面是一个完整的、自包含的 `rust-widgets` 应用程序。创建 `src/main.rs`：

```rust
use rust_widgets::prelude::*;
use rust_widgets::app::{App, AppConfig};

fn main() {
    // ── 1. 使用配置创建应用 ──
    let app = App::with_config(
        AppConfig::default()
            .with_app_name("HelloApp")
            .with_organization("Acme Corp"),
    )
    .on_startup(|| {
        println!("Application starting up...");
    })
    .on_shutdown(|| {
        println!("Application shutting down.");
    });

    // ── 2. 初始化运行时（平台后端 + 渲染器）──
    app.init();

    // ── 3. 创建主窗口 ──
    let window = app.new_window("My First rust-widgets Window", 100, 100, 800, 600);

    // ── 4. 使用句柄创建控件 ──
    let button = window.new_button("Click Me!", 10, 10, 120, 32);
    let label = window.new_label("Hello, rust-widgets!", 10, 60, 300, 24);

    // ── 5. 连接信号 ──
    let mut counter = 0;
    button.on_click(move || {
        counter += 1;
        label.set_text(&format!("Clicked {} times", counter));
    });

    // ── 6. 运行事件循环 ──
    app.run();
}
```

> **注意**：`prelude` 模块重新导出了最常用的类型：所有几何类型（`Point`、`Size`、`Rect`）、颜色（`Color`）、字体（`Font`）和控件构造器。在每个 `rust-widgets` 应用的顶部都请导入它。

---

## 构建与运行

```sh
# 开发构建（编译快，无优化）：
cargo build

# 运行：
cargo run

# 发布构建（启用 LTO，codegen-units=1）：
cargo build --release
cargo run --release
```

---

## 控件创建模式

`rust-widgets` 提供了两种互补的控件创建 API：

### 1. 顶层 `create_*` 函数（底层）

这些是 `lib.rs` 中符合 C-ABI 的顶层函数，用于分配控件并返回其 `ObjectId`：

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

let window_id = create_window("My Window", 100, 100, 800, 600);
let button_id = create_button("Submit", 10, 10, 120, 32);
let label_id = create_label("Status", 10, 60, 300, 24);
```

可用的 `create_*` 函数：

| 函数 | 创建对象 |
|---|---|
| `create_window(name, x, y, w, h)` | 顶层窗口 |
| `create_button(text, x, y, w, h)` | 按钮 |
| `create_checkbox(text, x, y, w, h)` | 复选框 |
| `create_radio_button(text, x, y, w, h)` | 单选按钮 |
| `create_label(text, x, y, w, h)` | 文本标签 |
| `create_line_edit(text, x, y, w, h)` | 单行文本输入 |
| `create_slider(min, max, val, x, y, w, h)` | 水平滑块 |
| `create_progress_bar(min, max, val, x, y, w, h)` | 进度指示器 |
| `create_combo_box(x, y, w, h)` | 下拉组合框 |
| `create_list_box(x, y, w, h)` | 滚动列表 |
| `create_spin_box(min, max, step, val, x, y, w, h)` | 数字微调框 |
| `create_list_view(x, y, w, h)` | 多列列表 |
| `create_scroll_area(x, y, w, h)` | 可滚动容器 |
| `create_panel(x, y, w, h)` | 面板/分组框 |
| `create_message_box(title, msg)` | 模态消息对话框 |
| `create_file_dialog(title, dir, filter)` | 文件选择器 |
| `create_color_dialog(title)` | 颜色选择对话框 |
| `create_font_dialog(title)` | 字体选择对话框 |

### 2. App API 与类型化句柄（推荐）

`App` API 返回**类型化句柄**，提供编译时安全性和便捷方法。这是新应用推荐的方式：

```rust
use rust_widgets::app::{App, AppConfig};

let app = App::new();
app.init();

let window = app.new_window("Title", 100, 100, 640, 480);

// 每个句柄暴露控件特定的方法：
let btn  = window.new_button("OK", 10, 10, 80, 24);
let chk  = window.new_checkbox("Enable", 10, 40, 120, 24);
let edit = window.new_line_edit("", 10, 70, 200, 24);
let lbl  = window.new_label("Output", 10, 100, 300, 24);
let cb   = window.new_combo_box(10, 130, 200, 24);
let list = window.new_list_box(10, 160, 200, 100);
let prog = window.new_progress_bar(0, 100, 50, 10, 270, 200, 20);
let spin = window.new_spin_box(0, 100, 1, 50, 10, 300, 80, 24);
let grid = window.new_grid(10, 330, 400, 200);
let frame = window.new_frame("Section", 10, 540, 400, 50);
let radio = window.new_radio_button("Option A", 420, 10, 120, 24);
let slider = window.new_slider(0, 100, 50, 420, 40, 200, 24);
let scroll = window.new_scroll_area(420, 70, 200, 150);
let tab = window.new_tab_widget(420, 230, 200, 150);
let web = window.new_web_view(420, 390, 200, 150);
```

> **设计原则**：`rust-widgets` 对控件类型使用**构建器模式**。每个控件结构体都暴露一个 `new(...)` 构造器，但 `App` API 通过 `WindowHandle` 提供更符合人体工程学的工厂方法。

---

## 事件循环

`rust-widgets` 管理自己的事件循环。三个关键的生命周期函数是：

```rust
// 在 lib.rs 中——顶层 API：
pub fn init();   // 初始化平台后端 + 渲染器
pub fn run();    // 进入事件循环（阻塞直到退出）
pub fn quit();   // 通知事件循环退出
```

### 使用 App API

`App` 结构体封装了这些生命周期函数：

```rust
let app = App::with_config(AppConfig::default()
    .with_app_name("MyApp"))
    .on_startup(|| { /* 初始化数据 */ })
    .on_shutdown(|| { /* 清理 */ });

app.init();  // ← 创建控件前必须调用
// ... 创建控件 ...
app.run();   // ← 在此阻塞，处理事件直到窗口关闭
```

### AppConfig 构建器

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_name: String,        // 用于窗口标题等
    pub organization: String,    // 厂商/组织名称
    pub enable_i18n: bool,       // 初始化 i18n 子系统（默认：true）
}

impl AppConfig {
    pub fn with_app_name(mut self, name: &str) -> Self;
    pub fn with_organization(mut self, org: &str) -> Self;
    pub fn with_i18n(mut self, enable: bool) -> Self;
}
```

### 生命周期状态机

`AppLifecycle` 状态机跟踪应用程序状态转换：

```rust
use rust_widgets::app::lifecycle::{AppLifecycle, AppLifecycleState, LifecycleEvent};

// 状态：
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
        LifecycleEvent::WillEnterBackground => { /* 暂停工作 */ }
        LifecycleEvent::DidEnterForeground => { /* 恢复工作 */ }
        LifecycleEvent::MemoryWarning => { /* 释放缓存 */ }
        _ => {}
    }
}));
```

---

## 在运行时加载 JSON 布局

`rust-widgets` 支持以 JSON 定义控件树，用于动态 UI 加载：

```rust
use rust_widgets::json;

let json_str = r#"
{
    "widgets": [
        {
            "kind": "Window",
            "title": "JSON Window",
            "geometry": { "x": 100, "y": 100, "width": 800, "height": 600 },
            "children": [
                {
                    "kind": "Button",
                    "id": "btn_submit",
                    "text": "Submit",
                    "geometry": { "x": 10, "y": 10, "width": 120, "height": 32 }
                },
                {
                    "kind": "Label",
                    "id": "lbl_status",
                    "text": "Ready",
                    "geometry": { "x": 10, "y": 60, "width": 300, "height": 24 }
                }
            ]
        }
    ]
}
"#;

// 从 JSON 解析并构建控件树：
let result = json::build_from_json(json_str);
```

JSON 定义的控件可以与编程方式创建的控件混合使用。JSON 模块支持所有 `WidgetKind` 变体及其构造器。

---

## 使用 `tr!()` 宏进行国际化

`tr!()` 宏提供编译时基于键的翻译。翻译键是静态提取的，并在构建时通过覆盖率审计器进行检查。

```rust
use rust_widgets::tr;

// 基本翻译：
let greeting = tr!("hello_world");        // → "Hello, world!"（英文）
                                          // → "你好，世界！"（中文）

// 基于上下文的翻译：
let save = tr!("save");                   // 通用
let save_file = tr!("save", context: "file");  // 上下文感知

// 复数形式：
let items = tr!("item_count", count: 1);  // → "1 item"
let items = tr!("item_count", count: 5);  // → "5 items"
```

### 在控件上设置已翻译的提示文本

```rust
// 在控件 trait 上：
widget.set_translated_tooltip("tooltip.save_button");

// 提示文本将显示与区域设置对应的翻译。
```

国际化子系统内置了**英文**（en）、**简体中文**（zh-CN）和**繁体中文**（zh-TW）翻译。`audit_keys()` 函数在构建时捕获缺失的翻译，确保你的翻译覆盖率始终完整。

---

## 控件句柄模式

控件句柄是围绕 `ObjectId` 的轻量级包装器，为每种控件类型提供类型安全、符合人体工程学的操作。

### `WidgetHandle` Trait

```rust
pub trait WidgetHandle: Sized {
    fn raw_id(&self) -> ObjectId;
    fn from_raw(id: ObjectId) -> Self;

    // 可见性
    fn show(&self);
    fn hide(&self);
    fn set_visible(&self, visible: bool);
    fn is_visible(&self) -> bool;

    // 几何属性
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32);

    // 文本
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    // 启用状态
    fn enable(&self);
    fn disable(&self);
    fn is_enabled(&self) -> bool;

    // 事件回调
    fn on_click<F: FnMut() + 'static>(&self, f: F);
    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F);
}
```

### 专用的句柄类型

| 句柄类型 | 控件 | 额外方法 |
|---|---|---|
| `WindowHandle` | Window | `new_button()`, `new_label()`, `new_line_edit()` 等 |
| `ButtonHandle` | Button | 点击回调 |
| `CheckBoxHandle` | CheckBox | `set_checked()`, `is_checked()`, `CheckState` |
| `LabelHandle` | Label | 文本获取/设置 |
| `LineEditHandle` | LineEdit | `set_echo_mode()`, `EchoMode` |
| `ComboBoxHandle` | ComboBox | `add_item()`, `clear()`, `set_current_index()` |
| `ListBoxHandle` | ListBox | `add_item()`, `remove_item()`, `current_index()` |
| `SliderHandle` | Slider | `set_value()`, `value()`, `set_range()` |
| `ProgressBarHandle` | ProgressBar | `set_value()`, `set_range()` |
| `SpinBoxHandle` | SpinBox | `set_value()`, `value()`, `set_range()` |
| `ScrollBarHandle` | ScrollBar | `set_value()`, `set_range()` |
| `TabWidgetHandle` | TabWidget | `add_tab()`, `set_current_index()` |
| `ScrollAreaHandle` | ScrollArea | 其他控件的容器 |
| `ListViewHandle` | ListView | `ListModel` 集成 |
| `TextEditHandle` | TextEdit | 多行编辑 |
| `FrameHandle` | Frame/GroupBox | 容器框架 |
| `GridWidgetHandle` | Grid | 网格布局 |
| `RadioButtonHandle` | RadioButton | 选中状态 |
| `MessageBoxHandle` | MessageBox | 模态消息显示 |
| `WebViewHandle` | WebView | 网页内容显示 |
| `DialogHandle` | Dialog/PopupWindow | 模态对话框 |

### 回调分发

回调按 `ObjectId` 存储在线程本地存储中，并在平台后端发出触发事件时分发：

```rust
// 内部分发函数：
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool;

// WidgetHandle::on_click 注册回调：
button.on_click(|| println!("Clicked!"));

// WidgetHandle::on_value_changed 注册值变化回调：
combo.on_value_changed(|text| println!("Selected: {}", text));
```

---

## 常用模式与最佳实践

### 1. 在容器句柄中对相关控件进行分组

```rust
// 好：通过窗口句柄创建子控件
let button = window.new_button("OK", 10, 10, 80, 32);

// 窗口句柄内部跟踪父子关系。
```

### 2. 使用作用域信号连接

当直接使用 `Signal` 系统（而非通过句柄）时，请使用 `ConnectionScope` 进行自动清理：

```rust
use rust_widgets::signal::ConnectionScope;

let scope = ConnectionScope::new();
my_signal.connect_scoped(&scope, |value| {
    println!("Received: {:?}", value);
});
// 当 `scope` 被丢弃时，通过它建立的所有连接都会断开。
```

### 3. 优先使用构建器风格的 AppConfig

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

### 4. 将 UI 创建与业务逻辑分离

```rust
struct AppState {
    counter: i32,
}

fn build_ui(window: &WindowHandle, state: &mut AppState) {
    let label = window.new_label("Count: 0", 10, 10, 200, 24);
    let button = window.new_button("Increment", 10, 40, 100, 32);

    button.on_click(move || {
        state.counter += 1;
        label.set_text(&format!("Count: {}", state.counter));
    });
}
```

### 5. 使用 `#[cfg]` 检查功能可用性

```rust
#[cfg(not(feature = "mini"))]
fn create_rich_editor(parent: &WindowHandle) {
    // 使用仅在非 mini 配置文件中可用的高级控件
    let _editor = parent.new_text_edit("", 10, 10, 400, 300);
}

#[cfg(feature = "mini")]
fn create_rich_editor(_parent: &WindowHandle) {
    // mini 配置文件的回退方案
}
```

### 6. 遵循坐标系统

`rust-widgets` 使用**屏幕坐标**，原点位于**左上角**：
- X 向右增加
- Y 向下增加

控件定位遵循 `Rect::new(x, y, width, height)`，其中 `(x, y)` 是左上角的像素坐标。

### 7. 销毁控件时清理回调

```rust
use rust_widgets::app::handle::remove_callbacks;

// 当控件被销毁时：
remove_callbacks(widget_id);
```

### 8. 在创建控件之前初始化

始终在创建任何控件之前调用 `app.init()`。这会初始化平台后端、渲染管线以及国际化子系统。

---

## 下一步

现在你已经有了一个可运行的 `rust-widgets` 应用，可以深入了解更多内容：

- **架构概览**——理解分层架构、crate 层次结构以及编译时与运行时的设计决策
- **核心类型**——掌握 `ObjectId`、`Color`、`Rect`、`Size`、`Point`、`Font` 以及所有基本构建块
- **控件系统**——探索完整的控件层次结构、`Widget` trait，以及如何创建自定义控件
