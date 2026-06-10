# Layout System

The `rust-widgets` layout system positions and sizes widgets within their parent containers using a pluggable architecture of 14 layout managers. Each layout implements the `Layout` trait and supports DPI-aware scaling via `LayoutContext`.

---

## Core Concepts

### The `Layout` Trait

Every layout manager implements `Layout`, the central abstraction:

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

Key points:
- `add_widget` accepts a **stretch factor** used by proportional layouts (BoxLayout, SplitterLayout, FlexLayout). Higher stretch values claim more space.
- `update` is the core method: given a parent `Rect` and a callback, it computes and emits the `Rect` for each child widget.
- `update_with_context` applies device-aware scaling. Layouts that override it (like `BoxLayout`) scale spacing/margin by `context.layout_scale`.
- `as_any` / `as_any_mut` enable downcasting to concrete layout types for introspection and the layout inspector.

### `SizePolicy`

Each widget item in a box layout can declare its sizing preference:

```rust
pub enum SizePolicy {
    Fixed,      // Uses exact constraints.max (or constraints.min) — no stretch allocation
    Preferred,  // Uses natural/desired size, participates in stretch negotiation
    Expanding,  // Grows to consume remaining space (default for BoxLayout items)
}
```

### `LayoutConstraints`

Per-child min/max limits enforced during layout calculation:

```rust
pub struct LayoutConstraints {
    pub min: u32,
    pub max: Option<u32>,
}
```

Use `set_constraints(widget_id, LayoutConstraints::new(80, Some(200)))` to cap a widget between 80 px minimum and 200 px maximum along the major axis.

### `LayoutContext`

Device adaptation parameters passed through `Layout::update_with_context`:

```rust
pub struct LayoutContext {
    pub layout_scale: f32,    // Scale factor for spacing, margins, padding
    pub font_scale: f32,      // Scale factor for font/metric sizes
    pub min_touch_size: Size,  // Minimum touch-target size (default: 32×32)
}
```

By default, all scales are `1.0` and `min_touch_size` is `Size::new(32, 32)`. On high-DPI displays, `layout_scale` may be `2.0` or higher.

**Example — DPI-aware box layout:**

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

## Layout Managers

### BoxLayout — Linear Row/Column

Arranges children in a single row (`Horizontal`) or column (`Vertical`) with stretch-weighted proportional allocation and optional spacer items.

```rust
let mut layout = BoxLayout::new(Orientation::Horizontal, 4, 8);
layout.add_widget(button_id, 1);   // stretch=1
layout.add_spacer(1);              // stretch=1 spacer (no widget)
layout.add_widget(label_id, 2);    // stretch=2 → gets 2× the space

layout.set_constraints(label_id, LayoutConstraints::new(100, Some(300)));
layout.set_size_policy(button_id, SizePolicy::Fixed);
```

Space allocation algorithm:
1. Fixed-policy items get exactly their `constraints.max` (or `min` if no max).
2. Expanding/Preferred items split the remaining space proportionally by `stretch`.
3. If total assigned ≠ available, the algorithm iteratively grows/shrinks assignments respecting per-item constraints.

**Named aliases — `HBoxLayout` and `VBoxLayout`:**

```rust
let mut hbox = HBoxLayout::new(4, 8);  // spacing=4, margin=8, horizontal
hbox.add_widget(widget_a, 1);
hbox.add_spacer(2);
hbox.add_widget(widget_b, 1);

let mut vbox = VBoxLayout::new(2, 4);  // spacing=2, margin=4, vertical
vbox.add_widget(header, 1);
vbox.add_widget(body, 3);             // body gets 3× the height
```

### FlexLayout — CSS Flexbox

A full flexbox implementation inspired by CSS, supporting direction, wrapping, justification, alignment, gap, padding, and per-item `align_self` overrides.

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

**Enums at a glance:**

| Enum | Variants |
|---|---|
| `FlexDirection` | `Row`, `RowReverse`, `Column`, `ColumnReverse` |
| `FlexWrap` | `NoWrap`, `Wrap`, `WrapReverse` |
| `JustifyContent` | `FlexStart`, `FlexEnd`, `Center`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly` |
| `AlignItems` | `Stretch`, `FlexStart`, `FlexEnd`, `Center`, `Baseline` |

**Per-item overrides via `FlexItem`:**

```rust
flex.items_mut().get_mut(0).unwrap().align_self = Some(AlignItems::FlexEnd);
flex.items_mut().get_mut(0).unwrap().min_size = Some(50);
flex.items_mut().get_mut(0).unwrap().max_size = Some(200);
```

**Flexbox row-reverse example:**

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

### GridLayout — Fixed-Cell Grid

A grid with fixed rows and columns where widgets are placed at explicit `(row, col)` positions.

```rust
let mut grid = GridLayout::new(3, 4, 2, 4); // rows=3, cols=4, spacing=2, margin=4
grid.set_widget(0, 0, header_id);   // row 0, col 0
grid.set_widget(0, 1, title_id);     // row 0, col 1
grid.set_widget(1, 0, content_id);   // row 1, col 0

// Cells are evenly divided: each cell = (available / cols) wide, (available / rows) tall
// add_widget fills the first empty cell (row-major order)
grid.add_widget(footer_id, 0);       // fills row 1, col 1
```

**Stretch factors for proportional row/column sizing:**

```rust
grid.set_column_stretch(2);  // columns get 2× stretch factor
grid.set_row_stretch(1);     // rows get 1× stretch factor
```

### UniformGridLayout — Equal-Size Grid Cells

Similar to `GridLayout` but guarantees all cells have identical dimensions.

```rust
let mut grid = UniformGridLayout::new(3, 4, 2, 0);
for r in 0..3 {
    for c in 0..4 {
        grid.set_widget(r, c, widget_ids[r * 4 + c]);
    }
}
// Every cell reports the same (width, height) regardless of content
```

### StackLayout — Card Stack

Shows one child at a time from an ordered list, like a tab panel or wizard.

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

### SplitterLayout — Resizable Split Panes

Distributes space proportionally by pane ratios, supporting horizontal or vertical orientation.

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

### AbsoluteLayout — 9-Anchor Positioning

Positions children using an `Anchor` reference point. Each child has an anchor position with an optional `Constraint` for min/max and aspect ratio.

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

**Nine anchor points:**

<div style="display:none"></div>
| Anchor | Aligned Edge |
|---|---|
| `TopLeft` | Top-left corner of `(x, y)` |
| `TopCenter` | Top edge, horizontally centered at `x` |
| `TopRight` | Top-right corner at `(x, y)` |
| `CenterLeft` | Left edge, vertically centered at `y` |
| `Center` | Fully centered on `(x, y)` |
| `CenterRight` | Right edge, vertically centered at `y` |
| `BottomLeft` | Bottom-left corner at `(x, y)` |
| `BottomCenter` | Bottom edge, horizontally centered at `x` |
| `BottomRight` | Bottom-right corner at `(x, y)` |

### CenterLayout — Single-Child Centering

Centers a single child with configurable width/height factors (`0.0..=1.0`).

```rust
let mut center = CenterLayout::with_factors(0.8, 0.6);
center.add_widget(logo_id, 0);
// Child gets 80% of parent width, 60% of parent height, dead-center

// Fill to 100%
center.set_width_factor(1.0);
center.set_height_factor(1.0);
```

### ConstraintLayout — Anchor-Based Constraints

Positions children relative to each other using constraint rules (not a full Cassowary solver). Constraints define edge-to-edge, center, or dimension relationships.

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

**All constraint types:**

| Constraint | Meaning |
|---|---|
| `LeftToLeft` | Widget left = target left + offset |
| `LeftToRight` | Widget left = target right + offset |
| `RightToLeft` | Widget right = target left + offset |
| `RightToRight` | Widget right = target right + offset |
| `TopToTop` | Widget top = target top + offset |
| `TopToBottom` | Widget top = target bottom + offset |
| `BottomToTop` | Widget bottom = target top + offset |
| `BottomToBottom` | Widget bottom = target bottom + offset |
| `CenterX` | Widget center-x = target center-x + offset |
| `CenterY` | Widget center-y = target center-y + offset |
| `Width` | Widget width = target width × multiplier |
| `Height` | Widget height = target height × multiplier |
| `AspectRatio(f32)` | Width = height × ratio (maintained to fit) |

### FlowLayout — Horizontal/Vertical Flow With Wrap

Arranges children sequentially in a row or column, with optional wrapping, alignment, and padding.

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

**FlowAlignment variants:** `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`

> **Note:** `FlowLayout` stores `Box<dyn Widget>` children rather than `ObjectId`. Use `add_child()`/`remove_child()` instead of `add_widget()`.

### WrapLayout — Overflow Wrapping

Automatically wraps children to the next row (horizontal) or column (vertical) when they overflow the container width.

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

`WrapAlignment` supports: `Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`.

### FormLayout — Label/Field Two-Column

Arranges label–field pairs in a two-column form with a 1:2 width ratio.

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

### AspectRatioLayout — Enforced Width/Height Ratio

Constrains a single child to a specific aspect ratio, optionally bounded by parent dimensions.

```rust
let layout = AspectRatioLayout::new(16.0 / 9.0, true);  // 16:9, respect parent bounds
// Child is centered and sized to fit within parent

// Without parent bounds (may overflow)
let free_layout = AspectRatioLayout::new(2.0, false);
// Uses parent width as base, derives height
```

### KeyboardAwareLayout — Mobile Keyboard Offset

Wraps any inner layout and shifts children upward by the keyboard height, preventing the focused input from being obscured.

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

## LayoutInspector — Diagnostics

The `LayoutInspector` detects common layout issues at runtime without modifying widget positions:

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

**Detected issues:**

| Category | Issue | Severity |
|---|---|---|
| **Structural** | Orphan widgets (no parent) | Warning |
| **Structural** | Empty layouts with child widgets declared | Info |
| **Geometric** | Zero-width or zero-height rects | Error |
| **Geometric** | Overlapping sibling rects (excluding touch targets) | Warning |

**Recommendations** are generated automatically for each detected issue with a title, summary, and detailed fix suggestion.

**Registering native platform layouts** for introspection:

```rust
LayoutInspector::register_native_layout(parent_id, "NavBar", 5, "NativeNavBar");
```

---

## Declarative Layouts via JSON

Layouts can be declared in JSON and instantiated at runtime using `DeclarativeLayoutKind`:

```rust
use rust_widgets::json::layout::{parse_layout_kind, create_layout_from_kind, DeclarativeLayoutKind};

let json = r#"{"type": "vbox", "spacing": 4, "margin": 8}"#;
let value: serde_json::Value = serde_json::from_str(json).unwrap();
let kind = parse_layout_kind(&value).unwrap();
let layout = create_layout_from_kind(&kind);
```

**Supported JSON layout types:**

| JSON `"type"` | Aliases | Parameters |
|---|---|---|
| `"hbox"` | `"HBox"`, `"horizontal"` | `spacing`, `margin` |
| `"vbox"` | `"VBox"`, `"vertical"` | `spacing`, `margin` |
| `"grid"` | `"Grid"` | `columns`, `spacing`, `margin` |
| `"stack"` | `"Stack"` | `spacing` |
| `"splitter"` | `"Splitter"` | `orientation` (`"horizontal"`/`"vertical"`), `margin` |
| `"form"` | `"Form"` | `spacing`, `margin` |

**Child layout attributes** can specify stretch, grid position, and spans:

```json
{
    "stretch": 3,
    "col": 1,
    "row": 2,
    "col_span": 2,
    "row_span": 1
}
```

Parse them with `ChildLayoutAttrs::from_value(&json_value)`.

---

## Custom Layout Creation

Implement the `Layout` trait to create a custom layout manager:

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

## Common Patterns

### Centered Modal Dialog

```rust
let mut root = VBoxLayout::new(0, 0);
let mut center = CenterLayout::with_factors(0.5, 0.4);
center.add_widget(dialog_id, 0);
// Nest CenterLayout inside a VBox — but Layout isn't a Widget.
// Use a container widget that holds a layout internally.
```

### Toolbar + Content + Status Bar

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

### Responsive Flex Wrap

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

### DPI-Aware Adaptive Spacing

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

### Multi-Page Wizard with Keyboard Awareness

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
