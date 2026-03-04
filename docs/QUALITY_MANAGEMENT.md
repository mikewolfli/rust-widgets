# Adaptive Rendering Quality Management

## Overview

The adaptive rendering quality system provides automatic performance optimization for GUI applications by dynamically adjusting rendering quality based on GPU capabilities and runtime performance. This ensures smooth user experience across different hardware configurations.

## Features

- **Automatic Quality Adjustment**: Monitors frame times and adjusts quality levels dynamically
- **GPU Capability Detection**: Evaluates hardware capabilities at initialization
- **Configurable Thresholds**: Customize degradation and upgrade behavior
- **Hysteresis Control**: Prevents quality level oscillation
- **Three Quality Levels**: High, Medium, and Low quality presets

## Architecture

### Quality Levels

```rust
pub enum QualityLevel {
    High,   // Full effects: anti-aliasing, shadows, complex shaders
    Medium, // Basic effects: simple shaders, no shadows
    Low,    // Minimal rendering: solid fills, no textures
}
```

### Components

1. **QualityLevel**: Enum defining three quality tiers
2. **QualityConfig**: Configuration for quality adjustment behavior
3. **GpuCapability**: Hardware capability detection
4. **FrameTimeMonitor**: Performance monitoring
5. **QualityManager**: Main controller for quality management

## Usage

### Basic Setup

```rust
use rust_widgets::quality::{QualityManager, QualityConfig};

// Create quality manager with default configuration
let mut manager = QualityManager::new();

// Or with custom configuration
let config = QualityConfig {
    target_frame_rate: 60.0,
    degrade_threshold: 1.5,
    upgrade_threshold: 0.7,
    max_quality: QualityLevel::High,
    min_quality: QualityLevel::Low,
    degrade_frame_count: 5,
    upgrade_frame_count: 10,
};
let mut manager = QualityManager::with_config(config);
```

### Integration with Render Loop

```rust
use rust_widgets::quality::{QualityManager, QualityLevel};
use std::time::Instant;

fn render_loop(backend: &mut dyn PaintBackend, manager: &mut QualityManager) {
    loop {
        let frame_start = Instant::now();

        // Query current quality level
        let quality = manager.quality_level();

        // Render based on quality level
        match quality {
            QualityLevel::High => {
                // Full effects: anti-aliasing, shadows, complex shaders
                render_with_full_effects(backend);
            }
            QualityLevel::Medium => {
                // Basic effects: simple shaders, no shadows
                render_with_basic_effects(backend);
            }
            QualityLevel::Low => {
                // Minimal rendering: solid fills, no textures
                render_minimal(backend);
            }
        }

        // Record frame duration and update quality
        let frame_duration = frame_start.elapsed();
        manager.finish_frame(frame_duration);
    }
}
```

### Widget-Level Adaptation

Widgets can query the current quality level and adjust their rendering:

```rust
impl Widget for Button {
    fn draw(&self, backend: &mut dyn PaintBackend) {
        let quality = backend.quality_level();

        match quality {
            QualityLevel::High => {
                // Draw with shadow and anti-aliasing
                backend.draw_rect_with_shadow(self.bounds, self.bg_color, 5.0);
                backend.draw_text_with_antialias(self.text_pos, &self.text, &self.font, self.text_color);
            }
            QualityLevel::Medium => {
                // Draw with border but no shadow
                backend.draw_rect(self.bounds, self.bg_color, self.border);
                backend.draw_text(self.text_pos, &self.text, &self.font, self.text_color);
            }
            QualityLevel::Low => {
                // Simple draw without border
                backend.draw_rect(self.bounds, self.bg_color, None);
                backend.draw_text_simple(self.text_pos, &self.text, &self.font, self.text_color);
            }
        }
    }
}
```

## Configuration

### QualityConfig Parameters

- **target_frame_rate**: Target FPS (default: 60.0)
- **degrade_threshold**: Multiplier for degrading quality (default: 1.5)
- **upgrade_threshold**: Multiplier for upgrading quality (default: 0.7)
- **max_quality**: Maximum allowed quality level
- **min_quality**: Minimum allowed quality level
- **degrade_frame_count**: Consecutive frames to trigger degradation (default: 5)
- **upgrade_frame_count**: Consecutive frames to trigger upgrade (default: 10)

### Example Configuration

```rust
// Aggressive quality adjustment for low-end devices
let config = QualityConfig {
    target_frame_rate: 30.0,
    degrade_threshold: 1.3,
    upgrade_threshold: 0.8,
    max_quality: QualityLevel::Medium,
    min_quality: QualityLevel::Low,
    degrade_frame_count: 3,
    upgrade_frame_count: 8,
};

// Conservative quality adjustment for high-end devices
let config = QualityConfig {
    target_frame_rate: 60.0,
    degrade_threshold: 2.0,
    upgrade_threshold: 0.5,
    max_quality: QualityLevel::High,
    min_quality: QualityLevel::Medium,
    degrade_frame_count: 10,
    upgrade_frame_count: 20,
};
```

## GPU Capability Detection

The system automatically detects GPU capabilities:

```rust
#[cfg(feature = "gpu-wgpu")]
use rust_widgets::quality::GpuCapability;

// Automatic detection from wgpu adapter
let adapter_info = adapter.get_info();
let gpu_capability = GpuCapability::from_adapter_info(&adapter_info);

// Manual creation for testing
let custom_capability = GpuCapability {
    supports_high_quality: true,
    is_integrated: false,
    performance_tier: 5,
};
```

### Performance Tiers

- **Tier 5**: Discrete GPU (High quality recommended)
- **Tier 3**: Integrated GPU (Medium quality recommended)
- **Tier 2**: Other GPU (Medium quality recommended)
- **Tier 1**: CPU/Virtual GPU (Low quality recommended)

## Performance Monitoring

### Frame Time Monitor

```rust
use rust_widgets::quality::FrameTimeMonitor;

let mut monitor = FrameTimeMonitor::new(60.0);

// Record frame times
for _ in 0..10 {
    monitor.record_frame(0.016); // 60 FPS
}

// Get statistics
let avg_frame_time = monitor.average_frame_time();
let current_fps = monitor.current_fps();
```

### Quality Manager Statistics

```rust
let manager = QualityManager::new();

// Get current performance metrics
let current_fps = manager.current_fps();
let avg_frame_time = manager.average_frame_time();
let current_quality = manager.quality_level();

// Get configuration
let config = manager.config();
let gpu_cap = manager.gpu_capability();
```

## Hysteresis Control

The system uses hysteresis to prevent quality level oscillation:

```rust
// Quality only degrades after 5 consecutive slow frames
let config = QualityConfig {
    degrade_frame_count: 5,
    upgrade_frame_count: 10,  // Quality only upgrades after 10 consecutive fast frames
    ..Default::default()
};
```

## Best Practices

### 1. Start with Appropriate Initial Quality

```rust
let gpu_capability = detect_gpu_capability();
let initial_quality = gpu_capability.recommended_initial_quality();
manager.set_quality_level(initial_quality);
```

### 2. Monitor Quality Changes

```rust
let mut previous_quality = manager.quality_level();

loop {
    // Render frame
    render_frame(backend);

    // Update quality
    let frame_duration = frame_start.elapsed();
    manager.finish_frame(frame_duration);

    // Log quality changes
    let current_quality = manager.quality_level();
    if current_quality != previous_quality {
        println!("Quality changed: {:?} -> {:?}", previous_quality, current_quality);
        previous_quality = current_quality;
    }
}
```

### 3. Adjust Quality Levels Per-Widget

Different widgets can use different quality strategies:

```rust
impl Widget for ComplexWidget {
    fn draw(&self, backend: &mut dyn PaintBackend) {
        match backend.quality_level() {
            QualityLevel::High => {
                // Full rendering with all effects
                self.draw_high_quality(backend);
            }
            QualityLevel::Medium => {
                // Simplified rendering
                self.draw_medium_quality(backend);
            }
            QualityLevel::Low => {
                // Minimal rendering - skip non-critical elements
                self.draw_low_quality(backend);
            }
        }
    }
}
```

### 4. Provide User Override Option

```rust
// Allow users to manually set quality
fn set_quality_preference(level: Option<QualityLevel>) {
    if let Some(level) = level {
        manager.set_quality_level(level);
    } else {
        // Re-enable automatic adjustment
        manager.reset();
    }
}
```

## Integration with PaintBackend

The quality system integrates with the rendering pipeline through the `PaintBackend` trait:

```rust
pub trait PaintBackend {
    // ... existing methods ...

    /// Returns the current quality level for adaptive rendering
    fn quality_level(&self) -> QualityLevel {
        QualityLevel::High
    }
}
```

### SoftwarePaintBackend Implementation

```rust
impl PaintBackend for SoftwarePaintBackend {
    fn quality_level(&self) -> QualityLevel {
        // Software rendering typically uses medium quality
        QualityLevel::Medium
    }

    // ... other methods ...
}
```

## Testing

### Unit Tests

The quality module includes comprehensive unit tests:

```bash
cargo test quality
```

### Performance Testing

Test quality adjustment under various load conditions:

```rust
#[test]
fn test_quality_degradation_under_load() {
    let mut manager = QualityManager::new();

    // Simulate heavy load
    for _ in 0..10 {
        manager.finish_frame(Duration::from_secs_f32(0.030)); // 33 FPS
    }

    assert_eq!(manager.quality_level(), QualityLevel::Medium);
}
```

## Troubleshooting

### Quality Not Adjusting

1. Check if frame times are being recorded correctly
2. Verify threshold configuration
3. Ensure frame count requirements are met
4. Check if quality is manually locked

### Oscillating Quality

1. Increase `degrade_frame_count` and `upgrade_frame_count`
2. Adjust `degrade_threshold` and `upgrade_threshold`
3. Reduce frame time variance

### Performance Still Poor

1. Lower `min_quality` to `QualityLevel::Low`
2. Reduce `target_frame_rate`
3. Implement widget-level optimizations
4. Consider reducing overall scene complexity

## Future Enhancements

- Per-widget quality settings
- Quality presets for different use cases
- Machine learning-based quality prediction
- User-configurable quality profiles
- Quality transition animations
- Quality metrics dashboard

## Examples

See `demos/demo_quality.rs` for comprehensive examples of:
- Quality level navigation
- Configuration options
- Frame time monitoring
- Quality manager usage
- GPU capability detection
- Integration patterns

## License

This feature is part of the rust-widgets library and follows the same license terms.
