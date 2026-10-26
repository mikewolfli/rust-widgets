# API 參考

本章提供完整的、按模組劃分的 `rust_widgets` 公開 API 參考文件。當您需要為任務查找正確的型別、函式或特徵時，可將其用作快速查閱手冊。

此處記載的函式庫版本為 **2.9.3**。程式碼範例假設使用 `use rust_widgets::*;` 或如所示使用顯式路徑。

---

## 目錄

1. [頂層函式](#頂層函式)
2. [應用生命週期 (`app`)](#應用生命週期-app)
3. [核心型別 (`core`)](#核心型別-core)
4. [控制項系統 (`widget`)](#控制項系統-widget)
5. [控制項屬性 (`WidgetProperties`)](#控制項屬性)
6. [佈局系統 (`layout`)](#佈局系統-layout)
7. [事件系統 (`event`)](#事件系統-event)
8. [渲染系統 (`render`)](#渲染系統-render)
9. [渲染引擎 (`render_engine`)](#渲染引擎-render_engine)
10. [樣式與主題 (`style`, `theme`)](#樣式與主題-style-theme)
11. [平台抽象 (`platform`)](#平台抽象-platform)
12. [錯誤系統 (`error`)](#錯誤系統-error)
13. [動作框架 (`action`)](#動作框架-action)
14. [快捷鍵系統 (`shortcut`)](#快捷鍵系統-shortcut)
15. [資料繫結 (`data_binding`)](#資料繫結-data_binding)
16. [信號/槽 (`signal`)](#信號槽-signal)
17. [國際化 (`i18n`)](#國際化-i18n)
18. [手勢辨識 (`gesture`)](#手勢辨識-gesture)
19. [圖表與資料視覺化 (`chart`)](#圖表與資料視覺化-chart)
20. [PDF 生成 (`pdf`)](#pdf-生成-pdf)
21. [列印 (`print`)](#列印-print)
22. [記憶體管理 (`memory`)](#記憶體管理-memory)
23. [效能 (`performance`)](#效能-performance)
24. [自適應品質 (`quality`)](#自適應品質-quality)
25. [控制後端 (`control_backend`)](#控制後端-control_backend)
26. [物件系統 (`object`)](#物件系統-object)
27. [Web 能力 (`web`)](#web-能力-web)
28. [復原/重做 (`undo`)](#復原重做-undo)
29. [剪貼簿 (`clipboard`)](#剪貼簿-clipboard)
30. [GPU 加速 (`gpu`, `wgpu_backend`)](#gpu-加速-gpu-wgpu_backend)
31. [嵌入式支援 (`embedded`)](#嵌入式支援-embedded)
32. [語言繫結 (`bindings`)](#語言繫結-bindings)
33. [媒體模組 (`audio`、`video`、`image`)](#媒體模組-audio-video-image)
34. [功能旗標參考](#功能旗標參考)
35. [錯誤碼參考](#錯誤碼參考)
36. [FFI / C ABI 參考](#ffi--c-abi-參考)

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
| `create_frame(parent, x, y, w, h)` | Frame | 為其子控件繪製邊框/框架 |
| `create_dialog(parent, title, x, y, w, h)` | Dialog | 通用帶標題對話方塊，承載一個內容控件 |
| `create_message_box(parent, title, text, x, y, w, h)` | MessageBox | 模態訊息對話方塊 |
| `create_file_dialog(parent, title, x, y, w, h)` | FileDialog | 檔案選擇器 |
| `create_color_dialog(parent, title, x, y, w, h)` | ColorDialog | 色彩選擇器 |
| `create_font_dialog(parent, title, x, y, w, h)` | FontDialog | 字型選擇器 |
| `create_spin_box(parent, x, y, w, h)` | SpinBox | 數值微調控制項 |
| `create_list_view(parent, x, y, w, h)` | ListView | 表格樣式清單 |
| `create_tree_table(parent, x, y, w, h)` | TreeTable | 樹狀列 + 表格欄 |
| `create_breadcrumb(parent, x, y, w, h)` | Breadcrumb | 分段導覽路徑 |
| `create_signature_pad(parent, x, y, w, h)` | SignaturePad | 手寫簽名擷取面 |
| `create_drop_zone(parent, x, y, w, h)` | DropZone | 具名拖放目標 |
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
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>;
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

### 事件迴圈組裝

`EventLoop` 要能投遞任何事件，必須先具備三個掛鉤：控制項分派函式、父級查找與快速鍵分派器。只安裝其中一部分會得到一個「看起來已接線」卻靜默丟掉某個角色的迴圈，因此 `app` 提供一次呼叫裝齊整套。

```rust
pub fn assemble_event_loop(event_loop: &mut EventLoop, runtime: &ActionRuntime);

pub fn assemble_event_loop_with(
    event_loop: &mut EventLoop,
    runtime: &ActionRuntime,
    parent_lookup: Option<Box<dyn Fn(ObjectId) -> Option<ObjectId> + Send + Sync>>,
    dispatch_fn: Option<EventDispatchFn>,
);
```

在建構迴圈之後、啟動之前呼叫一次：執行緒化迴圈在建立執行緒時捕獲這些閉包。`assemble_event_loop_with` 在對應參數為 `None` 時保留預設的父級查找與分派函式，並始終從 `runtime` 取得快速鍵掛鉤。需要不同快速鍵策略的宿主可在呼叫返回後再安裝自己的實作。

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

`WidgetKind` 列舉系統中的每一種控制項型別（裝置設定檔下共 228 種）。選取的部分變體：

- `Button`, `CheckBox`, `RadioButton`, `Label`
- `LineEdit`, `TextArea`, `ComboBox`, `ListBox`, `SpinBox`, `Dropdown`
- `Slider`, `ProgressBar`, `ScrollBar`, `Spinner`, `Meter`, `Arc`, `Roller`
- `ImageView`, `MiniCanvas`, `MiniChart`, `Line`, `LCDNumber`
- `GroupBox`, `Frame`, `ScrollArea`, `Splitter`, `TabWidget`, `StackedWidget`
- `CollapsiblePane`, `DockWidget`, `MdiArea`, `ToolBox`
- `Window`
- `ToggleButton`, `Switch`
- `Calendar`, `DateEdit`, `TimeEdit`, `DateTimeEdit`, `Dial`
- `KeySequenceEdit`, `PieMenu`, `RibbonBar`, `TabBar`
- `Menu`, `MenuBar`, `StatusBar`, `ToolBar`, `ToolButton`
- `Dialog`, `MessageBox`, `FileDialog`, `ColorDialog`, `FontDialog`, `InputDialog`
- `ProgressDialog`, `PopupWindow`
- `ListView`, `TreeView`, `TreeTable`, `Table`, `DataGrid`, `VirtualList`, `VirtualTable`
- `Breadcrumb`, `SignaturePad`, `DropZone`
- `WebEngineView`, `WebEnginePage`
- （完整清單見控制項系統章節）

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
| `Dialog` | 通用帶標題對話方塊，承載一個內容控件；預設為模態 |
| `MessageBox` | 模態訊息對話方塊 |
| `FileDialog`（別名 `DirectoryDialog`） | 檔案/目錄選擇器 |
| `ColorDialog` | 色彩選擇器 |
| `FontDialog` | 字型選擇器 |
| `InputDialog` | 輸入提示對話方塊 |
| `ProgressDialog` | 進度模態視窗 |
| `PopupWindow` | 僅含外框的彈出層（標題列 + 內容區） |

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
| `chart_widgets/` | `BarChart`, `LineChart`, `PieChart`, `Sparkline` + 共用的圖表繪圖引擎（`charts`, `layout`, `svg`, `types`, `adapter`） |
| `media_widgets/` | `AnimatedImage`, `AudioVisualizer`, `CameraPreview`, `HeroAnimation`, `LottieWidget`, `RiveWidget`, `VideoPlayer` |
| `overlay_widgets/` | `FAB`, `PullToRefresh`, `RefreshControl`, `SwipeToDismiss` |
| `cupertino/` | `CupertinoAlertDialog`, `CupertinoDatePicker`, `CupertinoNavigationBar`, `CupertinoSegmentedControl`, `CupertinoSlider`, `CupertinoSwitch`, `MaterialNavigationRail`, `MaterialSnackbar` |
| `misc_widgets/` | `Avatar`, `BarcodeScanner`, `BezierCurveEditor`, `DateRangePicker`, `DropZone`, `MobileDatePicker`, `QRCode`, `SegmentedButton` |
| `input_widgets/`（擴展） | `AutoCompleteEdit`, `EditableComboBox`, `ImePreedit`, `InplaceEditor`, `MaskedEdit`, `MultiSelectComboBox`, `RangeSlider`, `SearchBar`, `SearchBox`, `ShortcutEditor`, `TagInput` |
| `display_widgets/`（擴展） | `Badge`, `ColorHistory`, `ColorWell`, `Divider`, `EmptyState`, `FloatingLabel`, `FontPreview`, `Icon`, `ProgressCircle`, `Rating`, `SkeletonLoader`, `Switch` |
| `container_widgets/`（擴展） | `Carousel`, `MasonryLayout`, `SafeArea`, `Stepper` |
| `dialog/`（擴展） | `BottomSheet`, `Dialog`, `FindReplaceDialog`, `ModalBottomSheet`, `Popover`, `Tooltip`, `WizardDialog` |
| `menu_toolbar/`（擴展） | `DropdownMenu`, `MenuButton` |
| `special_widgets/`（擴展） | `Breadcrumb`, `SignaturePad`, `TimelineWidget` |
| `view_widgets/`（擴展） | `ImageGallery`, `PropertiesPanel`, `PropertyGrid`, `TreeTable` |

### 伴隨控制項 *(非 mini)*

伴隨控制項（companion）是主控制項的夥伴：每個控制項只承擔主控制項旁邊的一塊介面 —— 按鈕的選單面板、媒體播放器的佇列清單 —— 並把每一次使用者意圖表達為一個信號。它們不在能力屬性表中，因此透過下表所列的模組存取，而不是透過 `WidgetFactory`。

| 伴隨控制項 | 模組 | 用途 |
|---|---|---|
| `MenuPanel` | `companions` | 錨定的下拉列清單；發出 `item_clicked`、`highlighted_changed`、`cancelled` |
| `SpeedDial` | `companions` | `FAB` 旁可展開的次要動作堆疊 |
| `SwipeActions` | `companions` | 滑動列的多動作背景，分為 `leading` 與 `trailing` 兩側 |
| `DrawerSectionHeader` | `companions` | 為抽屜項目分組的不可選取標題列 |
| `LayerPanel` | `companions` | `Canvas` 圖層的唯讀鏡像，以圖層 id 定址 |
| `PlaylistPanel` | `companions` | `MediaPlayer` 的佇列清單；信號中的索引是完整清單中的位置 |
| `StickyHeader` | `container_widgets::sticky_header` | `ScrollArea` 黏性帶承載的內容；透過 `ScrollArea::set_sticky_header` 掛載 |
| `TagGroup` | `display_widgets::tag_group` | 佈局並選取真實 `Tag` 子控制項的容器 |
| `Chip` | `input_widgets::input_companions` | 單一可移除、帶拖曳狀態的 chip；列容器是 `TagGroup`，多項目膠囊控制項是 `special_widgets` 中另一個 `Chip` |
| `StepButtons` | `input_widgets::input_companions` | 與數字欄位配對的增減按鈕 |
| `Scrollbar` | `input_widgets::input_companions` | 編輯器使用的滾動指示條；與 `ScrollBar` 控制項不同 |
| `FormatToolbar` | `input_widgets::input_companions` | 富文字編輯器的加粗/斜體/……動作集 |
| `SuggestionPopup` | `input_widgets::suggestion_popup` | 組合框與補全欄位的候選清單，兩階段導航 |
| `ValidationBubble` | `input_widgets::validation_bubble` | 錨定在欄位旁的驗證訊息，攜帶嚴重層級 |
| `RecentValuesPopup` | `input_widgets::recent_values_popup` | 去重、容量受限的已提交值歷史 |
| `ChartCrosshair` | `companions` | 圖表指標下的參考線與坐標軸標籤 |
| `GridFindBar` | `view_widgets::grid_find_bar` | 跨表格欄的增量尋找 |
| `ColumnChooser` | `view_widgets::column_chooser` | 顯示/隱藏並重排表格欄 |
| `ThumbnailCache` | `view_widgets::thumbnail_cache` | 有界 LRU 的降取樣點陣圖快取；本身不繪製任何內容 |
| `ModalHost` | `dialog::modal_host` | 決定哪個模態在最上層、關閉請求路由到何處的模態堆疊 |
| `MediaTransportBar` | `media_widgets::media_transport` | 媒體控制項的播放/暫停、進度、音量與全螢幕列 |
| `MediaScrubber` | `media_widgets::media_scrubber` | 帶緩衝段與章節標記的進度軌道 |
| `WebFindBar` | `web_widgets::find_bar` | Web 檢視的尋找列 |
| `SubtitleBar` | `media_widgets::subtitle_bar` | 自動換行的多行字幕帶 |
| `ValueSpinBox` | `macos::value_spinbox` | 在同一控制項內提供可編輯數字欄位與步進箭頭 |
| `PopupItemList` | `macos::popup_item_list` | 在某一點開啟的小型帶邊框項目清單 |
| `CupertinoNavigationStack` | `cupertino::navigation_stack` | 與 `CupertinoNavigationBar` 配對的標題堆疊；發出 `popped` 與 `stack_changed` |

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
impl WidgetFactory {
    pub fn create(&self, kind_or_name: &str, geometry: Rect, text: &str) -> Option<Box<dyn Widget>>;
    pub fn capability(&self, kind_or_name: &str) -> Option<&WidgetCapability>;
    pub fn read_property(&self, widget: &dyn Widget, name: &str) -> Result<CapabilityValue, CapabilityAccessError>;
    pub fn write_property(&self, widget: &mut dyn Widget, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError>;
}

pub struct WidgetCapability {
    pub kind: WidgetKind,
    pub properties: &'static [PropertySchema],
    pub events: &'static [EventSchema],
    pub commands: &'static [&'static str],
}

pub enum CapabilityValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Color(Color),
    Rect(Rect),
    /// 複合酬載：元組、清單或混合群組；各分量保留各自的型別。
    Tuple(Vec<CapabilityValue>),
    /// 有序且以 id 為鍵的記錄清單——集合屬性。見「控制項系統」章「集合屬性」一節。
    Records(RecordList),
}

pub struct PropertySchema {
    pub name: &'static str,
    pub value_kind: PropertyValueKind,
    pub readable: bool,
    pub writable: bool,
    pub accepted_tokens: &'static [&'static str],
    pub constraints: &'static [Constraint],
    pub record_schema: Option<&'static RecordSchema>,
}

pub enum PropertyValueKind {
    Bool, Int, UInt, Float,
    Number,          // 整數或小數，由另一個屬性決定
    String, Enum, Color, Rect,
    Records,         // 形狀由 `PropertySchema::record_schema` 宣告
}

pub enum CapabilityAccessError {
    UnknownWidget,
    UnknownProperty,
    ReadOnlyProperty,
    TypeMismatch,
    UnsupportedOnWidget,
    OutOfRange,
    UnknownCommand,
}
```

### 控制項狀態持久化 *(2.9.1)*

`SaveState` 是「工作階段恢復」的選擇性加入部分：控件宣告哪些欄位屬於使用者狀態、用哪個格式版本，框架只負責搬運位元組。不會自動序列化任何東西；鍵由宿主決定 —— `ObjectId` 在執行時分配，無法跨重啟存活。

```rust
use rust_widgets::{collect_save_state, restore_save_state, SavedState};

// 由宿主為關心的實例命名。
let key_of = |id| {
    if id == splitter_id { Some("workspace.split".to_string()) }
    else if id == tabs_id { Some("workspace.tabs".to_string()) }
    else { None }
};

let saved: Vec<SavedState> = collect_save_state(window_id, key_of);
// ……把 `saved` 存進你自己的設定儲存……
let applied = restore_save_state(window_id, &saved, key_of);
```

遍歷會同時涵蓋控件樹子節點與已掛載 surface，因此用 `mount_surface` 託管的控件不會被遺漏。條目按 key 排序，並攜帶控件的格式版本；版本不符會被拒絕而不是誤讀，損壞的位元組不會改動控件。

已接入的控件：`Splitter`（分欄佈局）、`TabWidget`（選取頁）、`DockWidget` 與 `CollapsiblePane`（既有 blob）、`Pagination`（每頁筆數與目前頁）。控件透過實作 `SaveState` 並在 `Widget::save_state_dyn` / `save_state_dyn_mut` 回傳 `Some(self)` 來加入。

`Pagination` 另新增兩個可選單元：`set_show_jump_input(true)` 增加「Go」跳頁輸入（輸入數字後按 Enter），`set_show_page_size_selector(true)` 搭配 `set_page_size_options` 增加每頁筆數選單。兩者預設關閉。

`BarChart::set_animate_value_changes(true)` 會把資料更新從螢幕上的目前值緩動到新值，而不是直接跳變；`false` 保持原本的立即更新行為。

### 控制項渲染（離屏）（`widget::runtime`）

一個視窗的畫面由兩部分組成：控制項樹，**以及**掛載在它下面的每個面。`Platform::mount_surface` 給掛載的控制項一個自己的原生視圖，而不是把它變成樹子節點，平台單獨繪製這個視圖——所以只畫樹會在該控制項所在處留下一個空洞。下面這幾個入口讓測試、文件建置或 `examples/` 二進位不必開啟視窗就能得到真實畫面：

```rust
use rust_widgets::core::{Color, Size};
use rust_widgets::widget::runtime;

// 平台實際呈現的整個視窗：先樹，再疊上每個掛載面。
let frame = runtime::render_window_frame(window_id, Size::new(1180, 760), Color::WHITE);

// 只畫樹，不疊任何面。
let tree = runtime::render_frame_tree(window_id, Size::new(1180, 760), Color::WHITE);

// 單一控制項；epoch 未變時重用快取幀，即面視圖所用的那條路徑。
let control = runtime::render_frame_cached(control_id, Size::new(320, 200), Color::TRANSPARENT);

// 這個視窗下有哪些控制項被掛載為面。
let surfaces = runtime::surfaces_of(window_id);
```

三個渲染呼叫都回傳 `Option<Vec<u8>>`：id 未掛載或任一維度為 `0` 時是 `None`，否則是以列儲存的直通（非預乘）RGBA8，每列 `width * 4` 位元組。`render_window_frame` 把每個面自己的幀按該控制項的 `geometry()` 合成到樹上，因此沒有面覆蓋的區域與樹幀逐位元組一致。面可能在自己的矩形裡留空（透明），所以這裡是**合成**而不是複製。

要「像截圖一樣的畫面」用 `render_window_frame`；只有視窗沒有掛載任何面時 `render_frame_tree` 才是完整的。兩者都必須在 UI 執行緒呼叫，`render_frame_cached` 的幀會留在幀快取裡直到下一個 epoch。

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

## 控制項屬性

自 2.0.0 起，每個控件都會公佈自己的屬性合約。你可以在不知道控件具體型別的情況下，讀取、寫入並**列舉**控件的狀態 —— 同一份程式碼可套用於任何控件、任何平台。

### 特徵

```rust
pub trait WidgetProperties {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError>;
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError>;

    /// Names this control exposes, including the four it inherits.
    /// A `&'static [&'static str]`, so enumeration allocates nothing.
    fn property_names(&self) -> &'static [&'static str];
}
```

### 四個共用屬性

每個控件都能透過 `base_property_get` / `base_property_set` 回答這些屬性：

```rust
pub const BASE_PROPERTY_NAMES: &[&str] = &["enabled", "visible", "tooltip", "geometry"];
```

`geometry` 在這個合約中刻意設計為**唯讀**：控件的矩形由擺放它的佈局所擁有，因此在這裡寫入會在下一輪佈局時默默與佈局互相拉扯。若要移動控件，請使用 `widget::runtime::set_geometry`。

### 值型別

```rust
pub enum CapabilityValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Null,
}
```

**沒有**任何 `From` 轉換可以產生 `CapabilityValue` —— 請明確建構變體（`CapabilityValue::Int(7)`）。這是刻意的：隱式轉換會讓型別不符在呼叫端變得不可見。

### 以參考讀取

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::WidgetFactory;
use rust_widgets::CapabilityValue;

let factory = WidgetFactory::new_with_defaults();
let mut slider = factory.create("slider", Rect::new(0, 0, 200, 24), "").unwrap();

factory.write_property(slider.as_mut(), "value", CapabilityValue::Int(42))?;
let value = factory.read_property(slider.as_ref(), "value")?;
assert_eq!(value, CapabilityValue::Int(42));
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

### 列舉合約

這是用來建構屬性編輯器、序列化器或文件表格的 API：

```rust
use rust_widgets::widget::{widget_property_get, widget_property_names};

for name in widget_property_names(slider.as_ref()).unwrap_or(&[]) {
    println!("{name} = {:?}", widget_property_get(slider.as_ref(), name)?);
}
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

由於這份清單來自控件本身，因此不可能過時；而且只要控件公佈了它不願回答的屬性，測試（`no_published_property_answers_unknown_when_written`）就會以屬性名稱失敗。

### 以 id 讀取

後端與腳本宿主經常持有的是 id 而非參考。這些存取子會透過 widget runtime 解析 id，因此**控件必須先完成註冊**，而且 `runtime::register` 會**指派**它往後據以回答的 id —— 請使用它的回傳值，而不是 factory 發出的那個：

```rust
use rust_widgets::widget::{read_widget_property_by_id, write_widget_property_by_id};
use rust_widgets::widget::runtime;

let id = runtime::register(widget).expect("must run on the UI thread");
write_widget_property_by_id(id, "value", CapabilityValue::Int(7))?;
let value = read_widget_property_by_id(id, "value")?;

runtime::unregister(id);
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

### 錯誤語意

| 錯誤 | 含義 |
|---|---|
| `UnknownProperty` | 此控件**沒有這個名稱的屬性** —— 呼叫端的錯誤。 |
| `ReadOnlyProperty` | 屬性**存在**但不可寫入（例如 `geometry`、`row_count`）。請繪製為停用狀態的欄位。 |
| `TypeMismatch` | 值型別錯誤，或值超出範圍。 |
| `UnsupportedOnWidget` | 此控件完全沒有屬性合約。在 2.0.0 中不應發生。 |
| `UnknownWidget` | 該 id 未指向任何東西（僅適用於以 id 存取的方式）。 |

### 為自己的控件加入合約

```rust
impl WidgetProperties for MyControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "progress" => Ok(CapabilityValue::Float(self.progress())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "progress" => {
                self.set_progress(expect_f64(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["progress", BASE_PROPERTY_NAMES]
    }
}
```

並在它的 `impl Widget` 區塊中、`impl_draw_bridge!();` 的旁邊：

```rust
impl_widget_property_hooks!();
```

此合約必須滿足的規則（每一條都有測試把關）：

1. **`property_names()` 中的每個名稱都必須能由 `get` 讀取。**
2. `property_names()` 必須以 `BASE_PROPERTY_NAMES` 結尾。
3. **沒有 setter 的名稱必須回傳 `ReadOnlyProperty`，絕不可回傳 `UnknownProperty`。**
   `UnknownProperty` 代表「這個名稱不存在」。
4. 透過控件自身的存取子讀取真實狀態，絕不要使用硬編碼的常數。

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
| `BorderLayout` | `border` | 固定邊緣軌道（`north`/`south`/`west`/`east`）加彈性中心區 |
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

pub enum EventPriority { High, Normal, Idle }
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
    // Timers are not an `EventLoop` method: `TimerManager` owns them and posts
    // ticks into the loop's queue. See the Timer section below.
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
    pub fn focus_next(&mut self) -> Option<ObjectId>;
    pub fn focus_previous(&mut self) -> Option<ObjectId>;
    /// Replaces the focus traversal order wholesale.
    pub fn set_focus_order(&mut self, ids: Vec<ObjectId>);
    pub fn register_focusable(&mut self, id: ObjectId);
    pub fn unregister_focusable(&mut self, id: ObjectId);
    pub fn focusable_widgets(&self) -> &[ObjectId];
    pub fn set_traversal_strategy(&mut self, strategy: FocusTraversalStrategy);
    pub fn set_a11y_callback(&mut self, callback: Box<dyn FnMut(ObjectId)>);
}
```

### 指標捕獲

```rust
pub struct PointerCaptureManager { /* ... */ }

impl PointerCaptureManager {
    pub fn new() -> Self;
    /// Captures the pointer for `widget_id`. `false` when another widget already
    /// holds the capture -- capture is exclusive.
    pub fn set_capture(&mut self, widget_id: ObjectId) -> bool;
    /// Releases the capture. `false` if nothing was captured.
    pub fn release_capture(&mut self) -> bool;
    /// The widget holding the capture, if any.
    pub fn capturing_widget(&self) -> Option<ObjectId>;
    /// Whether `widget_id` is the current captor.
    pub fn has_capture(&self, widget_id: ObjectId) -> bool;
}
```

### 計時器

```rust
pub struct TimerManager { /* ... */ }

impl TimerManager {
    /// Takes the sender the timer callbacks are delivered through: the manager
    /// does not own a loop, it posts into one.
    pub fn new(sender: EventSender) -> Self;
    /// Starts a timer on `target`; the tick is delivered as an event.
    pub fn start_timer(&self, target: ObjectId, interval: Duration, id: u32, repeat: bool) -> bool;
    /// Stops one timer on `target`. `false` if no such timer was running.
    pub fn stop_timer(&self, target: ObjectId, id: u32) -> bool;
    /// Stops every timer belonging to `target`; returns how many were stopped.
    pub fn stop_timers_for_target(&self, target: ObjectId) -> usize;
    /// Stops every timer on every target.
    pub fn clear(&self);
    /// Delivers ticks for all timers whose deadline has passed. Call once per
    /// loop iteration.
    pub fn pump(&self);
}
```

### 佇列工具函式

```rust
pub struct EventQueue { /* ... */ }
impl EventQueue {
    pub fn new() -> Self;
    pub fn sender(&self) -> EventSender;
    pub fn pop(&self) -> Option<Event>;
}

impl EventSender {
    pub fn send(&self, event: Event);
}

pub enum QueueError { Full, Empty, Closed }
```

> `FixedSizeQueue` / `PriorityQueue` / `BoundedQueue` 及 `DEFAULT_QUEUE_CAPACITY`
> 已刪除（零消費者，與 `EventQueue` 重複）。

### 動畫幀請求

```rust
pub struct AnimationFrameRequest { /* ... */ }
impl AnimationFrameRequest {
    pub fn new(callback: Box<dyn FnMut()>) -> Self;
    pub fn request(&mut self);
    pub fn cancel(&mut self);
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
    pub fn push_clip(&mut self, x: i32, y: i32, w: u32, h: u32);
    pub fn pop_clip(&mut self);
    pub fn execute(&mut self, cmd: &RenderCommand);
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

文字截斷是渲染後端的能力（`RenderContext::draw_text_fitted`），不是獨立函式。
`TextOverflow` / `TextClamp` / `apply_text_overflow` / `apply_text_clamp` 已刪除
（零消費者，與 `fit_text_to_width` 重複）。

### 字素叢集

```rust
/// 遍歷文字的叢集（「使用者感知的一個字元」），回呼得到內容與位元組範圍。
pub fn for_each_cluster(text: &str, visit: impl FnMut(&str, (usize, usize)));
```

`GraphemeCluster` / `GraphemeProcessor` 已刪除（與 `for_each_cluster` 重複）。
`is_combining_mark` / `is_variation_selector` 保留在 `render::grapheme`。

### 路徑幾何

```rust
/// 解析 SVG path data（`d`）為曲線段。支援 `M/L/H/V/C/Q/A/Z` 及全部相對形式。
/// 畸形輸入回傳 `Err`，不 panic。展平由 `render::text::raster` 負責。
pub fn parse(input: &str) -> Result<Vec<Subpath>, PathError>;

pub struct Point { pub x: f32, pub y: f32 }
pub enum Segment {
    Line(Point),
    Quad { ctrl: Point, to: Point },
    Cubic { ctrl1: Point, ctrl2: Point, to: Point },
    Arc { rx: f32, ry: f32, x_rotation: f32, large_arc: bool, sweep: bool, to: Point },
}
pub struct Subpath { pub start: Point, pub segments: Vec<Segment>, pub closed: bool }
pub enum PathError { UnknownCommand(char), ExpectedNumber, SegmentsBeforeMove, TooManyPoints }
pub const MAX_PATH_POINTS: usize = 1024;
```

### SVG 渲染

```rust
pub struct SvgPaintBackend { /* ... */ }
impl SvgPaintBackend {
    pub fn new(size: Size) -> Self;
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
// The trait is `RenderEngine` (src/render_engine/engine_trait.rs). It borrows
// `&self`, not `&mut self`, and takes no `Result`: engine lifecycle is
// infallible by design, so there is nothing for a caller to handle.
pub trait RenderEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn profile(&self) -> RuntimeProfile;
    fn init(&self);
    fn run(&self);
    fn quit(&self);
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64;
    fn create_button(
        &self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32,
    ) -> u64;
}
```

### 原生引擎

```rust
pub struct NativeRenderEngine;

impl NativeRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for NativeRenderEngine { /* ... */ }
```

### 嵌入式引擎

```rust
pub struct EmbeddedRenderEngine;

impl EmbeddedRenderEngine {
    pub const fn new() -> Self;
}
impl RenderEngine for EmbeddedRenderEngine { /* ... */ }

// The engine itself is a thin façade: the counters and the frame loop live in the
// process-wide embedded runtime (`render_engine::embedded`), which is what the
// engine methods delegate to. Task submission is a free function, not a method:
// `submit_embedded_task(label, callback)`.

/// Builds the engine for the compile-time profile.
///
/// `NativeRenderEngine` when an OS host exists, `EmbeddedRenderEngine` otherwise
/// (and always under the alloc-frugal `mini` profile).
pub fn default_render_engine() -> Box<dyn RenderEngine>;
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
    pub corner_radii: Option<CornerRadii>,
    pub padding: Padding,
    pub margin: Margin,
    pub shadow: Option<Shadow>,
    pub touch_target: Option<Size>,
    pub opacity: Option<f32>,
}
```

建構子方法：`with_background(c)`, `with_text_color(c)`, `with_font(f)`,
`with_border(c, w, r)`, `with_corner_radii(r)`, `with_padding(p)`, `with_margin(m)`,
`with_shadow(s)`, `with_touch_target(s)`, `with_gradient(g)`, `with_opacity(o)`。

實例方法：`inherit_from(parent)`, `merge(other)`, `resolved_corner_radii()`。

### 內邊距與外邊距

```rust
pub struct Padding { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
pub struct Margin  { pub top: u32, pub right: u32, pub bottom: u32, pub left: u32 }
```

兩者都支援：`new(top, right, bottom, left)`, `all(value)`, `symmetric(v, h)`,
`normalized(top, right, bottom, left)` — 負值會被限制為 0。

### 圓角

```rust
pub struct CornerRadii {
    pub top_left: u32,
    pub top_right: u32,
    pub bottom_right: u32,
    pub bottom_left: u32,
}

impl CornerRadii {
    pub const fn new(top_left: u32, top_right: u32, bottom_right: u32, bottom_left: u32) -> Self;
    pub const fn all(radius: u32) -> Self;
    pub const fn is_uniform(&self) -> bool;
}
```

`WidgetStyle::border_radius` 是純量寫法，含義仍然是「四角相同」。當
`WidgetStyle::corner_radii` 也被設定時，逐角值優先；請透過
`WidgetStyle::resolved_corner_radii` 讀取解析後的形狀，而不是直接讀任一欄位。
`CornerRadii::new` 的參數順序與 CSS `border-radius` 一致：左上、右上、右下、左下。

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
pub struct CssParser { /* ... */ }
pub struct Selector { /* ... */ }

// 熱重載 CSS 監視器
pub struct AssetWatcher (crate::asset::watcher) { /* ... */ }
impl AssetWatcher (crate::asset::watcher) {
    pub fn watch(path: &str) -> Result<Self, ()>;
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
    pub icons: IconTokens,
    // ...
}

impl Theme {
    pub fn new(name: &str) -> Self;
    pub fn dark() -> Self;                   // 內建深色主題
    pub fn light() -> Self;                  // 內建淺色主題
    pub fn icon_size(&self, slot: IconSlot) -> u32;   // 槽位的標稱邊長（像素）
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
    pub fn icon_size(&self, slot: IconSlot) -> Option<u32>;
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
    pub small: u32,
    pub medium: u32,
    pub large: u32,
    pub extra_large: u32,
}

pub struct Borders {
    pub width: u32,
    pub radius: u32,
    pub shadow: bool,
}

pub struct IconTokens {
    pub small: u32,    // 16 px — 內聯與工具列圖示
    pub medium: u32,   // 18 px — 按鈕圖示
    pub large: u32,    // 32 px — 功能區或空狀態圖示
}

pub enum IconSlot { Small, Medium, Large }

pub struct PaddingToken {
    pub all: Option<u32>,
    pub vertical: Option<u32>,
    pub horizontal: Option<u32>,
    pub top: Option<u32>,
    pub right: Option<u32>,
    pub bottom: Option<u32>,
    pub left: Option<u32>,
}
```

### 主題樣式代幣

```rust
pub struct ThemeStyleToken {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub border: Option<Color>,
    pub border_width: Option<u32>,
    pub radius: Option<u32>,
    pub padding: Option<PaddingToken>,
    pub font: Option<Font>,
    pub opacity: Option<f32>,
    pub shadow: ShadowOverride,
    // ...
}
```

每個欄位都是覆蓋：`None` 保留基礎解析產出的值，因此主題可以只調整某個角色的一個屬性而不必重述其餘部分。
`ThemeStyleToken::padding` 透過 `PaddingToken::resolve` 按與樣式建構子相同的
`邊 → 軸 → 全部 → 0` 級聯解析，因此 `"button": { "padding": { "all": 4 } }` 只移動該角色。
`PaddingToken` 讓主題模式保持在可序列化的原始型別上，而不借用樣式層的內邊距型別。

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
pub struct Keyboard (widget::input_widgets::keyboard) { /* ... */ }
impl Keyboard (widget::input_widgets::keyboard) {
    pub fn show(&mut self);
    pub fn hide(&mut self);
    pub fn is_visible(&self) -> bool;
}
```

### 剪貼簿（平台）

```rust
pub struct Option<&'static dyn crate::platform::clipboard::RichClipboardBackend> { /* ... */ }
impl Option<&'static dyn crate::platform::clipboard::RichClipboardBackend> {
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
    pub fn connect_undo_redo_stack(&mut self, stack: Arc<Mutex<UndoManager>>) -> bool;
}
```

### 復原 / 重做

`connect_undo_redo` 把兩個閉包綁定到平台加速鍵上。下面的堆疊型別記錄動作**做了什麼**，因此宿主不必自己搭建歷史。

```rust
pub const DEFAULT_UNDO_LIMIT: usize = 256;
pub const GLOBAL_UNDO_CONTEXT: &str = "";

pub struct Command { /* ... */ }
impl Command {
    pub fn new<U, R>(label: impl Into<String>, undo: U, redo: R) -> Self
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn label(&self) -> &str;
    pub fn is_empty(&self) -> bool;
}

pub struct UndoStack { /* ... */ }
impl UndoStack {
    pub fn new() -> Self;
    pub fn with_limit(limit: usize) -> Self;
    pub fn limit(&self) -> usize;
    pub fn set_limit(&mut self, limit: usize);
    pub fn push(&mut self, command: Command);
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn begin_group(&mut self, label: impl Into<String>);
    pub fn end_group(&mut self) -> bool;
    pub fn is_group_open(&self) -> bool;
    pub fn undo(&mut self) -> Option<String>;
    pub fn redo(&mut self) -> Option<String>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn undo_len(&self) -> usize;
    pub fn redo_len(&self) -> usize;
    pub fn is_dirty(&self) -> bool;
    pub fn mark_saved(&mut self);
    pub fn clear(&mut self);
}

pub struct UndoManager { /* ... */ }
impl UndoManager {
    pub fn new() -> Self;
    pub fn with_limit(limit: usize) -> Self;
    pub fn active_context(&self) -> &str;
    pub fn set_active_context(&mut self, context: impl Into<String>) -> String;
    pub fn stack_for(&self, context: &str) -> Option<&UndoStack>;
    pub fn stack(&self) -> &UndoStack;
    pub fn stack_mut(&mut self) -> &mut UndoStack;
    pub fn contexts(&self) -> Vec<String>;
    pub fn push(&mut self, command: Command);
    pub fn record<U, R>(&mut self, label: impl Into<String>, undo: U, redo: R)
        where U: FnMut() + Send + 'static, R: FnMut() + Send + 'static;
    pub fn begin_group(&mut self, label: impl Into<String>);
    pub fn end_group(&mut self) -> bool;
    pub fn undo(&mut self) -> Option<String>;
    pub fn redo(&mut self) -> Option<String>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn is_dirty(&self) -> bool;
    pub fn mark_saved(&mut self);
    pub fn clear(&mut self);
}
```

用兩個閉包記錄一步，然後重放它：

```rust
let mut stack = UndoStack::new();
stack.record("insert_text", move || remove(), move || insert_again());
if stack.can_undo() {
    stack.undo();
}
```

新命令會清空重做堆疊（歷史已分叉）。`UndoStack::begin_group` 與 `UndoStack::end_group`
把它們之間推入的每條命令合成一步，復原時按相反順序執行；允許巢狀，且只有最外層一對建立邊界。
`UndoManager` 為每個焦點上下文各保留一個堆疊，並把 `undo` / `redo` 路由到活動堆疊；
`GLOBAL_UNDO_CONTEXT` 是初始上下文，因此只用一個堆疊的宿主根本不需要呼叫 `set_active_context`。
`ActionRouter::connect_undo_redo_stack` 把平台復原/重做加速鍵綁定到活動堆疊。

---

## 快捷鍵系統 (`shortcut`)

```rust
pub struct ShortcutManager { /* ... */ }

impl ShortcutManager {
    pub fn new() -> Self;
    pub fn register(&mut self, action_id: impl Into<String>, shortcut: Shortcut, description: impl Into<String>) -> bool;
    pub fn register_in(&mut self, scope: impl Into<String>, action_id: impl Into<String>, shortcut: Shortcut, description: impl Into<String>) -> bool;
    pub fn unregister(&mut self, id: &str) -> bool;
    pub fn trigger(&self, shortcut: &Shortcut) -> bool;
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String>;
    pub fn push_scope(&mut self, scope: impl Into<String>);
    pub fn push_modal_scope(&mut self, scope: impl Into<String>);
    pub fn pop_scope(&mut self) -> Option<String>;
    pub fn active_scope(&self) -> Option<&str>;
    pub fn is_scope_active(&self, scope: &str) -> bool;
    pub fn active_scopes(&self) -> Vec<String>;
    pub fn clear_scopes(&mut self);
    pub fn clear(&mut self);
}
```

`ShortcutEntry::scope` 在全域作用域中為 `None`，當綁定屬於某個上下文時為 `Some(name)`。
作用域綁定僅在它的作用域位於 `push_scope` 推入的堆疊上時生效，並遮蔽同一按鍵組合上的全域綁定。
`push_modal_scope` 還阻止查找到達其下方的所有作用域與全域綁定，這就是對話框能重新綁定 Enter
而不與主視窗衝突的原因。衝突只在**同一作用域內**判定，因此同一按鍵組合可以綁定在不同作用域中。

### 快捷鍵型別

```rust
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

pub struct ShortcutEntry {
    pub action_id: String,
    pub description: String,
    pub shortcut: Shortcut,
    pub enabled: bool,
    pub scope: Option<String>,
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

### 接線查詢

`connect_event` 回傳 `Ok` 只證明事件名已發佈，**不**證明有訂閱者能到達該控制項。兩個執行時查詢回答這個差異。

```rust
pub fn is_wired(widget_id: ObjectId, event_name: &str) -> bool;
pub fn unwired_events(widget_id: ObjectId) -> Option<Vec<&'static str>>;
```

當控制項自身的 `EventSignalRef` 報告 `event_name` 至少有一個存活訂閱者時，`signal::is_wired` 為 `true`。
`signal::unwired_events` 回傳該控制項所有沒有任何存活訂閱者的已發佈事件；當 id 不是已掛載控制項時回傳 `None`
—— 這與表示「全部已接線」的 `Some(vec![])` 是不同的答案。在精簡設定檔中能力表被編譯掉，
因此 `is_wired` 回傳 `false`，`unwired_events` 回傳 `None`。

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
    pub fn process(&mut self, event: &Event, now_ms: u64) -> Option<Event>;
    pub fn reset_all(&mut self);
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

Data visualization lives under `widget::chart_widgets`. The chart *engine* (layout, axes, ticks, SVG context) and the chart *widgets* are two layers of one feature, so they share a module:

```rust
// The engine and the chart widgets both live under `widget::chart_widgets`:
// they are two layers of one feature, and there is no top-level `chart` module.
pub struct ChartLayout { /* ... */ }          // widget::chart_widgets::layout
pub struct SvgChartContext { /* ... */ }      // widget::chart_widgets::svg
pub struct MemoryChartContext { /* ... */ }   // widget::chart_widgets::svg
pub trait Chart { /* ... */ }                 // widget::chart_widgets::types
pub trait ChartContext { /* ... */ }          // widget::chart_widgets::types
// 子模組：charts, layout, svg, types
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
pub struct ObjectPool<T> / SharedPool<T> / PoolManager { /* ... */ }
impl ObjectPool<T> / SharedPool<T> / PoolManager {
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

pub fn render_dirty_regions_into(
    tracker: &mut DirtyRegionTracker,
    ctx: &mut RenderContext,
    regions: &mut Vec<Rect>,
    render_all: impl FnMut(&mut RenderContext),
);
```

`render_dirty_regions_into` 是復用緩衝區的形式：它先清空 `regions`，再用合併後的區域矩形填充它，然後進行分桶遍歷，
因此影格迴圈可以終生只持有一個 `Vec`，無需每幀為區域分配記憶體。當走上「超出容量」的回退路徑時，
單一裁剪由包圍矩形構成，緩衝區則保持為空。

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
    pub fn property_keys(&self) -> Vec<&str>;
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
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&mut self) -> Result<(), String>;
    fn on_unload(&mut self) -> Result<(), String>;
}

pub struct PluginManager { /* ... */ }
impl PluginManager {
    pub fn new() -> Self;
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<u32, String>;
    pub fn unregister(&mut self, id: u32) -> bool;
    pub fn list(&self) -> Vec<&dyn Plugin>;
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
}

pub struct CookieJar { /* ... */ }
impl CookieJar {
    pub fn new() -> Self;
    pub fn add(&mut self, cookie: Cookie);
    pub fn remove(&mut self, domain: &str, name: &str);
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
}
```

---

## 剪貼簿 (`clipboard`)

```rust
pub fn set_clipboard_text(text: &str);
pub fn get_clipboard_text() -> String;
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>;
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

完整的 C API 請參閱下方的 [FFI / C ABI 參考](#ffi--c-abi-參考) 一節。

---

## 媒體模組 (`audio`、`video`、`image`)

這三個模組分別由 `audio`、`video`、`image` 功能旗標啟用。`desktop`、`tablet`、`mobile` 會啟用全部三個；
`mini` 與 `embedded` 不包含。

自 2.9.1 起，所有接收整塊資料的入口都把解碼放在工作執行緒上，影格迴圈不會被長音訊或大圖片阻塞。
每個入口都保留同步形式；所有非同步入口在執行緒無法建立時退回同步解碼——失敗模式是「更慢」，
而不是「永遠載入不出來」。

### 音訊（`audio`）

`AudioEngine` 持有一個已解碼緩衝與一個可選的裝置輸出。

```rust
use rust_widgets::audio::AudioEngine;

let mut engine = AudioEngine::new();
engine.load(&wav_bytes)?;       // 同步：在呼叫執行緒解碼
engine.play();
let played = engine.tick(1024); // 推進位置，回傳消耗的取樣數

// 或在背景下解碼，完成後安裝結果：
engine.load_async(wav_bytes);   // 立即回傳
// 每幀輪詢一次：
if let Some(result) = engine.poll_load() {
    result?;                    // 本次呼叫完成安裝
}
// 或一次性阻塞：
engine.wait_for_load()?;
```

| 方法 | 簽名 | 說明 |
|---|---|---|
| `new` | `fn() -> Self` | 空引擎 |
| `load` | `fn(&mut self, data: &[u8]) -> Result<(), String>` | 同步解碼 |
| `load_async` | `fn(&mut self, data: Vec<u8>)` | 背景下解碼；新呼叫會取代上一次載入 |
| `is_loading` | `fn(&self) -> bool` | 背景下載仍在進行 |
| `poll_load` | `fn(&mut self) -> Option<Result<(), String>>` | 安裝已完成的載入；仍在進行時為 `None` |
| `wait_for_load` | `fn(&mut self) -> Result<(), String>` | 阻塞直到載入完成 |
| `play` / `pause` / `stop` | `fn(&mut self)` | 播放狀態 |
| `set_volume` / `volume` | `fn(&mut self, f32)` / `fn(&self) -> f32` | 鉗制到 `0.0..=1.0` |
| `tick` | `fn(&mut self, samples: usize) -> usize` | 推進位置，到末尾飽和 |
| `current_samples` | `fn(&self, count: usize) -> Vec<f32>` | 当前位置下的取樣 |
| `duration_seconds` | `fn(&self) -> f64` | 未載入時為 `0.0` |
| `play_to_device` | `fn(&mut self) -> Result<(), String>` | 需要 `audio-output`（cpal） |

格式支援由 `AudioFormat` 描述；`symphonia-codecs` 增加 FLAC、OGG Vorbis、AAC 與 Opus 解碼。

### 影片（`video`）

`VideoEngine` 驅動一個 `VideoDecoder` 並持有主時間軸。影格在背景執行緒解碼，因此 `tick_frame`
從有界預讀佇列取幀，而不是在影格迴圈裡解碼。每次 seek 開啟一個新的世代：為舊位置解碼的影格會被
丟棄而不是顯示。

```rust
use rust_widgets::video::VideoEngine;

let mut engine = VideoEngine::open(mp4_bytes)?; // 根據容器選擇解碼器
engine.play();
let frame = engine.tick_frame(16, 0.008)?;      // 一個顯示幀；到期時回傳 Some(frame)
```

| 方法 | 簽名 | 說明 |
|---|---|---|
| `open` | `fn(Vec<u8>) -> Result<Self, String>` | 識別容器；H.264 等需要 `video-codecs` |
| `with_decoder` | `fn(Box<dyn VideoDecoder + Send>) -> Self` | 自帶解碼器 |
| `metadata` / `duration` | `fn(&self) -> &VideoMetadata` / `fn(&self) -> f64` | 容器資訊 |
| `play` / `pause` / `stop` | `fn(&mut self)` | 結束後 `play` 從第一幀重新開始 |
| `seek` | `fn(&mut self, time: f64) -> Result<(), String>` | 舊位置的影格被丟棄 |
| `tick_frame` | `fn(&mut self, delta_ms: u32, tolerance: f64) -> Result<Option<VideoFrame>, String>` | `None` 表示本幀沒有到期畫面 |
| `next_frame` / `step_frame` | `fn(&mut self) -> Result<Option<VideoFrame>, String>` | 走同一個單幀槽位 |
| `take_dropped_frames` | `fn(&mut self) -> u64` | 為追趕進度而丟棄的影格數 |
| `clock` / `clock_mut` | 時間軸存取器 | 主 `MediaClock` |

### 圖片（`image`）

```rust
use rust_widgets::image::{decode, decode_async};

let image = decode(&png_bytes)?;   // 同步
let job = decode_async(png_bytes); // 背景下；回傳 ImageDecodeJob
let image = job.wait()?;           // 或每幀一次 job.try_take()
```

`decode` 回傳 `DecodedImage`（`format`、`data`、`width`、`height`、`exif`、`color_space`）。
`decode_to_rgba8` 一步完成轉換，`decode_animation` 回傳 GIF/WebP 的每一幀及其延遲。
`ImageDecodeJob::try_take` 在解碼仍在進行時回傳 `None`，`wait` 則阻塞到完成。

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
| `mini` | LVGL 風格最小化（約 28 個控制項） | Software, custom controls, `heapless`, `hashbrown`, `spin`, `bumpalo` |

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
| `image` | 圖片解碼/編碼模組 |
| `video` | 影片引擎模組（容器識別、影格解碼、播放） |
| `audio` | 音訊引擎模組（格式識別、解碼、緩衝） |
| `audio-output` | 透過 cpal 輸出至系統裝置 |
| `symphonia-codecs` | FLAC / OGG Vorbis / AAC / Opus 音訊解碼 |
| `video-codecs` | FFmpeg 影片解碼 |
| `pdf-encryption` | 加密 PDF 輸出 |
| `js-engine` | 內嵌 JavaScript 引擎（boa） |
| `web-http` | Web 能力的 HTTP 用戶端（ureq） |
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
// 組合框沒有專用的新增函式：新增項經屬性層，每個持有項的控件都會回應。
bool rw_set_widget_property(uint64_t widget_id, const char* name,
                            int kind, int64_t num, const char* str);

bool rw_combo_box_clear_items(uint64_t combo_box);
int rw_combo_box_current_index(uint64_t combo_box);
bool rw_combo_box_set_current_index(uint64_t combo_box, unsigned int index);
unsigned int rw_combo_box_item_count(uint64_t combo_box);
const char* rw_combo_box_item_text(uint64_t combo_box, unsigned int index);
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

### 佈局（Layout）

```c
#include "rw_generated.h"

// 為 `parent` 建立並儲存佈局。`kind_name` 使用與宣告式文件相同的拼寫
// （"hbox"、"vbox"、"grid"、"stack"、"form"、"flow"、"wrap"、"flex"、
// "splitter"），因此同一個名稱在兩處通用。未知類型會被拒絕，而非預設回退。
bool rw_widget_set_layout(uint64_t parent, const char* kind_name,
                          int spacing, int margin);

// 將子控件註冊進佈局。`stretch` 最小被提升為 1。
bool rw_widget_layout_add(uint64_t parent, uint64_t child, unsigned int stretch);

// 可伸縮的空隙，無需佔位控件。
bool rw_widget_layout_add_spacer(uint64_t parent, unsigned int stretch);

bool rw_widget_layout_remove(uint64_t parent, uint64_t child);
bool rw_widget_layout_clear(uint64_t parent);

// 在給定矩形內重新計算並移動子控件。回傳被定位的子控件數；
// 未儲存佈局時回傳 0。
unsigned int rw_widget_layout_apply(uint64_t parent, int x, int y,
                                    unsigned int width, unsigned int height);

// 佈局持有多少子控件（不實際套用）。
unsigned int rw_widget_layout_child_count(uint64_t parent);
```

### 樣式（Style）

```c
#include "rw_generated.h"

// 套用一條寫作 "property: value" 的樣式宣告，屬性名與取值語法
// 與樣式表一致："background-color: #FF0000"、"border-radius: 4"、"font-size: 14"。
//
// 與樣式表不同——依 CSS 規範，樣式表會忽略不認識的屬性——此處拼錯的屬性
// 會被拒絕，因為這次呼叫明確只指定了一個屬性，靜默會讓拼字錯誤被當作成功。
bool rw_widget_set_style(uint64_t widget_id, const char* declaration);
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

**Rust 1.88** — 需要 `edition = "2021"` 和目前的依賴版本。
