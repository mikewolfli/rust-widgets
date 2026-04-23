# 大型模块拆分 - 详细执行指南

## 概述

本指南提供分步骤的拆分指令，用于将超大型 mod.rs 文件分解为专用的实现模块。

---

## 第 1 部分：render/mod.rs 拆分 (6,533 行)

### 目标结构

```
src/render/
├── mod.rs                    (导出中心, ~80 行)
├── text_primitives.rs        (TextMetrics, TextCluster, ShapedText)
├── buffer.rs                 (BackBuffer, SoftwareSurface, Config)
├── context.rs                (RenderContext, PaintBackend trait)
├── scene.rs                  (SceneLayer, RenderScene)
├── backend.rs                (SoftwarePaintBackend)
├── commands.rs               (RenderCommand 枚举)
├── batch.rs                  (已存在)
├── quality.rs                (已存在)
├── text_cache.rs             (已存在)
└── [其他控件模块...]
```

### 步骤 1.1：创建 text_primitives.rs

**源：原 mod.rs 中的行 92-137**

```rust
//! Text measurement and shaping primitives
//! 
//! Contains core text measurement structures and shaping results.

use crate::core::{Font, Size};

/// Text measurement result for width, height, and baseline metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Measured text width in logical pixels.
    pub width: u32,
    /// Measured text height in logical pixels.
    pub height: u32,
    /// Baseline ascent in logical pixels.
    pub ascent: u32,
    /// Baseline descent in logical pixels.
    pub descent: u32,
}

/// One shaped text cluster produced by the render text shaper.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    /// Cluster source text (one or more unicode scalars).
    pub text: String,
    /// Logical horizontal advance in pixels.
    pub advance: f32,
}

/// Shaped text run composed from ordered clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    clusters: Vec<TextCluster>,
    advance: f32,
}

impl ShapedText {
    /// Returns ordered text clusters in this shaped run.
    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }

    /// Returns cluster count in this shaped run.
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Returns total horizontal advance in logical pixels.
    pub fn advance(&self) -> f32 {
        self.advance
    }
}
```

### 步骤 1.2：创建 buffer.rs

**源：原 mod.rs 中的行 138-243**

包含：
- `BackBuffer` 结构体和实现
- `SoftwareSurface` 结构体
- `SoftwareRenderConfig` 结构体和实现
- `set_default_software_render_config()` 函数
- `default_software_render_config()` 函数
- `global_software_render_config()` 静态引用

### 步骤 1.3：创建 context.rs

**源：原 mod.rs 中的行 246-430**

包含：
- `RenderContext<'a>` 结构体
- `PaintBackend` trait 定义
- `RenderContext` impl 块（所有方法）

### 步骤 1.4：创建 commands.rs

**源：原 mod.rs 中的行 599-687**

包含：
- `RenderCommand` 枚举定义

### 步骤 1.5：创建 scene.rs

**源：原 mod.rs 中的行 751-904**

包含：
- `SceneLayer` 结构体和实现
- `RenderScene` 结构体和实现
- `compose_scene_to_surface_software()` 函数
- `GpuRenderError` 枚举（如果启用 gpu-wgpu 特性）
- `compose_scene_to_surface_wgpu()` 函数（如果启用 gpu-wgpu 特性）

### 步骤 1.6：创建 backend.rs

**源：原 mod.rs 中的行 431-598**

包含：
- `SoftwarePaintBackend` 结构体
- `impl SoftwarePaintBackend` 块
- `impl PaintBackend for SoftwarePaintBackend` 块

### 步骤 1.7：更新 mod.rs

保留：
- 顶部注释和坐标系统说明（行 1-79）
- 模块导出声明（行 1-63）
- 所有 `pub use` 导出（导出子模块中的类型）

添加新的模块声明：
```rust
mod text_primitives;
mod buffer;
mod context;
mod scene;
mod backend;
mod commands;

// 重新导出所有公共类型
pub use text_primitives::{TextCluster, TextMetrics, ShapedText};
pub use buffer::{BackBuffer, SoftwareRenderConfig, SoftwareSurface, 
                 default_software_render_config, set_default_software_render_config};
pub use context::{PaintBackend, RenderContext};
pub use scene::{RenderScene, SceneLayer};
pub use backend::SoftwarePaintBackend;
pub use commands::RenderCommand;
```

删除原 mod.rs 中的实现代码（行 85-904），保留：
- 导出/导入语句
- 工具函数（`is_empty_rect()`, `pixel_bytes_len()`, `centered_text_origin()` 等）
- 所有 `append_*_visual_commands()` 函数

### 验证步骤 1

```bash
cd /home/mikeli/workspace/rust-widgets
cargo check --lib
# 应该看到: "Compiling rust_widgets"
# 最后应该看到: "Finished `dev` profile"
```

---

## 第 2 部分：platform/mod.rs 拆分 (1,640 行)

### 目标结构

```
src/platform/
├── mod.rs                    (导出中心, ~200 行)
├── types.rs                  (所有枚举和结构体)
├── stub_platform.rs          (StubPlatform 实现)
├── harmony/                  (已存在)
├── linux/                    (已存在)
├── windows/                  (已存在)
└── macos/                    (已存在)
```

### 步骤 2.1：创建 types.rs

**源：原 mod.rs 中的行 32-140**

包含所有类型定义：
- `WidgetTriggerKind` enum
- `WidgetTriggerEvent` struct
- `DropEvent` struct
- `DesktopBackend` enum
- `MobileBackend` enum
- `PlatformCapabilities` struct
- `NativeCapabilityContract` struct
- `EmbeddedCapabilityContract` struct
- `CapabilityContract` enum
- `impl NativeCapabilityContract`

### 步骤 2.2：创建 stub_platform.rs

**源：原 mod.rs 中的行 466-1366**

包含：
- `StubPlatform` 结构体
- `impl StubPlatform` 块
- `impl Platform for StubPlatform` 块（大部分实现）

### 步骤 2.3：更新 mod.rs

保留：
- 顶部文档注释
- 模块导出声明（harmony, linux, windows 等）
- `Platform` trait 定义（行 154-440）
- `MobilePlatformExtension` trait（行 441-465）
- `RuntimeGuiMode` enum（行 1367+）
- 平台初始化函数

添加新模块声明：
```rust
mod types;
mod stub_platform;

pub use types::*;
pub use stub_platform::StubPlatform;
```

---

## 第 3 部分：pdf/mod.rs 拆分 (1,808 行)

### 目标结构

```
src/pdf/
├── mod.rs                    (导出中心, ~100 行)
├── implementation.rs         (所有实现块)
├── metadata.rs               (元数据结构)
├── annotation.rs             (已存在)
├── form.rs                   (已存在)
├── hyperlink.rs              (已存在)
└── security.rs               (已存在)
```

### 步骤 3.1：创建 metadata.rs

**源：原 mod.rs 中的行 122-186, 704-1260**

包含：
- `PdfMetadata` struct
- `impl Default for PdfMetadata`
- `PdfSecurity` struct
- `impl Default for PdfSecurity`
- `PdfPagination` struct
- `impl Default for PdfPagination`
- `PdfFontResource` struct
- `impl PdfFontResource`
- `ImageEncodingRoute` struct
- `impl ImageEncodingRoute`

### 步骤 3.2：创建 implementation.rs

**源：原 mod.rs 中的行 187-222, 229-654**

包含：
- `PdfWriter` struct
- `impl PdfWriter`
- `impl Default for PdfWriter`
- `PdfReader` struct
- `impl PdfReader`
- `impl Default for PdfReader`
- `PdfDocumentImpl` struct
- `impl PdfDocumentImpl`
- `impl PdfDocument for PdfDocumentImpl`
- `PdfPageImpl` struct
- `impl PdfPageImpl`
- `impl PdfPage for PdfPageImpl`
- `PdfFormField` enum

### 步骤 3.3：更新 mod.rs

保留：
- 顶部坐标系统说明
- 模块导出声明（annotation, form, hyperlink, security）
- `PdfPage` trait 定义（行 32-70）
- `PdfDocument` trait 定义（行 71-121）

添加新模块声明：
```rust
mod implementation;
mod metadata;

pub use implementation::*;
pub use metadata::*;
```

---

## 验证和测试

### 编译验证（每个拆分后执行）

```bash
cargo check --lib
```

### 单元测试验证

```bash
cargo test --lib
```

### 完整构建验证

```bash
cargo build --release
```

### 文档生成验证

```bash
cargo doc --no-deps --open
```

---

## 完成标准

每个拆分完成后，应满足以下条件：

- ✅ 编译无错误：`cargo check --all` 成功
- ✅ 所有测试通过：`cargo test --all` 成功
- ✅ 没有警告（或只有预期的警告）
- ✅ 所有公共类型导出正确
- ✅ 私有实现保持隐藏
- ✅ 向后兼容性：API 不变
- ✅ 文档完整：使用 `///` 和 `//!` 注释

---

## 常见问题

### Q1：如何处理循环导入？
**A：** 使用重导出（pub use）在 mod.rs 中统一导出，避免子模块间直接导入。

### Q2：拆分后文件会不会变得太多？
**A：** 这是标准的模块化设计。Rust 项目普遍使用此模式以提高可维护性。

### Q3：如何验证导出的完整性？
**A：** 在原 mod.rs 中的 `pub use` 语句中，将每个导出添加到新 mod.rs 中。运行 `cargo doc` 生成文档，检查所有类型是否可见。

---

## 时间估计

- render/mod.rs 拆分：30-45 分钟（7 个新文件）
- platform/mod.rs 拆分：15-20 分钟（2 个新文件）
- pdf/mod.rs 拆分：20-25 分钟（2 个新文件）
- 测试和验证：15-20 分钟

**总计：约 80-110 分钟**

---

## Git 提交建议

```bash
# 初始分析
git commit -m "docs: add module refactor analysis and plan"

# 重命名完成
git commit -m "refactor: rename module_impl.rs files for clarity"

# render 拆分
git commit -m "refactor(render): split mod.rs into specialized modules"

# platform 拆分
git commit -m "refactor(platform): split mod.rs into types and implementation"

# pdf 拆分
git commit -m "refactor(pdf): split mod.rs into implementation and metadata"

# 最终验证
git commit -m "test: verify all modules compile and tests pass"
```
