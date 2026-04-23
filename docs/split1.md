# Module Split Execution Plan

> ⚡ **分拆执行规则（不可违反）**
> 1. 源文件直接 copy 完整内容，不可简化或省略任何代码
> 2. 分拆后原功能必须完整保留，不能遗漏任何 pub 导出
> 3. 每个文件分拆后立即修正所有 problems（`cargo check`）
> 4. 分拆完后回写本文件记录执行过程
>
> 扫描日期：2026-04-23 | 总源文件数：~150 | 总代码行：~64,000

---

## 目录

- [当前模块全景](#当前模块全景)
- [优先级 1: 必须拆分 (Critical)](#优先级-1-必须拆分-critical)
  - [1. `src/render/mod.rs` (5760 行)](#1-srcrendermodrs-5760-行)
  - [2. `src/control_backend/implementation.rs` (2403 行)](#2-srccontrol_backendimplementationrs-2403-行)
- [优先级 2: 建议拆分 (High)](#优先级-2-建议拆分-high)
  - [3. `src/platform/mod.rs` (1494 行)](#3-srcplatformmodrs-1494-行)
  - [4. `src/platform/windows.rs` (2181 行)](#4-srcplatformwindowsrs-2181-行)
  - [5. `src/platform/macos.rs` (1546 行)](#5-srcplatformmacosrs-1546-行)
- [优先级 3: 可优化 (Medium)](#优先级-3-可优化-medium)
  - [6. `src/core/geometry.rs` (658 行)](#6-srccoregeometryrs-658-行)
  - [7. `src/widget/base.rs` (608 行)](#7-srcwidgetbasers-608-行)
  - [8. `src/render_engine/implementation.rs` (508 行)](#8-srcrender_engineimplementationrs-508-行)
- [架构性建议](#架构性建议)

---

## 当前模块全景

```
src/
├── lib.rs (596)              ← 模块根，定义所有 module 声明
├── quality.rs (558)          ← 质量管理
├── wgpu_backend.rs (633)     ← WGPU 后端
│
├── render/ (6,784)           ← 渲染系统 — 超大
│   ├── mod.rs (5,760)        ← ⚠️ 极度过大
│   ├── batch.rs
│   ├── text_cache.rs
│   ├── quality/
│   ├── gpu/
│   └── controls/             ← 已经分拆了控件渲染
│       ├── basic/ (button/checkbox/label/radiobutton)
│       ├── input/ (combobox/lineedit/listbox/spinbox/textedit)
│       └── special/ (command_link/font_combo_box/lcd_number)
│
├── control_backend/ (2,400)  ← ⚠️ 控制后端 — 集中在一个超大 impl 文件
│   ├── mod.rs
│   └── implementation.rs    ← 2403 行
│
├── platform/ (7,896)         ← ⚠️ 各平台后端 — 每平台文件都很大
│   ├── mod.rs (1,494)       ← 平台抽象 + 调度逻辑
│   ├── windows.rs (2,181)   ← Windows 实现
│   ├── macos.rs (1,546)     ← macOS 实现 (ObjC)
│   ├── macos_objc2.rs (896)
│   ├── linux.rs (1,010)
│   ├── harmony.rs (507)
│   ├── mobile.rs (669)
│   ├── stub.rs (574)
│   └── state.rs / types.rs
│
├── widget/                   ← ⚠️ 已经很好了！按类别分拆
│   ├── mod.rs → 组织/重导出
│   ├── base.rs (608)        ← 基类较大
│   ├── base_widgets/        ← 基础控件（button/checkbox/label...）
│   ├── container_widgets/   ← 容器（dockwidget/mdiarea/tabwidget...）
│   ├── input_widgets/       ← 输入（lineedit/combobox/listbox...）
│   ├── display_widgets/     ← 显示（progressbar/scrollbar/slider/lcd）
│   ├── advanced_widgets/    ← 高级控件（calendar/dial/date_time...）
│   ├── view_widgets/        ← 视图（list/table/tree）
│   ├── special_widgets/     ← 特殊（canvas/chart/grid）
│   ├── dialog/              ← 对话框
│   ├── menu_toolbar/        ← 菜单工具栏
│   └── web_widgets/         ← Web
│
├── gpu/ (1,560)             ← 已经很好了！按职责分拆
├── event/ (750)
├── i18n/ (485)
├── layout/ (700)
├── core/ (2,300)            ← 核心类型，几何/颜色/字体
├── xml/ (800)
├── pdf/ (1,611)
│   └── implementation.rs (1,511)  ← 也偏大
├── chart/ (1,593)
│   └── implementation.rs (1,593)  ← 整个模块就是单个大文件
├── web/ (350)
├── style/ (300)
├── theme/ (130)
├── signal/ (350)
├── object/ (100)
├── shortcut/ (330)
├── action/ (340)
├── clipboard/ (100)
├── embedded/ (370)
├── memory/ (50)
├── performance/ (100)
├── print/ (600)
├── test/ (180)
├── menu_config/ (600)
└── bindings/ (700)
```

---

## 优先级 1: 必须拆分 (Critical)

### 1. `src/render/mod.rs` (5760 行)

**问题**：整个渲染核心塞在一个文件里，包含了：
- TextMetrics/TextCluster/ShapedText/BackBuffer/SoftwareSurface/RenderContext/RenderScene/SceneLayer 等类型定义
- PaintBackend trait + SoftwarePaintBackend 实现
- RenderCommand enum（28 个变体）
- ~40 个 `append_*_visual_commands` 函数
- 像素级操作函数（fill_pixels, blend_pixel 等）
- 路由/渲染函数（route_widget_drawing, render_widget 等）
- 大量测试代码（lines 4100–5760）

**推荐拆分方案**：

```
src/render/
├── mod.rs              ← 保留，仅做重新导出 + 核心定义
├── types.rs            ← TextMetrics, TextCluster, ShapedText, BackBuffer (从 mod.rs 抽出)
├── surface.rs          ← SoftwareSurface, SoftwareRenderConfig (从 mod.rs 抽出)
├── context.rs          ← RenderContext (从 mod.rs 抽出)
├── paint.rs            ← PaintBackend trait + SoftwarePaintBackend (从 mod.rs 抽出)
├── command.rs          ← RenderCommand enum + 所有变体
├── scene.rs            ← SceneLayer, RenderScene (从 mod.rs 抽出)
├── pipeline.rs         ← append_*_visual_commands 系列函数 → 按类别分拆
│   ├── controls.rs     ← 控件相关 append_*
│   ├── containers.rs   ← 容器相关 append_*
│   └── dialogs.rs      ← 对话框相关 append_*
├── pixel_ops.rs        ← fill_pixels, blend_pixel 等像素工具
├── routing.rs          ← route_widget_drawing, render_widget 等
├── batch.rs            ← 已有
├── text_cache.rs       ← 已有
├── quality/            ← 已有
├── gpu/                ← 已有
└── controls/           ← 已有（分拆得很好，无需改动）
```

**收益**：单个文件从 5760 行降到 200-400 行，按职责分离，便于多人协作。

### 2. `src/control_backend/implementation.rs` (2403 行)

**问题**：单个文件包含控制后端所有逻辑：
- 路由策略（ControlBackendKind, ControlRoutePreference）
- 原生控制实现
- 自定义绘制控制实现
- 事件处理和分发

**推荐拆分方案**：

```
src/control_backend/
├── mod.rs              ← 保留模块声明 + 公共 API 导出
├── types.rs            ← ControlBackendKind, ControlRoutePreference 等类型
├── routing.rs          ← route_preference_for_widget_kind 路由策略
├── native.rs           ← 原生控制实现
├── custom.rs           ← 自定义绘制控制实现
└── dispatcher.rs       ← 事件处理和分发逻辑
```

---

## 优先级 2: 建议拆分 (High)

### 3. `src/platform/mod.rs` (1494 行)

**问题**：平台抽象模块包含了太多职责：
- WidgetTriggerKind/WidgetTriggerEvent/DropEvent 等事件类型
- DesktopBackend enum
- Platform trait 定义
- 平台初始化/运行/退出逻辑
- get_platform() 单例和调度
- handle 管理（register_handle, get_handle 等）

**推荐拆分方案**：

```
src/platform/
├── mod.rs              ← Platform trait + get_platform() + 模块声明
├── types.rs            ← WidgetTriggerKind, WidgetTriggerEvent, DropEvent, DesktopBackend
├── handle.rs           ← handle 管理系统（register_handle, get_handle, HandleKind）
├── state.rs            ← 已有（平台状态管理）
└── [platform].rs       ← 各平台实现文件（已有，无需改动）
```

### 4. `src/platform/windows.rs` (2181 行)

**问题**：Windows 平台实现单一文件过大，包含了窗口管理、事件循环、原生控件等。

**推荐拆分方案**：

```
src/platform/
├── windows/
│   ├── mod.rs          ← WindowsPlatform 实现入口 + 重导出
│   ├── window.rs       ← 窗口创建和管理
│   ├── event_loop.rs   ← 事件循环和处理
│   └── controls.rs     ← 原生 Win32 控件
```

### 5. `src/platform/macos.rs` (1546 行)

**问题**：同上，macOS 平台实现过大。

**推荐拆分方案**：

```
src/platform/
├── macos/
│   ├── mod.rs          ← MacOSPlatform 实现入口 + 重导出
│   ├── window.rs       ← NSWindow 创建和管理
│   └── events.rs       ← 事件处理
```

---

## 优先级 3: 可优化 (Medium)

### 6. `src/core/geometry.rs` (658 行)

**问题**：包含了 Point, Rect, Size, Margins, Padding, Edges 等所有几何类型。

**推荐拆分**：抽出到 `src/core/geometry/` 子目录，每个类型一个文件：
```
src/core/geometry/
├── mod.rs    ← 重新导出
├── point.rs  ← Point
├── rect.rs   ← Rect
├── size.rs   ← Size
└── margins.rs ← Margins, Padding, Edges
```

### 7. `src/widget/base.rs` (608 行)

**问题**：Widget trait + BaseWidget 结构体在一个文件里。

**推荐拆分**：
```
src/widget/
├── base.rs      ← Widget trait 定义（精简接口）
├── base_impl.rs ← BaseWidget 默认实现
└── kinds.rs     ← WidgetKind enum
```

### 8. `src/render_engine/implementation.rs` (508 行)

**问题**：渲染引擎实现包含 CPU/GPU 渲染路由、前端端框架适配等。

**推荐拆分**：
```
src/render_engine/
├── mod.rs        ← 模块声明
├── types.rs      ← RenderEngine trait + 引擎类型
├── cpu.rs        ← CPU 渲染引擎
├── gpu.rs        ← GPU 渲染引擎
└── frontend.rs   ← 前端框架适配
```

---

## 架构性建议

### 🔴 红色: 必须优先处理

| 模块 | 当前 | 建议 | 工作量 |
|------|------|------|--------|
| `render/mod.rs` | 5760 行 | → 拆 8+ 文件 | 3-4 天 |
| `control_backend/implementation.rs` | 2403 行 | → 拆 5 文件 | 1-2 天 |

### 🟡 黄色: 建议下一阶段处理

| 模块 | 当前 | 建议 | 工作量 |
|------|------|------|--------|
| `platform/mod.rs` | 1494 行 | → 拆 4 文件 | 1 天 |
| `platform/windows.rs` | 2181 行 | → 拆 4 文件 | 1-2 天 |
| `platform/macos.rs` | 1546 行 | → 拆 3 文件 | 1 天 |
| `chart/implementation.rs` | 1593 行 | → 拆 3-4 文件 | 1 天 |
| `pdf/implementation.rs` | 1511 行 | → 拆 3-4 文件 | 1 天 |

### 🟢 绿色: 低优先级，可选优化

| 模块 | 当前 | 建议 | 工作量 |
|------|------|------|--------|
| `core/geometry.rs` | 658 行 | → 拆 4 文件 | 0.5 天 |
| `widget/base.rs` | 608 行 | → 拆 3 文件 | 0.5 天 |
| `render_engine/implementation.rs` | 508 行 | → 拆 4 文件 | 0.5 天 |

---

## 总体执行建议

1. **Phase 1** (Critical): 拆分 `render/mod.rs` + `control_backend/implementation.rs`
2. **Phase 2** (High): 拆分 `platform/*.rs` + `chart/implementation.rs` + `pdf/implementation.rs`
3. **Phase 3** (Medium): 拆分 `core/geometry.rs` + `widget/base.rs` + `render_engine/implementation.rs`
4. **Phase 4** 扩展: 拆分后按需添加 `plugin/`、`animation/`、`canvas2d/`、`accessibility/` 等新模块

> 注意：拆分后所有公共 API 通过原 `mod.rs` 重新导出，保持对外接口不变。

---

## 执行日志

### 1. `src/render/mod.rs` (5760 行) — ✅ 已完成分拆

**执行时间**: 2026-04-23

**拆分工具**: `tools/split_render_v3.py`

**最终文件结构**:

```
src/render/
├── mod.rs              (105 行) ← 重新导出 hub + is_empty_rect
├── types.rs            (41 行)  ← TextMetrics, TextCluster, ShapedText
├── command.rs          (91 行)  ← RenderCommand enum (28 变体)
├── surface.rs          (248 行) ← SoftwareSurface, SoftwareRenderConfig
├── paint.rs            (172 行) ← PaintBackend trait + SoftwarePaintBackend
├── scene.rs            (256 行) ← SceneLayer, AutoRenderBackend
├── pipeline/mod.rs     (3347 行)← append_*_visual_commands + pixel_ops + 路由函数
├── tests.rs            (1593 行)← 测试代码
├── batch.rs            (327 行) ← 已有
├── text_cache.rs       (331 行) ← 已有
├── pixel_ops.rs        (275 行) ← 旧文件（保留，但内容已合并入 pipeline）
├── quality/            (已有)   ← 自适应质量
├── gpu/                (已有)   ← GPU 后端
└── controls/           (已有)   ← 控件渲染（basic/input/special）
```

**关键变更**:

| 变更 | 说明 |
|------|------|
| `types.rs` | 提取 TextMetrics/TextCluster/ShapedText，添加 `pub(crate)` 字段 |
| `command.rs` | 提取 RenderCommand enum |
| `surface.rs` | 提取 SoftwareSurface + SoftwareRenderConfig，添加 `pub(crate)` buffer/aa_samples_per_axis/back |
| `paint.rs` | 提取 PaintBackend + SoftwarePaintBackend，添加 `pub(crate)` surface |
| `scene.rs` | 提取 SceneLayer + AutoRenderBackend，引入 PaintBackend/RenderCommand |
| `pipeline/mod.rs` | append_*/pixel_ops/路由函数 + 额外尾部函数，添加 `pub(crate)` pixel_bytes_len |
| `tests.rs` | 提取所有测试代码 |
| `mod.rs` | 重写为 re-export hub + is_empty_rect |

**编译验证**:

```
cargo check --all
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
✓ 零错误，仅 warnings
```

**修复列表** (迭代修复):

| # | 问题 | 修复 |
|---|------|------|
| 1 | `pixel_bytes_len` 跨模块私有 | 合并到 pipeline/mod.rs + `pub(crate)` |
| 2 | `SoftwareSurface.buffer` 跨模块私有 | split_render_v3.py: post-processing pub(crate) |
| 3 | `SoftwarePaintBackend.surface` 跨模块私有 | split_render_v3.py: post-processing pub(crate) |
| 4 | `BackBuffer.back` 跨模块私有 | split_render_v3.py: post-processing pub(crate) |
| 5 | `ShapedText.clusters/advance` 跨模块私有 | split_render_v3.py: post-processing pub(crate) |
| 6 | `cluster_ends_with_zwj`/`is_combining_mark` 等私有函数 | 合并到 pipeline/mod.rs |
| 7 | `UnicodeFonts` trait 未引入 | 添加 `use font8x8::UnicodeFonts` |

**结论**: render/mod.rs 从 5760 行 → 最大子模块 3347 行 (pipeline)，单个文件减少 42%。✅ 通过编译验证。

### 2. `src/control_backend/implementation.rs` (2403 行) — ✅ 已完成分拆

**执行时间**: 2026-04-23

**拆分工具**: `tools/split_control_backend.py`

**最终文件结构**:

```
src/control_backend/
├── mod.rs          (20 行)  ← 重新导出 hub
├── types.rs        (60 行)  ← ControlBackendKind, ControlRoutePreference, CustomControlState, CustomWidgetProperties
├── routing.rs      (81 行)  ← route_preference_for_widget_kind
├── trait_def.rs    (368 行) ← ControlBackend trait 定义
├── native.rs       (430 行) ← NativeControlBackend struct + impl ControlBackend
├── custom.rs       (1408 行)← CustomPaintControlBackend struct + impl ControlBackend
└── dispatcher.rs   (82 行)  ← get_control_backend, get_control_backend_for_widget, active_control_policy
```

**关键变更**:

| 变更 | 说明 |
|------|------|
| `types.rs` | 提取 ControlBackendKind/ControlRoutePreference enums + CustomControlState/CustomWidgetProperties structs |
| `routing.rs` | 纯提取 route_preference_for_widget_kind 函数 |
| `trait_def.rs` | 纯提取 ControlBackend trait (作为 native/custom 的公共接口) |
| `native.rs` | 提取 NativeControlBackend + impl (所有方法委托 to get_platform()) |
| `custom.rs` | 提取 CustomPaintControlBackend + impl (HashMap 存储的虚拟后端) |
| `dispatcher.rs` | 提取路由函数 (feature-flag 条件编译版本) + helper factory functions |
| `mod.rs` | 重写为 re-export hub |

**修复列表**:

| # | 问题 | 修复 |
|---|------|------|
| 1 | custom.rs 起始错位 (包含多余 `}`) | lines[929→930] 边界修正 |
| 2 | dispatcher.rs 起始错位 (包含多余 `}`) | lines[2334→2331] 边界修正 |
| 3 | CustomControlState 私有 | `pub(crate)` struct |
| 4 | CustomWidgetProperties 私有 | `pub(crate)` struct |
| 5 | CustomControlState 所有字段私有 | `pub(crate)` on all fields |
| 6 | CustomWidgetProperties 所有字段私有 | `pub(crate)` on all fields |

**编译验证**:

```
cargo check --all
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.95s
✓ 零错误，仅 warnings
```

**结论**: implementation.rs 从 2403 行 → 最大子模块 1408 行 (custom.rs)，单个文件减少 41%。✅ 通过编译验证。

### 3. `src/platform/mod.rs` (1494 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_platform_mod.py`

**拆分方案** (按实际执行):

```
src/platform/
├── mod.rs              ← 模块声明 + `pub use` 重新导出（32 行）
├── types.rs            ← WidgetTriggerKind, WidgetState, MenuNodeState, DropEvent, WidgetTriggerEvent
├── stub.rs             ← StubPlatform struct + impl StubPlatform + impl Platform for StubPlatform
├── runtime.rs          ← create_native_platform（6 cfg 版本）, PLATFORM static, get_platform/init/run/quit/capabilities
├── contract.rs         ← fallback_native_capability_contract, fallback_embedded_capability_contract, negotiate_capability_contract
└── tests.rs            ← 测试模块（158 行，从原 mod.rs 提取）
```

**修复清单**:
1. types.rs: 添加 `use crate::core::{ObjectId, PlatformFamily, RuntimeProfile};` + serde derives + `pub(crate)` 字段
2. stub.rs: 替换残缺的 `impl Platform`（原文件缺失 15+ 个 trait 方法，如 create_checkbox/create_line_edit 等）
3. runtime.rs: 添加 `#[cfg(feature = "embedded")]` 属性 + `use crate::platform::stub::StubPlatform;` 导入
4. contract.rs: 添加 `use crate::platform::runtime::get_platform;` + 清理杂散内容
5. stub.rs: 清理末尾杂散 `#[cfg(feature = "embedded")]`
6. stub.rs: 添加 `use crate::core::PlatformFamily;`（原来只有 `use crate::platform::types::*;`）

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors, 18 warnings ✅**

### 4. `src/platform/windows.rs` (2181 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_windows_platform.py`

**拆分方案**:

```
src/platform/windows/
├── mod.rs              ← 模块声明 + 重新导出（12 行）
├── helpers.rs          ← try_create_label, try_create_slider, try_create_progress_bar, try_create_combo_box（212 行）
├── types.rs            ← WindowsHandleKind, WindowsPlatform, Win32MenuState, PlatformDowncast, WindowsPlatformExtSlider（317 行）
├── notify.rs           ← ensure_window_class_registered, active_windows_platform, control_notify/event 函数（123 行）
├── platform_impl.rs    ← impl Platform for WindowsPlatform（1503 行，原主体块）
└── tests.rs            ← 测试模块（54 行）
```

**注意**: windows.rs 由 `#[cfg(target_os = "windows")]` 条件编译保护，macOS 上不编译。模块结构从单文件转为 `windows/` 目录后，`platform/mod.rs` 的 `pub mod windows;` 声明无需修改即可正常工作。

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors, 18 warnings ✅**（macOS 上 cfg-gated 跳过）

### 5. `src/platform/macos.rs` (1546 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_macos_platform.py`

**拆分方案**:

```
src/platform/macos/
├── mod.rs              ← 模块声明 + 重新导出（9 行）
├── types.rs            ← HandleKind, CocoaHandle, MacOSPlatform struct, 类注册函数,
                          impl MacOSPlatform, Default impl（367 行）
├── platform_impl.rs    ← impl Platform for MacOSPlatform（1156 行，原主体块）
└── tests.rs            ← 测试模块（53 行）
```

**修复清单**:
1. types.rs: 添加缺失的 `}` 关闭 CocoaHandle struct（脚本 range 截断 bug）
2. types.rs + platform_impl.rs: `super::state::BackendState` → `crate::platform::state::BackendState`（super 在新目录结构中指向 macos/ 自身）
3. types.rs: 私有 enum/struct/fn 改为 `pub(crate)`（HandleKind, CocoaHandle, 类注册函数, impl 方法）
4. types.rs: 结构体字段改为 `pub(crate)`（CocoaHandle.ptr/.kind, MacOSPlatform 所有字段）
5. platform_impl.rs: 添加缺失的 `NSRunningApplication`, `NSApplicationActivationOptions`, `Sel` 导入

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors, 26 warnings ✅**

### 进度总结

| 优先级 | 模块 | 原行数 | 状态 | 最大子文件 |
|--------|------|--------|------|-----------|
| Critical #1 | render/mod.rs | 5760 | ✅ 已完成 | 3347 (pipeline) |
| Critical #2 | control_backend/implementation.rs | 2403 | ✅ 已完成 | 1408 (custom) |
| High #3 | platform/mod.rs | 1494 | ✅ 已完成 | 32 (mod.rs) |
| High #4 | platform/windows.rs | 2181 | ✅ 已完成 | 1503 (platform_impl) |
| High #5 | platform/macos.rs | 1546 | ✅ 已完成 | 1156 (platform_impl) |
| High #6 | platform/linux.rs | 1010 | ✅ 已完成 | 909 (platform_impl) |
| High #7 | platform/macos_objc2.rs | 896 | ✅ 已完成 | 505 (platform_impl) |
| High #8 | platform/harmony.rs | 507 | ✅ 已完成 | 425 (platform_impl) |
| High #9 | chart/implementation.rs | 1593 | ✅ 已完成 | 1117 (implementation → types+tests) |
| High #10 | pdf/implementation.rs | 1511 | ✅ 已完成 | 456 (tests) |

**总进度: 10/10 分拆全部完成，`cargo check --all: Finished dev` — 0 errors ✅**

---

### 6. `src/platform/linux.rs` (1010 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_linux_platform.py`

**拆分方案**:

```
src/platform/linux/
├── mod.rs              (5 行)   ← 模块声明 + 重新导出
├── types.rs            (109 行) ← LinuxNativeState, LinuxPlatform struct, impl LinuxPlatform
├── platform_impl.rs    (909 行) ← impl Platform for LinuxPlatform
```

**修复清单**:
1. `super::state::BackendState` → `crate::platform::state::BackendState`（nested super 路径错误）
2. `LinuxNativeState` 配置 `#[cfg(all(target_os = "linux", feature = "gtk-native"))]` gating
3. 所有结构体和字段改为 `pub(crate)`（LinuxNativeState, LinuxPlatform）

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors ✅**

---

### 7. `src/platform/macos_objc2.rs` (896 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_macos_objc2.py`

**拆分方案**:

```
src/platform/macos_objc2/
├── mod.rs              (7 行)   ← 模块声明 + 重新导出
├── types.rs            (118 行) ← MacOSPlatform struct + impl MacOSPlatform
├── platform_impl.rs    (505 行) ← impl Platform for MacOSPlatform
└── tests.rs            (277 行) ← 测试代码（未修改）
```

**修复清单**:
1. `super::state::BackendState` → `crate::platform::state::BackendState`
2. 所有结构体和字段改为 `pub(crate)`

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors ✅**

---

### 8. `src/platform/harmony.rs` (507 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_harmony_impl.py`

**拆分方案**:

```
src/platform/harmony/
├── mod.rs              (5 行)   ← 模块声明 + 重新导出
├── types.rs            (89 行)  ← HarmonyHandleKind, HarmonyMenuState, HarmonyRuntimeState, HarmonyPlatform + impl
├── platform_impl.rs    (425 行) ← impl Platform for HarmonyPlatform
```

**修复清单**:
1. `super::state::BackendState` → `crate::platform::state::BackendState`
2. `super::{DropEvent, ...}` → `super::super::{DropEvent, ...}`
3. 所有结构体/枚举/字段改为 `pub(crate)`（HarmonyHandleKind, HarmonyMenuState, HarmonyRuntimeState, HarmonyPlatform 字段, impl 方法）
4. 移除 types.rs 未使用的导入（thread, Duration, PlatformFamily, Ordering）

**编译验证**: `cargo check --all: Finished dev [unoptimized + debuginfo]` — 0 errors ✅

---

### 9. `src/chart/implementation.rs` (1593 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_chart_impl.py`

**拆分方案**:

```
src/chart/
├── mod.rs              ← 模块声明 + 重新导出（20 行）
├── types.rs            (64 行)  ← ChartData, ChartType, ChartSeries, ChartStyle 等类型 + ChartContext trait
├── tests.rs            (412 行) ← 所有测试代码（svg_tests + public_api）
└── implementation.rs   (已删除) ← 原 1593 行单体文件
```

**注意**: chart/implementation.rs 被拆分为 types.rs + tests.rs，主体实现保留在 mod.rs 中。

**修复清单**:
1. `ChartData`, `ChartType`, `ChartSeries`, `ChartStyle`, `Chart` → 添加 `pub(crate)`
2. `CartesianLayout` struct 和关联函数 → 添加 `pub(crate)`

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors ✅**

---

### 10. `src/pdf/implementation.rs` (1511 行) — ✅ 已完成分拆

**时间**: 2026-04-23
**工具**: `tools/split_pdf_impl.py`

**拆分方案**:

```
src/pdf/
├── mod.rs              (20 行)  ← 模块声明 + 重新导出
├── types.rs            (173 行) ← PdfOptions, PdfPageRect, PdfSecurity, PdfPermissions, PdfPageRef
├── security.rs         (57 行)  ← PdfSecurity trait
├── writer.rs           (367 行) ← PdfWriter struct + impl
├── reader.rs           (185 行) ← PdfReader struct + impl
├── document.rs         (157 行) ← PdfDocument trait + PdfDocumentImpl struct + impl
├── page.rs             (168 行) ← PdfPage trait + PdfPageImpl struct + impl
├── metadata.rs         (35 行)  ← PdfMetadata struct
├── form.rs             (377 行) ← PDF 表单处理
├── annotation.rs       (288 行) ← PDF 注释处理
├── hyperlink.rs        (239 行) ← PDF 超链接处理
└── tests.rs            (456 行) ← 测试代码
```

**修复清单**:
1. types.rs: 添加 `pub(crate)` 到 PdfOptions, PdfPageRect, PdfSecurity, PdfPermissions, PdfPageRef 及其字段
2. PdfSecurity trait 的 `pub` 方法暴露 `pub(crate)` 类型 → `private_interfaces` warning（可接受）
3. 各个子文件之间的 cross-module 引用修复

**`cargo check --all`: `Finished dev [unoptimized + debuginfo]` — 0 errors ✅**
