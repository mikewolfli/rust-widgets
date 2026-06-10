# 版面配置系統

`rust-widgets` 版面配置系統透過一個由 14 個版面配置管理器組成的可插拔架構，在父容器內定位和調整控制項大小。每個版面配置都實作 `Layout` 特徵，並支援透過 `LayoutContext` 進行 DPI 感知縮放。

---

## 核心概念

### `Layout` 特徵

每個版面配置管理器都實作 `Layout`，這是核心抽象：

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

### `SizePolicy`

每個框版面配置中的控制項項目可以宣告其大小偏好：

```rust
pub enum SizePolicy {
    Fixed,      // 使用確切的 constraints.max（或 constraints.min）——不分配伸縮空間
    Preferred,  // 使用自然/期望大小，參與伸縮協商
    Expanding,  // 成長以消耗剩餘空間（BoxLayout 項目的預設值）
}
```

### `LayoutContext`

透過 `Layout::update_with_context` 傳遞的裝置適應參數：

```rust
pub struct LayoutContext {
    pub layout_scale: f32,
    pub font_scale: f32,
    pub min_touch_size: Size,
}
```

---

## 版面配置管理器

### BoxLayout——線性行/列

以單行（`Horizontal`）或單列（`Vertical`）排列子項目，使用伸縮加權比例分配和可選間隔項目：

```rust
let mut layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
layout.add_widget(button_id, 1);
layout.add_spacer(1);
layout.add_widget(label_id, 2);
```

### FlexLayout——CSS Flexbox

一個完整的 flexbox 實作，靈感來自 CSS，支援方向、換行、對齊、間距、內距和每個項目的 `align_self` 覆蓋：

```rust
let mut flex = FlexLayout::with_params(
    FlexDirection::Row,
    FlexWrap::Wrap,
    JustifyContent::SpaceBetween,
    AlignItems::Center,
    8, 4,
);
```

### GridLayout——固定儲存格網格

具有固定行和列的網格，控制項放置在明確的 `(row, col)` 位置：

```rust
let mut grid = GridLayout::new(3, 4, 2, 4);
grid.set_widget(0, 0, header_id);
grid.set_widget(0, 1, title_id);
```

### StackLayout——卡片堆疊

一次顯示一個子項目，來自有序列表，像標籤面板或精靈：

```rust
let mut stack = StackLayout::new();
stack.add_widget(page_one, 0);
stack.add_widget(page_two, 0);
stack.set_current_index(1);
```

### SplitterLayout——可調整大小的分割面板

按面板比例分配空間，支援水平或垂直方向：

```rust
let mut splitter = SplitterLayout::new(Orientation::Horizontal, 2);
splitter.add_pane(left_pane, 3);
splitter.add_pane(right_pane, 2);
splitter.normalize_ratios();
```

### AbsoluteLayout——9 錨點定位

使用 `Anchor` 參考點定位子項目：

```rust
let mut abs = AbsoluteLayout::new();
abs.add_child(
    Box::new(my_widget),
    AbsolutePosition::new(100, 50).with_anchor(Anchor::Center, 0, 0),
);
```

### CenterLayout——單一子項目置中

將單一子項目置中，可設定寬度/高度因子：

```rust
let mut center = CenterLayout::with_factors(0.8, 0.6);
center.add_widget(logo_id, 0);
```

### ConstraintLayout——基於錨點的限制

使用限制規則將子項目相對於彼此定位：

```rust
let mut layout = ConstraintLayout::new();
layout.add_widget(button_a, 0);
layout.add_constraint(button_b, button_a, ConstraintType::LeftToRight, 10, 1.0);
```

### FlowLayout——水平/垂直流動換行

按順序排列子項目，可選換行、對齊和內距：

```rust
let config = FlowLayoutConfig {
    direction: FlowDirection::Horizontal,
    alignment: FlowAlignment::Center,
    spacing: 8, padding: 12, wrap: true,
};
let mut flow = FlowLayout::with_config(config);
```

### FormLayout——標籤/欄位雙欄

以 1：2 寬度比例在雙欄表單中排列標籤-欄位配對：

```rust
let mut form = FormLayout::new(4, 8);
form.add_row_pair(name_label_id, name_field_id);
```

### AspectRatioLayout——強制寬高比

將單一子項目限制為特定的寬高比：

```rust
let layout = AspectRatioLayout::new(16.0 / 9.0, true);
```

### KeyboardAwareLayout——行動鍵盤偏移

包裝任何內部版面配置，向上移動子項目以補償鍵盤高度：

```rust
let inner = Box::new(VBoxLayout::new(4, 8));
let mut keyboard = KeyboardAwareLayout::new(inner, 200);
```

---

## LayoutInspector——診斷

在執行時期偵測常見版面配置問題，而不修改控制項位置：

```rust
LayoutInspector::enable();
LayoutInspector::record_geometry(widget_id, Rect::new(0, 0, 100, 50));
let report = LayoutInspector::run_once();
```

---

## 宣告式版面配置 via JSON

版面配置可以在 JSON 中宣告並在執行時期實例化：

```rust
let json = r#"{"type": "vbox", "spacing": 4, "margin": 8}"#;
let value: serde_json::Value = serde_json::from_str(json).unwrap();
let kind = parse_layout_kind(&value).unwrap();
let layout = create_layout_from_kind(&kind);
```

---

## 自訂版面配置建立

```rust
pub struct DiagLayout {
    children: Vec<ObjectId>,
}

impl Layout for DiagLayout {
    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        self.children.push(widget_id);
    }
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        for (i, &child) in self.children.iter().enumerate() {
            let offset = i as u32 * 20;
            widgets(child, Rect::new(rect.x + offset as i32, rect.y + offset as i32, 100, 40));
        }
    }
    // ... 其他方法
}
```

---

## 常見模式

### 工具列 + 內容 + 狀態列

```rust
let mut vbox = VBoxLayout::new(0, 0);
vbox.add_widget(toolbar_id, 0);
vbox.add_widget(content_id, 1);
vbox.add_widget(status_id, 0);
vbox.set_constraints(toolbar_id, LayoutConstraints::new(40, Some(40)));
vbox.set_size_policy(toolbar_id, SizePolicy::Fixed);
```

### DPI 感知適應性間距

```rust
let ctx = LayoutContext {
    layout_scale: 2.0,
    font_scale: 1.5,
    min_touch_size: Size::new(44, 44),
};
layout.update_with_context(rect, &ctx, &mut |id, rect| {});
```
