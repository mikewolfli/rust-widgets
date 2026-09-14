# 版面配置系統

`rust-widgets` 版面配置系統透過一個由 14 個版面配置管理器組成的可插拔架構，在父容器內定位和調整控制項大小。每個版面配置都實作 `Layout` 特徵，並支援透過 `LayoutContext` 進行 DPI 感知縮放。

---

## 核心概念

### `Layout` 特徵

每個版面配置管理器都實作 `Layout` 這個核心抽象：

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

重點：

- `add_widget` 接受一個**伸縮因子**，供比例式版面配置使用（BoxLayout、SplitterLayout、FlexLayout）。伸縮值越高，能取得的空間越多。
- `update` 是核心方法：給定父容器的 `Rect` 和一個回呼，它會計算並發出每個子控制項的 `Rect`。
- `update_with_context` 套用裝置感知縮放。覆寫它的版面配置（例如 `BoxLayout`）會依 `context.layout_scale` 縮放間距／邊距。
- `as_any` / `as_any_mut` 允許向下轉型為具體版面配置型別，以進行自我檢查和版面配置檢查器。

### `SizePolicy`

每個框版面配置中的控制項項目可以宣告其大小偏好：

```rust
pub enum SizePolicy {
    Fixed,      // 使用確切的 constraints.max（或 constraints.min）——不分配伸縮空間
    Preferred,  // 使用自然/期望大小，參與伸縮協商
    Expanding,  // 成長以消耗剩餘空間（BoxLayout 項目的預設值）
}
```

### `LayoutConstraints`

每個子項目在版面配置計算期間強制套用的最小／最大限制：

```rust
pub struct LayoutConstraints {
    pub min: u32,
    pub max: Option<u32>,
}
```

使用 `set_constraints(widget_id, LayoutConstraints::new(80, Some(200)))`，即可將控制項在主要軸向上的尺寸限制在最小 80 px、最大 200 px 之間。

### `LayoutContext`

透過 `Layout::update_with_context` 傳遞的裝置適應參數：

```rust
pub struct LayoutContext {
    pub layout_scale: f32,    // Scale factor for spacing, margins, padding
    pub font_scale: f32,      // Scale factor for font/metric sizes
    pub min_touch_size: Size,  // Minimum touch-target size (default: 32×32)
}
```

依預設，所有縮放值皆為 `1.0`，且 `min_touch_size` 為 `Size::new(32, 32)`。在高 DPI 顯示器上，`layout_scale` 可能為 `2.0` 或更高。

**範例——DPI 感知的框版面配置：**

```rust
use rust_widgets::layout::{BoxLayout, LayoutContext};
use rust_widgets::core::{Orientation, Rect};

let layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
let ctx = LayoutContext {
    layout_scale: 2.0,
    font_scale: 2.0,
    ..Default::default()
};

// Spacing and margin are automatically doubled by the scale factor
let mut rects = std::collections::HashMap::new();
layout.update_with_context(
    Rect::new(0, 0, 600, 100),
    &ctx,
    &mut |id, rect| { rects.insert(id, rect); }
);
```

---

## 版面配置管理器

### BoxLayout——線性行/列

以單行（`Horizontal`）或單列（`Vertical`）排列子項目，使用伸縮加權比例分配和可選間隔項目：

```rust
let mut layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
layout.add_widget(button_id, 1);   // stretch=1
layout.add_spacer(1);              // stretch=1 spacer (no widget)
layout.add_widget(label_id, 2);    // stretch=2 → gets 2× the space

layout.set_constraints(label_id, LayoutConstraints::new(100, Some(300)));
layout.set_size_policy(button_id, SizePolicy::Fixed);
```

空間分配演算法：

1. `Fixed` 原則的項目恰好取得其 `constraints.max`（若無上限則取 `min`）。
2. `Expanding`／`Preferred` 項目依 `stretch` 比例分配剩餘空間。
3. 若總分配量 ≠ 可用空間，演算法會反覆擴張／縮減分配量，同時遵守每個項目的限制。

**命名別名——`HBoxLayout` 與 `VBoxLayout`：**

```rust
let mut hbox = HBoxLayout::new(4, 8);  // spacing=4, margin=8, horizontal
hbox.add_widget(widget_a, 1);
hbox.add_spacer(2);
hbox.add_widget(widget_b, 1);

let mut vbox = VBoxLayout::new(2, 4);  // spacing=2, margin=4, vertical
vbox.add_widget(header, 1);
vbox.add_widget(body, 3);             // body gets 3× the height
```

### FlexLayout——CSS Flexbox

一個完整的 flexbox 實作，靈感來自 CSS，支援方向、換行、對齊、間距、內距和每個項目的 `align_self` 覆寫：

```rust
use rust_widgets::layout::{
    FlexLayout, FlexDirection, FlexWrap, JustifyContent, AlignItems,
};

let mut flex = FlexLayout::with_params(
    FlexDirection::Row,
    FlexWrap::Wrap,
    JustifyContent::SpaceBetween,
    AlignItems::Center,
    8,   // gap
    4,   // padding
);

flex.add_widget(item_a, 1);  // flex_grow=1
flex.add_widget(item_b, 2);  // flex_grow=2 (gets 2× share of extra space)
```

**列舉一覽：**

| 列舉 | 變體 |
|---|---|
| `FlexDirection` | `Row`、`RowReverse`、`Column`、`ColumnReverse` |
| `FlexWrap` | `NoWrap`、`Wrap`、`WrapReverse` |
| `JustifyContent` | `FlexStart`、`FlexEnd`、`Center`、`SpaceBetween`、`SpaceAround`、`SpaceEvenly` |
| `AlignItems` | `Stretch`、`FlexStart`、`FlexEnd`、`Center`、`Baseline` |

**透過 `FlexItem` 進行每個項目的覆寫：**

```rust
flex.items_mut().get_mut(0).unwrap().align_self = Some(AlignItems::FlexEnd);
flex.items_mut().get_mut(0).unwrap().min_size = Some(50);
flex.items_mut().get_mut(0).unwrap().max_size = Some(200);
```

**Flexbox row-reverse 範例：**

```rust
let flex = FlexLayout::with_params(
    FlexDirection::RowReverse,
    FlexWrap::NoWrap,
    JustifyContent::FlexStart,
    AlignItems::Stretch,
    0, 0,
);
// Children are laid out right-to-left
```

### GridLayout——固定儲存格網格

具有固定行和列的網格，控制項放置在明確的 `(row, col)` 位置：

```rust
let mut grid = GridLayout::new(3, 4, 2, 4); // rows=3, cols=4, spacing=2, margin=4
grid.set_widget(0, 0, header_id);   // row 0, col 0
grid.set_widget(0, 1, title_id);     // row 0, col 1
grid.set_widget(1, 0, content_id);   // row 1, col 0

// Cells are evenly divided: each cell = (available / cols) wide, (available / rows) tall
// add_widget fills the first empty cell (row-major order)
grid.add_widget(footer_id, 0);       // fills row 1, col 1
```

**用於比例式行／列尺寸的伸縮因子：**

```rust
grid.set_column_stretch(2);  // columns get 2× stretch factor
grid.set_row_stretch(1);     // rows get 1× stretch factor
```

### UniformGridLayout——等尺寸網格儲存格

類似 `GridLayout`，但保證所有儲存格具有完全相同的尺寸。

```rust
let mut grid = UniformGridLayout::new(3, 4, 2, 0);
for r in 0..3 {
    for c in 0..4 {
        grid.set_widget(r, c, widget_ids[r * 4 + c]);
    }
}
// Every cell reports the same (width, height) regardless of content
```

### StackLayout——卡片堆疊

一次顯示一個子項目，來自有序列表，像標籤面板或精靈：

```rust
let mut stack = StackLayout::new();
stack.add_widget(page_one, 0);
stack.add_widget(page_two, 0);
stack.add_widget(page_three, 0);

stack.set_current_index(1);  // show page_two
// Only page_two receives a layout update — others are invisible

// Navigate pages
stack.set_current_index(stack.current_index() + 1);  // show page_three

// Remove a page
stack.remove_widget(page_one);
// current_index auto-adjusts if needed
```

### SplitterLayout——可調整大小的分割面板

按面板比例分配空間，支援水平或垂直方向：

```rust
let mut splitter = SplitterLayout::new(Orientation::Horizontal, 2); // gap=2
splitter.add_pane(left_pane, 3);   // ratio=3 (gets 3/5 of space)
splitter.add_pane(right_pane, 2);  // ratio=2 (gets 2/5 of space)

// Normalize ratios so they sum to 1.0
splitter.normalize_ratios();

// Adjust at runtime
splitter.set_ratio(0, 0.7).unwrap();
splitter.set_ratio(1, 0.3).unwrap();

// Switch orientation
splitter.set_orientation(Orientation::Vertical);
```

### AbsoluteLayout——9 錨點定位

使用 `Anchor` 參考點定位子項目。每個子項目都有一個錨點位置，並可選擇性地搭配 `Constraint`，以指定最小／最大值與寬高比。

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

// Layout within a parent rect
let positions = abs.layout(Rect::new(0, 0, 600, 400));
```

**九個錨點：**

<div style="display:none"></div>

| 錨點 | 對齊邊緣 |
|---|---|
| `TopLeft` | `(x, y)` 的左上角 |
| `TopCenter` | 上邊緣，在 `x` 處水平置中 |
| `TopRight` | `(x, y)` 的右上角 |
| `CenterLeft` | 左邊緣，在 `y` 處垂直置中 |
| `Center` | 完全置中於 `(x, y)` |
| `CenterRight` | 右邊緣，在 `y` 處垂直置中 |
| `BottomLeft` | `(x, y)` 的左下角 |
| `BottomCenter` | 下邊緣，在 `x` 處水平置中 |
| `BottomRight` | `(x, y)` 的右下角 |

### CenterLayout——單一子項目置中

將單一子項目置中，可設定寬度/高度因子（`0.0..=1.0`）：

```rust
let mut center = CenterLayout::with_factors(0.8, 0.6);
center.add_widget(logo_id, 0);
// Child gets 80% of parent width, 60% of parent height, dead-center

// Fill to 100%
center.set_width_factor(1.0);
center.set_height_factor(1.0);
```

### ConstraintLayout——基於錨點的限制

使用限制規則將子項目相對於彼此定位（並非完整的 Cassowary 求解器）。限制規則定義邊對邊、中心或尺寸關係。

```rust
use rust_widgets::layout::{ConstraintLayout, ConstraintType};

let mut layout = ConstraintLayout::new();
layout.add_widget(button_a, 0);  // base widget
layout.add_widget(button_b, 0);

// button_b left edge = button_a right edge + 10 px
layout.add_constraint(button_b, button_a, ConstraintType::LeftToRight, 10, 1.0);

// label centered horizontally on button_a
layout.add_constraint(label_id, button_a, ConstraintType::CenterX, 0, 1.0);

// field width = parent width × 0.8
layout.add_constraint(field_id, parent_id, ConstraintType::Width, 0, 0.8);

// Aspect ratio 16:9
layout.add_constraint(video_id, video_id, ConstraintType::AspectRatio(16.0 / 9.0), 0, 1.0);
```

**所有限制型別：**

| 限制 | 含義 |
|---|---|
| `LeftToLeft` | 控制項左邊 = 目標左邊 + 位移 |
| `LeftToRight` | 控制項左邊 = 目標右邊 + 位移 |
| `RightToLeft` | 控制項右邊 = 目標左邊 + 位移 |
| `RightToRight` | 控制項右邊 = 目標右邊 + 位移 |
| `TopToTop` | 控制項上邊 = 目標上邊 + 位移 |
| `TopToBottom` | 控制項上邊 = 目標下邊 + 位移 |
| `BottomToTop` | 控制項下邊 = 目標上邊 + 位移 |
| `BottomToBottom` | 控制項下邊 = 目標下邊 + 位移 |
| `CenterX` | 控制項中心 x = 目標中心 x + 位移 |
| `CenterY` | 控制項中心 y = 目標中心 y + 位移 |
| `Width` | 控制項寬度 = 目標寬度 × 乘數 |
| `Height` | 控制項高度 = 目標高度 × 乘數 |
| `AspectRatio(f32)` | 寬度 = 高度 × 比例（維持以符合容器） |

### FlowLayout——水平/垂直流動換行

按順序排列子項目，可選換行、對齊和內距：

```rust
use rust_widgets::layout::{FlowLayout, FlowLayoutConfig, FlowDirection, FlowAlignment};

let config = FlowLayoutConfig {
    direction: FlowDirection::Horizontal,
    alignment: FlowAlignment::Center,
    spacing: 8,
    padding: 12,
    wrap: true,   // wrap to next row when overflowing
};

let mut flow = FlowLayout::with_config(config);
flow.add_child(Box::new(widget_one));
flow.add_child(Box::new(widget_two));
flow.add_child(Box::new(widget_three));

let positions = flow.layout(Rect::new(0, 0, 400, 200));
```

**`FlowAlignment` 變體：**`Start`、`Center`、`End`、`SpaceBetween`、`SpaceAround`

> **注意：**`FlowLayout` 儲存的是 `Box<dyn Widget>` 子項目，而非 `ObjectId`。請使用 `add_child()`／`remove_child()` 取代 `add_widget()`。

### WrapLayout——溢出換行

當子項目超出容器寬度時，自動換行至下一列（水平）或下一欄（垂直）。

```rust
use rust_widgets::layout::{WrapLayout, WrapDirection, WrapAlignment};

let mut wrap = WrapLayout::new(WrapDirection::Horizontal, WrapAlignment::Start, 4, 8);
wrap.add_widget(item_1, 0);
wrap.add_widget(item_2, 0);
wrap.add_widget(item_3, 0);
wrap.add_widget(item_4, 0);

// Set per-child sizes for the wrapping algorithm
wrap.set_child_size(item_1, Size::new(100, 30));
wrap.set_child_size(item_2, Size::new(80, 30));

// With a narrow container, items 3 and 4 automatically wrap to the second row
```

`WrapAlignment` 支援：`Start`、`Center`、`End`、`SpaceBetween`、`SpaceAround`。

### FormLayout——標籤/欄位雙欄

以 1：2 寬度比例在雙欄表單中排列標籤-欄位配對：

```rust
let mut form = FormLayout::new(4, 8);
form.add_row_pair(name_label_id, name_field_id);
form.add_row_pair(email_label_id, email_field_id);
form.add_row_pair(password_label_id, password_field_id);

// Labels get 1/3 of the width, fields get 2/3
// All rows are evenly distributed vertically

// Add standalone full-width items
form.add_widget(submit_button_id, 0);
```

### AspectRatioLayout——強制寬高比

將單一子項目限制為特定的寬高比，並可選擇性地受父容器尺寸約束。

```rust
let layout = AspectRatioLayout::new(16.0 / 9.0, true);  // 16:9, respect parent bounds
// Child is centered and sized to fit within parent

// Without parent bounds (may overflow)
let free_layout = AspectRatioLayout::new(2.0, false);
// Uses parent width as base, derives height
```

### KeyboardAwareLayout——行動鍵盤偏移

包裝任何內部版面配置，向上移動子項目以補償鍵盤高度，避免聚焦中的輸入欄位被遮擋。

```rust
let inner = Box::new(VBoxLayout::new(4, 8));
let mut keyboard = KeyboardAwareLayout::new(inner, 200); // animation_duration=200ms

// When keyboard appears
keyboard.set_keyboard_offset(300);  // shift content up by 300 px

// When keyboard dismisses
keyboard.set_keyboard_offset(0);    // restore original positions

// Access inner layout
keyboard.inner_layout_mut().add_widget(input_field, 1);
```

---

## LayoutInspector——診斷

`LayoutInspector` 在執行時期偵測常見版面配置問題，而不修改控制項位置：

```rust
use rust_widgets::layout::LayoutInspector;

LayoutInspector::enable();

// Record widget geometries during layout
LayoutInspector::record_geometry(widget_id, Rect::new(0, 0, 100, 50));

// Run diagnostics
let report = LayoutInspector::run_once();
println!("{}", report);  // detailed diagnostic report

// Check specific conditions
if report.has_errors() {
    eprintln!("Layout errors detected!");
}
if report.has_warnings() {
    println!("Warnings: {}", report.count_by_severity());
}

LayoutInspector::disable();
```

**偵測到的問題：**

| 類別 | 問題 | 嚴重性 |
|---|---|---|
| **結構性** | 孤兒控制項（無父容器） | 警告 |
| **結構性** | 已宣告子控制項的空版面配置 | 資訊 |
| **幾何性** | 寬度或高度為零的矩形 | 錯誤 |
| **幾何性** | 重疊的兄弟矩形（不含觸控目標） | 警告 |

每個偵測到的問題都會自動產生**建議**，包含標題、摘要和詳細的修正建議。

**註冊原生平台版面配置**以供自我檢查：

```rust
LayoutInspector::register_native_layout(parent_id, "NavBar", 5, "NativeNavBar");
```

---

## 宣告式版面配置 via JSON

版面配置可以在 JSON 中宣告並在執行時期使用 `DeclarativeLayoutKind` 實例化：

```rust
use rust_widgets::json::layout::{parse_layout_kind, create_layout_from_kind, DeclarativeLayoutKind};

let json = r#"{"type": "vbox", "spacing": 4, "margin": 8}"#;
let value: serde_json::Value = serde_json::from_str(json).unwrap();
let kind = parse_layout_kind(&value).unwrap();
let layout = create_layout_from_kind(&kind);
```

**支援的 JSON 版面配置型別：**

| JSON `"type"` | 別名 | 參數 |
|---|---|---|
| `"hbox"` | `"HBox"`、`"horizontal"` | `spacing`、`margin` |
| `"vbox"` | `"VBox"`、`"vertical"` | `spacing`、`margin` |
| `"grid"` | `"Grid"` | `columns`、`spacing`、`margin` |
| `"stack"` | `"Stack"` | `spacing` |
| `"splitter"` | `"Splitter"` | `orientation`（`"horizontal"`／`"vertical"`）、`margin` |
| `"form"` | `"Form"` | `spacing`、`margin` |

**子版面配置屬性**可指定伸縮、網格位置與跨距：

```json
{
    "stretch": 3,
    "col": 1,
    "row": 2,
    "col_span": 2,
    "row_span": 1
}
```

使用 `ChildLayoutAttrs::from_value(&json_value)` 進行解析。

---

## 自訂版面配置建立

實作 `Layout` 特徵來建立自訂版面配置管理器：

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

## 常見模式

### 置中的強制回應對話方塊

```rust
let mut root = VBoxLayout::new(0, 0);
let mut center = CenterLayout::with_factors(0.5, 0.4);
center.add_widget(dialog_id, 0);
// Nest CenterLayout inside a VBox — but Layout isn't a Widget.
// Use a container widget that holds a layout internally.
```

### 工具列 + 內容 + 狀態列

```rust
let mut vbox = VBoxLayout::new(0, 0);
vbox.add_widget(toolbar_id, 0);   // fixed height toolbar
vbox.add_widget(content_id, 1);   // stretch=1 → fills remaining space
vbox.add_widget(status_id, 0);    // fixed height status bar

vbox.set_constraints(toolbar_id, LayoutConstraints::new(40, Some(40)));
vbox.set_size_policy(toolbar_id, SizePolicy::Fixed);
vbox.set_constraints(status_id, LayoutConstraints::new(24, Some(24)));
vbox.set_size_policy(status_id, SizePolicy::Fixed);
```

### 回應式 Flex 換行

```rust
let flex = FlexLayout::with_params(
    FlexDirection::Row,
    FlexWrap::Wrap,          // wrap when out of space
    JustifyContent::FlexStart,
    AlignItems::Stretch,
    4, 8,
);

for card in cards {
    flex.add_widget(card, 1);
    flex.set_child_sizes(card, Some(Size::new(200, 150)));
}
// Cards wrap to the next row when the container is too narrow
```

### DPI 感知適應性間距

```rust
let ctx = LayoutContext {
    layout_scale: 2.0,
    font_scale: 1.5,
    min_touch_size: Size::new(44, 44),  // tablet-sized touch targets
};

layout.update_with_context(rect, &ctx, &mut |id, rect| {
    // Child rects have DPI-scaled spacing applied
});
```

### 具鍵盤感知的多頁精靈

```rust
let mut stack = StackLayout::new();
stack.add_widget(page_1, 0);
stack.add_widget(page_2, 0);

let mut keyboard_layout = KeyboardAwareLayout::new(
    Box::new(stack), 200
);

// Keyboard-aware stack: pages shift up when keyboard appears
keyboard_layout.set_keyboard_offset(280);
```
