# Introduction

**rust-widgets** is a pure Rust, cross-platform native GUI library for building
applications that run everywhere — from desktop workstations to embedded
microcontrollers, and from mobile devices to the web.

## What Is rust-widgets?

rust-widgets gives you a single Rust codebase that produces native-looking
interfaces on every major platform. It includes an extensive widget library,
hardware-adaptive rendering, and deep platform integration — all through a
clean, idiomatic Rust API.

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

## Key Features

### Rich Widget Library — 140+ Widgets

Over 140 built-in widgets span every common UI need:

- **Core controls**: Button, CheckBox, RadioButton, Label, LineEdit, TextEdit,
  ComboBox, SpinBox, Slider, ScrollBar, ProgressBar
- **Containers**: Window, Dialog, TabWidget, Splitter, GroupBox, StackedWidget,
  DockWidget, MdiArea, ToolBox, CollapsiblePane
- **Lists & views**: ListView, TreeView, Table, Grid, Canvas
- **Date & time**: Calendar, DatePicker, TimePicker, DateTimePicker,
  DateRangePicker
- **Menus**: MenuBar, ContextMenu, PieMenu, RibbonBar, DropdownMenu, Popover
- **Mobile-first**: BottomNavigationBar, NavigationDrawer, AppBar, SafeArea,
  PullToRefresh, Cupertino-style controls
- **Input**: MaskedEdit, AutoCompleteEdit, SearchBox, CommandPalette,
  KeySequenceEdit
- **Display**: LCDNumber, Dial, ProgressCircle, Rating, Sparkline, Badge, Chip,
  Avatar, SkeletonLoader
- **Specialized**: QRCode, VideoPlayer, CameraPreview, BarcodeScanner, MapView,
  TerminalView, MediaPlayer, CodeEditor, DiffViewer

### Hardware-Adaptive Rendering

Three rendering backends, automatically selected for your target:

| Backend | Target | Description |
|---------|--------|-------------|
| **GPU (wgpu)** | Desktop, tablet, mobile | Hardware-accelerated rendering via wgpu |
| **SoftwarePaintBackend** | Embedded, mini | CPU rasterizer to RGBA framebuffer |
| **SvgPaintBackend** | Testing, docs | SVG pipeline output for pixel-accurate verification |

### Eight Platforms, One API

| Platform | Backend | Feature Flag |
|----------|---------|:------------:|
| Linux (Wayland) | `linux-wayland` | `linux-wayland` |
| Windows (Win32) | `windows` | `windows` |
| macOS (Cocoa/objc2) | `macos` | `macos` |
| iOS (UIKit) | `ios` | `ios` |
| Android (JNI) | `android` | `android` |
| Web (WASM) | `wasm` | `wasm` |
| HarmonyOS | `harmony` | `harmony` |
| Embedded (no_std) | `embedded` / `mini` | `embedded` / `mini` |

### Touch & Gesture

Eleven gesture recognizers — Tap, DoubleTap, LongPress, Swipe, Pan, Fling,
TwoFingerTap, TwoFingerSwipe, LongPressDrag, Pinch, and Rotate — with automatic
touch-target expansion for accessibility on small screens.

### Internationalization

The `tr!()` macro provides compile-time key-based translation with support for
English, Simplified Chinese, and Traditional Chinese, plus context-based and
plural variants. A coverage auditor (`audit_keys()`) catches missing
translations at build time.

### Charts & Data Visualization

Built-in chart widgets — LineChart, BarChart, PieChart, and Sparkline — render
directly through the same rendering pipeline, with no external charting
dependency.

### PDF & Printing

Generate PDF documents and send jobs to system print services through a unified
API. SVG-pipeline-accurate output ensures what you see on screen matches what
prints.

### Accessibility

The `a11y` feature integrates with platform accessibility APIs (AT-SPI on
Linux via zbus) to expose widget trees to screen readers and assistive
technologies.

### Web Engine

Full WebView integration with settings management, cookie store, download
handling, WebChannel communication, and context menu customization.

## Design Philosophy

- **Zero `unsafe` in the public API.** All `unsafe` blocks are confined to
  platform FFI boundaries with exhaustive validation and panic safety.
- **`no_std` support for embedded.** A single codebase serves both std and
  `no_std` targets via conditional compilation. The `compat.rs` bridge maps
  std types (`HashMap`, `Mutex`) to arena-allocated and heapless alternatives
  (`BTreeMap`, `RefCell`, `MiniVec`, `MiniString`).
- **Modular feature system.** Three independent axes — Device Profile, OS
  Backend, and Capabilities — let you compose exactly the binary you need.
  Pull in charts, printing, or i18n only when you use them.
- **Builder pattern everywhere.** Compile-time validation through Rust's type
  system. Every widget, style, and layout uses an ergonomic builder API.

## What This Cookbook Covers

| Chapter | Topics |
|---------|--------|
| **Getting Started** | Setup, first app, project templates |
| **Architecture Overview** | Layer model, feature system, crate structure |
| **Core Types** | `Widget`, `Style`, `Color`, `Rect`, `Size`, signals |
| **Widget System** | Widget lifecycle, composition, custom widgets |
| **Layout System** | Box, Grid, Stack, Flow, Absolute, Masonry layouts |
| **Event System** | Event loop, input handling, gesture recognition |
| **Rendering System** | GPU/CPU/SVG backends, dirty regions, partial refresh |
| **Styling & Theming** | CSS engine, themes, hot-reload, `StyleSheetManager` |
| **Platform Support** | Per-platform setup, conditional compilation, backends |
| **Language Bindings** | C ABI, Python, Java/JNI, C++ integration |
| **Internationalization** | `tr!()` macro, translation files, plural rules |
| **Charts & Data Visualization** | LineChart, BarChart, PieChart, Sparkline |
| **PDF & Printing** | Document generation, system print services |
| **Performance & Quality** | Benchmarking, SVG regression tests, profiling |
| **Memory Management** | Arena allocation, `no_std` memory model, leak detection |
| **Embedded Support** | `no_std` profile, software raster, resource constraints |
| **Web Engine** | WebView setup, settings, channels, security |
| **Advanced Topics** | Custom backends, unsafe FFI, async integration |
| **API Reference** | Module-level docs, trait reference, type index |

## Prerequisites

- **Rust 1.87** or later (MSRV)
- **Platform dependencies**:
  | Platform | Dependencies |
  |----------|-------------|
  | Linux (GTK) | `libgtk-3-dev` |
  | Linux (Wayland) | `libwayland-dev`, `wayland-protocols` |
  | macOS / iOS | Xcode Command Line Tools |
  | Windows | Visual Studio Build Tools (MSVC) |
  | Android | Android NDK, `cargo-ndk` |
  | WASM | `wasm-bindgen-cli`, `wasm-pack` |

## Project Status

| | |
|---|---|
| **Version** | 1.0.0 |
| **License** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **Repository** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **Tests** | 3400+ |
| **MSRV** | Rust 1.87 |

Ready to begin? Head to [Getting Started](chapters/getting-started.md).
