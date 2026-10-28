# 控制項族詳解

本章是 crate 所提供控制項的逐個參考。[控制項系統](widget-system.md) 一章解釋所有控制項共享的機制，[API參考](api-reference.md) 列出模組與型別；本章回答宿主真正會問的問題——*這件事該掛哪個控制項，能設定什麼，它會發出什麼，我該怎麼呼叫它？*

本章按控制項族組織——即 `src/widget/` 下的模組目錄——族內再按 `WidgetKind` 排列。`src/widget/kind.rs` 中宣告的每個類型都會出現在這些頁面裡，包括只作為別名或作為其他控制項子項存在的類型。名稱的權威清單是 capability 註冊表（`WidgetFactory::capabilities()`），型別的權威清單是列舉本身。

## 如何閱讀本章

每個控制項條目結構一致：

* **標題** —— `WidgetKind` 變體，括號內是規範工廠名稱。一個類型可以有多個註冊名（`line_edit`、`password_edit`、`phone_input`、`email_input` 都回報 `WidgetKind::LineEdit`）；每個名稱都有自己的條目，因為各有自己的建構函式與角色。

* **屬性** —— 屬性名稱、`PropertyValueKind`，列舉屬性還列出可接受的 token。唯讀屬性會標註。每個 schema 還帶下一節說明的四個共享屬性。條目中給出的常數名（`BUTTON_PROPERTIES` 等）就是該控制項發布的 schema，可在 `src/widget/capability/` 中核對。

* **事件** —— `WidgetFactory::connect_event` 接受的事件名，以及載荷型別與形狀（`Scalar`、`Tuple2`、清單、可選、混合）。無載荷事件就是一般通知。名稱與載荷來自產生的 `event_payloads.rs` 表，`tools/check_event_payload_types.py` 保證它與真實訊號一致。

* **命令** —— `WidgetFactory::invoke_command` 為該控制項接受的命令名。

* **Rust API** —— 註冊的建構函式，以及宿主通常使用的兩個工廠入口。建構函式簽名為 `fn(Rect, &str) -> Box<dyn Widget>`。

* **別名** —— capability 列發布的其他工廠拼寫（若有）。

按名讀寫屬性、呼叫命令、訂閱事件都走同一個註冊表，因此一個入口服務所有控制項：

```rust
use rust_widgets::core::Rect;
use rust_widgets::signal::CustomSignalHub;
use rust_widgets::widget::capability::WidgetFactory;

let factory = WidgetFactory::shared();
let mut button = factory
    .create("button", Rect::new(10, 10, 120, 32), "儲存")
    .expect("註冊表認識 `button`");

factory
    .write_property("button", &mut *button, "text", "另存為")
    .expect("`text` 是可寫 String 屬性");
let current = factory
    .read_property("button", &*button, "text")
    .expect("`text` 可讀");

let hub = CustomSignalHub::new();
factory
    .connect_event("button", "clicked", &hub, || { /* 處理點擊 */ })
    .expect("`button` 發布了 `clicked`");

factory
    .invoke_command("button", &mut *button, "click")
    .expect("`click` 是已發布命令");
```

## 四個共享屬性

`src/widget/capability/` 中的每個屬性 schema 都以同樣的四個條目結尾。它們由基礎層實作，而不是各控制項自行實作，因此在各處行為一致：

| 屬性 | 型別 | 含義 |
|---|---|---|
| `enabled` | Bool | 控制項是否接受輸入。停用的控制項仍會繪製並回報其模型。 |
| `visible` | Bool | 控制項是否繪製並參與命中測試。 |
| `tooltip` | String | 懸停時顯示的提示。`TooltipManager` 配套控制項也可為整個子樹驅動工具提示。 |
| `geometry` | String | 控制項的矩形。schema 把可讀與可寫都宣告為 `false`，因此它*不能*透過 `read_property`/`write_property` 存取；放置屬於佈局與 `Widget::set_geometry`。 |

一些控制項還透過屬性暴露列舉的字串拼寫，可接受 token 列在條目中。這些 token 與控制項 setter 解析的拼寫相同，因此從 schema 讀出的 token 一定能寫回。標為*唯讀*的屬性可讀，寫入回傳 `ReadOnlyProperty`；條目中不存在的屬性根本沒有發布，寫入回傳 `UnknownProperty`。

## 控制項族一覽

| 控制項族 | 原始碼目錄 | 控制項數 | 用途 |
|---|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 | 視窗外殼與始終可用的控制項 |
| `overlay` | `src/widget/overlay_widgets/` | 5 | 浮動動作、通知與生命週期浮層 |
| `nav` | `src/widget/nav_widgets/` | 7 | 頂層導覽與頁面容器 |
| `input` | `src/widget/input_widgets/` | 29 | 文字、選擇與數值輸入 |
| `display` | `src/widget/display_widgets/` | 27 | 唯讀指示器與小型顯示標記 |
| `container` | `src/widget/container_widgets/` | 27 | 放置：捲動、分頁、分割與佈局盒 |
| `view` | `src/widget/view_widgets/` | 15 | 模型驅動的清單、樹、表格與檢查器 |
| `chart` | `src/widget/chart_widgets/` | 5 | 折線、柱狀、圓餅、迷你與儀表視覺化 |
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 | 選單、工具列、狀態列與命令項 |
| `dialog` | `src/widget/dialog/` | 17 | 模態決策與臨時面板 |
| `special` | `src/widget/special_widgets/` | 39 | 完整應用介面及其配套控制項 |
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 | 結構化文字編輯器及其導覽外殼 |
| `finance` | `src/widget/chart_widgets/finance/` | 6 | 行情：蠟燭、深度、階梯與指標 |
| `advanced` | `src/widget/advanced_widgets/` | 10 | 日期時間、旋鈕、標籤、功能區、圓餅選單與快速鍵 |
| `misc` | `src/widget/misc_widgets/` | 9 | 工具與行動控制項 |
| `media` | `src/widget/media_widgets/` | 7 | 播放、動畫與攝影機預覽 |
| `web` | `src/widget/web_widgets/` | 1 | 內嵌 Web 內容 |
| `cupertino` | `src/widget/cupertino/` | 8 | iOS 風格與 Material 自適應呈現 |
| `macos` | `src/widget/macos/` | 5 | 原生外觀的 macOS 外殼 |

## 1. 基礎控制項與視窗外殼

基礎族是始終可用的核心：視窗及其分組外殼，以及幾乎每張表單都會用到的四個控制項。它們也是精簡設定下無條件保留的控制項類型，因此需要在最小建置上執行的宿主應優先以這一族建構介面。

單一確定動作選 `Button`，獨立的開關設定選 `CheckBox` 或 `ToggleButton`，互斥群組內的一項選 `RadioButton`，唯讀文字選 `Label`。需要給一組控制項可見邊界時用 `Frame`、`GroupBox` 與 `Panel`。不要把 `Label` 當可編輯欄位、把 `Button` 當導覽容器，也不要把 `Frame` 當佈局引擎——`Frame` 只畫邊框，不排列子控制項。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 |

### Window (`window`)

頂層視窗：標題、帶關閉入口的標題列，以及承載其他控制項的表面。每個頂層介面用一個；次要或臨時視窗用 `Dialog` 或 `PopupWindow`，因為它們自帶模態與生命週期。

**屬性.** 除四個共享屬性外共 4 項：`title`: String · `title_bar_height`: UInt · `close_button_size`: UInt · `button_spacing`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`

**命令.** `set_title`, `close`

**Rust API.** `create_window(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("window", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `main_window`

### GroupBox (`panel`)

`Panel` 是 `GroupBox` 外殼的第二個工廠名稱：帶標題、用於歸組相關控制項的框。完整建置中 `create_panel` 掛載 `panel` 名稱，控制項回報 `WidgetKind::GroupBox`；按 `GroupBox` 的方式使用即可，不要尋找 `Panel` 專屬的 capability 列——它不存在。

**屬性.** 除四個共享屬性外共 4 項：`title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_panel(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("panel", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Frame (`frame`)

邊框為一個子控制項繪製邊框、3D 陰影或分隔線。用於給佈局已經放好的內容加視覺結構；不要用它排列或裁剪子控制項——它只畫外殼。

**屬性.** 除四個共享屬性外共 7 項：`frame_shape`: Enum (`no_frame` | `box` | `panel` | `styled_panel` | `hline` | `vline` | `win_panel`) · `frame_shadow`: Enum (`plain` | `raised` | `sunken`) · `line_width`: Float · `mid_line_width`: Float · `frame_width`: Float · `contents_margins`: String · `theme_bevel`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_frame_shape`, `set_frame_shadow`, `set_line_width`

**Rust API.** `create_frame(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("frame", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `frame_widget`

### Button (`button`)

按鈕在啟用時發出 `clicked`，並分別回報按下、釋放與取消。每個按鈕表達一個確定動作；需要選單時設定 `menu_open` 並用 `MenuPanel` 配套控制項，而不是自建彈層；也不要把它當導覽容器。

**屬性.** 除四個共享屬性外共 17 項：`text`: String · `pressed`: Bool · `default`: Bool · `icon_source`: IconSpec · `icon_size`: UInt · `shortcut`: String · `auto_default`: Bool · `auto_exclusive`: Bool · `flat`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `drag_enabled`: Bool · `accepts_drops`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `menu_open`: Bool · `menu_item_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `clicked` · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String

**命令.** `click`

**Rust API.** `create_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `pushbutton`, `btn`

### Label (`label`)

不可編輯的文字面，支援對齊、換行、省略、縮排，以及帶連結的可選富文字。用於說明、說明文字與內聯連結；使用者需要修改文字時改用 `LineEdit`、`TextArea` 或 `RichEdit`。

**屬性.** 除四個共享屬性外共 10 項：`text`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `word_wrap`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `margin`: UInt · `scaled_contents`: Bool · `indent`: UInt · `text_format`: Enum (`plain` | `rich`) · `open_external_links`: Bool · `link_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `link_activated`: String · `link_hovered`: String

**命令.** `set_text`, `set_alignment`

**Rust API.** `create_label(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("label", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `text_label`

### CheckBox (`check_box`)

帶標籤的方框，擁有獨立的選中狀態與可選的第三態 `partially_checked`。用於彼此獨立的設定；多個選項中必須恰好選一個時用 `RadioButton`，並注意 `auto_exclusive` 可以讓同級控制項互斥而無需單選群組。

**屬性.** 除四個共享屬性外共 8 項：`text`: String · `state`: Enum (`unchecked` | `partially_checked` | `checked`) · `checked`: Bool · `tristate_enabled`: Bool · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `state_changed`: String · `toggled`: Bool

**命令.** `set_checked`, `toggle`, `set_state`

**Rust API.** `create_check_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("check_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### RadioButton (`radio_button`)

圓形選擇控制項，與共享 `group_id` 的其他單選按鈕互斥。用於數量少、需要始終可見的備選項；清單很長時用 `ComboBox`，選項彼此獨立時用 `CheckBox`。

**屬性.** 除四個共享屬性外共 7 項：`text`: String · `checked`: Bool · `group_id`: String · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `checked_changed`: Bool · `selected`

**命令.** `set_checked`, `set_group_id`

**Rust API.** `create_radio_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("radio_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ToggleButton (`toggle_button`)

在選中與未選中之間鎖存的按鈕，可選帶彈出箭頭（`arrow_type`/`popup_mode`）與選單。用於保持開啟的模式，或擁有選單的工具按鈕；一次性動作用一般 `Button`，設定式開關列用 `Switch`。

**屬性.** 除四個共享屬性外共 14 項：`text`: String · `checked`: Bool · `auto_exclusive`: Bool · `auto_raise`: Bool · `group_id`: String · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `icon_source`: IconSpec · `arrow_type`: Enum (`none` | `down` | `up` | `left` | `right`) · `popup_mode`: Enum (`instant` | `menu_button` | `delayed`) · `menu_open`: Bool · `menu_item_count`: UInt (唯讀) · `state`: Enum (`normal` | `hover` | `pressed` | `checked` | `disabled`) (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `checked_changed`: Bool · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String · `toggled`: Bool

**命令.** `set_checked`, `toggle`, `set_text`

**Rust API.** `create_toggle_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("toggle_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `toggle`

## 2. 浮層與浮動控制項

浮層控制項位於頁面內容之上，而不佔用閱讀順序中的位置。它們涵蓋主浮動動作、持續通知、手勢關閉、下拉重新整理與啟動畫面。

頁面的唯一主操作選 `FAB`，需要使用者確認的提示選 `Banner`，列表列透過明確滑動刪除時選 `SwipeToDismiss`，可捲動主體上的下拉重新整理選 `RefreshControl`，應用程式初始化期間用 `SplashScreen`。不要用浮層表達必須阻擋其餘介面的決策——那是對話框的職責；瞬時的 `Toast` 或 `Snackbar` 場景也不要用 `Banner`。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `overlay` | `src/widget/overlay_widgets/` | 5 |

### Banner (`banner`)

整寬、持久的通知，帶嚴重級別、最多若干動作，以及由使用者關閉的狀態。用於使用者必須看到並確認的狀況；瞬時訊息用 `Snackbar` 或 `Toast`，必須應答後才能繼續時用 `MessageBox`。

**屬性.** 除四個共享屬性外共 5 項：`text`: String · `severity`: Enum (`info` | `success` | `warning` | `error`) · `dismissible`: Bool · `dismissed`: Bool (唯讀) · `action_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_clicked`: String · `dismissed`

**命令.** `dismiss`, `show`, `set_actions`, `activate_action`

**Rust API.** `create_banner(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("banner", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `notice`

### FAB (`fab`)

圓形、帶高度的主操作按鈕，浮在內容之上。每頁一個，表達主操作；如果主操作會展開成多個，配 `SpeedDial` 配套控制項，而不是堆多個 FAB；不要用於破壞性或極少使用的命令。

**屬性.** 除四個共享屬性外共 2 項：`icon`: String · `icon_source`: IconSpec — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`

**命令.** `set_icon`

**Rust API.** `create_fab(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("fab", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `floating_action_button`, `action_button`

### RefreshControl (`refresh_control`)

下拉重新整理表面：使用者把內容下拉超過閾值，控制項發出 `refresh_triggered`；工作期間宿主設定 `refreshing`。用於觸控裝置上的可捲動主體；桌面端優先提供顯式重新整理命令。

**屬性.** 除四個共享屬性外共 1 項：`refreshing`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `refresh_triggered`

**命令.** `set_refreshing`

**Rust API.** `create_refresh_control(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("refresh_control", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `pull_to_refresh`

### SplashScreen (`splash_screen`)

啟動畫面：標誌/標題、可選副標題與進度，由程式在初始化完成時關閉，而不是由使用者關閉。僅在應用程式啟動期間使用；視窗已經可見後由使用者發起的工作改用 `ProgressDialog`。

**屬性.** 除四個共享屬性外共 4 項：`title`: String · `subtitle`: String · `progress`: Float · `skippable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `fade_out_finished` · `finished`: String · `skipped`: String

**命令.** `finish`, `set_progress`, `set_title`

**Rust API.** `create_splash_screen(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("splash_screen", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `splash`

### SwipeToDismiss (`swipe_to_dismiss`)

手勢容器：子內容被滑動時露出動作背景，超過閾值後發出 `dismissed`。用於以滑動為主要刪除手勢的列表列；為鍵盤與桌面使用者保留可見的替代操作；背景需要多個動作時配 `SwipeActions`。

**屬性.** 除四個共享屬性外共 1 項：`is_dismissed`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** 未發布命令。

**Rust API.** `create_swipe_to_dismiss(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("swipe_to_dismiss", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `swipe_dismiss`

## 3. 導覽與頁面容器

導覽控制項表達「使用者在哪裡」和「能去哪裡」：頂部應用列、底部標籤列、側滑抽屜、推入/彈出頁面堆疊、分頁標籤檢視、分頁索引，以及把它們組合起來的腳手架。

標題加前後置操作用 `AppBar`，手機上三到五個頂層目的用 `BottomNavigationBar`，目的更多時用 `NavigationDrawer`，逐層下鑽流程用 `NavigationStack`，並列頁面用 `TabView`，編號分頁用 `Pagination`。不要用導覽控制項承載任意內容——`AdaptiveScaffold` 負責組合，頁面仍由容器族的 `TabWidget` 持有。也不要把 `Pagination` 當頁面容器：它只是索引。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `nav` | `src/widget/nav_widgets/` | 7 |

### AdaptiveScaffold (`adaptive_scaffold`)

把 `AppBar`、內容區與可選底部導覽列組合成一個自適應框架的腳手架。用於平板與行動端的頂層頁面外殼；桌面端應顯式放置導覽外殼，讓視窗自身的選單列仍是主命令面。

**屬性.** 除四個共享屬性外共 5 項：`title`: String · `badge_style`: Enum (`text` | `dot`) · `drawer_animated`: Bool · `scrim_opacity`: Float · `page_transition`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `nav_selected`: UInt

**命令.** `set_title`

**Rust API.** `create_adaptive_scaffold(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("adaptive_scaffold", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `scaffold`

### AppBar (`app_bar`)

頂部列：標題、前置返回入口與後置動作；兩個文字入口按閱讀方向（`direction`）放置，而不是固定左右。用於頁面標題加一兩個動作；動作更多時收進溢出選單，桌面端用 `ToolBar`。

**屬性.** 除四個共享屬性外共 2 項：`title`: String · `direction`: Enum (`ltr` | `rtl`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_index_pressed`: UInt · `action_overflow_pressed` · `action_pressed` · `back_pressed`

**命令.** `set_title`

**Rust API.** `create_app_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("app_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `top_app_bar`

### BottomNavigationBar (`bottom_navigation_bar`)

底部一列三到五個頂層目的，帶選中狀態。用於手機寬度的頂層導覽；目的更多時用 `NavigationDrawer`；頁面是同一螢幕內的並列內容而非應用層級目的時用 `TabView`。

**屬性.** 除四個共享屬性外共 2 項：`item_count`: UInt (唯讀) · `selected_index`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_bottom_navigation_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("bottom_navigation_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `bottom_nav`, `bottomnav`

### NavigationDrawer (`navigation_drawer`)

側滑面板，包含分區、標題與選中項，並帶一個可關閉它的遮罩。用於較長的目的地清單或帳戶/設定入口；用 `DrawerSectionHeader` 配套控制項給列分組；寬佈局下不要讓它常駐開啟——那種場景應在佈局裡放側欄。

**屬性.** 除四個共享屬性外共 3 項：`open`: Bool · `width`: Float · `section_separator_visible`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed` · `header_clicked` · `item_selected`: UInt · `opened`

**命令.** `set_open`, `set_width`

**Rust API.** `create_navigation_drawer(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("navigation_drawer", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `nav_drawer`, `drawer`

### NavigationStack (`navigation_stack`)

推入/彈出頁面堆疊：`push`、`pop`、`set_current_page`，`navigation_changed` 回報新的頂層頁面。用於返回必須回到上一頁的下鑽流程；由程式而非使用者選頁、且不隱含歷史時用 `StackedWidget`。

**屬性.** 除四個共享屬性外共 3 項：`page_count`: UInt (唯讀) · `current_page`: UInt · `back_visible`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_index_pressed`: UInt · `navigation_changed`: String

**命令.** `push`, `pop`, `set_current_page`

**Rust API.** `create_navigation_stack(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("navigation_stack", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `nav_stack`

### Pagination (`pagination`)

編號分頁索引：頁大小、相鄰頁數量、可選導覽按鈕與滾輪翻頁。它不持有內容：頁面由 `Carousel`、表格或清單持有，`Pagination` 只回報使用者要求的頁號。不要把它當頁面容器本身。

**屬性.** 除四個共享屬性外共 8 項：`total`: UInt · `page_size`: UInt · `page`: UInt · `page_count`: UInt (唯讀) · `last_page`: UInt (唯讀) · `sibling_count`: UInt · `show_nav_buttons`: Bool · `wheel_paging_enabled`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`: UInt · `page_changed`: UInt · `page_hover_changed`: UInt or null

**命令.** `next_page`, `previous_page`, `set_page`, `set_total`

**Rust API.** `create_pagination(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("pagination", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `pager`, `page_numbers`

### TabView (`tab_view`)

標籤列加內容槽，可選鍵盤導覽，關閉/移動請求以事件回報。用於同一螢幕內的並列頁面；每頁是需要自身佈局的任意子容器時用 `TabWidget`，應用層級目的用 `BottomNavigationBar`。

**屬性.** 除四個共享屬性外共 3 項：`tab_count`: UInt (唯讀) · `selected_index`: UInt · `keyboard_navigation_enabled`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tab_changed`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `set_selected_index`

**Rust API.** `create_tab_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tab_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `page_tab_view`

## 4. 文字與數值輸入

輸入族把按鍵、選擇與拖曳轉換為帶型別的值。它涵蓋單行欄位、遮罩輸入、自動補全、多行編輯、虛擬鍵盤與數值選擇器。

按*值的形狀*選擇：一行自由文字用 `LineEdit`，段落用 `TextArea` 或 `TextEdit`，有界數字用 `SpinBox` 或 `NumberPicker`，固定選項用 `ComboBox`/`Dropdown`，建議來自資料時用 `AutoCompleteEdit` 或 `Mention`，格式固定時用 `MaskedEdit`，階層路徑用 `Cascader`。不要用 `LineEdit` 編輯多行文字，不要把 `ComboBox` 當可編輯自由文字欄位（那是 `EditableComboBox`），也不要用 `ListBox` 承載需要虛擬化的資料量——`VirtualList` 屬於檢視族。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `input` | `src/widget/input_widgets/` | 29 |

### LineEdit (`line_edit`)

單行文字欄位，支援選擇、復原/重做可用性、回顯模式與驗證裝飾（`helper`、`error`、`counter`、`validation_state`）。用於一行自由文字；段落用 `TextArea`，固定格式用 `MaskedEdit`，數值用 `SpinBox`。

**屬性.** 除四個共享屬性外共 21 項：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (唯讀) · `over_limit`: Bool (唯讀) · `valid`: Bool (唯讀) · `validation_message`: String (唯讀) · `validation_state`: String (唯讀) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_line_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("line_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `text_input`, `input`

### LineEdit (`password_edit`)

`LineEdit` 的密碼角色：`echo_mode` 為 `password`，`password_echo_on_edit` 控制輸入時的短暫顯示。用於機密；不要記錄其值；`phone_input` 與 `email_input` 只用於各自的取值形狀。

**屬性.** 除四個共享屬性外共 21 項：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (唯讀) · `over_limit`: Bool (唯讀) · `valid`: Bool (唯讀) · `validation_message`: String (唯讀) · `validation_state`: String (唯讀) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_password_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("password_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `password_input`, `password_line_edit`

### LineEdit (`phone_input`)

`LineEdit` 的電話角色：建構函式安裝電話驗證器，拒絕空白與非電話字元，`valid` 回報值是否為 7 到 15 位數字。用於電話號碼；它會驗證，但不會替你做顯示格式化。

**屬性.** 除四個共享屬性外共 21 項：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (唯讀) · `over_limit`: Bool (唯讀) · `valid`: Bool (唯讀) · `validation_message`: String (唯讀) · `validation_state`: String (唯讀) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_phone_input(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("phone_input", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `phone_edit`, `telephone_input`

### LineEdit (`email_input`)

`LineEdit` 的電子郵件角色：建構函式安裝郵件驗證器，拒絕空白，`valid` 回報值是否為 `local@domain.tld`。用於地址；它驗證形狀，但無效值如何處理仍由你決定。

**屬性.** 除四個共享屬性外共 21 項：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (唯讀) · `over_limit`: Bool (唯讀) · `valid`: Bool (唯讀) · `validation_message`: String (唯讀) · `validation_state`: String (唯讀) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_email_input(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("email_input", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `email_edit`, `email_field`

### ListBox (`list_box`)

可捲動條目清單，支援單選、多選或擴展選擇，並有目前列。用於規模適中、已完全物化、每列是純文字的清單；列需要模型、委派或虛擬化時改用 `ListView` 或 `VirtualList`。

**屬性.** 除四個共享屬性外共 9 項：`item_count`: UInt (唯讀) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `current_row`: UInt · `item_height`: Float · `selected_count`: UInt (唯讀) · `selected_indices`: String (唯讀) · `hovered_row`: UInt (唯讀) · `font_size`: Float (唯讀) · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `item_activated`: UInt · `item_changed`: UInt · `item_selected`: UInt · `rename_requested`: UInt · `selection_changed`

**命令.** `add_item`, `remove_item`, `clear`, `clear_selection`, `set_selection_mode`

**Rust API.** `create_list_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("list_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### SpinBox (`spin_box`)

帶步進按鈕、範圍、小數位、可選前後綴與特殊值的數值欄位。用於有界數字；滾輪式選擇用 `NumberPicker`，角度式操作用 `Dial`，非數值輸入用 `LineEdit`。

**屬性.** 除四個共享屬性外共 14 項：`minimum`: Number · `maximum`: Number · `value`: Number · `single_step`: Number · `decimals`: UInt · `prefix`: String · `suffix`: String · `display_text`: String (唯讀) · `value_text`: String (唯讀) · `special_value_text`: String · `wrapping`: Bool · `keyboard_tracking`: Bool · `direction`: Enum (`ltr` | `rtl`) · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `validation_failed`: String · `value_changed`: Int

**命令.** `set_range`, `set_value`, `step_up`, `step_down`

**Rust API.** `create_spin_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("spin_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ComboBox (`combo_box`)

帶下拉清單、可選可編輯文字與可見彈層狀態的條目欄位。用於固定選項集合與單一目前值；使用者可以輸入清單外的值時用 `EditableComboBox`；選項是命令選單而不是取值時用 `DropdownMenu`。

**屬性.** 除四個共享屬性外共 13 項：`item_count`: UInt (唯讀) · `insert_policy`: Enum (`no_insert` | `insert_at_top` | `insert_at_current` | `insert_at_bottom` | `insert_alphabetically`) · `size_adjust_policy`: Enum (`adjust_to_minimum_contents_length` | `adjust_to_contents` | `adjust_to_contents_on_first_show`) · `items`: StringList · `current_index`: UInt · `current_text`: String · `placeholder_text`: String · `minimum_contents_length`: UInt · `editable`: Bool · `max_visible_items`: UInt · `open`: Bool · `direction`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `current_index_changed`: UInt or null · `current_text_changed`: String · `highlighted`: UInt · `popup_visibility_changed`: Bool · `text_edited`: String

**命令.** `set_items`, `set_current_index`, `clear`

**Rust API.** `create_combo_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("combo_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Dropdown (`dropdown`)

獨立下拉選擇器，帶佔位文字、展開狀態與可見條目數。欄位只是選擇器、不承載可編輯文字時用；控制項同時擁有文字值時用 `ComboBox`，彈層是命令選單時用 `MenuButton`。

**屬性.** 除四個共享屬性外共 8 項：`text`: String (唯讀) · `placeholder`: String · `selected_index`: UInt · `item_count`: UInt (唯讀) · `expanded`: Bool · `max_visible`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `changed` · `highlighted`: UInt

**命令.** `set_items`, `set_selected_index`, `set_expanded`, `toggle`

**Rust API.** `create_dropdown(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dropdown", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `dropdown_widget`, `combo`

### TextArea (`text_area`)

多行純文字輸入，支援軟換行、內容邊距、區塊數上限與橫向捲動。用於段落與自由筆記；需要帶邊框、按列換行的編輯器控制項時用 `TextEdit`，結構化原始碼用 `CodeEditor`。

**屬性.** 除四個共享屬性外共 8 項：`text`: String · `placeholder`: String · `read_only`: Bool · `max_length`: UInt · `soft_wrap`: Bool · `content_margin`: UInt · `max_block_count`: UInt · `scroll_x`: Int (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `context_menu_requested`: String · `cursor_position_changed`: UInt · `redo_available`: Bool · `selection_changed` · `undo_available`: Bool

**命令.** `set_text`, `set_placeholder`, `set_read_only`, `insert`, `delete_char`

**Rust API.** `create_textarea(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("text_area", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `multiline_edit`

### TextEdit (`text_edit`)

帶邊框的多行編輯器，支援選擇文字、游標位置與換行開關。用於需要編輯器外殼的通用多行欄位；需要格式屬性時用 `RichEdit`，帶提示符的附加式輸出用 `TerminalView`。

**屬性.** 除四個共享屬性外共 6 項：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `line_wrap`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cursor_position_changed`: UInt · `selection_changed` · `text_changed`: String

**命令.** `set_text`, `set_placeholder_text`, `set_max_length`, `set_read_only`, `set_line_wrap`

**Rust API.** `create_text_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("text_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### CommandLink (`command_link`)

帶標題、說明描述與可選盾牌標記的按鈕——Windows 風格的命令連結。用於對話框裡少量、需要解釋的選項；一行標題就夠時用一般 `Button`。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `description`: String · `shield`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `hovered`: Bool

**命令.** `set_text`, `set_description`, `set_enabled`, `click`

**Rust API.** `create_command_link(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("command_link", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### FontComboBox (`font_combo_box`)

以已安裝字型填入、每項用自身字型渲染、並帶過濾與範例文字的下拉框。用於 `FontDialog` 或設定表單裡的字型選擇；用 `FontPreview` 展示目前選擇的更大字樣。

**屬性.** 除四個共享屬性外共 9 項：`current_font_family`: String (唯讀) · `item_count`: Int (唯讀) · `current_index`: Int · `editable`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `max_visible_items`: Int · `font_filters`: String · `writing_system_filter`: String · `sample_text`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: Int · `current_font_changed`: String · `current_index_changed`: Int · `popup_hidden` · `popup_shown` · `text_edited`: String

**命令.** `set_current_index`, `set_editable`, `set_max_visible_items`, `show_popup`, `hide_popup`

**Rust API.** `create_font_combo_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("font_combo_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Keyboard (`keyboard`)

螢幕虛擬鍵盤，支援 qwerty 或數字佈局、大小寫狀態與按鍵事件；`text_committed` 攜帶要插入的字元。用於沒有硬體鍵盤的觸控裝置；同時保留硬體輸入路徑，桌面端不要展示它以免佔用螢幕。

**屬性.** 除四個共享屬性外共 3 項：`layout`: Enum (`qwerty` | `numeric`) · `lowercase`: Bool · `press_animation`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `backspace_pressed` · `enter_pressed` · `key_pressed`: UInt ×2 · `space_pressed` · `text_committed`: String

**命令.** `set_layout`, `set_lowercase`, `toggle_shift`

**Rust API.** `create_keyboard(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("keyboard", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `keyboard_widget`, `virtual_keyboard`

### Mention (`mention`)

在游標前補全 token、且一個欄位可包含多個提及的文字欄位。用於訊息或留言裡的 `@` 提及；整段欄位按單一建議補全時用 `AutoCompleteEdit`。

**屬性.** 除四個共享屬性外共 9 項：`text`: String · `trigger`: String · `candidate_count`: UInt (唯讀) · `visible_candidate_count`: UInt (唯讀) · `mention_count`: UInt (唯讀) · `popup_open`: Bool (唯讀) · `caret`: UInt (唯讀) · `max_visible`: UInt · `row_height`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `mention_inserted`: String · `popup_toggled`: Bool · `text_changed`: String

**命令.** `set_candidates`, `set_trigger`, `complete`, `open_popup`

**Rust API.** `create_mention(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mention", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `mention_edit`, `at_mention`

### Cascader (`cascader`)

多層選擇器，沿選項樹走一條路徑並回報所選路徑。用於每一層都有意義的階層分類；平面清單用 `ComboBox`，階層本身需要始終可見可瀏覽時用 `TreeView`。

**屬性.** 除四個共享屬性外共 8 項：`value`: String (唯讀) · `placeholder`: String · `depth`: UInt (唯讀) · `expanded`: Bool · `separator`: String · `filterable`: Bool · `filter`: String (唯讀) · `max_visible`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: list of UInt

**命令.** `set_options`, `set_selected_path`, `expand`, `collapse`

**Rust API.** `create_cascader(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cascader", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `cascade`, `multi_level_select`

### SearchBox (`search_box`)

單行搜尋欄位，前置搜尋入口與後置清除按鈕；`search_submitted` 攜帶查詢。用於清單或表格上方的快速過濾；還需要取消動作與平台搜尋樣式時用 `SearchBar`。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `search_submitted`: String · `text_changed`: String

**命令.** `set_text`, `set_placeholder`, `clear`

**Rust API.** `create_search_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("search_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `search`

### TagInput (`tag_input`)

按 Enter 或逗號產生可移除標籤的文字欄位，支援多行模式。用於自由形式的短標籤集合；集合固定、只改變選擇狀態時用 `Chip` 列控制項；token 指向人而不是任意標籤時用 `Mention`。

**屬性.** 除四個共享屬性外共 4 項：`tags`: String (唯讀) · `multiline`: Bool · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tags_changed`: list of String

**命令.** `add_tag`, `remove_tag`, `set_placeholder`

**Rust API.** `create_tag_input(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tag_input", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `tag_editor`, `chip_input`

### ImePreedit (`ime_preedit`)

在宿主給定位置繪製輸入法預編輯文字與游標的浮層。僅在自訂文字面上實作 IME 組合時使用；已經接受 IME 輸入的控制項（`LineEdit`、`TextArea` 等）會自行放置預編輯文字。

**屬性.** 除四個共享屬性外共 2 項：`text`: String · `cursor_position`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_text`, `set_cursor_position`

**Rust API.** `create_ime_preedit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("ime_preedit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `preedit`

### InplaceEditor (`inplace_editor`)

就地開始與結束編輯的小型編輯器，供表格與樹儲存格使用。用於 Enter 提交、Escape 放棄的儲存格編輯；欄位是表單常駐部分時用 `LineEdit`。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `editing`: Bool · `edit_triggers`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `edit_accepted`: String · `edit_canceled`

**命令.** `set_text`, `start_editing`, `finish_editing`

**Rust API.** `create_inplace_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("inplace_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `inline_editor`

### MaskedEdit (`masked_edit`)

由遮罩驅動的格式化文字欄位，拒絕不符合的輸入，並用 `input_rejected` 回報出錯位置。用於電話、日期、郵遞區號與形狀固定的識別碼；格式只是建議而非強制時用 `LineEdit` 加驗證。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `mask`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `input_rejected`: UInt · `return_pressed` · `text_changed`: String

**命令.** `set_text`, `set_mask`

**Rust API.** `create_masked_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("masked_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `masked_input`

### AutoCompleteEdit (`auto_complete_edit`)

整段值與候選清單匹配、並提供下拉的文字欄位，支援匹配模式、大小寫敏感、內聯補全與關閉策略。用於從建議中選擇單一值；token 補全用 `Mention`，清單封閉時用 `ComboBox`。

**屬性.** 除四個共享屬性外共 15 項：`text`: String · `placeholder`: String · `suggestion_count`: UInt (唯讀) · `selected_index`: UInt · `selected_suggestion`: String (唯讀) · `dropdown_visible`: Bool (唯讀) · `max_visible`: UInt · `alignment`: Enum (`left` | `centre` | `right`) · `can_undo`: Bool (唯讀) · `can_redo`: Bool (唯讀) · `match_mode`: Enum (`contains` | `starts_with` | `ends_with` | `fixed_string` | `regexp`) · `case_sensitive`: Bool · `inline_completion`: Bool · `close_policy`: Enum (`always` | `never` | `on_commit`) · `completion_prefix`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `suggestion_selected`: String · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_auto_complete_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("auto_complete_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `autocomplete`

### MultiSelectComboBox (`multi_select_combo_box`)

清單中保留多個選擇、並以選中集合回報的下拉框。用於緊湊多選欄位；選項需要始終可見的獨立列時用 `CheckListBox`；使用者還可以輸入未提供的值時用 `TagInput`。

**屬性.** 除四個共享屬性外共 5 項：`selected_count`: UInt (唯讀) · `selected_items`: String (唯讀) · `placeholder`: String · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: list of UInt

**命令.** `set_expanded`

**Rust API.** `create_multi_select_combo_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("multi_select_combo_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `multi_combo`

### RangeSlider (`range_slider`)

雙把手滑桿，在同一軌道上選擇上下界，並以 `range_changed` 同時回報兩者。用於兩端在視覺上一起移動的粗略範圍；邊界必須精確鍵入時用兩個 `SpinBox`，日期範圍用 `DateRangePicker`。

**屬性.** 除四個共享屬性外共 11 項：`min_value`: Float · `step`: Float · `min_range`: Float · `tick_interval`: Float · `tick_labels`: Bool · `value_hint_enabled`: Bool · `max_value`: Float · `lower`: Float · `upper`: Float · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `range_changed`: Float ×2

**命令.** `set_lower`, `set_upper`, `set_range`

**Rust API.** `create_range_slider(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("range_slider", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `dual_slider`

### EditableComboBox (`editable_combo_box`)

欄位可以保存清單外文字的下拉框；`item_selected` 回報選擇，`text_changed` 回報鍵入。用於通常選擇、偶爾輸入的值；封閉集合用 `ComboBox`，需要邊輸入邊匹配建議時用 `AutoCompleteEdit`。

**屬性.** 除四個共享屬性外共 6 項：`text`: String · `item_count`: UInt (唯讀) · `cursor_position`: UInt · `max_visible_items`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `item_selected`: UInt · `return_pressed` · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_editable_combo_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("editable_combo_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `editable_combo`

### SearchBar (`search_bar`)

平台風格的搜尋欄位，帶取消動作、`search_submitted` 與 `text_changed`。用於 iOS 風格介面裡的搜尋-取消流程；桌面端緊湊、無取消入口的欄位用 `SearchBox`。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `search_submitted`: String · `text_changed`: String

**命令.** `set_text`, `set_placeholder`

**Rust API.** `create_search_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("search_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `search_field`

### ShortcutEditor (`shortcut_editor`)

按類別分組、可搜尋的快速鍵清單，支援過濾、空狀態與復原/重做可用性。用於快速鍵參考/編輯面板；任務是錄製一個新快速鍵時用 `KeySequenceEdit`。

**屬性.** 除四個共享屬性外共 8 項：`filter_text`: String · `shortcut_count`: UInt (唯讀) · `visible_count`: UInt (唯讀) · `category_count`: UInt (唯讀) · `categories`: String (唯讀) · `empty`: Bool (唯讀) · `can_undo`: Bool (唯讀) · `can_redo`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `shortcut_changed`: mixed (String parts)

**命令.** `set_filter_text`

**Rust API.** `create_shortcut_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("shortcut_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `keyboard_shortcut_editor`

### OtpInput (`otp_input`)

分段驗證碼欄位：每字元一格、自動前進，支援貼上、退格與完成訊號。用於一次性驗證碼與短 PIN；較長的格式化識別碼用 `MaskedEdit`，一般密碼用 `LineEdit`。

**屬性.** 除四個共享屬性外共 8 項：`value`: String · `length`: UInt · `masked`: Bool · `read_only`: Bool · `digits_only`: Bool · `separator`: String · `focused_index`: UInt (唯讀) · `is_complete`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `completed`: String · `submitted`: String · `value_changed`: String

**命令.** `insert_char`, `backspace`, `paste`, `clear`

**Rust API.** `create_otp_input(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("otp_input", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `otp`

### NumberPicker (`number_picker`)

縱向滾動的數字輪，顯示相鄰值，透過拖曳或輕掃而非逐步點擊改變。用於從短範圍中選擇有界數字；需要精確鍵入或逐步調整時用 `SpinBox`。

**屬性.** 除四個共享屬性外共 12 項：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wrap`: Bool · `suffix`: String · `prefix`: String · `decimals`: UInt · `special_value_text`: String · `row_count`: UInt (唯讀) · `selected_row`: UInt (唯讀) · `row_height`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `scroll_rows`, `set_value`, `set_range`, `set_step`

**Rust API.** `create_number_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("number_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `picker`

### RichEdit (`rich_edit`)

帶字元與段落格式屬性、唯讀與換行開關、游標/選擇事件的多行編輯器。用於富格式散文；來源是 Markdown 時用 `MarkdownEditor`，純文字用 `TextArea`。

**屬性.** 除四個共享屬性外共 5 項：`text`: String · `line_count`: UInt (唯讀) · `read_only`: Bool · `line_wrap`: Bool · `placeholder_text`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cursor_position_changed`: UInt · `read_only_changed`: Bool · `selection_changed`: (UInt, UInt) or null · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_rich_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("rich_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `rich_text_editor`

## 5. 顯示與指示器

顯示控制項回報狀態而不是接受輸入：進度、數值、圖示、狀態標記、色塊與小面積裝飾面。其中一些也是輸入組合的視覺半邊（例如 `Slider` 配一個數值標籤）。

使用者只讀不改時用顯示控制項：完成度用 `ProgressBar` 與 `ProgressCircle`，只有在直接操縱數值時才用 `Slider` 與 `ScrollBar`，儀表式讀數用 `Arc`、`Meter`、`LCDNumber` 與 `Roller`，緊湊狀態用 `Badge`、`Tag`、`Chip` 與 `Avatar`，分隔用 `Divider` 與 `Line`。不要讓顯示控制項成為資料的唯一儲存——設定顯示值的同時仍要保存模型。需要精確鍵入的數值不要只用 `Slider`，應與 `SpinBox` 配對。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `display` | `src/widget/display_widgets/` | 27 |

### Slider (`slider`)

沿軌道拖曳把手，在範圍內選擇整數，支援刻度、翻頁步進與追蹤。用於可直接操縱的粗略值；精確輸入用 `SpinBox`，角度式儀表用 `Dial`，雙邊界用 `RangeSlider`。

**屬性.** 除四個共享屬性外共 10 項：`minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `tick_position`: Enum (`none` | `above` | `below` | `both`) · `tick_interval`: Int · `tracking`: Bool · `slider_position`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_slider_position`

**Rust API.** `create_slider(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("slider", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ProgressBar (`progress_bar`)

有界填充指示器，可選文字標籤、方向與不確定模式。用於已知總量中的已知比例；緊湊圓環用 `ProgressCircle`，時長未知、只表達活動時用 `Spinner`。

**屬性.** 除四個共享屬性外共 9 項：`minimum`: Int · `maximum`: Int · `value`: Int · `text_visible`: Bool · `orientation`: Enum (`horizontal` | `vertical`) · `inverted_appearance`: Bool · `indeterminate`: Bool · `direction`: Enum (`ltr` | `rtl`) · `progress`: Float (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_orientation`

**Rust API.** `create_progress_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("progress_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ScrollBar (`scroll_bar`)

軌道上的滑塊，用於捲動視埠，支援閒置淡出、翻頁步進與滑塊比例。用於自訂可捲動表面旁邊；希望 crate 擁有視埠並自動放置捲軸時用 `ScrollArea`。

**屬性.** 除四個共享屬性外共 11 項：`idle_delay_ms`: UInt · `idle_opacity`: Float · `minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) · `slider_size`: Float (唯讀) · `slider_position`: Float (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_steps`, `set_orientation`

**Rust API.** `create_scroll_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("scroll_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### LCDNumber (`lcd_number`)

七段數碼讀數，可配置位數、進制與段樣式。用於儀表式數字（計數器、時鐘、測量值）；一般文字用 `Label`，數值還需要範圍語境時用 `Meter`。

**屬性.** 除四個共享屬性外共 7 項：`value`: Float · `min_value`: Float · `max_value`: Float · `num_digits`: Int · `small_decimal_point`: Bool · `mode`: Enum (`hex` | `dec` | `oct` | `bin`) · `segment_style`: Enum (`outline` | `filled` | `flat`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `overflow` · `value_changed`: Float

**命令.** `set_value`, `set_mode`, `set_segment_style`

**Rust API.** `create_lcd_number(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("lcd_number", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Arc (`arc`)

環形進度或指示弧，支援掃過角度、粗細、可選圓角端點與置中數值。用於緊湊的單值表盤；Material 風格進度環用 `ProgressCircle`，需要刻度與指針時用 `Gauge`。

**屬性.** 除四個共享屬性外共 10 項：`value_format`: String · `value`: UInt · `minimum`: UInt · `maximum`: UInt · `thickness`: UInt · `sweep_angle`: UInt · `start_angle`: Float · `indeterminate`: Bool · `rounded`: Bool · `show_value`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_value`, `set_range`

**Rust API.** `create_arc(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("arc", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `arc_widget`

### Spinner (`spinner`)

旋轉活動指示器，支援弧線或輻條樣式、速度、粗細與尺寸比例。用於請求進行中且時長未知時；已知比例時用 `ProgressBar`，操作結束要停止，避免永久動畫。

**屬性.** 除四個共享屬性外共 7 項：`active`: Bool · `track_color`: Color · `arc_color`: Color · `thickness`: UInt · `speed`: Float · `size_ratio`: Float · `style`: Enum (`arc` | `spokes`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `active_changed`: Bool

**命令.** `set_active`, `set_speed`, `set_thickness`, `set_size_ratio`

**Rust API.** `create_spinner(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("spinner", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `spinner_widget`, `loading_spinner`

### Roller (`roller`)

滾輪式選擇器，顯示目前選項及上下相鄰項；拖曳或輕掃改變選擇。用於觸控上的短小有序選項集；桌面用 `ComboBox`，數值用 `NumberPicker`。

**屬性.** 除四個共享屬性外共 4 項：`options`: StringList · `selected_index`: UInt · `visible_count`: UInt · `item_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_options`, `set_selected_index`, `set_visible_count`

**Rust API.** `create_roller(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("roller", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `roller_widget`, `scroll_wheel`

### Switch (`switch`)

開/關軌道與滑塊控制項，支援 Material、Cupertino 或 macOS 幾何。用於立即生效的設定；選擇屬於稍後提交的表單時用 `CheckBox`，設計要求 iOS 呈現時用 `CupertinoSwitch`。

**屬性.** 除四個共享屬性外共 4 項：`checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_checked`, `toggle`

**Rust API.** `create_switch(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("switch", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `switch_widget`, `toggle_switch`

### Line (`line`)

水平或垂直線，帶粗細與顏色，用作輕量分隔。自訂佈局中需要一條線時用；需要平台分隔樣式與方向屬性時用 `Divider`。

**屬性.** 除四個共享屬性外共 3 項：`orientation`: Enum (`horizontal` | `vertical`) · `thickness`: UInt · `color`: Color — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_orientation`

**Rust API.** `create_line(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("line", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `line_widget`

### Meter (`meter`)

帶範圍、刻度、可選單位與閾值帶的儀表。用於有實際量程的測量值；完整徑向表盤用 `Gauge`，緊湊圓環用 `Arc`，一般完成度用 `ProgressBar`。

**屬性.** 除四個共享屬性外共 8 項：`value`: UInt · `minimum`: UInt · `maximum`: UInt · `tick_count`: UInt · `show_tick_labels`: Bool · `unit`: String · `value_text`: String (唯讀) · `threshold_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_value`, `set_range`

**Rust API.** `create_meter(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("meter", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `meter_widget`

### EmojiPicker (`emoji_picker`)

字形選擇器外殼：分類、搜尋、最近使用與網格；字形表本身由呼叫方透過 `set_glyphs` 提供，因為 crate 不打包 emoji 資料。用於編輯器或反應選擇器，並在 `glyph_chosen` 中插入字元。

**屬性.** 除四個共享屬性外共 7 項：`recent_capacity`: UInt · `glyph_count`: UInt (唯讀) · `visible_glyph_count`: UInt (唯讀) · `tab_count`: UInt (唯讀) · `active_tab`: UInt · `search`: String · `recent_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `glyph_chosen`: String · `glyph_hover_changed`: String

**命令.** `set_glyphs`, `set_categories`, `set_search`, `choose`

**Rust API.** `create_emoji_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("emoji_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `emoji`

### MiniChart (`mini_chart`)

無座標軸的緊湊折線或柱狀圖，用於列、卡片或磚塊內。用於一眼可讀的趨勢；僅折線、帶容量與末端標記的內聯趨勢用 `Sparkline`，需要座標軸與互動時用圖表族。

**屬性.** 除四個共享屬性外共 1 項：`chart_type`: Enum (`line` | `bar`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `point_hover_changed`: UInt or null

**命令.** `set_chart_type`, `set_data`, `set_range`

**Rust API.** `create_mini_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mini_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `mini_chart_widget`

### ImageView (`image_view`)

顯示 `Image` 的控制項，支援縮放、旋轉、翻轉、對齊、邊距與縮放/平移偏移。用於單張靜態圖片；多圖帶縮圖用 `ImageGallery`，逐格動畫用 `AnimatedImage`，需要共享縮放與感興趣區域狀態時配 `ZoomPanController`。

**屬性.** 除四個共享屬性外共 6 項：`scaled`: Bool · `rotation`: Enum (`0` | `90` | `180` | `270`) · `flip_horizontal`: Bool · `flip_vertical`: Bool · `zoom_level`: Float · `pan_offset`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `double_clicked` · `image_changed` · `load_failed`: String

**命令.** `set_image`, `set_scaled`

**Rust API.** `create_image_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("image_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `image_viewer`

### MiniCanvas (`mini_canvas`)

小型即時模式繪圖表面，提供填充/描邊原語與清空。用於裝飾性繪製或自訂小標記；需要圖層、縮放與指標路由時用完整 `Canvas`；不要在沒有繪製入口的控制項裡硬塞繪圖邏輯。

**屬性.** 該 schema 沒有自己的屬性；每個控制項都帶四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `mouse_pressed` · `mouse_released`

**命令.** `fill_rect`, `draw_rect`, `draw_line`, `fill_circle`, `clear`

**Rust API.** `create_mini_canvas(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mini_canvas", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Badge (`badge`)

小型浮層標記，承載計數、圓點或短標籤，錨定在另一控制項上。用於未讀數與狀態點；它是註解而不是按鈕，可點擊行為應放在被錨定的控制項上。

**屬性.** 除四個共享屬性外共 4 項：`text`: String · `count`: Int · `dot_mode`: Bool · `level`: Enum (`info` | `success` | `warning` | `error`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `count_changed`: UInt · `level_changed`: String

**命令.** `set_text`, `set_count`

**Rust API.** `create_badge(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("badge", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `badge_widget`, `notification_badge`

### SkeletonLoader (`skeleton_loader`)

內容載入期間顯示的灰色佔位塊，支援矩形、圓形與文字列形狀。用於保持佈局並表達進度；內容到達後不要繼續顯示；沒有可預覽形狀時用 `Spinner`。

**屬性.** 除四個共享屬性外共 3 項：`active`: Bool · `text_line_rows`: UInt · `shape`: Enum (`rect` | `circle` | `text_line`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_active`

**Rust API.** `create_skeleton_loader(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("skeleton_loader", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `skeleton`, `shimmer`

### Divider (`divider`)

分隔相鄰區塊的細線，支援線型、方向、粗細與顏色。用於表單或清單中的組間分隔；自訂繪製裡需要獨立線條時用 `Line`。

**屬性.** 除四個共享屬性外共 4 項：`line_style`: Enum (`plain` | `sunken` | `raised`) · `orientation`: String · `thickness`: Float · `color`: Color — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_orientation`, `set_thickness`

**Rust API.** `create_divider(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("divider", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `divider_widget`, `separator`

### Rating (`rating`)

一列可選擇的星形或圖示，支援半步與數值。用於主觀評分；連續量用 `Slider`，評分還需要精確鍵入時用 `SpinBox`。

**屬性.** 除四個共享屬性外共 6 項：`value`: Float · `allow_half_step`: Bool · `max`: UInt · `star_size`: UInt · `fill`: Float (唯讀) · `display_text`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `rating_changed`: UInt

**命令.** `set_value`, `set_max`

**Rust API.** `create_rating(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("rating", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `rating_widget`, `star_rating`

### EmptyState (`empty_state`)

檢視無內容時顯示的佔位：訊息、描述與可選次要動作。用於內容可能為空的清單、表格或頁面；不要把它當載入指示器——那是 `SkeletonLoader` 或 `Spinner`。

**屬性.** 除四個共享屬性外共 3 項：`message`: String · `secondary_action_text`: String · `description`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_pressed`

**命令.** `set_message`, `set_description`

**Rust API.** `create_empty_state(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("empty_state", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `empty_placeholder`

### ColorHistory (`color_history`)

最近使用顏色的歷史，以色塊網格呈現，支援新增、刪除、選擇與懸停訊號。用於顏色選擇器旁，讓使用者回到最近的選擇；固定調色盤用 `Swatch`，單一目前顏色用 `ColorWell`。

**屬性.** 除四個共享屬性外共 1 項：`color_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `color_added`: Color · `color_hovered`: Color · `color_removed`: UInt · `color_selected`: Color · `context_menu_requested`: UInt

**命令.** 未發布命令。

**Rust API.** `create_color_history(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("color_history", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ColorWell (`color_well`)

緊湊色塊，顯示一個目前顏色並發出 `clicked` 與 `picker_requested`。用作顏色欄位的摺疊形態；收到 `picker_requested` 時開啟 `ColorPicker` 或 `ColorDialog`，色塊只持有目前值。

**屬性.** 除四個共享屬性外共 2 項：`color`: Color · `show_border`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `color_changed`: Color · `picker_requested`

**命令.** `set_color`

**Rust API.** `create_color_well(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("color_well", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### FloatingLabel (`floating_label`)

帶浮動標籤的文字欄位，欄位有內容或取得焦點時標籤浮到輸入上方。用於緊湊的 Material 風格表單欄位；欄位上方靜態標籤更合適時用 `LineEdit`；不要用於多行文字。

**屬性.** 除四個共享屬性外共 5 項：`text`: String · `label`: String · `placeholder`: String · `focused`: Bool · `floating_label_behavior`: Enum (`auto` | `always` | `never`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `text_changed`: String

**命令.** `set_text`, `set_placeholder`, `set_focused`

**Rust API.** `create_floating_label(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("floating_label", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `floating_input`

### FontPreview (`font_preview`)

字樣面板，用選定的字族與字號渲染範例字串。用於 `FontComboBox` 旁或 `FontDialog` 內，讓使用者以可讀尺寸看到選擇；它只是預覽，不持有字型選擇狀態。

**屬性.** 除四個共享屬性外共 3 項：`font_family`: String · `font_size`: Float · `preview_text`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_font_family`, `set_font_size`, `set_preview_text`

**Rust API.** `create_font_preview(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("font_preview", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `font_viewer`

### ProgressCircle (`progress_circle`)

圓形進度指示器，支援粗細、顏色、直徑、不確定模式與可選中心文字。用於緊湊完成度指示；表盤式掃過用 `Arc`，時長未知時用 `Spinner`。

**屬性.** 除四個共享屬性外共 7 項：`value`: Float · `thickness`: Float · `progress_color`: Color · `diameter`: UInt · `indeterminate`: Bool · `show_center_text`: Bool · `center_text_format`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `indeterminate_changed`: Bool · `value_changed`: Float

**命令.** `set_value`, `set_thickness`, `set_indeterminate`

**Rust API.** `create_progress_circle(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("progress_circle", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `circular_progress`

### Icon (`icon`)

按名稱或來源描述、以指定尺寸與顏色渲染圖示的控制項。用於工具列與空狀態裡的獨立字形；字形屬於標籤或按鈕時，設定該控制項的 `icon_source`，不要額外疊一個 `Icon`。

**屬性.** 除四個共享屬性外共 4 項：`icon_name`: String · `icon_source`: IconSpec · `size`: Float · `color`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_icon_name`, `set_size`

**Rust API.** `create_icon(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("icon", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `icon_widget`, `icon_display`

### Tag (`tag`)

緊湊膠囊標籤，用於分類或過濾內容，帶色調、可選圖示與關閉入口。用於單一唯讀分類標記；多條目可選/可重排的列用 `Chip`，使用者建立標籤時用 `TagInput`。

**屬性.** 除四個共享屬性外共 5 項：`text`: String · `icon`: String · `tone`: Enum (`neutral` | `primary` | `success` | `warning` | `error`) · `selected`: Bool · `closable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`: String · `selected_changed`: Bool

**命令.** 未發布命令。

**Rust API.** `create_tag(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tag", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `label_tag`

### Clock (`clock`)

類比時鐘面，顯示一天中的時間，可選秒針，時間來源可以是手動值或系統時鐘。用於時鐘展示；選日期用 `Calendar`，編輯時間用 `TimePicker`。

**屬性.** 除四個共享屬性外共 5 項：`hour`: UInt · `minute`: UInt · `second`: UInt · `show_seconds`: Bool · `time_source`: Enum (`manual` | `system`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tick`: UInt

**命令.** 未發布命令。

**Rust API.** `create_clock(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("clock", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `analog_clock`

## 6. 容器與佈局原語

容器持有子控制項：排列、裁剪、分頁、分割或給它們固定盒子。本族既包括傳統的標籤/分割/停靠外殼，也包括 Flutter 風格的單子佈局盒（`Center`、`Padding`、`Expanded` 等）。

容器用來解決*放置*，而不只是裝飾：內容可能超出視埠時用 `ScrollArea`，使用者可調整窗格時用 `Splitter`，多頁共享一個槽位時用 `TabWidget`，由程式切頁時用 `StackedWidget`，隱藏次要區塊用 `Accordion`/`CollapsiblePane`，線性或換行排列用 `Flex`/`FlowLayout`/`WrapLayout`。不要用容器實作模型驅動的列表（用檢視族），也不要把 `Frame` 當佈局引擎。`Spacer` 確實什麼都不畫——這是它的契約，不是缺失實作。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `container` | `src/widget/container_widgets/` | 27 |

### GroupBox (`group_box`)

帶標題的框架，用於歸組相關控制項，可選可勾選，以便用一個狀態啟用/停用整組。用於表單中帶標籤的區塊；沒有標題時用一般 `Frame`，需要摺疊時用 `Accordion`。

**屬性.** 除四個共享屬性外共 4 項：`title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_group_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("group_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Splitter (`splitter`)

可拖曳分隔條，把容器分成可調整大小的窗格，拖曳時回報佈局變化與即時預覽。用於使用者需要在兩個區域間取捨空間；同一行為在佈局族名稱下是 `split_layout`；比例永不變化時用固定 `Flex`。

**屬性.** 除四個共享屬性外共 2 項：`orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**命令.** `set_orientation`

**Rust API.** `create_splitter(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("splitter", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `pane_splitter`

### Toolbox (`tool_box`)

縱向或橫向的帶標題區塊堆疊，一次展開一個，支援互斥、非互斥或彈出模式。用於緊湊工具面板；頁面內容區塊用 `Accordion`，每個區塊是完整頁面時用 `TabWidget`。

**屬性.** 除四個共享屬性外共 4 項：`item_count`: UInt (唯讀) · `current_index`: UInt · `orientation`: Enum (`horizontal` | `vertical`) · `mode`: Enum (`exclusive` | `non_exclusive` | `popup`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt · `item_expanded_changed`: mixed (String parts)

**命令.** `add_item`, `remove_item`, `set_current_index`, `set_orientation`

**Rust API.** `create_tool_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tool_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ScrollArea (`scroll_area`)

可捲動視埠，裁剪並偏移一個更大的子控制項，水平與垂直捲動策略獨立，並可選黏性區域。整個子樹可能超出可見區域時使用；用 `StickyHeader` 配套控制項保持一致可見的條帶；不要在同方向巢狀捲動區域。

**屬性.** 除四個共享屬性外共 8 項：`widget_resizable`: Bool · `horizontal_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `vertical_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `scroll_position_x`: Int · `scroll_position_y`: Int · `content_width`: UInt · `content_height`: UInt · `sticky_region_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `scroll_position_changed`: Int ×2

**命令.** `set_widget_resizable`, `set_horizontal_scroll_bar_policy`, `set_vertical_scroll_bar_policy`

**Rust API.** `create_scroll_area(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("scroll_area", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### TabWidget (`tab_widget`)

標籤頁容器，僅顯示選中頁，標籤可關閉、可移動，位置與形狀可設定。每頁是需要自身佈局的容器時使用；輕量並列頁面用 `TabView`，由程式切頁且不需要標籤列時用 `StackedWidget`。

**屬性.** 除四個共享屬性外共 9 項：`tab_count`: UInt (唯讀) · `current_index`: UInt · `text`: String · `title`: String · `closable`: Bool · `movable`: Bool · `tab_position`: Enum (`north` | `south` | `west` | `east`) · `tab_shape`: Enum (`rounded` | `triangular` | `rectangular`) · `document_mode`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `current_changed`: UInt · `drag_finished`: String · `tab_bar_clicked`: UInt · `tab_bar_double_clicked`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`, `set_tab_position`, `set_tab_shape`

**Rust API.** `create_tab_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tab_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### StackedWidget (`stacked_widget`)

一次顯示一個子頁面、由程式按索引選擇的容器。用於由宿主決定頁面的精靈式或模式化內容；使用者選頁時用 `TabWidget`，存在返回歷史時用 `NavigationStack`。

**屬性.** 除四個共享屬性外共 2 項：`widget_count`: UInt (唯讀) · `current_index`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `add_widget`, `remove_widget`, `set_current_index`

**Rust API.** `create_stacked_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("stacked_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `stacked`

### CollapsiblePane (`collapsible_pane`)

帶可點擊標題、用於展開或隱藏一個內容子控制項的區塊。用於單個可選詳情塊；多個此類區塊需要協同用 `Accordion`，區塊始終展開用 `GroupBox`。

**屬性.** 除四個共享屬性外共 2 項：`title`: String · `collapsed`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_collapsed`, `toggle`

**Rust API.** `create_collapsible_pane(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("collapsible_pane", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `collapsible`

### DockWidget (`dock_widget`)

一個可停靠面板，帶標題，可浮動或停靠到邊緣。用於使用者可以重新排列的單個工具面板；多區域停靠系統不是這個控制項，`MdiArea` 承載獨立子視窗而不是停靠邊緣。

**屬性.** 除四個共享屬性外共 3 項：`title`: String · `floating`: Bool · `docked`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dock_location_changed`: String · `features_changed`: String · `top_level_changed`: Bool

**命令.** `set_title`, `set_floating`, `set_features`, `set_allowed_areas`

**Rust API.** `create_dock_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dock_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `dock`

### MdiArea (`mdi_area`)

承載多個獨立子視窗的工作區，支援子視窗或標籤檢視，並追蹤作用中子視窗。用於以文件為中心的桌面應用；簡單頁面用 `TabWidget`，單一線性歷史用 `NavigationStack`。

**屬性.** 除四個共享屬性外共 3 項：`subwindow_count`: UInt (唯讀) · `active_subwindow`: UInt (唯讀) · `view_mode`: Enum (`sub_window_view` | `tabbed`) (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `subwindow_activated`: UInt · `subwindow_title_changed`: mixed (String parts)

**命令.** `add_subwindow`, `remove_subwindow`, `set_view_mode`, `activate_subwindow`

**Rust API.** `create_mdi_area(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mdi_area", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `mdi`

### Stepper (`stepper`)

兩側帶加減按鈕、按固定步長調整數值的數字欄位。用於小的整數數量（計數、購物車數量）；使用者還可以鍵入數值時用 `SpinBox`，滾輪用 `NumberPicker`。

**屬性.** 除四個共享屬性外共 4 項：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `set_value`, `set_minimum`, `set_maximum`, `set_step`

**Rust API.** `create_stepper(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("stepper", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `stepper_widget`, `step_control`

### MasonryLayout (`masonry_layout`)

Pinterest 風格瀑布流網格，把條目放入最短欄，支援欄數與間距。用於高度不定的卡片（照片、筆記）；列需要對齊時用 `FlowLayout`/`WrapLayout`，每個儲存格同尺寸時用 `Grid`。

**屬性.** 除四個共享屬性外共 3 項：`column_count`: UInt · `item_count`: UInt (唯讀) · `item_spacing`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** `set_column_count`

**Rust API.** `create_masonry_layout(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("masonry_layout", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `waterfall_layout`

### SafeArea (`safe_area`)

按上、下、左、右內縮內容的容器，避免瀏海、狀態列與 Home 指示條重疊。用於行動端螢幕根部；內縮值應來自平台查詢，而不是硬編碼裝置度量。

**屬性.** 除四個共享屬性外共 4 項：`top_inset`: Float · `bottom_inset`: Float · `left_inset`: Float · `right_inset`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_top_inset`, `set_bottom_inset`, `set_left_inset`, `set_right_inset`

**Rust API.** `create_safe_area(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("safe_area", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `safe_area_insets`

### Carousel (`carousel`)

可橫向滑動的頁面輪播，帶圓點、條或數字指示器，可選循環與自動播放。用於少量整寬頁面；頁索引需要放在輪播之外時配 `Pagination`；頁面由標籤而非滑動選擇時用 `TabView`。

**屬性.** 除四個共享屬性外共 7 項：`current_index`: UInt · `item_count`: UInt (唯讀) · `current_page_title`: String (唯讀) · `loop`: Bool · `autoplay_interval`: UInt · `indicator_style`: Enum (`dots` | `bars` | `numeric` | `hidden`) · `indicator_position`: Enum (`bottom` | `top` | `left` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `page_changed`: UInt

**命令.** 未發布命令。

**Rust API.** `create_carousel(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("carousel", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `carousel_widget`, `swipe_view`

### Spacer (`spacer`)

在線性佈局中消耗剩餘空間的空白不可見區域，帶彈性權重、基礎尺寸與可選繪製提示。用於把同級控制項推開；它什麼都不畫是契約，因此不要期待背景或邊框。

**屬性.** 除四個共享屬性外共 4 項：`flex`: UInt · `base_size`: UInt · `paint_hint`: Bool · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_spacer(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("spacer", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `flexible_space`

### Center (`center`)

單子盒子，把內容置中，可選按父容器寬高的比例。用於置中表單或訊息；需要中心以外的九個錨點之一時用 `Align`。

**屬性.** 除四個共享屬性外共 6 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `width_factor`: Float · `height_factor`: Float · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_center(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("center", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `centre`, `center_widget`

### Align (`align`)

單子盒子，把內容放在九個錨點之一。用於把子控制項固定到角或邊；中心情形用 `Center`；偏移是固定內縮而不是錨點時用 `Padding`。

**屬性.** 除四個共享屬性外共 5 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `alignment`: Enum (`top_left` | `top_centre` | `top_right` | `centre_left` | `centre` | `centre_right` | `bottom_left` | `bottom_centre` | `bottom_right`) · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_align(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("align", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `alignment_widget`

### SizedBox (`sized_box`)

具有固定寬高、包含一個子控制項的盒子。用於預留精確槽位（`box_width`、`box_height`）；一個維度應隨另一個變化時用 `AspectRatio`，尺寸應隨內容變化時用 `IntrinsicWidth`。

**屬性.** 除四個共享屬性外共 6 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `box_width`: UInt · `box_height`: UInt · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_sized_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("sized_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `size_box`

### Padding (`padding`)

單子盒子，用四個獨立邊距內縮內容。用於屬於內容而不是父容器的間距；同級之間的間隙用父容器的 `spacing`/`gap`。

**屬性.** 除四個共享屬性外共 8 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `padding_top`: UInt · `padding_right`: UInt · `padding_bottom`: UInt · `padding_left`: UInt · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_padding(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("padding", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `padding_widget`

### Expanded (`expanded`)

沿主軸佔據剩餘空間、按 `flex` 加權的子控制項，必須放在線性佈局內。用於讓一個子控制項吸收餘量；子控制項也可以停在偏好尺寸而不填滿時用 `Flexible`。

**屬性.** 除四個共享屬性外共 5 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_expanded(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("expanded", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `expand`

### Flexible (`flexible`)

可以收縮到下限、但最多成長到偏好尺寸的子控制項，`fit` 選擇寬鬆或緊緻約束。用於同級控制項應讓出空間但不一定填滿時；必須填滿時用 `Expanded`。

**屬性.** 除四個共享屬性外共 6 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `fit`: Enum (`loose` | `tight`) · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_flexible(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("flexible", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `flex_child`

### IntrinsicWidth (`intrinsic_width`)

寬度取內容自身偏好寬度的盒子，使欄位或標籤不被拉伸。用於收縮適配的欄；精確寬度用 `SizedBox`，高度需要隨寬度變化時用 `AspectRatio`。

**屬性.** 除四個共享屬性外共 4 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_intrinsic_width(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("intrinsic_width", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `shrink_wrap`

### AspectRatio (`aspect_ratio`)

透過固定比例從一個維度推導另一個維度的盒子，可選服從父約束。用於媒體磚塊與縮圖；兩個維度都已知時用 `SizedBox`，內容應決定寬度時用 `IntrinsicWidth`。

**屬性.** 除四個共享屬性外共 6 項：`content`: String · `content_width`: UInt · `content_height`: UInt · `ratio`: Float · `respect_parent`: Bool · `widget`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_aspect_ratio(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("aspect_ratio", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ratio_box`

### Flex (`flex`)

沿列或欄按權重分配子控制項的線性佈局，支援換行、主軸對齊、交叉軸對齊與間隙。用於大多數一維佈局；二維用 `Grid`，需要按測量尺寸換行時用 `FlowLayout`/`WrapLayout`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `direction`: Enum (`row` | `row_reverse` | `column` | `column_reverse`) · `wrap`: Enum (`no_wrap` | `wrap` | `wrap_reverse`) · `justify_content`: Enum (`flex_start` | `flex_end` | `center` | `space_between` | `space_around` | `space_evenly`) · `align_items`: Enum (`stretch` | `flex_start` | `flex_end` | `center` | `baseline`) · `gap`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未發布命令。

**Rust API.** `create_flex(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("flex", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `flex_box`

### FlowLayout (`flow_layout`)

按閱讀順序放置子控制項、列滿後開始新列的佈局，每一列由子控制項自身提示決定尺寸，並遵循書寫方向。用於應自然流動的標籤、chip 與工具按鈕；列是固定尺寸儲存格網格時用 `WrapLayout`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `padding`: UInt · `wrap`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未發布命令。

**Rust API.** `create_flow_layout(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("flow_layout", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `flow`

### WrapLayout (`wrap_layout`)

放置固定尺寸子控制項、滿後換到下一列的佈局，支援獨立間距與列間距。用於等大儲存格的畫廊；子控制項應保持自身測量尺寸並遵循書寫方向時用 `FlowLayout`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `run_spacing`: UInt · `padding`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未發布命令。

**Rust API.** `create_wrap_layout(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("wrap_layout", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `wrap`

### Accordion (`accordion`)

縱向可摺疊區塊堆疊，支援單展開或多展開模式。用於 FAQ 式或設定分組；單個獨立區塊用 `CollapsiblePane`，區塊是工具面板而非頁面內容時用 `ToolBox`。

**屬性.** 除四個共享屬性外共 3 項：`section_count`: UInt (唯讀) · `single_expand`: Bool · `expanded_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `section_toggled`: mixed (String parts)

**命令.** 未發布命令。

**Rust API.** `create_accordion(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("accordion", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `collapsible_list`

### Splitter (`split_layout`)

`Splitter` 的佈局族拼寫：窗格間的可拖曳分隔條，帶方向與窗格數。分割屬於佈局描述而不是控制項子樹時使用；它回報與 `Splitter` 相同的三個佈局訊號。

**屬性.** 除四個共享屬性外共 2 項：`orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**命令.** `set_orientation`

**Rust API.** `create_split_layout(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("split_layout", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `split_view`

## 7. 模型驅動的條目檢視

檢視呈現模型：列、欄、樹以及它們周圍的外殼。它們負責捲動、選擇、焦點，部分還負責編輯，並且只向宿主提供的資料來源索取目前可見的列。

按模型的*形狀*選擇：平面清單用 `ListView` 或 `VirtualList`，樹用 `TreeView`，網格用 `Table`、`GridTable` 或 `VirtualTable`，帶欄的樹用 `TreeTable`。排序、篩選與凍結欄是功能的一部分時用 `DataGrid`；資料本身是條件集合或具名屬性集合時用 `QueryBuilder`/`PropertyGrid`/`PropertiesPanel`。不要用 `ListBox` 承載數千列，不要用 `Table` 表達階層資料，也不要把 `ListView` 當表單：本 crate 的檢視不在列內承載任意控制項。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `view` | `src/widget/view_widgets/` | 15 |

### ListView (`list_view`)

模型驅動的清單，支援檢視模式（清單、圖示、詳情、縮圖）、焦點列與選擇。用於攜帶模型或委派的家；小型純文字清單用 `ListBox`，模型大到需要視窗化投影時用 `VirtualList`。

**屬性.** 除四個共享屬性外共 7 項：`has_model`: Bool (唯讀) · `row_count`: UInt (唯讀) · `focused_row`: UInt · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `view_mode`: Enum (`list` | `icon` | `details` | `thumbnails`) · `alignment`: Enum (`left` | `centre` | `right`) · `icon_visible_in_list_mode`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `focused_row_changed`: UInt or null · `row_activated`: UInt · `row_reordered`: UInt ×2 · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_list_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("list_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### TreeView (`tree_view`)

階層檢視，可展開/摺疊節點，帶焦點節點、選中節點、延遲載入與動畫展開。用於樹模型；每列還需要欄時用 `TreeTable`；`ColumnView` 是同一樹族的別名角色，而不是第二個控制項。

**屬性.** 除四個共享屬性外共 10 項：`has_model`: Bool (唯讀) · `node_count`: UInt (唯讀) · `focused_node`: UInt · `selected_node`: UInt (唯讀) · `has_delegate`: Bool (唯讀) · `expanded_focused_node`: Bool (唯讀) · `expansion_animated`: Bool · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `collapsed`: UInt · `expanded`: UInt · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `focused_node_changed`: UInt or null · `node_double_clicked`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_node`, `expand`, `collapse`, `toggle`

**Rust API.** `create_tree_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tree_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Table (`table_widget`)

儲存格表格，帶表頭、目前列、選擇模式與編輯請求訊號。用於規模適中的平面網格；需要排序、篩選與凍結欄時用 `DataGrid`，大模型用 `VirtualTable`。

**屬性.** 除四個共享屬性外共 8 項：`has_model`: Bool (唯讀) · `has_delegate`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("table_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Table (`table`)

同一表格控制項在 `table_widget` 之外的 `table` 工廠名稱；兩個名稱接受相同的屬性與事件契約。按宿主已有拼寫選擇其一，不要為同一個控制項同時註冊兩個。

**屬性.** 除四個共享屬性外共 8 項：`has_model`: Bool (唯讀) · `has_delegate`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("table", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Table (`data_grid`)

資料網格，帶資料來源、排序、篩選、凍結欄、列高欄寬、選擇行為與彙總。排序/篩選是功能一部分時使用；更輕的網格用 `GridTable`，列有階層時用 `TreeTable`。

**屬性.** 除四個共享屬性外共 18 項：`has_data_source`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `frozen_columns`: UInt · `sort_spec_count`: UInt (唯讀) · `filter_count`: UInt (唯讀) · `sort_specs`: String · `filters`: String · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) · `selection_behavior`: Enum (`cells` | `rows` | `columns`) · `sort_indicator_visible`: Bool · `section_resize_mode`: Enum (`fixed` | `stretch` | `interactive`) · `current_cell`: String (唯讀) · `summary_text`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `filter_changed`: String · `header_clicked`: UInt · `page_applied`: UInt · `sort_changed`: mixed (String parts) · `visible_window_changed`: UInt ×4

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_frozen_columns`

**Rust API.** `create_data_grid(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("data_grid", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### TreeTable (`tree_table`)

階層表格：列構成可展開/摺疊的樹，每列還帶欄儲存格，支援排序角色、大小寫敏感與視窗化/即時投影。用於節點還帶表格屬性的樹；純樹用 `TreeView`，平面列用 `Table`。

**屬性.** 除四個共享屬性外共 15 項：`has_model`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `selected_row`: UInt · `row_height`: UInt · `column_width`: UInt · `header_visible`: Bool · `expands_on_double_click`: Bool · `middle_click_expands`: Bool · `sort_role`: Enum (`text` | `number`) · `case_sensitive`: Bool · `projection_strategy`: Enum (`windowed` | `eager`) · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `expansion_changed`: mixed (String parts) · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `projection_changed`: UInt · `selection_changed`: UInt or null · `sort_changed`: mixed (String parts)

**命令.** `set_model`, `clear_model`, `expand_row`, `collapse_row`, `select_row`

**Rust API.** `create_tree_table(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tree_table", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Table (`virtual_table`)

基於資料來源、只投影可見視窗的表格，支援列欄過掃描與滾輪捲動模式。用於大型遠端資料表；需要排序/篩選且模型在本機時用 `DataGrid`。

**屬性.** 除四個共享屬性外共 12 項：`has_data_source`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `overscan_rows`: UInt · `overscan_columns`: UInt · `visible_window`: String (唯讀) · `wheel_scroll_mode`: Enum (`per_row` | `per_page`) · `header_visible`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: (UInt, UInt) or null · `visible_window_changed`: UInt ×4

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_overscan_rows`, `set_overscan_columns`, `fetch_visible_window`

**Rust API.** `create_virtual_table(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("virtual_table", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### DataView (`virtual_list`)

基於資料來源的單欄虛擬清單，支援可見視窗、過掃描與多選。用於等高空列的長清單；列需要檢視模式或委派時用 `ListView`，短小已物化清單用 `ListBox`。

**屬性.** 除四個共享屬性外共 8 項：`has_data_source`: Bool (唯讀) · `row_count`: UInt (唯讀) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (唯讀) · `selected_rows`: NumberList (唯讀) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**命令.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_virtual_list(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("virtual_list", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### DataView (`data_view`)

`VirtualList` 的 `data_view` 拼寫；它呈現一欄模型列，不切換顯示模式。宿主希望以通用 data-view 名稱使用虛擬清單契約時用它，不要期待第二套實作。

**屬性.** 除四個共享屬性外共 8 項：`has_data_source`: Bool (唯讀) · `row_count`: UInt (唯讀) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (唯讀) · `selected_rows`: NumberList (唯讀) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**命令.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_data_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("data_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### GridTable (`grid_table`)

功能豐富的虛擬化表格，帶網格線、表頭、排序、選擇與資料來源。用於表單內的電子表格式表面；需要篩選/凍結欄時用 `DataGrid`，表頭需要在網格外共享時配 `GridViewHeader`。

**屬性.** 除四個共享屬性外共 12 項：`has_data_source`: Bool (唯讀) · `row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `selection_mode`: Enum (`none` | `cell` | `row` | `column`) · `sort_spec_count`: UInt (唯讀) · `selected_cell`: String (唯讀) · `sorting_enabled`: Bool · `sort_role`: String · `auto_refresh`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_double_clicked`: UInt ×2 · `cell_selected`: UInt ×2 · `context_menu_requested`: String · `edit_requested`: UInt ×2 · `header_clicked`: UInt · `sort_changed`: mixed (String parts)

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `toggle_sort_column`, `clear_selection`

**Rust API.** `create_grid_table(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("grid_table", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### QueryBuilder (`query_builder`)

把遞迴過濾條件編輯成「欄位、運算子、運算元」列的建構器，帶連接詞、即時更新與提交延遲。用於需要可匯出文字的結構化篩選；表格旁的快速一列篩選用 `FilterRow`，自由文字搜尋用 `SearchBox`。

**屬性.** 除四個共享屬性外共 8 項：`row_count`: UInt (唯讀) · `incomplete_row_count`: UInt (唯讀) · `field_count`: UInt (唯讀) · `conjunction`: Enum (`and` | `or`) · `active_row`: UInt · `live_update`: Bool · `commit_delay_ms`: UInt · `has_pending_commit`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `query_changed`: String · `query_committed`: String · `row_field_clicked`: UInt · `row_operator_clicked`: UInt · `rows_changed`: list of Records

**命令.** `set_fields`, `add_row`, `remove_row`, `toggle_conjunction`

**Rust API.** `create_query_builder(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("query_builder", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `filter_builder`

### PropertiesPanel (`properties_panel`)

分類屬性編輯器，以網格佈局排列列，並回報編輯、檔案選擇請求與編輯器開啟。用於畫布旁的檢查器；較扁平的兩欄表單用 `PropertyGrid`，分類重要時用 `PropertiesPanel`。

**屬性.** 除四個共享屬性外共 3 項：`property_count`: UInt (唯讀) · `description_mode`: Enum (`tooltip` | `status_bar` | `hidden`) · `edit_trigger`: Enum (`single_click` | `double_click`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editor_opened`: String · `file_picker_requested`: String · `property_changed`: String ×2

**命令.** 未發布命令。

**Rust API.** `create_properties_panel(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("properties_panel", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `property_panel`

### PropertyGrid (`property_grid`)

兩欄編輯器，列出具名屬性與可編輯值儲存格，帶選中列與逐儲存格變化訊號。用於緊湊鍵/值檢查器；列需要按類別分組時用 `PropertiesPanel`。

**屬性.** 除四個共享屬性外共 2 項：`property_count`: UInt (唯讀) · `selected_index`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_changed`: mixed (String parts) · `context_menu_requested`: String · `selected`: UInt

**命令.** `add_property`, `set_value`, `clear`, `set_selected_index`

**Rust API.** `create_property_grid(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("property_grid", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `inspector`

### ImageGallery (`image_gallery`)

圖片瀏覽器，帶主圖、縮圖列、滾輪模式與全螢幕請求。用於需要顯示目前索引的照片集合；單張圖片用 `ImageView`，縮圖列很大時配 `ThumbnailCache`。

**屬性.** 除四個共享屬性外共 7 項：`current_index`: UInt · `thumbnail_size`: UInt · `show_thumbnails`: Bool · `strip_offset`: UInt · `strip_scroll_enabled`: Bool · `wheel_mode`: Enum (`strip` | `navigate` | `zoom`) · `arrow_key_navigation_enabled`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `fullscreen_requested`: UInt · `image_changed`: UInt · `selection_changed`: list of UInt · `strip_scrolled`: UInt

**命令.** `set_current_index`

**Rust API.** `create_image_gallery(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("image_gallery", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `gallery`

### GridViewHeader (`grid_view_header`)

網格或表格的共享表頭列：可排序、可調整的區段與篩選入口。表頭需要位於捲動主體之外時使用，並把 `section_clicked`、`section_resized`、`filter_clicked` 回報給持有模型的網格。

**屬性.** 除四個共享屬性外共 6 項：`columns`: StringList · `column_count`: UInt (唯讀) · `sort_indicator_visible`: Bool · `resize_mode`: Enum (`fixed` | `stretch` | `stretch_last` | `interactive`) · `filter_icon_visible`: Bool · `sort_columns`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `filter_clicked`: UInt · `section_clicked`: UInt · `section_resized`: UInt ×2

**命令.** 未發布命令。

**Rust API.** `create_grid_view_header(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("grid_view_header", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `table_header_bar`

## 8. 圖表與儀表

圖表族是共享繪圖引擎之上的控制項層：折線、柱狀、圓餅、迷你趨勢圖，以及徑向 `Gauge`。引擎負責座標軸、刻度、邊距與圖例位置；控制項負責模型、訊號與類型語意。

有序序列用 `LineChart`，分類比較用 `BarChart`，少量扇區的佔比快照用 `PieChart`，無軸的緊湊內聯趨勢用 `Sparkline`，單值對範圍用 `Gauge`。不要用圖表顯示 OHLC 行情（金融族）、熱力圖或雷達圖（特殊族），也不要為單一數字使用圖表——`Label` 或 `LCDNumber` 更清楚。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `chart` | `src/widget/chart_widgets/` | 5 |

### LineChart (`line_chart`)

用於一個或多個有序序列的笛卡爾折線圖，透過共享圖表引擎繪製。用於有序軸上的趨勢；趨勢需要內聯且無需座標軸時用 `Sparkline`，技術指標用 `IndicatorChart`。

**屬性.** 除四個共享屬性外共 1 項：`stroke_width`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `point_clicked`: UInt ×2 · `point_hovered`: UInt ×2 · `point_unhovered`: UInt ×2

**命令.** `set_stroke_width`

**Rust API.** `create_line_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("line_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `line_graph`

### Sparkline (`sparkline`)

無座標軸、網格與邊距的緊湊內聯趨勢，由值列表建構，帶末端標記與懸停回報。用於表格儲存格或 KPI 磚塊；需要座標軸或縮放時用 `LineChart`。它有意不接入圖表引擎，因為它沒有引擎要統一的軸、網格、刻度與邊距。

**屬性.** 除四個共享屬性外共 7 項：`data`: NumberList · `stroke_width`: Float · `show_end_value`: Bool · `fill`: Bool · `show_points`: Bool · `show_last_marker`: Bool · `capacity`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `last_value_clicked`: Float · `value_hovered`: mixed (String parts)

**命令.** `set_stroke_width`

**Rust API.** `create_sparkline(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("sparkline", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `sparkline_chart`

### BarChart (`bar_chart`)

用於分類比較的縱向柱狀圖，支援柱間距與逐柱懸停/點擊訊號。用於少量分類；有序序列用 `LineChart`，佔比用 `PieChart`，多軸輪廓用 `RadarChart`。

**屬性.** 除四個共享屬性外共 1 項：`bar_spacing`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt

**命令.** `set_bar_spacing`

**Rust API.** `create_bar_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("bar_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `bar_graph`

### PieChart (`pie_chart`)

帶彩色扇區的圓形圖，可選環形，回報指標下的扇區與被點擊的扇區。用於少量扇區的佔比快照；分類很多或數值接近時用 `BarChart`。

**屬性.** 除四個共享屬性外共 1 項：`donut`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slice_clicked`: UInt · `slice_hovered`: UInt · `slice_unhovered`: UInt

**命令.** `set_donut`

**Rust API.** `create_pie_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("pie_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `pie_graph`

### Gauge (`gauge`)

徑向表盤，用範圍、色帶、刻度、指針樣式與可選單位標籤顯示單一值。用於有實際量程的單一測量值；線性儀表用 `Meter`，完成度用 `Arc`/`ProgressCircle`。

**屬性.** 除四個共享屬性外共 14 項：`value`: Float · `minimum`: Float · `maximum`: Float · `label`: String · `unit`: String · `zone_count`: UInt (唯讀) · `animated`: Bool · `thickness`: UInt · `bands`: StringList · `tick_count`: UInt · `start_angle`: Float · `end_angle`: Float · `needle_style`: Enum (`none` | `line` | `triangle`) · `unit_label`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `band_clicked`: String · `value_changed`: Float

**命令.** 未發布命令。

**Rust API.** `create_gauge(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("gauge", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `dial_gauge`

## 9. 選單、工具列與命令面

本族承載命令外殼：選單列、下拉與內容選單、工具列、狀態列、單一動作，以及開啟選單或拆成兩個動作的按鈕。

應用層級命令分組用 `MenuBar`，由按鈕或次要點擊開啟的命令清單用 `Menu` 或 `ContextMenu`，常用動作放 `ToolBar`，簡短狀態列用 `StatusBar`，需要「描述一次、多處渲染」的命令用 `Action`。不要把選單當帶選擇狀態的資料清單（那是檢視），也不要把命令狀態存進 `Menu`——把狀態放在 `Action` 的註解和宿主模型裡。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 |

### Action (`action`)

可由選單與工具列渲染的命令描述：文字、圖示、快速鍵、可勾選/選中狀態、分隔符標記與命令 id。用於同一命令出現在多個命令面；不要把它單獨當可見按鈕——具體控制項由命令面實例化。

**屬性.** 除四個共享屬性外共 8 項：`text`: String · `icon_text`: String · `shortcut`: String · `checkable`: Bool · `checked`: Bool · `icon_source`: IconSpec · `separator`: Bool (唯讀) · `command_id`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `hovered` · `toggled`: Bool · `triggered`: Bool

**命令.** `set_text`, `set_checkable`, `set_checked`, `trigger`

**Rust API.** `create_action(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("action", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `command_action`

### Menu (`menu`)

命令與子選單的下拉清單，帶懸停、動作觸發、子選單請求與顯示/隱藏前訊號。用於從選單列、按鈕或內容點擊開啟的命令；彈層選擇值時用 `DropdownMenu`，按鈕擁有選單時用 `MenuButton`。

**屬性.** 除四個共享屬性外共 6 項：`title`: String · `item_count`: UInt (唯讀) · `hovered_index`: UInt (唯讀) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_menu(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("menu", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Menu (`context_menu`)

`ContextMenu` 是 `Menu` 的角色拼寫（`pub type ContextMenu = Menu`），以自己的名稱註冊，用於在指標處開啟的選單。用於次要點擊選單；它不是第二套選單實作，屬性與事件就是 `Menu` 的集合。

**屬性.** 除四個共享屬性外共 6 項：`title`: String · `item_count`: UInt (唯讀) · `hovered_index`: UInt (唯讀) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_context_menu(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("context_menu", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `context_actions`

### MenuBar (`menu_bar`)

頂層選單標題的橫向列，追蹤作用中與懸停條目。用於視窗頂部的應用命令分組；常用命令用 `ToolBar`，頁面內單一下拉用 `MenuButton`。

**屬性.** 除四個共享屬性外共 3 項：`entry_count`: UInt (唯讀) · `active_index`: UInt (唯讀) · `hovered_index`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_hovered`: String · `triggered`: String · `triggered_entry`: mixed (String parts)

**命令.** `clear`, `add_menu`, `remove_menu`

**Rust API.** `create_menu_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("menu_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ToolBar (`tool_bar`)

圖示按鈕與分隔條的條帶，可橫向或縱向、可移動、可浮動，並回報方向與可見性變化。用於常用命令；命令需要帶標籤的分組與標籤頁時用 `RibbonBar`，單個條目用 `ToolButton`。

**屬性.** 除四個共享屬性外共 5 項：`orientation`: Enum (`horizontal` | `vertical`) · `icon_size`: Float · `movable`: Bool · `floatable`: Bool · `item_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_triggered`: String · `customize_requested` · `float_changed`: Bool · `orientation_changed`: Bool · `top_level_changed`: Bool · `visibility_changed`: Bool

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_tool_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tool_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### StatusBar (`status_bar`)

視窗底部的簡短狀態列，帶訊息與尺寸抓手。用於瞬時狀態文字；`Snackbar` 也以這個類型註冊，但它是浮動通知，不要混淆——這裡設定 `message`，由宿主更新。

**屬性.** 除四個共享屬性外共 1 項：`message`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `message_changed`: String · `size_grip_dragged`: Int ×2

**命令.** `set_message`

**Rust API.** `create_status_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("status_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `status`

### ToolButton (`tool_button`)

用於工具列的緊湊圖示加標題按鈕，帶選中狀態與點擊/切換訊號。用於 `ToolBar` 或 `SplitButton` 群組內；標題是主要目標時用完整 `Button`，按鈕擁有彈層時用 `ToggleButton`。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `checked`: Bool · `icon_source`: IconSpec — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`: Bool · `toggled`: Bool · `triggered`

**命令.** `set_text`, `set_checked`

**Rust API.** `create_tool_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tool_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `toolbar_button`

### ToolButton (`split_button`)

拆成主操作與備選選單的工具按鈕，支援彈出方向、列高與捲動位置。用於一個預設操作、多個變體共享空間時；每次點擊都開啟選單時用 `MenuButton`。

**屬性.** 除四個共享屬性外共 7 項：`text`: String · `action_count`: UInt (唯讀) · `menu_open`: Bool · `row_height`: UInt · `popup_direction`: Enum (`down` | `up` | `auto`) · `max_menu_height`: UInt · `menu_scroll`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_selected`: String · `menu_toggled`: Bool · `triggered`: String

**命令.** `add_action`, `open_menu`, `close_menu`, `trigger_primary`

**Rust API.** `create_split_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("split_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### DropdownMenu (`dropdown_menu`)

帶條目清單、選中索引、展開狀態與子選單請求的下拉選擇器。彈層從清單選值時使用；欄位還需要在文字框中顯示目前值時用 `ComboBox`，條目是命令時用 `Menu`。

**屬性.** 除四個共享屬性外共 4 項：`item_count`: UInt (唯讀) · `selected_index`: UInt · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `item_selected`: String · `item_toggled`: String · `submenu_requested`: String

**命令.** `set_selected_index`, `set_expanded`

**Rust API.** `create_dropdown_menu(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dropdown_menu", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### MenuButton (`menu_button`)

點擊後開啟選單的按鈕，帶圖示、條目數與顯示/隱藏前及條目觸發訊號。用於頁面內單一下拉命令面；主操作也必須可點擊時用 `SplitButton`。

**屬性.** 除四個共享屬性外共 4 項：`text`: String · `icon_source`: IconSpec · `item_count`: UInt (唯讀) · `expanded`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `clicked` · `item_triggered`: UInt · `submenu_requested`: String

**命令.** `set_text`, `set_expanded`

**Rust API.** `create_menu_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("menu_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `dropdown_button`

## 10. 對話框與臨時面板

對話框是帶自身結果的獨立介面：訊息與確認框、檔案/顏色/字型/輸入選擇器、進度與精靈對話框，以及非模態的底部面板、氣泡與工具提示。

需要決策時用 `MessageBox` 或 alert/confirm 對話框；需要帶標題、承載一個內容控制項的外殼時用 `Dialog`；標準選擇器用 `FileDialog`/`ColorDialog`/`FontDialog`/`InputDialog`；可取消的長工作用 `ProgressDialog`；有序步驟用 `WizardDialog`。`BottomSheet`、`ModalBottomSheet`、`Popover` 與 `Tooltip` 用於不擁有視窗的臨時內容。不要用對話框表達非阻塞通知（`Banner`、`Toast`、`Snackbar`），也不要在另一個模態自己的堆疊裡再嵌模態——統一交給 `ModalHost`。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `dialog` | `src/widget/dialog/` | 17 |

### Dialog (`dialog`)

帶標題、承載一個內容控制項的對話框外殼，帶模態、接受/拒絕/結果與說明訊號。需要在自己的對話框框體內放內容時使用；標準決策用 `MessageBox` 與 alert/confirm 對話框，僅浮層外殼用 `PopupWindow`。

**屬性.** 除四個共享屬性外共 7 項：`title`: String · `modal`: Bool · `has_content`: Bool (唯讀) · `result`: Int (唯讀) · `accepted`: Bool (唯讀) · `rejected`: Bool (唯讀) · `closable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `closed` · `finished`: Int · `help_requested` · `opened` · `rejected`

**命令.** `accept`, `reject`

**Rust API.** `create_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### MessageBox (`message_box`)

帶圖示、提示與詳細文字、可選核取方塊與固定回應按鈕的模態訊息。用於簡短決策或確認；更風格化的提示用 `AlertDialog`/`ConfirmDialog`，內容自訂時用 `Dialog`。

**屬性.** 除四個共享屬性外共 10 項：`title`: String · `text`: String · `icon`: Enum (`none` | `information` | `question` | `warning` | `critical`) · `modal`: Bool · `informative_text`: String · `detailed_text`: String · `check_box`: String · `check_box_checked`: Bool · `default_button`: String · `escape_button`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `button_clicked`: String · `details_toggled`: Bool · `focus_changed`: UInt · `rejected`

**命令.** `set_text`, `set_title`, `set_icon`

**Rust API.** `create_message_box(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("message_box", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `msgbox`

### FileDialog (`file_dialog`)

模態檔案系統瀏覽器，支援開啟、多選開啟、儲存與選擇目錄模式，帶導覽歷史與側欄。使用者必須選擇檔案路徑時使用；`DirectoryDialog` 是這個控制項的目錄模式角色，不是獨立實作。

**屬性.** 除四個共享屬性外共 12 項：`title`: String · `modal`: Bool · `directory`: String · `selected_file`: String · `mode`: Enum (`open_file` | `open_files` | `save_file` | `select_directory`) · `label_text`: String · `default_suffix`: String · `file_count`: UInt (唯讀) · `can_go_back`: Bool (唯讀) · `can_go_forward`: Bool (唯讀) · `view_mode`: Enum (`list` | `details`) · `sidebar_urls`: StringList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `current_changed`: String · `directory_entered`: String · `file_selected`: String · `files_selected`: list of String · `focus_changed`: UInt · `overwrite_requested`: String · `quick_look_requested`: String · `rejected` · `selection_changed`: list of String · `sort_changed`: mixed (String parts) · `url_selected`: String

**命令.** `set_mode`, `set_directory`, `open`

**Rust API.** `create_file_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("file_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ColorDialog (`color_dialog`)

模態顏色選擇器，帶色輪或色軌、預設、十六進位輸入與可選螢幕取色。用於完整顏色決策；選擇器嵌在表單內時用內聯 `ColorPicker`，單色塊摺疊欄位用 `ColorWell`。

**屬性.** 除四個共享屬性外共 10 項：`current_color`: String (唯讀) · `hex_rgba`: String · `preset_index`: UInt · `show_hex_input`: Bool · `show_presets`: Bool · `show_hue_rail`: Bool · `custom_color_count`: UInt (唯讀) · `modal`: Bool · `options_alpha`: Bool · `show_screen_picker`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `color_selected`: Color · `current_color_changed`: Color · `focus_changed`: UInt · `rejected` · `screen_color_sampled`: Color · `screen_pick_canceled` · `screen_pick_started`

**命令.** `set_hex`, `apply_preset`

**Rust API.** `create_color_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("color_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `colour_dialog`

### FontDialog (`font_dialog`)

模態字型選擇器，支援字族、樣式與字號選擇、預覽與接受/拒絕。使用者必須選擇字型並看到字樣時使用；內聯字族欄位用 `FontComboBox`，旁邊的字樣用 `FontPreview`。

**屬性.** 除四個共享屬性外共 5 項：`modal`: Bool · `underline`: Bool · `strikethrough`: Bool · `no_buttons`: Bool · `show_preview`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `font_selected`: String · `rejected`

**命令.** `set_current_font`, `accept`, `reject`

**Rust API.** `create_font_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("font_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### InputDialog (`input_dialog`)

模態單值提示，支援文字、整數、浮點或條目模式，並帶對應值屬性與變化訊號。需要不搭表單就取一個值時使用；涉及多個值或驗證時用帶真實欄位的 `Dialog`。

**屬性.** 除四個共享屬性外共 9 項：`title`: String · `label_text`: String · `mode`: Enum (`text` | `integer` | `double` | `item`) · `text_value`: String · `int_value`: Int · `double_value`: Float · `int_step`: Int · `current_item`: UInt · `dropdown_open`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `double_value_changed`: Float · `focus_changed`: UInt · `int_value_changed`: Int · `rejected` · `text_value_changed`: String

**命令.** `set_mode`, `set_text_value`, `set_int_value`, `set_double_value`

**Rust API.** `create_input_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("input_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ProgressDialog (`progress_dialog`)

模態進度介面，帶標籤、進度條、範圍、可選取消按鈕與自動重設。用於使用者發起的可取消工作；程式擁有的啟動工作用 `SplashScreen`，工作內聯且不阻擋時用一般 `ProgressBar`。

**屬性.** 除四個共享屬性外共 11 項：`title`: String · `label_text`: String · `value`: Int · `minimum`: Int · `maximum`: Int · `auto_reset`: Bool · `show_cancel_button`: Bool · `show_bar`: Bool · `minimum_duration`: UInt · `animated`: Bool · `accent_fill`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `finished` · `label_changed`: String · `value_changed`: Int

**命令.** `set_value`, `set_range`, `set_title`, `set_label_text`

**Rust API.** `create_progress_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("progress_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### PopupWindow (`popup_window`)

無邊框、可選瞬態的浮動視窗，帶自己的外殼旗標、標題、內容與調整大小訊號。用於只需要浮層外殼、不是完整對話框的內容；錨定氣泡用 `Popover`，需要模態語意時用 `Dialog`。

**屬性.** 除四個共享屬性外共 6 項：`title`: String · `has_content`: Bool (唯讀) · `modal`: Bool · `transient_parent`: UInt · `floating_chrome`: Bool · `resizable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed` · `opened` · `resized`: UInt ×2

**命令.** `set_content_widget`

**Rust API.** `create_popup_window(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("popup_window", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `popup`

### BottomSheet (`bottom_sheet`)

從底邊滑上、可拖回的 panel，帶 peek 高度與展開狀態。用於觸控佈局的次要內容；sheet 必須阻擋頁面時用 `ModalBottomSheet`，小型錨定氣泡用 `Popover`。

**屬性.** 除四個共享屬性外共 2 項：`expanded`: Bool · `peek_height`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** `set_expanded`, `set_peek_height`

**Rust API.** `create_bottom_sheet(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("bottom_sheet", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `sheet`

### ModalBottomSheet (`modal_bottom_sheet`)

Material 風格模態底部面板，高度可設定、帶動畫呈現與拖曳關閉的 `dismissed` 訊號。用於手機上阻擋式的選項或詳情；非阻擋 peek 內容用 `BottomSheet`，短訊息決策用 `Dialog`。

**屬性.** 除四個共享屬性外共 2 項：`sheet_height`: UInt · `animated`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** `set_visible`, `show`, `dismiss`

**Rust API.** `create_modal_bottom_sheet(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("modal_bottom_sheet", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `modal_sheet`

### Tooltip (`tooltip`)

懸停時顯示上下文資訊的小彈層文字，支援換行與顯示狀態。用於已有可見標籤的控制項的提示；不要把必要說明只放在工具提示裡；延遲與位置需要多個控制項共享時用 `TooltipManager` 配套控制項。

**屬性.** 除四個共享屬性外共 3 項：`text`: String · `shown`: Bool · `wrap`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_text`, `show`, `hide`

**Rust API.** `create_tooltip(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tooltip", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `tooltip_widget`, `hover_tip`

### Popover (`popover`)

帶錨定箭頭的浮動氣泡卡片，支援瞬態/半瞬態/常駐行為與顯示狀態。用於控制項附近的小塊上下文內容；僅文字提示用 `Tooltip`，內容需要視窗外殼時用 `PopupWindow`。

**屬性.** 除四個共享屬性外共 6 項：`shown`: Bool · `text`: String (唯讀) · `behavior`: Enum (`transient` | `semi_transient` | `permanent`) · `arrow_direction`: Enum (`auto` | `up` | `down` | `left` | `right`) · `placeholder_text`: String · `translucent`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `dismissed` · `shown`

**命令.** `set_visible`, `set_text`

**Rust API.** `create_popover(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("popover", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `popover_widget`, `popup_card`

### FindReplaceDialog (`find_replace_dialog`)

尋找/取代對話框，帶查詢與取代欄位、大小寫/全詞/正規表示式開關、全部醒目提示與循環尋找，以及下一個/上一個/取代/全部取代訊號。需要自有模態的整文件搜尋用；編輯器內聯列用 `FindBar`。

**屬性.** 除四個共享屬性外共 7 項：`find_text`: String · `replace_text`: String · `match_case`: Bool · `whole_word`: Bool · `use_regex`: Bool · `highlight_all`: Bool · `wrap_around`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `close` · `find_next`: String · `find_previous`: String · `focus_changed`: UInt · `replace`: String · `replace_all`: String ×3

**命令.** `set_find_text`, `set_replace_text`, `set_match_case`, `set_wrap_around`

**Rust API.** `create_find_replace_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("find_replace_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `find_replace`

### WizardDialog (`wizard_dialog`)

多頁對話框，按順序引導使用者完成步驟，支援上一步/下一步/完成、逐步有效性與側欄頁面。順序重要的設定流程用；步驟不是模態流程時用 `StackedWidget` 加自建按鈕；單頁用 `Dialog`。

**屬性.** 除四個共享屬性外共 6 項：`title`: String · `subtitle`: String · `current_step`: UInt (唯讀) · `step_count`: UInt (唯讀) · `can_go_back`: Bool (唯讀) · `can_go_forward`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `custom_button_clicked`: UInt · `finished` · `help_requested` · `sidebar_page_activated`: UInt · `step_changed`: UInt

**命令.** `next`, `back`, `finish`

**Rust API.** `create_wizard_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("wizard_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `wizard`

### AlertDialog (`alert_dialog`)

模態提示，帶訊息、圖示與一個確認動作，以及可選提示/詳細文字與核取方塊。用於使用者必須確認的資訊；存在真正的接受/取消選擇時用 `ConfirmDialog`，經典固定按鈕形式用 `MessageBox`。

**屬性.** 除四個共享屬性外共 10 項：`title`: String · `message`: String · `button_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `focus_changed`: UInt · `rejected`

**命令.** 未發布命令。

**Rust API.** `create_alert_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("alert_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `notice_dialog`

### ConfirmDialog (`confirm_dialog`)

模態問題，帶接受動作、取消動作、可選提示/詳細文字與核取方塊。用於破壞性或不可逆操作之前；僅資訊用 `AlertDialog`，按鈕必須是經典集合時用 `MessageBox`。

**屬性.** 除四個共享屬性外共 11 項：`title`: String · `message`: String · `accept_text`: String · `reject_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `focus_changed`: UInt · `rejected`

**命令.** 未發布命令。

**Rust API.** `create_confirm_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("confirm_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `confirmation_dialog`

### AboutDialog (`about_dialog`)

標準應用資訊對話框：應用名稱、版本、訊息、提示與詳細文字，以及可選核取方塊。用於說明/關於；不要把它當通用對話框——這些欄位就是全部契約。

**屬性.** 除四個共享屬性外共 11 項：`title`: String · `application_name`: String · `version`: String · `message`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`

**命令.** 未發布命令。

**Rust API.** `create_about_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("about_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `about_box`

## 11. 複合應用介面

特殊族承載「整個應用介面」而非單一欄位的控制項：繪圖畫布、地圖、通用圖表、甘特與時間軸、看板、顏色與形狀選擇器、程式碼相關檢視器（差異、終端機、Markdown）、通知/Toast 面，以及與這些面配對的配套控制項。

當功能*本身*就是介面時用特殊控制項——自由繪圖用 canvas，瓦片地理用 `MapView`，兩級拖放用 `KanbanBoard`，並排或統一對比用 `DiffViewer`。它們各自持有模型並暴露很窄的訊號集；宿主餵資料、對訊號作出反應。不要把它們當通用容器，也不要用一堆小控制項拼出它們的互動：拖曳、命中測試與視埠規則正是這些控制項的價值。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `special` | `src/widget/special_widgets/` | 39 |

### Canvas (`canvas`)

自由繪圖表面，帶圖層、縮放、平移、網格吸附與攜帶位置的指標事件。用於宿主自行繪製的場景；場景是可編輯的可選形狀集合時用 `ShapeCanvas`，小型即時模式裝飾用 `MiniCanvas`。

**屬性.** 除四個共享屬性外共 8 項：`command_count`: UInt (唯讀) · `layer_count`: UInt (唯讀) · `anti_alias`: Bool · `show_grid`: Bool · `grid_step`: UInt · `snap_to_grid`: Bool · `zoom_range`: NumberList · `zoom_step`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_picked`: UInt · `content_changed`: UInt · `context_menu_requested`: String · `double_clicked`: mixed (String parts) · `layer_changed`: UInt · `mouse_moved`: String · `mouse_pressed`: mixed (String parts) · `mouse_released`: mixed (String parts) · `zoom_changed`: Float

**命令.** `set_zoom`, `set_center`, `clear`

**Rust API.** `create_canvas(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("canvas", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `canvas_widget`, `drawing_surface`

### Canvas (`map_view`)

瓦片地圖檢視，帶中心、縮放、投影與標記，並擁有 `TileLoader`、`ZoomControl`、`ScaleBar`、`Compass` 配套控制項。宿主抓取瓦片並呼叫 `deliver_tile`/`reject_tile`；檢視透過 `tile_requested` 請求，控制項本身不做 I/O。

**屬性.** 除四個共享屬性外共 6 項：`center_x`: Float · `center_y`: Float · `zoom`: Float · `marker_count`: UInt (唯讀) · `selected_marker_id`: String (唯讀) · `projection`: Enum (`linear` | `mercator`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `center_changed`: Float ×2 · `cluster_clicked`: Float ×2 · `marker_clicked`: String · `marker_selected`: String · `tile_requested`: mixed (String parts) · `zoom_changed`: Float

**命令.** `set_markers`, `set_center_x`, `set_center_y`, `set_zoom`

**Rust API.** `create_map_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("map_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Chart (`chart`)

通用圖表控制項：圖表類型（柱狀、折線、面積、圓餅、散點、瀑布、漏斗、蠟燭、箱線）、資料序列、標籤、縮放與懸停讀數。類型在執行時選擇時使用；固定類型用專用 `LineChart`/`BarChart`/`PieChart`，行情資料用金融族。

**屬性.** 除四個共享屬性外共 4 項：`chart_type`: Enum (`bar` | `line` | `area` | `pie` | `scatter` | `waterfall` | `funnel` | `candlestick` | `box_plot`) · `point_count`: UInt (唯讀) · `label_count`: UInt (唯讀) · `series_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `data_point_clicked`: UInt · `data_point_hovered`: UInt · `data_point_unhovered`: UInt · `viewport_changed`: UInt ×2 · `zoom_changed`: (Float, Float) or null

**命令.** 未發布命令。

**Rust API.** `create_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `chart_widget`, `chart_surface`

### Chart (`gantt_widget`)

按視埠繪製任務的甘特圖，支援里程碑、相依、關鍵路徑、進度調整與今日標記；它擁有 `TimelineAxis` 與 `DependencyEditor`。用於專案排程；非排程的事件車道檢視用 `TimelineWidget`。

**屬性.** 除四個共享屬性外共 7 項：`task_count`: UInt (唯讀) · `selected_id`: String (唯讀) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_critical_path`: Bool · `show_today`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2 · `task_rescheduled`: String · `task_resize_finished`: String · `task_resize_started`: String · `task_selected`: String · `viewport_changed`: Int ×2

**命令.** `set_tasks`, `zoom`, `set_viewport`, `select_task`

**Rust API.** `create_gantt_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("gantt_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `gantt`

### Chart (`timeline_widget`)

按時間軸放置條目的時間軸，支援列或車道緊湊佈局與可選條目。用於事件歷史與動態流；任務有工期、相依與進度時用 `GanttWidget`。

**屬性.** 除四個共享屬性外共 7 項：`item_count`: UInt (唯讀) · `selected_index`: UInt (唯讀) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_time_labels`: Bool · `lane_mode`: Enum (`row` | `lane_packed`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_hovered`: String or null · `item_selected`: String · `viewport_changed`: Int ×2

**命令.** `set_items`, `set_viewport`, `zoom`, `select_index`

**Rust API.** `create_timeline_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("timeline_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `timeline`, `timeline_view`

### ColorPicker (`color_picker`)

內聯 HSVA 顏色選擇器，帶十六進位輸入、alpha、預設與色相值；它嵌在表單裡而不是放在視窗中。顏色欄位始終可見時使用；模態選擇器用 `ColorDialog`，摺疊單色塊欄位用 `ColorWell`。

**屬性.** 除四個共享屬性外共 5 項：`hex_rgba`: String · `show_alpha`: Bool · `preset_count`: UInt (唯讀) · `presets`: StringList · `hue_degrees`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `color_changed`: Color · `color_committed`: Color · `color_previewed`: Color · `hex_changed`: String

**命令.** `set_hex`, `apply_preset`

**Rust API.** `create_color_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("color_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Grid (`grid`)

把子控制項排成列欄儲存格的矩形容器，支援間距、儲存格尺寸、表頭與分頁。用於固定的控制項或數值矩陣；儲存格來自模型時用 `Table`，純佈局放置用 `Flex`/`Grid` 佈局名稱。

**屬性.** 除四個共享屬性外共 11 項：`rows`: UInt · `columns`: UInt · `spacing`: UInt · `line_color`: String · `cell_width`: UInt (唯讀) · `cell_height`: UInt (唯讀) · `selected_cell`: UInt · `selected_count`: UInt (唯讀) · `page`: UInt · `page_count`: UInt (唯讀) · `headers`: StringList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `context_menu_requested`: UInt ×2 · `page_changed`: UInt

**命令.** `set_rows`, `set_columns`, `set_spacing`, `set_line_color`

**Rust API.** `create_grid(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("grid", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `grid_widget`, `gridwidget`

### FreeformShape (`freeform_shape`)

基於路徑、非矩形的可點擊形狀，帶填滿、描邊、頂點、歷史與移動/頂點訊號。用於圖節點或自訂命中區域；多個形狀組成可編輯場景時用 `ShapeCanvas`，非互動繪圖用 `MiniCanvas`。

**屬性.** 除四個共享屬性外共 5 項：`path_kind`: Enum (`bubble` | `rounded_rect` | `polygon` | `star` | `heart` | `custom`) (唯讀) · `fill_rgba`: String · `stroke_rgba`: String · `stroke_width`: UInt · `movable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `geometry_changed`: Rect · `history_changed`: UInt ×2 · `hover_changed`: Bool · `moved`: String · `pressed_changed`: Bool · `vertex_changed`: mixed (String parts)

**命令.** `set_fill_rgba`, `set_stroke_rgba`, `set_stroke_width`

**Rust API.** `create_freeform_shape(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("freeform_shape", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### RadarChart (`radar_chart`)

雷達（蜘蛛）圖，帶共享維度軸、以閉合多邊形繪製的多個序列、網格、軸標籤與圖例。用於維度可比的輪廓（技能、KPI）；單一有序軸用 `LineChart`，矩陣資料用 `Heatmap`。

**屬性.** 除四個共享屬性外共 7 項：`axis_count`: UInt (唯讀) · `series_count`: UInt (唯讀) · `show_grid`: Bool · `show_axis_labels`: Bool · `show_legend`: Bool · `hovered_axis`: UInt (唯讀) · `series_names`: StringList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `axis_hovered`: UInt · `series_clicked`: UInt · `vertex_clicked`: UInt ×2 · `vertex_hovered`: UInt ×2

**命令.** `set_axes`, `set_series`, `add_series`

**Rust API.** `create_radar_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("radar_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `radar`, `spider_chart`

### Heatmap (`heatmap`)

以彩色儲存格渲染的值矩陣，帶色階範圍、標籤、圖例與視埠視窗。用於兩個軸都是分類的密度或相關性矩陣；一般儲存格用 `Grid`，按索引的單值序列用 `Chart`。

**屬性.** 除四個共享屬性外共 10 項：`row_count`: UInt (唯讀) · `column_count`: UInt (唯讀) · `scale_minimum`: Float · `scale_maximum`: Float · `resolved_minimum`: Float (唯讀) · `resolved_maximum`: Float (唯讀) · `show_labels`: Bool · `show_legend`: Bool · `viewport_rows`: UInt · `viewport_columns`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_clicked`: UInt ×2 · `cell_double_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `region_selected`: UInt ×4

**命令.** `set_data`, `set_cell`, `clear`

**Rust API.** `create_heatmap(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("heatmap", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `heatmap_chart`

### KanbanBoard (`kanban_board`)

由欄組成、承載可拖曳卡片的看板，支援泳道、欄寬、水平偏移與卡片移動/啟用訊號。配 `KanbanColumnHeader` 提供重新命名/計數/WIP 外殼；卡片需要真實可編輯控制項時配 `KanbanCardView`；看板持有模型與放置規則。

**屬性.** 除四個共享屬性外共 9 項：`column_count`: UInt (唯讀) · `card_count`: UInt (唯讀) · `dragging_card_id`: String (唯讀) · `hovered_column`: UInt (唯讀) · `column_width`: UInt · `horizontal_scroll_offset`: UInt · `lane_mode`: Enum (`none` | `lanes`) · `swimlanes`: Bool · `lane_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `card_activated`: String · `card_moved`: String ×2

**命令.** `add_column`, `add_card`, `move_card`, `cancel_drag`

**Rust API.** `create_kanban_board(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("kanban_board", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `kanban`, `board`

### SignaturePad (`signature_pad`)

觸控友好的手寫簽名表面：帶寬度、顏色、平滑閾值與壓力的筆畫，支援復原與筆畫完成訊號。用於簽名或手寫輸入；不要把它當通用繪圖軟體——它保存折線而不是點陣圖。

**屬性.** 除四個共享屬性外共 9 項：`stroke_count`: UInt (唯讀) · `stroke_width`: UInt · `stroke_color`: Color · `min_point_distance`: Float · `min_point_interval_ms`: UInt · `pressure_affects_width`: Bool · `last_pressure`: Float (唯讀) · `last_tilt_x`: Float (唯讀) · `last_tilt_y`: Float (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `stroke_completed`: String

**命令.** `undo`

**Rust API.** `create_signature_pad(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("signature_pad", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `signature`

### Breadcrumb (`breadcrumb`)

由區段組成的橫向導覽軌跡，支援摺疊、溢出、內聯路徑編輯與逐區段啟用。用於使用者可向上跳轉的階層；編號分頁用 `Pagination`，頂層命令用 `MenuBar`。

**屬性.** 除四個共享屬性外共 4 項：`segment_count`: UInt (唯讀) · `selected_index`: UInt (唯讀) · `collapsed_count`: UInt (唯讀) · `separator`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `overflow_clicked`: UInt · `path_edited`: String · `segment_activated`: String

**命令.** `set_segments`, `push_segment`, `clear_segments`

**Rust API.** `create_breadcrumb(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("breadcrumb", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `nav_breadcrumb`

### Chip (`chip`)

多條目膠囊列：chip 可選擇、移除、重排與溢出，並帶焦點索引。用於作為一組編輯的 token/標籤集合；單一唯讀標籤用 `Tag`，使用者鍵入新值時用 `TagInput`。

**屬性.** 除四個共享屬性外共 4 項：`item_count`: UInt (唯讀) · `multi_select`: Bool · `focused_index`: UInt (唯讀) · `selected_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `chip_removed`: String · `chip_reordered`: String · `chip_toggled`: String · `overflow_activated`: UInt

**命令.** `set_items`, `toggle_index`, `move_focus`

**Rust API.** `create_chip(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("chip", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `chips`

### Toast (`toast`)

單條瞬時通知訊息，帶級別、存活時間與關閉訊號。應透過 toast 面或 `ToastHost` 佇列建立，而不是管理大量獨立 toast；通知需要持久時用 `Banner`，屬於視窗邊緣時用 `Snackbar`。

**屬性.** 除四個共享屬性外共 4 項：`message`: String · `level`: Enum (`info` | `success` | `warning` | `error`) · `ttl_ms`: UInt · `dismissible`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`: String

**命令.** `dismiss`, `set_message`, `set_level`

**Rust API.** `create_toast(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("toast", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `notification`, `toast_message`

### PopupWindow (`toast_stack`)

排隊並佈局 toast 的容器，帶可見數、最大可見數、錨點與堆疊方向。作為 toast 放置的唯一所有者；佇列需要在進入堆疊前過期與限流時配 `ToastHost`。

**屬性.** 除四個共享屬性外共 6 項：`toast_count`: UInt (唯讀) · `selected_id`: String (唯讀) · `row_height`: UInt · `max_visible`: UInt · `anchor`: Enum (`top_left` | `top_right` | `bottom_left` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_bottom` | `newest_at_top`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toast_action`: String · `toast_activated`: String · `toast_dismissed`: String · `toast_expired`: String

**命令.** `push`, `clear`, `select_index`, `activate_selected`, `dismiss_selected`

**Rust API.** `create_toast_stack(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("toast_stack", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `toasts`

### ListView (`command_palette`)

可搜尋的命令清單浮層：查詢、過濾後數量、醒目提示索引與 `command_activated`。用於鍵盤優先的啟動器；滑鼠驅動的命令清單用 `Menu`，編輯綁定用 `ShortcutEditor`。

**屬性.** 除四個共享屬性外共 6 項：`query`: String · `entry_count`: UInt (唯讀) · `filtered_count`: UInt (唯讀) · `highlighted_index`: UInt (唯讀) · `row_height`: UInt · `group_by_category`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_activated`: String · `query_changed`: String

**命令.** `set_entries`, `set_query`, `clear_query`, `move_highlight`, `activate_highlighted`

**Rust API.** `create_command_palette(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("command_palette", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `command_box`

### ListView (`notification_center`)

通知清單，帶未讀數、級別過濾、最大條目與啟用/關閉/全部清除訊號。用於事件的持久收件匣；瞬時打斷用 `Toast`/`ToastStack`，單條持久通知用 `Banner`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `unread_count`: UInt (唯讀) · `selected_index`: UInt (唯讀) · `row_height`: UInt · `level_filter`: Enum (`all` | `info` | `warning` | `error`) · `max_items`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clear_all_requested` · `notification_activated`: String · `notification_dismissed`: String · `notification_selected`: String · `unread_count_changed`: UInt

**命令.** `push`, `clear`, `set_read`, `mark_all_read`, `select_index`, `activate_selected`

**Rust API.** `create_notification_center(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("notification_center", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `notifications`

### Table (`diff_viewer`)

對兩段文字做並排或統一差異檢視，帶變更數、摺疊閾值、空白模式、詞級粒度與差異塊展開。用於展示比較；配 `DiffFileList` 在多檔案間選擇；不要把它當編輯器——它是唯讀的。

**屬性.** 除四個共享屬性外共 12 項：`left_text`: String · `right_text`: String · `line_count`: UInt (唯讀) · `change_count`: UInt (唯讀) · `selected_index`: UInt (唯讀) · `view_mode`: Enum (`side_by_side` | `unified`) · `fold_unchanged`: Bool · `fold_threshold`: UInt · `fold_context_lines`: UInt · `unified_changed_pair`: Bool · `whitespace_mode`: Enum (`show` | `ignore_eol` | `ignore_all` | `mark_blank_changes`) · `word_granularity`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `compared`: UInt · `fold_changed`: Bool · `hunk_expanded`: UInt · `selection_changed`: UInt or null · `view_mode_changed`: String

**命令.** `set_texts`, `select_index`

**Rust API.** `create_diff_viewer(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("diff_viewer", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `diff`

### SegmentedControl (`segmented_control`)

單選分段列，指示器在區段間滑動，每段有 id。用於兩到五個互斥檢視；多選或圖示分段用 `SegmentedButton`，區段是頁面標籤時用 `TabBar`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `selected_index`: UInt (唯讀) · `selected_id`: String (唯讀) · `size_to_fit`: Bool · `min_segment_width`: UInt · `multi_select`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: String

**命令.** `set_items`, `move_selection`

**Rust API.** `create_segmented_control(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("segmented_control", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### StatusBar (`snackbar`)

錨定在視窗邊緣的瞬時訊息，帶動作標籤、佇列長度與最大佇列。用於不阻擋地確認已完成動作；訊息需要持久時用 `Banner`，多條訊息需要堆疊時用 `ToastStack`。

**屬性.** 除四個共享屬性外共 5 項：`message`: String · `action_label`: String (唯讀) · `queued_count`: UInt (唯讀) · `max_queued`: UInt · `anchor`: Enum (`top` | `center` | `bottom`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_triggered`: String · `dismissed`

**命令.** `show`, `show_with_action`, `dismiss`

**Rust API.** `create_snackbar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("snackbar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `toast_bar`

### WebEngineView (`media_player`)

媒體表面，帶來源、播放/暫停、位置、時長、音量與全螢幕，並回報播放與位置變化。宿主提供媒體管線時使用；配 `MediaTransportBar`、`MediaScrubber`、`SubtitleBar` 配套控制項，而不是重寫這些控制項。

**屬性.** 除四個共享屬性外共 9 項：`source`: String · `playing`: Bool · `duration_ms`: UInt (唯讀) · `position_ms`: UInt · `volume`: UInt · `muted`: Bool · `fullscreen`: Bool · `playlist_index`: UInt · `track_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `chapter_selected`: UInt · `fullscreen_requested`: Bool · `media_ended` · `playback_changed`: Bool · `position_changed`: UInt · `source_changed`: String · `volume_changed`: UInt

**命令.** `set_source`, `clear_source`, `play`, `pause`, `seek_to`, `set_volume`

**Rust API.** `create_media_player(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("media_player", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ShapeCanvas (`shape_canvas`)

承載、選擇與排序形狀的場景，帶新增/修改/移除/選擇訊號、吸附與網格步長。用於圖編輯器；單個形狀用 `FreeformShape`，非形狀物件的任意繪圖用 `Canvas`。

**屬性.** 除四個共享屬性外共 6 項：`shapes`: NumberList (唯讀) · `shape_count`: UInt (唯讀) · `selection`: NumberList · `editable`: Bool · `snap_to_grid`: Bool · `grid_step`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `shape_added`: UInt · `shape_modified`: UInt · `shape_removed`: UInt · `shape_selected`: list of UInt

**命令.** 未發布命令。

**Rust API.** `create_shape_canvas(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("shape_canvas", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `shape_board`

### KanbanCardView (`kanban_card_view`)

看板欄中的真實卡片控制項：標題、描述、標籤、負責人、完成狀態與就地編輯。卡片必須是真正可編輯控制項時使用；`KanbanBoard` 持有放置與拖曳狀態，本控制項持有一張卡片的內容與編輯生命週期。

**屬性.** 除四個共享屬性外共 7 項：`title`: String · `description`: String · `done`: Bool · `tags`: StringList · `assignee`: String · `draggable`: Bool · `editing`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `card_clicked` · `card_done_toggled`: Bool · `card_edited`: String

**命令.** 未發布命令。

**Rust API.** `create_kanban_card_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("kanban_card_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `kanban_card_control`

### KanbanColumnHeader (`kanban_column_header`)

看板欄的真實表頭：標題、卡片數、WIP 上限、摺疊與就地重新命名。放在 `KanbanBoard` 欄上方；表頭持有自己的重新命名與摺疊狀態，並回報 `column_renamed`、`column_collapsed_changed` 與 `add_card_requested`。

**屬性.** 除四個共享屬性外共 7 項：`title`: String · `count`: UInt · `wip_limit`: UInt · `collapsed`: Bool · `editable`: Bool · `at_wip_limit`: Bool (唯讀) · `editing`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `add_card_requested` · `column_collapsed_changed`: Bool · `column_renamed`: String

**命令.** 未發布命令。

**Rust API.** `create_kanban_column_header(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("kanban_column_header", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `kanban_column_bar`

### DiffFileList (`diff_file_list`)

差異檢視器旁的多檔案選擇器：路徑清單，帶選擇模式與啟用/選擇訊號。比較由檢視器持有；本控制項只選擇檢視器應顯示哪個路徑。

**屬性.** 除四個共享屬性外共 4 項：`files`: StringList · `file_count`: UInt (唯讀) · `selected_path`: String (唯讀) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `file_activated`: String · `file_selected`: String

**命令.** 未發布命令。

**Rust API.** `create_diff_file_list(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("diff_file_list", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `diff_files`

### Swatch (`swatch`)

顏色控制項共享的可選色格網格，帶選中索引與啟用/選擇訊號。目前顏色由對話框或選擇器持有；色格持有調色盤與選擇狀態，因此把 `swatch_selected` 接到擁有者的顏色屬性。

**屬性.** 除四個共享屬性外共 4 項：`colors`: Color · `selected_index`: UInt · `cell_size`: UInt · `columns`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `swatch_activated`: Color · `swatch_selected`: UInt

**命令.** 未發布命令。

**Rust API.** `create_swatch(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("swatch", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `color_swatch`, `colour_swatch`

### FilterRow (`filter_row`)

每欄一個編輯器、發布該列過濾條件與即時旗標的控制項。用於表格旁的緊湊多欄過濾；模型由表格或網格持有，本列只產生條件。

**屬性.** 除四個共享屬性外共 4 項：`columns`: StringList · `conditions`: StringList · `operator`: Enum (`contains` | `equals` | `starts_with` | `ends_with` | `not_contains` | `equals_number` | `greater_than` | `greater_or_equal` | `less_than` | `less_or_equal`) · `live`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `filter_changed`: String · `filter_cleared`

**命令.** 未發布命令。

**Rust API.** `create_filter_row(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("filter_row", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `filter_bar`

### Legend (`legend`)

圖表與畫布宿主的可勾選序列鍵：條目、方向、位置與點擊/切換訊號。圖表持有序列資料；圖例持有哪些序列可見，因此把 `series_toggled` 轉給圖表的可見性狀態。

**屬性.** 除四個共享屬性外共 4 項：`entries`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `checkable`: Bool · `position`: Enum (`top` | `right` | `bottom` | `left` | `outside`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_clicked`: UInt · `series_toggled`: mixed (String parts)

**命令.** 未發布命令。

**Rust API.** `create_legend(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("legend", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `chart_legend`

### ChartTooltip (`chart_tooltip`)

跟隨指標、承載圖表序列值的浮泡，帶錨點、偏移、跟隨旗標與內容列。圖表透過 `show_at` 定位它；工具提示只持有文字與可見性。

**屬性.** 除四個共享屬性外共 4 項：`anchor`: NumberList · `offset`: NumberList · `follow_pointer`: Bool · `content`: StringList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `hidden` · `shown`

**命令.** 未發布命令。

**Rust API.** `create_chart_tooltip(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("chart_tooltip", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `tooltip_bubble`

### TooltipManager (`tooltip_manager`)

把懸停轉換為延遲工具提示的宿主管線：延遲、持續時間、跟隨指標與最大寬度，並帶顯示/隱藏索引。多個控制項共享提示時序時，為一個子樹使用一個管理器；單個控制項仍可用自己的 `tooltip` 屬性。

**屬性.** 除四個共享屬性外共 4 項：`delay_ms`: UInt · `duration_ms`: UInt · `follow_pointer`: Bool · `max_width`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tooltip_hidden`: UInt · `tooltip_shown`: UInt

**命令.** 未發布命令。

**Rust API.** `create_tooltip_manager(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tooltip_manager", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `tooltip_pipeline`

### ZoomPanController (`zoom_pan_controller`)

影像檢視的縮放、平移與感興趣區域狀態，帶縮放模式、上下限與變化訊號。多個控制項必須共享同一檢視變換時使用；影像檢視負責繪製，控制器持有變換。

**屬性.** 除四個共享屬性外共 6 項：`zoom_level`: Float · `pan_offset`: NumberList · `zoom_mode`: Enum (`free` | `fit_width` | `fit_height` | `original`) · `min_zoom`: Float · `max_zoom`: Float · `roi`: Rect — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `pan_changed`: Int ×2 · `roi_changed`: Rect or null · `zoom_changed`: Float

**命令.** 未發布命令。

**Rust API.** `create_zoom_pan_controller(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("zoom_pan_controller", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `zoom_pan`

### ToastHost (`toast_host`)

toast 堆疊前的有界、會過期佇列：最大可見、最大排隊、預設時長、錨點、堆疊方向與溢出策略。toast 數量必須限流時使用；堆疊負責繪製，宿主決定哪些 toast 能存活到進入堆疊。

**屬性.** 除四個共享屬性外共 6 項：`max_visible`: UInt · `max_queued`: UInt · `default_duration_ms`: UInt · `anchor`: Enum (`top_left` | `top_center` | `top_right` | `bottom_left` | `bottom_center` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_top` | `newest_at_bottom`) · `overflow_policy`: Enum (`drop_oldest` | `drop_newest` | `queue`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `queue_overflow`: UInt · `toast_dismissed`: UInt · `toast_expired`: UInt

**命令.** 未發布命令。

**Rust API.** `create_toast_host(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("toast_host", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `toast_queue`

### TileLoader (`tile_loader`)

地圖檢視的瓦片尋址、快取與載入結果：URL 模板、縮放範圍、瓦片尺寸、快取上限與載入成功/失敗訊號。地圖檢視持有一個並索取 URL；宿主執行請求並呼叫 `deliver_tile`。

**屬性.** 除四個共享屬性外共 6 項：`url_template`: String · `min_zoom`: UInt · `max_zoom`: UInt · `tile_size`: UInt · `cache_limit`: UInt · `cache_size`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tile_failed`: Int ×3 · `tile_loaded`: Int ×3

**命令.** `clear_cache`

**Rust API.** `create_tile_loader(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tile_loader", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `map_tile_loader`

### ZoomControl (`zoom_control`)

地圖旁的三按鈕縮放列：縮放級別、上下限與 `zoom_requested` 訊號。地圖持有該列並套用請求；如果地圖也回報縮放變化，不要讓該列直接修改地圖縮放。

**屬性.** 除四個共享屬性外共 3 項：`zoom_level`: Float · `min`: Float · `max`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `zoom_requested`: Float

**命令.** 未發布命令。

**Rust API.** `create_zoom_control(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("zoom_control", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `map_zoom_control`

### ScaleBar (`scale_bar`)

地圖的公制或英制距離圖例，由每像素公尺數計算。地圖持有並更新換算；該列只負責繪製圖例。

**屬性.** 除四個共享屬性外共 3 項：`meters_per_pixel`: Float · `units`: Enum (`metric` | `imperial`) · `style`: Enum (`plain` | `boxed`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** 未發布命令。

**Rust API.** `create_scale_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("scale_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `map_scale_bar`

### Compass (`compass`)

可點擊的方位盤，點擊時請求正北朝上，並回報自身方位。用於可旋轉地圖之上；地圖持有方位並回應 `reset_north_requested`。

**屬性.** 除四個共享屬性外共 2 項：`bearing_deg`: Float · `clickable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `reset_north_requested`

**命令.** 未發布命令。

**Rust API.** `create_compass(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("compass", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `map_compass`

### TimelineAxis (`timeline_axis`)

日期/數值軸，支援像素換算、刻度密度、縮放與刻度變化回報。甘特圖持有一個並使用 `pixel_to_date`/`date_to_pixel`；宿主也可以在圖表旁再放一個以共享同一對應。

**屬性.** 除四個共享屬性外共 6 項：`tick_density`: UInt · `date_format`: Enum (`numeric` | `iso`) · `zoom_level`: Float · `range`: NumberList (唯讀) · `range_start`: Int · `range_end`: Int — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `scale_changed`: Int ×2

**命令.** 未發布命令。

**Rust API.** `create_timeline_axis(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("timeline_axis", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `gantt_timeline_axis`

### DependencyEditor (`dependency_editor`)

甘特圖的可編輯前置/後繼邊，帶新增/移除/選擇訊號、命中測試與連線幾何。甘特圖持有一個並可從任務同步相依；編輯器持有邊清單與驗證。

**屬性.** 除四個共享屬性外共 2 項：`dependencies`: UInt (唯讀) · `editable`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2

**命令.** 未發布命令。

**Rust API.** `create_dependency_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dependency_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `gantt_dependency_editor`

## 12. 程式碼與文件編輯器

本族彙集內容為結構化文字的文字編輯器：自帶緩衝區的程式碼編輯器、帶預覽與標題的 Markdown 編輯器，以及用於導覽的尋找列與大綱面板。

帶行號、標記、摺疊與游標事件的原始碼文字用 `CodeEditor`；帶預覽分欄的散文用 `MarkdownEditor`；宿主想自建尋找介面時用 `FindBar`；標題導覽用 `MarkdownOutlinePanel`。不要用它們編輯單行值（`LineEdit`）或富格式文字（`RichEdit`），需要語法標記或行槽時也不要退回 `TextArea`。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 |

### RichEdit (`code_editor`)

自帶緩衝區的原始碼編輯器：列數、游標行列、標記、摺疊、跳轉列與斷點槽，並回報游標、摺疊、標記與搜尋事件。用於原始碼文字；散文用 `MarkdownEditor`，格式文字用 `RichEdit`，附加輸出用 `TerminalView`。

**屬性.** 除四個共享屬性外共 9 項：`text`: String · `line_count`: UInt (唯讀) · `cursor_line`: UInt (唯讀) · `cursor_column`: UInt (唯讀) · `marker_count`: UInt (唯讀) · `undo_stack_limit`: UInt · `undo_depth`: UInt (唯讀) · `goto_bar_visible`: Bool · `breakpoint_gutter`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `breakpoint_toggled`: mixed (String parts) · `completion_changed`: Bool · `cursor_moved`: UInt ×2 · `fold_changed`: UInt · `marker_activated`: UInt · `palette_changed` · `search_changed`: UInt · `search_result`: UInt ×4 · `selection_changed`: Rect or null · `tab_changed`: UInt · `text_changed`: String · `zoom_changed`: Float

**命令.** `set_text`, `append_line`, `set_markers`, `set_cursor`

**Rust API.** `create_code_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("code_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### RichEdit (`markdown_editor`)

帶原始碼與預覽模式、標題計數、游標追蹤與可交給 `MarkdownOutlinePanel` 顯示的大綱的 Markdown 編輯器。用於 Markdown 文件；配 `FindBar` 做內聯搜尋；非 Markdown 格式用 `RichEdit`。

**屬性.** 除四個共享屬性外共 10 項：`text`: String · `preview_mode`: Bool · `line_count`: UInt (唯讀) · `word_count`: UInt (唯讀) · `heading_count`: UInt (唯讀) · `cursor_line`: UInt (唯讀) · `show_scrollbar`: Bool · `gfm_extensions`: Bool · `code_highlight`: Bool · `undo_stack_limit`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `heading_activated`: UInt · `preview_mode_changed`: Bool · `selection_changed`: Bool · `text_changed`: String

**命令.** `set_text`, `append_line`, `toggle_preview_mode`, `set_preview_mode`, `undo`

**Rust API.** `create_markdown_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("markdown_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `md_editor`

### FindBar (`find_bar`)

編輯器內聯搜尋列：查詢、大小寫、全詞、取代文字、符合數與目前範圍，並回報搜尋與取代請求。用於編輯器表面內；搜尋必須模態時用 `FindReplaceDialog`。

**屬性.** 除四個共享屬性外共 7 項：`query`: String · `case_sensitive`: Bool · `whole_word`: Bool · `replace_text`: String · `replace_visible`: Bool · `match_count`: UInt (唯讀) · `current_range`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `replace_requested`: mixed (String parts) · `search_changed`: UInt ×2

**命令.** 未發布命令。

**Rust API.** `create_find_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("find_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `find_replace_bar`

### MarkdownOutlinePanel (`markdown_outline_panel`)

導覽 Markdown 編輯器的標題大綱：條目、作用中索引與寬度，發出 `entry_selected`。編輯器持有文件與 `outline()`；面板只列出標題，並透過 `goto_outline` 把所選索引回報回去。

**屬性.** 除四個共享屬性外共 4 項：`entries`: StringList · `active_index`: UInt · `width`: UInt · `entry_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_selected`: UInt

**命令.** 未發布命令。

**Rust API.** `create_markdown_outline_panel(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("markdown_outline_panel", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `markdown_outline`

### TextEdit (`terminal_view`)

附加式終端表面：回捲輸出、輸入列、提示符、游標樣式、ANSI 調色盤與 `command_submitted` 訊號。用於展示程序或命令主控台；可編輯文字用 `TextArea`，原始碼用 `CodeEditor`。

**屬性.** 除四個共享屬性外共 10 項：`output_line_count`: UInt (唯讀) · `input_line`: String · `scrollback_lines`: UInt · `wrap_output`: Bool · `show_scrollbar`: Bool · `caret_style`: Enum (`line` | `block` | `underline`) · `caret_blink_enabled`: Bool · `prompt`: String · `selection_mode`: Enum (`lines` | `characters`) · `ansi_palette`: NumberList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_submitted`: String · `completion_requested`: String · `interrupt_requested`: String · `output_received`: String · `resized`: UInt ×2 · `selection_changed`: Bool

**命令.** `append_output`, `submit`

**Rust API.** `create_terminal_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("terminal_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `terminal`

## 13. 行情資料控制項

金融族是一套縱向整合的行情檢視：帶疊加指標的蠟燭圖、對齊的成交量柱、深度曲線、訂單簿階梯、報價板與指標面板。它們共享序列/佈局詞彙，使各面板能對齊到同一價格-時間軸。

只有在處理市場或交易資料時才用金融控制項：OHLC 柱用 `CandlestickChart`，其下對齊 `VolumeChart`，累計流動性用 `DepthChart`，即時階梯用 `OrderBook`，自選用 `QuoteBoard`，MACD/RSI/KDJ 類面板用 `IndicatorChart`。不要把它們用於通用業務圖表（圖表族）；不需要即時更新時也不要餵歷史快照——靜態表格更省。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `finance` | `src/widget/chart_widgets/finance/` | 6 |

### CandlestickChart (`candlestick_chart`)

OHLC 蠟燭圖，帶疊加序列、價格軸、逐柱懸停/點擊與檢視變化。用作行情檢視的主價格面板；其下對齊 `VolumeChart`，技術面板用 `IndicatorChart`。不要用於非 OHLC 業務資料。

**屬性.** 除四個共享屬性外共 4 項：`series`: String · `overlay_count`: UInt · `show_price_levels`: Bool · `show_overlay_legend`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`, `add_overlay`

**Rust API.** `create_candlestick_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("candlestick_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `candlestick`, `kline`, `k_line`, `k_line_chart`, `ohlc_chart`

### VolumeChart (`volume_chart`)

與蠟燭圖柱對齊的成交量柱狀圖，支援按方向或統一著色與頂部留白。價格圖持有時間軸；本面板讀取同一序列並渲染成交量，因此宿主需要同步兩個視埠。

**屬性.** 除四個共享屬性外共 3 項：`series`: String · `color_mode`: Enum (`direction` | `uniform`) · `headroom`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`

**Rust API.** `create_volume_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("volume_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `volume`, `volume_histogram`, `volume_bars`

### DepthChart (`depth_chart`)

累計買賣深度曲線，深度可設定、買賣顏色分離，並回報指標下的價格檔位。用於流動性；帶掛單量的離散階梯用 `OrderBook`。

**屬性.** 除四個共享屬性外共 3 項：`depth`: UInt · `bid_color`: Color · `ask_color`: Color — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_clicked`: Float · `level_hovered`: Float

**命令.** `set_series`

**Rust API.** `create_depth_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("depth_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `market_depth`, `depth_graph`, `liquidity_chart`

### OrderBook (`order_book`)

即時買賣階梯，每檔帶掛單量、價差列與逐檔懸停/點擊。用於蠟燭圖旁的市場微觀結構；平滑累計曲線用 `DepthChart`，多標的自選用 `QuoteBoard`。

**屬性.** 除四個共享屬性外共 3 項：`depth`: UInt · `decimals`: UInt · `show_spread`: Bool (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_clicked`: mixed (String parts) · `level_hovered`: mixed (String parts) · `level_unhovered`: mixed (String parts)

**命令.** `set_series`

**Rust API.** `create_order_book(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("order_book", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `book_ladder`, `market_depth_ladder`

### QuoteBoard (`quote_board`)

標的表格，帶最新價、漲跌、成交量與排序模式，並回報報價點擊/懸停/選擇。用作自選清單；單一標的的階梯用 `OrderBook`，非行情資料用 `Table`。

**屬性.** 除四個共享屬性外共 4 項：`sort`: Enum (`none` | `symbol` | `last_descending` | `change_magnitude`) · `selected_index`: UInt · `row_height`: UInt (唯讀) · `scroll_offset`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `quote_clicked`: String · `quote_hovered`: String · `selection_changed`: String or null

**命令.** `set_series`

**Rust API.** `create_quote_board(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("quote_board", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `quotes`, `watchlist`, `quote_table`, `market_watch`

### IndicatorChart (`indicator_chart`)

技術指標面板：MACD、RSI、隨機指標、資金流量、ATR 或能量潮，帶週期與參考線。放在價格圖下方；它消費同一序列而不擁有行情資料，因此宿主需要同時餵兩個面板。

**屬性.** 除四個共享屬性外共 4 項：`series`: String · `mode`: Enum (`macd` | `rsi` | `stochastic` | `money_flow_index` | `atr` | `on_balance_volume`) · `period`: UInt · `show_reference_levels`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`

**Rust API.** `create_indicator_chart(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("indicator_chart", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `indicator`, `technical_indicator`, `oscillator`, `macd_chart`

## 14. 日期、時間與進階選擇

進階族涵蓋較複雜的選擇器：日曆與日期/時間編輯器、旋鈕、標籤與功能區列、圓餅選單，以及鍵盤快速鍵編輯器。

以月曆網格為主介面時用 `Calendar`，單一欄位加彈層用 `DatePicker`/`TimePicker`/`DateTimePicker`，透過角度操縱有界數值用 `Dial`，與容器解耦的標籤列用 `TabBar`，Office 風格命令標籤用 `RibbonBar`，徑向次要動作選單用 `PieMenu`，錄製快速鍵用 `KeySequenceEdit`。一般 `LineEdit` 加驗證就夠的地方不要用日期編輯器；長清單也不要用 `PieMenu`——徑向選單只適合少量條目。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `advanced` | `src/widget/advanced_widgets/` | 10 |

### Calendar (`calendar`)

用於瀏覽與選擇日期的月曆網格，支援單選、多選或範圍選擇、週數、可設定首日與日期上下限。用作主日期表面；帶彈層的欄位用 `DatePicker`，帶預設的起止對用 `DateRangePicker`。

**屬性.** 除四個共享屬性外共 12 項：`selected_date`: String · `minimum_date`: String · `maximum_date`: String · `first_day_of_week`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) · `grid_visible`: Bool · `navigation_bar_visible`: Bool · `horizontal_header_visible`: Bool · `vertical_header_visible`: Bool · `date_format`: String · `show_week_numbers`: Bool · `selection_mode`: Enum (`single` | `multi` | `range`) · `selected_dates`: StringList — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: String · `current_page_changed`: mixed (String parts) · `selection_changed`: String

**命令.** `set_selected_date`, `set_date_range`, `set_first_day_of_week`, `show_today`

**Rust API.** `create_calendar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("calendar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `date_calendar`

### DatePicker (`date_edit`)

單行日期欄位，開啟日曆彈層，帶顯示格式、日期上下限與彈層/日期變化訊號。用於表單中的一個日期；網格始終可見時用 `Calendar`，值包含時間時用 `DateTimePicker`。

**屬性.** 除四個共享屬性外共 7 項：`date`: String · `minimum_date`: String · `maximum_date`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `popup_visibility_changed`: Bool

**命令.** `set_date`, `set_minimum_date`, `set_maximum_date`, `set_display_format`

**Rust API.** `create_date_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("date_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `date_picker`

### TimePicker (`time_edit`)

單行時間欄位，帶時鐘彈層、12/24 小時制、時間上下限與 `time_changed` 訊號。用於一天中的時間；選擇器風格的拼寫用 `TimePicker`，還需要日期時用 `DateTimePicker`。

**屬性.** 除四個共享屬性外共 7 項：`time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("time_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `time_input`

### TimePicker (`time_picker`)

帶時鐘彈層的時間欄位的 `time_picker` 工廠名稱；它與 `time_edit` 共享 `TimePicker` 類型與 `TIME_EDIT_PROPERTIES` schema。按宿主已有拼寫選擇其一，不要為同一個值同時掛載兩者。

**屬性.** 除四個共享屬性外共 7 項：`time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("time_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `time_field`

### DateTimePicker (`date_time_edit`)

組合日期時間欄位，帶日曆彈層、顯示格式、日期上下限，以及日期與日期時間兩種變化訊號。一個值同時包含兩部分時使用；兩者分開編輯時用 `DatePicker` 加 `TimePicker`。

**屬性.** 除四個共享屬性外共 7 項：`datetime`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `minimum`: String · `maximum`: String · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `datetime_changed`: String · `editing_finished` · `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_datetime`, `set_display_format`, `set_calendar_popup`

**Rust API.** `create_date_time_edit(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("date_time_edit", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `datetimepicker`, `date_time_picker`

### Dial (`dial`)

把角度對應為有界整數的旋鈕，帶刻度、目標刻度與迴繞。用於角度自然成立的儀表式控制（音量、聲像、表盤）；線性範圍用 `Slider`，精確輸入用 `SpinBox`。

**屬性.** 除四個共享屬性外共 8 項：`minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `notches_visible`: Bool · `notch_target`: Float · `wrapping`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_wrapping`

**Rust API.** `create_dial(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("dial", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `knob`

### TabBar (`tab_bar`)

與頁面解耦的獨立標籤列：標籤數、目前索引、可關閉、可移動、最小/最大寬度與標籤事件。標籤列與頁面容器分離時使用；控制項應同時擁有兩者時用 `TabWidget`。

**屬性.** 除四個共享屬性外共 7 項：`tab_count`: UInt (唯讀) · `current_index`: UInt · `closable`: Bool · `movable`: Bool · `tab_min_width`: UInt · `tab_max_width`: UInt · `direction`: Enum (`ltr` | `rtl`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt · `tab_bar_clicked`: UInt or null · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`

**Rust API.** `create_tab_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("tab_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### RibbonBar (`ribbon_bar`)

Office 風格功能區：標籤、命令群組、大小條目、快速存取列與摺疊/最小化。用於命令密集的桌面應用；單列動作用 `ToolBar`，經典選單用 `MenuBar`。

**屬性.** 除四個共享屬性外共 6 項：`tab_count`: UInt (唯讀) · `current_tab`: UInt · `expanded`: Bool · `minimized`: Bool · `quick_access_items`: StringList · `quick_access_overflow_menu`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `application_clicked` · `current_tab_changed`: UInt · `group_overflow_clicked`: UInt ×2 · `item_triggered`: UInt ×3 · `quick_access_clicked`: UInt · `quick_access_overflow_clicked`

**命令.** `add_tab`, `add_group`, `add_item`, `add_large_item`, `clear`

**Rust API.** `create_ribbon_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("ribbon_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ribbon`

### PieMenu (`pie_menu`)

徑向選單，條目圍繞指標排成一圈，帶外半徑、內半徑與目前索引。用於在某個點呼叫少量次要動作；更長或需要鍵盤導覽的清單用 `Menu`/`ContextMenu`。

**屬性.** 除四個共享屬性外共 4 項：`item_count`: UInt (唯讀) · `radius`: Float · `inner_radius`: Float · `current_index`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `triggered`: UInt · `triggered_text`: String

**命令.** `add_item`, `remove_item`, `set_radius`, `set_current_index`

**Rust API.** `create_pie_menu(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("pie_menu", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `radial_menu`

### LineEdit (`key_sequence_editor`)

把鍵盤快速鍵錄製為按鍵序列的欄位，回報衝突與新序列。該 schema 沒有發布屬性列，因此應透過控制項 API 與訊號驅動，而不是透過 `WidgetFactory` 的屬性名稱。

**屬性.** 該 schema 沒有自己的屬性；每個控制項都帶四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `conflict_detected`: String · `editing_finished` · `key_sequence_changed`: String · `sequence_changed`: list of String

**命令.** 未發布命令。

**Rust API.** `create_key_sequence_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("key_sequence_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `key_sequence_edit`

## 15. 工具與行動控制項

雜項族收集不需要獨立家族的小控制項：頭像、QR 碼、拖放目標、日期範圍、分段按鈕、條碼掃描、曲線編輯與範圍預設。

使用者頭像（帶首字母回退）用 `Avatar`，渲染確定性 QR 碼用 `QRCode`，按 MIME 類型過濾的具名拖放目標用 `DropZone`，起止日期用 `DateRangePicker`，多選分段用 `SegmentedButton`，攝影機掃描用 `BarcodeScanner`，緩動曲線用 `BezierCurveEditor`，範圍選擇器旁用 `RangePresetBar`。不要把 `DropZone` 當通用拖曳來源（應實作 `DragSource`），也不要用 `QRCode` 解碼條碼——它只負責渲染。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `misc` | `src/widget/misc_widgets/` | 9 |

### DropZone (`drop_zone`)

按 MIME 類型過濾的具名拖放目標，帶懸停狀態、多檔案旗標與進入/離開/放下/拒絕訊號。宿主接受檔案或拖曳載荷時使用；目標本身是已有控制項而不是獨立區域時，直接實作 `DropTarget`。

**屬性.** 除四個共享屬性外共 8 項：`accepted_type`: String · `state`: Enum (`idle` | `hovering` | `accepted` | `rejected` | `dropped`) · `hovered`: Bool (唯讀) · `accepted_types`: StringList · `multi_file`: Bool · `hover_style`: String · `show_count_badge`: Bool · `dropped_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drop_rejected` · `dropped`: list of String · `payload_dropped`: String · `zone_entered`: list of String · `zone_exited`: list of String

**命令.** 未發布命令。

**Rust API.** `create_drop_zone(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("drop_zone", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `drop_target`

### MobileDatePicker (`mobile_date_picker`)

為觸控而非桌面日曆網格佈局的日期選擇器，帶年份跨度與日期變化訊號。用於手機佈局；桌面端用 `Calendar` 或 `DatePicker`，需要 iOS 滾輪時用 `CupertinoDatePicker`。

**屬性.** 除四個共享屬性外共 2 項：`selected_date`: String · `year_span`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `date_changed`: String

**命令.** `set_selected_date`

**Rust API.** `create_mobile_date_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mobile_date_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### QRCode (`qr_code`)

從資料字串渲染確定性 QR 圖案的控制項，帶尺寸、靜區、糾錯級別、版本邊界與顏色。用於顯示供使用者掃描的碼；它只渲染不解碼，讀取用 `BarcodeScanner`。

**屬性.** 除四個共享屬性外共 8 項：`data`: String · `size`: UInt · `quiet_zone`: UInt · `ec_level`: Enum (`L` | `M` | `Q` | `H`) · `min_version`: UInt · `max_version`: UInt · `foreground_color`: Color · `background_color`: Color — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 該控制項不發布任何 capability 事件。

**命令.** `set_data`, `set_size`

**Rust API.** `create_qr_code(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("qr_code", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `qr`

### Avatar (`avatar`)

圓形或方形使用者影像佔位，帶首字母回退、尺寸、可選徽標、影像來源與圓環。用於個人或帳戶身分；只有在計數屬於別處時才額外配 `Badge`——頭像本身已經有 `badge` 文字。

**屬性.** 除四個共享屬性外共 7 項：`initials`: String · `square`: Bool · `size`: UInt · `image_source`: String · `badge`: String · `ring_color`: Color · `ring_width`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `avatar_clicked`

**命令.** `set_initials`, `set_image_source`

**Rust API.** `create_avatar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("avatar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `avatar_widget`, `user_avatar`

### SegmentedButton (`segmented_button`)

帶 id 與圖示、支援單選或多選與群組 id 的橫向可選分段組。用於 Material 3 風格圖示分段或多選；單選、帶標籤、指示器滑動時用 `SegmentedControl`，iOS 膠囊外觀用 `CupertinoSegmentedControl`。

**屬性.** 除四個共享屬性外共 5 項：`selected_index`: UInt · `selected_indices`: String · `allows_multiple`: Bool · `segment_count`: UInt (唯讀) · `group_id`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_segmented_button(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("segmented_button", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `segmented_btn`

### DateRangePicker (`date_range_picker`)

基於日曆的起止範圍選擇器，帶預設、雙月檢視與最少/最多夜數。用於預訂或報表區間；預設需要放在選擇器之外時配 `RangePresetBar`，單一日期用 `Calendar`。

**屬性.** 除四個共享屬性外共 6 項：`start_date`: String · `end_date`: String · `presets`: StringList · `double_month`: Bool · `min_nights`: UInt · `max_nights`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `preset_selected`: UInt · `range_changed`: String

**命令.** `set_start_date`, `set_end_date`

**Rust API.** `create_date_range_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("date_range_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `range_picker`

### BarcodeScanner (`barcode_scanner`)

掃描取景器，帶掃描旗標與 `barcode_detected` 載荷。攝影機與解碼由宿主負責；本控制項持有掃描狀態並回報解碼文字。`QRCode` 用於渲染碼，不用於讀取。

**屬性.** 除四個共享屬性外共 1 項：`is_scanning`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `barcode_detected`: String

**命令.** `set_is_scanning`

**Rust API.** `create_barcode_scanner(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("barcode_scanner", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `scanner`

### BezierCurveEditor (`bezier_curve_editor`)

用於自訂緩動曲線的互動式三次 Bézier 編輯器：控制點、段數、吸附與曲線變化載荷。用於運動或動畫設定面板；保存發出的曲線文字並交給動畫引擎，而不是重新計算。

**屬性.** 除四個共享屬性外共 3 項：`snap_to_grid`: Bool · `segments`: UInt (唯讀) · `curve_points`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `curve_changed`: mixed (String parts)

**命令.** `set_snap_to_grid`

**Rust API.** `create_bezier_curve_editor(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("bezier_curve_editor", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `curve_editor`

### RangePresetBar (`range_preset_bar`)

範圍選擇器旁的一列快速日期區間 chip，帶方向、緊湊模式與選中標籤。日期由選擇器持有；該列只回報所選預設，因此把 `preset_selected` 接到 `DateRangePicker::apply_preset` 或你自己的對應。

**屬性.** 除四個共享屬性外共 4 項：`presets`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `compact`: Bool · `selected_label`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `preset_selected`: String ×2

**命令.** 未發布命令。

**Rust API.** `create_range_preset_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("range_preset_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `preset_bar`

## 16. 播放與動畫

媒體族渲染隨時間變化的內容：逐格動畫與 Lottie/Rive 動畫、Hero 轉場、視訊播放、音訊電平與攝影機預覽。

逐格序列用 `AnimatedImage`，向量動畫資源用 `LottieWidget` 或 `RiveWidget`，兩個佈局間的共享元素轉場用 `HeroAnimation`，帶傳輸狀態的視訊用 `VideoPlayer`，電平或頻譜讀數用 `AudioVisualizer`，取景器用 `CameraPreview`。互動式動畫圖（Rive）不要用 `AnimatedImage`；純音訊播放也不要用 `VideoPlayer`——它的畫面與影格訊號都白費。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `media` | `src/widget/media_widgets/` | 7 |

### AnimatedImage (`animated_image`)

播放動圖（GIF、APNG 或 WebP 影格序列），支援播放/暫停、方向、影格快取上限與影格/動畫完成訊號。用於逐格序列資源；向量動畫用 `LottieWidget` 或 `RiveWidget`，定時媒體用 `VideoPlayer`。

**屬性.** 除四個共享屬性外共 3 項：`playing`: Bool · `direction`: Enum (`forward` | `reverse`) · `frame_cache_limit`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished` · `frame_changed`: UInt

**命令.** `set_playing`

**Rust API.** `create_animated_image(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("animated_image", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `anim_image`

### HeroAnimation (`hero_animation`)

在兩個佈局之間插值位置、尺寸與透明度的共享元素轉場，帶進度值與完成訊號。同一物件出現在兩個螢幕時使用；宿主驅動 `animation_progress`，結束時換回真實內容。

**屬性.** 除四個共享屬性外共 1 項：`animation_progress`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_completed`

**命令.** `set_animation_progress`

**Rust API.** `create_hero_animation(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("hero_animation", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `hero`

### LottieWidget (`lottie_widget`)

Lottie JSON 動畫播放器，支援播放/暫停、循環次數、方向與速度。用於設計工具匯出的向量動效資源；動畫需要互動式狀態機與輸入時用 `RiveWidget`。

**屬性.** 除四個共享屬性外共 5 項：`playing`: Bool · `loop_count`: Int · `loops_completed`: Int (唯讀) · `direction`: Enum (`forward` | `reverse`) · `speed`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished`

**命令.** `set_playing`

**Rust API.** `create_lottie_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("lottie_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `lottie`

### RiveWidget (`rive_widget`)

Rive 執行時表面，帶播放狀態、循環、狀態字串與輸入觸發/狀態變化訊號。用於帶狀態機的互動動畫；線性匯出動畫用 `LottieWidget`，逐格序列用 `AnimatedImage`。

**屬性.** 除四個共享屬性外共 4 項：`is_playing`: Bool · `loop_count`: Int · `loops_completed`: Int (唯讀) · `state`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished` · `input_fired`: String · `state_changed`: String

**命令.** `set_is_playing`

**Rust API.** `create_rive_widget(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("rive_widget", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `rive`

### VideoPlayer (`video_player`)

視訊表面，帶播放狀態、音量、靜音、播放速率、控制項、循環與影格/中繼資料/錯誤訊號集。宿主提供媒體管線時使用；配傳輸配套控制項，並且只透過控制項自身方法停止/定位，以保持其時鐘權威。

**屬性.** 除四個共享屬性外共 7 項：`is_playing`: Bool · `volume`: Float · `muted`: Bool · `playback_rate`: Float · `controls_visible`: Bool · `looping`: Bool · `pipeline`: String (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `buffering_progress_changed`: Float · `duration_changed`: Float · `error_occurred`: String · `frame_available`: UInt · `frame_presented`: Float · `fullscreen_changed`: Bool · `media_status_changed`: String · `metadata_changed` · `mute_changed`: Bool · `playback_ended` · `playback_paused` · `playback_rate_changed`: Float · `playback_started` · `time_updated`: Float · `volume_changed`: Float

**命令.** `set_is_playing`, `set_volume`

**Rust API.** `create_video_player(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("video_player", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `video`

### AudioVisualizer (`audio_visualizer`)

即時音訊電平或頻譜檢視，帶柱數、峰值閾值、模式與週期性電平回報。用於音訊播放時的回饋；分析由宿主完成並餵入電平；不要把它當音訊編輯器。

**屬性.** 除四個共享屬性外共 4 項：`bar_count`: UInt · `peak_threshold`: Float · `mode`: Enum (`waveform` | `spectrum`) · `level_report_interval`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_changed`: Float · `mode_changed`: String · `peak_exceeded`: Float

**命令.** `set_bar_count`

**Rust API.** `create_audio_visualizer(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("audio_visualizer", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `audio_viz`

### CameraPreview (`camera_preview`)

攝影機取景器，帶活動旗標、像素格式、目標影格率與攝影機 id，並回報影格、格式協商、權限拒絕與錯誤。用於拍攝或掃描；裝置工作階段由宿主持有，控制項只呈現工作階段回報的事實。

**屬性.** 除四個共享屬性外共 4 項：`is_active`: Bool · `pixel_format`: String · `target_fps`: UInt · `camera_id`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `error_occurred`: String · `format_negotiated`: UInt ×2 · `frame_available`: UInt · `frame_presented`: Float · `permission_denied`

**命令.** `set_is_active`

**Rust API.** `create_camera_preview(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("camera_preview", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `camera`

## 17. 內嵌 Web 內容

Web 族只暴露一個控制項 `WebEngineView`，以及定位同一檢視不同部分的具名包裝型別（`WebView`、`WebEnginePage`、`WebEngineSettings`、`WebEngineContextMenuRequest`、`WebEngineCookieStore`、`WebEngineDownloadItem`、`WebEngineFindTextResult`、`WebEngineNotification`、`WebEngineScriptDialog`、`WebEngineWebChannel`）。

宿主必須渲染自己不擁有的 HTML 時才用 `WebEngineView`——說明頁、第三方登入、內嵌儀表板。crate 自己能渲染的內容不要用它：富文字 `Label`、`Canvas` 或原生檢視可以避免引擎的程序、記憶體與安全成本。把導覽、下載與腳本對話框當作宿主必須應答的事件。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `web` | `src/widget/web_widgets/` | 1 |

### WebEngineView (`web_engine_view`)

內嵌 Web 檢視：URL、載入狀態、標題、前進/後退、使用者代理、透明背景與 Cookie 持久化，並帶導覽、載入、主控台、下載、憑證與崩潰訊號。僅用於宿主不擁有的 HTML；把每個訊號都當作宿主必須決定的事項。

**屬性.** 除四個共享屬性外共 8 項：`url`: String · `loading`: Bool (唯讀) · `title`: String · `can_go_back`: Bool · `can_go_forward`: Bool · `user_agent`: String · `background_transparent`: Bool · `cookie_persistence_enabled`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `certificate_error`: String · `channel_message`: String ×2 · `console_message`: mixed (String parts) · `download_requested`: String · `error_occurred`: String · `js_evaluated`: mixed (String parts) · `link_hovered`: String · `load_progress_changed`: UInt · `loading_finished`: String · `loading_started`: String · `navigation_state_changed`: Bool ×2 · `page_created`: UInt · `page_destroyed`: UInt · `render_process_crashed`: String · `title_changed`: String · `url_changed`: String

**命令.** `set_url`, `load_url`, `reload`, `go_back`, `go_forward`, `stop`

**Rust API.** `create_web_view(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("web_engine_view", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `webview`, `web_view`

## 18. Cupertino 與 Material 呈現

本族提供平台風格的呈現：iOS 風格的開關、警告框、滑桿、導覽列、分段控制項與日期選擇器，以及補全自適應組合的 Material snackbar 與導覽列。

介面必須在 Apple 平台上看起來原生、且宿主已透過執行時查詢確定平台時，選 Cupertino 控制項。需要 iOS 拇指與軌道外觀時用 `CupertinoSwitch` 而不是 `Switch`；它有自己的類型、capability 列與建構函式，不是別名。不要在共享程式碼裡按目標 OS 分支——執行時詢問平台層——也不要在同一介面混用 Cupertino 與 Material 外殼。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `cupertino` | `src/widget/cupertino/` | 8 |

### CupertinoSwitch (`cupertino_switch`)

iOS 風格開關，擁有自己的類型、capability 列與建構函式；它不是 `Switch` 的別名。需要 iOS 拇指與軌道呈現時使用；只有幾何不同時用 `Switch` 並設定 `switch_geometry`。

**屬性.** 除四個共享屬性外共 4 項：`checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_checked`, `toggle`

**Rust API.** `create_cupertino_switch(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_switch", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ios_switch`

### CupertinoAlertDialog (`cupertino_alert_dialog`)

iOS 風格模態警告框，帶標題、訊息與確認/取消結果。用於二選一的 iOS 警告；桌面警告族用 `AlertDialog`/`ConfirmDialog`，經典固定按鈕形式用 `MessageBox`。

**屬性.** 除四個共享屬性外共 2 項：`title`: String · `message`: String — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `confirmed`

**命令.** `set_title`, `set_message`

**Rust API.** `create_cupertino_alert_dialog(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_alert_dialog", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ios_alert`

### CupertinoSlider (`cupertino_slider`)

iOS 風格滑桿，圓角軌道與圓形把手，帶浮點值與上下限。用於 iOS 風格介面上的連續值；整數步進用 `Slider`，雙邊界用 `RangeSlider`。

**屬性.** 除四個共享屬性外共 3 項：`value`: Float · `min`: Float · `max`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Float

**命令.** `set_value`, `set_min`, `set_max`

**Rust API.** `create_cupertino_slider(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_slider", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ios_slider`

### CupertinoNavigationBar (`cupertino_navigation_bar`)

iOS 風格導覽列，帶標題、可選大標題、提示、前後置條目與摺疊偏移。用於 iOS 螢幕頂部；標題需要跟隨頁面堆疊時配 `CupertinoNavigationStack` 配套控制項。

**屬性.** 除四個共享屬性外共 4 項：`title`: String · `large_title`: Bool · `prompt`: String · `collapse_offset`: Float — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `back_pressed` · `leading_item_pressed`: UInt · `trailing_item_pressed`: UInt

**命令.** `set_title`, `set_large_title`

**Rust API.** `create_cupertino_navigation_bar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_navigation_bar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `cupertinonavbar`, `ios_nav_bar`

### CupertinoSegmentedControl (`cupertino_segmented_control`)

iOS 風格膠囊分段控制項，帶選中索引與值變化訊號。用於 iOS 螢幕上兩到五個互斥選擇；桌面呈現用 `SegmentedControl`，多選用 `SegmentedButton`。

**屬性.** 除四個共享屬性外共 2 項：`selected_index`: UInt · `segment_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_cupertino_segmented_control(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_segmented_control", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `cupertinosegmented`, `ios_segmented`

### CupertinoDatePicker (`cupertino_date_picker`)

iOS UIPickerView 風格滾動輪日期選擇器，支援日期、時間、日期時間或倒數模式、24 小時制與分鐘間隔。用於 iOS 風格面板內；Material 觸控佈局用 `MobileDatePicker`，桌面用 `DatePicker`。

**屬性.** 除四個共享屬性外共 4 項：`selected_date`: String · `mode`: Enum (`date` | `time` | `datetime` | `countdown`) · `use_24_hour`: Bool · `minute_interval`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `date_changed`: mixed (String parts) · `time_changed`: UInt ×2

**命令.** `set_selected_date`

**Rust API.** `create_cupertino_date_picker(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("cupertino_date_picker", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `ios_date_picker`

### MaterialSnackbar (`material_snackbar`)

Material snackbar，帶訊息、動作標籤、持續時間與滑入進度。用於帶一個動作的簡短確認；持久通知用 `Banner`，多條訊息需要排隊時用 `ToastStack`/`ToastHost`。

**屬性.** 除四個共享屬性外共 4 項：`message`: String · `action_text`: String · `duration_ms`: UInt · `slide_progress`: Float (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_pressed` · `dismissed`

**命令.** `show`, `dismiss`, `set_message`, `set_action_text`

**Rust API.** `create_material_snackbar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("material_snackbar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### MaterialNavigationRail (`material_navigation_rail`)

平板寬度佈局的 Material 側導覽列，帶選中索引與目前變化訊號。視窗太窄放不下抽屜、又太寬不適合底部列時使用；長目的清單用 `NavigationDrawer`，手機用 `BottomNavigationBar`。

**屬性.** 除四個共享屬性外共 1 項：`selected_index`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_material_navigation_rail(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("material_navigation_rail", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

**別名.** `nav_rail`, `navigation_rail`

## 19. macOS 呈現

macOS 族提供顯式命名、原生外觀的 macOS 外殼控制項：分段控制項、帶重複的步進器、帶溢出的統一台工具列、來源清單與視窗外殼列。它們各自回報既有的 `WidgetKind`，但透過自己的工廠名稱存取，由宿主顯式選擇。

macOS 介面需要符合平台度量與互動習慣時選它們。不要在其他平台預設使用，也不要假設它們與通用對應物可互換：屬性表不同（`mac_stepper` 多了重複延遲，`mac_toolbar` 多了 `selectable`，`mac_window_chrome` 多了 `document_edited`）。

| 控制項族 | 原始碼目錄 | 控制項數 |
|---|---|---|
| `macos` | `src/widget/macos/` | 5 |

### SegmentedControl (`mac_segmented_control`)

分段控制項的 macOS 呈現：選中索引、分段數與值變化訊號，採用平台度量。用於 macOS 原生外殼；通用控制項用 `SegmentedControl`，iOS 用 `CupertinoSegmentedControl`。

**屬性.** 除四個共享屬性外共 2 項：`selected_index`: UInt · `segment_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: UInt

**命令.** 未發布命令。

**Rust API.** `create_mac_segmented_control(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mac_segmented_control", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Stepper (`mac_stepper`)

步進器的 macOS 呈現，在通用值/範圍/步長模型上增加 `wraps`、`repeat_delay_ms` 與 `repeat_interval_ms`。用於 macOS 原生數字外殼；可移植控制項用 `Stepper`，需要鍵入時用 `SpinBox`。

**屬性.** 除四個共享屬性外共 7 項：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wraps`: Bool · `repeat_delay_ms`: UInt · `repeat_interval_ms`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** 未發布命令。

**Rust API.** `create_mac_stepper(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mac_stepper", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ToolBar (`mac_toolbar`)

macOS 統一風格的工具列呈現：條目、圖示尺寸、方向、可選/選中條目與溢出數。用於 macOS 視窗頂部；可移植列用 `ToolBar`，命令需要帶標籤分組時用 `RibbonBar`。

**屬性.** 除四個共享屬性外共 6 項：`item_count`: UInt (唯讀) · `icon_size`: Float · `orientation`: String (唯讀) · `selectable`: Bool · `selected_index`: UInt · `overflow_count`: UInt (唯讀) — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_activated`: UInt · `overflow_activated`: UInt · `overflow_changed`: UInt

**命令.** 未發布命令。

**Rust API.** `create_mac_toolbar(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mac_toolbar", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### ListView (`mac_source_list`)

清單的 macOS 來源清單呈現：分區、列、選中列、列高與徽標/選擇訊號。用於 macOS 側欄；可移植清單用 `ListView`，使用抽屜時配 `DrawerSectionHeader`。

**屬性.** 除四個共享屬性外共 4 項：`section_count`: UInt (唯讀) · `row_count`: UInt (唯讀) · `row_height`: UInt (唯讀) · `selected_row`: UInt — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `badge_changed`: mixed (String parts) · `selection_changed`: String

**命令.** 未發布命令。

**Rust API.** `create_mac_source_list(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mac_source_list", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

### Window (`mac_window_chrome`)

macOS 視窗外殼：標題、外殼高度、紅綠燈按鈕、拖曳/全螢幕/文件已編輯旗標與視窗生命週期請求。macOS 視窗需要符合平台標題列時使用；可移植視窗用 `Window`。

**屬性.** 除四個共享屬性外共 7 項：`title`: String · `chrome_height`: UInt (唯讀) · `buttons_visible`: Bool · `draggable`: Bool · `fullscreen`: Bool · `document_edited`: Bool · `focused`: Bool — 外加四個共享屬性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `close_requested` · `fullscreen_requested` · `minimize_requested` · `window_focus_changed`: Bool · `zoom_requested`

**命令.** 未發布命令。

**Rust API.** `create_mac_window_chrome(geometry, text)` —— 註冊的建構函式。透過工廠：`WidgetFactory::create("mac_window_chrome", geometry, text)`；註冊表用 `WidgetFactory::shared()`。

## 複合控制項：狀態歸誰

有些控制項是設計成與夥伴一起使用的，而不是單獨使用。下表說明每一對中哪一側持有狀態、哪一側負責繪製，以及把它們接起來的呼叫。相同資訊也出現在相關控制項條目中；這裡集中列出，方便宿主在接線整屏時一次看全。

| 主控制項 | 配套控制項 | 狀態歸屬 | 繪製方與接線方式 |
|---|---|---|---|
| `ColorWell` | `ColorPicker`, `ColorDialog` | 色塊持有目前顏色；選擇器持有編輯狀態。 | `picker_requested` 開啟選擇器或對話框；接受選擇後由 `color_selected` 呼叫 `ColorWell::set_color`。 |
| `ColorDialog` | `Swatch` | 對話框持有目前顏色；色格持有調色盤與選中索引。 | `swatch_selected` 透過 `set_hex` 寫入對話框顏色；`Swatch::set_selected_index` 反映別處選中的顏色。 |
| `ColorPicker` | `ColorHistory` | 選擇器持有目前顏色；歷史持有最近顏色清單。 | 提交時把顏色加入歷史；歷史的 `color_selected` 設定選擇器顏色。 |
| `DataGrid` | `FilterRow` | 網格持有模型與已套用篩選；列持有條件編輯器。 | `filter_changed` 攜帶條件文字；網格重新查詢，並同步列的 `live` 旗標。 |
| `GridTable` | `GridViewHeader` | 網格持有列欄與捲動；表頭持有排序/篩選入口與區段寬度。 | 把 `section_clicked` 轉給 `toggle_sort_column`，`section_resized` 轉給 `set_column_width`，`filter_clicked` 轉給網格篩選狀態。 |
| `GridTable` | `GridFindBar`, `ColumnChooser` | 網格持有資料與選擇；尋找列持有查詢；欄選擇器持有欄可見性。 | 尋找列掃描網格可見欄；欄選擇器的開關呼叫網格的欄可見性設定方法。 |
| `LineChart`, `BarChart`, `PieChart` | `Legend` | 圖表持有序列與可見性旗標；圖例持有條目順序與醒目提示。 | `series_toggled` 呼叫圖表的序列可見性設定方法；圖例根據圖表的序列名重建條目。 |
| any chart | `ChartTooltip` | 圖表持有資料與幾何；工具提示只持有文字列與可見性。 | 懸停時圖表用吸附點呼叫 `show_at`，離開時呼叫 `hide`，並用懸停資料填充工具提示的列。 |
| any control | `TooltipManager` | 控制項持有自己的 `tooltip` 文字；管理器持有延遲、持續時間與佇列。 | 把懸停進入/離開餵給管理器；它在 `delay_ms` 後顯示 `Tooltip`，並在 `duration_ms` 後隱藏。 |
| `ImageView` | `ZoomPanController` | 影像檢視持有像素與繪製；控制器持有縮放、平移與 ROI。 | `zoom_changed` 呼叫 `ImageView::set_zoom_level`，`pan_changed` 呼叫 `set_pan_offset`，`roi_changed` 驅動裁切或詳情檢視。 |
| `ImageGallery` | `ThumbnailCache` | 畫廊持有目前索引與縮圖列；快取持有降取樣點陣圖。 | 畫廊按索引向快取索取縮圖並繪製；快取負責淘汰，從不繪製。 |
| `MapView` | `TileLoader`, `ZoomControl`, `ScaleBar`, `Compass` | 地圖持有四個配套控制項（存取器 `tile_loader()`、`zoom_control()`、`scale_bar()`、`compass()`）；瓦片抓取由宿主持有。 | `TileLoader::load_tile` 產生 `TileRequest`；宿主抓取後呼叫 `MapView::deliver_tile` 或 `reject_tile`；`zoom_requested` 與 `reset_north_requested` 由地圖套用。 |
| `GanttWidget` | `TimelineAxis`, `DependencyEditor` | 甘特圖持有兩個配套控制項（`timeline_axis()`、`dependency_editor()`）與任務清單。 | `set_dependencies` / `sync_dependencies_from_tasks` 保持編輯器與長條一致；`pixel_to_date` 與 `date_to_pixel` 來自軸。 |
| `DiffViewer` | `DiffFileList` | 檢視器持有兩段文字、差異與選擇；檔案清單只持有選中的路徑。 | `file_selected` 告訴宿主讀取哪個路徑；隨後宿主呼叫 `DiffViewer::set_texts`。 |
| `MarkdownEditor` | `MarkdownOutlinePanel`, `FindBar` | 編輯器持有文件、標題與游標；面板只持有作用中大綱列；尋找列持有查詢。 | 用 `outline()` 建構面板；`entry_selected` 呼叫 `goto_outline`；`search_changed` 呼叫 `find_next` / `find_previous`。 |
| `KanbanBoard` | `KanbanCardView`, `KanbanColumnHeader` | 看板持有欄、卡片與拖放；卡片檢視持有一張卡片的內容；表頭持有自己的重新命名與摺疊狀態。 | 從 `columns()`/`cards` 建構列；`column_renamed` 呼叫看板的欄修改方法；`card_edited` 透過看板寫回，以免遺失拖曳狀態。 |
| `DateRangePicker` | `RangePresetBar` | 選擇器持有起止日期；預設列持有預設標籤與選中 chip。 | `preset_selected` 對應到 `DateRangePicker::apply_preset`（或你自己的日期計算）；`range_changed` 更新預設列的選中標籤。 |
| `FAB` | `SpeedDial` | FAB 持有摺疊的主操作；快速撥號持有展開的次級動作。 | FAB 的 `clicked` 切換快速撥號；每個動作發出自己的意圖，觸發後由宿主收起撥號。 |
| `Button`, `ToggleButton`, `ToolButton` | `MenuPanel` | 按鈕持有按下/選中狀態與 `menu_open` 旗標；面板持有條目清單與醒目提示列。 | 設定 `menu_open`，把面板錨定在按鈕下方，並把 `item_clicked` 轉給命令分發器；面板在 `cancelled` 時自行關閉。 |
| `SwipeToDismiss` | `SwipeActions` | 容器持有滑動偏移與已關閉狀態；動作持有背景按鈕及其顏色。 | 容器回報露出的側與偏移；動作繪製該條帶，並在被點擊時發出自己的意圖。 |
| `NavigationDrawer` | `DrawerSectionHeader` | 抽屜持有開啟狀態、選擇與寬度；分區標題是不可選擇的標籤列。 | 在條目列之間插入標題；它們不回報選擇，因此鍵盤導覽會跳過。 |
| `Canvas` | `LayerPanel` | 畫布持有圖層及其順序；面板是按圖層 id 尋址的唯讀鏡像。 | 面板讀取圖層 id 與名稱；點擊請求畫布選中該 id，圖層順序的唯一寫入者仍是畫布。 |
| `ScrollArea` | `StickyHeader` | 捲動區域持有視埠與捲動偏移；黏性標題持有它在條帶中繪製的內容。 | 用 `ScrollArea::set_sticky_header` 掛載；區域偏移子控制項，但把標題保持在條帶處。 |
| `VideoPlayer` | `MediaTransportBar`, `MediaScrubber`, `SubtitleBar` | 播放器持有時鐘、來源與播放狀態；配套控制項持有自己的控制項並繪製自己的條帶。 | 按鈕呼叫 `play`、`pause`、`seek_to`、`set_volume`；`time_updated` 與 `duration_changed` 驅動進度列與字幕時序。 |
| `WebEngineView` | `WebFindBar` | 檢視持有文件與導覽狀態；尋找列持有查詢與符合數。 | `search_changed` 驅動檢視的尋找操作；檢視把符合數回報回尋找列。 |
| `CupertinoNavigationBar` | `CupertinoNavigationStack` | 導覽列持有標題/提示呈現；頁面堆疊持有頁面清單與返回歷史。 | `stack_changed` 更新導覽列標題與返回可見性；`back_pressed` 呼叫頁面堆疊的 `pop`。 |
| `RichEdit` | `FormatToolbar` | 編輯器持有文件與選擇；工具列持有格式動作集及其啟用狀態。 | `FormatAction` 套用到目前選擇；工具列狀態跟隨選擇的格式，而不是反過來。 |
| `ComboBox`, `AutoCompleteEdit` | `SuggestionPopup` | 欄位持有文字與已提交值；彈層持有候選清單與醒目提示索引。 | 欄位從自己的模型填充彈層候選；`suggestion_selected` 把所選值寫回欄位。 |

## WidgetKind 覆蓋表

`src/widget/kind.rs` 中的列舉宣告了 221 個變體。下面列出每一個及其所屬族。有 11 個變體沒有自己的 capability 列：它們是別名或子項型別，條目中說明了它們如何解析。

| 變體 | 記錄於 |
|---|---|
| `Window` | `base` |
| `Dialog` | `dialog` |
| `MessageBox` | `dialog` |
| `FileDialog` | `dialog` |
| `ColorDialog` | `dialog` |
| `FontDialog` | `dialog` |
| `InputDialog` | `dialog` |
| `ProgressDialog` | `dialog` |
| `PopupWindow` | `dialog` |
| `Button` | `base` |
| `CheckBox` | `base` |
| `RadioButton` | `base` |
| `Label` | `base` |
| `LineEdit` | `input` |
| `TextEdit` | `input` |
| `RichEdit` | `input` |
| `ComboBox` | `input` |
| `SpinBox` | `input` |
| `ListBox` | `input` |
| `ListView` | `view` |
| `TreeView` | `view` |
| `ProgressBar` | `display` |
| `Slider` | `display` |
| `ScrollBar` | `display` |
| `ScrollArea` | `container` |
| `Panel` | `base` |
| `Frame` | `base` |
| `DockPanel` | `container` |
| `GroupBox` | `base` |
| `TabWidget` | `container` |
| `Splitter` | `container` |
| `MdiArea` | `container` |
| `MenuBar` | `menu_toolbar` |
| `Menu` | `menu_toolbar` |
| `MenuItem` | `menu_toolbar` |
| `ContextMenu` | `menu_toolbar` |
| `ToolBar` | `menu_toolbar` |
| `StatusBar` | `menu_toolbar` |
| `Canvas` | `special` |
| `Table` | `view` |
| `Grid` | `special` |
| `Chart` | `special` |
| `RadarChart` | `special` |
| `Heatmap` | `special` |
| `KanbanBoard` | `special` |
| `ToggleButton` | `base` |
| `CheckListBox` | `input` |
| `DoubleSpinBox` | `input` |
| `Dial` | `advanced` |
| `Wizard` | `dialog` |
| `DatePicker` | `advanced` |
| `TimePicker` | `advanced` |
| `DateTimePicker` | `advanced` |
| `DirectoryDialog` | `dialog` |
| `DataView` | `view` |
| `EmojiPicker` | `display` |
| `Mention` | `input` |
| `QueryBuilder` | `view` |
| `PropertyGrid` | `view` |
| `Toolbox` | `container` |
| `StackedWidget` | `container` |
| `CollapsiblePane` | `container` |
| `DockWidget` | `container` |
| `ActivityIndicator` | `display` |
| `Calendar` | `advanced` |
| `ColumnView` | `view` |
| `UndoView` | `view` |
| `CommandLink` | `input` |
| `LCDNumber` | `display` |
| `FontComboBox` | `input` |
| `WebEngineView` | `special` |
| `Action` | `menu_toolbar` |
| `ToolButton` | `menu_toolbar` |
| `Cascader` | `input` |
| `FreeformShape` | `special` |
| `TabBar` | `advanced` |
| `PieMenu` | `advanced` |
| `RibbonBar` | `advanced` |
| `Line` | `display` |
| `Meter` | `display` |
| `MiniChart` | `display` |
| `ImageView` | `display` |
| `MiniCanvas` | `display` |
| `Arc` | `display` |
| `Spinner` | `display` |
| `Roller` | `display` |
| `Dropdown` | `input` |
| `TextArea` | `input` |
| `Keyboard` | `input` |
| `Switch` | `display` |
| `SearchBox` | `input` |
| `Chip` | `special` |
| `Badge` | `display` |
| `SkeletonLoader` | `display` |
| `FAB` | `overlay` |
| `BottomSheet` | `dialog` |
| `BottomNavigationBar` | `nav` |
| `NavigationDrawer` | `nav` |
| `AppBar` | `nav` |
| `MobileDatePicker` | `misc` |
| `Divider` | `display` |
| `Stepper` | `container` |
| `Rating` | `display` |
| `Avatar` | `misc` |
| `EmptyState` | `display` |
| `Carousel` | `container` |
| `ColorHistory` | `display` |
| `ColorWell` | `display` |
| `TagInput` | `input` |
| `ImePreedit` | `input` |
| `InplaceEditor` | `input` |
| `QRCode` | `misc` |
| `MasonryLayout` | `container` |
| `CupertinoSwitch` | `cupertino` |
| `MaterialSnackbar` | `cupertino` |
| `AdaptiveScaffold` | `nav` |
| `WizardDialog` | `dialog` |
| `SafeArea` | `container` |
| `CupertinoAlertDialog` | `cupertino` |
| `CupertinoSlider` | `cupertino` |
| `MaterialNavigationRail` | `cupertino` |
| `Tooltip` | `dialog` |
| `SegmentedButton` | `misc` |
| `NavigationStack` | `nav` |
| `ProgressCircle` | `display` |
| `Icon` | `display` |
| `DropdownMenu` | `menu_toolbar` |
| `MaskedEdit` | `input` |
| `MenuButton` | `menu_toolbar` |
| `Popover` | `dialog` |
| `AutoCompleteEdit` | `input` |
| `MultiSelectComboBox` | `input` |
| `RangeSlider` | `input` |
| `FloatingLabel` | `display` |
| `FontPreview` | `display` |
| `CupertinoNavigationBar` | `cupertino` |
| `CupertinoSegmentedControl` | `cupertino` |
| `SwipeToDismiss` | `overlay` |
| `TabView` | `nav` |
| `SearchBar` | `input` |
| `ShortcutEditor` | `input` |
| `RefreshControl` | `overlay` |
| `ModalBottomSheet` | `dialog` |
| `LineChart` | `chart` |
| `Sparkline` | `chart` |
| `BarChart` | `chart` |
| `CandlestickChart` | `finance` |
| `VolumeChart` | `finance` |
| `DepthChart` | `finance` |
| `OrderBook` | `finance` |
| `QuoteBoard` | `finance` |
| `IndicatorChart` | `finance` |
| `FindReplaceDialog` | `dialog` |
| `PropertiesPanel` | `view` |
| `PieChart` | `chart` |
| `CupertinoDatePicker` | `cupertino` |
| `EditableComboBox` | `input` |
| `DateRangePicker` | `misc` |
| `AnimatedImage` | `media` |
| `HeroAnimation` | `media` |
| `BezierCurveEditor` | `misc` |
| `LottieWidget` | `media` |
| `RiveWidget` | `media` |
| `VideoPlayer` | `media` |
| `ImageGallery` | `view` |
| `AudioVisualizer` | `media` |
| `CameraPreview` | `media` |
| `BarcodeScanner` | `misc` |
| `GridTable` | `view` |
| `NumberPicker` | `input` |
| `OtpInput` | `input` |
| `Banner` | `overlay` |
| `Pagination` | `nav` |
| `ColorPicker` | `special` |
| `Toast` | `special` |
| `SplashScreen` | `overlay` |
| `TreeTable` | `view` |
| `Breadcrumb` | `special` |
| `SignaturePad` | `special` |
| `DropZone` | `misc` |
| `SegmentedControl` | `special` |
| `Spacer` | `container` |
| `Center` | `container` |
| `Align` | `container` |
| `SizedBox` | `container` |
| `Padding` | `container` |
| `Expanded` | `container` |
| `Flexible` | `container` |
| `IntrinsicWidth` | `container` |
| `AspectRatio` | `container` |
| `Flex` | `container` |
| `FlowLayout` | `container` |
| `WrapLayout` | `container` |
| `Tag` | `display` |
| `Gauge` | `chart` |
| `Clock` | `display` |
| `Accordion` | `container` |
| `AlertDialog` | `dialog` |
| `ConfirmDialog` | `dialog` |
| `AboutDialog` | `dialog` |
| `Swatch` | `special` |
| `FilterRow` | `special` |
| `Legend` | `special` |
| `ChartTooltip` | `special` |
| `TooltipManager` | `special` |
| `ZoomPanController` | `special` |
| `ToastHost` | `special` |
| `TileLoader` | `special` |
| `ZoomControl` | `special` |
| `ScaleBar` | `special` |
| `Compass` | `special` |
| `TimelineAxis` | `special` |
| `DependencyEditor` | `special` |
| `DiffFileList` | `special` |
| `MarkdownOutlinePanel` | `code_editor` |
| `FindBar` | `code_editor` |
| `ShapeCanvas` | `special` |
| `KanbanCardView` | `special` |
| `KanbanColumnHeader` | `special` |
| `RangePresetBar` | `misc` |
| `GridViewHeader` | `view` |

## 有意省略與邊界

* **Profile 門控。** 大多數變體宣告在 `#[cfg(widgets_unstripped)]` 下。精簡（`mini` 或 `embedded`）建置只保留始終可用的子集，因此面向此類建置的宿主必須確認名稱可建構，而不能假設本章的每個控制項都存在。

* **`key_sequence_editor` 不發布屬性列。** 除四個共享屬性外，它的 schema 為空。條目如實說明，而不是編造名稱；透過建構函式與訊號驅動它。

* **別名與子項型別。** `Panel`、`DockPanel`、`MenuItem`、`ContextMenu`、`CheckListBox`、`DoubleSpinBox`、`Wizard`、`DirectoryDialog`、`ActivityIndicator`、`ColumnView`、`UndoView` 沒有自己的 capability 列。它們在所屬族中記錄，並說明解析到的名稱。

* **配套控制項是控制項，不是主控制項的零件。** 配套控制項有自己的建構函式、訊號，並且（除非另有說明）有自己的 capability 列；配對表說明誰持有哪部分狀態。繪製總是由擁有該幾何的控制項完成。

* **名稱自解釋的屬性不做轉述。** 名為 `row_count`、型別為 `UInt` 的屬性就是列數；為 700 個名稱重複這句話會淹沒真正需要解釋的名稱。不自解釋的名稱會在控制項條目中註釋。

* **本章不編譯程式碼片段。** 它們是使用真實建構函式與工廠形狀的片段；`mdbook test` 要求每段都是完整程式。`tools/check_cookbook.sh` 校驗識別碼，並以零警告建置三本書。
