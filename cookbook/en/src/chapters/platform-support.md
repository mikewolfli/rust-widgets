# Platform Support

rust-widgets provides a unified API across eight supported platforms. This chapter
covers the platform abstraction layer, backend selection, device detection,
clipboard, drag & drop, IME, accessibility, menus, capability negotiation, and
virtual keyboard support.

---

## 1. Eight Supported Platforms

| # | Platform | Backend(s) | Feature Flag | Status |
|---|----------|-----------|:---:|:---:|
| 1 | **Linux (GTK)** | Native GTK3 windowing | `gtk-native` | ✅ Native windows |
| 2 | **Linux (Wayland)** | Native Wayland protocol | `wayland-native` | ✅ Auto-detect session |
| 3 | **Windows** | Win32 API | *(always on)* | ✅ Native |
| 4 | **macOS** | Cocoa / objc2 bridge | `objc2-macos` | ✅ Native |
| 5 | **iOS** | UIKit state-backed | `ios` | ✅ State-driven |
| 6 | **Android** | JNI bridge | `android-jni` | ✅ JNI bridge |
| 7 | **WASM** | WebAssembly canvas | `wasm` | ✅ Browser |
| 8 | **HarmonyOS** | NAPI bridge | `harmony` | ✅ Preview |
| 9 | **Embedded** | Stub / no_std | `embedded` / `mini` | ✅ no_std |

On Linux, the runtime auto-detects between Wayland and X11/GTK via the
`$WAYLAND_DISPLAY` and `$XDG_SESSION_TYPE` environment variables.

---

## 2. The `Platform` Trait — Universal Contract

The `Platform` trait defines ~70 methods across 26 widget creation functions.
Every backend implements this trait, ensuring identical API surface across
platforms.

```rust
use rust_widgets::platform::{Platform, PlatformCapabilities};

fn inspect_backend(platform: &dyn Platform) {
    println!("Backend: {}", platform.backend_name());
    println!("Family:  {:?}", platform.family());

    let caps: PlatformCapabilities = platform.capabilities();
    println!("DPI scaling:    {}", caps.dpi_scaling);
    println!("IME:            {}", caps.ime);
    println!("Accessibility:  {}", caps.accessibility);
    println!("Native menus:   {}", caps.native_menu);
}
```

### Widget Creation Methods (subset)

| Method | Widget | Signature |
|--------|--------|-----------|
| `create_window` | Window | `(title, x, y, w, h) -> ObjectId` |
| `create_button` | Button | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_checkbox` | CheckBox | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_line_edit` | LineEdit | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_label` | Label | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_radio_button` | RadioButton | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_slider` | Slider | `(parent, x, y, w, h) -> ObjectId` |
| `create_progress_bar` | ProgressBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_combo_box` | ComboBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_box` | ListBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_panel` | Panel | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu_bar` | MenuBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu` | Menu | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_tool_bar` | ToolBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_status_bar` | StatusBar | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_message_box` | MessageBox | `(parent, title, text, x, y, w, h) -> ObjectId` |
| `create_file_dialog` | FileDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_color_dialog` | ColorDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_font_dialog` | FontDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_spin_box` | SpinBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_view` | ListView | `(parent, x, y, w, h) -> ObjectId` |
| `create_scroll_area` | ScrollArea | `(parent, x, y, w, h) -> ObjectId` |

Common widget mutation methods: `show_widget`, `hide_widget`, `set_widget_geometry`,
`set_widget_text`, `get_widget_text`, `set_widget_enabled`, `is_widget_enabled`,
`set_widget_visible`, `is_widget_visible`, `set_widget_ime_enabled`,
`is_widget_ime_enabled`, `set_widget_accessibility_name`, `get_widget_accessibility_name`.

---

## 3. `BackendState<K>` — Thread-Safe HashMap State Store

`BackendState<K>` is a thread-safe, serde-serializable state store used by
state-driven backends (Android, iOS, WASM, Harmony, Embedded). It stores
widget records, menu events, widget trigger events, clipboard text, and
drag-and-drop events behind `Mutex` guards.

```rust
use rust_widgets::platform::state::BackendState;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MyKind { Button, Label }

let state = BackendState::<MyKind>::new();

// Create a widget; returns an auto-incrementing ObjectId
let id = state.create_widget(MyKind::Button, "Click Me", 0, 0, 120, 32);

// Query widget state
assert!(state.contains_widget(id));
assert_eq!(state.kind_of(id), Some(MyKind::Button));
assert!(state.is_kind(id, MyKind::Button));
assert_eq!(state.text(id), "Click Me");

// Mutate widget state
state.set_visible(id, false);
state.set_geometry(id, 10, 20, 200, 40);
state.set_text(id, "Updated");
state.set_enabled(id, false);
state.set_ime_enabled(id, true);
state.set_accessibility_name(id, "Submit button");
```

### Event Queues

`BackendState` maintains internal queues for menu, widget trigger, clipboard, and
drag-drop events:

```rust
// Menu events
state.push_menu_event(item_id);
while let Some(id) = state.pop_menu_event() {
    println!("Menu item {} triggered", id);
}

// Typed widget trigger events
state.inject_widget_trigger_event(widget_id, WidgetTriggerKind::Clicked);
while let Some(event) = state.pop_widget_trigger_event() {
    match event.kind {
        WidgetTriggerKind::Clicked => { /* handle click */ }
        WidgetTriggerKind::ValueChanged => { /* handle change */ }
        _ => {}
    }
}

// Clipboard
state.set_clipboard_text("Hello clipboard");
let text = state.clipboard_text();
```

---

## 4. Runtime Backend Selection

Backend selection happens at compile time and auto-detection at runtime:

### Compile-Time Selection

```rust
// src/platform/runtime.rs — conditional compilation per target

#[cfg(all(target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

#[cfg(all(target_os = "macos", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())  // Dispatches to objc2 or cocoa
}

#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "wayland-native"))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}
```

### Global Singleton

The platform backend is stored in a `OnceLock` singleton, initialized on first access:

```rust
use rust_widgets::platform;

// Initialize, run, quit
platform::init();
platform::run();
platform::quit();

// Query capabilities
let caps = platform::capabilities();

// Get DPI scale factor
let dpi = platform::dpi_scale_factor();

// Check runtime GUI mode
match platform::runtime_gui_mode() {
    RuntimeGuiMode::NativeInteractive => println!("Running with native windows"),
    RuntimeGuiMode::PreviewOrStub => println!("Running in preview/stub mode"),
}
```

---

## 5. Device Environment Detection

`DeviceEnvironment` provides runtime detection of device class, touch capability,
screen size, DPI, orientation, and accessibility preferences.

```rust
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{DeviceClass, Size};

// Auto-detect from screen dimensions and DPI
let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);

println!("Device class:  {:?}", env.device_class);  // Desktop
println!("Touch capable: {}", env.touch_capable);
println!("Orientation:   {:?}", env.orientation);
println!("DPI scale:     {:.1}", env.dpi_scale);

// Touch target recommendations (logical pixels)
let target = env.min_touch_target();  // Desktop: 32×32, Tablet: 44×44, Mobile: 48×48
println!("Min touch target: {}×{}", target.width, target.height);
println!("Touch spacing:    {}", env.touch_spacing());

// Layout scale (projection mode adds 20%)
println!("Layout scale: {:.1}", env.layout_scale());

// Detect device class by screen size heuristic (no feature flags):
//   width < 480     → Mobile
//   width < 1024    → Tablet
//   DPI ≥ 2.0, <1440 → Tablet
//   otherwise       → Desktop
```

### Device Class Enum

| Class | Touch Target | Touch Spacing | Typical Use |
|-------|:---:|:---:|-------------|
| `Desktop` | 32×32 | 8px | Mouse + keyboard |
| `Tablet` | 44×44 | 12px | Touch-first large screen |
| `Mobile` | 48×48 | 16px | One-handed touch |
| `Embedded` | 40×40 | 10px | Dedicated hardware |
| `Projector` | 24×24 | 6px | Remote control navigation |

### Accessibility Preferences

```rust
let mut env = DeviceEnvironment::default();
env.set_high_contrast(true);
env.set_reduced_motion(true);
env.set_font_scale(1.5);  // Clamped to [0.5, 3.0]
```

---

## 6. Clipboard System

### `RichClipboardBackend` Trait

Each platform can implement rich clipboard support for text, HTML, RTF, images,
and file lists:

```rust
use rust_widgets::platform::clipboard::{
    RichClipboardBackend, ClipboardContent, MockClipboard,
};

// Use MockClipboard for testing
let clip = MockClipboard::new();

// Set plain text
clip.set_contents(ClipboardContent::Text("Hello".into()));

// Set HTML with plain-text fallback
clip.set_contents(ClipboardContent::Html {
    html: "<b>bold</b>".into(),
    plain: "bold".into(),
});

// Check format support
assert!(clip.has_format("text/plain"));
assert!(!clip.has_format("text/html"));

// Retrieve content
if let Some(content) = clip.get_contents() {
    match content {
        ClipboardContent::Text(t) => println!("Text: {}", t),
        ClipboardContent::Html { html, plain } => println!("HTML: {}, Plain: {}", html, plain),
        ClipboardContent::Rtf(data) => println!("RTF: {} bytes", data.len()),
        ClipboardContent::Image { width, height, .. } => println!("Image: {}×{}", width, height),
        ClipboardContent::Files(paths) => println!("Files: {:?}", paths),
    }
}
```

### Platform Clipboard Integration

The `Platform` trait exposes `clipboard_backend()` which returns
`Option<&dyn RichClipboardBackend>`. Desktop platforms provide real clipboard
integration; embedded platforms return `None`.

```rust
let platform = rust_widgets::platform::get_platform();

// Plain text via Platform trait
platform.set_clipboard_text("Copied text");
let text = platform.get_clipboard_text();

// Rich content via backend
if let Some(backend) = platform.clipboard_backend() {
    backend.set_clipboard_html("<h1>Title</h1>", "Title");
    backend.set_clipboard_image(&rgba_data, 64, 64);
}
```

---

## 7. Drag & Drop

```rust
use rust_widgets::platform::types::DropEvent;

// Begin a drag operation from a source widget
platform.begin_drag(source_id, "text/plain", b"Dragged text");

// Poll for drop events
while let Some(event) = platform.poll_drop_event() {
    println!("Source:  {}", event.source_widget_id);
    println!("Target:  {}", event.target_widget_id);
    println!("MIME:    {}", event.mime);
    println!("Payload: {} bytes", event.payload.len());
}

// Programmatic injection (for testing)
platform.inject_drop_event(DropEvent {
    source_widget_id: 1,
    target_widget_id: 2,
    mime: "text/plain".into(),
    payload: b"test".to_vec(),
});
```

`BackendState` provides the same operations:

```rust
state.begin_drag(src_id, "text/plain", payload);
if let Some(event) = state.pop_drop_event() {
    // Handle drop
}
state.inject_drop_event(event);
```

---

## 8. IME System

The IME bridge provides input method editor integration for East Asian
language input.

### `ImeBridge` Trait

```rust
use rust_widgets::platform::ime::{
    ImeBridge, ImeComposition, ImeCandidatePosition, MockImeBridge,
};

let bridge = MockImeBridge::new();

// Widget gains input focus
bridge.focus_in(text_edit_id);

// Update composition preview (pre-edit text)
bridge.set_composition(&ImeComposition {
    text: "nihao".into(),
    cursor_position: 5,
    selection_length: 0,
});

// Commit finalized text
bridge.commit_text("你好");

// Position the candidate window
bridge.set_candidate_window_position(ImeCandidatePosition { x: 100, y: 200 });

// Widget loses focus
bridge.focus_out(text_edit_id);

assert_eq!(bridge.focused_widget(), None);
```

### Platform IME Backends

| Platform | Implementation | Module |
|----------|---------------|--------|
| Linux | IBus integration | `platform::ime_linux` |
| macOS | `NSTextInputContext` | `platform::ime_macos` |
| Windows | TSF (Text Services Framework) | `platform::ime_windows` |

The `Platform` trait exposes `ime_bridge() -> Option<&dyn ImeBridge>`:

```rust
let platform = rust_widgets::platform::get_platform();
if let Some(bridge) = platform.ime_bridge() {
    if bridge.is_active() {
        bridge.focus_in(widget_id);
    }
}
```

---

## 9. Accessibility

### `A11yTree` — Cross-Platform Accessibility Node Tree

The accessibility system tracks 28 semantic roles and supports screen reader
navigation.

```rust
use rust_widgets::platform::accessibility::{
    A11yTree, A11yNode, A11yState, A11yRole, A11yProvider,
};

let mut tree = A11yTree::new();

// Register a widget node
let node = A11yNode::new(
    42,
    A11yState {
        role: A11yRole::Button,
        label: "Submit".into(),
        enabled: true,
        ..Default::default()
    },
);
tree.register_node(node);

// Query by role
let buttons = tree.find_by_role(A11yRole::Button);
for id in &buttons {
    if let Some(node) = tree.get(*id) {
        println!("Found button: {}", node.state.label);
    }
}

// Focus navigation
tree.focus_next();
tree.focus_previous();

// Dynamic queries
let query_results = tree.query(|node| {
    node.state.role == A11yRole::Button && node.state.enabled
});
```

### A11yRole Enum (28 roles)

`Unknown` • `Button` • `Label` • `TextField` • `CheckBox` • `RadioButton` •
`Slider` • `ProgressBar` • `List` • `Table` • `Image` • `Link` • `Heading` •
`Paragraph` • `Group` • `Window` • `Dialog` • `Menu` • `MenuItem` • `Tab` •
`Switch` • `Alert` • `ComboBox` • `SpinButton` • `StatusBar` • `ToolTip` • `Tree`

Roles automatically map to platform-specific roles: `NSAccessibilityRole` (macOS),
UIA control types (Windows), and AT-SPI roles (Linux).

### `A11yProvider` Trait

```rust
pub trait A11yProvider {
    fn register_widget(&mut self, id: ObjectId, role: A11yRole, label: &str);
    fn unregister_widget(&mut self, id: ObjectId);
    fn update_widget_state(&mut self, id: ObjectId, state: A11yState);
    fn announce(&self, message: &str);
    fn focus_next(&mut self) -> Option<ObjectId>;
    fn focus_previous(&mut self) -> Option<ObjectId>;
    fn tree(&self) -> &A11yTree;
    fn tree_mut(&mut self) -> &mut A11yTree;
}
```

### `AccessibilityBridge` Trait (Platform-Level)

```rust
pub trait AccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str);
    fn accessibility_name(&self, id: ObjectId) -> String;
    fn notify_name_changed(&self, id: ObjectId);
    fn notify_value_changed(&self, id: ObjectId);
    fn notify_state_changed(&self, id: ObjectId);
    fn notify_focus_changed(&self, id: ObjectId);
    fn set_aria_properties(&self, id: ObjectId, properties: AriaProperties);
}
```

Wiring focus management to accessibility:

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// Focus changes are now forwarded to the platform accessibility bridge
```

### Platform Accessibility Modules

| Platform | Module | Bridge |
|----------|--------|--------|
| macOS | `platform::accessibility::macos` | NSAccessibility |
| Windows | `platform::accessibility::windows` | UIAutomation |
| Linux | `platform::accessibility::linux` | AT-SPI (via zbus) |

---

## 10. Menu System

```rust
use rust_widgets::platform::get_platform;

let platform = get_platform();

// Create a menu bar attached to a window
let menu_bar = platform.create_menu_bar(window_id, 0, 0, 800, 24);
platform.attach_menu_bar_to_window(window_id, menu_bar);

// Create sub-menus
let file_menu = platform.create_menu(menu_bar, "File", 0, 0, 60, 24);

// Add menu items
let new_id = platform.menu_add_item(file_menu, "New", Some("Ctrl+N"));
let open_id = platform.menu_add_item(file_menu, "Open...", Some("Ctrl+O"));
platform.menu_add_item(file_menu, "Save", Some("Ctrl+S"));

// Poll for menu triggers
while let Some(triggered_id) = platform.poll_menu_triggered() {
    if triggered_id == new_id {
        println!("New file");
    } else if triggered_id == open_id {
        println!("Open file");
    }
}

// Programmatic injection (for testing)
platform.inject_menu_trigger(new_id);

// Poll typed widget triggers
while let Some(trigger) = platform.poll_widget_trigger_event() {
    match trigger.kind {
        WidgetTriggerKind::Clicked => { /* handle click */ }
        WidgetTriggerKind::ValueChanged => { /* handle value change */ }
        WidgetTriggerKind::SelectionChanged => { /* handle selection */ }
        WidgetTriggerKind::Closed => { /* handle close */ }
        WidgetTriggerKind::Unknown => { /* fallback */ }
    }
}
```

### `WidgetTriggerKind` Enum

| Variant | Value | Description |
|---------|:---:|-------------|
| `Unknown` | 0 | No concrete trigger semantic |
| `Clicked` | 1 | Primary activation (button click, checkbox toggle) |
| `ValueChanged` | 2 | Stateful value changed (line edit, slider) |
| `SelectionChanged` | 3 | Current selection updated (combo/list/tree/table) |
| `Closed` | 4 | Widget/window closed lifecycle trigger |

---

## 11. Capability Negotiation

The `CapabilityContract` system negotiates runtime capabilities between native
desktop profiles and constrained embedded profiles.

### `PlatformCapabilities` Flags

```rust
pub struct PlatformCapabilities {
    pub dpi_scaling: bool,           // High-DPI support
    pub ime: bool,                   // IME integration
    pub accessibility: bool,         // Accessibility bridge
    pub native_menu: bool,           // Native menu support
    pub typed_widget_trigger: bool,  // Typed widget events
}
```

### `NativeCapabilityContract`

Used by desktop runtimes (Windows, macOS, Linux):

| Field | Description |
|-------|-------------|
| `dpi_scaling` | DPI-aware geometry and text |
| `ime` | Input method editor support |
| `accessibility` | Screen reader bridge |
| `native_menu` | Platform-native menu bar |
| `typed_widget_trigger` | Typed trigger events |

### `EmbeddedCapabilityContract`

Used by embedded/constrained runtimes:

| Field | Description |
|-------|-------------|
| `fixed_dpi` | Fixed DPI scale factor (1.0) |
| `low_memory_mode` | Low-memory behavior expected |
| `typed_widget_trigger` | Typed trigger events |

### Negotiation

```rust
use rust_widgets::platform::{negotiate_capability_contract, CapabilityContract};
use rust_widgets::core::RuntimeProfile;

let contract = negotiate_capability_contract(RuntimeProfile::Full);
match contract {
    CapabilityContract::Native(native) => {
        println!("DPI scaling:   {}", native.dpi_scaling);
        println!("IME:           {}", native.ime);
        println!("Accessibility: {}", native.accessibility);
        println!("Native menus:  {}", native.native_menu);
    }
    CapabilityContract::Embedded(embedded) => {
        println!("Fixed DPI:       {}", embedded.fixed_dpi);
        println!("Low memory mode: {}", embedded.low_memory_mode);
    }
}
```

Fallback contracts are provided when the platform backend does not publish a
contract — ensuring deterministic behavior in all environments.

---

## 12. Virtual Keyboard (Mobile)

The `VirtualKeyboard` controller manages on-screen keyboard lifecycle and
layout adaptation for touch-based text input.

```rust
use rust_widgets::platform::virtual_keyboard::{
    VirtualKeyboard, KeyboardNotch, KeyboardState,
};
use rust_widgets::core::Rect;

let mut vkb = VirtualKeyboard::new();

// Request keyboard for a focused text field
vkb.request_show(
    text_field_id,
    Rect::new(0, 700, 200, 40),  // Widget rect in screen coords
    800,                           // Screen height
    KeyboardNotch::new(300),       // Keyboard overlay height
);

// Check state
assert_eq!(vkb.state(), KeyboardState::Showing);
assert!(vkb.is_keyboard_active());

// Transition to visible
vkb.on_shown();

// Apply layout shift to keep the widget visible
let mut widget_rect = Rect::new(10, 200, 100, 30);
vkb.apply_layout_shift(&mut widget_rect);
// widget_rect.y is now shifted upward if it would be covered

// Hide keyboard
vkb.request_hide();
vkb.on_hidden();
assert_eq!(vkb.state(), KeyboardState::Hidden);

// Reset all state (e.g., on window deactivation)
vkb.reset();
```

### State Machine

```
Hidden → (request_show) → Showing → (on_shown) → Visible
                                                      ↓
Hidden ← (on_hidden) ← Hiding ← (request_hide) ←─────┘
```

---

## 13. Platform-Specific Backends Overview

### Linux

```rust
// Auto-detection of Wayland vs X11/GTK
#[cfg(all(target_os = "linux", feature = "wayland-native"))]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}
```

### macOS (objc2 Bridge)

The `macos_objc2` module provides a modern Objective-C bridge. The
`SelectedMacOSPlatform` dispatches to the appropriate backend based on
feature flags.

### Windows

`WindowsPlatform` provides full Win32 API integration with native windowing,
clipboard, drag-and-drop, and accessibility via UIAutomation.

### Mobile (iOS / Android)

State-driven backends (`IosMobilePlatform`, Android JNI bridge) use
`BackendState<K>` for widget management. The Android JNI bridge exposes
native methods for view creation.

```rust
#[cfg(feature = "mobile-api")]
rust_widgets::platform::mobile_attach_to_native_view(native_handle);
let name = rust_widgets::platform::mobile_backend_name();
```

### WASM / Embedded

Both use `BackendState`-based state management. Embedded targets support
`no_std` via the `mini` feature flag with arena-allocated collections.

---

## 14. Cross-Platform Patterns

### Feature-Gated Platform Code

```rust
#[cfg(target_os = "linux")]
fn platform_specific_setup() { /* GTK init */ }

#[cfg(target_os = "macos")]
fn platform_specific_setup() { /* NSApplication init */ }

#[cfg(target_os = "windows")]
fn platform_specific_setup() { /* CoInitialize */ }
```

### Query Backend Identity at Runtime

```rust
let platform = rust_widgets::platform::get_platform();

match platform.backend_name() {
    "cocoa" | "WindowsPlatform" => {
        // Desktop native mode
    }
    "wayland" => {
        // Wayland native mode
    }
    "gtk" => {
        // GTK native mode
    }
    "harmony-desktop" | "android-mobile" | "macos-objc2-preview" => {
        // Preview/stub mode
    }
    _ => {
        // Unknown — preview mode
    }
}
```

### Wire Accessibility to Focus Manager

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// All focus changes are now relayed to the platform accessibility bridge
```

### Complete Cross-Platform Initialization

```rust
use rust_widgets::platform;
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{Size, RuntimeProfile};

fn main() {
    let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
    println!("Running on {:?} device", env.device_class);

    platform::init();

    let caps = platform::capabilities();
    if caps.ime {
        println!("IME support: enabled");
    }

    if let Some(bridge) = platform::get_platform().accessibility_bridge() {
        println!("Accessibility bridge: available");
    }

    let contract = negotiate_capability_contract(RuntimeProfile::Full);
    println!("Capability contract: {:?}", contract);

    // ... create windows, widgets ...

    platform::run();
}
```
