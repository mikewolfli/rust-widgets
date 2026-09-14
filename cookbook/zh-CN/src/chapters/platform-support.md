# 平台支持

rust-widgets 在九个支持的平台上提供了统一的 API。本章涵盖平台抽象层、后端选择、设备检测、剪贴板、拖放、IME、无障碍、菜单、能力协商和虚拟键盘支持。

> **请先读这一段 —— 所有控件都是自绘的。**
>
> 后端**不会**创建原生操作系统控件。它只拥有四样东西：**绘制表面**、**事件循环**、
> **输入转换**，以及**平台服务**（IME、剪贴板、无障碍、原生菜单、文件对话框、DPI、壁纸）。
>
> 因此下文所说的“平台支持”指的是*某个平台如何提供这四样东西* —— **而不是**存在哪些控件。
> 控件的可用性是**配置文件**的问题，而不是操作系统的问题；参见 [§1.2](#12-控件可用性按配置文件划分而非按操作系统)。

---

## 1. 九个支持的平台

### 1.1 后端

| # | 平台 | 后端提供 | 特性标志 | 状态 |
|---|------|------|:---:|:---:|
| 1 | **Windows** | Win32 窗口 + 消息循环 | `windows` | ✅ 已验证 |
| 2 | **macOS** | Cocoa/objc2 `NSView` 绘制表面 | `macos` | ✅ 已验证 |
| 3 | **macOS**（预览） | objc2，状态驱动 | `macos` | ✅ 预览 |
| 4 | **Linux (GTK)** | GTK3 窗口 + 事件循环 | `linux-gtk` | ✅ 已验证 |
| 5 | **Linux (Wayland)** | Wayland `wl_surface` + 输入 | `linux-wayland` | ✅ 已验证 |
| 6 | **iOS** | UIKit 绘制表面，状态驱动 | `ios` | ✅ 已验证 |
| 7 | **Android** | JNI 绘制表面，状态驱动 | `android` / `android-jni` | ✅ 已验证 |
| 8 | **HarmonyOS** | NAPI 桥接 | `harmony` | ✅ 预览 |
| 9 | **WASM** | DOM canvas + 浏览器事件 | `wasm` | ✅ 已验证 |
| 10 | **Portable** | 内存帧缓冲，无操作系统 | —（无匹配项时的默认值） | ✅ 已验证 |

在 Linux 上，运行时通过 `$WAYLAND_DISPLAY` 和 `$XDG_SESSION_TYPE` 环境变量自动检测 Wayland 和 X11/GTK。

如果没有任何后端匹配目标平台，就会选中 `portable`：一个背后没有操作系统的内存绘制表面。这是一种受支持的配置，而不是降级方案 —— `mini` 和无宿主的 `embedded` 构建正是这样工作的，而且它与其他任何后端一样容易测试，因为绘制路径中没有任何环节依赖操作系统。

### 1.2 控件可用性按配置文件划分，而非按操作系统

因为所有控件都是自绘的，**同样的 167 种控件在每个操作系统上都能工作**。真正有差异的是
**编译进来多少控件**，而这由*配置文件*决定：

| 配置文件 | 控件种类 | 注册表 | 自绘控件托管 | 渲染器 |
|---------|:-----------:|:--------:|:-----------------------:|----------|
| `desktop` | 167（完整） | ✅ | ✅ | wgpu（GPU） |
| `tablet` | 167（完整） | ✅ | ✅ | wgpu（GPU） |
| `mobile` | 167（完整） | ✅ | ✅ | wgpu（GPU） |
| `embedded` | 精简核心集 | — | — | 软件 |
| `mini` | 精简核心集 | — | — | 软件 |

破折号表示**被编译移除，而非降级**：模块不存在，因此
`supports_custom_widgets()` 返回 `false`，你应当拒绝该操作，
而不是挂载到空白表面上。

**实际影响：** 你在 macOS 上编写并测试的控件，在 Windows、Linux、iOS 和 Web 上会
逐像素渲染出完全一致的结果，而你的代码中不需要任何 `cfg(target_os)`。
只有当你需要后端所拥有的四件事之一时，才需要接触操作系统专有 API。

### 1.3 平台服务确实因操作系统而异

用 `PlatformCapabilities` 查询*宿主*提供什么。切勿假设 —— 一个运行在与其编译目标
不匹配的操作系统上的后端会返回 `false`。

| 操作系统 | DPI 缩放 | IME | 无障碍 | 原生菜单 |
|----|:-----------:|:---:|:-------------:|:-----------:|
| Windows | ✅ | ✅ | ✅ | ✅ |
| macOS | ✅ | ✅ | ✅ | ✅ |
| Linux / GTK | ✅ | ✅ | ✅ | ✅ |
| Linux / Wayland | ✅ | ✅ | ✅ | ❌ |
| iOS | ✅ | ✅ | ✅ | ❌ |
| Android | ✅ | ✅ | ✅ | ❌ |
| HarmonyOS | ✅ | ✅ | ✅ | ❌ |
| WASM | ❌ | ❌ | ❌ | ❌ |
| Portable | ❌ | ❌ | ❌ | ❌ |

Wayland 没有菜单栏协议，因此其后端把菜单树保存在进程内、由宿主负责渲染 ——
在那里声称支持原生菜单将是虚假的。

`native_menu` 这一列很容易被误读，所以值得说明这些取值的来源：
`Platform::capabilities` 的默认值是「若后端报告 `Desktop` 家族则为 `true`」，
只有 Wayland、iOS、Android 与 HarmonyOS 将其覆写为 `false`。这意味着一个桌面家族的
后端如果*忘记*覆写，就会静默地继承 `native_menu: true` —— 默认值是过度声称，
覆写才是诚实。`default_capabilities_for(family)` 暴露了这个默认值，
便于你与后端自身的报告作对照；而上表由测试钉住，不会与源码脱节。

---

## 2. `Platform` 特质 — 通用契约

`Platform` 特质定义了后端必须提供的**六个必需方法** —— 绘制表面、事件循环和生命周期。其余所有方法都有一个诚实的默认实现：当宿主缺少某项能力时，它会报告 `UnsupportedOnWidget` / `None`，而不是报告写入成功却没有真正生效。

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
// src/platform/runtime.rs — 按目标条件编译。
//
// 注意选择后端是*为了*什么：它的绘制表面和事件循环。它从来不是因为
// “它能构建哪些控件”而被选中的，因为每个后端绘制的都是同一套 Rust 自绘控件。

#[cfg(all(target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

#[cfg(all(target_os = "macos", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())  // 分发到 objc2 或 cocoa
}

#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "linux-wayland"))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}

// 没有匹配的操作系统：一个背后没有宿主的内存绘制表面。这是受支持的
// 后端，而不是错误 —— 正是它让 `mini` 可测试。
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(PortablePlatform::new())
}
```

### 控件从何而来

既然后端不再构建控件，请求一个控件就要通过控制后端 / 工厂，而绝不会通过 `Platform`：

```rust
use rust_widgets::widget::WidgetFactory;
use rust_widgets::core::Rect;

let factory = WidgetFactory::new_with_defaults();
let mut button = factory
    .create("button", Rect::new(10, 10, 100, 30), "OK")
    .expect("button is a registered widget kind");

// Properties are read and written through the same contract, whatever the OS.
let label = factory.read_property(button.as_ref(), "text")?;
factory.write_property(button.as_mut(), "text", "Save".into())?;
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

应该优先选用这套通用属性 API。每个控件自己实现 `WidgetProperties`，因此一个存在的属性在每个平台上都以同样的方式可读可写 —— 调用点不需要按操作系统写 `if`/`else`。

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

供具备桌面能力的运行时（Windows、macOS、Linux）使用。

它是 `PlatformCapabilities` 的**类型别名**，而不是一个独立的 struct。它携带同样的五个标志，而且必须如此：否则协商结果与后端自己的报告就可能相互矛盾，新增的能力也可能被加到其中一边而被另一边默默丢弃。保留这个名字是因为它属于协商 API 的词汇（`CapabilityContract::Native(..)`、`Platform::native_capability_contract`）；正因为它是别名，两者可以互换，也不存在任何可能弄错的转换。

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
#[cfg(all(target_os = "linux", feature = "linux-wayland"))]
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

状态驱动后端（`IosMobilePlatform`、Android JNI 桥接）使用 `BackendState<K>` 进行窗口部件管理。Android JNI 桥接暴露用于接入原生视图（绘制表面）的方法。

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
