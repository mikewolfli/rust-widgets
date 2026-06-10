# 核心型別

本章提供 `src/core/` 模組中每個型別的完整參考 — 這些是構成整個 rust-widgets 函式庫的基礎建構區塊。

---

## 模組概觀

```rust
// src/core/mod.rs — 公開重新匯出
pub use alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
pub use color::Color;
pub use font::Font;
pub use geometry::{Orientation, Point, Rect, Size};
pub use mutex_ext::MutexExt;
pub use types::{
    CoreConfig, CoreError, CoreObject, CoreResult, DeviceClass,
    ObjectId, PlatformCapabilities, PlatformFamily, RuntimeProfile, Version,
};
pub mod coords;
pub mod rect_merge;
```

| 檔案 | 內容 |
|---|---|
| `types.rs` | `ObjectId`、`RuntimeProfile`、`DeviceClass`、`PlatformFamily`、`Version`、`PlatformCapabilities`、`CoreConfig`、`CoreError`、`CoreObject`、`CoreResult` |
| `geometry.rs` | `Point`、`Size`、`Rect`、`Orientation` |
| `color.rs` | `Color` — RGBA，含 55+ 個預定義常數 |
| `alignment.rs` | `Alignment`、`HorizontalAlignment`、`VerticalAlignment` |
| `font.rs` | `Font` — 家族、大小、粗細、粗體、斜體 |
| `coords.rs` | 螢幕、笛卡兒和 PDF 系統之間的座標轉換 |
| `rect_merge.rs` | `merge_intersecting_rects()`、`bounding_rect()` |
| `mutex_ext.rs` | `MutexExt` — 中毒恢復擴充 trait |

---

## ObjectId

一個穩定的 `u64` 包裝，用於唯一識別每個 widget 和核心物件。

```rust
pub type ObjectId = u64;
```

`ObjectId` 是一個型別別名，而非包裝結構體，以獲得最大的 C 互操作性。每個 widget 都以唯一的 `ObjectId` 建立，整個 widget 階層使用 `ObjectId` 參考而非 Rust 參考，避免了生命週期的複雜性。

```rust
use rust_widgets::core::ObjectId;

let id: ObjectId = 42;

// ObjectId 隨處可用：
fn show_widget(id: ObjectId);
fn set_widget_text(id: ObjectId, text: &str);
fn get_widget_text(id: ObjectId) -> String;
```

---

## Color

一個 4 通道 RGBA 顏色結構，附帶大量工具方法。

### 結構體定義

```rust
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
```

### 建構函式

```rust
use rust_widgets::core::Color;

// RGBA 建構函式（alpha 0–255）：
let red = Color::rgba(255, 0, 0, 255);
let semi_transparent = Color::rgba(0, 0, 255, 128);

// RGB 建構函式（alpha = 255）：
let green = Color::rgb(0, 255, 0);

// 命名建構函式：
let blue = Color::from_rgb(0, 0, 255);
let teal = Color::from_rgba(0, 128, 128, 255);

// 從 f32（0.0–1.0）：
let coral = Color::from_f32(1.0, 0.5, 0.31, 1.0);

// 從 i32（0–255）：
let pink = Color::from_i32(255, 192, 203, 255);

// 從 u32 打包：
let purple = Color::from_u32_rgba(0x800080FF);
let orange = Color::from_u32_rgb(0xFFA500);

// 從元組：
let navy = Color::from_u8_tuple((0, 0, 128));
let gray = Color::from_u8_rgb_tuple((128, 128, 128));
let mint = Color::from_f32_tuple((0.6, 1.0, 0.6));
```

### 55+ 個預定義顏色常數

**標準顏色：**
```rust
Color::BLACK           // (0, 0, 0)
Color::WHITE           // (255, 255, 255)
Color::RED             // (255, 0, 0)
Color::GREEN           // (0, 255, 0)
Color::BLUE            // (0, 0, 255)
Color::YELLOW          // (255, 255, 0)
Color::CYAN            // (0, 255, 255)
Color::MAGENTA         // (255, 0, 255)
Color::TRANSPARENT     // (0, 0, 0, 0)
```

**灰階：**
```rust
Color::GRAY                // (128, 128, 128)
Color::LIGHT_GRAY          // (211, 211, 211)
Color::DARK_GRAY           // (64, 64, 64)
Color::EXTRA_LIGHT_GRAY    // (238, 238, 238)
Color::MEDIUM_GRAY         // (160, 160, 160)
Color::EXTRA_DARK_GRAY     // (32, 32, 32)
```

**亮/暗變體：**
```rust
Color::LIGHT_RED      Color::DARK_RED
Color::LIGHT_GREEN    Color::DARK_GREEN
Color::LIGHT_BLUE     Color::DARK_BLUE
Color::LIGHT_YELLOW   Color::DARK_YELLOW
```

**語意顏色：**
```rust
Color::PRIMARY        // 主要品牌顏色
Color::SECONDARY      // 次要強調色
Color::SUCCESS        // 成功 / 正面回饋
Color::WARNING        // 警告 / 注意
Color::ERROR          // 錯誤 / 危險
Color::BACKGROUND     // 預設背景
Color::FOREGROUND     // 預設前景（文字）
Color::LINK           // 超連結顏色
Color::LINK_HOVER     // 超連結懸停顏色
Color::BORDER         // 預設邊框
Color::DIVIDER        // 分隔線
Color::SELECTION      // 文字選取高亮
Color::TOOLTIP        // 工具提示背景
Color::MENU_BACKGROUND
Color::MENU_FOREGROUND
Color::INFO
Color::NOTIFICATION
Color::DISABLED_BACKGROUND
Color::DISABLED_FOREGROUND
```

**命名網頁顏色：**
```rust
Color::ALICE_BLUE     Color::BEIGE        Color::CORAL
Color::GOLD           Color::IVORY        Color::LAVENDER
Color::ROSE           Color::SILVER       Color::TAN
Color::AQUA           Color::BROWN        Color::FOREST_GREEN
Color::INDIGO         Color::MAROON       Color::NAVY
Color::OLIVE          Color::ORANGE       Color::PINK
Color::PURPLE         Color::TEAL
Color::SKY_BLUE       Color::STEEL_BLUE
Color::SLATE_GRAY     Color::DARK_SLATE_GRAY
Color::LIGHT_SLATE_GRAY
Color::LIGHT_CYAN     Color::LIGHT_GOLDENROD_YELLOW
Color::LIGHT_PINK     Color::LIGHT_SALMON
```

### 十六進位解析

```rust
// 解析十六進位字串（#RGB、#RGBA、#RRGGBB、#RRGGBBAA）：
let red = Color::parse_hex("#FF0000").unwrap();
let semi_blue = Color::parse_hex("#0000FF80").unwrap();
let short_green = Color::parse_hex("#0F0").unwrap();

// 序列化回十六進位：
let hex_rgb = red.to_hex_rgb();        // "#ff0000"
let hex_rgba = semi_blue.to_hex_rgba(); // "#0000ff80"
```

### 打包整數轉換

```rust
let color = Color::rgb(0xAA, 0xBB, 0xCC);

// 打包為 u32：
let packed = color.to_rgba_u32();  // 0xAABBCCFF

// 從 u32 解包：
let restored = Color::from_rgba_u32(packed);
assert_eq!(color, restored);
```

### 顏色操作

```rust
let base = Color::rgb(200, 100, 50);

// 變更 alpha：
let faded = base.with_alpha(128);               // u8
let faded2 = base.with_alpha_f32(0.5);           // f32

// Alpha 混合（over 運算子）：
let bg = Color::WHITE;
let fg = Color::rgba(255, 0, 0, 128);
let blended = bg.blend(&fg);
// 結果：(255, 128, 128) — 紅色混合在白色上

// 亮度（感知亮度）：
let lum = blended.luminance();  // 0.0–1.0
assert!(blended.is_light());    // luminance > 0.5

// 對比色（黑色或白色，取對比較高者）：
let contrast = blended.contrast_color();

// 反轉：
let inverted = blended.invert();
```

### Trait 實作

```rust
// Default = BLACK：
let default_color = Color::default();  // Color::BLACK

// 從十六進位字串：
let from_str: Color = "#FF8800".into();

// Display（十六進位 RGBA）：
println!("{}", Color::rgb(255, 128, 0));  // "#ff8000ff"
```

---

## Point

一個 2D 整數座標。

```rust
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

### 建構函式

```rust
use rust_widgets::core::Point;

// 直接：
let p1 = Point::new(10, 20);

// 原點（0, 0）：
let origin = Point::origin();

// 從各種數值型別：
let p2 = Point::from_f32(10.5, 20.7);       // (11, 21)
let p3 = Point::from_f32_trunc(10.9, 20.1); // (10, 20)
let p4 = Point::from_u32(100, 200);          // (100, 200)
let p5 = Point::from_i64(1000, -500);        // (1000, -500)
let p6 = Point::from_f64(3.14, 2.72);        // (3, 3)
let p7 = Point::from_usize(640, 480);        // (640, 480)
let p8 = Point::from_isize(-10, 10);         // (-10, 10)

// 從元組：
let p9 = Point::from_i32_tuple((10, 20));
let p10 = Point::from_f32_tuple((10.5, 20.5));
let p11 = Point::from_u32_tuple((100, 200));
let p12 = Point::from_f64_tuple((3.14, 2.72));
let p13 = Point::from_usize_tuple((640, 480));
let p14 = Point::from_isize_tuple((-10, 10));

// 從 trait 轉換：
let p15: Point = (10, 20).into();
```

### 轉換

```rust
let p = Point::new(10, 20);

let (x, y) = p.to_f32();   // (10.0, 20.0)
let (x, y) = p.to_f64();   // (10.0, 20.0)
let (x, y) = p.to_u32();   // (10, 20)
```

### 算術運算

```rust
let p = Point::new(10, 20);

// 加上元組偏移：
let offset = p + (5, 10);  // Point { x: 15, y: 30 }
```

### Display

```rust
println!("{}", Point::new(10, 20));  // "(10, 20)"
```

---

## Size

一個 2D 尺寸（寬 × 高），以像素為單位。

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

### 建構函式

```rust
use rust_widgets::core::Size;

// 直接：
let s1 = Size::new(640, 480);

// 從各種數值型別：
let s2 = Size::from_f32(640.5, 480.5);         // (641, 481)
let s3 = Size::from_f32_trunc(640.9, 480.9);   // (640, 480)
let s4 = Size::from_i32(640, 480);              // (640, 480)
let s5 = Size::from_i64(1920, 1080);            // (1920, 1080)
let s6 = Size::from_f64(640.0, 480.0);          // (640, 480)
let s7 = Size::from_usize(800, 600);            // (800, 600)
let s8 = Size::from_isize(1024, 768);           // (1024, 768)

// 從元組：
let s9 = Size::from_u32_tuple((640, 480));
let s10 = Size::from_f32_tuple((640.0, 480.0));
let s11 = Size::from_i32_tuple((640, 480));
let s12 = Size::from_f64_tuple((640.0, 480.0));
let s13 = Size::from_usize_tuple((800, 600));
let s14 = Size::from_isize_tuple((1024, 768));
```

### 工具方法

```rust
let s = Size::new(640, 480);

// 檢查是否為零尺寸：
assert!(!s.is_empty());

// 計算面積：
let area = s.area();  // 640 * 480 = 307200

// 寬高比（width / height）：
let ratio = s.aspect_ratio();  // 1.333...
```

### 轉換

```rust
let s = Size::new(640, 480);

let (w, h) = s.to_f32();  // (640.0, 480.0)
let (w, h) = s.to_f64();  // (640.0, 480.0)
let (w, h) = s.to_i32();  // (640, 480)
```

### 算術運算

```rust
let s = Size::new(100, 100);
let bigger = s + (50, 50);  // Size { width: 150, height: 150 }
```

### Display

```rust
println!("{}", Size::new(640, 480));  // "640x480"
```

---

## Rect

一個定位矩形 — 最常用的幾何型別。

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

### 建構函式

```rust
use rust_widgets::core::Rect;

// 直接（x, y, width, height）：
let r1 = Rect::new(10, 20, 640, 480);

// 從混合型別：
let r2 = Rect::from_mixed(10, 20, 640u32, 480u32);

// 從位置 + 尺寸：
let r3 = Rect::from_position_size(Point::new(10, 20), Size::new(640, 480));

// 從元組：
let r4 = Rect::from_tuple((10, 20, 640, 480));
let r5 = Rect::from_u32_tuple((10, 20, 640, 480));
let r6 = Rect::from_f32_tuple((10.0, 20.0, 640.0, 480.0));

// 從各種數值型別：
let r7 = Rect::from_f32(10.0, 20.0, 640.0, 480.0);
let r8 = Rect::from_f64(10.0, 20.0, 640.0, 480.0);
let r9 = Rect::from_i64(10, 20, 640, 480);
let r10 = Rect::from_usize(10, 20, 640, 480);
let r11 = Rect::from_isize(10, 20, 640, 480);

// 從兩個角點：
let r12 = Rect::from_points(Point::new(10, 20), Point::new(650, 500));

// 從中心點和尺寸：
let r13 = Rect::from_center(Point::new(330, 260), Size::new(640, 480));
```

### 分解

```rust
let rect = Rect::new(10, 20, 640, 480);

let position = rect.position();       // Point { x: 10, y: 20 }
let size = rect.size();               // Size { width: 640, height: 480 }
let (x, y, w, h) = rect.decompose();  // (10, 20, 640, 480)

// 邊緣座標：
let right = rect.right();    // x + width = 650
let bottom = rect.bottom();  // y + height = 500
let center = rect.center();  // Point { x: 330, y: 260 }
```

### 驗證與包含

```rust
let rect = Rect::new(10, 20, 640, 480);

// 矩形是否有效？（width > 0 && height > 0）
assert!(rect.is_valid());

// 點包含（max edge 為獨占）：
let inside = Point::new(100, 100);
let outside = Point::new(1000, 1000);
assert!(rect.contains_point(inside));
assert!(!rect.contains_point(outside));

// 將點限制在矩形內：
let clamped = rect.clamp_point(Point::new(1000, 1000));
// → Point { x: 649, y: 499 }

// 矩形相交測試：
let other = Rect::new(300, 200, 500, 400);
assert!(rect.intersects(&other));

// 完整包含：
let small = Rect::new(100, 100, 50, 50);
assert!(rect.contains_rect(&small));
assert!(rect.contains(small));  // 別名

// 面積：
let area = rect.area();  // 640 * 480 = 307200
```

### 布林運算

```rust
let a = Rect::new(0, 0, 200, 200);
let b = Rect::new(100, 100, 200, 200);

// 聯集（能包含兩者的最小矩形）：
let union = a.union(&b);         // Rect { x: 0, y: 0, width: 300, height: 300 }

// 交集（重疊區域）：
let intersection = a.intersection(&b);
// → Some(Rect { x: 100, y: 100, width: 100, height: 100 })

let disjoint = Rect::new(500, 500, 100, 100);
assert!(a.intersection(&disjoint).is_none());
```

### 變換

```rust
let rect = Rect::new(100, 100, 200, 200);

// 加入內距（向內擴張）：
let padded = rect.with_padding(10);  // 所有邊：10px
let padded_vh = rect.with_padding((5, 10));  // 垂直：5，水平：10

// 加入外距（向外擴張）：
let margined = rect.with_margin(10);  // 所有邊：10px
let margined_ltrb = rect.with_margin((1, 2, 3, 4));  // 左、上、右、下

// 縮小指定量：
let shrunk = rect.shrink(10);

// 放大指定量：
let grown = rect.grow(10);

// 擴展以包含一個點：
let extended = rect.extend_to_include(Point::new(500, 500));

// 觸控目標擴展（不小於 44x44）：
let touch = rect.expand_to_touch_target();
// 如果 width < 44 或 height < 44，則在兩側均等擴展
```

### 轉換

```rust
let rect = Rect::new(10, 20, 640, 480);

let (x, y, w, h) = rect.to_f32();  // (10.0, 20.0, 640.0, 480.0)
let (x, y, w, h) = rect.to_f64();  // (10.0, 20.0, 640.0, 480.0)
let (x, y, w, h) = rect.to_u32();  // (10, 20, 640, 480)
```

### Default

```rust
let default = Rect::default();
// Rect { x: 0, y: 0, width: 0, height: 0 }
```

### Display

```rust
println!("{}", Rect::new(10, 20, 640, 480));
// "Rect { x: 10, y: 20, width: 640, height: 480 }"
```

---

## Orientation

一個簡單的兩變體列舉，用於水平與垂直佈局。

```rust
pub enum Orientation {
    Horizontal,
    Vertical,
}
```

```rust
use rust_widgets::core::Orientation;

let direction = Orientation::Horizontal;

match direction {
    Orientation::Horizontal => { /* 從左到右佈局 */ }
    Orientation::Vertical   => { /* 從上到下佈局 */ }
}
```

---

## Alignment

三個對齊列舉，用於在佈局中定位元素。

### `Alignment` — 完整 5 向

```rust
pub enum Alignment {
    Left,
    Center,
    Right,
    Top,
    Bottom,
}
```

```rust
use rust_widgets::core::Alignment;

let align = Alignment::Center;

// 軸向檢查：
assert!(align.is_horizontal());  // false（Center 適用於兩個軸）
assert!(Alignment::Left.is_horizontal());
assert!(Alignment::Top.is_vertical());

// 從字串解析：
let from_str = Alignment::parse_str("center");  // Some(Alignment::Center)
let short = Alignment::parse_str("l");          // Some(Alignment::Left)

// 轉換為字串：
let s = align.as_str();  // "center"

// CSS 值：
let text_align = Alignment::Left.css_text_align();       // Some("left")
let vert_align = Alignment::Top.css_vertical_align();    // Some("top")

// 相反：
assert_eq!(Alignment::Left.opposite(), Alignment::Right);
assert_eq!(Alignment::Top.opposite(), Alignment::Bottom);
assert_eq!(Alignment::Center.opposite(), Alignment::Center);
```

### `HorizontalAlignment` — 3 向

```rust
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
}
```

```rust
use rust_widgets::core::HorizontalAlignment;

let h = HorizontalAlignment::Center;

// 從字串：
let h = HorizontalAlignment::parse_str("center").unwrap();
assert!(h.is_center());

// 轉為字串：
assert_eq!(h.as_str(), "center");

// 與 Alignment 之間的轉換：
let from_gen = HorizontalAlignment::from_alignment(Alignment::Right);
// → Some(HorizontalAlignment::Right)
let from_gen = HorizontalAlignment::from_alignment(Alignment::Top);
// → None（Top 不是水平方向）

let to_gen: Alignment = HorizontalAlignment::Left.into();

let back: HorizontalAlignment = Alignment::Left.try_into().unwrap();
```

### `VerticalAlignment` — 3 向

```rust
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}
```

```rust
use rust_widgets::core::VerticalAlignment;

let v = VerticalAlignment::Bottom;

// 從字串：
let v = VerticalAlignment::parse_str("bottom").unwrap();
assert!(v.is_bottom());

// 轉為字串：
assert_eq!(v.as_str(), "bottom");

// 與 Alignment 之間的轉換：
let from_gen = VerticalAlignment::from_alignment(Alignment::Bottom);
// → Some(VerticalAlignment::Bottom)
let from_gen = VerticalAlignment::from_alignment(Alignment::Left);
// → None（Left 不是垂直方向）

let to_gen: Alignment = VerticalAlignment::Bottom.into();

let back: VerticalAlignment = Alignment::Bottom.try_into().unwrap();
```

---

## Font

一個完整的字型描述符，用於文字渲染和主題設定。

```rust
pub struct Font {
    pub family: String,  // 字型家族名稱（例如 "Arial"、"Noto Sans"）
    pub size: f32,       // 點大小
    pub weight: u16,     // CSS 風格的粗細：100..=900（正規化為 100 的倍數）
    pub bold: bool,      // 衍生：weight >= 700 時為 true
    pub italic: bool,    // 斜體樣式
}
```

### 常數

```rust
Font::REGULAR_WEIGHT  // 400
Font::BOLD_WEIGHT     // 700
```

### 建構函式

```rust
use rust_widgets::core::Font;

// 完整建構函式（粗細從 bold 標記衍生）：
let f1 = Font::new("Arial", 14.0, true, false);   // 粗體
let f2 = Font::new("Arial", 14.0, false, true);   // 斜體
let f3 = Font::new("Arial", 14.0, true, true);    // 粗體斜體

// 明確粗細建構函式：
let f4 = Font::with_weight("Arial", 14.0, 300, false);  // 細體
let f5 = Font::with_weight("Arial", 14.0, 600, false);  // 半粗體

// 便利建構函式：
let f6 = Font::simple("Arial", 14.0);             // 一般
let f7 = Font::bold("Arial", 14.0);               // 粗體
let f8 = Font::italic("Arial", 14.0);             // 斜體
let f9 = Font::bold_italic("Arial", 14.0);         // 粗體斜體

// 從數值型別：
let f10 = Font::with_i32_size("Arial", 14, false, false);
let f11 = Font::with_u32_size("Arial", 14, false, false);
let f12 = Font::with_f64_size("Arial", 14.0, false, false);

// 從元組：
let f13 = Font::from_tuple("Arial", 14.0);                    // 一般
let f14 = Font::from_tuple_with_bold("Arial", 14.0, true);    // 粗體
let f15 = Font::from_full_tuple("Arial", 14.0, true, true);   // 粗體斜體

// 預設 UI 字型：
let ui = Font::default_ui();          // Arial 14px 一般
let ui_bold = Font::default_ui_bold(); // Arial 14px 粗體
```

### 建構器風格變異（不可變）

所有變異方法都回傳一個**新的** `Font`：

```rust
let base = Font::simple("Arial", 14.0);

let bigger = base.with_size(18.0);
let light = base.with_weight_value(300);
let normal = base.with_bold(false);
let slanted = base.with_italic(true);
let serif = base.with_family("Times New Roman");

// 縮放：
let double = base.scaled(2.0);      // 28px
let half = base.scaled_down(2.0);   // 7px
```

### 驗證

```rust
let valid = Font::simple("Arial", 14.0);
assert!(valid.is_valid());  // ✓

let empty_family = Font::simple("", 14.0);
assert!(!empty_family.is_valid());  // ✗

let zero_size = Font::simple("Arial", 0.0);
assert!(!zero_size.is_valid());  // ✗
```

### 粗細正規化

粗細會自動正規化為最接近的 100 倍數，並限制在 `[100, 900]` 範圍內：

```rust
let w = Font::normalize_weight(149);   // → 100
let w = Font::normalize_weight(550);   // → 600
let w = Font::normalize_weight(2000);  // → 900
```

`bold` 標記是衍生而來的：weight ≥ 700 → `bold = true`。

### 查詢方法

```rust
let font = Font::with_weight("Arial", 14.0, 300, true);

assert!(font.is_light());     // weight ≤ 300
assert!(!font.is_regular());  // weight != 400
assert!(!font.is_bold());     // weight < 700
```

### CSS 輸出

```rust
let font = Font::bold_italic("Arial", 14.0);

assert_eq!(font.weight_css(), "700");
assert_eq!(font.style_css(), "italic");
assert_eq!(font.to_css(), "italic 700 14px Arial");
```

### 大小存取

```rust
let font = Font::simple("Arial", 13.7);

let size_i32 = font.size_i32();  // 14（四捨五入）
let size_u32 = font.size_u32();  // 14
```

### Serde 支援

`Font` 實作了 `Serialize`/`Deserialize`，向後相容的反序列化可從舊版 `bold` 欄位衍生 `weight`：

```json
// 新格式：
{"family": "Arial", "size": 14.0, "weight": 700, "bold": true, "italic": false}

// 舊版格式（weight 從 bold 衍生）：
{"family": "Arial", "size": 14.0, "bold": true, "italic": false}
```

---

## RuntimeProfile

控制編譯期的功能可用性和後端選擇。

```rust
pub enum RuntimeProfile {
    Full,      // 桌面導向，可選進階模組
    Embedded,  // 受限環境
}
```

由 Cargo 功能標記選擇：`desktop`/`tablet`/`mobile` → `Full`；`embedded`/`mini` → `Embedded`。

```rust
use rust_widgets::core::RuntimeProfile;

let profile = RuntimeProfile::Full;
assert_eq!(profile, RuntimeProfile::Full);
assert_ne!(profile, RuntimeProfile::Embedded);
```

---

## DeviceClass

針對觸控目標尺寸和佈局適應的裝置外型分類。

```rust
pub enum DeviceClass {
    Desktop,    // 大螢幕、滑鼠+鍵盤、可選觸控
    Tablet,     // 中螢幕、觸控優先
    Mobile,     // 小螢幕、觸控優先
    Embedded,   // 受限顯示器、有限輸入
    Projector,  // 大型唯讀顯示器、遙控器輸入
}
```

```rust
use rust_widgets::core::DeviceClass;

let class = DeviceClass::Desktop;

match class {
    DeviceClass::Desktop   => { /* 1920x1080、滑鼠+鍵盤 */ }
    DeviceClass::Tablet    => { /* 觸控優先、中螢幕 */ }
    DeviceClass::Mobile    => { /* 緊湊、僅觸控 */ }
    DeviceClass::Embedded  => { /* 受限 */ }
    DeviceClass::Projector => { /* 大型、唯讀 */ }
}
```

---

## PlatformFamily

用於後端選擇的平台家族分類。

```rust
pub enum PlatformFamily {
    Desktop,
    Embedded,
    Mobile,
    Tablet,
    Projector,
}
```

```rust
use rust_widgets::core::PlatformFamily;

let family = PlatformFamily::Desktop;
```

---

## Version

含解析、比較和打包整數支援的語意化版本。

```rust
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

### 建構函式與轉換

```rust
use rust_widgets::core::Version;

// 直接：
let v1 = Version::new(0, 9, 6);

// 從/到打包的 u32：
let packed = Version::from_u32(0x000906);  // { major: 0, minor: 9, patch: 6 }
let v = Version::new(0, 9, 6);
assert_eq!(v.to_u32(), 0x000906);

// 從字串解析：
let v2 = Version::parse_str("1.2.3").unwrap();
let v3: Version = "0.9.6".parse().unwrap();
```

### 比較

```rust
let v1 = Version::new(1, 0, 0);
let v2 = Version::new(1, 5, 0);
let v3 = Version::new(2, 0, 0);

// 相容性（相同 major）：
assert!(v1.is_compatible_with(&v2));   // ✓ 兩者都是 1.x
assert!(!v1.is_compatible_with(&v3));  // ✗ 1.x vs 2.x

// 排序：
assert!(v2.is_newer_than(&v1));
assert!(v1.is_older_than(&v3));
```

### Display 與錯誤處理

```rust
let v = Version::new(0, 9, 6);
println!("{}", v);  // "0.9.6"

// 無效字串：
assert!(Version::parse_str("1.2").is_err());       // 缺少 patch
assert!(Version::parse_str("1.2.3.4").is_err());   // 元件過多
assert!(Version::parse_str("a.b.c").is_err());     // 非數值
```

---

## PlatformCapabilities

描述目前平台的硬體和輸入能力。

```rust
pub struct PlatformCapabilities {
    pub has_gpu: bool,
    pub has_touch: bool,
    pub has_keyboard: bool,
    pub has_mouse: bool,
    pub screen_width: u32,
    pub screen_height: u32,
    pub dpi_scale: f32,
}
```

### 預定義平台預設集

```rust
use rust_widgets::core::PlatformCapabilities;

// Desktop：1920×1080、GPU、鍵盤+滑鼠、無觸控、1x DPI
let desktop = PlatformCapabilities::desktop();

// Embedded：800×480、無 GPU、觸控、無鍵盤/滑鼠、1x DPI
let embedded = PlatformCapabilities::embedded();

// Mobile：1080×1920（直立）、GPU、觸控、無鍵盤/滑鼠、2x DPI
let mobile = PlatformCapabilities::mobile();
```

### 工具方法

```rust
let caps = PlatformCapabilities::desktop();

// 取得螢幕尺寸為 Size：
let size = caps.screen_size();    // Size { width: 1920, height: 1080 }

// 取得螢幕矩形：
let rect = caps.screen_rect();    // Rect { x: 0, y: 0, width: 1920, height: 1080 }
```

---

## CoreConfig

將設定檔、平台、功能和版本打包成一個設定。

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### 預定義設定

```rust
use rust_widgets::core::CoreConfig;

// Desktop：Full 設定檔、Desktop 平台、桌面功能、v0.9.6
let desktop_config = CoreConfig::desktop();

// Embedded：Embedded 設定檔、Embedded 平台、內嵌功能、v0.9.6
let embedded_config = CoreConfig::embedded();

// Mobile：Full 設定檔、Mobile 平台、行動功能、v0.9.6
let mobile_config = CoreConfig::mobile();
```

---

## CoreError 與 CoreResult

### `CoreError` — 核心操作錯誤

```rust
pub enum CoreError {
    InvalidArgument(String),
    NotSupported(String),
    NotFound(String),
    Internal(String),
}
```

實作了 `Display`、`Error` 和 `From<crate::error::RwError>`。

```rust
use rust_widgets::core::CoreError;

let err = CoreError::InvalidArgument("width must be positive".to_string());
println!("{}", err);  // "Invalid argument: width must be positive"

// 從 RwError 轉換（將錯誤 ID 對應到 CoreError 變體）：
// INVALID_ARGUMENT → CoreError::InvalidArgument
// UNSUPPORTED_OPERATION / NOT_IMPLEMENTED → CoreError::NotSupported
// FILE_NOT_FOUND → CoreError::NotFound
// 其他全部 → CoreError::Internal
```

### `CoreResult<T>`

```rust
pub type CoreResult<T> = Result<T, CoreError>;
```

---

## CoreObject Trait

由可透過 `ObjectId` 定址的物件實作。

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

此 trait 是物件系統的基礎 — 函式庫中的每個 `Object` 都實作 `CoreObject`：

```rust
use rust_widgets::core::{CoreObject, ObjectId};

struct MyObject {
    id: ObjectId,
}

impl CoreObject for MyObject {
    fn id(&self) -> ObjectId {
        self.id
    }
    fn set_id(&mut self, id: ObjectId) {
        self.id = id;
    }
    fn type_name(&self) -> &'static str {
        "MyObject"
    }
}
```

---

## 座標轉換工具（`core::coords`）

座標系統使用**螢幕座標**原點（左上角）。轉換函式橋接了笛卡兒（左下角）和 PDF 系統。

### 螢幕 ↔ 笛卡兒

```rust
use rust_widgets::core::coords;

// Y 軸轉換：
let screen_y = to_screen_y(0.0, 100.0);      // → 100.0
let cart_y = to_cartesian_y(0.0, 100.0);      // → 100.0

// 整數變體：
let screen_y = to_screen_y_i32(0, 100);       // → 100
let cart_y = to_cartesian_y_i32(0, 100);       // → 100

// 點轉換：
let screen_pt = point_to_screen(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }
let cart_pt = point_to_cartesian(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }

// 浮點數變體：
let (sx, sy) = point_to_screen_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)
let (cx, cy) = point_to_cartesian_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)

// 矩形轉換：
let cart_rect = Rect::new(10, 0, 50, 30);
let screen_rect = rect_to_screen(cart_rect, 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
let back = rect_to_cartesian(screen_rect, 100);
// → Rect { x: 10, y: 0, width: 50, height: 30 }
```

### 螢幕 ↔ PDF

```rust
use rust_widgets::core::coords;

let pdf_y = to_pdf_y(0.0, 100.0);      // → 100.0
let screen_y = from_pdf_y(0.0, 100.0); // → 100.0
```

### 翻轉操作

```rust
let flipped = flip_y(0.0, 100.0);                  // → 100.0
let flipped_pt = flip_point_y(Point::new(10, 0), 100);  // → (10, 100)
let flipped_rect = flip_rect_y(Rect::new(10, 0, 50, 30), 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
```

### 座標正規化

```rust
let (nx, ny) = normalize_coords(100.0, 50.0, 200.0, 100.0);
// → (0.5, 0.5)

let (px, py) = denormalize_coords(0.5, 0.5, 200.0, 100.0);
// → (100.0, 50.0)
```

### 限制

```rust
let rect = Rect::new(10, 20, 640, 480);

// 將點限制在矩形範圍內：
let clamped = clamp_point_to_rect(Point::new(1000, 1000), rect);
// → Point { x: 649, y: 499 }

// 浮點數變體：
let (cx, cy) = clamp_point_to_rect_f32(1000.0, 1000.0, 10.0, 20.0, 640.0, 480.0);
// → (649.0, 499.0)
```

### DPI 縮放

```rust
let px = dpi_to_pixels(100.0, 2.0);      // → 200.0
let dp = pixels_to_dpi(200.0, 2.0);     // → 100.0

let px_i32 = dpi_to_pixels_i32(100, 2.0);    // → 200
let dp_i32 = pixels_to_dpi_i32(200, 2.0);    // → 100
```

### 往返不變性

所有轉換配對都是精確的反函式：

```rust
let y = 42.0;
let height = 100.0;
assert_eq!(to_cartesian_y(to_screen_y(y, height), height), y);
assert_eq!(to_screen_y(to_cartesian_y(y, height), height), y);
```

---

## 矩形合併（`core::rect_merge`）

### `merge_intersecting_rects`

使用具有重複遍歷的貪婪演算法，將重疊矩形合併為最小覆蓋集合：

```rust
use rust_widgets::core::{Rect, rect_merge::merge_intersecting_rects};

let rects = vec![
    Rect::new(0, 0, 100, 100),      // 與下一個重疊
    Rect::new(50, 50, 100, 100),    // 與前一個重疊
    Rect::new(200, 200, 50, 50),    // 不相交
];

let merged = merge_intersecting_rects(&rects);
assert_eq!(merged.len(), 2);
// merged[0]: Rect { x: 0, y: 0, width: 150, height: 150 }
// merged[1]: Rect { x: 200, y: 200, width: 50, height: 50 }
```

此功能由 `performance::region` 中的髒區域追蹤器以及渲染批次系統使用，以最小化繪圖呼叫。

### `bounding_rect`

計算一組矩形的最小邊界矩形：

```rust
use rust_widgets::core::{Rect, rect_merge::bounding_rect};

let rects = vec![
    Rect::new(0, 0, 10, 10),
    Rect::new(100, 100, 50, 50),
];

let bounds = bounding_rect(&rects);
assert_eq!(bounds, Some(Rect::new(0, 0, 150, 150)));

// 空輸入：
assert_eq!(bounding_rect(&[]), None);
```

---

## Mutex 擴充（`core::MutexExt`）

提供 mutex 鎖定的中毒恢復，避免在執行緒持有鎖定時發生 panic 而導致問題。

```rust
use rust_widgets::core::MutexExt;
use rust_widgets::compat::Mutex;

let mutex = Mutex::new(42i32);

// 不要這樣：
// let guard = mutex.lock().expect("mutex poisoned");

// 而要：
let guard = mutex.lock_guard();
// 如果中毒，則透過 into_inner() 恢復並繼續執行。
```

### Trait 定義

```rust
pub trait MutexExt<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
```

---

## 座標慣例摘要

```text
螢幕座標（原點左上角）：
  (0, 0) -------------> X（向右增加）
    |
    |    Widget 定位
    |    渲染
    |    SVG 輸出
    |
    v Y（向下增加）

笛卡兒座標（原點左下角）：
    ^ Y（向上增加）
    |
    |    圖表資料座標
    |
    |
  (0, 0) -------------> X（向右增加）

PDF 座標（原點左下角）：
    ^ Y（向上增加）
    |
    |    PDF 輸出
    |
    |
  (0, 0) -------------> X（向右增加）
```

| 系統 | 原點 | 使用於 | 轉換 |
|---|---|---|---|
| **螢幕** | 左上角 | Widgets、佈局、渲染、SVG | 預設 |
| **笛卡兒** | 左下角 | 圖表資料 | `to_cartesian_y`、`point_to_cartesian` |
| **PDF** | 左下角 | PDF 輸出 | `to_pdf_y`、`from_pdf_y` |

---

## 後續步驟

- **Widget 系統** — 了解 widgets 如何使用這些核心型別、`Widget` trait、`BaseWidget` 以及完整的 widget 階層
- **佈局系統** — 了解 `Rect`、`Size` 和 `Point` 如何被佈局演算法用於定位 widgets
- **渲染系統** — 學習 `Color`、`Font` 和座標變換如何輸入到渲染管線
