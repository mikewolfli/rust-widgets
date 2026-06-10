# rust_widgets — 纯 Rust GUI 库

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

纯 Rust 编写的跨平台原生 GUI 库。支持桌面、平板、手机、嵌入式以及 **no_std mini** 目标。

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![tests](https://img.shields.io/badge/tests-3400%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

<p align="center">
  <a href="README.md">
    <img src="https://img.shields.io/badge/lang-English-blue" alt="English">
  </a>
</p>

---

## 快速开始

```bash
# 桌面（默认）
cargo check

# Mini（no_std，30+ 控件）
cargo check --no-default-features --features mini

# 嵌入式
cargo check --no-default-features --features embedded

# 测试
cargo test --lib
```

### 设备配置

| 配置 | 命令 | 渲染后端 | 控件数 | i18n | GPU |
|------|------|----------|--------|------|-----|
| 桌面 | `cargo check` | 原生 OS | 80+ | ✅ | ✅ wgpu |
| 平板 | `--features tablet` | 原生 OS | 80+ | ✅ | ✅ wgpu |
| 手机 | `--features mobile` | 手机 API | 80+ | ✅ | ✅ wgpu |
| 嵌入式 | `--features embedded` | 软件 | 30+ | — | — |
| **Mini** | `--features mini` | **no_std** + alloc | **30+** | — | — |

### 操作系统支持

| 系统 | 特性 | 自动检测 |
|------|------|:--------:|
| Windows (Win32) | `windows` | ✅ |
| macOS (Cocoa/objc2) | `macos` | ✅ |
| iOS (UIKit) | `ios` | ✅ |
| Linux (GTK) | `linux-gtk` | — |
| Linux (Wayland) | `linux-wayland` | — |
| Android (JNI) | `android` | ✅ |
| Web (WASM) | `wasm` | — |
| HarmonyOS | `harmony` | — |

---

## 架构

```
┌────────────────────────────────────────────────────────────┐
│  API 层 — lib.rs + compat.rs (no_std 桥接)                │
├────────────────────────────────────────────────────────────┤
│  控件库  │  事件系统    │  布局引擎                         │
│  (30-80) │  (EventLoop,│  (Box, Grid, Flow,               │
│          │   Gesture)  │   Stack, Absolute)                │
├──────────┴─────────────┴──────────────────────────────────┤
│  i18n │ 主题 │ 信号系统 │ 控制后端                          │
├────────────────────────────────────────────────────────────┤
│  渲染：SoftwarePaintBackend / SvgPaintBackend / GPU         │
├────────────────────────────────────────────────────────────┤
│  平台：Windows │ macOS │ Linux │ iOS │ Android │ WASM      │
└────────────────────────────────────────────────────────────┘
```

---

## 特性

### Rust 原生设计
- 条件 `no_std`：`#![cfg_attr(feature = "mini", no_std)]` — 同一代码库同时支持 std 和嵌入式
- `compat.rs` 桥接：`HashMap→BTreeMap`，`Mutex→RefCell`，`MiniVec<T,64>`，`MiniString<256>`，`MiniArena`
- `enum WidgetKind` + `trait Widget/Draw/EventHandler` — 零成本抽象
- Builder 模式：`Style::new().bg_color(RED).pad_all(8).build()`

### 渲染后端
- **SoftwarePaintBackend**：CPU 光栅化（RGBA 帧缓冲），用于 mini/嵌入式
- **SvgPaintBackend**：SVG 管线输出，用于测试和文档
- **GPU (wgpu)**：硬件加速，用于桌面/平板/手机

### 触摸与手势
- 11 个手势识别器：点击、双击、长按、滑动、平移、甩动、双指点击、双指滑动、长按拖拽、捏合、旋转
- 触摸目标自动扩展，适配小控件

### 布局
- Box、HBox、VBox、Grid、Form、Stack、Flow、Absolute、Anchor、Masonry
- 设备自适应布局缩放、字体缩放、最小触摸尺寸

### CSS 样式
- CSS 解析器 + 选择器引擎（`CssParser`、`CssSelector`）
- `Widget::apply_css(css, class)` — 控件级 CSS 应用
- `StyleSheetManager` — 全局样式表注册
- `CssWatcher` — 轮询式 CSS 热加载

### 局部刷新
- `DirtyRegionTracker` 脏矩形追踪与合并
- `render_dirty_regions()` — 基于裁剪区域的局部重绘

### 国际化（i18n）
- `tr!()` 宏实现编译期键值翻译
- 中 / 英 / 繁 翻译包（各 30+ UI 字符串）
- 上下文翻译与复数形式支持
- `audit_keys()` 翻译覆盖率验证

---

## 控件库

### 桌面/平板/手机（80+ 控件）

**核心**：Window、Dialog、MessageBox、FileDialog、ColorDialog、FontDialog、InputDialog、ProgressDialog、PopupWindow、Button、CheckBox、RadioButton、Label、LineEdit、TextEdit、RichEdit、ComboBox、SpinBox、ListBox、ListView、TreeView、ProgressBar、Slider、ScrollBar、ScrollArea、TabWidget、Splitter、GroupBox、MenuBar、Menu、MenuItem、ContextMenu、ToolBar、StatusBar、Canvas、Table、Grid、Chart、ToggleButton

**日期与时间**：Calendar、DateEdit、TimeEdit、DateTimeEdit、DatePicker、TimePicker、DateTimePicker、CupertinoDatePicker、DateRangePicker、MobileDatePicker

**容器**：CollapsiblePane、DockWidget、MdiArea、StackedWidget、ToolBox、TabBar、NavigationStack、PagerPageView、Carousel、BottomSheet、ModalBottomSheet

**移动端**：BottomNavigationBar、NavigationDrawer、AppBar、SafeArea、PullToRefresh、RefreshControl、SearchBar、CupertinoSwitch、CupertinoSlider、CupertinoNavigationBar、CupertinoSegmentedControl、AdaptiveScaffold

**输入**：CommandLink、FontComboBox、KeySequenceEdit、MaskedEdit、AutoCompleteEdit、MultiSelectComboBox、EditableComboBox、RangeSlider、FloatingLabel、TagInput、InplaceEditor、SearchBox、ShortcutEditor

**显示**：LCDNumber、Dial、ProgressCircle、Rating、Icon、Sparkline、Tooltip、Badge、Chip、Avatar、SkeletonLoader、EmptyState

**图表**：LineChart、BarChart、PieChart、Sparkline

**网页**：WebView、WebEngineView、WebEnginePage、WebEngineSettings、WebEngineDownloadItem、WebEngineCookieStore、WebEngineWebChannel

**菜单**：PieMenu、RibbonBar、MenuButton、DropdownMenu、Popover、SegmentedButton

### Mini（30+ 控件，no_std）

Window、Dialog、PopupWindow、Button、CheckBox、RadioButton、Label、LineEdit、ComboBox、SpinBox、ListBox、ProgressBar、Slider、ScrollBar、ScrollArea、GroupBox、Menu、MenuItem、ToggleButton、Switch、Arc、Spinner、Roller、Dropdown、TextArea、Keyboard、TileView、Line、Meter、MiniChart、ImageView、MiniCanvas、TabView、AnimatedImage

---

## C ABI 与语言绑定

```bash
cargo build --release
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/release -lrust_widgets -o target/release/c_abi_poll_demo
python examples/python/demo_basic.py
```

| 语言 | 状态 |
|------|:----:|
| C | ✅ |
| C++ | ✅ |
| Python | ✅ |
| Java (JNI) | ✅ |

---

## 核心模块

| 模块 | 说明 | 可用范围 |
|------|------|:--------:|
| `core` | Point、Rect、Size、Color、Font、ObjectId | 全部 |
| `widget` | 控件实现 | 全部 |
| `event` | 事件类型、EventLoop、GestureEngine | 全部 |
| `compat` | std↔no_std 桥接、MiniVec、MiniString、MiniArena | 全部 |
| `render` | SoftwarePaintBackend、SvgPaintBackend、GPU | 全部 |
| `layout` | Box、Grid、Flow、Stack、Absolute、Masonry | 全部 |
| `signal` | GenericSignal、Signal1、ConnectionScope | 全部 |
| `style` | WidgetStyle、CSS 解析器、动画、主题状态 | 全部 |
| `object` | 对象/类名系统 | 全部 |
| `platform` | Windows、macOS、Linux、iOS、Android、WASM、Harmony | 桌面+ |
| `gesture` | 11 个手势识别器 | 桌面+ (touch) |
| `i18n` | `tr!()` 宏、I18nManager、中/英/繁 | 桌面+ |
| `theme` | 主题管理器、深色/浅色模式 | 桌面+ |
| `gpu` | GPU 适配器检测、缓冲池 | 桌面+ |
| `chart` | 折线图、柱状图、饼图、散点图 | 桌面+ |
| `web` | WebEngine、WebView、JS 引擎 | 桌面+ |
| `pdf` | PDF 文档创建 | 桌面+ |
| `print` | 打印支持 | 桌面+ |

---

## 构建要求

| 配置 | Rust 版本 | 依赖 |
|------|:---------:|------|
| 桌面 | 1.87+ | wgpu、GTK/Wayland (Linux)、objc2 (macOS) |
| Mini | 1.87+ | heapless、hashbrown、bumpalo (no_std) |
| 嵌入式 | 1.87+ | 无 |

---

## 许可

MIT License — 详见 [LICENSE](LICENSE)。

## 支持

- Issues：[GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)
- 文档：[docs/](docs/) 目录
