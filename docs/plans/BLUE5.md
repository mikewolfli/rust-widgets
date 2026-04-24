# BLUE5 — 布局检查器与推荐重布局系统设计

> 基于 PUA 质量标准的第五轮设计：布局诊断引擎 + 智能修复推荐  
> 规划日期: 2026-04-27  
> 当前基线: `cargo check --all: Finished dev [unoptimized]` (0 errors, 0 warnings)  
> 当前测试: **357/357 passed (298 unit + 47 integration + 12 doc) — ✅ ALL PASSING**

---

## 1. 动机与目标

### 1.1 为什么需要布局检查器

`rust_widgets` 拥有两套布局系统：

| 系统 | 机制 | 用途 |
|------|------|------|
| **JSON 声明式布局** | `JsonLoader` → `LAYOUT_MAP` 存储 | JSON 加载的 UI |
| **原生布局系统** | `Layout trait` (HBox/VBox/Grid/Stack/Splitter/Form) | 程序化创建的 UI |

两者共享底层 `Layout trait`，但诊断时需要分别收集状态。当前没有任何机制能在布局完成后验证其正确性。常见问题：

- **孤儿控件**：JSON 中标签正确但嵌套错误，控件在注册表中没有 parent
- **空布局**：JSON 声明了 layout 但 children 为空，或原生布局创建后未 `add_widget()`
- **零尺寸**：父容器空间不足，子控件计算后尺寸为 0（导致不可见）
- **重叠**：同一父容器下的两个子控件被分配到重叠区域（stretch 分配不合理）
- **布局失调**：BoxLayout 显示方向与预期不符（期望 horizontal 实际 vertical）

### 1.2 设计目标

| 目标 | 说明 |
|------|------|
| **延迟启用** | 默认关闭，布局完成后按需启用，禁用时零开销（AtomicBool） |
| **双通道检查** | 同时检查 JSON 声明式布局 + 原生布局系统的状态 |
| **可操作推荐** | 发现问题时给出具体的 recalculate / 结构调整建议 |
| **无侵入** | 不需要修改 Layout trait 或任何布局实现，通过包装回调收集数据 |

---

## 2. 架构设计

### 2.1 核心模块

```
src/layout/inspector.rs          ← 检查器主体（新建）
src/layout/mod.rs                ← 添加 pub mod inspector;
src/json/layout.rs               ← 添加 collect_layout_snapshots()（修改）
```

### 2.2 数据流

```
┌──────────────┐     ┌──────────────────────┐     ┌──────────────────┐
│  JsonLoader   │────→│  LAYOUT_MAP (JSON)    │     │  WidgetRegistry   │
└──────────────┘     └──────────┬───────────┘     └────────┬─────────┘
                                │                          │
                                ▼                          ▼
                    ┌──────────────────────────────────────────┐
                    │         LayoutInspector                   │
                    │                                          │
                    │  ┌──────────────────────────────────┐    │
                    │  │  1. check_orphans()              │    │
                    │  │  2. check_empty_layouts()        │    │
                    │  │  3. check_zero_size()            │    │
                    │  │  4. check_overlaps()             │    │
                    │  │  5. check_layout_mismatch()      │    │
                    │  └──────────────────────────────────┘    │
                    │                                          │
                    │  ┌──────────────────────────────────┐    │
                    │  │  Recommendation Engine            │    │
                    │  │  → recalculate()                  │    │
                    │  │  → add_min_size()                 │    │
                    │  │  → fix_json_nesting()             │    │
                    │  │  → adjust_stretch()               │    │
                    │  └──────────────────────────────────┘    │
                    └──────────────────────────────────────────┘
                                │
                                ▼
                    ┌──────────────────────┐
                    │  DiagnosticReport     │
                    │  → Display (终端)    │
                    │  → has_issues()       │
                    │  → has_errors()       │
                    │  → recommendations    │
                    └──────────────────────┘
```

### 2.3 延迟模式设计

检查器使用 `AtomicBool` 全局开关，配合 `thread_local!` 存储快照：

```rust
static ENABLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static GEOMETRY_SNAPSHOT: RefCell<Vec<(ObjectId, Rect)>> = ...;
    static NATIVE_LAYOUTS: RefCell<Vec<NativeLayoutInfo>> = ...;
}

impl LayoutInspector {
    pub fn enable() { ENABLED.store(true, Ordering::Release); }
    pub fn disable() { ENABLED.store(false, Ordering::Release); }
    pub fn is_enabled() -> bool { ENABLED.load(Ordering::Acquire); }

    pub fn run_once(registry: &WidgetRegistry) -> DiagnosticReport {
        if !Self::is_enabled() { return DiagnosticReport::empty(); }
        // ... 收集快照 → 检测 → 生成推荐 → 清理快照
    }
}
```

**零开销保证**：`record_geometry()` 和 `register_native_layout()` 等收集函数第一行都检查 `is_enabled()`，关闭时直接返回，不产生任何分配。

---

## 3. 诊断项详解

### 3.1 P0 — 结构诊断

#### D1: 孤儿控件 (Orphan Widget)

| 属性 | 值 |
|------|----|
| **严重度** | Warning |
| **触发条件** | `WidgetEntry.parent == None && WidgetKind != Window` |
| **检测来源** | `WidgetRegistry::all_ids()` 遍历 |
| **误报风险** | 低 — Window 已排除，暂时的根控件不会误报 |

**典型场景**：
```json
{
    "window": {
        "children": [
            { "button": { "text": "OK" } }   // ✅ 有 parent
        ]
    },
    "button": { "text": "Cancel" }             // ❌ 孤儿 — 不在任何 children 中
}
```

**推荐处理**：
- 🔗 将该控件移入正确的容器 children 列表
- 如果设计上就是顶层控件，确保其类型为 `window`

#### D2: 空布局 (Empty Layout)

| 属性 | 值 |
|------|----|
| **严重度** | Info |
| **触发条件** | 布局的 `item_count() == 0` |
| **检测来源** | JSON `LAYOUT_MAP` 和 `NATIVE_LAYOUTS` 注册表 |

**典型场景**：
```rust
let mut hbox = HBoxLayout::new(Orientation::Horizontal, 4, 2);
store_layout(win_id, Box::new(hbox));
// ❌ 忘记 add_widget() — hbox 是空的
```

**推荐处理**：
- 如果是 JSON 布局：检查 children 数组是否为空或缺失
- 如果是原生布局：检查是否调用了 `layout.add_widget(id, stretch)`

---

### 3.2 P1 — 几何诊断

#### D3: 零尺寸控件 (Zero-Size Widget)

| 属性 | 值 |
|------|----|
| **严重度** | Error |
| **触发条件** | `rect.width == 0 || rect.height == 0` |
| **检测来源** | 通过 `LayoutInspector::record_geometry()` 收集 |

**典型场景**：
```json
{
    "window": {
        "layout": { "type": "hbox" },
        "children": [
            { "button": { "text": "OK", "min_width": 0, "min_height": 0 } }
        ]
    }
}
// ❌ 按钮尺寸为 0×0 — 不可见
```

**根因分析**：
1. 父容器尺寸太小，无法容纳子控件
2. 子控件没有设置 `min_width`/`min_height`
3. BoxLayout 中 stretch 分配导致某个控件被压缩到 0
4. 父容器本身是零尺寸（递归问题）

**推荐处理**：
- 📏 为控件添加 `min_width` 和 `min_height`
- 📐 增大父容器的尺寸
- 🔄 调整 BoxLayout 的 stretch 因子，避免某个控件被压到 0

#### D4: 重叠控件 (Overlapping Widgets)

| 属性 | 值 |
|------|----|
| **严重度** | Warning |
| **触发条件** | 同一父容器下两个子控件的 `Rect` 相交且不相等 |
| **检测来源** | 按 parent 分组后两两对比 |

**典型场景**：
```json
{
    "window": {
        "layout": { "type": "hbox" },
        "children": [
            { "button": { "stretch": 1 } },
            { "button": { "stretch": 3 } }
        ]
    }
}
// 如果父容器宽度 = 200, spacing = 4, margin = 2
// 两个按钮可能因为整数除法 + 剩余分配异常导致重叠
```

**推荐处理**：
- 📐 检查 stretch 值是否合理（避免极端比例如 100:1）
- 📏 添加明确的 `min_width` 约束
- 🔄 考虑更换布局类型（如 Grid 替代 Box）

---

### 3.3 P2 — 语义诊断

#### D5: 布局类型不一致 (Layout Mismatch)

| 属性 | 值 |
|------|----|
| **严重度** | Info |
| **触发条件** | JSON 中声明的布局类型与实际创建的布局类型不匹配 |
| **检测来源** | 比较 `JSON layout.type` 与 `Layout trait` 的 downcast 结果 |

**典型场景**：
```json
{
    "window": {
        "layout": { "type": "vbox" },
        "children": [
            { "button": { "text": "Top" } },
            { "button": { "text": "Bottom" } }
        ]
    }
}
// 如果 loader 错误地创建了 HBoxLayout 而非 VBoxLayout
```

**推荐处理**：
- 🔄 确认 `parse_layout_kind()` 和 `create_layout_from_kind()` 一致
- 添加测试覆盖每种 layout type 的解析→创建→更新链路

#### D6: 布局过载 (Layout Overload)

| 属性 | 值 |
|------|----|
| **严重度** | Info |
| **触发条件** | BoxLayout 中的 items 数量超过 8 |
| **检测来源** | `BoxLayout::item_count()` |

**典型场景**：
```rust
// HBox 中放 15 个按钮 — 每个按钮宽度 = (container - margin*2 - 14*spacing) / 15
// 如果 container=400, margin=2, spacing=2 → 每个按钮 ≈ 24px — 非常拥挤
```

**推荐处理**：
- 📐 考虑更换为 GridLayout 或使用嵌套布局
- 🔄 减少直接子控件数量，用 GroupBox 分组

---

## 4. 推荐引擎设计

### 4.1 推荐分类

| 类别 | 触发条件 | 推荐动作 | 代码表达 |
|------|----------|----------|----------|
| **R1: 重布局** | 有任何 Error 或 Warning | 调用 `recalculate()` 重新触发 `layout.update()` | `🔄 建议执行重布局 (Recalculate)` |
| **R2: 小尺寸修复** | 有零尺寸控件 | 添加 `min_width`/`min_height` 约束 | `📏 设置 min_width / min_height` |
| **R3: 嵌套修复** | 有孤儿控件 | 检查 JSON 嵌套结构 | `🔗 检查 JSON 嵌套结构` |
| **R4: Stretch 调整** | 有重叠控件 | 调整 stretch 因子或改用 Grid | `📐 检查 widget 的尺寸和约束` |
| **R5: 布局类型** | 类型不一致 | 统一 JSON type 和实际布局类型 | `🔍 检查 parse_layout_kind() 路由` |
| **R6: 通用** | 总有 | 建议在 JSON 加载后立即触发诊断 | `🔍 在 JSON 加载后触发诊断` |

### 4.2 推荐语言

推荐使用中文描述，因为这是面向开发者调试的工具：

```rust
fn generate_recommendations(issues: &[Issue]) -> Vec<Recommendation> {
    let mut recs = Vec::new();

    if has_errors_or_warnings(issues) {
        recs.push(Recommendation::new(
            "🔄 建议执行重布局 (Recalculate)",
            "调用 recalculate() 或重新触发 layout.update() 以使布局重新计算",
            "LayoutInspector 检测到 {n} 个问题，请修复后调用 recalculate() 重新布局",
        ));
    }
    if has_zero_size(issues) {
        recs.push(Recommendation::new(
            "📏 设置最小尺寸约束",
            "零尺寸控件需要显式设置 min_width/min_height",
            "在 JSON 中添加 min_width/min_height，或使用 SizePolicy::Expanding",
        ));
    }
    // ...
}
```

### 4.3 推荐优先级

```
diagnose() 完成后:
  if has_errors()  → 优先推荐 R1 (重布局是唯一能验证修复是否有效的方式)
  if has_orphans() → 推荐 R3 (必须先修复嵌套结构)
  if has_zero()    → 推荐 R2 (需要设置尺寸约束)
  if has_overlap() → 推荐 R4 (需要调整 stretch)
  else             → 推荐 R6 (通用建议)
```

---

## 5. 集成方案

### 5.1 默认使用模式 (推荐)

```rust
use rust_widgets::layout::inspector::LayoutInspector;
use rust_widgets::json::layout::collect_layout_snapshots;

// 1. 加载 JSON
let layout = JsonLoader::load(json_str)?;

// 2. 启用检查器
LayoutInspector::enable();

// 3. 触发布局计算（通常由渲染循环或窗口 resize 触发）
//    layout.update() 会在回调中自动调用 LayoutInspector::record_geometry()

// 4. 运行诊断
let report = LayoutInspector::run_once(&registry);

// 5. 处理结果
if report.has_issues() {
    println!("{}", report);  // 打印问题 + 推荐方案

    if report.has_errors() {
        // 严重问题 — 建议修复后重新触发布局
        // fix_layout_issues(&report);
        // request_layout_recalculation();
    }
}

// 6. 关闭
LayoutInspector::disable();
```

### 5.2 JSON 加载器集成

在 `JsonLoader::load()` 返回后：

```rust
pub fn load(json_str: &str) -> Result<BoundJsonLayout, String> {
    let result = Self::load_internal(json_str)?;

    // 集成点：如果检查器已启用，自动收集 JSON 布局快照
    if LayoutInspector::is_enabled() {
        // 遍历 LAYOUT_MAP 为每个 layout 注册检查信息
        for (parent_id, _layout) in LAYOUT_MAP 中 {
            LayoutInspector::register_native_layout(
                parent_id,
                &format!("json_layout_{}", parent_id),
                layout.item_count(),
                get_layout_type_name(&*layout),
            );
        }
    }

    result
}
```

### 5.3 layout.update() 几何收集集成

在每个 `layout.update(rect, callback)` 调用点，用包装回调收集几何数据：

```rust
// Before:
layout.update(rect, &mut |id, child_rect| {
    callback(id, child_rect);
});

// After (if inspector enabled):
layout.update(rect, &mut |id, child_rect| {
    LayoutInspector::record_geometry(id, child_rect);  // ← 新增
    callback(id, child_rect);
});
```

这不需要修改任何 Layout trait 实现，只需在调用 update 的 3-5 个位置添加包装。

---

## 6. 数据结构

### 6.1 DiagnosticReport

```rust
pub struct DiagnosticReport {
    pub issues: Vec<Issue>,
    pub recommendations: Vec<Recommendation>,
    pub widgets_inspected: usize,
    pub layouts_inspected: usize,
}

impl DiagnosticReport {
    pub fn has_issues(&self) -> bool { !self.issues.is_empty() }
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }
}
```

### 6.2 Issue

```rust
pub struct Issue {
    pub severity: Severity,       // Info | Warning | Error
    pub description: String,      // 人类可读描述
    pub widget_id: Option<ObjectId>,
    pub category: &'static str,   // "结构" | "几何" | "布局"
}
```

### 6.3 Recommendation

```rust
pub struct Recommendation {
    pub title: String,    // "🔄 建议执行重布局 (Recalculate)"
    pub summary: String,  // 一句话摘要
    pub detail: String,   // 展开的详细说明或代码示例
}
```

### 6.4 Severity

```rust
pub enum Severity { Info, Warning, Error }
```

---

## 7. 实现计划

### 第一阶段 — 核心引擎 (P0)

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| 1 | 创建 `LayoutInspector` 结构体 + 全局开关 | `src/layout/inspector.rs` | 小 |
| 2 | 实现几何快照收集 (`record_geometry`) | `src/layout/inspector.rs` | 小 |
| 3 | 实现原生布局注册 (`register_native_layout`) | `src/layout/inspector.rs` | 小 |
| 4 | 实现 4 项核心诊断 (orphan/empty/zero/overlap) | `src/layout/inspector.rs` | 中 |
| 5 | 实现推荐引擎 (R1-R6) | `src/layout/inspector.rs` | 中 |
| 6 | 添加 `pub mod inspector` 到 `src/layout/mod.rs` | `src/layout/mod.rs` | 小 |
| 7 | 添加 `collect_layout_snapshots()` 到 `src/json/layout.rs` | `src/json/layout.rs` | 中 |
| 8 | 单元测试覆盖所有诊断和推荐 | `src/layout/inspector.rs` | 中 |

**验收标准**:
- `cargo check --all`: 0 errors, 0 warnings
- 所有 8 个诊断单元的测试覆盖 (orphan, empty, zero, overlap × 每个分 enabled/disabled 两态)
- 推荐引擎在发现问题时生成至少 R1 推荐

### 第二阶段 — JSON 集成 (P1)

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| 9 | 在 `JsonLoader::load()` 中添加自动布局收集 | `src/json/loader.rs` | 小 |
| 10 | 在 layout.update() 调用点添加 geometry 包装 | `src/app/*.rs` | 小 |
| 11 | 集成测试：加载 JSON → 运行诊断 → 验证结果 | `tests/` | 中 |
| 12 | 示例：`examples/inspector_demo.rs` | `examples/` | 中 |

**验收标准**:
- JSON 加载后自动收集布局快照
- layout.update() 回调中自动收集几何数据
- 集成测试覆盖 JSON 场景下的完整诊断链路

### 第三阶段 — 高级诊断 (P2)

| # | 任务 | 说明 | 工作量 |
|---|------|------|--------|
| 13 | D5: 布局类型一致性检查 | 通过 `Any::downcast_ref` 对比 JSON 类型 | 中 |
| 14 | D6: 布局过载检查 | BoxLayout item_count > 8 时提示 | 小 |
| 15 | 递归布局检测 (Grid 3 层嵌套 → 建议拆分) | 分析嵌套深度 | 小 |
| 16 | 性能分析提示 (Splitter pane 比例和 < 1.0) | 检查 ratios 总和 | 小 |

---

## 8. 测试策略

### 8.1 单元测试

| 测试 | 覆盖内容 | 状态 |
|------|----------|------|
| 默认禁用 | 检查 `is_enabled()` 返回 false | ✅ 设计 |
| 启用切换 | enable/disable 原子切换 | ✅ 设计 |
| 禁用时零诊断 | `run_once()` 返回空 report | ✅ 设计 |
| 检测孤儿控件 | 注册 button 无 parent → Warning | ✅ 设计 |
| 排除 Window | 注册 window 无 parent → 无 orphan | ✅ 设计 |
| 检测空布局 | LayoutSnapshot item_count=0 → Info | ✅ 设计 |
| 检测零尺寸 | record_geometry(0x100) → Error | ✅ 设计 |
| 检测重叠 | 两个 sibling rect 相交 → Warning | ✅ 设计 |
| 推荐生成 | 有 issues 时生成 R1 | ✅ 设计 |
| 干净时无推荐 | 只有 window → 无 recommendation | ✅ 设计 |
| Report format | Display 包含 "Layout Inspector Report" | ✅ 设计 |

### 8.2 集成测试

```rust
// tests/layout_inspector_test.rs

#[test]
fn json_load_then_diagnose() {
    LayoutInspector::enable();
    let json = r#"{
        "window": {
            "title": "Test",
            "layout": { "type": "hbox" },
            "children": [
                { "button": { "text": "A" } },
                { "button": { "text": "B" } }
            ]
        }
    }"#;
    let layout = JsonLoader::load(json).unwrap();
    // 触发布局计算...
    let report = LayoutInspector::run_once(&registry);
    assert!(!report.has_issues());  // ✅ 正常布局无问题
    LayoutInspector::disable();
}

#[test]
fn detect_orphan_in_json() {
    LayoutInspector::enable();
    let json = r#"{
        "window": { "title": "Win" },
        "button": { "text": "Lost" }
    }"#;
    let layout = JsonLoader::load(json).unwrap();
    let report = LayoutInspector::run_once(&registry);
    assert!(report.has_issues());  // ❌ 孤儿 button
    LayoutInspector::disable();
}
```

---

## 9. 推荐启用时机

| 场景 | 推荐 | 原因 |
|------|------|------|
| **日常开发** | ❌ 关闭 | 零开销，不影响性能 |
| **布局完成后检查 (推荐)** | ✅ 启用 | 在 `layout.update()` 之后立即调用 `run_once()`，诊断最准确 |
| **UI 布局调试** | ✅ 启用 | 看到控件重叠、缺失、错位时，一键诊断 + 获得推荐方案 |
| **重构 layout 模块后** | ✅ 启用 | 验证没有引入回归 |
| **新增 widget type 后** | ✅ 启用 | 验证 JSON 布局集成正确 |
| **生产环境** | ❌ 关闭 | 诊断有分配开销，不推荐 production 开启 |

**最佳实践**：在开发调试入口处加一个条件编译：

```rust
#[cfg(debug_assertions)]
{
    LayoutInspector::enable();
    // ... 加载布局、运行 ...
    let report = LayoutInspector::run_once(&registry);
    if report.has_issues() {
        log::warn!("{}", report);
    }
    LayoutInspector::disable();
}
```

---

## 10. 质量度量标准

| 维度 | 当前值 | 第一阶段目标 | 最终目标 |
|------|--------|-------------|---------|
| 总测试数 | 357 | 370+ | 400+ |
| 布局检查单元测试 | 0 | 12+ | 20+ |
| 布局检查集成测试 | 0 | 4+ | 8+ |
| 构建警告 | 0 | 0 | 0 |
| 诊断项 (D1-D6) | 0 | 4 (D1-D4) | 6 (D1-D6) |
| 推荐类型 (R1-R6) | 0 | 4 (R1-R4) | 6 (R1-R6) |
| 零开销禁用态 | ✅ 设计 | ✅ 实现 | ✅ 验证 |

---

## 11. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| `record_geometry()` 收集点遗漏 | 中 | 中 | 在 `Layout::update()` 的所有调用点添加包装，搜索所有 `.update(` 调用 |
| 快照数据过期 | 低 | 低 | `run_once()` 自动清理快照，多次调用自动刷新 |
| 双重收集 (同 widget 两次 update) | 低 | 低 | `record_geometry()` 会按 id 更新而非追加 |
| 跟 Layout trait 修改冲突 | 低 | 低 | 检查器通过回调包装收集数据，不修改 Layout trait |

---

## 12. BLUE5 自检 (PUA 质量门禁)

| 自检项 | 状态 |
|--------|------|
| 不出现 qt/qml 相关字眼 | ✅ 全文零提及 |
| 基于现有代码现状 | ✅ 基于 `src/layout/`, `src/json/layout.rs`, `src/index/registry.rs` 实际代码分析 |
| 分阶段可执行计划 | ✅ 3 阶段，每阶段有验收标准 |
| 零开销设计要求明确 | ✅ AtomicBool 全局开关 + thread_local 快照 |
| 双通道检查覆盖 JSON + 原生 | ✅ D1-D6 分别来自两种布局源 |
| 测试策略明确 | ✅ 单元 + 集成 + 边界条件 |
| 推荐引擎设计完整 | ✅ R1-R6 对应不同问题类别 |
| 风险评估完整 | ✅ 4 项风险 + 缓解措施 |

---

*本文档为 Rust Widgets 布局检查器与推荐重布局系统的完整设计规划。第一阶段 (P0) 可立即开始实现。*
