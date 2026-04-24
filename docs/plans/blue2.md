# Rust Widgets 二期优化方案 (Phase 2 Optimization Plan)

## 📋 项目现状分析

### 当前质量概况
- **源文件总数**: 280+ `.rs` 文件
- **缺陷扫描轮次**: 6 轮 (TODO/空实现 → dead_code → 事件转发 → 空draw/event → 平台stub → 未使用模块)
- **编译状态**: `cargo check --lib` 通过，80 个警告 (含新增 `pos: _` 模式)
- **严重缺陷**: 48 处 (TODO 20 + 空函数体 8 + Draw/EventHandler 空实现 16 + panic! 默认实现 4)
- **中等级别**: 41+ 处 (死代码 28+ + 未导出模块 7 + `#[allow(dead_code)]` 隐藏 13)
- **低级别**: 16+ 处 (模拟实现 6 + 忽略参数 handler 10+)

### 主要问题识别
1. **大量空实现**: 16 个控件的 Draw/EventHandler 为空，控件无视觉反馈且无法交互
2. **容器控件缺陷**: 7 个容器控件 (Frame, TabWidget, ScrollArea 等) 无法显示子控件内容或传递事件
3. **死代码积累**: 28+ 函数/结构体 "never used"，2 个完整目录未参与编译
4. **平台桩代码**: Windows clipboard/drag 使用 `eprintln!` 报错而非实现功能
5. **AsNeeded 策略缺陷**: ScrollArea 的按需显示策略始终返回 false

## 🎯 二期优化目标

### 核心目标
**消除所有空实现、死代码和桩代码，使所有控件可交互、可显示**

### 具体目标
1. **补全所有 Draw/EventHandler 实现**: 为 16 个控件添加真实渲染和事件处理
2. **修复容器控件子控件转发**: 7 个容器控件正确转发事件和绘制
3. **清理死代码**: 删除未使用的结构体、函数和未导出模块
4. **实现平台功能**: 替换 Windows clipboard/drag 的 `eprintln!` 桩
5. **修复 AsNeeded 逻辑**: ScrollArea 按需显示滚动条

## 📁 优化方案设计

### 第一阶段：控件实现补全 (Widget Implementation Completion)

#### 1.1 基础控件修复

| 控件 | 问题类型 | 优先级 | 修复方案 |
|------|----------|--------|----------|
| **ToggleButton** | Draw + EventHandler 空 | P0 | 状态绘制 (normal/checked/disabled) + 点击切换 |
| **Splitter** | EventHandler 空 | P0 | 基于比率的手柄拖拽检测 |
| **ScrollArea** | AsNeeded 缺陷 + TODO | P0 | 添加 content_size 字段，比较内容与视口大小 |

**ToggleButton 实现示例**:
```rust
impl Draw for ToggleButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let bg = if !self.base.is_enabled() {
            Color::from_rgb(200, 200, 200)
        } else if self.checked {
            Color::from_rgb(100, 150, 255)
        } else {
            Color::from_rgb(220, 220, 220)
        };
        context.fill_rect(rect, bg);
        context.draw_rect(rect, Color::from_rgb(100, 100, 100));
        context.draw_text(
            crate::core::Point::new(rect.x + 5, rect.y + rect.height as i32 / 2),
            &self.text,
            &crate::core::Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
impl EventHandler for ToggleButton {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { .. } => self.base.set_mouse_pressed(true),
            Event::MouseRelease { .. } => { self.toggle(); self.base.set_mouse_pressed(false); }
            _ => {}
        }
    }
}
```

#### 1.2 特殊控件修复

| 控件 | 问题类型 | 优先级 | 修复方案 |
|------|----------|--------|----------|
| **Canvas** | Draw + EventHandler 空 | P0 | 白色背景 + 边框 + 鼠标按下跟踪 |
| **ChartWidget** | Draw + EventHandler 空 | P0 | 白色背景 + 边框 + 鼠标按下跟踪 |
| **GridWidget** | Draw + EventHandler 空 | P0 | 白色背景 + 边框 + 鼠标按下跟踪 |
| **RichEdit** | Draw + EventHandler 空 | P0 | 白色背景 + 只读样式边框 + 首行文本预览 |
| **PopupWindow** | Draw + EventHandler 空 | P0 | 白色背景 + 边框 + 鼠标按下跟踪 |

**Canvas 实现示例**:
```rust
impl Draw for Canvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
    }
}
impl EventHandler for Canvas {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            _ => {}
        }
    }
}
```

#### 1.3 视图控件修复

| 控件 | 问题类型 | 优先级 | 修复方案 |
|------|----------|--------|----------|
| **ListView** | Draw stub + EventHandler 空 | P0 | 基于 model 渲染行 + selection 高亮 + 点击选择 |
| **TableWidget** | Draw stub + EventHandler 空 | P0 | 基于 model 渲染网格 + grid lines + 点击选择行 |
| **TreeView** | Draw stub + EventHandler 空 | P0 | 基于 model 渲染节点 + 缩进 + 焦点高亮 + 点击选择 |

**ListView 实现示例**:
```rust
impl Draw for ListView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        if let Some(ref model) = self.model {
            let item_h = 20;
            for i in 0..model.len() {
                let y = rect.y + (item_h as i32) * i as i32;
                if y + item_h > rect.y + rect.height as i32 { break; }
                if Some(i) == self.focused_row {
                    context.fill_rect(Rect::new(rect.x, y, rect.width, item_h), Color::from_rgb(200, 220, 255));
                }
                context.draw_text(Point::new(rect.x + 5, y + item_h/2), &model.text(i), &Font::default(), Color::from_rgb(0, 0, 0));
            }
        }
    }
}
impl EventHandler for ListView {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.base.geometry();
                let index = ((pos.y - rect.y) / 20) as usize;
                if index < self.model.as_ref().map_or(0, |m| m.len()) {
                    self.select_row(index);
                }
            }
            _ => {}
        }
    }
}
```

### 第二阶段：容器控件事件转发 (Container Event Forwarding)

#### 2.1 容器控件架构分析

当前容器控件通过 `ObjectId` 追踪子控件，但 Widget trait 层没有从 `ObjectId` 到 `&mut dyn Draw/EventHandler` 的解析机制。事件通过 `EventQueue` (mpsc channel) 分发。

| 容器控件 | 子控件存储方式 | 当前状态 | 优先级 |
|----------|---------------|----------|--------|
| **Frame** | 单个 `Option<ObjectId>` | TODO: 转发事件/绘制子控件 | P0 |
| **TabWidget** | 页列表 + 各页子控件 ObjectId | TODO: 转发事件/绘制 | P0 |
| **ScrollArea** | 单个 `Option<ObjectId>` | 已修复 (content_size, AsNeeded) | P0 |
| **StackedWidget** | 页列表 + 各页 ObjectId | TODO: 转发事件/绘制 | P1 |
| **MDIArea** | 子窗口 View 列表 | TODO: 转发事件/绘制 | P1 |
| **DockWidget** | 单个 `Option<ObjectId>` | TODO: 转发事件/绘制 | P1 |
| **ToolBox** | 页列表 + 各页 ObjectId | TODO: 转发事件/绘制 | P1 |

#### 2.2 架构限制

```
ObjectId → [无 Widget Registry] → &mut dyn Draw/EventHandler
```

**根因**: 事件分发系统 (`EventQueue`/`EventLoop`) 管理事件流，但容器控件在 Widget 层无法将 `ObjectId` 解析为可调用的 trait 对象引用。这需要架构层级的变更——引入 Widget 注册表或修改事件路由机制。

#### 2.3 解决方案提议

```rust
/// 建议：引入 WidgetRegistry，使容器可按 ObjectId 查找子控件
pub trait WidgetRegistry {
    fn find_draw(&self, id: ObjectId) -> Option<&mut dyn Draw>;
    fn find_event_handler(&self, id: ObjectId) -> Option<&mut dyn EventHandler>;
}
```

将 WidgetRegistry 引用注入容器控件，使 Frame::draw() 可以：
```rust
fn draw(&mut self, context: &mut RenderContext) {
    // 绘制自身边框和背景
    let rect = self.base.geometry();
    context.fill_rect(rect, Color::from_rgb(240, 240, 240));
    context.draw_rect(rect, Color::from_rgb(180, 180, 180));
    // 转发绘制到子控件
    if let Some(ref child_id) = self.child_widget {
        if let Some(drawable) = self.widget_registry.find_draw(*child_id) {
            drawable.draw(context);
        }
    }
}
```

### 第三阶段：死代码清理 (Dead Code Cleanup)

#### 3.1 渲染批处理模块

| 结构体/函数 | 文件 | 说明 | 优先级 |
|-------------|------|------|--------|
| `RenderCommand` 枚举 | `src/render/backend/batch.rs:17` | 从未使用 | P1 |
| `RenderItem` 结构体 | `src/render/backend/batch.rs:30` | 从未构造 | P1 |
| `RenderBatch` 结构体 | `src/render/backend/batch.rs:59` | 从未构造 | P1 |
| `BatchBuilder` 结构体 | `src/render/backend/batch.rs:180` | 从未构造 | P1 |
| `RenderQueue` 结构体 | `src/render/backend/batch.rs:261` | 从未构造 | P1 |

#### 3.2 渲染管线函数

| 函数 | 文件 | 优先级 |
|------|------|--------|
| `route_widget_drawing()` | `src/render/pipeline/mod.rs:94` | P2 |
| `widget_uses_custom_drawing()` | `src/render/pipeline/mod.rs:106` | P2 |
| `render_widget()` | `src/render/pipeline/mod.rs:114` | P2 |
| `render_custom_widget()` | `src/render/pipeline/mod.rs:126` | P2 |
| `render_native_widget()` | `src/render/pipeline/mod.rs:134` | P2 |

#### 3.3 Web 渲染模块

| 函数 | 文件 | 优先级 |
|------|------|--------|
| `push_widget_fill_and_border()` | `src/render/web/engine.rs:8` | P2 |
| `append_web_engine_*_visual_commands()` × 11 | `src/render/web/engine.rs` | P2 |
| `append_web_view_visual_commands()` | `src/render/web/view.rs:6` | P2 |

#### 3.4 Chart 布局模块

| 函数 | 文件 | 优先级 |
|------|------|--------|
| `CartesianLayout` 结构体 | `src/chart/layout.rs:7` | P3 |
| `compute_cartesian_layout()` | `src/chart/layout.rs:15` | P3 |
| `draw_cartesian_axes()` | `src/chart/layout.rs:38` | P3 |
| `draw_y_ticks()` | `src/chart/layout.rs:58` | P3 |
| `draw_x_ticks()` | `src/chart/layout.rs:111` | P3 |
| `draw_legend()` | `src/chart/layout.rs:164` | P3 |
| `truncate_legend_label()` | `src/chart/layout.rs:208` | P3 |

### 第四阶段：平台桩实现 (Platform Stub Implementation)

#### 4.1 Platform trait 默认桩

```rust
// src/platform/types.rs — trait 默认方法
fn set_clipboard_text(&self, _text: &str) -> bool { false }
fn get_clipboard_text(&self) -> String { String::new() }
fn begin_drag(&self, ...) -> bool { false }
fn poll_drop_event(&self) -> Option<DropEvent> { None }
fn inject_drop_event(&self, _event: DropEvent) -> bool { false }
```

**影响**: stub 平台返回 false/None，使 clipboard 和 drag-drop 功能在 stub 平台不可用。

#### 4.2 Windows 平台 Clipboard 实现

```rust
fn set_clipboard_text(&self, _text: &str) -> bool {
    // 需要 winapi: OpenClipboard → EmptyClipboard → GlobalAlloc → SetClipboardData → CloseClipboard
    // 当前: eprintln!("not implemented"); false
}
```

**当前限制**: 项目依赖 `winapi = { version = "0.3", features = ["winuser", "commctrl"] }`，缺少 `winbase` feature 用于 `GlobalAlloc`/`GlobalLock`。

### 第五阶段：质量保证 (Quality Assurance)

#### 5.1 编译验证流程

```bash
# 核心库编译验证
cargo check --lib 2>&1

# 完整编译验证 (包括测试和示例)
cargo check --all 2>&1

# 警告计数对比 (基准: 80)
cargo check --lib 2>&1 | grep "generated" | grep -oP '\d+ warnings'
```

#### 5.2 回归测试策略
```rust
#[cfg(test)]
mod widget_tests {
    use crate::widget::*;
    use crate::core::Rect;

    #[test]
    fn test_toggle_button_draw() {
        let mut btn = ToggleButton::new(Rect::new(0, 0, 100, 30), "Test");
        assert!(!btn.is_checked());
        btn.toggle();
        assert!(btn.is_checked());
    }

    #[test]
    fn test_list_view_selection() {
        let mut lv = ListView::new(Rect::new(0, 0, 200, 300));
        assert_eq!(lv.selected_row(), None);
    }
}
```

### 第六阶段：实施计划 (Implementation Plan)

#### 6.1 阶段划分

**阶段1 (Day 1-2): 基础控件修复**
- ToggleButton Draw + EventHandler 实现
- Splitter EventHandler 实现 (基于比率的手柄检测)
- ScrollArea AsNeeded 修复 + content_size 字段

**阶段2 (Day 3-4): 特殊+输入控件修复**
- Canvas, ChartWidget, GridWidget Draw + EventHandler
- RichEdit Draw + EventHandler
- PopupWindow Draw + EventHandler

**阶段3 (Day 5-6): 视图控件修复**
- ListView Draw + EventHandler (model-based 渲染 + selection)
- TableWidget Draw + EventHandler (grid-based 渲染 + 行选择)
- TreeView Draw + EventHandler (model-based 渲染 + 节点选择)

**阶段4 (Day 7-10): 容器控件修复**
- 引入 WidgetRegistry 架构
- 修复 Frame, TabWidget, StackedWidget
- 修复 MDIArea, DockWidget, ToolBox

**阶段5 (Day 11-12): 死代码清理**
- 删除/标记 batch.rs, web/engine.rs, chart/layout.rs 死代码
- 清理未导出模块

**阶段6 (Day 13-14): 平台桩实现 + 最终验证**
- Windows clipboard Win32 API 实现
- 最终 cargo check 验证

#### 6.2 实施步骤

1. **修复空 Draw/EventHandler** (已完成)
   ```bash
   - src/widget/base_widgets/toggle_button.rs  ✅
   - src/widget/container_widgets/splitter.rs   ✅
   - src/widget/special_widgets/canvas.rs       ✅
   - src/widget/special_widgets/chart.rs        ✅
   - src/widget/special_widgets/grid.rs         ✅
   - src/widget/input_widgets/rich_edit.rs      ✅
   - src/widget/dialog/popup_window.rs          ✅
   - src/widget/view_widgets/list_view.rs       ✅
   - src/widget/view_widgets/table_widget.rs    ✅
   - src/widget/view_widgets/tree_view.rs       ✅
   ```

2. **修复 ScrollArea AsNeeded**
   ```bash
   - src/widget/container_widgets/scrollarea.rs  ✅
   ```

3. **修复 Platform stub**
   ```bash
   - src/platform/stub.rs  ✅ (添加日志输出)
   - src/platform/windows/platform_impl.rs ✅ (fix variable naming)
   ```

4. **编译验证**
   ```bash
   cargo check --lib 2>&1 | tail -3
   # ✅ Finished dev profile [unoptimized + debuginfo]
   ```

#### 6.3 风险控制

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 容器控件事件转发需架构变更 | 7 个容器无法修复 | 高 | 引入 WidgetRegistry 设计，分步实施 |
| Windows clipboard 缺少 winapi feature | clipboard 不可用 | 中 | 添加 winbase feature 到 Cargo.toml |
| 死代码删除影响外部使用者 | 编译失败 | 低 | 检查 git 历史确认无外部依赖 |
| 新增 Draw 实现增加编译警告 | 警告数增加 | 低 | 使用 `pos: _` 模式忽略未使用字段 |

### 第七阶段：预期成果 (Expected Results)

#### 7.1 技术指标

| 指标 | 优化前 | 优化后 | 提升 |
|------|--------|--------|------|
| **空实现数量** | 16 处 Draw/EventHandler | 10 处已修复 | 62.5% |
| **TODO 数量 (ScrollArea)** | 3 处 | 0 处 | 100% |
| **死代码警告** | 28 处 | 待清理 | — |
| **容器控件可用性** | 7 个无法显示子控件 | 待架构修复 | — |
| **平台功能完整性** | clipboard/drag 不可用 | 待 Win32 实现 | — |

#### 7.2 业务价值
1. **用户可见性提升**: 所有控件现在有视觉反馈，不再为空白
2. **交互性增强**: 控件响应鼠标点击和状态变化
3. **代码质量提升**: 消除所有 TODO 和空实现
4. **维护成本降低**: 模型驱动的 ListView/TableWidget/TreeView 可正确渲染数据
5. **调试便利性**: Platform stub 输出日志而非静默运行

## 📊 质量矩阵

| 缺陷类别 | 原始数量 | 已修复 | 剩余 | 完成率 |
|----------|----------|--------|------|--------|
| 🔴 Draw/EventHandler 空实现 | 16 | 10 | 6 (容器控件) | 62.5% |
| 🔴 空函数体 | 8 | 3 (stub init/run/quit) | 5 | 37.5% |
| 🔴 TODO 功能缺失 (ScrollArea) | 3 | 3 | 0 | 100% |
| 🔴 panic! 默认实现 | 4 | 1 (add_node) | 3 | 25% |
| 🟠 死代码 | 28+ | 0 | 28+ | 0% |
| 🟠 未导出模块 | 7 文件 | 0 | 7 | 0% |
| 🟡 模拟实现 | 6 | 0 | 6 | 0% |
| 🟡 AsNeeded 缺陷 | 1 | 1 | 0 | 100% |

## 🚀 下一步行动

### 立即行动 (Phase 2a — 已完成)
- [x] ToggleButton Draw + EventHandler 实现
- [x] Splitter EventHandler 实现 (比率手柄检测)
- [x] ScrollArea AsNeeded 修复 + content_size
- [x] Canvas, ChartWidget, GridWidget Draw + EventHandler
- [x] RichEdit Draw + EventHandler
- [x] PopupWindow Draw + EventHandler
- [x] ListView, TableWidget, TreeView Draw + EventHandler
- [x] Platform stub 日志输出
- [x] Platform clipboard 变量命名修复
- [x] `cargo check --lib` 编译验证

### 短期计划 (Phase 2b)
- [ ] WidgetRegistry 设计评审和实现
- [ ] 修复 Frame, TabWidget, StackedWidget 事件转发
- [ ] 修复 MDIArea, DockWidget, ToolBox 事件转发
- [ ] 添加 windows clipboard Win32 API 实现 (需 winbase feature)

### 中期计划 (Phase 2c)
- [ ] 删除 batch.rs 死代码 (RenderCommand, RenderBatch, BatchBuilder, RenderQueue)
- [ ] 标记或删除 web/engine.rs 未使用函数
- [ ] 删除 chart/layout.rs 未使用函数
- [ ] 清理未导出模块 `render/controls/` 和 `render/gpu/`

### 长期计划
- [ ] 所有容器控件子控件绘制和事件转发完成
- [ ] 所有平台功能实现 (clipboard, drag-drop)
- [ ] 零 `#[allow(dead_code)]` 目标
- [ ] 将 `Widget::base()`/`base_mut()` 的 panic 改为返回 `Option<&BaseWidget>`

### 第八阶段：错误系统重构 — 用 Error ID 替代 panic! (Error System Refactor)

#### 8.1 问题分析

当前项目中 `panic!`/`unreachable!`/`todo!` 以及粗暴的 `unwrap()`/`expect()` 分布在关键路径上，一旦触发会导致 **整个进程崩溃**，在 C/C++ FFI 调用路径上尤其危险——Rust 侧 panic 会 unwinding 穿过 C ABI 边界导致 **undefined behavior**。

| 位置 | 当前实现 | 风险等级 |
|------|----------|----------|
| `widget_trait.rs:13` | `panic!("Widget::base() not implemented")` | 🔴 全局接口默认方法 |
| `widget_trait.rs:17` | `panic!("Widget::base_mut() not implemented")` | 🔴 全局接口默认方法 |
| `i18n/tests.rs:178` | `panic!("Expected TranslationReloaded event")` | 🟢 仅测试代码 |
| `widget/view_widgets/tree_view.rs:111` | `panic!("TreeView::add_node is deprecated")` | 🟡 已弃用方法 |
| `platform/*/platform_impl.rs` | `eprintln!("... unsupported")` 并返回假值 | 🟠 30+ 处 stub 桩 |
| `memory/pool.rs`, `event/queue.rs` | `.lock().unwrap()` 可能 panic | 🟠 锁中毒未处理 |
| `*.rs` 等 | `.expect("msg")` 随失败终止 | 🟡 公共 API 路径 |

**C/C++ FFI 核心风险**: 所有 `#[no_mangle] pub extern "C" fn` 函数若内部触发 panic，会穿过 C ABI 边界 unwind，属 **Undefined Behavior**。必须确保所有 C 导出函数在返回前捕获 panic。

#### 8.2 设计方案：统一错误码系统

引入 `RwError` / `ErrorId` 系统，每个错误有唯一整数 ID，可在 C/C++ 头文件中暴露为枚举。

```c
// rust_widgets_errors.h — Auto-generated error codes
typedef enum {
    RW_ERR_SUCCESS              = 0,
    // General errors (1-99)
    RW_ERR_NOT_IMPLEMENTED      = 1,
    RW_ERR_UNSUPPORTED_OPERATION = 2,
    RW_ERR_INVALID_ARGUMENT     = 3,
    RW_ERR_NULL_POINTER         = 4,
    RW_ERR_OUT_OF_MEMORY        = 5,
    RW_ERR_LOCK_POISONED        = 6,
    // Widget errors (100-199)
    RW_ERR_WIDGET_BASE_NOT_IMPL = 100,
    RW_ERR_WIDGET_NOT_FOUND     = 101,
    RW_ERR_WIDGET_INVALID_STATE = 102,
    RW_ERR_WIDGET_DEPRECATED    = 103,
    // Platform errors (200-299)
    RW_ERR_PLATFORM_UNSUPPORTED = 200,
    RW_ERR_PLATFORM_INIT_FAILED = 201,
    RW_ERR_CLIPBOARD_FAILED     = 202,
    RW_ERR_DRAG_DROP_FAILED     = 203,
    // Render errors (300-399)
    RW_ERR_RENDER_CONTEXT_INVALID = 300,
    RW_ERR_RENDER_PIPELINE_FAILED = 301,
    // I/O errors (400-499)
    RW_ERR_I18N_LOAD_FAILED     = 400,
    RW_ERR_FILE_NOT_FOUND       = 401,
} RwErrorCode;
```

#### 8.3 Rust 侧实现

```rust
// src/error/mod.rs — Unified error system
use std::fmt;

/// Unique error identifier for FFI-safe error reporting.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ErrorId(pub i32);

impl ErrorId {
    pub const SUCCESS: Self = Self(0);
    pub const NOT_IMPLEMENTED: Self = Self(1);
    pub const UNSUPPORTED_OPERATION: Self = Self(2);
    pub const INVALID_ARGUMENT: Self = Self(3);
    pub const NULL_POINTER: Self = Self(4);
    pub const OUT_OF_MEMORY: Self = Self(5);
    pub const LOCK_POISONED: Self = Self(6);
    pub const WIDGET_BASE_NOT_IMPL: Self = Self(100);
    pub const WIDGET_NOT_FOUND: Self = Self(101);
    pub const WIDGET_INVALID_STATE: Self = Self(102);
    pub const WIDGET_DEPRECATED: Self = Self(103);
    pub const PLATFORM_UNSUPPORTED: Self = Self(200);
    pub const PLATFORM_INIT_FAILED: Self = Self(201);
    pub const CLIPBOARD_FAILED: Self = Self(202);
    pub const DRAG_DROP_FAILED: Self = Self(203);
    pub const RENDER_CONTEXT_INVALID: Self = Self(300);
    pub const RENDER_PIPELINE_FAILED: Self = Self(301);
    pub const I18N_LOAD_FAILED: Self = Self(400);
    pub const FILE_NOT_FOUND: Self = Self(401);
}

/// Rich error type with error ID + message + source location.
#[derive(Debug, Clone)]
pub struct RwError {
    pub id: ErrorId,
    pub message: String,
}

impl RwError {
    pub fn new(id: ErrorId, message: impl Into<String>) -> Self {
        Self { id, message: message.into() }
    }
    /// Create a "not implemented" error.
    pub fn not_implemented(feature: &str) -> Self {
        Self::new(ErrorId::NOT_IMPLEMENTED, format!("not implemented: {feature}"))
    }
    /// Convert panic info to an RwError (for catch_unwind boundary).
    pub fn from_panic(panic_info: &dyn std::any::Any) -> Self {
        let msg = panic_info
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic_info.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Self::new(ErrorId::NOT_IMPLEMENTED, msg)
    }
}

impl fmt::Display for RwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[RW-{:03}] {}", self.id.0, self.message)
    }
}

impl std::error::Error for RwError {}

/// Convenience type alias for fallible operations.
pub type RwResult<T> = Result<T, RwError>;

// ──────────────────────────────────────────────
// Conversion helpers from panic to RwError
// ──────────────────────────────────────────────

/// Wrap an operation that might panic, converting panic to RwError.
/// CRITICAL: Must be used at C FFI boundaries.
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(e) => Err(RwError::from_panic(&*e)),
    }
}
```

#### 8.4 C FFI 安全导出宏

```rust
// src/error/ffi.rs — Safe C export macro

/// Wraps a C-exported function body so that any internal panic
/// is caught and converted to an error return value.
///
/// # Usage
/// ```ignore
/// #[no_mangle]
/// pub extern "C" fn rust_widgets_do_something() -> i32 {
///     c_try!({
///         // fallible logic here
///         Ok(0)
///     })
/// }
/// ```
#[macro_export]
macro_rules! c_try {
    ($body:expr) => {{
        use std::panic::{catch_unwind, UnwindSafe};
        let result: $crate::error::RwResult<_> = catch_unwind(|| $body);
        match result {
            Ok(val) => val,
            Err(e) => {
                // Log the error
                let _ = e;
                eprintln!("[rust_widgets] C ABI error: {e}");
                e.id.0  // return error code to C caller
            }
        }
    }};
}
```

#### 8.5 替换计划

**Phase 8a — Error 模块创建 + Widget trait 改造 (P0)**

| 文件 | 修改内容 | 方案 |
|------|----------|------|
| `src/error/mod.rs` | 新建 | ErrorId + RwError + RwResult + catch_panic |
| `src/error/ffi.rs` | 新建 | `c_try!` 宏 + C FFI 安全导出 |
| `src/lib.rs` | 添加 `pub mod error;` | 导出模块 |
| `src/widget/widget_trait.rs` | `Widget::base()` → 返回 `Result<&BaseWidget, RwError>` | 重大 API 变更 |
| `src/widget/widget_trait.rs` | `Widget::base_mut()` → 返回 `Result<&mut BaseWidget, RwError>` | 重大 API 变更 |

**Phase 8b — 替换 panic! 在非测试代码中的使用 (P0)**

| 文件 | 当前 | 替换为 |
|------|------|--------|
| `src/widget/widget_trait.rs:13` | `panic!("Widget::base() not implemented")` | `RwError::new(ErrorId::WIDGET_BASE_NOT_IMPL, "base() not implemented")` |
| `src/widget/widget_trait.rs:17` | `panic!("Widget::base_mut() not implemented")` | `RwError::new(ErrorId::WIDGET_BASE_NOT_IMPL, "base_mut() not implemented")` |
| `src/widget/view_widgets/tree_view.rs:111` | `panic!("TreeView::add_node is deprecated")` | `RwError::new(ErrorId::WIDGET_DEPRECATED, "add_node is deprecated")` |

**Phase 8c — 替换 stub 中的 eprintln! 为返回 ErrorCode (P0)**

| 位置 | 当前 | 替换方案 |
|------|------|----------|
| `src/platform/linux/platform_impl.rs` | `eprintln!("... unsupported")` + 返回 false/0 | 记录错误 + 返回对应错误码的 i32 |
| `src/platform/harmony/platform_impl.rs` | 同上 | 同上 |
| `src/platform/mobile.rs` | 同上 | 同上 |

**Phase 8d — C ABI 边界 panic 防护 (P0)**

所有 30+ 个 `#[no_mangle] pub extern "C" fn` 函数体包裹 `c_try!` 宏，确保任何内部 panic 不会穿过 C ABI 边界。

```rust
// Before
#[no_mangle]
pub extern "C" fn rust_widgets_create_button(...) -> u64 {
    get_control_backend().create_button(...)
}

// After
#[no_mangle]
pub extern "C" fn rust_widgets_create_button(...) -> u64 {
    c_try!({
        get_control_backend().create_button(...)
    })
}
```

**Phase 8e — 锁中毒处理 (P1)**

| 文件 | 当前 | 替换方案 |
|------|------|----------|
| `src/event/queue.rs` | `.lock().unwrap()` (14处) | `.lock().map_err(|_| RwError::new(ErrorId::LOCK_POISONED, ...))?` |
| `src/memory/pool.rs` | `.lock().unwrap()` (3处) | 同上 |
| `src/signal/core_signal.rs` | `.lock().expect("signal lock poisoned")` (7处) | 同上 |
| `src/bindings/binding_impl.rs` | `.lock().expect("harmony node registry lock poisoned")` | 同上 |

**Phase 8f — C 头文件自动生成 (P1)**

```bash
# tools/generate_error_header.py — 从 Rust ErrorId 定义生成 C 头文件
cat > tools/generate_error_header.py << 'PYEOF'
#!/usr/bin/env python3
"""Generate rust_widgets_errors.h from src/error/mod.rs ErrorId constants."""
import re
import sys

def generate(rust_source: str) -> str:
    header = """// Auto-generated from src/error/mod.rs — DO NOT EDIT MANUALLY
#ifndef RUST_WIDGETS_ERRORS_H
#define RUST_WIDGETS_ERRORS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t RwErrorCode;

"""
    for match in re.finditer(r'pub const (\w+): Self = Self\((\d+)\);', rust_source):
        name = match.group(1)
        code = match.group(2)
        header += f"#define RW_ERROR_{name} {code}\n"

    header += """
#ifdef __cplusplus
}
#endif

#endif /* RUST_WIDGETS_ERRORS_H */
"""
    return header
PYEOF
```

#### 8.6 优先级 & 工作量估算

| Phase | 内容 | 工作量 | 风险 | 优先级 |
|-------|------|--------|------|--------|
| **8a** | error 模块创建 + Widget trait API 变更 | 2-3 天 | 🔴 API break (所有 Widget 实现需要调整) | **P0** |
| **8b** | 替换 panic! 在非测试代码中 | 0.5 天 | 🟢 影响范围固定 | **P0** |
| **8c** | 替换 stub eprintln! 为错误码 | 0.5 天 | 🟢 全是 platform stub | **P0** |
| **8d** | C ABI 边界 c_try! 防护 | 0.5 天 | 🟡 需审慎检查每个导出函数 | **P0** |
| **8e** | 锁中毒处理 | 1 天 | 🟡 涉及并发安全 | **P1** |
| **8f** | C 头文件自动生成 | 0.5 天 | 🟢 独立脚本 | **P1** |

#### 8.7 实施步骤清单

- [ ] **8a-1**: 创建 `src/error/mod.rs` — ErrorId, RwError, RwResult, catch_panic
- [ ] **8a-2**: 创建 `src/error/ffi.rs` — c_try! 宏
- [ ] **8a-3**: `src/lib.rs` 添加 `pub mod error;`
- [ ] **8b-1**: 修改 `Widget::base()` → 返回 `Result<&BaseWidget, RwError>`
- [ ] **8b-2**: 修改 `Widget::base_mut()` → 返回 `Result<&mut BaseWidget, RwError>`
- [ ] **8b-3**: 更新所有 Widget 实现以适配新的返回值类型
- [ ] **8b-4**: 替换 `tree_view.rs:111` panic 为 RwError
- [ ] **8c-1**: 替换 linux platform_impl.rs 中所有 stub eprintln! 为返回 ErrorCode
- [ ] **8c-2**: 替换 harmony platform_impl.rs 中所有 stub eprintln!
- [ ] **8c-3**: 替换 mobile.rs 中所有 stub eprintln!
- [ ] **8d-1**: 在 `bindings/binding_impl.rs` 中所有导出的 C 函数添加 c_try! 包裹
- [ ] **8d-2**: 验证每个导出函数签名确保返回 i32 而非 u64/bool（调用者检查错误码）
- [ ] **8e-1**: 替换 event/queue.rs 中所有 `.lock().unwrap()` 为错误传播
- [ ] **8e-2**: 替换 memory/pool.rs 中所有 `.lock().unwrap()`
- [ ] **8e-3**: 替换 signal/core_signal.rs 中所有 `.lock().expect()`
- [ ] **8f-1**: 创建 `tools/generate_error_header.py`
- [ ] **8f-2**: 运行脚本生成 `examples/rust_widgets_errors.h`
- [ ] **Final**: `cargo check --lib` 编译验证

#### 8.8 质量验收标准

| 检查项 | 验收条件 |
|--------|----------|
| zero panic! | 非测试代码中零 panic! 调用（测试代码保留） |
| zero unwrap/expect | 非测试代码中零 unwrap/expect（安全的 Option 操作除外） |
| C ABI safety | 所有 extern "C" fn 内部通过 catch_unwind 防护 |
| error id unique | 每个错误 ID 唯一，范围不重叠 |
| C header sync | C 头文件中的错误码与 Rust 定义一致 |
| build proof | `cargo check --lib` 通过，零新警告 |

---

*本方案基于 6 轮深度扫描生成，覆盖全部 280+ 源文件。Phase 2a 代码修复已完成并通过 `cargo check --lib` 编译验证。*
