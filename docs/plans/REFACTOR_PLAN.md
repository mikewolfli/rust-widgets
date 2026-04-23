# 模块结构整理完整计划

## 目标
重组项目模块结构，使得每个 mod.rs 文件仅包含：
- 模块声明 (`mod xxx;`)
- 公共接口导出 (`pub use xxx::*;`)
- 架构说明注释

所有实现代码都移至独立的文件中。

---

## 第一阶段：超大型文件拆分（>1000 行）

### 1. render/mod.rs (6,533 行) → 拆分为 5 个文件

#### 1.1 创建 `render/text_engine.rs` (包含行: 92-137)
- TextMetrics
- TextCluster
- ShapedText
- 相关实现

#### 1.2 创建 `render/buffer.rs` (包含行: 138-205)
- BackBuffer
- SoftwareSurface
- SoftwareRenderConfig
- 相关实现 + 全局配置函数

#### 1.3 创建 `render/context.rs` (包含行: 246-430)
- RenderContext
- RenderBackend trait
- 相关实现

#### 1.4 创建 `render/scene.rs` (包含行: 751-904)
- SceneLayer
- RenderScene
- GpuRenderError
- 相关实现

#### 1.5 创建 `render/backend.rs` (包含行: 431-598)
- SoftwarePaintBackend
- PaintBackend trait
- 相关实现

#### 1.6 更新 `render/mod.rs` (保留: 1-79, 207-231, 599-688, 707-750, 1043+)
- 模块导出
- 工具函数 (is_empty_rect, pixel_bytes_len)
- 导出函数 (set_quality_level, current_quality_level 等)
- 所有 append_*_visual_commands 函数 → 可选再拆为 `render/commands.rs`

---

### 2. platform/mod.rs (1,640 行) → 拆分为 3 个文件

#### 2.1 创建 `platform/types.rs` (包含行: 32-140)
- WidgetTriggerKind
- WidgetTriggerEvent
- DropEvent
- DesktopBackend
- MobileBackend
- PlatformCapabilities
- CapabilityContract 及其实现

#### 2.2 创建 `platform/stub_platform.rs` (包含行: 466-1366)
- StubPlatform
- impl Platform for StubPlatform (全部 800+ 行)

#### 2.3 更新 `platform/mod.rs` (保留: 1-31, 141-153, 154-440, 441-465, 1367-1640)
- 模块导出
- Platform trait 定义
- MobilePlatformExtension trait
- RuntimeGuiMode enum
- 平台初始化函数

---

### 3. pdf/mod.rs (1,808 行) → 拆分为 3 个文件

#### 3.1 创建 `pdf/implementation.rs` (包含行: 187-654)
- PdfWriter
- PdfReader
- PdfDocumentImpl
- PdfPageImpl
- PdfFormField
- 相关实现

#### 3.2 创建 `pdf/metadata.rs` (包含行: 122-186)
- PdfMetadata
- PdfSecurity
- PdfPagination
- PdfFontResource
- ImageEncodingRoute
- 相关实现

#### 3.3 更新 `pdf/mod.rs` (保留: 1-31, 32-121)
- 模块导出
- 坐标系统说明
- PdfPage trait
- PdfDocument trait

---

## 第二阶段：大型文件重构（300-1000 行）

### 4. layout/mod.rs (780 行) → 拆分为 2 个文件

#### 4.1 创建 `layout/box_layout.rs`
- BoxLayout
- 布局计算实现

#### 4.2 更新 `layout/mod.rs`
- 保留 Layout trait
- SizePolicy, Orientation, LayoutConstraints
- 导出 BoxLayout

---

### 5. performance/mod.rs (371 行) → 拆分为 2 个文件

#### 5.1 创建 `performance/profiler.rs`
- DirtyRegion, DirtyRegionTracker
- 性能分析逻辑

#### 5.2 更新 `performance/mod.rs`
- 导出为中心
- 导出子模块

---

## 第三阶段：名称规范化

### 6. 重命名 `module_impl.rs` 文件
目标文件：
- `chart/module_impl.rs` → `chart/chart_impl.rs`
- `print/module_impl.rs` → `print/print_impl.rs`
- `xml/module_impl.rs` → `xml/xml_impl.rs`
- `control_backend/module_impl.rs` → `control_backend/backend_impl.rs`
- `render_engine/module_impl.rs` → `render_engine/engine_impl.rs`
- `bindings/module_impl.rs` → `bindings/binding_impl.rs`

---

## 执行顺序

1. 开始拆分 render/mod.rs（最大影响）
2. 拆分 platform/mod.rs
3. 拆分 pdf/mod.rs
4. 拆分 layout/mod.rs
5. 重命名 module_impl.rs 文件
6. 全项目 `cargo check` 验证
7. 运行 `cargo test` 确保功能完整

---

## 最终结果

- 3 个超大型 mod.rs 被拆分为 11 个专用实现文件
- 所有 mod.rs 文件行数 < 100 行
- 清晰的模块化结构，易于维护
- 代码查找和修改更容易定位

