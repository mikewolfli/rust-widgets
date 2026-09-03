# Core Types

This chapter provides a comprehensive reference for every type in the
`src/core/` module — the fundamental building blocks upon which the entire
rust-widgets library is built.

---

## Module Overview

```rust
// src/core/mod.rs — public re-exports
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

| File | Contents |
|---|---|
| `types.rs` | `ObjectId`, `RuntimeProfile`, `DeviceClass`, `PlatformFamily`, `Version`, `PlatformCapabilities`, `CoreConfig`, `CoreError`, `CoreObject`, `CoreResult` |
| `geometry.rs` | `Point`, `Size`, `Rect`, `Orientation` |
| `color.rs` | `Color` — RGBA with 55+ predefined constants |
| `alignment.rs` | `Alignment`, `HorizontalAlignment`, `VerticalAlignment` |
| `font.rs` | `Font` — family, size, weight, bold, italic |
| `coords.rs` | Coordinate conversion between screen, Cartesian, and PDF systems |
| `rect_merge.rs` | `merge_intersecting_rects()`, `bounding_rect()` |
| `mutex_ext.rs` | `MutexExt` — poison recovery extension trait |

---

## ObjectId

A stable `u64` wrapper that uniquely identifies every widget and core object.

```rust
pub type ObjectId = u64;
```

`ObjectId` is a type alias, not a wrapper struct, for maximum C-interop
compatibility. Every widget is created with a unique `ObjectId`, and the
entire widget hierarchy uses `ObjectId` references instead of Rust references,
avoiding lifetime contortions.

```rust
use rust_widgets::core::ObjectId;

let id: ObjectId = 42;

// ObjectId is used everywhere:
fn show_widget(id: ObjectId);
fn set_widget_text(id: ObjectId, text: &str);
fn get_widget_text(id: ObjectId) -> String;
```

---

## Color

A 4-channel RGBA color structure with extensive utility methods.

### Struct Definition

```rust
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
```

### Constructors

```rust
use rust_widgets::core::Color;

// RGBA constructor (alpha 0–255):
let red = Color::rgba(255, 0, 0, 255);
let semi_transparent = Color::rgba(0, 0, 255, 128);

// RGB constructor (alpha = 255):
let green = Color::rgb(0, 255, 0);

// Named constructors:
let blue = Color::from_rgb(0, 0, 255);
let teal = Color::from_rgba(0, 128, 128, 255);

// From f32 (0.0–1.0):
let coral = Color::from_f32(1.0, 0.5, 0.31, 1.0);

// From i32 (0–255):
let pink = Color::from_i32(255, 192, 203, 255);

// From u32 packed:
let purple = Color::from_u32_rgba(0x800080FF);
let orange = Color::from_u32_rgb(0xFFA500);

// From tuples:
let navy = Color::from_u8_tuple((0, 0, 128));
let gray = Color::from_u8_rgb_tuple((128, 128, 128));
let mint = Color::from_f32_tuple((0.6, 1.0, 0.6));
```

### 55+ Predefined Color Constants

**Standard Colors:**
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

**Grayscale:**
```rust
Color::GRAY                // (128, 128, 128)
Color::LIGHT_GRAY          // (211, 211, 211)
Color::DARK_GRAY           // (64, 64, 64)
Color::EXTRA_LIGHT_GRAY    // (238, 238, 238)
Color::MEDIUM_GRAY         // (160, 160, 160)
Color::EXTRA_DARK_GRAY     // (32, 32, 32)
```

**Light/Dark Variants:**
```rust
Color::LIGHT_RED      Color::DARK_RED
Color::LIGHT_GREEN    Color::DARK_GREEN
Color::LIGHT_BLUE     Color::DARK_BLUE
Color::LIGHT_YELLOW   Color::DARK_YELLOW
```

**Semantic Colors:**
```rust
Color::PRIMARY        // Primary brand color
Color::SECONDARY      // Secondary accent
Color::SUCCESS        // Success / positive feedback
Color::WARNING        // Warning / caution
Color::ERROR          // Error / danger
Color::BACKGROUND     // Default background
Color::FOREGROUND     // Default foreground (text)
Color::LINK           // Hyperlink color
Color::LINK_HOVER     // Hyperlink hover color
Color::BORDER         // Default border
Color::DIVIDER        // Divider line
Color::SELECTION      // Text selection highlight
Color::TOOLTIP        // Tooltip background
Color::MENU_BACKGROUND
Color::MENU_FOREGROUND
Color::INFO
Color::NOTIFICATION
Color::DISABLED_BACKGROUND
Color::DISABLED_FOREGROUND
```

**Named Web Colors:**
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

### Hex Parsing

```rust
// Parse hex strings (#RGB, #RGBA, #RRGGBB, #RRGGBBAA):
let red = Color::parse_hex("#FF0000").unwrap();
let semi_blue = Color::parse_hex("#0000FF80").unwrap();
let short_green = Color::parse_hex("#0F0").unwrap();

// Serialize back to hex:
let hex_rgb = red.to_hex_rgb();        // "#ff0000"
let hex_rgba = semi_blue.to_hex_rgba(); // "#0000ff80"
```

### Packed Integer Conversions

```rust
let color = Color::rgb(0xAA, 0xBB, 0xCC);

// Pack to u32:
let packed = color.to_rgba_u32();  // 0xAABBCCFF

// Unpack from u32:
let restored = Color::from_rgba_u32(packed);
assert_eq!(color, restored);
```

### Color Manipulation

```rust
let base = Color::rgb(200, 100, 50);

// Change alpha:
let faded = base.with_alpha(128);               // u8
let faded2 = base.with_alpha_f32(0.5);           // f32

// Alpha blending (over operator):
let bg = Color::WHITE;
let fg = Color::rgba(255, 0, 0, 128);
let blended = bg.blend(&fg);
// result: (255, 128, 128) — red blended over white

// Luminance (perceptual brightness):
let lum = blended.luminance();  // 0.0–1.0
assert!(blended.is_light());    // luminance > 0.5

// Contrast color (black or white, whichever has higher contrast):
let contrast = blended.contrast_color();

// Invert:
let inverted = blended.invert();
```

### Trait Implementations

```rust
// Default = BLACK:
let default_color = Color::default();  // Color::BLACK

// From hex string:
let from_str: Color = "#FF8800".into();

// Display (hex RGBA):
println!("{}", Color::rgb(255, 128, 0));  // "#ff8000ff"
```

---

## Point

A 2D integer coordinate.

```rust
pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

### Constructors

```rust
use rust_widgets::core::Point;

// Direct:
let p1 = Point::new(10, 20);

// Value (0, 0):
let origin = Point::origin();

// From various numeric types:
let p2 = Point::from_f32(10.5, 20.7);       // (11, 21)
let p3 = Point::from_f32_trunc(10.9, 20.1); // (10, 20)
let p4 = Point::from_u32(100, 200);          // (100, 200)
let p5 = Point::from_i64(1000, -500);        // (1000, -500)
let p6 = Point::from_f64(3.14, 2.72);        // (3, 3)
let p7 = Point::from_usize(640, 480);        // (640, 480)
let p8 = Point::from_isize(-10, 10);         // (-10, 10)

// From tuples:
let p9 = Point::from_i32_tuple((10, 20));
let p10 = Point::from_f32_tuple((10.5, 20.5));
let p11 = Point::from_u32_tuple((100, 200));
let p12 = Point::from_f64_tuple((3.14, 2.72));
let p13 = Point::from_usize_tuple((640, 480));
let p14 = Point::from_isize_tuple((-10, 10));

// From trait conversion:
let p15: Point = (10, 20).into();
```

### Conversions

```rust
let p = Point::new(10, 20);

let (x, y) = p.to_f32();   // (10.0, 20.0)
let (x, y) = p.to_f64();   // (10.0, 20.0)
let (x, y) = p.to_u32();   // (10, 20)
```

### Arithmetic

```rust
let p = Point::new(10, 20);

// Add a tuple offset:
let offset = p + (5, 10);  // Point { x: 15, y: 30 }
```

### Display

```rust
println!("{}", Point::new(10, 20));  // "(10, 20)"
```

---

## Size

A 2D dimension (width × height) in pixels.

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

### Constructors

```rust
use rust_widgets::core::Size;

// Direct:
let s1 = Size::new(640, 480);

// From various numeric types:
let s2 = Size::from_f32(640.5, 480.5);         // (641, 481)
let s3 = Size::from_f32_trunc(640.9, 480.9);   // (640, 480)
let s4 = Size::from_i32(640, 480);              // (640, 480)
let s5 = Size::from_i64(1920, 1080);            // (1920, 1080)
let s6 = Size::from_f64(640.0, 480.0);          // (640, 480)
let s7 = Size::from_usize(800, 600);            // (800, 600)
let s8 = Size::from_isize(1024, 768);           // (1024, 768)

// From tuples:
let s9 = Size::from_u32_tuple((640, 480));
let s10 = Size::from_f32_tuple((640.0, 480.0));
let s11 = Size::from_i32_tuple((640, 480));
let s12 = Size::from_f64_tuple((640.0, 480.0));
let s13 = Size::from_usize_tuple((800, 600));
let s14 = Size::from_isize_tuple((1024, 768));
```

### Utility Methods

```rust
let s = Size::new(640, 480);

// Check for zero size:
assert!(!s.is_empty());

// Calculate area:
let area = s.area();  // 640 * 480 = 307200

// Aspect ratio (width / height):
let ratio = s.aspect_ratio();  // 1.333...
```

### Conversions

```rust
let s = Size::new(640, 480);

let (w, h) = s.to_f32();  // (640.0, 480.0)
let (w, h) = s.to_f64();  // (640.0, 480.0)
let (w, h) = s.to_i32();  // (640, 480)
```

### Arithmetic

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

A positioned rectangle — the most-used geometry type.

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

### Constructors

```rust
use rust_widgets::core::Rect;

// Direct (x, y, width, height):
let r1 = Rect::new(10, 20, 640, 480);

// From mixed types:
let r2 = Rect::from_mixed(10, 20, 640u32, 480u32);

// From position + size:
let r3 = Rect::from_position_size(Point::new(10, 20), Size::new(640, 480));

// From tuples:
let r4 = Rect::from_tuple((10, 20, 640, 480));
let r5 = Rect::from_u32_tuple((10, 20, 640, 480));
let r6 = Rect::from_f32_tuple((10.0, 20.0, 640.0, 480.0));

// From various numeric types:
let r7 = Rect::from_f32(10.0, 20.0, 640.0, 480.0);
let r8 = Rect::from_f64(10.0, 20.0, 640.0, 480.0);
let r9 = Rect::from_i64(10, 20, 640, 480);
let r10 = Rect::from_usize(10, 20, 640, 480);
let r11 = Rect::from_isize(10, 20, 640, 480);

// From two corner points:
let r12 = Rect::from_points(Point::new(10, 20), Point::new(650, 500));

// From center point and size:
let r13 = Rect::from_center(Point::new(330, 260), Size::new(640, 480));
```

### Decomposition

```rust
let rect = Rect::new(10, 20, 640, 480);

let position = rect.position();       // Point { x: 10, y: 20 }
let size = rect.size();               // Size { width: 640, height: 480 }
let (x, y, w, h) = rect.decompose();  // (10, 20, 640, 480)

// Edge coordinates:
let right = rect.right();    // x + width = 650
let bottom = rect.bottom();  // y + height = 500
let center = rect.center();  // Point { x: 330, y: 260 }
```

### Validation and Containment

```rust
let rect = Rect::new(10, 20, 640, 480);

// Is the rectangle valid? (width > 0 && height > 0)
assert!(rect.is_valid());

// Point containment (exclusive on max edge):
let inside = Point::new(100, 100);
let outside = Point::new(1000, 1000);
assert!(rect.contains_point(inside));
assert!(!rect.contains_point(outside));

// Clamp a point to the rectangle:
let clamped = rect.clamp_point(Point::new(1000, 1000));
// → Point { x: 649, y: 499 }

// Rectangle intersection test:
let other = Rect::new(300, 200, 500, 400);
assert!(rect.intersects(&other));

// Full containment:
let small = Rect::new(100, 100, 50, 50);
assert!(rect.contains_rect(&small));
assert!(rect.contains(small));  // alias

// Area:
let area = rect.area();  // 640 * 480 = 307200
```

### Boolean Operations

```rust
let a = Rect::new(0, 0, 200, 200);
let b = Rect::new(100, 100, 200, 200);

// Union (smallest rect containing both):
let union = a.union(&b);         // Rect { x: 0, y: 0, width: 300, height: 300 }

// Intersection (overlapping region):
let intersection = a.intersection(&b);
// → Some(Rect { x: 100, y: 100, width: 100, height: 100 })

let disjoint = Rect::new(500, 500, 100, 100);
assert!(a.intersection(&disjoint).is_none());
```

### Transformation

```rust
let rect = Rect::new(100, 100, 200, 200);

// Add padding (grows inward):
let padded = rect.with_padding(10);  // All sides: 10px
let padded_vh = rect.with_padding((5, 10));  // vertical: 5, horizontal: 10

// Add margin (grows outward):
let margined = rect.with_margin(10);  // All sides: 10px
let margined_ltrb = rect.with_margin((1, 2, 3, 4));  // left, top, right, bottom

// Shrink by amount:
let shrunk = rect.shrink(10);

// Grow by amount:
let grown = rect.grow(10);

// Expand to include a point:
let extended = rect.extend_to_include(Point::new(500, 500));

// Touch target expansion (never smaller than 44x44):
let touch = rect.expand_to_touch_target();
// If width < 44 or height < 44, expands equally on both sides
```

### Conversions

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

A simple two-variant enum for horizontal vs vertical layout.

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
    Orientation::Horizontal => { /* lay out left-to-right */ }
    Orientation::Vertical   => { /* lay out top-to-bottom */ }
}
```

---

## Alignment

Three alignment enums for positioning elements within layouts.

### `Alignment` — Full 5-Way

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

// Axis checks:
assert!(align.is_horizontal());  // false (Center applies to both axes)
assert!(Alignment::Left.is_horizontal());
assert!(Alignment::Top.is_vertical());

// Parse from string:
let from_str = Alignment::parse_str("center");  // Some(Alignment::Center)
let short = Alignment::parse_str("l");          // Some(Alignment::Left)

// Convert to string:
let s = align.as_str();  // "center"

// CSS values:
let text_align = Alignment::Left.css_text_align();       // Some("left")
let vert_align = Alignment::Top.css_vertical_align();    // Some("top")

// Opposite:
assert_eq!(Alignment::Left.opposite(), Alignment::Right);
assert_eq!(Alignment::Top.opposite(), Alignment::Bottom);
assert_eq!(Alignment::Center.opposite(), Alignment::Center);
```

### `HorizontalAlignment` — 3-Way

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

// From string:
let h = HorizontalAlignment::parse_str("center").unwrap();
assert!(h.is_center());

// To string:
assert_eq!(h.as_str(), "center");

// Convert to/from Alignment:
let from_gen = HorizontalAlignment::from_alignment(Alignment::Right);
// → Some(HorizontalAlignment::Right)
let from_gen = HorizontalAlignment::from_alignment(Alignment::Top);
// → None (Top is not horizontal)

let to_gen: Alignment = HorizontalAlignment::Left.into();

let back: HorizontalAlignment = Alignment::Left.try_into().unwrap();
```

### `VerticalAlignment` — 3-Way

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

// From string:
let v = VerticalAlignment::parse_str("bottom").unwrap();
assert!(v.is_bottom());

// To string:
assert_eq!(v.as_str(), "bottom");

// Convert to/from Alignment:
let from_gen = VerticalAlignment::from_alignment(Alignment::Bottom);
// → Some(VerticalAlignment::Bottom)
let from_gen = VerticalAlignment::from_alignment(Alignment::Left);
// → None (Left is not vertical)

let to_gen: Alignment = VerticalAlignment::Bottom.into();

let back: VerticalAlignment = Alignment::Bottom.try_into().unwrap();
```

---

## Font

A complete font descriptor used across text rendering and theming.

```rust
pub struct Font {
    pub family: String,  // Font family name (e.g., "Arial", "Noto Sans")
    pub size: f32,       // Point size
    pub weight: u16,     // CSS-like weight: 100..=900 (normalized to multiples of 100)
    pub bold: bool,      // Derived: true when weight >= 700
    pub italic: bool,    // Italic style
}
```

### Constants

```rust
Font::REGULAR_WEIGHT  // 400
Font::BOLD_WEIGHT     // 700
```

### Constructors

```rust
use rust_widgets::core::Font;

// Full constructor (weight derived from bold flag):
let f1 = Font::new("Arial", 14.0, true, false);   // bold
let f2 = Font::new("Arial", 14.0, false, true);   // italic
let f3 = Font::new("Arial", 14.0, true, true);    // bold italic

// Explicit weight constructor:
let f4 = Font::with_weight("Arial", 14.0, 300, false);  // light
let f5 = Font::with_weight("Arial", 14.0, 600, false);  // semi-bold

// Convenience constructors:
let f6 = Font::simple("Arial", 14.0);             // regular
let f7 = Font::bold("Arial", 14.0);               // bold
let f8 = Font::italic("Arial", 14.0);             // italic
let f9 = Font::bold_italic("Arial", 14.0);         // bold italic

// From numeric types:
let f10 = Font::with_i32_size("Arial", 14, false, false);
let f11 = Font::with_u32_size("Arial", 14, false, false);
let f12 = Font::with_f64_size("Arial", 14.0, false, false);

// From tuples:
let f13 = Font::from_tuple("Arial", 14.0);                    // regular
let f14 = Font::from_tuple_with_bold("Arial", 14.0, true);    // bold
let f15 = Font::from_full_tuple("Arial", 14.0, true, true);   // bold italic

// Default UI fonts:
let ui = Font::default_ui();          // Arial 14px regular
let ui_bold = Font::default_ui_bold(); // Arial 14px bold
```

### Builder-Style Mutations (Immutable)

All mutation methods return a **new** `Font`:

```rust
let base = Font::simple("Arial", 14.0);

let bigger = base.with_size(18.0);
let light = base.with_weight_value(300);
let normal = base.with_bold(false);
let slanted = base.with_italic(true);
let serif = base.with_family("Times New Roman");

// Scale up/down:
let double = base.scaled(2.0);      // 28px
let half = base.scaled_down(2.0);   // 7px
```

### Validation

```rust
let valid = Font::simple("Arial", 14.0);
assert!(valid.is_valid());  // ✓

let empty_family = Font::simple("", 14.0);
assert!(!empty_family.is_valid());  // ✗

let zero_size = Font::simple("Arial", 0.0);
assert!(!zero_size.is_valid());  // ✗
```

### Weight Normalization

Weights are automatically normalized to the nearest multiple of 100,
clamped to `[100, 900]`:

```rust
let w = Font::normalize_weight(149);   // → 100
let w = Font::normalize_weight(550);   // → 600
let w = Font::normalize_weight(2000);  // → 900
```

The `bold` flag is derived: weight ≥ 700 → `bold = true`.

### Query Methods

```rust
let font = Font::with_weight("Arial", 14.0, 300, true);

assert!(font.is_light());     // weight ≤ 300
assert!(!font.is_regular());  // weight != 400
assert!(!font.is_bold());     // weight < 700
```

### CSS Output

```rust
let font = Font::bold_italic("Arial", 14.0);

assert_eq!(font.weight_css(), "700");
assert_eq!(font.style_css(), "italic");
assert_eq!(font.to_css(), "italic 700 14px Arial");
```

### Size Access

```rust
let font = Font::simple("Arial", 13.7);

let size_i32 = font.size_i32();  // 14 (rounds)
let size_u32 = font.size_u32();  // 14
```

### Serde Support

`Font` implements `Serialize`/`Deserialize` with backward-compatible
deserialization that derives `weight` from legacy `bold` fields:

```json
// New format:
{"family": "Arial", "size": 14.0, "weight": 700, "bold": true, "italic": false}

// Legacy format (weight derived from bold):
{"family": "Arial", "size": 14.0, "bold": true, "italic": false}
```

---

## RuntimeProfile

Controls feature availability and backend selection at compile time.

```rust
pub enum RuntimeProfile {
    Full,      // Desktop-oriented with optional advanced modules
    Embedded,  // Constrained environments
}
```

Selected by Cargo feature flag: `desktop`/`tablet`/`mobile` → `Full`;
`embedded`/`mini` → `Embedded`.

```rust
use rust_widgets::core::RuntimeProfile;

let profile = RuntimeProfile::Full;
assert_eq!(profile, RuntimeProfile::Full);
assert_ne!(profile, RuntimeProfile::Embedded);
```

---

## DeviceClass

Form-factor classification for touch target sizing and layout adaptation.

```rust
pub enum DeviceClass {
    Desktop,    // Large screen, mouse+keyboard, optionally touch
    Tablet,     // Medium screen, touch-first
    Mobile,     // Small screen, touch-first
    Embedded,   // Constrained display, limited input
    Projector,  // Large read-only display, remote control input
}
```

```rust
use rust_widgets::core::DeviceClass;

let class = DeviceClass::Desktop;

match class {
    DeviceClass::Desktop   => { /* 1920x1080, mouse+keyboard */ }
    DeviceClass::Tablet    => { /* touch-first, medium screen */ }
    DeviceClass::Mobile    => { /* compact, touch-only */ }
    DeviceClass::Embedded  => { /* constrained */ }
    DeviceClass::Projector => { /* large, read-only */ }
}
```

---

## PlatformFamily

Platform family classification for backend selection.

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

Semantic version with parsing, comparison, and packed integer support.

```rust
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

### Constructors and Conversions

```rust
use rust_widgets::core::Version;

// Direct:
let v1 = Version::new(0, 9, 6);

// From/To packed u32:
let packed = Version::from_u32(0x000906);  // { major: 0, minor: 9, patch: 6 }
let v = Version::new(0, 9, 6);
assert_eq!(v.to_u32(), 0x000906);

// Parse from string:
let v2 = Version::parse_str("1.2.3").unwrap();
let v3: Version = "1.0.0".parse().unwrap();
```

### Comparison

```rust
let v1 = Version::new(1, 0, 0);
let v2 = Version::new(1, 5, 0);
let v3 = Version::new(2, 0, 0);

// Compatibility (same major):
assert!(v1.is_compatible_with(&v2));   // ✓ both 1.x
assert!(!v1.is_compatible_with(&v3));  // ✗ 1.x vs 2.x

// Ordering:
assert!(v2.is_newer_than(&v1));
assert!(v1.is_older_than(&v3));
```

### Display and Error Handling

```rust
let v = Version::new(0, 9, 6);
println!("{}", v);  // "1.0.0"

// Invalid strings:
assert!(Version::parse_str("1.2").is_err());       // missing patch
assert!(Version::parse_str("1.2.3.4").is_err());   // too many components
assert!(Version::parse_str("a.b.c").is_err());     // non-numeric
```

---

## PlatformCapabilities

Describes the hardware and input capabilities of the current platform.

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

### Predefined Platform Presets

```rust
use rust_widgets::core::PlatformCapabilities;

// Desktop: 1920×1080, GPU, keyboard+mouse, no touch, 1x DPI
let desktop = PlatformCapabilities::desktop();

// Embedded: 800×480, no GPU, touch, no keyboard/mouse, 1x DPI
let embedded = PlatformCapabilities::embedded();

// Mobile: 1080×1920 (portrait), GPU, touch, no keyboard/mouse, 2x DPI
let mobile = PlatformCapabilities::mobile();
```

### Utility Methods

```rust
let caps = PlatformCapabilities::desktop();

// Get screen size as Size:
let size = caps.screen_size();    // Size { width: 1920, height: 1080 }

// Get screen rectangle:
let rect = caps.screen_rect();    // Rect { x: 0, y: 0, width: 1920, height: 1080 }
```

---

## CoreConfig

Bundles profile, platform, capabilities, and version into one configuration.

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### Predefined Configurations

```rust
use rust_widgets::core::CoreConfig;

// Desktop: Full profile, Desktop platform, desktop capabilities, v1.0.0
let desktop_config = CoreConfig::desktop();

// Embedded: Embedded profile, Embedded platform, embedded capabilities, v1.0.0
let embedded_config = CoreConfig::embedded();

// Mobile: Full profile, Mobile platform, mobile capabilities, v1.0.0
let mobile_config = CoreConfig::mobile();
```

---

## CoreError and CoreResult

### `CoreError` — Core Operation Errors

```rust
pub enum CoreError {
    InvalidArgument(String),
    NotSupported(String),
    NotFound(String),
    Internal(String),
}
```

Implements `Display`, `Error`, and `From<crate::error::RwError>`.

```rust
use rust_widgets::core::CoreError;

let err = CoreError::InvalidArgument("width must be positive".to_string());
println!("{}", err);  // "Invalid argument: width must be positive"

// Conversion from RwError (maps error IDs to CoreError variants):
// INVALID_ARGUMENT → CoreError::InvalidArgument
// UNSUPPORTED_OPERATION / NOT_IMPLEMENTED → CoreError::NotSupported
// FILE_NOT_FOUND → CoreError::NotFound
// All others → CoreError::Internal
```

### `CoreResult<T>`

```rust
pub type CoreResult<T> = Result<T, CoreError>;
```

---

## CoreObject Trait

Implemented by objects that can be addressed by `ObjectId`.

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

This trait is the foundation of the object system — every `Object` in the
library implements `CoreObject`:

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

## Coordinate Conversion Utilities (`core::coords`)

The coordinate system uses a **screen-coordinate** origin (top-left).
Conversion functions bridge to Cartesian (bottom-left) and PDF systems.

### Screen ↔ Cartesian

```rust
use rust_widgets::core::coords;

// Y-axis conversions:
let screen_y = to_screen_y(0.0, 100.0);      // → 100.0
let cart_y = to_cartesian_y(0.0, 100.0);      // → 100.0

// Integer variants:
let screen_y = to_screen_y_i32(0, 100);       // → 100
let cart_y = to_cartesian_y_i32(0, 100);       // → 100

// Point conversions:
let screen_pt = point_to_screen(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }
let cart_pt = point_to_cartesian(Point::new(10, 0), 100);
// → Point { x: 10, y: 100 }

// Floating-point variants:
let (sx, sy) = point_to_screen_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)
let (cx, cy) = point_to_cartesian_f32(10.0, 0.0, 100.0);
// → (10.0, 100.0)

// Rect conversions:
let cart_rect = Rect::new(10, 0, 50, 30);
let screen_rect = rect_to_screen(cart_rect, 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
let back = rect_to_cartesian(screen_rect, 100);
// → Rect { x: 10, y: 0, width: 50, height: 30 }
```

### Screen ↔ PDF

```rust
use rust_widgets::core::coords;

let pdf_y = to_pdf_y(0.0, 100.0);      // → 100.0
let screen_y = from_pdf_y(0.0, 100.0); // → 100.0
```

### Flip Operations

```rust
let flipped = flip_y(0.0, 100.0);                  // → 100.0
let flipped_pt = flip_point_y(Point::new(10, 0), 100);  // → (10, 100)
let flipped_rect = flip_rect_y(Rect::new(10, 0, 50, 30), 100);
// → Rect { x: 10, y: 70, width: 50, height: 30 }
```

### Coordinate Normalization

```rust
let (nx, ny) = normalize_coords(100.0, 50.0, 200.0, 100.0);
// → (0.5, 0.5)

let (px, py) = denormalize_coords(0.5, 0.5, 200.0, 100.0);
// → (100.0, 50.0)
```

### Clamping

```rust
let rect = Rect::new(10, 20, 640, 480);

// Clamp point to rect bounds:
let clamped = clamp_point_to_rect(Point::new(1000, 1000), rect);
// → Point { x: 649, y: 499 }

// Floating-point variant:
let (cx, cy) = clamp_point_to_rect_f32(1000.0, 1000.0, 10.0, 20.0, 640.0, 480.0);
// → (649.0, 499.0)
```

### DPI Scaling

```rust
let px = dpi_to_pixels(100.0, 2.0);      // → 200.0
let dp = pixels_to_dpi(200.0, 2.0);     // → 100.0

let px_i32 = dpi_to_pixels_i32(100, 2.0);    // → 200
let dp_i32 = pixels_to_dpi_i32(200, 2.0);    // → 100
```

### Roundtrip Invariance

All conversion pairs are exact inverses:

```rust
let y = 42.0;
let height = 100.0;
assert_eq!(to_cartesian_y(to_screen_y(y, height), height), y);
assert_eq!(to_screen_y(to_cartesian_y(y, height), height), y);
```

---

## Rectangle Merging (`core::rect_merge`)

### `merge_intersecting_rects`

Merges overlapping rectangles into a minimal covering set using a greedy
algorithm with repeated passes:

```rust
use rust_widgets::core::{Rect, rect_merge::merge_intersecting_rects};

let rects = vec![
    Rect::new(0, 0, 100, 100),      // overlaps with next
    Rect::new(50, 50, 100, 100),    // overlaps with previous
    Rect::new(200, 200, 50, 50),    // disjoint
];

let merged = merge_intersecting_rects(&rects);
assert_eq!(merged.len(), 2);
// merged[0]: Rect { x: 0, y: 0, width: 150, height: 150 }
// merged[1]: Rect { x: 200, y: 200, width: 50, height: 50 }
```

This is used by the dirty-region tracker in `performance::region` and by the
render batch system to minimize draw calls.

### `bounding_rect`

Computes the bounding rectangle of a set:

```rust
use rust_widgets::core::{Rect, rect_merge::bounding_rect};

let rects = vec![
    Rect::new(0, 0, 10, 10),
    Rect::new(100, 100, 50, 50),
];

let bounds = bounding_rect(&rects);
assert_eq!(bounds, Some(Rect::new(0, 0, 150, 150)));

// Empty input:
assert_eq!(bounding_rect(&[]), None);
```

---

## Mutex Extension (`core::MutexExt`)

Provides poison recovery for mutex locks, avoiding panics when a thread
panicked while holding a lock.

```rust
use rust_widgets::core::MutexExt;
use rust_widgets::compat::Mutex;

let mutex = Mutex::new(42i32);

// Instead of:
// let guard = mutex.lock().expect("mutex poisoned");

// Use:
let guard = mutex.lock_guard();
// If poisoned, recovers via into_inner() and continues.
```

### Trait Definition

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

## Summary of Coordinate Conventions

```text
Screen Coordinates (origin top-left):
  (0, 0) -------------> X (increases right)
    |
    |    Widget positioning
    |    Rendering
    |    SVG output
    |
    v Y (increases down)

Cartesian Coordinates (origin bottom-left):
    ^ Y (increases up)
    |
    |    Chart data coordinates
    |
    |
  (0, 0) -------------> X (increases right)

PDF Coordinates (origin bottom-left):
    ^ Y (increases up)
    |
    |    PDF output
    |
    |
  (0, 0) -------------> X (increases right)
```

| System | Origin | Used By | Conversion |
|---|---|---|---|
| **Screen** | Top-left | Widgets, layouts, rendering, SVG | Default |
| **Cartesian** | Bottom-left | Chart data | `to_cartesian_y`, `point_to_cartesian` |
| **PDF** | Bottom-left | PDF output | `to_pdf_y`, `from_pdf_y` |

---

## Next Steps

- **Widget System** — understand how widgets use these core types, the
  `Widget` trait, `BaseWidget`, and the full widget hierarchy
- **Layout System** — see how `Rect`, `Size`, and `Point` are used by
  layout algorithms to position widgets
- **Rendering System** — learn how `Color`, `Font`, and coordinate
  transformations feed into the rendering pipeline
