# Introduction

**rust-widgets** is a pure Rust, cross-platform GUI library for building
applications that run everywhere — from desktop workstations to embedded
microcontrollers, and from mobile devices to the web.

## All Controls Are Self-Drawn

**rust-widgets paints 100% of its own controls and creates no native OS controls**
**on any platform.** There is no `CreateWindowExW`, no `NSButton`, no
`gtk_button_new` and no `android.widget.Button` in this crate. A backend supplies
a **surface**, an **event loop**, **input translation** and **platform services**
(IME, clipboard, accessibility, native menus, file dialogs, DPI) — and nothing
else.

Two consequences worth internalising before you read further:

1. **A control looks identical on every OS.** Your button has the same pixels on
   Windows, macOS, Linux, iOS, Android and the web, because the same Rust
   rasterizer drew all of them.
2. **Widget availability is a *profile* question, not an OS question.** All 180
   widget kinds are available on every platform in the `desktop`/`tablet`/`mobile`
   profiles. Only the resource-constrained `embedded`/`mini` profiles compile a
   reduced set.

This is why the platform chapter documents [what each OS
*provides*](chapters/platform-support.md#13-platform-services-do-vary-by-os)
(DPI, IME, accessibility, native menus) rather than listing which controls work
where — because that list would be the same everywhere.

## What Is rust-widgets?

rust-widgets gives you a single Rust codebase that produces identical, consistent
interfaces on every major platform. It includes an extensive widget library,
hardware-adaptive rendering, and deep platform integration — all through a
clean, idiomatic Rust API.

> The snippet below is illustrative of the intended API shape. For code that
> compiles against 2.6.0, start from [`chapters/getting-started.md`](chapters/getting-started.md),
> which is verified against the current crate.

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

### Rich Widget Library — 180 Widget Kinds

180 built-in widget kinds span every common UI need, and **all of them are
self-drawn and therefore available on every platform**:

- **Core controls**: Button, CheckBox, RadioButton, Label, LineEdit, TextEdit,
  ComboBox, SpinBox, Slider, ScrollBar, ProgressBar
- **Containers**: Window, Dialog, Frame, TabWidget, Splitter, GroupBox, StackedWidget,
  DockWidget, MdiArea, ToolBox, CollapsiblePane
- **Lists & views**: ListView, TreeView, TreeTable, Table, Grid, Canvas
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
  TerminalView, MediaPlayer, CodeEditor, DiffViewer, SignaturePad, DropZone,
  Breadcrumb

### Hardware-Adaptive Rendering

Three rendering backends, automatically selected for your target:

| Backend | Target | Description |
|---------|--------|-------------|
| **GPU (wgpu)** | Desktop, tablet, mobile | Hardware-accelerated rendering via wgpu |
| **SoftwarePaintBackend** | Embedded, mini | CPU rasterizer to RGBA framebuffer |
| **SvgPaintBackend** | Testing, docs | SVG pipeline output for pixel-accurate verification |

### Nine Platforms, One API

The table below lists **how each platform provides a surface and event loop** — not
which controls are available. Because every control is self-drawn, all 180 widget
kinds work on every platform listed; only the `embedded`/`mini` profiles reduce the
compiled-in set.

| Platform | Backend supplies | Feature Flag |
|----------|------------------|:------------:|
| Windows (Win32) | Win32 window + message loop | `windows` |
| macOS (Cocoa/objc2) | `NSView` surface | `macos` |
| Linux (GTK) | GTK3 window + event loop | `linux-gtk` |
| Linux (Wayland) | `wl_surface` + input | `linux-wayland` |
| iOS (UIKit) | UIKit surface | `ios` |
| Android (JNI) | JNI surface | `android` |
| HarmonyOS | NAPI bridge | `harmony` |
| Web (WASM) | DOM canvas + browser events | `wasm` |
| Portable / headless | In-memory framebuffer, no OS | `embedded` / `mini` |

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
| **Version** | 2.6.0 |
| **License** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **Repository** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **Tests** | 5300+ |
| **MSRV** | Rust 1.87 |

Ready to begin? Head to [Getting Started](chapters/getting-started.md).
