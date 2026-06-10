# Getting Started

This chapter walks you through installing rust-widgets, configuring your first
project, writing a complete application, and understanding the fundamental
patterns that every rust-widgets application follows.

---

## Prerequisites

Before you begin, ensure your environment meets these requirements:

| Requirement | Minimum | Notes |
|---|---|---|
| **Rust** | 1.87+ (MSRV) | Check with `rustc --version` |
| **OS** | Linux, macOS, Windows, Android, iOS, WASM, HarmonyOS | |
| **Platform SDKs** | See table below | Only needed for the target you build for |

### Platform Dependencies

| Platform | Required Packages |
|---|---|
| **Linux (GTK)** | `libgtk-3-dev` |
| **Linux (Wayland)** | `libwayland-dev`, `wayland-protocols` |
| **macOS / iOS** | Xcode Command Line Tools |
| **Windows** | Visual Studio Build Tools (MSVC) |
| **Android** | Android NDK, `cargo-ndk` |
| **WASM** | `wasm-bindgen-cli`, `wasm-pack` |

Install Rust via [rustup](https://rustup.rs) if you haven't already:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

---

## Adding rust-widgets to Your Project

Create a new binary crate and add `rust_widgets` as a dependency:

```sh
cargo new my_rust_widgets_app
cd my_rust_widgets_app
```

Edit `Cargo.toml` and add the dependency:

```toml
[package]
name = "my_rust_widgets_app"
version = "0.1.0"
edition = "2021"

[dependencies]
rust_widgets = "0.9.6"
```

The default feature set (`desktop`) pulls in the full desktop profile: GPU
rendering via wgpu, OS-native platform backend, touch support, i18n, charts,
PDF/printing, accessibility, and advanced widgets.

---

## Feature Selection — The Three-Axis System

rust-widgets uses a **three-axis feature system** that lets you compose exactly
the binary you need. Pick one option from each axis:

### Axis 1: Device Profile (mutually exclusive — pick ONE)

| Profile | Feature Flag | Description |
|---|---|---|
| **Desktop** | `desktop` (default) | Full native platform, GPU, touch, i18n, charts, print, PDF, a11y |
| **Tablet** | `tablet` | Touch-first, GPU, native controls, no desktop extras |
| **Mobile** | `mobile` | Touch, GPU, mobile API, native controls |
| **Embedded** | `embedded` | Software raster, no GPU, low memory, `no_std`-compatible |
| **Mini** | `mini` | LVGL-style: ~15 core widgets, software raster, no alloc-heavy deps |

### Axis 2: OS Backend (pick one or auto-detect)

| Backend | Feature Flag | Target |
|---|---|---|
| **Auto-detect** | `os-auto` (default) | Picks backend by `target_os` |
| **macOS (objc2)** | `macos` | macOS via objc2 bindings |
| **macOS (legacy)** | `macos-legacy` | macOS via cocoa crate |
| **iOS** | `ios` | iOS via UIKit FFI |
| **Windows** | `windows` | Windows via Win32 API |
| **Linux GTK** | `linux-gtk` | Linux via GTK3 bindings |
| **Linux Wayland** | `linux-wayland` | Linux via Wayland protocol |
| **Android** | `android` | Android via JNI |
| **WASM** | `wasm` | Web via wasm-bindgen |
| **HarmonyOS** | `harmony` | HarmonyOS native |

### Axis 3: Capabilities (arbitrary composition)

| Capability | Feature Flag | Pulls In |
|---|---|---|
| **GPU rendering** | `wgpu` / `gpu` | `wgpu` crate |
| **Software raster** | `software` | CPU renderer |
| **Touch & gestures** | `touch` | 11 gesture recognizers |
| **i18n** | `i18n` | `tr!()` macro + translation infrastructure |
| **Charts** | `chart` | LineChart, BarChart, PieChart, Sparkline |
| **PDF output** | `pdf` | Document generation pipeline |
| **Printing** | `print` | System print services |
| **Accessibility** | `a11y` | AT-SPI bridge via zbus |
| **Holographic** | `holographic` | Holographic display support |
| **Projection** | `projection` | Projector/presentation display |

### Example `Cargo.toml` Selections

```toml
# Desktop Linux with Wayland, keep everything:
[dependencies]
rust_widgets = { version = "0.9.6", features = ["desktop", "linux-wayland"] }

# Tablet with auto-detected OS:
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["tablet"] }

# Minimal embedded (no std):
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["embedded"] }

# Mobile Android:
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["mobile", "android"] }

# WASM web app:
[dependencies]
rust_widgets = { version = "0.9.6", default-features = false, features = ["mobile", "wasm", "touch"] }
```

### Build Profiles

Two additional release profiles are provided for constrained targets:

```toml
# In your Cargo.toml:
[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"

[profile.release-mini]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

Build with:
```sh
cargo build --profile release-embedded --features embedded
```

---

## Your First Application: Complete Working Example

Below is a complete, self-contained rust-widgets application. Create
`src/main.rs`:

```rust
use rust_widgets::prelude::*;
use rust_widgets::app::{App, AppConfig};

fn main() {
    // ── 1. Create the app with configuration ──
    let app = App::with_config(
        AppConfig::default()
            .with_app_name("HelloApp")
            .with_organization("Acme Corp"),
    )
    .on_startup(|| {
        println!("Application starting up...");
    })
    .on_shutdown(|| {
        println!("Application shutting down.");
    });

    // ── 2. Initialize the runtime (platform backend + renderer) ──
    app.init();

    // ── 3. Create the main window ──
    let window = app.new_window("My First rust-widgets Window", 100, 100, 800, 600);

    // ── 4. Create widgets using handles ──
    let button = window.new_button("Click Me!", 10, 10, 120, 32);
    let label = window.new_label("Hello, rust-widgets!", 10, 60, 300, 24);

    // ── 5. Connect signals ──
    let mut counter = 0;
    button.on_click(move || {
        counter += 1;
        label.set_text(&format!("Clicked {} times", counter));
    });

    // ── 6. Run the event loop ──
    app.run();
}
```

> **Note**: The `prelude` module re-exports the most commonly used types:
> all geometry types (`Point`, `Size`, `Rect`), colors (`Color`), fonts (`Font`),
> and widget constructors. Import it at the top of every rust-widgets
> application.

---

## Building and Running

```sh
# Development build (fast compile, no optimizations):
cargo build

# Run:
cargo run

# Release build (with LTO, codegen-units=1):
cargo build --release
cargo run --release
```

---

## The Widget Creation Pattern

rust-widgets provides two complementary widget-creation APIs:

### 1. Top-Level `create_*` Functions (Low-Level)

These are C-ABI-friendly top-level functions in `lib.rs` that allocate a widget
and return its `ObjectId`:

```rust
use rust_widgets::{
    create_window, create_button, create_label,
    create_checkbox, create_line_edit, create_slider,
    create_progress_bar, create_combo_box, create_list_box,
    create_spin_box, create_list_view, create_scroll_area,
    create_panel, create_message_box, create_file_dialog,
    create_color_dialog, create_font_dialog,
    create_radio_button,
};

let window_id = create_window("My Window", 100, 100, 800, 600);
let button_id = create_button("Submit", 10, 10, 120, 32);
let label_id = create_label("Status", 10, 60, 300, 24);
```

Available `create_*` functions:

| Function | Creates |
|---|---|
| `create_window(name, x, y, w, h)` | Top-level window |
| `create_button(text, x, y, w, h)` | Push button |
| `create_checkbox(text, x, y, w, h)` | Check box |
| `create_radio_button(text, x, y, w, h)` | Radio button |
| `create_label(text, x, y, w, h)` | Text label |
| `create_line_edit(text, x, y, w, h)` | Single-line text input |
| `create_slider(min, max, val, x, y, w, h)` | Horizontal slider |
| `create_progress_bar(min, max, val, x, y, w, h)` | Progress indicator |
| `create_combo_box(x, y, w, h)` | Drop-down combo |
| `create_list_box(x, y, w, h)` | Scrolling list |
| `create_spin_box(min, max, step, val, x, y, w, h)` | Numeric spinner |
| `create_list_view(x, y, w, h)` | Multi-column list |
| `create_scroll_area(x, y, w, h)` | Scrollable container |
| `create_panel(x, y, w, h)` | Panel/GroupBox |
| `create_message_box(title, msg)` | Modal message dialog |
| `create_file_dialog(title, dir, filter)` | File chooser |
| `create_color_dialog(title)` | Color picker dialog |
| `create_font_dialog(title)` | Font selection dialog |

### 2. App API with Typed Handles (Recommended)

The `App` API returns **typed handles** that provide compile-time safety and
convenience methods. This is the recommended approach for new applications:

```rust
use rust_widgets::app::{App, AppConfig};

let app = App::new();
app.init();

let window = app.new_window("Title", 100, 100, 640, 480);

// Each handle exposes widget-specific methods:
let btn  = window.new_button("OK", 10, 10, 80, 24);
let chk  = window.new_checkbox("Enable", 10, 40, 120, 24);
let edit = window.new_line_edit("", 10, 70, 200, 24);
let lbl  = window.new_label("Output", 10, 100, 300, 24);
let cb   = window.new_combo_box(10, 130, 200, 24);
let list = window.new_list_box(10, 160, 200, 100);
let prog = window.new_progress_bar(0, 100, 50, 10, 270, 200, 20);
let spin = window.new_spin_box(0, 100, 1, 50, 10, 300, 80, 24);
let grid = window.new_grid(10, 330, 400, 200);
let frame = window.new_frame("Section", 10, 540, 400, 50);
let radio = window.new_radio_button("Option A", 420, 10, 120, 24);
let slider = window.new_slider(0, 100, 50, 420, 40, 200, 24);
let scroll = window.new_scroll_area(420, 70, 200, 150);
let tab = window.new_tab_widget(420, 230, 200, 150);
let web = window.new_web_view(420, 390, 200, 150);
```

> **Design principle**: rust-widgets uses a **builder pattern** for widget
> types. Every widget struct exposes a `new(...)` constructor but the `App` API
> offers a more ergonomic factory via `WindowHandle`.

---

## The Event Loop

rust-widgets manages its own event loop. The three key lifecycle functions are:

```rust
// In lib.rs — top-level API:
pub fn init();   // Initialize the platform backend + renderer
pub fn run();    // Enter the event loop (blocks until quit)
pub fn quit();   // Signal the event loop to exit
```

### Using the App API

The `App` struct wraps these lifecycle functions:

```rust
let app = App::with_config(AppConfig::default()
    .with_app_name("MyApp"))
    .on_startup(|| { /* initialize data */ })
    .on_shutdown(|| { /* cleanup */ });

app.init();  // ← must call before creating widgets
// ... create widgets ...
app.run();   // ← blocks here, processes events until window closes
```

### The AppConfig Builder

```rust
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_name: String,        // Used for window titles, etc.
    pub organization: String,    // Vendor/organization name
    pub enable_i18n: bool,       // Initialize i18n subsystem (default: true)
}

impl AppConfig {
    pub fn with_app_name(mut self, name: &str) -> Self;
    pub fn with_organization(mut self, org: &str) -> Self;
    pub fn with_i18n(mut self, enable: bool) -> Self;
}
```

### Lifecycle State Machine

The `AppLifecycle` state machine tracks application state transitions:

```rust
use rust_widgets::app::lifecycle::{AppLifecycle, AppLifecycleState, LifecycleEvent};

// States:
// Starting → Foreground → Background → Suspended → Terminating
//
// Events:
// WillEnterForeground, DidEnterForeground,
// WillEnterBackground, DidEnterBackground,
// WillTerminate, MemoryWarning, StateRestored

let mut lifecycle = AppLifecycle::new();
lifecycle.transition(AppLifecycleState::Foreground);

lifecycle.add_listener(Box::new(move |event| {
    match event {
        LifecycleEvent::WillEnterBackground => { /* pause work */ }
        LifecycleEvent::DidEnterForeground => { /* resume work */ }
        LifecycleEvent::MemoryWarning => { /* free caches */ }
        _ => {}
    }
}));
```

---

## Loading JSON Layouts at Runtime

rust-widgets supports defining widget trees in JSON for dynamic UI loading:

```rust
use rust_widgets::json;

let json_str = r#"
{
    "widgets": [
        {
            "kind": "Window",
            "title": "JSON Window",
            "geometry": { "x": 100, "y": 100, "width": 800, "height": 600 },
            "children": [
                {
                    "kind": "Button",
                    "id": "btn_submit",
                    "text": "Submit",
                    "geometry": { "x": 10, "y": 10, "width": 120, "height": 32 }
                },
                {
                    "kind": "Label",
                    "id": "lbl_status",
                    "text": "Ready",
                    "geometry": { "x": 10, "y": 60, "width": 300, "height": 24 }
                }
            ]
        }
    ]
}
"#;

// Parse and build the widget tree from JSON:
let result = json::build_from_json(json_str);
```

JSON-defined widgets can be mixed with programmatically created widgets. The
JSON module supports all `WidgetKind` variants and their constructors.

---

## Using the `tr!()` Macro for i18n

The `tr!()` macro provides compile-time key-based translation. Translation
keys are extracted statically and checked at build time via a coverage auditor.

```rust
use rust_widgets::tr;

// Basic translation:
let greeting = tr!("hello_world");        // → "Hello, world!" (en)
                                          // → "你好，世界！" (zh-CN)

// Context-based translation:
let save = tr!("save");                   // generic
let save_file = tr!("save", context: "file");  // context-aware

// Plural forms:
let items = tr!("item_count", count: 1);  // → "1 item"
let items = tr!("item_count", count: 5);  // → "5 items"
```

### Setting Translated Tooltips on Widgets

```rust
// On widget trait:
widget.set_translated_tooltip("tooltip.save_button");

// The tooltip will display the locale-appropriate translation.
```

The i18n subsystem ships with **English** (en), **Simplified Chinese** (zh-CN),
and **Traditional Chinese** (zh-TW) translations built in. The `audit_keys()`
function catches missing translations at build time, ensuring your translation
coverage is always complete.

---

## Widget Handles Pattern

Widget handles are thin wrappers around `ObjectId` that provide type-safe,
ergonomic operations for each widget kind.

### The `WidgetHandle` Trait

```rust
pub trait WidgetHandle: Sized {
    fn raw_id(&self) -> ObjectId;
    fn from_raw(id: ObjectId) -> Self;

    // Visibility
    fn show(&self);
    fn hide(&self);
    fn set_visible(&self, visible: bool);
    fn is_visible(&self) -> bool;

    // Geometry
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32);

    // Text
    fn set_text(&self, text: &str);
    fn text(&self) -> String;

    // Enabled state
    fn enable(&self);
    fn disable(&self);
    fn is_enabled(&self) -> bool;

    // Event callbacks
    fn on_click<F: FnMut() + 'static>(&self, f: F);
    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F);
}
```

### Specialized Handle Types

| Handle Type | Widget | Extra Methods |
|---|---|---|
| `WindowHandle` | Window | `new_button()`, `new_label()`, `new_line_edit()`, etc. |
| `ButtonHandle` | Button | Click callbacks |
| `CheckBoxHandle` | CheckBox | `set_checked()`, `is_checked()`, `CheckState` |
| `LabelHandle` | Label | Text get/set |
| `LineEditHandle` | LineEdit | `set_echo_mode()`, `EchoMode` |
| `ComboBoxHandle` | ComboBox | `add_item()`, `clear()`, `set_current_index()` |
| `ListBoxHandle` | ListBox | `add_item()`, `remove_item()`, `current_index()` |
| `SliderHandle` | Slider | `set_value()`, `value()`, `set_range()` |
| `ProgressBarHandle` | ProgressBar | `set_value()`, `set_range()` |
| `SpinBoxHandle` | SpinBox | `set_value()`, `value()`, `set_range()` |
| `ScrollBarHandle` | ScrollBar | `set_value()`, `set_range()` |
| `TabWidgetHandle` | TabWidget | `add_tab()`, `set_current_index()` |
| `ScrollAreaHandle` | ScrollArea | Container for other widgets |
| `ListViewHandle` | ListView | `ListModel` integration |
| `TextEditHandle` | TextEdit | Multi-line editing |
| `FrameHandle` | Frame/GroupBox | Container frame |
| `GridWidgetHandle` | Grid | Grid layout |
| `RadioButtonHandle` | RadioButton | Selection state |
| `MessageBoxHandle` | MessageBox | Modal message display |
| `WebViewHandle` | WebView | Web content display |
| `DialogHandle` | Dialog/PopupWindow | Modal dialog |

### Callback Dispatch

Callbacks are stored per-`ObjectId` in thread-local storage and dispatched when
the platform backend emits trigger events:

```rust
// Internal dispatch function:
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool;

// WidgetHandle::on_click registers a callback:
button.on_click(|| println!("Clicked!"));

// WidgetHandle::on_value_changed registers a value-change callback:
combo.on_value_changed(|text| println!("Selected: {}", text));
```

---

## Common Patterns and Best Practices

### 1. Group Related Widgets in Container Handles

```rust
// Good: Create children through the window handle
let button = window.new_button("OK", 10, 10, 80, 32);

// The window handle tracks the parent-child relationship internally.
```

### 2. Use Scoped Signal Connections

When working with the `Signal` system directly (not via handles), use
`ConnectionScope` for automatic cleanup:

```rust
use rust_widgets::signal::ConnectionScope;

let scope = ConnectionScope::new();
my_signal.connect_scoped(&scope, |value| {
    println!("Received: {:?}", value);
});
// When `scope` drops, all connections made through it are disconnected.
```

### 3. Prefer the Builder-Style AppConfig

```rust
let app = App::with_config(
    AppConfig::default()
        .with_app_name("ProductionApp")
        .with_organization("MyCompany")
        .with_i18n(true),
)
.on_startup(|| { setup_logging(); })
.on_shutdown(|| { save_state(); });
```

### 4. Separate UI Creation from Business Logic

```rust
struct AppState {
    counter: i32,
}

fn build_ui(window: &WindowHandle, state: &mut AppState) {
    let label = window.new_label("Count: 0", 10, 10, 200, 24);
    let button = window.new_button("Increment", 10, 40, 100, 32);

    button.on_click(move || {
        state.counter += 1;
        label.set_text(&format!("Count: {}", state.counter));
    });
}
```

### 5. Check Feature Availability with `#[cfg]`

```rust
#[cfg(not(feature = "mini"))]
fn create_rich_editor(parent: &WindowHandle) {
    // Use advanced widgets only available in non-mini profiles
    let _editor = parent.new_text_edit("", 10, 10, 400, 300);
}

#[cfg(feature = "mini")]
fn create_rich_editor(_parent: &WindowHandle) {
    // Fallback for mini profile
}
```

### 6. Follow the Coordinate System

rust-widgets uses **screen coordinates** with the origin at the **top-left**:
- X increases to the right
- Y increases downward

Widget positioning follows `Rect::new(x, y, width, height)` where `(x, y)` is
the top-left corner in pixels.

### 7. Clean Up Callbacks When Destroying Widgets

```rust
use rust_widgets::app::handle::remove_callbacks;

// When a widget is destroyed:
remove_callbacks(widget_id);
```

### 8. Initialize Before Creating Widgets

Always call `app.init()` before creating any widgets. This initializes the
platform backend, rendering pipeline, and i18n subsystem.

---

## Next Steps

Now that you have a working rust-widgets application, dive deeper:

- **Architecture Overview** — understand the layered architecture, crate
  hierarchy, and compile-time vs runtime design decisions
- **Core Types** — master `ObjectId`, `Color`, `Rect`, `Size`, `Point`, `Font`,
  and all the primitive building blocks
- **Widget System** — explore the full widget hierarchy, the `Widget` trait, and
  how to create custom widgets
