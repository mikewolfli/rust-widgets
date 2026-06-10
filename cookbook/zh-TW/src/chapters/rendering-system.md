# 渲染系統

`rust-widgets` 渲染系統提供了一個三層架構，抽象化 GPU 和 CPU 後端。控制項發出 `RenderCommand` 繪圖呼叫，組合成 z 排序的 `RenderScene` 圖層，由活動的 `PaintBackend` 執行。單一的 `compose_to_config_auto` 呼叫自動選擇最佳後端（透過 wgpu 的 GPU，或 CPU 軟體光柵化），而品質系統根據幀預算動態調整保真度。

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

## `RenderCommand` 列舉——30 個變體

```rust
pub enum RenderCommand {
    FillRect { rect: Rect, color: Color },
    DrawRect { rect: Rect, color: Color },
    DrawRectStroke { rect: Rect, color: Color, width: u32 },
    FillRoundedRect { rect: Rect, radius: u32, color: Color },
    FillRoundedRectAA { rect: Rect, radius: u32, color: Color },
    DrawLine { from: Point, to: Point, color: Color },
    DrawLineAA { from: Point, to: Point, color: Color },
    FillCircle { center: Point, radius: u32, color: Color },
    FillCircleAA { center: Point, radius: u32, color: Color },
    DrawText { origin: Point, text: String, font: Font, color: Color, alignment: HorizontalAlignment },
    DrawImage { x: i32, y: i32, width: u32, height: u32, data: Vec<u8> },
    PushClip { x: i32, y: i32, width: u32, height: u32 },
    PopClip,
    DrawGradient { rect: Rect, gradient: Gradient },
    DrawArc { center: Point, radius: u32, start_angle: f32, end_angle: f32, color: Color, filled: bool },
    DrawPath { points: Vec<Point>, closed: bool, color: Color, filled: bool, width: f32 },
    Transform { matrix: [[f32; 3]; 3] },
    BlendCommand { mode: BlendMode },
}
```

---

## 軟體渲染路徑

### BackBuffer——雙緩衝像素儲存

```rust
let mut buffer = BackBuffer::new(Size::new(800, 600), 1.0);
buffer.back_mut().fill(0);
buffer.present();
let pixels: &[u8] = buffer.front();
```

### SoftwareSurface——光柵表面

```rust
let mut surface = SoftwareSurface::new(Size::new(800, 600), 1.0);
surface.begin_frame(Color::WHITE);
surface.fill_rounded_rect_aa(Rect::new(10, 10, 100, 40), 8, Color::BLUE);
surface.end_frame();
```

---

## GPU 渲染路徑

GPU 路徑（由 `feature = "gpu-wgpu"` 門控）使用 `WgpuRenderer` 搭配 WGSL 著色器：

```rust
#[cfg(feature = "gpu-wgpu")]
use rust_widgets::render::gpu::{GpuRenderer, GpuCapability};

let mut gpu = GpuRenderer::new()?;
gpu.begin_frame(Color::WHITE);
gpu.execute_command(&RenderCommand::FillRect {
    rect: Rect::new(0, 0, 800, 600),
    color: Color::BACKGROUND,
});
gpu.end_frame();
```

---

## 自動後端選擇

`RenderScene::compose_to_config_auto` 自動選擇最佳後端：

```rust
let scene = RenderScene::new();
let config = SoftwareRenderConfig::default();
let result = scene.compose_to_config_auto(&config);
```

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

---

## SVG 匯出

使用 `SvgPaintBackend` 將任何控制項樹渲染為 SVG 檔案：

```rust
let mut svg = SvgPaintBackend::new(Size::new(800, 600));
svg.begin_frame(Color::TRANSPARENT);
// ... 執行命令 ...
std::fs::write("button.svg", svg.to_svg_string()).expect("寫入 SVG 失敗");
```

---

## 漸層系統

三種漸層型別，支援色標插值：

```rust
let linear = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
    .add_stop(0.0, Color::RED)
    .add_stop(0.5, Color::GREEN)
    .add_stop(1.0, Color::BLUE);
```

---

## 混合模式——16 種模式

```rust
pub enum BlendMode {
    Normal, Multiply, Screen, Overlay,
    Darken, Lighten, ColorDodge, ColorBurn,
    HardLight, SoftLight, Difference, Exclusion,
    Hue, Saturation, Color, Luminosity,
}
```

---

## 文字溢出處理

三種溢出模式控制文字超出其容器時的行為：

```rust
use rust_widgets::render::{TextOverflow, apply_text_overflow};

let ellipsis = apply_text_overflow("很長的一段文字...", 100.0, font_size, TextOverflow::Ellipsis);
```

---

## 常見模式

### 按鈕渲染方法

```rust
impl MyButton {
    fn render(&self, scene: &mut RenderScene) {
        let mut layer = SceneLayer::new(self.z_index);
        let bg_color = match self.state {
            WidgetState::Normal => Color::from_rgb(33, 150, 243),
            WidgetState::Hover => Color::from_rgb(66, 165, 245),
            WidgetState::Pressed => Color::from_rgb(25, 118, 210),
            _ => Color::from_rgb(33, 150, 243),
        };
        layer.push(RenderCommand::FillRoundedRectAA { rect: self.rect, radius: 8, color: bg_color });
        scene.add_layer(layer);
    }
}
```

### 表單控制項的 SVG 匯出

```rust
fn export_form_to_svg() -> String {
    let mut svg = SvgPaintBackend::new(Size::new(400, 300));
    svg.begin_frame(Color::WHITE);
    svg.execute_command(&RenderCommand::DrawText { /* ... */ });
    svg.end_frame();
    svg.to_svg_string()
}
```
