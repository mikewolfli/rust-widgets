# 布局系统

`rust-widgets` 布局系统通过 14 种可插拔的布局管理器，在父容器内对控件进行定位和尺寸调整。每种布局都实现了 `Layout` trait，并通过 `LayoutContext` 支持 DPI 感知缩放。

---

## 核心概念

### `Layout` Trait

每个布局管理器都实现了 `Layout`，这是核心抽象：

```rust
pub trait Layout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    fn remove_widget(&mut self, widget_id: ObjectId);
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn update_with_context(&self, rect: Rect, context: &LayoutContext, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn child_ids(&self) -> Vec<ObjectId>;
    fn has_child(&self, id: ObjectId) -> bool;
    fn clear(&mut self);
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
```

要点：
- `add_widget` 接受一个**拉伸因子 (stretch factor)**，由比例布局（BoxLayout、SplitterLayout、FlexLayout）使用。更高的拉伸值占用更多空间。
- `update` 是核心方法：给定父级 `Rect` 和一个回调函数，它为每个子控件计算并输出对应的 `Rect`。
- `update_with_context` 应用设备感知缩放。重写此方法的布局（如 `BoxLayout`）会根据 `context.layout_scale` 缩放间距/边距。
- `as_any` / `as_any_mut` 支持向下转换为具体布局类型，用于内省和布局检查器。

### `SizePolicy`

每个在盒子布局中的控件项可以声明其尺寸偏好：

```rust
pub enum SizePolicy {
    Fixed,      // 使用确切的 constraints.max（或 constraints.min）—— 不参与拉伸分配
    Preferred,  // 使用自然/期望尺寸，参与拉伸协商
    Expanding,  // 增长以消耗剩余空间（BoxLayout 项的默认值）
}
```

### `LayoutConstraints`

每个子项的最小/最大限制，在布局计算中强制执行：

```rust
pub struct LayoutConstraints {
    pub min: u32,
    pub max: Option<u32>,
}
```

使用 `set_constraints(widget_id, LayoutConstraints::new(80, Some(200)))` 将控件在主轴方向限制在最小 80 像素和最大 200 像素之间。

### `LayoutContext`

通过 `Layout::update_with_context` 传递的设备适配参数：

```rust
pub struct LayoutContext {
    pub layout_scale: f32,    // 间距、边距、内边距的缩放因子
    pub font_scale: f32,      // 字体/度量尺寸的缩放因子
    pub min_touch_size: Size,  // 最小触摸目标尺寸（默认：32×32）
}
```

默认情况下，所有缩放因子均为 `1.0`，`min_touch_size` 为 `Size::new(32, 32)`。在高 DPI 显示器上，`layout_scale` 可能为 `2.0` 或更高。

**示例 — 支持 DPI 的盒布局：**

```rust
use rust_widgets::layout::{BoxLayout, LayoutContext};
use rust_widgets::core::{Orientation, Rect};

let layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
let ctx = LayoutContext {
    layout_scale: 2.0,
    font_scale: 2.0,
    ..Default::default()
};

// 间距和边距由缩放因子自动加倍
let mut rects = std::collections::HashMap::new();
layout.update_with_context(
    Rect::new(0, 0, 600, 100),
    &ctx,
    &mut |id, rect| { rects.insert(id, rect); }
);
```

---

## 布局管理器

### BoxLayout — 线性行/列

将子项排列在单行（`Horizontal`）或单列（`Vertical`）中，支持按拉伸权重进行比例分配和可选的间隔项。

```rust
let mut layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
layout.add_widget(button_id, 1);   // stretch=1
layout.add_spacer(1);              // stretch=1 间隔（无控件）
layout.add_widget(label_id, 2);    // stretch=2 → 获得 2 倍空间

layout.set_constraints(label_id, LayoutConstraints::new(100, Some(300)));
layout.set_size_policy(button_id, SizePolicy::Fixed);
```

空间分配算法：
1. `Fixed` 策略的项精确获得其 `constraints.max`（若无最大值则为 `min`）。
2. `Expanding`/`Preferred` 项按 `stretch` 比例分割剩余空间。
3. 如果总分配空间 ≠ 可用空间，算法会迭代增大/缩小分配，同时遵守每个项的限制。

**命名别名 — `HBoxLayout` 和 `VBoxLayout`：**

```rust
let mut hbox = HBoxLayout::new(4, 8);  // spacing=4, margin=8, 水平
hbox.add_widget(widget_a, 1);
hbox.add_spacer(2);
hbox.add_widget(widget_b, 1);

let mut vbox = VBoxLayout::new(2, 4);  // spacing=2, margin=4, 垂直
vbox.add_widget(header, 1);
vbox.add_widget(body, 3);             // body 获得 3 倍高度
```

### FlexLayout — CSS Flexbox

一个完整的 flexbox 实现，受 CSS 启发，支持方向、换行、对齐、间隙、内边距以及每个项的 `align_self` 覆盖。

```rust
use rust_widgets::layout::{
    FlexLayout, FlexDirection, FlexWrap, JustifyContent, AlignItems,
};

let mut flex = FlexLayout::with_params(
    FlexDirection::Row,
    FlexWrap::Wrap,
    JustifyContent::SpaceBetween,
    AlignItems::Center,
    8,   // gap（间隙）
    4,   // padding（内边距）
);

flex.add_widget(item_a, 1);  // flex_grow=1
flex.add_widget(item_b, 2);  // flex_grow=2（获得 2 倍额外空间）
```

**枚举一览：**

| 枚举 | 变体 |
|---|---|
| `FlexDirection` | `Row`, `RowReverse`, `Column`, `ColumnReverse` |
| `FlexWrap` | `NoWrap`, `Wrap`, `WrapReverse` |
| `JustifyContent` | `FlexStart`, `FlexEnd`, `Center`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly` |
| `AlignItems` | `Stretch`, `FlexStart`, `FlexEnd`, `Center`, `Baseline` |

**通过 `FlexItem` 进行每项覆盖：**

```rust
flex.items_mut().get_mut(0).unwrap().align_self = Some(AlignItems::FlexEnd);
flex.items_mut().get_mut(0).unwrap().min_size = Some(50);
flex.items_mut().get_mut(0).unwrap().max_size = Some(200);
```

**Flexbox 行反向示例：**

```rust
let flex = FlexLayout::with_params(
    FlexDirection::RowReverse,
    FlexWrap::NoWrap,
    JustifyContent::FlexStart,
    AlignItems::Stretch,
    0, 0,
);
// 子项从右到左排列
```

### GridLayout — 固定单元格网格

具有固定行和列的网格，控件放置在明确的 `(row, col)` 位置。

```rust
let mut grid = GridLayout::new(3, 4, 2, 4); // rows=3, cols=4, spacing=2, margin=4
grid.set_widget(0, 0, header_id);   // 第 0 行，第 0 列
grid.set_widget(0, 1, title_id);     // 第 0 行，第 1 列
grid.set_widget(1, 0, content_id);   // 第 1 行，第 0 列

// 单元格均匀分割：每个单元格宽度 = (可用宽度 / 列数)，高度 = (可用高度 / 行数)
// add_widget 填充第一个空单元格（按行主序）
grid.add_widget(footer_id, 0);       // 填充第 1 行，第 1 列
```

**用于比例行/列尺寸的拉伸因子：**

```rust
grid.set_column_stretch(2);  // 列获得 2 倍拉伸因子
grid.set_row_stretch(1);     // 行获得 1 倍拉伸因子
```

### UniformGridLayout — 等大单元格网格

与 `GridLayout` 类似，但保证所有单元格具有相同的尺寸。

```rust
let mut grid = UniformGridLayout::new(3, 4, 2, 0);
for r in 0..3 {
    for c in 0..4 {
        grid.set_widget(r, c, widget_ids[r * 4 + c]);
    }
}
// 每个单元格报告相同的（宽度, 高度），无论内容如何
```

### StackLayout — 卡片堆叠

从有序列表中一次显示一个子项，类似标签面板或向导页面。

```rust
let mut stack = StackLayout::new();
stack.add_widget(page_one, 0);
stack.add_widget(page_two, 0);
stack.add_widget(page_three, 0);

stack.set_current_index(1);  // 显示 page_two
// 仅 page_two 接收布局更新 — 其他项不可见

// 页面导航
stack.set_current_index(stack.current_index() + 1);  // 显示 page_three

// 移除页面
stack.remove_widget(page_one);
// current_index 会根据需要自动调整
```

### SplitterLayout — 可调大小分割面板

按面板比例分配空间，支持水平或垂直方向。

```rust
let mut splitter = SplitterLayout::new(Orientation::Horizontal, 2); // gap=2
splitter.add_pane(left_pane, 3);   // ratio=3（获得 3/5 的空间）
splitter.add_pane(right_pane, 2);  // ratio=2（获得 2/5 的空间）

// 归一化比例，使其总和为 1.0
splitter.normalize_ratios();

// 运行时调整
splitter.set_ratio(0, 0.7).unwrap();
splitter.set_ratio(1, 0.3).unwrap();

// 切换方向
splitter.set_orientation(Orientation::Vertical);
```

### AbsoluteLayout — 9 锚点定位

使用 `Anchor` 参考点对子项进行定位。每个子项都有一个锚点位置，以及可选的 `Constraint` 用于最小/最大值和宽高比。

```rust
use rust_widgets::layout::{AbsoluteLayout, AbsolutePosition, Anchor, Constraint};

let mut abs = AbsoluteLayout::new();
abs.add_child(
    Box::new(my_widget),
    AbsolutePosition::new(100, 50).with_anchor(Anchor::Center, 0, 0),
);
abs.add_child_with_constraint(
    Box::new(my_other_widget),
    AbsolutePosition::new(10, 10).with_anchor_only(Anchor::TopRight),
    Constraint::new().with_min_width(50).with_aspect_ratio(1.5),
);

// 在父矩形内布局
let positions = abs.layout(Rect::new(0, 0, 600, 400));
```

**九个锚点：**

<div style="display:none"></div>
| 锚点 | 对齐边缘 |
|---|---|
| `TopLeft` | `(x, y)` 的左上角 |
| `TopCenter` | 顶部边缘，水平居中于 `x` |
| `TopRight` | `(x, y)` 的右上角 |
| `CenterLeft` | 左侧边缘，垂直居中于 `y` |
| `Center` | 完全居中于 `(x, y)` |
| `CenterRight` | 右侧边缘，垂直居中于 `y` |
| `BottomLeft` | `(x, y)` 的左下角 |
| `BottomCenter` | 底部边缘，水平居中于 `x` |
| `BottomRight` | `(x, y)` 的右下角 |

### CenterLayout — 单子项居中

将单个子项居中，支持可配置的宽/高因子（`0.0..=1.0`）。

```rust
let mut center = CenterLayout::with_factors(0.8, 0.6);
center.add_widget(logo_id, 0);
// 子项获得父级 80% 宽度、60% 高度，完美居中

// 填充至 100%
center.set_width_factor(1.0);
center.set_height_factor(1.0);
```

### ConstraintLayout — 基于锚点的约束

使用约束规则相对于彼此定位子项（非完整的 Cassowary 求解器）。约束定义边到边、中心或尺寸关系。

```rust
use rust_widgets::layout::{ConstraintLayout, ConstraintType};

let mut layout = ConstraintLayout::new();
layout.add_widget(button_a, 0);  // 基础控件
layout.add_widget(button_b, 0);

// button_b 左边缘 = button_a 右边缘 + 10 px
layout.add_constraint(button_b, button_a, ConstraintType::LeftToRight, 10, 1.0);

// label 水平居中于 button_a
layout.add_constraint(label_id, button_a, ConstraintType::CenterX, 0, 1.0);

// field 宽度 = 父级宽度 × 0.8
layout.add_constraint(field_id, parent_id, ConstraintType::Width, 0, 0.8);

// 宽高比 16:9
layout.add_constraint(video_id, video_id, ConstraintType::AspectRatio(16.0 / 9.0), 0, 1.0);
```

**所有约束类型：**

| 约束 | 含义 |
|---|---|
| `LeftToLeft` | 控件左边缘 = 目标左边缘 + 偏移量 |
| `LeftToRight` | 控件左边缘 = 目标右边缘 + 偏移量 |
| `RightToLeft` | 控件右边缘 = 目标左边缘 + 偏移量 |
| `RightToRight` | 控件右边缘 = 目标右边缘 + 偏移量 |
| `TopToTop` | 控件顶边缘 = 目标顶边缘 + 偏移量 |
| `TopToBottom` | 控件顶边缘 = 目标底边缘 + 偏移量 |
| `BottomToTop` | 控件底边缘 = 目标顶边缘 + 偏移量 |
| `BottomToBottom` | 控件底边缘 = 目标底边缘 + 偏移量 |
| `CenterX` | 控件中心 X = 目标中心 X + 偏移量 |
| `CenterY` | 控件中心 Y = 目标中心 Y + 偏移量 |
| `Width` | 控件宽度 = 目标宽度 × 乘数 |
| `Height` | 控件高度 = 目标高度 × 乘数 |
| `AspectRatio(f32)` | 宽度 = 高度 × 比例（保持适配） |

### FlowLayout — 水平/垂直流动换行

将子项按顺序排列成行或列，支持可选的换行、对齐和内边距。

```rust
use rust_widgets::layout::{FlowLayout, FlowLayoutConfig, FlowDirection, FlowAlignment};

let config = FlowLayoutConfig {
    direction: FlowDirection::Horizontal,
    alignment: FlowAlignment::Center,
    spacing: 8,
    padding: 12,
    wrap: true,   // 超出时换行到下一行
};

let mut flow = FlowLayout::with_config(config);
flow.add_child(Box::new(widget_one));
flow.add_child(Box::new(widget_two));
flow.add_child(Box::new(widget_three));

let positions = flow.layout(Rect::new(0, 0, 400, 200));
```

**FlowAlignment 变体：** `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`

> **注意：** `FlowLayout` 存储 `Box<dyn Widget>` 子项而非 `ObjectId`。使用 `add_child()`/`remove_child()` 替代 `add_widget()`。

### WrapLayout — 溢出换行

当子项超出容器宽度时，自动换行到下一行（水平）或下一列（垂直）。

```rust
use rust_widgets::layout::{WrapLayout, WrapDirection, WrapAlignment};

let mut wrap = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 4, 8);
wrap.add_widget(item_1, 0);
wrap.add_widget(item_2, 0);
wrap.add_widget(item_3, 0);
wrap.add_widget(item_4, 0);

// 为换行算法设置每个子项的尺寸
wrap.set_child_size(item_1, Size::new(100, 30));
wrap.set_child_size(item_2, Size::new(80, 30));

// 在较窄的容器中，第 3 和第 4 项会自动换行到第二行
```

`WrapAlignment` 支持：`Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`。

### FormLayout — 标签/字段双列

将标签—字段对排列成双列表单，宽度比例为 1:2。

```rust
let mut form = FormLayout::new(4, 8);
form.add_row_pair(name_label_id, name_field_id);
form.add_row_pair(email_label_id, email_field_id);
form.add_row_pair(password_label_id, password_field_id);

// 标签获得 1/3 宽度，字段获得 2/3 宽度
// 所有行在垂直方向上均匀分布

// 添加独立的通栏项目
form.add_widget(submit_button_id, 0);
```

### AspectRatioLayout — 强制宽高比

将单个子项约束为特定的宽高比，可选择以父容器尺寸为边界。

```rust
let layout = AspectRatioLayout::new(16.0 / 9.0, true);  // 16:9，遵守父容器边界
// 子项居中并按比例缩放以适配父容器

// 不设父容器边界（可能溢出）
let free_layout = AspectRatioLayout::new(2.0, false);
// 以父容器宽度为基准，推导高度
```

### KeyboardAwareLayout — 移动键盘偏移

包装任意内部布局，当键盘弹出时将子项向上偏移键盘高度，防止焦点输入被遮挡。

```rust
let inner = Box::new(VBoxLayout::new(4, 8));
let mut keyboard = KeyboardAwareLayout::new(inner, 200); // animation_duration=200ms

// 键盘出现时
keyboard.set_keyboard_offset(300);  // 将内容上移 300 px

// 键盘关闭时
keyboard.set_keyboard_offset(0);    // 恢复原始位置

// 访问内部布局
keyboard.inner_layout_mut().add_widget(input_field, 1);
```

---

## LayoutInspector — 诊断工具

`LayoutInspector` 在不修改控件位置的情况下，在运行时检测常见布局问题：

```rust
use rust_widgets::layout::LayoutInspector;

LayoutInspector::enable();

// 记录控件几何信息
LayoutInspector::record_geometry(widget_id, Rect::new(0, 0, 100, 50));

// 运行诊断
let report = LayoutInspector::run_once();
println!("{}", report);  // 详细诊断报告

// 检查特定条件
if report.has_errors() {
    eprintln!("检测到布局错误！");
}
if report.has_warnings() {
    println!("警告：{}", report.count_by_severity());
}

LayoutInspector::disable();
```

**检测到的问题：**

| 类别 | 问题 | 严重级别 |
|---|---|---|
| **结构性** | 孤儿控件（无父级） | 警告 |
| **结构性** | 声明了子控件但布局为空 | 信息 |
| **几何性** | 零宽度或零高度的矩形 | 错误 |
| **几何性** | 兄弟矩形重叠（排除触摸目标） | 警告 |

**建议**会为每个检测到的问题自动生成，包含标题、摘要和详细的修复建议。

**注册原生平台布局**以供内省：

```rust
LayoutInspector::register_native_layout(parent_id, "NavBar", 5, "NativeNavBar");
```

---

## 通过 JSON 声明式布局

布局可以使用 JSON 声明，并在运行时通过 `DeclarativeLayoutKind` 实例化：

```rust
use rust_widgets::json::layout::{parse_layout_kind, create_layout_from_kind, DeclarativeLayoutKind};

let json = r#"{"type": "vbox", "spacing": 4, "margin": 8}"#;
let value: serde_json::Value = serde_json::from_str(json).unwrap();
let kind = parse_layout_kind(&value).unwrap();
let layout = create_layout_from_kind(&kind);
```

**支持的 JSON 布局类型：**

| JSON `"type"` | 别名 | 参数 |
|---|---|---|
| `"hbox"` | `"HBox"`, `"horizontal"` | `spacing`, `margin` |
| `"vbox"` | `"VBox"`, `"vertical"` | `spacing`, `margin` |
| `"grid"` | `"Grid"` | `columns`, `spacing`, `margin` |
| `"stack"` | `"Stack"` | `spacing` |
| `"splitter"` | `"Splitter"` | `orientation` (`"horizontal"`/`"vertical"`), `margin` |
| `"form"` | `"Form"` | `spacing`, `margin` |

**子布局属性**可以指定拉伸、网格位置和跨列/跨行：

```json
{
    "stretch": 3,
    "col": 1,
    "row": 2,
    "col_span": 2,
    "row_span": 1
}
```

使用 `ChildLayoutAttrs::from_value(&json_value)` 解析它们。

---

## 自定义布局创建

实现 `Layout` trait 以创建自定义布局管理器：

```rust
use rust_widgets::layout::Layout;
use rust_widgets::core::{ObjectId, Rect};

pub struct DiagLayout {
    children: Vec<ObjectId>,
}

impl DiagLayout {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }
}

impl Layout for DiagLayout {
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.children.push(widget_id);
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|id| *id != widget_id);
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        self.children.clone()
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.contains(&id)
    }

    fn clear(&mut self) {
        self.children.clear();
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        for (i, &child) in self.children.iter().enumerate() {
            let offset = i as u32 * 20;
            widgets(
                child,
                Rect::new(
                    rect.x + offset as i32,
                    rect.y + offset as i32,
                    100,
                    40,
                ),
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
```

---

## 常见模式

### 居中的模态对话框

```rust
let mut root = VBoxLayout::new(0, 0);
let mut center = CenterLayout::with_factors(0.5, 0.4);
center.add_widget(dialog_id, 0);
// 将 CenterLayout 嵌套在 VBox 内 — 但 Layout 不是 Widget。
// 请使用内部持有布局的容器控件。
```

### 工具栏 + 内容区域 + 状态栏

```rust
let mut vbox = VBoxLayout::new(0, 0);
vbox.add_widget(toolbar_id, 0);   // 固定高度工具栏
vbox.add_widget(content_id, 1);   // stretch=1 → 填充剩余空间
vbox.add_widget(status_id, 0);    // 固定高度状态栏

vbox.set_constraints(toolbar_id, LayoutConstraints::new(40, Some(40)));
vbox.set_size_policy(toolbar_id, SizePolicy::Fixed);
vbox.set_constraints(status_id, LayoutConstraints::new(24, Some(24)));
vbox.set_size_policy(status_id, SizePolicy::Fixed);
```

### 响应式弹性换行

```rust
let flex = FlexLayout::with_params(
    FlexDirection::Row,
    FlexWrap::Wrap,          // 空间不足时换行
    JustifyContent::FlexStart,
    AlignItems::Stretch,
    4, 8,
);

for card in cards {
    flex.add_widget(card, 1);
    flex.set_child_sizes(card, Some(Size::new(200, 150)));
}
// 当容器太窄时，卡片自动换行到下一行
```

### DPI 感知的自适应间距

```rust
let ctx = LayoutContext {
    layout_scale: 2.0,
    font_scale: 1.5,
    min_touch_size: Size::new(44, 44),  // 平板尺寸的触摸目标
};

layout.update_with_context(rect, &ctx, &mut |id, rect| {
    // 子矩形已应用 DPI 缩放的间距
});
```

### 支持键盘感知的多页面向导

```rust
let mut stack = StackLayout::new();
stack.add_widget(page_1, 0);
stack.add_widget(page_2, 0);

let mut keyboard_layout = KeyboardAwareLayout::new(
    Box::new(stack), 200
);

// 键盘感知堆叠：键盘出现时页面向上偏移
keyboard_layout.set_keyboard_offset(280);
```
