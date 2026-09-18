# 平台支援

rust-widgets 在九個支援的平台上提供統一的 API。本章涵蓋平台抽象層、後端選擇、裝置檢測、剪貼簿、拖放、IME、無障礙、選單、能力協商及虛擬鍵盤支援。

> **請先讀這一段 —— 每個控件都是自繪的。**
>
> 後端**不會**建立作業系統的原生控件。它只擁抱四件事，除此之外別無其他：**繪製面**、**事件迴圈**、**輸入轉譯**，以及**平台服務**（IME、剪貼簿、無障礙、原生選單、檔案對話框、DPI、桌布）。
>
> 因此下方的「平台支援」指的是*某個平台如何提供這四件事* —— **而不是**有哪些控件存在。控件是否可用屬於**設定檔**問題，而非作業系統問題；請參閱 [§1.2](#12-widget-可用性取決於設定檔而非作業系統)。

---

## 1. 九個支援的平台

### 1.1 後端

| # | 平台 | 後端提供 | 功能標誌 | 狀態 |
|---|----------|------------------|:---:|:---:|
| 1 | **Windows** | Win32 視窗 + 訊息迴圈 | `windows` | ✅ 已驗證 |
| 2 | **macOS** | Cocoa/objc2 `NSView` 繪製面 | `macos` | ✅ 已驗證 |
| 3 | **macOS**（預覽版） | objc2，狀態驅動 | `macos` | ✅ 預覽版 |
| 4 | **Linux (GTK)** | GTK3 視窗 + 事件迴圈 | `linux-gtk` | ✅ 已驗證 |
| 5 | **Linux (Wayland)** | Wayland `wl_surface` + 輸入 | `linux-wayland` | ✅ 已驗證 |
| 6 | **iOS** | UIKit 繪製面，狀態驅動 | `ios` | ✅ 已驗證 |
| 7 | **Android** | JNI 繪製面，狀態驅動 | `android` / `android-jni` | ✅ 已驗證 |
| 8 | **HarmonyOS** | NAPI 橋接 | `harmony` | ✅ 預覽版 |
| 9 | **WASM** | DOM 畫布 + 瀏覽器事件 | `wasm` | ✅ 已驗證 |
| 10 | **Portable** | 記憶體內幀緩衝區，無作業系統 | —（無任何後端匹配時的預設值） | ✅ 已驗證 |

在 Linux 上，執行時期會透過 `$WAYLAND_DISPLAY` 和 `$XDG_SESSION_TYPE` 環境變數自動偵測 Wayland 與 X11/GTK。

若無任何後端符合目標平台，就會選擇 `portable`：一個背後沒有作業系統的記憶體內繪製面。這是一種受支援的設定，而非降級設定 —— `mini` 與無宿主環境的 `embedded` 建構就是這樣運作的，而且它與其他任何後端一樣可測試，因為繪製路徑中沒有任何東西與作業系統相關。

### 1.2 Widget 可用性取決於設定檔，而非作業系統

因為每個控件都是自繪的，**相同的 179 種 widget 在每個作業系統上都能運作**。真正有差異的是「有多少 widget 集合被編譯進來」，而這是由*設定檔*決定的：

| 設定檔 | Widget 種類數 | 註冊表 | 自訂繪製控件 | 渲染器 |
|---------|:-----------:|:--------:|:-----------------------:|----------|
| `desktop` | 179（完整） | ✅ | ✅ | wgpu (GPU) |
| `tablet` | 179（完整） | ✅ | ✅ | wgpu (GPU) |
| `mobile` | 179（完整） | ✅ | ✅ | wgpu (GPU) |
| `embedded` | 精簡的核心集合 | — | — | 軟體 |
| `mini` | 精簡的核心集合 | — | — | 軟體 |

破折號代表**不存在，而非降級**：該模組已被編譯掉，因此 `supports_custom_widgets()` 會回報 `false`，而你應該拒絕該操作，而不是把它掛載到一片空白的繪製面上。

**實際的影響：** 你在 macOS 上編寫並測試的控件，在 Windows、Linux、iOS 與網頁上會以相同方式渲染，逐像素一致，而且你的程式碼裡不需要任何一個 `cfg(target_os)`。唯有當你需要後端所擁有的四件事之一時，才需要碰觸作業系統特定的 API。

### 1.3 平台服務確實會因作業系統而異

使用 `PlatformCapabilities` 查詢*宿主*提供哪些能力。絕不要假設 —— 執行在非其編譯目標之作業系統上的後端會回報 `false`。

| 作業系統 | DPI 縮放 | IME | 無障礙 | 原生選單 |
|----|:-----------:|:---:|:-------------:|:-----------:|
| Windows | ✅ | ✅ | ✅ | ✅ |
| macOS | ✅ | ✅ | ✅ | ✅ |
| Linux / GTK | ✅ | ✅ | ✅ | ✅ |
| Linux / Wayland | ✅ | ✅ | ✅ | ❌ |
| iOS | ✅ | ✅ | ✅ | ❌ |
| Android | ✅ | ✅ | ✅ | ❌ |
| HarmonyOS | ✅ | ✅ | ✅ | ❌ |
| WASM | ❌ | ❌ | ❌ | ❌ |
| Portable | ❌ | ❌ | ❌ | ❌ |

Wayland 沒有選單列的協議，因此它的後端會把選單樹留在行程內並由宿主渲染 —— 在此處宣稱有原生選單會是不實的。

`native_menu` 這個欄位很容易誤讀，因此值得了解這些值的來源：`Platform::capabilities` 的預設是「若後端回報 `Desktop` 家族則為 `true`」，而只有 Wayland、iOS、Android 與 HarmonyOS 會將它覆寫為 `false`。這意味著一個*忘記*覆寫的桌面家族後端，會默默繼承 `native_menu: true` —— 預設值是過度宣稱，而覆寫才是誠實的。`default_capabilities_for(family)` 會公開這個預設值，讓你可以將它與後端自身的回報做比較，而且上表有測試把關，因此不會與原始碼脫節。

---

## 2. `Platform` Trait — 通用合約

`Platform` trait 定義了後端必須提供的**六個必要方法** —— 繪製面、事件迴圈與生命週期。其餘所有方法都有誠實的預設實作：對宿主缺少的能力回報 `UnsupportedOnWidget` / `None`，而不是回報成功卻未實際生效的寫入。

```rust
use rust_widgets::platform::{Platform, PlatformCapabilities};

fn inspect_backend(platform: &dyn Platform) {
    println!("Backend: {}", platform.backend_name());
    println!("Family:  {:?}", platform.family());

    let caps: PlatformCapabilities = platform.capabilities();
    println!("DPI scaling:    {}", caps.dpi_scaling);
    println!("IME:            {}", caps.ime);
    println!("Accessibility:  {}", caps.accessibility);
    println!("Native menus:   {}", caps.native_menu);
}
```

### Widget 建立方法（子集）

| 方法 | Widget | 簽名 |
|--------|--------|-----------|
| `create_window` | Window | `(title, x, y, w, h) -> ObjectId` |
| `create_button` | Button | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_checkbox` | CheckBox | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_line_edit` | LineEdit | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_label` | Label | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_radio_button` | RadioButton | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_slider` | Slider | `(parent, x, y, w, h) -> ObjectId` |
| `create_progress_bar` | ProgressBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_combo_box` | ComboBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_box` | ListBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_panel` | Panel | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu_bar` | MenuBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_menu` | Menu | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_tool_bar` | ToolBar | `(parent, x, y, w, h) -> ObjectId` |
| `create_status_bar` | StatusBar | `(parent, text, x, y, w, h) -> ObjectId` |
| `create_message_box` | MessageBox | `(parent, title, text, x, y, w, h) -> ObjectId` |
| `create_file_dialog` | FileDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_color_dialog` | ColorDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_font_dialog` | FontDialog | `(parent, x, y, w, h) -> ObjectId` |
| `create_spin_box` | SpinBox | `(parent, x, y, w, h) -> ObjectId` |
| `create_list_view` | ListView | `(parent, x, y, w, h) -> ObjectId` |
| `create_scroll_area` | ScrollArea | `(parent, x, y, w, h) -> ObjectId` |

常見 widget 變異方法：`show_widget`、`hide_widget`、`set_widget_geometry`、`set_widget_text`、`get_widget_text`、`set_widget_enabled`、`is_widget_enabled`、`set_widget_visible`、`is_widget_visible`、`set_widget_ime_enabled`、`is_widget_ime_enabled`、`set_widget_accessibility_name`、`get_widget_accessibility_name`。

---

## 3. `BackendState<K>` — 執行緒安全的 HashMap 狀態儲存

`BackendState<K>` 是一個執行緒安全、可序列化的狀態儲存，用於狀態驅動的後端（Android、iOS、WASM、Harmony、Embedded）。它在 `Mutex` 防護下儲存 widget 記錄、選單事件、widget 觸發事件、剪貼簿文字以及拖放事件。

```rust
use rust_widgets::platform::state::BackendState;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MyKind { Button, Label }

let state = BackendState::<MyKind>::new();

// 建立一個 widget；回傳自動遞增的 ObjectId
let id = state.create_widget(MyKind::Button, "Click Me", 0, 0, 120, 32);

// 查詢 widget 狀態
assert!(state.contains_widget(id));
assert_eq!(state.kind_of(id), Some(MyKind::Button));
assert!(state.is_kind(id, MyKind::Button));
assert_eq!(state.text(id), "Click Me");

// 變異 widget 狀態
state.set_visible(id, false);
state.set_geometry(id, 10, 20, 200, 40);
state.set_text(id, "Updated");
state.set_enabled(id, false);
state.set_ime_enabled(id, true);
state.set_accessibility_name(id, "Submit button");
```

### 事件佇列

`BackendState` 維護內部佇列，用於選單、widget 觸發、剪貼簿及拖放事件：

```rust
// 選單事件
state.push_menu_event(item_id);
while let Some(id) = state.pop_menu_event() {
    println!("Menu item {} triggered", id);
}

// 型別化 widget 觸發事件
state.inject_widget_trigger_event(widget_id, WidgetTriggerKind::Clicked);
while let Some(event) = state.pop_widget_trigger_event() {
    match event.kind {
        WidgetTriggerKind::Clicked => { /* 處理點擊 */ }
        WidgetTriggerKind::ValueChanged => { /* 處理變更 */ }
        _ => {}
    }
}

// 剪貼簿
state.set_clipboard_text("Hello clipboard");
let text = state.clipboard_text();
```

---

## 4. 執行時期後端選擇

後端選擇在編譯時期進行，並在執行時期自動偵測：

### 編譯時期選擇

```rust
// src/platform/runtime.rs — 按目標進行條件編譯。
//
// 注意一個後端是「為了什麼」被選中的：它的繪製面與事件迴圈。它絕不是因為
// 「能建立哪些控件」而被選中，因為每個後端繪製的都是同一套 Rust 自繪控件。

#[cfg(all(target_os = "windows", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

#[cfg(all(target_os = "macos", not(feature = "embedded")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(SelectedMacOSPlatform::new())  // Dispatches to objc2 or cocoa
}

#[cfg(all(target_os = "linux", not(feature = "embedded"), feature = "linux-wayland"))]
fn create_native_platform() -> Box<dyn Platform> {
    if is_wayland_session() {
        Box::new(WaylandPlatform::new())
    } else {
        Box::new(LinuxPlatform::new())
    }
}

// No OS matched: an in-memory surface with no host behind it. This is a
// supported backend, not an error — it is what makes `mini` testable.
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(PortablePlatform::new())
}
```

### 控件從哪裡來

由於後端已不再建立控件，要取得一個控件必須透過 control backend / factory，而不是 `Platform`：

```rust
use rust_widgets::widget::WidgetFactory;
use rust_widgets::core::Rect;

let factory = WidgetFactory::new_with_defaults();
let mut button = factory
    .create("button", Rect::new(10, 10, 100, 30), "OK")
    .expect("button is a registered widget kind");

// Properties are read and written through the same contract, whatever the OS.
let label = factory.read_property(button.as_ref(), "text")?;
factory.write_property(button.as_mut(), "text", "Save".into())?;
# Ok::<(), rust_widgets::CapabilityAccessError>(())
```

請優先使用這個通用屬性 API。每個控件都自行實作 `WidgetProperties`，因此存在的屬性在每個平台都能以相同方式讀取與寫入 —— 呼叫端不需要任何依作業系統分支的 `if`/`else`。

### 全域單例

平台後端儲存在 `OnceLock` 單例中，首次存取時初始化：

```rust
use rust_widgets::platform;

// 初始化、執行、結束
platform::init();
platform::run();
platform::quit();

// 查詢能力
let caps = platform::capabilities();

// 取得 DPI 縮放因子
let dpi = platform::dpi_scale_factor();

// 檢查執行時期 GUI 模式
match platform::runtime_gui_mode() {
    RuntimeGuiMode::NativeInteractive => println!("以原生視窗執行"),
    RuntimeGuiMode::PreviewOrStub => println!("以預覽/存根模式執行"),
}
```

---

## 5. 裝置環境檢測

`DeviceEnvironment` 提供執行時期檢測裝置類別、觸控能力、螢幕尺寸、DPI、方向及無障礙偏好設定。

```rust
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{DeviceClass, Size};

// 從螢幕尺寸和 DPI 自動偵測
let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);

println!("Device class:  {:?}", env.device_class);  // Desktop
println!("Touch capable: {}", env.touch_capable);
println!("Orientation:   {:?}", env.orientation);
println!("DPI scale:     {:.1}", env.dpi_scale);

// 觸控目標建議（邏輯像素）
let target = env.min_touch_target();  // Desktop: 32×32, Tablet: 44×44, Mobile: 48×48
println!("Min touch target: {}×{}", target.width, target.height);
println!("Touch spacing:    {}", env.touch_spacing());

// 佈局縮放（投影模式增加 20%）
println!("Layout scale: {:.1}", env.layout_scale());

// 根據螢幕尺寸啟發式偵測裝置類別（無需功能標誌）：
//   width < 480     → Mobile
//   width < 1024    → Tablet
//   DPI ≥ 2.0, <1440 → Tablet
//   否則             → Desktop
```

### 裝置類別列舉

| 類別 | 觸控目標 | 觸控間距 | 典型用途 |
|-------|:---:|:---:|-------------|
| `Desktop` | 32×32 | 8px | 滑鼠 + 鍵盤 |
| `Tablet` | 44×44 | 12px | 觸控優先的大螢幕 |
| `Mobile` | 48×48 | 16px | 單手觸控 |
| `Embedded` | 40×40 | 10px | 專用硬體 |
| `Projector` | 24×24 | 6px | 遙控器導航 |

### 無障礙偏好設定

```rust
let mut env = DeviceEnvironment::default();
env.set_high_contrast(true);
env.set_reduced_motion(true);
env.set_font_scale(1.5);  // 限制在 [0.5, 3.0]
```

---

## 6. 剪貼簿系統

### `RichClipboardBackend` Trait

每個平台可以實作豐富的剪貼簿支援，包括文字、HTML、RTF、圖片和檔案清單：

```rust
use rust_widgets::platform::clipboard::{
    RichClipboardBackend, ClipboardContent, MockClipboard,
};

// 使用 MockClipboard 進行測試
let clip = MockClipboard::new();

// 設定純文字
clip.set_contents(ClipboardContent::Text("Hello".into()));

// 設定 HTML 並附帶純文字備援
clip.set_contents(ClipboardContent::Html {
    html: "<b>bold</b>".into(),
    plain: "bold".into(),
});

// 檢查格式支援
assert!(clip.has_format("text/plain"));
assert!(!clip.has_format("text/html"));

// 取得內容
if let Some(content) = clip.get_contents() {
    match content {
        ClipboardContent::Text(t) => println!("Text: {}", t),
        ClipboardContent::Html { html, plain } => println!("HTML: {}, Plain: {}", html, plain),
        ClipboardContent::Rtf(data) => println!("RTF: {} bytes", data.len()),
        ClipboardContent::Image { width, height, .. } => println!("Image: {}×{}", width, height),
        ClipboardContent::Files(paths) => println!("Files: {:?}", paths),
    }
}
```

### 平台剪貼簿整合

`Platform` trait 公開 `clipboard_backend()` 方法，回傳 `Option<&dyn RichClipboardBackend>`。桌面平台提供真實的剪貼簿整合；嵌入式平台回傳 `None`。

```rust
let platform = rust_widgets::platform::get_platform();

// 透過 Platform trait 使用純文字
platform.set_clipboard_text("Copied text");
let text = platform.get_clipboard_text();

// 透過後端使用豐富內容
if let Some(backend) = platform.clipboard_backend() {
    backend.set_clipboard_html("<h1>Title</h1>", "Title");
    backend.set_clipboard_image(&rgba_data, 64, 64);
}
```

---

## 7. 拖放（Drag & Drop）

```rust
use rust_widgets::platform::types::DropEvent;

// 從來源 widget 開始拖曳操作
platform.begin_drag(source_id, "text/plain", b"Dragged text");

// 輪詢放置事件
while let Some(event) = platform.poll_drop_event() {
    println!("Source:  {}", event.source_widget_id);
    println!("Target:  {}", event.target_widget_id);
    println!("MIME:    {}", event.mime);
    println!("Payload: {} bytes", event.payload.len());
}

// 程式化注入（用於測試）
platform.inject_drop_event(DropEvent {
    source_widget_id: 1,
    target_widget_id: 2,
    mime: "text/plain".into(),
    payload: b"test".to_vec(),
});
```

`BackendState` 提供相同操作：

```rust
state.begin_drag(src_id, "text/plain", payload);
if let Some(event) = state.pop_drop_event() {
    // 處理放置
}
state.inject_drop_event(event);
```

---

## 8. IME 系統

IME 橋接器提供東亞語言輸入的輸入法編輯器整合。

### `ImeBridge` Trait

```rust
use rust_widgets::platform::ime::{
    ImeBridge, ImeComposition, ImeCandidatePosition, MockImeBridge,
};

let bridge = MockImeBridge::new();

// Widget 獲得輸入焦點
bridge.focus_in(text_edit_id);

// 更新組字預覽（預編輯文字）
bridge.set_composition(&ImeComposition {
    text: "nihao".into(),
    cursor_position: 5,
    selection_length: 0,
});

// 提交最終文字
bridge.commit_text("你好");

// 定位候選視窗
bridge.set_candidate_window_position(ImeCandidatePosition { x: 100, y: 200 });

// Widget 失去焦點
bridge.focus_out(text_edit_id);

assert_eq!(bridge.focused_widget(), None);
```

### 平台 IME 後端

| 平台 | 實作 | 模組 |
|----------|---------------|--------|
| Linux | IBus 整合 | `platform::ime_linux` |
| macOS | `NSTextInputContext` | `platform::ime_macos` |
| Windows | TSF (文字服務框架) | `platform::ime_windows` |

`Platform` trait 公開 `ime_bridge() -> Option<&dyn ImeBridge>`：

```rust
let platform = rust_widgets::platform::get_platform();
if let Some(bridge) = platform.ime_bridge() {
    if bridge.is_active() {
        bridge.focus_in(widget_id);
    }
}
```

---

## 9. 無障礙（Accessibility）

### `A11yTree` — 跨平台無障礙節點樹

無障礙系統追蹤 28 種語意角色，並支援螢幕閱讀器導航。

```rust
use rust_widgets::platform::accessibility::{
    A11yTree, A11yNode, A11yState, A11yRole, A11yProvider,
};

let mut tree = A11yTree::new();

// 註冊一個 widget 節點
let node = A11yNode::new(
    42,
    A11yState {
        role: A11yRole::Button,
        label: "Submit".into(),
        enabled: true,
        ..Default::default()
    },
);
tree.register_node(node);

// 按角色查詢
let buttons = tree.find_by_role(A11yRole::Button);
for id in &buttons {
    if let Some(node) = tree.get(*id) {
        println!("Found button: {}", node.state.label);
    }
}

// 焦點導航
tree.focus_next();
tree.focus_previous();

// 動態查詢
let query_results = tree.query(|node| {
    node.state.role == A11yRole::Button && node.state.enabled
});
```

### A11yRole 列舉（28 種角色）

`Unknown` • `Button` • `Label` • `TextField` • `CheckBox` • `RadioButton` • `Slider` • `ProgressBar` • `List` • `Table` • `Image` • `Link` • `Heading` • `Paragraph` • `Group` • `Window` • `Dialog` • `Menu` • `MenuItem` • `Tab` • `Switch` • `Alert` • `ComboBox` • `SpinButton` • `StatusBar` • `ToolTip` • `Tree`

角色會自動對應到平台特定角色：`NSAccessibilityRole`（macOS）、UIA 控制項類型（Windows）和 AT-SPI 角色（Linux）。

### `A11yProvider` Trait

```rust
pub trait A11yProvider {
    fn register_widget(&mut self, id: ObjectId, role: A11yRole, label: &str);
    fn unregister_widget(&mut self, id: ObjectId);
    fn update_widget_state(&mut self, id: ObjectId, state: A11yState);
    fn announce(&self, message: &str);
    fn focus_next(&mut self) -> Option<ObjectId>;
    fn focus_previous(&mut self) -> Option<ObjectId>;
    fn tree(&self) -> &A11yTree;
    fn tree_mut(&mut self) -> &mut A11yTree;
}
```

### `AccessibilityBridge` Trait（平台層級）

```rust
pub trait AccessibilityBridge {
    fn set_accessibility_name(&self, id: ObjectId, name: &str);
    fn accessibility_name(&self, id: ObjectId) -> String;
    fn notify_name_changed(&self, id: ObjectId);
    fn notify_value_changed(&self, id: ObjectId);
    fn notify_state_changed(&self, id: ObjectId);
    fn notify_focus_changed(&self, id: ObjectId);
    fn set_aria_properties(&self, id: ObjectId, properties: AriaProperties);
}
```

將焦點管理連結到無障礙系統：

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// 焦點變更現在會轉發到平台無障礙橋接
```

### 平台無障礙模組

| 平台 | 模組 | 橋接器 |
|----------|--------|--------|
| macOS | `platform::accessibility::macos` | NSAccessibility |
| Windows | `platform::accessibility::windows` | UIAutomation |
| Linux | `platform::accessibility::linux` | AT-SPI（透過 zbus） |

---

## 10. 選單系統

```rust
use rust_widgets::platform::get_platform;

let platform = get_platform();

// 建立附加到視窗的選單列
let menu_bar = platform.create_menu_bar(window_id, 0, 0, 800, 24);
platform.attach_menu_bar_to_window(window_id, menu_bar);

// 建立子選單
let file_menu = platform.create_menu(menu_bar, "File", 0, 0, 60, 24);

// 新增選單項目
let new_id = platform.menu_add_item(file_menu, "New", Some("Ctrl+N"));
let open_id = platform.menu_add_item(file_menu, "Open...", Some("Ctrl+O"));
platform.menu_add_item(file_menu, "Save", Some("Ctrl+S"));

// 輪詢選單觸發
while let Some(triggered_id) = platform.poll_menu_triggered() {
    if triggered_id == new_id {
        println!("New file");
    } else if triggered_id == open_id {
        println!("Open file");
    }
}

// 程式化注入（用於測試）
platform.inject_menu_trigger(new_id);

// 輪詢型別化 widget 觸發
while let Some(trigger) = platform.poll_widget_trigger_event() {
    match trigger.kind {
        WidgetTriggerKind::Clicked => { /* 處理點擊 */ }
        WidgetTriggerKind::ValueChanged => { /* 處理數值變更 */ }
        WidgetTriggerKind::SelectionChanged => { /* 處理選擇變更 */ }
        WidgetTriggerKind::Closed => { /* 處理關閉 */ }
        WidgetTriggerKind::Unknown => { /* 後備處理 */ }
    }
}
```

### `WidgetTriggerKind` 列舉

| 變體 | 數值 | 說明 |
|---------|:---:|-------------|
| `Unknown` | 0 | 無具體觸發語意 |
| `Clicked` | 1 | 主要啟用（按鈕點擊、核取方塊切換） |
| `ValueChanged` | 2 | 有狀態數值變更（文字編輯、滑桿） |
| `SelectionChanged` | 3 | 當前選擇更新（下拉式、清單、樹狀、表格） |
| `Closed` | 4 | Widget/視窗關閉的生命週期觸發 |

---

## 11. 能力協商（Capability Negotiation）

`CapabilityContract` 系統在原生桌面設定檔與受限的嵌入式設定檔之間協商執行時期能力。

### `PlatformCapabilities` 旗標

```rust
pub struct PlatformCapabilities {
    pub dpi_scaling: bool,           // 高 DPI 支援
    pub ime: bool,                   // IME 整合
    pub accessibility: bool,         // 無障礙橋接
    pub native_menu: bool,           // 原生選單支援
    pub typed_widget_trigger: bool,  // 型別化 widget 事件
}
```

### `NativeCapabilityContract`

由具備桌面能力的執行時期（Windows、macOS、Linux）使用。

這是 `PlatformCapabilities` 的**型別別名**，而非另一個獨立的 struct。它帶有相同的五個旗標，而且必須如此：否則協商結果與後端自身的回報可能會互相矛盾，而新加入的能力可能被加入到其中一邊、卻被另一邊默默丟掉。之所以保留這個名稱，是因為它是協商 API 的詞彙（`CapabilityContract::Native(..)`、`Platform::native_capability_contract`）；由於它是別名，兩者可互換使用，也不存在任何可能寫錯的轉換。

| 欄位 | 說明 |
|-------|-------------|
| `dpi_scaling` | 感知 DPI 的幾何和文字 |
| `ime` | 輸入法編輯器支援 |
| `accessibility` | 螢幕閱讀器橋接 |
| `native_menu` | 平台原生選單列 |
| `typed_widget_trigger` | 型別化觸發事件 |

### `EmbeddedCapabilityContract`

由嵌入式/受限執行時期使用：

| 欄位 | 說明 |
|-------|-------------|
| `fixed_dpi` | 固定 DPI 縮放因子 (1.0) |
| `low_memory_mode` | 預期低記憶體行為 |
| `typed_widget_trigger` | 型別化觸發事件 |

### 協商

```rust
use rust_widgets::platform::{negotiate_capability_contract, CapabilityContract};
use rust_widgets::core::RuntimeProfile;

let contract = negotiate_capability_contract(RuntimeProfile::Full);
match contract {
    CapabilityContract::Native(native) => {
        println!("DPI scaling:   {}", native.dpi_scaling);
        println!("IME:           {}", native.ime);
        println!("Accessibility: {}", native.accessibility);
        println!("Native menus:  {}", native.native_menu);
    }
    CapabilityContract::Embedded(embedded) => {
        println!("Fixed DPI:       {}", embedded.fixed_dpi);
        println!("Low memory mode: {}", embedded.low_memory_mode);
    }
}
```

當平台後端未發布合約時，會提供備援合約，確保在所有環境中都有確定性行為。

---

## 12. 螢幕上鍵盤（行動裝置）

> **本節先前記錄的是 `rust_widgets::platform::virtual_keyboard::{VirtualKeyboard, KeyboardNotch, KeyboardState}`。**
> crate 中**不存在**該模組 —— 它在 2.0.0 之前就已被移除（見
> [`docs/MIGRATION_GUIDE.md`](../../../docs/MIGRATION_GUIDE.md)，其中把
> `platform::virtual_keyboard::VirtualKeyboard` 列為已刪除）。先前給出的範例**無法編譯**。
> 下文已更換為真實存在的 API。

本函式庫**不負責**螢幕上鍵盤。軟體鍵盤是宿主的職責：iOS 與 Android 透過系統顯示與隱藏它，
平台層只把由此產生的事實報告出來，供佈局程式碼回應。可觀測的入口是輸入法（IME）契約
加上後端報告的能力集：

```rust
use rust_widgets::platform::{backend_name, capabilities, get_platform};

// 詢問後端是什麼。呼叫方程式碼裡沒有 `cfg(target_os)`：答案是一個執行期事實。
println!("backend: {}", backend_name());

// 觸控裝置上的文字輸入走輸入法入口。每個後端實作 `ImeBridge` trait
// （見 `src/platform/ime.rs`）；宿主透過平台的 IME 入口驅動組字過程，
// 而不是透過一個本函式庫不得不憑空發明的「鍵盤生命週期物件」。
let _ = get_platform();

// 能力是後端報告的事實，因此同一份呼叫方程式碼在每個平台上都能執行並在執行期自適應。
let caps = capabilities();
println!("ime: {}", caps.ime);
```

注意：文字輸入應檢查的能力項是 `ime`；**沒有**單獨的「軟體鍵盤」旗標 —— 鍵盤是硬體還是
螢幕上的，是宿主的事情；而 *輸入法組字* 才是本函式庫支援的部分。

---

## 13. 平台特定後端概覽

### Linux

```rust
// 自動偵測 Wayland 與 X11/GTK
#[cfg(all(target_os = "linux", feature = "linux-wayland"))]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}
```

### macOS（objc2 橋接）

`macos_objc2` 模組提供現代的 Objective-C 橋接。`SelectedMacOSPlatform` 根據功能標誌分派到適當的後端。

### Windows

`WindowsPlatform` 提供完整的 Win32 API 整合，包括原生視窗、剪貼簿、拖放以及透過 UIAutomation 的無障礙支援。

### 行動裝置（iOS / Android）

狀態驅動後端（`IosMobilePlatform`、Android JNI 橋接）使用 `BackendState<K>` 進行 widget 管理。Android JNI 橋接公開用於建立檢視的原生方法。

```rust
#[cfg(feature = "mobile-api")]
rust_widgets::platform::mobile_attach_to_native_view(native_handle);
let name = rust_widgets::platform::mobile_backend_name();
```

### WASM / 嵌入式

兩者都使用基於 `BackendState` 的狀態管理。嵌入式目標透過 `mini` 功能標誌支援 `no_std`，並使用競技場分配的集合。

---

## 14. 跨平台模式

### 功能標誌控制的平台程式碼

```rust
#[cfg(target_os = "linux")]
fn platform_specific_setup() { /* GTK 初始化 */ }

#[cfg(target_os = "macos")]
fn platform_specific_setup() { /* NSApplication 初始化 */ }

#[cfg(target_os = "windows")]
fn platform_specific_setup() { /* CoInitialize */ }
```

### 執行時期查詢後端身份

```rust
let platform = rust_widgets::platform::get_platform();

match platform.backend_name() {
    "cocoa" | "WindowsPlatform" => {
        // 桌面原生模式
    }
    "wayland" => {
        // Wayland 原生模式
    }
    "gtk" => {
        // GTK 原生模式
    }
    "harmony-desktop" | "android-mobile" | "macos-objc2-preview" => {
        // 預覽/存根模式
    }
    _ => {
        // 未知 — 預覽模式
    }
}
```

### 將無障礙系統連結到焦點管理器

```rust
use rust_widgets::platform::wire_focus_manager_to_a11y;
use rust_widgets::event::focus::FocusManager;

let mut fm = FocusManager::new();
wire_focus_manager_to_a11y(&mut fm);
// 所有焦點變更現在都會轉發到平台無障礙橋接
```

### 完整的跨平台初始化

```rust
use rust_widgets::platform;
use rust_widgets::platform::detector::DeviceEnvironment;
use rust_widgets::core::{Size, RuntimeProfile};

fn main() {
    let env = DeviceEnvironment::detect(Size::new(1920, 1080), 1.0);
    println!("Running on {:?} device", env.device_class);

    platform::init();

    let caps = platform::capabilities();
    if caps.ime {
        println!("IME support: enabled");
    }

    if let Some(bridge) = platform::get_platform().accessibility_bridge() {
        println!("Accessibility bridge: available");
    }

    let contract = negotiate_capability_contract(RuntimeProfile::Full);
    println!("Capability contract: {:?}", contract);

    // ... 建立視窗、widget ...

    platform::run();
}
```
