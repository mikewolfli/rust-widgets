# 核心类型

本章提供了 `src/core/` 模块中每个类型的全面参考——这些是整个 `rust-widgets` 库所构建的基础构建块。

---

## 模块概览

```rust
// src/core/mod.rs — 公有重新导出
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

| 文件 | 内容 |
|---|---|
| `types.rs` | `ObjectId`, `RuntimeProfile`, `DeviceClass`, `PlatformFamily`, `Version`, `PlatformCapabilities`, `CoreConfig`, `CoreError`, `CoreObject`, `CoreResult` |
| `geometry.rs` | `Point`, `Size`, `Rect`, `Orientation` |
| `color.rs` | `Color` — RGBA，包含 55+ 预定义常量 |
| `alignment.rs` | `Alignment`, `HorizontalAlignment`, `VerticalAlignment` |
| `font.rs` | `Font` — family, size, weight, bold, italic |
| `coords.rs` | 屏幕、笛卡尔和 PDF 系统之间的坐标转换 |
| `rect_merge.rs` | `merge_intersecting_rects()`, `bounding_rect()` |
| `mutex_ext.rs` | `MutexExt` — 毒化恢复扩展 trait |

---

## ObjectId

一个稳定的 `u64` 包装器，唯一标识每个控件和核心对象。

```rust
pub type ObjectId = u64;
```

`ObjectId` 是一个类型别名，而非包装结构体，以实现最大的 C 互操作兼容性。每个控件都使用唯一的 `ObjectId` 创建，整个控件层次结构使用 `ObjectId` 引用而不是 Rust 引用，避免了生命周期上的麻烦。

```rust
use rust_widgets::core::ObjectId;

let id: ObjectId = 42;

// ObjectId 无处不在：
fn show_widget(id: ObjectId);
fn set_widget_text(id: ObjectId, text: &str);
fn get_widget_text(id: ObjectId) -> String;
```

---

## Color

一个 4 通道 RGBA 颜色结构体，包含丰富的实用方法。

### 结构体定义

```rust
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
```

### 构造器

```rust
use rust_widgets::core::Color;

// RGBA 构造器（alpha 0–255）：
let red = Color::rgba(255, 0, 0, 255);
let semi_transparent = Color::rgba(0, 0, 255, 128);

// RGB 构造器（alpha = 255）：
let green = Color::rgb(0, 255, 0);

// 命名构造器：
let blue = Color::from_rgb(0, 0, 255);
let teal = Color::from_rgba(0, 128, 128, 255);

// 从 f32（0.0–1.0）：
let coral = Color::from_f32(1.0, 0.5, 0.31, 1.0);

// 从 i32（0–255）：
let pink = Color::from_i32(255, 192, 203, 255);

// 从 u32 打包：
let purple = Color::from_u32_rgba(0x800080FF);
let orange = Color::from_u32_rgb(0xFFA500);

// 从元组：
let navy = Color::from_u8_tuple((0, 0, 128));
let gray = Color::from_u8_rgb_tuple((128, 128, 128));
let mint = Color::from_f32_tuple((0.6, 1.0, 0.6));
```

### 55+ 预定义颜色常量

**标准颜色：**
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

**灰度：**
```rust
Color::GRAY                // (128, 128, 128)
Color::LIGHT_GRAY          // (211, 211, 211)
Color::DARK_GRAY           // (64, 64, 64)
Color::EXTRA_LIGHT_GRAY    // (238, 238, 238)
Color::MEDIUM_GRAY         // (160, 160, 160)
Color::EXTRA_DARK_GRAY     // (32, 32, 32)
```

**亮色/暗色变体：**
```rust
Color::LIGHT_RED      Color::DARK_RED
Color::LIGHT_GREEN    Color::DARK_GREEN
Color::LIGHT_BLUE     Color::DARK_BLUE
Color::LIGHT_YELLOW   Color::DARK_YELLOW
```

**语义化颜色：**
```rust
Color::PRIMARY        // 主品牌色
Color::SECONDARY      // 次要强调色
Color::SUCCESS        // 成功/正面反馈
Color::WARNING        // 警告/谨慎
Color::ERROR          // 错误/危险
Color::BACKGROUND     // 默认背景
Color::FOREGROUND     // 默认前景（文本）
Color::LINK           // 超链接颜色
Color::LINK_HOVER     // 超链接悬停颜色
Color::BORDER         // 默认边框
Color::DIVIDER        // 分隔线
Color::SELECTION      // 文本选择高亮
Color::TOOLTIP        // 提示文本背景
Color::MENU_BACKGROUND
Color::MENU_FOREGROUND
Color::INFO
Color::NOTIFICATION
Color::DISABLED_BACKGROUND
Color::DISABLED_FOREGROUND
```

**命名 Web 颜色：**
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

### 十六进制解析

```rust
// 解析十六进制字符串（#RGB, #RGBA, #RRGGBB, #RRGGBBAA）：
let red = Color::parse_hex("#FF0000").unwrap();
let semi_blue = Color::parse_hex("#0000FF80").unwrap();
let short_green = Color::parse_hex("#0F0").unwrap();

// 序列化回十六进制：
let hex_rgb = red.to_hex_rgb();        // "#ff0000"
let hex_rgba = semi_blue.to_hex_rgba(); // "#0000ff80"
```

### 打包整数转换

```rust
let color = Color::rgb(0xAA, 0xBB, 0xCC);

// 打包为 u32：
let packed = color.to_rgba_u32();  // 0xAABBCCFF

// 从 u32 解包：
let restored = Color::from_rgba_u32(packed);
assert_eq!(color, restored);
```

### 颜色操作

```rust
let base = Color::rgb(200, 100, 50);

// 修改 alpha：
let faded = base.with_alpha(128);               // u8
let faded2 = base.with_alpha_f32(0.5);           // f32

// Alpha 混合（over 操作符）：
let bg = Color::WHITE;
let fg = Color::rgba(255, 0, 0, 128);
let blended = bg.blend(&fg);
// 结果：(255, 128, 128) — 红色混合在白色上

// 亮度（感知亮度）：
let lum = blended.luminance();  // 0.0–1.0
assert!(blended.is_light());    // luminance > 0.5

// 对比色（黑色或白色，取对比度更高的那个）：
let contrast = blended.contrast_color();

// 反转：
let inverted = blended.invert();
```

### Trait 实现

```rust
// Default = BLACK：
let default_color = Color::default();  // Color::BLACK

// 从十六进制字符串：
let from_str: Color = "#FF8800".into();

// Display（十六进制 RGBA）：
println!("{}", Color::rgb(255, 128, 0));  // "#ff8000ff"
```

---

## Point

2D 整数坐标。

```rust
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

### 构造器

```rust
use rust_widgets::core::Point;

// 直接：
let p1 = Point::new(10, 20);

// 原点 (0, 0)：
let origin = Point::origin();

// 从各种数字类型：
let p2 = Point::from_f32(10.5, 20.7);       // (11, 21)
let p3 = Point::from_f32_trunc(10.9, 20.1); // (10, 20)
let p4 = Point::from_u32(100, 200);          // (100, 200)
let p5 = Point::from_i64(1000, -500);        // (1000, -500)
let p6 = Point::from_f64(3.14, 2.72);        // (3, 3)
let p7 = Point::from_usize(640, 480);        // (640, 480)
let p8 = Point::from_isize(-10, 10);         // (-10, 10)

// 从元组：
let p9 = Point::from_i32_tuple((10, 20));
let p10 = Point::from_f32_tuple((10.5, 20.5));
let p11 = Point::from_u32_tuple((100, 200));
let p12 = Point::from_f64_tuple((3.14, 2.72));
let p13 = Point::from_usize_tuple((640, 480));
let p14 = Point::from_isize_tuple((-10, 10));

// 通过 trait 转换：
let p15: Point = (10, 20).into();
```

### 转换

```rust
let p = Point::new(10, 20);

let (x, y) = p.to_f32();   // (10.0, 20.0)
let (x, y) = p.to_f64();   // (10.0, 20.0)
let (x, y) = p.to_u32();   // (10, 20)
```

### 算术

```rust
let p = Point::new(10, 20);

// 添加元组偏移量：
let offset = p + (5, 10);  // Point { x: 15, y: 30 }
```

### 显示

```rust
println!("{}", Point::new(10, 20));  // "(10, 20)"
```

---

## Size

2D 尺寸（宽 × 高），以像素为单位。

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

### 构造器

```rust
use rust_widgets::core::Size;

// 直接：
let s1 = Size::new(640, 480);

// 从各种数字类型：
let s2 = Size::from_f32(640.5, 480.5);         // (641, 481)
let s3 = Size::from_f32_trunc(640.9, 480.9);   // (640, 480)
let s4 = Size::from_i32(640, 480);              // (640, 480)
let s5 = Size::from_i64(1920, 1080);            // (1920, 1080)
let s6 = Size::from_f64(640.0, 480.0);          // (640, 480)
let s7 = Size::from_usize(800, 600);            // (800, 600)
let s8 = Size::from_isize(1024, 768);           // (1024, 768)

// 从元组：
let s9 = Size::from_u32_tuple((640, 480));
let s10 = Size::from_f32_tuple((640.0, 480.0));
let s11 = Size::from_i32_tuple((640, 480));
let s12 = Size::from_f64_tuple((640.0, 480.0));
let s13 = Size::from_usize_tuple((800, 600));
let s14 = Size::from_isize_tuple((1024, 768));
```

### 实用方法

```rust
let s = Size::new(640, 480);

// 检查是否为零尺寸：
assert!(!s.is_empty());

// 计算面积：
let area = s.area();  // 640 * 480 = 307200

// 宽高比（width / height）：
let ratio = s.aspect_ratio();  // 1.333...
```

### 转换

```rust
let s = Size::new(640, 480);

let (w, h) = s.to_f32();  // (640.0, 480.0)
let (w, h) = s.to_f64();  // (640.0, 480.0)
let (w, h) = s.to_i32();  // (640, 480)
```

### 算术

```rust
let s = Size::new(100, 100);
let bigger = s + (50, 50);  // Size { width: 150, height: 150 }
```

### 显示

```rust
println!("{}", Size::new(640, 480));  // "640x480"
```

---

## Rect

定位的矩形——最常用的几何类型。

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

### 构造器

```rust
use rust_widgets::core::Rect;

// 直接 (x, y, width, height)：
let r1 = Rect::new(10, 20, 640, 480);

// 从混合类型：
let r2 = Rect::from_mixed(10, 20, 640u32, 480u32);

// 从位置 + 尺寸：
let r3 = Rect::from_position_size(Point::new(10, 20), Size::new(640, 480));

// 从元组：
let r4 = Rect::from_tuple((10, 20, 640, 480));
let r5 = Rect::from_u32_tuple((10, 20, 640, 480));
let r6 = Rect::from_f32_tuple((10.0, 20.0, 640.0, 480.0));

// 从各种数字类型：
let r7 = Rect::from_f32(10.0, 20.0, 640.0, 480.0);
let r8 = Rect::from_f64(10.0, 20.0, 640.0, 480.0);
let r9 = Rect::from_i64(10, 20, 640, 480);
let r10 = Rect::from_usize(10, 20, 640, 480);
let r11 = Rect::from_isize(10, 20, 640, 480);

// 从两个角点：
let r12 = Rect::from_points(Point::new(10, 20), Point::new(650, 500));

// 从中心点和尺寸：
let r13 = Rect::from_center(Point::new(330, 260), Size::new(640, 480));
```

### 分解

```rust
let rect = Rect::new(10, 20, 640, 480);

let position = rect.position();       // Point { x: 10, y: 20 }
let size = rect.size();               // Size { width: 640, height: 480 }
let (x, y, w, h) = rect.decompose();  // (10, 20, 640, 480)

// 边坐标：
let right = rect.right();    // x + width = 650
let bottom = rect.bottom();  // y + height = 500
let center = rect.center();  // Point { x: 330, y: 260 }
```

### 验证与包含关系

```rust
let rect = Rect::new(10, 20, 640, 480);

// 矩形是否有效？（width > 0 && height > 0）
assert!(rect.is_valid());

// 点包含（最大边为开区间）：
let inside = Point::new(100, 100);
let outside = Point::new(1000, 1000);
assert!(rect.contains_point(inside));
assert!(!rect.contains_point(outside));

// 将点限制在矩形内：
let clamped = rect.clamp_point(Point::new(1000, 1000));
// → Point { x: 649, y: 499 }

// 矩形相交测试：
let other = Rect::new(300, 200, 500, 400);
assert!(rect.intersects(&other));

// 完全包含：
let small = Rect::new(100, 100, 50, 50);
assert!(rect.contains_rect(&small));
assert!(rect.contains(small));  // 别名

// 面积：
let area = rect.area();  // 640 * 480 = 307200
```

### 布尔运算

```rust
let a = Rect::new(0, 0, 200, 200);
let b = Rect::new(100, 100, 200, 200);

// 并集（包含两者的最小矩形）：
let union = a.union(&b);         // Rect { x: 0, y: 0, width: 300, height: 300 }

// 交集（重叠区域）：
let intersection = a.intersection(&b);
// → Some(Rect { x: 100, y: 100, width: 100, height: 100 })

let disjoint = Rect::new(500, 500, 100, 100);
assert!(a.intersection(&disjoint).is_none());
```

### 变换

```rust
let rect = Rect::new(100, 100, 200, 200);

// 添加内边距（向内扩展）：
let padded = rect.with_padding(10);  // 所有边：10px
let padded_vh = rect.with_padding((5, 10));  // 垂直：5，水平：10

// 添加外边距（向外扩展）：
let margined = rect.with_margin(10);  // 所有边：10px
let margined_ltrb = rect.with_margin((1, 2, 3, 4));  // 左，上，右，下

// 按量收缩：
let shrunk = rect.shrink(10);

// 按量扩大：
let grown = rect.grow(10);

// 扩展以包含某点：
let extended = rect.extend_to_include(Point::new(500, 500));

// 触摸目标扩展（不会小于 44x44）：
let touch = rect.expand_to_touch_target();
// 如果 width < 44 或 height < 44，则在两侧均匀扩展
```

### 转换

```rust
let rect = Rect::new(10, 20, 640, 480);

let (x, y, w, h) = rect.to_f32();  // (10.0, 20.0, 640.0, 480.0)
let (x, y, w, h) = rect.to_f64();  // (10.0, 20.0, 640.0, 480.0)
let (x, y, w, h) = rect.to_u32();  // (10, 20, 640, 480)
```

### 默认值

```rust
let default = Rect::default();
// Rect { x: 0, y: 0, width: 0, height: 0 }
```

### 显示

```rust
println!("{}", Rect::new(10, 20, 640, 480));
// "Rect { x: 10, y: 20, width: 640, height: 480 }"
```

---

## Orientation

用于水平与垂直布局的简单两变体枚举。

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
    Orientation::Horizontal => { /* 从左到右布局 */ }
    Orientation::Vertical   => { /* 从上到下布局 */ }
}
```

---

## Alignment

三种对齐枚举，用于在布局中定位元素。

### `Alignment` — 完整的 5 方向

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

// 轴检查：
assert!(align.is_horizontal());  // false（Center 适用于两个轴）
assert!(Alignment::Left.is_horizontal());
assert!(Alignment::Top.is_vertical());

// 从字符串解析：
let from_str = Alignment::parse_str("center");  // Some(Alignment::Center)
let short = Alignment::parse_str("l");          // Some(Alignment::Left)

// 转换为字符串：
let s = align.as_str();  // "center"

// CSS 值：
let text_align = Alignment::Left.css_text_align();       // Some("left")
let vert_align = Alignment::Top.css_vertical_align();    // Some("top")

// 相反方向：
assert_eq!(Alignment::Left.opposite(), Alignment::Right);
assert_eq!(Alignment::Top.opposite(), Alignment::Bottom);
assert_eq!(Alignment::Center.opposite(), Alignment::Center);
```

### `HorizontalAlignment` — 3 方向

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

// 从字符串：
let h = HorizontalAlignment::parse_str("center").unwrap();
assert!(h.is_center());

// 转换为字符串：
assert_eq!(h.as_str(), "center");

// 与 Alignment 相互转换：
let from_gen = HorizontalAlignment::from_alignment(Alignment::Right);
// → Some(HorizontalAlignment::Right)
let from_gen = HorizontalAlignment::from_alignment(Alignment::Top);
// → None（Top 不是水平方向）

let to_gen: Alignment = HorizontalAlignment::Left.into();

let back: HorizontalAlignment = Alignment::Left.try_into().unwrap();
```

### `VerticalAlignment` — 3 方向

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

// 从字符串：
let v = VerticalAlignment::parse_str("bottom").unwrap();
assert!(v.is_bottom());

// 转换为字符串：
assert_eq!(v.as_str(), "bottom");

// 与 Alignment 相互转换：
let from_gen = VerticalAlignment::from_alignment(Alignment::Bottom);
// → Some(VerticalAlignment::Bottom)
let from_gen = VerticalAlignment::from_alignment(Alignment::Left);
// → None（Left 不是垂直方向）

let to_gen: Alignment = VerticalAlignment::Bottom.into();

let back: VerticalAlignment = Alignment::Bottom.try_into().unwrap();
```

---

## Font

完整的字体描述符，用于文本渲染和主题系统中。

```rust
pub struct Font {
    pub family: String,  // 字体系列名称（例如 "Arial", "Noto Sans"）
    pub size: f32,       // 字号大小（point size）
    pub weight: u16,     // CSS 风格字重：100..=900（归一化为 100 的倍数）
    pub bold: bool,      // 派生属性：weight >= 700 时为 true
    pub italic: bool,    // 斜体样式
}
```

### 常量

```rust
Font::REGULAR_WEIGHT  // 400
Font::BOLD_WEIGHT     // 700
```

### 构造器

```rust
use rust_widgets::core::Font;

// 完整构造器（weight 从 bold 标志派生）：
let f1 = Font::new("Arial", 14.0, true, false);   // 粗体
let f2 = Font::new("Arial", 14.0, false, true);   // 斜体
let f3 = Font::new("Arial", 14.0, true, true);    // 粗体斜体

// 显式字重构造器：
let f4 = Font::with_weight("Arial", 14.0, 300, false);  // 细体
let f5 = Font::with_weight("Arial", 14.0, 600, false);  // 半粗体

// 便捷构造器：
let f6 = Font::simple("Arial", 14.0);             // 常规
let f7 = Font::bold("Arial", 14.0);               // 粗体
let f8 = Font::italic("Arial", 14.0);             // 斜体
let f9 = Font::bold_italic("Arial", 14.0);         // 粗体斜体

// 从数字类型：
let f10 = Font::with_i32_size("Arial", 14, false, false);
let f11 = Font::with_u32_size("Arial", 14, false, false);
let f12 = Font::with_f64_size("Arial", 14.0, false, false);

// 从元组：
let f13 = Font::from_tuple("Arial", 14.0);                    // 常规
let f14 = Font::from_tuple_with_bold("Arial", 14.0, true);    // 粗体
let f15 = Font::from_full_tuple("Arial", 14.0, true, true);   // 粗体斜体

// 默认 UI 字体：
let ui = Font::default_ui();          // Arial 14px 常规
let ui_bold = Font::default_ui_bold(); // Arial 14px 粗体
```

### 构建器风格变更（不可变）

所有变更方法返回一个**新的** `Font`：

```rust
let base = Font::simple("Arial", 14.0);

let bigger = base.with_size(18.0);
let light = base.with_weight_value(300);
let normal = base.with_bold(false);
let slanted = base.with_italic(true);
let serif = base.with_family("Times New Roman");

// 放大/缩小：
let double = base.scaled(2.0);      // 28px
let half = base.scaled_down(2.0);   // 7px
```

### 验证

```rust
let valid = Font::simple("Arial", 14.0);
assert!(valid.is_valid());  // ✓

let empty_family = Font::simple("", 14.0);
assert!(!empty_family.is_valid());  // ✗

let zero_size = Font::simple("Arial", 0.0);
assert!(!zero_size.is_valid());  // ✗
```

### 字重归一化

字重自动归一化为最近的 100 的倍数，限制在 `[100, 900]` 范围内：

```rust
let w = Font::normalize_weight(149);   // → 100
let w = Font::normalize_weight(550);   // → 600
let w = Font::normalize_weight(2000);  // → 900
```

`bold` 标志是派生的：weight ≥ 700 → `bold = true`。

### 查询方法

```rust
let font = Font::with_weight("Arial", 14.0, 300, true);

assert!(font.is_light());     // weight ≤ 300
assert!(!font.is_regular());  // weight != 400
assert!(!font.is_bold());     // weight < 700
```

### CSS 输出

```rust
let font = Font::bold_italic("Arial", 14.0);

assert_eq!(font.weight_css(), "700");
assert_eq!(font.style_css(), "italic");
assert_eq!(font.to_css(), "italic 700 14px Arial");
```

### 大小访问

```rust
let font = Font::simple("Arial", 13.7);

let size_i32 = font.size_i32();  // 14（四舍五入）
let size_u32 = font.size_u32();  // 14
```

### Serde 支持

`Font` 实现了 `Serialize`/`Deserialize`，具有向后兼容的反序列化功能，可从旧的 `bold` 字段派生 `weight`：

```json
// 新格式：
{"family": "Arial", "size": 14.0, "weight": 700, "bold": true, "italic": false}

// 旧格式（weight 从 bold 派生）：
{"family": "Arial", "size": 14.0, "bold": true, "italic": false}
```

---

## RuntimeProfile

控制编译时的功能可用性和后端选择。

```rust
pub enum RuntimeProfile {
    Full,      // 面向桌面，带有可选的高级模块
    Embedded,  // 受限环境
}
```

由 Cargo 功能标志选择：`desktop`/`tablet`/`mobile` → `Full`；`embedded`/`mini` → `Embedded`。

```rust
use rust_widgets::core::RuntimeProfile;

let profile = RuntimeProfile::Full;
assert_eq!(profile, RuntimeProfile::Full);
assert_ne!(profile, RuntimeProfile::Embedded);
```

---

## DeviceClass

用于触摸目标尺寸和布局自适应的外形因素分类。

```rust
pub enum DeviceClass {
    Desktop,    // 大屏幕，鼠标+键盘，可选触控
    Tablet,     // 中等屏幕，触控优先
    Mobile,     // 小屏幕，触控优先
    Embedded,   // 受限显示，有限输入
    Projector,  // 大型只读显示，遥控输入
}
```

```rust
use rust_widgets::core::DeviceClass;

let class = DeviceClass::Desktop;

match class {
    DeviceClass::Desktop   => { /* 1920x1080, 鼠标+键盘 */ }
    DeviceClass::Tablet    => { /* 触控优先，中等屏幕 */ }
    DeviceClass::Mobile    => { /* 紧凑型，仅触控 */ }
    DeviceClass::Embedded  => { /* 受限 */ }
    DeviceClass::Projector => { /* 大型，只读 */ }
}
```

---

## PlatformFamily

用于后端选择的平台家族分类。

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

语义化版本，支持解析、比较和打包整数。

```rust
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

### 构造器与转换

```rust
use rust_widgets::core::Version;

// 直接：
let v1 = Version::new(0, 9, 6);

// 与打包 u32 相互转换：
let packed = Version::from_u32(0x000906);  // { major: 0, minor: 9, patch: 6 }
let v = Version::new(0, 9, 6);
assert_eq!(v.to_u32(), 0x000906);

// 从字符串解析：
let v2 = Version::parse_str("1.2.3").unwrap();
let v3: Version = "0.9.6".parse().unwrap();
```

### 比较

```rust
let v1 = Version::new(1, 0, 0);
let v2 = Version::new(1, 5, 0);
let v3 = Version::new(2, 0, 0);

// 兼容性（相同 major）：
assert!(v1.is_compatible_with(&v2));   // ✓ 两者都是 1.x
assert!(!v1.is_compatible_with(&v3));  // ✗ 1.x vs 2.x

// 排序：
assert!(v2.is_newer_than(&v1));
assert!(v1.is_older_than(&v3));
```

### 显示与错误处理

```rust
let v = Version::new(0, 9, 6);
println!("{}", v);  // "0.9.6"

// 无效字符串：
assert!(Version::parse_str("1.2").is_err());       // 缺少 patch
assert!(Version::parse_str("1.2.3.4").is_err());   // 组件过多
assert!(Version::parse_str("a.b.c").is_err());     // 非数字
```

---

## PlatformCapabilities

描述当前平台的硬件和输入能力。

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

### 预定义平台预设

```rust
use rust_widgets::core::PlatformCapabilities;

// 桌面：1920×1080，GPU，键盘+鼠标，无触控，1x DPI
let desktop = PlatformCapabilities::desktop();

// 嵌入式：800×480，无 GPU，触控，无键盘/鼠标，1x DPI
let embedded = PlatformCapabilities::embedded();

// 移动端：1080×1920（竖屏），GPU，触控，无键盘/鼠标，2x DPI
let mobile = PlatformCapabilities::mobile();
```

### 实用方法

```rust
let caps = PlatformCapabilities::desktop();

// 以 Size 形式获取屏幕尺寸：
let size = caps.screen_size();    // Size { width: 1920, height: 1080 }

// 获取屏幕矩形：
let rect = caps.screen_rect();    // Rect { x: 0, y: 0, width: 1920, height: 1080 }
```

---

## CoreConfig

将配置文件、平台、能力和版本捆绑到一个配置中。

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### 预定义配置

```rust
use rust_widgets::core::CoreConfig;

// 桌面：Full profile, Desktop platform, desktop capabilities, v0.9.6
let desktop_config = CoreConfig::desktop();

// 嵌入式：Embedded profile, Embedded platform, embedded capabilities, v0.9.6
let embedded_config = CoreConfig::embedded();

// 移动端：Full profile, Mobile platform, mobile capabilities, v0.9.6
let mobile_config = CoreConfig::mobile();
```

---

## CoreError 与 CoreResult

### `CoreError` — 核心操作错误

```rust
pub enum CoreError {
    InvalidArgument(String),
    NotSupported(String),
    NotFound(String),
    Internal(String),
}
```

实现了 `Display`、`Error` 和 `From<crate::error::RwError>`。

```rust
use rust_widgets::core::CoreError;

let err = CoreError::InvalidArgument("width must be positive".to_string());
println!("{}", err);  // "Invalid argument: width must be positive"

// 从 RwError 转换（将错误 ID 映射到 CoreError 变体）：
// INVALID_ARGUMENT → CoreError::InvalidArgument
// UNSUPPORTED_OPERATION / NOT_IMPLEMENTED → CoreError::NotSupported
// FILE_NOT_FOUND → CoreError::NotFound
// 其他所有 → CoreError::Internal
```

### `CoreResult<T>`

```rust
pub type CoreResult<T> = Result<T, CoreError>;
```

---

## CoreObject Trait

可由 `ObjectId` 寻址的对象实现该 trait。

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

这个 trait 是对象系统的基础——库中的每个 `Object` 都实现了 `CoreObject`：

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

## 坐标转换工具（`core::coords`）

坐标系使用**屏幕坐标**原点（左上角）。转换函数桥接到笛卡尔坐标系（左下角）和 PDF 系统。

### 屏幕 ↔ 笛卡尔

```rust
use rust_widgets::core::coords;

// Y 轴转换：
let screen_y = to_screen_y(0.0, 100.0);      // → 100.0
let cart_y = to_cartesian_y(0.0, 100.0);      // → 100.0

// 整数变体：
let screen_y = to_screen_y_i32(0, 100);       // → 100
let cart_y = to_cartesian_y_i32(0, 100);       // → 100

// 点转换：
let screen_pt = point_to_screen(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }
let cart_pt = point_to_cartesian(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }

// 浮点变体：
let (sx, sy) = point_to_screen_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)
let (cx, cy) = point_to_cartesian_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)

// 矩形转换：
let cart_rect = Rect::new(10, 0, 50, 30);
let screen_rect = rect_to_screen(cart_rect, 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
let back = rect_to_cartesian(screen_rect, 100);
// → Rect { x: 10, y: 0, width: 50, height: 30 }
```

### 屏幕 ↔ PDF

```rust
use rust_widgets::core::coords;

let pdf_y = to_pdf_y(0.0, 100.0);      // → 100.0
let screen_y = from_pdf_y(0.0, 100.0); // → 100.0
```

### 翻转操作

```rust
let flipped = flip_y(0.0, 100.0);                  // → 100.0
let flipped_pt = flip_point_y(Point::new(10, 0), 100);  // → (10, 100)
let flipped_rect = flip_rect_y(Rect::new(10, 0, 50, 30), 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
```

### 坐标标准化

```rust
let (nx, ny) = normalize_coords(100.0, 50.0, 200.0, 100.0);
// → (0.5, 0.5)

let (px, py) = denormalize_coords(0.5, 0.5, 200.0, 100.0);
// → (100.0, 50.0)
```

### 限制

```rust
let rect = Rect::new(10, 20, 640, 480);

// 将点限制在矩形边界内：
let clamped = clamp_point_to_rect(Point::new(1000, 1000), rect);
// → Point { x: 649, y: 499 }

// 浮点变体：
let (cx, cy) = clamp_point_to_rect_f32(1000.0, 1000.0, 10.0, 20.0, 640.0, 480.0);
// → (649.0, 499.0)
```

### DPI 缩放

```rust
let px = dpi_to_pixels(100.0, 2.0);      // → 200.0
let dp = pixels_to_dpi(200.0, 2.0);     // → 100.0

let px_i32 = dpi_to_pixels_i32(100, 2.0);    // → 200
let dp_i32 = pixels_to_dpi_i32(200, 2.0);    // → 100
```

### 往返不变性

所有转换对都是精确的逆操作：

```rust
let y = 42.0;
let height = 100.0;
assert_eq!(to_cartesian_y(to_screen_y(y, height), height), y);
assert_eq!(to_screen_y(to_cartesian_y(y, height), height), y);
```

---

## 矩形合并（`core::rect_merge`）

### `merge_intersecting_rects`

使用带重复遍历的贪心算法将重叠矩形合并为最小的覆盖集合：

```rust
use rust_widgets::core::{Rect, rect_merge::merge_intersecting_rects};

let rects = vec![
    Rect::new(0, 0, 100, 100),      // 与下一个重叠
    Rect::new(50, 50, 100, 100),    // 与前一个重叠
    Rect::new(200, 200, 50, 50),    // 不相交
];

let merged = merge_intersecting_rects(&rects);
assert_eq!(merged.len(), 2);
// merged[0]: Rect { x: 0, y: 0, width: 150, height: 150 }
// merged[1]: Rect { x: 200, y: 200, width: 50, height: 50 }
```

这被 `performance::region` 中的脏区域跟踪器和渲染批次系统用来最小化绘制调用。

### `bounding_rect`

计算一组矩形的包围盒：

```rust
use rust_widgets::core::{Rect, rect_merge::bounding_rect};

let rects = vec![
    Rect::new(0, 0, 10, 10),
    Rect::new(100, 100, 50, 50),
];

let bounds = bounding_rect(&rects);
assert_eq!(bounds, Some(Rect::new(0, 0, 150, 150)));

// 空输入：
assert_eq!(bounding_rect(&[]), None);
```

---

## Mutex 扩展（`core::MutexExt`）

提供互斥锁的毒化恢复功能，避免在持有锁的线程发生 panic 时出现 panic。

```rust
use rust_widgets::core::MutexExt;
use rust_widgets::compat::Mutex;

let mutex = Mutex::new(42i32);

// 不是：
// let guard = mutex.lock().expect("mutex poisoned");

// 而是：
let guard = mutex.lock_guard();
// 如果中毒，通过 into_inner() 恢复并继续执行。
```

### Trait 定义

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

## 坐标约定总结

```text
屏幕坐标（原点左上角）：
  (0, 0) -------------> X（向右增加）
    |
    |    控件定位
    |    渲染
    |    SVG 输出
    |
    v Y（向下增加）

笛卡尔坐标（原点左下角）：
    ^ Y（向上增加）
    |
    |    图表数据坐标
    |
    |
  (0, 0) -------------> X（向右增加）

PDF 坐标（原点左下角）：
    ^ Y（向上增加）
    |
    |    PDF 输出
    |
    |
  (0, 0) -------------> X（向右增加）
```

| 系统 | 原点 | 用途 | 转换函数 |
|---|---|---|---|
| **屏幕** | 左上角 | 控件、布局、渲染、SVG | 默认 |
| **笛卡尔** | 左下角 | 图表数据 | `to_cartesian_y`, `point_to_cartesian` |
| **PDF** | 左下角 | PDF 输出 | `to_pdf_y`, `from_pdf_y` |

---

## 下一步

- **控件系统**——理解控件如何使用这些核心类型、`Widget` trait、`BaseWidget` 以及完整的控件层次结构
- **布局系统**——了解 `Rect`、`Size` 和 `Point` 如何被布局算法用于定位控件
- **渲染系统**——学习 `Color`、`Font` 和坐标变换如何输入到渲染管线
