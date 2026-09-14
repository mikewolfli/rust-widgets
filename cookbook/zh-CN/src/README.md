# 简介

**rust-widgets** 是一个纯 Rust 编写的跨平台 GUI 库，用于构建能够在各种环境中运行的应用——从桌面工作站到嵌入式微控制器，从移动设备到 Web，无处不在。

## ✨ 所有控件均为自绘

**rust-widgets 会自行绘制 100% 的控件，在任何平台上都不会创建操作系统的原生控件。**
这个 crate 中没有 `CreateWindowExW`、没有 `NSButton`、没有 `gtk_button_new`，
也没有 `android.widget.Button`。后端提供的是**绘制表面**、**事件循环**、
**输入转换**与**平台服务**（IME、剪贴板、无障碍、原生菜单、文件对话框、DPI）
—— 除此之外别无其他。

在继续阅读之前，有两个结果值得先内化：

1. **控件在每个操作系统上看起来完全相同。** 你的按钮在 Windows、macOS、Linux、iOS、Android 与 Web 上拥有相同的像素，因为它们都是由同一套 Rust 光栅化器绘制的。
2. **控件可用性属于*配置文件*问题，而非操作系统问题。** 在 `desktop`/`tablet`/`mobile` 配置文件中，全部 167 种控件在每个平台上都可用。只有资源受限的 `embedded`/`mini` 配置文件会编译一个精简的集合。

这就是为什么平台章节记录的是[每个操作系统*提供*什么](chapters/platform-support.md#13-平台服务确实因操作系统而异)（DPI、IME、无障碍、原生菜单），而不是列出哪些控件在哪些地方能用 —— 因为那张列表到处都一样。

## 什么是 rust-widgets？

rust-widgets 让您只需一套 Rust 代码库，即可在各大平台上生成一致的界面。它包含了丰富的控件库、硬件自适应渲染以及深度平台集成——这一切都通过简洁、地道的 Rust API 实现。

> 下文的代码片段仅用于说明预期的 API 形式。若需要能对 2.0.0 编译的代码，请从 [`chapters/getting-started.md`](chapters/getting-started.md) 开始，
> 该文件已针对当前的 crate 验证过。

```rust
use rust_widgets::prelude::*;

fn main() {
    let mut app = Application::new();
    let window = Window::builder()
        .title("Hello, rust-widgets!")
        .size(800, 600)
        .build();
    let button = Button::builder()
        .label("Click Me")
        .on_click(|_| println!("Hello, world!"))
        .build();
    window.set_content(button);
    app.run(window);
}
```

## 主要特性

### 丰富的控件库——167 种控件

内置 167 种控件，涵盖各种常见的 UI 需求，而且**全部都是自绘的，因此每个平台都能使用**：

- **核心控件**：Button、CheckBox、RadioButton、Label、LineEdit、TextEdit、ComboBox、SpinBox、Slider、ScrollBar、ProgressBar
- **容器**：Window、Dialog、TabWidget、Splitter、GroupBox、StackedWidget、DockWidget、MdiArea、ToolBox、CollapsiblePane
- **列表与视图**：ListView、TreeView、Table、Grid、Canvas
- **日期与时间**：Calendar、DatePicker、TimePicker、DateTimePicker、DateRangePicker
- **菜单**：MenuBar、ContextMenu、PieMenu、RibbonBar、DropdownMenu、Popover
- **移动端优先**：BottomNavigationBar、NavigationDrawer、AppBar、SafeArea、PullToRefresh、Cupertino 风格控件
- **输入**：MaskedEdit、AutoCompleteEdit、SearchBox、CommandPalette、KeySequenceEdit
- **显示**：LCDNumber、Dial、ProgressCircle、Rating、Sparkline、Badge、Chip、Avatar、SkeletonLoader
- **专用控件**：QRCode、VideoPlayer、CameraPreview、BarcodeScanner、MapView、TerminalView、MediaPlayer、CodeEditor、DiffViewer

### 硬件自适应渲染

三种渲染后端，根据目标平台自动选择：

| 后端 | 目标平台 | 描述 |
|---------|--------|-------------|
| **GPU (wgpu)** | 桌面、平板、移动设备 | 通过 wgpu 实现硬件加速渲染 |
| **SoftwarePaintBackend** | 嵌入式、mini 模式 | CPU 光栅化，输出 RGBA 帧缓冲 |
| **SvgPaintBackend** | 测试、文档 | SVG 流水线输出，用于像素级精确验证 |

### 九大平台，统一 API

下表列出的是**各平台如何提供绘图表面与事件循环**——而非哪些控件可用。
因为所有控件均为自绘，下列每个平台都支持全部 167 种控件；只有
`embedded`/`mini` 配置会减少编译进来的控件数量。

| 平台 | 后端提供 | Feature 标志 |
|----------|------------------|:------------:|
| Windows (Win32) | Win32 窗口 + 消息循环 | `windows` |
| macOS (Cocoa/objc2) | `NSView` 表面 | `macos` |
| Linux (GTK) | GTK3 窗口 + 事件循环 | `linux-gtk` |
| Linux (Wayland) | `wl_surface` + 输入 | `linux-wayland` |
| iOS (UIKit) | UIKit 表面 | `ios` |
| Android (JNI) | JNI 表面 | `android` |
| HarmonyOS | NAPI 桥接 | `harmony` |
| Web (WASM) | DOM canvas + 浏览器事件 | `wasm` |
| Portable / 无头 | 内存帧缓冲，无操作系统 | `embedded` / `mini` |

### 触控与手势

提供十一种手势识别器——Tap、DoubleTap、LongPress、Swipe、Pan、Fling、TwoFingerTap、TwoFingerSwipe、LongPressDrag、Pinch 和 Rotate——并自动扩展触控目标，确保在小屏幕上的无障碍体验。

### 国际化

`tr!()` 宏提供编译期基于键值的翻译功能，支持英语、简体中文和繁体中文，同时支持上下文变体和复数形式。配套的覆盖率审计工具（`audit_keys()`）可在构建时捕获缺失的翻译。

### 图表与数据可视化

内置图表控件——LineChart、BarChart、PieChart 和 Sparkline——直接通过同一渲染管线绘制，无需任何外部图表依赖。

### PDF 与打印

通过统一 API 生成 PDF 文档并发送打印任务到系统打印服务。基于 SVG 流水线的精确输出确保屏幕显示的内容与打印结果完全一致。

### 无障碍

`a11y` 特性集成了平台无障碍 API（Linux 上通过 zbus 使用 AT-SPI），将控件树暴露给屏幕阅读器和辅助技术。

### Web 引擎

完整的 WebView 集成，支持设置管理、Cookie 存储、下载处理、WebChannel 通信以及上下文菜单定制。

## 设计理念

- **公开 API 中零 `unsafe`。** 所有 `unsafe` 代码块都被严格限制在平台 FFI 边界内，并经过全面验证和 panic 安全处理。
- **嵌入式 `no_std` 支持。** 同一套代码库通过条件编译同时服务于 std 和 `no_std` 目标。`compat.rs` 桥接层将 std 类型（`HashMap`、`Mutex`）映射为基于竞技场分配和低资源适用的替代方案（`BTreeMap`、`RefCell`、`MiniVec`、`MiniString`）。
- **模块化特性系统。** 三个独立的维度——设备配置（Device Profile）、操作系统后端（OS Backend）和能力（Capabilities）——让您能够精确组合所需的目标二进制文件。只有在使用时才引入图表、打印或国际化等特性。
- **处处皆 Builder 模式。** 通过 Rust 的类型系统实现编译期验证。每个控件、每种样式和布局都采用符合人体工学的 builder API。

## 本手册涵盖的内容

| 章节 | 主题 |
|---------|--------|
| **快速入门** | 环境搭建、第一个应用、项目模板 |
| **架构概述** | 分层模型、特性系统、crate 结构 |
| **核心类型** | `Widget`、`Style`、`Color`、`Rect`、`Size`、信号 |
| **控件系统** | 控件生命周期、组合、自定义控件 |
| **布局系统** | Box、Grid、Stack、Flow、Absolute、Masonry 布局 |
| **事件系统** | 事件循环、输入处理、手势识别 |
| **渲染系统** | GPU/CPU/SVG 后端、脏区域、局部刷新 |
| **样式与主题** | CSS 引擎、主题、热重载、`StyleSheetManager` |
| **平台支持** | 各平台设置、条件编译、后端 |
| **语言绑定** | C ABI、Python、Java/JNI、C++ 集成 |
| **国际化** | `tr!()` 宏、翻译文件、复数规则 |
| **图表与数据可视化** | LineChart、BarChart、PieChart、Sparkline |
| **PDF 与打印** | 文档生成、系统打印服务 |
| **性能与质量** | 基准测试、SVG 回归测试、性能分析 |
| **内存管理** | 竞技场分配、`no_std` 内存模型、泄漏检测 |
| **嵌入式支持** | `no_std` 配置、软件光栅化、资源约束 |
| **Web 引擎** | WebView 设置、通道、安全 |
| **高级主题** | 自定义后端、unsafe FFI、异步集成 |
| **API 参考** | 模块级文档、trait 参考、类型索引 |

## 前置要求

- **Rust 1.87** 或更新版本（MSRV）
- **平台依赖**：
  | 平台 | 依赖项 |
  |----------|-------------|
  | Linux (GTK) | `libgtk-3-dev` |
  | Linux (Wayland) | `libwayland-dev`、`wayland-protocols` |
  | macOS / iOS | Xcode Command Line Tools |
  | Windows | Visual Studio Build Tools (MSVC) |
  | Android | Android NDK、`cargo-ndk` |
  | WASM | `wasm-bindgen-cli`、`wasm-pack` |

## 项目状态

| | |
|---|---|
| **版本** | 2.0.0 |
| **许可证** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **仓库** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **测试** | 4000+ |
| **MSRV** | Rust 1.87 |

准备好了吗？前往[快速入门](chapters/getting-started.md)开始吧。
