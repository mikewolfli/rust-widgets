# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

Cross-platform native GUI library in pure Rust. Hardware-adaptive rendering, widget library, touch/gesture support, i18n, and SVG output. Supports desktop, tablet, mobile, embedded, and **no_std mini** targets.

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![tests](https://img.shields.io/badge/tests-3400%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

<p align="center">
  <a href="README.zh-CN.md">
    <img src="https://img.shields.io/badge/%E4%B8%AD%E6%96%87-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-blue" alt="简体中文">
  </a>
</p>

---

## Quick Start

```bash
# Desktop (default)
cargo check

# Mini (no_std, 30+ widgets)
cargo check --no-default-features --features mini

# Embedded
cargo check --no-default-features --features embedded

# Tests
cargo test --lib
```

### Device Profiles

| Profile | Command | Backend | Widgets | i18n | GPU |
|---------|---------|---------|---------|------|-----|
| Desktop | `cargo check` | Native OS | 80+ | ✅ | ✅ wgpu |
| Tablet | `--features tablet` | Native OS | 80+ | ✅ | ✅ wgpu |
| Mobile | `--features mobile` | Mobile API | 80+ | ✅ | ✅ wgpu |
| Embedded | `--features embedded` | Software | 30+ | — | — |
| **Mini** | `--features mini` | **no_std** + alloc | **30+** | — | — |

### OS Backends

| OS | Feature | Auto-detect |
|----|---------|:-----------:|
| Windows (Win32) | `windows` | ✅ |
| macOS (Cocoa/objc2) | `macos` | ✅ |
| iOS (UIKit) | `ios` | ✅ |
| Linux (GTK) | `linux-gtk` | — |
| Linux (Wayland) | `linux-wayland` | — |
| Android (JNI) | `android` | ✅ |
| Web (WASM) | `wasm` | — |
| HarmonyOS | `harmony` | — |

---

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│  API Layer — lib.rs + compat.rs (no_std bridge)           │
├────────────────────────────────────────────────────────────┤
│  Widgets  │  Event System  │  Layout Engine                │
│  (30-80)  │  (EventLoop,   │  (Box, Grid, Flow,           │
│           │   Gesture)     │   Stack, Absolute)            │
├───────────┴────────────────┴──────────────────────────────┤
│  i18n  │  Theme  │  Signal System  │  Control Backend       │
├────────────────────────────────────────────────────────────┤
│  Rendering: SoftwarePaintBackend / SvgPaintBackend / GPU   │
├────────────────────────────────────────────────────────────┤
│  Platform: Windows │ macOS │ Linux │ iOS │ Android │ WASM  │
└────────────────────────────────────────────────────────────┘
```

---

## Features

### Rust-Native Design
- Conditional `no_std` via `#![cfg_attr(feature = "mini", no_std)]` — single codebase for both std and embedded
- `compat.rs` bridge: `HashMap→BTreeMap`, `Mutex→RefCell`, `MiniVec<T,64>`, `MiniString<256>`, `MiniArena` (bumpalo)
- `enum WidgetKind` + `trait Widget` + `trait Draw` + `trait EventHandler` — zero-cost abstractions
- Builder pattern: `Style::new().bg_color(RED).pad_all(8).build()` — compile-time checking

### Rendering Backends
- **SoftwarePaintBackend**: CPU rasterizer (RGBA framebuffer), used by mini/embedded
- **SvgPaintBackend**: SVG pipeline output for testing and documentation
- **GPU (wgpu)**: Hardware-accelerated for desktop/tablet/mobile

### Touch & Gesture
- 11 gesture recognizers: Tap, DoubleTap, LongPress, Swipe, Pan, Fling, TwoFingerTap, TwoFingerSwipe, LongPressDrag, Pinch, Rotate
- Touch-target expansion for small widgets on touch devices

### Layout
- Box, HBox, VBox, Grid, Form, Stack, Flow, Absolute, Anchor, Masonry
- Device-adaptive layout scale, font scale, and minimum touch size

### CSS Styling
- CSS parser + selector engine (`CssParser`, `CssSelector`)
- `Widget::apply_css(css, class)` — per-widget CSS application
- `StyleSheetManager` — global stylesheet registration
- `CssWatcher` — polling-based CSS hot-reload

### Partial Refresh
- `DirtyRegionTracker` with rectangle merging
- `render_dirty_regions()` — clip-based partial redraw via `push_clip/pop_clip`

### Internationalization
- `tr!()` macro for compile-time key-based translation
- en / zh-cn / zh-tw translations (30+ strings per language)
- Context-based and plural variants
- `audit_keys()` for coverage validation

---

## Widget Library

### Desktop/Tablet/Mobile (80+ widgets)

**Core**: Window, Dialog, MessageBox, FileDialog, ColorDialog, FontDialog, InputDialog, ProgressDialog, PopupWindow, Button, CheckBox, RadioButton, Label, LineEdit, TextEdit, RichEdit, ComboBox, SpinBox, ListBox, ListView, TreeView, ProgressBar, Slider, ScrollBar, ScrollArea, TabWidget, Splitter, GroupBox, MenuBar, Menu, MenuItem, ContextMenu, ToolBar, StatusBar, Canvas, Table, Grid, Chart, ToggleButton

**Date & Time**: Calendar, DateEdit, TimeEdit, DateTimeEdit, DatePicker, TimePicker, DateTimePicker, CupertinoDatePicker, DateRangePicker, MobileDatePicker

**Containers**: CollapsiblePane, DockWidget, MdiArea, StackedWidget, ToolBox, TabBar, NavigationStack, PagerPageView, Carousel, BottomSheet, ModalBottomSheet

**Mobile**: BottomNavigationBar, NavigationDrawer, AppBar, SafeArea, PullToRefresh, RefreshControl, SearchBar, CupertinoSwitch, CupertinoSlider, CupertinoNavigationBar, CupertinoSegmentedControl, AdaptiveScaffold

**Input**: CommandLink, FontComboBox, KeySequenceEdit, MaskedEdit, AutoCompleteEdit, MultiSelectComboBox, EditableComboBox, RangeSlider, FloatingLabel, TagInput, InplaceEditor, SearchBox, ShortcutEditor

**Display**: LCDNumber, Dial, ProgressCircle, Rating, Icon, Sparkline, Tooltip, Badge, Chip, Avatar, SkeletonLoader, EmptyState

**Charts**: LineChart, BarChart, PieChart, Sparkline

**Web**: WebView, WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem, WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult, WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest

**Menus**: PieMenu, RibbonBar, MenuButton, DropdownMenu, Popover, SegmentedButton

**Special**: FreeformShape, QRCode, ColorHistory, ColorWell, MasonryLayout, Stepper, Divider, SwipeToDismiss, Toolbox, PropertiesPanel, PropertyGrid, WizardDialog, Wizard, AnimatedImage, HeroAnimation, BezierCurveEditor, LottieWidget, RiveWidget, VideoPlayer, ImageGallery, AudioVisualizer, CameraPreview, BarcodeScanner, Breakcrumb, CodeEditor, ColorPicker, CommandEntry, CommandPalette, DiffViewer, MapView, MediaPlayer, NotificationCenter, Snackbar, SplitButton, TerminalView, ToastStack

### Mini (30+ widgets, no_std)

Window, Dialog, PopupWindow, Button, CheckBox, RadioButton, Label, LineEdit, ComboBox, SpinBox, ListBox, ProgressBar, Slider, ScrollBar, ScrollArea, GroupBox, Menu, MenuItem, ToggleButton, Switch, Arc, Spinner, Roller, Dropdown, TextArea, Keyboard, TileView, Line, Meter, MiniChart, ImageView, MiniCanvas, TabView, AnimatedImage

---

## C ABI & Language Bindings

```bash
cargo build --release
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/release -lrust_widgets -o target/release/c_abi_poll_demo
python examples/python/demo_basic.py
```

| Language | Status |
|----------|:------:|
| C | ✅ |
| C++ | ✅ |
| Python | ✅ |
| Java (JNI) | ✅ |

---

## Core Modules

| Module | Description | Availability |
|--------|-------------|:------------:|
| `core` | Point, Rect, Size, Color, Font, ObjectId | All profiles |
| `widget` | Widget implementations | All profiles |
| `event` | Event types, EventLoop, GestureEngine | All profiles |
| `compat` | std↔no_std bridge, MiniVec, MiniString, MiniArena | All profiles |
| `render` | SoftwarePaintBackend, SvgPaintBackend, GPU (wgpu) | All profiles |
| `layout` | Box, Grid, Flow, Stack, Absolute, Anchor, Masonry | All profiles |
| `signal` | GenericSignal, Signal1, ConnectionScope | All profiles |
| `style` | WidgetStyle, CSS parser, animations, theme states | All profiles |
| `object` | Object/class-name system | All profiles |
| `platform` | Windows, macOS, Linux, iOS, Android, WASM, Harmony | Desktop+ |
| `gesture` | 11 gesture recognizers | Desktop+ (touch) |
| `i18n` | `tr!()` macro, I18nManager, en/zh-cn/zh-tw | Desktop+ |
| `theme` | Theme manager, dark/light mode | Desktop+ |
| `gpu` | GPU adapter detection, buffer pools | Desktop+ |
| `chart` | Line, Bar, Pie, Scatter, Area charts | Desktop+ |
| `web` | WebEngine, WebView, JS engine | Desktop+ |
| `pdf` | PDF document creation | Desktop+ |
| `print` | Print support | Desktop+ |
| `performance` | Profiler, frame rate monitor | Desktop+ |
| `memory` | ObjectPool, ArenaAllocator, BufferPool | Desktop+ |

---

## Build Requirements

| Profile | Rust Version | Dependencies |
|---------|:------------:|--------------|
| Desktop | 1.87+ | wgpu, GTK/Wayland (Linux), objc2 (macOS) |
| Mini | 1.87+ | heapless, hashbrown, bumpalo (no_std) |
| Embedded | 1.87+ | None (software-only) |

---

## Performance

| Metric | Desktop | Mini (target) |
|--------|---------|---------------|
| Binary size | ~5MB | < 100KB |
| RAM (typical) | < 100MB | < 32KB |
| Frame rate | 60 FPS | 30 FPS |
| Widget creation | < 1ms | < 0.1ms |

---

## License

MIT License — see [LICENSE](LICENSE).

## Support

- Issues: [GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)
- Documentation: [docs/](docs/) directory
