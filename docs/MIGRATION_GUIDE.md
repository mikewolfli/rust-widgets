# Migration Guide

> **Latest: 1.x → 2.0.0** — see [the 2.0.0 section](#10x--200-self-drawn-controls) below.
> The rest of this document covers the earlier 0.9.x → 1.0.0 transition and is kept
> for projects still on that line.

---

# 1.x → 2.0.0: Self-Drawn Controls

2.0.0 is a **major** release: the library no longer creates native OS controls on any
platform. There are three breaking changes, and they all stem from that one decision.

## TL;DR

| You used to call | Now call |
|---|---|
| `Platform::create_button(parent, text, …)` | `WidgetFactory::create("button", rect, text)` |
| `platform.get_widget_text(id)` / `set_widget_text` | `WidgetFactory::read_property` / `write_property` |
| `platform.set_slider_value(id, v)` (or any `set_*` control method) | `write_property(widget, "value", CapabilityValue::Int(v))` |
| `NativeCapabilityContract::from_platform_caps(c)` | nothing — they are the same type now |
| `rust_widgets::widget::image::Image` | `rust_widgets::image::Image` |
| `rust_widgets::util::asset_watcher::AssetWatcher` | `rust_widgets::asset::AssetWatcher` |

If your code only ever used widgets through the `widget::runtime` API, **you very
likely need no changes at all** — that API was already the self-drawn path.

---

## 1. Native control creation is gone

### What changed

Every backend's `create_*` methods were deleted — **606 functions → 0**. That covers
Windows `CreateWindowExW`, macOS `NSButton`/`NSTextView`, GTK `gtk_button_new`,
Wayland control surfaces, iOS `UIButton`, Android `android.widget.*`, and the
Harmony/WASM state-backed equivalents.

### Why

A native control and a self-drawn control cannot be made to look the same, and
keeping both meant every feature had to be implemented twice with two sets of bugs.
The self-drawn path was already the one the widget library used; this release removed
the second path rather than maintaining both.

### What a backend still owns

Exactly four things, and nothing else:

1. **A drawing surface** — window creation, the paint callback, resize.
2. **The event loop.**
3. **Input translation** — keyboard/mouse/touch into a unified `Event`.
4. **Platform services** — IME, clipboard, accessibility bridge, native menus, file
   dialogs, DPI, wallpaper.

### How to migrate

```rust
// BEFORE (1.x) — a native control, built by the platform backend
let button = platform.create_button(window, "OK", 10, 10, 100, 30);
platform.set_widget_text(button, "Save");
let text = platform.get_widget_text(button);

// AFTER (2.0) — a self-drawn control, built by the factory
use rust_widgets::widget::WidgetFactory;
use rust_widgets::core::Rect;

let factory = WidgetFactory::new_with_defaults();
let mut button = factory
    .create("button", Rect::new(10, 10, 100, 30), "OK")
    .expect("button is registered");

factory.write_property(button.as_mut(), "text", CapabilityValue::String("Save".into()))?;
let text = factory.read_property(button.as_ref(), "text")?;
```

Controls are still addressed by id, so code that stores a handle keeps working. Only
*construction* moved.

### If you need a capability the host has

The four items above are queried, not assumed:

```rust
use rust_widgets::platform;

let caps = platform::capabilities();
if caps.ime { /* the host has an IME integration to talk to */ }
if caps.native_menu { /* the host exposes a menu-bar protocol */ }
```

`caps` reflects the **host**, not the target triple — a backend running on an OS it
was not compiled for answers `false` rather than claiming a feature it cannot deliver.

---

## 2. The property layer is per-control

### What changed

The centralised property dispatch was deleted: `read_widget_property_legacy`,
`write_widget_property_legacy`, their 18 `access_{read,write}_*.in.rs` include files,
**535 match arms**, and the 98 imports only they used.

Each control now implements `WidgetProperties` (`get` / `set` / `property_names`)
in its own file. All **156** controls do; a test asserts every factory-constructible
control has one.

### Why

The centralised table restated the contract for 39 controls, and the two copies had
already drifted. It also meant a control could not be added without editing a
central file — the opposite of the extensibility the capability layer exists for.

### How to migrate

`Platform::get_*` / `set_*` control-property methods are gone. Use either level:

```rust
// By widget reference (what most applications want)
let value = factory.read_property(list.as_ref(), "selected_row")?;
factory.write_property(list.as_mut(), "selected_row", CapabilityValue::UInt(4))?;

// By id (for a backend or a scripting host that holds ids).
//
// Note: these resolve the id through the widget runtime, so the control must be
// registered first — and `runtime::register` assigns the id it will answer for,
// so use its return value rather than the id the factory handed out.
use rust_widgets::widget::{read_widget_property_by_id, write_widget_property_by_id};
use rust_widgets::widget::runtime;

let id = runtime::register(widget).expect("must run on the UI thread");
write_widget_property_by_id(id, "selected_row", CapabilityValue::UInt(4))?;
let value = read_widget_property_by_id(id, "selected_row")?;
```

### Enumerate a control's properties at runtime

```rust
use rust_widgets::{widget_property_get, widget_property_names, widget_property_set};

// The published contract, including the four every control shares
// (enabled, visible, tooltip, geometry).
for name in widget_property_names(widget.as_ref()).unwrap_or(&[]) {
    println!("  {name} = {:?}", widget_property_get(widget.as_ref(), name)?);
}
```

This is the API for building a property editor or a serialiser; it cannot go stale
because it reads the same contract the control answers through.

### Error semantics to be aware of

The two "this did not work" errors mean different things, and 2.0.0 enforces the
distinction (42 call sites were corrected):

| Error | Meaning |
|---|---|
| `UnknownProperty` | This control has **no property by that name**. |
| `ReadOnlyProperty` | The property **exists**, but is not writable (e.g. `geometry`, `row_count`). |
| `UnsupportedOnWidget` | The **control itself** has no contract — it was not migrated. Should not occur in 2.0.0. |
| `TypeMismatch` | Wrong value type, or an out-of-range value. |

A property editor should render `ReadOnlyProperty` as a disabled field and
`UnknownProperty` as a bug in its own name list.

---

## 3. `Platform` lost its control methods

Required `Platform` methods went **75 → 6** (surface, event loop, lifecycle). The
rest became honest defaults that report `UnsupportedOnWidget` / `None` rather than
silently doing nothing.

If you implemented the `Platform` trait yourself, you can now delete the control
methods from that impl — they are no longer part of the contract.

---

## 4. Smaller breaks

| Removed | Replacement |
|---|---|
| `NativeCapabilityContract::from_platform_caps(c)` | None needed — `NativeCapabilityContract` is now `pub type NativeCapabilityContract = PlatformCapabilities;` |
| `rust_widgets::widget::image::{Image, ImageFormat}` | `rust_widgets::image::{Image, ImageFormat}` |
| `rust_widgets::util::asset_watcher::{AssetWatcher, AssetEvent}` | `rust_widgets::asset::{AssetWatcher, AssetEvent}` |
| `platform::detector::DeviceEnvironment` | Deleted (no in-tree consumer) |
| `platform::virtual_keyboard::VirtualKeyboard` | Deleted (no in-tree consumer) |
| `render::text_cache::TextCache` | Deleted (no in-tree consumer) |
| `style::css_watcher::CssWatcher` | Deleted (no in-tree consumer) |
| `widget::overlay_widgets::pull_to_refresh` | `rust_widgets::widget::PullToRefresh` (unchanged) |

**ABI is unchanged**: `rw_bindings_api_version` remains `8`, and no exported `rw_*`
symbol was added, removed or changed. C, Python and Node bindings need no update.

---

## 5. Behaviour changes worth knowing

These are fixes, not migrations, but they change what you observe:

- **`TagInput::placeholder` is now real.** The renderer used to draw a hardcoded
  `"Type and press Enter..."` regardless of state. The placeholder is now a settable
  property (default `"Type and press Enter\u{2026}"`), and the draw path honours it.
- **`TabBar.current_index` can now be cleared.** `set` accepts `Null` to select no tab,
  matching what `get` publishes for that state. Previously `get` could return `Null`
  but `set` rejected it, so a read → write → read round trip did not close.
- **`Arc` publishes five more properties** (`minimum`, `maximum`, `sweep_angle`,
  `thickness`, `indeterminate`). They were writable before but absent from
  `property_names()`, i.e. invisible to anything that enumerates the contract.
- **`DataGrid` is not paginated.** It never was; it is a virtualised scroller with a
  window cache. If you are looking for paging properties, there are none to find.

---

# Migration Guide: 0.9.x → 1.0.0

> **Note (2026-09-02)**: this guide was originally drafted for the planned
> "0.10.0" milestone. The project now releases that milestone as the stable
> **1.0.0** line — every "0.10.x" version mentioned below should be read as
> "1.0.0".

## Overview

This guide covers breaking changes, new features, and migration steps when upgrading
from rust_widgets v0.9.x to v1.0.0. This release adds 55+ new controls, 6 new layout
managers, native FFI for macOS/iOS/Wayland/WASM/Android, a style sheet engine, i18n/l10n
infrastructure, undo/redo framework, data binding, print framework, and PDF export.

**Version:** 1.0.0
**Target Rust version:** 1.87+
**Edition:** 2021

---

## Breaking Changes

### WidgetKind Enum

Several `WidgetKind` variants have been renamed for consistency:

| 0.9.x (Old) | 1.0.0 (New) | Reason |
|---|---|---|
| `ToolBox` | `Toolbox` | Lowercase 'b' for naming consistency |
| `DataView` | Type alias `DataView` | Now an explicit type alias in widget::data_view |
| `ColumnView` | Type alias `ColumnView` | Now an explicit type alias in widget::column_view |
| `UndoView` | Type alias `UndoView` | Now an explicit type alias in widget::undo_view |
| `CommandLink` | Type alias `CommandLink` | Now an explicit type alias |
| `LCDNumber` | Type alias `LcdNumber` | Now an explicit type alias |
| `FontComboBox` | Type alias `FontComboBox` | Now an explicit type alias |

**Action required:** Replace `WidgetKind::ToolBox` with `WidgetKind::Toolbox` in match arms.
Orphan variants now use widget-specific type aliases — import from their respective modules.

### Platform Backend Changes

#### macOS (objc2 native backend)
- The `objc2-macos` feature is required for native NSWindow/NSButton creation.
- The legacy `cocoa 0.24` backend remains available but is **deprecated** and will be removed in 0.11.
- New module: `src/platform/macos_objc2/` with full AppKit FFI wrappers.
- **Migration:** Enable `objc2-macos` feature in Cargo.toml:
  ```toml
  [features]
  my-profile = ["objc2-macos"]
  ```

#### iOS (UIKit FFI)
- The `ios-uikit-ffi` feature enables real UIKit view creation via `objc2-ui-kit`.
- Previously state-only on iOS; now creates actual UIView/UIButton/UILabel etc.
- **Migration:** Enable `ios-uikit-ffi` in Cargo.toml:
  ```toml
  [features]
  my-profile = ["ios-uikit-ffi"]
  ```

#### Android (JNI Bridge)
- New module: `src/platform/android/` — requires `android-jni` feature.
- Native `android.widget.Button`, `CheckBox`, `RadioButton`, `ProgressBar`, `SeekBar`, etc. are created via JNI.
- The `with_jni_env()` function provides safe JNI environment access.
- **Migration:** Enable `android-jni` and add the `jni` dependency:
  ```toml
  [features]
  my-profile = ["android-jni"]
  ```

#### WASM (WebAssembly)
- New module: `src/platform/wasm/` — requires `wasm` feature.
- Uses `wasm-bindgen`, `web-sys`, and `js-sys` for browser DOM access.
- Canvas rendering via HTML canvas element.
- **Migration:** Enable `wasm` feature:
  ```toml
  [features]
  my-profile = ["wasm"]
  ```

#### Wayland (Native Protocol)
- The `wayland-native` feature wires `wayland-client` / `wayland-protocols` for real compositor interaction.
- xdg_toplevel surface creation, wl_output DPI detection, and event dispatch.
- **Migration:** Enable `wayland-native` on Linux:
  ```toml
  [features]
  my-profile = ["wayland-native"]
  ```

### IME API Changes

The IME bridge types have been renamed for consistency with the `Real` suffix convention:

| 0.9.x (Old) | 0.10.x (New) | Platform |
|---|---|---|
| `MacOsImeBridge` | `MacOsImeBridgeReal` | macOS (NSTextInputContext) |
| `WindowsImeBridge` | `WindowsImeBridgeReal` | Windows (TSF) |
| *(new)* | `LinuxImeBridgeReal` | Linux (IBus via zbus) |

**Action required:** Update type references and imports:
```rust
// 0.9.x
use crate::platform::ime_macos::MacOsImeBridge;

// 1.0.0
use crate::platform::ime_macos::MacOsImeBridgeReal;
```

### Feature Profile Renames

| 0.9.x Feature | 0.10.x Feature | Notes |
|---|---|---|
| *(not available)* | `desktop` | Default profile — full desktop PC |
| *(not available)* | `tablet` | Touch-enabled, GPU-accelerated |
| *(not available)* | `mobile` | Mobile with touch + mobile API |
| *(not available)* | `embedded` | Software-rendered core profile; use with `--no-default-features` |
| *(not available)* | `full` | Meta-feature enabling all compatible features |

The `full` meta-feature is **not** a runtime device profile — it enables everything that can coexist
for documentation builds and testing. Use a device-class profile (`desktop`/`tablet`/`mobile`/`embedded`)
as the base for production builds.

---

## New Features

### Infrastructure

- **i18n/l10n system** — `tr!("key")` macro, `I18nManager`, JSON translation files, hot reload.
  Translations are loaded from `language/<lang>.json` at runtime.

- **StyleSheet engine** — CSS-like selectors with property matching. Supports widget type selectors,
  class selectors, and ID selectors with property-value pairs.

- **App Lifecycle management** — `AppLifecycle` with foreground/background state tracking.
  Handles platform `applicationWillResignActive` / `applicationDidBecomeActive` events
  and suspends/resumes rendering accordingly.

- **Undo/Redo framework** — `UndoStack`, `UndoCommand` trait with merge support.
  Supports nested undo groups and command compression.

- **Data binding** — `Binding<T>`, `ObservableList<T>`, `Computed<T>` for reactive
  Model → View automatic synchronization.

- **Print framework** — `PrintManager`, `PrintJob`, `PrintDocument` trait for system print dialogs.

- **PDF export** — `PdfExporter`, `export_to_pdf()` for page-based PDF generation.
  Supports text, images, and vector graphics.

### Layout System (6 new layouts)

| Layout | Description |
|---|---|
| `FlexLayout` | CSS Flexbox-style elastic layout with grow/shrink/basis properties |
| `WrapLayout` | Auto-wrap flow layout that wraps items to next line on overflow |
| `KeyboardAwareLayout` | Mobile keyboard avoidance — adjusts content when virtual keyboard appears |
| `ConstraintLayout` | Anchor-based constraint layout (similar to iOS Auto Layout) |
| `CenterLayout` | Single child centering — both horizontal and vertical centering |
| `AspectRatioLayout` | Aspect ratio preservation — maintains a fixed width/height ratio |

### New Controls (55+)

| Control | Module | Description |
|---|---|---|
| `ToggleButton` | `widget::toggle_button` | Push-button with checked state and auto-exclusive support |
| `CheckListBox` | `widget::check_list_box` | List with checkboxes per item |
| `DoubleSpinBox` | `widget::double_spin_box` | Double-precision numeric input with up/down arrows |
| `Dial` | `widget::dial` | Rotary dial control with angle-based value |
| `Wizard` | `widget::wizard` | Multi-step dialog with back/next/finish buttons |
| `DatePicker` | `widget::date_picker` | Calendar-based date selection |
| `TimePicker` | `widget::time_picker` | Spin-based time selection (hours/minutes/seconds) |
| `DateTimePicker` | `widget::date_time_picker` | Combined date + time selection |
| `DirectoryPicker` | `widget::directory_picker` | Directory selection dialog |
| `DataView` | `widget::data_view` | Tabular data visualization with sorting |
| `PropertyGrid` | `widget::property_grid` | Property editing interface |
| `Toolbox` | `widget::toolbox` | Tool palette with categorized items |
| `StackedWidget` | `widget::stacked_widget` | Notebook/stacked container |
| `CollapsiblePane` | `widget::collapsible_pane` | Collapsible container with header |
| `DockWidget` | `widget::dock_widget` | Dockable panel with drag-to-detach |
| `WebView` | `widget::web_view` | Web browser content display |
| `ActivityIndicator` | `widget::activity_indicator` | Progress/activity spinner |
| `Calendar` | `widget::calendar` | Calendar display and date selection |
| `ColumnView` | `widget::column_view` | Column-based data view (tree table) |
| `UndoView` | `widget::undo_view` | Undo/redo stack visualization |
| `CommandLink` | `widget::command_link` | Command link button with description |
| `LcdNumber` | `widget::lcd_number` | Digital number display (7-segment) |
| `FontComboBox` | `widget::font_combo_box` | Font selection combo box |

**Web Engine Controls** (feature: `advanced-widgets`):

| Control | Description |
|---|---|
| `WebEngineView` | Web content display with navigation |
| `WebEnginePage` | Web content page management |
| `WebEngineSettings` | Web engine configuration |
| `WebEngineDownloadItem` | Download management |
| `WebEngineCookieStore` | Cookie management |
| `WebEngineWebChannel` | JavaScript ↔ Rust communication |
| `WebEngineFindTextResult` | Text search results |
| `WebEngineNotification` | Web notifications |
| `WebEngineScriptDialog` | JavaScript dialogs |
| `WebEngineContextMenuRequest` | Context menu handling |

### Platform-Specific Improvements

- **macOS:** objc2 native NSWindow/NSButton/NSSlider/NSTextField with `MainThreadMarker` safety.
- **iOS:** UIKit UIView/UIButton/UILabel/UISwitch/UISlider/etc. via objc2-ui-kit.
- **Windows:** Win32 native controls (Button, Label, CheckBox, RadioButton, LineEdit, ComboBox,
  ListBox, ProgressBar, Slider, Trackbar, etc.) with window procedure event dispatch.
- **Wayland:** xdg_toplevel surface creation with compositor registry, wl_output DPI detection.
- **Android:** JNI bridge for native android.widget views.
- **WASM:** Browser DOM integration via web-sys, canvas rendering.

---

## Deprecations

| Deprecated | Replacement | Removal Version |
|---|---|---|
| `cocoa 0.24` backend (macOS) | `objc2-macos` backend (`objc2-app-kit`) | 0.11 |
| State-only macOS backend | Native objc2 AppKit FFI | 0.11 |
| State-only iOS backend | Native UIKit FFI via `ios-uikit-ffi` | 0.11 |
| State-only Wayland backend | Native wayland-client protocol via `wayland-native` | 0.12 |
| `MacOsImeBridge` (old name) | `MacOsImeBridgeReal` | 0.11 |
| `WindowsImeBridge` (old name) | `WindowsImeBridgeReal` | 0.11 |
| `ToolBox` WidgetKind variant | `Toolbox` (lowercase 'b') | 0.11 |

---

## Migration Steps

### Step 1: Update Cargo.toml

Update the version requirement:
```toml
[dependencies]
rust_widgets = "1.0"
```

Choose a device profile and enable desired features:
```toml
[features]
# Production: desktop
my-app = ["rust_widgets/desktop", "rust_widgets/objc2-macos"]

# Or for mobile:
# my-app = ["rust_widgets/mobile", "rust_widgets/android-jni"]
```

### Step 2: Replace Deprecated API Calls

Search your codebase for deprecated identifiers and update:
```rust
// Before
use rust_widgets::platform::ime_macos::MacOsImeBridge;

// After
use rust_widgets::platform::ime_macos::MacOsImeBridgeReal;
```

### Step 3: Update WidgetKind References

```rust
// Before
WidgetKind::ToolBox => { /* ... */ }

// After
WidgetKind::Toolbox => { /* ... */ }

// Orphan variants now use type aliases:
// Before
let kind = WidgetKind::DataView;
// After  
let kind = widget::data_view::DataView::widget_kind();
```

### Step 4: Enable Native FFI Features

For improved performance, enable the appropriate native FFI features per platform:

- **macOS:** `objc2-macos`
- **iOS:** `ios-uikit-ffi`
- **Linux (Wayland):** `wayland-native`
- **Android:** `android-jni`
- **WASM:** `wasm`

```rust
// Check if native FFI is active:
if cfg!(feature = "objc2-macos") {
    // macOS native NSWindow is available
}
```

### Step 5: Review Feature Profiles

If you were previously using custom feature sets, review the new device profiles:
- `desktop` — replaces most custom desktop configurations
- `mobile` — replaces mobile configurations
- `embedded` — replaces embedded/stripped configurations
- `tablet` — new profile for touch-enabled tablets

### Step 6: Test Platform-Specific Code

Run tests on each target platform:
```bash
# Linux
cargo test --no-default-features --features desktop,wayland-native

# macOS
cargo test --no-default-features --features desktop,objc2-macos

# Windows
cargo test --no-default-features --features desktop  # Win32 is automatically enabled

# WASM
cargo check --target wasm32-unknown-unknown --no-default-features --features wasm
```

---

## New Cargo Features Reference

```toml
[features]
# Device-class profiles
default = ["desktop"]
desktop = ["desktop-runtime", "wgpu", "quality-management", "controls-native", "controls-custom", "advanced-widgets", "print", "pdf", "chart"]
tablet  = ["touch", "wgpu", "quality-management", "controls-native", "controls-custom"]
mobile  = ["touch", "wgpu", "quality-management", "mobile-api", "controls-native", "controls-custom"]
embedded = ["software", "controls-custom"]

# Interaction features
touch       = []   # Touch events + 11 gesture recognizers
holographic = []   # Z-axis depth events (laser holographic)
projection  = []   # Remote-control / air gestures

# Platform backends
desktop-runtime = []
wgpu            = ["gpu"]
wayland-native  = ["wayland-client", "wayland-protocols", "wayland-cursor"]
gtk-native      = ["gtk"]
objc2-macos     = ["objc2", "objc2-foundation", "objc2-app-kit", "objc2-core-graphics"]
ios-uikit-ffi   = ["objc2", "objc2-foundation", "objc2-ui-kit"]
android-jni     = ["jni"]
wasm            = ["wasm-bindgen", "web-sys", "js-sys"]

# Content modules
print  = []
pdf    = []
chart  = []
advanced-widgets = []
```

---

## Need Help?

- **Issues:** https://github.com/mikewolfli/rust-widgets/issues
- **Documentation:** `cargo doc --features full --open`
- **Examples:** See the `examples/` directory for working code samples
