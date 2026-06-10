# API Reference

This chapter provides a comprehensive, module-by-module reference for the
entire `rust_widgets` public API. Use this as a quick lookup when you need to
find the right type, function, or trait for your task.

The library version documented here is **0.9.6**. Code examples assume
`use rust_widgets::*;` or explicit paths as shown.

---

## Table of Contents

1. [Top-Level Functions](#top-level-functions)
2. [Application Lifecycle (`app`)](#application-lifecycle-app)
3. [Core Primitives (`core`)](#core-primitives-core)
4. [Widget System (`widget`)](#widget-system-widget)
5. [Layout System (`layout`)](#layout-system-layout)
6. [Event System (`event`)](#event-system-event)
7. [Rendering System (`render`)](#rendering-system-render)
8. [Render Engine (`render_engine`)](#render-engine-render_engine)
9. [Style & Theming (`style`, `theme`)](#style--theming-style-theme)
10. [Platform Abstraction (`platform`)](#platform-abstraction-platform)
11. [Error System (`error`)](#error-system-error)
12. [Action Framework (`action`)](#action-framework-action)
13. [Shortcut System (`shortcut`)](#shortcut-system-shortcut)
14. [Data Binding (`data_binding`)](#data-binding-data_binding)
15. [Signal/Slot (`signal`)](#signalslot-signal)
16. [Internationalization (`i18n`)](#internationalization-i18n)
17. [Gesture Recognition (`gesture`)](#gesture-recognition-gesture)
18. [Charts & Data Visualization (`chart`)](#charts--data-visualization-chart)
19. [PDF Generation (`pdf`)](#pdf-generation-pdf)
20. [Printing (`print`)](#printing-print)
21. [Memory Management (`memory`)](#memory-management-memory)
22. [Performance (`performance`)](#performance-performance)
23. [Adaptive Quality (`quality`)](#adaptive-quality-quality)
24. [Control Backend (`control_backend`)](#control-backend-control_backend)
25. [Object System (`object`)](#object-system-object)
26. [Web Capabilities (`web`)](#web-capabilities-web)
27. [Undo/Redo (`undo`)](#undoredo-undo)
28. [Clipboard (`clipboard`)](#clipboard-clipboard)
29. [GPU Acceleration (`gpu`, `wgpu_backend`)](#gpu-acceleration-gpu-wgpu_backend)
30. [Embedded Support (`embedded`)](#embedded-support-embedded)
31. [Language Bindings (`bindings`)](#language-bindings-bindings)
32. [Feature Flags Reference](#feature-flags-reference)
33. [Error Codes Reference](#error-codes-reference)
34. [FFI / C ABI Reference](#ffi--c-abi-reference)

---

## Top-Level Functions

The crate root (`rust_widgets`) exposes a set of convenience functions for rapid
application development without the `App` wrapper. These are ideal for simple
scripts or FFI entry points.

### Lifecycle Functions

| Function | Signature | Description |
|---|---|---|
| `init` | `fn()` | Initialize the runtime (picks platform backend automatically) |
| `run` | `fn()` | Enter the main event loop (blocks until `quit`) |
| `quit` | `fn()` | Signal the event loop to exit |

### Window Creation

```rust
pub fn create_window(title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
```

### Widget Creation

Each function creates a widget, returns its `ObjectId`, and adds it as a child
of `parent`.

| Function | Return `ObjectId` | Notes |
|---|---|---|
| `create_button(parent, text, x, y, w, h)` | Button | Standard push button |
| `create_checkbox(parent, text, x, y, w, h)` | CheckBox | Toggle checkbox |
| `create_line_edit(parent, text, x, y, w, h)` | LineEdit | Single-line text input |
| `create_label(parent, text, x, y, w, h)` | Label | Static text display |
| `create_radio_button(parent, text, x, y, w, h)` | RadioButton | Exclusive selection |
| `create_slider(parent, x, y, w, h)` | Slider | Value slider |
| `create_progress_bar(parent, x, y, w, h)` | ProgressBar | Progress indicator |
| `create_combo_box(parent, x, y, w, h)` | ComboBox | Dropdown selector |
| `create_list_box(parent, x, y, w, h)` | ListBox | List selection |
| `create_panel(parent, x, y, w, h)` | Panel (GroupBox) | Container panel |
| `create_message_box(parent, title, text, x, y, w, h)` | MessageBox | Modal message dialog |
| `create_file_dialog(parent, title, x, y, w, h)` | FileDialog | File picker |
| `create_color_dialog(parent, title, x, y, w, h)` | ColorDialog | Color picker |
| `create_font_dialog(parent, title, x, y, w, h)` | FontDialog | Font picker |
| `create_spin_box(parent, x, y, w, h)` | SpinBox | Numeric spin control |
| `create_list_view(parent, x, y, w, h)` | ListView | Table-style list |
| `create_scroll_area(parent, x, y, w, h)` | ScrollArea | Scrollable container |

### Widget Manipulation

```rust
pub fn show_widget(id: ObjectId);
pub fn hide_widget(id: ObjectId);
pub fn set_widget_geometry(id: ObjectId, x: i32, y: i32, w: u32, h: u32);
pub fn set_widget_text(id: ObjectId, text: &str) -> String;
pub fn get_widget_text(id: ObjectId) -> String;
pub fn set_widget_enabled(id: ObjectId, enabled: bool);
pub fn is_widget_enabled(id: ObjectId) -> bool;
pub fn set_widget_visible(id: ObjectId, visible: bool);
pub fn is_widget_visible(id: ObjectId) -> bool;
```

### Combo Box Operations

```rust
pub fn combo_box_add_item(id: ObjectId, text: &str);
pub fn combo_box_clear_items(id: ObjectId);
pub fn combo_box_set_current_index(id: ObjectId, index: u32);
pub fn combo_box_current_index(id: ObjectId) -> i32;
pub fn combo_box_item_count(id: ObjectId) -> u32;
pub fn combo_box_item_text(id: ObjectId, index: u32) -> String;
```

### List Box Operations

```rust
pub fn list_box_add_item(id: ObjectId, text: &str);
pub fn list_box_remove_item(id: ObjectId, index: u32);
pub fn list_box_clear_items(id: ObjectId);
pub fn list_box_set_current_index(id: ObjectId, index: u32);
pub fn list_box_current_index(id: ObjectId) -> i32;
pub fn list_box_item_count(id: ObjectId) -> u32;
pub fn list_box_item_text(id: ObjectId, index: u32) -> String;
```

### Event Polling (Polling API)

```rust
pub fn poll_widget_triggered() -> Option<ObjectId>;
pub fn poll_widget_trigger_event() -> Option<(ObjectId, u32)>;
pub fn inject_widget_trigger_event(id: ObjectId, kind: u32) -> bool;
```

### Clipboard

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> PlatformClipboard;
```

### Menu/ToolBar

```rust
pub fn create_menu_bar(parent: ObjectId, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn create_menu(parent: ObjectId, text: &str, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn attach_menu_bar_to_window(window: ObjectId, menu_bar: ObjectId) -> bool;
pub fn menu_add_item(parent_menu: ObjectId, text: &str, shortcut: &str) -> ObjectId;
pub fn poll_menu_triggered() -> Option<ObjectId>;
pub fn inject_menu_trigger(menu_item_id: ObjectId) -> bool;
pub fn create_tool_bar(parent: ObjectId, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
pub fn create_status_bar(parent: ObjectId, text: &str, x: i32, y: i32, w: u32, h: u32) -> ObjectId;
```

### Drag & Drop

```rust
pub fn begin_drag(source: ObjectId, mime_type: &str, payload: &[u8]) -> bool;
pub fn poll_drop_event() -> Option<DropEvent>;
pub fn inject_drop_event(source: ObjectId, target: ObjectId, mime: &str, payload: &[u8]) -> bool;
```

### IME & Accessibility

```rust
pub fn set_widget_ime_enabled(id: ObjectId, enabled: bool);
pub fn is_widget_ime_enabled(id: ObjectId) -> bool;
pub fn platform_ime_bridge() -> Option<ImeBridge>;
pub fn set_widget_accessibility_name(id: ObjectId, name: &str);
pub fn get_widget_accessibility_name(id: ObjectId) -> String;
```

---

## Application Lifecycle (`app`)

The `app` module is the **preferred entry point** for production applications.

### Core Types

```rust
pub struct App { /* ... */ }
pub struct AppConfig {
    pub app_name: String,
    pub enable_i18n: bool,
    // ...
}
```

### App Methods

| Method | Signature | Description |
|---|---|---|
| `new` | `fn(config: AppConfig) -> Self` | Create application with config |
| `run` | `fn(self)` | Run the event loop |
| `window` | `fn(&self) -> &WindowHandle` | Get the main window handle |
| `quit` | `fn(&self)` | Quit the application |

### Widget Handle Types

Each handle wraps an `ObjectId` and exposes type-safe operations.

| Handle | Widget Type | Key Operations (besides WidgetHandle) |
|---|---|---|
| `WidgetHandle` | (base trait) | `raw_id()`, `from_raw()`, `show()`, `hide()`, `set_geometry()`, `set_text()`, `text()`, `enable()`, `disable()`, `is_enabled()`, `set_visible()`, `is_visible()`, `on_click()`, `on_value_changed()` |
| `WindowHandle` | Window | `set_title()`, `title()`, `resize()`, `minimize()`, `maximize()`, `restore()`, `close()` |
| `ButtonHandle` | Button | `set_text()`, `text()` — inherits `WidgetHandle` |
| `LabelHandle` | Label | `set_text()`, `text()` |
| `LineEditHandle` | LineEdit | `set_text()`, `text()`, `set_echo_mode()`, `echo_mode()` |
| `CheckBoxHandle` | CheckBox | `set_checked()`, `is_checked()`, `check_state()`, `set_check_state()` |
| `RadioButtonHandle` | RadioButton | `set_checked()`, `is_checked()` |
| `SliderHandle` | Slider | `set_value()`, `value()`, `set_range()`, `range()` |
| `ProgressBarHandle` | ProgressBar | `set_value()`, `value()`, `set_range()`, `range()` |
| `ComboBoxHandle` | ComboBox | `add_item()`, `clear()`, `set_current_index()`, `current_index()`, `item_count()`, `item_text()` |
| `ListBoxHandle` | ListBox | `add_item()`, `remove_item()`, `clear()`, `set_current_index()`, `current_index()`, `item_count()` |
| `SpinBoxHandle` | SpinBox | `set_value()`, `value()`, `set_range()`, `set_step()` |
| `ScrollAreaHandle` | ScrollArea | `set_widget()`, `ensure_visible()`, `scroll_to()` |
| `ScrollBarHandle` | ScrollBar | `set_value()`, `value()`, `set_range()` |
| `PanelHandle` | Panel | `set_layout()` |
| `ListViewHandle` | ListView | `set_model()`, `model()` |
| `TabWidgetHandle` | TabWidget | `add_tab()`, `remove_tab()`, `set_current_index()`, `current_index()` |
| `TextEditHandle` | TextEdit | `set_text()`, `text()`, `append()`, `clear()` |
| `WebViewHandle` | WebView | `load_url()`, `url()`, `reload()`, `go_back()`, `go_forward()` |
| `MessageBoxHandle` | MessageBox | `set_title()`, `set_text()`, `add_button()` |
| `DialogHandle` | Dialog | `open()`, `close()`, `result()` |
| `FrameHandle` | Frame | Generic frame container |
| `GridWidgetHandle` | GridWidget | Grid-specialized operations |

### Supporting Types

```rust
pub enum CheckState { Unchecked, Checked, PartiallyChecked }
pub enum EchoMode { Normal, Password, NoEcho }
pub enum SelectionMode { Single, Multi, Extended, None }

pub trait ListModel {
    fn row_count(&self) -> usize;
    fn text(&self, row: usize, col: usize) -> String;
    fn set_text(&mut self, row: usize, col: usize, text: &str);
}
```

---

## Core Primitives (`core`)

### Geometry Types

```rust
pub type ObjectId = u64;

pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

`Point` constructors: `new(x, y)`, `origin()`, plus `from_f32()`, `from_u32()`,
`from_i64()`, `from_f64()`, `from_usize()`, `from_isize()` and their `_tuple`
variants. Arithmetic: `Add<(i32, i32)>`.

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

`Size` constructors: `new(w, h)`, plus `from_f32()`, `from_i32()`, `from_i64()`,
`from_f64()`, `from_usize()`, `from_isize()` and `_tuple` variants.
Methods: `is_empty()`, `area()`, `aspect_ratio()`.

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

| Method | Signature | Description |
|---|---|---|
| `new` | `(x, y, w, h)` | Create rect |
| `from_position_size` | `(Point, Size)` | Create from position + size |
| `position()` / `size()` | — | Decompose |
| `contains_point(p)` | `-> bool` | Point hit test |
| `intersects(r)` | `-> bool` | Overlap test |
| `contains_rect(r)` | `-> bool` | Full containment |
| `union(r)` | `-> Rect` | Bounding union |
| `intersection(r)` | `-> Rect` | Overlap intersection |
| `expand_to_touch_target(min)` | `-> Rect` | Expand to minimum touch size |
| `center()` | `-> Point` | Rectangle center |
| `right()` / `bottom()` | `-> i32` | Edge coordinates |
| `with_padding(p)` / `with_margin(m)` | `-> Rect` | Inset/outset |
| `shrink(amount)` / `grow(amount)` | `-> Rect` | Uniform inset/outset |
| `clamp_point(p)` | `-> Point` | Clamp point inside |
| `extend_to_include(p)` | `-> Rect` | Expand to include point |
| `area()` | `-> u32` | Width × height |

```rust
pub enum Orientation { Horizontal, Vertical }
```

### Color

```rust
pub struct Color {
    pub r: u8,  // 0-255
    pub g: u8,  // 0-255
    pub b: u8,  // 0-255
    pub a: u8,  // 0-255
}
```

| Method | Description |
|---|---|
| `rgba(r, g, b, a)` | Create from 0-255 values |
| `rgb(r, g, b)` | Opaque color (alpha = 255) |
| `from_rgba_u32(v)` | Packed 0xRRGGBBAA |
| `from_f32(r, g, b, a)` | From 0.0-1.0 floats |
| `parse_hex(s)` | `"#RGB"`, `"#RGBA"`, `"#RRGGBB"`, or `"#RRGGBBAA"` |
| `to_hex_rgb()` / `to_hex_rgba()` | Serialize |
| `with_alpha(a)` | New color with different alpha |
| `blend(other)` | Alpha compositing |
| `luminance()` | Perceived brightness |
| `is_dark()` / `is_light()` | Brightness classification |
| `contrast_color()` | Black or white for readability |
| `invert()` | RGB inversion |

Predefined constants (55+): `BLACK`, `WHITE`, `RED`, `GREEN`, `BLUE`, `YELLOW`,
`CYAN`, `MAGENTA`, `GRAY`, `LIGHT_GRAY`, `DARK_GRAY`, `TRANSPARENT`,
semantic colors (`PRIMARY`, `SECONDARY`, `SUCCESS`, `WARNING`, `ERROR`,
`BACKGROUND`, `FOREGROUND`, `LINK`, `BORDER`, `DIVIDER`, `SELECTION`,
`TOOLTIP`, `INFO`, `NOTIFICATION`, `DISABLED_BACKGROUND`,
`DISABLED_FOREGROUND`), and extended web colors (`ALICE_BLUE`, `BEIGE`,
`CORAL`, `GOLD`, `INDIGO`, `MAROON`, `NAVY`, `OLIVE`, `ORANGE`, `PINK`,
`PURPLE`, `TEAL`, `SKY_BLUE`, `STEEL_BLUE`, etc.).

### Alignment

```rust
pub enum Alignment { Left, Center, Right, Top, Bottom }
pub enum HorizontalAlignment { Left, Center, Right }
pub enum VerticalAlignment { Top, Center, Bottom }
```

Each supports `parse_str()`, `as_str()`, `is_*()` query methods, and conversion
between horizontal/vertical via `from_alignment()`.

### Font

```rust
pub struct Font {
    pub family: String,
    pub size: f32,
    pub weight: u32,    // 100-900 (CSS weight scale)
    pub bold: bool,
    pub italic: bool,
}
```

### Core Enums & Structs

```rust
pub enum RuntimeProfile { Full, Embedded }
pub enum DeviceClass { Desktop, Tablet, Mobile, Embedded, Projector }
pub enum PlatformFamily { Desktop, Embedded, Mobile, Tablet, Projector }

pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}
```

`Version` methods: `new()`, `from_u32()`, `to_u32()`, `parse_str()`,
`is_compatible_with()`, `is_newer_than()`, `is_older_than()`.

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

Factory methods: `desktop()`, `embedded()`, `mobile()`.

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### Core Traits

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

### Coordinate Utilities

The `coords` module provides coordinate system conversions:

```rust
pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32;
pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32;
pub fn to_pdf_y(screen_y: f32, page_height: f32) -> f32;
```

The `rect_merge` module provides:

```rust
pub fn merge_intersecting_rects(rects: &[Rect]) -> Vec<Rect>;
pub fn bounding_rect(rects: &[Rect]) -> Option<Rect>;
```

The `MutexExt` extension trait adds poison recovery:

```rust
pub trait MutexExt<T> {
    fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R;
}
```

---

## Widget System (`widget`)

### Core Traits

```rust
pub trait Widget: EventHandler + Any {
    // Required
    fn base(&self) -> &BaseWidget;
    fn base_mut(&mut self) -> &mut BaseWidget;

    // Identity
    fn id(&self) -> ObjectId;
    fn kind(&self) -> WidgetKind;

    // Geometry (10+ methods)
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    fn position(&self) -> Point;
    fn size(&self) -> Size;
    fn min_size(&self) -> Option<Size>;
    fn max_size(&self) -> Option<Size>;
    fn set_min_size(&mut self, ...);
    fn set_max_size(&mut self, ...);

    // State (10+ methods)
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn has_focus(&self) -> bool;

    // Hierarchy
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn children(&self) -> Vec<ObjectId>;
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);

    // Signals
    fn triggered(&self) -> &Signal<ObjectId>;
    fn value_changed(&self) -> &Signal<String>;

    // Styling
    fn style(&self) -> &WidgetStyle;
    fn style_mut(&mut self) -> &mut WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
    fn css_class(&self) -> &[String];

    // DPI / Layout
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, scale: f32);
    fn layout_scale(&self) -> f32;

    // Tooltip
    fn set_tooltip(&mut self, text: String);
    fn tooltip(&self) -> Option<&str>;
}

pub trait Draw {
    fn draw(&self, ctx: &mut RenderContext);
}
```

`BaseWidget` implements all the shared state and 60+ default methods.

### Widget Kind Enum

`WidgetKind` enumerates every widget type in the system. Selected variants:

- `Button`, `CheckBox`, `RadioButton`, `Label`
- `LineEdit`, `TextArea`, `ComboBox`, `ListBox`, `SpinBox`, `Dropdown`
- `Slider`, `ProgressBar`, `ScrollBar`, `Spinner`, `Meter`, `Arc`, `Roller`
- `ImageView`, `MiniCanvas`, `MiniChart`, `Line`, `LCDNumber`
- `GroupBox`, `ScrollArea`, `Splitter`, `TabWidget`, `StackedWidget`
- `TileView`, `CollapsiblePane`, `DockWidget`, `MdiArea`, `ToolBox`
- `Window`
- `ToggleButton`, `Switch` (new widgets)
- `Calendar`, `DateEdit`, `TimeEdit`, `DateTimeEdit`, `Dial`
- `KeySequenceEdit`, `PieMenu`, `RibbonBar`, `TabBar`
- `Menu`, `MenuBar`, `StatusBar`, `ToolBar`, `ToolButton`
- `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`, `InputDialog`
- `ProgressDialog`, `PopupWindow`
- `ListView`, `TableView`, `TreeView`, `DataGrid`, `VirtualList`, `VirtualTable`
- `WebView`, `WebEngine`
- (60+ new widget types — see below)

### Widget Categories

**Base Widgets** (always available):

| Widget | Type Alias | Module |
|---|---|---|
| `Button` | — | `base_widgets::button` |
| `CheckBox` | — | `base_widgets::checkbox` |
| `Label` | — | `base_widgets::label` |
| `RadioButton` | — | `base_widgets::radiobutton` |
| `ToggleButton` | — | `base_widgets::toggle_button` *(non-mini)* |

Input types:

| Widget | Type Alias | Features |
|---|---|---|
| `LineEdit` | — | EchoMode support |
| `TextArea` | — | Multi-line |
| `ComboBox` | — | Dropdown selection |
| `Dropdown` | — | — |
| `ListBox` | — | SelectionMode |
| `SpinBox` | — | Numeric spin |
| `Keyboard` | — | Virtual keyboard |
| `TextEdit` | — | *(non-mini)* |
| `RichEdit` | — | *(non-mini)* |
| `CommandLink` | — | *(non-mini)* |
| `FontComboBox` | — | *(non-mini)* |

Display types:

| Widget | Description |
|---|---|
| `Arc` | Arc/ring display |
| `ImageView` | Image viewer |
| `Line` | Horizontal/vertical separator |
| `Meter` | Gauge meter |
| `MiniCanvas` | Custom drawing surface |
| `MiniChart` | Inline mini chart |
| `ProgressBar` | Progress bar |
| `Roller` | Roller display |
| `ScrollBar` | Scroll bar |
| `Slider` | Value slider |
| `Spinner` | Activity spinner |
| `LcdNumber` | LCD-style display *(non-mini)* |

Container types:

| Widget | Description | Features |
|---|---|---|
| `GroupBox` (a.k.a. `Panel`) | Grouped container | Always available |
| `ScrollArea` | Scrollable viewport | Always available |
| `TileView` | Tile container | Always available |
| `CollapsiblePane` | Collapsible section | *(non-mini)* |
| `DockWidget` (a.k.a. `DockPanel`) | Dockable panel | *(non-mini)* |
| `MdiArea` | MDI container | *(non-mini)* |
| `Splitter` | Resizable split | *(non-mini)* |
| `StackedWidget` | Stack/pages | *(non-mini)* |
| `TabWidget` | Tab container | *(non-mini)* |
| `ToolBox` | Toolbox container | *(non-mini)* |

Dialog types *(all non-mini)*:

| Widget | Description |
|---|---|
| `MessageBox` | Modal message dialog |
| `FileDialog` (a.k.a. `DirectoryDialog`) | File/directory picker |
| `ColorDialog` | Color picker |
| `FontDialog` | Font picker |
| `InputDialog` | Input prompt dialog |
| `ProgressDialog` | Progress modal |
| `PopupWindow` (a.k.a. `Dialog`) | Popup window |

Menu/Toolbar types *(all non-mini)*:

| Widget | Description |
|---|---|
| `MenuBar` | Top-level menu bar |
| `Menu` (a.k.a. `ContextMenu`) | Dropdown menu |
| `ToolBar` | Toolbar |
| `StatusBar` | Status bar |
| `ToolButton` | Toolbar button |
| `Action` | Action/command |

View types *(all non-mini)*:

| Widget | Description |
|---|---|
| `ListView` | List with model |
| `TableWidget` | Table view |
| `DataGrid` | Filterable data grid |
| `TreeView` (a.k.a. `ColumnView`) | Tree view |
| `VirtualList` (a.k.a. `DataView`) | Virtualized list |
| `VirtualTable` | Virtualized table |
| `TreeTable` | Tree-table hybrid |

### Widget Categories — Reclassified Organization

Widgets from the former `new_widgets` module have been reclassified into dedicated sub-directories:

| Directory | Widgets |
|---|---|
| `nav_widgets/` | `AdaptiveScaffold`, `AppBar`, `BottomNavigationBar`, `NavigationDrawer`, `NavigationStack`, `TabView` |
| `chart_widgets/` | `BarChart`, `LineChart`, `PieChart`, `Sparkline` |
| `media_widgets/` | `AnimatedImage`, `AudioVisualizer`, `CameraPreview`, `HeroAnimation`, `LottieWidget`, `RiveWidget`, `VideoPlayer` |
| `overlay_widgets/` | `FAB`, `PullToRefresh`, `RefreshControl`, `SwipeToDismiss` |
| `cupertino/` | `CupertinoAlertDialog`, `CupertinoDatePicker`, `CupertinoNavigationBar`, `CupertinoSegmentedControl`, `CupertinoSlider`, `CupertinoSwitch`, `MaterialNavigationRail`, `MaterialSnackbar` |
| `misc_widgets/` | `Avatar`, `BarcodeScanner`, `BezierCurveEditor`, `DateRangePicker`, `MobileDatePicker`, `QRCode`, `SegmentedButton` |
| `input_widgets/` (extended) | `AutoCompleteEdit`, `EditableComboBox`, `ImePreedit`, `InplaceEditor`, `MaskedEdit`, `MultiSelectComboBox`, `RangeSlider`, `SearchBar`, `SearchBox`, `ShortcutEditor`, `TagInput` |
| `display_widgets/` (extended) | `Badge`, `ColorHistory`, `ColorWell`, `Divider`, `EmptyState`, `FloatingLabel`, `FontPreview`, `Icon`, `ProgressCircle`, `Rating`, `SkeletonLoader`, `Switch` |
| `container_widgets/` (extended) | `Carousel`, `MasonryLayout`, `PagerPageView`, `SafeArea`, `Stepper` |
| `dialog/` (extended) | `BottomSheet`, `FindReplaceDialog`, `ModalBottomSheet`, `Popover`, `Tooltip`, `WizardDialog` |
| `menu_toolbar/` (extended) | `DropdownMenu`, `MenuButton` |
| `view_widgets/` (extended) | `ImageGallery`, `PropertiesPanel`, `PropertyGrid` |

### Web Widgets *(non-mini)*

| Widget | Description |
|---|---|
| `WebView` | Embedded web browser view |
| `WebEngine` | Web engine for rendering |

Associated types: `WebEngineContextMenuRequest`, `WebEngineCookieStore`,
`WebEngineDownloadItem`, `WebEngineFindTextResult`, `WebEngineNotification`,
`WebEnginePage`, `WebEngineScriptDialog`, `WebEngineSettings`,
`WebEngineWebChannel`.

### Advanced Widgets *(non-mini)*

| Widget | Description |
|---|---|
| `Calendar` | Date calendar picker |
| `DateEdit` (a.k.a. `DatePicker`) | Date editor |
| `TimeEdit` (a.k.a. `TimePicker`) | Time editor |
| `DateTimeEdit` (a.k.a. `DateTimePicker`) | DateTime editor |
| `Dial` | Rotary dial |
| `KeySequenceEdit` | Keyboard shortcut recorder |
| `PieMenu` | Radial/pie menu |
| `RibbonBar` | Ribbon-style toolbar |

### Special Widgets *(non-mini)*

| Widget | Description |
|---|---|
| `Canvas` | Free-form drawing surface |
| `ChartWidget` | Chart display widget |
| `CodeEditor` | Source code editor |
| `ColorPicker` | Color selection widget |
| `CommandEntry` | Command input |
| `CommandPalette` | Command palette overlay |
| `DiffViewer` | Side-by-side diff viewer |
| `GanttWidget` | Gantt chart |
| `GridWidget` | Grid display |
| `MapView` | Map display |
| `MarkdownEditor` | Markdown editing |
| `MediaPlayer` | Media player controls |
| `NotificationCenter` | Notification center |
| `SegmentedControl` | Segmented button group |
| `SplitButton` | Split action button |
| `TerminalView` | Terminal emulator |
| `TimelineWidget` | Timeline display |

### Widget Capability System

```rust
pub trait WidgetFactory {
    fn create(&self, id: ObjectId, parent: Option<ObjectId>, kind: WidgetKind) -> Box<dyn Widget>;
}

pub trait WidgetCapability {
    fn capability(&self) -> &'static str;
    fn value(&self) -> CapabilityValue;
    fn schema(&self) -> PropertySchema;
}

pub enum CapabilityValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(Color),
    Rect(Rect),
    Size(Size),
}

pub struct PropertySchema {
    pub name: &'static str,
    pub kind: PropertyValueKind,
    pub description: &'static str,
}

pub enum PropertyValueKind { Bool, Int, Float, String, Color, Rect, Size, Enum(Vec<&'static str>) }

pub enum CapabilityAccessError { NotFound, WrongType, ReadOnly }
```

### Widget Sub-types

| Type | As |
|---|---|
| `ToggleButtonState` | `struct { checked: bool }` |
| `ButtonState` | `struct { pressed: bool, hovered: bool }` |
| `CheckState` | `enum { Unchecked, Checked, PartiallyChecked }` |
| `EchoMode` | `enum { Normal, Password, NoEcho }` |
| `SelectionMode` | `enum { Single, Multi, Extended, None }` |
| `LineOrientation` | `enum { Horizontal, Vertical }` |
| `RangeSliderOrientation` | `enum { Horizontal, Vertical }` |

---

## Layout System (`layout`)

### Core Trait

```rust
pub trait Layout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    fn remove_widget(&mut self, widget_id: ObjectId);
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn update_from_position_size(&self, position: Point, size: Size, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn child_ids(&self) -> Vec<ObjectId>;
    fn has_child(&self, id: ObjectId) -> bool;
    fn clear(&mut self);
    fn update_with_context(&self, rect: Rect, context: &LayoutContext, widgets: &mut dyn FnMut(ObjectId, Rect));
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

### Layout Types

| Layout | Module | Description |
|---|---|---|
| `BoxLayout` | `box_layout` | Horizontal or vertical box — distributes space proportionally by stretch factor |
| `HBoxLayout` | `box_layout` | `BoxLayout` with `Orientation::Horizontal` (named convenience) |
| `VBoxLayout` | `box_layout` | `BoxLayout` with `Orientation::Vertical` (named convenience) |
| `GridLayout` | `grid` | Row/column grid with `set_widget(row, col, id)` |
| `StackLayout` | `stack` | Only shows one child at a time (card stack) with `set_current_index()` |
| `FlowLayout` | `flow` | Wrapping flow (like CSS flex-wrap) |
| `FlexLayout` | `flex` | CSS flexbox-style layout |
| `FormLayout` | `form` | Label-field pair layout |
| `SplitterLayout` | `splitter` | Resizable split panes |
| `AbsoluteLayout` | `absolute` | Free-form positioning |
| `CenterLayout` | `center` | Centers a single child |
| `AspectRatioLayout` | `aspect_ratio` | Maintains aspect ratio |
| `WrapLayout` | `wrap` | Wrapping horizontal/vertical |
| `UniformGridLayout` | `uniform_grid` | Fixed cells |
| `KeyboardAwareLayout` | `keyboard_aware` | Adjusts for virtual keyboard |

### Support Types

```rust
pub enum SizePolicy { Fixed, Preferred, Expanding }

pub struct LayoutConstraints {
    pub min: u32,
    pub max: Option<u32>,
}

pub struct LayoutContext {
    pub layout_scale: f32,
    pub font_scale: f32,
    pub min_touch_size: Size,
}
```

---

## Event System (`event`)

### Event Types

```rust
pub enum Event {
    // Mouse
    MouseDown((Point, u32)),
    MouseUp((Point, u32)),
    MouseMove { pos: Point },
    MousePress { pos: Point, button: u32 },
    MouseRelease { pos: Point, button: u32 },
    MouseDoubleClick { pos: Point, button: u32 },
    MouseEnter { pos: Point },
    MouseLeave { pos: Point },
    MouseMoveLegacy((Point, u32)),
    Wheel { delta: Point, modifiers: u32 },

    // Keyboard
    KeyDown((u32, u32)),
    KeyUp((u32, u32)),
    KeyPress { key: u32, modifiers: u32 },
    KeyRelease { key: u32, modifiers: u32 },

    // Focus
    FocusGained,
    FocusLost,

    // Window
    Paint,
    Resize { size: Size },
    OrientationChanged { orientation: ScreenOrientation },
    Quit,

    // Timer
    Timer { id: u32 },

    // Custom
    Custom { name: String, payload: Vec<u8> },

    // Touch (feature = "touch")
    TouchBegin { pos: Point, touch_id: TouchId },
    TouchEnd { pos: Point, touch_id: TouchId },
    TouchMove { pos: Point, touch_id: TouchId },
    Tap { pos: Point },
    DoubleTap { pos: Point },
    LongPress { pos: Point },
    Swipe { start: Point, end: Point, velocity: f32 },
    Pinch { scale: f32, center: Point },
    Rotate { angle: f32, center: Point },
}
```

### Core Types

```rust
pub type TouchId = u64;
pub type MouseEvent = (Point, u32);
pub type KeyEvent = (u32, u32);

pub enum EventPriority { Low, Normal, High }
pub enum GestureClass { Single, Multi, Holographic }
pub enum ScreenOrientation { Portrait, Landscape, ReversePortrait, ReverseLandscape }

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event);
}
```

### Event Loop

```rust
pub struct EventLoop {
    // ...
}

impl EventLoop {
    pub fn new() -> Self;
    pub fn run(&mut self) -> !;
    pub fn quit(&self);
    pub fn post_event(&self, event: Event);
    pub fn add_timer(&mut self, interval_ms: u64, id: u32);
    pub fn remove_timer(&mut self, id: u32);
}
```

### Event Queue

```rust
pub struct EventQueue { /* ... */ }
pub struct EventSender { /* ... */ }

impl EventQueue {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, event: Event);
    pub fn pop(&mut self) -> Option<Event>;
    pub fn sender(&self) -> EventSender;
    pub fn clear(&mut self);
}

impl EventSender {
    pub fn send(&self, event: Event) -> Result<(), Event>;
}
```

### Focus Manager

```rust
pub struct FocusManager { /* ... */ }

impl FocusManager {
    pub fn new() -> Self;
    pub fn focused_widget(&self) -> Option<ObjectId>;
    pub fn set_focus(&mut self, id: ObjectId) -> bool;
    pub fn clear_focus(&mut self);
    pub fn next_widget(&mut self) -> Option<ObjectId>;
    pub fn prev_widget(&mut self) -> Option<ObjectId>;
    pub fn register_tab_order(&mut self, ids: &[ObjectId]);
    pub fn set_a11y_callback(&mut self, callback: Box<dyn FnMut(ObjectId)>);
}
```

### Pointer Capture

```rust
pub struct PointerCaptureManager { /* ... */ }

impl PointerCaptureManager {
    pub fn new() -> Self;
    pub fn capture(&mut self, widget_id: ObjectId);
    pub fn release(&mut self);
    pub fn captured_widget(&self) -> Option<ObjectId>;
    pub fn is_captured_by(&self, widget_id: ObjectId) -> bool;
}
```

### Timer

```rust
pub struct TimerManager { /* ... */ }

impl TimerManager {
    pub fn new() -> Self;
    pub fn add_timer(&mut self, interval_ms: u64) -> u32;
    pub fn remove_timer(&mut self, id: u32);
    pub fn process_timers(&mut self, now_ms: u64) -> Vec<u32>;
    pub fn clear(&mut self);
}
```

### Queue Utilities

```rust
pub struct FixedSizeQueue<T> { /* ... */ }
impl<T> FixedSizeQueue<T> {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, item: T) -> Result<(), QueueError>;
    pub fn pop(&mut self) -> Option<T>;
    pub fn is_empty(&self) -> bool;
    pub fn is_full(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
}

pub enum QueueError { Full, Empty }
pub const DEFAULT_QUEUE_CAPACITY: usize = 1024;
```

### Animation Frame Request

```rust
pub struct AnimationFrameRequest { /* ... */ }
impl AnimationFrameRequest {
    pub fn new(callback: Box<dyn FnMut()>) -> Self;
    pub fn request(&mut self);
    pub fn cancel(&mut self);
    pub fn is_pending(&self) -> bool;
}
```

---

## Rendering System (`render`)

### Core Rendering Types

```rust
pub enum BlendMode { Normal, Multiply, Screen, Overlay, Additive }

pub enum RenderCommand {
    Noop,
    Clear(Color),
    DrawRect { rect: Rect, color: Color, width: u32 },
    FillRect { rect: Rect, color: Color },
    DrawRoundedRect { rect: Rect, radius: u32, color: Color, width: u32 },
    FillRoundedRect { rect: Rect, radius: u32, color: Color },
    DrawCircle { cx: i32, cy: i32, radius: u32, color: Color, width: u32 },
    FillCircle { cx: i32, cy: i32, radius: u32, color: Color },
    DrawText { text: String, x: i32, y: i32, color: Color, font_size: f32 },
    DrawLine { x1: i32, y1: i32, x2: i32, y2: i32, color: Color, width: u32 },
    DrawImage { data: Vec<u8>, rect: Rect },
    PushClip { x: i32, y: i32, width: u32, height: u32 },
    PopClip,
    DrawGradient { rect: Rect, gradient: Gradient },
    DrawShadow { rect: Rect, shadow: Shadow },
    DrawPath { commands: Vec<PathCommand>, color: Color, width: u32 },
    FillPath { commands: Vec<PathCommand>, color: Color },
    DrawArc { rect: Rect, start_angle: f32, span_angle: f32, color: Color, width: u32 },
    FillArc { rect: Rect, start_angle: f32, span_angle: f32, color: Color },
}
```

### Paint Backend Trait

```rust
pub trait PaintBackend {
    fn begin_frame(&mut self, clear_color: Color);
    fn end_frame(&mut self);
    fn draw_pixels(&mut self, x: i32, y: i32, width: u32, height: u32, pixels: &[u8]);
    fn size(&self) -> Size;
    fn dpi_scale(&self) -> f32;
}
```

### Render Context

```rust
pub struct RenderContext<'a> {
    // Wraps a &mut dyn PaintBackend
}

impl RenderContext<'_> {
    pub fn new(backend: &mut dyn PaintBackend) -> Self;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self);
    pub fn draw_text(&mut self, text: &str, x: i32, y: i32, font_size: f32, color: Color);
    pub fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color, width: u32);
    pub fn draw_rect(&mut self, rect: Rect, color: Color, width: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: Color);
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color, width: u32);
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color);
    pub fn draw_image(&mut self, data: &[u8], rect: Rect);
    pub fn draw_gradient(&mut self, rect: Rect, gradient: &Gradient);
    pub fn draw_shadow(&mut self, rect: Rect, shadow: &Shadow);
    pub fn push_clip(&mut self, x: i32, y: i32, w: u32, h: u32);
    pub fn pop_clip(&mut self);
    pub fn execute(&mut self, cmd: &RenderCommand);
    pub fn execute_batch(&mut self, cmds: &[RenderCommand]);
}
```

### Back Buffer

```rust
pub struct BackBuffer { /* ... */ }
impl BackBuffer {
    pub fn new(size: Size) -> Self;
    pub fn clear(&mut self, color: Color);
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color>;
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color);
    pub fn pixels(&self) -> &[u8];
    pub fn pixels_mut(&mut self) -> &mut [u8];
    pub fn size(&self) -> Size;
    pub fn present(&self, target: &mut dyn PaintBackend, x: i32, y: i32);
    pub fn resize(&mut self, new_size: Size);
}
```

### Software Surface

```rust
pub struct SoftwareSurface { /* ... */ }
impl SoftwareSurface {
    pub fn new(size: Size) -> Self;
    pub fn context(&mut self) -> RenderContext;
    pub fn present(&mut self) -> &[u8];
    pub fn resize(&mut self, new_size: Size);
    pub fn size(&self) -> Size;
    pub fn set_size(&mut self, size: Size);
}
```

### Scene & Batch

```rust
pub enum SceneLayer { Background, Content, Foreground, Overlay, Tooltip }
pub struct RenderScene { /* ... */ }
pub struct BatchId(u32);
pub struct BatchCommand { /* ... */ }
pub struct BatchRenderer { /* ... */ }
```

### Auto Render Backend

```rust
pub struct AutoRenderBackend(SoftwarePaintBackend);
impl AutoRenderBackend {
    pub fn new(size: Size, dpi: f32) -> Self;
    pub fn as_software(&self) -> &SoftwarePaintBackend;
}

pub fn default_software_render_config() -> SoftwareRenderConfig;
pub fn set_default_software_render_config(config: SoftwareRenderConfig);
pub fn last_auto_render_backend() -> Option<AutoRenderBackend>;
```

### Text Shaping

```rust
pub trait TextShaper {
    fn shape(&self, text: &str, font: &Font, dpi_scale: f32) -> ShapedText;
}

pub struct SimpleTextShaper;
impl TextShaper for SimpleTextShaper { /* ... */ }

pub struct ShapedText { /* ... */ }
pub struct ShapedGlyphRun { /* ... */ }
pub struct TextMetrics { /* ... */ }
pub struct TextCluster { /* ... */ }
```

### Rich Text

```rust
pub struct RichText { /* ... */ }
pub struct TextSpan {
    pub text: String,
    pub style: TextStyle,
}
pub struct TextStyle {
    pub font: Font,
    pub color: Color,
    pub background: Option<Color>,
}
```

### Text Overflow

```rust
pub enum TextOverflow { Clip, Ellipsis }
pub enum TextClamp { None, Lines(u32), Pixels(f32) }
pub fn apply_text_overflow(text: &str, max_width: f32, font: &Font, overflow: TextOverflow) -> String;
pub fn apply_text_clamp(text: &str, max_lines: u32, font: &Font, width: f32, clamp: TextClamp) -> String;
```

### Grapheme Support

```rust
pub struct GraphemeCluster { /* ... */ }
pub struct GraphemeProcessor { /* ... */ }
impl GraphemeProcessor {
    pub fn new() -> Self;
    pub fn grapheme_clusters(&self, text: &str) -> Vec<GraphemeCluster>;
}
```

### SVG Rendering

```rust
pub struct SvgPaintBackend { /* ... */ }
impl SvgPaintBackend {
    pub fn new(size: Size) -> Self;
    pub fn render_to_string(&self) -> String;
    pub fn render_to_bytes(&self) -> Vec<u8>;
}
```

### Quality

```rust
pub fn current_fps() -> f64;
pub fn average_frame_time() -> f64;
pub fn current_quality_level() -> QualityLevel;
pub fn set_quality_level(level: QualityLevel);
```

### GPU Rendering (feature = `gpu-wgpu`)

```rust
pub struct GpuRenderer { /* ... */ }
impl GpuRenderer {
    pub fn new() -> Result<Self, ()>;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self) -> Result<(), ()>;
    pub fn submit(&mut self, commands: &[RenderCommand]);
    pub fn resize(&mut self, size: Size);
}

pub enum GpuCapability { Software, Basic, Medium, High }
```

### Projection (feature = `projection`)

```rust
pub struct PresentationController { /* ... */ }
pub struct ProjectionRenderConfig { /* ... */ }
pub struct ProjectionLayoutHelper { /* ... */ }
```

---

## Render Engine (`render_engine`)

```rust
pub trait EngineTrait {
    fn name(&self) -> &'static str;
    fn init(&mut self) -> Result<(), RwError>;
    fn run(&mut self) -> Result<(), RwError>;
    fn quit(&mut self);
    fn submit_frame(&mut self, surface: &mut SoftwareSurface);
    fn is_running(&self) -> bool;
}
```

### Native Engine

```rust
pub struct NativeEngine { /* ... */ }
impl NativeEngine {
    pub fn new() -> Self;
}
impl EngineTrait for NativeEngine { /* ... */ }
```

### Embedded Engine

```rust
pub struct EmbeddedEngine { /* ... */ }
impl EmbeddedEngine {
    pub fn new() -> Self;
    pub fn init(&mut self) -> bool;
    pub fn task_count(&self) -> u64;
    pub fn submit_noop(&self, label: &str) -> u64;
    pub fn frame_count(&self) -> u64;
    pub fn button_count(&self) -> u64;
    pub fn window_count(&self) -> u64;
    pub fn target_fps(&self) -> u32;
    pub fn set_target_fps(&mut self, fps: u32) -> u32;
    pub fn is_running(&self) -> bool;
    pub fn is_initialized(&self) -> bool;
}
impl EngineTrait for EmbeddedEngine { /* ... */ }
```

---

## Style & Theming (`style`, `theme`)

### Style Types

```rust
pub struct WidgetStyle {
    pub background_color: Option<Color>,
    pub background_gradient: Option<Gradient>,
    pub text_color: Option<Color>,
    pub font: Option<Font>,
    pub border_color: Option<Color>,
    pub border_width: u32,
    pub border_radius: u32,
    pub padding: Padding,
    pub margin: Margin,
    pub shadow: Option<Shadow>,
    pub touch_target: Option<Size>,
    pub opacity: Option<f32>,
}
```

Builder methods: `with_background(c)`, `with_text_color(c)`, `with_font(f)`,
`with_border(c, w, r)`, `with_padding(p)`, `with_margin(m)`, `with_shadow(s)`,
`with_touch_target(s)`, `with_gradient(g)`, `with_opacity(o)`.

Instance methods: `inherit_from(parent)`, `merge(other)`.

### Padding & Margin

```rust
pub struct Padding { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
pub struct Margin  { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
```

Both support: `new(top, right, bottom, left)`, `all(value)`, `symmetric(v, h)`,
`normalized(top, right, bottom, left)` — negative values clamped to 0.

### Shadow

```rust
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub blur: u32,
    pub color: Color,
}
```

Builder: `new()`, `with_offset(x, y)`, `with_blur(b)`, `with_color(c)`.

### Touch Target

```rust
pub enum TouchTargetSize { Desktop, Tablet, Phone, Embedded, Projection }
impl TouchTargetSize {
    pub fn dimensions(self) -> Size;
    pub fn spacing(self) -> u32;
}
```

### Reduced Motion

```rust
pub enum ReducedMotionPreference { NoPreference, ReduceMotion }
```

### CSS & Selector System

```rust
// CSS property resolution
pub struct CssEngine { /* ... */ }
pub struct Selector { /* ... */ }

// Hot-reload CSS watcher
pub struct CssWatcher { /* ... */ }
impl CssWatcher {
    pub fn watch(path: &str) -> Result<Self, ()>;
    pub fn poll_changed(&mut self) -> bool;
}
```

### Gradients

```rust
pub enum Gradient {
    Linear { start: Point, end: Point, colors: Vec<(f32, Color)> },
    Radial { center: Point, radius: f32, colors: Vec<(f32, Color)> },
}
```

### Theme

```rust
pub struct Theme {
    pub name: String,
    pub colors: Colors,
    pub fonts: Fonts,
    pub spacing: Spacing,
    pub borders: Borders,
}

impl Theme {
    pub fn new(name: &str) -> Self;
    pub fn dark() -> Self;                   // Built-in dark theme
    pub fn light() -> Self;                  // Built-in light theme
}
```

### Theme Manager

```rust
pub struct ThemeManager { /* ... */ }

impl ThemeManager {
    pub fn new() -> Self;
    pub fn set_theme(&mut self, theme: Theme);
    pub fn current_theme(&self) -> Option<&Theme>;
    pub fn register_theme(&mut self, name: &str, theme: Theme) -> bool;
    pub fn switch_to(&mut self, name: &str) -> bool;
    pub fn available_themes(&self) -> Vec<&str>;
}
```

### Theme Sub-types

```rust
pub struct Colors {
    pub background: Color,
    pub foreground: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub border: Color,
    pub highlight: Color,
    pub error: Color,
    pub success: Color,
    pub warning: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub disabled: Color,
    // ...
}

pub struct Fonts {
    pub default: Font,
    pub heading: Font,
    pub caption: Font,
    pub monospace: Font,
}

pub struct Spacing {
    pub xs: u32, pub sm: u32, pub md: u32,
    pub lg: u32, pub xl: u32, pub xxl: u32,
}

pub struct Borders {
    pub width: u32,
    pub radius: u32,
}
```

### Theme Style Token

```rust
pub struct ThemeStyleToken { /* ... */ }
```

### Style Inheritance Chain

```
1. Global Theme defaults (ThemeManager → Theme)
2. ThemeOverrides per widget class (e.g. "Button", "Label")
3. Widget instance state (StatefulTheme → WidgetState)
4. Inline style overrides
```

---

## Platform Abstraction (`platform`)

### Runtime Functions

```rust
pub fn init();
pub fn run();
pub fn quit();
pub fn get_platform() -> &'static Box<dyn PlatformBackend>;
pub fn capabilities() -> PlatformCapabilities;
pub fn dpi_scale_factor() -> f32;
pub fn runtime_gui_mode() -> RuntimeGuiMode;
pub fn runtime_gui_mode_for(family: PlatformFamily) -> RuntimeGuiMode;
#[cfg(feature = "mobile-api")]
pub fn mobile_attach_to_native_view(native_handle: u64) -> bool;
#[cfg(feature = "mobile-api")]
pub fn mobile_backend_name() -> String;
```

### Platform Types

```rust
pub enum RuntimeGuiMode { Native, Embedded, Headless }

pub struct CapabilityContract { /* ... */ }
pub fn negotiate_capability_contract(profile: u32) -> CapabilityContract;
```

### Accessibility

```rust
pub trait AccessibilityBridge: Send + Sync {
    fn notify_focus_changed(&self, widget_id: ObjectId);
    fn notify_text_changed(&self, widget_id: ObjectId, text: &str);
    fn notify_selection_changed(&self, widget_id: ObjectId);
    fn notify_value_changed(&self, widget_id: ObjectId, value: &str);
}

pub fn wire_focus_manager_to_a11y(fm: &mut FocusManager);
```

### IME (Input Method Editor)

```rust
pub trait ImeBridge {
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn commit_text(&self, text: &str);
    fn composition_range(&self) -> Option<(u32, u32)>;
    fn composition_text(&self) -> Option<String>;
}
```

### Virtual Keyboard

```rust
pub struct VirtualKeyboardController { /* ... */ }
impl VirtualKeyboardController {
    pub fn show(&mut self);
    pub fn hide(&mut self);
    pub fn is_visible(&self) -> bool;
    pub fn keyboard_height(&self) -> u32;
}
```

### Clipboard (Platform)

```rust
pub struct PlatformClipboard { /* ... */ }
impl PlatformClipboard {
    pub fn set(&mut self, content: ClipboardContent);
    pub fn get(&self) -> Option<ClipboardContent>;
    pub fn clear(&mut self);
}
```

### Drag & Drop

```rust
pub struct DropEvent {
    pub source: ObjectId,
    pub target: ObjectId,
    pub mime_type: String,
    pub payload: Vec<u8>,
}
pub fn begin_drag(source: ObjectId, mime: &str, payload: &[u8]) -> bool;
pub fn inject_drop_event(source: ObjectId, target: ObjectId, mime: &str, payload: &[u8]) -> bool;
```

---

## Error System (`error`)

### Core Types

```rust
pub struct RwError {
    pub id: ErrorId,
    pub message: String,
}

impl RwError {
    pub fn new(id: ErrorId, message: impl Into<String>) -> Self;
    pub fn not_implemented(feature: impl Into<String>) -> Self;
    pub fn msg(message: impl Into<String>) -> Self;
    pub fn from_panic(panic_info: &dyn Any) -> Self;
}

impl fmt::Display for RwError;
impl std::error::Error for RwError;

pub type RwResult<T> = Result<T, RwError>;
```

### Error ID (stable for FFI)

```rust
pub struct ErrorId(pub i32);

// General
pub const SUCCESS: ErrorId = ErrorId(0);
pub const NOT_IMPLEMENTED: ErrorId = ErrorId(1);
pub const UNSUPPORTED_OPERATION: ErrorId = ErrorId(2);
pub const INVALID_ARGUMENT: ErrorId = ErrorId(3);
pub const NULL_POINTER: ErrorId = ErrorId(4);      // Reserved
pub const OUT_OF_MEMORY: ErrorId = ErrorId(5);      // Reserved
pub const LOCK_POISONED: ErrorId = ErrorId(6);      // Reserved

// Widget (100-199)
pub const WIDGET_BASE_NOT_IMPL: ErrorId = ErrorId(100);   // Reserved
pub const WIDGET_NOT_FOUND: ErrorId = ErrorId(101);        // Reserved
pub const WIDGET_INVALID_STATE: ErrorId = ErrorId(102);    // Reserved
pub const WIDGET_DEPRECATED: ErrorId = ErrorId(103);       // Reserved

// Platform (200-299)
pub const PLATFORM_UNSUPPORTED: ErrorId = ErrorId(200);    // Reserved
pub const PLATFORM_INIT_FAILED: ErrorId = ErrorId(201);    // Reserved
pub const CLIPBOARD_FAILED: ErrorId = ErrorId(202);         // Reserved
pub const DRAG_DROP_FAILED: ErrorId = ErrorId(203);         // Reserved

// Render (300-399)
pub const RENDER_CONTEXT_INVALID: ErrorId = ErrorId(300);   // Reserved
pub const RENDER_PIPELINE_FAILED: ErrorId = ErrorId(301);   // Reserved

// I/O (400-499)
pub const I18N_LOAD_FAILED: ErrorId = ErrorId(400);         // Reserved
pub const FILE_NOT_FOUND: ErrorId = ErrorId(401);
```

### Panic Safety

```rust
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where F: FnOnce() -> T + std::panic::UnwindSafe;

pub fn to_error_id(result: RwResult<()>) -> i32;

pub trait CAbiSafe { /* ... */ }
pub fn c_try_fallback(/* ... */);
```

---

## Action Framework (`action`)

```rust
pub struct ActionManager { /* ... */ }

impl ActionManager {
    pub fn new() -> Self;
    pub fn register_action(&mut self, id: &str, text: &str) -> bool;
    pub fn unregister_action(&mut self, id: &str) -> bool;
    pub fn action(&self, id: &str) -> Option<&Action>;
    pub fn action_mut(&mut self, id: &str) -> Option<&mut Action>;
    pub fn trigger_action(&mut self, id: &str) -> bool;
    pub fn set_action_enabled(&mut self, id: &str, enabled: bool) -> bool;
    pub fn bind_shortcut(&mut self, shortcut: &str, action_id: &str) -> bool;
    pub fn trigger_shortcut(&mut self, shortcut: &str) -> bool;
    pub fn bind_action_to_button(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bind_action_to_menu(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bind_action_to_toolbar(&mut self, action_id: &str, host: ObjectId) -> bool;
    pub fn bindings_for_host(&self, host: ObjectId) -> Vec<ActionBinding>;
}
```

### Action

```rust
pub struct Action { /* ... */ }

impl Action {
    pub fn new(id: &str, text: &str) -> Self;
    pub fn id(&self) -> &str;
    pub fn text(&self) -> &str;
    pub fn set_text(&mut self, text: &str);
    pub fn set_checkable(&mut self, checkable: bool);
    pub fn is_checkable(&self) -> bool;
    pub fn is_checked(&self) -> bool;
    pub fn set_checked(&mut self, checked: bool);
    pub fn is_enabled(&self) -> bool;
    pub fn set_enabled(&mut self, enabled: bool);
    pub fn shortcut(&self) -> Option<&str>;
    pub fn set_shortcut(&mut self, shortcut: &str);
    pub fn connect_triggered(&self, f: impl FnMut() + 'static) -> ConnectionHandle;
    pub fn connect_toggled(&self, f: impl FnMut(bool) + 'static) -> ConnectionHandle;
    pub fn connect_enabled_changed(&self, f: impl FnMut(bool) + 'static) -> ConnectionHandle;
}
```

### Supporting Types

```rust
pub struct ActionBinding {
    pub action_id: String,
    pub host: ObjectId,
    pub kind: ActionHostKind,
}

pub enum ActionHostKind { Button, Menu, ToolBar, Shortcut }

pub struct ActionRouter { /* ... */ }
impl ActionRouter {
    pub fn new() -> Self;
    pub fn route(&self, action_id: &str, kind: ActionHostKind) -> Option<ObjectId>;
}
```

---

## Shortcut System (`shortcut`)

```rust
pub struct ShortcutManager { /* ... */ }

impl ShortcutManager {
    pub fn new() -> Self;
    pub fn register(&mut self, entry: ShortcutEntry) -> bool;
    pub fn unregister(&mut self, id: &str) -> bool;
    pub fn trigger(&self, shortcut: &Shortcut) -> bool;
    pub fn detect_conflicts(&self, entry: &ShortcutEntry) -> Vec<&ShortcutEntry>;
    pub fn all_shortcuts(&self) -> Vec<&ShortcutEntry>;
    pub fn clear(&mut self);
}
```

### Shortcut Types

```rust
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

pub struct ShortcutEntry {
    pub id: String,
    pub shortcut: Shortcut,
    pub description: String,
}

pub enum Key {
    A, B, C, ... Z,       // Alphabet
    F1, F2, ... F24,       // Function keys
    Enter, Escape, Space,
    Tab, Backspace, Delete,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    Insert, Menu, // ...
}

pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,    // Windows key / Cmd key
}
```

---

## Data Binding (`data_binding`)

### Binding (Single Value)

```rust
pub struct Binding<T: Clone + PartialEq + 'static> { /* ... */ }

impl<T: Clone + PartialEq + 'static> Binding<T> {
    pub fn new(value: T) -> Self;
    pub fn get(&self) -> T;
    pub fn set(&mut self, value: T);
    pub fn map<R: Clone + PartialEq + 'static>(&self, f: impl Fn(&T) -> R) -> Computed<R>;
    pub fn subscribe(&mut self, key: &str, listener: Box<dyn BoxedListener<T>>);
    pub fn unsubscribe(&mut self, key: &str);
    pub fn bind_to(&mut self, target: &mut dyn BindingListener<T>);
    pub fn has_subscribers(&self) -> bool;
}
```

### ObservableList

```rust
pub struct ObservableList<T: Clone + 'static> { /* ... */ }

impl<T: Clone + 'static> ObservableList<T> {
    pub fn new() -> Self;
    pub fn push(&mut self, value: T);
    pub fn pop(&mut self) -> Option<T>;
    pub fn insert(&mut self, index: usize, value: T);
    pub fn remove(&mut self, index: usize) -> T;
    pub fn clear(&mut self);
    pub fn get(&self, index: usize) -> Option<&T>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn subscribe(&mut self, listener: Box<dyn FnMut(ListChange)>);
}
```

### Computed (Derived Value)

```rust
pub struct Computed<T: Clone + 'static> { /* ... */ }

impl<T: Clone + 'static> Computed<T> {
    pub fn new(compute: Box<dyn Fn() -> T>) -> Self;
    pub fn get(&self) -> T;
    pub fn invalidate(&mut self);
    pub fn subscribe(&mut self, listener: Box<dyn FnMut()>);
}
```

### Listener Traits

```rust
pub trait BindingListener<T> {
    fn on_value_changed(&mut self, value: &T);
}

pub trait BoxedListener<T>: 'static {
    fn call(&mut self, key: &str, value: &T);
}

pub struct FnListener<T: 'static> { /* ... */ }
impl<T: 'static> FnListener<T> {
    pub fn new(f: impl FnMut(&str) + 'static) -> Self;
}
impl<T: 'static> BoxedListener<T> for FnListener<T> { /* ... */ }
```

### Macros

```rust
// Create a Binding<T> with inferred type
binding!(value);

// Create a Computed<T> with a closure
computed!(|| expression);
```

---

## Signal/Slot (`signal`)

### Typed Signal

```rust
pub struct Signal<T: 'static> { /* ... */ }

impl<T: 'static> Signal<T> {
    pub fn new() -> Self;
    pub fn emit(&self, value: T);
    pub fn connect<F>(&self, f: F) -> ConnectionHandle
        where F: FnMut(T) + 'static;
    pub fn connect_once<F>(&self, f: F) -> ConnectionHandle
        where F: FnOnce(T) + 'static;
    pub fn disconnect(&self, handle: ConnectionHandle);
    pub fn slot_count(&self) -> usize;
}
```

### GenericSignal (no-argument)

```rust
pub struct GenericSignal { /* ... */ }

impl GenericSignal {
    pub fn new() -> Self;
    pub fn emit(&self);
    pub fn connect<F>(&self, f: F) -> ConnectionHandle
        where F: FnMut() + 'static;
    pub fn connect_once<F>(&self, f: F) -> ConnectionHandle
        where F: FnOnce() + 'static;
    pub fn connect_scoped<F>(&self, owner: &ConnectionScope, f: F) -> ConnectionHandle
        where F: FnMut() + 'static;
    pub fn disconnect(&self, handle: ConnectionHandle);
    pub fn slot_count(&self) -> usize;
}
```

### Signal1 (single-argument generic)

```rust
pub struct Signal1<A: 'static> { /* ... */ }
// Same API as GenericSignal but with one argument
```

### Connection Management

```rust
pub struct ConnectionHandle(usize);

pub struct ConnectionScope { /* ... */ }
impl ConnectionScope {
    pub fn new() -> Self;
    // Connections auto-disconnect when scope is dropped
}

pub struct CustomSignalHub { /* ... */ }
impl CustomSignalHub {
    pub fn new() -> Self;
    pub fn signal(&self, name: &str) -> &GenericSignal;
    pub fn emit(&self, name: &str);
    pub fn connect(&self, name: &str, f: impl FnMut() + 'static) -> ConnectionHandle;
}
```

---

## Internationalization (`i18n`)

### Core API

```rust
pub struct I18nManager { /* ... */ }

impl I18nManager {
    pub fn new() -> Self;
    pub fn load_translations(&mut self, path: &str) -> Result<(), RwError>;
    pub fn set_language(&mut self, lang: &str);
    pub fn language(&self) -> &str;
    pub fn translate(&self, key: &str) -> String;
    pub fn translate_with_context(&self, key: &str, context: &[(&str, &str)]) -> String;
    pub fn available_languages(&self) -> Vec<String>;
    pub fn reload(&mut self) -> Result<(), RwError>;
}
```

### Global Functions

```rust
pub fn init(locales_dir: &str) -> Result<(), RwError>;
pub fn init_with_options(options: InitOptions) -> Result<InitReport, RwError>;
pub fn translate(key: &str) -> String;
pub fn translate_with_context(key: &str, context: &[(&str, &str)]) -> String;
pub fn get_manager() -> &'static I18nManager;
pub fn check_and_reload_all() -> Result<(), RwError>;

pub use crate::tr;  // Macro: tr!("hello") → translated string
```

### Options & Types

```rust
pub struct InitOptions {
    pub fallback_language: String,
    pub auto_load: bool,
    pub hot_reload: bool,
    // ...
}

pub struct InitReport {
    pub loaded_files: Vec<String>,
    pub errors: Vec<String>,
}

pub struct Translation { /* ... */ }
pub struct TranslationFile { /* ... */ }
pub enum ReloadEvent { FileChanged(String), Reloaded, Failed(String) }
```

### Hot Reload

```rust
pub fn init_with_hot_reload(locales_dir: &str) -> Result<I18nFileWatcher, RwError>;
pub fn process_reload_events(watcher: &mut I18nFileWatcher) -> Vec<ReloadEvent>;

pub struct I18nFileWatcher { /* ... */ }
```

---

## Gesture Recognition (`gesture`)

### Core Trait

```rust
pub trait GestureRecognizer: Debug + Send {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    fn reset(&mut self);
}
```

### Gesture Engine

```rust
pub struct GestureEngine { /* ... */ }

impl GestureEngine {
    pub fn new() -> Self;       // Pre-populated with all standard recognizers
    pub fn with_recognizers(recognizers: Vec<Box<dyn GestureRecognizer>>) -> Self;
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    pub fn reset_all(&mut self);
    pub fn last_timestamp(&self) -> u64;
}
```

### Recognizers

| Recognizer | Event Produced | Description |
|---|---|---|
| `TapGesture` | `Event::Tap` | Quick touch-and-release (<300ms, <15px movement) |
| `DoubleTapGesture` | `Event::DoubleTap` | Two taps within 400ms |
| `LongPressGesture` | `Event::LongPress` | Hold ≥500ms |
| `SwipeGesture` | `Event::Swipe` | Rapid motion (≥0.5 px/ms, ≥30px) |
| `PanGesture` | `Event::MouseMove` | Continuous drag tracking |
| `LongPressDragGesture` | `Event::Swipe` | Long press then drag |
| `FlingGesture` | `Event::Swipe` | Velocity-based fling/flick |
| `TwoFingerTapGesture` | 'Custom' | Two-finger tap |
| `TwoFingerSwipeGesture` | 'Custom' | Two-finger swipe |
| `PinchGesture` | `Event::Pinch` | Two-finger distance change (scale) |
| `RotateGesture` | `Event::Rotate` | Two-finger angle change |

---

## Charts & Data Visualization (`chart`)

### Chart Types

The `chart` module provides the foundation for data visualization:

```rust
pub struct ChartLayout { /* ... */ }
pub struct ChartSvgRenderer { /* ... */ }

// Sub-modules: charts, layout, svg, types

pub use crate::chart::charts::*;
pub use crate::chart::svg::*;
pub use crate::chart::types::*;
```

Chart data types include axis configuration, series definitions, legends,
and data point structures for line, bar, pie, and scatter charts.

---

## PDF Generation (`pdf`)

### Core Traits

```rust
pub trait PdfPage {
    fn size(&self) -> Size;
    fn set_size(&mut self, size: Size);
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str);
    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool);
    fn add_button(&mut self, name: &str, rect: Rect, text: &str);
    fn content(&self) -> Vec<u8>;
    fn form_fields(&self) -> Vec<PdfFormField>;
}

pub trait PdfDocument {
    fn page_count(&self) -> u32;
    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage>;
    fn add_page(&mut self, size: Size) -> u32;
    fn insert_page(&mut self, index: u32, size: Size) -> u32;
    fn remove_page(&mut self, index: u32) -> bool;
    fn reorder_pages(&mut self, new_order: &[u32]) -> bool;
    fn metadata(&self) -> &PdfMetadata;
    fn set_metadata(&mut self, metadata: PdfMetadata);
    fn security(&self) -> &PdfSecurity;
    fn set_security(&mut self, security: PdfSecurity);
    fn set_page_numbering_enabled(&mut self, enabled: bool);
    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32);
    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32);
    fn save(&self, path: &str) -> Result<(), std::io::Error>;
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}
```

### Module Sub-types

```rust
pub mod annotation;      // PDF annotations
pub mod document;        // Document creation/manipulation
pub mod export;          // PDF export
pub mod form;            // Interactive form fields
pub mod hyperlink;       // Hyperlinks
pub mod metadata;        // PDF metadata (author, title, etc.)
pub mod page;            // Page management
pub mod reader;          // PDF reading/parsing
pub mod security;        // Encryption, passwords, permissions
pub mod types;           // Shared PDF types
pub mod writer;          // PDF writing/serialization
```

---

## Printing (`print`)

```rust
// Provides print dialog integration and document layout support
pub mod print_impl;
pub use print_impl::*;
```

---

## Memory Management (`memory`)

### Pool Allocator

```rust
pub struct PoolAllocator { /* ... */ }
impl PoolAllocator {
    pub fn new() -> Self;
    pub fn allocate(&mut self, size: usize) -> Option<*mut u8>;
    pub fn deallocate(&mut self, ptr: *mut u8, size: usize);
    pub fn clear(&mut self);
}
```

### Arena Allocator

```rust
pub struct ArenaAllocator { /* ... */ }
impl ArenaAllocator {
    pub fn new(capacity: usize) -> Self;
    pub fn allocate<T>(&mut self) -> Option<NonNull<T>>;
    pub fn reset(&mut self);
    pub fn capacity(&self) -> usize;
    pub fn used(&self) -> usize;
    pub fn available(&self) -> usize;
}
```

### Stack Allocator

```rust
pub struct StackAllocator { /* ... */ }
impl StackAllocator {
    pub fn new(capacity: usize) -> Self;
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8>;
    pub fn push_marker(&mut self);
    pub fn pop_to_marker(&mut self);
    pub fn clear(&mut self);
    pub fn capacity(&self) -> usize;
    pub fn used(&self) -> usize;
    pub fn available(&self) -> usize;
}
```

### Memory Monitoring

```rust
pub struct MemoryStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub pool_hits: usize,
    pub pool_misses: usize,
}

pub enum MemoryPressure { None, Low, Medium, High, Critical }

pub trait MemoryPressureHandler: Send + Sync {
    fn on_pressure(&mut self, pressure: MemoryPressure);
}

pub struct MemoryMonitor { /* ... */ }
impl MemoryMonitor {
    pub fn new(warning_threshold: usize, critical_threshold: usize) -> Self;
    pub fn stats(&self) -> &MemoryStats;
    pub fn pressure(&self) -> MemoryPressure;
    pub fn register_handler(&mut self, handler: Box<dyn MemoryPressureHandler>);
    pub fn update(&mut self, current_usage: usize);
    pub fn record_allocation(&mut self, size: usize);
    pub fn record_deallocation(&mut self, size: usize);
}
```

---

## Performance (`performance`)

### Dirty Region Tracking

```rust
pub struct DirtyRegionTracker { /* ... */ }
impl DirtyRegionTracker {
    pub fn new() -> Self;
    pub fn add(&mut self, rect: Rect);
    pub fn merge(&mut self);
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
    pub fn regions(&self) -> &[DirtyRegion];
    pub fn get_bounding_rect(&self) -> Option<Rect>;
}

pub fn render_dirty_regions(
    tracker: &mut DirtyRegionTracker,
    ctx: &mut RenderContext,
    render_all: impl FnMut(&mut RenderContext),
);
```

### Update Batching

```rust
pub struct UpdateBatcher { /* ... */ }
impl UpdateBatcher {
    pub fn new(coalesce_ms: u64) -> Self;
    pub fn add(&mut self, rect: Rect);
    pub fn flush(&mut self) -> Vec<Rect>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

### Profiling

```rust
pub struct Profiler { /* ... */ }
impl Profiler {
    pub fn new() -> Self;
    pub fn begin_frame(&mut self);
    pub fn end_frame(&mut self);
    pub fn record(&mut self, label: &str, duration_ns: u64);
    pub fn frame_count(&self) -> u64;
    pub fn average_frame_time_ns(&self) -> u64;
    pub fn timing(&self, label: &str) -> Option<&TimingSample>;
}
```

---

## Adaptive Quality (`quality`)

### Quality Manager

```rust
pub struct QualityManager { /* ... */ }

impl QualityManager {
    pub fn new() -> Self;
    pub fn with_config(config: QualityConfig) -> Self;
    pub fn with_config_and_capability(config: QualityConfig, gpu: GpuCapability) -> Self;
    pub fn quality_level(&self) -> QualityLevel;
    pub fn set_quality_level(&mut self, level: QualityLevel);
    pub fn finish_frame_secs(&mut self, frame_time_secs: f32);
    pub fn reset(&mut self);
}
```

### Quality Level

```rust
pub enum QualityLevel { Low, Medium, High }

impl QualityLevel {
    pub fn lower(&self) -> Option<Self>;
    pub fn higher(&self) -> Option<Self>;
    pub fn clamp(self, min: Self, max: Self) -> Self;
}
```

### Configuration

```rust
pub struct QualityConfig {
    pub target_frame_rate: f32,
    pub degrade_threshold: f32,       // frame time multiplier to trigger degrade
    pub upgrade_threshold: f32,       // frame time multiplier to trigger upgrade
    pub degrade_frame_count: u32,     // consecutive slow frames to degrade
    pub upgrade_frame_count: u32,     // consecutive fast frames to upgrade
    pub max_quality: QualityLevel,
    pub min_quality: QualityLevel,
}

impl QualityConfig {
    pub fn normalized(self) -> Self;
}
```

### Frame Time Monitor

```rust
pub struct FrameTimeMonitor { /* ... */ }
impl FrameTimeMonitor {
    pub fn new(target_fps: f32) -> Self;
    pub fn record_frame(&mut self, frame_time_secs: f32);
    pub fn average_frame_time(&self) -> f32;
    pub fn should_degrade(&self, threshold: f32, consecutive_count: u32) -> bool;
    pub fn should_upgrade(&self, threshold: f32, consecutive_count: u32) -> bool;
}
```

### GPU Capability

```rust
pub struct GpuCapability {
    pub supports_high_quality: bool,
    pub is_integrated: bool,
    pub performance_tier: u32,
}

impl GpuCapability {
    pub fn recommended_initial_quality(&self) -> QualityLevel;
}
```

---

## Control Backend (`control_backend`)

### Core Types

```rust
pub enum ControlBackendKind { Native, Custom }
pub enum ControlRoutePreference { NativePreferred, NativeRequired, CustomPreferred, CustomRequired }
```

### Backend Trait

```rust
pub trait ControlBackend {
    fn backend_name(&self) -> &'static str;
    fn draw_button(&self, ctx: &mut RenderContext, rect: Rect, state: &ButtonState);
    fn draw_checkbox(&self, ctx: &mut RenderContext, rect: Rect, state: &CheckState);
    fn draw_slider(&self, ctx: &mut RenderContext, rect: Rect, value: f32);
    // ... one draw method per control type
}
```

### Dispatch

```rust
pub fn get_control_backend() -> Box<dyn ControlBackend>;
pub fn get_control_backend_for_widget(kind: WidgetKind) -> Box<dyn ControlBackend>;
pub fn active_control_policy() -> &'static str;
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference;
```

### Backend Implementations

```rust
pub struct NativeControlBackend { /* ... */ }
impl NativeControlBackend {
    pub fn new() -> Self;
}
impl ControlBackend for NativeControlBackend { /* ... */ }

#[cfg(feature = "controls-custom")]
pub struct CustomPaintControlBackend { /* ... */ }
```

---

## Object System (`object`)

```rust
pub struct Object { /* ... */ }

impl Object {
    pub fn new(class_name: &str) -> Self;
    pub fn id(&self) -> ObjectId;
    pub fn class_name(&self) -> &str;
    pub fn set_property(&mut self, key: &str, value: PropertyValue);
    pub fn property(&self, key: &str) -> Option<&PropertyValue>;
    pub fn has_property(&self, key: &str) -> bool;
    pub fn property_keys(&self) -> Vec<&str>;
    pub fn dynamic_properties(&self) -> std::collections::HashMap<String, PropertyValue>;
}

pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(Color),
    Rect(Rect),
    Size(Size),
}
```

---

## Web Capabilities (`web`)

### Browser History

```rust
pub struct BrowserHistory { /* ... */ }
impl BrowserHistory {
    pub fn new() -> Self;
    pub fn add_entry(&mut self, url: String, title: String);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn entries(&self) -> &[HistoryEntry];
    pub fn clear(&mut self);
}

pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visit: u64,
}
```

### Session History

```rust
pub struct SessionHistory { /* ... */ }
impl SessionHistory {
    pub fn new(max_entries: usize) -> Self;
    pub fn navigate(&mut self, url: String);
    pub fn back(&mut self) -> bool;
    pub fn forward(&mut self) -> bool;
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
    pub fn current(&self) -> Option<&str>;
    pub fn clear(&mut self);
}
```

### JS Engine

```rust
pub enum JsValue { Null, Bool(bool), Number(f64), String(String), Object(HashMap<String, JsValue>), Array(Vec<JsValue>) }

pub struct JsContext { /* ... */ }
impl JsContext {
    pub fn new() -> Self;
    pub fn set(&mut self, name: &str, value: JsValue);
    pub fn get(&self, name: &str) -> Option<&JsValue>;
}

pub trait JsEngine {
    fn evaluate(&self, script: &str, context: &mut JsContext) -> Result<JsValue, String>;
}

pub struct SimpleJsEngine;
impl JsEngine for SimpleJsEngine { /* ... */ }
```

### Navigation

```rust
pub struct NavigationEntry {
    pub url: String,
    pub title: String,
}

pub struct NavigationHistory { /* ... */ }
impl NavigationHistory {
    pub fn new(max_size: usize) -> Self;
    pub fn push(&mut self, entry: NavigationEntry);
    pub fn back(&mut self) -> Option<&NavigationEntry>;
    pub fn forward(&mut self) -> Option<&NavigationEntry>;
    pub fn is_empty(&self) -> bool;
}

pub struct WebSettings {
    pub javascript_enabled: bool,
    pub cookies_enabled: bool,
    pub local_storage_enabled: bool,
    pub allow_geolocation: bool,
    pub allow_notifications: bool,
    pub allow_popups: bool,
    pub user_agent: String,
}

pub struct SecuritySettings {
    pub block_popups: bool,
    pub block_mixed_content: bool,
    pub enable_web_security: bool,
    pub certificate_check: bool,
}
```

### Plugins

```rust
pub trait WebPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&mut self) -> Result<(), String>;
    fn on_unload(&mut self) -> Result<(), String>;
}

pub struct PluginManager { /* ... */ }
impl PluginManager {
    pub fn new() -> Self;
    pub fn register(&mut self, plugin: Box<dyn WebPlugin>) -> Result<u32, String>;
    pub fn unregister(&mut self, id: u32) -> bool;
    pub fn list(&self) -> Vec<&dyn WebPlugin>;
    pub fn clear(&mut self);
}

pub enum PluginError { /* ... */ }
pub struct ContentPlugin { /* ... */ }
```

### Privacy

```rust
pub struct PrivacySettings { /* ... */ }
impl PrivacySettings {
    pub fn new() -> Self;
    pub fn strict() -> Self;
    pub fn lenient() -> Self;
}

pub struct CookieJar { /* ... */ }
impl CookieJar {
    pub fn new() -> Self;
    pub fn add(&mut self, cookie: Cookie);
    pub fn remove(&mut self, domain: &str, name: &str);
    pub fn get_for_url(&self, url: &str) -> Vec<&Cookie>;
    pub fn len(&self) -> usize;
    pub fn clear(&mut self);
}

pub enum TrackingType { Advertising, Analytics, Social, Fingerprinting, Malware }
pub struct TrackingProtection { /* ... */ }
pub struct BrowsingData { /* ... */ }
pub enum LoadStatus { NotStarted, Loading, Loaded, Failed }
```

### Web Engine & Web View (non-mini)

```rust
pub struct WebEngineViewEnhanced { /* ... */ }
impl WebEngineViewEnhanced {
    pub fn new(rect: Rect) -> Self;
    pub fn url(&self) -> &str;
    pub fn set_url(&mut self, url: &str);
    pub fn reload(&mut self);
    pub fn stop(&mut self);
    pub fn go_back(&mut self);
    pub fn go_forward(&mut self);
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
}

pub struct WebViewEnhanced { /* ... */ }
impl WebViewEnhanced {
    pub fn new(rect: Rect) -> Self;
    pub fn url(&self) -> &str;
    pub fn load_url(&mut self, url: &str);
    pub fn load_html(&mut self, html: &str, base_url: &str);
    pub fn reload(&mut self);
    pub fn stop(&mut self);
    pub fn go_back(&mut self);
    pub fn go_forward(&mut self);
}

pub struct WebViewCore { /* ... */ }
```

### Web Widget Types *(non-mini)*

```rust
pub struct WebEngine { /* ... */ }
pub struct WebView { /* ... */ }

// Associated types used by web widgets:
pub struct WebEngineContextMenuRequest { /* ... */ }
pub struct WebEngineCookieStore { /* ... */ }
pub struct WebEngineDownloadItem { /* ... */ }
pub struct WebEngineFindTextResult { /* ... */ }
pub struct WebEngineNotification { /* ... */ }
pub struct WebEnginePage { /* ... */ }
pub struct WebEngineScriptDialog { /* ... */ }
pub struct WebEngineSettings { /* ... */ }
pub struct WebEngineWebChannel { /* ... */ }
```

---

## Undo/Redo (`undo`)

```rust
pub trait UndoCommand: Send + Sync {
    fn id(&self) -> &str;
    fn execute(&mut self) -> Result<(), RwError>;
    fn undo(&mut self) -> Result<(), RwError>;
    fn merge_with(&mut self, other: &dyn UndoCommand) -> bool;
}

pub struct UndoStack { /* ... */ }
impl UndoStack {
    pub fn new(max_undo: usize) -> Self;
    pub fn push(&mut self, command: Box<dyn UndoCommand>);
    pub fn undo(&mut self) -> bool;
    pub fn redo(&mut self) -> bool;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn clear(&mut self);
    pub fn undo_text(&self) -> Option<&str>;
    pub fn redo_text(&self) -> Option<&str>;
    pub fn is_clean(&self) -> bool;
    pub fn set_clean(&mut self);
}
```

---

## Clipboard (`clipboard`)

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> PlatformClipboard;
```

---

## GPU Acceleration (`gpu`, `wgpu_backend`)

The GPU module provides hardware-accelerated rendering via `wgpu`:

```rust
#[cfg(feature = "gpu-wgpu")]
pub mod gpu;

#[cfg(feature = "gpu-wgpu")]
pub use gpu::{GpuCapability, GpuRenderer};

// gpu module contents:
pub enum GpuCapability { Software, Basic, Medium, High }
pub struct GpuRenderer { /* ... */ }
impl GpuRenderer {
    pub fn new() -> Result<Self, ()>;
    pub fn begin_frame(&mut self, clear_color: Color);
    pub fn end_frame(&mut self) -> Result<(), ()>;
    pub fn submit(&mut self, commands: &[RenderCommand]);
    pub fn resize(&mut self, size: Size);
}
```

The `wgpu_backend` module provides the concrete wgpu implementation.

---

## Embedded Support (`embedded`)

```rust
pub mod embedded;

// Embedded engine types
pub use render_engine::{
    EmbeddedEngine,
    // see Render Engine section above for method details
};
```

---

## Language Bindings (`bindings`)

The `bindings` module provides FFI infrastructure for C/C++ interop.

```rust
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
```

See the [FFI / C ABI Reference](#ffi--c-abi-reference) section below for the
full C API.

---

## Feature Flags Reference

The library uses a **three-axis feature system** — pick one from each axis:

### Axis 1: Device Profile (mutually exclusive)

| Feature | Description | Includes |
|---|---|---|
| `desktop` (default) | Full desktop | GPU, touch, i18n, chart, print, PDF, a11y, quality, advanced widgets |
| `tablet` | Touch-first tablet | GPU, touch, i18n, quality |
| `mobile` | Mobile-optimized | GPU, touch, i18n, quality, mobile API |
| `embedded` | No GPU, software raster | Software, custom controls |
| `mini` | LVGL-style minimal (~15 widgets) | Software, custom controls, `heapless`, `hashbrown`, `spin`, `bumpalo` |

### Axis 2: OS Backend

| Feature | Platform | Key Dependencies |
|---|---|---|
| `os-auto` | Auto-detect | (none) |
| `macos` | macOS (modern) | `objc2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-graphics` |
| `macos-legacy` | macOS (legacy) | `cocoa`, `objc`, `objc-foundation` |
| `ios` | iOS | `objc2`, `objc2-foundation`, `objc2-ui-kit` |
| `windows` | Windows | `winapi` |
| `linux-gtk` | Linux (GTK) | `gtk` |
| `linux-wayland` | Linux (Wayland) | `wayland-client`, `wayland-protocols`, `wayland-cursor` |
| `linux-a11y` | Linux accessibility | `zbus`, `pollster` |
| `android` | Android | `jni` |
| `wasm` | WebAssembly | `wasm-bindgen`, `web-sys`, `js-sys` |
| `harmony` | HarmonyOS | (none) |

### Axis 3: Capabilities (arbitrary composition)

| Feature | Description |
|---|---|
| `touch` | Touch event support |
| `gpu` / `wgpu` | GPU-accelerated rendering |
| `software` | Software rasterizer |
| `i18n` | Internationalization |
| `chart` | Chart widgets |
| `print` | Printing |
| `pdf` | PDF generation |
| `a11y` | Accessibility |
| `holographic` | Holographic/3D gesture detection (experimental) |
| `projection` | Projection/presentation mode |
| `controls-native` | Native OS-style control rendering |
| `controls-custom` | Custom (themed) control rendering |
| `advanced-widgets` | Advanced widgets (calendar, date/time pickers, ribbon bar, etc.) |
| `unstable-pipeline-routing` | Experimental pipeline routing |
| `unstable-special-widgets` | Experimental special widgets |

### Profile Convenience

| Feature | Description |
|---|---|
| `full` | Enables everything (not for production) |
| `desktop-runtime` | Internal: enables file watcher, channel deps |

### Compile-Time Profiles

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1

[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"

[profile.release-mini]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

---

## Error Codes Reference

| Constant | Code | Description |
|---|---|---|
| `SUCCESS` | 0 | Operation completed successfully |
| `NOT_IMPLEMENTED` | 1 | Feature not yet implemented |
| `UNSUPPORTED_OPERATION` | 2 | Operation not supported on this platform |
| `INVALID_ARGUMENT` | 3 | Invalid parameter or argument |
| `NULL_POINTER` | 4 | Null pointer detected *(reserved)* |
| `OUT_OF_MEMORY` | 5 | Memory allocation failed *(reserved)* |
| `LOCK_POISONED` | 6 | Mutex/ lock poisoned *(reserved)* |
| `WIDGET_BASE_NOT_IMPL` | 100 | Widget base method not implemented *(reserved)* |
| `WIDGET_NOT_FOUND` | 101 | Widget not found *(reserved)* |
| `WIDGET_INVALID_STATE` | 102 | Widget in invalid state *(reserved)* |
| `WIDGET_DEPRECATED` | 103 | Widget deprecated *(reserved)* |
| `PLATFORM_UNSUPPORTED` | 200 | Platform not supported *(reserved)* |
| `PLATFORM_INIT_FAILED` | 201 | Platform initialization failed *(reserved)* |
| `CLIPBOARD_FAILED` | 202 | Clipboard operation failed *(reserved)* |
| `DRAG_DROP_FAILED` | 203 | Drag & drop operation failed *(reserved)* |
| `RENDER_CONTEXT_INVALID` | 300 | Render context invalid *(reserved)* |
| `RENDER_PIPELINE_FAILED` | 301 | Render pipeline failed *(reserved)* |
| `I18N_LOAD_FAILED` | 400 | i18n file load failed *(reserved)* |
| `FILE_NOT_FOUND` | 401 | File not found |

---

## FFI / C ABI Reference

The library exposes a stable C ABI for language interop. All C functions are
prefixed with `rw_`. The generated header is at `include/rw_generated.h`.

### Lifecycle

```c
void rw_init(void);
void rw_run(void);
void rw_quit(void);
```

### Widget Creation

```c
uint64_t rw_create_window(const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_button(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_checkbox(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_label(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_line_edit(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_radio_button(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_slider(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_progress_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_combo_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_list_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_panel(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_message_box(uint64_t parent, const char* title, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_file_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_color_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_font_dialog(uint64_t parent, const char* title, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_spin_box(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_list_view(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_scroll_area(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_menu_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_menu(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_tool_bar(uint64_t parent, int x, int y, uint32_t width, uint32_t height);
uint64_t rw_create_status_bar(uint64_t parent, const char* text, int x, int y, uint32_t width, uint32_t height);
```

### Widget Manipulation

```c
void rw_show_widget(uint64_t widget_id);
void rw_hide_widget(uint64_t widget_id);
void rw_set_widget_geometry(uint64_t widget_id, int x, int y, uint32_t width, uint32_t height);
bool rw_get_widget_geometry(uint64_t widget_id, int* x_out, int* y_out, uint32_t* width_out, uint32_t* height_out);
void rw_set_widget_text(uint64_t widget_id, const char* text);
const char* rw_get_widget_text(uint64_t widget_id);
void rw_set_widget_enabled(uint64_t widget_id, bool enabled);
bool rw_is_widget_enabled(uint64_t widget_id);
void rw_set_widget_visible(uint64_t widget_id, bool visible);
bool rw_is_widget_visible(uint64_t widget_id);
```

### Combo Box

```c
bool rw_combo_box_add_item(uint64_t combo_box, const char* text);
bool rw_combo_box_clear_items(uint64_t combo_box);
int rw_combo_box_current_index(uint64_t combo_box);
bool rw_combo_box_set_current_index(uint64_t combo_box, uint32_t index);
uint32_t rw_combo_box_item_count(uint64_t combo_box);
const char* rw_combo_box_item_text(uint64_t combo_box, uint32_t index);
```

### List Box

```c
bool rw_list_box_add_item(uint64_t list_box, const char* text);
bool rw_list_box_remove_item(uint64_t list_box, uint32_t index);
bool rw_list_box_clear_items(uint64_t list_box);
int rw_list_box_current_index(uint64_t list_box);
bool rw_list_box_set_current_index(uint64_t list_box, uint32_t index);
uint32_t rw_list_box_item_count(uint64_t list_box);
const char* rw_list_box_item_text(uint64_t list_box, uint32_t index);
```

### Menu

```c
bool rw_attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar);
uint64_t rw_menu_add_item(uint64_t parent_menu, const char* text, const char* shortcut);
uint64_t rw_poll_menu_triggered(void);
bool rw_inject_menu_trigger(uint64_t menu_item_id);
```

### Clipboard

```c
bool rw_set_clipboard_text(const char* text);
const char* rw_get_clipboard_text(void);
```

### Event Polling

```c
uint64_t rw_poll_widget_triggered(void);
uint32_t rw_poll_widget_trigger_event(uint64_t* widget_id_out);
bool rw_inject_widget_trigger_event(uint64_t widget_id, uint32_t kind_code);
```

### Drag & Drop

```c
bool rw_begin_drag(uint64_t source, const char* mime_type, const uint8_t* payload, uint32_t payload_len);
bool rw_poll_drop_event(uint64_t* source_out, uint64_t* target_out, char** mime_out, uint8_t** payload_out, uint32_t* payload_len_out);
```

### IME & Accessibility

```c
bool rw_set_widget_ime_enabled(uint64_t widget_id, bool enabled);
bool rw_is_widget_ime_enabled(uint64_t widget_id);
bool rw_set_widget_accessibility_name(uint64_t widget_id, const char* name);
const char* rw_get_widget_accessibility_name(uint64_t widget_id);
```

### Platform Info

```c
const char* rw_backend_name(void);
uint32_t rw_platform_capabilities(void);
uint32_t rw_platform_capability_contract(uint32_t profile_code);
float rw_platform_dpi_scale_factor(void);
```

### Render

```c
uint32_t rw_set_render_aa_samples_per_axis(uint32_t samples);
uint32_t rw_get_render_aa_samples_per_axis(void);
```

### Embedded Engine

```c
bool rw_embedded_engine_is_initialized(void);
bool rw_embedded_engine_is_running(void);
uint64_t rw_embedded_engine_window_count(void);
uint64_t rw_embedded_engine_button_count(void);
uint64_t rw_embedded_engine_frame_count(void);
uint64_t rw_embedded_engine_pending_task_count(void);
uint64_t rw_submit_embedded_noop_task(const char* label);
uint32_t rw_set_embedded_target_fps(uint32_t fps);
uint32_t rw_get_embedded_target_fps(void);
```

### HarmonyOS Bridge (experimental)

```c
bool rw_harmony_bind_node(uint64_t node_handle, uint64_t widget_id);
void rw_harmony_clear_node_bindings(void);
uint64_t rw_harmony_lookup_widget_id(uint64_t node_handle);
bool rw_harmony_on_click(uint64_t widget_id);
bool rw_harmony_on_value_changed(uint64_t widget_id);
bool rw_harmony_on_widget_event(uint64_t widget_id, uint32_t kind_code);
bool rw_harmony_unbind_node(uint64_t node_handle);
```

### Error Handling (C)

```c
const char* rw_error_message(uint64_t handle);
int32_t rw_error_code(uint64_t handle);
```

### Memory Management (C)

```c
void rw_free_string(char* s);
void rw_free_rust_string(char* s);
```

### Binding Status

```c
uint32_t rw_bindings_api_version(void);
uint32_t rw_cpp_binding_status(void);
uint32_t rw_java_binding_status(void);
uint32_t rw_python_binding_status(void);
```

### Mobile

```c
bool rw_mobile_attach_native_view(uint64_t native_handle);
const char* rw_mobile_backend_name(void);
```

---

## Coordinate System Reference

All rendering and widget positioning uses a **screen coordinate system**:

```
(0, 0) -------------> X (increases right)
  |
  |    Screen Space (pixels)
  |    Origin: Top-Left Corner
  |
  v Y (increases down)
```

**Chart coordinates** use Cartesian (Y increases up), automatically converted.
**PDF coordinates** use bottom-left origin, converted automatically during rendering.
**SVG** uses the same top-left origin as screen coordinates — no conversion needed.

Helper functions in `core::coords`:

| Function | Purpose |
|---|---|
| `to_screen_y(cartesian_y, height)` | Cartesian → screen Y |
| `to_cartesian_y(screen_y, height)` | Screen → Cartesian Y |
| `to_pdf_y(screen_y, page_height)` | Screen → PDF Y |

---

## Style Inheritance Chain

```
1. Global Theme defaults      (ThemeManager → Theme)
2. Theme overrides            (per widget class, e.g. "Button", "Label")
3. Widget instance state      (WidgetStyle set on individual widget)
4. Inline style overrides     (future)
```

Each step falls through to the next if unset. Use `WidgetStyle::inherit_from()`
to manually compose styles.

---

## Thread Safety Notes

| Type Category | Safety |
|---|---|
| Widget handles | `Send + Sync` (backed by `ObjectId` u64) |
| Platform backend | `Send + Sync` |
| Signal/Slot | `Send + Sync` on signal, closure must be `Send + 'static` |
| Render backends | Single-threaded access per surface |
| Event loop | Single-threaded (the event loop thread) |
| I18nManager | `Send` (global singleton with RwLock) |
| ThemeManager | `Send` |
| ObjectId | `Copy + Send + Sync` |
| ArenaAllocator | `Send` |
| MemoryMonitor | `Send + Sync` |

---

## Minimum Supported Rust Version (MSRV)

**Rust 1.87** — required for `edition = "2021"` and current dependency versions.
