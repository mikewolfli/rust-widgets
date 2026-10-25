# API 参考

本章提供整个 `rust_widgets` 公共 API 的逐模块完整参考。当你需要为任务查找正确的类型、函数或 trait 时，可将其作为快速查阅手册。

本文档所描述的库版本为 **2.9.2**。代码示例假定已使用 `use rust_widgets::*;` 或按所示使用显式路径。

---

## 目录

1. [顶层函数](#顶层函数)
2. [应用生命周期 (`app`)](#应用生命周期-app)
3. [核心原语 (`core`)](#核心原语-core)
4. [控件系统 (`widget`)](#控件系统-widget)
5. [控件属性 (`WidgetProperties`)](#控件属性)
6. [布局系统 (`layout`)](#布局系统-layout)
7. [事件系统 (`event`)](#事件系统-event)
8. [渲染系统 (`render`)](#渲染系统-render)
9. [渲染引擎 (`render_engine`)](#渲染引擎-render_engine)
10. [样式与主题 (`style`, `theme`)](#样式与主题-style-theme)
11. [平台抽象 (`platform`)](#平台抽象-platform)
12. [错误系统 (`error`)](#错误系统-error)
13. [动作框架 (`action`)](#动作框架-action)
14. [快捷键系统 (`shortcut`)](#快捷键系统-shortcut)
15. [数据绑定 (`data_binding`)](#数据绑定-data_binding)
16. [信号/槽 (`signal`)](#信号槽-signal)
17. [国际化 (`i18n`)](#国际化-i18n)
18. [手势识别 (`gesture`)](#手势识别-gesture)
19. [图表与数据可视化 (`chart`)](#图表与数据可视化-chart)
20. [PDF 生成 (`pdf`)](#pdf-生成-pdf)
21. [打印 (`print`)](#打印-print)
22. [内存管理 (`memory`)](#内存管理-memory)
23. [性能 (`performance`)](#性能-performance)
24. [自适应质量 (`quality`)](#自适应质量-quality)
25. [控制后端 (`control_backend`)](#控制后端-control_backend)
26. [对象系统 (`object`)](#对象系统-object)
27. [Web 能力 (`web`)](#web-能力-web)
28. [撤销/重做 (`undo`)](#撤销重做-undo)
29. [剪贴板 (`clipboard`)](#剪贴板-clipboard)
30. [GPU 加速 (`gpu`, `wgpu_backend`)](#gpu-加速-gpu-wgpu_backend)
31. [嵌入式支持 (`embedded`)](#嵌入式支持-embedded)
32. [语言绑定 (`bindings`)](#语言绑定-bindings)
33. [媒体模块 (`audio`、`video`、`image`)](#媒体模块-audio-video-image)
34. [特性标志参考](#特性标志参考)
35. [错误代码参考](#错误代码参考)
36. [FFI / C ABI 参考](#ffi--c-abi-参考)

---

## 顶层函数

Crate 根 (`rust_widgets`) 公开了一组便利函数，可在不使用 `App` 包装器的情况下快速开发应用程序。这些函数非常适合简单的脚本或 FFI 入口点。

### 生命周期函数

| 函数 | 签名 | 描述 |
|---|---|---|
| `init` | `fn()` | 初始化运行时（自动选择平台后端） |
| `run` | `fn()` | 进入主事件循环（阻塞直到调用 `quit`） |
| `quit` | `fn()` | 向事件循环发出退出信号 |

### 窗口创建

```rust
pub fn create_window(title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
```

### 控件创建

每个函数创建一个控件，返回其 `ObjectId`，并将其添加为 `parent` 的子控件。

| 函数 | 返回 `ObjectId` | 说明 |
|---|---|---|
| `create_button(parent, text, x, y, w, h)` | Button | 标准按钮 |
| `create_checkbox(parent, text, x, y, w, h)` | CheckBox | 切换复选框 |
| `create_line_edit(parent, text, x, y, w, h)` | LineEdit | 单行文本输入 |
| `create_label(parent, text, x, y, w, h)` | Label | 静态文本显示 |
| `create_radio_button(parent, text, x, y, w, h)` | RadioButton | 单选选择 |
| `create_slider(parent, x, y, w, h)` | Slider | 值滑动条 |
| `create_progress_bar(parent, x, y, w, h)` | ProgressBar | 进度指示器 |
| `create_combo_box(parent, x, y, w, h)` | ComboBox | 下拉选择器 |
| `create_list_box(parent, x, y, w, h)` | ListBox | 列表选择 |
| `create_panel(parent, x, y, w, h)` | Panel (GroupBox) | 容器面板 |
| `create_frame(parent, x, y, w, h)` | Frame | 为其子控件绘制边框/框架 |
| `create_dialog(parent, title, x, y, w, h)` | Dialog | 通用的带标题对话框，承载一个内容控件 |
| `create_message_box(parent, title, text, x, y, w, h)` | MessageBox | 模态消息对话框 |
| `create_file_dialog(parent, title, x, y, w, h)` | FileDialog | 文件选择器 |
| `create_color_dialog(parent, title, x, y, w, h)` | ColorDialog | 颜色选择器 |
| `create_font_dialog(parent, title, x, y, w, h)` | FontDialog | 字体选择器 |
| `create_spin_box(parent, x, y, w, h)` | SpinBox | 数字微调控件 |
| `create_list_view(parent, x, y, w, h)` | ListView | 表格风格列表 |
| `create_tree_table(parent, x, y, w, h)` | TreeTable | 树形行 + 表格列 |
| `create_breadcrumb(parent, x, y, w, h)` | Breadcrumb | 分段导航路径 |
| `create_signature_pad(parent, x, y, w, h)` | SignaturePad | 手写签名采集面 |
| `create_drop_zone(parent, x, y, w, h)` | DropZone | 命名的拖放目标 |
| `create_scroll_area(parent, x, y, w, h)` | ScrollArea | 可滚动容器 |

### 控件操作

```rust
pub fn show_widget(id: ObjectId);
pub fn hide_widget(id: ObjectId);
pub fn set_widget_geometry(id: ObjectId, x: i32, y: i32, w: u32, h: u32);
pub fn set_widget_text(id: ObjectId, text: &str) -> String;
pub fn get_widget_text(id: ObjectId) -> String;
pub fn set_widget_enabled(id: ObjectId, enabled: bool);
pub fn is_widget_enabled(id: ObjectId) -> bool;
pub fn set_widget_visible(id: ObjectId, visible: bool);
pub fn is_widget_visible(id: ObjectId) -> bool;
```

### Combo Box 操作

```rust
pub fn combo_box_add_item(id: ObjectId, text: &str);
pub fn combo_box_clear_items(id: ObjectId);
pub fn combo_box_set_current_index(id: ObjectId, index: u32);
pub fn combo_box_current_index(id: ObjectId) -> i32;
pub fn combo_box_item_count(id: ObjectId) -> u32;
pub fn combo_box_item_text(id: ObjectId, index: u32) -> String;
```

### List Box 操作

```rust
pub fn list_box_add_item(id: ObjectId, text: &str);
pub fn list_box_remove_item(id: ObjectId, index: u32);
pub fn list_box_clear_items(id: ObjectId);
pub fn list_box_set_current_index(id: ObjectId, index: u32);
pub fn list_box_current_index(id: ObjectId) -> i32;
pub fn list_box_item_count(id: ObjectId) -> u32;
pub fn list_box_item_text(id: ObjectId, index: u32) -> String;
```

### 事件轮询（轮询 API）

```rust
pub fn poll_widget_triggered() -> Option<ObjectId>;
pub fn poll_widget_trigger_event() -> Option<(ObjectId, u32)>;
pub fn inject_widget_trigger_event(id: ObjectId, kind: u32) -> bool;
```

### 剪贴板

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>;
```

### 菜单/工具栏

```rust
pub fn create_menu_bar(parent: ObjectId, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn create_menu(parent: ObjectId, text: &str, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn attach_menu_bar_to_window(window: ObjectId, menu_bar: ObjectId) -> bool;
pub fn menu_add_item(parent_menu: ObjectId, text: &str, shortcut: &str) -> ObjectId;
pub fn poll_menu_triggered() -> Option<ObjectId>;
pub fn inject_menu_trigger(menu_item_id: ObjectId) -> bool;
pub fn create_tool_bar(parent: ObjectId, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn create_status_bar(parent: ObjectId, text: &str, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
```

### 拖放

```rust
pub fn begin_drag(source: ObjectId, mime_type: &str, payload: &[u8]) -> bool;
pub fn poll_drop_event() -> Option<DropEvent>;
pub fn inject_drop_event(source: ObjectId, target: ObjectId, mime: &str, payload: &[u8]) -> bool;
```

### IME 与无障碍

```rust
pub fn set_widget_ime_enabled(id: ObjectId, enabled: bool);
pub fn is_widget_ime_enabled(id: ObjectId) -> bool;
pub fn platform_ime_bridge() -> Option<ImeBridge>;
pub fn set_widget_accessibility_name(id: ObjectId, name: &str);
pub fn get_widget_accessibility_name(id: ObjectId) -> String;
```

---

## 应用生命周期 (`app`)

`app` 模块是生产应用程序的**首选入口点**。

### 核心类型

```rust
pub struct App { /* ... */ }
pub struct AppConfig {
    pub app_name: String,
    pub enable_i18n: bool,
    // ...
}
```

### App 方法

| 方法 | 签名 | 描述 |
|---|---|---|
| `new` | `fn(config: AppConfig) -> Self` | 使用配置创建应用 |
| `run` | `fn(self)` | 运行事件循环 |
| `window` | `fn(&self) -> &WindowHandle` | 获取主窗口句柄 |
| `quit` | `fn(&self)` | 退出应用 |

### 控件句柄类型

每个句柄封装一个 `ObjectId` 并公开类型安全的操作。

| 句柄 | 控件类型 | 关键操作（除 WidgetHandle 外） |
|---|---|---|
| `WidgetHandle` | (基础 trait) | `raw_id()`, `from_raw()`, `show()`, `hide()`, `set_geometry()`, `set_text()`, `text()`, `enable()`, `disable()`, `is_enabled()`, `set_visible()`, `is_visible()`, `on_click()`, `on_value_changed()` |
| `WindowHandle` | Window | `set_title()`, `title()`, `resize()`, `minimize()`, `maximize()`, `restore()`, `close()` |
| `ButtonHandle` | Button | `set_text()`, `text()` — 继承 `WidgetHandle` |
| `LabelHandle` | Label | `set_text()`, `text()` |
| `LineEditHandle` | LineEdit | `set_text()`, `text()`, `set_echo_mode()`, `echo_mode()` |
| `CheckBoxHandle` | CheckBox | `set_checked()`, `is_checked()`, `check_state()`, `set_check_state()` |
| `RadioButtonHandle` | RadioButton | `set_checked()`, `is_checked()` |
| `SliderHandle` | Slider | `set_value()`, `value()`, `set_range()`, `range()` |
| `ProgressBarHandle` | ProgressBar | `set_value()`, `value()`, `set_range()`, `range()` |
| `ComboBoxHandle` | ComboBox | `add_item()`, `clear()`, `set_current_index()`, `current_index()`, `item_count()`, `item_text()` |
| `ListBoxHandle` | ListBox | `add_item()`, `remove_item()`, `clear()`, `set_current_index()`, `current_index()`, `item_count()` |
| `SpinBoxHandle` | SpinBox | `set_value()`, `value()`, `set_range()`, `set_step()` |
| `ScrollAreaHandle` | ScrollArea | `set_widget()`, `ensure_visible()`, `scroll_to()` |
| `ScrollBarHandle` | ScrollBar | `set_value()`, `value()`, `set_range()` |
| `PanelHandle` | Panel | `set_layout()` |
| `ListViewHandle` | ListView | `set_model()`, `model()` |
| `TabWidgetHandle` | TabWidget | `add_tab()`, `remove_tab()`, `set_current_index()`, `current_index()` |
| `TextEditHandle` | TextEdit | `set_text()`, `text()`, `append()`, `clear()` |
| `WebViewHandle` | WebView | `load_url()`, `url()`, `reload()`, `go_back()`, `go_forward()` |
| `MessageBoxHandle` | MessageBox | `set_title()`, `set_text()`, `add_button()` |
| `DialogHandle` | Dialog | `open()`, `close()`, `result()` |
| `FrameHandle` | Frame | 通用框架容器 |
| `GridWidgetHandle` | GridWidget | 网格专用操作 |

### 支持类型

```rust
pub enum CheckState { Unchecked, Checked, PartiallyChecked }
pub enum EchoMode { Normal, Password, NoEcho }
pub enum SelectionMode { Single, Multi, Extended, None }

pub trait ListModel {
    fn row_count(&self) -> usize;
    fn text(&self, row: usize, col: usize) -> String;
    fn set_text(&mut self, row: usize, col: usize, text: &str);
}
```

### 事件循环装配

`EventLoop` 要能投递任何事件，必须先具备三个钩子：控件分发函数、父级查找与快捷键分发器。只安装其中一部分会得到一个「看起来已接线」却静默丢掉某个角色的循环，因此 `app` 提供一次调用装齐整套。

```rust
pub fn assemble_event_loop(event_loop: &mut EventLoop, runtime: &ActionRuntime);

pub fn assemble_event_loop_with(
    event_loop: &mut EventLoop,
    runtime: &ActionRuntime,
    parent_lookup: Option<Box<dyn Fn(ObjectId) -> Option<ObjectId> + Send + Sync>>,
    dispatch_fn: Option<EventDispatchFn>,
);
```

在构造循环之后、启动之前调用一次：线程化循环在创建线程时捕获这些闭包。`assemble_event_loop_with` 在对应参数为 `None` 时保留默认的父级查找与分发函数，并始终从 `runtime` 取得快捷键钩子。需要不同快捷键策略的宿主可在调用返回后再安装自己的实现。

---

## 核心原语 (`core`)

### 几何类型

```rust
pub type ObjectId = u64;

pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

`Point` 构造函数：`new(x, y)`, `origin()`, 以及 `from_f32()`, `from_u32()`,
`from_i64()`, `from_f64()`, `from_usize()`, `from_isize()` 及其 `_tuple`
变体。算术：`Add<(i32, i32)>`。

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

`Size` 构造函数：`new(w, h)`, 以及 `from_f32()`, `from_i32()`, `from_i64()`,
`from_f64()`, `from_usize()`, `from_isize()` 和 `_tuple` 变体。
方法：`is_empty()`, `area()`, `aspect_ratio()`。

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

| 方法 | 签名 | 描述 |
|---|---|---|
| `new` | `(x, y, w, h)` | 创建矩形 |
| `from_position_size` | `(Point, Size)` | 从位置+大小创建 |
| `position()` / `size()` | — | 分解 |
| `contains_point(p)` | `-> bool` | 点命中测试 |
| `intersects(r)` | `-> bool` | 重叠测试 |
| `contains_rect(r)` | `-> bool` | 完全包含 |
| `union(r)` | `-> Rect` | 包围盒并集 |
| `intersection(r)` | `-> Rect` | 重叠交集 |
| `expand_to_touch_target(min)` | `-> Rect` | 扩展到最小触摸尺寸 |
| `center()` | `-> Point` | 矩形中心 |
| `right()` / `bottom()` | `-> i32` | 边缘坐标 |
| `with_padding(p)` / `with_margin(m)` | `-> Rect` | 内缩/外扩 |
| `shrink(amount)` / `grow(amount)` | `-> Rect` | 均匀内缩/外扩 |
| `clamp_point(p)` | `-> Point` | 将点限制在内部 |
| `extend_to_include(p)` | `-> Rect` | 扩展以包含点 |
| `area()` | `-> u32` | 宽 × 高 |

```rust
pub enum Orientation { Horizontal, Vertical }
```

### Color

```rust
pub struct Color {
    pub r: u8,  // 0-255
    pub g: u8
,  // 0-255
    pub b: u8,  // 0-255
    pub a: u8,  // 0-255
}
```

| 方法 | 描述 |
|---|---|
| `rgba(r, g, b, a)` | 从 0-255 值创建 |
| `rgb(r, g, b)` | 不透明颜色（alpha = 255） |
| `from_rgba_u32(v)` | 打包的 0xRRGGBBAA |
| `from_f32(r, g, b, a)` | 从 0.0-1.0 浮点数创建 |
| `parse_hex(s)` | `"#RGB"`, `"#RGBA"`, `"#RRGGBB"` 或 `"#RRGGBBAA"` |
| `to_hex_rgb()` / `to_hex_rgba()` | 序列化 |
| `with_alpha(a)` | 不同 alpha 的新颜色 |
| `blend(other)` | Alpha 合成 |
| `luminance()` | 感知亮度 |
| `is_dark()` / `is_light()` | 亮度分类 |
| `contrast_color()` | 返回黑色或白色以确保可读性 |
| `invert()` | RGB 反转 |

预定义常量（55+）：`BLACK`, `WHITE`, `RED`, `GREEN`, `BLUE`, `YELLOW`,
`CYAN`, `MAGENTA`, `GRAY`, `LIGHT_GRAY`, `DARK_GRAY`, `TRANSPARENT`,
语义颜色（`PRIMARY`, `SECONDARY`, `SUCCESS`, `WARNING`, `ERROR`,
`BACKGROUND`, `FOREGROUND`, `LINK`, `BORDER`, `DIVIDER`, `SELECTION`,
`TOOLTIP`, `INFO`, `NOTIFICATION`, `DISABLED_BACKGROUND`,
`DISABLED_FOREGROUND`），以及扩展的网页颜色（`ALICE_BLUE`, `BEIGE`,
`CORAL`, `GOLD`, `INDIGO`, `MAROON`, `NAVY`, `OLIVE`, `ORANGE`, `PINK`,
`PURPLE`, `TEAL`, `SKY_BLUE`, `STEEL_BLUE` 等）。

### 对齐

```rust
pub enum Alignment { Left, Center, Right, Top, Bottom }
pub enum HorizontalAlignment { Left, Center, Right }
pub enum VerticalAlignment { Top, Center, Bottom }
```

每种都支持 `parse_str()`, `as_str()`, `is_*()` 查询方法，以及通过 `from_alignment()` 在水平/垂直之间转换。

### 字体

```rust
pub struct Font {
    pub family: String,
    pub size: f32,
    pub weight: u32,    // 100-900 (CSS 权重比例)
    pub bold: bool,
    pub italic: bool,
}
```

### 核心枚举与结构体

```rust
pub enum RuntimeProfile { Full, Embedded }
pub enum DeviceClass { Desktop, Tablet, Mobile, Embedded, Projector }
pub enum PlatformFamily { Desktop, Embedded, Mobile, Tablet, Projector }

pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

`Version` 方法：`new()`, `from_u32()`, `to_u32()`, `parse_str()`,
`is_compatible_with()`, `is_newer_than()`, `is_older_than()`。

```rust
pub struct PlatformCapabilities {
    pub has_gpu: bool,
    pub has_touch: bool,
    pub has_keyboard: bool,
    pub has_mouse: bool,
    pub screen_width: u32,
    pub screen_height: u32,
    pub dpi_scale: f32,
}
```

工厂方法：`desktop()`, `embedded()`, `mobile()`。

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### 核心 Traits

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

### 坐标工具

`coords` 模块提供坐标系转换：

```rust
pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32;
pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32;
pub fn to_pdf_y(screen_y: f32, page_height: f32) -> f32;
```

`rect_merge` 模块提供：

```rust
pub fn merge_intersecting_rects(rects: &[Rect]) -> Vec<Rect>;
pub fn bounding_rect(rects: &[Rect]) -> Option<Rect>;
```

`MutexExt` 扩展 trait 添加了毒锁恢复：

```rust
pub trait MutexExt<T> {
    fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R;
}
```

---

## 控件系统 (`widget`)

### 核心 Traits

```rust
pub trait Widget: EventHandler + Any {
    // 必须实现
    fn base(&self) -> &BaseWidget;
    fn base_mut(&mut self) -> &mut BaseWidget;

    // 身份标识
    fn id(&self) -> ObjectId;
    fn kind(&self) -> WidgetKind;

    // 几何（10+ 方法）
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    fn position(&self) -> Point;
    fn size(&self) -> Size;
    fn min_size(&self) -> Option<Size>;
    fn max_size(&self) -> Option<Size>;
    fn set_min_size(&mut self, ...);
    fn set_max_size(&mut self, ...);

    // 状态（10+ 方法）
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn has_focus(&self) -> bool;

    // 层级
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn children(&self) -> Vec<ObjectId>;
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);

    // 信号
    fn triggered(&self) -> &Signal<ObjectId>;
    fn value_changed(&self) -> &Signal<String>;

    // 样式
    fn style(&self) -> &WidgetStyle;
    fn style_mut(&mut self) -> &mut WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
    fn css_class(&self) -> &[String];

    // DPI / 布局
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, scale: f32);
    fn layout_scale(&self) -> f32;

    // 工具提示
    fn set_tooltip(&mut self, text: String);
    fn tooltip(&self) -> Option<&str>;
}

pub trait Draw {
    fn draw(&self, ctx: &mut RenderContext);
}
```

`BaseWidget` 实现了所有共享状态和 60+ 默认方法。

### 控件种类枚举

`WidgetKind` 枚举系统中的每种控件类型（设备档位下共 200 种）。选定的变体：

- `Button`, `CheckBox`, `RadioButton`, `Label`
- `LineEdit`, `TextArea`, `ComboBox`, `ListBox`, `SpinBox`, `Dropdown`
- `Slider`, `ProgressBar`, `ScrollBar`, `Spinner`, `Meter`, `Arc`, `Roller`
- `ImageView`, `MiniCanvas`, `MiniChart`, `Line`, `LCDNumber`
- `GroupBox`, `Frame`, `ScrollArea`, `Splitter`, `TabWidget`, `StackedWidget`
- `CollapsiblePane`, `DockWidget`, `MdiArea`, `ToolBox`
- `Window`
- `ToggleButton`, `Switch`
- `Calendar`, `DateEdit`, `TimeEdit`, `DateTimeEdit`, `Dial`
- `KeySequenceEdit`, `PieMenu`, `RibbonBar`, `TabBar`
- `Menu`, `MenuBar`, `StatusBar`, `ToolBar`, `ToolButton`
- `Dialog`, `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`, `InputDialog`
- `ProgressDialog`, `PopupWindow`
- `ListView`, `TreeView`, `TreeTable`, `Table`, `DataGrid`, `VirtualList`, `VirtualTable`
- `Breadcrumb`, `SignaturePad`, `DropZone`
- `WebEngineView`, `WebEnginePage`
- （完整列表见控件系统章节）

### 控件分类

**基础控件**（始终可用）：

| 控件 | 类型别名 | 模块 |
|---|---|---|
| `Button` | — | `base_widgets::button` |
| `CheckBox` | — | `base_widgets::checkbox` |
| `Label` | — | `base_widgets::label` |
| `RadioButton` | — | `base_widgets::radiobutton` |
| `ToggleButton` | — | `base_widgets::toggle_button` *(非 mini)* |

输入类型：

| 控件 | 类型别名 | 特性 |
|---|---|---|
| `LineEdit` | — | EchoMode 支持 |
| `TextArea` | — | 多行 |
| `ComboBox` | — | 下拉选择 |
| `Dropdown` | — | — |
| `ListBox` | — | SelectionMode |
| `SpinBox` | — | 数字微调 |
| `Keyboard` | — | 虚拟键盘 |
| `TextEdit` | — | *(非 mini)* |
| `RichEdit` | — | *(非 mini)* |
| `CommandLink` | — | *(非 mini)* |
| `FontComboBox` | — | *(非 mini)* |

显示类型：

| 控件 | 描述 |
|---|---|
| `Arc` | 弧/圆环显示 |
| `ImageView` | 图像查看器 |
| `Line` | 水平/垂直分隔线 |
| `Meter` | 仪表盘 |
| `MiniCanvas` | 自定义绘制表面 |
| `MiniChart` | 内联迷你图表 |
| `ProgressBar` | 进度条 |
| `Roller` | 滚轮显示 |
| `ScrollBar` | 滚动条 |
| `Slider` | 值滑动条 |
| `Spinner` | 活动旋转指示器 |
| `LcdNumber` | LCD 风格显示 *(非 mini)* |

容器类型：

| 控件 | 描述 | 特性 |
|---|---|---|
| `GroupBox`（又名 `Panel`） | 分组容器 | 始终可用 |
| `ScrollArea` | 可滚动视口 | 始终可用 |
| `CollapsiblePane` | 可折叠节 | *(非 mini)* |
| `DockWidget`（又名 `DockPanel`） | 可停靠面板 | *(非 mini)* |
| `MdiArea` | MDI 容器 | *(非 mini)* |
| `Splitter` | 可调整大小分割器 | *(非 mini)* |
| `StackedWidget` | 堆叠/页面 | *(非 mini)* |
| `TabWidget` | 选项卡容器 | *(非 mini)* |
| `ToolBox` | 工具箱容器 | *(非 mini)* |

对话框类型 *（全部非 mini）*：

| 控件 | 描述 |
|---|---|
| `Dialog` | 通用的带标题对话框，承载一个内容控件；默认模态 |
| `MessageBox` | 模态消息对话框 |
| `FileDialog`（又名 `DirectoryDialog`） | 文件/目录选择器 |
| `ColorDialog` | 颜色选择器 |
| `FontDialog` | 字体选择器 |
| `InputDialog` | 输入提示对话框 |
| `ProgressDialog` | 进度模态框 |
| `PopupWindow` | 仅含边框的弹出层（标题栏 + 内容区） |

菜单/工具栏类型 *（全部非 mini）*：

| 控件 | 描述 |
|---|---|
| `MenuBar` | 顶层菜单栏 |
| `Menu`（又名 `ContextMenu`） | 下拉菜单 |
| `ToolBar` | 工具栏 |
| `StatusBar` | 状态栏 |
| `ToolButton` | 工具栏按钮 |
| `Action` | 动作/命令 |

视图类型 *（全部非 mini）*：

| 控件 | 描述 |
|---|---|
| `ListView` | 带模型的列表 |
| `TableWidget` | 表格视图 |
| `DataGrid` | 可过滤数据网格 |
| `TreeView`（又名 `ColumnView`） | 树形视图 |
| `VirtualList`（又名 `DataView`） | 虚拟化列表 |
| `VirtualTable` | 虚拟化表格 |
| `TreeTable` | 树表混合 |

### 控件分类 — 重新分类组织

来自原 `new_widgets` 模块的控件已重新归类到专用子目录中：

| 目录 | 控件 |
|---|---|
| `nav_widgets/` | `AdaptiveScaffold`, `AppBar`, `BottomNavigationBar`, `NavigationDrawer`, `NavigationStack`, `TabView` |
| `chart_widgets/` | `BarChart`, `LineChart`, `PieChart`, `Sparkline` + 共享的图表绘图引擎（`charts`, `layout`, `svg`, `types`, `adapter`） |
| `media_widgets/` | `AnimatedImage`, `AudioVisualizer`, `CameraPreview`, `HeroAnimation`, `LottieWidget`, `RiveWidget`, `VideoPlayer` |
| `overlay_widgets/` | `FAB`, `PullToRefresh`, `RefreshControl`, `SwipeToDismiss` |
| `cupertino/` | `CupertinoAlertDialog`, `CupertinoDatePicker`, `CupertinoNavigationBar`, `CupertinoSegmentedControl`, `CupertinoSlider`, `CupertinoSwitch`, `MaterialNavigationRail`, `MaterialSnackbar` |
| `misc_widgets/` | `Avatar`, `BarcodeScanner`, `BezierCurveEditor`, `DateRangePicker`, `DropZone`, `MobileDatePicker`, `QRCode`, `SegmentedButton` |
| `input_widgets/`（扩展） | `AutoCompleteEdit`, `EditableComboBox`, `ImePreedit`, `InplaceEditor`, `MaskedEdit`, `MultiSelectComboBox`, `RangeSlider`, `SearchBar`, `SearchBox`, `ShortcutEditor`, `TagInput` |
| `display_widgets/`（扩展） | `Badge`, `ColorHistory`, `ColorWell`, `Divider`, `EmptyState`, `FloatingLabel`, `FontPreview`, `Icon`, `ProgressCircle`, `Rating`, `SkeletonLoader`, `Switch` |
| `container_widgets/`（扩展） | `Carousel`, `MasonryLayout`, `SafeArea`, `Stepper` |
| `dialog/`（扩展） | `BottomSheet`, `Dialog`, `FindReplaceDialog`, `ModalBottomSheet`, `Popover`, `Tooltip`, `WizardDialog` |
| `menu_toolbar/`（扩展） | `DropdownMenu`, `MenuButton` |
| `special_widgets/`（扩展） | `Breadcrumb`, `SignaturePad`, `TimelineWidget` |
| `view_widgets/`（扩展） | `ImageGallery`, `PropertiesPanel`, `PropertyGrid`, `TreeTable` |

### 伴生控件 *(非 mini)*

伴生控件（companion）是主控件的伙伴：每个控件只承担主控件旁边的一块界面 —— 按钮的菜单面板、媒体播放器的队列列表 —— 并把每一次用户意图表达为一个信号。它们不在能力属性表中，因此通过下表所列的模块访问，而不是通过 `WidgetFactory`。

| 伴生控件 | 模块 | 用途 |
|---|---|---|
| `MenuPanel` | `companions` | 锚定的下拉行列表；发出 `item_clicked`、`highlighted_changed`、`cancelled` |
| `SpeedDial` | `companions` | `FAB` 旁可展开的次要动作栈 |
| `SwipeActions` | `companions` | 滑动行的多动作背景，分为 `leading` 与 `trailing` 两侧 |
| `DrawerSectionHeader` | `companions` | 为抽屉条目分组的不可选中标题行 |
| `LayerPanel` | `companions` | `Canvas` 图层的只读镜像，以图层 id 寻址 |
| `PlaylistPanel` | `companions` | `MediaPlayer` 的队列列表；信号中的索引是完整列表中的位置 |
| `StickyHeader` | `container_widgets::sticky_header` | `ScrollArea` 粘性带承载的内容；通过 `ScrollArea::set_sticky_header` 挂载 |
| `TagGroup` | `display_widgets::tag_group` | 布局并选择真实 `Tag` 子控件的容器 |
| `Chip` | `input_widgets::input_companions` | 单个可移除、带拖拽状态的 chip；行容器是 `TagGroup`，多条目胶囊控件是 `special_widgets` 中另一个 `Chip` |
| `StepButtons` | `input_widgets::input_companions` | 与数字字段配对的增减按钮 |
| `Scrollbar` | `input_widgets::input_companions` | 编辑器使用的滚动指示条；与 `ScrollBar` 控件不同 |
| `FormatToolbar` | `input_widgets::input_companions` | 富文本编辑器的加粗/斜体/……动作集 |
| `SuggestionPopup` | `input_widgets::suggestion_popup` | 组合框与补全字段的候选列表，两阶段导航 |
| `ValidationBubble` | `input_widgets::validation_bubble` | 锚定在字段旁的校验消息，携带严重级别 |
| `RecentValuesPopup` | `input_widgets::recent_values_popup` | 去重、容量受限的已提交值历史 |
| `ChartCrosshair` | `companions` | 图表指针下的参考线与坐标轴标签 |
| `GridFindBar` | `view_widgets::grid_find_bar` | 跨表格列的增量查找 |
| `ColumnChooser` | `view_widgets::column_chooser` | 显示/隐藏并重排表格列 |
| `ThumbnailCache` | `view_widgets::thumbnail_cache` | 有界 LRU 的降采样位图缓存；本身不绘制任何内容 |
| `ModalHost` | `dialog::modal_host` | 决定哪个模态在最上层、关闭请求路由到何处的模态栈 |
| `MediaTransportBar` | `media_widgets::media_transport` | 媒体控件的播放/暂停、进度、音量与全屏栏 |
| `MediaScrubber` | `media_widgets::media_scrubber` | 带缓冲段与章节标记的进度轨道 |
| `WebFindBar` | `web_widgets::find_bar` | Web 视图的查找栏 |
| `SubtitleBar` | `media_widgets::subtitle_bar` | 自动换行的多行字幕带 |
| `ValueSpinBox` | `macos::value_spinbox` | 在同一控件内提供可编辑数字字段与步进箭头 |
| `PopupItemList` | `macos::popup_item_list` | 在某一点打开的小型带边框条目列表 |
| `CupertinoNavigationStack` | `cupertino::navigation_stack` | 与 `CupertinoNavigationBar` 配对的标题栈；发出 `popped` 与 `stack_changed` |

### Web 控件 *(非 mini)*

| 控件 | 描述 |
|---|---|
| `WebView` | 嵌入式网页浏览器视图 |
| `WebEngine` | 用于渲染的 Web 引擎 |

关联类型：`WebEngineContextMenuRequest`, `WebEngineCookieStore`,
`WebEngineDownloadItem`, `WebEngineFindTextResult`, `WebEngineNotification`,
`WebEnginePage`, `WebEngineScriptDialog`, `WebEngineSettings`,
`WebEngineWebChannel`。

### 高级控件 *(非 mini)*

| 控件 | 描述 |
|---|---|
| `Calendar` | 日期日历选择器 |
| `DateEdit`（又名 `DatePicker`） | 日期编辑 |
| `TimeEdit`（又名 `TimePicker`） | 时间编辑 |
| `DateTimeEdit`（又名 `DateTimePicker`） | 日期时间编辑 |
| `Dial` | 旋转拨号 |
| `KeySequenceEdit` | 键盘快捷键录制 |
| `PieMenu` | 径向/圆形菜单 |
| `RibbonBar` | 功能区风格工具栏 |

### 特殊控件 *(非 mini)*

| 控件 | 描述 |
|---|---|
| `Canvas` | 自由绘制表面 |
| `ChartWidget` | 图表显示控件 |
| `CodeEditor` | 源代码编辑器 |
| `ColorPicker` | 颜色选择控件 |
| `CommandEntry` | 命令输入 |
| `CommandPalette` | 命令面板叠加 |
| `DiffViewer` | 并排差异查看器 |
| `GanttWidget` | 甘特图 |
| `GridWidget` | 网格显示 |
| `MapView` | 地图显示 |
| `MarkdownEditor` | Markdown 编辑 |
| `MediaPlayer` | 媒体播放器控件 |
| `NotificationCenter` | 通知中心 |
| `SegmentedControl` | 分段按钮组 |
| `SplitButton` | 拆分操作按钮 |
| `TerminalView` | 终端模拟器 |
| `TimelineWidget` | 时间线显示 |

### 控件能力系统

```rust
impl WidgetFactory {
    pub fn create(&self, kind_or_name: &str, geometry: Rect, text: &str) -> Option<Box<dyn Widget>>;
    pub fn capability(&self, kind_or_name: &str) -> Option<&WidgetCapability>;
    pub fn read_property(&self, widget: &dyn Widget, name: &str) -> Result<CapabilityValue, CapabilityAccessError>;
    pub fn write_property(&self, widget: &mut dyn Widget, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError>;
}

pub struct WidgetCapability {
    pub kind: WidgetKind,
    pub properties: &'static [PropertySchema],
    pub events: &'static [EventSchema],
    pub commands: &'static [&'static str],
}

pub enum CapabilityValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Color(Color),
    Rect(Rect),
    /// 复合载荷：元组、列表或混合组；各分量保留各自的类型。
    Tuple(Vec<CapabilityValue>),
    /// 有序且以 id 为键的记录列表——集合属性。见「控件系统」章「集合属性」一节。
    Records(RecordList),
}

pub struct PropertySchema {
    pub name: &'static str,
    pub value_kind: PropertyValueKind,
    pub readable: bool,
    pub writable: bool,
    pub accepted_tokens: &'static [&'static str],
    pub constraints: &'static [Constraint],
    pub record_schema: Option<&'static RecordSchema>,
}

pub enum PropertyValueKind {
    Bool, Int, UInt, Float,
    Number,          // 整数或小数，由另一个属性决定
    String, Enum, Color, Rect,
    Records,         // 形状由 `PropertySchema::record_schema` 声明
}

pub enum CapabilityAccessError {
    UnknownWidget,
    UnknownProperty,
    ReadOnlyProperty,
    TypeMismatch,
    UnsupportedOnWidget,
    OutOfRange,
    UnknownCommand,
}
```

### 控件状态持久化 *(2.9.1)*

`SaveState` 是「会话恢复」的选择性加入部分：控件声明哪些字段属于用户状态、用哪个格式版本，框架只负责搬运字节。不会自动序列化任何东西；键由宿主决定 —— `ObjectId` 在运行时分配，无法跨重启存活。

```rust
use rust_widgets::{collect_save_state, restore_save_state, SavedState};

// 由宿主为关心的实例命名。
let key_of = |id| {
    if id == splitter_id { Some("workspace.split".to_string()) }
    else if id == tabs_id { Some("workspace.tabs".to_string()) }
    else { None }
};

let saved: Vec<SavedState> = collect_save_state(window_id, key_of);
// ……把 `saved` 存进你自己的设置存储……
let applied = restore_save_state(window_id, &saved, key_of);
```

遍历会同时覆盖控件树子节点与已挂载 surface，因此用 `mount_surface` 托管的控件不会被遗漏。条目按 key 排序，并携带控件的格式版本；版本不匹配会被拒绝而不是误读，损坏的字节不会改动控件。

已接入的控件：`Splitter`（分栏布局）、`TabWidget`（选中页）、`DockWidget` 与 `CollapsiblePane`（既有 blob）、`Pagination`（每页条数与当前页）。控件通过实现 `SaveState` 并在 `Widget::save_state_dyn` / `save_state_dyn_mut` 返回 `Some(self)` 来加入。

`Pagination` 另新增两个可选单元：`set_show_jump_input(true)` 增加「Go」跳页输入（输入数字后回车），`set_show_page_size_selector(true)` 配合 `set_page_size_options` 增加每页条数菜单。两者默认关闭。

`BarChart::set_animate_value_changes(true)` 会把数据更新从屏幕上的当前值缓动到新值，而不是直接跳变；`false` 保持原有的立即更新行为。

### 控件渲染（离屏）（`widget::runtime`）

一个窗口的画面由两部分组成：控件树，**以及**挂载在它下面的每个面。`Platform::mount_surface` 给挂载的控件一个自己的原生视图，而不是把它变成树子节点，平台单独绘制这个视图——所以只画树会在该控件所在处留下一个空洞。下面这几个入口让测试、文档构建或 `examples/` 二进制不必打开窗口就能得到真实画面：

```rust
use rust_widgets::core::{Color, Size};
use rust_widgets::widget::runtime;

// 平台实际呈现的整个窗口：先树，再叠上每个挂载面。
let frame = runtime::render_window_frame(window_id, Size::new(1180, 760), Color::WHITE);

// 只画树，不叠任何面。
let tree = runtime::render_frame_tree(window_id, Size::new(1180, 760), Color::WHITE);

// 单个控件；epoch 未变时复用缓存帧，即面视图所用的那条路径。
let control = runtime::render_frame_cached(control_id, Size::new(320, 200), Color::TRANSPARENT);

// 这个窗口下有哪些控件被挂载为面。
let surfaces = runtime::surfaces_of(window_id);
```

三个渲染调用都返回 `Option<Vec<u8>>`：id 未挂载或任一维度为 `0` 时是 `None`，否则是按行存储的直通（非预乘）RGBA8，每行 `width * 4` 字节。`render_window_frame` 把每个面自己的帧按该控件的 `geometry()` 合成到树上，因此没有面覆盖的区域与树帧逐字节一致。面可能在自己的矩形里留空（透明），所以这里是**合成**而不是拷贝。

要「像截图一样的画面」用 `render_window_frame`；只有窗口没有挂载任何面时 `render_frame_tree` 才是完整的。两者都必须在 UI 线程调用，`render_frame_cached` 的帧会留在帧缓存里直到下一个 epoch。

### 控件子类型

| 类型 | 形式 |
|---|---|
| `ToggleButtonState` | `struct { checked: bool }` |
| `ButtonState` | `struct { pressed: bool, hovered: bool }` |
| `CheckState` | `enum { Unchecked, Checked, PartiallyChecked }` |
| `EchoMode` | `enum { Normal, Password, NoEcho }` |
| `SelectionMode` | `enum { Single, Multi, Extended, None }` |
| `LineOrientation` | `enum { Horizontal, Vertical }` |
| `RangeSliderOrientation` | `enum { Horizontal, Vertical }` |

---

## 控件属性

自 2.0.0 起，每个控件都会公开自己的属性契约。你可以在不知道控件具体类型的情况下读取、写入并**枚举**它的状态 —— 同一段代码对任何控件、在任何平台上都适用。

### Trait

```rust
pub trait WidgetProperties {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError>;
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError>;

    /// Names this control exposes, including the four it inherits.
    /// A `&'static [&'static str]`, so enumeration allocates nothing.
    fn property_names(&self) -> &'static [&'static str];
}
```

### 四个共享属性

每个控件都通过 `base_property_get` / `base_property_set` 响应这些属性：

```rust
pub const BASE_PROPERTY_NAMES: &[&str] = &["enabled", "visible", "tooltip", "geometry"];
```

`geometry` 在这个契约中刻意是**只读**的：控件的矩形归放置它的布局所有，在这里写入它会在下一轮布局中与布局静默地相互冲突。要移动控件，请使用 `widget::runtime::set_geometry`。

### 值类型

```rust
pub enum CapabilityValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Null,
}
```

**没有**任何 `From` 转换可以生成 `CapabilityValue` —— 请显式构造变体（`CapabilityValue::Int(7)`）。这是刻意为之：隐式转换会让类型不匹配在调用点变得不可见。

### 按引用读取

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::WidgetFactory;
use rust_widgets::CapabilityValue;

let factory = WidgetFactory::new_with_defaults();
let mut slider = factory.create("slider", Rect::new(0, 0, 200, 24), "").unwrap();

factory.write_property(slider.as_mut(), "value", CapabilityValue::Int(42))?;
let value = factory.read_property(slider.as_ref(), "value")?;
assert_eq!(value, CapabilityValue::Int(42));
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

### 枚举契约

这是用于构建属性编辑器、序列化器或文档表格的 API：

```rust
use rust_widgets::widget::{widget_property_get, widget_property_names};

for name in widget_property_names(slider.as_ref()).unwrap_or(&[]) {
    println!("{name} = {:?}", widget_property_get(slider.as_ref(), name)?);
}
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

由于这个列表来自控件自身，它不会过期；如果某个控件公开了一个它并不会响应的属性，测试（`no_published_property_answers_unknown_when_written`）会按名称失败。

### 按 id 读取

后端和脚本宿主通常持有的是 id 而不是引用。这些访问器会通过控件运行时来解析 id，因此**控件必须先完成注册**，而且 `runtime::register` 会**分配**它将要响应的 id —— 请使用它的返回值，而不是工厂分发的那个：

```rust
use rust_widgets::widget::{read_widget_property_by_id, write_widget_property_by_id};
use rust_widgets::widget::runtime;

let id = runtime::register(widget).expect("must run on the UI thread");
write_widget_property_by_id(id, "value", CapabilityValue::Int(7))?;
let value = read_widget_property_by_id(id, "value")?;

runtime::unregister(id);
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

### 错误语义

| 错误 | 含义 |
|---|---|
| `UnknownProperty` | 该控件**没有这个名称的属性** —— 这是调用方的 bug。 |
| `ReadOnlyProperty` | 该属性**存在**但不可写（例如 `geometry`、`row_count`）。请渲染为禁用状态的字段。 |
| `TypeMismatch` | 值的类型错误，或值超出范围。 |
| `UnsupportedOnWidget` | 该控件完全没有属性契约。在 2.0.0 中不应出现。 |
| `UnknownWidget` | 该 id 没有指向任何对象（仅按 id 访问的接口）。 |

### 为自己的控件添加契约

```rust
impl WidgetProperties for MyControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "progress" => Ok(CapabilityValue::Float(self.progress())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "progress" => {
                self.set_progress(expect_f64(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["progress", BASE_PROPERTY_NAMES]
    }
}
```

并在它的 `impl Widget` 块中，紧挨着 `impl_draw_bridge!();`：

```rust
impl_widget_property_hooks!();
```

契约必须满足的规则（每一条都有测试强制保证）：

1. **`property_names()` 中的每个名称都必须能被 `get` 读取。**
2. `property_names()` 必须以 `BASE_PROPERTY_NAMES` 结尾。
3. **没有 setter 的名称必须返回 `ReadOnlyProperty`，绝不能返回 `UnknownProperty`。**
   `UnknownProperty` 意味着“这个名称不存在”。
4. 通过控件自身的访问器读取真实状态，绝不要返回硬编码的常量。

---

## 布局系统 (`layout`)

### 核心 Trait

```rust
pub trait Layout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    fn remove_widget(&mut self, widget_id: ObjectId);
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn update_from_position_size(&self, position: Point, size: Size, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn child_ids(&self) -> Vec<ObjectId>;
    fn has_child(&self, id: ObjectId) -> bool;
    fn clear(&mut self);
    fn update_with_context(&self, rect: Rect, context: &LayoutContext, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

### 布局类型

| 布局 | 模块 | 描述 |
|---|---|---|
| `BoxLayout` | `box_layout` | 水平或垂直盒子 — 按拉伸因子比例分配空间 |
| `HBoxLayout` | `box_layout` | `BoxLayout` 使用 `Orientation::Horizontal`（命名便利） |
| `VBoxLayout` | `box_layout` | `BoxLayout` 使用 `Orientation::Vertical`（命名便利） |
| `GridLayout` | `grid` | 行/列网格，使用 `set_widget(row, col, id)` |
| `BorderLayout` | `border` | 固定边缘轨道（`north`/`south`/`west`/`east`）加弹性中心区 |
| `StackLayout` | `stack` | 一次只显示一个子项（卡片堆叠），使用 `set_current_index()` |
| `FlowLayout` | `flow` | 自动换行流动（类似 CSS flex-wrap） |
| `FlexLayout` | `flex` | CSS flexbox 风格布局 |
| `FormLayout` | `form` | 标签-字段对布局 |
| `SplitterLayout` | `splitter` | 可调整大小的分割面板 |
| `AbsoluteLayout` | `absolute` | 自由形式定位 |
| `CenterLayout` | `center` | 居中单个子项 |
| `AspectRatioLayout` | `aspect_ratio` | 保持宽高比 |
| `WrapLayout` | `wrap` | 换行水平/垂直 |
| `UniformGridLayout` | `uniform_grid` | 固定单元格 |
| `KeyboardAwareLayout` | `keyboard_aware` | 适应虚拟键盘 |

### 支持类型

```rust
pub enum SizePolicy { Fixed, Preferred, Expanding }

pub struct LayoutConstraints {
    pub min: u32,
    pub max: Option<u32>,
}

pub struct LayoutContext {
    pub layout_scale: f32,
    pub font_scale: f32,
    pub min_touch_size: Size,
}
```

---

## 事件系统 (`event`)

### 事件类型

```rust
pub enum Event {
    // 鼠标
    MouseDown((Point, u32)),
    MouseUp((Point, u32)),
    MouseMove { pos: Point },
    MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 },
    MouseDoubleClick { pos: Point, button: u32 },
    MouseEnter { pos: Point },
    MouseLeave { pos: Point },
    MouseMoveLegacy((Point, u32)),
    Wheel { delta: Point, modifiers: u32 },

    // 键盘
    KeyDown((u32, u32)),
    KeyUp((u32, u32)),
    KeyPress { key: u32, modifiers: u32 },
    KeyRelease { key: u32, modifiers: u32 },

    // 焦点
    FocusGained,
    FocusLost,

    // 窗口
    Paint,
    Resize { size: Size },
    OrientationChanged { orientation: ScreenOrientation },
    Quit,

    // 定时器
    Timer { id: u32 },

    // 自定义
    Custom { name: String, payload: Vec<u8> },

    // 触摸（feature = "touch"）
    TouchBegin { pos: Point, touch_id: TouchId },
    TouchEnd { pos: Point, touch_id: TouchId },
    TouchMove { pos: Point, touch_id: TouchId },
    Tap { pos: Point },
    DoubleTap { pos: Point },
    LongPress { pos: Point },
    Swipe { start: Point, end: Point, velocity: f32 },
    Pinch { scale: f32, center: Point },
    Rotate { angle: f32, center: Point },
}
```

### 核心类型

```rust
pub type TouchId = u64;

pub enum EventPriority { High, Normal, Idle }
pub enum GestureClass { Single, Multi, Holographic }
pub enum ScreenOrientation { Portrait, Landscape, ReversePortrait, ReverseLandscape }

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}
```

### 事件循环

```rust
pub struct EventLoop {
    // ...
}

impl EventLoop {
    pub fn new() -> Self;
    pub fn run(&mut self) -> !;
    pub fn quit(&self);
    pub fn post_event(&self, event: Event);
    // Timers are not an `EventLoop` method: `TimerManager` owns them and posts
    // ticks into the loop's queue. See the Timer section below.
}
```

### 事件队列

```rust
pub struct EventQueue { /* ... */ }
pub struct EventSender { /* ... */ }

impl EventQueue {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, event: Event);
    pub fn pop(&mut self) -> Option<Event>;
    pub fn sender(&self) -> EventSender;
    pub fn clear(&mut self);
}

impl EventSender {
    pub fn send(&self, event: Event) -> Result<(), Event>;
}
```

### 焦点管理器

```rust
pub struct FocusManager { /* ... */ }

impl FocusManager {
    pub fn new() -> Self;
    pub fn focused_widget(&self) -> Option<ObjectId>;
    pub fn set_focus(&mut self, id: ObjectId) -> bool;
    pub fn clear_focus(&mut self);
    pub fn focus_next(&mut self) -> Option<ObjectId>;
    pub fn focus_previous(&mut self) -> Option<ObjectId>;
    /// Replaces the focus traversal order wholesale.
    pub fn set_focus_order(&mut self, ids: Vec<ObjectId>);
    pub fn register_focusable(&mut self, id: ObjectId);
    pub fn unregister_focusable(&mut self, id: ObjectId);
    pub fn focusable_widgets(&self) -> &[ObjectId];
    pub fn set_traversal_strategy(&mut self, strategy: FocusTraversalStrategy);
    pub fn set_a11y_callback(&mut self, callback: Box<dyn FnMut(ObjectId)>);
}
```

### 指针捕获

```rust
pub struct PointerCaptureManager { /* ... */ }

impl PointerCaptureManager {
    pub fn new() -> Self;
    /// Captures the pointer for `widget_id`. `false` when another widget already
    /// holds the capture -- capture is exclusive.
    pub fn set_capture(&mut self, widget_id: ObjectId) -> bool;
    /// Releases the capture. `false` if nothing was captured.
    pub fn release_capture(&mut self) -> bool;
    /// The widget holding the capture, if any.
    pub fn capturing_widget(&self) -> Option<ObjectId>;
    /// Whether `widget_id` is the current captor.
    pub fn has_capture(&self, widget_id: ObjectId) -> bool;
}
```

### 定时器

```rust
pub struct TimerManager { /* ... */ }

impl TimerManager {
    /// Takes the sender the timer callbacks are delivered through: the manager
    /// does not own a loop, it posts into one.
    pub fn new(sender: EventSender) -> Self;
    /// Starts a timer on `target`; the tick is delivered as an event.
    pub fn start_timer(&self, target: ObjectId, interval: Duration, id: u32, repeat: bool) -> bool;
    /// Stops one timer on `target`. `false` if no such timer was running.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool;
    /// Stops every timer belonging to `target`; returns how many were stopped.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize;
    /// Stops every timer on every target.
    pub fn clear(&self);
    /// Delivers ticks for all timers whose deadline has passed. Call once per
    /// loop iteration.
    pub fn pump(&self);
}
```

### 队列工具

```rust
pub struct EventQueue { /* ... */ }
impl EventQueue {
    pub fn new() -> Self;
    pub fn sender(&self) -> EventSender;
    pub fn pop(&self) -> Option<Event>;
}

impl EventSender {
    pub fn send(&self, event: Event);
}

pub enum QueueError { Full, Empty, Closed }
```

> `FixedSizeQueue` / `PriorityQueue` / `BoundedQueue` 及 `DEFAULT_QUEUE_CAPACITY`
> 已删除（零消费者，与 `EventQueue` 重复）。`event::queue::BlockingQueue` 是
> `view` 层内部使用，不属于公共契约。

### 动画帧请求

```rust
pub struct AnimationFrameRequest { /* ... */ }
impl AnimationFrameRequest {
    pub fn new(callback: Box<dyn FnMut()>) -> Self;
    pub fn request(&mut self);
    pub fn cancel(&mut self);
}
```

---

## 渲染系统 (`render`)

### 核心渲染类型

```rust
pub enum BlendMode { Normal, Multiply, Screen, Overlay, Additive }

pub enum RenderCommand {
    Noop,
    Clear(Color),
    DrawRect { rect: Rect, color: Color, width: u32 },
    FillRect { rect: Rect, color: Color },
    DrawRoundedRect { rect: Rect, radius: u32, color: Color, width: u32 },
    FillRoundedRect { rect: Rect, radius: u32, color: Color },
    DrawCircle { cx: i32, cy: i32, radius: u32, color: Color, width: u32 },
    FillCircle { cx: i32, cy: i32, radius: u32, color: Color },
    DrawText { text: String, x: i32, y: i32, color: Color, font_size: f32 },
    DrawLine { x1: i32, y1: i32, x2: i32, y2: i32, color: Color, width: u32 },
    DrawImage { data: Vec<u8>, rect: Rect },
    PushClip { x: i32, y: i32, width: u32, height: u32 },
    PopClip,
    DrawGradient { rect: Rect, gradient: Gradient },
    DrawShadow { rect: Rect, shadow: Shadow },
    DrawPath { commands: Vec<PathCommand>, color: Color, width: u32 },
    FillPath { commands: Vec<PathCommand>, color: Color },
    DrawArc { rect: Rect, start_angle: f32, span_angle: f32, color: Color, width: u32 },
    FillArc { rect: Rect, start_angle: f32, span_angle: f32, color: Color },
}
```

### Paint 后端 Trait

```rust
pub trait PaintBackend {
    fn begin_frame(&mut self, clear_color: Color);
    fn end_frame(&mut self);
    fn draw_pixels(&mut self, x: i32, y: i32, width: u32, height: u32, pixels: &[u8]);
    fn size(&self) -> Size;
    fn dpi_scale(&self) -> f32;
}
```

### 渲染上下文

```rust
pub struct RenderContext<'a> {
    // 封装 &mut dyn PaintBackend
}

impl RenderContext<'_> {
    pub fn new(backend: &mut dyn PaintBackend) -> Self;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self);
    pub fn draw_text(&mut self, text: &str, x: i32, y: i32, font_size: f32, color: Color);
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color, width: u32);
    pub fn draw_rect(&mut self, rect: Rect, color: Color, width: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: Color);
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color, width: u32);
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color);
    pub fn draw_image(&mut self, data: &[u8], rect: Rect);
    pub fn push_clip(&mut self, x: i32, y: i32, w: u32, h: u32);
    pub fn pop_clip(&mut self);
    pub fn execute(&mut self, cmd: &RenderCommand);
}
```

### 后缓冲

```rust
pub struct BackBuffer { /* ... */ }
impl BackBuffer {
    pub fn new(size: Size) -> Self;
    pub fn clear(&mut self, color: Color);
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color>;
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color);
    pub fn pixels(&self) -> &[u8];
    pub fn size(&self) -> Size;
    pub fn present(&self, target: &mut dyn PaintBackend, x: i32, y: i32);
    pub fn resize(&mut self, new_size: Size);
}
```

### 软件表面

```rust
pub struct SoftwareSurface { /* ... */ }
impl SoftwareSurface {
    pub fn new(size: Size) -> Self;
    pub fn context(&mut self) -> RenderContext;
    pub fn present(&mut self) -> &[u8];
    pub fn resize(&mut self, new_size: Size);
    pub fn size(&self) -> Size;
    pub fn set_size(&mut self, size: Size);
}
```

### 场景与批处理

```rust
pub enum SceneLayer { Background, Content, Foreground, Overlay, Tooltip }
pub struct RenderScene { /* ... */ }
pub struct BatchId(u32);
pub struct BatchCommand { /* ... */ }
pub struct BatchRenderer { /* ... */ }
```

### 自动渲染后端

```rust
pub struct AutoRenderBackend(SoftwarePaintBackend);
impl AutoRenderBackend {
    pub fn new(size: Size, dpi: f32) -> Self;
}

pub fn default_software_render_config() -> SoftwareRenderConfig;
pub fn set_default_software_render_config(config: SoftwareRenderConfig);
pub fn last_auto_render_backend() -> Option<AutoRenderBackend>;
```

### 文本塑形

```rust
pub trait TextShaper {
    fn shape(&self, text: &str, font: &Font, dpi_scale: f32) -> ShapedText;
}

pub struct SimpleTextShaper;
impl TextShaper for SimpleTextShaper { /* ... */ }

pub struct ShapedText { /* ... */ }
pub struct ShapedGlyphRun { /* ... */ }
pub struct TextMetrics { /* ... */ }
pub struct TextCluster { /* ... */ }
```

### 富文本

```rust
pub struct RichText { /* ... */ }
pub struct TextSpan {
    pub text: String,
    pub style: TextStyle,
}
pub struct TextStyle {
    pub font: Font,
    pub color: Color,
    pub background: Option<Color>,
}
```

### 文本溢出

文本截断是渲染后端的能力（`RenderContext::draw_text_fitted`），不是独立函数。
`TextOverflow` / `TextClamp` / `apply_text_overflow` / `apply_text_clamp` 已删除
（零消费者，与 `fit_text_to_width` 重复）。

### 字素聚类

```rust
/// 遍历文本的聚类（“用户感知的一个字符”），回调得到内容与字节范围。
pub fn for_each_cluster(text: &str, visit: impl FnMut(&str, (usize, usize)));
```

`GraphemeCluster` / `GraphemeProcessor` 已删除（与 `for_each_cluster` 重复）。
`is_combining_mark` / `is_variation_selector` 保留在 `render::grapheme`，供切分与
SVG 后端共享。

### 路径几何

```rust
/// 解析 SVG path data（`d`）为曲线段。支持 `M/L/H/V/C/Q/A/Z` 及全部相对形式。
/// 畸形输入返回 `Err`，不 panic。展平由 `render::text::raster` 负责，此处不重复实现。
pub fn parse(input: &str) -> Result<Vec<Subpath>, PathError>;

pub struct Point { pub x: f32, pub y: f32 }
pub enum Segment {
    Line(Point),
    Quad { ctrl: Point, to: Point },
    Cubic { ctrl1: Point, ctrl2: Point, to: Point },
    Arc { rx: f32, ry: f32, x_rotation: f32, large_arc: bool, sweep: bool, to: Point },
}
pub struct Subpath { pub start: Point, pub segments: Vec<Segment>, pub closed: bool }
pub enum PathError { UnknownCommand(char), ExpectedNumber, SegmentsBeforeMove, TooManyPoints }
pub const MAX_PATH_POINTS: usize = 1024;
```

### SVG 渲染

```rust
pub struct SvgPaintBackend { /* ... */ }
impl SvgPaintBackend {
    pub fn new(size: Size) -> Self;
}
```

### 质量

```rust
pub fn current_fps() -> f64;
pub fn average_frame_time() -> f64;
pub fn current_quality_level() -> QualityLevel;
pub fn set_quality_level(level: QualityLevel);
```

### GPU 渲染（feature = `gpu-wgpu`）

```rust
pub struct GpuRenderer { /* ... */ }
impl GpuRenderer {
    pub fn new() -> Result<Self, ()>;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self) -> Result<(), ()>;
    pub fn submit(&mut self, commands: &[RenderCommand]);
    pub fn resize(&mut self, size: Size);
}

pub enum GpuCapability { Software, Basic, Medium, High }
```

### 投影（feature = `projection`）

```rust
pub struct PresentationController { /* ... */ }
pub struct ProjectionRenderConfig { /* ... */ }
pub struct ProjectionLayoutHelper { /* ... */ }
```

---

## 渲染引擎 (`render_engine`)

```rust
// The trait is `RenderEngine` (src/render_engine/engine_trait.rs). It borrows
// `&self`, not `&mut self`, and takes no `Result`: engine lifecycle is
// infallible by design, so there is nothing for a caller to handle.
pub trait RenderEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn profile(&self) -> RuntimeProfile;
    fn init(&self);
    fn run(&self);
    fn quit(&self);
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
    fn create_button(
        &self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32,
    ) -> u64;
}
```

### 原生引擎

```rust
pub struct NativeRenderEngine;

impl NativeRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for NativeRenderEngine { /* ... */ }
```

### 嵌入式引擎

```rust
pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for EmbeddedRenderEngine { /* ... */ }

// The engine itself is a thin façade: the counters and the frame loop live in the
// process-wide embedded runtime (`render_engine::embedded`), which is what the
// engine methods delegate to. Task submission is a free function, not a method:
// `submit_embedded_task(label, callback)`.

/// Builds the engine for the compile-time profile.
///
/// `NativeRenderEngine` when an OS host exists, `EmbeddedRenderEngine` otherwise
/// (and always under the alloc-frugal `mini` profile).
pub fn default_render_engine() -> Box<dyn RenderEngine>;
```

---

## 样式与主题 (`style`, `theme`)

### 样式类型

```rust
pub struct WidgetStyle {
    pub background_color: Option<Color>,
    pub background_gradient: Option<Gradient>,
    pub text_color: Option<Color>,
    pub font: Option<Font>,
    pub border_color: Option<Color>,
    pub border_width: u32,
    pub border_radius: u32,
    pub corner_radii: Option<CornerRadii>,
    pub padding: Padding,
    pub margin: Margin,
    pub shadow: Option<Shadow>,
    pub touch_target: Option<Size>,
    pub opacity: Option<f32>,
}
```

Builder 方法：`with_background(c)`, `with_text_color(c)`, `with_font(f)`,
`with_border(c, w, r)`, `with_corner_radii(r)`, `with_padding(p)`, `with_margin(m)`,
`with_shadow(s)`, `with_touch_target(s)`, `with_gradient(g)`, `with_opacity(o)`。

实例方法：`inherit_from(parent)`, `merge(other)`, `resolved_corner_radii()`。

### 内边距与外边距

```rust
pub struct Padding { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
pub struct Margin  { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
```

两者都支持：`new(top, right, bottom, left)`, `all(value)`, `symmetric(v, h)`,
`normalized(top, right, bottom, left)` — 负值会被钳制为 0。

### 圆角

```rust
pub struct CornerRadii {
    pub top_left: u32,
    pub top_right: u32,
    pub bottom_right: u32,
    pub bottom_left: u32,
}

impl CornerRadii {
    pub const fn new(top_left: u32, top_right: u32, bottom_right: u32, bottom_left: u32) -> Self;
    pub const fn all(radius: u32) -> Self;
    pub const fn is_uniform(&self) -> bool;
}
```

`WidgetStyle::border_radius` 是标量写法，含义仍然是「四角相同」。当
`WidgetStyle::corner_radii` 也被设置时，逐角值优先；请通过
`WidgetStyle::resolved_corner_radii` 读取解析后的形状，而不是直接读任一字段。
`CornerRadii::new` 的参数顺序与 CSS `border-radius` 一致：左上、右上、右下、左下。

### 阴影

```rust
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub blur: u32,
    pub color: Color,
}
```

Builder：`new()`, `with_offset(x, y)`, `with_blur(b)`, `with_color(c)`。

### 触摸目标

```rust
pub enum TouchTargetSize { Desktop, Tablet, Phone, Embedded, Projection }
impl TouchTargetSize {
    pub fn dimensions(self) -> Size;
    pub fn spacing(self) -> u32;
}
```

### 减少动效

```rust
pub enum ReducedMotionPreference { NoPreference, ReduceMotion }
```

### CSS 与选择器系统

```rust
// CSS 属性解析
pub struct CssParser { /* ... */ }
pub struct Selector { /* ... */ }

// 热重载 CSS 监视器
pub struct AssetWatcher (crate::asset::watcher) { /* ... */ }
impl AssetWatcher (crate::asset::watcher) {
    pub fn watch(path: &str) -> Result<Self, ()>;
}
```

### 渐变

```rust
pub enum Gradient {
    Linear { start: Point, end: Point, colors: Vec<(f32, Color)> },
    Radial { center: Point, radius: f32, colors: Vec<(f32, Color)> },
}
```

### 主题

```rust
pub struct Theme {
    pub name: String,
    pub colors: Colors,
    pub fonts: Fonts,
    pub spacing: Spacing,
    pub borders: Borders,
    pub icons: IconTokens,
    // ...
}

impl Theme {
    pub fn new(name: &str) -> Self;
    pub fn dark() -> Self;                   // 内置深色主题
    pub fn light() -> Self;                  // 内置浅色主题
    pub fn icon_size(&self, slot: IconSlot) -> u32;   // 槽位的标称边长（像素）
}
```

### 主题管理器

```rust
pub struct ThemeManager { /* ... */ }

impl ThemeManager {
    pub fn new() -> Self;
    pub fn set_theme(&mut self, theme: Theme);
    pub fn current_theme(&self) -> Option<&Theme>;
    pub fn register_theme(&mut self, name: &str, theme: Theme) -> bool;
    pub fn icon_size(&self, slot: IconSlot) -> Option<u32>;
}
```

### 主题子类型

```rust
pub struct Colors {
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub border: Color,
    pub highlight: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub disabled: Color,
    // ...
}

pub struct Fonts {
    pub default: Font,
    pub heading: Font,
    pub caption: Font,
    pub monospace: Font,
}

pub struct Spacing {
    pub small: u32,
    pub medium: u32,
    pub large: u32,
    pub extra_large: u32,
}

pub struct Borders {
    pub width: u32,
    pub radius: u32,
    pub shadow: bool,
}

pub struct IconTokens {
    pub small: u32,    // 16 px — 内联与工具栏图标
    pub medium: u32,   // 18 px — 按钮图标
    pub large: u32,    // 32 px — 功能区或空状态图标
}

pub enum IconSlot { Small, Medium, Large }

pub struct PaddingToken {
    pub all: Option<u32>,
    pub vertical: Option<u32>,
    pub horizontal: Option<u32>,
    pub top: Option<u32>,
    pub right: Option<u32>,
    pub bottom: Option<u32>,
    pub left: Option<u32>,
}
```

### 主题样式令牌

```rust
pub struct ThemeStyleToken {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub border: Option<Color>,
    pub border_width: Option<u32>,
    pub radius: Option<u32>,
    pub padding: Option<PaddingToken>,
    pub font: Option<Font>,
    pub opacity: Option<f32>,
    pub shadow: ShadowOverride,
    // ...
}
```

每个字段都是覆盖：`None` 保留基础解析产出的值，因此主题可以只调整某个角色的一个属性而不必重述其余部分。
`ThemeStyleToken::padding` 通过 `PaddingToken::resolve` 按与样式构建器相同的
`边 → 轴 → 全部 → 0` 级联解析，因此 `"button": { "padding": { "all": 4 } }` 只移动该角色。
`PaddingToken` 让主题模式保持在可序列化的原始类型上，而不借用样式层的内边距类型。

### 样式继承链

```
1. 全局主题默认值     (ThemeManager → Theme)
2. 每控件类主题覆盖   (例如 "Button", "Label")
3. 控件实例状态       (StatefulTheme → WidgetState)
4. 内联样式覆盖
```

---

## 平台抽象 (`platform`)

### 运行时函数

```rust
pub fn init();
pub fn run();
pub fn quit();
pub fn get_platform() -> &'static Box<dyn PlatformBackend>;
pub fn capabilities() -> PlatformCapabilities;
pub fn dpi_scale_factor() -> f32;
pub fn runtime_gui_mode() -> RuntimeGuiMode;
pub fn runtime_gui_mode_for(family: PlatformFamily) -> RuntimeGuiMode;
#[cfg(feature = "mobile-api")]
pub fn mobile_attach_to_native_view(native_handle: u64) -> bool;
#[cfg(feature = "mobile-api")]
pub fn mobile_backend_name() -> String;
```

### 平台类型

```rust
pub enum RuntimeGuiMode { Native, Embedded, Headless }

pub struct CapabilityContract { /* ... */ }
pub fn negotiate_capability_contract(profile: u32) -> CapabilityContract;
```

### 无障碍

```rust
pub trait AccessibilityBridge: Send + Sync {
    fn notify_focus_changed(&self, widget_id: ObjectId);
    fn notify_text_changed(&self, widget_id: ObjectId, text: &str);
    fn notify_selection_changed(&self, widget_id: ObjectId);
    fn notify_value_changed(&self, widget_id: ObjectId, value: &str);
}

pub fn wire_focus_manager_to_a11y(fm: &mut FocusManager);
```

### IME（输入法）

```rust
pub trait ImeBridge {
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn commit_text(&self, text: &str);
    fn composition_range(&self) -> Option<(u32, u32)>;
    fn composition_text(&self) -> Option<String>;
}
```

### 虚拟键盘

```rust
pub struct Keyboard (widget::input_widgets::keyboard) { /* ... */ }
impl Keyboard (widget::input_widgets::keyboard) {
    pub fn show(&mut self);
    pub fn hide(&mut self);
    pub fn is_visible(&self) -> bool;
}
```

### 剪贴板（平台）

```rust
pub struct Option<&'static dyn crate::platform::clipboard::RichClipboardBackend> { /* ... */ }
impl Option<&'static dyn crate::platform::clipboard::RichClipboardBackend> {
    pub fn set(&mut self, content: ClipboardContent);
    pub fn get(&self) -> Option<ClipboardContent>;
    pub fn clear(&mut self);
}
```

### 拖放

```rust
pub struct DropEvent {
    pub source: ObjectId,
    pub target: ObjectId,
    pub mime_type: String,
    pub payload: Vec<u8>,
}
pub fn begin_drag(source: ObjectId, mime: &str, payload: &[u8]) -> bool;
pub fn inject_drop_event(source: ObjectId, target: ObjectId, mime: &str, payload: &[u8]) -> bool;
```

---

## 错误系统 (`error`)

### 核心类型

```rust
pub struct RwError {
    pub id: ErrorId,
    pub message: String,
}

impl RwError {
    pub fn new(id: ErrorId, message: impl Into<String>) -> Self;
    pub fn not_implemented(feature: impl Into<String>) -> Self;
    pub fn msg(message: impl Into<String>) -> Self;
    pub fn from_panic(panic_info: &dyn Any) -> Self;
}

impl fmt::Display for RwError;
impl std::error::Error for RwError;

pub type RwResult<T> = Result<T, RwError>;
```

### 错误 ID（对 FFI 稳定）

```rust
pub struct ErrorId(pub i32);

// 通用
pub const SUCCESS: ErrorId = ErrorId(0);
pub const NOT_IMPLEMENTED: ErrorId = ErrorId(1);
pub const UNSUPPORTED_OPERATION: ErrorId = ErrorId(2);
pub const INVALID_ARGUMENT: ErrorId = ErrorId(3);
pub const NULL_POINTER: ErrorId = ErrorId(4);      // 保留
pub const OUT_OF_MEMORY: ErrorId = ErrorId(5);      // 保留
pub const LOCK_POISONED: ErrorId = ErrorId(6);      // 保留

// 控件 (100-199)
pub const WIDGET_BASE_NOT_IMPL: ErrorId = ErrorId(100);   // 保留
pub const WIDGET_NOT_FOUND: ErrorId = ErrorId(101);        // 保留
pub const WIDGET_INVALID_STATE: ErrorId = ErrorId(102);    // 保留
pub const WIDGET_DEPRECATED: ErrorId = ErrorId(103);       // 保留

// 平台 (200-299)
pub const PLATFORM_UNSUPPORTED: ErrorId = ErrorId(200);    // 保留
pub const PLATFORM_INIT_FAILED: ErrorId = ErrorId(201);    // 保留
pub const CLIPBOARD_FAILED: ErrorId = ErrorId(202);         // 保留
pub const DRAG_DROP_FAILED: ErrorId = ErrorId(203);         // 保留

// 渲染 (300-399)
pub const RENDER_CONTEXT_INVALID: ErrorId = ErrorId(300);   // 保留
pub const RENDER_PIPELINE_FAILED: ErrorId = ErrorId(301);   // 保留

// I/O (400-499)
pub const I18N_LOAD_FAILED: ErrorId = ErrorId(400);         // 保留
pub const FILE_NOT_FOUND: ErrorId = ErrorId(401);
```

### 恐慌安全

```rust
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where F: FnOnce() -> T + std::panic::UnwindSafe;

pub fn to_error_id(result: RwResult<()>) -> i32;

pub trait CAbiSafe { /* ... */ }
pub fn c_try_fallback(/* ... */);
```

---

## 动作框架 (`action`)

```rust
pub struct ActionManager { /* ... */ }

impl ActionManager {
    pub fn new() -> Self;
    pub fn register_action(&mut self, id: &str, text: &str) -> bool;
    pub fn unregister_action(&mut self, id: &str) -> bool;
    pub fn action(&self, id: &str) -> Option<&Action>;
    pub fn action_mut(&mut self, id: &str) -> Option<&mut Action>;
    pub fn trigger_action(&mut self, id: &str) -> bool;
    pub fn set_action_enabled(&mut self, id: &str, enabled: bool) -> bool;
    pub fn bind_shortcut(&mut self, shortcut: &str, action_id: &str) -> bool;
    pub fn trigger_shortcut(&mut self, shortcut: &str) -> bool;
    pub fn bind_action_to_button(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bind_action_to_menu(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bind_action_to_toolbar(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bindings_for_host(&self, host: ObjectId) -> Vec<ActionBinding>;
}
```

### Action

```rust
pub struct Action { /* ... */ }

impl Action {
    pub fn new(id: &str, text: &str) -> Self;
    pub fn id(&self) -> &str;
    pub fn text(&self) -> &str;
    pub fn set_text(&mut self, text: &str);
    pub fn set_checkable(&mut self, checkable: bool);
    pub fn is_checkable(&self) -> bool;
    pub fn is_checked(&self) -> bool;
    pub fn set_checked(&mut self, checked: bool);
    pub fn is_enabled(&self) -> bool;
    pub fn set_enabled(&mut self, enabled: bool);
    pub fn shortcut(&self) -> Option<&str>;
    pub fn set_shortcut(&mut self, shortcut: &str);
    pub fn connect_triggered(&self, f: impl FnMut() + 'static) -> ConnectionHandle;
    pub fn connect_toggled(&self, f: impl FnMut(bool) + 'static) -> ConnectionHandle;
    pub fn connect_enabled_changed(&self, f: impl FnMut(bool) + 'static) -> ConnectionHandle;
}
```

### 支持类型

```rust
pub struct ActionBinding {
    pub action_id: String,
    pub host: ObjectId,
    pub kind: ActionHostKind,
}

pub enum ActionHostKind { Button, Menu, ToolBar, Shortcut }

pub struct ActionRouter { /* ... */ }
impl ActionRouter {
    pub fn new() -> Self;
    pub fn route(&self, action_id: &str, kind: ActionHostKind) -> Option<ObjectId>;
    pub fn connect_undo_redo_stack(&mut self, stack: Arc<Mutex<UndoManager>>) -> bool;
}
```

### 撤销 / 重做

`connect_undo_redo` 把两个闭包绑定到平台加速键上。下面的栈类型记录动作**做了什么**，因此宿主不必自己搭建历史。

```rust
pub const DEFAULT_UNDO_LIMIT: usize = 256;
pub const GLOBAL_UNDO_CONTEXT: &str = "";

pub struct Command { /* ... */ }
impl Command {
    pub fn new<U, R>(label: impl Into<String>, undo: U, redo: R) -> Self
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn label(&self) -> &str;
    pub fn is_empty(&self) -> bool;
}

pub struct UndoStack { /* ... */ }
impl UndoStack {
    pub fn new() -> Self;
    pub fn with_limit(limit: usize) -> Self;
    pub fn limit(&self) -> usize;
    pub fn set_limit(&mut self, limit: usize);
    pub fn push(&mut self, command: Command);
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn begin_group(&mut self, label: impl Into<String>);
    pub fn end_group(&mut self) -> bool;
    pub fn is_group_open(&self) -> bool;
    pub fn undo(&mut self) -> Option<String>;
    pub fn redo(&mut self) -> Option<String>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn undo_len(&self) -> usize;
    pub fn redo_len(&self) -> usize;
    pub fn is_dirty(&self) -> bool;
    pub fn mark_saved(&mut self);
    pub fn clear(&mut self);
}

pub struct UndoManager { /* ... */ }
impl UndoManager {
    pub fn new() -> Self;
    pub fn with_limit(limit: usize) -> Self;
    pub fn active_context(&self) -> &str;
    pub fn set_active_context(&mut self, context: impl Into<String>) -> String;
    pub fn stack_for(&self, context: &str) -> Option<&UndoStack>;
    pub fn stack(&self) -> &UndoStack;
    pub fn stack_mut(&mut self) -> &mut UndoStack;
    pub fn contexts(&self) -> Vec<String>;
    pub fn push(&mut self, command: Command);
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn begin_group(&mut self, label: impl Into<String>);
    pub fn end_group(&mut self) -> bool;
    pub fn undo(&mut self) -> Option<String>;
    pub fn redo(&mut self) -> Option<String>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn is_dirty(&self) -> bool;
    pub fn mark_saved(&mut self);
    pub fn clear(&mut self);
}
```

用两个闭包记录一步，然后重放它：

```rust
let mut stack = UndoStack::new();
stack.record("insert_text", move || remove(), move || insert_again());
if stack.can_undo() {
    stack.undo();
}
```

新命令会清空重做栈（历史已分叉）。`UndoStack::begin_group` 与 `UndoStack::end_group`
把它们之间推入的每条命令合成一步，撤销时按相反顺序执行；允许嵌套，且只有最外层一对创建边界。
`UndoManager` 为每个焦点上下文各保留一个栈，并把 `undo` / `redo` 路由到活动栈；
`GLOBAL_UNDO_CONTEXT` 是初始上下文，因此只用一个栈的宿主根本不需要调用 `set_active_context`。
`ActionRouter::connect_undo_redo_stack` 把平台撤销/重做加速键绑定到活动栈。

---

## 快捷键系统 (`shortcut`)

```rust
pub struct ShortcutManager { /* ... */ }

impl ShortcutManager {
    pub fn new() -> Self;
    pub fn register(&mut self, action_id: impl Into<String>, shortcut: Shortcut, description: impl Into<String>) -> bool;
    pub fn register_in(&mut self, scope: impl Into<String>, action_id: impl Into<String>, shortcut: Shortcut, description: impl Into<String>) -> bool;
    pub fn unregister(&mut self, id: &str) -> bool;
    pub fn trigger(&self, shortcut: &Shortcut) -> bool;
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String>;
    pub fn push_scope(&mut self, scope: impl Into<String>);
    pub fn push_modal_scope(&mut self, scope: impl Into<String>);
    pub fn pop_scope(&mut self) -> Option<String>;
    pub fn active_scope(&self) -> Option<&str>;
    pub fn is_scope_active(&self, scope: &str) -> bool;
    pub fn active_scopes(&self) -> Vec<String>;
    pub fn clear_scopes(&mut self);
    pub fn clear(&mut self);
}
```

`ShortcutEntry::scope` 在全局作用域中为 `None`，当绑定属于某个上下文时为 `Some(name)`。
作用域绑定的仅在它的作用域位于 `push_scope` 推入的栈上时生效，并遮蔽同一按键组合上的全局绑定。
`push_modal_scope` 还阻止查找到达其下方的所有作用域与全局绑定，这就是对话框能重新绑定 Enter
而不与主窗口冲突的原因。冲突只在**同一作用域内**判定，因此同一按键组合可以绑定在不同作用域中。

### 快捷键类型

```rust
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

pub struct ShortcutEntry {
    pub action_id: String,
    pub description: String,
    pub shortcut: Shortcut,
    pub enabled: bool,
    pub scope: Option<String>,
}

pub enum Key {
    A, B, C, ... Z,       // 字母
    F1, F2, ... F24,       // 功能键
    Enter, Escape, Space,
    Tab, Backspace, Delete,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    Insert, Menu, // ...
}

pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,    // Windows 键 / Cmd 键
}
```

---

## 数据绑定 (`data_binding`)

### Binding（单值）

```rust
pub struct Binding<T: Clone + PartialEq + 'static> { /* ... */ }

impl<T: Clone + PartialEq + 'static> Binding<T> {
    pub fn new(value: T) -> Self;
    pub fn get(&self) -> T;
    pub fn set(&mut self, value: T);
    pub fn map<R: Clone + PartialEq + 'static>(&self, f: impl Fn(&T) -> R) -> Computed<R>;
    pub fn subscribe(&mut self, key: &str, listener: Box<dyn BoxedListener<T>>);
    pub fn unsubscribe(&mut self, key: &str);
    pub fn bind_to(&mut self, target: &mut dyn BindingListener<T>);
}
```

### ObservableList

```rust
pub struct ObservableList<T: Clone + 'static> { /* ... */ }

impl<T: Clone + 'static> ObservableList<T> {
    pub fn new() -> Self;
    pub fn push(&mut self, value: T);
    pub fn pop(&mut self) -> Option<T>;
    pub fn insert(&mut self, index: usize, value: T);
    pub fn remove(&mut self, index: usize) -> T;
    pub fn clear(&mut self);
    pub fn get(&self, index: usize) -> Option<&T>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn subscribe(&mut self, listener: Box<dyn FnMut(ListChange)>);
}
```

### Computed（派生值）

```rust
pub struct Computed<T: Clone + 'static> { /* ... */ }

impl<T: Clone + 'static> Computed<T> {
    pub fn new(compute: Box<dyn Fn() -> T>) -> Self;
    pub fn get(&self) -> T;
    pub fn invalidate(&mut self);
    pub fn subscribe(&mut self, listener: Box<dyn FnMut()>);
}
```

### 监听器 Traits

```rust
pub trait BindingListener<T> {
    fn on_value_changed(&mut self, value: &T);
}

pub trait BoxedListener<T>: 'static {
    fn call(&mut self, key: &str, value: &T);
}

pub struct FnListener<T: 'static> { /* ... */ }
impl<T: 'static> FnListener<T> {
    pub fn new(f: impl FnMut(&str) + 'static) -> Self;
}
impl<T: 'static> BoxedListener<T> for FnListener<T> { /* ... */ }
```

### 宏

```rust
// 创建具有推断类型的 Binding<T>
binding!(value);

// 使用闭包创建 Computed<T>
computed!(|| expression);
```

---

## 信号/槽 (`signal`)

### 类型化信号

```rust
pub struct Signal<T: 'static> { /* ... */ }

impl<T: 'static> Signal<T> {
    pub fn new() -> Self;
    pub fn emit(&self, value: T);
    pub fn connect<F>(&self, f: F) -> ConnectionHandle
        where F: FnMut(T) + 'static;
    pub fn connect_once<F>(&self, f: F) -> ConnectionHandle
        where F: FnOnce(T) + 'static;
    pub fn disconnect(&self, handle: ConnectionHandle);
    pub fn slot_count(&self) -> usize;
}
```

### GenericSignal（无参数）

```rust
pub struct GenericSignal { /* ... */ }

impl GenericSignal {
    pub fn new() -> Self;
    pub fn emit(&self);
    pub fn connect<F>(&self, f: F) -> ConnectionHandle
        where F: FnMut() + 'static;
    pub fn connect_once<F>(&self, f: F) -> ConnectionHandle
        where F: FnOnce() + 'static;
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, f: F) -> ConnectionHandle
        where F: FnMut() + 'static;
    pub fn disconnect(&self, handle: ConnectionHandle);
    pub fn slot_count(&self) -> usize;
}
```

### Signal1（单参数泛型）

```rust
pub struct Signal1<A: 'static> { /* ... */ }
// 与 GenericSignal 相同的 API，但带一个参数
```

### 连接管理

```rust
pub struct ConnectionHandle(usize);

pub struct ConnectionScope { /* ... */ }
impl ConnectionScope {
    pub fn new() -> Self;
    // 当 scope 被丢弃时，连接自动断开
}

pub struct CustomSignalHub { /* ... */ }
impl CustomSignalHub {
    pub fn new() -> Self;
    pub fn signal(&self, name: &str) -> &GenericSignal;
    pub fn emit(&self, name: &str);
    pub fn connect(&self, name: &str, f: impl FnMut() + 'static) -> ConnectionHandle;
}
```

### 接线查询

`connect_event` 返回 `Ok` 只证明事件名已发布，**不**证明有订阅者能到达该控件。两个运行时查询回答这个差异。

```rust
pub fn is_wired(widget_id: ObjectId, event_name: &str) -> bool;
pub fn unwired_events(widget_id: ObjectId) -> Option<Vec<&'static str>>;
```

当控件自身的 `EventSignalRef` 报告 `event_name` 至少有一个存活订阅者时，`signal::is_wired` 为 `true`。
`signal::unwired_events` 返回该控件所有没有任何存活订阅者的已发布事件；当 id 不是已挂载控件时返回 `None`
—— 这与表示「全部已接线」的 `Some(vec![])` 是不同的答案。在裁剪配置中能力表被编译掉，
因此 `is_wired` 返回 `false`，`unwired_events` 返回 `None`。

---

## 国际化 (`i18n`)

### 核心 API

```rust
pub struct I18nManager { /* ... */ }

impl I18nManager {
    pub fn new() -> Self;
    pub fn load_translations(&mut self, path: &str) -> Result<(), RwError>;
    pub fn set_language(&mut self, lang: &str);
    pub fn language(&self) -> &str;
    pub fn translate(&self, key: &str) -> String;
    pub fn translate_with_context(&self, key: &str, context: &[(&str, &str)]) -> String;
    pub fn reload(&mut self) -> Result<(), RwError>;
}
```

### 全局函数

```rust
pub fn init(locales_dir: &str) -> Result<(), RwError>;
pub fn init_with_options(options: InitOptions) -> Result<InitReport, RwError>;
pub fn translate(key: &str) -> String;
pub fn translate_with_context(key: &str, context: &[(&str, &str)]) -> String;
pub fn get_manager() -> &'static I18nManager;
pub fn check_and_reload_all() -> Result<(), RwError>;

pub use crate::tr;  // 宏：tr!("hello") → 翻译后的字符串
```

### 选项与类型

```rust
pub struct InitOptions {
    pub fallback_language: String,
    pub auto_load: bool,
    pub hot_reload: bool,
    // ...
}

pub struct InitReport {
    pub loaded_files: Vec<String>,
    pub errors: Vec<String>,
}

pub struct Translation { /* ... */ }
pub struct TranslationFile { /* ... */ }
pub enum ReloadEvent { FileChanged(String), Reloaded, Failed(String) }
```

### 热重载

```rust
pub fn init_with_hot_reload(locales_dir: &str) -> Result<I18nFileWatcher, RwError>;
pub fn process_reload_events(watcher: &mut I18nFileWatcher) -> Vec<ReloadEvent>;

pub struct I18nFileWatcher { /* ... */ }
```

---

## 手势识别 (`gesture`)

### 核心 Trait

```rust
pub trait GestureRecognizer: Debug + Send {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    fn reset(&mut self);
}
```

### 手势引擎

```rust
pub struct GestureEngine { /* ... */ }

impl GestureEngine {
    pub fn new() -> Self;       // 预填充所有标准识别器
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    pub fn reset_all(&mut self);
}
```

### 识别器

| 识别器 | 产生的事件 | 描述 |
|---|---|---|
| `TapGesture` | `Event::Tap` | 快速触摸并释放（<300ms，<15px 移动） |
| `DoubleTapGesture` | `Event::DoubleTap` | 400ms 内两次点击 |
| `LongPressGesture` | `Event::LongPress` | 保持 ≥500ms |
| `SwipeGesture` | `Event::Swipe` | 快速滑动（≥0.5 px/ms，≥30px） |
| `PanGesture` | `Event::MouseMove` | 连续拖拽跟踪 |
| `LongPressDragGesture` | `Event::Swipe` | 长按后拖拽 |
| `FlingGesture` | `Event::Swipe` | 基于速度的快速滑动/轻拂 |
| `TwoFingerTapGesture` | 'Custom' | 双指点击 |
| `TwoFingerSwipeGesture` | 'Custom' | 双指滑动 |
| `PinchGesture` | `Event::Pinch` | 双指距离变化（缩放） |
| `RotateGesture` | `Event::Rotate` | 双指角度变化 |

---

## 图表与数据可视化 (`chart`)

### 图表类型

Data visualization lives under `widget::chart_widgets`. The chart *engine* (layout, axes, ticks, SVG context) and the chart *widgets* are two layers of one feature, so they share a module:

```rust
// The engine and the chart widgets both live under `widget::chart_widgets`:
// they are two layers of one feature, and there is no top-level `chart` module.
pub struct ChartLayout { /* ... */ }          // widget::chart_widgets::layout
pub struct SvgChartContext { /* ... */ }      // widget::chart_widgets::svg
pub struct MemoryChartContext { /* ... */ }   // widget::chart_widgets::svg
pub trait Chart { /* ... */ }                 // widget::chart_widgets::types
pub trait ChartContext { /* ... */ }          // widget::chart_widgets::types
// 子模块：charts, layout, svg, types
```

图表数据类型包括坐标轴配置、系列定义、图例以及用于折线图、柱状图、饼图和散点图的数据点结构。

---

## PDF 生成 (`pdf`)

### 核心 Traits

```rust
pub trait PdfPage {
    fn size(&self) -> Size;
    fn set_size(&mut self, size: Size);
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str);
    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool);
    fn add_button(&mut self, name: &str, rect: Rect, text: &str);
    fn content(&self) -> Vec<u8>;
    fn form_fields(&self) -> Vec<PdfFormField>;
}

pub trait PdfDocument {
    fn page_count(&self) -> u32;
    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage>;
    fn add_page(&mut self, size: Size) -> u32;
    fn insert_page(&mut self, index: u32, size: Size) -> u32;
    fn remove_page(&mut self, index: u32) -> bool;
    fn reorder_pages(&mut self, new_order: &[u32]) -> bool;
    fn metadata(&self) -> &PdfMetadata;
    fn set_metadata(&mut self, metadata: PdfMetadata);
    fn security(&self) -> &PdfSecurity;
    fn set_security(&mut self, security: PdfSecurity);
    fn set_page_numbering_enabled(&mut self, enabled: bool);
    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32);
    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32);
    fn save(&self, path: &str) -> Result<(), std::io::Error>;
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}
```

### 模块子类型

```rust
pub mod annotation;      // PDF 注释
pub mod document;        // 文档创建/操作
pub mod export;          // PDF 导出
pub mod form;            // 交互式表单字段
pub mod hyperlink;       // 超链接
pub mod metadata;        // PDF 元数据（作者、标题等）
pub mod page;            // 页面管理
pub mod reader;          // PDF 读取/解析
pub mod security;        // 加密、密码、权限
pub mod types;           // 共享 PDF 类型
pub mod writer;          // PDF 写入/序列化
```

---

## 打印 (`print`)

```rust
// 提供打印对话框集成和文档布局支持
pub mod print_impl;
pub use print_impl::*;
```

---

## 内存管理 (`memory`)

### 池分配器

```rust
pub struct ObjectPool<T> / SharedPool<T> / PoolManager { /* ... */ }
impl ObjectPool<T> / SharedPool<T> / PoolManager {
    pub fn new() -> Self;
    pub fn allocate(&mut self, size: usize) -> Option<*mut u8>;
    pub fn deallocate(&mut self, ptr: *mut u8, size: usize);
    pub fn clear(&mut self);
}
```

### Arena 分配器

```rust
pub struct ArenaAllocator { /* ... */ }
impl ArenaAllocator {
    pub fn new(capacity: usize) -> Self;
    pub fn allocate<T>(&mut self) -> Option<NonNull<T>>;
    pub fn reset(&mut self);
    pub fn capacity(&self) -> usize;
    pub fn used(&self) -> usize;
    pub fn available(&self) -> usize;
}
```

### 栈分配器

```rust
pub struct StackAllocator { /* ... */ }
impl StackAllocator {
    pub fn new(capacity: usize) -> Self;
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8>;
    pub fn push_marker(&mut self);
    pub fn pop_to_marker(&mut self);
    pub fn clear(&mut self);
    pub fn capacity(&self) -> usize;
    pub fn used(&self) -> usize;
    pub fn available(&self) -> usize;
}
```

### 内存监控

```rust
pub struct MemoryStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub pool_hits: usize,
    pub pool_misses: usize,
}

pub enum MemoryPressure { None, Low, Medium, High, Critical }

pub trait MemoryPressureHandler: Send + Sync {
    fn on_pressure(&mut self, pressure: MemoryPressure);
}

pub struct MemoryMonitor { /* ... */ }
impl MemoryMonitor {
    pub fn new(warning_threshold: usize, critical_threshold: usize) -> Self;
    pub fn stats(&self) -> &MemoryStats;
    pub fn pressure(&self) -> MemoryPressure;
    pub fn register_handler(&mut self, handler: Box<dyn MemoryPressureHandler>);
    pub fn update(&mut self, current_usage: usize);
    pub fn record_allocation(&mut self, size: usize);
    pub fn record_deallocation(&mut self, size: usize);
}
```

---

## 性能 (`performance`)

### 脏区域跟踪

```rust
pub struct DirtyRegionTracker { /* ... */ }
impl DirtyRegionTracker {
    pub fn new() -> Self;
    pub fn add(&mut self, rect: Rect);
    pub fn merge(&mut self);
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
    pub fn regions(&self) -> &[DirtyRegion];
    pub fn get_bounding_rect(&self) -> Option<Rect>;
}

pub fn render_dirty_regions(
    tracker: &mut DirtyRegionTracker,
    ctx: &mut RenderContext,
    render_all: impl FnMut(&mut RenderContext),
);

pub fn render_dirty_regions_into(
    tracker: &mut DirtyRegionTracker,
    ctx: &mut RenderContext,
    regions: &mut Vec<Rect>,
    render_all: impl FnMut(&mut RenderContext),
);
```

`render_dirty_regions_into` 是复用缓冲区的形式：它先清空 `regions`，再用合并后的区域矩形填充它，然后进行分桶遍历，
因此帧循环可以终生只持有一个 `Vec`，无需每帧为区域分配内存。当走上「超出容量」的回退路径时，
单一裁剪由包围矩形构成，缓冲区则保持为空。

### 更新批处理

```rust
pub struct UpdateBatcher { /* ... */ }
impl UpdateBatcher {
    pub fn new(coalesce_ms: u64) -> Self;
    pub fn add(&mut self, rect: Rect);
    pub fn flush(&mut self) -> Vec<Rect>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

### 性能分析

```rust
pub struct Profiler { /* ... */ }
impl Profiler {
    pub fn new() -> Self;
    pub fn begin_frame(&mut self);
    pub fn end_frame(&mut self);
    pub fn record(&mut self, label: &str, duration_ns: u64);
    pub fn frame_count(&self) -> u64;
    pub fn timing(&self, label: &str) -> Option<&TimingSample>;
}
```

---

## 自适应质量 (`quality`)

### 质量管理器

```rust
pub struct QualityManager { /* ... */ }

impl QualityManager {
    pub fn new() -> Self;
    pub fn with_config(config: QualityConfig) -> Self;
    pub fn with_config_and_capability(config: QualityConfig, gpu: GpuCapability) -> Self;
    pub fn quality_level(&self) -> QualityLevel;
    pub fn set_quality_level(&mut self, level: QualityLevel);
    pub fn finish_frame_secs(&mut self, frame_time_secs: f32);
    pub fn reset(&mut self);
}
```

### 质量等级

```rust
pub enum QualityLevel { Low, Medium, High }

impl QualityLevel {
    pub fn lower(&self) -> Option<Self>;
    pub fn higher(&self) -> Option<Self>;
    pub fn clamp(self, min: Self, max: Self) -> Self;
}
```

### 配置

```rust
pub struct QualityConfig {
    pub target_frame_rate: f32,
    pub degrade_threshold: f32,       // 触发降级的帧时间倍数
    pub upgrade_threshold: f32,       // 触发升级的帧时间倍数
    pub degrade_frame_count: u32,     // 连续慢帧数以触发降级
    pub upgrade_frame_count: u32,     // 连续快帧数以触发升级
    pub max_quality: QualityLevel,
    pub min_quality: QualityLevel,
}

impl QualityConfig {
    pub fn normalized(self) -> Self;
}
```

### 帧时间监控器

```rust
pub struct FrameTimeMonitor { /* ... */ }
impl FrameTimeMonitor {
    pub fn new(target_fps: f32) -> Self;
    pub fn record_frame(&mut self, frame_time_secs: f32);
    pub fn average_frame_time(&self) -> f32;
    pub fn should_degrade(&self, threshold: f32, consecutive_count: u32) -> bool;
    pub fn should_upgrade(&self, threshold: f32, consecutive_count: u32) -> bool;
}
```

### GPU 能力

```rust
pub struct GpuCapability {
    pub supports_high_quality: bool,
    pub is_integrated: bool,
    pub performance_tier: u32,
}

impl GpuCapability {
    pub fn recommended_initial_quality(&self) -> QualityLevel;
}
```

---

## 控制后端 (`control_backend`)

### 核心类型

```rust
pub enum ControlBackendKind { Native, Custom }
pub enum ControlRoutePreference { NativePreferred, NativeRequired, CustomPreferred, CustomRequired }
```

### 后端 Trait

```rust
pub trait ControlBackend {
    fn backend_name(&self) -> &'static str;
    fn draw_button(&self, ctx: &mut RenderContext, rect: Rect, state: &ButtonState);
    fn draw_checkbox(&self, ctx: &mut RenderContext, rect: Rect, state: &CheckState);
    fn draw_slider(&self, ctx: &mut RenderContext, rect: Rect, value: f32);
    // ...每种控件类型一个绘制方法
}
```

### 调度

```rust
pub fn get_control_backend() -> Box<dyn ControlBackend>;
pub fn get_control_backend_for_widget(kind: WidgetKind) -> Box<dyn ControlBackend>;
pub fn active_control_policy() -> &'static str;
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference;
```

### 后端实现

```rust
pub struct NativeControlBackend { /* ... */ }
impl NativeControlBackend {
    pub fn new() -> Self;
}
impl ControlBackend for NativeControlBackend { /* ... */ }

#[cfg(feature = "controls-custom")]
pub struct CustomPaintControlBackend { /* ... */ }
```

---

## 对象系统 (`object`)

```rust
pub struct Object { /* ... */ }

impl Object {
    pub fn new(class_name: &str) -> Self;
    pub fn id(&self) -> ObjectId;
    pub fn class_name(&self) -> &str;
    pub fn set_property(&mut self, key: &str, value: PropertyValue);
    pub fn property(&self, key: &str) -> Option<&PropertyValue>;
    pub fn property_keys(&self) -> Vec<&str>;
}

pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(Color),
    Rect(Rect),
    Size(Size),
}
```

---

## Web 能力 (`web`)

### 浏览器历史

```rust
pub struct BrowserHistory { /* ... */ }
impl BrowserHistory {
    pub fn new() -> Self;
    pub fn add_entry(&mut self, url: String, title: String);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn entries(&self) -> &[HistoryEntry];
    pub fn clear(&mut self);
}

pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visit: u64,
}
```

### 会话历史

```rust
pub struct SessionHistory { /* ... */ }
impl SessionHistory {
    pub fn new(max_entries: usize) -> Self;
    pub fn navigate(&mut self, url: String);
    pub fn back(&mut self) -> bool;
    pub fn forward(&mut self) -> bool;
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
    pub fn current(&self) -> Option<&str>;
    pub fn clear(&mut self);
}
```

### JS 引擎

```rust
pub enum JsValue { Null, Bool(bool), Number(f64), String(String), Object(HashMap<String, JsValue>), Array(Vec<JsValue>) }

pub struct JsContext { /* ... */ }
impl JsContext {
    pub fn new() -> Self;
    pub fn set(&mut self, name: &str, value: JsValue);
    pub fn get(&self, name: &str) -> Option<&JsValue>;
}

pub trait JsEngine {
    fn evaluate(&self, script: &str, context: &mut JsContext) -> Result<JsValue, String>;
}

pub struct SimpleJsEngine;
impl JsEngine for SimpleJsEngine { /* ... */ }
```

### 导航

```rust
pub struct NavigationEntry {
    pub url: String,
    pub title: String,
}

pub struct NavigationHistory { /* ... */ }
impl NavigationHistory {
    pub fn new(max_size: usize) -> Self;
    pub fn push(&mut self, entry: NavigationEntry);
    pub fn back(&mut self) -> Option<&NavigationEntry>;
    pub fn forward(&mut self) -> Option<&NavigationEntry>;
    pub fn is_empty(&self) -> bool;
}

pub struct WebSettings {
    pub javascript_enabled: bool,
    pub cookies_enabled: bool,
    pub local_storage_enabled: bool,
    pub allow_geolocation: bool,
    pub allow_notifications: bool,
    pub allow_popups: bool,
    pub user_agent: String,
}

pub struct SecuritySettings {
    pub block_popups: bool,
    pub block_mixed_content: bool,
    pub enable_web_security: bool,
    pub certificate_check: bool,
}
```

### 插件

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&mut self) -> Result<(), String>;
    fn on_unload(&mut self) -> Result<(), String>;
}

pub struct PluginManager { /* ... */ }
impl PluginManager {
    pub fn new() -> Self;
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<u32, String>;
    pub fn unregister(&mut self, id: u32) -> bool;
    pub fn list(&self) -> Vec<&dyn Plugin>;
    pub fn clear(&mut self);
}

pub enum PluginError { /* ... */ }
pub struct ContentPlugin { /* ... */ }
```

### 隐私

```rust
pub struct PrivacySettings { /* ... */ }
impl PrivacySettings {
    pub fn new() -> Self;
    pub fn strict() -> Self;
}

pub struct CookieJar { /* ... */ }
impl CookieJar {
    pub fn new() -> Self;
    pub fn add(&mut self, cookie: Cookie);
    pub fn remove(&mut self, domain: &str, name: &str);
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
}

pub enum TrackingType { Advertising, Analytics, Social, Fingerprinting, Malware }
pub struct TrackingProtection { /* ... */ }
pub struct BrowsingData { /* ... */ }
pub enum LoadStatus { NotStarted, Loading, Loaded, Failed }
```

### Web Engine 与 Web View（非 mini）

```rust
pub struct WebEngineViewEnhanced { /* ... */ }
impl WebEngineViewEnhanced {
    pub fn new(rect: Rect) -> Self;
    pub fn url(&self) -> &str;
    pub fn set_url(&mut self, url: &str);
    pub fn reload(&mut self);
    pub fn stop(&mut self);
    pub fn go_back(&mut self);
    pub fn go_forward(&mut self);
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
}

pub struct WebViewEnhanced { /* ... */ }
impl WebViewEnhanced {
    pub fn new(rect: Rect) -> Self;
    pub fn url(&self) -> &str;
    pub fn load_url(&mut self, url: &str);
    pub fn load_html(&mut self, html: &str, base_url: &str);
    pub fn reload(&mut self);
    pub fn stop(&mut self);
    pub fn go_back(&mut self);
    pub fn go_forward(&mut self);
}

pub struct WebViewCore { /* ... */ }
```

### Web 控件类型 *(非 mini)*

```rust
pub struct WebEngine { /* ... */ }
pub struct WebView { /* ... */ }

// Web 控件使用的关联类型：
pub struct WebEngineContextMenuRequest { /* ... */ }
pub struct WebEngineCookieStore { /* ... */ }
pub struct WebEngineDownloadItem { /* ... */ }
pub struct WebEngineFindTextResult { /* ... */ }
pub struct WebEngineNotification { /* ... */ }
pub struct WebEnginePage { /* ... */ }
pub struct WebEngineScriptDialog { /* ... */ }
pub struct WebEngineSettings { /* ... */ }
pub struct WebEngineWebChannel { /* ... */ }
```

---

## 撤销/重做 (`undo`)

```rust
pub trait UndoCommand: Send + Sync {
    fn id(&self) -> &str;
    fn execute(&mut self) -> Result<(), RwError>;
    fn undo(&mut self) -> Result<(), RwError>;
    fn merge_with(&mut self, other: &dyn UndoCommand) -> bool;
}

pub struct UndoStack { /* ... */ }
impl UndoStack {
    pub fn new(max_undo: usize) -> Self;
    pub fn push(&mut self, command: Box<dyn UndoCommand>);
    pub fn undo(&mut self) -> bool;
    pub fn redo(&mut self) -> bool;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn clear(&mut self);
    pub fn undo_text(&self) -> Option<&str>;
    pub fn redo_text(&self) -> Option<&str>;
    pub fn is_clean(&self) -> bool;
}
```

---

## 剪贴板 (`clipboard`)

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>;
```

---

## GPU 加速 (`gpu`, `wgpu_backend`)

GPU 模块通过 `wgpu` 提供硬件加速渲染：

```rust
#[cfg(feature = "gpu-wgpu")]
pub mod gpu;

#[cfg(feature = "gpu-wgpu")]
pub use gpu::{GpuCapability, GpuRenderer};

// gpu 模块内容：
pub enum GpuCapability { Software, Basic, Medium, High }
pub struct GpuRenderer { /* ... */ }
impl GpuRenderer {
    pub fn new() -> Result<Self, ()>;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self) -> Result<(), ()>;
    pub fn submit(&mut self, commands: &[RenderCommand]);
    pub fn resize(&mut self, size: Size);
}
```

`wgpu_backend` 模块提供具体的 wgpu 实现。

---

## 嵌入式支持 (`embedded`)

```rust
pub mod embedded;

// 嵌入式引擎类型
pub use render_engine::{
    EmbeddedEngine,
    // 详见上方渲染引擎章节
};
```

---

## 语言绑定 (`bindings`)

`bindings` 模块提供用于 C/C++ 互操作的 FFI 基础设施。

```rust
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
```

完整 C API 请参见下方的 [FFI / C ABI 参考](#ffi--c-abi-参考) 章节。

---

## 媒体模块 (`audio`、`video`、`image`)

这三个模块分别由 `audio`、`video`、`image` 特性启用。`desktop`、`tablet`、`mobile` 会启用全部三个；
`mini` 与 `embedded` 不包含。

自 2.9.1 起，所有接收整块数据的入口都把解码放在工作线程上，帧循环不会被长音频或大图片阻塞。
每个入口都保留同步形式；所有异步入口在线程无法创建时退回同步解码——失败模式是“更慢”，
而不是“永远加载不出来”。

### 音频（`audio`）

`AudioEngine` 持有一个已解码缓冲与一个可选的设备输出。

```rust
use rust_widgets::audio::AudioEngine;

let mut engine = AudioEngine::new();
engine.load(&wav_bytes)?;       // 同步：在调用线程解码
engine.play();
let played = engine.tick(1024); // 推进位置，返回消费的采样数

// 或在后台解码，完成后安装结果：
engine.load_async(wav_bytes);   // 立即返回
// 每帧轮询一次：
if let Some(result) = engine.poll_load() {
    result?;                    // 本次调用完成安装
}
// 或一次性阻塞：
engine.wait_for_load()?;
```

| 方法 | 签名 | 说明 |
|---|---|---|
| `new` | `fn() -> Self` | 空引擎 |
| `load` | `fn(&mut self, data: &[u8]) -> Result<(), String>` | 同步解码 |
| `load_async` | `fn(&mut self, data: Vec<u8>)` | 后台解码；新调用会取代上一次加载 |
| `is_loading` | `fn(&self) -> bool` | 后台加载仍在进行 |
| `poll_load` | `fn(&mut self) -> Option<Result<(), String>>` | 安装已完成的加载；仍在进行时为 `None` |
| `wait_for_load` | `fn(&mut self) -> Result<(), String>` | 阻塞直到加载完成 |
| `play` / `pause` / `stop` | `fn(&mut self)` | 播放状态 |
| `set_volume` / `volume` | `fn(&mut self, f32)` / `fn(&self) -> f32` | 钳制到 `0.0..=1.0` |
| `tick` | `fn(&mut self, samples: usize) -> usize` | 推进位置，到末尾饱和 |
| `current_samples` | `fn(&self, count: usize) -> Vec<f32>` | 当前位置下的采样 |
| `duration_seconds` | `fn(&self) -> f64` | 未加载时为 `0.0` |
| `play_to_device` | `fn(&mut self) -> Result<(), String>` | 需要 `audio-output`（cpal） |

格式支持由 `AudioFormat` 描述；`symphonia-codecs` 增加 FLAC、OGG Vorbis、AAC 与 Opus 解码。

### 视频（`video`）

`VideoEngine` 驱动一个 `VideoDecoder` 并持有主时间轴。帧在后台线程解码，因此 `tick_frame`
从有界预读队列取帧，而不是在帧循环里解码。每次 seek 开启一个新的世代：为旧位置解码的帧会被
丢弃而不是显示。

```rust
use rust_widgets::video::VideoEngine;

let mut engine = VideoEngine::open(mp4_bytes)?; // 根据容器选择解码器
engine.play();
let frame = engine.tick_frame(16, 0.008)?;      // 一个显示帧；到期时返回 Some(frame)
```

| 方法 | 签名 | 说明 |
|---|---|---|
| `open` | `fn(Vec<u8>) -> Result<Self, String>` | 识别容器；H.264 等需要 `video-codecs` |
| `with_decoder` | `fn(Box<dyn VideoDecoder + Send>) -> Self` | 自带解码器 |
| `metadata` / `duration` | `fn(&self) -> &VideoMetadata` / `fn(&self) -> f64` | 容器信息 |
| `play` / `pause` / `stop` | `fn(&mut self)` | 结束后 `play` 从第一帧重新开始 |
| `seek` | `fn(&mut self, time: f64) -> Result<(), String>` | 旧位置的帧被丢弃 |
| `tick_frame` | `fn(&mut self, delta_ms: u32, tolerance: f64) -> Result<Option<VideoFrame>, String>` | `None` 表示本帧没有到期画面 |
| `next_frame` / `step_frame` | `fn(&mut self) -> Result<Option<VideoFrame>, String>` | 走同一个单帧槽位 |
| `take_dropped_frames` | `fn(&mut self) -> u64` | 为追赶进度而丢弃的帧数 |
| `clock` / `clock_mut` | 时间轴访问器 | 主 `MediaClock` |

### 图片（`image`）

```rust
use rust_widgets::image::{decode, decode_async};

let image = decode(&png_bytes)?;   // 同步
let job = decode_async(png_bytes); // 后台；返回 ImageDecodeJob
let image = job.wait()?;           // 或每帧一次 job.try_take()
```

`decode` 返回 `DecodedImage`（`format`、`data`、`width`、`height`、`exif`、`color_space`）。
`decode_to_rgba8` 一步完成转换，`decode_animation` 返回 GIF/WebP 的每一帧及其延迟。
`ImageDecodeJob::try_take` 在解码仍在进行时返回 `None`，`wait` 则阻塞到完成。

---

## 特性标志参考

库使用**三轴特性系统** — 从每个轴中选择一个：

### 轴 1：设备配置文件（互斥）

| 特性 | 描述 | 包含 |
|---|---|---|
| `desktop`（默认） | 完整桌面 | GPU、触摸、i18n、图表、打印、PDF、无障碍、质量、高级控件 |
| `tablet` | 触摸优先平板 | GPU、触摸、i18n、质量 |
| `mobile` | 移动优化 | GPU、触摸、i18n、质量、移动 API |
| `embedded` | 无 GPU，软件光栅化 | 软件、自定义控件 |
| `mini` | LVGL 风格最小化（约 28 个控件） | 软件、自定义控件、`heapless`、`hashbrown`、`spin`、`bumpalo` |

### 轴 2：操作系统后端

| 特性 | 平台 | 关键依赖 |
|---|---|---|
| `os-auto` | 自动检测 | (无) |
| `macos` | macOS（现代） | `objc2`、`objc2-foundation`、`objc2-app-kit`、`objc2-core-graphics` |
| `macos-legacy` | macOS（旧版） | `cocoa`、`objc`、`objc-foundation` |
| `ios` | iOS | `objc2`、`objc2-foundation`、`objc2-ui-kit` |
| `windows` | Windows | `winapi` |
| `linux-gtk` | Linux（GTK） | `gtk` |
| `linux-wayland` | Linux（Wayland） | `wayland-client`、`wayland-protocols`、`wayland-cursor` |
| `linux-a11y` | Linux 无障碍 | `zbus`、`pollster` |
| `android` | Android | `jni` |
| `wasm` | WebAssembly | `wasm-bindgen`、`web-sys`、`js-sys` |
| `harmony` | HarmonyOS | (无) |

### 轴 3：能力（任意组合）

| 特性 | 描述 |
|---|---|
| `touch` | 触摸事件支持 |
| `gpu` / `wgpu` | GPU 加速渲染 |
| `software` | 软件光栅化器 |
| `i18n` | 国际化 |
| `chart` | 图表控件 |
| `print` | 打印 |
| `pdf` | PDF 生成 |
| `a11y` | 无障碍 |
| `holographic` | 全息/3D 手势检测（实验性） |
| `projection` | 投影/演示模式 |
| `controls-native` | 原生 OS 风格控件渲染 |
| `controls-custom` | 自定义（主题化）控件渲染 |
| `advanced-widgets` | 高级控件（日历、日期/时间选择器、功能区栏等） |
| `image` | 图片解码/编码模块 |
| `video` | 视频引擎模块（容器识别、帧解码、播放） |
| `audio` | 音频引擎模块（格式识别、解码、缓冲） |
| `audio-output` | 通过 cpal 输出到系统设备 |
| `symphonia-codecs` | FLAC / OGG Vorbis / AAC / Opus 音频解码 |
| `video-codecs` | FFmpeg 视频解码 |
| `pdf-encryption` | 加密 PDF 输出 |
| `js-engine` | 内嵌 JavaScript 引擎（boa） |
| `web-http` | Web 能力的 HTTP 客户端（ureq） |
| `unstable-pipeline-routing` | 实验性管线路由 |
| `unstable-special-widgets` | 实验性特殊控件 |

### 配置文件便利

| 特性 | 描述 |
|---|---|
| `full` | 启用所有功能（不适用于生产） |
| `desktop-runtime` | 内部：启用文件监视器、通道依赖 |

### 编译时配置文件

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1

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

---

## 错误代码参考

| 常量 | 代码 | 描述 |
|---|---|---|
| `SUCCESS` | 0 | 操作成功完成 |
| `NOT_IMPLEMENTED` | 1 | 功能尚未实现 |
| `UNSUPPORTED_OPERATION` | 2 | 此平台不支持该操作 |
| `INVALID_ARGUMENT` | 3 | 无效的参数 |
| `NULL_POINTER` | 4 | 检测到空指针 *(保留)* |
| `OUT_OF_MEMORY` | 5 | 内存分配失败 *(保留)* |
| `LOCK_POISONED` | 6 | 互斥锁中毒 *(保留)* |
| `WIDGET_BASE_NOT_IMPL` | 100 | 控件基础方法未实现 *(保留)* |
| `WIDGET_NOT_FOUND` | 101 | 控件未找到 *(保留)* |
| `WIDGET_INVALID_STATE` | 102 | 控件处于无效状态 *(保留)* |
| `WIDGET_DEPRECATED` | 103 | 控件已弃用 *(保留)* |
| `PLATFORM_UNSUPPORTED` | 200 | 平台不受支持 *(保留)* |
| `PLATFORM_INIT_FAILED` | 201 | 平台初始化失败 *(保留)* |
| `CLIPBOARD_FAILED` | 202 | 剪贴板操作失败 *(保留)* |
| `DRAG_DROP_FAILED` | 203 | 拖放操作失败 *(保留)* |
| `RENDER_CONTEXT_INVALID` | 300 | 渲染上下文无效 *(保留)* |
| `RENDER_PIPELINE_FAILED` | 301 | 渲染管线失败 *(保留)* |
| `I18N_LOAD_FAILED` | 400 | i18n 文件加载失败 *(保留)* |
| `FILE_NOT_FOUND` | 401 | 文件未找到 |

---

## FFI / C ABI 参考

库公开了稳定的 C ABI 以支持语言互操作。所有 C 函数均以 `rw_` 为前缀。生成的头文件位于 `include/rw_generated.h`。

### 生命周期

```c
void rw_init(void);
void rw_run(void);
void rw_quit(void);
```

### 控件创建

```c
uint64_t rw_create_window(const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_button(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_checkbox(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_label(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_line_edit(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_radio_button(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_slider(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_progress_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_combo_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_list_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_panel(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_message_box(uint64_t parent, const char* title, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_file_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_color_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_font_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_spin_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_list_view(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_scroll_area(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_menu_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_menu(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_tool_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_status_bar(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
```

### 控件操作

```c
void rw_show_widget(uint64_t widget_id);
void rw_hide_widget(uint64_t widget_id);
void rw_set_widget_geometry(uint64_t widget_id, int x, int y, uint32_t width, uint32_t height);
bool rw_get_widget_geometry(uint64_t widget_id, int* x_out, int* y_out, uint32_t* width_out, uint32_t* height_out);
void rw_set_widget_text(uint64_t widget_id, const char* text);
const char* rw_get_widget_text(uint64_t widget_id);
void rw_set_widget_enabled(uint64_t widget_id, bool enabled);
bool rw_is_widget_enabled(uint64_t widget_id);
void rw_set_widget_visible(uint64_t widget_id, bool visible);
bool rw_is_widget_visible(uint64_t widget_id);
```

### Combo Box

```c
// 组合框没有专用的添加函数：添加项经属性层，每个持有项的控件都会应答。
bool rw_set_widget_property(uint64_t widget_id, const char* name,
                            int kind, int64_t num, const char* str);

bool rw_combo_box_clear_items(uint64_t combo_box);
int rw_combo_box_current_index(uint64_t combo_box);
bool rw_combo_box_set_current_index(uint64_t combo_box, unsigned int index);
unsigned int rw_combo_box_item_count(uint64_t combo_box);
const char* rw_combo_box_item_text(uint64_t combo_box, unsigned int index);
```

### List Box

```c
bool rw_list_box_add_item(uint64_t list_box, const char* text);
bool rw_list_box_remove_item(uint64_t list_box, uint32_t index);
bool rw_list_box_clear_items(uint64_t list_box);
int rw_list_box_current_index(uint64_t list_box);
bool rw_list_box_set_current_index(uint64_t list_box, uint32_t index);
uint32_t rw_list_box_item_count(uint64_t list_box);
const char* rw_list_box_item_text(uint64_t list_box, uint32_t index);
```

### 布局（Layout）

```c
#include "rw_generated.h"

// 为 `parent` 创建并存储布局。`kind_name` 使用与声明式文档相同的拼写
// （"hbox"、"vbox"、"grid"、"stack"、"form"、"flow"、"wrap"、"flex"、
// "splitter"），因此同一个名字在两处通用。未知类型会被拒绝，而非默认回退。
bool rw_widget_set_layout(uint64_t parent, const char* kind_name,
                          int spacing, int margin);

// 将子控件注册进布局。`stretch` 最小被提升为 1。
bool rw_widget_layout_add(uint64_t parent, uint64_t child, unsigned int stretch);

// 可伸缩的空隙，无需占位控件。
bool rw_widget_layout_add_spacer(uint64_t parent, unsigned int stretch);

bool rw_widget_layout_remove(uint64_t parent, uint64_t child);
bool rw_widget_layout_clear(uint64_t parent);

// 在给定矩形内重新计算并移动子控件。返回被定位的子控件数；
// 未存储布局时返回 0。
unsigned int rw_widget_layout_apply(uint64_t parent, int x, int y,
                                    unsigned int width, unsigned int height);

// 布局持有多少子控件（不实际应用）。
unsigned int rw_widget_layout_child_count(uint64_t parent);
```

### 样式（Style）

```c
#include "rw_generated.h"

// 应用一条写作 "property: value" 的样式声明，属性名与取值语法
// 与样式表一致："background-color: #FF0000"、"border-radius: 4"、"font-size: 14"。
//
// 与样式表不同——按 CSS 规范，样式表会忽略不认识的属性——此处拼错的属性
// 会被拒绝，因为这次调用明确只指定了一个属性，静默会让拼写错误被当作成功。
bool rw_widget_set_style(uint64_t widget_id, const char* declaration);
```

### 菜单

```c
bool rw_attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar);
uint64_t rw_menu_add_item(uint64_t parent_menu, const char* text, const char* shortcut);
uint64_t rw_poll_menu_triggered(void);
bool rw_inject_menu_trigger(uint64_t menu_item_id);
```

### 剪贴板

```c
bool rw_set_clipboard_text(const char* text);
const char* rw_get_clipboard_text(void);
```

### 事件轮询

```c
uint64_t rw_poll_widget_triggered(void);
uint32_t rw_poll_widget_trigger_event(uint64_t* widget_id_out);
bool rw_inject_widget_trigger_event(uint64_t widget_id, uint32_t kind_code);
```

### 拖放

```c
bool rw_begin_drag(uint64_t source, const char* mime_type, const uint8_t* payload, uint32_t payload_len);
bool rw_poll_drop_event(uint64_t* source_out, uint64_t* target_out, char** mime_out, uint8_t** payload_out, uint32_t* payload_len_out);
```

### IME 与无障碍

```c
bool rw_set_widget_ime_enabled(uint64_t widget_id, bool enabled);
bool rw_is_widget_ime_enabled(uint64_t widget_id);
bool rw_set_widget_accessibility_name(uint64_t widget_id, const char* name);
const char* rw_get_widget_accessibility_name(uint64_t widget_id);
```

### 平台信息

```c
const char* rw_backend_name(void);
uint32_t rw_platform_capabilities(void);
uint32_t rw_platform_capability_contract(uint32_t profile_code);
float rw_platform_dpi_scale_factor(void);
```

### 渲染

```c
uint32_t rw_set_render_aa_samples_per_axis(uint32_t samples);
uint32_t rw_get_render_aa_samples_per_axis(void);
```

### 嵌入式引擎

```c
bool rw_embedded_engine_is_initialized(void);
bool rw_embedded_engine_is_running(void);
uint64_t rw_embedded_engine_window_count(void);
uint64_t rw_embedded_engine_button_count(void);
uint64_t rw_embedded_engine_frame_count(void);
uint64_t rw_embedded_engine_pending_task_count(void);
uint64_t rw_submit_embedded_noop_task(const char* label);
uint32_t rw_set_embedded_target_fps(uint32_t fps);
uint32_t rw_get_embedded_target_fps(void);
```

### HarmonyOS 桥接（实验性）

```c
bool rw_harmony_bind_node(uint64_t node_handle, uint64_t widget_id);
void rw_harmony_clear_node_bindings(void);
uint64_t rw_harmony_lookup_widget_id(uint64_t node_handle);
bool rw_harmony_on_click(uint64_t widget_id);
bool rw_harmony_on_value_changed(uint64_t widget_id);
bool rw_harmony_on_widget_event(uint64_t widget_id, uint32_t kind_code);
bool rw_harmony_unbind_node(uint64_t node_handle);
```

### 错误处理（C）

```c
const char* rw_error_message(uint64_t handle);
int32_t rw_error_code(uint64_t handle);
```

### 内存管理（C）

```c
void rw_free_string(char* s);
void rw_free_rust_string(char* s);
```

### 绑定状态

```c
uint32_t rw_bindings_api_version(void);
uint32_t rw_cpp_binding_status(void);
uint32_t rw_java_binding_status(void);
uint32_t rw_python_binding_status(void);
```

### 移动端

```c
bool rw_mobile_attach_native_view(uint64_t native_handle);
const char* rw_mobile_backend_name(void);
```

---

## 坐标系参考

所有渲染和控件定位使用**屏幕坐标系**：

```
(0, 0) -------------> X（向右增加）
  |
  |    屏幕空间（像素）
  |    原点：左上角
  |
  v Y（向下增加）
```

**图表坐标**使用笛卡尔坐标系（Y 向上增加），自动转换。
**PDF 坐标**使用左下角原点，渲染时自动转换。
**SVG** 使用与屏幕坐标相同的左上角原点 — 无需转换。

`core::coords` 中的辅助函数：

| 函数 | 用途 |
|---|---|
| `to_screen_y(cartesian_y, height)` | 笛卡尔 Y → 屏幕 Y |
| `to_cartesian_y(screen_y, height)` | 屏幕 Y → 笛卡尔 Y |
| `to_pdf_y(screen_y, page_height)` | 屏幕 Y → PDF Y |

---

## 样式继承链

```
1. 全局主题默认值      (ThemeManager → Theme)
2. 主题覆盖            (按控件类，例如 "Button", "Label")
3. 控件实例状态        (在单个控件上设置的 WidgetStyle)
4. 内联样式覆盖        (未来)
```

每步在未设置时回退到下一步。使用 `WidgetStyle::inherit_from()` 手动组合样式。

---

## 线程安全说明

| 类型类别 | 安全性 |
|---|---|
| 控件句柄 | `Send + Sync`（由 `ObjectId` u64 支持） |
| 平台后端 | `Send + Sync` |
| 信号/槽 | 信号为 `Send + Sync`，闭包必须为 `Send + 'static` |
| 渲染后端 | 每个表面单线程访问 |
| 事件循环 | 单线程（事件循环线程） |
| I18nManager | `Send`（带 RwLock 的全局单例） |
| ThemeManager | `Send` |
| ObjectId | `Copy + Send + Sync` |
| ArenaAllocator | `Send` |
| MemoryMonitor | `Send + Sync` |

---

## 最低支持的 Rust 版本（MSRV）

**Rust 1.88** — 需要 `edition = "2021"` 和当前的依赖版本。
