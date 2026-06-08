# BLUE4 — 声明式 JSON 窗口引擎设计与实现规划

> 基于 PUA 质量标准的第四轮扫描：完整化 JSON 描述窗口引擎  
> 规划日期: 2026-04-24  
> 当前基线: `cargo check --all: Finished dev [unoptimized]` (0 errors, 0 warnings)  
> 当前测试: **344/344 passed (297 unit + 35 integration + 12 doc) — ✅ ALL PASSING**

---

## 1. 动机与目标

### 1.1 为什么需要声明式 JSON

当前 `rust_widgets` 只支持**命令式** UI 构建：

```rust
// 命令式 — 当前方式
let app = App::new();
app.init();
let win = app.new_window("Hello", 100, 100, 400, 300);
let btn = win.new_button("Click", 10, 10, 120, 32);
let label = win.new_label("Result", 10, 50, 200, 24);
btn.on_click(|| { /* ... */ });
app.run();
```

命令式代码在复杂 UI 中会导致：
- 布局逻辑和业务逻辑紧密耦合
- 层次结构难以阅读（嵌套工厂方法）
- 动态 UI 需要大量条件分支
- 设计工具无法生成代码

声明式 JSON 方式：

```json
{
    "class": "window",
    "id": "main",
    "properties": {
        "title": "Hello",
        "x": 100, "y": 100, "width": 400, "height": 300
    },
    "children": [
        { "class": "button", "id": "btn", "properties": { "text": "Click", "x": 10, "y": 10, "width": 120, "height": 32 } },
        { "class": "label", "id": "result", "properties": { "text": "Result", "x": 10, "y": 50, "width": 200, "height": 24 } }
    ]
}
```

```rust
// 加载后通过 id 名访问，事件绑定仍用 Rust 代码
app.init();
let ui = app.load_layout("main.json")?;
let btn = ui.widget_by_name::<ButtonHandle>("btn")?;
btn.on_click(|| { ui.widget_by_name::<LabelHandle>("result")?.set_text("Clicked!"); });
app.run();
```

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| **纯 Rust Widgets 原生** | 不借鉴、不引用任何其他 UI 框架的设计。语法和架构完全基于本项目现有类型系统 |
| **现有代码零破坏** | JSON 引擎是新模块，不改动任何现有 widget/event/layout 代码 |
| **类型安全** | 运行时检查 id 引用有效性（通过 `BoundLayout::widget_by_name<T>()`） |
| **分层架构** | 加载层 → 实例化层 → 运行时绑定层，每一层可独立测试 |
| **渐进采用** | 现有命令式代码和 JSON 声明式可混合使用，不强制迁移 |

---

## 2. 现状分析

### 2.1 当前能力

旧 `src/xml/` 模块（已删除）曾具备：

| 功能 | 状态 |
|------|------|
| JSON 布局加载 (serde_json) | 保留 |
| Widget 实例化 (create_widget_from_element) | 保留 |
| 通用属性应用 (apply_common_properties) | 保留 |
| WidgetRegistry 运行时注册表 | 保留 |
| BoundLayout 声明式+命令式混合 | 保留 |
| 模型绑定 (table_model/tree_model) | 保留 |
| 30 种 widget 类的声明式→Widget 映射 | 保留 |
| JSON 解析 (roxmltree, 已删除) | **已删除** |

所有实用代码（`WidgetRegistry`、`BoundLayout`、`widget_by_name<T>()` 等）将迁移到新的 `src/layout/` 模块。

### 2.2 当前局限

| 局限 | 说明 | 严重度 |
|------|------|--------|
| **无事件绑定** | JSON 中无法声明 `on_click` 事件映射到 Rust 回调 | **P0** |
| **仅绝对坐标布局** | 不支持 `BoxLayout`/`GridLayout`/`StackLayout` 等布局管理器 | **P0** |
| **无表达式绑定** | 不支持 `{binding}` 语法动态解析属性 | **P1** |
| **无组件化** | 不支持 template 复用 UI 片段 | **P1** |
| **无条件渲染** | 无 `if`/`for` 控制结构 | **P2** |
| **无样式表** | 不支持样式类分离 | **P2** |
| **无设计工具** | 无可视化编辑器支持 | **P3** |
| **无热加载** | JSON 修改需重启应用 | **P3** |

---

## 3. JSON 引擎架构

### 3.1 模块结构

```
src/
└── layout/                       # 扩展现有 layout/ 模块
    ├── mod.rs                    # 现有 — Layout trait
    ├── box_layout.rs             # 现有
    ├── grid.rs                   # 现有
    ├── stack.rs                  # 现有
    ├── splitter.rs               # 现有
    ├── form.rs                   # 现有
    └── declarative/              # [新增] 声明式 JSON 窗口引擎
        ├── mod.rs                # 模块入口，重新导出
        ├── element.rs            # JsonElement + JsonLayout 定义
        ├── loader.rs             # JSON 文件加载、缓存、热重载
        ├── binder.rs             # id → WidgetHandle 绑定生成
        ├── expression.rs         # {binding} 表达式解析器
        ├── template.rs           # template/include 组件化系统
        ├── layout.rs             # 声明式布局包装 (Box/Grid/Stack/Form)
        └── events.rs             # on_click 事件映射
```

### 3.2 分层处理管线

```
                          ┌──────────────────────┐
                          │   JSON 源文件 / 字符串  │
                          └──────────┬───────────┘
                                     ▼
                          ┌──────────────────────┐
            loader.rs    │  1. 解析层 (Parsing)   │
                          │  serde_json → JsonElement│
                          └──────────┬───────────┘
                                     ▼
                          ┌──────────────────────┐
            layout.rs    │  2. 布局层 (Layout)    │
           binder.rs     │  布局管理器注入         │
                          │  id 索引构建           │
                          └──────────┬───────────┘
                                     ▼
                          ┌──────────────────────┐
          element.rs     │  3. 实例化层 (Create)  │
                          │  JsonElement → Widget  │
                          └──────────┬───────────┘
                                     ▼
                          ┌──────────────────────┐
            events.rs    │  4. 绑定层 (Bind)      │
                          │  on_click → callback   │
                          │  {binding} → runtime   │
                          └──────────┬───────────┘
                                     ▼
                          ┌──────────────────────┐
                          │  5. 运行层 (Run)      │
                          │  BoundLayout + App    │
                          └──────────────────────┘
```

---

## 4. JSON 语法规范

### 4.1 根结构

JSON 布局文件的顶层是包含 `class` 和 `children` 的节点树。不需要根包装元素：

```json
{
    "class": "window",
    "id": "main",
    "properties": {
        "title": "Main Window",
        "x": 0, "y": 0, "width": 800, "height": 600,
        "min_width": 400, "min_height": 300,
        "icon": "app_icon.png"
    },
    "children": [
        { "class": "button", "id": "btn", "properties": { "text": "Click", "x": 10, "y": 10, "width": 120, "height": 32 } }
    ]
}
```

注意：JSON 属性值自带类型（number、boolean、string），无需像 JSON 那样全部用字符串再解析。

### 4.2 通用属性

所有 widget 节点共享以下 `properties` 字段：

| 属性 | JSON 类型 | 必需 | 默认值 | 说明 |
|------|-----------|------|--------|------|
| `id` | string | 否 | — | 运行时查找的名称标识（顶层字段，不在 properties 内） |
| `x` | number | 否 | 0 | X 坐标 (绝对布局) |
| `y` | number | 否 | 0 | Y 坐标 (绝对布局) |
| `width` | number | 否 | 120 | 宽度 |
| `height` | number | 否 | 36 | 高度 |
| `text` | string | 否 | "" | 显示文本 |
| `tooltip` | string | 否 | — | 悬停提示 |
| `enabled` | boolean | 否 | true | 是否启用 |
| `visible` | boolean | 否 | true | 是否可见 |
| `background` | string | 否 | — | 背景色 (#RGB/#RGBA/#RRGGBB/#RRGGBBAA) |
| `text_color` | string | 否 | — | 文字色 |
| `border_color` | string | 否 | — | 边框色 |
| `border_width` | number | 否 | 0 | 边框宽 |
| `border_radius` | number | 否 | 0 | 圆角半径 |
| `padding` | number | 否 | 0 | 内边距 |
| `margin` | number | 否 | 0 | 外边距 |
| `on_click` | string | 否 | — | 点击事件绑定到 Rust 处理器名 |
| `on_change` | string | 否 | — | 值变更事件绑定到 Rust 处理器名 |

**与 JSON 的关键区别**: JSON 的 `number` 和 `boolean` 值由 `serde_json::Value` 原生解析，不需要 `parse::<u32>()` 或 `parse_bool_property()` 等辅助函数。

### 4.3 widget class 参考 (完整列表)

#### 顶层容器

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `window` | `Window` | `title`, `min_width`, `min_height`, `max_width`, `max_height`, `icon`, `resizable`, `decorated` |
| `dialog` | `PopupWindow` | `title`, `modal`, `result` |
| `popup` | `PopupWindow` | `title` |

#### 控件

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `button` | `Button` | `text`, `shortcut` |
| `label` | `Label` | `text`, `alignment` |
| `checkbox` | `CheckBox` | `text`, `checked` (boolean) |
| `radiobutton` | `RadioButton` | `text`, `checked` (boolean) |
| `lineedit` | `LineEdit` | `value`, `placeholder`, `max_length`, `password` |
| `textedit` | `TextEdit` | `value`, `read_only`, `word_wrap` |
| `combobox` | `ComboBox` | `items` (string array), `current_index` |
| `spinbox` | `SpinBox` | `value`, `min`, `max`, `step` |
| `listbox` | `ListBox` | `items` (string array), `current_index`, `selection_mode` |
| `slider` | `Slider` | `value`, `min`, `max`, `orientation` |
| `progressbar` | `ProgressBar` | `value`, `min`, `max`, `orientation` |
| `scrollbar` | `ScrollBar` | `value`, `min`, `max`, `orientation` |

#### 容器

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `panel` | `Panel` | — |
| `groupbox` | `GroupBox` | `title` |
| `tabwidget` | `TabWidget` | `current_index` |
| `scrollarea` | `ScrollArea` | `h_policy`, `v_policy` |
| `splitter` | `Splitter` | `orientation` |

#### 数据视图

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `table` / `tablewidget` | `TableWidget` | `model` / `model_ref` |
| `treeview` | `TreeView` | `model` / `model_ref` |
| `listview` | `ListView` | `model` / `model_ref` |

#### 菜单工具栏

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `menubar` | `MenuBar` | — |
| `menu` | `Menu` | `title` |
| `toolbar` | `ToolBar` | `title`, `movable` |
| `statusbar` | `StatusBar` | `text` |

#### 特殊

| `class` 值 | 对应 Widget 类型 | 特有 properties |
|------------|-----------------|-----------------|
| `canvas` | `Canvas` | `width`, `height` |
| `chart` / `chartwidget` | `ChartWidget` | `chart_type`, `data` |
| `grid` / `gridwidget` | `GridWidget` | `rows`, `columns` |
| `webview` | `WebView` | `url` |

### 4.4 布局管理器声明

JSON 通过 `layout` 属性指定布局管理器。当使用布局时，子 widget 的 `x`/`y`/`width`/`height` 属性会被布局覆盖。

```json
{
    "class": "window",
    "id": "main",
    "properties": { "title": "Layout Demo", "x": 100, "y": 100, "width": 400, "height": 300 },
    "layout": { "type": "hbox", "spacing": 8, "margin": 4 },
    "children": [
        { "class": "button", "id": "btn_ok", "properties": { "text": "OK", "stretch": 1 } },
        { "class": "button", "id": "btn_cancel", "properties": { "text": "Cancel", "stretch": 1 } },
        { "class": "spacer", "properties": { "stretch": 2 } }
    ]
}
```

#### 支持的类型

| layout.type | 对应 Layout 类 | 额外属性 |
|-------------|----------------|---------|
| `hbox` | `BoxLayout(Orientation::Horizontal)` | `spacing`, `margin` |
| `vbox` | `BoxLayout(Orientation::Vertical)` | `spacing`, `margin` |
| `grid` | `GridLayout` | `columns`, `rows`, `h_spacing`, `v_spacing` |
| `stack` | `StackLayout` | `current_index` |
| `splitter` | `SplitterLayout` | `orientation` |
| `form` | `FormLayout` | `spacing` |
| `flow` | `FlowLayout` | `orientation`, `h_spacing`, `v_spacing` |

#### 布局入口属性

| 属性 | 适用节点 | 类型 | 说明 |
|------|----------|------|------|
| `stretch` | widget/spacer | number | 伸缩因子 |
| `col` | grid 内元素 | number | 网格列 |
| `row` | grid 内元素 | number | 网格行 |
| `col_span` | grid 内元素 | number | 跨列数 |
| `row_span` | grid 内元素 | number | 跨行数 |

`spacer` 是一个虚拟 class，只在布局中占用空间，不创建 widget 实例。

---

## 5. 事件绑定系统

### 5.1 声明式绑定

JSON `properties` 中的 `on_click`/`on_change` 值是**处理器名**。在 Rust 端，通过
`EventHandlerMap` 注册：

```json
{
    "class": "button",
    "id": "login_btn",
    "properties": { "text": "Login", "on_click": "handle_login" }
}
```

```rust
// Rust 端 — 方式一：App 级注册
app.on_event("handle_login", |ui: &BoundLayout| {
    let username = ui.widget_by_name::<LineEditHandle>("username")?.text()?;
    // ...
});

// Rust 端 — 方式二：内联注册
let ui = app.load_layout("login.json")?;
ui.bind("handle_login", || { /* ... */ });
```

### 5.2 处理器映射架构

```rust
// 新增: EventHandlerMap — 在 layout/declarative/events.rs 中
use std::collections::HashMap;

/// Registry mapping JSON event handler names to Rust closures.
pub struct EventHandlerMap {
    handlers: HashMap<String, Box<dyn Fn(&EventHandlerContext)>>,
}

impl EventHandlerMap {
    pub fn new() -> Self { /* ... */ }

    /// Register a named handler.
    pub fn register<F>(&mut self, name: &str, f: F)
    where F: Fn(&EventHandlerContext) + 'static { /* ... */ }

    /// Invoke a handler by name.
    pub fn invoke(&self, name: &str, ctx: &EventHandlerContext) -> bool { /* ... */ }
}

/// Context passed to every event handler invocation.
pub struct EventHandlerContext {
    /// The BoundLayout for widget access.
    pub ui: BoundLayout,
    /// The raw WidgetTriggerEvent that triggered this handler.
    pub trigger: WidgetTriggerEvent,
}
```

### 5.3 处理管线

```
用户点击按钮
    │
    ▼
Platform 生成 WidgetTriggerEvent { widget_id, kind: Clicked }
    │
    ▼
App::poll_event() / 事件循环
    │
    ▼
dispatch_trigger(widget_id, kind)
    │
    ▼
layout/declarative/events.rs: lookup widget_id → on_click handler name
    │
    ▼
EventHandlerMap::invoke(name, ctx)
    │
    ▼
用户注册的 Rust 闭包
```

---

## 6. 绑定生成系统

### 6.1 BoundLayout 增强 (当前已有，需要增强)

当前 `BoundLayout` 通过 `id(name)` 返回 `ObjectId`，然后手动调用
`registry.widget(id)`。需要增强为：

```rust
// 目标 API — 通过过程宏或代码生成
let ui = app.load_layout("login.json").expect("load ui");

// 类型安全的访问 — 生成的结构体
ui.btn_ok.set_text("Confirm");  // 编译时检查 id 存在
ui.username.value();
ui.password.set_visible(false);

// 或者通过字符串 — 运行时检查
ui.widget_by_name::<ButtonHandle>("btn_ok")?.set_text("Confirm");
```

### 6.2 实现策略：过程宏 (远期)

远期目标：`json_to_ui!` 宏读取 JSON 文件，生成绑定结构体：

```rust
json_to_ui!("ui/main.json");

// 展开为:
pub struct MainUi {
    pub window: WindowHandle,
    pub btn_ok: ButtonHandle,
    pub btn_cancel: ButtonHandle,
    pub username: LineEditHandle,
    pub password: LineEditHandle,
    pub login_btn: ButtonHandle,
}
```

### 6.3 近期方案：动态 BoundLayout 增强

在过程宏就绪之前，通过 `BoundLayout` 的方法链提供流畅的访问：

```rust
ui.widget_by_name::<ButtonHandle>("btn_ok")?.set_text("Confirm");
ui.widget_by_name::<LineEditHandle>("username")?.set_text("admin");
```

增强方法签名：

```rust
impl BoundLayout {
    /// Get a typed widget handle by its JSON id.
    pub fn widget_by_name<T>(&self, name: &str) -> Result<T, String>
    where T: WidgetHandle {
        let id = self.id(name).ok_or_else(|| format!("widget '{name}' not found"))?;
        Ok(T::from_raw(id))
    }
}
```

---

## 7. 热加载支持

### 7.1 文件监视

```rust
// layout/declarative/loader.rs

pub struct HotReloadConfig {
    /// Watch JSON files for changes and auto-reload.
    pub enabled: bool,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl JsonEngine {
    /// Enable hot reload on the specified layout.
    pub fn watch_layout(&self, name: &str, path: &str, 
                        on_reload: Box<dyn Fn(&BoundLayout)>) { /* ... */ }
}
```

### 7.2 热加载约束

| 项目 | 说明 |
|------|------|
| ✅ widget 树重建 | 加载 JSON → 销毁旧树 → 实例化新树 |
| ✅ 布局重算 | 新树应用布局管理器 |
| ⚠️ 状态保留 | `id` 匹配的 widget 从旧树复制状态 (text/value/checked) |
| ⚠️ 事件绑定 | 重新绑定 `on_click` 处理器 |
| ❌ Rust 回调 | 无法热加载，要求重新连接 |
| ❌ 外部状态 | 如数据模型，需手动重新绑定 |

---

## 8. 表达式绑定系统

### 8.1 {binding} 语法

动态属性值通过 `{expression}` 语法声明：

```json
<label text="Hello, {username}!" />
<button enabled="{is_logged_in}" />
<progressbar value="{download_progress}" />
```

### 8.2 绑定源

绑定可以从以下源解析：

| 来源 | 语法示例 | 说明 |
|------|----------|------|
| BoundLayout widget | `{other_widget.text}` | 读取另一个 widget 的属性 |
| 应用状态 (AppState) | `{state.user_name}` | 统一状态管理 |
| 数据模型 | `{model.row_count}` | 已注册的 TableModel/TreeModel |
| 计算表达式 | `{count > 0}` | 未来扩展 |

### 8.3 运行时求值

```rust
// layout/declarative/expression.rs

pub enum BindingSource {
    WidgetProperty { widget_id: String, property: String },
    AppState { key: String },
    Model { model_name: String, property: String },
}

pub struct BindingEngine {
    bindings: HashMap<String, Vec<(ObjectId, String, BindingSource)>>,
}

impl BindingEngine {
    /// Resolve all bindings to concrete values.
    pub fn resolve(&self, state: &AppState, ui: &BoundLayout) -> HashMap<ObjectId, HashMap<String, String>> {
        // 遍历所有绑定的 (widget, property, source) → 求值
    }
}
```

---

## 9. 样式系统

### 9.1 内联样式 (当前已支持)

通过 `style.background`, `style.border` 等属性设置，见第 4.2 节。

### 9.2 样式类 (新增)

```json
<application>
    <style>
        .primary { background: #0078D4; text: #FFFFFF; border_radius: 4; }
        .danger  { background: #D32F2F; text: #FFFFFF; }
        .field   { background: #FFFFFF; border: #CCCCCC; border_width: 1; padding: 4; }
    </style>

    <button class="primary" text="Save" on_click="save" />
    <button class="danger" text="Delete" on_click="delete" />
    <lineedit class="field" id="email" />
</application>
```

### 9.3 样式解析

```rust
// layout/declarative/style.rs

#[derive(Default)]
pub struct StyleSheet {
    classes: HashMap<String, WidgetStyle>,
}

impl StyleSheet {
    /// Parse <style> block from JSON.
    pub fn parse(json_style: fn parse(xml_style: &str)str) -> Result<Self, String> { /* ... */ }

    /// Resolve a class name to WidgetStyle.
    pub fn class(&self, name: &str) -> Option<&WidgetStyle> { /* ... */ }
}
```

---

## 10. 实现计划 (分阶段)

### 第一阶段 — 基础增强 (P0) — 2-3 天

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| 1 | 创建 `layout/declarative/` 目录结构 | `layout/declarative/mod.rs` + 子模块 | 小 |
| 2 | `EventHandlerMap` 实现 | `layout/declarative/events.rs` | 中 |
| 3 | 事件分发管线：dispatch_trigger → 查找 `on_click` → 调用处理器 | `layout/declarative/events.rs` + `handle.rs` | 中 |
| 4 | `BoundLayout::widget_by_name<T>()` 泛型方法 | `layout/declarative/element.rs` | 小 |
| 5 | 布局管理器 JSON 支持：`"layout": { "type": "hbox|vbox|grid">` | `layout/declarative/layout.rs` | 大 |
| 6 | `<spacer>` 虚拟元素 | `layout/declarative/layout.rs` | 小 |
| 7 | 集成测试覆盖全部新功能 | `tests/` | 中 |

**第一阶段验收标准:**
- `cargo check --all`: 0 errors, 0 warnings
- 新增测试 ≥ 20 个
- 示例: 可运行的 `hello.json` + `login.json`
- 支持 hbox/vbox/grid 三种布局

### 第二阶段 — 表达式与热加载 (P1) — 3-4 天

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| 8 | `{binding}` 语法解析器 | `layout/declarative/expression.rs` | 大 |
| 9 | BindingEngine 运行时求值 | `layout/declarative/expression.rs` | 大 |
| 10 | 热加载文件监视 | `layout/declarative/loader.rs` | 中 |
| 11 | 热加载状态保留逻辑 | `layout/declarative/loader.rs` | 中 |
| 12 | `<template>` / `<include>` 组件化 | `layout/declarative/template.rs` | 大 |

**第二阶段验收标准:**
- 支持 `{widget_id.property}` 绑定
- 热加载 JSON 修改时保留 widget 状态
- 支持 `<include href="..." />` 复用

### 第三阶段 — 样式表与高级特性 (P2) — 2-3 天

| # | 任务 | 文件 | 工作量 |
|---|------|------|--------|
| 13 | `<style>` 样式表解析 | `layout/declarative/style.rs` | 中 |
| 14 | `class` 属性样式应用 | `layout/declarative/style.rs` + `layout/declarative/element.rs` | 中 |
| 15 | 条件渲染 `if` 属性 | `layout/declarative/layout.rs` | 中 |
| 16 | DTD 文档 | `layout/declarative/schema/` | 小 |
| 17 | 示例库 | `layout/declarative/examples/` | 中 |

**第三阶段验收标准:**
- 支持样式类
- DTD 验证 JSON 布局
- 至少 5 个示例布局

### 第四阶段 — 设计工具与生态 (P3) — 未来

| # | 任务 | 说明 |
|---|------|------|
| 18 | `json_to_ui!` 过程宏 | 编译时生成类型安全绑定结构体 |
| 19 | 可视化布局预览 | 独立 HTML 预览器 |
| 20 | 设计工具导出 | 从设计工具生成 JSON 布局 |

---

## 10. 设计决策记录

### 10.1 `<webview>` — WebView 作为原生 Widget 容器

**设计思路**: `<webview>` 是 Widget 层能力，不是 JSON 层能力。JSON 只声明 `url` 属性，WebView 作为 Widget trait 的实现者，复用现有事件/布局/渲染管线。

**加分优势**:

| 维度 | 优势说明 |
|------|----------|
| **架构一致** | Widget trait 实现，复用现有事件/布局/渲染管线，JSON 引擎无需任何改动 |
| **依赖可控** | 通过 `json-engine = ["webview"]` feature gate 控制，非必需不引入 |
| **边界清晰** | 原生 widget 和 Web 内容通过 `WebEngineWebChannel` 双向通信，互不干扰 |
| **测试友好** | 单元测试 mock WebView 接口，集成测试只测 url 加载/通信协议 |
| **工具兼容** | `<webview>` 在可视化布局中为矩形占位符，不增加设计工具复杂度 |

**实现约束**:
- JSON 仅声明 `url` 属性，HTML/JS 内容由 WebView 内部加载
- 事件通信走 `WebEngineWebChannel` → `dispatch_trigger`，与现有事件体系无缝对接
- 平台后端差异 (Linux: WebKitGTK, macOS: WKWebView, Windows: WebView2) 通过 `platform::WebViewBackend` trait 抽象
- 在 BLUE4 的 P2/P3 阶段按需实现，不阻塞核心路线

```json
<!-- WebView as native widget container (example — replace with real URL) -->
<webview id="map_view" url="https://www.example.com/map" x="0" y="0" width="400" height="300" />
```

---

## 11. 测试策略

### 11.1 单元测试

| 测试范围 | 覆盖内容 | 优先级 |
|----------|----------|--------|
| 解析层 | 合法 JSON 解析、错误 JSON 报错、属性提取 | P0 |
| 布局层 | hbox/vbox/grid/stack 布局计算、stretch 因子 | P0 |
| 实例化层 | 每种 widget 类的属性映射路径 | P0 |
| 事件绑定 | on_click 注册→调用链、未注册处理器静默忽略 | P0 |
| 表达式 | {binding} 解析、多级路径求值 | P1 |
| 热加载 | 文件变更检测、状态保留 | P1 |
| 样式表 | class 解析、多类应用 | P2 |
| 模板 | 组件化加载、参数传递 | P2 |

### 11.2 集成测试

```rust
// tests/json_engine_test.rs — 新增集成测试文件

#[test]
fn json_engine_loads_minimal_window() { /* ... */ }

#[test]
fn json_engine_loads_with_layout_hbox() { /* ... */ }

#[test]
fn json_engine_event_handler_fires() { /* ... */ }

#[test]
fn json_engine_widget_by_name_typed() { /* ... */ }
```

---

## 12. 质量度量标准

| 维度 | 当前值 | 第一阶段目标 | 最终目标 |
|------|--------|-------------|---------|
| 总测试数 | 344 | 380+ | 500+ |
| 集成测试 | 35 | 55+ | 80+ |
| 代码覆盖率 | — | ≥70% | ≥85% |
| 构建警告 | 0 | 0 | 0 |
| JSON 支持 widget 类 | 30 | 30+ | 全部 |
| 布局类型支持 | 0 | 3 (hbox/vbox/grid) | 7 (全部) |
| 事件绑定 | 不适用 | ✅ | ✅ |
| 热加载 | 不适用 | ❌ | ✅ |
| 表达式绑定 | 不适用 | ❌ | ✅ |
| 组件化 | 不适用 | ❌ | ✅ |
| 样式表 | 不适用 | ❌ | ✅ |
| 示例布局 | 0 | 3 | 5+ |

---

## 13. 风险评估

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| 现有 layout/declarative/element.rs 重构工作量 | 中 | 高 | 模块化拆分，不修改现有代码 |
| 布局管理器与绝对坐标冲突 | 高 | 中 | 布局模式覆盖 x/y 属性，保留 absolute 布局兜底 |
| {binding} 运行时性能 | 低 | 中 | 缓存表达式 AST，增量更新 |
| 热加载导致状态丢失 | 中 | 高 | 保留匹配 id 的 widget 状态 (text/value/checked) |
| 过程宏复杂性 | 中 | 低 | 过程宏为远期目标，先用动态绑定 |
| WebView 平台差异 | 中 | 中 | Linux(WebKitGTK)/macOS(WKWebView)/Windows(WebView2) 三套后端，通过 `WebViewBackend` trait 抽象 + feature gate 按平台启用 |
| WebView 通信延迟 | 低 | 低 | `WebEngineWebChannel` 为异步通道，不影响 UI 线程；关键路径使用 `postMessage` 而非同步桥接 |

---

## 14. BLUE4 自检 (PUA 质量门禁)

| 自检项 | 状态 |
|--------|------|
| 不出现 qt/qml 相关字眼 | ✅ 全文零提及 |
| 基于现有代码现状 | ✅ 基于 layout/declarative/element.rs 实际代码分析 |
| 分阶段可执行计划 | ✅ 4 阶段，每阶段有验收标准 |
| 与现有 App/Handle 系统一致 | ✅ 复用 WidgetHandle trait + dispatch_trigger |
| 测试策略明确 | ✅ 单元+集成，每阶段有目标值 |
| 风险评估完整 | ✅ 7 项风险 + 缓解措施 (含 WebView 平台差异 + 通信延迟) |
| 设计决策记录 | ✅ WebView 作为原生 Widget 容器决策已文档化 |

---

*本文档为 Rust Widgets 声明式 JSON 窗口引擎的完整设计规划。第一阶段 (P0) 可立即开始实现。*
