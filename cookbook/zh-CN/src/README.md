# 简介

**rust-widgets** 是一个纯 Rust 编写的跨平台原生 GUI 库，用于构建能够在各种环境中运行的应用——从桌面工作站到嵌入式微控制器，从移动设备到 Web，无处不在。

## 什么是 rust-widgets？

rust-widgets 让您只需一套 Rust 代码库，即可在各大平台上生成原生风格的用户界面。它包含了丰富的控件库、硬件自适应渲染以及深度平台集成——这一切都通过简洁、地道的 Rust API 实现。

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

### 丰富的控件库——140 多个控件

超过 140 个内置控件，覆盖各种常见的 UI 需求：

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

### 八大平台，统一 API

| 平台 | 后端 | Feature 标志 |
|----------|---------|:------------:|
| Linux (Wayland) | `linux-wayland` | `linux-wayland` |
| Windows (Win32) | `windows` | `windows` |
| macOS (Cocoa/objc2) | `macos` | `macos` |
| iOS (UIKit) | `ios` | `ios` |
| Android (JNI) | `android` | `android` |
| Web (WASM) | `wasm` | `wasm` |
| HarmonyOS | `harmony` | `harmony` |
| 嵌入式 (no_std) | `embedded` / `mini` | `embedded` / `mini` |

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
| **版本** | 1.0.0 |
| **许可证** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **仓库** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **测试** | 3400+ |
| **MSRV** | Rust 1.87 |

准备好了吗？前往[快速入门](chapters/getting-started.md)开始吧。
