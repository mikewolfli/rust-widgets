# Embedded Support

rust-widgets supports embedded and resource-constrained targets through a
no_std-ready profile (`mini`, currently compiles on std) with `AtomicBool` global flags, fixed-DPI
mode, lightweight widget factories, hardware input handling, and adaptive
resource management.

## Architecture Overview

```
┌──────────────────────────────────────────────┐
│  AtomicBool globals: embedded_mode,           │
│  low_memory_mode, fixed_dpi                  │
├──────────────────────────────────────────────┤
│  EmbeddedConfig  →  ResourceManager          │
│  (screen, DPI,   →  (widget limits,          │
│   toggles)           memory limits)          │
├──────────────────────┬───────────────────────┤
│  HardwareInput       │  LightweightWidget    │
│  (TouchPoint,        │  (minimal heap,       │
│   gestures, filter)  │   reduced effects)    │
├──────────────────────┼───────────────────────┤
│  DPI Management      │  WidgetPool<T>        │
│  (fixed DPI, scale)  │  (object pool)        │
├──────────────────────┼───────────────────────┤
│  LightweightStyle    │  LightweightFactory   │
│  (compact defaults)  │  (rate-limited)       │
└──────────────────────┴───────────────────────┘
```

## Embedded Mode — AtomicBool Global Flags

Three global `AtomicBool` flags control the embedded subsystem without requiring
a global context object:

```rust
use rust_widgets::embedded;

// Check current mode
println!("Embedded: {}", embedded::is_embedded_mode());
println!("Low memory: {}", embedded::is_low_memory_mode());

// Enable embedded mode
embedded::set_embedded_mode(true);
assert!(embedded::is_embedded_mode());

// Enable low-memory mode
embedded::set_low_memory_mode(true);
assert!(embedded::is_low_memory_mode());

// Toggle back
embedded::set_embedded_mode(false);
embedded::set_low_memory_mode(false);
```

**Adaptive constants** adjust automatically based on mode:

```rust
use rust_widgets::embedded;
use rust_widgets::core::Size;

// Recommended buffer sizes
embedded::set_low_memory_mode(true);
let low_size = embedded::recommended_buffer_size();
assert_eq!(low_size, Size::new(800, 600));

embedded::set_low_memory_mode(false);
let normal_size = embedded::recommended_buffer_size();
assert_eq!(normal_size, Size::new(1920, 1080));

// Texture size limits
embedded::set_embedded_mode(true);
assert_eq!(embedded::max_texture_size(), 1024);  // Limited for embedded

embedded::set_embedded_mode(false);
assert_eq!(embedded::max_texture_size(), 4096);  // Desktop-range

// Font cache sizes
embedded::set_low_memory_mode(true);
assert_eq!(embedded::font_cache_size(), 256 * 1024);  // 256 KiB

embedded::set_low_memory_mode(false);
assert_eq!(embedded::font_cache_size(), 2 * 1024 * 1024);  // 2 MiB

// Event queue sizes
embedded::set_embedded_mode(true);
assert_eq!(embedded::event_queue_size(), 64);  // Limited

embedded::set_embedded_mode(false);
assert_eq!(embedded::event_queue_size(), 256);  // Standard
```

### init_embedded / init_desktop

Initialize the environment in a single call:

```rust
use rust_widgets::embedded::{init_embedded, init_desktop, EmbeddedConfig};
use rust_widgets::core::Size;

// Initialize for embedded target with fixed DPI
let config = EmbeddedConfig::new(Size::new(1024, 768))
    .with_fixed_dpi(96)
    .low_memory();
init_embedded(config);
assert!(embedded::is_embedded_mode());
assert!(embedded::is_low_memory_mode());

// Switch back to desktop
init_desktop();
assert!(!embedded::is_embedded_mode());
assert!(!embedded::is_low_memory_mode());
```

## EmbeddedConfig

```rust
use rust_widgets::embedded::EmbeddedConfig;
use rust_widgets::core::Size;

// Builder pattern
let config = EmbeddedConfig::new(Size::new(800, 600))
    .with_fixed_dpi(160)          // 160 DPI (2× density)
    .low_memory()                  // Enable low-memory optimizations
    .with_max_widgets(50)          // Cap at 50 widgets
    .with_touch(true)              // Touch input enabled
    .with_hardware_acceleration(false) // Software rendering
    .with_font_scale(1.2);         // 120% font size

println!("Screen: {}×{}", config.screen_size.width, config.screen_size.height);
println!("Fixed DPI: {:?}", config.fixed_dpi);
println!("Low memory: {}", config.low_memory_mode);
println!("Max widgets: {}", config.max_widgets);
println!("Animations: {}", config.enable_animations);
println!("Touch: {}", config.touch_enabled);
println!("Font scale: {}", config.font_scale);
```

**What `low_memory()` sets internally:**

| Setting | Default | After `.low_memory()` |
|---------|---------|----------------------|
| `max_widgets` | 100 | 50 |
| `max_texture_size` | 1024 | 512 |
| `enable_animations` | true | false |
| `enable_shadows` | false | false |
| `enable_gradients` | true | false |

## ResourceManager — Widget Count Limits & Memory Limits

```rust
use rust_widgets::embedded::{ResourceManager, ResourceConstraint};

// Constraint levels determine limits
let mut rm = ResourceManager::new(ResourceConstraint::Low);
// Low:   16 MiB memory, 50 widgets
// Medium: 64 MiB memory, 200 widgets
// High: 256 MiB memory, 1000 widgets
// None:  unlimited

// Memory allocation
assert!(rm.can_allocate(1024));
assert!(rm.allocate(1024));
assert_eq!(rm.memory_usage(), 1024);
assert_eq!(rm.memory_percentage(), (1024.0_f32 / (16.0 * 1024.0 * 1024.0)) * 100.0);

// Deallocate
rm.deallocate(512);
assert_eq!(rm.memory_usage(), 512);

// Widget tracking
assert!(rm.can_create_widget());
assert!(rm.register_widget());
assert_eq!(rm.widget_count(), 1);
rm.unregister_widget();
assert_eq!(rm.widget_count(), 0);

// Widget limits enforced
for _ in 0..50 {
    assert!(rm.register_widget());
}
assert!(!rm.register_widget());  // Exceeded max_widgets (50)
assert_eq!(rm.widget_count(), 50);

// Pressure detection
assert!(rm.is_under_pressure());  // 50/50 widgets = 100% > 90%
```

**Integration pattern:**

```rust
fn create_widget(rm: &mut ResourceManager, memory_needed: usize) -> Option<WidgetHandle> {
    if !rm.can_create_widget() {
        eprintln!("Widget limit reached ({}/{})", rm.widget_count(), rm.widget_count());
        return None;
    }
    if !rm.allocate(memory_needed) {
        eprintln!(
            "Memory limit exceeded ({:.1}% used)",
            rm.memory_percentage()
        );
        return None;
    }
    rm.register_widget();
    Some(WidgetHandle::new())
}

// On widget destruction
fn destroy_widget(rm: &mut ResourceManager, memory_freed: usize) {
    rm.deallocate(memory_freed);
    rm.unregister_widget();
}
```

## DPI Management

Fixed DPI mode uses global `AtomicU32` state for environments where the display
DPI never changes (embedded panels, fixed monitors):

```rust
use rust_widgets::embedded;

// Set fixed DPI
embedded::set_fixed_dpi(192);  // 2× density panel
assert!(embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), Some(192));

// Scale factor: DPI / 96
assert!((embedded::scale_factor() - 2.0).abs() < 0.01);

// Scale functions
assert_eq!(embedded::scale(100), 200);       // 100 × 2.0 = 200
assert_eq!(embedded::scale_u32(100), 200);   // u32 variant
assert!((embedded::scale_f32(50.0) - 100.0).abs() < 0.01);

// Point ↔ Pixel conversion (1pt = 1/72 inch)
let px = embedded::points_to_pixels(12.0, 96);
assert!((px - 16.0).abs() < 0.01);  // 12pt at 96dpi = 16px

let pt = embedded::pixels_to_points(16.0, 96);
assert!((pt - 12.0).abs() < 0.01);  // 16px at 96dpi = 12pt

// Clear fixed DPI — revert to system-detected DPI
embedded::clear_fixed_dpi();
assert!(!embedded::is_fixed_dpi());
assert_eq!(embedded::get_fixed_dpi(), None);
```

### DpiScaler

The `DpiScaler` struct provides a local, stack-allocated alternative to the
global fixed DPI:

```rust
use rust_widgets::embedded::DpiScaler;

let scaler = DpiScaler::new(144)              // 144 DPI (1.5×)
    .with_base_dpi(96);

assert!((scaler.scale_factor() - 1.5).abs() < 0.01);
assert_eq!(scaler.scale(100), 150);
assert_eq!(scaler.scale_u32(100), 150);
assert!((scaler.scale_f32(100.0) - 150.0).abs() < 0.01);

// Reverse scaling (pixels → logical)
assert_eq!(scaler.unscale(150), 100);
assert_eq!(scaler.unscale_u32(150), 100);
assert!((scaler.unscale_f32(150.0) - 100.0).abs() < 0.01);
```

## Hardware Input: Touch and Gestures

### TouchPoint

```rust
use rust_widgets::embedded::TouchPoint;

let point = TouchPoint::new(1, 100, 200)    // id=1, x=100, y=200
    .with_pressure(0.8);                     // 80% pressure

assert_eq!(point.id, 1);
assert_eq!(point.position.x, 100);
assert_eq!(point.position.y, 200);
assert!((point.pressure - 0.8).abs() < 0.01);
```

### HardwareInputManager

Handles multi-touch, 32 hardware buttons, and gesture detection:

```rust
use rust_widgets::embedded::{
    HardwareInputManager, TouchPoint, TouchEvent,
    GestureType, InputType,
};

let mut manager = HardwareInputManager::new();

// Process touch down
let point = TouchPoint::new(1, 100, 100);
manager.process_touch(TouchEvent::Down, point);
assert_eq!(manager.touch_point_count(), 1);

// Process touch move
let moved = TouchPoint::new(1, 120, 100);
manager.process_touch(TouchEvent::Move, moved);

// Process touch up — gesture is auto-detected
manager.process_touch(TouchEvent::Up, moved);

// Poll detected gestures
while let Some(gesture) = manager.get_gesture() {
    match gesture.gesture_type {
        GestureType::Tap => println!("Tap at ({}, {})", gesture.center.x, gesture.center.y),
        GestureType::SwipeRight => println!("Swipe right, velocity: {:?}", gesture.velocity),
        GestureType::LongPress => println!("Long press at ({}, {})", gesture.center.x, gesture.center.y),
        _ => println!("Gesture: {:?}", gesture.gesture_type),
    }
}

// Hardware buttons (up to 32)
manager.process_button(0, true);   // Button 0 pressed
assert!(manager.is_button_pressed(0));
manager.process_button(0, false);  // Released
assert!(!manager.is_button_pressed(0));

// Touch cancellation
manager.process_touch(TouchEvent::Cancel, point);
manager.clear();
```

**Gesture detection thresholds:**
- **Tap:** duration < 200ms, distance < 50px
- **Long press:** duration ≥ 500ms, distance < 50px
- **Swipe:** distance ≥ 50px — direction determined by dominant axis

### InputFilter

`InputFilter` provides pressure thresholding, dead-zone filtering, and position
smoothing:

```rust
use rust_widgets::embedded::{InputFilter, TouchPoint};

let mut filter = InputFilter::new()
    .with_dead_zone(10);  // 10px dead zone

// First touch passes through (no previous position)
let point1 = TouchPoint::new(1, 100, 100);
let result1 = filter.filter_touch(&point1);
assert!(result1.is_some());

// Slight movement within dead zone → filtered out
let point2 = TouchPoint::new(1, 105, 105);  // dx=5, dy=5 < 10
let result2 = filter.filter_touch(&point2);
assert!(result2.is_none());

// Significant movement beyond dead zone → smoothed
let point3 = TouchPoint::new(1, 150, 150);  // dx=50, dy=50
let result3 = filter.filter_touch(&point3);
assert!(result3.is_some());
// Position is smoothed: 100 + 0.5×(150-100) = 125

// Pressure threshold: touches below min_pressure are filtered
let weak = TouchPoint::new(2, 200, 200).with_pressure(0.05);
assert!(filter.filter_touch(&weak).is_none());  // Below min_pressure (0.1)

filter.reset();  // Clear state
```

## LightweightWidget — Resource-Constrained Rendering

```rust
use rust_widgets::embedded::{
    LightweightWidget, LightweightConfig, LightweightStyle,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

// Wrap any Widget in a lightweight shell
let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 30));
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());

// Access inner widget
println!("Inner widget kind: {:?}", lw.inner().kind());

// Unwrap
let label = lw.into_inner();
```

**LightweightConfig presets:**

```rust
// Flexible configuration
let config = LightweightConfig::new()
    .with_shadows_disabled()
    .with_animations_disabled()
    .with_gradients_disabled();

// Or use the minimal preset
let minimal = LightweightConfig::minimal();
assert!(minimal.disable_shadows);
assert!(minimal.disable_animations);
assert!(minimal.disable_gradients);
assert!(minimal.simple_borders);
assert!(minimal.reduced_padding);
assert!(minimal.minimal_signals);

// Create a widget with minimal config
let lw = LightweightWidget::new(label)
    .with_config(LightweightConfig::minimal());
```

### LightweightStyle — Minimal Heap Usage

```rust
use rust_widgets::embedded::LightweightStyle;

let style = LightweightStyle::new();
assert_eq!(style.padding, 4);
assert_eq!(style.font_size, 12);

let compact = LightweightStyle::compact();
assert_eq!(compact.padding, 2);
assert_eq!(compact.font_size, 10);
assert_eq!(compact.border_width, 1);
assert_eq!(compact.text_color, Some(0x000000));
assert_eq!(compact.border_color, Some(0x808080));
```

## LightweightWidgetFactory — Rate-Limited Widget Creation

```rust
use rust_widgets::embedded::{
    LightweightWidgetFactory, LightweightConfig,
};
use rust_widgets::widget::Label;
use rust_widgets::core::Rect;

let mut factory = LightweightWidgetFactory::new()
    .with_config(LightweightConfig::minimal())
    .with_max_widgets(5);

// Create widgets (returns None when limit exceeded)
for i in 0..10 {
    if let Some(widget) = factory.create(|| {
        Label::new(format!("Widget {}", i), Rect::new(0, 0, 100, 30))
    }) {
        println!("Created widget {}", i);
    } else {
        println!("Widget limit reached at {}", i);
    }
}

assert_eq!(factory.widget_count(), 5);

// Release a slot
factory.release();
assert_eq!(factory.widget_count(), 4);
assert!(factory.can_create());  // Room for one more now
```

## WidgetPool&lt;T&gt; — Object Pool for Widgets

```rust
use rust_widgets::embedded::WidgetPool;

let mut pool: WidgetPool<i32> = WidgetPool::new(3);  // Max 3 pooled items

// Acquire items (creates via factory closure)
let h1 = pool.acquire(|| 1);
assert!(h1.is_some());
assert_eq!(pool.used_count(), 1);
assert_eq!(pool.available_count(), 0);  // All pre-allocated slots used

let h2 = pool.acquire(|| 2);
assert!(h2.is_some());
assert_eq!(pool.used_count(), 2);

// Access items by handle
let h1_ref = h1.as_ref().unwrap();
assert_eq!(*pool.get(h1_ref.index()).unwrap(), 1);

// Drop handle → slot returned to pool
drop(h1);
assert_eq!(pool.used_count(), 1);

// The freed slot can be reused
let h3 = pool.acquire(|| 3);
assert!(h3.is_some());

// Pool full — further acquires return None
let h4 = pool.acquire(|| 4);
assert!(h4.is_none());
```

## Low-Memory Mode — Recommended Limits

When `low_memory_mode` is enabled, the framework automatically adjusts:

| Resource | Standard | Low-Memory |
|----------|----------|------------|
| Buffer size | 1920×1080 | 800×600 |
| Max texture size | 4096 | 1024 (512 with `.low_memory()`) |
| Font cache | 2 MiB | 256 KiB |
| Event queue | 256 | 64 |
| Max widgets (default) | 100 | 50 |
| Animations | Enabled | Disabled |
| Shadows | Disabled | Disabled |
| Gradients | Enabled | Disabled |

## Building for Embedded Targets

### Release-embedded Profile

Add to `Cargo.toml`:

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"           # Optimize for size
lto = true                # Link-time optimization
codegen-units = 1         # Single codegen unit for better LTO
strip = true              # Strip debug symbols
panic = "abort"           # No unwinding (smaller binary)
```

Build with the `mini` feature:

```sh
cargo build --profile release-embedded --no-default-features \
  --features "mini,embedded" --target thumbv7em-none-eabihf
```

### Feature Flag Configuration

```toml
[dependencies]
rust_widgets = { version = "1.0", default-features = false, features = [
    "mini",          # no_std-ready profile, heapless-backed MiniVec
    "embedded",      # embedded mode + lightweight widgets
] }
```

### Recommended `mini` Feature Usage

The `mini` feature replaces std types with arena-allocated and heapless
alternatives via `compat.rs`:
- `HashMap` → `BTreeMap`
- `Mutex` → `RefCell`
- `Vec` → `MiniVec`
- `String` → `MiniString`

## Complete Embedded Rendering Loop

A minimal embedded rendering loop combining all concepts:

```rust
use rust_widgets::embedded::{
    self, EmbeddedConfig, ResourceManager, ResourceConstraint,
    HardwareInputManager, TouchPoint, TouchEvent,
    LightweightWidgetFactory, LightweightConfig,
    InputFilter,
};
use rust_widgets::render::{SoftwarePaintBackend, PaintBackend, RenderContext};
use rust_widgets::core::{Size, Color, Rect, Point};
use rust_widgets::widget::Label;

fn embedded_main() {
    // 1. Initialize embedded mode
    let config = EmbeddedConfig::new(Size::new(800, 480))
        .with_fixed_dpi(120)
        .low_memory();
    embedded::init_embedded(config);

    // 2. Set up resource management
    let mut resources = ResourceManager::new(ResourceConstraint::Low);

    // 3. Input handling
    let mut input = HardwareInputManager::new();
    let mut filter = InputFilter::new().with_dead_zone(8);

    // 4. Widget factory
    let mut factory = LightweightWidgetFactory::new()
        .with_config(LightweightConfig::minimal())
        .with_max_widgets(20);

    // 5. Rendering backend
    let buf_size = embedded::recommended_buffer_size();
    let mut backend = SoftwarePaintBackend::new(buf_size, 1.0);

    // 6. Main loop
    loop {
        // --- Input ---
        // Read touch events from hardware (platform-specific)
        // Example: simulate a tap at (100, 200)
        let raw_point = TouchPoint::new(1, 100, 200);
        if let Some(filtered) = filter.filter_touch(&raw_point) {
            input.process_touch(TouchEvent::Down, filtered);

            let mapped_point = TouchPoint::new(1, 101, 201);
            input.process_touch(TouchEvent::Move, mapped_point);

            let end_point = TouchPoint::new(1, 105, 202);
            input.process_touch(TouchEvent::Up, end_point);
        }

        while let Some(gesture) = input.get_gesture() {
            // Handle gestures
            let _ = gesture;
        }

        // --- Layout & Create Widgets ---
        if factory.can_create() && resources.can_create_widget() {
            if resources.allocate(256) {
                let _widget = factory.create(|| {
                    Label::new(
                        "Embedded Label".to_string(),
                        Rect::new(10, 10, 200, 30),
                    )
                });
                resources.register_widget();
            }
        }

        // --- Render ---
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        // Draw widgets...
        backend.end_frame();

        // --- Resource Check ---
        if resources.is_under_pressure() {
            // Trim caches, drop non-essential widgets
            eprintln!("Memory: {:.1}%", resources.memory_percentage());
        }

        // Platform-specific: sleep until next frame
        // std::thread::sleep(Duration::from_millis(16));
        break;  // Exit after one iteration for this example
    }
}
```

## Summary

| Component | Purpose |
|-----------|---------|
| `embedded::set_embedded_mode()` | Global flag for embedded optimization |
| `embedded::set_low_memory_mode()` | Global flag for low-memory constraints |
| `EmbeddedConfig` | Screen size, fixed DPI, feature toggles |
| `ResourceManager` | Widget count limits, memory allocation tracking, pressure detection |
| `DpiScaler` / DPI functions | Fixed DPI management, scale factors, point conversions |
| `TouchPoint` | Multi-touch point with pressure and size |
| `HardwareInputManager` | Touch processing, gesture detection, 32 buttons |
| `InputFilter` | Pressure thresholding, dead zone, smoothing |
| `LightweightWidget<W>` | Wraps any Widget with resource-constrained config |
| `LightweightWidgetFactory` | Rate-limited widget creation with max count |
| `WidgetPool<T>` | Object pool with handle-based acquire/release |
| `LightweightStyle` | Minimal heap style with compact defaults |
| `LightweightConfig` | Feature toggles for embedded (shadows, animations, etc.) |
| `init_embedded()` / `init_desktop()` | One-shot environment initialization |
| `mini` feature | no_std-ready profile: core/alloc imports via compat.rs, heapless-backed MiniVec |
