# rust_widgets

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

**Pure Rust cross-platform native GUI library** — v0.9.6

Hardware-adaptive rendering, comprehensive 60+ widget library, touch/gesture support, full i18n, and SVG-pipeline-accurate output.

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![tests](https://img.shields.io/badge/tests-1375%20passing-brightgreen)]()
[![clippy](https://img.shields.io/badge/clippy-0%20warnings-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

---

## Quick Start

```bash
# Check (all features, all targets)
cargo check --all-features --all-targets

# Run the main demo
cargo run --example demo_basic

# Full test suite
cargo test --all-features -q

# Code quality (must pass — CI enforces zero warnings)
cargo clippy --all-features --all-targets -- -D warnings

# Formatting check
cargo fmt --all -- --check
```

### Feature Profiles

| Profile | Command | Description |
|---------|---------|-------------|
| Desktop (default) | `cargo check` | Full desktop: native platform, GPU, i18n, theme, touch, all widgets |
| Tablet | `cargo check --no-default-features --features tablet` | Touch-first, GPU, native + custom controls |
| Mobile | `cargo check --no-default-features --features mobile` | Touch, GPU, mobile API bindings |
| Embedded | `cargo check --no-default-features --features embedded` | Stripped: software-only, no i18n/touch |
| Desktop + Touch | `cargo check --features "desktop,touch"` | Desktop with touch input |
| Full + Holographic | `cargo check --features "full,holographic"` | All features including experimental |

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    API Layer (lib.rs)                 │
├──────────────────────────────────────────────────────┤
│  Widget Tree     │  Event System    │  Layout Engine  │
│  (60+ widgets)   │  (EventLoop +    │  (Box, Grid,    │
│                   │   GestureEngine) │   Flow, Stack)  │
├──────────────────┼──────────────────┼────────────────┤
│  i18n │ Theme    │  Signal System   │  Control Backend│
│  (en/zh-cn/zh-tw)│  (GenericSignal) │  (Native/Custom)│
├──────────────────┴──────────────────┴────────────────┤
│              Rendering Pipeline                       │
│  SvgPaintBackend │ SoftwarePaintBackend │ GPU (wgpu)  │
│  (SVG output)    │ (CPU rasterizer)     │ (Hardware)   │
├──────────────────────────────────────────────────────┤
│              Platform Backends                        │
│  Windows(Win32) │ macOS(Cocoa) │ Linux(GTK) │ Stub   │
└──────────────────────────────────────────────────────┘
```

---

## Key Features

### 🎨 SvgPaintBackend — Pipeline-Accurate SVG Output
- Single `PaintBackend` implementation converts all 17 `RenderCommand` variants to SVG
- Replaces 52 hand-written `to_svg()` implementations with **one backend**
- SVG output is **guaranteed identical** to pixel rendering (same render pipeline)
- `render_to_svg()` convenience wrapper auto-detects widget geometry

### 🤚 Touch & Gesture System
- **11 gesture recognizers**: Tap, DoubleTap, LongPress, Swipe, Pan, Fling, TwoFingerTap, TwoFingerSwipe, LongPressDrag, Pinch, Rotate
- `TouchEventTranslator` bridges touch→mouse events for legacy widgets
- `GestureEngine` integrated with `EventLoop` for runtime gesture dispatch
- Touch-target expansion via `contains_point_with_touch_expansion()`

### 🌐 Internationalization
- `tr!()` macro with key-based translation lookups
- Complete en/zh-cn/zh-tw translation packages (30+ UI string keys)
- `translate_with_context()` for plural forms and contextual translations
- `audit_keys()` for translation coverage validation

### 🖥 Hardware-Adaptive GPU Management
- Automatic GPU detection (discrete > integrated > CPU fallback)
- Hardware-specific buffer pool configuration
- Dynamic quality degradation based on performance monitoring
- Seamless GPU↔CPU fallback

### 📐 Layout System
- Box, HBox, VBox, Grid, Form, Stack, Flow, Absolute, Anchor, Masonry layouts
- `LayoutContext` with `layout_scale`, `font_scale`, and `min_touch_size` for device-adaptive layout
- Layout constraints: aspect ratio, min/max size, alignment, spacing

### 📊 Charts & PDF
- Line, Bar, Pie, Scatter, Area, Bubble, Candlestick charts
- PDF with annotations, hyperlinks, form fields, security (encryption, signatures)

### 🔧 Performance Optimizations
- Memory pooling: Object pools, arena allocators
- Render batching, dirty region tracking, text caching
- Performance profiling with frame rate monitoring

---

## Widget Library (60+)

### Core Widgets (100% complete)

| Widget | Native | Self-painted | SVG Output |
|--------|:------:|:------------:|:----------:|
| Window | ✅ | ✅ | ✅ via pipeline |
| Button | ✅ | ✅ | ✅ via pipeline |
| CheckBox | ✅ | ✅ | ✅ via pipeline |
| RadioButton | ✅ | ✅ | ✅ via pipeline |
| Label | ✅ | ✅ | ✅ via pipeline |
| LineEdit | ✅ | ✅ | ✅ via pipeline |
| TextEdit | — | ✅ | ✅ via pipeline |
| ComboBox | ✅ | ✅ | ✅ via pipeline |
| ListBox | ✅ | ✅ | ✅ via pipeline |
| SpinBox | ✅ | ✅ | ✅ via pipeline |
| Slider | ✅ | ✅ | ✅ via pipeline |
| ProgressBar | ✅ | ✅ | ✅ via pipeline |
| ScrollBar | ✅ | ✅ | ✅ via pipeline |
| ScrollArea | — | ✅ | ✅ via pipeline |
| TabWidget | — | ✅ | ✅ via pipeline |
| Splitter | — | ✅ | ✅ via pipeline |
| GroupBox | — | ✅ | ✅ via pipeline |
| MenuBar | ✅ | ✅ | ✅ via pipeline |
| Menu | ✅ | ✅ | ✅ via pipeline |
| ToolBar | ✅ | ✅ | ✅ via pipeline |
| StatusBar | — | ✅ | ✅ via pipeline |
| TreeView | ✅ | ✅ | ✅ via pipeline |
| TableView | ✅ | ✅ | ✅ via pipeline |
| ListView | ✅ | ✅ | ✅ via pipeline |
| Canvas | — | ✅ | ✅ via pipeline |
| Chart | — | ✅ | ✅ via pipeline |
| Grid | — | ✅ | ✅ via pipeline |
| Dialog | ✅ | — | ✅ via pipeline |
| MessageBox | ✅ | — | ✅ via pipeline |
| FileDialog | ✅ | — | ✅ via pipeline |
| ColorDialog | ✅ | — | ✅ via pipeline |
| FontDialog | ✅ | — | ✅ via pipeline |

### Extended Widgets

| Widget | Description |
|--------|-------------|
| ToggleButton, Dial, Calendar | Interactive controls |
| DateEdit, TimeEdit, DateTimeEdit | Date/time pickers |
| KeySequenceEdit | Keyboard shortcut input |
| PieMenu, RibbonBar | Advanced menus |
| TabBar, ToolBox, StackedWidget | Tab/stack containers |
| CollapsiblePane, DockWidget, MdiArea | Panel/dock/MDI |
| CommandLink, FontComboBox | Specialized inputs |
| LCDNumber | Seven-segment display |
| FreeformShapeWidget | Vector shape rendering |
| PopupWindow, InputDialog, ProgressDialog | Dialog variants |
| Action, ToolButton | Action system |
| WebEngineView, WebView | Web content display |
| WebEngineSettings, CookieStore, WebChannel | Web subsystems |

---

## C ABI & Language Bindings

```bash
# Build shared library
cargo build --release

# C sample (macOS)
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/release -lrust_widgets -o target/release/c_abi_poll_demo
DYLD_LIBRARY_PATH=target/release ./target/release/c_abi_poll_demo

# Python
python examples/python/demo_basic.py

# C++
g++ -Iexamples examples/cpp/demo_basic.cpp -Ltarget/release -lrust_widgets -o target/release/cpp_demo

# Java (JNI)
cd examples/java && javac RustWidgets.java && java RustWidgets
```

### Available Bindings

| Language | File | Status |
|----------|------|--------|
| C | `examples/c_abi_poll_demo.c` + `examples/rust_widgets.h` | ✅ |
| C++ | `examples/cpp/rust_widgets.hpp` + `demo_basic.cpp` | ✅ |
| Python | `examples/python/rust_widgets.py` + `demo_basic.py` | ✅ |
| Java | `examples/java/RustWidgets.java` + JNI bridge | ✅ |
| Harmony NAPI | `examples/harmony_napi_bridge_sample.c` | ✅ |

---

## Core Modules

| Module | Description |
|--------|-------------|
| `core` | Primitives: Point, Rect, Size, Color, Font, ObjectId |
| `widget` | 60+ widget implementations with Widget/Draw/EventHandler traits |
| `event` | Event types, EventLoop, GestureEngine, TouchEventTranslator |
| `gesture` | 11 gesture recognizers (Tap through Rotate) |
| `signal` | GenericSignal, Signal1, ConnectionScope |
| `layout` | Box, Grid, Flow, Stack, Absolute, Anchor, Masonry layouts |
| `render` | SvgPaintBackend, SoftwarePaintBackend, GPU (wgpu), Scene composition |
| `platform` | Windows(Win32), macOS(Cocoa/objc2), Linux(GTK/Wayland), Harmony, Mobile, Stub |
| `i18n` | tr!() macro, I18nManager, en/zh-cn/zh-tw translations |
| `control_backend` | Native + CustomPaint control backends, dispatcher |
| `style` | WidgetStyle, gradients, animations, theme states |
| `theme` | Theme manager, dark/light mode, theme tokens |
| `chart` | Line, Bar, Pie, Scatter, Area, Bubble, Candlestick charts |
| `web` | WebEngine, WebView, JS engine, navigation, plugins |
| `gpu` | GPU adapter detection, buffer pools, quality management |
| `memory` | ObjectPool, ArenaAllocator, BufferPool |
| `performance` | Profiler, frame rate monitor, metrics |
| `embedded` | Lightweight widget creation, fixed DPI, hardware input |
| `error` | RwError, ErrorId, FFI error handling |
| `pdf` | PDF writing, annotations, forms, security |
| `print` | Print support |
| `json` | JSON layout loader, event bindings |
| `object` | Object/class-name system |
| `action` | Action system |
| `clipboard` | Clipboard + drag-drop managers |
| `menu_config` | Menu configuration system |
| `shortcut` | Keyboard shortcut parsing |
| `quality` | Quality management, adaptive rendering |
| `render_engine` | Embedded render engine |
| `wgpu_backend` | wgpu-based GPU rendering |
| `test` | Testing harness, matchers, snapshots |
| `bindings` | C FFI bindings |
| `index` | Widget registry |

---

## Quality Baseline (BLUE8 Complete)

| Dimension | Score | Status |
|-----------|:-----:|:------:|
| 编译可靠性 Build Reliability | **10/10** | ✅ Zero errors, zero warnings |
| 触摸交互完整度 Touch Completeness | **10/10** | ✅ 11 recognizers, TouchEventTranslator, touch expansion |
| 手势识别能力 Gesture Recognition | **10/10** | ✅ Pan, Fling, TwoFingerTap, LongPressDrag, etc. |
| 设备自适应 Device Adaptation | **10/10** | ✅ Orientation, DPI, LayoutContext, accessibility settings |
| 测试覆盖 Test Coverage | **10/10** | ✅ 1375 tests (1328 unit + 47 integration + doc) |
| 平台后端正交性 Platform Orthogonality | **10/10** | ✅ Windows DPI/IME/OLE real impl, extension traits |
| i18n 支持 Internationalization | **10/10** | ✅ tr!() fix, audit_keys(), 3 translation packages |
| Widget 基础模式 Widget Foundation | **10/10** | ✅ 48 base() fix, 4700-line cleanup, 49 encapsulation fixes |

**1375 tests — 0 failures — 0 clippy warnings — 0 safety holes**

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|:------:|
| Frame rate | 60 FPS (adaptive) | ✅ |
| Memory (standard app) | < 100 MB | ✅ |
| Startup time | < 1 second | ✅ |
| Widget creation | < 1 ms | ✅ |

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Run tests: `cargo test --all-features -q && cargo clippy --all-features --all-targets -- -D warnings && cargo fmt --all -- --check`
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## License

MIT License — see [LICENSE](LICENSE) for details.

## Support

- Issues: [GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)
- Discussions: [GitHub Discussions](https://github.com/mikewolfli/rust-widgets/discussions)
- Documentation: [docs/](docs/) directory
