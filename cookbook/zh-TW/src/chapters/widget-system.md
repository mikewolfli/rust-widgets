# 控制項系統

本章提供整個控制項系統的完整參考：`Widget` 特徵、`BaseWidget`、渲染、控制項層級結構以及如何建立自訂控制項。

---

## 架構概覽

rust-widgets 中的每個控制項都遵循一致的模式：

```
┌──────────────────────────────────────────────────┐
│                   Widget 特徵                      │
│  (60+ 預設方法委派給 BaseWidget)                   │
├──────────────────────────────────────────────────┤
│                   BaseWidget                       │
│  共享狀態：geometry、visibility、signals、           │
│  styling、hierarchy、DPI、tooltip、accessibility    │
├──────────────┬──────────────┬────────────────────┤
│   Draw 特徵   │ EventHandler  │  自訂訊號           │
│  (rendering)  │  (input)     │  (widget-specific)  │
└──────────────┴──────────────┴────────────────────┘
```

具體控制項至少實作三件事：
1. **`Widget`**——`base()` 和 `base_mut()` 的取得器
2. **`EventHandler`**——如何回應事件
3. **`Draw`**——如何繪製控制項

---

## `Widget` 特徵（60+ 預設方法）

```rust
pub trait Widget: EventHandler + Any {
    fn base(&self) -> &BaseWidget;
    fn base_mut(&mut self) -> &mut BaseWidget;
    fn id(&self) -> ObjectId;
    fn kind(&self) -> WidgetKind;
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    fn show(&mut self);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn style(&self) -> &WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn children(&self) -> &[ObjectId];
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);
    fn set_tooltip(&mut self, tooltip: String);
    fn tooltip(&self) -> &str;
    fn set_translated_tooltip(&mut self, key: &str);
    fn accessible_name(&self) -> String;
    fn accessible_role(&self) -> AccessibleRole;
    fn accessible_description(&self) -> String;
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, scale: f32);
}
```

所有預設實作都委派給 `BaseWidget`。具體控制項只需實作 `base()` 和 `base_mut()`——其他一切都會繼承。

### 最小控制項實作

```rust
use rust_widgets::widget::{Widget, BaseWidget, WidgetKind, Draw};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::core::{Color, Font, Point, Rect};

struct MinimalWidget {
    base: BaseWidget,
}

impl MinimalWidget {
    fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "MinimalWidget"),
        }
    }
}

impl Widget for MinimalWidget {
    fn base(&self) -> &BaseWidget { &self.base }
    fn base_mut(&mut self) -> &mut BaseWidget { &mut self.base }
}

impl EventHandler for MinimalWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for MinimalWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(200, 200, 200));
        context.draw_text(
            Point::new(rect.x + 10, rect.y + 10),
            "Minimal Widget",
            &Font::simple("Arial", 12.0),
            Color::BLACK,
        );
    }
}
```

---

## `BaseWidget`——共享狀態和訊號

每個具體控制項都內嵌一個 `BaseWidget`：

```rust
pub struct BaseWidget {
    pub(crate) object: Object,
    pub(crate) kind: WidgetKind,
    pub(crate) geometry: Rect,
    pub(crate) min_size: Option<Size>,
    pub(crate) max_size: Option<Size>,
    pub(crate) parent: Option<ObjectId>,
    pub(crate) children: MiniVec<ObjectId>,
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    pub(crate) mouse_pressed: bool,
    pub(crate) dpi_scale: f32,
    pub(crate) style: WidgetStyle,
    pub(crate) tooltip: MiniString,
    pub(crate) connection_scope: ConnectionScope,
    pub clicked: GenericSignal,
    pub hover: Signal1<Point>,
    pub mouse_down: Signal1<(Point, u32)>,
    pub mouse_up: Signal1<(Point, u32)>,
    pub key_down: Signal1<(u32, u32)>,
    pub key_up: Signal1<(u32, u32)>,
    pub focus_gained: GenericSignal,
    pub focus_lost: GenericSignal,
    pub redraw_requested: GenericSignal,
    pub layout_requested: GenericSignal,
    pub changed: GenericSignal,
}
```

### 11 個基礎訊號

| 訊號 | 型別 | 發出時機 |
|---|---|---|
| `clicked` | `GenericSignal` | 使用者點擊/與控制項互動 |
| `hover` | `Signal1<Point>` | 滑鼠游標移到控制項上方 |
| `mouse_down` | `Signal1<(Point, u32)>` | 滑鼠按鈕在控制項上按下 |
| `mouse_up` | `Signal1<(Point, u32)>` | 滑鼠按鈕在控制項上釋放 |
| `key_down` | `Signal1<(u32, u32)>` | 控制項聚焦時按下按鍵 |
| `key_up` | `Signal1<(u32, u32)>` | 控制項聚焦時釋放按鍵 |
| `focus_gained` | `GenericSignal` | 控制項接收輸入焦點 |
| `focus_lost` | `GenericSignal` | 控制項失去輸入焦點 |
| `redraw_requested` | `GenericSignal` | 控制項需要重新繪製 |
| `layout_requested` | `GenericSignal` | 控制項需要重新計算版面配置 |
| `changed` | `GenericSignal` | 控制項的值/狀態變更 |

---

## `Draw` 特徵

```rust
pub trait Draw {
    fn draw(&mut self, context: &mut RenderContext);
    fn uses_custom_drawing(&self) -> bool { true }
    fn request_custom_redraw(&self) {}
}
```

### `RenderContext`——繪圖原始語

```rust
impl RenderContext {
    pub fn fill_rect(&mut self, rect: Rect, color: Color);
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color);
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color);
    pub fn draw_rect_stroke(&mut self, rect: Rect, color: Color, width: u32);
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color);
    pub fn draw_text(&mut self, pos: Point, text: &str, font: &Font, color: Color);
    pub fn draw_image(&mut self, rect: Rect, image: &Image);
}
```

---

## `EventHandler`——預設實作

`BaseWidget` 提供一個預設的 `EventHandler`，將平台事件對應到訊號發射。

---

## `WidgetKind` 列舉——109+ 變體

`WidgetKind` 列舉將每個控制項型別分類。它透過功能旗標進行門控：15 個變體始終可用；94+ 個需要非 `mini` 功能。

---

## 建立自訂控制項（完整範例）

```rust
use rust_widgets::widget::{Widget, BaseWidget, WidgetKind, Draw};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{GenericSignal, ConnectionScope};
use rust_widgets::core::{Color, Font, Point, Rect, Size, ObjectId};

pub struct CounterWidget {
    base: BaseWidget,
    count: u32,
    pub count_changed: GenericSignal,
}

impl CounterWidget {
    pub fn new(geometry: Rect) -> Self {
        let mut base = BaseWidget::new(WidgetKind::Panel, geometry, "CounterWidget");
        base.set_min_size(Some(Size::new(60, 30)));
        Self {
            base,
            count: 0,
            count_changed: GenericSignal::new(),
        }
    }
    // ... 更多方法
}
```

---

## 控制項生命週期摘要

```
┌──────────────────────────────────────────────────────────┐
│                     控制項生命週期                          │
├──────────────┬───────────────────────────────────────────┤
│  1. 建立     │ new(geometry) → BaseWidget(WidgetKind)     │
│  2. 設定     │ set_style, set_text, set_tooltip,          │
│              │   set_min_size, connect signals            │
│  3. 父層     │ set_parent(parent_id)                      │
│  4. 版面配置 │ 版面配置引擎設定 geometry                    │
│  5. 顯示     │ show() → visible = true                    │
│  6. 繪製     │ Draw::draw(context) → 渲染管線              │
│  7. 事件     │ EventHandler::handle_event → 訊號發射       │
│  8. 更新     │ set_geometry, set_style → redraw_requested │
│  9. 隱藏     │ hide() → visible = false                   │
│ 10. 銷毀     │ Drop impl → cleanup, disconnect signals    │
└──────────────┴───────────────────────────────────────────┘
```

---

## 訊號接線模式

### 模式 1：控制項對控制項通訊

```rust
button.base.clicked.connect({
    let label_id = label.id();
    move || {
        // 在真實應用程式中，使用基於控制代碼的文字更新
    }
});
```

### 模式 2：值對顯示繫結

```rust
slider.base.changed.connect({
    move || {
        let value = slider.value();
        label.set_text(&format!("數值：{}", value));
    }
});
```

### 模式 3：範圍限定連接用於臨時 UI

```rust
{
    let scope = ConnectionScope::new();
    ok_button.base.clicked.connect_scoped(&scope, || {
        dialog.accept();
    });
    cancel_button.base.clicked.connect_scoped(&scope, || {
        dialog.reject();
    });
} // scope 釋放 → 所有連接自動中斷
```

---

## 最佳實踐

1. **始終委派給 `base.handle_event()`**
2. **使用 `ConnectionScope` 清理連接**
3. **在昂貴工作前檢查可見性/啟用狀態**
4. **節制使用 `request_redraw()`**
5. **驗證輸入尺寸**

---

## 下一步

- **版面配置系統**——了解控制項如何使用 Box、Grid、Stack、Flow、Flex 和 Absolute 版面配置演算法定位
- **事件系統**——深入探討事件型別、傳播、手勢辨識和計時器管理
- **樣式與主題**——了解 `WidgetStyle`、基於 CSS 的主題設定和樣式表熱載入
- **渲染系統**——探索 GPU/CPU/SVG 後端、髒區域和部分重新整理最佳化
