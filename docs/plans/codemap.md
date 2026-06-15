# rust-widgets Code Map

> **Generated**: 2026-06-15 (Round 2 super-depth scan)
> **Version**: 0.9.9
> **Total Tests**: 3723 (all passing, 0 ignored)

---

## 1. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                   Public API (lib.rs)                    │
│  create_*(), WidgetHandle, App, EventLoop, Platform fn   │
└──────┬──────────────────────┬──────────────────────┬─────┘
       │                      │                      │
       ▼                      ▼                      ▼
┌──────────┐    ┌─────────────────────┐    ┌──────────────────┐
│  Widget  │    │  Platform (trait)   │    │  Render Engine   │
│  System  │    │  + Backend impls    │    │  (Platform wrap) │
├──────────┤    ├─────────────────────┤    ├──────────────────┤
│ • Widget │    │ • macos (cocoa/     │    │ • NativeRender   │
│ • Draw   │    │        objc2)       │    │   Engine         │
│ • kind   │    │ • windows (win32)   │    │ • EmbeddedRender │
│ • Base   │    │ • linux (gtk/       │    │   Engine         │
│ • Image  │    │        wayland)     │    │                  │
│ • Window │    │ • ios (uikit)       │    │                  │
│ • SVG    │    │ • android (jni)     │    │                  │
│ • reg.   │    │ • wasm              │    │                  │
└──────────┘    │ • harmony           │    └──────────────────┘
                │ • stub              │
                └─────────────────────┘
```

---

## 2. Module Tree (src/)

### 2.1 Core Layer (`src/core/`)
| File | Purpose |
|------|---------|
| `alignment.rs` | Alignment enums (Horizontal, Vertical, Combined) |
| `color.rs` | RGBA Color struct with blend/lerp/invert |
| `coords.rs` | Coordinate conversion helpers (screen↔cartesian↔PDF) |
| `font.rs` | Font descriptor (family, size, weight, style) |
| `geometry.rs` | Point, Size, Rect, Orientation, deg_to_rad |
| `mod.rs` | Re-exports all core types |
| `mutex_ext.rs` | Mutex extension trait for poisons |
| `rect_merge.rs` | Rect merging/union optimization |
| `types.rs` | ObjectId, Version, CoreConfig, PlatformCapabilities, CoreError |

### 2.2 Event System (`src/event/`)
| File | Purpose |
|------|---------|
| `types.rs` | Event enum (100+ variants), EventHandler trait, EventPriority, AsyncTask |
| `event_queue.rs` | EventQueue + EventSender (mpsc-based) |
| `focus.rs` | FocusManager (tab order, focus tracking) |
| `capture.rs` | PointerCaptureManager (mouse capture) |
| `loop.rs` | EventLoop, AnimationFrameRequest |
| `queue.rs` | FixedSizeQueue (lock-free SPSC for mini) |
| `timer.rs` | TimerManager (one-shot, interval, debounce) |
| `translator.rs` | Touch→Gesture event translator |

### 2.3 Widget System (`src/widget/`)
| Path | Contents |
|------|----------|
| `base.rs` | BaseWidget — shared state + signals for all widgets |
| `capability.rs` | WidgetCapability, WidgetFactory, generic property read/write |
| `draw.rs` | Draw trait (custom rendering) |
| `image.rs` | Image struct + ImageFormat |
| `kind.rs` | WidgetKind enum (166 variants) |
| `mod.rs` | Re-exports + type aliases |
| `registry.rs` | SimpleRegistry for child forwarding |
| `widget_trait.rs` | Widget trait (geometry, style, signals, CSS, a11y) |
| `window.rs` | Window widget with title bar rendering |
| `svg.rs` | SVG rendering backend for widgets |

#### Widget Subfolders

| Subfolder | Count | Description |
|-----------|-------|-------------|
| `base_widgets/` | 6 | Button, CheckBox, Frame, Label, RadioButton, ToggleButton |
| `input_widgets/` | 18 | ComboBox, Dropdown, Keyboard, LineEdit, ListBox, SpinBox, TextArea, TextEdit, AutoCompleteEdit, CommandLink, EditableComboBox, FontComboBox, ImePreedit, InplaceEditor, MaskedEdit, MultiSelectComboBox, RangeSlider, RichEdit, SearchBar, SearchBox, ShortcutEditor, TagInput |
| `container_widgets/` | 14 | Carousel, CollapsiblePane, DockWidget, GroupBox, MasonryLayout, MdiArea, PagerPageView, SafeArea, ScrollArea, Splitter, StackedWidget, Stepper, TabWidget, TileView, ToolBox |
| `display_widgets/` | 20 | Arc, Badge, ColorHistory, ColorWell, Divider, EmptyState, FloatingLabel, FontPreview, Icon, ImageView, LCDNumber, Line, Meter, MiniCanvas, MiniChart, ProgressBar, ProgressCircle, Rating, Roller, ScrollBar, SkeletonLoader, Slider, Spinner, Switch |
| `nav_widgets/` | 7 | AdaptiveScaffold, AppBar, BottomNavigationBar, NavigationDrawer, NavigationStack, TabView |
| `dialog/` | 12 | BottomSheet, ColorDialog, FileDialog, FindReplaceDialog, FontDialog, InputDialog, MessageBox, ModalBottomSheet, Popover, PopupWindow, ProgressDialog, Tooltip, WizardDialog |
| `chart_widgets/` | 4 | BarChart, LineChart, PieChart, Sparkline |
| `media_widgets/` | 8 | AnimatedImage, AudioVisualizer, CameraPreview, HeroAnimation, LottieWidget, RiveWidget, VideoPlayer |
| `menu_toolbar/` | 8 | Action, DropdownMenu, Menu, MenuBar, MenuButton, StatusBar, ToolBar, ToolButton |
| `overlay_widgets/` | 3 | FAB, RefreshControl, SwipeToDismiss |
| `view_widgets/` | 8 | DataGrid, ImageGallery, ListView, PropertiesPanel, PropertyGrid, TableWidget, TreeTable, TreeView, VirtualList, VirtualTable |
| `special_widgets/` | 21 | Breadcrumb, Canvas, ChartWidget, Chip, CodeEditor, ColorPicker, CommandPalette, DiffViewer, FreeformShapeWidget, GanttWidget, GridWidget, MapView, MarkdownEditor, MediaPlayer, NotificationCenter, SegmentedControl, Snackbar, SplitButton, TerminalView, TimelineWidget, ToastStack |
| `advanced_widgets/` | 10 | Calendar, DateEdit, DateTimeEdit, Dial, KeySequenceEdit, PieMenu, RibbonBar, TabBar, TimeEdit |
| `web_widgets/` | 8 | WebEngine, WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem, WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult, WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest |
| `cupertino/` | 6 | CupertinoAlertDialog, CupertinoSlider, CupertinoSwitch, CupertinoDatePicker, CupertinoNavigationBar, CupertinoSegmentedControl, MaterialNavigationRail, MaterialSnackbar |
| `misc_widgets/` | 9 | Avatar, BarcodeScanner, BezierCurveEditor, DateRangePicker, MobileDatePicker, QRCode, SegmentedButton |

### 2.4 Platform Layer (`src/platform/`)
| File/Module | Purpose |
|-------------|---------|
| `contract.rs` | Platform trait (full widget lifecycle + capability contracts) |
| `types.rs` | PlatformCapabilities, WidgetTriggerKind, DropEvent, capability contracts |
| `state.rs` | BackendState<K> — thread-safe widget state model |
| `stub.rs` | StubPlatform — in-memory mock for testing |
| `runtime.rs` | Platform init/run/quit, auto-detect Linux/Wayland |
| `clipboard.rs` | RichClipboardBackend trait, MockClipboard |
| `clipboard_stubs.rs` | Platform clipboard backends (macos-legacy, objc2, win32, linux, wasm) |
| `ime.rs` | ImeBridge trait, ImeComposition, MockImeBridge |
| `ime_stubs.rs` | macOS ImeBridge, Windows ImeBridge (TSF) |
| `ime_macos.rs` | Real macOS IME (NSTextInputContext) |
| `ime_linux.rs` | Real Linux IME (IBus) |
| `ime_windows.rs` | Real Windows IME (TSF) |
| `detector.rs` | DeviceClass detection |
| `virtual_keyboard.rs` | VirtualKeyboard controller |
| `mobile.rs` | Mobile platform extensions |
| `holographic.rs` | (Experimental) Holographic keyboard |
| `tests.rs` | Platform consistency tests |

#### Platform Backend Implementations

| Backend | Directory | Status |
|---------|-----------|--------|
| macOS (cocoa-legacy) | `macos/` | ✅ Native widgets (NSWindow, NSButton, etc.) |
| macOS (objc2) | `macos_objc2/` | ✅ Native widgets (objc2 bindings) |
| Windows | `windows/` | ✅ Win32 API |
| Linux (GTK) | `linux/` | ✅ GTK3 |
| Linux (Wayland) | `wayland/` | ✅ Wayland protocols |
| iOS | `ios/` | ✅ UIKit |
| Android | `android/` + `android_jni.rs` | ✅ JNI |
| WASM | `wasm/` | ✅ web-sys |
| Harmony | `harmony/` | ✅ Stub |

### 2.5 Render System

#### `src/render/`
| Module | Purpose |
|--------|---------|
| `core/` | RenderCommand, BlendMode, ShapedText, TextMetrics |
| `backend/` | BackBuffer, SoftwareSurface, RenderContext, PaintBackend, BatchRenderer |
| `pipeline/` | Visual command pipeline for all widget types |
| `quality/` | Adaptive quality management |
| `svg/` | SvgPaintBackend |
| `gpu/` | GPU accelerated rendering (wgpu) |
| `web/` | Web rendering types |
| `text_cache.rs` | Glyph/texture cache |
| `text_shaper.rs` | SimpleTextShaper, ShapedGlyphRun |
| `rich_text.rs` | RichText, TextSpan |
| `text_overflow.rs` | TextClamp, TextOverflow |
| `grapheme.rs` | GraphemeCluster, GraphemeProcessor |
| `projection.rs` | Projector mode rendering |

#### `src/render_engine/`
| File | Purpose |
|------|---------|
| `engine_trait.rs` | RenderEngine trait (Platform wrapper) |
| `native.rs` | NativeRenderEngine (delegates to Platform) |
| `embedded.rs` | Embedded runtime state + task queue |
| `embedded_engine.rs` | EmbeddedRenderEngine + default_render_engine() |

### 2.6 Layout System (`src/layout/`)
| File | Purpose |
|------|---------|
| `mod.rs` | Layout trait, LayoutContext, SizePolicy |
| `absolute.rs` | AbsoluteLayout |
| `aspect_ratio.rs` | AspectRatioLayout |
| `box_layout.rs` | BoxLayout, HBoxLayout, VBoxLayout |
| `center.rs` | CenterLayout |
| `flex.rs` | FlexLayout |
| `flow.rs` | FlowLayout (word-wrap) |
| `form.rs` | FormLayout (label+field) |
| `grid.rs` | GridLayout |
| `inspector.rs` | Layout inspector |
| `keyboard_aware.rs` | KeyboardAwareLayout (mobile) |
| `splitter.rs` | SplitterLayout |
| `stack.rs` | StackLayout (card stack) |
| `uniform_grid.rs` | UniformGridLayout |
| `wrap.rs` | WrapLayout (line-wrap) |

### 2.7 Style System (`src/style/`)
| File | Purpose |
|------|---------|
| `mod.rs` | WidgetStyle, EdgeOffsets (Padding/Margin), Shadow, TouchTargetSize |
| `css.rs` | CSS parser + applicator |
| `css_watcher.rs` | CSS file hot-reload |
| `selector.rs` | CSS selector matching |
| `stylesheet.rs` | Stylesheet + Rule collection |
| `animation.rs` | Animation (tween, keyframe) |
| `animation_group.rs` | AnimationGroup (parallel) |
| `gradient.rs` | Gradient (Linear, Radial, Conic) |
| `theme.rs` | Theme color definitions |
| `theme_state.rs` | StatefulTheme (hover/pressed/disabled) |

### 2.8 Signal System (`src/signal/`)
| File | Purpose |
|------|---------|
| `mod.rs` | Signal, GenericSignal, Signal1, ConnectionScope |
| `core_signal.rs` | Signal<T> — typed signal with priority, blocking, re-entrancy |
| `generic_signal.rs` | GenericSignal (void) + Signal1<A> (1-arg) |
| `hub.rs` | CustomSignalHub — string-keyed signal registry |

### 2.9 Supporting Modules

| Module | Purpose |
|--------|---------|
| `error/` | RwError, ErrorId, catch_panic, FFI safety |
| `app/` | App lifecycle, AppConfig, WidgetHandle (typed handles) |
| `action/` | Action, ActionManager, typed action types |
| `asset/` | Asset management |
| `audio/` | Decoder, encoder, output, resample, normalize |
| `bindings/` | FFI binding implementation (C ABI), Java JNI |
| `chart/` | Chart drawing contracts + definitions |
| `clipboard/` | (re-export from platform/clipboard) |
| `compat.rs` | MiniVec, MiniString, Mutex, OnceLock, Arena — no_std bridge |
| `control_backend/` | Native vs Custom paint routing |
| `data_binding/` | Binding<T>, ObservableList<T>, Computed<T> |
| `embedded/` | Embedded HAL abstractions |
| `gesture/` | Gesture recognizers (tap, swipe, pinch, rotate, fling) |
| `gpu/` | GPU capability types |
| `i18n/` | I18nManager, translation loading + hot-reload |
| `image/` | Image decoder, encoder, EXIF, SVG, format detection |
| `index/` | Index/search system |
| `json/` | Declarative JSON → widget tree loader |
| `memory/` | Pool allocator, arena allocator, stack allocator, MemoryMonitor |
| `menu_config/` | Menu configuration |
| `object/` | Object system (Object, ObjectId, PropertyValue) |
| `pdf/` | PDF generation (document, page, annotation, form, security) |
| `performance/` | DirtyRegionTracker, UpdateBatcher, FrameTimer, profiler |
| `print/` | Print utilities |
| `quality/` | QualityManager, QualityLevel, FrameTimeMonitor |
| `shortcut/` | ShortcutManager, ShortcutEntry |
| `theme/` | ThemeManager, Theme types |
| `undo/` | UndoStack, UndoCommand, grouped undo/redo |
| `util/` | Utility functions |
| `video/` | VideoEngine, decoder, frame, playback |
| `web/` | Web engine, web view |
| `wgpu_backend/` | WGPU initialization + surface management |

---

## 3. Key Trait Hierarchy

```
EventHandler (handle_event)
    └── BaseWidget (event → typed signals)
    └── Widget = EventHandler + Any
            ├── geometry() / set_geometry()
            ├── style() / set_style()
            ├── signals (clicked, changed, hover, mouse*, key*, focus*)
            ├── show/hide, set_enabled, tooltip
            └── accessible_name / accessible_role

Draw (draw, uses_custom_drawing)
    └── All 170+ widget types implement Draw

Platform (Send + Sync)
    ├── create_window/create_button/create_checkbox/...
    ├── init/run/quit
    ├── clipboard_backend() → RichClipboardBackend
    ├── ime_bridge() → ImeBridge
    └── accessibility_bridge() → AccessibilityBridge

Layout (add_widget, remove_widget, update)
    └── BoxLayout, FlexLayout, GridLayout, FlowLayout, etc.

RenderEngine (name, profile, init, run, quit, create_*)
    └── NativeRenderEngine (→ Platform)
    └── EmbeddedRenderEngine (→ embedded runtime)
```

---

## 4. Data Flow

```
User Input
    │
    ▼
Event Queue (mpsc)
    │
    ▼
Event Loop → FocusManager → PointerCapture
    │
    ▼
Widget.handle_event() → typed signal emission
    │
    ▼
Layout.update() → Rect recalculations
    │
    ▼
RenderContext → PaintBackend.dispatch()
    │
    ▼
SoftwareSurface / GPU / SVG / Native
```

---

## 5. Feature System (3-axis)

```
Axis 1: Device Profile (mutually exclusive)
    desktop | tablet | mobile | embedded | mini

Axis 2: OS Backend (composable, os-auto picks by target)
    macos | macos-legacy | ios | windows | linux-gtk | linux-wayland
    linux-a11y | android | wasm | harmony

Axis 3: Capabilities (arbitrary composition)
    touch | gpu | wgpu | software | i18n | chart | print | pdf
    a11y | image | video | audio | holographic | projection
```

---

## 6. WidgetKind→Module Mapping (166 variants)

| WidgetKind | Module Path | Type |
|-----------|-------------|------|
| Window | `widget::window::Window` | struct |
| Button | `widget::base_widgets::button::Button` | struct |
| CheckBox | `widget::base_widgets::checkbox::CheckBox` | struct |
| RadioButton | `widget::base_widgets::radiobutton::RadioButton` | struct |
| Label | `widget::base_widgets::label::Label` | struct |
| ToggleButton | `widget::base_widgets::toggle_button::ToggleButton` | struct |
| Frame | `widget::base_widgets::frame::Frame` | struct |
| ComboBox | `widget::input_widgets::combobox::ComboBox` | struct |
| LineEdit | `widget::input_widgets::lineedit::LineEdit` | struct |
| SpinBox | `widget::input_widgets::spinbox::SpinBox` | struct |
| ListBox | `widget::input_widgets::listbox::ListBox` | struct |
| TextEdit | `widget::input_widgets::textedit::TextEdit` | struct |
| RichEdit | `widget::input_widgets::rich_edit::RichEdit` | struct |
| TextArea | `widget::input_widgets::textarea::TextArea` | struct |
| Dropdown | `widget::input_widgets::dropdown::Dropdown` | struct |
| Keyboard | `widget::input_widgets::keyboard::Keyboard` | struct |
| SearchBox | `widget::input_widgets::search_box::SearchBox` | struct |
| SearchBar | `widget::input_widgets::search_bar::SearchBar` | struct |
| AutoCompleteEdit | `widget::input_widgets::auto_complete_edit::AutoCompleteEdit` | struct |
| TagInput | `widget::input_widgets::tag_input::TagInput` | struct |
| MaskedEdit | `widget::input_widgets::masked_edit::MaskedEdit` | struct |
| ... (155 more) | `view_widgets/*`, `special_widgets/*`, `dialog/*`, etc. | struct/alias |

---

## 7. Test Map

| Test File | Count | Type |
|-----------|-------|------|
| `[lib]` (inline `#[cfg(test)]`) | 3629 | Unit |
| `tests/integration_test.rs` | 48 | Integration |
| `tests/blue9_r1_api_symmetry_test.rs` | 7 | Symmetry |
| `tests/blue9_r6_platform_capability_test.rs` | 7 | Platform |
| `tests/snapshot_tests.rs` | 4 | Snapshot |
| `tests/property_based_tests.rs` | 4 | Property |
| Doc tests | 24 | Doc |
| **Total** | **3723** | **All passing** |

---

## 8. FFI Boundary

```
Rust API → extern "C" (C ABI)
    ├── rw_init / rw_run / rw_quit
    ├── rw_create_window / rw_create_button / ...
    ├── rw_set_widget_* / rw_get_widget_*
    ├── rw_combo_box_* / rw_list_box_*
    ├── rw_embedded_engine_*
    └── catch_panic safety at every entry point
```

---

## 9. File Count Summary

| Category | Count |
|----------|-------|
| Rust source files | 350+ |
| Widget implementations | 170+ |
| Layout managers | 14 |
| Platform backends | 12 |
| IME implementations | 4 |
| Clipboard backends | 5 |
| Gesture recognizers | 10 |
| Test files | 6 (+ inline) |
| Benchmark files | 5 |
