# 渲染系統

`rust-widgets` 渲染系統提供了一個三層架構，抽象化 GPU 和 CPU 後端。控制項發出 `RenderCommand` 繪圖呼叫，組合成 z 排序的 `RenderScene` 圖層，由活動的 `PaintBackend` 執行。單一的 `compose_to_config_auto` 呼叫自動選擇最佳後端（透過 wgpu 的 GPU，或 CPU 軟體光柵化），而品質系統根據幀預算動態調整保真度。

在 2.0.0（BLUE15）中，渲染系統是**唯一**繪製控制項的路徑。函式庫會 100% 自行繪製自己的控制項，且在任何平台上都**不會**建立原生作業系統控制項；後端只提供繪製面、事件迴圈、輸入轉譯與平台服務。因此本章的每一項內容對應用程式的實際外觀都至關重要。

---

## 三層架構

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

## 座標系統

所有渲染操作都使用畫面座標系統，原點位於**左上角**：

- **X 軸：** 由左至右遞增（0 → width）
- **Y 軸：** 由上至下遞增（0 → height）

所有座標皆以**邏輯像素**為單位。渲染情境（context）會在內部處理所有 DPI 轉換。

---

## `RenderCommand` 列舉——30 個變體

由控制項的渲染方法記錄、並交由繪製後端執行的繪圖命令：

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

**反鋸齒變體**（後綴為 `AA`）提供更平滑的邊緣，代價是更多的運算。軟體路徑使用多重取樣（每軸 1–8 個取樣點）。GPU 路徑則透過著色器取樣來處理反鋸齒。

---

## 軟體渲染路徑

### BackBuffer——雙緩衝像素儲存

```rust
let mut buffer = BackBuffer::new(Size::new(800, 600), 1.0);
buffer.back_mut().fill(0);                 // 清除後緩衝區
// ... 渲染命令會修改後緩衝區 ...
buffer.present();                           // 交換前 ↔ 後
let pixels: &[u8] = buffer.front();        // 讀取前緩衝區（RGBA）
```

### SoftwareSurface——光柵表面

`SoftwareSurface` 包裝 `BackBuffer`，並加入反鋸齒控制與裁剪堆疊：

```rust
let mut surface = SoftwareSurface::new(Size::new(800, 600), 1.0);

surface.begin_frame(Color::WHITE);         // 清除為白色

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

let frame: &[u8] = surface.frame_rgba();   // RGBA 像素資料
```

### SoftwareRenderConfig——反鋸齒品質

```rust
let config = SoftwareRenderConfig {
    aa_samples_per_axis: 4,  // 1..=8, default: 4
}.normalized();              // clamp to valid range

surface.apply_render_config(config);

// Or globally:
set_default_software_render_config(config);
```

| 取樣數 | 品質 | 效能 |
|---|---|---|
| 1 | 無反鋸齒（鋸齒狀） | 最快 |
| 2 | 最小平滑處理 | 快 |
| 4 | 良好平衡（預設） | 中等 |
| 8 | 最高品質 | 最慢 |

---

## GPU 渲染路徑

GPU 路徑（由 `feature = "gpu-wgpu"` 門控）使用 `WgpuRenderer` 搭配 WGSL 著色器：

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

**WGSL 著色器**會在執行階段從內嵌的著色器原始碼編譯。GPU 路徑支援：
- 實例化矩形渲染（批次化的填滿／描邊）
- 圓角著色器
- 漸層填滿的運算著色器
- 用於字形的紋理圖集取樣
- 透過多重取樣渲染目標實現反鋸齒

---

## 自動後端選擇

`RenderScene::compose_to_config_auto` 自動選擇最佳後端：

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

自動選擇的路徑為：
1. 若啟用 `feature = "gpu-wgpu"` 且有可用的 GPU 配接器 → 使用 `WgpuRenderer`
2. 否則 → 使用 `SoftwarePaintBackend`

---

## `PaintBackend` 特徵

所有渲染後端實作的策略特徵：

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

**實作自訂後端**——例如 PDF 匯出器或無頭測試後端：

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

## `RenderScene`——Z 排序圖層

`RenderScene` 可容納多個 `SceneLayer` 物件，每個圖層位於特定的 z 索引，並持有一份有序的 `RenderCommand` 清單：

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

圖層在合成前會依 `z_index` 排序。控制項的 `z` 屬性會對應到 `SceneLayer::z_index`。

---

## 批次渲染

`BatchRenderer` 特徵與 `BatchCommand` 系統可記錄並有效率地重播繪圖命令：

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

`BatchCommand` 的變體包括：`FillRect`、`StrokeRect`、`DrawLine`、`DrawImage`、`DrawImageSubrect`、`DrawText`、`PushClip`、`PopClip`、`Translate`、`SetOpacity`。

批次化對於靜態內容或經常重複使用的 UI 元素（格線、背景、圖示）特別有效。

---

## SVG 匯出

使用 `SvgPaintBackend` 將任何控制項樹渲染為 SVG 檔案：

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
    .expect("寫入 SVG 失敗");
```

**將整個控制項匯出為 SVG：**

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

## 富文字——`TextSpan` 與 `TextStyle`

在單一次渲染呼叫中，組合具有多種字型、色彩與格式的樣式化文字區塊：

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

**`TextStyle` 欄位：** `font_family`、`font_size`、`color`、`bold`、`italic`、`underline`、`strikethrough`。

---

## 文字溢出處理

三種溢出模式控制文字超出其容器時的行為：

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

## 文字塑形

`TextShaper` 特徵抽象化特定字型的字形版面配置：

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

`SimpleTextShaper` 以近似方式計算度量（每個字元 0.6 × font_size，行高 1.2）。在正式環境中，請改用以 HarfBuzz 為基礎的塑形器，以獲得精確的字距調整與連字。

---

## Unicode 字素叢集

`GraphemeProcessor` 處理複雜的 Unicode 序列，以實現正確的游標移動與文字選取：

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

**可辨識的序列：**
- 基底字元 + 組合標記（é = e + ́）
- 表情符號 + 膚色／髮型修飾符
- ZWJ（零寬連接符）多表情符號序列
- 區域指示符配對（🇺🇸 國旗）

---

## 漸層系統

三種漸層型別，支援色標插值：

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
    gradient,
});
```

---

## 混合模式——16 種模式

`BlendMode` 控制繪圖命令如何與現有像素合成：

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

## 投影模式

用於簡報／投影顯示（由 `feature = "projection"` 門控）：

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

## 品質管理——適應性渲染

`AdaptiveRenderer` 會動態調整品質以符合幀預算：

```rust
#[cfg(feature = "quality-management")]
use rust_widgets::render::{current_quality_level, set_quality_level, QualityLevel};

// Manual quality control. Levels feed the frame-budget adapters (the
// AdaptiveRenderer and the wgpu compose path). Per-axis AA sample counts are
// configured separately via `set_render_aa_samples_per_axis`.
set_quality_level(QualityLevel::High);
set_quality_level(QualityLevel::Medium);
set_quality_level(QualityLevel::Low);

// Query current metrics
// 查詢當前指標
let fps = current_fps();
let frame_time = average_frame_time();

// 適應模式：根據幀時間自動調整
// 若 frame_time > 16ms → 降低品質
// 若 frame_time < 8ms  → 提高品質
```

---

## 常見模式

### 按鈕渲染方法

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

### 複合後端管線

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

### 動畫漸層背景

```rust
fn render_animated_background(scene: &mut RenderScene, time: f32) {
    let mut layer = SceneLayer::new(0);

    let gradient = Gradient::linear(Point::new(0, 0), Point::new(800, 0))
        .add_stop(0.0, lerp_color(&Color::RED, &Color::BLUE, time.sin() * 0.5 + 0.5))
        .add_stop(0.5, lerp_color(&Color::GREEN, &Color::YELLOW, time.cos() * 0.5 + 0.5))
        .add_stop(1.0, lerp_color(&Color::BLUE, &Color::PURPLE, (time * 1.7).sin() * 0.5 + 0.5));

    layer.push(RenderCommand::DrawGradient {
        rect: Rect::new(0, 0, 800, 600),
        gradient,
    });

    scene.add_layer(layer);
}
```

### 表單控制項的 SVG 匯出

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

### UI 標籤的文字截斷

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
