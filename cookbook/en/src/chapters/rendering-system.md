# Rendering System

The `rust-widgets` rendering system provides a three-tier architecture that abstracts over GPU and CPU backends. Widgets emit `RenderCommand` draw calls that are composed into z-ordered `RenderScene` layers, which the active `PaintBackend` executes. A single `compose_to_config_auto` call automatically selects the best backend (GPU via wgpu, or CPU software rasterization), while the quality system dynamically adjusts fidelity based on frame budget.

---

## Three-Tier Architecture

```
Widget::render() → RenderCommand[] → RenderScene (z-ordered layers)
                                           │
                         ┌─────────────────┼─────────────────┐
                         ▼                                    ▼
                   PaintBackend                          PaintBackend
                   (SoftwarePaintBackend)                (WgpuRenderer)
                         │                                    │
                    BackBuffer                           GPU Pipeline
                   (CPU raster,                          (WGSL shaders,
                    AA 1–8 samples)                      adapter selection)
```

---

## Coordinate System

All rendering operations use the screen coordinate system with the origin at **top-left**:

- **X axis:** increases left to right (0 → width)
- **Y axis:** increases top to bottom (0 → height)

All coordinates are in **logical pixels**. The rendering context handles any DPI transformations internally.

---

## `RenderCommand` Enum — 30 Variants

Draw commands recorded by widget render methods and executed by the paint backend:

```rust
pub enum RenderCommand {
    // Rectangle Fill/Stroke
    FillRect { rect: Rect, color: Color },
    DrawRect { rect: Rect, color: Color },
    DrawRectStroke { rect: Rect, color: Color, width: u32 },
    FillRoundedRect { rect: Rect, radius: u32, color: Color },
    FillRoundedRectAA { rect: Rect, radius: u32, color: Color },
    DrawRoundedRectStroke { rect: Rect, radius: u32, color: Color, width: u32 },
    DrawRoundedRectStrokeAA { rect: Rect, radius: u32, color: Color, width: u32 },

    // Lines
    DrawLine { from: Point, to: Point, color: Color },
    DrawLineAA { from: Point, to: Point, color: Color },
    DrawLineStroke { from: Point, to: Point, color: Color, width: u32 },
    DrawLineStrokeAA { from: Point, to: Point, color: Color, width: u32 },

    // Circles
    FillCircle { center: Point, radius: u32, color: Color },
    FillCircleAA { center: Point, radius: u32, color: Color },
    DrawCircle { center: Point, radius: u32, color: Color },
    DrawCircleStroke { center: Point, radius: u32, color: Color, width: u32 },

    // Text & Images
    DrawText { origin: Point, text: String, font: Font, color: Color, alignment: HorizontalAlignment },
    DrawImage { x: i32, y: i32, width: u32, height: u32, data: Vec<u8> },

    // Clip & Gradient
    PushClip { x: i32, y: i32, width: u32, height: u32 },
    PopClip,
    DrawGradient { rect: Rect, gradient: Gradient },

    // Curves
    DrawArc { center: Point, radius: u32, start_angle: f32, end_angle: f32, color: Color, filled: bool },
    DrawPath { points: Vec<Point>, closed: bool, color: Color, filled: bool, width: f32 },

    // Transform
    Transform { matrix: [[f32; 3]; 3] },

    // Blending
    BlendCommand { mode: BlendMode },

    // Additional variants for ellipses, polygons, and text runs...
}
```

**Anti-aliased variants** (suffixed `AA`) provide smoother edges at the cost of more computation. The software path uses multi-sampling (1–8 samples per axis). The GPU path handles AA via shader sampling.

---

## Software Rendering Path

### `BackBuffer` — Double-Buffered Pixel Storage

```rust
let mut buffer = BackBuffer::new(Size::new(800, 600), 1.0);
buffer.back_mut().fill(0);                 // clear back buffer
// ... render commands modify back buffer
buffer.present();                           // swap front ↔ back
let pixels: &[u8] = buffer.front();        // read front buffer (RGBA)
```

### `SoftwareSurface` — Raster Surface

Wraps `BackBuffer` with anti-aliasing controls and a clip stack:

```rust
let mut surface = SoftwareSurface::new(Size::new(800, 600), 1.0);

surface.begin_frame(Color::WHITE);         // clear to white

surface.fill_rounded_rect_aa(Rect::new(10, 10, 100, 40), 8, Color::BLUE);
surface.draw_line_aa(Point::new(0, 0), Point::new(800, 600), Color::RED);
surface.draw_text(
    Point::new(20, 20),
    "Hello, World!",
    &Font::simple("Arial", 16.0),
    Color::BLACK,
    HorizontalAlignment::Left,
);

// Clip region
surface.push_clip(50, 50, 200, 100);
surface.fill_rect(Rect::new(0, 0, 800, 600), Color::GREEN); // clipped!
surface.pop_clip();

surface.end_frame();

let frame: &[u8] = surface.frame_rgba();   // RGBA pixel data
```

### `SoftwareRenderConfig` — Anti-Aliasing Quality

```rust
let config = SoftwareRenderConfig {
    aa_samples_per_axis: 4,  // 1..=8, default: 4
}.normalized();              // clamp to valid range

surface.apply_render_config(config);

// Or globally:
set_default_software_render_config(config);
```

| Samples | Quality | Performance |
|---|---|---|
| 1 | No AA (aliased) | Fastest |
| 2 | Minimal smoothing | Fast |
| 4 | Good balance (default) | Moderate |
| 8 | Maximum quality | Slowest |

---

## GPU Rendering Path

The GPU path (gated behind `feature = "gpu-wgpu"`) uses `WgpuRenderer` with WGSL shaders:

```rust
#[cfg(feature = "gpu-wgpu")]
use rust_widgets::render::gpu::{GpuRenderer, GpuCapability};

// GPU renderer automatically selects the best adapter
let mut gpu = GpuRenderer::new()?;

// Check capabilities
let caps: GpuCapability = gpu.capabilities();
println!("Max texture size: {}", caps.max_texture_size);

// Begin a frame
gpu.begin_frame(Color::WHITE);

// Execute render commands (same RenderCommand API)
gpu.execute_command(&RenderCommand::FillRect {
    rect: Rect::new(0, 0, 800, 600),
    color: Color::BACKGROUND,
});

gpu.end_frame();
```

**WGSL Shaders** are compiled at runtime from embedded shader sources. The GPU path supports:
- Instanced rectangle rendering (batched fill/stroke)
- Rounded corner shaders
- Gradient fill compute shaders
- Texture atlas sampling for glyphs
- Anti-aliasing via multi-sampled render targets

---

## Auto Backend Selection

`RenderScene::compose_to_config_auto` selects the best backend automatically:

```rust
let scene = RenderScene::new();
// ... populate layers with commands ...

let config = SoftwareRenderConfig::default();

// Auto-selects GPU if available, falls back to CPU software
let result = scene.compose_to_config_auto(&config);

// Check which backend was used
match last_auto_render_backend() {
    AutoRenderBackend::GpuWgpu => println!("Using GPU rendering"),
    AutoRenderBackend::CpuSoftware => println!("Using CPU software rendering"),
}
```

The auto-selection path:
1. If `feature = "gpu-wgpu"` is enabled and a GPU adapter is available → use `WgpuRenderer`
2. Otherwise → use `SoftwarePaintBackend`

---

## `PaintBackend` Trait

The strategy trait implemented by all render backends:

```rust
pub trait PaintBackend {
    fn begin_frame(&mut self, clear: Color);
    fn end_frame(&mut self);
    fn execute_command(&mut self, command: &RenderCommand);
    fn size(&self) -> Size;
    fn set_size(&mut self, size: Size);
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, dpi_scale: f32);
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics;
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText;
    fn frame_rgba(&self) -> &[u8];
    fn apply_render_config(&mut self, config: SoftwareRenderConfig);
    fn render_config(&self) -> SoftwareRenderConfig;
}
```

**Implementing a custom backend** — for example, a PDF exporter or headless testing backend:

```rust
struct NullBackend {
    size: Size,
    dpi_scale: f32,
}

impl PaintBackend for NullBackend {
    fn begin_frame(&mut self, _clear: Color) {}
    fn end_frame(&mut self) {}
    fn execute_command(&mut self, _command: &RenderCommand) {}
    fn size(&self) -> Size { self.size }
    fn set_size(&mut self, size: Size) { self.size = size; }
    fn dpi_scale(&self) -> f32 { self.dpi_scale }
    fn set_dpi_scale(&mut self, dpi: f32) { self.dpi_scale = dpi; }
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        TextMetrics {
            width: text.len() as f32 * font.size * 0.6,
            height: font.size * 1.2,
        }
    }
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        ShapedText::simple(text, font.size)
    }
    fn frame_rgba(&self) -> &[u8] { &[] }
}
```

---

## `RenderScene` — Z-Ordered Layers

A `RenderScene` holds multiple `SceneLayer` objects, each at a specific z-index with an ordered list of `RenderCommand` entries:

```rust
use rust_widgets::render::{RenderScene, SceneLayer, PaintBackend};

let mut scene = RenderScene::new();

// Background layer (z=0)
let mut bg_layer = SceneLayer::new(0);
bg_layer.push(RenderCommand::FillRect {
    rect: Rect::new(0, 0, 800, 600),
    color: Color::from_rgb(240, 240, 240),
});
scene.add_layer(bg_layer);

// Content layer (z=10)
let mut content_layer = SceneLayer::new(10);
content_layer.push(RenderCommand::DrawText {
    origin: Point::new(20, 20),
    text: "Hello".into(),
    font: Font::simple("Arial", 16.0),
    color: Color::BLACK,
    alignment: HorizontalAlignment::Left,
});
scene.add_layer(content_layer);

// Overlay layer (z=100)
let mut overlay_layer = SceneLayer::new(100);
overlay_layer.push(RenderCommand::FillRoundedRectAA {
    rect: Rect::new(300, 200, 200, 100),
    radius: 12,
    color: Color::rgba(0, 0, 0, 180),
});
scene.add_layer(overlay_layer);

// Compose all layers to the backend
scene.compose(&mut backend);
```

Layers are sorted by `z_index` before composition. A widget's `z` property maps to `SceneLayer::z_index`.

---

## Batch Rendering

The `BatchRenderer` trait and `BatchCommand` system enable recording and efficiently replaying draw commands:

```rust
use rust_widgets::render::{BatchCommand, BatchRenderer, BatchId};

fn render_cache(batcher: &mut impl BatchRenderer) -> Result<(), BatchError> {
    // Record a batch
    let batch_id = batcher.begin_batch();

    batcher.record(BatchCommand::FillRect {
        rect: Rect::new(0, 0, 100, 100),
        color: Color::RED,
    })?;

    batcher.record(BatchCommand::StrokeRect {
        rect: Rect::new(0, 0, 100, 100),
        color: Color::BLACK,
        width: 2.0,
    })?;

    batcher.end_batch();

    // Replay the batch (efficiently replayed by the GPU or CPU backend)
    batcher.replay(batch_id);

    // Destroy when no longer needed
    batcher.destroy_batch(batch_id);
    Ok(())
}
```

`BatchCommand` variants include: `FillRect`, `StrokeRect`, `DrawLine`, `DrawImage`, `DrawImageSubrect`, `DrawText`, `PushClip`, `PopClip`, `Translate`, `SetOpacity`.

Batching is particularly effective for static content or frequently reused UI elements (grid lines, backgrounds, icons).

---

## SVG Export

Render any widget tree to an SVG file using `SvgPaintBackend`:

```rust
use rust_widgets::render::SvgPaintBackend;

let mut svg = SvgPaintBackend::new(Size::new(800, 600));

svg.begin_frame(Color::TRANSPARENT);

// Render a button
svg.execute_command(&RenderCommand::FillRoundedRect {
    rect: Rect::new(10, 10, 120, 40),
    radius: 8,
    color: Color::from_rgb(33, 150, 243),
});
svg.execute_command(&RenderCommand::DrawText {
    origin: Point::new(20, 18),
    text: "Click Me".into(),
    font: Font::simple("Arial", 14.0),
    color: Color::WHITE,
    alignment: HorizontalAlignment::Left,
});

svg.end_frame();

// Write SVG to file
std::fs::write("button.svg", svg.to_svg_string())
    .expect("Failed to write SVG");
```

**Export an entire widget to SVG:**

```rust
fn render_widget_to_svg(widget: &dyn Widget, size: Size) -> String {
    let mut svg = SvgPaintBackend::new(size);
    svg.begin_frame(Color::TRANSPARENT);

    let mut scene = RenderScene::new();
    widget.render(&mut scene);
    scene.compose(&mut svg);

    svg.end_frame();
    svg.to_svg_string()
}
```

---

## Rich Text — `TextSpan` and `TextStyle`

Compose styled text blocks with multiple fonts, colors, and formatting in a single render call:

```rust
use rust_widgets::render::{RichText, TextSpan, TextStyle};

let mut rich = RichText::new();

rich.add_span(TextSpan {
    text: "Bold ".into(),
    style: TextStyle {
        font_family: "Arial".into(),
        font_size: 16.0,
        color: Color::BLACK,
        bold: true,
        ..Default::default()
    },
});

rich.add_span(TextSpan {
    text: "Red Italic".into(),
    style: TextStyle {
        font_family: "Arial".into(),
        font_size: 16.0,
        color: Color::from_rgb(255, 0, 0),
        italic: true,
        ..Default::default()
    },
});

// Measure the full text block
let metrics = rich.measure(&shaper);
println!("Rich text: {} × {} px", metrics.width, metrics.height);

// Render each span in sequence
for span in rich.spans() {
    backend.execute_command(&RenderCommand::DrawText {
        origin: cursor,
        text: span.text.clone(),
        font: Font::simple(&span.style.font_family, span.style.font_size),
        color: span.style.color,
        alignment: HorizontalAlignment::Left,
    });
    cursor.x += span.width;
}
```

**`TextStyle` fields:** `font_family`, `font_size`, `color`, `bold`, `italic`, `underline`, `strikethrough`.

---

## Text Overflow Handling

Three overflow modes control how text behaves when it exceeds its container:

```rust
use rust_widgets::render::{TextOverflow, apply_text_overflow, TextClamp, apply_text_clamp};

// Clip: text is simply cut at the boundary
let clipped = apply_text_overflow("Very long text...", 100.0, font_size, TextOverflow::Clip);

// Ellipsis: truncated text ends with "..."
let ellipsis = apply_text_overflow("Very long text...", 100.0, font_size, TextOverflow::Ellipsis);

// Fade: opacity gradually reduces toward the overflow edge
let faded = apply_text_overflow("Very long text...", 100.0, font_size, TextOverflow::Fade);

// Multi-line clamp (max N lines)
let clamped = apply_text_clamp(
    "Long paragraph text that spans multiple lines...",
    200.0,     // max width
    font_size,
    TextClamp::Lines(3),  // max 3 lines, ellipsis on overflow
);
```

---

## Text Shaping

The `TextShaper` trait abstracts over font-specific glyph layout:

```rust
use rust_widgets::render::{TextShaper, SimpleTextShaper, ShapedGlyphRun};

let shaper = SimpleTextShaper::new();

let width = shaper.measure_width("Hello, World!", 16.0);
let height = shaper.measure_height("Hello, World!", 16.0);

// Get detailed glyph positions
let runs: Vec<ShapedGlyphRun> = shaper.shape("Hello", 16.0);
for run in &runs {
    for (glyph_id, (x, y)) in run.glyph_ids.iter().zip(run.positions.iter()) {
        println!("Glyph {} at ({}, {})", glyph_id, x, y);
    }
}
```

`SimpleTextShaper` approximates metrics (0.6 × font_size per character, 1.2 line height). For production use, replace with a HarfBuzz-based shaper for accurate kerning and ligatures.

---

## Unicode Grapheme Clustering

The `GraphemeProcessor` handles complex Unicode sequences for correct cursor movement and text selection:

```rust
use rust_widgets::render::{GraphemeCluster, GraphemeProcessor};

let text = "Hello 👨‍👩‍👧‍👦 World! é";
let clusters: Vec<GraphemeCluster> = GraphemeProcessor::split_graphemes(text);

for cluster in &clusters {
    println!("'{}' — {} chars, ~{:.1}px wide",
        cluster.content, cluster.char_count, cluster.width);
}

// Output:
// 'H' — 1 chars, ~8.4px wide
// 'e' — 1 chars, ~8.4px wide
// ...
// '👨‍👩‍👧‍👦' — 7 chars, ~8.4px wide  (ZWJ family emoji — one cluster!)
```

**Recognized sequences:**
- Base character + combining marks (é = e + ́)
- Emoji + skin tone / hair style modifiers
- ZWJ (Zero-Width Joiner) multi-emoji sequences
- Regional indicator pairs (🇺🇸 flags)

---

## Gradient System

Three gradient types with color-stop interpolation:

```rust
use rust_widgets::render::Gradient;
use rust_widgets::core::{Color, Point};

// Linear gradient: left-to-right fade
let linear = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
    .add_stop(0.0, Color::RED)
    .add_stop(0.5, Color::GREEN)
    .add_stop(1.0, Color::BLUE);

// Radial gradient: center-out
let radial = Gradient::radial(Point::new(50, 50), 100.0)
    .add_stop(0.0, Color::WHITE)
    .add_stop(1.0, Color::BLACK);

// Conic gradient: angular sweep
let conic = Gradient::conic(Point::new(50, 50), 0.0) // angle in radians
    .add_stop(0.0, Color::RED)
    .add_stop(0.33, Color::GREEN)
    .add_stop(0.66, Color::BLUE)
    .add_stop(1.0, Color::RED);  // wrap around

// Interpolate a color at any position
let mid_color = linear.interpolate(0.5);  // midway between RED and GREEN

// Reverse the gradient
let reversed = linear.reverse();

// Builder pattern
use rust_widgets::render::GradientBuilder;
let gradient = GradientBuilder::linear(Point::new(0, 0), Point::new(200, 0))
    .stop(0.0, Color::rgba(255, 0, 0, 255))
    .stop(0.5, Color::rgba(0, 255, 0, 128))
    .stop(1.0, Color::rgba(0, 0, 255, 255))
    .build();

// Apply to a RenderCommand
backend.execute_command(&RenderCommand::DrawGradient {
    rect: Rect::new(0, 0, 200, 100),
    gradient: &gradient,
});
```

---

## Blend Modes — 16 Modes

`BlendMode` controls how draw commands composite with existing pixels:

```rust
pub enum BlendMode {
    Normal, Multiply, Screen, Overlay,
    Darken, Lighten, ColorDodge, ColorBurn,
    HardLight, SoftLight, Difference, Exclusion,
    Hue, Saturation, Color, Luminosity,
}
```

```rust
// Apply a blend mode via command
scene_layer.push(RenderCommand::BlendCommand {
    mode: BlendMode::Multiply,
});

// In the software backend, blend_pixel() applies the mode
use rust_widgets::render::blend_pixel;
let blended = blend_pixel(src_rgba, dst_rgba, BlendMode::Overlay);
```

---

## Projection Mode

For presentation/projection displays (gated behind `feature = "projection"`):

```rust
#[cfg(feature = "projection")]
use rust_widgets::render::projection::{
    PresentationController, ProjectionLayoutHelper, ProjectionRenderConfig,
};

let config = ProjectionRenderConfig {
    target_width: 1920,
    target_height: 1080,
    scale_to_fit: true,
    letterbox_color: Color::BLACK,
};

let mut controller = PresentationController::new(config);
let helper = ProjectionLayoutHelper::new(&controller);

// Layout adjusts to fit the projection surface while maintaining aspect ratio
let adjusted_rect = helper.adjust_widget_rect(widget_rect);
```

---

## Quality Management — Adaptive Rendering

The `AdaptiveRenderer` dynamically adjusts quality to meet a frame budget:

```rust
#[cfg(feature = "quality-management")]
use rust_widgets::render::quality::{QualityLevel, set_quality_level, current_quality_level};

// Manual quality control
set_quality_level(QualityLevel::High);   // AA 8×, full effects
set_quality_level(QualityLevel::Medium); // AA 4×, simplified shadows
set_quality_level(QualityLevel::Low);    // AA 1×, no shadows

// Query current metrics
let fps = current_fps();
let frame_time = average_frame_time();

// Adaptive mode: auto-adjusts based on frame time
// If frame_time > 16ms → reduce quality
// If frame_time < 8ms  → increase quality
```

---

## Common Patterns

### Button Render Method

```rust
impl MyButton {
    fn render(&self, scene: &mut RenderScene) {
        let mut layer = SceneLayer::new(self.z_index);

        // Background
        let bg_color = match self.state {
            WidgetState::Normal => Color::from_rgb(33, 150, 243),
            WidgetState::Hover => Color::from_rgb(66, 165, 245),
            WidgetState::Pressed => Color::from_rgb(25, 118, 210),
            WidgetState::Disabled => Color::from_rgb(189, 189, 189),
            _ => Color::from_rgb(33, 150, 243),
        };

        layer.push(RenderCommand::FillRoundedRectAA {
            rect: self.rect,
            radius: 8,
            color: bg_color,
        });

        // Label text (centered)
        let text_width = shaper.measure_width(&self.label, self.font_size);
        let text_x = self.rect.x + (self.rect.width as i32 - text_width as i32) / 2;
        let text_y = self.rect.y + (self.rect.height as i32 - self.font_size as i32) / 2;

        layer.push(RenderCommand::DrawText {
            origin: Point::new(text_x, text_y),
            text: self.label.clone(),
            font: Font::simple("Arial", self.font_size),
            color: Color::WHITE,
            alignment: HorizontalAlignment::Left,
        });

        scene.add_layer(layer);
    }
}
```

### Composite Backend Pipeline

```rust
fn render_frame(widget_tree: &WidgetTree, size: Size) -> Vec<u8> {
    let mut scene = RenderScene::new();

    // Walk the widget tree, each widget adds commands to the scene
    widget_tree.render(&mut scene);

    // Create a software backend for the frame
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    backend.begin_frame(Color::WHITE);

    // Compose the scene
    scene.compose(&mut backend);

    backend.end_frame();

    backend.frame_rgba().to_vec()
}
```

### Animated Gradient Background

```rust
fn render_animated_background(scene: &mut RenderScene, time: f32) {
    let mut layer = SceneLayer::new(0);

    let gradient = Gradient::linear(Point::new(0, 0), Point::new(800, 0))
        .add_stop(0.0, lerp_color(&Color::RED, &Color::BLUE, time.sin() * 0.5 + 0.5))
        .add_stop(0.5, lerp_color(&Color::GREEN, &Color::YELLOW, time.cos() * 0.5 + 0.5))
        .add_stop(1.0, lerp_color(&Color::BLUE, &Color::PURPLE, (time * 1.7).sin() * 0.5 + 0.5));

    layer.push(RenderCommand::DrawGradient {
        rect: Rect::new(0, 0, 800, 600),
        gradient: &gradient,
    });

    scene.add_layer(layer);
}
```

### SVG Export of a Form Widget

```rust
fn export_form_to_svg() -> String {
    let mut svg = SvgPaintBackend::new(Size::new(400, 300));

    svg.begin_frame(Color::WHITE);

    // Form title
    svg.execute_command(&RenderCommand::DrawText {
        origin: Point::new(20, 20),
        text: "Login".into(),
        font: Font::bold("Arial", 18.0),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });

    // Input field border
    svg.execute_command(&RenderCommand::DrawRoundedRectStrokeAA {
        rect: Rect::new(20, 60, 360, 36),
        radius: 4,
        color: Color::from_rgb(158, 158, 158),
        width: 1,
    });

    // Submit button
    svg.execute_command(&RenderCommand::FillRoundedRectAA {
        rect: Rect::new(20, 120, 120, 40),
        radius: 6,
        color: Color::from_rgb(33, 150, 243),
    });

    svg.end_frame();
    svg.to_svg_string()
}
```

### Text Truncation for UI Labels

```rust
fn render_truncated_label(
    command_list: &mut Vec<RenderCommand>,
    text: &str,
    max_width: f32,
    font_size: f32,
    origin: Point,
) {
    use rust_widgets::render::{apply_text_overflow, TextOverflow};

    let display_text = apply_text_overflow(text, max_width, font_size, TextOverflow::Ellipsis);

    command_list.push(RenderCommand::DrawText {
        origin,
        text: display_text,
        font: Font::simple("Arial", font_size),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
}
```
