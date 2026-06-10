# 平台支持

rust-widgets 在八个支持的平台上提供了统一的 API。本章涵盖平台抽象层、后端选择、设备检测、剪贴板、拖放、IME、无障碍、菜单、能力协商和虚拟键盘支持。

---

## 1. 八个支持的平台

| # | 平台 | 后端 | 特性标志 | 状态 |
|---|------|------|:---:|:---:|
| 1 | **Linux (GTK)** | 原生 GTK3 窗口 | `gtk-native` | ✅ 原生窗口 |
| 2 | **Linux (Wayland)** | 原生 Wayland 协议 | `wayland-native` | ✅ 自动检测会话 |
| 3 | **Windows** | Win32 API | *(始终开启)* | ✅ 原生 |
| 4 | **macOS** | Cocoa / objc2 桥接 | `objc2-macos` | ✅ 原生 |
| 5 | **iOS** | UIKit 状态后端 | `ios` | ✅ 状态驱动 |
| 6 | **Android** | JNI 桥接 | `android-jni` | ✅ JNI 桥接 |
| 7 | **WASM** | WebAssembly 画布 | `wasm` | ✅ 浏览器 |
| 8 | **HarmonyOS** | NAPI 桥接 | `harmony` | ✅ 预览版 |
| 9 | **嵌入式** | Stub / no_std | `embedded` / `mini` | ✅ no_std |

在 Linux 上，运行时通过 `$WAYLAND_DISPLAY` 和 `$XDG_SESSION_TYPE` 环境变量自动检测 Wayland 和 X11/GTK。

---

## 2. `Platform` 特质 — 通用契约

`Platform` 特质定义了约 70 个方法，涵盖 26 个窗口部件创建函数。每个后端都实现此特质，确保跨平台的 API 表面一致。

```rust
use rust_widgets::platform::{Platform, PlatformCapabilities};

fn inspect_backend(platform: &dyn Platform) {
    println!("后端: {}", platform.backend_name());
    println!("系列:  {:?}", platform.family());

    let caps: PlatformCapabilities = platform.capabilities();
    println!("DPI 缩放:    {}", caps.dpi_scaling);
    println!("IME:            {}", caps.ime);
    println!("无障碍:  {}", caps.accessibility);
    println!("原生菜单:   {}", caps.native_menu);
}
```

### 窗口部件创建方法（子集）

| 方法 | 窗口部件 | 签名 |
|--------|--------|-----------|
| `create_window` | Window | `(title, x, y, w, h) -> ObjectId` |
| `create_button` | Button | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_checkbox` | CheckBox | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_line_edit` | LineEdit | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_label` | Label | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_radio_button` | RadioButton | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_slider` | Slider | `(parent, x, y, w, h) -> ObjectId` |
| `create_progress_bar` | ProgressBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_combo_box` | ComboBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_box` | ListBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_panel` | Panel | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu_bar` | MenuBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu` | Menu | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_tool_bar` | ToolBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_status_bar` | StatusBar | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_message_box` | MessageBox | `(parent, title, text, x, y, w, h) -> ObjectId` |
| `create_file_dialog` | FileDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_color_dialog` | ColorDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_font_dialog` | FontDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_spin_box` | SpinBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_view` | ListView | `(parent, x, y, w, h) -> ObjectId` |
| `create_scroll_area` | ScrollArea | `(parent, x, y, w, h) -> ObjectId` |

常用的窗口部件操作方法：`show_widget`、`hide_widget`、`set_widget_geometry`、`set_widget_text`、`get_widget_text`、`set_widget_enabled`、`is_widget_enabled`、`set_widget_visible`、`is_widget_visible`、`set_widget_ime_enabled`、`is_widget_ime_enabled`、`set_widget_accessibility_name`、`get_widget_accessibility_name`。

---

## 3. `BackendState<K>` — 线程安全的 HashMap 状态存储

`BackendState<K>` 是一个线程安全、可 serde 序列化的状态存储，供状态驱动后端（Android、iOS、WASM、Harmony、嵌入式）使用。它在 `Mutex` 保护下存储窗口部件记录、菜单事件、窗口部件触发事件、剪贴板文本和拖放事件。

```rust
use rust_widgets::platform::state::BackendState;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MyKind { Button, Label }

let state = BackendState::<MyKind>::new();

// 创建窗口部件；返回一个自增的 ObjectId
let id = state.create_widget(MyKind::Button, "Click Me", 0, 0, 120, 32);

// 查询窗口部件状态
assert!(state.contains_widget(id));
assert_eq!(state.kind_of(id), Some(MyKind::Button));
assert!(state.is_kind(id, MyKind::Button));
assert_eq!(state.text(id), "Click Me");

// 修改窗口部件状态
state.set_visible(id, false);
state.set_geometry(id, 10, 20, 200, 40);
state.set_text(id, "Updated");
state.set_enabled(id, false);
state.set_ime_enabled(id, true);
state.set_accessibility_name(id, "Submit button");
```

### 事件队列

`BackendState` 维护菜单、窗口部件触发、剪贴板和拖放事件的内部队列：

```rust
// 菜单事件
state.push_menu_event(item_id);
while let Some(id) = state.pop_menu_event() {
    println!("菜单项 {} 被触发", id);
}

// 带类型的窗口部件触发事件
state.inject_widget_trigger_event(widget_id, WidgetTriggerKind::Clicked);
while let Some(event) = state.pop_widget_trigger_event() {
    match event.kind {
        WidgetTriggerKind::Clicked => { /* 处理点击 */ }
        WidgetTriggerKind::ValueChanged => { /* 处理值变化 */ }
        _ => {}
    }
}

// 剪贴板
state.set_clipboard_text("Hello clipboard");
let text = state.clipboard_text();
```

---

## 4. 运行时后端选择

后端选择在编译时确定，并在运行时自动检测：

### 编译时选择

```rust
// src/platform/runtime.rs — 按目标条件编译

#[cfg(all(target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

#[cfg(all(target_os = "macos", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())  // 分发到 objc2 或 cocoa
}

#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "wayland-native"))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}
```

### 全局单例

平台后端存储在 `OnceLock` 单例中，首次访问时初始化：

```rust
use rust_widgets::platform;

// 初始化、运行、退出
platform::init();
platform::run();
platform::quit();

// 查询能力
let caps = platform::capabilities();

// 获取 DPI 缩放因子
let dpi = platform::dpi_scale_factor();

// 检查运行时 GUI 模式
match platform::runtime_gui_mode() {
    RuntimeGuiMode::NativeInteractive => println!("正在使用原生窗口运行"),
    RuntimeGuiMode::PreviewOrStub => println("正在预览/存根模式下运行"),
}
```

---

## 5. 设备环境检测

`DeviceEnvironment` 提供设备类别、触摸能力、屏幕尺寸、DPI、方向和辅助功能偏好的运行时检测。

```rust
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{DeviceClass, Size};

// 根据屏幕尺寸和 DPI 自动检测
let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);

println!("设备类别:  {:?}", env.device_class);  // Desktop
println!("支持触摸: {}", env.touch_capable);
println!("方向:   {:?}", env.orientation);
println!("DPI 缩放:     {:.1}", env.dpi_scale);

// 触摸目标建议（逻辑像素）
let target = env.min_touch_target();  // Desktop: 32×32, Tablet: 44×44, Mobile: 48×48
println!("最小触摸目标: {}×{}", target.width, target.height);
println!("触摸间距:    {}", env.touch_spacing());

// 布局缩放（投影模式增加 20%）
println!("布局缩放: {:.1}", env.layout_scale());

// 通过屏幕尺寸启发式检测设备类别（无需特性标志）：
//   width < 480     → Mobile
//   width < 1024    → Tablet
//   DPI ≥ 2.0, <1440 → Tablet
//   否则       → Desktop
```

### 设备类别枚举

| 类别 | 触摸目标 | 触摸间距 | 典型用途 |
|-------|:---:|:---:|----------|
| `Desktop` | 32×32 | 8px | 鼠标 + 键盘 |
| `Tablet` | 44×44 | 12px | 触摸优先的大屏幕 |
| `Mobile` | 48×48 | 16px | 单手触摸 |
| `Embedded` | 40×40 | 10px | 专用硬件 |
| `Projector` | 24×24 | 6px | 遥控导航 |

### 辅助功能偏好

```rust
let mut env = DeviceEnvironment::default();
env.set_high_contrast(true);
env.set_reduced_motion(true);
env.set_font_scale(1.5);  // 限制在 [0.5, 3.0] 范围内
```

---

## 6. 剪贴板系统

### `RichClipboardBackend` 特质

每个平台可以实现对文本、HTML、RTF、图像和文件列表的丰富剪贴板支持：

```rust
use rust_widgets::platform::clipboard::{
    RichClipboardBackend, ClipboardContent, MockClipboard,
};

// 使用 MockClipboard 进行测试
let clip = MockClipboard::new();

// 设置纯文本
clip.set_contents(ClipboardContent::Text("Hello".into()));

// 设置带纯文本回退的 HTML
clip.set_contents(ClipboardContent::Html {
    html: "<b>bold</b>".into(),
    plain: "bold".into(),
});

// 检查格式支持
assert!(clip.has_format("text/plain"));
assert!(!clip.has_format("text/html"));

// 获取内容
if let Some(content) = clip.get_contents() {
    match content {
        ClipboardContent::Text(t) => println!("文本: {}", t),
        ClipboardContent::Html { html, plain } => println!("HTML: {}, 纯文本: {}", html, plain),
        ClipboardContent::Rtf(data) => println!("RTF: {} bytes", data.len()),
        ClipboardContent::Image { width, height, .. } => println!("图像: {}×{}", width, height),
        ClipboardContent::Files(paths) => println!("文件: {:?}", paths),
    }
}
```

### 平台剪贴板集成

`Platform` 特质暴露 `clipboard_backend()`，返回 `Option<&dyn RichClipboardBackend>`。桌面平台提供真实的剪贴板集成；嵌入式平台返回 `None`。

```rust
let platform = rust_widgets::platform::get_platform();

// 通过 Platform 特质操作纯文本
platform.set_clipboard_text("已复制的文本");
let text = platform.get_clipboard_text();

// 通过后端操作丰富内容
if let Some(backend) = platform.clipboard_backend() {
    backend.set_clipboard_html("<h1>标题</h1>", "标题");
    backend.set_clipboard_image(&rgba_data, 64, 64);
}
```

---

## 7. 拖放

```rust
use rust_widgets::platform::types::DropEvent;

// 从源窗口部件开始拖拽操作
platform.begin_drag(source_id, "text/plain", b"被拖拽的文本");

// 轮询放置事件
while let Some(event) = platform.poll_drop_event() {
    println!("源:  {}", event.source_widget_id);
    println!("目标:  {}", event.target_widget_id);
    println!("MIME:    {}", event.mime);
    println!("负载: {} 字节", event.payload.len());
}

// 程序化注入（用于测试）
platform.inject_drop_event(DropEvent {
    source_widget_id: 1,
    target_widget_id: 2,
    mime: "text/plain".into(),
    payload: b"test".to_vec(),
});
```

`BackendState` 提供相同的操作：

```rust
state.begin_drag(src_id, "text/plain", payload);
if let Some(event) = state.pop_drop_event() {
    // 处理放置
}
state.inject_drop_event(event);
```

---

## 8. IME 系统

IME 桥接器为东亚语言输入提供输入法编辑器集成。

### `ImeBridge` 特质

```rust
use rust_widgets::platform::ime::{
    ImeBridge, ImeComposition, ImeCandidatePosition, MockImeBridge,
};

let bridge = MockImeBridge::new();

// 窗口部件获得输入焦点
bridge.focus_in(text_edit_id);

// 更新组合预览（预编辑文本）
bridge.set_composition(&ImeComposition {
    text: "nihao".into(),
    cursor_position: 5,
    selection_length: 0,
});

// 提交最终文本
bridge.commit_text("你好");

// 定位候选窗口
bridge.set_candidate_window_position(ImeCandidatePosition { x: 100, y: 200 });

// 窗口部件失去焦点
bridge.focus_out(text_edit_id);

assert_eq!(bridge.focused_widget(), None);
```

### 平台 IME 后端

| 平台 | 实现 | 模块 |
|----------|---------------|--------|
| Linux | IBus 集成 | `platform::ime_linux` |
| macOS | `NSTextInputContext` | `platform::ime_macos` |
| Windows | TSF（文本服务框架） | `platform::ime_windows` |

`Platform` 特质暴露 `ime_bridge() -> Option<&dyn ImeBridge>`：

```rust
let platform = rust_widgets::platform::get_platform();
if let Some(bridge) = platform.ime_bridge() {
    if bridge.is_active() {
        bridge.focus_in(widget_id);
    }
}
```

---

## 9. 无障碍

### `A11yTree` — 跨平台无障碍节点树

无障碍系统追踪 28 种语义角色，并支持屏幕阅读器导航。

```rust
use rust_widgets::platform::accessibility::{
    A11yTree, A11yNode, A11yState, A11yRole, A11yProvider,
};

let mut tree = A11yTree::new();

// 注册窗口部件节点
let node = A11yNode::new(
    42,
    A11yState {
        role: A11yRole::Button,
        label: "提交".into(),
        enabled: true,
        ..Default::default()
    },
);
tree.register_node(node);

// 按角色查询
let buttons = tree.find_by_role(A11yRole::Button);
for id in &buttons {
    if let Some(node) = tree.get(*id) {
        println!("找到按钮: {}", node.state.label);
    }
}

// 焦点导航
tree.focus_next();
tree.focus_previous();

// 动态查询
let query_results = tree.query(|node| {
    node.state.role == A11yRole::Button && node.state.enabled
});
```

### A11yRole 枚举（28 种角色）

`Unknown` • `Button` • `Label` • `TextField` • `CheckBox` • `RadioButton` • `Slider` • `ProgressBar` • `List` • `Table` • `Image` • `Link` • `Heading` • `Paragraph` • `Group` • `Window` • `Dialog` • `Menu` • `MenuItem` • `Tab` • `Switch` • `Alert` • `ComboBox` • `SpinButton` • `StatusBar` • `ToolTip` • `Tree`

角色自动映射到平台特定的角色：`NSAccessibilityRole`（macOS）、UIA 控件类型（Windows）和 AT-SPI 角色（Linux）。

### `A11yProvider` 特质

```rust
pub trait A11yProvider {
    fn register_widget(&mut self, id: ObjectId, role: A11yRole, label: &str);
    fn unregister_widget(&mut self, id: ObjectId);
    fn update_widget_state(&mut self, id: ObjectId, state: A11yState);
    fn announce(&self, message: &str);
    fn focus_next(&mut self) -> Option<ObjectId>;
    fn focus_previous(&mut self) -> Option<ObjectId>;
    fn tree(&self) -> &A11yTree;
    fn tree_mut(&mut self) -> &mut A11yTree;
}
```

### `AccessibilityBridge` 特质（平台层级）

```rust
pub trait AccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str);
    fn accessibility_name(&self, id: ObjectId) -> String;
    fn notify_name_changed(&self, id: ObjectId);
    fn notify_value_changed(&self, id: ObjectId);
    fn notify_state_changed(&self, id: ObjectId);
    fn notify_focus_changed(&self, id: ObjectId);
    fn set_aria_properties(&self, id: ObjectId, properties: AriaProperties);
}
```

将焦点管理与无障碍关联：

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// 焦点更改现在转发到平台无障碍桥接器
```

### 平台无障碍模块

| 平台 | 模块 | 桥接器 |
|----------|--------|--------|
| macOS | `platform::accessibility::macos` | NSAccessibility |
| Windows | `platform::accessibility::windows` | UIAutomation |
| Linux | `platform::accessibility::linux` | AT-SPI（通过 zbus） |

---

## 10. 菜单系统

```rust
use rust_widgets::platform::get_platform;

let platform = get_platform();

// 创建附加到窗口的菜单栏
let menu_bar = platform.create_menu_bar(window_id, 0, 0, 800, 24);
platform.attach_menu_bar_to_window(window_id, menu_bar);

// 创建子菜单
let file_menu = platform.create_menu(menu_bar, "文件", 0, 0, 60, 24);

// 添加菜单项
let new_id = platform.menu_add_item(file_menu, "新建", Some("Ctrl+N"));
let open_id = platform.menu_add_item(file_menu, "打开...", Some("Ctrl+O"));
platform.menu_add_item(file_menu, "保存", Some("Ctrl+S"));

// 轮询菜单触发事件
while let Some(triggered_id) = platform.poll_menu_triggered() {
    if triggered_id == new_id {
        println!("新建文件");
    } else if triggered_id == open_id {
        println!("打开文件");
    }
}

// 程序化注入（用于测试）
platform.inject_menu_trigger(new_id);

// 轮询带类型的窗口部件触发事件
while let Some(trigger) = platform.poll_widget_trigger_event() {
    match trigger.kind {
        WidgetTriggerKind::Clicked => { /* 处理点击 */ }
        WidgetTriggerKind::ValueChanged => { /* 处理值变化 */ }
        WidgetTriggerKind::SelectionChanged => { /* 处理选择变化 */ }
        WidgetTriggerKind::Closed => { /* 处理关闭 */ }
        WidgetTriggerKind::Unknown => { /* 回退 */ }
    }
}
```

### `WidgetTriggerKind` 枚举

| 变体 | 值 | 描述 |
|---------|:---:|-------------|
| `Unknown` | 0 | 无具体触发语义 |
| `Clicked` | 1 | 主要激活（按钮点击、复选框切换） |
| `ValueChanged` | 2 | 有状态值更改（行编辑、滑块） |
| `SelectionChanged` | 3 | 当前选择更新（组合框/列表/树/表格） |
| `Closed` | 4 | 窗口部件/窗口关闭生命周期触发 |

---

## 11. 能力协商

`CapabilityContract` 系统在原生桌面配置文件和受限的嵌入式配置文件之间协商运行时能力。

### `PlatformCapabilities` 标志

```rust
pub struct PlatformCapabilities {
    pub dpi_scaling: bool,           // 高 DPI 支持
    pub ime: bool,                   // IME 集成
    pub accessibility: bool,         // 无障碍桥接
    pub native_menu: bool,           // 原生菜单支持
    pub typed_widget_trigger: bool,  // 带类型的窗口部件事件
}
```

### `NativeCapabilityContract`

用于桌面运行时（Windows、macOS、Linux）：

| 字段 | 描述 |
|-------|-------------|
| `dpi_scaling` | 支持 DPI 感知的几何和文本 |
| `ime` | 输入法编辑器支持 |
| `accessibility` | 屏幕阅读器桥接 |
| `native_menu` | 平台原生菜单栏 |
| `typed_widget_trigger` | 带类型的触发事件 |

### `EmbeddedCapabilityContract`

用于嵌入式/受限运行时：

| 字段 | 描述 |
|-------|-------------|
| `fixed_dpi` | 固定 DPI 缩放因子（1.0） |
| `low_memory_mode` | 预期低内存行为 |
| `typed_widget_trigger` | 带类型的触发事件 |

### 协商

```rust
use rust_widgets::platform::{negotiate_capability_contract, CapabilityContract};
use rust_widgets::core::RuntimeProfile;

let contract = negotiate_capability_contract(RuntimeProfile::Full);
match contract {
    CapabilityContract::Native(native) => {
        println!("DPI 缩放:   {}", native.dpi_scaling);
        println!("IME:           {}", native.ime);
        println!("无障碍: {}", native.accessibility);
        println!("原生菜单:  {}", native.native_menu);
    }
    CapabilityContract::Embedded(embedded) => {
        println!("固定 DPI:       {}", embedded.fixed_dpi);
        println!("低内存模式: {}", embedded.low_memory_mode);
    }
}
```

当平台后端未发布协约时，会提供回退协约——确保在所有环境中都有确定性行为。

---

## 12. 虚拟键盘（移动端）

`VirtualKeyboard` 控制器管理屏幕键盘的生命周期和布局适配，用于基于触摸的文本输入。

```rust
use rust_widgets::platform::virtual_keyboard::{
    VirtualKeyboard, KeyboardNotch, KeyboardState,
};
use rust_widgets::core::Rect;

let mut vkb = VirtualKeyboard::new();

// 为获得焦点的文本字段请求键盘
vkb.request_show(
    text_field_id,
    Rect::new(0, 700, 200, 40),  // 窗口部件在屏幕坐标中的矩形
    800,                           // 屏幕高度
    KeyboardNotch::new(300),       // 键盘覆盖高度
);

// 检查状态
assert_eq!(vkb.state(), KeyboardState::Showing);
assert!(vkb.is_keyboard_active());

// 过渡到可见
vkb.on_shown();

// 应用布局偏移以保持窗口部件可见
let mut widget_rect = Rect::new(10, 200, 100, 30);
vkb.apply_layout_shift(&mut widget_rect);
// 如果 widget_rect.y 会被键盘覆盖，现在向上偏移

// 隐藏键盘
vkb.request_hide();
vkb.on_hidden();
assert_eq!(vkb.state(), KeyboardState::Hidden);

// 重置所有状态（例如，窗口停用时）
vkb.reset();
```

### 状态机

```
Hidden → (request_show) → Showing → (on_shown) → Visible
                                                      ↓
Hidden ← (on_hidden) ← Hiding ← (request_hide) ←─────┘
```

---

## 13. 平台特定后端概览

### Linux

```rust
// 自动检测 Wayland 与 X11/GTK
#[cfg(all(target_os = "linux", feature = "wayland-native"))]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}
```

### macOS（objc2 桥接）

`macos_objc2` 模块提供现代的 Objective-C 桥接。`SelectedMacOSPlatform` 根据特性标志分发到适当的后端。

### Windows

`WindowsPlatform` 提供完整的 Win32 API 集成，包括原生窗口、剪贴板、拖放以及通过 UIAutomation 实现的无障碍支持。

### 移动端（iOS / Android）

状态驱动后端（`IosMobilePlatform`、Android JNI 桥接）使用 `BackendState<K>` 进行窗口部件管理。Android JNI 桥接暴露用于视图创建的原生方法。

```rust
#[cfg(feature = "mobile-api")]
rust_widgets::platform::mobile_attach_to_native_view(native_handle);
let name = rust_widgets::platform::mobile_backend_name();
```

### WASM / 嵌入式

两者都使用基于 `BackendState` 的状态管理。嵌入式目标通过 `mini` 特性标志支持 `no_std`，使用 arena 分配的集合。

---

## 14. 跨平台模式

### 特性门控的平台代码

```rust
#[cfg(target_os = "linux")]
fn platform_specific_setup() { /* GTK 初始化 */ }

#[cfg(target_os = "macos")]
fn platform_specific_setup() { /* NSApplication 初始化 */ }

#[cfg(target_os = "windows")]
fn platform_specific_setup() { /* CoInitialize */ }
```

### 运行时查询后端身份

```rust
let platform = rust_widgets::platform::get_platform();

match platform.backend_name() {
    "cocoa" | "WindowsPlatform" => {
        // 桌面原生模式
    }
    "wayland" => {
        // Wayland 原生模式
    }
    "gtk" => {
        // GTK 原生模式
    }
    "harmony-desktop" | "android-mobile" | "macos-objc2-preview" => {
        // 预览/存根模式
    }
    _ => {
        // 未知 — 预览模式
    }
}
```

### 将无障碍接入焦点管理器

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// 所有焦点更改现在转发到平台无障碍桥接器
```

### 完整的跨平台初始化

```rust
use rust_widgets::platform;
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{Size, RuntimeProfile};

fn main() {
    let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
    println!("正在 {:?} 设备上运行", env.device_class);

    platform::init();

    let caps = platform::capabilities();
    if caps.ime {
        println!("IME 支持：已启用");
    }

    if let Some(bridge) = platform::get_platform().accessibility_bridge() {
        println!("无障碍桥接：可用");
    }

    let contract = negotiate_capability_contract(RuntimeProfile::Full);
    println!("能力协约：{:?}", contract);

    // ... 创建窗口、窗口部件 ...

    platform::run();
}
```
