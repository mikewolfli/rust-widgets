# Advanced Topics

This chapter covers creating custom widgets, layout managers, paint backends,
control backends, advanced signal/slot patterns, data binding, custom themes,
CSS hot-reloading, performance profiling, testing, no_std builds, security
considerations, and contributing guidelines.

## Creating Custom Widgets

A custom widget implements three traits: `Widget`, `EventHandler`, and
optionally `Draw` for custom rendering.

### Implementing Widget + EventHandler + Draw

```rust
use rust_widgets::widget::{Widget, BaseWidget, WidgetKind, Draw};
use rust_widgets::core::{Rect, Size, Color, ObjectId, Point};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::style::WidgetStyle;

#[derive(Debug)]
struct Gauge {
    base: BaseWidget,
    value: f32,           // 0.0 – 1.0
    min: f32,
    max: f32,
    color: Color,
}

impl Gauge {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Custom("Gauge"), geometry),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            color: Color::BLUE,
        }
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
        self.request_redraw();
    }
}

impl EventHandler for Gauge {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::MousePress { pos, .. } = event {
            // Calculate value from click position
            let rect = self.geometry();
            let pct = ((pos.x - rect.x) as f32 / rect.width as f32).clamp(0.0, 1.0);
            self.set_value(self.min + (self.max - self.min) * pct);
        }
    }
}

impl Widget for Gauge {
    fn base(&self) -> &BaseWidget { &self.base }
    fn base_mut(&mut self) -> &mut BaseWidget { &mut self.base }
}

impl Draw for Gauge {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let rect = self.geometry();
        let pct = (self.value - self.min) / (self.max - self.min);

        // Background track
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height, Color::LIGHT_GRAY);

        // Filled portion
        let filled_width = (rect.width as f32 * pct) as u32;
        ctx.fill_rect(
            rect.x,
            rect.y,
            filled_width,
            rect.height,
            self.color,
        );

        // Border
        ctx.draw_rect(rect.x, rect.y, rect.width, rect.height, Color::DARK_GRAY, 1);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

// Usage
let gauge = Gauge::new(Rect::new(10, 10, 200, 24));
let _id = gauge.id();
println!("Custom widget kind: {:?}", gauge.kind());
```

### Widget with Signals

```rust
use rust_widgets::signal::{Signal1, GenericSignal};

struct Slider {
    base: BaseWidget,
    value: f32,
    pub value_changed: Signal1<f32>,
}

impl Draw for Slider {
    fn draw(&mut self, _ctx: &mut RenderContext) {
        // ... custom rendering ...
    }
}

impl EventHandler for Slider {
    fn handle_event(&mut self, event: &Event) {
        if let Event::MousePress { pos, .. } = event {
            let rect = self.geometry();
            let new_value = ((pos.x - rect.x) as f32 / rect.width as f32).clamp(0.0, 1.0);
            self.value = new_value;
            self.value_changed.emit(new_value);
        }
    }
}
```

## Custom Layout Managers

Implement the `Layout` trait to create custom layout algorithms:

```rust
use rust_widgets::core::{ObjectId, Rect, Point, Size};
use rust_widgets::layout::{Layout, LayoutContext, LayoutConstraints, SizePolicy};

struct CircularLayout {
    children: Vec<(ObjectId, u32)>,  // (id, stretch)
    radius: u32,
    center_x: u32,
    center_y: u32,
    start_angle: f32,
}

impl CircularLayout {
    pub fn new(radius: u32, cx: u32, cy: u32) -> Self {
        Self {
            children: Vec::new(),
            radius,
            center_x: cx,
            center_y: cy,
            start_angle: 0.0,
        }
    }
}

impl Layout for CircularLayout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32) {
        self.children.push((widget_id, stretch));
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|(id, _)| *id != widget_id);
    }

    fn update(&self, _rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let count = self.children.len() as u32;
        for (i, (id, _)) in self.children.iter().enumerate() {
            let angle = self.start_angle + (i as f32 / count as f32) * std::f32::consts::TAU;
            let x = self.center_x as f32 + self.radius as f32 * angle.cos();
            let y = self.center_y as f32 + self.radius as f32 * angle.sin();
            widgets(*id, Rect::new(x as i32, y as i32, 40, 40));
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|(id, _)| *id).collect()
    }

    fn clear(&mut self) {
        self.children.clear();
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// Usage
// let mut circular = CircularLayout::new(100, 200, 200);
// circular.add_widget(widget_a.id(), 1);
// circular.add_widget(widget_b.id(), 1);
// circular.update(container_rect, &mut |id, rect| { /* apply */ });
```

## Custom Paint Backends

Implement the `PaintBackend` trait to add custom rendering backends:

```rust
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::core::{Color, Size, Rect, Point};

struct CustomPaintBackend {
    width: u32,
    height: u32,
    scale: f32,
    buffer: Vec<u32>,  // Custom pixel format
}

impl CustomPaintBackend {
    pub fn new(size: Size, scale: f32) -> Self {
        let pixel_count = (size.width as usize) * (size.height as usize);
        Self {
            width: size.width,
            height: size.height,
            scale,
            buffer: vec![0xFFFFFFFF; pixel_count],
        }
    }
}

impl PaintBackend for CustomPaintBackend {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    fn scale(&self) -> f32 {
        self.scale
    }

    fn begin_frame(&mut self, _background: Color) {}

    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        // Convert color to u32 ABGR
        let pixel = (color.a as u32) << 24
            | (color.b as u32) << 16
            | (color.g as u32) << 8
            | color.r as u32;

        let y_start = y.max(0) as u32;
        let y_end = (y + h as i32).min(self.height as i32) as u32;
        let x_start = x.max(0) as u32;
        let x_end = (x + w as i32).min(self.width as i32) as u32;

        for row in y_start..y_end {
            let row_start = row as usize * self.width as usize;
            for col in x_start..x_end {
                self.buffer[row_start + col as usize] = pixel;
            }
        }
    }

    fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color, _width: u32) {
        let pixel = pixel_from_color(color);
        // Draw four edges ...
    }

    fn end_frame(&mut self) {
        // Flush buffer to actual display
        // platform::display_write(&self.buffer);
    }
}

fn pixel_from_color(c: Color) -> u32 {
    (c.a as u32) << 24 | (c.b as u32) << 16 | (c.g as u32) << 8 | c.r as u32
}

// Use through RenderContext:
// let mut backend = CustomPaintBackend::new(Size::new(800, 600), 1.0);
// backend.begin_frame(Color::WHITE);
// let mut ctx = RenderContext::new(&mut backend);
// ctx.fill_rect(0, 0, 100, 50, Color::RED);
// backend.end_frame();
```

## Control Backend Customization

The control backend manages widget dispatch policies and routing:

```rust
use rust_widgets::control_backend::Dispatcher;
use rust_widgets::core::ObjectId;

// Configure dispatch policy
struct CustomDispatchPolicy;

impl Dispatcher for CustomDispatchPolicy {
    fn should_dispatch_to_widget(&self, widget_id: ObjectId, action: &str) -> bool {
        // Custom routing logic
        !action.contains("internal")
    }

    fn route_action(&self, widget_id: ObjectId, action: &str) -> Option<ObjectId> {
        // Route actions to specific handlers
        None  // Default routing
    }
}
```

## Signal/Slot Advanced Patterns

### ConnectionScope — Automatic Disconnection

```rust
use rust_widgets::signal::{ConnectionScope, Signal, GenericSignal};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

// Connections scoped to owner lifetime
let signal = GenericSignal::new();
let hits = Arc::new(AtomicUsize::new(0));

{
    let owner = ConnectionScope::new();
    let h = Arc::clone(&hits);

    signal.connect_scoped(&owner, move || {
        h.fetch_add(1, Ordering::SeqCst);
    });

    signal.emit();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

// After owner is dropped, the connection is automatically disconnected
signal.emit();
assert_eq!(hits.load(Ordering::SeqCst), 1);  // Not incremented again
```

### Signal Once

```rust
let signal = GenericSignal::new();
let hits = Arc::new(AtomicUsize::new(0));

{
    let h = Arc::clone(&hits);
    signal.connect_once(move || {
        h.fetch_add(1, Ordering::SeqCst);
    });
}

signal.emit();  // Fires
signal.emit();  // No-op — disconnected after first emit
assert_eq!(hits.load(Ordering::SeqCst), 1);
```

### Re-entrant Safety

Signal emits are re-entrant safe — connecting new slots during emission does not
deadlock:

```rust
let signal = Signal::<u32>::new();
let emitted = Arc::new(AtomicUsize::new(0));

let e1 = Arc::clone(&emitted);
let e2 = Arc::clone(&emitted);
let s2 = signal.clone();

signal.connect(move |v| {
    e1.fetch_add(1, Ordering::SeqCst);
    if *v == 1 {
        // Connect another slot during emission (re-entrant)
        s2.connect(move |_| {
            e2.fetch_add(1, Ordering::SeqCst);
        });
    }
});

signal.emit(1);  // First emit — new slot connected
signal.emit(2);  // Second emit — both slots fire

assert_eq!(emitted.load(Ordering::SeqCst), 3);
```

### Signal with Typed Data

```rust
use rust_widgets::signal::Signal1;
use std::sync::Arc;

let signal = Signal1::<String>::new();

signal.connect(|msg: Arc<String>| {
    println!("Received: {}", msg);
});

signal.emit("Hello!".to_string());
```

## Two-Way Data Binding

rust-widgets provides a reactive data binding system for MVVM-style UIs:

```rust
use rust_widgets::data_binding::{Binding, FnListener, Computed};

// Single reactive value
let mut name = Binding::new("World".to_string());

name.subscribe("log", Box::new(FnListener::new(|key| {
    println!("[{}] changed!", key);
})));

name.set("Rust".to_string());
assert_eq!(name.get(), "Rust");

// Derived / computed values
let mut full_name = Binding::new("John Doe".to_string());
let mut greeting = Computed::new(|| {
    format!("Hello, {}!", full_name.get())
});

assert_eq!(greeting.get(), "Hello, John Doe!");
full_name.set("Jane Doe".to_string());
greeting.invalidate();
assert_eq!(greeting.get(), "Hello, Jane Doe!");
```

## Custom Chart Types

Implement the chart trait to create custom visualizations:

```rust
use rust_widgets::chart::{
    ChartContext, Chart, ChartData,
};

struct ScatterPoint {
    x: f64,
    y: f64,
    label: String,
}

struct ScatterChart {
    data: Vec<ScatterPoint>,
    show_grid: bool,
}

impl ScatterChart {
    pub fn new() -> Self {
        Self { data: Vec::new(), show_grid: true }
    }

    pub fn add_point(&mut self, x: f64, y: f64, label: &str) {
        self.data.push(ScatterPoint {
            x, y,
            label: label.to_string(),
        });
    }
}

impl Chart for ScatterChart {
    fn data(&self) -> ChartData { ChartData::default() }
    fn data_mut(&mut self) -> &mut ChartData { unimplemented!() }
    fn render(&self, ctx: &mut ChartContext) {
        // Access the render context for drawing
        // ctx.fill_rect(...);
        for point in &self.data {
            // Draw each point
        }
    }
}
```

## Custom Themes — Extending ThemeManager

```rust
use rust_widgets::theme::{ThemeManager, Theme, Colors, Fonts, Spacing, Borders, ThemeOverrides};
use rust_widgets::core::{Color, Font};
use std::collections::HashMap;

let mut theme_manager = ThemeManager::default();

// Access the current theme
let current = theme_manager.current_theme().unwrap();
println!("Current theme: {}", current.name);

// Create a custom theme
let mut custom_theme = Theme {
    name: "corporate-blue".to_string(),
    colors: Colors {
        background: Color::from_hex("#FFFFFF").unwrap(),
        foreground: Color::from_hex("#1A1A1A").unwrap(),
        primary: Color::from_hex("#0052CC").unwrap(),
        secondary: Color::from_hex("#6B778C").unwrap(),
        accent: Color::from_hex("#00B8D9").unwrap(),
        error: Color::from_hex("#DE350B").unwrap(),
        warning: Color::from_hex("#FF991F").unwrap(),
        success: Color::from_hex("#36B37E").unwrap(),
        disabled: Color::from_hex("#A5ADBA").unwrap(),
        info: Color::from_hex("#0065FF").unwrap(),
    },
    fonts: Fonts::default(),
    spacing: Spacing {
        small: 4, medium: 8, large: 16, extra_large: 32,
    },
    borders: Borders { width: 1, radius: 4, shadow: true },
    overrides: ThemeOverrides {
        styles: HashMap::new(),
    },
};

// Use the built-in dark theme
let dark = Theme::dark();
println!("Dark theme background: {:?}", dark.colors.background);

// ThemeStyleToken for per-class overrides
use rust_widgets::theme::ThemeStyleToken;
let button_override = ThemeStyleToken {
    background: Some(Color::from_hex("#0052CC").unwrap()),
    foreground: Some(Color::WHITE),
    border: None,
    border_width: None,
    radius: Some(6),
};
```

## CSS Hot-Reloading Workflow

```rust
use rust_widgets::widget::Widget;
use rust_widgets::core::Rect;
use rust_widgets::widget::Button;
use std::fs;
use std::time::{Duration, Instant};

fn css_hot_reload() {
    let mut button = Button::new("Styled Button".to_string(), Rect::new(0, 0, 200, 40));
    let css_path = "styles/button.css";
    let mut last_modified = fs::metadata(css_path)
        .and_then(|m| m.modified())
        .ok();

    loop {
        // Check for file changes
        if let Ok(metadata) = fs::metadata(css_path) {
            if let Ok(modified) = metadata.modified() {
                if Some(modified) != last_modified {
                    // Reload CSS
                    let css = fs::read_to_string(css_path).unwrap_or_default();
                    if !css.is_empty() {
                        let _ = button.apply_css(&css, Some("custom-button"));
                        println!("CSS hot-reloaded");
                    }
                    last_modified = Some(modified);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
        break;  // Example: single iteration
    }
}
```

## Performance Profiling with PerformanceMonitor

```rust
use rust_widgets::performance::PerformanceMonitor;
use std::time::Duration;

// Full-system profiling
let mut monitor = PerformanceMonitor::new();

// Frame-level profiling
monitor.begin_frame();

monitor.begin_section("layout");
// Layout work...
monitor.end_section();

monitor.measure("render", || {
    // Measured block
});

monitor.end_frame();

// Generate report
if monitor.is_enabled() {
    let report = monitor.report();
    println!("{}", report.to_string_summary());
    // Shows FPS, avg/min/max frame times, and section breakdowns
}

// Profiler for hotspot detection
let profiler = monitor.profiler();
if let Some(avg) = profiler.get_average_duration("render") {
    if avg > Duration::from_millis(8) {
        eprintln!("Warning: render pass average >8ms ({:?})", avg);
    }
}

// Reset for clean measurement
monitor.reset();
```

## Testing with TestHarness, WidgetTester, LayoutTester

### TestHarness

```rust
use rust_widgets::test::TestHarness;
use rust_widgets::core::{Size, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::Label;

let mut harness = TestHarness::new()
    .with_screen_size(Size::new(1024, 768));

// Send events
harness.send_mouse_click(100, 100, 0);  // x, y, button
harness.send_mouse_move(200, 200);
harness.send_key_press(65, 0);           // 'A'
harness.send_key_release(65, 0);

// Dispatch to a widget
let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));
let handled = harness.dispatch_to(&mut label);
println!("{} events dispatched", handled);
```

### WidgetTester

```rust
use rust_widgets::test::WidgetTester;
use rust_widgets::core::{Rect, Size, Point};
use rust_widgets::widget::Button;

let mut tester = WidgetTester::new(
    Button::new("Click Me".to_string(), Rect::new(0, 0, 100, 32))
);

// Chainable assertions
tester
    .assert_visible()
    .assert_enabled()
    .assert_geometry(Rect::new(0, 0, 100, 32))
    .assert_size(Size::new(100, 32))
    .assert_position(Point::new(0, 0));

// Simulate interactions
tester.click(50, 16);   // Click center of button
tester.move_mouse(10, 10);
tester.press_key(13);   // Enter key

// Check state after interaction
// assert_eq!(tester.widget().text(), "Clicked!");
```

### LayoutTester

```rust
use rust_widgets::test::LayoutTester;
use rust_widgets::core::Rect;

let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));

// Test layout function
let positions = vec![
    Rect::new(0, 0, 100, 50),
    Rect::new(100, 0, 100, 50),
    Rect::new(200, 0, 100, 50),
];

tester.assert_fits_in_container(&positions);   // All inside 400×300
tester.assert_no_overlap(&positions);           // No intersections

// Exact position matching
tester.test_layout(
    |container| {
        // Your layout function
        vec![Rect::new(0, 0, container.width / 2, container.height)]
    },
    &[Rect::new(0, 0, 200, 300)],
);
```

## Snapshot Testing for Visual Regression

```rust
use rust_widgets::test::{SnapshotManager, SnapshotConfig};

// Configure snapshot testing
let mut snapshots = SnapshotManager::new();

let config = SnapshotConfig {
    tolerance: 0.01,     // 1% per-pixel tolerance
    update: false,       // Set true to update baselines
    output_dir: "tests/snapshots/".to_string(),
};

// Compare rendered frame against baseline
fn test_widget_rendering() {
    let rendered_frame = render_widget_to_buffer();
    // snapshots.compare("button_default", &rendered_frame, &config);
}
```

## Feature Flag Matrix and Combinatorial Testing

rust-widgets uses a three-axis feature system:

| Axis | Example Flags | Description |
|------|--------------|-------------|
| Device Profile | `desktop`, `mini`, `embedded` | Feature set scope |
| OS Backend | `linux`, `windows`, `macos`, `wasm` | Platform backend |
| Capabilities | `gpu-wgpu`, `chart`, `pdf`, `i18n` | Optional modules |

**Testing combinations:**

```rust
// Test with different feature combinations
#[test]
fn test_chart_without_gpu() {
    // Only chart feature, no GPU
    // cargo test --features "chart"
}

#[test]
fn test_chart_with_gpu() {
    // Full chart + GPU
    // cargo test --features "chart,gpu-wgpu"
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_std_only_feature() {
    // This test only runs on desktop/embedded profiles, not mini
}
```

## Building for no_std / mini Profile

```toml
# Cargo.toml
[dependencies]
rust_widgets = { version = "0.9", default-features = false, features = [
    "mini",
] }

[profile.mini]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

The `mini` feature provides `no_std` compatibility:

```rust
// In no_std mode, HashMap → BTreeMap (via compat.rs)
// Mutex → RefCell
// Vec → MiniVec
// String → MiniString
// All trait implementations must be Send + Sync compatible

#![no_std]
extern crate alloc;

use rust_widgets::embedded::{
    EmbeddedConfig, ResourceManager, ResourceConstraint,
    LightweightWidget, LightweightConfig,
};
use rust_widgets::render::SoftwarePaintBackend;
use rust_widgets::core::{Size, Color};

fn mini_main() {
    let config = EmbeddedConfig::new(Size::new(320, 240))
        .low_memory();
    rust_widgets::embedded::init_embedded(config);

    let mut resources = ResourceManager::new(ResourceConstraint::Low);
    let mut backend = SoftwarePaintBackend::new(Size::new(320, 240), 1.0);

    loop {
        backend.begin_frame(Color::WHITE);
        // ... render minimal UI ...
        backend.end_frame();

        if resources.is_under_pressure() {
            // Emergency memory recovery
        }
    }
}
```

## Security Considerations

1. **JavaScript injection:** Always call `set_javascript_enabled(false)` on
   `WebViewEnhanced` / `WebEngineViewEnhanced` when displaying untrusted content.

2. **Mixed content:** Use `SecuritySettings` to block mixed HTTP/HTTPS content:
   ```rust
   view.security_mut().allow_insecure_content = false;
   view.security_mut().allow_mixed_content = false;
   ```

3. **Plugin permissions:** Restrict plugins using `PluginManager` permissions:
   ```rust
   manager.revoke_permission(plugin_id, PluginPermission::FileSystemAccess);
   manager.revoke_permission(plugin_id, PluginPermission::NetworkAccess);
   ```

4. **Cookie isolation:** Use `CookieJar` with strict domain scoping:
   ```rust
   let cookie = Cookie::new("session", token, "app.example.com");
   cookie.http_only = true;
   cookie.secure = true;
   ```

5. **Private browsing:** Enable for sensitive sessions:
   ```rust
   engine.set_private_browsing(true);
   // Clears cookies, history, cache on exit
   ```

6. **Tracking protection:** Block fingerprinting and tracking:
   ```rust
   let privacy = TrackingProtection::new(PrivacySettings::strict());
   ```

7. **Memory safety:** Use `ArenaAllocator` and `ObjectPool` to avoid heap
   fragmentation in security-critical paths.

8. **Feature reduction:** Disable unused features:
   ```toml
   default-features = false
   features = ["mini"]  # Minimal attack surface
   ```

## Contributing Guidelines

### Code Style

```rust
// Follow the project's formatting via rustfmt.toml
// $ cargo fmt --all

// Check with clippy
// $ cargo clippy --all-features --all-targets
```

### Project Rules

- **No empty `todo!()` placeholders** — every branch must implement actual logic
- **Validate all symbol pairs** (`{}`, `()`, `[]`, `<>`) before and after edits
- **No circular dependencies** between modules — use traits/interfaces to decouple
- **No unused imports or variables** — enable `#![deny(unused)]` in development
- **Edge cases handled** — `Option`, `Result`, bounds checks for all inputs
- **Error handling** — prefer `Result<T, E>` returns over panicking

### Testing Requirements

```rust
// Every module must have:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Core happy path
    }

    #[test]
    fn test_edge_cases() {
        // Empty input, zero values, overflow
    }

    #[test]
    fn test_error_handling() {
        // Invalid input, out-of-bounds, null states
    }
}
```

### Pull Request Process

1. Run `cargo test --all-features` and verify all tests pass
2. Run `cargo clippy --all-features --all-targets` and fix warnings
3. Run `cargo fmt --all --check`
4. Update any relevant documentation in the `cookbook/` directory
5. Add snapshot tests for visual changes

### Module Documentation Standard

Every public module must include:

```rust
//! Module-level doc with:
//! 1. Purpose statement (one paragraph)
//! 2. Architecture diagram or bullet-pointed feature list
//! 3. `# Examples` section with at least one code example
//! 4. `# Feature flags` section if gated
```

## Summary

| Topic | Key Components |
|-------|---------------|
| Custom widgets | `Widget` + `EventHandler` + `Draw` traits |
| Custom layouts | `Layout` trait with `update()`, `add_widget()` |
| Custom backends | `PaintBackend` trait (fill_rect, draw_rect, etc.) |
| Control backends | `Dispatcher` trait for dispatch policy |
| Signal/slot | `ConnectionScope`, `connect_once()`, re-entrant safety |
| Data binding | `Binding<T>`, `Computed<T>`, `FnListener` |
| Custom charts | `Chart` trait + `ChartContext` |
| Custom themes | `ThemeManager`, `Theme`, `ThemeStyleToken` |
| CSS hot-reload | `Widget::apply_css()` + file watcher |
| Profiling | `PerformanceMonitor`, `Profiler`, `FrameProfiler` |
| Testing | `TestHarness`, `WidgetTester`, `LayoutTester` |
| Snapshot testing | `SnapshotManager`, per-pixel tolerance |
| Feature matrix | 3 axes × multiple values = combinatorial testing |
| no_std build | `mini` profile, `MiniVec`, `MiniString` |
| Security | JS disable, mixed content, plugin permissions, cookie isolation |
