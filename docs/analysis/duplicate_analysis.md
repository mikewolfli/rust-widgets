# 重复模块与结构分析报告

> 生成日期: 2026-04-24
> 扫描范围: 全部 `src/` 源码
> 分析方法: 结构对比 + 类型签名对比 + 功能语义分析

---

## 汇总

| 级别 | 发现 | 建议 |
|------|------|------|
| 🔴 **严重重复** | 4 处 | 直接合并或删除 |
| 🟡 **部分重叠** | 3 处 | 统一到一个模块 |
| 🔵 **代码一致性** | 2 处 | 合并内部定义 |

---

## 🔴 严重重复

### D1. `widget::container_widgets::splitter::Splitter` × `layout::splitter::SplitterLayout`

**问题**: 两个完全独立的 splitter 实现, 具有相同的核心逻辑 (panes + ratios + orientation)。

| 属性 | `widget::container_widgets::splitter::Splitter` | `layout::splitter::SplitterLayout` |
|------|------|------|
| panes | `Vec<ObjectId>` | `Vec<ObjectId>` |
| ratios | `Vec<f32>` | `Vec<f32>` |
| orientation | `SplitterOrientation` | `Orientation` |
| 方法 | `add_pane/remove_pane/set_ratio/set_ratios/normalize_ratios` | `add_widget/remove_widget/set_ratio` |
| update | 内置信号触发 | 通过 `Layout` trait |
| 行数 | ~200 行 | ~70 行 |

**分析**: 二者本质相同, `Splitter` 是 Widget 封装版, `SplitterLayout` 是纯布局算法版。`Splitter` 内部重新实现了 `SplitterLayout` 的空间分配逻辑。

**建议**: `Splitter::draw()` 中的空间计算应委托给 `SplitterLayout`, 移除重复的比例分配逻辑。

---

### D2. `action::Action` × `widget::menu_toolbar::action::Action` (同名不同义)

**问题**: 存在两个 `Action` 类型, 都支持 `checkable`/`checked`/`triggered`/`toggled`。

| 属性 | `action::Action` | `widget::menu_toolbar::action::Action` |
|------|------|------|
| 位置 | `src/action/types.rs` | `src/widget/menu_toolbar/action.rs` |
| 用途 | 命令/快捷键系统 | 菜单和工具栏的视觉 Action |
| 信号 | `triggered`/`toggled`/`enabled_changed` | `triggered`/`toggled`/`hovered`/`changed` |
| 支持 shortcut | ❌ 无 (由 `ActionManager` 管理) | ✅ `set_shortcut()` |
| 支持 icon | ❌ | ✅ `set_icon_text()` |

**分析**: 两个 Action 在语义上重叠度达 70%+。`widget::menu_toolbar::Action` 是视觉版, 继承自 `BaseWidget`; `action::Action` 是纯数据模型版。但重复了 `checkable/checked/triggered/toggled` 等全部机制。

**建议**:
- 让 `widget::menu_toolbar::Action` **内部持有** `action::Action` 作为数据模型, 或
- 让 `widget::menu_toolbar::Action` 直接继承自 `action::Action + BaseWidget`, 消除重复的 checkable/checked 逻辑

---

### D3. `display_widgets::Orientation` (3 次定义) + `container_widgets::toolbox::Orientation` + `layout::Orientation`

**问题**: `Orientation` 枚举在 5 个文件中独立定义:

| 位置 | 值 |
|------|----|
| `layout/mod.rs:61` | `Horizontal`, `Vertical` |
| `widget/display_widgets/slider.rs:29` | `Horizontal`, `Vertical` |
| `widget/display_widgets/progressbar.rs:21` | `Horizontal`, `Vertical` |
| `widget/display_widgets/scrollbar.rs:25` | `Horizontal`, `Vertical` |
| `widget/container_widgets/toolbox.rs:26` | `Horizontal`, `Vertical` |

**分析**: 所有定义完全一致 (`Horizontal`/`Vertical`), 但各自独立。这导致:
- 无法统一匹配 pattern match
- 每个 Orientation 需要单独的转换函数
- 跨模块操作需额外转换层

**建议**: 统一到 `core` 模块 `pub use geometry::Orientation;`, 或 `layout::Orientation` 成为唯一来源。

---

### D4. `gpu::GpuCapability` × `quality::GpuCapability` (结构完全重复)

| 属性 | `quality::GpuCapability` | `gpu::adapter::...` 中的等效信息 |
|------|------|------|
| `supports_high_quality: bool` | ✅ 在 `quality/gpu.rs` | ✅ 间接通过 `GpuDeviceType` 可推导 |
| `is_integrated: bool` | ✅ | ✅ 就是 `GpuDeviceType::IntegratedGpu` |
| `performance_tier: u8` | ✅ | ✅ 在 `GpuManager` 中通过 `device_type.performance_tier()` |

**分析**: `quality::GpuCapability` 和 `gpu` 模块中的硬件探测功能高度重叠。`gpu::GpuManager` 内部创建 `QualityManager` 并传入 `GpuCapability`。实际上 quality 模块的 `GpuCapability` 完全可以通过 gpu 模块的 `AdapterInfo` 推导出来。

**建议**: 将 `quality::GpuCapability` 标记为 deprecated, 统一使用 `gpu` 模块的 `AdapterInfo.device_type` 来推导 GPU 能力。

---

## 🟡 部分重叠

### O1. `widget::view_widgets::TableView` × `TableWidget`

**问题**: `TableView` 是 `TableWidget` 的薄包装:

```rust
pub struct TableView {
    table: TableWidget,  // 内部仅持有一个 TableWidget
}
impl TableView {
    pub fn new(geometry: Rect) -> Self { Self { table: TableWidget::new(geometry) } }
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) { self.table.set_model(model); }
    pub fn row_count(&self) -> usize { self.table.row_count() }
    // ... 其余方法均直接委托到 self.table
}
```

**分析**: `TableView` 没有增加任何新的行为, 只是暴露了 `TableWidget` 的一个子集 API。`TableView` 的 `header()` 方法返回 `Some(format!("Column {}", col + 1))` 与 `TableWidget` 的模型行为不一致(绕过 model)。

**建议**: 删除 `TableView`, 让用户直接使用 `TableWidget`; 或者让 `TableView` 只增加 `set_model` 的简化绑定而不重新实现每个方法。

---

### O2. `event::Event` + `action::Action` + `shortcut::Shortcut` 三者的事件/触发机制

**问题**: 三个模块都涉及"触发一个操作":
- `event::Event` — 键盘/鼠标的低级事件
- `action::Action` — 高级命令, 触发 `triggered` signal
- `shortcut::ShortcutManager` — 监听 `Key`+`Modifiers`, 触发 `shortcut_triggered`

但三者之间**没有直接连接**:
- `action::ActionManager` 内部存储 `shortcut_to_action: HashMap<String, String>`, 但它用 `normalize_shortcut(&str)` 处理, 不依赖 `shortcut::Shortcut` 类型
- `shortcut::ShortcutManager` 有自己的 `handle_key_event(Key, Modifiers)` 但不知道 `action::Action`
- `action::Action` 有自己的触发机制, 不依赖 `shortcut::Shortcut`

**分析**: 快捷键的链路应该是 `event → shortcut → action`, 但目前 event 和 shortcut 之间是手动桥接的, action 和 shortcut 也是通过字符串 action_id 松散耦合。

**建议**: 
- `action::ActionManager` 应提供方法接受 `shortcut::Shortcut` 类型(而非仅 `&str`)
- `shortcut::Shortcut` 应直接触发 `action::Action`
- 考虑移除 `action::ActionManager` 内部的自建 shortcut 映射, 统一使用 `shortcut::ShortcutManager`

---

### ~~O3. `performance::DirtyRegionTracker` × `render::backend::scene::SceneLayer`~~ ✅ 完成

**解决**: 
- 创建 `src/core/rect_merge.rs` — 包含 `merge_intersecting_rects()` 和 `bounding_rect()` 
- `DirtyRegionTracker::merge()` 改为委托 `merge_intersecting_rects()`
- `DirtyRegionTracker::get_bounding_rect()` 改为委托 `bounding_rect()`
- `RenderBatch::merge_adjacent_rects()` 改为按渲染属性分组后使用 `merge_intersecting_rects()`
- `cargo check --all`: 0 errors

---

## 🔵 代码一致性问题

### C1. `style::Padding` × `style::EdgeInsets` — 结构完全相同的两个类型

```rust
pub struct Padding {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}
pub struct EdgeInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}
```

**问题**: `Padding` 和 `EdgeInsets` 在同一个模块 `style/mod.rs` 中定义, 四个字段名称和类型完全一致, 只是名称不同。这在概念上让 API 使用者困惑: 何时用 Padding? 何时用 EdgeInsets?

**建议**: 合并为一个, 建议保留 `Padding`, 删除 `EdgeInsets`。`EdgeInsets::all()` 可以迁移到 `Padding::all()`。

---

### C2. `widget::display_widgets::Orientation` (3 个独立定义) × `layout::Orientation` — 相同字段但无法互相 match

参见 D3, 每个 widget 的 `Orientation` 是独立类型, 无法在 `match` 中与 `layout::Orientation` 统一处理。

---

## 📋 行动计划

| 优先级 | 项目 | 工作量 | 影响范围 | 风险 |
|--------|------|--------|----------|------|
| ~~P0~~ | ~~合并 `Orientation` 到 `core` (5个文件)~~ | ✅ 完成 | 6个文件 | ✅ `cargo check` 通过 |
| ~~P0~~ | ~~删除 `TableView` (薄包装)~~ | ✅ 完成 | 3个文件 | ✅ 类型别名, 向后兼容 |
| ~~P0~~ | ~~合并 `style::Padding` + `EdgeInsets`~~ | ✅ 完成 | 1个文件 | ✅ `to_padding()` 替换 `to_insets()` |
| ~~P1~~ | ~~统一 `quality::GpuCapability` 到 `gpu` 模块~~ | ✅ 完成 | quality + gpu 模块 | ✅ 添加deprecated标注 |
| ~~P1~~ | ~~`Splitter` 委托到 `SplitterLayout`~~ | ✅ 完成 | container_widgets + layout | ✅ `SplitterLayout`增强API |
| ~~P2~~ | ~~统一 `widget::menu_toolbar::Action` × `action::Action`~~ | ✅ 完成 | widget.Action 内含 action::Action | `cargo check` 通过 |
| ~~P2~~ | ~~打通 event → shortcut → action 链路~~ | ✅ 完成 | ActionRouter 桥接 ShortcutManager ↔ ActionManager | `cargo check` 通过 |
| ~~P3~~ | ~~DirtyRegionTracker × RenderBatch 矩形合并重复~~ | ✅ 完成 | 提取到 `core::rect_merge` | `cargo check` 通过 |

---

## 详细文件级重复统计

| 文件 | 行数 | 重复类型 | 重复程度 |
|------|------|----------|----------|
| `widget/display_widgets/slider.rs` | ~250 | `Orientation` 重复定义 | 100% 相同 |
| `widget/display_widgets/scrollbar.rs` | ~200 | `Orientation` 重复定义 | 100% 相同 |
| `widget/display_widgets/progressbar.rs` | ~200 | `Orientation` 重复定义 | 100% 相同 |
| `widget/container_widgets/toolbox.rs` | ~150 | `Orientation` 重复定义 | 100% 相同 |
| `widget/container_widgets/splitter.rs` | ~200 | Splitter 逻辑 | ~70% 与 SplitterLayout 重叠 |
| `widget/menu_toolbar/action.rs` | ~200 | Action 机制 | ~70% 与 action::Action 重叠 |
| `widget/view_widgets/table_view.rs` | ~150 | TableView | 100% 委托给 TableWidget |
| `style/mod.rs` | ~206 | Padding/EdgeInsets | 结构完全重复 |
| `quality/gpu.rs` | ~50 | GpuCapability | 功能与 gpu 模块重叠 |
