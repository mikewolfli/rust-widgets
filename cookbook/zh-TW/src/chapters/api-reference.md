# API 參考

本章提供完整的、按模組劃分的 `rust_widgets` 公開 API 參考文件。當您需要為任務查找正確的型別、函式或特徵時，可將其用作快速查閱手冊。

此處記載的函式庫版本為 **1.0.0**。程式碼範例假設使用 `use rust_widgets::*;` 或如所示使用顯式路徑。

---

## 目錄

1. [頂層函式](#top-level-functions)
2. [應用生命週期 (`app`)](#application-lifecycle-app)
3. [核心型別 (`core`)](#core-primitives-core)
4. [控制項系統 (`widget`)](#widget-system-widget)
5. [佈局系統 (`layout`)](#layout-system-layout)
6. [事件系統 (`event`)](#event-system-event)
7. [渲染系統 (`render`)](#rendering-system-render)
8. [渲染引擎 (`render_engine`)](#render-engine-render_engine)
9. [樣式與主題 (`style`, `theme`)](#style--theming-style-theme)
10. [平台抽象 (`platform`)](#platform-abstraction-platform)
11. [錯誤系統 (`error`)](#error-system-error)
12. [動作框架 (`action`)](#action-framework-action)
13. [快捷鍵系統 (`shortcut`)](#shortcut-system-shortcut)
14. [資料繫結 (`data_binding`)](#data-binding-data_binding)
15. [信號/槽 (`signal`)](#signalslot-signal)
16. [國際化 (`i18n`)](#internationalization-i18n)
17. [手勢辨識 (`gesture`)](#gesture-recognition-gesture)
18. [圖表與資料視覺化 (`chart`)](#charts--data-visualization-chart)
19. [PDF 生成 (`pdf`)](#pdf-generation-pdf)
20. [列印 (`print`)](#printing-print)
21. [記憶體管理 (`memory`)](#memory-management-memory)
22. [效能 (`performance`)](#performance-performance)
23. [自適應品質 (`quality`)](#adaptive-quality-quality)
24. [控制後端 (`control_backend`)](#control-backend-control_backend)
25. [物件系統 (`object`)](#object-system-object)
26. [Web 能力 (`web`)](#web-capabilities-web)
27. [復原/重做 (`undo`)](#undoredo-undo)
28. [剪貼簿 (`clipboard`)](#clipboard-clipboard)
29. [GPU 加速 (`gpu`, `wgpu_backend`)](#gpu-acceleration-gpu-wgpu_backend)
30. [嵌入式支援 (`embedded`)](#embedded-support-embedded)
31. [語言繫結 (`bindings`)](#language-bindings-bindings)
32. [功能旗標參考](#feature-flags-reference)
33. [錯誤碼參考](#error-codes-reference)
34. [FFI / C ABI 參考](#ffi--c-abi-reference)

---

## 頂層函式

Crate 根目錄 (`rust_widgets`) 公開了一組便利函式，可在不使用 `App` 包裝器的情況下快速開發應用程式。這些函式非常適合簡單的腳本或 FFI 入口點。

### 生命週期函式

| 函式 | 簽名 | 說明 |
|---|---|---|
| `init` | `fn()` | 初始化執行環境（自動選擇平台後端） |
| `run` | `fn()` | 進入主事件迴圈（阻塞直到 `quit`） |
| `quit` | `fn()` | 通知事件迴圈結束 |

### 視窗建立

```rust
pub fn create_window(title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
```

### 控制項建立

每個函式建立一個控制項，返回其 `ObjectId`，並將其作為 `parent` 的子控制項新增。

| 函式 | 返回 `ObjectId` | 備註 |
|---|---|---|
| `create_button(parent, text, x, y, w, h)` | Button | 標準按鈕 |
| `create_checkbox(parent, text, x, y, w, h)` | CheckBox | 切換核取方塊 |
| `create_line_edit(parent, text, x, y, w, h)` | LineEdit | 單行文字輸入 |
| `create_label(parent, text, x, y, w, h)` | Label | 靜態文字顯示 |
| `create_radio_button(parent, text, x, y, w, h)` | RadioButton | 互斥選項 |
| `create_slider(parent, x, y, w, h)` | Slider | 數值滑桿 |
| `create_progress_bar(parent, x, y, w, h)` | ProgressBar | 進度指示器 |
| `create_combo_box(parent, x, y, w, h)` | ComboBox | 下拉選擇器 |
| `create_list_box(parent, x, y, w, h)` | ListBox | 清單選取 |
| `create_panel(parent, x, y, w, h)` | Panel (GroupBox) | 容器面板 |
| `create_message_box(parent, title, text, x, y, w, h)` | MessageBox | 模態訊息對話方塊 |
| `create_file_dialog(parent, title, x, y, w, h)` | FileDialog | 檔案選擇器 |
| `create_color_dialog(parent, title, x, y, w, h)` | ColorDialog | 色彩選擇器 |
| `create_font_dialog(parent, title, x, y, w, h)` | FontDialog | 字型選擇器 |
| `create_spin_box(parent, x, y, w, h)` | SpinBox | 數值微調控制項 |
| `create_list_view(parent, x, y, w, h)` | ListView | 表格樣式清單 |
| `create_scroll_area(parent, x, y, w, h)` | ScrollArea | 可捲動容器 |

### 控制項操作

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

### Combo Box 操作

```rust
pub fn combo_box_add_item(id: ObjectId, text: &str);
pub fn combo_box_clear_items(id: ObjectId);
pub fn combo_box_set_current_index(id: ObjectId, index: u32);
pub fn combo_box_current_index(id: ObjectId) -> i32;
pub fn combo_box_item_count(id: ObjectId) -> u32;
pub fn combo_box_item_text(id: ObjectId, index: u32) -> String;
```

### List Box 操作

```rust
pub fn list_box_add_item(id: ObjectId, text: &str);
pub fn list_box_remove_item(id: ObjectId, index: u32);
pub fn list_box_clear_items(id: ObjectId);
pub fn list_box_set_current_index(id: ObjectId, index: u32);
pub fn list_box_current_index(id: ObjectId) -> i32;
pub fn list_box_item_count(id: ObjectId) -> u32;
pub fn list_box_item_text(id: ObjectId, index: u32) -> String;
```

### 事件輪詢（輪詢 API）

```rust
pub fn poll_widget_triggered() -> Option<ObjectId>;
pub fn poll_widget_trigger_event() -> Option<(ObjectId, u32)>;
pub fn inject_widget_trigger_event(id: ObjectId, kind: u32) -> bool;
```

### 剪貼簿

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> PlatformClipboard;
```

### 選單/工具列

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

### 拖放

```rust
pub fn begin_drag(source: ObjectId, mime_type: &str, payload: &[u8]) -> bool;
pub fn poll_drop_event() -> Option<DropEvent>;
pub fn inject_drop_event(source: ObjectId, target: ObjectId, mime: &str, payload: &[u8]) -> bool;
```

### IME 與無障礙

```rust
pub fn set_widget_ime_enabled(id: ObjectId, enabled: bool);
pub fn is_widget_ime_enabled(id: ObjectId) -> bool;
pub fn platform_ime_bridge() -> Option<ImeBridge>;
pub fn set_widget_accessibility_name(id: ObjectId, name: &str);
pub fn get_widget_accessibility_name(id: ObjectId) -> String;
```

---

## 應用生命週期 (`app`)

`app` 模組是正式應用程式的**建議入口點**。

### 核心型別

```rust
pub struct App { /* ... */ }
pub struct AppConfig {
    pub app_name: String,
    pub enable_i18n: bool,
    // ...
}
```

### App 方法

| 方法 | 簽名 | 說明 |
|---|---|---|
| `new` | `fn(config: AppConfig) -> Self` | 使用設定建立應用程式 |
| `run` | `fn(self)` | 執行事件迴圈 |
| `window` | `fn(&self) -> &WindowHandle` | 取得主視窗控制代碼 |
| `quit` | `fn(&self)` | 結束應用程式 |

### 控制項控制代碼型別

每個控制代碼包裝一個 `ObjectId`，並公開型別安全的操作。

| 控制代碼 | 控制項型別 | 主要操作（除 WidgetHandle 外） |
|---|---|---|
| `WidgetHandle` | (基礎特徵) | `raw_id()`, `from_raw()`, `show()`, `hide()`, `set_geometry()`, `set_text()`, `text()`, `enable()`, `disable()`, `is_enabled()`, `set_visible()`, `is_visible()`, `on_click()`, `on_value_changed()` |
| `WindowHandle` | Window | `set_title()`, `title()`, `resize()`, `minimize()`, `maximize()`, `restore()`, `close()` |
| `ButtonHandle` | Button | `set_text()`, `text()` — 繼承 `WidgetHandle` |
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
| `FrameHandle` | Frame | 通用框架容器 |
| `GridWidgetHandle` | GridWidget | 網格專用操作 |

### 輔助型別

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

## 核心型別 (`core`)

### 幾何型別

```rust
pub type ObjectId = u64;

pub struct Point {
    pub x: i32,
    pub y: i32,
}
```

`Point` 建構子：`new(x, y)`, `origin()`，以及 `from_f32()`, `from_u32()`,
`from_i64()`, `from_f64()`, `from_usize()`, `from_isize()` 和它們的 `_tuple`
變體。算術運算：`Add<(i32, i32)>`。

```rust
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

`Size` 建構子：`new(w, h)`，以及 `from_f32()`, `from_i32()`, `from_i64()`,
`from_f64()`, `from_usize()`, `from_isize()` 和 `_tuple` 變體。
方法：`is_empty()`, `area()`, `aspect_ratio()`。

```rust
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
```

| 方法 | 簽名 | 說明 |
|---|---|---|
| `new` | `(x, y, w, h)` | 建立矩形 |
| `from_position_size` | `(Point, Size)` | 從位置+尺寸建立 |
| `position()` / `size()` | — | 分解取得位置/尺寸 |
| `contains_point(p)` | `-> bool` | 點擊命中測試 |
| `intersects(r)` | `-> bool` | 重疊測試 |
| `contains_rect(r)` | `-> bool` | 完全包含 |
| `union(r)` | `-> Rect` | 合併外接矩形 |
| `intersection(r)` | `-> Rect` | 重疊交集 |
| `expand_to_touch_target(min)` | `-> Rect` | 擴展到最小觸控尺寸 |
| `center()` | `-> Point` | 矩形中心點 |
| `right()` / `bottom()` | `-> i32` | 邊緣座標 |
| `with_padding(p)` / `with_margin(m)` | `-> Rect` | 內縮/外擴 |
| `shrink(amount)` / `grow(amount)` | `-> Rect` | 均勻內縮/外擴 |
| `clamp_point(p)` | `-> Point` | 將點限制在矩形內 |
| `extend_to_include(p)` | `-> Rect` | 擴展以包含該點 |
| `area()` | `-> u32` | 寬度 × 高度 |

```rust
pub enum Orientation { Horizontal, Vertical }
```

### 色彩

```rust
pub struct Color {
    pub r: u8,  // 0-255
    pub g: u8,  // 0-255
    pub b: u8,  // 0-255
    pub a: u8,  // 0-255
}
```

| 方法 | 說明 |
|---|---|
| `rgba(r, g, b, a)` | 從 0-255 數值建立 |
| `rgb(r, g, b)` | 不透明色彩（alpha = 255） |
| `from_rgba_u32(v)` | 打包的 0xRRGGBBAA |
| `from_f32(r, g, b, a)` | 從 0.0-1.0 浮點數建立 |
| `parse_hex(s)` | `"#"RGB"`, `"#RGBA"`, `"#RRGGBB"` 或 `"#RRGGBBAA"` |
| `to_hex_rgb()` / `to_hex_rgba()` | 序列化 |
| `with_alpha(a)` | 使用不同 Alpha 值的新色彩 |
| `blend(other)` | Alpha 混合 |
| `luminance()` | 感知亮度 |
| `is_dark()` / `is_light()` | 亮度分類 |
| `contrast_color()` | 返回黑色或白色以確保可讀性 |
| `invert()` | RGB 反轉 |

預定義常數（55+）：`BLACK`, `WHITE`, `RED`, `GREEN`, `BLUE`, `YELLOW`,
`CYAN`, `MAGENTA`, `GRAY`, `LIGHT_GRAY`, `DARK_GRAY`, `TRANSPARENT`，
語意色彩（`PRIMARY`, `SECONDARY`, `SUCCESS`, `WARNING`, `ERROR`,
`BACKGROUND`, `FOREGROUND`, `LINK`, `BORDER`, `DIVIDER`, `SELECTION`,
`TOOLTIP`, `INFO`, `NOTIFICATION`, `DISABLED_BACKGROUND`,
`DISABLED_FOREGROUND`），以及擴展網頁色彩（`ALICE_BLUE`, `BEIGE`,
`CORAL`, `GOLD`, `INDIGO`, `MAROON`, `NAVY`, `OLIVE`, `ORANGE`, `PINK`,
`PURPLE`, `TEAL`, `SKY_BLUE`, `STEEL_BLUE` 等）。

### 對齊

```rust
pub enum Alignment { Left, Center, Right, Top, Bottom }
pub enum HorizontalAlignment { Left, Center, Right }
pub enum VerticalAlignment { Top, Center, Bottom }
```

每一個都支援 `parse_str()`, `as_str()`, `is_*()` 查詢方法，以及透過
`from_alignment()` 進行水平/垂直之間的轉換。

### 字型

```rust
pub struct Font {
    pub family: String,
    pub size: f32,
    pub weight: u32,    // 100-900（CSS 字重比例）
    pub bold: bool,
    pub italic: bool,
}
```

### 核心列舉與結構體

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

`Version` 方法：`new()`, `from_u32()`, `to_u32()`, `parse_str()`,
`is_compatible_with()`, `is_newer_than()`, `is_older_than()`。

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

工廠方法：`desktop()`, `embedded()`, `mobile()`。

```rust
pub struct CoreConfig {
    pub profile: RuntimeProfile,
    pub platform: PlatformFamily,
    pub capabilities: PlatformCapabilities,
    pub version: Version,
}
```

### 核心特徵

```rust
pub trait CoreObject: Debug + Send + Sync {
    fn id(&self) -> ObjectId;
    fn set_id(&mut self, id: ObjectId);
    fn type_name(&self) -> &'static str;
}
```

### 座標工具函式

`coords` 模組提供座標系統轉換：

```rust
pub fn to_screen_y(cartesian_y: f32, height: f32) -> f32;
pub fn to_cartesian_y(screen_y: f32, height: f32) -> f32;
pub fn to_pdf_y(screen_y: f32, page_height: f32) -> f32;
```

`rect_merge` 模組提供：

```rust
pub fn merge_intersecting_rects(rects: &[Rect]) -> Vec<Rect>;
pub fn bounding_rect(rects: &[Rect]) -> Option<Rect>;
```

`MutexExt` 擴展特徵新增了鎖中毒恢復：

```rust
pub trait MutexExt<T> {
    fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R;
}
```

---

## 控制項系統 (`widget`)

### 核心特徵

```rust
pub trait Widget: EventHandler + Any {
    // 必要方法
    fn base(&self) -> &BaseWidget;
    fn base_mut(&mut self) -> &mut BaseWidget;

    // 識別
    fn id(&self) -> ObjectId;
    fn kind(&self) -> WidgetKind;

    // 幾何（10+ 方法）
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    fn position(&self) -> Point;
    fn size(&self) -> Size;
    fn min_size(&self) -> Option<Size>;
    fn max_size(&self) -> Option<Size>;
    fn set_min_size(&mut self, ...);
    fn set_max_size(&mut self, ...);

    // 狀態（10+ 方法）
    fn is_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn has_focus(&self) -> bool;

    // 層級結構
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn children(&self) -> Vec<ObjectId>;
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);

    // 信號
    fn triggered(&self) -> &Signal<ObjectId>;
    fn value_changed(&self) -> &Signal<String>;

    // 樣式
    fn style(&self) -> &WidgetStyle;
    fn style_mut(&mut self) -> &mut WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
    fn css_class(&self) -> &[String];

    // DPI / 佈局
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, scale: f32);
    fn layout_scale(&self) -> f32;

    // 工具提示
    fn set_tooltip(&mut self, text: String);
    fn tooltip(&self) -> Option<&str>;
}

pub trait Draw {
    fn draw(&self, ctx: &mut RenderContext);
}
```

`BaseWidget` 實作了所有共用狀態和 60+ 個預設方法。

### 控制項種類列舉

`WidgetKind` 列舉系統中的每一種控制項型別。選取的部分變體：

- `Button`, `CheckBox`, `RadioButton`, `Label`
- `LineEdit`, `TextArea`, `ComboBox`, `ListBox`, `SpinBox`, `Dropdown`
- `Slider`, `ProgressBar`, `ScrollBar`, `Spinner`, `Meter`, `Arc`, `Roller`
- `ImageView`, `MiniCanvas`, `MiniChart`, `Line`, `LCDNumber`
- `GroupBox`, `ScrollArea`, `Splitter`, `TabWidget`, `StackedWidget`
- `TileView`, `CollapsiblePane`, `DockWidget`, `MdiArea`, `ToolBox`
- `Window`
- `ToggleButton`, `Switch`（新控制項）
- `Calendar`, `DateEdit`, `TimeEdit`, `DateTimeEdit`, `Dial`
- `KeySequenceEdit`, `PieMenu`, `RibbonBar`, `TabBar`
- `Menu`, `MenuBar`, `StatusBar`, `ToolBar`, `ToolButton`
- `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`, `InputDialog`
- `ProgressDialog`, `PopupWindow`
- `ListView`, `TableView`, `TreeView`, `DataGrid`, `VirtualList`, `VirtualTable`
- `WebView`, `WebEngine`
- （60+ 個新控制項型別 — 見下方）

### 控制項分類

**基礎控制項**（始終可用）：

| 控制項 | 型別別名 | 模組 |
|---|---|---|
| `Button` | — | `base_widgets::button` |
| `CheckBox` | — | `base_widgets::checkbox` |
| `Label` | — | `base_widgets::label` |
| `RadioButton` | — | `base_widgets::radiobutton` |
| `ToggleButton` | — | `base_widgets::toggle_button` *(非 mini)* |

輸入型別：

| 控制項 | 型別別名 | 功能 |
|---|---|---|
| `LineEdit` | — | EchoMode 支援 |
| `TextArea` | — | 多行 |
| `ComboBox` | — | 下拉選取 |
| `Dropdown` | — | — |
| `ListBox` | — | SelectionMode |
| `SpinBox` | — | 數值微調 |
| `Keyboard` | — | 虛擬鍵盤 |
| `TextEdit` | — | *(非 mini)* |
| `RichEdit` | — | *(非 mini)* |
| `CommandLink` | — | *(非 mini)* |
| `FontComboBox` | — | *(非 mini)* |

顯示型別：

| 控制項 | 說明 |
|---|---|
| `Arc` | 弧形/環形顯示 |
| `ImageView` | 圖片檢視器 |
| `Line` | 水平/垂直分隔線 |
| `Meter` | 儀表 |
| `MiniCanvas` | 自訂繪圖表面 |
| `MiniChart` | 內嵌迷你圖表 |
| `ProgressBar` | 進度條 |
| `Roller` | 滾輪顯示 |
| `ScrollBar` | 捲軸 |
| `Slider` | 數值滑桿 |
| `Spinner` | 活動指示器 |
| `LcdNumber` | LCD 風格顯示 *(非 mini)* |

容器型別：

| 控制項 | 說明 | 功能 |
|---|---|---|
| `GroupBox`（別名 `Panel`） | 分組容器 | 始終可用 |
| `ScrollArea` | 可捲動視口 | 始終可用 |
| `TileView` | 磚塊容器 | 始終可用 |
| `CollapsiblePane` | 可摺疊區塊 | *(非 mini)* |
| `DockWidget`（別名 `DockPanel`） | 可停靠面板 | *(非 mini)* |
| `MdiArea` | MDI 容器 | *(非 mini)* |
| `Splitter` | 可調整大小分割器 | *(非 mini)* |
| `StackedWidget` | 堆疊/頁面 | *(非 mini)* |
| `TabWidget` | 分頁容器 | *(非 mini)* |
| `ToolBox` | 工具箱容器 | *(非 mini)* |

對話方塊型別 *(均為非 mini)*：

| 控制項 | 說明 |
|---|---|
| `MessageBox` | 模態訊息對話方塊 |
| `FileDialog`（別名 `DirectoryDialog`） | 檔案/目錄選擇器 |
| `ColorDialog` | 色彩選擇器 |
| `FontDialog` | 字型選擇器 |
| `InputDialog` | 輸入提示對話方塊 |
| `ProgressDialog` | 進度模態視窗 |
| `PopupWindow`（別名 `Dialog`） | 彈出視窗 |

選單/工具列型別 *(均為非 mini)*：

| 控制項 | 說明 |
|---|---|
| `MenuBar` | 頂層選單列 |
| `Menu`（別名 `ContextMenu`） | 下拉選單 |
| `ToolBar` | 工具列 |
| `StatusBar` | 狀態列 |
| `ToolButton` | 工具列按鈕 |
| `Action` | 動作/命令 |

檢視型別 *(均為非 mini)*：

| 控制項 | 說明 |
|---|---|
| `ListView` | 帶模型的清單 |
| `TableWidget` | 表格檢視 |
| `DataGrid` | 可篩選資料網格 |
| `TreeView`（別名 `ColumnView`） | 樹狀檢視 |
| `VirtualList`（別名 `DataView`） | 虛擬化清單 |
| `VirtualTable` | 虛擬化表格 |
| `TreeTable` | 樹狀表格混合 |

### 控制項分類 — 重新歸類後的組織

來自舊 `new_widgets` 模組的控制項已重新歸類到專用子目錄：

| 目錄 | 控制項 |
|---|---|
| `nav_widgets/` | `AdaptiveScaffold`, `AppBar`, `BottomNavigationBar`, `NavigationDrawer`, `NavigationStack`, `TabView` |
| `chart_widgets/` | `BarChart`, `LineChart`, `PieChart`, `Sparkline` |
| `media_widgets/` | `AnimatedImage`, `AudioVisualizer`, `CameraPreview`, `HeroAnimation`, `LottieWidget`, `RiveWidget`, `VideoPlayer` |
| `overlay_widgets/` | `FAB`, `PullToRefresh`, `RefreshControl`, `SwipeToDismiss` |
| `cupertino/` | `CupertinoAlertDialog`, `CupertinoDatePicker`, `CupertinoNavigationBar`, `CupertinoSegmentedControl`, `CupertinoSlider`, `CupertinoSwitch`, `MaterialNavigationRail`, `MaterialSnackbar` |
| `misc_widgets/` | `Avatar`, `BarcodeScanner`, `BezierCurveEditor`, `DateRangePicker`, `MobileDatePicker`, `QRCode`, `SegmentedButton` |
| `input_widgets/`（擴展） | `AutoCompleteEdit`, `EditableComboBox`, `ImePreedit`, `InplaceEditor`, `MaskedEdit`, `MultiSelectComboBox`, `RangeSlider`, `SearchBar`, `SearchBox`, `ShortcutEditor`, `TagInput` |
| `display_widgets/`（擴展） | `Badge`, `ColorHistory`, `ColorWell`, `Divider`, `EmptyState`, `FloatingLabel`, `FontPreview`, `Icon`, `ProgressCircle`, `Rating`, `SkeletonLoader`, `Switch` |
| `container_widgets/`（擴展） | `Carousel`, `MasonryLayout`, `PagerPageView`, `SafeArea`, `Stepper` |
| `dialog/`（擴展） | `BottomSheet`, `FindReplaceDialog`, `ModalBottomSheet`, `Popover`, `Tooltip`, `WizardDialog` |
| `menu_toolbar/`（擴展） | `DropdownMenu`, `MenuButton` |
| `view_widgets/`（擴展） | `ImageGallery`, `PropertiesPanel`, `PropertyGrid` |

### Web 控制項 *(非 mini)*

| 控制項 | 說明 |
|---|---|
| `WebView` | 嵌入式網頁瀏覽器檢視 |
| `WebEngine` | 用於渲染的 Web 引擎 |

關聯型別：`WebEngineContextMenuRequest`, `WebEngineCookieStore`,
`WebEngineDownloadItem`, `WebEngineFindTextResult`, `WebEngineNotification`,
`WebEnginePage`, `WebEngineScriptDialog`, `WebEngineSettings`,
`WebEngineWebChannel`。

### 進階控制項 *(非 mini)*

| 控制項 | 說明 |
|---|---|
| `Calendar` | 日期日曆選擇器 |
| `DateEdit`（別名 `DatePicker`） | 日期編輯器 |
| `TimeEdit`（別名 `TimePicker`） | 時間編輯器 |
| `DateTimeEdit`（別名 `DateTimePicker`） | 日期時間編輯器 |
| `Dial` | 旋轉撥盤 |
| `KeySequenceEdit` | 鍵盤快捷鍵記錄器 |
| `PieMenu` | 放射狀/圓形選單 |
| `RibbonBar` | 功能區風格工具列 |

### 特殊控制項 *(非 mini)*

| 控制項 | 說明 |
|---|---|
| `Canvas` | 自由繪圖表面 |
| `ChartWidget` | 圖表顯示控制項 |
| `CodeEditor` | 原始碼編輯器 |
| `ColorPicker` | 色彩選取控制項 |
| `CommandEntry` | 命令輸入 |
| `CommandPalette` | 命令面板覆蓋層 |
| `DiffViewer` | 並排比對檢視器 |
| `GanttWidget` | 甘特圖 |
| `GridWidget` | 網格顯示 |
| `MapView` | 地圖顯示 |
| `MarkdownEditor` | Markdown 編輯 |
| `MediaPlayer` | 媒體播放器控制項 |
| `NotificationCenter` | 通知中心 |
| `SegmentedControl` | 分段按鈕群組 |
| `SplitButton` | 分割動作按鈕 |
| `TerminalView` | 終端機模擬器 |
| `TimelineWidget` | 時間軸顯示 |

### 控制項能力系統

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

### 控制項子型別

| 型別 | 形式 |
|---|---|
| `ToggleButtonState` | `struct { checked: bool }` |
| `ButtonState` | `struct { pressed: bool, hovered: bool }` |
| `CheckState` | `enum { Unchecked, Checked, PartiallyChecked }` |
| `EchoMode` | `enum { Normal, Password, NoEcho }` |
| `SelectionMode` | `enum { Single, Multi, Extended, None }` |
| `LineOrientation` | `enum { Horizontal, Vertical }` |
| `RangeSliderOrientation` | `enum { Horizontal, Vertical }` |

---

## 佈局系統 (`layout`)

### 核心特徵

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

### 佈局型別

| 佈局 | 模組 | 說明 |
|---|---|---|
| `BoxLayout` | `box_layout` | 水平或垂直盒子 — 按伸縮因子比例分配空間 |
| `HBoxLayout` | `box_layout` | `Orientation::Horizontal` 的 `BoxLayout`（命名便利） |
| `VBoxLayout` | `box_layout` | `Orientation::Vertical` 的 `BoxLayout`（命名便利） |
| `GridLayout` | `grid` | 行/列網格，使用 `set_widget(row, col, id)` |
| `StackLayout` | `stack` | 一次只顯示一個子控制項（卡片堆疊），使用 `set_current_index()` |
| `FlowLayout` | `flow` | 自動換行（類似 CSS flex-wrap） |
| `FlexLayout` | `flex` | CSS flexbox 風格佈局 |
| `FormLayout` | `form` | 標籤-欄位配對佈局 |
| `SplitterLayout` | `splitter` | 可調整大小的分割窗格 |
| `AbsoluteLayout` | `absolute` | 自由形式定位 |
| `CenterLayout` | `center` | 居中單一子控制項 |
| `AspectRatioLayout` | `aspect_ratio` | 保持長寬比 |
| `WrapLayout` | `wrap` | 自動換行水平/垂直 |
| `UniformGridLayout` | `uniform_grid` | 固定大小儲存格 |
| `KeyboardAwareLayout` | `keyboard_aware` | 適應虛擬鍵盤 |

### 輔助型別

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

## 事件系統 (`event`)

### 事件型別

```rust
pub enum Event {
    // 滑鼠
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

    // 鍵盤
    KeyDown((u32, u32)),
    KeyUp((u32, u32)),
    KeyPress { key: u32, modifiers: u32 },
    KeyRelease { key: u32, modifiers: u32 },

    // 焦點
    FocusGained,
    FocusLost,

    // 視窗
    Paint,
    Resize { size: Size },
    OrientationChanged { orientation: ScreenOrientation },
    Quit,

    // 計時器
    Timer { id: u32 },

    // 自訂
    Custom { name: String, payload: Vec<u8> },

    // 觸控（feature = "touch"）
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

### 核心型別

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

### 事件迴圈

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

### 事件佇列

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

### 焦點管理器

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

### 指標捕獲

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

### 計時器

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

### 佇列工具函式

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

### 動畫幀請求

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

## 渲染系統 (`render`)

### 核心渲染型別

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

### 繪圖後端特徵

```rust
pub trait PaintBackend {
    fn begin_frame(&mut self, clear_color: Color);
    fn end_frame(&mut self);
    fn draw_pixels(&mut self, x: i32, y: i32, width: u32, height: u32, pixels: &[u8]);
    fn size(&self) -> Size;
    fn dpi_scale(&self) -> f32;
}
```

### 渲染上下文

```rust
pub struct RenderContext<'a> {
    // 包裝一個 &mut dyn PaintBackend
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

### 後緩衝區

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

### 軟體表面

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

### 場景與批次

```rust
pub enum SceneLayer { Background, Content, Foreground, Overlay, Tooltip }
pub struct RenderScene { /* ... */ }
pub struct BatchId(u32);
pub struct BatchCommand { /* ... */ }
pub struct BatchRenderer { /* ... */ }
```

### 自動渲染後端

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

### 文字塑形

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

### 富文字

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

### 文字溢出

```rust
pub enum TextOverflow { Clip, Ellipsis }
pub enum TextClamp { None, Lines(u32), Pixels(f32) }
pub fn apply_text_overflow(text: &str, max_width: f32, font: &Font, overflow: TextOverflow) -> String;
pub fn apply_text_clamp(text: &str, max_lines: u32, font: &Font, width: f32, clamp: TextClamp) -> String;
```

### 字簇支援

```rust
pub struct GraphemeCluster { /* ... */ }
pub struct GraphemeProcessor { /* ... */ }
impl GraphemeProcessor {
    pub fn new() -> Self;
    pub fn grapheme_clusters(&self, text: &str) -> Vec<GraphemeCluster>;
}
```

### SVG 渲染

```rust
pub struct SvgPaintBackend { /* ... */ }
impl SvgPaintBackend {
    pub fn new(size: Size) -> Self;
    pub fn render_to_string(&self) -> String;
    pub fn render_to_bytes(&self) -> Vec<u8>;
}
```

### 品質

```rust
pub fn current_fps() -> f64;
pub fn average_frame_time() -> f64;
pub fn current_quality_level() -> QualityLevel;
pub fn set_quality_level(level: QualityLevel);
```

### GPU 渲染（feature = `gpu-wgpu`）

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

### 投影（feature = `projection`）

```rust
pub struct PresentationController { /* ... */ }
pub struct ProjectionRenderConfig { /* ... */ }
pub struct ProjectionLayoutHelper { /* ... */ }
```

---

## 渲染引擎 (`render_engine`)

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

### 原生引擎

```rust
pub struct NativeEngine { /* ... */ }
impl NativeEngine {
    pub fn new() -> Self;
}
impl EngineTrait for NativeEngine { /* ... */ }
```

### 嵌入式引擎

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

## 樣式與主題 (`style`, `theme`)

### 樣式型別

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

建構子方法：`with_background(c)`, `with_text_color(c)`, `with_font(f)`,
`with_border(c, w, r)`, `with_padding(p)`, `with_margin(m)`, `with_shadow(s)`,
`with_touch_target(s)`, `with_gradient(g)`, `with_opacity(o)`。

實例方法：`inherit_from(parent)`, `merge(other)`。

### 內邊距與外邊距

```rust
pub struct Padding { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
pub struct Margin  { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
```

兩者都支援：`new(top, right, bottom, left)`, `all(value)`, `symmetric(v, h)`,
`normalized(top, right, bottom, left)` — 負值會被限制為 0。

### 陰影

```rust
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub blur: u32,
    pub color: Color,
}
```

建構子：`new()`, `with_offset(x, y)`, `with_blur(b)`, `with_color(c)`。

### 觸控目標

```rust
pub enum TouchTargetSize { Desktop, Tablet, Phone, Embedded, Projection }
impl TouchTargetSize {
    pub fn dimensions(self) -> Size;
    pub fn spacing(self) -> u32;
}
```

### 減少動畫

```rust
pub enum ReducedMotionPreference { NoPreference, ReduceMotion }
```

### CSS 與選擇器系統

```rust
// CSS 屬性解析
pub struct CssEngine { /* ... */ }
pub struct Selector { /* ... */ }

// 熱重載 CSS 監視器
pub struct CssWatcher { /* ... */ }
impl CssWatcher {
    pub fn watch(path: &str) -> Result<Self, ()>;
    pub fn poll_changed(&mut self) -> bool;
}
```

### 漸層

```rust
pub enum Gradient {
    Linear { start: Point, end: Point, colors: Vec<(f32, Color)> },
    Radial { center: Point, radius: f32, colors: Vec<(f32, Color)> },
}
```

### 主題

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
    pub fn dark() -> Self;                   // 內建深色主題
    pub fn light() -> Self;                  // 內建淺色主題
}
```

### 主題管理器

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

### 主題子型別

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

### 主題樣式代幣

```rust
pub struct ThemeStyleToken { /* ... */ }
```

### 樣式繼承鏈

```
1. 全域主題預設值 (ThemeManager → Theme)
2. 每個控制項類別的主題覆蓋（例如 "Button", "Label"）
3. 控制項實例狀態（StatefulTheme → WidgetState）
4. 內聯樣式覆蓋
```

---

## 平台抽象 (`platform`)

### 執行階段函式

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

### 平台型別

```rust
pub enum RuntimeGuiMode { Native, Embedded, Headless }

pub struct CapabilityContract { /* ... */ }
pub fn negotiate_capability_contract(profile: u32) -> CapabilityContract;
```

### 無障礙

```rust
pub trait AccessibilityBridge: Send + Sync {
    fn notify_focus_changed(&self, widget_id: ObjectId);
    fn notify_text_changed(&self, widget_id: ObjectId, text: &str);
    fn notify_selection_changed(&self, widget_id: ObjectId);
    fn notify_value_changed(&self, widget_id: ObjectId, value: &str);
}

pub fn wire_focus_manager_to_a11y(fm: &mut FocusManager);
```

### IME（輸入法編輯器）

```rust
pub trait ImeBridge {
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn commit_text(&self, text: &str);
    fn composition_range(&self) -> Option<(u32, u32)>;
    fn composition_text(&self) -> Option<String>;
}
```

### 虛擬鍵盤

```rust
pub struct VirtualKeyboardController { /* ... */ }
impl VirtualKeyboardController {
    pub fn show(&mut self);
    pub fn hide(&mut self);
    pub fn is_visible(&self) -> bool;
    pub fn keyboard_height(&self) -> u32;
}
```

### 剪貼簿（平台）

```rust
pub struct PlatformClipboard { /* ... */ }
impl PlatformClipboard {
    pub fn set(&mut self, content: ClipboardContent);
    pub fn get(&self) -> Option<ClipboardContent>;
    pub fn clear(&mut self);
}
```

### 拖放

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

## 錯誤系統 (`error`)

### 核心型別

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

### 錯誤 ID（穩定供 FFI 使用）

```rust
pub struct ErrorId(pub i32);

// 一般
pub const SUCCESS: ErrorId = ErrorId(0);
pub const NOT_IMPLEMENTED: ErrorId = ErrorId(1);
pub const UNSUPPORTED_OPERATION: ErrorId = ErrorId(2);
pub const INVALID_ARGUMENT: ErrorId = ErrorId(3);
pub const NULL_POINTER: ErrorId = ErrorId(4);      // 保留
pub const OUT_OF_MEMORY: ErrorId = ErrorId(5);      // 保留
pub const LOCK_POISONED: ErrorId = ErrorId(6);      // 保留

// 控制項 (100-199)
pub const WIDGET_BASE_NOT_IMPL: ErrorId = ErrorId(100);   // 保留
pub const WIDGET_NOT_FOUND: ErrorId = ErrorId(101);        // 保留
pub const WIDGET_INVALID_STATE: ErrorId = ErrorId(102);    // 保留
pub const WIDGET_DEPRECATED: ErrorId = ErrorId(103);       // 保留

// 平台 (200-299)
pub const PLATFORM_UNSUPPORTED: ErrorId = ErrorId(200);    // 保留
pub const PLATFORM_INIT_FAILED: ErrorId = ErrorId(201);    // 保留
pub const CLIPBOARD_FAILED: ErrorId = ErrorId(202);         // 保留
pub const DRAG_DROP_FAILED: ErrorId = ErrorId(203);         // 保留

// 渲染 (300-399)
pub const RENDER_CONTEXT_INVALID: ErrorId = ErrorId(300);   // 保留
pub const RENDER_PIPELINE_FAILED: ErrorId = ErrorId(301);   // 保留

// I/O (400-499)
pub const I18N_LOAD_FAILED: ErrorId = ErrorId(400);         // 保留
pub const FILE_NOT_FOUND: ErrorId = ErrorId(401);
```

### 恐慌安全性

```rust
pub fn catch_panic<F, T>(f: F) -> RwResult<T>
where F: FnOnce() -> T + std::panic::UnwindSafe;

pub fn to_error_id(result: RwResult<()>) -> i32;

pub trait CAbiSafe { /* ... */ }
pub fn c_try_fallback(/* ... */);
```

---

## 動作框架 (`action`)

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

### 輔助型別

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

## 快捷鍵系統 (`shortcut`)

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

### 快捷鍵型別

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
    A, B, C, ... Z,       // 字母
    F1, F2, ... F24,       // 功能鍵
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
    pub meta: bool,    // Windows 鍵 / Command 鍵
}
```

---

## 資料繫結 (`data_binding`)

### Binding（單一值）

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

### Computed（衍生值）

```rust
pub struct Computed<T: Clone + 'static> { /* ... */ }

impl<T: Clone + 'static> Computed<T> {
    pub fn new(compute: Box<dyn Fn() -> T>) -> Self;
    pub fn get(&self) -> T;
    pub fn invalidate(&mut self);
    pub fn subscribe(&mut self, listener: Box<dyn FnMut()>);
}
```

### 監聽器特徵

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

### 巨集

```rust
// 建立型別推斷的 Binding<T>
binding!(value);

// 使用閉包建立 Computed<T>
computed!(|| expression);
```

---

## 信號/槽 (`signal`)

### 有型別信號

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

### GenericSignal（無參數）

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

### Signal1（單參數泛型）

```rust
pub struct Signal1<A: 'static> { /* ... */ }
// 與 GenericSignal 相同 API，但帶一個參數
```

### 連線管理

```rust
pub struct ConnectionHandle(usize);

pub struct ConnectionScope { /* ... */ }
impl ConnectionScope {
    pub fn new() -> Self;
    // 當 Scope 被丟棄時，連線會自動斷開
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

## 國際化 (`i18n`)

### 核心 API

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

### 全域函式

```rust
pub fn init(locales_dir: &str) -> Result<(), RwError>;
pub fn init_with_options(options: InitOptions) -> Result<InitReport, RwError>;
pub fn translate(key: &str) -> String;
pub fn translate_with_context(key: &str, context: &[(&str, &str)]) -> String;
pub fn get_manager() -> &'static I18nManager;
pub fn check_and_reload_all() -> Result<(), RwError>;

pub use crate::tr;  // 巨集：tr!("hello") → 翻譯後字串
```

### 選項與型別

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

### 熱重載

```rust
pub fn init_with_hot_reload(locales_dir: &str) -> Result<I18nFileWatcher, RwError>;
pub fn process_reload_events(watcher: &mut I18nFileWatcher) -> Vec<ReloadEvent>;

pub struct I18nFileWatcher { /* ... */ }
```

---

## 手勢辨識 (`gesture`)

### 核心特徵

```rust
pub trait GestureRecognizer: Debug + Send {
    fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    fn reset(&mut self);
}
```

### 手勢引擎

```rust
pub struct GestureEngine { /* ... */ }

impl GestureEngine {
    pub fn new() -> Self;       // 預先填充所有標準辨識器
    pub fn with_recognizers(recognizers: Vec<Box<dyn GestureRecognizer>>) -> Self;
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    pub fn reset_all(&mut self);
    pub fn last_timestamp(&self) -> u64;
}
```

### 辨識器

| 辨識器 | 產生的事件 | 說明 |
|---|---|---|
| `TapGesture` | `Event::Tap` | 快速點擊並釋放（<300ms，<15px 移動） |
| `DoubleTapGesture` | `Event::DoubleTap` | 400ms 內的兩次點擊 |
| `LongPressGesture` | `Event::LongPress` | 按住 ≥500ms |
| `SwipeGesture` | `Event::Swipe` | 快速滑動（≥0.5 px/ms，≥30px） |
| `PanGesture` | `Event::MouseMove` | 持續拖曳追蹤 |
| `LongPressDragGesture` | `Event::Swipe` | 長按後拖曳 |
| `FlingGesture` | `Event::Swipe` | 基於速度的輕彈/甩動 |
| `TwoFingerTapGesture` | 'Custom' | 雙指點擊 |
| `TwoFingerSwipeGesture` | 'Custom' | 雙指滑動 |
| `PinchGesture` | `Event::Pinch` | 雙指距離變化（縮放） |
| `RotateGesture` | `Event::Rotate` | 雙指角度變化 |

---

## 圖表與資料視覺化 (`chart`)

### 圖表型別

`chart` 模組提供了資料視覺化的基礎：

```rust
pub struct ChartLayout { /* ... */ }
pub struct ChartSvgRenderer { /* ... */ }

// 子模組：charts, layout, svg, types

pub use crate::chart::charts::*;
pub use crate::chart::svg::*;
pub use crate::chart::types::*;
```

圖表資料型別包含軸設定、數列定義、圖例以及用於折線圖、長條圖、圓餅圖和散佈圖的資料點結構。

---

## PDF 生成 (`pdf`)

### 核心特徵

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

### 模組子型別

```rust
pub mod annotation;      // PDF 註釋
pub mod document;        // 文件建立/操作
pub mod export;          // PDF 匯出
pub mod form;            // 互動式表單欄位
pub mod hyperlink;       // 超連結
pub mod metadata;        // PDF 元資料（作者、標題等）
pub mod page;            // 頁面管理
pub mod reader;          // PDF 讀取/解析
pub mod security;        // 加密、密碼、權限
pub mod types;           // 共用 PDF 型別
pub mod writer;          // PDF 寫入/序列化
```

---

## 列印 (`print`)

```rust
// 提供列印對話方塊整合和文件佈局支援
pub mod print_impl;
pub use print_impl::*;
```

---

## 記憶體管理 (`memory`)

### 池分配器

```rust
pub struct PoolAllocator { /* ... */ }
impl PoolAllocator {
    pub fn new() -> Self;
    pub fn allocate(&mut self, size: usize) -> Option<*mut u8>;
    pub fn deallocate(&mut self, ptr: *mut u8, size: usize);
    pub fn clear(&mut self);
}
```

### Arena 分配器

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

### 堆疊分配器

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

### 記憶體監控

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

## 效能 (`performance`)

### 髒區域追蹤

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

### 更新批次處理

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

### 效能分析

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

## 自適應品質 (`quality`)

### 品質管理器

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

### 品質等級

```rust
pub enum QualityLevel { Low, Medium, High }

impl QualityLevel {
    pub fn lower(&self) -> Option<Self>;
    pub fn higher(&self) -> Option<Self>;
    pub fn clamp(self, min: Self, max: Self) -> Self;
}
```

### 設定

```rust
pub struct QualityConfig {
    pub target_frame_rate: f32,
    pub degrade_threshold: f32,       // 觸發降級的幀時間倍數
    pub upgrade_threshold: f32,       // 觸發升級的幀時間倍數
    pub degrade_frame_count: u32,     // 連續慢幀數以觸發降級
    pub upgrade_frame_count: u32,     // 連續快幀數以觸發升級
    pub max_quality: QualityLevel,
    pub min_quality: QualityLevel,
}

impl QualityConfig {
    pub fn normalized(self) -> Self;
}
```

### 幀時間監控器

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

### GPU 能力

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

## 控制後端 (`control_backend`)

### 核心型別

```rust
pub enum ControlBackendKind { Native, Custom }
pub enum ControlRoutePreference { NativePreferred, NativeRequired, CustomPreferred, CustomRequired }
```

### 後端特徵

```rust
pub trait ControlBackend {
    fn backend_name(&self) -> &'static str;
    fn draw_button(&self, ctx: &mut RenderContext, rect: Rect, state: &ButtonState);
    fn draw_checkbox(&self, ctx: &mut RenderContext, rect: Rect, state: &CheckState);
    fn draw_slider(&self, ctx: &mut RenderContext, rect: Rect, value: f32);
    // ... 每種控制項型別一個繪製方法
}
```

### 分派

```rust
pub fn get_control_backend() -> Box<dyn ControlBackend>;
pub fn get_control_backend_for_widget(kind: WidgetKind) -> Box<dyn ControlBackend>;
pub fn active_control_policy() -> &'static str;
pub fn route_preference_for_widget_kind(kind: WidgetKind) -> ControlRoutePreference;
```

### 後端實作

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

## 物件系統 (`object`)

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

## Web 能力 (`web`)

### 瀏覽器歷史

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

### 工作階段歷史

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

### JS 引擎

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

### 導航

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

### 外掛程式

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

### 隱私

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

### Web 引擎與 Web View（非 mini）

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

### Web 控制項型別 *(非 mini)*

```rust
pub struct WebEngine { /* ... */ }
pub struct WebView { /* ... */ }

// Web 控制項使用的關聯型別：
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

## 復原/重做 (`undo`)

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

## 剪貼簿 (`clipboard`)

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> PlatformClipboard;
```

---

## GPU 加速 (`gpu`, `wgpu_backend`)

GPU 模組透過 `wgpu` 提供硬體加速渲染：

```rust
#[cfg(feature = "gpu-wgpu")]
pub mod gpu;

#[cfg(feature = "gpu-wgpu")]
pub use gpu::{GpuCapability, GpuRenderer};

// gpu 模組內容：
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

`wgpu_backend` 模組提供具體的 wgpu 實作。

---

## 嵌入式支援 (`embedded`)

```rust
pub mod embedded;

// 嵌入式引擎型別
pub use render_engine::{
    EmbeddedEngine,
    // 詳見上方渲染引擎章節的方法說明
};
```

---

## 語言繫結 (`bindings`)

`bindings` 模組為 C/C++ 互操作提供 FFI 基礎設施。

```rust
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
```

完整的 C API 請參閱下方的 [FFI / C ABI 參考](#ffi--c-abi-reference) 一節。

---

## 功能旗標參考

此函式庫使用**三軸功能系統** — 從每個軸各選一個：

### 軸 1：裝置設定檔（互斥）

| 功能旗標 | 說明 | 包含內容 |
|---|---|---|
| `desktop`（預設） | 完整桌面版 | GPU, touch, i18n, chart, print, PDF, a11y, quality, advanced widgets |
| `tablet` | 觸控優先平板 | GPU, touch, i18n, quality |
| `mobile` | 行動裝置最佳化 | GPU, touch, i18n, quality, mobile API |
| `embedded` | 無 GPU，軟體光柵化 | Software, custom controls |
| `mini` | LVGL 風格最小化（約 15 個控制項） | Software, custom controls, `heapless`, `hashbrown`, `spin`, `bumpalo` |

### 軸 2：作業系統後端

| 功能旗標 | 平台 | 主要依賴 |
|---|---|---|
| `os-auto` | 自動偵測 | (無) |
| `macos` | macOS（現代） | `objc2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-graphics` |
| `macos-legacy` | macOS（舊版） | `cocoa`, `objc`, `objc-foundation` |
| `ios` | iOS | `objc2`, `objc2-foundation`, `objc2-ui-kit` |
| `windows` | Windows | `winapi` |
| `linux-gtk` | Linux（GTK） | `gtk` |
| `linux-wayland` | Linux（Wayland） | `wayland-client`, `wayland-protocols`, `wayland-cursor` |
| `linux-a11y` | Linux 無障礙 | `zbus`, `pollster` |
| `android` | Android | `jni` |
| `wasm` | WebAssembly | `wasm-bindgen`, `web-sys`, `js-sys` |
| `harmony` | HarmonyOS | (無) |

### 軸 3：能力（任意組合）

| 功能旗標 | 說明 |
|---|---|
| `touch` | 觸控事件支援 |
| `gpu` / `wgpu` | GPU 加速渲染 |
| `software` | 軟體光柵化 |
| `i18n` | 國際化 |
| `chart` | 圖表控制項 |
| `print` | 列印 |
| `pdf` | PDF 生成 |
| `a11y` | 無障礙 |
| `holographic` | 全像/3D 手勢偵測（實驗性） |
| `projection` | 投影/簡報模式 |
| `controls-native` | 原生 OS 風格控制項渲染 |
| `controls-custom` | 自訂（主題化）控制項渲染 |
| `advanced-widgets` | 進階控制項（日曆、日期/時間選擇器、功能區列等） |
| `unstable-pipeline-routing` | 實驗性管線路由 |
| `unstable-special-widgets` | 實驗性特殊控制項 |

### 設定檔便利旗標

| 功能旗標 | 說明 |
|---|---|
| `full` | 啟用全部功能（非正式環境使用） |
| `desktop-runtime` | 內部：啟用檔案監視器、通道依賴 |

### 編譯時期設定檔

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

## 錯誤碼參考

| 常數 | 碼 | 說明 |
|---|---|---|
| `SUCCESS` | 0 | 操作成功完成 |
| `NOT_IMPLEMENTED` | 1 | 功能尚未實作 |
| `UNSUPPORTED_OPERATION` | 2 | 此平台不支援的操作 |
| `INVALID_ARGUMENT` | 3 | 無效的參數或引數 |
| `NULL_POINTER` | 4 | 偵測到空指標 *(保留)* |
| `OUT_OF_MEMORY` | 5 | 記憶體分配失敗 *(保留)* |
| `LOCK_POISONED` | 6 | Mutex/鎖已中毒 *(保留)* |
| `WIDGET_BASE_NOT_IMPL` | 100 | 控制項基礎方法未實作 *(保留)* |
| `WIDGET_NOT_FOUND` | 101 | 找不到控制項 *(保留)* |
| `WIDGET_INVALID_STATE` | 102 | 控制項處於無效狀態 *(保留)* |
| `WIDGET_DEPRECATED` | 103 | 控制項已棄用 *(保留)* |
| `PLATFORM_UNSUPPORTED` | 200 | 不支援的平台 *(保留)* |
| `PLATFORM_INIT_FAILED` | 201 | 平台初始化失敗 *(保留)* |
| `CLIPBOARD_FAILED` | 202 | 剪貼簿操作失敗 *(保留)* |
| `DRAG_DROP_FAILED` | 203 | 拖放操作失敗 *(保留)* |
| `RENDER_CONTEXT_INVALID` | 300 | 渲染上下文無效 *(保留)* |
| `RENDER_PIPELINE_FAILED` | 301 | 渲染管線失敗 *(保留)* |
| `I18N_LOAD_FAILED` | 400 | i18n 檔案載入失敗 *(保留)* |
| `FILE_NOT_FOUND` | 401 | 找不到檔案 |

---

## FFI / C ABI 參考

此函式庫為語言互操作公開了穩定的 C ABI。所有 C 函式均以 `rw_` 為前綴。生成的標頭檔位於 `include/rw_generated.h`。

### 生命週期

```c
void rw_init(void);
void rw_run(void);
void rw_quit(void);
```

### 控制項建立

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

### 控制項操作

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

### 選單

```c
bool rw_attach_menu_bar_to_window(uint64_t window, uint64_t menu_bar);
uint64_t rw_menu_add_item(uint64_t parent_menu, const char* text, const char* shortcut);
uint64_t rw_poll_menu_triggered(void);
bool rw_inject_menu_trigger(uint64_t menu_item_id);
```

### 剪貼簿

```c
bool rw_set_clipboard_text(const char* text);
const char* rw_get_clipboard_text(void);
```

### 事件輪詢

```c
uint64_t rw_poll_widget_triggered(void);
uint32_t rw_poll_widget_trigger_event(uint64_t* widget_id_out);
bool rw_inject_widget_trigger_event(uint64_t widget_id, uint32_t kind_code);
```

### 拖放

```c
bool rw_begin_drag(uint64_t source, const char* mime_type, const uint8_t* payload, uint32_t payload_len);
bool rw_poll_drop_event(uint64_t* source_out, uint64_t* target_out, char** mime_out, uint8_t** payload_out, uint32_t* payload_len_out);
```

### IME 與無障礙

```c
bool rw_set_widget_ime_enabled(uint64_t widget_id, bool enabled);
bool rw_is_widget_ime_enabled(uint64_t widget_id);
bool rw_set_widget_accessibility_name(uint64_t widget_id, const char* name);
const char* rw_get_widget_accessibility_name(uint64_t widget_id);
```

### 平台資訊

```c
const char* rw_backend_name(void);
uint32_t rw_platform_capabilities(void);
uint32_t rw_platform_capability_contract(uint32_t profile_code);
float rw_platform_dpi_scale_factor(void);
```

### 渲染

```c
uint32_t rw_set_render_aa_samples_per_axis(uint32_t samples);
uint32_t rw_get_render_aa_samples_per_axis(void);
```

### 嵌入式引擎

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

### HarmonyOS 橋接（實驗性）

```c
bool rw_harmony_bind_node(uint64_t node_handle, uint64_t widget_id);
void rw_harmony_clear_node_bindings(void);
uint64_t rw_harmony_lookup_widget_id(uint64_t node_handle);
bool rw_harmony_on_click(uint64_t widget_id);
bool rw_harmony_on_value_changed(uint64_t widget_id);
bool rw_harmony_on_widget_event(uint64_t widget_id, uint32_t kind_code);
bool rw_harmony_unbind_node(uint64_t node_handle);
```

### 錯誤處理（C）

```c
const char* rw_error_message(uint64_t handle);
int32_t rw_error_code(uint64_t handle);
```

### 記憶體管理（C）

```c
void rw_free_string(char* s);
void rw_free_rust_string(char* s);
```

### 繫結狀態

```c
uint32_t rw_bindings_api_version(void);
uint32_t rw_cpp_binding_status(void);
uint32_t rw_java_binding_status(void);
uint32_t rw_python_binding_status(void);
```

### 行動裝置

```c
bool rw_mobile_attach_native_view(uint64_t native_handle);
const char* rw_mobile_backend_name(void);
```

---

## 座標系統參考

所有渲染和控制項定位均使用**螢幕座標系統**：

```
(0, 0) -------------> X（向右增加）
  |
  |    螢幕空間（像素）
  |    原點：左上角
  |
  v Y（向下增加）
```

**圖表座標**使用笛卡爾座標（Y 向上增加），會自動轉換。
**PDF 座標**使用左下角原點，在渲染期間會自動轉換。
**SVG** 使用與螢幕座標相同的左上角原點 — 無需轉換。

`core::coords` 中的輔助函式：

| 函式 | 用途 |
|---|---|
| `to_screen_y(cartesian_y, height)` | 笛卡爾 → 螢幕 Y |
| `to_cartesian_y(screen_y, height)` | 螢幕 → 笛卡爾 Y |
| `to_pdf_y(screen_y, page_height)` | 螢幕 → PDF Y |

---

## 樣式繼承鏈

```
1. 全域主題預設值      (ThemeManager → Theme)
2. 主題覆蓋            （每個控制項類別，例如 "Button", "Label"）
3. 控制項實例狀態      （設定在個別控制項上的 WidgetStyle）
4. 內聯樣式覆蓋        （未來）
```

每一層若未設定則會向下尋找。使用 `WidgetStyle::inherit_from()`
來手動組合樣式。

---

## 執行緒安全注意事項

| 型別類別 | 安全性 |
|---|---|
| 控制項控制代碼 | `Send + Sync`（由 `ObjectId` u64 支援） |
| 平台後端 | `Send + Sync` |
| 信號/槽 | `Send + Sync`（信號），閉包必須為 `Send + 'static` |
| 渲染後端 | 每個表面單執行緒存取 |
| 事件迴圈 | 單執行緒（事件迴圈執行緒） |
| I18nManager | `Send`（使用 RwLock 的全域單例） |
| ThemeManager | `Send` |
| ObjectId | `Copy + Send + Sync` |
| ArenaAllocator | `Send` |
| MemoryMonitor | `Send + Sync` |

---

## 最低支援的 Rust 版本（MSRV）

**Rust 1.87** — 需要 `edition = "2021"` 和目前的依賴版本。
