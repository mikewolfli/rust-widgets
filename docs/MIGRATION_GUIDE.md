# Migration Guide: 0.9.x → 0.10.x

## Overview

This guide covers breaking changes, new features, and migration steps when upgrading
from rust_widgets v0.9.x to v0.10.x. This release adds 55+ new controls, 6 new layout
managers, native FFI for macOS/iOS/Wayland/WASM/Android, a style sheet engine, i18n/l10n
infrastructure, undo/redo framework, data binding, print framework, and PDF export.

**Version:** 0.10.0  
**Target Rust version:** 1.87+  
**Edition:** 2021

---

## Breaking Changes

### WidgetKind Enum

Several `WidgetKind` variants have been renamed for consistency:

| 0.9.x (Old) | 0.10.x (New) | Reason |
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

// 0.10.x
use crate::platform::ime_macos::MacOsImeBridgeReal;
```

### Feature Profile Renames

| 0.9.x Feature | 0.10.x Feature | Notes |
|---|---|---|
| *(not available)* | `desktop` | Default profile — full desktop PC |
| *(not available)* | `tablet` | Touch-enabled, GPU-accelerated |
| *(not available)* | `mobile` | Mobile with touch + mobile API |
| *(not available)* | `embedded` | Stripped-down, no touch/i18n |
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
rust_widgets = "0.10"
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
cargo test --features desktop,wayland-native

# macOS
cargo test --features desktop,objc2-macos

# Windows
cargo test --features desktop  # Win32 is automatically enabled

# WASM
cargo check --target wasm32-unknown-unknown --features wasm
```

---

## New Cargo Features Reference

```toml
[features]
# Device-class profiles
default = ["desktop"]
desktop = ["desktop-runtime", "gpu-wgpu", "quality-management", "controls-native", "controls-custom", "advanced-widgets", "print", "pdf", "chart"]
tablet  = ["touch", "gpu-wgpu", "quality-management", "controls-native", "controls-custom"]
mobile  = ["touch", "gpu-wgpu", "quality-management", "mobile-api", "controls-native", "controls-custom"]
embedded = []  # stripped-down, no i18n/touch

# Interaction features
touch       = []   # Touch events + 11 gesture recognizers
holographic = []   # Z-axis depth events (laser holographic)
projection  = []   # Remote-control / air gestures

# Platform backends
desktop-runtime = []
gpu-wgpu        = ["wgpu"]
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
