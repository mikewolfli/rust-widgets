# rust_widgets — Pure Rust GUI Library

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

Cross-platform GUI library in pure Rust. Hardware-adaptive rendering, widget library, touch/gesture support, i18n, and SVG output. Supports desktop, tablet, mobile, embedded, and minimal-profile **mini** targets.

## ✨ Every control is self-drawn

**The library paints 100% of its own controls. It does not create native OS controls — on any platform.**

There is no `CreateWindowExW`/`NSButton`/`gtk_button_new`/`android.widget.Button` anywhere in this crate. Each backend's only job is to hand the renderer a surface to paint into; every button, list, editor, menu and chart below is drawn by the same Rust rasterizer, so a control looks and behaves identically whether it is running on Windows, macOS, Linux, iOS, Android or the web.

```
        ┌──────────────────────────────────────────┐
        │  rust_widgets  —  paints its own controls │
        └──────────────────────────────────────────┘
             │  rasterizer output (RGBA / SVG / GPU)
             ▼
  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
  │ Windows HWND │   │ macOS NSView │   │  GTK widget  │   … one surface per backend
  └──────────────┘   └──────────────┘   └──────────────┘
```

### Why this matters

| Property | Self-drawn (this library) | Native controls |
|---|---|---|
| Appearance | **Identical on every OS** | Differs per OS toolkit and version |
| Widget count | **179 kinds, all platforms** | Only what the OS toolkit offers |
| Dependency weight | **No GUI toolkit linked** | GTK / AppKit / Win32 / Android SDK |
| Headless & embedded | **Runs with no OS at all** (`mini`, SVG) | Impossible |
| Deterministic tests | **Pixel/serialise snapshots** | Needs a real display |

### What each backend *does* own

Self-drawing is not "one backend". A backend still owns the parts that genuinely belong to the operating system, and only those:

- **Surface + event loop** — window creation, the paint callback, resize.
- **Input** — keyboard/mouse/touch translated into a unified `Event`.
- **Platform services** — IME, clipboard, accessibility bridge, file dialogs, DPI scaling.

A backend that cannot supply even a surface (for example a bare framebuffer) still works: it paints into an in-memory buffer instead. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

> **Migrating from 1.x?** Native control creation was removed from all ten backends in 2.0.0. See [`CHANGELOG.md`](CHANGELOG.md) and [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md).

All 179 widget kinds are self-drawn. Every one of them resolves a constructor through
`factory_name_for_kind` (`377` accepted names in total, counting aliases);
`tools/check_widget_registration_fidelity.sh` fails if a kind is added without an
answer, or resolves to no constructor at all — the latter caught four `create_*`
methods (`Frame`, `DockPanel`, `CupertinoSwitch` and nine WebEngine names) that
returned id `0` while every other gate passed. See
[`docs/plans/blue16.md`](docs/plans/blue16.md) §12 for the per-kind audit. The platform
capability matrix
([`docs/plans/platform_capability_matrix.md`](docs/plans/platform_capability_matrix.md))
is generated from source and gated for drift in CI.

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.4.2-blue)]()
[![tests](https://img.shields.io/badge/tests-4900%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

**Verified in 2.4.2:** `4986` lib tests pass on `desktop`, and `cargo test` reports 0
failures across all 28 test binaries. `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --check` are clean, all five device profiles build with zero warnings, and the
`tools/check_*.sh` gate suite passes. See [`CHANGELOG.md`](CHANGELOG.md) and
[`docs/log/log-20260919-2.md`](docs/log/log-20260919-2.md) for per-fix evidence.

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

# Mini (reduced std profile, minimal widget set)
cargo check --no-default-features --features mini

# Embedded
cargo check --no-default-features --features embedded

# Tests (lib suite; the CI command is `cargo test --all-features -q`)
cargo test --lib

# Cross-compile checks used by CI (no system libraries required)
cargo check --target wasm32-unknown-unknown --no-default-features --features wasm
cargo check --target x86_64-pc-windows-msvc --no-default-features \
  --features "windows desktop-runtime wgpu touch i18n controls-native controls-custom serde serde_json advanced-widgets quality-management"
```

> **Android:** build the JNI test APK with `./tools/build_android_testapp.sh`
> (`ANDROID_SDK_ROOT` defaults to `~/Android/Sdk`; the NDK is taken from
> `$ANDROID_SDK_ROOT/ndk`). See [Build Requirements](#build-requirements).

### Device Profiles

**Pick exactly one.** The device profiles are mutually exclusive: `mini`/`embedded`
compile parts of the crate *out*, so combining one with `desktop` is not a
"lowest common denominator" — it breaks the build.

```bash
# ✅ correct
cargo check                                        # desktop (default)
cargo check --no-default-features --features mini
cargo check --no-default-features --features embedded

# ❌ wrong: desktop stays on, so mobile-profile modules are still compiled
cargo check --features mini
```

| Profile | Command | Backend | Widgets | i18n | GPU |
|---------|---------|---------|---------|------|-----|
| Desktop | `cargo check` | OS surface + event loop | Full widget set | ✅ | ✅ (wgpu enabled by desktop) |
| Tablet | `--no-default-features --features tablet` | OS surface + event loop | Full widget set | ✅ | ✅ (wgpu enabled by tablet) |
| Mobile | `--no-default-features --features mobile` | Mobile API | Full widget set | ✅ | ✅ (wgpu enabled by mobile) |
| Embedded | `--no-default-features --features embedded` | Software | Core widget set | — | — |
| **Mini** | `--no-default-features --features mini` | **reduced std** + alloc | **Core widget set** | — | — |

#### What each profile turns off

The API is the same across profiles; what differs is which capabilities *exist*.
Only profiles that include a platform backend **and** keep `widget::runtime` can
host custom-painted widgets:

| Capability | Desktop | Embedded | Mini |
|------------|:-------:|:--------:|:----:|
| `widget::runtime` (widget registry) | ✅ | — | — |
| Custom-painted widgets (`mount_custom_widget`) | ✅ | — | — |
| `supports_custom_widgets()` | `true` | `false` | `false` |
| Menus / tool bars / status bars | ✅ | ✅ | ✅ |
| Menu shortcuts (displayed) | ✅ | ✅ | ✅ |
| Menu shortcuts (actually fire) | ✅ | ✅ | ✅ |

Where the table shows `—` the capability is **absent, not degraded**: the module
is compiled out, so `supports_custom_widgets()` reports `false` and callers are
expected to refuse the operation rather than mount into a blank window (see
`demo/code_editor`'s startup check).

Menus and shortcuts are deliberately *not* affected: their code carries no
`mini` gate, so a `mini` build is best described as **"no custom-painted widget
surface, but fully working menus"**.

The device profiles are **mutually exclusive**, so verification always names one
profile explicitly — `cargo check --no-default-features --features <profile>`.

> **Why not `--all-features`?** That command turns `desktop` and `mini` on at the
> same time, and `mini` switches the crate to `no_std`, which removes the `alloc`
> prelude that most of the code resolves `String`/`Vec` through. The combination
> cannot compile, so it can never act as a check. CI and this guide used to
> require it; that requirement made the documented contributor workflow
> unrunnable. Use the profile matrix instead.

#### `tablet` / `mobile` need an explicit OS backend

Unlike `desktop`, the `tablet` and `mobile` profiles do **not** pull in an OS
backend by themselves — their only backend entry is `os-auto`, which is currently
an empty feature. Build them with a backend named explicitly:

```bash
# ⚠️ resolves to a stub backend on every OS: no real widgets at all
cargo check --no-default-features --features tablet

# ✅ real backend
cargo check --no-default-features --features "tablet,macos"
```

Two consequences worth knowing before you rely on these profiles:

* Without a backend feature you silently get `macos-fallback-stub` (or the
  per-OS equivalent) rather than an error. Check
  `rust_widgets::backend_name()` if you are unsure which one you built.
* On macOS, `tablet`/`mobile` select the **objc2 preview** backend, which does
  *not* implement custom-painted widget hosting yet. On macOS that currently
  requires the `desktop` profile. Query `supports_custom_widgets()` rather than
  assuming.

### OS Backends

| OS | Feature | Auto-detect | Cross-checked from macOS |
|----|---------|:-----------:|:-----------------------:|
| Windows (Win32) | `windows` | ✅ | ✅ `x86_64-pc-windows-gnu` (0 warnings) |
| macOS (Cocoa/objc2) | `macos` | ✅ | n/a — host |
| iOS (UIKit) | `ios` | ✅ | ✅ device **and** simulator, incl. `--all-targets` |
| Linux (GTK) | `linux-gtk` | — | needs a cross sysroot (CI job) |
| Linux (Wayland) | `linux-wayland` | — | needs a cross sysroot (CI job) |
| Android (JNI) | `android` | ✅ | needs the NDK (CI job) |
| Web (WASM) | `wasm` | — | ✅ `wasm32-unknown-unknown`, incl. `--all-targets` |
| HarmonyOS | `harmony` | — | ✅ `aarch64`/`armv7`/`x86_64-unknown-linux-ohos`, **built and linked** |

> **"Cross-checked" means compiled, not merely claimed.** The HarmonyOS rows are
> built *and linked* against the OpenHarmony SDK sysroot, and the resulting
> `librust_widgets.so` machine type is asserted (`AArch64` / `ARM` / `X86-64`) — a
> `cargo check` alone never links, so it cannot catch a wrong or missing toolchain.
> `loongarch64-unknown-linux-ohos` is **not buildable**: rustup ships no std for it
> (Tier 3) and the SDK ships no libc for that architecture. Rather than report a pass
> for a target it never touched, `tools/check_harmony_cross.sh` pins that specific
> expected outcome and fails if it ever changes.
>

---

## OS Support Matrix

### 1. Platform services per OS

These are the capabilities a backend *reports about the operating system*. Every
one is queried at runtime through `PlatformCapabilities`
(`rust_widgets::PlatformCapabilities`) — read it rather than assume, because a
backend running on an OS it was not compiled for reports `false`.

| OS | Backend | Family | DPI scaling | IME | Accessibility | Native menu | Configurable |
|----|---------|--------|:-----------:|:---:|:-------------:|:------------:|:------------:|
| **Windows** | `WindowsPlatform` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **macOS** | `cocoa` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **macOS** (objc2 preview) | `macos-objc2-preview` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux / GTK** | GTK backend | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux / Wayland** | `wayland` | Desktop | ✅ | ✅ | ✅ | ❌ | ✅ |
| **iOS** | `ios-state-backend` | Mobile | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Android** | `android-state-backend` | Mobile | ✅ | ✅ | ✅ | ❌ | ❌ |
| **HarmonyOS** | `harmony-desktop` | Desktop | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Web (WASM)** | `wasm-state-backend` | Embedded | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Portable / no-OS** | `portable` | Embedded | ❌ | ❌ | ❌ | ❌ | ❌ |

**Legend.** *Native menu* means the OS exposes a menu-bar protocol. Wayland has none,
so its backend keeps the menu tree in-process and the host renders it — advertising a
native menu would be false. *Configurable* means the backend exposes OS-level settings
(theme, accent colour, notifier) beyond the capability flags.

> **How to read the `native_menu` column.** A backend that does not override
> `Platform::capabilities` inherits the trait default, which is
> "`true` if the backend reports the `Desktop` family". Wayland, iOS, Android and
> HarmonyOS override it to `false` because they genuinely have no menu protocol;
> Windows, macOS and GTK keep the default. The values above are pinned by a test
> (`published_os_capability_matrix_matches_the_trait_default`), so they cannot drift.
>
> **The control set is *not* in this table, on purpose.** Because every control is
> self-drawn, widget availability does not vary by OS — it varies by **profile**.
> That is the next table.

### 2. Widget availability per profile

What differs across targets is how much of the widget set is **compiled in**, not
what the OS can draw.

| Profile | Widget set | Registry | Custom-painted controls | GPU | i18n |
|---------|-----------|:--------:|:-----------------------:|:---:|:----:|
| `desktop` | **179 kinds** (full) | ✅ | ✅ | ✅ wgpu | ✅ |
| `tablet` | **179 kinds** (full) | ✅ | ✅ | ✅ wgpu | ✅ |
| `mobile` | **179 kinds** (full) | ✅ | ✅ | ✅ wgpu | ✅ |
| `embedded` | reduced core set | — | — | — software | — |
| `mini` | reduced core set | — | — | — software | — |

A `—` is **absent, not degraded**: the module is compiled out, so
`supports_custom_widgets()` returns `false` and callers are expected to refuse the
operation rather than mount into a blank surface.

The reduced `embedded`/`mini` set is: Window, Button, CheckBox, RadioButton, Label,
LineEdit, ComboBox, SpinBox, ListBox, ProgressBar, Slider, ScrollBar, ScrollArea,
Panel, Frame, GroupBox, Line, Meter, MiniChart, ImageView, MiniCanvas,
Arc, Spinner, Roller, Dropdown, TextArea, Keyboard, Switch.

### 3. What "support" means per OS

Reading the two tables together:

| Concern | Varies by OS? | Varies by profile? |
|---|:---:|:---:|
| Control appearance | ❌ (self-drawn) | ❌ |
| Which controls exist | ❌ | ✅ |
| DPI scaling / IME / a11y | ✅ | ❌ |
| Native menu bar | ✅ | ❌ |
| File/colour/font dialogs | ✅ (host-provided) | ❌ |
| Rendering backend | ❌ | ✅ (GPU vs software) |

So an app that avoids OS-specific APIs is portable by construction: build it once
per profile, and it renders the same everywhere.

---

## Architecture

```
┌────────────────────────────────────────────────────────────┐
│  API Layer — lib.rs + compat.rs (core/alloc bridge)     │
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
- no_std-ready architecture: all files import shared types via `compat.rs` (`core`/`alloc`) so enabling `#![cfg_attr(feature = "mini", no_std)]` is a tracked step — the `mini` profile currently compiles on std.
- `compat.rs` bridge: `HashMap→BTreeMap`, lightweight-profile lock compatibility, `MiniVec<T,64>`, `MiniString<256>`, `MiniArena` (bumpalo)
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

### Theme System
- `ThemeManager` — named themes, light/dark switching, JSON load/save
- Semantic tokens (colours, fonts, spacing, borders) resolved per widget role
- `HighContrastMode` — forced background/foreground pair with a measurable contrast ratio
- Applied automatically to every control the library creates

### Declarative JSON UI (library API only)
- `JsonLoader` — build a widget tree from a JSON description
- Property application routed through each control's own property contract
- Optional per-node `class` / `css` for stylesheet-driven appearance
- **Not exposed over the C ABI** — the loader has no generated entry point

### Declarative view layer (`view`, device profiles)

Describe a widget tree as a function of your state, and let the library work out what
changed.

```rust
use rust_widgets::view::{Node, View, ViewEngine};
use rust_widgets::widget::capability::CapabilityValue;

struct Counter { count: i64 }

impl View for Counter {
    fn build(&self) -> Node {
        Node::new("group_box").key("root").child(
            Node::new("label")
                .key("count")
                .prop("text", CapabilityValue::String(format!("Count: {}", self.count))),
        )
    }
}

let mut engine = ViewEngine::new();
engine.mount(&state, &create);                  // build the tree
// ...state changes...
let report = engine.update(&state, &create);    // only the differences are applied
assert_eq!(report.patches.len(), 1);            // one SetProperty, nothing else
```

**Why use it.** Without it, every place that mutates state also has to reach the right
control and know which property to set — so "what should the screen look like when
`count == 3`" is answered nowhere in particular. With it, that question has exactly one
answer: `build`. Updates become cheap and local: the engine diffs the new tree against the
old one and touches only what differs, so a control's focus, scroll offset and internal
state survive an edit to a sibling.

**The retained model is unchanged.** Controls are still long-lived objects with an
`ObjectId`, `add_child` still works, and the two can be mixed. This layer adds a
*description* of structure; it replaces nothing.

**How to use it**

1. Implement `View::build` — return a `Node` tree from your state. Use the same factory
   names and property names the JSON loader uses.
2. Give it a constructor: `Fn(&Node) -> Option<ObjectId>`, usually `WidgetFactory::create`
   plus `runtime::register`. Injected rather than hardwired, so the layer is testable
   **without a window**.
3. `engine.mount(&state, &create)` once, then `engine.update(&state, &create)` whenever
   state changes.
4. Give list items a **`key`**. Keys are how the diff recognises the same control across
   rebuilds; without one, matching falls back to position and a head insert shifts every
   later node's identity onto the wrong controls. `report.positional_matches > 0` means
   "add keys".

**From reactive state** — `ReactiveHost` drives `update` from a `Binding`. Since
`BindingListener` must be `Send` while the engine is `!Send`, the listener records the
change on a queue and the UI thread's `pump()` does the work, so a `Binding::set` on a
worker thread reaches a live control.

| Profile | `view` |
|---|---|
| `desktop` / `tablet` / `mobile` | ✅ compiled **by default**; add `no-declarative-view` to leave it out |
| `mini` / `embedded` | ❌ **absent** — `add_child` only (allocation budget + no per-frame re-evaluation caller) |

> Not exposed over the C ABI — Rust only, like the JSON loader.
> Full guide: [cookbook/en/src/chapters/declarative-view.md](cookbook/en/src/chapters/declarative-view.md).

> **Financial controls:** six self-drawn market-data controls — a K-line chart with
> indicator overlays, a volume pane, a depth curve, an order book, a quote board and an
> oscillator pane — plus the technical-analysis arithmetic they share. See
> [cookbook/en/src/chapters/finance.md](cookbook/en/src/chapters/finance.md).

> **C ABI coverage.** The C ABI (`include/rw_generated.h`, 129 `rw_*` functions)
> covers window management, widget creation, per-widget properties and theme
> selection. Creation and property access are **generic**:
> `rw_create_widget_of_kind(parent, "tree_view", ...)` reaches every registered
> control (`rw_widget_kind_names` lists them), and
> `rw_set_widget_property(id, "tooltip", ...)` reaches every published property
> (`rw_widget_property_names` lists those). Themes are reachable through
> `rw_set_theme` / `rw_theme_names` / `rw_set_high_contrast`.
>
> Two things remain Rust-only: the **JSON layout loader** (no generated entry
> point) and **CSS stylesheets as documents** (individual style properties are
> settable per widget, but there is no ABI for shipping a stylesheet).

### C ABI, by capability

| Capability | Entry points |
|---|---|
| Window lifecycle | `rw_create_window`, `rw_run`, `rw_quit` |
| Generic creation | `rw_create_widget_of_kind`, `rw_widget_kind_names` |
| Typed creation | `rw_create_button`, `rw_create_slider`, … |
| Lifetime | `rw_destroy_widget`, `rw_show_widget`, `rw_hide_widget` |
| Generic properties | `rw_get_widget_property`, `rw_set_widget_property`, `rw_widget_property_names` |
| Text & geometry | `rw_set_widget_text`, `rw_get_widget_text`, `rw_set_widget_geometry` |
| Collections | `rw_widget_list_add`, `rw_widget_list_clear`, `rw_widget_list_count`, `rw_list_box_add_item`, `rw_combo_box_add_item`, … |
| Scrolling | `rw_widget_set_scroll_position`, `rw_widget_scroll_to` |
| Theme | `rw_set_theme`, `rw_theme_names`, `rw_set_high_contrast` |
| Errors | `rw_error_code`, `rw_error_message` |

Every binding under `bindings/` is checked against this list by
`tools/check_binding_symbol_coverage.sh`, so a function added to the ABI cannot
silently stay unreachable from a language.

### Partial Refresh (automatic, wired into the frame loop)
- `DirtyRegionTracker` with rectangle merging, and `render_dirty_regions()` for
  clip-based partial redraw via `push_clip` / `pop_clip`
- **Driven by `widget::runtime::RepaintMode`**: `mark_dirty_rect` records damage and
  `render_frame_incremental` repaints only the damaged regions, carrying the rest of
  the previous frame forward
- **Automatic: the library decides when to enable it.** At mount time a control is
  judged by `should_track_damage`, which enables `RepaintMode::Adaptive` only where
  regioning can pay off — a large surface, carrying more than one control, that has
  actually asked to be repainted. A surface below `AUTO_REPAINT_MIN_PIXELS` (≈500×500)
  or one with no children stays in `Full` and pays no bookkeeping, because for those
  regioning costs more than it saves
- Damage is recorded by `BaseWidget::request_redraw`, the single point every appearance
  change converges on, so partial repaint is correct by construction rather than
  depending on a list of mutation sites
- `Adaptive` is self-correcting: a frame whose damage covers the surface falls back to a
  whole paint for that frame and resumes regioning once the damage shrinks, so the
  automatic decision can never produce a wrong frame — only bounded bookkeeping
- The decision is observable, not a black box: `should_track_damage`,
  `enable_damage_tracking_if_useful`, `adaptive_large_damage_run`, and
  `set_repaint_mode` to overrule it

### Internationalization
- `tr!()` macro for compile-time key-based translation
- en / zh-cn / zh-tw translations (30+ strings per language)
- Context-based and plural variants
- `audit_keys()` for coverage validation

---

## Widget Library

### Desktop/Tablet/Mobile (179 widget kinds)

**Core**: Window, Dialog, MessageBox, FileDialog, ColorDialog, FontDialog, InputDialog, ProgressDialog, PopupWindow, Button, CheckBox, RadioButton, Label, LineEdit, TextEdit, RichEdit, ComboBox, SpinBox, ListBox, ListView, TreeView, TreeTable, ProgressBar, Slider, ScrollBar, ScrollArea, TabWidget, Splitter, GroupBox, Frame, MenuBar, Menu, MenuItem, ContextMenu, ToolBar, StatusBar, Canvas, Table, Grid, Chart, ToggleButton

**Date & Time**: Calendar, DateEdit, TimeEdit, DateTimeEdit, DatePicker, TimePicker, DateTimePicker, CupertinoDatePicker, DateRangePicker, MobileDatePicker

**Containers**: CollapsiblePane, DockWidget, MdiArea, StackedWidget, ToolBox, TabBar, NavigationStack, Carousel, BottomSheet, ModalBottomSheet

**Mobile**: BottomNavigationBar, NavigationDrawer, AppBar, SafeArea, PullToRefresh, RefreshControl, SearchBar, CupertinoSwitch, CupertinoSlider, CupertinoNavigationBar, CupertinoSegmentedControl, AdaptiveScaffold

**Input**: CommandLink, FontComboBox, KeySequenceEdit, MaskedEdit, AutoCompleteEdit, MultiSelectComboBox, EditableComboBox, RangeSlider, FloatingLabel, TagInput, InplaceEditor, SearchBox, ShortcutEditor

**Display**: LCDNumber, Dial, ProgressCircle, Rating, Icon, Sparkline, Tooltip, Badge, Chip, Avatar, SkeletonLoader, EmptyState

**Charts**: LineChart, BarChart, PieChart, Sparkline

**Web**: WebView, WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem, WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult, WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest

**Menus**: PieMenu, RibbonBar, MenuButton, DropdownMenu, Popover, SegmentedButton

**Special**: FreeformShape, QRCode, ColorHistory, ColorWell, MasonryLayout, Stepper, Divider, SwipeToDismiss, Toolbox, PropertiesPanel, PropertyGrid, WizardDialog, Wizard, AnimatedImage, HeroAnimation, BezierCurveEditor, LottieWidget, RiveWidget, VideoPlayer, ImageGallery, AudioVisualizer, CameraPreview, BarcodeScanner, Breadcrumb, SignaturePad, DropZone, CodeEditor, ColorPicker, CommandEntry, CommandPalette, DiffViewer, MapView, MediaPlayer, NotificationCenter, Snackbar, SplitButton, TerminalView, ToastStack

### Mini / Embedded (reduced core widget set)

Window, Button, CheckBox, RadioButton, Label, LineEdit, ComboBox, SpinBox, ListBox, ProgressBar, Slider, ScrollBar, ScrollArea, Panel, Frame, GroupBox, Line, Meter, MiniChart, ImageView, MiniCanvas, Arc, Spinner, Roller, Dropdown, TextArea, Keyboard, Switch

---

## Widget Properties

Every control publishes its own property contract, so you can read, write and
**enumerate** a control's state without knowing its concrete type. The same code
works for a button, a chart and a code editor, on every platform.

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::{
    widget_property_get, widget_property_names, widget_property_set, WidgetFactory,
};
use rust_widgets::CapabilityValue;

let factory = WidgetFactory::new_with_defaults();
let mut button = factory.create("button", Rect::new(10, 10, 100, 30), "OK").unwrap();

// Read and write by name
factory.write_property(button.as_mut(), "text", CapabilityValue::String("Save".into())).unwrap();
let text = factory.read_property(button.as_ref(), "text").unwrap();
assert_eq!(text, CapabilityValue::String("Save".into()));

// Or enumerate the whole contract — the API for a property editor or a serialiser.
// `enabled`, `visible`, `tooltip` and `geometry` appear here for every control.
for name in widget_property_names(button.as_ref()).unwrap() {
    println!("{name} = {:?}", widget_property_get(button.as_ref(), name).unwrap());
}
```

Because the list comes from the control itself, it cannot go stale — and a test
fails by name if a control advertises a property it will not answer.

### Error semantics

| Error | Meaning |
|---|---|
| `UnknownProperty` | This control has **no property by that name** — a caller bug. |
| `ReadOnlyProperty` | The property **exists** but is not writable (e.g. `geometry`, `row_count`). Render a disabled field. |
| `TypeMismatch` | Wrong value type, or a value out of range. |
| `UnsupportedOnWidget` | The control has no contract at all. Should not occur in 2.0.0. |

> Reading by **id** (`rust_widgets::widget::read_widget_property_by_id`) resolves
> through the widget runtime, so the control must be registered first; use the id
> `runtime::register` returns. See [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md).

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
| `compat` | core/alloc bridge, MiniVec, MiniString, MiniArena | All profiles |
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
| Mini | 1.87+ | heapless, hashbrown, bumpalo (no_std-ready; profile compiles on std) |
| Embedded | 1.87+ | None (software-only) |

### Image codecs and cross-compilation

AVIF support uses the **pure-Rust** `avif` codec (ravif), not `avif-native`, so
building `mobile`/`tablet`/`desktop` for a foreign target does **not** require a
`dav1d` sysroot or cross-configured `pkg-config`. Earlier releases pulled in
`dav1d-sys`, which failed to cross-compile for Android/iOS/wasm unless a
pkg-config sysroot was set up by hand.

The trade-off is decode speed: the pure-Rust codec is slower than the C `dav1d`
backend, and it adds ~15 build-time crates (`rav1e` et al.).

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
- **Cookbook**: [cookbook/](cookbook/) — the primary documentation, in English (`cookbook/en/`),
  Simplified Chinese (`cookbook/zh-CN/`) and Traditional Chinese (`cookbook/zh-TW/`)
