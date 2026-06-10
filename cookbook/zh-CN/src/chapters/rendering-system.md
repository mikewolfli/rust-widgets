# 渲染系统

`rust-widgets` 渲染系统采用三层架构，抽象了 GPU 和 CPU 后端。窗口部件（Widget）发射 `RenderCommand` 绘制调用，这些调用被组合成按 Z 轴排序的 `RenderScene` 层，再由活跃的 `PaintBackend` 执行。一次 `compose_to_config_auto` 调用即可自动选择最佳后端（通过 wgpu 的 GPU，或 CPU 软件光栅化），同时质量系统会根据帧预算动态调整渲染精度。

---

## 三层架构

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

## 坐标系

所有渲染操作使用屏幕坐标系，原点位于**左上角**：

- **X 轴：** 从左到右递增（0 → width）
- **Y 轴：** 从上到下递增（0 → height）

所有坐标均为**逻辑像素**。渲染上下文内部处理所有 DPI 变换。

---

## `RenderCommand` 枚举 — 30 种变体

由窗口部件渲染方法记录、再由绘制后端执行的绘制命令：

```rust
pub enum RenderCommand {
    // 矩形填充/描边
    FillRect { rect: Rect, color: Color },
    DrawRect { rect: Rect, color: Color },
    DrawRectStroke { rect: Rect, color: Color, width: u32 },
    FillRoundedRect { rect: Rect, radius: u32, color: Color },
    FillRoundedRectAA { rect: Rect, radius: u32, color: Color },
    DrawRoundedRectStroke { rect: Rect, radius: u32, color: Color, width: u32 },
    DrawRoundedRectStrokeAA { rect: Rect, radius: u32, color: Color, width: u32 },

    // 线条
    DrawLine { from: Point, to: Point, color: Color },
    DrawLineAA { from: Point, to: Point, color: Color },
    DrawLineStroke { from: Point, to: Point, color: Color, width: u32 },
    DrawLineStrokeAA { from: Point, to: Point, color: Color, width: u32 },

    // 圆形
    FillCircle { center: Point, radius: u32, color: Color },
    FillCircleAA { center: Point, radius: u32, color: Color },
    DrawCircle { center: Point, radius: u32, color: Color },
    DrawCircleStroke { center: Point, radius: u32, color: Color, width: u32 },

    // 文本与图像
    DrawText { origin: Point, text: String, font: Font, color: Color, alignment: HorizontalAlignment },
    DrawImage { x: i32, y: i32, width: u32, height: u32, data: Vec<u8> },

    // 裁剪与渐变
    PushClip { x: i32, y: i32, width: u32, height: u32 },
    PopClip,
    DrawGradient { rect: Rect, gradient: Gradient },

    // 曲线
    DrawArc { center: Point, radius: u32, start_angle: f32, end_angle: f32, color: Color, filled: bool },
    DrawPath { points: Vec<Point>, closed: bool, color: Color, filled: bool, width: f32 },

    // 变换
    Transform { matrix: [[f32; 3]; 3] },

    // 混合
    BlendCommand { mode: BlendMode },

    // 椭圆、多边形和文本运行等其他变体...
}
```

**抗锯齿变体**（后缀 `AA`）提供更平滑的边缘，但计算开销更大。软件路径使用多重采样（每轴 1–8 个样本）。GPU 路径通过着色器采样处理抗锯齿。

---

## 软件渲染路径

### `BackBuffer` — 双缓冲像素存储

```rust
let mut buffer = BackBuffer::new(Size::new(800, 600), 1.0);
buffer.back_mut().fill(0);                 // 清空后备缓冲
// ... 渲染命令修改后备缓冲
buffer.present();                           // 交换前台 ↔ 后台
let pixels: &[u8] = buffer.front();        // 读取前台缓冲 (RGBA)
```

### `SoftwareSurface` — 光栅曲面

将 `BackBuffer` 与抗锯齿控制和裁剪栈封装在一起：

```rust
let mut surface = SoftwareSurface::new(Size::new(800, 600), 1.0);

surface.begin_frame(Color::WHITE);         // 清除为白色

surface.fill_rounded_rect_aa(Rect::new(10, 10, 100, 40), 8, Color::BLUE);
surface.draw_line_aa(Point::new(0, 0), Point::new(800, 600), Color::RED);
surface.draw_text(
    Point::new(20, 20),
    "Hello, World!",
    &Font::simple("Arial", 16.0),
    Color::BLACK,
    HorizontalAlignment::Left,
);

// 裁剪区域
surface.push_clip(50, 50, 200, 100);
surface.fill_rect(Rect::new(0, 0, 800, 600), Color::GREEN); // 被裁剪！
surface.pop_clip();

surface.end_frame();

let frame: &[u8] = surface.frame_rgba();   // RGBA 像素数据
```

### `SoftwareRenderConfig` — 抗锯齿质量

```rust
let config = SoftwareRenderConfig {
    aa_samples_per_axis: 4,  // 1..=8, 默认: 4
}.normalized();              // 限制到有效范围

surface.apply_render_config(config);

// 或全局设置：
set_default_software_render_config(config);
```

| 样本数 | 质量 | 性能 |
|:---:|---|---|
| 1 | 无抗锯齿（有锯齿） | 最快 |
| 2 | 最小平滑 | 快 |
| 4 | 良好平衡（默认） | 中等 |
| 8 | 最高质量 | 最慢 |

---

## GPU 渲染路径

GPU 路径（在 `feature = "gpu-wgpu"` 后启用）使用 `WgpuRenderer` 和 WGSL 着色器：

```rust
#[cfg(feature = "gpu-wgpu")]
use rust_widgets::render::gpu::{GpuRenderer, GpuCapability};

// GPU 渲染器自动选择最佳适配器
let mut gpu = GpuRenderer::new()?;

// 检查能力
let caps: GpuCapability = gpu.capabilities();
println!("Max texture size: {}", caps.max_texture_size);

// 开始一帧
gpu.begin_frame(Color::WHITE);

// 执行渲染命令（相同的 RenderCommand API）
gpu.execute_command(&RenderCommand::FillRect {
    rect: Rect::new(0, 0, 800, 600),
    color: Color::BACKGROUND,
});

gpu.end_frame();
```

**WGSL 着色器**在运行时从内嵌的着色器源码编译。GPU 路径支持：
- 实例化矩形渲染（批量填充/描边）
- 圆角着色器
- 渐变填充计算着色器
- 用于字形的纹理图集采样
- 通过多重采样渲染目标实现的抗锯齿

---

## 自动后端选择

`RenderScene::compose_to_config_auto` 自动选择最佳后端：

```rust
let scene = RenderScene::new();
// ... 用命令填充层 ...

let config = SoftwareRenderConfig::default();

// 自动选择 GPU（如果可用），否则回退到 CPU 软件
let result = scene.compose_to_config_auto(&config);

// 检查使用了哪个后端
match last_auto_render_backend() {
    AutoRenderBackend::GpuWgpu => println!("使用 GPU 渲染"),
    AutoRenderBackend::CpuSoftware => println!("使用 CPU 软件渲染"),
}
```

自动选择路径：
1. 如果启用了 `feature = "gpu-wgpu"` 并且 GPU 适配器可用 → 使用 `WgpuRenderer`
2. 否则 → 使用 `SoftwarePaintBackend`

---

## `PaintBackend` 特质

所有渲染后端实现的策略特质：

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

**实现自定义后端** — 例如，PDF 导出器或无头测试后端：

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

## `RenderScene` — Z 轴排序的层

`RenderScene` 持有多个 `SceneLayer` 对象，每个对象在特定 z-index 上包含一个有序的 `RenderCommand` 列表：

```rust
use rust_widgets::render::{RenderScene, SceneLayer, PaintBackend};

let mut scene = RenderScene::new();

// 背景层 (z=0)
let mut bg_layer = SceneLayer::new(0);
bg_layer.push(RenderCommand::FillRect {
    rect: Rect::new(0, 0, 800, 600),
    color: Color::from_rgb(240, 240, 240),
});
scene.add_layer(bg_layer);

// 内容层 (z=10)
let mut content_layer = SceneLayer::new(10);
content_layer.push(RenderCommand::DrawText {
    origin: Point::new(20, 20),
    text: "Hello".into(),
    font: Font::simple("Arial", 16.0),
    color: Color::BLACK,
    alignment: HorizontalAlignment::Left,
});
scene.add_layer(content_layer);

// 覆盖层 (z=100)
let mut overlay_layer = SceneLayer::new(100);
overlay_layer.push(RenderCommand::FillRoundedRectAA {
    rect: Rect::new(300, 200, 200, 100),
    radius: 12,
    color: Color::rgba(0, 0, 0, 180),
});
scene.add_layer(overlay_layer);

// 将所有层合成到后端
scene.compose(&mut backend);
```

各层在合成前按 `z_index` 排序。窗口部件的 `z` 属性映射到 `SceneLayer::z_index`。

---

## 批量渲染

`BatchRenderer` 特质和 `BatchCommand` 系统支持录制和高效重放绘制命令：

```rust
use rust_widgets::render::{BatchCommand, BatchRenderer, BatchId};

fn render_cache(batcher: &mut impl BatchRenderer) -> Result<(), BatchError> {
    // 录制一个批次
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

    // 重放该批次（由 GPU 或 CPU 后端高效重放）
    batcher.replay(batch_id);

    // 不再需要时销毁
    batcher.destroy_batch(batch_id);
    Ok(())
}
```

`BatchCommand` 的变体包括：`FillRect`、`StrokeRect`、`DrawLine`、`DrawImage`、`DrawImageSubrect`、`DrawText`、`PushClip`、`PopClip`、`Translate`、`SetOpacity`。

批量处理对于静态内容或频繁重用的 UI 元素（网格线、背景、图标）尤其有效。

---

## SVG 导出

使用 `SvgPaintBackend` 将任意窗口部件树渲染为 SVG 文件：

```rust
use rust_widgets::render::SvgPaintBackend;

let mut svg = SvgPaintBackend::new(Size::new(800, 600));

svg.begin_frame(Color::TRANSPARENT);

// 渲染一个按钮
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

// 写入 SVG 文件
std::fs::write("button.svg", svg.to_svg_string())
    .expect("写入 SVG 失败");
```

**将整个窗口部件导出为 SVG：**

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

## 富文本 — `TextSpan` 和 `TextStyle`

在单次渲染调用中组合具有多种字体、颜色和格式的样式化文本块：

```rust
use rust_widgets::render::{RichText, TextSpan, TextStyle};

let mut rich = RichText::new();

rich.add_span(TextSpan {
    text: "加粗 ".into(),
    style: TextStyle {
        font_family: "Arial".into(),
        font_size: 16.0,
        color: Color::BLACK,
        bold: true,
        ..Default::default()
    },
});

rich.add_span(TextSpan {
    text: "红色斜体".into(),
    style: TextStyle {
        font_family: "Arial".into(),
        font_size: 16.0,
        color: Color::from_rgb(255, 0, 0),
        italic: true,
        ..Default::default()
    },
});

// 测量整个文本块
let metrics = rich.measure(&shaper);
println!("富文本: {} × {} px", metrics.width, metrics.height);

// 依次渲染每个片段
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

**`TextStyle` 字段：** `font_family`、`font_size`、`color`、`bold`、`italic`、`underline`、`strikethrough`。

---

## 文本溢出处理

三种溢出模式控制文本超出容器时的行为：

```rust
use rust_widgets::render::{TextOverflow, apply_text_overflow, TextClamp, apply_text_clamp};

// 裁剪：文本在边界处直接截断
let clipped = apply_text_overflow("超长文本...", 100.0, font_size, TextOverflow::Clip);

// 省略号：截断文本以"..."结尾
let ellipsis = apply_text_overflow("超长文本...", 100.0, font_size, TextOverflow::Ellipsis);

// 渐隐：不透明度向溢出边缘逐渐降低
let faded = apply_text_overflow("超长文本...", 100.0, font_size, TextOverflow::Fade);

// 多行限制（最多 N 行）
let clamped = apply_text_clamp(
    "跨越多行的长段落文本...",
    200.0,     // 最大宽度
    font_size,
    TextClamp::Lines(3),  // 最多 3 行，溢出时使用省略号
);
```

---

## 文本塑形

`TextShaper` 特质抽象了特定字体的字形布局：

```rust
use rust_widgets::render::{TextShaper, SimpleTextShaper, ShapedGlyphRun};

let shaper = SimpleTextShaper::new();

let width = shaper.measure_width("Hello, World!", 16.0);
let height = shaper.measure_height("Hello, World!", 16.0);

// 获取详细的字形位置
let runs: Vec<ShapedGlyphRun> = shaper.shape("Hello", 16.0);
for run in &runs {
    for (glyph_id, (x, y)) in run.glyph_ids.iter().zip(run.positions.iter()) {
        println!("Glyph {} at ({}, {})", glyph_id, x, y);
    }
}
```

`SimpleTextShaper` 近似计算度量（每字符 0.6 × font_size，1.2 行高）。在生产环境中，应替换为基于 HarfBuzz 的塑形器以获得准确的字距和连字。

---

## Unicode 字素聚类

`GraphemeProcessor` 处理复杂的 Unicode 序列，实现正确的光标移动和文本选择：

```rust
use rust_widgets::render::{GraphemeCluster, GraphemeProcessor};

let text = "Hello 👨‍👩‍👧‍👦 World! é";
let clusters: Vec<GraphemeCluster> = GraphemeProcessor::split_graphemes(text);

for cluster in &clusters {
    println!("'{}' — {} 个字符, ~{:.1}px 宽",
        cluster.content, cluster.char_count, cluster.width);
}

// 输出：
// 'H' — 1 个字符, ~8.4px 宽
// 'e' — 1 个字符, ~8.4px 宽
// ...
// '👨‍👩‍👧‍👦' — 7 个字符, ~8.4px 宽  (ZWJ 家庭表情 — 一个聚类!)
```

**识别的序列：**
- 基础字符 + 组合标记（é = e + ́）
- 表情符号 + 肤色/发型修饰符
- ZWJ（零宽连字）多表情符号序列
- 区域指示符对（🇺🇸 国旗）

---

## 渐变系统

三种渐变类型，支持色标插值：

```rust
use rust_widgets::render::Gradient;
use rust_widgets::core::{Color, Point};

// 线性渐变：从左到右渐变
let linear = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
    .add_stop(0.0, Color::RED)
    .add_stop(0.5, Color::GREEN)
    .add_stop(1.0, Color::BLUE);

// 径向渐变：从中心向外
let radial = Gradient::radial(Point::new(50, 50), 100.0)
    .add_stop(0.0, Color::WHITE)
    .add_stop(1.0, Color::BLACK);

// 锥形渐变：角度扫描
let conic = Gradient::conic(Point::new(50, 50), 0.0) // 角度以弧度为单位
    .add_stop(0.0, Color::RED)
    .add_stop(0.33, Color::GREEN)
    .add_stop(0.66, Color::BLUE)
    .add_stop(1.0, Color::RED);  // 环绕

// 在任意位置插值颜色
let mid_color = linear.interpolate(0.5);  // RED 和 GREEN 之间的中点

// 反转渐变
let reversed = linear.reverse();

// 构建器模式
use rust_widgets::render::GradientBuilder;
let gradient = GradientBuilder::linear(Point::new(0, 0), Point::new(200, 0))
    .stop(0.0, Color::rgba(255, 0, 0, 255))
    .stop(0.5, Color::rgba(0, 255, 0, 128))
    .stop(1.0, Color::rgba(0, 0, 255, 255))
    .build();

// 应用于 RenderCommand
backend.execute_command(&RenderCommand::DrawGradient {
    rect: Rect::new(0, 0, 200, 100),
    gradient: &gradient,
});
```

---

## 混合模式 — 16 种模式

`BlendMode` 控制绘制命令如何与现有像素合成：

```rust
pub enum BlendMode {
    Normal, Multiply, Screen, Overlay,
    Darken, Lighten, ColorDodge, ColorBurn,
    HardLight, SoftLight, Difference, Exclusion,
    Hue, Saturation, Color, Luminosity,
}
```

```rust
// 通过命令应用混合模式
scene_layer.push(RenderCommand::BlendCommand {
    mode: BlendMode::Multiply,
});

// 在软件后端中，blend_pixel() 应用该模式
use rust_widgets::render::blend_pixel;
let blended = blend_pixel(src_rgba, dst_rgba, BlendMode::Overlay);
```

---

## 投影模式

用于演示/投影显示（在 `feature = "projection"` 后启用）：

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

// 布局会适应投影表面，同时保持宽高比
let adjusted_rect = helper.adjust_widget_rect(widget_rect);
```

---

## 质量管理 — 自适应渲染

`AdaptiveRenderer` 动态调整质量以满足帧预算：

```rust
#[cfg(feature = "quality-management")]
use rust_widgets::render::quality::{QualityLevel, set_quality_level, current_quality_level};

// 手动质量控制
set_quality_level(QualityLevel::High);   // AA 8×, 完整效果
set_quality_level(QualityLevel::Medium); // AA 4×, 简化的阴影
set_quality_level(QualityLevel::Low);    // AA 1×, 无阴影

// 查询当前指标
let fps = current_fps();
let frame_time = average_frame_time();

// 自适应模式：根据帧时间自动调整
// 如果 frame_time > 16ms → 降低质量
// 如果 frame_time < 8ms  → 提高质量
```

---

## 常见模式

### 按钮渲染方法

```rust
impl MyButton {
    fn render(&self, scene: &mut RenderScene) {
        let mut layer = SceneLayer::new(self.z_index);

        // 背景
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

        // 标签文本（居中）
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

### 复合后端管道

```rust
fn render_frame(widget_tree: &WidgetTree, size: Size) -> Vec<u8> {
    let mut scene = RenderScene::new();

    // 遍历窗口部件树，每个部件向场景添加命令
    widget_tree.render(&mut scene);

    // 为当前帧创建软件后端
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    backend.begin_frame(Color::WHITE);

    // 合成场景
    scene.compose(&mut backend);

    backend.end_frame();

    backend.frame_rgba().to_vec()
}
```

### 动画渐变背景

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

### 表单窗口部件的 SVG 导出

```rust
fn export_form_to_svg() -> String {
    let mut svg = SvgPaintBackend::new(Size::new(400, 300));

    svg.begin_frame(Color::WHITE);

    // 表单标题
    svg.execute_command(&RenderCommand::DrawText {
        origin: Point::new(20, 20),
        text: "登录".into(),
        font: Font::bold("Arial", 18.0),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });

    // 输入字段边框
    svg.execute_command(&RenderCommand::DrawRoundedRectStrokeAA {
        rect: Rect::new(20, 60, 360, 36),
        radius: 4,
        color: Color::from_rgb(158, 158, 158),
        width: 1,
    });

    // 提交按钮
    svg.execute_command(&RenderCommand::FillRoundedRectAA {
        rect: Rect::new(20, 120, 120, 40),
        radius: 6,
        color: Color::from_rgb(33, 150, 243),
    });

    svg.end_frame();
    svg.to_svg_string()
}
```

### UI 标签的文本截断

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
