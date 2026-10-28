# 控件族详解

本章是 crate 所提供控件的逐个参考。[控件系统](widget-system.md) 一章解释所有控件共享的机制，[API参考](api-reference.md) 列出模块与类型；本章回答宿主真正会问的问题——*这件事该挂哪个控件，能设置什么，它会发出什么，我该怎么调用它？*

本章按控件族组织——即 `src/widget/` 下的模块目录——族内再按 `WidgetKind` 排列。`src/widget/kind.rs` 中声明的每个类型都会出现在这些页面里，包括只作为别名或作为其他控件子项存在的类型。名称的权威列表是 capability 注册表（`WidgetFactory::capabilities()`），类型的权威列表是枚举本身。

## 如何阅读本章

每个控件条目结构一致：

* **标题** —— `WidgetKind` 变体，括号内是规范工厂名。一个类型可以有多个注册名（`line_edit`、`password_edit`、`phone_input`、`email_input` 都报告 `WidgetKind::LineEdit`）；每个名称都有自己的条目，因为各有自己的构造函数与角色。

* **属性** —— 属性名、`PropertyValueKind`，枚举属性还列出可接受的 token。只读属性会标注。每个 schema 还带下一节说明的四个共享属性。条目中给出的常量名（`BUTTON_PROPERTIES` 等）就是该控件发布的 schema，可在 `src/widget/capability/` 中核对。

* **事件** —— `WidgetFactory::connect_event` 接受的事件名，以及载荷类型与形状（`Scalar`、`Tuple2`、列表、可选、混合）。无载荷事件就是普通通知。名称与载荷来自生成的 `event_payloads.rs` 表，`tools/check_event_payload_types.py` 保证它与真实信号一致。

* **命令** —— `WidgetFactory::invoke_command` 为该控件接受的命令名。

* **Rust API** —— 注册的构造函数，以及宿主通常使用的两个工厂入口。构造函数签名为 `fn(Rect, &str) -> Box<dyn Widget>`。

* **别名** —— capability 行发布的其他工厂拼写（若有）。

按名读写属性、调用命令、订阅事件都走同一个注册表，因此一个入口服务所有控件：

```rust
use rust_widgets::core::Rect;
use rust_widgets::signal::CustomSignalHub;
use rust_widgets::widget::capability::WidgetFactory;

let factory = WidgetFactory::shared();
let mut button = factory
    .create("button", Rect::new(10, 10, 120, 32), "保存")
    .expect("注册表认识 `button`");

factory
    .write_property("button", &mut *button, "text", "另存为")
    .expect("`text` 是可写 String 属性");
let current = factory
    .read_property("button", &*button, "text")
    .expect("`text` 可读");

let hub = CustomSignalHub::new();
factory
    .connect_event("button", "clicked", &hub, || { /* 处理点击 */ })
    .expect("`button` 发布了 `clicked`");

factory
    .invoke_command("button", &mut *button, "click")
    .expect("`click` 是已发布命令");
```

## 四个共享属性

`src/widget/capability/` 中的每个属性 schema 都以同样的四个条目结尾。它们由基础层实现，而不是各控件自行实现，因此在各处行为一致：

| 属性 | 类型 | 含义 |
|---|---|---|
| `enabled` | Bool | 控件是否接受输入。禁用的控件仍会绘制并报告其模型。 |
| `visible` | Bool | 控件是否绘制并参与命中测试。 |
| `tooltip` | String | 悬停时显示的提示。`TooltipManager` 配套控件也可为整个子树驱动工具提示。 |
| `geometry` | String | 控件的矩形。schema 把可读与可写都声明为 `false`，因此它*不能*通过 `read_property`/`write_property` 访问；放置属于布局与 `Widget::set_geometry`。 |

一些控件还通过属性暴露枚举的字符串拼写，可接受 token 列在条目中。这些 token 与控件 setter 解析的拼写相同，因此从 schema 读出的 token 一定能写回。标为*只读*的属性可读，写入返回 `ReadOnlyProperty`；条目中不存在的属性根本没有发布，写入返回 `UnknownProperty`。

## 控件族一览

| 控件族 | 源码目录 | 控件数 | 用途 |
|---|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 | 窗口外壳与始终可用的控件 |
| `overlay` | `src/widget/overlay_widgets/` | 5 | 浮动动作、通知与生命周期浮层 |
| `nav` | `src/widget/nav_widgets/` | 7 | 顶层导航与页面容器 |
| `input` | `src/widget/input_widgets/` | 29 | 文本、选择与数值输入 |
| `display` | `src/widget/display_widgets/` | 27 | 只读指示器与小型显示标记 |
| `container` | `src/widget/container_widgets/` | 27 | 放置：滚动、分页、分割与布局盒 |
| `view` | `src/widget/view_widgets/` | 15 | 模型驱动的列表、树、表格与检查器 |
| `chart` | `src/widget/chart_widgets/` | 5 | 折线、柱状、饼、迷你与仪表可视化 |
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 | 菜单、工具栏、状态栏与命令项 |
| `dialog` | `src/widget/dialog/` | 17 | 模态决策与临时面板 |
| `special` | `src/widget/special_widgets/` | 39 | 完整应用界面及其配套控件 |
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 | 结构化文本编辑器及其导航外壳 |
| `finance` | `src/widget/chart_widgets/finance/` | 6 | 行情：蜡烛、深度、阶梯与指标 |
| `advanced` | `src/widget/advanced_widgets/` | 10 | 日期时间、旋钮、标签、功能区、饼菜单与快捷键 |
| `misc` | `src/widget/misc_widgets/` | 9 | 工具与移动控件 |
| `media` | `src/widget/media_widgets/` | 7 | 播放、动画与相机预览 |
| `web` | `src/widget/web_widgets/` | 1 | 嵌入式 Web 内容 |
| `cupertino` | `src/widget/cupertino/` | 8 | iOS 风格与 Material 自适应呈现 |
| `macos` | `src/widget/macos/` | 5 | 原生外观的 macOS 外壳 |

## 1. 基础控件与窗口外壳

基础族是始终可用的核心：窗口及其分组外壳，以及几乎每张表单都会用到的四个控件。它们也是精简配置下无条件保留的控件类型，因此需要在最小构建上运行的宿主应优先用这一族搭建界面。

单个确定动作选 `Button`，独立的开关设置选 `CheckBox` 或 `ToggleButton`，互斥组内的一项选 `RadioButton`，只读文本选 `Label`。需要给一组控件可见边界时用 `Frame`、`GroupBox` 与 `Panel`。不要把 `Label` 当可编辑字段、把 `Button` 当导航容器，也不要把 `Frame` 当布局引擎——`Frame` 只画边框，不排列子控件。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `base` | `src/widget/base_widgets/` plus `src/widget/window.rs` | 8 |

### Window (`window`)

顶层窗口：标题、带关闭入口的标题栏，以及承载其他控件的表面。每个顶层界面用一个；次要或临时窗口用 `Dialog` 或 `PopupWindow`，因为它们自带模态与生命周期。

**属性.** 除四个共享属性外共 4 项：`title`: String · `title_bar_height`: UInt · `close_button_size`: UInt · `button_spacing`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`

**命令.** `set_title`, `close`

**Rust API.** `create_window(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("window", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `main_window`

### GroupBox (`panel`)

`Panel` 是 `GroupBox` 外壳的第二个工厂名：带标题、用于归组相关控件的框。完整构建中 `create_panel` 挂载 `panel` 名称，控件报告 `WidgetKind::GroupBox`；按 `GroupBox` 的方式使用即可，不要寻找 `Panel` 专属的 capability 行——它不存在。

**属性.** 除四个共享属性外共 4 项：`title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_panel(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("panel", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Frame (`frame`)

边框为一个子控件绘制边框、3D 阴影或分隔线。用于给布局已经放好的内容加视觉结构；不要用它排列或裁剪子控件——它只画外壳。

**属性.** 除四个共享属性外共 7 项：`frame_shape`: Enum (`no_frame` | `box` | `panel` | `styled_panel` | `hline` | `vline` | `win_panel`) · `frame_shadow`: Enum (`plain` | `raised` | `sunken`) · `line_width`: Float · `mid_line_width`: Float · `frame_width`: Float · `contents_margins`: String · `theme_bevel`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_frame_shape`, `set_frame_shadow`, `set_line_width`

**Rust API.** `create_frame(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("frame", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `frame_widget`

### Button (`button`)

按钮在激活时发出 `clicked`，并分别报告按下、释放与取消。每个按钮表达一个确定动作；需要菜单时设置 `menu_open` 并用 `MenuPanel` 配套控件，而不是自建弹层；也不要把它当导航容器。

**属性.** 除四个共享属性外共 17 项：`text`: String · `pressed`: Bool · `default`: Bool · `icon_source`: IconSpec · `icon_size`: UInt · `shortcut`: String · `auto_default`: Bool · `auto_exclusive`: Bool · `flat`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `drag_enabled`: Bool · `accepts_drops`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `menu_open`: Bool · `menu_item_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `clicked` · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String

**命令.** `click`

**Rust API.** `create_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `pushbutton`, `btn`

### Label (`label`)

不可编辑的文本面，支持对齐、换行、省略、缩进，以及带链接的可选富文本。用于说明、帮助文本与内联链接；用户需要修改文本时改用 `LineEdit`、`TextArea` 或 `RichEdit`。

**属性.** 除四个共享属性外共 10 项：`text`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `word_wrap`: Bool · `elide`: Enum (`none` | `left` | `middle` | `right`) · `margin`: UInt · `scaled_contents`: Bool · `indent`: UInt · `text_format`: Enum (`plain` | `rich`) · `open_external_links`: Bool · `link_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `link_activated`: String · `link_hovered`: String

**命令.** `set_text`, `set_alignment`

**Rust API.** `create_label(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("label", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `text_label`

### CheckBox (`check_box`)

带标签的方框，拥有独立的选中状态与可选的第三态 `partially_checked`。用于彼此独立的设置；多个选项中必须恰好选一个时用 `RadioButton`，并注意 `auto_exclusive` 可以让同级控件互斥而无需单选组。

**属性.** 除四个共享属性外共 8 项：`text`: String · `state`: Enum (`unchecked` | `partially_checked` | `checked`) · `checked`: Bool · `tristate_enabled`: Bool · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `state_changed`: String · `toggled`: Bool

**命令.** `set_checked`, `toggle`, `set_state`

**Rust API.** `create_check_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("check_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### RadioButton (`radio_button`)

圆形选择控件，与共享 `group_id` 的其他单选按钮互斥。用于数量少、需要始终可见的备选项；列表很长时用 `ComboBox`，选项彼此独立时用 `CheckBox`。

**属性.** 除四个共享属性外共 7 项：`text`: String · `checked`: Bool · `group_id`: String · `auto_exclusive`: Bool · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `checked_changed`: Bool · `selected`

**命令.** `set_checked`, `set_group_id`

**Rust API.** `create_radio_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("radio_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ToggleButton (`toggle_button`)

在选中与未选中之间锁存的按钮，可选带弹出箭头（`arrow_type`/`popup_mode`）与菜单。用于保持开启的模式，或拥有菜单的工具按钮；一次性动作用普通 `Button`，设置式开关行用 `Switch`。

**属性.** 除四个共享属性外共 14 项：`text`: String · `checked`: Bool · `auto_exclusive`: Bool · `auto_raise`: Bool · `group_id`: String · `auto_repeat`: Bool · `auto_repeat_delay`: UInt · `auto_repeat_interval`: UInt · `icon_source`: IconSpec · `arrow_type`: Enum (`none` | `down` | `up` | `left` | `right`) · `popup_mode`: Enum (`instant` | `menu_button` | `delayed`) · `menu_open`: Bool · `menu_item_count`: UInt (只读) · `state`: Enum (`normal` | `hover` | `pressed` | `checked` | `disabled`) (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `checked_changed`: Bool · `item_triggered`: UInt · `pressed` · `released` · `state_changed`: String · `toggled`: Bool

**命令.** `set_checked`, `toggle`, `set_text`

**Rust API.** `create_toggle_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("toggle_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `toggle`

## 2. 浮层与浮动控件

浮层控件位于页面内容之上，而不占用阅读顺序中的位置。它们覆盖主浮动动作、持久通知、手势关闭、下拉刷新与启动画面。

页面的唯一主操作选 `FAB`，需要用户确认的提示选 `Banner`，列表行通过明确滑动删除时选 `SwipeToDismiss`，可滚动主体上的下拉刷新选 `RefreshControl`，应用初始化期间用 `SplashScreen`。不要用浮层表达必须阻塞其余界面的决策——那是对话框的职责；瞬时的 `Toast` 或 `Snackbar` 场景也不要用 `Banner`。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `overlay` | `src/widget/overlay_widgets/` | 5 |

### Banner (`banner`)

整宽、持久的通知，带严重级别、最多若干动作，以及由用户关闭的状态。用于用户必须看到并确认的状况；瞬时信息用 `Snackbar` 或 `Toast`，必须应答后才能继续时用 `MessageBox`。

**属性.** 除四个共享属性外共 5 项：`text`: String · `severity`: Enum (`info` | `success` | `warning` | `error`) · `dismissible`: Bool · `dismissed`: Bool (只读) · `action_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_clicked`: String · `dismissed`

**命令.** `dismiss`, `show`, `set_actions`, `activate_action`

**Rust API.** `create_banner(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("banner", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `notice`

### FAB (`fab`)

圆形、带高度的主操作按钮，浮在内容之上。每页一个，表达主操作；如果主操作会展开成多个，配 `SpeedDial` 配套控件，而不是堆多个 FAB；不要用于破坏性或极少使用的命令。

**属性.** 除四个共享属性外共 2 项：`icon`: String · `icon_source`: IconSpec — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`

**命令.** `set_icon`

**Rust API.** `create_fab(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("fab", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `floating_action_button`, `action_button`

### RefreshControl (`refresh_control`)

下拉刷新表面：用户把内容下拉超过阈值，控件发出 `refresh_triggered`；工作期间宿主设置 `refreshing`。用于触屏设备上的可滚动主体；桌面端优先提供显式刷新命令。

**属性.** 除四个共享属性外共 1 项：`refreshing`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `refresh_triggered`

**命令.** `set_refreshing`

**Rust API.** `create_refresh_control(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("refresh_control", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `pull_to_refresh`

### SplashScreen (`splash_screen`)

启动画面：标志/标题、可选副标题与进度，由程序在初始化完成时关闭，而不是由用户关闭。仅在应用启动期间使用；窗口已经可见后由用户发起的工作改用 `ProgressDialog`。

**属性.** 除四个共享属性外共 4 项：`title`: String · `subtitle`: String · `progress`: Float · `skippable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `fade_out_finished` · `finished`: String · `skipped`: String

**命令.** `finish`, `set_progress`, `set_title`

**Rust API.** `create_splash_screen(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("splash_screen", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `splash`

### SwipeToDismiss (`swipe_to_dismiss`)

手势容器：子内容被滑动时露出动作背景，超过阈值后发出 `dismissed`。用于以滑动为主要删除手势的列表行；为键盘与桌面用户保留可见的替代操作；背景需要多个动作时配 `SwipeActions`。

**属性.** 除四个共享属性外共 1 项：`is_dismissed`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** 未发布命令。

**Rust API.** `create_swipe_to_dismiss(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("swipe_to_dismiss", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `swipe_dismiss`

## 3. 导航与页面容器

导航控件表达“用户在哪里”和“能去哪里”：顶部应用栏、底部标签栏、侧滑抽屉、压栈/出栈页面栈、分页标签视图、分页索引，以及把它们组合起来的脚手架。

标题加前后置操作用 `AppBar`，手机上三到五个顶层目的地用 `BottomNavigationBar`，目的地更多时用 `NavigationDrawer`，逐层下钻流程用 `NavigationStack`，并列页面用 `TabView`，编号分页用 `Pagination`。不要用导航控件承载任意内容——`AdaptiveScaffold` 负责组合，页面仍由容器族的 `TabWidget` 持有。也不要把 `Pagination` 当页面容器：它只是索引。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `nav` | `src/widget/nav_widgets/` | 7 |

### AdaptiveScaffold (`adaptive_scaffold`)

把 `AppBar`、内容区与可选底部导航栏组合成一个自适应框架的脚手架。用于平板与移动端的顶层页面外壳；桌面端应显式放置导航外壳，让窗口自身的菜单栏仍是主命令面。

**属性.** 除四个共享属性外共 5 项：`title`: String · `badge_style`: Enum (`text` | `dot`) · `drawer_animated`: Bool · `scrim_opacity`: Float · `page_transition`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `nav_selected`: UInt

**命令.** `set_title`

**Rust API.** `create_adaptive_scaffold(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("adaptive_scaffold", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `scaffold`

### AppBar (`app_bar`)

顶部栏：标题、前置返回入口与后置动作；两个文本入口按阅读方向（`direction`）放置，而不是固定左右。用于页面标题加一两个动作；动作更多时收进溢出菜单，桌面端用 `ToolBar`。

**属性.** 除四个共享属性外共 2 项：`title`: String · `direction`: Enum (`ltr` | `rtl`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_index_pressed`: UInt · `action_overflow_pressed` · `action_pressed` · `back_pressed`

**命令.** `set_title`

**Rust API.** `create_app_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("app_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `top_app_bar`

### BottomNavigationBar (`bottom_navigation_bar`)

底部一行三到五个顶层目的地，带选中状态。用于手机宽度的顶层导航；目的地更多时用 `NavigationDrawer`；页面是同一屏幕内的并列内容而非应用级目的地时用 `TabView`。

**属性.** 除四个共享属性外共 2 项：`item_count`: UInt (只读) · `selected_index`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_bottom_navigation_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("bottom_navigation_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `bottom_nav`, `bottomnav`

### NavigationDrawer (`navigation_drawer`)

侧滑面板，包含分区、标题与选中项，并带一个可关闭它的遮罩。用于较长的目的地列表或账户/设置入口；用 `DrawerSectionHeader` 配套控件给行分组；宽布局下不要让它常驻打开——那种场景应在布局里放侧栏。

**属性.** 除四个共享属性外共 3 项：`open`: Bool · `width`: Float · `section_separator_visible`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed` · `header_clicked` · `item_selected`: UInt · `opened`

**命令.** `set_open`, `set_width`

**Rust API.** `create_navigation_drawer(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("navigation_drawer", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `nav_drawer`, `drawer`

### NavigationStack (`navigation_stack`)

压栈/出栈页面栈：`push`、`pop`、`set_current_page`，`navigation_changed` 报告新的顶层页面。用于返回必须回到上一页的下钻流程；由程序而非用户选页、且不隐含历史时用 `StackedWidget`。

**属性.** 除四个共享属性外共 3 项：`page_count`: UInt (只读) · `current_page`: UInt · `back_visible`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_index_pressed`: UInt · `navigation_changed`: String

**命令.** `push`, `pop`, `set_current_page`

**Rust API.** `create_navigation_stack(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("navigation_stack", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `nav_stack`

### Pagination (`pagination`)

编号分页索引：页大小、相邻页数量、可选导航按钮与滚轮翻页。它不持有内容：页面由 `Carousel`、表格或列表持有，`Pagination` 只报告用户请求的页号。不要把它当页面容器本身。

**属性.** 除四个共享属性外共 8 项：`total`: UInt · `page_size`: UInt · `page`: UInt · `page_count`: UInt (只读) · `last_page`: UInt (只读) · `sibling_count`: UInt · `show_nav_buttons`: Bool · `wheel_paging_enabled`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`: UInt · `page_changed`: UInt · `page_hover_changed`: UInt or null

**命令.** `next_page`, `previous_page`, `set_page`, `set_total`

**Rust API.** `create_pagination(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("pagination", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `pager`, `page_numbers`

### TabView (`tab_view`)

标签条加内容槽，可选键盘导航，关闭/移动请求以事件报告。用于同一屏幕内的并列页面；每页是需要自身布局的任意子容器时用 `TabWidget`，应用级目的地用 `BottomNavigationBar`。

**属性.** 除四个共享属性外共 3 项：`tab_count`: UInt (只读) · `selected_index`: UInt · `keyboard_navigation_enabled`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tab_changed`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `set_selected_index`

**Rust API.** `create_tab_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tab_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `page_tab_view`

## 4. 文本与数值输入

输入族把按键、选择与拖拽转换为带类型的值。它覆盖单行字段、掩码输入、自动补全、多行编辑、虚拟键盘与数值选择器。

按*值的形状*选择：一行自由文本用 `LineEdit`，段落用 `TextArea` 或 `TextEdit`，有界数字用 `SpinBox` 或 `NumberPicker`，固定选项用 `ComboBox`/`Dropdown`，建议来自数据时用 `AutoCompleteEdit` 或 `Mention`，格式固定时用 `MaskedEdit`，层级路径用 `Cascader`。不要用 `LineEdit` 编辑多行文本，不要把 `ComboBox` 当可编辑自由文本字段（那是 `EditableComboBox`），也不要用 `ListBox` 承载需要虚拟化的数据量——`VirtualList` 属于视图族。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `input` | `src/widget/input_widgets/` | 29 |

### LineEdit (`line_edit`)

单行文本字段，支持选择、撤销/重做可用性、回显模式与校验装饰（`helper`、`error`、`counter`、`validation_state`）。用于一行自由文本；段落用 `TextArea`，固定格式用 `MaskedEdit`，数值用 `SpinBox`。

**属性.** 除四个共享属性外共 21 项：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (只读) · `over_limit`: Bool (只读) · `valid`: Bool (只读) · `validation_message`: String (只读) · `validation_state`: String (只读) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_line_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("line_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `text_input`, `input`

### LineEdit (`password_edit`)

`LineEdit` 的密码角色：`echo_mode` 为 `password`，`password_echo_on_edit` 控制输入时的短暂显示。用于机密；不要记录其值；`phone_input` 与 `email_input` 只用于各自的取值形状。

**属性.** 除四个共享属性外共 21 项：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (只读) · `over_limit`: Bool (只读) · `valid`: Bool (只读) · `validation_message`: String (只读) · `validation_state`: String (只读) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_password_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("password_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `password_input`, `password_line_edit`

### LineEdit (`phone_input`)

`LineEdit` 的电话角色：构造函数安装电话校验器，拒绝空白与非电话字符，`valid` 报告值是否为 7 到 15 位数字。用于电话号码；它会校验，但不会替你做显示格式化。

**属性.** 除四个共享属性外共 21 项：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (只读) · `over_limit`: Bool (只读) · `valid`: Bool (只读) · `validation_message`: String (只读) · `validation_state`: String (只读) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_phone_input(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("phone_input", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `phone_edit`, `telephone_input`

### LineEdit (`email_input`)

`LineEdit` 的电子邮件角色：构造函数安装邮件校验器，拒绝空白，`valid` 报告值是否为 `local@domain.tld`。用于地址；它校验形状，但无效值如何处理仍由你决定。

**属性.** 除四个共享属性外共 21 项：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `drag_enabled`: Bool · `echo_mode`: Enum (`normal` | `password` | `no_echo`) · `cursor_position`: UInt · `prefix`: String · `suffix`: String · `helper`: String · `error`: String · `counter`: String (只读) · `over_limit`: Bool (只读) · `valid`: Bool (只读) · `validation_message`: String (只读) · `validation_state`: String (只读) · `alignment`: Enum (`left` | `centre` | `right`) · `modified`: Bool · `clear_button_enabled`: Bool · `password_echo_on_edit`: Bool · `scroll_x`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `copy_available`: Bool · `cursor_position_changed`: UInt · `editing_finished` · `redo_available`: Bool · `return_pressed` · `selection_changed` · `text_changed`: String · `text_edited`: String · `undo_available`: Bool

**命令.** `clear`, `select_all`

**Rust API.** `create_email_input(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("email_input", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `email_edit`, `email_field`

### ListBox (`list_box`)

可滚动条目列表，支持单选、多选或扩展选择，并有当前行。用于规模适中、已完全物化、每行是纯文本的列表；行需要模型、委托或虚拟化时改用 `ListView` 或 `VirtualList`。

**属性.** 除四个共享属性外共 9 项：`item_count`: UInt (只读) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `current_row`: UInt · `item_height`: Float · `selected_count`: UInt (只读) · `selected_indices`: String (只读) · `hovered_row`: UInt (只读) · `font_size`: Float (只读) · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `item_activated`: UInt · `item_changed`: UInt · `item_selected`: UInt · `rename_requested`: UInt · `selection_changed`

**命令.** `add_item`, `remove_item`, `clear`, `clear_selection`, `set_selection_mode`

**Rust API.** `create_list_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("list_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### SpinBox (`spin_box`)

带步进按钮、范围、小数位、可选前后缀与特殊值的数值字段。用于有界数字；滚轮式选择用 `NumberPicker`，角度式操作用 `Dial`，非数值输入用 `LineEdit`。

**属性.** 除四个共享属性外共 14 项：`minimum`: Number · `maximum`: Number · `value`: Number · `single_step`: Number · `decimals`: UInt · `prefix`: String · `suffix`: String · `display_text`: String (只读) · `value_text`: String (只读) · `special_value_text`: String · `wrapping`: Bool · `keyboard_tracking`: Bool · `direction`: Enum (`ltr` | `rtl`) · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `validation_failed`: String · `value_changed`: Int

**命令.** `set_range`, `set_value`, `step_up`, `step_down`

**Rust API.** `create_spin_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("spin_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ComboBox (`combo_box`)

带下拉列表、可选可编辑文本与可见弹层状态的条目字段。用于固定选项集合与单个当前值；用户可以输入列表外的值时用 `EditableComboBox`；选项是命令菜单而不是取值时用 `DropdownMenu`。

**属性.** 除四个共享属性外共 13 项：`item_count`: UInt (只读) · `insert_policy`: Enum (`no_insert` | `insert_at_top` | `insert_at_current` | `insert_at_bottom` | `insert_alphabetically`) · `size_adjust_policy`: Enum (`adjust_to_minimum_contents_length` | `adjust_to_contents` | `adjust_to_contents_on_first_show`) · `items`: StringList · `current_index`: UInt · `current_text`: String · `placeholder_text`: String · `minimum_contents_length`: UInt · `editable`: Bool · `max_visible_items`: UInt · `open`: Bool · `direction`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `current_index_changed`: UInt or null · `current_text_changed`: String · `highlighted`: UInt · `popup_visibility_changed`: Bool · `text_edited`: String

**命令.** `set_items`, `set_current_index`, `clear`

**Rust API.** `create_combo_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("combo_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Dropdown (`dropdown`)

独立下拉选择器，带占位文本、展开状态与可见条目数。字段只是选择器、不承载可编辑文本时用；控件同时拥有文本值时用 `ComboBox`，弹层是命令菜单时用 `MenuButton`。

**属性.** 除四个共享属性外共 8 项：`text`: String (只读) · `placeholder`: String · `selected_index`: UInt · `item_count`: UInt (只读) · `expanded`: Bool · `max_visible`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `changed` · `highlighted`: UInt

**命令.** `set_items`, `set_selected_index`, `set_expanded`, `toggle`

**Rust API.** `create_dropdown(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dropdown", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `dropdown_widget`, `combo`

### TextArea (`text_area`)

多行纯文本输入，支持软换行、内容边距、块数上限与横向滚动。用于段落与自由笔记；需要带边框、按行换行的编辑器控件时用 `TextEdit`，结构化源码用 `CodeEditor`。

**属性.** 除四个共享属性外共 8 项：`text`: String · `placeholder`: String · `read_only`: Bool · `max_length`: UInt · `soft_wrap`: Bool · `content_margin`: UInt · `max_block_count`: UInt · `scroll_x`: Int (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `context_menu_requested`: String · `cursor_position_changed`: UInt · `redo_available`: Bool · `selection_changed` · `undo_available`: Bool

**命令.** `set_text`, `set_placeholder`, `set_read_only`, `insert`, `delete_char`

**Rust API.** `create_textarea(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("text_area", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `multiline_edit`

### TextEdit (`text_edit`)

带边框的多行编辑器，支持选择文本、光标位置与换行开关。用于需要编辑器外壳的通用多行字段；需要格式属性时用 `RichEdit`，带提示符的追加式输出用 `TerminalView`。

**属性.** 除四个共享属性外共 6 项：`text`: String · `placeholder_text`: String · `max_length`: UInt · `read_only`: Bool · `line_wrap`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cursor_position_changed`: UInt · `selection_changed` · `text_changed`: String

**命令.** `set_text`, `set_placeholder_text`, `set_max_length`, `set_read_only`, `set_line_wrap`

**Rust API.** `create_text_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("text_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### CommandLink (`command_link`)

带标题、说明描述与可选盾牌标记的按钮——Windows 风格的命令链接。用于对话框里少量、需要解释的选项；一行标题就够时用普通 `Button`。

**属性.** 除四个共享属性外共 3 项：`text`: String · `description`: String · `shield`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `hovered`: Bool

**命令.** `set_text`, `set_description`, `set_enabled`, `click`

**Rust API.** `create_command_link(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("command_link", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### FontComboBox (`font_combo_box`)

以已安装字体填充、每项用自身字体渲染、并带过滤与示例文本的下拉框。用于 `FontDialog` 或设置表单里的字体选择；用 `FontPreview` 展示当前选择的更大字样。

**属性.** 除四个共享属性外共 9 项：`current_font_family`: String (只读) · `item_count`: Int (只读) · `current_index`: Int · `editable`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `max_visible_items`: Int · `font_filters`: String · `writing_system_filter`: String · `sample_text`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: Int · `current_font_changed`: String · `current_index_changed`: Int · `popup_hidden` · `popup_shown` · `text_edited`: String

**命令.** `set_current_index`, `set_editable`, `set_max_visible_items`, `show_popup`, `hide_popup`

**Rust API.** `create_font_combo_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("font_combo_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Keyboard (`keyboard`)

屏幕虚拟键盘，支持 qwerty 或数字布局、大小写状态与按键事件；`text_committed` 携带要插入的字符。用于没有硬件键盘的触屏设备；同时保留硬件输入路径，桌面端不要展示它以免占用屏幕。

**属性.** 除四个共享属性外共 3 项：`layout`: Enum (`qwerty` | `numeric`) · `lowercase`: Bool · `press_animation`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `backspace_pressed` · `enter_pressed` · `key_pressed`: UInt ×2 · `space_pressed` · `text_committed`: String

**命令.** `set_layout`, `set_lowercase`, `toggle_shift`

**Rust API.** `create_keyboard(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("keyboard", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `keyboard_widget`, `virtual_keyboard`

### Mention (`mention`)

在光标前补全 token、且一个字段可包含多个提及的文本字段。用于消息或评论里的 `@` 提及；整段字段按单个建议补全时用 `AutoCompleteEdit`。

**属性.** 除四个共享属性外共 9 项：`text`: String · `trigger`: String · `candidate_count`: UInt (只读) · `visible_candidate_count`: UInt (只读) · `mention_count`: UInt (只读) · `popup_open`: Bool (只读) · `caret`: UInt (只读) · `max_visible`: UInt · `row_height`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `mention_inserted`: String · `popup_toggled`: Bool · `text_changed`: String

**命令.** `set_candidates`, `set_trigger`, `complete`, `open_popup`

**Rust API.** `create_mention(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mention", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `mention_edit`, `at_mention`

### Cascader (`cascader`)

多级选择器，沿选项树走一条路径并报告所选路径。用于每一级都有意义的层级分类；平面列表用 `ComboBox`，层级本身需要始终可见可浏览时用 `TreeView`。

**属性.** 除四个共享属性外共 8 项：`value`: String (只读) · `placeholder`: String · `depth`: UInt (只读) · `expanded`: Bool · `separator`: String · `filterable`: Bool · `filter`: String (只读) · `max_visible`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: list of UInt

**命令.** `set_options`, `set_selected_path`, `expand`, `collapse`

**Rust API.** `create_cascader(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cascader", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `cascade`, `multi_level_select`

### SearchBox (`search_box`)

单行搜索字段，前置搜索入口与后置清除按钮；`search_submitted` 携带查询。用于列表或表格上方的快速过滤；还需要取消动作与平台搜索样式时用 `SearchBar`。

**属性.** 除四个共享属性外共 3 项：`text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `search_submitted`: String · `text_changed`: String

**命令.** `set_text`, `set_placeholder`, `clear`

**Rust API.** `create_search_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("search_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `search`

### TagInput (`tag_input`)

按回车或逗号生成可移除标签的文本字段，支持多行模式。用于自由形式的短标签集合；集合固定、只改变选择状态时用 `Chip` 行控件；token 指向人而不是任意标签时用 `Mention`。

**属性.** 除四个共享属性外共 4 项：`tags`: String (只读) · `multiline`: Bool · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tags_changed`: list of String

**命令.** `add_tag`, `remove_tag`, `set_placeholder`

**Rust API.** `create_tag_input(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tag_input", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `tag_editor`, `chip_input`

### ImePreedit (`ime_preedit`)

在宿主给定位置绘制输入法预编辑文本与光标的浮层。仅在自定义文本面上实现 IME 组合时使用；已经接受 IME 输入的控件（`LineEdit`、`TextArea` 等）会自行放置预编辑文本。

**属性.** 除四个共享属性外共 2 项：`text`: String · `cursor_position`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_text`, `set_cursor_position`

**Rust API.** `create_ime_preedit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("ime_preedit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `preedit`

### InplaceEditor (`inplace_editor`)

就地开始与结束编辑的小型编辑器，供表格与树单元格使用。用于回车提交、Escape 放弃的单元格编辑；字段是表单常驻部分时用 `LineEdit`。

**属性.** 除四个共享属性外共 3 项：`text`: String · `editing`: Bool · `edit_triggers`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `edit_accepted`: String · `edit_canceled`

**命令.** `set_text`, `start_editing`, `finish_editing`

**Rust API.** `create_inplace_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("inplace_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `inline_editor`

### MaskedEdit (`masked_edit`)

由掩码驱动的格式化文本字段，拒绝不符合的输入，并用 `input_rejected` 报告出错位置。用于电话、日期、邮编与形状固定的标识；格式只是建议而非强制时用 `LineEdit` 加校验。

**属性.** 除四个共享属性外共 3 项：`text`: String · `mask`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `input_rejected`: UInt · `return_pressed` · `text_changed`: String

**命令.** `set_text`, `set_mask`

**Rust API.** `create_masked_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("masked_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `masked_input`

### AutoCompleteEdit (`auto_complete_edit`)

整段值与候选列表匹配、并提供下拉的文本字段，支持匹配模式、大小写敏感、内联补全与关闭策略。用于从建议中选择单个值；token 补全用 `Mention`，列表封闭时用 `ComboBox`。

**属性.** 除四个共享属性外共 15 项：`text`: String · `placeholder`: String · `suggestion_count`: UInt (只读) · `selected_index`: UInt · `selected_suggestion`: String (只读) · `dropdown_visible`: Bool (只读) · `max_visible`: UInt · `alignment`: Enum (`left` | `centre` | `right`) · `can_undo`: Bool (只读) · `can_redo`: Bool (只读) · `match_mode`: Enum (`contains` | `starts_with` | `ends_with` | `fixed_string` | `regexp`) · `case_sensitive`: Bool · `inline_completion`: Bool · `close_policy`: Enum (`always` | `never` | `on_commit`) · `completion_prefix`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `suggestion_selected`: String · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_auto_complete_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("auto_complete_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `autocomplete`

### MultiSelectComboBox (`multi_select_combo_box`)

列表中保留多个选择、并以选中集合报告的下拉框。用于紧凑多选字段；选项需要始终可见的独立行时用 `CheckListBox`；用户还可以输入未提供的值时用 `TagInput`。

**属性.** 除四个共享属性外共 5 项：`selected_count`: UInt (只读) · `selected_items`: String (只读) · `placeholder`: String · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: list of UInt

**命令.** `set_expanded`

**Rust API.** `create_multi_select_combo_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("multi_select_combo_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `multi_combo`

### RangeSlider (`range_slider`)

双把手滑块，在同一轨道上选择上下界，并以 `range_changed` 同时报告两者。用于两端在视觉上一起移动的粗略范围；边界必须精确键入时用两个 `SpinBox`，日期范围用 `DateRangePicker`。

**属性.** 除四个共享属性外共 11 项：`min_value`: Float · `step`: Float · `min_range`: Float · `tick_interval`: Float · `tick_labels`: Bool · `value_hint_enabled`: Bool · `max_value`: Float · `lower`: Float · `upper`: Float · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `range_changed`: Float ×2

**命令.** `set_lower`, `set_upper`, `set_range`

**Rust API.** `create_range_slider(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("range_slider", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `dual_slider`

### EditableComboBox (`editable_combo_box`)

字段可以保存列表外文本的下拉框；`item_selected` 报告选择，`text_changed` 报告键入。用于通常选择、偶尔输入的值；封闭集合用 `ComboBox`，需要边输入边匹配建议时用 `AutoCompleteEdit`。

**属性.** 除四个共享属性外共 6 项：`text`: String · `item_count`: UInt (只读) · `cursor_position`: UInt · `max_visible_items`: UInt · `scroll_offset`: UInt · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editing_finished` · `item_selected`: UInt · `return_pressed` · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_editable_combo_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("editable_combo_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `editable_combo`

### SearchBar (`search_bar`)

平台风格的搜索字段，带取消动作、`search_submitted` 与 `text_changed`。用于 iOS 风格界面里的搜索-取消流程；桌面端紧凑、无取消入口的字段用 `SearchBox`。

**属性.** 除四个共享属性外共 3 项：`text`: String · `placeholder`: String · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `search_submitted`: String · `text_changed`: String

**命令.** `set_text`, `set_placeholder`

**Rust API.** `create_search_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("search_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `search_field`

### ShortcutEditor (`shortcut_editor`)

按类别分组、可搜索的快捷键列表，支持过滤、空状态与撤销/重做可用性。用于快捷键参考/编辑面板；任务是录制一个新快捷键时用 `KeySequenceEdit`。

**属性.** 除四个共享属性外共 8 项：`filter_text`: String · `shortcut_count`: UInt (只读) · `visible_count`: UInt (只读) · `category_count`: UInt (只读) · `categories`: String (只读) · `empty`: Bool (只读) · `can_undo`: Bool (只读) · `can_redo`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `shortcut_changed`: mixed (String parts)

**命令.** `set_filter_text`

**Rust API.** `create_shortcut_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("shortcut_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `keyboard_shortcut_editor`

### OtpInput (`otp_input`)

分段验证码字段：每字符一格、自动前进，支持粘贴、退格与完成信号。用于一次性验证码与短 PIN；较长的格式化标识用 `MaskedEdit`，普通密码用 `LineEdit`。

**属性.** 除四个共享属性外共 8 项：`value`: String · `length`: UInt · `masked`: Bool · `read_only`: Bool · `digits_only`: Bool · `separator`: String · `focused_index`: UInt (只读) · `is_complete`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `completed`: String · `submitted`: String · `value_changed`: String

**命令.** `insert_char`, `backspace`, `paste`, `clear`

**Rust API.** `create_otp_input(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("otp_input", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `otp`

### NumberPicker (`number_picker`)

纵向滚动的数字轮，显示相邻值，通过拖拽或轻扫而非逐步点击改变。用于从短范围中选择有界数字；需要精确键入或逐步调整时用 `SpinBox`。

**属性.** 除四个共享属性外共 12 项：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wrap`: Bool · `suffix`: String · `prefix`: String · `decimals`: UInt · `special_value_text`: String · `row_count`: UInt (只读) · `selected_row`: UInt (只读) · `row_height`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `scroll_rows`, `set_value`, `set_range`, `set_step`

**Rust API.** `create_number_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("number_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `picker`

### RichEdit (`rich_edit`)

带字符与段落格式属性、只读与换行开关、光标/选择事件的多行编辑器。用于富格式散文；来源是 Markdown 时用 `MarkdownEditor`，纯文本用 `TextArea`。

**属性.** 除四个共享属性外共 5 项：`text`: String · `line_count`: UInt (只读) · `read_only`: Bool · `line_wrap`: Bool · `placeholder_text`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cursor_position_changed`: UInt · `read_only_changed`: Bool · `selection_changed`: (UInt, UInt) or null · `text_changed`: String

**命令.** `set_text`

**Rust API.** `create_rich_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("rich_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `rich_text_editor`

## 5. 显示与指示器

显示控件报告状态而不是接受输入：进度、数值、图标、状态标记、色块与小面积装饰面。其中一些也是输入组合的视觉半边（例如 `Slider` 配一个数值标签）。

用户只读不改时用显示控件：完成度用 `ProgressBar` 与 `ProgressCircle`，只有在直接操纵数值时才用 `Slider` 与 `ScrollBar`，仪表式读数用 `Arc`、`Meter`、`LCDNumber` 与 `Roller`，紧凑状态用 `Badge`、`Tag`、`Chip` 与 `Avatar`，分隔用 `Divider` 与 `Line`。不要让显示控件成为数据的唯一存储——设置显示值的同时仍要保存模型。需要精确键入的数值不要只用 `Slider`，应与 `SpinBox` 配对。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `display` | `src/widget/display_widgets/` | 27 |

### Slider (`slider`)

沿轨道拖动把手，在范围内选择整数，支持刻度、翻页步进与跟踪。用于可直接操纵的粗略值；精确输入用 `SpinBox`，角度式仪表用 `Dial`，双边界用 `RangeSlider`。

**属性.** 除四个共享属性外共 10 项：`minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `tick_position`: Enum (`none` | `above` | `below` | `both`) · `tick_interval`: Int · `tracking`: Bool · `slider_position`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_slider_position`

**Rust API.** `create_slider(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("slider", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ProgressBar (`progress_bar`)

有界填充指示器，可选文本标签、方向与不确定模式。用于已知总量中的已知比例；紧凑圆环用 `ProgressCircle`，时长未知、只表达活动时用 `Spinner`。

**属性.** 除四个共享属性外共 9 项：`minimum`: Int · `maximum`: Int · `value`: Int · `text_visible`: Bool · `orientation`: Enum (`horizontal` | `vertical`) · `inverted_appearance`: Bool · `indeterminate`: Bool · `direction`: Enum (`ltr` | `rtl`) · `progress`: Float (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_orientation`

**Rust API.** `create_progress_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("progress_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ScrollBar (`scroll_bar`)

轨道上的滑块，用于滚动视口，支持空闲淡出、翻页步进与滑块比例。用于自定义可滚动表面旁边；希望 crate 拥有视口并自动放置滚动条时用 `ScrollArea`。

**属性.** 除四个共享属性外共 11 项：`idle_delay_ms`: UInt · `idle_opacity`: Float · `minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `orientation`: Enum (`horizontal` | `vertical`) · `direction`: Enum (`ltr` | `rtl`) · `slider_size`: Float (只读) · `slider_position`: Float (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_moved`: Int · `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_steps`, `set_orientation`

**Rust API.** `create_scroll_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("scroll_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### LCDNumber (`lcd_number`)

七段数码读数，可配置位数、进制与段样式。用于仪表式数字（计数器、时钟、测量值）；普通文本用 `Label`，数值还需要范围语境时用 `Meter`。

**属性.** 除四个共享属性外共 7 项：`value`: Float · `min_value`: Float · `max_value`: Float · `num_digits`: Int · `small_decimal_point`: Bool · `mode`: Enum (`hex` | `dec` | `oct` | `bin`) · `segment_style`: Enum (`outline` | `filled` | `flat`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `overflow` · `value_changed`: Float

**命令.** `set_value`, `set_mode`, `set_segment_style`

**Rust API.** `create_lcd_number(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("lcd_number", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Arc (`arc`)

环形进度或指示弧，支持扫过角度、粗细、可选圆角端点与居中数值。用于紧凑的单值表盘；Material 风格进度环用 `ProgressCircle`，需要刻度与指针时用 `Gauge`。

**属性.** 除四个共享属性外共 10 项：`value_format`: String · `value`: UInt · `minimum`: UInt · `maximum`: UInt · `thickness`: UInt · `sweep_angle`: UInt · `start_angle`: Float · `indeterminate`: Bool · `rounded`: Bool · `show_value`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_value`, `set_range`

**Rust API.** `create_arc(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("arc", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `arc_widget`

### Spinner (`spinner`)

旋转活动指示器，支持弧线或辐条样式、速度、粗细与尺寸比例。用于请求进行中且时长未知时；已知比例时用 `ProgressBar`，操作结束要停止，避免永久动画。

**属性.** 除四个共享属性外共 7 项：`active`: Bool · `track_color`: Color · `arc_color`: Color · `thickness`: UInt · `speed`: Float · `size_ratio`: Float · `style`: Enum (`arc` | `spokes`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `active_changed`: Bool

**命令.** `set_active`, `set_speed`, `set_thickness`, `set_size_ratio`

**Rust API.** `create_spinner(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("spinner", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `spinner_widget`, `loading_spinner`

### Roller (`roller`)

滚轮式选择器，显示当前选项及上下相邻项；拖拽或轻扫改变选择。用于触屏上的短小有序选项集；桌面用 `ComboBox`，数值用 `NumberPicker`。

**属性.** 除四个共享属性外共 4 项：`options`: StringList · `selected_index`: UInt · `visible_count`: UInt · `item_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_options`, `set_selected_index`, `set_visible_count`

**Rust API.** `create_roller(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("roller", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `roller_widget`, `scroll_wheel`

### Switch (`switch`)

开/关轨道与滑块控件，支持 Material、Cupertino 或 macOS 几何。用于立即生效的设置；选择属于稍后提交的表单时用 `CheckBox`，设计要求 iOS 呈现时用 `CupertinoSwitch`。

**属性.** 除四个共享属性外共 4 项：`checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_checked`, `toggle`

**Rust API.** `create_switch(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("switch", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `switch_widget`, `toggle_switch`

### Line (`line`)

水平或垂直线，带粗细与颜色，用作轻量分隔。自定义布局中需要一条线时用；需要平台分隔样式与方向属性时用 `Divider`。

**属性.** 除四个共享属性外共 3 项：`orientation`: Enum (`horizontal` | `vertical`) · `thickness`: UInt · `color`: Color — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_orientation`

**Rust API.** `create_line(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("line", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `line_widget`

### Meter (`meter`)

带范围、刻度、可选单位与阈值带的表计。用于有实际量程的测量值；完整径向表盘用 `Gauge`，紧凑圆环用 `Arc`，普通完成度用 `ProgressBar`。

**属性.** 除四个共享属性外共 8 项：`value`: UInt · `minimum`: UInt · `maximum`: UInt · `tick_count`: UInt · `show_tick_labels`: Bool · `unit`: String · `value_text`: String (只读) · `threshold_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed`

**命令.** `set_value`, `set_range`

**Rust API.** `create_meter(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("meter", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `meter_widget`

### EmojiPicker (`emoji_picker`)

字形选择器外壳：分类、搜索、最近使用与网格；字形表本身由调用方通过 `set_glyphs` 提供，因为 crate 不打包 emoji 数据。用于编辑器或反应选择器，并在 `glyph_chosen` 中插入字符。

**属性.** 除四个共享属性外共 7 项：`recent_capacity`: UInt · `glyph_count`: UInt (只读) · `visible_glyph_count`: UInt (只读) · `tab_count`: UInt (只读) · `active_tab`: UInt · `search`: String · `recent_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `glyph_chosen`: String · `glyph_hover_changed`: String

**命令.** `set_glyphs`, `set_categories`, `set_search`, `choose`

**Rust API.** `create_emoji_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("emoji_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `emoji`

### MiniChart (`mini_chart`)

无坐标轴的紧凑折线或柱状图，用于行、卡片或磁贴内。用于一眼可读的趋势；仅折线、带容量与末端标记的内联趋势用 `Sparkline`，需要坐标轴与交互时用图表族。

**属性.** 除四个共享属性外共 1 项：`chart_type`: Enum (`line` | `bar`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `point_hover_changed`: UInt or null

**命令.** `set_chart_type`, `set_data`, `set_range`

**Rust API.** `create_mini_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mini_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `mini_chart_widget`

### ImageView (`image_view`)

显示 `Image` 的控件，支持缩放、旋转、翻转、对齐、边距与缩放/平移偏移。用于单张静态图片；多图带缩略图用 `ImageGallery`，逐帧动画用 `AnimatedImage`，需要共享缩放与感兴趣区域状态时配 `ZoomPanController`。

**属性.** 除四个共享属性外共 6 项：`scaled`: Bool · `rotation`: Enum (`0` | `90` | `180` | `270`) · `flip_horizontal`: Bool · `flip_vertical`: Bool · `zoom_level`: Float · `pan_offset`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `double_clicked` · `image_changed` · `load_failed`: String

**命令.** `set_image`, `set_scaled`

**Rust API.** `create_image_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("image_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `image_viewer`

### MiniCanvas (`mini_canvas`)

小型即时模式绘图表面，提供填充/描边原语与清空。用于装饰性绘制或自定义小标记；需要图层、缩放与指针路由时用完整 `Canvas`；不要在没有绘制入口的控件里硬塞绘图逻辑。

**属性.** 该 schema 没有自己的属性；每个控件都带四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `mouse_pressed` · `mouse_released`

**命令.** `fill_rect`, `draw_rect`, `draw_line`, `fill_circle`, `clear`

**Rust API.** `create_mini_canvas(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mini_canvas", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Badge (`badge`)

小型浮层标记，承载计数、圆点或短标签，锚定在另一控件上。用于未读数与状态点；它是注解而不是按钮，可点击行为应放在被锚定的控件上。

**属性.** 除四个共享属性外共 4 项：`text`: String · `count`: Int · `dot_mode`: Bool · `level`: Enum (`info` | `success` | `warning` | `error`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `count_changed`: UInt · `level_changed`: String

**命令.** `set_text`, `set_count`

**Rust API.** `create_badge(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("badge", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `badge_widget`, `notification_badge`

### SkeletonLoader (`skeleton_loader`)

内容加载期间显示的灰色占位块，支持矩形、圆形与文本行形状。用于保持布局并表达进度；内容到达后不要继续显示；没有可预览形状时用 `Spinner`。

**属性.** 除四个共享属性外共 3 项：`active`: Bool · `text_line_rows`: UInt · `shape`: Enum (`rect` | `circle` | `text_line`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_active`

**Rust API.** `create_skeleton_loader(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("skeleton_loader", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `skeleton`, `shimmer`

### Divider (`divider`)

分隔相邻区块的细线，支持线型、方向、粗细与颜色。用于表单或列表中的组间分隔；自定义绘制里需要独立线条时用 `Line`。

**属性.** 除四个共享属性外共 4 项：`line_style`: Enum (`plain` | `sunken` | `raised`) · `orientation`: String · `thickness`: Float · `color`: Color — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_orientation`, `set_thickness`

**Rust API.** `create_divider(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("divider", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `divider_widget`, `separator`

### Rating (`rating`)

一行可选择的星形或图标，支持半步与数值。用于主观评分；连续量用 `Slider`，评分还需要精确键入时用 `SpinBox`。

**属性.** 除四个共享属性外共 6 项：`value`: Float · `allow_half_step`: Bool · `max`: UInt · `star_size`: UInt · `fill`: Float (只读) · `display_text`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `rating_changed`: UInt

**命令.** `set_value`, `set_max`

**Rust API.** `create_rating(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("rating", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `rating_widget`, `star_rating`

### EmptyState (`empty_state`)

视图无内容时显示的占位：消息、描述与可选次要动作。用于内容可能为空的列表、表格或页面；不要把它当加载指示器——那是 `SkeletonLoader` 或 `Spinner`。

**属性.** 除四个共享属性外共 3 项：`message`: String · `secondary_action_text`: String · `description`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_pressed`

**命令.** `set_message`, `set_description`

**Rust API.** `create_empty_state(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("empty_state", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `empty_placeholder`

### ColorHistory (`color_history`)

最近使用颜色的历史，以色块网格呈现，支持添加、删除、选择与悬停信号。用于颜色选择器旁，让用户回到最近的选择；固定调色板用 `Swatch`，单个当前颜色用 `ColorWell`。

**属性.** 除四个共享属性外共 1 项：`color_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `color_added`: Color · `color_hovered`: Color · `color_removed`: UInt · `color_selected`: Color · `context_menu_requested`: UInt

**命令.** 未发布命令。

**Rust API.** `create_color_history(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("color_history", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ColorWell (`color_well`)

紧凑色块，显示一个当前颜色并发出 `clicked` 与 `picker_requested`。用作颜色字段的折叠形态；收到 `picker_requested` 时打开 `ColorPicker` 或 `ColorDialog`，色块只持有当前值。

**属性.** 除四个共享属性外共 2 项：`color`: Color · `show_border`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `color_changed`: Color · `picker_requested`

**命令.** `set_color`

**Rust API.** `create_color_well(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("color_well", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### FloatingLabel (`floating_label`)

带浮动标签的文本字段，字段有内容或获得焦点时标签浮到输入上方。用于紧凑的 Material 风格表单字段；字段上方静态标签更合适时用 `LineEdit`；不要用于多行文本。

**属性.** 除四个共享属性外共 5 项：`text`: String · `label`: String · `placeholder`: String · `focused`: Bool · `floating_label_behavior`: Enum (`auto` | `always` | `never`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `text_changed`: String

**命令.** `set_text`, `set_placeholder`, `set_focused`

**Rust API.** `create_floating_label(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("floating_label", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `floating_input`

### FontPreview (`font_preview`)

字样面板，用选定的字族与字号渲染示例字符串。用于 `FontComboBox` 旁或 `FontDialog` 内，让用户以可读尺寸看到选择；它只是预览，不持有字体选择状态。

**属性.** 除四个共享属性外共 3 项：`font_family`: String · `font_size`: Float · `preview_text`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_font_family`, `set_font_size`, `set_preview_text`

**Rust API.** `create_font_preview(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("font_preview", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `font_viewer`

### ProgressCircle (`progress_circle`)

圆形进度指示器，支持粗细、颜色、直径、不确定模式与可选中心文本。用于紧凑完成度指示；表盘式扫过用 `Arc`，时长未知时用 `Spinner`。

**属性.** 除四个共享属性外共 7 项：`value`: Float · `thickness`: Float · `progress_color`: Color · `diameter`: UInt · `indeterminate`: Bool · `show_center_text`: Bool · `center_text_format`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `indeterminate_changed`: Bool · `value_changed`: Float

**命令.** `set_value`, `set_thickness`, `set_indeterminate`

**Rust API.** `create_progress_circle(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("progress_circle", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `circular_progress`

### Icon (`icon`)

按名称或来源描述、以指定尺寸与颜色渲染图标的控件。用于工具栏与空状态里的独立字形；字形属于标签或按钮时，设置该控件的 `icon_source`，不要额外叠一个 `Icon`。

**属性.** 除四个共享属性外共 4 项：`icon_name`: String · `icon_source`: IconSpec · `size`: Float · `color`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_icon_name`, `set_size`

**Rust API.** `create_icon(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("icon", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `icon_widget`, `icon_display`

### Tag (`tag`)

紧凑胶囊标签，用于分类或过滤内容，带色调、可选图标与关闭入口。用于单个只读分类标记；多条目可选/可重排的行用 `Chip`，用户创建标签时用 `TagInput`。

**属性.** 除四个共享属性外共 5 项：`text`: String · `icon`: String · `tone`: Enum (`neutral` | `primary` | `success` | `warning` | `error`) · `selected`: Bool · `closable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`: String · `selected_changed`: Bool

**命令.** 未发布命令。

**Rust API.** `create_tag(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tag", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `label_tag`

### Clock (`clock`)

模拟时钟面，显示一天中的时间，可选秒针，时间来源可以是手动值或系统时钟。用于时钟展示；选日期用 `Calendar`，编辑时间用 `TimePicker`。

**属性.** 除四个共享属性外共 5 项：`hour`: UInt · `minute`: UInt · `second`: UInt · `show_seconds`: Bool · `time_source`: Enum (`manual` | `system`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tick`: UInt

**命令.** 未发布命令。

**Rust API.** `create_clock(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("clock", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `analog_clock`

## 6. 容器与布局原语

容器持有子控件：排列、裁剪、分页、分割或给它们固定盒子。本族既包括传统的标签/分割/停靠外壳，也包括 Flutter 风格的单子布局盒（`Center`、`Padding`、`Expanded` 等）。

容器用来解决*放置*，而不只是装饰：内容可能超出视口时用 `ScrollArea`，用户可调整窗格时用 `Splitter`，多页共享一个槽位时用 `TabWidget`，由程序切页时用 `StackedWidget`，隐藏次要区块用 `Accordion`/`CollapsiblePane`，线性或换行排列用 `Flex`/`FlowLayout`/`WrapLayout`。不要用容器实现模型驱动的列表（用视图族），也不要把 `Frame` 当布局引擎。`Spacer` 确实什么都不画——这是它的契约，不是缺失实现。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `container` | `src/widget/container_widgets/` | 27 |

### GroupBox (`group_box`)

带标题的框架，用于归组相关控件，可选可勾选，以便用一个状态启用/禁用整组。用于表单中带标签的区块；没有标题时用普通 `Frame`，需要折叠时用 `Accordion`。

**属性.** 除四个共享属性外共 4 项：`title`: String · `alignment`: Enum (`left` | `centre` | `right` | `top` | `bottom`) · `checkable`: Bool · `checked`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_checkable`, `set_checked`, `toggle`

**Rust API.** `create_group_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("group_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Splitter (`splitter`)

可拖拽分隔条，把容器分成可调整大小的窗格，拖动时报告布局变化与实时预览。用于用户需要在两个区域间取舍空间；同一行为在布局族名称下是 `split_layout`；比例永不变化时用固定 `Flex`。

**属性.** 除四个共享属性外共 2 项：`orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**命令.** `set_orientation`

**Rust API.** `create_splitter(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("splitter", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `pane_splitter`

### Toolbox (`tool_box`)

纵向或横向的带标题区块堆叠，一次展开一个，支持互斥、非互斥或弹出模式。用于紧凑工具面板；页面内容区块用 `Accordion`，每个区块是完整页面时用 `TabWidget`。

**属性.** 除四个共享属性外共 4 项：`item_count`: UInt (只读) · `current_index`: UInt · `orientation`: Enum (`horizontal` | `vertical`) · `mode`: Enum (`exclusive` | `non_exclusive` | `popup`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt · `item_expanded_changed`: mixed (String parts)

**命令.** `add_item`, `remove_item`, `set_current_index`, `set_orientation`

**Rust API.** `create_tool_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tool_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ScrollArea (`scroll_area`)

可滚动视口，裁剪并偏移一个更大的子控件，水平与垂直滚动策略独立，并可选粘性区域。整个子树可能超出可见区域时使用；用 `StickyHeader` 配套控件保持条带可见；不要在同方向嵌套滚动区域。

**属性.** 除四个共享属性外共 8 项：`widget_resizable`: Bool · `horizontal_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `vertical_scroll_bar_policy`: Enum (`always_on` | `always_off` | `as_needed`) · `scroll_position_x`: Int · `scroll_position_y`: Int · `content_width`: UInt · `content_height`: UInt · `sticky_region_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `scroll_position_changed`: Int ×2

**命令.** `set_widget_resizable`, `set_horizontal_scroll_bar_policy`, `set_vertical_scroll_bar_policy`

**Rust API.** `create_scroll_area(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("scroll_area", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### TabWidget (`tab_widget`)

标签页容器，仅显示选中页，标签可关闭、可移动，位置与形状可配置。每页是需要自身布局的容器时使用；轻量并列页面用 `TabView`，由程序切页且不需要标签条时用 `StackedWidget`。

**属性.** 除四个共享属性外共 9 项：`tab_count`: UInt (只读) · `current_index`: UInt · `text`: String · `title`: String · `closable`: Bool · `movable`: Bool · `tab_position`: Enum (`north` | `south` | `west` | `east`) · `tab_shape`: Enum (`rounded` | `triangular` | `rectangular`) · `document_mode`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `current_changed`: UInt · `drag_finished`: String · `tab_bar_clicked`: UInt · `tab_bar_double_clicked`: UInt · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`, `set_tab_position`, `set_tab_shape`

**Rust API.** `create_tab_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tab_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### StackedWidget (`stacked_widget`)

一次显示一个子页面、由程序按索引选择的容器。用于由宿主决定页面的向导式或模式化内容；用户选页时用 `TabWidget`，存在返回历史时用 `NavigationStack`。

**属性.** 除四个共享属性外共 2 项：`widget_count`: UInt (只读) · `current_index`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `add_widget`, `remove_widget`, `set_current_index`

**Rust API.** `create_stacked_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("stacked_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `stacked`

### CollapsiblePane (`collapsible_pane`)

带可点击标题、用于展开或隐藏一个内容子控件的区块。用于单个可选详情块；多个此类区块需要协同用 `Accordion`，区块始终展开用 `GroupBox`。

**属性.** 除四个共享属性外共 2 项：`title`: String · `collapsed`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_title`, `set_collapsed`, `toggle`

**Rust API.** `create_collapsible_pane(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("collapsible_pane", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `collapsible`

### DockWidget (`dock_widget`)

一个可停靠面板，带标题，可浮动或停靠到边缘。用于用户可以重新排列的单个工具面板；多区域停靠系统不是这个控件，`MdiArea` 承载独立子窗口而不是停靠边缘。

**属性.** 除四个共享属性外共 3 项：`title`: String · `floating`: Bool · `docked`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dock_location_changed`: String · `features_changed`: String · `top_level_changed`: Bool

**命令.** `set_title`, `set_floating`, `set_features`, `set_allowed_areas`

**Rust API.** `create_dock_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dock_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `dock`

### MdiArea (`mdi_area`)

承载多个独立子窗口的工作区，支持子窗口或标签视图，并跟踪活动子窗口。用于以文档为中心的桌面应用；简单页面用 `TabWidget`，单一线性历史用 `NavigationStack`。

**属性.** 除四个共享属性外共 3 项：`subwindow_count`: UInt (只读) · `active_subwindow`: UInt (只读) · `view_mode`: Enum (`sub_window_view` | `tabbed`) (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `subwindow_activated`: UInt · `subwindow_title_changed`: mixed (String parts)

**命令.** `add_subwindow`, `remove_subwindow`, `set_view_mode`, `activate_subwindow`

**Rust API.** `create_mdi_area(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mdi_area", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `mdi`

### Stepper (`stepper`)

两侧带加减按钮、按固定步长调整数值的数字字段。用于小的整数数量（计数、购物车数量）；用户还可以键入数值时用 `SpinBox`，滚轮用 `NumberPicker`。

**属性.** 除四个共享属性外共 4 项：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** `set_value`, `set_minimum`, `set_maximum`, `set_step`

**Rust API.** `create_stepper(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("stepper", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `stepper_widget`, `step_control`

### MasonryLayout (`masonry_layout`)

Pinterest 风格瀑布流网格，把条目放入最短列，支持列数与间距。用于高度不定的卡片（照片、笔记）；行需要对齐时用 `FlowLayout`/`WrapLayout`，每个单元格同尺寸时用 `Grid`。

**属性.** 除四个共享属性外共 3 项：`column_count`: UInt · `item_count`: UInt (只读) · `item_spacing`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** `set_column_count`

**Rust API.** `create_masonry_layout(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("masonry_layout", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `waterfall_layout`

### SafeArea (`safe_area`)

按上、下、左、右内缩内容的容器，避免刘海、状态栏与 Home 指示条重叠。用于移动端屏幕根部；内缩值应来自平台查询，而不是硬编码设备度量。

**属性.** 除四个共享属性外共 4 项：`top_inset`: Float · `bottom_inset`: Float · `left_inset`: Float · `right_inset`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_top_inset`, `set_bottom_inset`, `set_left_inset`, `set_right_inset`

**Rust API.** `create_safe_area(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("safe_area", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `safe_area_insets`

### Carousel (`carousel`)

可横向滑动的页面轮播，带圆点、条或数字指示器，可选循环与自动播放。用于少量整宽页面；页索引需要放在轮播之外时配 `Pagination`；页面由标签而非滑动选择时用 `TabView`。

**属性.** 除四个共享属性外共 7 项：`current_index`: UInt · `item_count`: UInt (只读) · `current_page_title`: String (只读) · `loop`: Bool · `autoplay_interval`: UInt · `indicator_style`: Enum (`dots` | `bars` | `numeric` | `hidden`) · `indicator_position`: Enum (`bottom` | `top` | `left` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `page_changed`: UInt

**命令.** 未发布命令。

**Rust API.** `create_carousel(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("carousel", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `carousel_widget`, `swipe_view`

### Spacer (`spacer`)

在线性布局中消耗剩余空间的空白不可见区域，带弹性权重、基础尺寸与可选绘制提示。用于把同级控件推开；它什么都不画是契约，因此不要期待背景或边框。

**属性.** 除四个共享属性外共 4 项：`flex`: UInt · `base_size`: UInt · `paint_hint`: Bool · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_spacer(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("spacer", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `flexible_space`

### Center (`center`)

单子盒子，把内容居中，可选按父容器宽高的比例。用于居中表单或消息；需要中心以外的九个锚点之一时用 `Align`。

**属性.** 除四个共享属性外共 6 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `width_factor`: Float · `height_factor`: Float · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_center(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("center", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `centre`, `center_widget`

### Align (`align`)

单子盒子，把内容放在九个锚点之一。用于把子控件固定到角或边；中心情形用 `Center`；偏移是固定内缩而不是锚点时用 `Padding`。

**属性.** 除四个共享属性外共 5 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `alignment`: Enum (`top_left` | `top_centre` | `top_right` | `centre_left` | `centre` | `centre_right` | `bottom_left` | `bottom_centre` | `bottom_right`) · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_align(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("align", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `alignment_widget`

### SizedBox (`sized_box`)

具有固定宽高、包含一个子控件的盒子。用于预留精确槽位（`box_width`、`box_height`）；一个维度应随另一个变化时用 `AspectRatio`，尺寸应随内容变化时用 `IntrinsicWidth`。

**属性.** 除四个共享属性外共 6 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `box_width`: UInt · `box_height`: UInt · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_sized_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("sized_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `size_box`

### Padding (`padding`)

单子盒子，用四个独立边距内缩内容。用于属于内容而不是父容器的间距；同级之间的间隙用父容器的 `spacing`/`gap`。

**属性.** 除四个共享属性外共 8 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `padding_top`: UInt · `padding_right`: UInt · `padding_bottom`: UInt · `padding_left`: UInt · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_padding(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("padding", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `padding_widget`

### Expanded (`expanded`)

沿主轴占据剩余空间、按 `flex` 加权的子控件，必须放在线性布局内。用于让一个子控件吸收余量；子控件也可以停在首选尺寸而不填满时用 `Flexible`。

**属性.** 除四个共享属性外共 5 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_expanded(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("expanded", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `expand`

### Flexible (`flexible`)

可以收缩到下限、但最多增长到偏好尺寸的子控件，`fit` 选择宽松或紧致约束。用于同级控件应让出空间但不一定填满时；必须填满时用 `Expanded`。

**属性.** 除四个共享属性外共 6 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `flex`: UInt · `fit`: Enum (`loose` | `tight`) · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_flexible(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("flexible", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `flex_child`

### IntrinsicWidth (`intrinsic_width`)

宽度取内容自身首选宽度的盒子，使字段或标签不被拉伸。用于收缩适配的列；精确宽度用 `SizedBox`，高度需要随宽度变化时用 `AspectRatio`。

**属性.** 除四个共享属性外共 4 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_intrinsic_width(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("intrinsic_width", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `shrink_wrap`

### AspectRatio (`aspect_ratio`)

通过固定比例从一个维度推导另一个维度的盒子，可选服从父约束。用于媒体磁贴与缩略图；两个维度都已知时用 `SizedBox`，内容应决定宽度时用 `IntrinsicWidth`。

**属性.** 除四个共享属性外共 6 项：`content`: String · `content_width`: UInt · `content_height`: UInt · `ratio`: Float · `respect_parent`: Bool · `widget`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_aspect_ratio(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("aspect_ratio", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ratio_box`

### Flex (`flex`)

沿行或列按权重分配子控件的线性布局，支持换行、主轴对齐、交叉轴对齐与间隙。用于大多数一维布局；二维用 `Grid`，需要按测量尺寸换行时用 `FlowLayout`/`WrapLayout`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `direction`: Enum (`row` | `row_reverse` | `column` | `column_reverse`) · `wrap`: Enum (`no_wrap` | `wrap` | `wrap_reverse`) · `justify_content`: Enum (`flex_start` | `flex_end` | `center` | `space_between` | `space_around` | `space_evenly`) · `align_items`: Enum (`stretch` | `flex_start` | `flex_end` | `center` | `baseline`) · `gap`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未发布命令。

**Rust API.** `create_flex(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("flex", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `flex_box`

### FlowLayout (`flow_layout`)

按阅读顺序放置子控件、行满后开始新行的布局，每一行由子控件自身提示决定尺寸，并遵循书写方向。用于应自然流动的标签、chip 与工具按钮；行是固定尺寸单元格网格时用 `WrapLayout`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `padding`: UInt · `wrap`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未发布命令。

**Rust API.** `create_flow_layout(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("flow_layout", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `flow`

### WrapLayout (`wrap_layout`)

放置固定尺寸子控件、满后换到下一行的布局，支持独立间距与行间距。用于等大单元格的画廊；子控件应保持自身测量尺寸并遵循书写方向时用 `FlowLayout`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `direction`: Enum (`horizontal` | `vertical`) · `alignment`: Enum (`start` | `center` | `end` | `space_between` | `space_around`) · `spacing`: UInt · `run_spacing`: UInt · `padding`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_clicked`: UInt

**命令.** 未发布命令。

**Rust API.** `create_wrap_layout(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("wrap_layout", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `wrap`

### Accordion (`accordion`)

纵向可折叠区块堆叠，支持单展开或多展开模式。用于 FAQ 式或设置分组；单个独立区块用 `CollapsiblePane`，区块是工具面板而非页面内容时用 `ToolBox`。

**属性.** 除四个共享属性外共 3 项：`section_count`: UInt (只读) · `single_expand`: Bool · `expanded_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `section_toggled`: mixed (String parts)

**命令.** 未发布命令。

**Rust API.** `create_accordion(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("accordion", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `collapsible_list`

### Splitter (`split_layout`)

`Splitter` 的布局族拼写：窗格间的可拖拽分隔条，带方向与窗格数。分割属于布局描述而不是控件子树时使用；它报告与 `Splitter` 相同的三个布局信号。

**属性.** 除四个共享属性外共 2 项：`orientation`: Enum (`horizontal` | `vertical`) · `pane_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `orientation_changed`: String · `pane_layout_changed`: list of Float · `pane_layout_preview`: list of Float

**命令.** `set_orientation`

**Rust API.** `create_split_layout(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("split_layout", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `split_view`

## 7. 模型驱动的条目视图

视图呈现模型：行、列、树以及它们周围的外壳。它们负责滚动、选择、焦点，部分还负责编辑，并且只向宿主提供的数据源索取当前可见的行。

按模型的*形状*选择：平面列表用 `ListView` 或 `VirtualList`，树用 `TreeView`，网格用 `Table`、`GridTable` 或 `VirtualTable`，带列的树用 `TreeTable`。排序、筛选与冻结列是功能的一部分时用 `DataGrid`；数据本身是条件集合或命名属性集合时用 `QueryBuilder`/`PropertyGrid`/`PropertiesPanel`。不要用 `ListBox` 承载数千行，不要用 `Table` 表达层级数据，也不要把 `ListView` 当表单：本 crate 的视图不在行内承载任意控件。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `view` | `src/widget/view_widgets/` | 15 |

### ListView (`list_view`)

模型驱动的列表，支持视图模式（列表、图标、详情、缩略图）、焦点行与选择。用于携带模型或委托的行；小型纯文本列表用 `ListBox`，模型大到需要窗口化投影时用 `VirtualList`。

**属性.** 除四个共享属性外共 7 项：`has_model`: Bool (只读) · `row_count`: UInt (只读) · `focused_row`: UInt · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `view_mode`: Enum (`list` | `icon` | `details` | `thumbnails`) · `alignment`: Enum (`left` | `centre` | `right`) · `icon_visible_in_list_mode`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `focused_row_changed`: UInt or null · `row_activated`: UInt · `row_reordered`: UInt ×2 · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_list_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("list_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### TreeView (`tree_view`)

层级视图，可展开/折叠节点，带焦点节点、选中节点、懒加载与动画展开。用于树模型；每行还需要列时用 `TreeTable`；`ColumnView` 是同一树族的别名角色，而不是第二个控件。

**属性.** 除四个共享属性外共 10 项：`has_model`: Bool (只读) · `node_count`: UInt (只读) · `focused_node`: UInt · `selected_node`: UInt (只读) · `has_delegate`: Bool (只读) · `expanded_focused_node`: Bool (只读) · `expansion_animated`: Bool · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: UInt · `collapsed`: UInt · `expanded`: UInt · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `focused_node_changed`: UInt or null · `node_double_clicked`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_node`, `expand`, `collapse`, `toggle`

**Rust API.** `create_tree_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tree_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Table (`table_widget`)

单元格表格，带表头、当前行、选择模式与编辑请求信号。用于规模适中的平面网格；需要排序、筛选与冻结列时用 `DataGrid`，大模型用 `VirtualTable`。

**属性.** 除四个共享属性外共 8 项：`has_model`: Bool (只读) · `has_delegate`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("table_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Table (`table`)

同一表格控件在 `table_widget` 之外的 `table` 工厂名；两个名称接受相同的属性与事件契约。按宿主已有拼写选择其一，不要为同一个控件同时注册两个。

**属性.** 除四个共享属性外共 8 项：`has_model`: Bool (只读) · `has_delegate`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `selection_mode`: Enum (`none` | `single` | `multi` | `extended`) · `header_visible`: Bool · `editable`: Bool · `edit_triggers`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `focused_row_changed`: UInt or null · `row_activated`: UInt · `selection_changed`: UInt

**命令.** `clear_selection`, `clear_focused_row`

**Rust API.** `create_table(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("table", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Table (`data_grid`)

数据网格，带数据源、排序、筛选、冻结列、行高列宽、选择行为与汇总。排序/筛选是功能一部分时使用；更轻的网格用 `GridTable`，行有层级时用 `TreeTable`。

**属性.** 除四个共享属性外共 18 项：`has_data_source`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `frozen_columns`: UInt · `sort_spec_count`: UInt (只读) · `filter_count`: UInt (只读) · `sort_specs`: String · `filters`: String · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) · `selection_behavior`: Enum (`cells` | `rows` | `columns`) · `sort_indicator_visible`: Bool · `section_resize_mode`: Enum (`fixed` | `stretch` | `interactive`) · `current_cell`: String (只读) · `summary_text`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `edit_requested`: UInt ×2 · `filter_changed`: String · `header_clicked`: UInt · `page_applied`: UInt · `sort_changed`: mixed (String parts) · `visible_window_changed`: UInt ×4

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_frozen_columns`

**Rust API.** `create_data_grid(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("data_grid", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### TreeTable (`tree_table`)

层级表格：行构成可展开/折叠的树，每行还带列单元格，支持排序角色、大小写敏感与窗口化/即时投影。用于节点还带表格属性的树；纯树用 `TreeView`，平面行用 `Table`。

**属性.** 除四个共享属性外共 15 项：`has_model`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `selected_row`: UInt · `row_height`: UInt · `column_width`: UInt · `header_visible`: Bool · `expands_on_double_click`: Bool · `middle_click_expands`: Bool · `sort_role`: Enum (`text` | `number`) · `case_sensitive`: Bool · `projection_strategy`: Enum (`windowed` | `eager`) · `lazy_loading`: Bool · `loading_indicator_visible`: Bool · `fetch_timeout_ms`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `expansion_changed`: mixed (String parts) · `fetch_finished`: mixed (String parts) · `fetch_started`: list of UInt · `projection_changed`: UInt · `selection_changed`: UInt or null · `sort_changed`: mixed (String parts)

**命令.** `set_model`, `clear_model`, `expand_row`, `collapse_row`, `select_row`

**Rust API.** `create_tree_table(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tree_table", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Table (`virtual_table`)

基于数据源、只投影可见窗口的表格，支持行列过扫描与滚轮滚动模式。用于大型远程数据表；需要排序/筛选且模型在本地时用 `DataGrid`。

**属性.** 除四个共享属性外共 12 项：`has_data_source`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `column_width`: UInt · `overscan_rows`: UInt · `overscan_columns`: UInt · `visible_window`: String (只读) · `wheel_scroll_mode`: Enum (`per_row` | `per_page`) · `header_visible`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: (UInt, UInt) or null · `visible_window_changed`: UInt ×4

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `set_row_height`, `set_column_width`, `set_overscan_rows`, `set_overscan_columns`, `fetch_visible_window`

**Rust API.** `create_virtual_table(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("virtual_table", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### DataView (`virtual_list`)

基于数据源的单列虚拟列表，支持可见窗口、过扫描与多选。用于等高空行的长列表；行需要视图模式或委托时用 `ListView`，短小已物化列表用 `ListBox`。

**属性.** 除四个共享属性外共 8 项：`has_data_source`: Bool (只读) · `row_count`: UInt (只读) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (只读) · `selected_rows`: NumberList (只读) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**命令.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_virtual_list(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("virtual_list", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### DataView (`data_view`)

`VirtualList` 的 `data_view` 拼写；它呈现一列模型行，不切换显示模式。宿主希望以通用 data-view 名称使用虚拟列表契约时用它，不要期待第二套实现。

**属性.** 除四个共享属性外共 8 项：`has_data_source`: Bool (只读) · `row_count`: UInt (只读) · `scroll_row`: UInt · `row_height`: UInt · `overscan`: UInt · `selected_row`: UInt (只读) · `selected_rows`: NumberList (只读) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drag_finished`: String · `selected_rows_changed`: list of UInt · `selection_changed`: UInt or null · `visible_window_changed`: UInt ×2

**命令.** `clear_data_source`, `fetch_visible_rows`

**Rust API.** `create_data_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("data_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### GridTable (`grid_table`)

功能丰富的虚拟化表格，带网格线、表头、排序、选择与数据源。用于表单内的电子表格式表面；需要筛选/冻结列时用 `DataGrid`，表头需要在网格外共享时配 `GridViewHeader`。

**属性.** 除四个共享属性外共 12 项：`has_data_source`: Bool (只读) · `row_count`: UInt (只读) · `column_count`: UInt (只读) · `scroll_row`: UInt · `scroll_column`: UInt · `row_height`: UInt · `selection_mode`: Enum (`none` | `cell` | `row` | `column`) · `sort_spec_count`: UInt (只读) · `selected_cell`: String (只读) · `sorting_enabled`: Bool · `sort_role`: String · `auto_refresh`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_double_clicked`: UInt ×2 · `cell_selected`: UInt ×2 · `context_menu_requested`: String · `edit_requested`: UInt ×2 · `header_clicked`: UInt · `sort_changed`: mixed (String parts)

**命令.** `set_data_source`, `clear_data_source`, `set_scroll_row`, `set_scroll_column`, `toggle_sort_column`, `clear_selection`

**Rust API.** `create_grid_table(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("grid_table", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### QueryBuilder (`query_builder`)

把递归过滤条件编辑成“字段、运算符、操作数”行的构建器，带连接词、实时更新与提交延迟。用于需要可导出文本的结构化筛选；表格旁的快速一行筛选用 `FilterRow`，自由文本搜索用 `SearchBox`。

**属性.** 除四个共享属性外共 8 项：`row_count`: UInt (只读) · `incomplete_row_count`: UInt (只读) · `field_count`: UInt (只读) · `conjunction`: Enum (`and` | `or`) · `active_row`: UInt · `live_update`: Bool · `commit_delay_ms`: UInt · `has_pending_commit`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `query_changed`: String · `query_committed`: String · `row_field_clicked`: UInt · `row_operator_clicked`: UInt · `rows_changed`: list of Records

**命令.** `set_fields`, `add_row`, `remove_row`, `toggle_conjunction`

**Rust API.** `create_query_builder(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("query_builder", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `filter_builder`

### PropertiesPanel (`properties_panel`)

分类属性编辑器，以网格布局排列行，并报告编辑、文件选择请求与编辑器打开。用于画布旁的检查器；较扁平的两列表单用 `PropertyGrid`，分类重要时用 `PropertiesPanel`。

**属性.** 除四个共享属性外共 3 项：`property_count`: UInt (只读) · `description_mode`: Enum (`tooltip` | `status_bar` | `hidden`) · `edit_trigger`: Enum (`single_click` | `double_click`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `editor_opened`: String · `file_picker_requested`: String · `property_changed`: String ×2

**命令.** 未发布命令。

**Rust API.** `create_properties_panel(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("properties_panel", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `property_panel`

### PropertyGrid (`property_grid`)

两列编辑器，列出命名属性与可编辑值单元格，带选中行与逐单元格变化信号。用于紧凑键/值检查器；行需要按类别分组时用 `PropertiesPanel`。

**属性.** 除四个共享属性外共 2 项：`property_count`: UInt (只读) · `selected_index`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_changed`: mixed (String parts) · `context_menu_requested`: String · `selected`: UInt

**命令.** `add_property`, `set_value`, `clear`, `set_selected_index`

**Rust API.** `create_property_grid(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("property_grid", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `inspector`

### ImageGallery (`image_gallery`)

图片浏览器，带主图、缩略图条、滚轮模式与全屏请求。用于需要显示当前索引的照片集合；单张图片用 `ImageView`，缩略图条很大时配 `ThumbnailCache`。

**属性.** 除四个共享属性外共 7 项：`current_index`: UInt · `thumbnail_size`: UInt · `show_thumbnails`: Bool · `strip_offset`: UInt · `strip_scroll_enabled`: Bool · `wheel_mode`: Enum (`strip` | `navigate` | `zoom`) · `arrow_key_navigation_enabled`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `fullscreen_requested`: UInt · `image_changed`: UInt · `selection_changed`: list of UInt · `strip_scrolled`: UInt

**命令.** `set_current_index`

**Rust API.** `create_image_gallery(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("image_gallery", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `gallery`

### GridViewHeader (`grid_view_header`)

网格或表格的共享表头条：可排序、可调整的区段与筛选入口。表头需要位于滚动主体之外时使用，并把 `section_clicked`、`section_resized`、`filter_clicked` 报告给持有模型的网格。

**属性.** 除四个共享属性外共 6 项：`columns`: StringList · `column_count`: UInt (只读) · `sort_indicator_visible`: Bool · `resize_mode`: Enum (`fixed` | `stretch` | `stretch_last` | `interactive`) · `filter_icon_visible`: Bool · `sort_columns`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `filter_clicked`: UInt · `section_clicked`: UInt · `section_resized`: UInt ×2

**命令.** 未发布命令。

**Rust API.** `create_grid_view_header(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("grid_view_header", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `table_header_bar`

## 8. 图表与仪表

图表族是共享绘图引擎之上的控件层：折线、柱状、饼图、迷你趋势图，以及径向 `Gauge`。引擎负责坐标轴、刻度、边距与图例位置；控件负责模型、信号与类型语义。

有序序列用 `LineChart`，分类比较用 `BarChart`，少量扇区的占比快照用 `PieChart`，无轴的紧凑内联趋势用 `Sparkline`，单值对范围用 `Gauge`。不要用图表显示 OHLC 行情（金融族）、热力图或雷达图（特殊族），也不要为单个数字使用图表——`Label` 或 `LCDNumber` 更清楚。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `chart` | `src/widget/chart_widgets/` | 5 |

### LineChart (`line_chart`)

用于一个或多个有序序列的笛卡尔折线图，通过共享图表引擎绘制。用于有序轴上的趋势；趋势需要内联且无需坐标轴时用 `Sparkline`，技术指标用 `IndicatorChart`。

**属性.** 除四个共享属性外共 1 项：`stroke_width`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `point_clicked`: UInt ×2 · `point_hovered`: UInt ×2 · `point_unhovered`: UInt ×2

**命令.** `set_stroke_width`

**Rust API.** `create_line_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("line_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `line_graph`

### Sparkline (`sparkline`)

无坐标轴、网格与边距的紧凑内联趋势，由值列表构建，带末端标记与悬停报告。用于表格单元格或 KPI 磁贴；需要坐标轴或缩放时用 `LineChart`。它有意不接入图表引擎，因为它没有引擎要统一的轴、网格、刻度与边距。

**属性.** 除四个共享属性外共 7 项：`data`: NumberList · `stroke_width`: Float · `show_end_value`: Bool · `fill`: Bool · `show_points`: Bool · `show_last_marker`: Bool · `capacity`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `last_value_clicked`: Float · `value_hovered`: mixed (String parts)

**命令.** `set_stroke_width`

**Rust API.** `create_sparkline(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("sparkline", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `sparkline_chart`

### BarChart (`bar_chart`)

用于分类比较的纵向柱状图，支持柱间距与逐柱悬停/点击信号。用于少量分类；有序序列用 `LineChart`，占比用 `PieChart`，多轴轮廓用 `RadarChart`。

**属性.** 除四个共享属性外共 1 项：`bar_spacing`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt

**命令.** `set_bar_spacing`

**Rust API.** `create_bar_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("bar_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `bar_graph`

### PieChart (`pie_chart`)

带彩色扇区的圆形图，可选环形，报告指针下的扇区与被点击的扇区。用于少量扇区的占比快照；分类很多或数值接近时用 `BarChart`。

**属性.** 除四个共享属性外共 1 项：`donut`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slice_clicked`: UInt · `slice_hovered`: UInt · `slice_unhovered`: UInt

**命令.** `set_donut`

**Rust API.** `create_pie_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("pie_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `pie_graph`

### Gauge (`gauge`)

径向表盘，用范围、色带、刻度、指针样式与可选单位标签显示单个值。用于有实际量程的单个测量值；线性仪表用 `Meter`，完成度用 `Arc`/`ProgressCircle`。

**属性.** 除四个共享属性外共 14 项：`value`: Float · `minimum`: Float · `maximum`: Float · `label`: String · `unit`: String · `zone_count`: UInt (只读) · `animated`: Bool · `thickness`: UInt · `bands`: StringList · `tick_count`: UInt · `start_angle`: Float · `end_angle`: Float · `needle_style`: Enum (`none` | `line` | `triangle`) · `unit_label`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `band_clicked`: String · `value_changed`: Float

**命令.** 未发布命令。

**Rust API.** `create_gauge(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("gauge", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `dial_gauge`

## 9. 菜单、工具栏与命令面

本族承载命令外壳：菜单栏、下拉与上下文菜单、工具栏、状态栏、单个动作，以及打开菜单或拆成两个动作的按钮。

应用级命令分组用 `MenuBar`，由按钮或次要点击打开的命令列表用 `Menu` 或 `ContextMenu`，常用动作放 `ToolBar`，简短状态行用 `StatusBar`，需要“描述一次、多处渲染”的命令用 `Action`。不要把菜单当带选择状态的数据列表（那是视图），也不要把命令状态存进 `Menu`——把状态放在 `Action` 的注解和宿主模型里。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `menu_toolbar` | `src/widget/menu_toolbar/` | 10 |

### Action (`action`)

可由菜单与工具栏渲染的命令描述：文本、图标、快捷键、可勾选/选中状态、分隔符标记与命令 id。用于同一命令出现在多个命令面；不要把它单独当可见按钮——具体控件由命令面实例化。

**属性.** 除四个共享属性外共 8 项：`text`: String · `icon_text`: String · `shortcut`: String · `checkable`: Bool · `checked`: Bool · `icon_source`: IconSpec · `separator`: Bool (只读) · `command_id`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `hovered` · `toggled`: Bool · `triggered`: Bool

**命令.** `set_text`, `set_checkable`, `set_checked`, `trigger`

**Rust API.** `create_action(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("action", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `command_action`

### Menu (`menu`)

命令与子菜单的下拉列表，带悬停、动作触发、子菜单请求与显示/隐藏前信号。用于从菜单栏、按钮或上下文点击打开的命令；弹层选择值时用 `DropdownMenu`，按钮拥有菜单时用 `MenuButton`。

**属性.** 除四个共享属性外共 6 项：`title`: String · `item_count`: UInt (只读) · `hovered_index`: UInt (只读) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_menu(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("menu", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Menu (`context_menu`)

`ContextMenu` 是 `Menu` 的角色拼写（`pub type ContextMenu = Menu`），以自己的名称注册，用于在指针处打开的菜单。用于次要点击菜单；它不是第二套菜单实现，属性与事件就是 `Menu` 的集合。

**属性.** 除四个共享属性外共 6 项：`title`: String · `item_count`: UInt (只读) · `hovered_index`: UInt (只读) · `direction`: Enum (`ltr` | `rtl`) · `show_heading`: Bool · `column_count`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `action_triggered`: String · `submenu_requested`: String · `tearoff_requested` · `triggered`: String · `triggered_index`: UInt

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_context_menu(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("context_menu", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `context_actions`

### MenuBar (`menu_bar`)

顶层菜单标题的横向条，跟踪活动与悬停条目。用于窗口顶部的应用命令分组；常用命令用 `ToolBar`，页面内单个下拉用 `MenuButton`。

**属性.** 除四个共享属性外共 3 项：`entry_count`: UInt (只读) · `active_index`: UInt (只读) · `hovered_index`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_hovered`: String · `triggered`: String · `triggered_entry`: mixed (String parts)

**命令.** `clear`, `add_menu`, `remove_menu`

**Rust API.** `create_menu_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("menu_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ToolBar (`tool_bar`)

图标按钮与分隔条的条带，可横向或纵向、可移动、可浮动，并报告方向与可见性变化。用于常用命令；命令需要带标签的分组与标签页时用 `RibbonBar`，单个条目用 `ToolButton`。

**属性.** 除四个共享属性外共 5 项：`orientation`: Enum (`horizontal` | `vertical`) · `icon_size`: Float · `movable`: Bool · `floatable`: Bool · `item_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_triggered`: String · `customize_requested` · `float_changed`: Bool · `orientation_changed`: Bool · `top_level_changed`: Bool · `visibility_changed`: Bool

**命令.** `clear`, `add_action`, `add_separator`

**Rust API.** `create_tool_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tool_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### StatusBar (`status_bar`)

窗口底部的简短状态行，带消息与尺寸抓手。用于瞬时状态文本；`Snackbar` 也以这个类型注册，但它是浮动通知，不要混淆——这里设置 `message`，由宿主更新。

**属性.** 除四个共享属性外共 1 项：`message`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `message_changed`: String · `size_grip_dragged`: Int ×2

**命令.** `set_message`

**Rust API.** `create_status_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("status_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `status`

### ToolButton (`tool_button`)

用于工具栏的紧凑图标加标题按钮，带选中状态与点击/切换信号。用于 `ToolBar` 或 `SplitButton` 组内；标题是主要目标时用完整 `Button`，按钮拥有弹层时用 `ToggleButton`。

**属性.** 除四个共享属性外共 3 项：`text`: String · `checked`: Bool · `icon_source`: IconSpec — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked`: Bool · `toggled`: Bool · `triggered`

**命令.** `set_text`, `set_checked`

**Rust API.** `create_tool_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tool_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `toolbar_button`

### ToolButton (`split_button`)

拆成主操作与备选菜单的工具按钮，支持弹出方向、行高与滚动位置。用于一个默认操作、多个变体共享空间时；每次点击都打开菜单时用 `MenuButton`。

**属性.** 除四个共享属性外共 7 项：`text`: String · `action_count`: UInt (只读) · `menu_open`: Bool · `row_height`: UInt · `popup_direction`: Enum (`down` | `up` | `auto`) · `max_menu_height`: UInt · `menu_scroll`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_selected`: String · `menu_toggled`: Bool · `triggered`: String

**命令.** `add_action`, `open_menu`, `close_menu`, `trigger_primary`

**Rust API.** `create_split_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("split_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### DropdownMenu (`dropdown_menu`)

带条目列表、选中索引、展开状态与子菜单请求的下拉选择器。弹层从列表选值时使用；字段还需要在文本框中显示当前值时用 `ComboBox`，条目是命令时用 `Menu`。

**属性.** 除四个共享属性外共 4 项：`item_count`: UInt (只读) · `selected_index`: UInt · `expanded`: Bool · `alignment`: Enum (`left` | `centre` | `right`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `item_selected`: String · `item_toggled`: String · `submenu_requested`: String

**命令.** `set_selected_index`, `set_expanded`

**Rust API.** `create_dropdown_menu(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dropdown_menu", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### MenuButton (`menu_button`)

点击后打开菜单的按钮，带图标、条目数与显示/隐藏前及条目触发信号。用于页面内单个下拉命令面；主操作也必须可点击时用 `SplitButton`。

**属性.** 除四个共享属性外共 4 项：`text`: String · `icon_source`: IconSpec · `item_count`: UInt (只读) · `expanded`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `clicked` · `item_triggered`: UInt · `submenu_requested`: String

**命令.** `set_text`, `set_expanded`

**Rust API.** `create_menu_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("menu_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `dropdown_button`

## 10. 对话框与临时面板

对话框是带自身结果的独立界面：消息与确认框、文件/颜色/字体/输入选择器、进度与向导对话框，以及非模态的底部面板、气泡与工具提示。

需要决策时用 `MessageBox` 或 alert/confirm 对话框；需要带标题、承载一个内容控件的外壳时用 `Dialog`；标准选择器用 `FileDialog`/`ColorDialog`/`FontDialog`/`InputDialog`；可取消的长任务用 `ProgressDialog`；有序步骤用 `WizardDialog`。`BottomSheet`、`ModalBottomSheet`、`Popover` 与 `Tooltip` 用于不拥有窗口的临时内容。不要用对话框表达非阻塞通知（`Banner`、`Toast`、`Snackbar`），也不要在另一个模态自己的栈里再嵌模态——统一交给 `ModalHost`。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `dialog` | `src/widget/dialog/` | 17 |

### Dialog (`dialog`)

带标题、承载一个内容控件的对话框外壳，带模态、接受/拒绝/结果与帮助信号。需要在自己的对话框框体内放内容时使用；标准决策用 `MessageBox` 与 alert/confirm 对话框，仅浮层外壳用 `PopupWindow`。

**属性.** 除四个共享属性外共 7 项：`title`: String · `modal`: Bool · `has_content`: Bool (只读) · `result`: Int (只读) · `accepted`: Bool (只读) · `rejected`: Bool (只读) · `closable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `closed` · `finished`: Int · `help_requested` · `opened` · `rejected`

**命令.** `accept`, `reject`

**Rust API.** `create_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### MessageBox (`message_box`)

带图标、提示与详细文本、可选复选框与固定响应按钮的模态消息。用于简短决策或确认；更风格化的提示用 `AlertDialog`/`ConfirmDialog`，内容自定义时用 `Dialog`。

**属性.** 除四个共享属性外共 10 项：`title`: String · `text`: String · `icon`: Enum (`none` | `information` | `question` | `warning` | `critical`) · `modal`: Bool · `informative_text`: String · `detailed_text`: String · `check_box`: String · `check_box_checked`: Bool · `default_button`: String · `escape_button`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `button_clicked`: String · `details_toggled`: Bool · `focus_changed`: UInt · `rejected`

**命令.** `set_text`, `set_title`, `set_icon`

**Rust API.** `create_message_box(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("message_box", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `msgbox`

### FileDialog (`file_dialog`)

模态文件系统浏览器，支持打开、多选打开、保存与选择目录模式，带导航历史与侧栏。用户必须选择文件路径时使用；`DirectoryDialog` 是这个控件的目录模式角色，不是独立实现。

**属性.** 除四个共享属性外共 12 项：`title`: String · `modal`: Bool · `directory`: String · `selected_file`: String · `mode`: Enum (`open_file` | `open_files` | `save_file` | `select_directory`) · `label_text`: String · `default_suffix`: String · `file_count`: UInt (只读) · `can_go_back`: Bool (只读) · `can_go_forward`: Bool (只读) · `view_mode`: Enum (`list` | `details`) · `sidebar_urls`: StringList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `current_changed`: String · `directory_entered`: String · `file_selected`: String · `files_selected`: list of String · `focus_changed`: UInt · `overwrite_requested`: String · `quick_look_requested`: String · `rejected` · `selection_changed`: list of String · `sort_changed`: mixed (String parts) · `url_selected`: String

**命令.** `set_mode`, `set_directory`, `open`

**Rust API.** `create_file_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("file_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ColorDialog (`color_dialog`)

模态颜色选择器，带色轮或色轨、预设、十六进制输入与可选屏幕取色。用于完整颜色决策；选择器嵌在表单内时用内联 `ColorPicker`，单色块折叠字段用 `ColorWell`。

**属性.** 除四个共享属性外共 10 项：`current_color`: String (只读) · `hex_rgba`: String · `preset_index`: UInt · `show_hex_input`: Bool · `show_presets`: Bool · `show_hue_rail`: Bool · `custom_color_count`: UInt (只读) · `modal`: Bool · `options_alpha`: Bool · `show_screen_picker`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `color_selected`: Color · `current_color_changed`: Color · `focus_changed`: UInt · `rejected` · `screen_color_sampled`: Color · `screen_pick_canceled` · `screen_pick_started`

**命令.** `set_hex`, `apply_preset`

**Rust API.** `create_color_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("color_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `colour_dialog`

### FontDialog (`font_dialog`)

模态字体选择器，支持字族、样式与字号选择、预览与接受/拒绝。用户必须选择字体并看到字样时使用；内联字族字段用 `FontComboBox`，旁边的字样用 `FontPreview`。

**属性.** 除四个共享属性外共 5 项：`modal`: Bool · `underline`: Bool · `strikethrough`: Bool · `no_buttons`: Bool · `show_preview`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `font_selected`: String · `rejected`

**命令.** `set_current_font`, `accept`, `reject`

**Rust API.** `create_font_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("font_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### InputDialog (`input_dialog`)

模态单值提示，支持文本、整数、浮点或条目模式，并带对应值属性与变化信号。需要不搭表单就取一个值时使用；涉及多个值或校验时用带真实字段的 `Dialog`。

**属性.** 除四个共享属性外共 9 项：`title`: String · `label_text`: String · `mode`: Enum (`text` | `integer` | `double` | `item`) · `text_value`: String · `int_value`: Int · `double_value`: Float · `int_step`: Int · `current_item`: UInt · `dropdown_open`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `double_value_changed`: Float · `focus_changed`: UInt · `int_value_changed`: Int · `rejected` · `text_value_changed`: String

**命令.** `set_mode`, `set_text_value`, `set_int_value`, `set_double_value`

**Rust API.** `create_input_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("input_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ProgressDialog (`progress_dialog`)

模态进度界面，带标签、进度条、范围、可选取消按钮与自动重置。用于用户发起的可取消工作；程序拥有的启动工作用 `SplashScreen`，工作内联且不阻塞时用普通 `ProgressBar`。

**属性.** 除四个共享属性外共 11 项：`title`: String · `label_text`: String · `value`: Int · `minimum`: Int · `maximum`: Int · `auto_reset`: Bool · `show_cancel_button`: Bool · `show_bar`: Bool · `minimum_duration`: UInt · `animated`: Bool · `accent_fill`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `finished` · `label_changed`: String · `value_changed`: Int

**命令.** `set_value`, `set_range`, `set_title`, `set_label_text`

**Rust API.** `create_progress_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("progress_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### PopupWindow (`popup_window`)

无边框、可选瞬态的浮动窗口，带自己的外壳标志、标题、内容与调整大小信号。用于只需要浮层外壳、不是完整对话框的内容；锚定气泡用 `Popover`，需要模态语义时用 `Dialog`。

**属性.** 除四个共享属性外共 6 项：`title`: String · `has_content`: Bool (只读) · `modal`: Bool · `transient_parent`: UInt · `floating_chrome`: Bool · `resizable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed` · `opened` · `resized`: UInt ×2

**命令.** `set_content_widget`

**Rust API.** `create_popup_window(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("popup_window", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `popup`

### BottomSheet (`bottom_sheet`)

从底边滑上、可拖回的 panel，带 peek 高度与展开状态。用于触屏布局的次要内容；sheet 必须阻塞页面时用 `ModalBottomSheet`，小型锚定气泡用 `Popover`。

**属性.** 除四个共享属性外共 2 项：`expanded`: Bool · `peek_height`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** `set_expanded`, `set_peek_height`

**Rust API.** `create_bottom_sheet(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("bottom_sheet", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `sheet`

### ModalBottomSheet (`modal_bottom_sheet`)

Material 风格模态底部面板，高度可配置、带动画呈现与拖拽关闭的 `dismissed` 信号。用于手机上阻塞式的选项或详情；非阻塞 peek 内容用 `BottomSheet`，短消息决策用 `Dialog`。

**属性.** 除四个共享属性外共 2 项：`sheet_height`: UInt · `animated`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`

**命令.** `set_visible`, `show`, `dismiss`

**Rust API.** `create_modal_bottom_sheet(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("modal_bottom_sheet", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `modal_sheet`

### Tooltip (`tooltip`)

悬停时显示上下文信息的小弹层文本，支持换行与显示状态。用于已有可见标签的控件的提示；不要把必要说明只放在工具提示里；延迟与位置需要多个控件共享时用 `TooltipManager` 配套控件。

**属性.** 除四个共享属性外共 3 项：`text`: String · `shown`: Bool · `wrap`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_text`, `show`, `hide`

**Rust API.** `create_tooltip(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tooltip", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `tooltip_widget`, `hover_tip`

### Popover (`popover`)

带锚定箭头的浮动气泡卡片，支持瞬态/半瞬态/常驻行为与显示状态。用于控件附近的小块上下文内容；仅文本提示用 `Tooltip`，内容需要窗口外壳时用 `PopupWindow`。

**属性.** 除四个共享属性外共 6 项：`shown`: Bool · `text`: String (只读) · `behavior`: Enum (`transient` | `semi_transient` | `permanent`) · `arrow_direction`: Enum (`auto` | `up` | `down` | `left` | `right`) · `placeholder_text`: String · `translucent`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `dismissed` · `shown`

**命令.** `set_visible`, `set_text`

**Rust API.** `create_popover(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("popover", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `popover_widget`, `popup_card`

### FindReplaceDialog (`find_replace_dialog`)

查找/替换对话框，带查询与替换字段、大小写/全词/正则开关、全部高亮与循环查找，以及下一个/上一个/替换/全部替换信号。需要自有模态的整文档搜索用；编辑器内联条用 `FindBar`。

**属性.** 除四个共享属性外共 7 项：`find_text`: String · `replace_text`: String · `match_case`: Bool · `whole_word`: Bool · `use_regex`: Bool · `highlight_all`: Bool · `wrap_around`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `close` · `find_next`: String · `find_previous`: String · `focus_changed`: UInt · `replace`: String · `replace_all`: String ×3

**命令.** `set_find_text`, `set_replace_text`, `set_match_case`, `set_wrap_around`

**Rust API.** `create_find_replace_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("find_replace_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `find_replace`

### WizardDialog (`wizard_dialog`)

多页对话框，按顺序引导用户完成步骤，支持上一步/下一步/完成、逐步有效性与侧栏页面。顺序重要的设置流程用；步骤不是模态流程时用 `StackedWidget` 加自建按钮；单页用 `Dialog`。

**属性.** 除四个共享属性外共 6 项：`title`: String · `subtitle`: String · `current_step`: UInt (只读) · `step_count`: UInt (只读) · `can_go_back`: Bool (只读) · `can_go_forward`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `custom_button_clicked`: UInt · `finished` · `help_requested` · `sidebar_page_activated`: UInt · `step_changed`: UInt

**命令.** `next`, `back`, `finish`

**Rust API.** `create_wizard_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("wizard_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `wizard`

### AlertDialog (`alert_dialog`)

模态提示，带消息、图标与一个确认动作，以及可选提示/详细文本与复选框。用于用户必须确认的信息；存在真正的接受/取消选择时用 `ConfirmDialog`，经典固定按钮形式用 `MessageBox`。

**属性.** 除四个共享属性外共 10 项：`title`: String · `message`: String · `button_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `focus_changed`: UInt · `rejected`

**命令.** 未发布命令。

**Rust API.** `create_alert_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("alert_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `notice_dialog`

### ConfirmDialog (`confirm_dialog`)

模态问题，带接受动作、取消动作、可选提示/详细文本与复选框。用于破坏性或不可逆操作之前；仅信息用 `AlertDialog`，按钮必须是经典集合时用 `MessageBox`。

**属性.** 除四个共享属性外共 11 项：`title`: String · `message`: String · `accept_text`: String · `reject_text`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `accepted` · `focus_changed`: UInt · `rejected`

**命令.** 未发布命令。

**Rust API.** `create_confirm_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("confirm_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `confirmation_dialog`

### AboutDialog (`about_dialog`)

标准应用信息对话框：应用名、版本、消息、提示与详细文本，以及可选复选框。用于帮助/关于；不要把它当通用对话框——这些字段就是全部契约。

**属性.** 除四个共享属性外共 11 项：`title`: String · `application_name`: String · `version`: String · `message`: String · `informative_text`: String · `detailed_text`: String · `modal`: Bool · `default_button`: UInt · `check_box`: String · `check_box_checked`: Bool · `icon`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `closed`

**命令.** 未发布命令。

**Rust API.** `create_about_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("about_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `about_box`

## 11. 复合应用界面

特殊族承载“整个应用界面”而非单个字段的控件：绘图画布、地图、通用图表、甘特与时间线、看板、颜色与形状选择器、代码相关查看器（差异、终端、Markdown）、通知/Toast 面，以及与这些面配对的配套控件。

当功能*本身*就是界面时用特殊控件——自由绘图用 canvas，瓦片地理用 `MapView`，两级拖放用 `KanbanBoard`，并排或统一对比用 `DiffViewer`。它们各自持有模型并暴露很窄的信号集；宿主喂数据、对信号作出反应。不要把它们当通用容器，也不要用一堆小控件拼出它们的交互：拖拽、命中测试与视口规则正是这些控件的价值。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `special` | `src/widget/special_widgets/` | 39 |

### Canvas (`canvas`)

自由绘图表面，带图层、缩放、平移、网格吸附与携带位置的指针事件。用于宿主自行绘制的场景；场景是可编辑的可选形状集合时用 `ShapeCanvas`，小型即时模式装饰用 `MiniCanvas`。

**属性.** 除四个共享属性外共 8 项：`command_count`: UInt (只读) · `layer_count`: UInt (只读) · `anti_alias`: Bool · `show_grid`: Bool · `grid_step`: UInt · `snap_to_grid`: Bool · `zoom_range`: NumberList · `zoom_step`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_picked`: UInt · `content_changed`: UInt · `context_menu_requested`: String · `double_clicked`: mixed (String parts) · `layer_changed`: UInt · `mouse_moved`: String · `mouse_pressed`: mixed (String parts) · `mouse_released`: mixed (String parts) · `zoom_changed`: Float

**命令.** `set_zoom`, `set_center`, `clear`

**Rust API.** `create_canvas(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("canvas", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `canvas_widget`, `drawing_surface`

### Canvas (`map_view`)

瓦片地图视图，带中心、缩放、投影与标记，并拥有 `TileLoader`、`ZoomControl`、`ScaleBar`、`Compass` 配套控件。宿主抓取瓦片并调用 `deliver_tile`/`reject_tile`；视图通过 `tile_requested` 请求，控件本身不做 I/O。

**属性.** 除四个共享属性外共 6 项：`center_x`: Float · `center_y`: Float · `zoom`: Float · `marker_count`: UInt (只读) · `selected_marker_id`: String (只读) · `projection`: Enum (`linear` | `mercator`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `center_changed`: Float ×2 · `cluster_clicked`: Float ×2 · `marker_clicked`: String · `marker_selected`: String · `tile_requested`: mixed (String parts) · `zoom_changed`: Float

**命令.** `set_markers`, `set_center_x`, `set_center_y`, `set_zoom`

**Rust API.** `create_map_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("map_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Chart (`chart`)

通用图表控件：图表类型（柱状、折线、面积、饼、散点、瀑布、漏斗、蜡烛、箱线）、数据序列、标签、缩放与悬停读数。类型在运行时选择时使用；固定类型用专用 `LineChart`/`BarChart`/`PieChart`，行情数据用金融族。

**属性.** 除四个共享属性外共 4 项：`chart_type`: Enum (`bar` | `line` | `area` | `pie` | `scatter` | `waterfall` | `funnel` | `candlestick` | `box_plot`) · `point_count`: UInt (只读) · `label_count`: UInt (只读) · `series_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `data_point_clicked`: UInt · `data_point_hovered`: UInt · `data_point_unhovered`: UInt · `viewport_changed`: UInt ×2 · `zoom_changed`: (Float, Float) or null

**命令.** 未发布命令。

**Rust API.** `create_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `chart_widget`, `chart_surface`

### Chart (`gantt_widget`)

按视口绘制任务的甘特图，支持里程碑、依赖、关键路径、进度调整与今日标记；它拥有 `TimelineAxis` 与 `DependencyEditor`。用于项目排程；非排程的事件车道视图用 `TimelineWidget`。

**属性.** 除四个共享属性外共 7 项：`task_count`: UInt (只读) · `selected_id`: String (只读) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_critical_path`: Bool · `show_today`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2 · `task_rescheduled`: String · `task_resize_finished`: String · `task_resize_started`: String · `task_selected`: String · `viewport_changed`: Int ×2

**命令.** `set_tasks`, `zoom`, `set_viewport`, `select_task`

**Rust API.** `create_gantt_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("gantt_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `gantt`

### Chart (`timeline_widget`)

按时间轴放置条目的时间线，支持行或车道紧凑布局与可选条目。用于事件历史与动态流；任务有工期、依赖与进度时用 `GanttWidget`。

**属性.** 除四个共享属性外共 7 项：`item_count`: UInt (只读) · `selected_index`: UInt (只读) · `viewport_start`: Int · `viewport_end`: Int · `row_height`: UInt · `show_time_labels`: Bool · `lane_mode`: Enum (`row` | `lane_packed`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_hovered`: String or null · `item_selected`: String · `viewport_changed`: Int ×2

**命令.** `set_items`, `set_viewport`, `zoom`, `select_index`

**Rust API.** `create_timeline_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("timeline_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `timeline`, `timeline_view`

### ColorPicker (`color_picker`)

内联 HSVA 颜色选择器，带十六进制输入、alpha、预设与色相值；它嵌在表单里而不是放在窗口中。颜色字段始终可见时使用；模态选择器用 `ColorDialog`，折叠单色块字段用 `ColorWell`。

**属性.** 除四个共享属性外共 5 项：`hex_rgba`: String · `show_alpha`: Bool · `preset_count`: UInt (只读) · `presets`: StringList · `hue_degrees`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `color_changed`: Color · `color_committed`: Color · `color_previewed`: Color · `hex_changed`: String

**命令.** `set_hex`, `apply_preset`

**Rust API.** `create_color_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("color_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Grid (`grid`)

把子控件排成行列单元格的矩形容器，支持间距、单元格尺寸、表头与分页。用于固定的控件或数值矩阵；单元格来自模型时用 `Table`，纯布局放置用 `Flex`/`Grid` 布局名称。

**属性.** 除四个共享属性外共 11 项：`rows`: UInt · `columns`: UInt · `spacing`: UInt · `line_color`: String · `cell_width`: UInt (只读) · `cell_height`: UInt (只读) · `selected_cell`: UInt · `selected_count`: UInt (只读) · `page`: UInt · `page_count`: UInt (只读) · `headers`: StringList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `context_menu_requested`: UInt ×2 · `page_changed`: UInt

**命令.** `set_rows`, `set_columns`, `set_spacing`, `set_line_color`

**Rust API.** `create_grid(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("grid", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `grid_widget`, `gridwidget`

### FreeformShape (`freeform_shape`)

基于路径、非矩形的可点击形状，带填充、描边、顶点、历史与移动/顶点信号。用于图节点或自定义命中区域；多个形状组成可编辑场景时用 `ShapeCanvas`，非交互绘图用 `MiniCanvas`。

**属性.** 除四个共享属性外共 5 项：`path_kind`: Enum (`bubble` | `rounded_rect` | `polygon` | `star` | `heart` | `custom`) (只读) · `fill_rgba`: String · `stroke_rgba`: String · `stroke_width`: UInt · `movable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clicked` · `geometry_changed`: Rect · `history_changed`: UInt ×2 · `hover_changed`: Bool · `moved`: String · `pressed_changed`: Bool · `vertex_changed`: mixed (String parts)

**命令.** `set_fill_rgba`, `set_stroke_rgba`, `set_stroke_width`

**Rust API.** `create_freeform_shape(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("freeform_shape", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### RadarChart (`radar_chart`)

雷达（蜘蛛）图，带共享维度轴、以闭合多边形绘制的多个序列、网格、轴标签与图例。用于维度可比的轮廓（技能、KPI）；单一有序轴用 `LineChart`，矩阵数据用 `Heatmap`。

**属性.** 除四个共享属性外共 7 项：`axis_count`: UInt (只读) · `series_count`: UInt (只读) · `show_grid`: Bool · `show_axis_labels`: Bool · `show_legend`: Bool · `hovered_axis`: UInt (只读) · `series_names`: StringList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `axis_hovered`: UInt · `series_clicked`: UInt · `vertex_clicked`: UInt ×2 · `vertex_hovered`: UInt ×2

**命令.** `set_axes`, `set_series`, `add_series`

**Rust API.** `create_radar_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("radar_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `radar`, `spider_chart`

### Heatmap (`heatmap`)

以彩色单元格渲染的值矩阵，带色阶范围、标签、图例与视口窗口。用于两个轴都是分类的密度或相关性矩阵；普通单元格用 `Grid`，按索引的单值序列用 `Chart`。

**属性.** 除四个共享属性外共 10 项：`row_count`: UInt (只读) · `column_count`: UInt (只读) · `scale_minimum`: Float · `scale_maximum`: Float · `resolved_minimum`: Float (只读) · `resolved_maximum`: Float (只读) · `show_labels`: Bool · `show_legend`: Bool · `viewport_rows`: UInt · `viewport_columns`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `cell_clicked`: UInt ×2 · `cell_double_clicked`: UInt ×2 · `cell_hovered`: UInt ×2 · `region_selected`: UInt ×4

**命令.** `set_data`, `set_cell`, `clear`

**Rust API.** `create_heatmap(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("heatmap", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `heatmap_chart`

### KanbanBoard (`kanban_board`)

由列组成、承载可拖拽卡片的看板，支持泳道、列宽、水平偏移与卡片移动/激活信号。配 `KanbanColumnHeader` 提供重命名/计数/WIP 外壳；卡片需要真实可编辑控件时配 `KanbanCardView`；看板持有模型与放置规则。

**属性.** 除四个共享属性外共 9 项：`column_count`: UInt (只读) · `card_count`: UInt (只读) · `dragging_card_id`: String (只读) · `hovered_column`: UInt (只读) · `column_width`: UInt · `horizontal_scroll_offset`: UInt · `lane_mode`: Enum (`none` | `lanes`) · `swimlanes`: Bool · `lane_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `card_activated`: String · `card_moved`: String ×2

**命令.** `add_column`, `add_card`, `move_card`, `cancel_drag`

**Rust API.** `create_kanban_board(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("kanban_board", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `kanban`, `board`

### SignaturePad (`signature_pad`)

触屏友好的手写签名表面：带宽度、颜色、平滑阈值与压力的笔画，支持撤销与笔画完成信号。用于签名或手写输入；不要把它当通用绘图软件——它保存折线而不是位图。

**属性.** 除四个共享属性外共 9 项：`stroke_count`: UInt (只读) · `stroke_width`: UInt · `stroke_color`: Color · `min_point_distance`: Float · `min_point_interval_ms`: UInt · `pressure_affects_width`: Bool · `last_pressure`: Float (只读) · `last_tilt_x`: Float (只读) · `last_tilt_y`: Float (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `changed` · `stroke_completed`: String

**命令.** `undo`

**Rust API.** `create_signature_pad(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("signature_pad", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `signature`

### Breadcrumb (`breadcrumb`)

由区段组成的横向导航轨迹，支持折叠、溢出、内联路径编辑与逐区段激活。用于用户可向上跳转的层级；编号分页用 `Pagination`，顶层命令用 `MenuBar`。

**属性.** 除四个共享属性外共 4 项：`segment_count`: UInt (只读) · `selected_index`: UInt (只读) · `collapsed_count`: UInt (只读) · `separator`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `context_menu_requested`: String · `overflow_clicked`: UInt · `path_edited`: String · `segment_activated`: String

**命令.** `set_segments`, `push_segment`, `clear_segments`

**Rust API.** `create_breadcrumb(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("breadcrumb", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `nav_breadcrumb`

### Chip (`chip`)

多条目胶囊行：chip 可选择、移除、重排与溢出，并带焦点索引。用于作为一组编辑的 token/标签集合；单个只读标签用 `Tag`，用户键入新值时用 `TagInput`。

**属性.** 除四个共享属性外共 4 项：`item_count`: UInt (只读) · `multi_select`: Bool · `focused_index`: UInt (只读) · `selected_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `chip_removed`: String · `chip_reordered`: String · `chip_toggled`: String · `overflow_activated`: UInt

**命令.** `set_items`, `toggle_index`, `move_focus`

**Rust API.** `create_chip(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("chip", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `chips`

### Toast (`toast`)

单条瞬时通知消息，带级别、存活时间与关闭信号。应通过 toast 面或 `ToastHost` 队列创建，而不是管理大量独立 toast；通知需要持久时用 `Banner`，属于窗口边缘时用 `Snackbar`。

**属性.** 除四个共享属性外共 4 项：`message`: String · `level`: Enum (`info` | `success` | `warning` | `error`) · `ttl_ms`: UInt · `dismissible`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dismissed`: String

**命令.** `dismiss`, `set_message`, `set_level`

**Rust API.** `create_toast(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("toast", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `notification`, `toast_message`

### PopupWindow (`toast_stack`)

排队并布局 toast 的容器，带可见数、最大可见数、锚点与堆叠方向。作为 toast 放置的唯一所有者；队列需要在进入堆栈前过期与限流时配 `ToastHost`。

**属性.** 除四个共享属性外共 6 项：`toast_count`: UInt (只读) · `selected_id`: String (只读) · `row_height`: UInt · `max_visible`: UInt · `anchor`: Enum (`top_left` | `top_right` | `bottom_left` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_bottom` | `newest_at_top`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toast_action`: String · `toast_activated`: String · `toast_dismissed`: String · `toast_expired`: String

**命令.** `push`, `clear`, `select_index`, `activate_selected`, `dismiss_selected`

**Rust API.** `create_toast_stack(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("toast_stack", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `toasts`

### ListView (`command_palette`)

可搜索的命令列表浮层：查询、过滤后数量、高亮索引与 `command_activated`。用于键盘优先的启动器；鼠标驱动的命令列表用 `Menu`，编辑绑定用 `ShortcutEditor`。

**属性.** 除四个共享属性外共 6 项：`query`: String · `entry_count`: UInt (只读) · `filtered_count`: UInt (只读) · `highlighted_index`: UInt (只读) · `row_height`: UInt · `group_by_category`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_activated`: String · `query_changed`: String

**命令.** `set_entries`, `set_query`, `clear_query`, `move_highlight`, `activate_highlighted`

**Rust API.** `create_command_palette(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("command_palette", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `command_box`

### ListView (`notification_center`)

通知列表，带未读数、级别过滤、最大条目与激活/关闭/全部清除信号。用于事件的持久收件箱；瞬时打断用 `Toast`/`ToastStack`，单条持久通知用 `Banner`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `unread_count`: UInt (只读) · `selected_index`: UInt (只读) · `row_height`: UInt · `level_filter`: Enum (`all` | `info` | `warning` | `error`) · `max_items`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `clear_all_requested` · `notification_activated`: String · `notification_dismissed`: String · `notification_selected`: String · `unread_count_changed`: UInt

**命令.** `push`, `clear`, `set_read`, `mark_all_read`, `select_index`, `activate_selected`

**Rust API.** `create_notification_center(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("notification_center", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `notifications`

### Table (`diff_viewer`)

对两段文本做并排或统一差异视图，带变更数、折叠阈值、空白模式、词级粒度与差异块展开。用于展示比较；配 `DiffFileList` 在多文件间选择；不要把它当编辑器——它是只读的。

**属性.** 除四个共享属性外共 12 项：`left_text`: String · `right_text`: String · `line_count`: UInt (只读) · `change_count`: UInt (只读) · `selected_index`: UInt (只读) · `view_mode`: Enum (`side_by_side` | `unified`) · `fold_unchanged`: Bool · `fold_threshold`: UInt · `fold_context_lines`: UInt · `unified_changed_pair`: Bool · `whitespace_mode`: Enum (`show` | `ignore_eol` | `ignore_all` | `mark_blank_changes`) · `word_granularity`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `compared`: UInt · `fold_changed`: Bool · `hunk_expanded`: UInt · `selection_changed`: UInt or null · `view_mode_changed`: String

**命令.** `set_texts`, `select_index`

**Rust API.** `create_diff_viewer(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("diff_viewer", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `diff`

### SegmentedControl (`segmented_control`)

单选分段条，指示器在区段间滑动，每段有 id。用于两到五个互斥视图；多选或图标分段用 `SegmentedButton`，区段是页面标签时用 `TabBar`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `selected_index`: UInt (只读) · `selected_id`: String (只读) · `size_to_fit`: Bool · `min_segment_width`: UInt · `multi_select`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `selection_changed`: String

**命令.** `set_items`, `move_selection`

**Rust API.** `create_segmented_control(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("segmented_control", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### StatusBar (`snackbar`)

锚定在窗口边缘的瞬时消息，带动作标签、队列长度与最大队列。用于不阻塞地确认已完成动作；消息需要持久时用 `Banner`，多条消息需要堆叠时用 `ToastStack`。

**属性.** 除四个共享属性外共 5 项：`message`: String · `action_label`: String (只读) · `queued_count`: UInt (只读) · `max_queued`: UInt · `anchor`: Enum (`top` | `center` | `bottom`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_triggered`: String · `dismissed`

**命令.** `show`, `show_with_action`, `dismiss`

**Rust API.** `create_snackbar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("snackbar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `toast_bar`

### WebEngineView (`media_player`)

媒体表面，带来源、播放/暂停、位置、时长、音量与全屏，并报告播放与位置变化。宿主提供媒体管线时使用；配 `MediaTransportBar`、`MediaScrubber`、`SubtitleBar` 配套控件，而不是重写这些控件。

**属性.** 除四个共享属性外共 9 项：`source`: String · `playing`: Bool · `duration_ms`: UInt (只读) · `position_ms`: UInt · `volume`: UInt · `muted`: Bool · `fullscreen`: Bool · `playlist_index`: UInt · `track_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `chapter_selected`: UInt · `fullscreen_requested`: Bool · `media_ended` · `playback_changed`: Bool · `position_changed`: UInt · `source_changed`: String · `volume_changed`: UInt

**命令.** `set_source`, `clear_source`, `play`, `pause`, `seek_to`, `set_volume`

**Rust API.** `create_media_player(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("media_player", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ShapeCanvas (`shape_canvas`)

承载、选择与排序形状的场景，带新增/修改/移除/选择信号、吸附与网格步长。用于图编辑器；单个形状用 `FreeformShape`，非形状对象的任意绘图用 `Canvas`。

**属性.** 除四个共享属性外共 6 项：`shapes`: NumberList (只读) · `shape_count`: UInt (只读) · `selection`: NumberList · `editable`: Bool · `snap_to_grid`: Bool · `grid_step`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `shape_added`: UInt · `shape_modified`: UInt · `shape_removed`: UInt · `shape_selected`: list of UInt

**命令.** 未发布命令。

**Rust API.** `create_shape_canvas(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("shape_canvas", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `shape_board`

### KanbanCardView (`kanban_card_view`)

看板列中的真实卡片控件：标题、描述、标签、负责人、完成状态与就地编辑。卡片必须是真正可编辑控件时使用；`KanbanBoard` 持有放置与拖拽状态，本控件持有一张卡片的内容与编辑生命周期。

**属性.** 除四个共享属性外共 7 项：`title`: String · `description`: String · `done`: Bool · `tags`: StringList · `assignee`: String · `draggable`: Bool · `editing`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `card_clicked` · `card_done_toggled`: Bool · `card_edited`: String

**命令.** 未发布命令。

**Rust API.** `create_kanban_card_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("kanban_card_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `kanban_card_control`

### KanbanColumnHeader (`kanban_column_header`)

看板列的真实表头：标题、卡片数、WIP 上限、折叠与就地重命名。放在 `KanbanBoard` 列上方；表头持有自己的重命名与折叠状态，并报告 `column_renamed`、`column_collapsed_changed` 与 `add_card_requested`。

**属性.** 除四个共享属性外共 7 项：`title`: String · `count`: UInt · `wip_limit`: UInt · `collapsed`: Bool · `editable`: Bool · `at_wip_limit`: Bool (只读) · `editing`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `add_card_requested` · `column_collapsed_changed`: Bool · `column_renamed`: String

**命令.** 未发布命令。

**Rust API.** `create_kanban_column_header(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("kanban_column_header", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `kanban_column_bar`

### DiffFileList (`diff_file_list`)

差异查看器旁的多文件选择器：路径列表，带选择模式与激活/选择信号。比较由查看器持有；本控件只选择查看器应显示哪个路径。

**属性.** 除四个共享属性外共 4 项：`files`: StringList · `file_count`: UInt (只读) · `selected_path`: String (只读) · `selection_mode`: Enum (`single` | `multi` | `extended` | `none`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `file_activated`: String · `file_selected`: String

**命令.** 未发布命令。

**Rust API.** `create_diff_file_list(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("diff_file_list", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `diff_files`

### Swatch (`swatch`)

颜色控件共享的可选色格网格，带选中索引与激活/选择信号。当前颜色由对话框或选择器持有；色格持有调色板与选择状态，因此把 `swatch_selected` 接到拥有者的颜色属性。

**属性.** 除四个共享属性外共 4 项：`colors`: Color · `selected_index`: UInt · `cell_size`: UInt · `columns`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `swatch_activated`: Color · `swatch_selected`: UInt

**命令.** 未发布命令。

**Rust API.** `create_swatch(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("swatch", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `color_swatch`, `colour_swatch`

### FilterRow (`filter_row`)

每列一个编辑器、发布该行过滤条件与实时标志的控件。用于表格旁的紧凑多列过滤；模型由表格或网格持有，本行只产生条件。

**属性.** 除四个共享属性外共 4 项：`columns`: StringList · `conditions`: StringList · `operator`: Enum (`contains` | `equals` | `starts_with` | `ends_with` | `not_contains` | `equals_number` | `greater_than` | `greater_or_equal` | `less_than` | `less_or_equal`) · `live`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `filter_changed`: String · `filter_cleared`

**命令.** 未发布命令。

**Rust API.** `create_filter_row(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("filter_row", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `filter_bar`

### Legend (`legend`)

图表与画布宿主的可勾选序列键：条目、方向、位置与点击/切换信号。图表持有序列数据；图例持有哪些序列可见，因此把 `series_toggled` 转给图表的可见性状态。

**属性.** 除四个共享属性外共 4 项：`entries`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `checkable`: Bool · `position`: Enum (`top` | `right` | `bottom` | `left` | `outside`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_clicked`: UInt · `series_toggled`: mixed (String parts)

**命令.** 未发布命令。

**Rust API.** `create_legend(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("legend", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `chart_legend`

### ChartTooltip (`chart_tooltip`)

跟随指针、承载图表序列值的浮泡，带锚点、偏移、跟随标志与内容行。图表通过 `show_at` 定位它；工具提示只持有文本与可见性。

**属性.** 除四个共享属性外共 4 项：`anchor`: NumberList · `offset`: NumberList · `follow_pointer`: Bool · `content`: StringList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `hidden` · `shown`

**命令.** 未发布命令。

**Rust API.** `create_chart_tooltip(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("chart_tooltip", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `tooltip_bubble`

### TooltipManager (`tooltip_manager`)

把悬停转换为延迟工具提示的宿主管线：延迟、持续时间、跟随指针与最大宽度，并带显示/隐藏索引。多个控件共享提示时序时，为一个子树使用一个管理器；单个控件仍可用自己的 `tooltip` 属性。

**属性.** 除四个共享属性外共 4 项：`delay_ms`: UInt · `duration_ms`: UInt · `follow_pointer`: Bool · `max_width`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tooltip_hidden`: UInt · `tooltip_shown`: UInt

**命令.** 未发布命令。

**Rust API.** `create_tooltip_manager(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tooltip_manager", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `tooltip_pipeline`

### ZoomPanController (`zoom_pan_controller`)

图像视图的缩放、平移与感兴趣区域状态，带缩放模式、上下限与变化信号。多个控件必须共享同一视图变换时使用；图像视图负责绘制，控制器持有变换。

**属性.** 除四个共享属性外共 6 项：`zoom_level`: Float · `pan_offset`: NumberList · `zoom_mode`: Enum (`free` | `fit_width` | `fit_height` | `original`) · `min_zoom`: Float · `max_zoom`: Float · `roi`: Rect — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `pan_changed`: Int ×2 · `roi_changed`: Rect or null · `zoom_changed`: Float

**命令.** 未发布命令。

**Rust API.** `create_zoom_pan_controller(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("zoom_pan_controller", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `zoom_pan`

### ToastHost (`toast_host`)

toast 堆栈前的有界、会过期队列：最大可见、最大排队、默认时长、锚点、堆叠方向与溢出策略。toast 数量必须限流时使用；堆栈负责绘制，宿主决定哪些 toast 能存活到进入堆栈。

**属性.** 除四个共享属性外共 6 项：`max_visible`: UInt · `max_queued`: UInt · `default_duration_ms`: UInt · `anchor`: Enum (`top_left` | `top_center` | `top_right` | `bottom_left` | `bottom_center` | `bottom_right`) · `stacking_direction`: Enum (`newest_at_top` | `newest_at_bottom`) · `overflow_policy`: Enum (`drop_oldest` | `drop_newest` | `queue`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `queue_overflow`: UInt · `toast_dismissed`: UInt · `toast_expired`: UInt

**命令.** 未发布命令。

**Rust API.** `create_toast_host(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("toast_host", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `toast_queue`

### TileLoader (`tile_loader`)

地图视图的瓦片寻址、缓存与加载结果：URL 模板、缩放范围、瓦片尺寸、缓存上限与加载成功/失败信号。地图视图持有一个并索取 URL；宿主执行请求并调用 `deliver_tile`。

**属性.** 除四个共享属性外共 6 项：`url_template`: String · `min_zoom`: UInt · `max_zoom`: UInt · `tile_size`: UInt · `cache_limit`: UInt · `cache_size`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `tile_failed`: Int ×3 · `tile_loaded`: Int ×3

**命令.** `clear_cache`

**Rust API.** `create_tile_loader(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tile_loader", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `map_tile_loader`

### ZoomControl (`zoom_control`)

地图旁的三按钮缩放条：缩放级别、上下限与 `zoom_requested` 信号。地图持有该条并应用请求；如果地图也报告缩放变化，不要让该条直接修改地图缩放。

**属性.** 除四个共享属性外共 3 项：`zoom_level`: Float · `min`: Float · `max`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `zoom_requested`: Float

**命令.** 未发布命令。

**Rust API.** `create_zoom_control(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("zoom_control", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `map_zoom_control`

### ScaleBar (`scale_bar`)

地图的公制或英制距离图例，由每像素米数计算。地图持有并更新换算；该条只负责绘制图例。

**属性.** 除四个共享属性外共 3 项：`meters_per_pixel`: Float · `units`: Enum (`metric` | `imperial`) · `style`: Enum (`plain` | `boxed`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** 未发布命令。

**Rust API.** `create_scale_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("scale_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `map_scale_bar`

### Compass (`compass`)

可点击的方位盘，点击时请求正北朝上，并报告自身方位。用于可旋转地图之上；地图持有方位并响应 `reset_north_requested`。

**属性.** 除四个共享属性外共 2 项：`bearing_deg`: Float · `clickable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `reset_north_requested`

**命令.** 未发布命令。

**Rust API.** `create_compass(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("compass", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `map_compass`

### TimelineAxis (`timeline_axis`)

日期/数值轴，支持像素换算、刻度密度、缩放与刻度变化报告。甘特图持有一个并使用 `pixel_to_date`/`date_to_pixel`；宿主也可以在图表旁再放一个以共享同一映射。

**属性.** 除四个共享属性外共 6 项：`tick_density`: UInt · `date_format`: Enum (`numeric` | `iso`) · `zoom_level`: Float · `range`: NumberList (只读) · `range_start`: Int · `range_end`: Int — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `scale_changed`: Int ×2

**命令.** 未发布命令。

**Rust API.** `create_timeline_axis(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("timeline_axis", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `gantt_timeline_axis`

### DependencyEditor (`dependency_editor`)

甘特图的可编辑前置/后继边，带新增/移除/选择信号、命中测试与连线几何。甘特图持有一个并可从任务同步依赖；编辑器持有边列表与校验。

**属性.** 除四个共享属性外共 2 项：`dependencies`: UInt (只读) · `editable`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `dependency_added`: String ×2 · `dependency_removed`: String ×2 · `link_selected`: String ×2

**命令.** 未发布命令。

**Rust API.** `create_dependency_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dependency_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `gantt_dependency_editor`

## 12. 代码与文档编辑器

本族汇集内容为结构化文本的编辑器：自带缓冲区的代码编辑器、带预览与标题的 Markdown 编辑器，以及用于导航的查找条与大纲面板。

带行号、标记、折叠与光标事件的源码文本用 `CodeEditor`；带预览分栏的散文用 `MarkdownEditor`；宿主想自建查找界面时用 `FindBar`；标题导航用 `MarkdownOutlinePanel`。不要用它们编辑单行值（`LineEdit`）或富格式文本（`RichEdit`），需要语法标记或行槽时也不要退回 `TextArea`。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `code_editor` | `src/widget/special_widgets/code_editor/` and `src/widget/special_widgets/markdown_editor.rs` | 5 |

### RichEdit (`code_editor`)

自带缓冲区的源码编辑器：行数、光标行列、标记、折叠、跳转条与断点槽，并报告光标、折叠、标记与搜索事件。用于源码文本；散文用 `MarkdownEditor`，格式文本用 `RichEdit`，追加输出用 `TerminalView`。

**属性.** 除四个共享属性外共 9 项：`text`: String · `line_count`: UInt (只读) · `cursor_line`: UInt (只读) · `cursor_column`: UInt (只读) · `marker_count`: UInt (只读) · `undo_stack_limit`: UInt · `undo_depth`: UInt (只读) · `goto_bar_visible`: Bool · `breakpoint_gutter`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `breakpoint_toggled`: mixed (String parts) · `completion_changed`: Bool · `cursor_moved`: UInt ×2 · `fold_changed`: UInt · `marker_activated`: UInt · `palette_changed` · `search_changed`: UInt · `search_result`: UInt ×4 · `selection_changed`: Rect or null · `tab_changed`: UInt · `text_changed`: String · `zoom_changed`: Float

**命令.** `set_text`, `append_line`, `set_markers`, `set_cursor`

**Rust API.** `create_code_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("code_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### RichEdit (`markdown_editor`)

带源码与预览模式、标题计数、光标跟踪与可交给 `MarkdownOutlinePanel` 显示的大纲的 Markdown 编辑器。用于 Markdown 文档；配 `FindBar` 做内联搜索；非 Markdown 格式用 `RichEdit`。

**属性.** 除四个共享属性外共 10 项：`text`: String · `preview_mode`: Bool · `line_count`: UInt (只读) · `word_count`: UInt (只读) · `heading_count`: UInt (只读) · `cursor_line`: UInt (只读) · `show_scrollbar`: Bool · `gfm_extensions`: Bool · `code_highlight`: Bool · `undo_stack_limit`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `heading_activated`: UInt · `preview_mode_changed`: Bool · `selection_changed`: Bool · `text_changed`: String

**命令.** `set_text`, `append_line`, `toggle_preview_mode`, `set_preview_mode`, `undo`

**Rust API.** `create_markdown_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("markdown_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `md_editor`

### FindBar (`find_bar`)

编辑器内联搜索条：查询、大小写、全词、替换文本、匹配数与当前范围，并报告搜索与替换请求。用于编辑器表面内；搜索必须模态时用 `FindReplaceDialog`。

**属性.** 除四个共享属性外共 7 项：`query`: String · `case_sensitive`: Bool · `whole_word`: Bool · `replace_text`: String · `replace_visible`: Bool · `match_count`: UInt (只读) · `current_range`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `replace_requested`: mixed (String parts) · `search_changed`: UInt ×2

**命令.** 未发布命令。

**Rust API.** `create_find_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("find_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `find_replace_bar`

### MarkdownOutlinePanel (`markdown_outline_panel`)

导航 Markdown 编辑器的标题大纲：条目、活动索引与宽度，发出 `entry_selected`。编辑器持有文档与 `outline()`；面板只列出标题，并通过 `goto_outline` 把所选索引报告回去。

**属性.** 除四个共享属性外共 4 项：`entries`: StringList · `active_index`: UInt · `width`: UInt · `entry_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `entry_selected`: UInt

**命令.** 未发布命令。

**Rust API.** `create_markdown_outline_panel(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("markdown_outline_panel", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `markdown_outline`

### TextEdit (`terminal_view`)

追加式终端表面：回滚输出、输入行、提示符、光标样式、ANSI 调色板与 `command_submitted` 信号。用于展示进程或命令控制台；可编辑文本用 `TextArea`，源码用 `CodeEditor`。

**属性.** 除四个共享属性外共 10 项：`output_line_count`: UInt (只读) · `input_line`: String · `scrollback_lines`: UInt · `wrap_output`: Bool · `show_scrollbar`: Bool · `caret_style`: Enum (`line` | `block` | `underline`) · `caret_blink_enabled`: Bool · `prompt`: String · `selection_mode`: Enum (`lines` | `characters`) · `ansi_palette`: NumberList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `command_submitted`: String · `completion_requested`: String · `interrupt_requested`: String · `output_received`: String · `resized`: UInt ×2 · `selection_changed`: Bool

**命令.** `append_output`, `submit`

**Rust API.** `create_terminal_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("terminal_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `terminal`

## 13. 行情数据控件

金融族是一套纵向整合的行情视图：带叠加指标的蜡烛图、对齐的成交量柱、深度曲线、订单簿阶梯、报价板与指标面板。它们共享序列/布局词汇，使各面板能对齐到同一价格-时间轴。

只有在处理市场或交易数据时才用金融控件：OHLC 柱用 `CandlestickChart`，其下对齐 `VolumeChart`，累计流动性用 `DepthChart`，实时阶梯用 `OrderBook`，自选用 `QuoteBoard`，MACD/RSI/KDJ 类面板用 `IndicatorChart`。不要把它们用于通用业务图表（图表族）；不需要实时更新时也不要喂历史快照——静态表格更省。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `finance` | `src/widget/chart_widgets/finance/` | 6 |

### CandlestickChart (`candlestick_chart`)

OHLC 蜡烛图，带叠加序列、价格轴、逐柱悬停/点击与视图变化。用作行情视图的主价格面板；其下对齐 `VolumeChart`，技术面板用 `IndicatorChart`。不要用于非 OHLC 业务数据。

**属性.** 除四个共享属性外共 4 项：`series`: String · `overlay_count`: UInt · `show_price_levels`: Bool · `show_overlay_legend`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `bar_unhovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`, `add_overlay`

**Rust API.** `create_candlestick_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("candlestick_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `candlestick`, `kline`, `k_line`, `k_line_chart`, `ohlc_chart`

### VolumeChart (`volume_chart`)

与蜡烛图柱对齐的成交量柱状图，支持按方向或统一着色与顶部留白。价格图持有时间轴；本面板读取同一序列并渲染成交量，因此宿主需要同步两个视口。

**属性.** 除四个共享属性外共 3 项：`series`: String · `color_mode`: Enum (`direction` | `uniform`) · `headroom`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`

**Rust API.** `create_volume_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("volume_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `volume`, `volume_histogram`, `volume_bars`

### DepthChart (`depth_chart`)

累计买卖深度曲线，深度可配置、买卖颜色分离，并报告指针下的价格档位。用于流动性；带挂单量的离散阶梯用 `OrderBook`。

**属性.** 除四个共享属性外共 3 项：`depth`: UInt · `bid_color`: Color · `ask_color`: Color — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_clicked`: Float · `level_hovered`: Float

**命令.** `set_series`

**Rust API.** `create_depth_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("depth_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `market_depth`, `depth_graph`, `liquidity_chart`

### OrderBook (`order_book`)

实时买卖阶梯，每档带挂单量、价差行与逐档悬停/点击。用于蜡烛图旁的市场微观结构；平滑累计曲线用 `DepthChart`，多标的自选用 `QuoteBoard`。

**属性.** 除四个共享属性外共 3 项：`depth`: UInt · `decimals`: UInt · `show_spread`: Bool (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_clicked`: mixed (String parts) · `level_hovered`: mixed (String parts) · `level_unhovered`: mixed (String parts)

**命令.** `set_series`

**Rust API.** `create_order_book(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("order_book", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `book_ladder`, `market_depth_ladder`

### QuoteBoard (`quote_board`)

标的表格，带最新价、涨跌、成交量与排序模式，并报告报价点击/悬停/选择。用作自选列表；单个标的的阶梯用 `OrderBook`，非行情数据用 `Table`。

**属性.** 除四个共享属性外共 4 项：`sort`: Enum (`none` | `symbol` | `last_descending` | `change_magnitude`) · `selected_index`: UInt · `row_height`: UInt (只读) · `scroll_offset`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `quote_clicked`: String · `quote_hovered`: String · `selection_changed`: String or null

**命令.** `set_series`

**Rust API.** `create_quote_board(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("quote_board", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `quotes`, `watchlist`, `quote_table`, `market_watch`

### IndicatorChart (`indicator_chart`)

技术指标面板：MACD、RSI、随机指标、资金流量、ATR 或能量潮，带周期与参考线。放在价格图下方；它消费同一序列而不拥有行情数据，因此宿主需要同时喂两个面板。

**属性.** 除四个共享属性外共 4 项：`series`: String · `mode`: Enum (`macd` | `rsi` | `stochastic` | `money_flow_index` | `atr` | `on_balance_volume`) · `period`: UInt · `show_reference_levels`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `bar_clicked`: UInt · `bar_hovered`: UInt · `view_changed`: UInt ×2

**命令.** `set_series`

**Rust API.** `create_indicator_chart(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("indicator_chart", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `indicator`, `technical_indicator`, `oscillator`, `macd_chart`

## 14. 日期、时间与高级选择

高级族覆盖较复杂的选择器：日历与日期/时间编辑器、旋钮、标签与功能区条、饼菜单，以及键盘快捷键编辑器。

以月历网格为主界面时用 `Calendar`，单个字段加弹层用 `DatePicker`/`TimePicker`/`DateTimePicker`，通过角度操纵有界数值用 `Dial`，与容器解耦的标签条用 `TabBar`，Office 风格命令标签用 `RibbonBar`，径向次级动作菜单用 `PieMenu`，录制快捷键用 `KeySequenceEdit`。普通 `LineEdit` 加校验就够的地方不要用日期编辑器；长列表也不要用 `PieMenu`——径向菜单只适合少量条目。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `advanced` | `src/widget/advanced_widgets/` | 10 |

### Calendar (`calendar`)

用于浏览与选择日期的月历网格，支持单选、多选或范围选择、周数、可配置首日与日期上下限。用作主日期表面；带弹层的字段用 `DatePicker`，带预设的起止对用 `DateRangePicker`。

**属性.** 除四个共享属性外共 12 项：`selected_date`: String · `minimum_date`: String · `maximum_date`: String · `first_day_of_week`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) · `grid_visible`: Bool · `navigation_bar_visible`: Bool · `horizontal_header_visible`: Bool · `vertical_header_visible`: Bool · `date_format`: String · `show_week_numbers`: Bool · `selection_mode`: Enum (`single` | `multi` | `range`) · `selected_dates`: StringList — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `activated`: String · `current_page_changed`: mixed (String parts) · `selection_changed`: String

**命令.** `set_selected_date`, `set_date_range`, `set_first_day_of_week`, `show_today`

**Rust API.** `create_calendar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("calendar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `date_calendar`

### DatePicker (`date_edit`)

单行日期字段，打开日历弹层，带显示格式、日期上下限与弹层/日期变化信号。用于表单中的一个日期；网格始终可见时用 `Calendar`，值包含时间时用 `DateTimePicker`。

**属性.** 除四个共享属性外共 7 项：`date`: String · `minimum_date`: String · `maximum_date`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `popup_visibility_changed`: Bool

**命令.** `set_date`, `set_minimum_date`, `set_maximum_date`, `set_display_format`

**Rust API.** `create_date_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("date_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `date_picker`

### TimePicker (`time_edit`)

单行时间字段，带时钟弹层、12/24 小时制、时间上下限与 `time_changed` 信号。用于一天中的时间；选择器风格的拼写用 `TimePicker`，还需要日期时用 `DateTimePicker`。

**属性.** 除四个共享属性外共 7 项：`time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("time_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `time_input`

### TimePicker (`time_picker`)

带时钟弹层的时间字段的 `time_picker` 工厂名；它与 `time_edit` 共享 `TimePicker` 类型与 `TIME_EDIT_PROPERTIES` schema。按宿主已有拼写选择其一，不要为同一个值同时挂载两者。

**属性.** 除四个共享属性外共 7 项：`time`: String · `minimum_time`: String · `maximum_time`: String · `display_format`: String · `clock_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `time_format`: Enum (`12h` | `24h`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_time`, `set_minimum_time`, `set_maximum_time`, `set_display_format`, `set_clock_popup`

**Rust API.** `create_time_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("time_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `time_field`

### DateTimePicker (`date_time_edit`)

组合日期时间字段，带日历弹层、显示格式、日期上下限，以及日期与日期时间两种变化信号。一个值同时包含两部分时使用；两者分开编辑时用 `DatePicker` 加 `TimePicker`。

**属性.** 除四个共享属性外共 7 项：`datetime`: String · `display_format`: String · `calendar_popup`: Bool · `alignment`: Enum (`left` | `centre` | `right`) · `minimum`: String · `maximum`: String · `calendar_first_day`: Enum (`mon` | `tue` | `wed` | `thu` | `fri` | `sat` | `sun`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `calendar_closed` · `calendar_opened` · `date_changed`: String · `datetime_changed`: String · `editing_finished` · `popup_visibility_changed`: Bool · `time_changed`: String

**命令.** `set_datetime`, `set_display_format`, `set_calendar_popup`

**Rust API.** `create_date_time_edit(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("date_time_edit", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `datetimepicker`, `date_time_picker`

### Dial (`dial`)

把角度映射为有界整数的旋钮，带刻度、目标刻度与回绕。用于角度自然成立的仪表式控制（音量、声像、表盘）；线性范围用 `Slider`，精确输入用 `SpinBox`。

**属性.** 除四个共享属性外共 8 项：`minimum`: Int · `maximum`: Int · `value`: Int · `single_step`: Int · `page_step`: Int · `notches_visible`: Bool · `notch_target`: Float · `wrapping`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `slider_pressed` · `slider_released` · `value_changed`: Int

**命令.** `set_range`, `set_value`, `set_wrapping`

**Rust API.** `create_dial(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("dial", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `knob`

### TabBar (`tab_bar`)

与页面解耦的独立标签条：标签数、当前索引、可关闭、可移动、最小/最大宽度与标签事件。标签条与页面容器分离时使用；控件应同时拥有两者时用 `TabWidget`。

**属性.** 除四个共享属性外共 7 项：`tab_count`: UInt (只读) · `current_index`: UInt · `closable`: Bool · `movable`: Bool · `tab_min_width`: UInt · `tab_max_width`: UInt · `direction`: Enum (`ltr` | `rtl`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt · `tab_bar_clicked`: UInt or null · `tab_close_requested`: UInt · `tab_moved`: UInt ×2

**命令.** `add_tab`, `remove_tab`, `set_current_index`, `set_closable`, `set_movable`

**Rust API.** `create_tab_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("tab_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### RibbonBar (`ribbon_bar`)

Office 风格功能区：标签、命令组、大小条目、快速访问条与折叠/最小化。用于命令密集的桌面应用；单行动作用 `ToolBar`，经典菜单用 `MenuBar`。

**属性.** 除四个共享属性外共 6 项：`tab_count`: UInt (只读) · `current_tab`: UInt · `expanded`: Bool · `minimized`: Bool · `quick_access_items`: StringList · `quick_access_overflow_menu`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `application_clicked` · `current_tab_changed`: UInt · `group_overflow_clicked`: UInt ×2 · `item_triggered`: UInt ×3 · `quick_access_clicked`: UInt · `quick_access_overflow_clicked`

**命令.** `add_tab`, `add_group`, `add_item`, `add_large_item`, `clear`

**Rust API.** `create_ribbon_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("ribbon_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ribbon`

### PieMenu (`pie_menu`)

径向菜单，条目围绕指针排成一圈，带外半径、内半径与当前索引。用于在某个点调用少量次级动作；更长或需要键盘导航的列表用 `Menu`/`ContextMenu`。

**属性.** 除四个共享属性外共 4 项：`item_count`: UInt (只读) · `radius`: Float · `inner_radius`: Float · `current_index`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `about_to_hide` · `about_to_show` · `triggered`: UInt · `triggered_text`: String

**命令.** `add_item`, `remove_item`, `set_radius`, `set_current_index`

**Rust API.** `create_pie_menu(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("pie_menu", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `radial_menu`

### LineEdit (`key_sequence_editor`)

把键盘快捷键录制为按键序列的字段，报告冲突与新序列。该 schema 没有发布属性行，因此应通过控件 API 与信号驱动，而不是通过 `WidgetFactory` 的属性名。

**属性.** 该 schema 没有自己的属性；每个控件都带四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `conflict_detected`: String · `editing_finished` · `key_sequence_changed`: String · `sequence_changed`: list of String

**命令.** 未发布命令。

**Rust API.** `create_key_sequence_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("key_sequence_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `key_sequence_edit`

## 15. 工具与移动控件

杂项族收集不需要独立家族的小控件：头像、二维码、拖放目标、日期范围、分段按钮、条码扫描、曲线编辑与范围预设。

用户头像（带首字母回退）用 `Avatar`，渲染确定性二维码用 `QRCode`，按 MIME 类型过滤的命名拖放目标用 `DropZone`，起止日期用 `DateRangePicker`，多选分段用 `SegmentedButton`，摄像头扫描用 `BarcodeScanner`，缓动曲线用 `BezierCurveEditor`，范围选择器旁用 `RangePresetBar`。不要把 `DropZone` 当通用拖拽源（应实现 `DragSource`），也不要用 `QRCode` 解码条码——它只负责渲染。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `misc` | `src/widget/misc_widgets/` | 9 |

### DropZone (`drop_zone`)

按 MIME 类型过滤的命名拖放目标，带悬停状态、多文件标志与进入/离开/放下/拒绝信号。宿主接受文件或拖拽载荷时使用；目标本身是已有控件而不是独立区域时，直接实现 `DropTarget`。

**属性.** 除四个共享属性外共 8 项：`accepted_type`: String · `state`: Enum (`idle` | `hovering` | `accepted` | `rejected` | `dropped`) · `hovered`: Bool (只读) · `accepted_types`: StringList · `multi_file`: Bool · `hover_style`: String · `show_count_badge`: Bool · `dropped_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `drop_rejected` · `dropped`: list of String · `payload_dropped`: String · `zone_entered`: list of String · `zone_exited`: list of String

**命令.** 未发布命令。

**Rust API.** `create_drop_zone(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("drop_zone", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `drop_target`

### MobileDatePicker (`mobile_date_picker`)

为触屏而非桌面日历网格布局的日期选择器，带年份跨度与日期变化信号。用于手机布局；桌面端用 `Calendar` 或 `DatePicker`，需要 iOS 滚轮时用 `CupertinoDatePicker`。

**属性.** 除四个共享属性外共 2 项：`selected_date`: String · `year_span`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `date_changed`: String

**命令.** `set_selected_date`

**Rust API.** `create_mobile_date_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mobile_date_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### QRCode (`qr_code`)

从数据字符串渲染确定性 QR 图案的控件，带尺寸、静区、纠错级别、版本边界与颜色。用于显示供用户扫描的码；它只渲染不解码，读取用 `BarcodeScanner`。

**属性.** 除四个共享属性外共 8 项：`data`: String · `size`: UInt · `quiet_zone`: UInt · `ec_level`: Enum (`L` | `M` | `Q` | `H`) · `min_version`: UInt · `max_version`: UInt · `foreground_color`: Color · `background_color`: Color — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** 该控件不发布任何 capability 事件。

**命令.** `set_data`, `set_size`

**Rust API.** `create_qr_code(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("qr_code", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `qr`

### Avatar (`avatar`)

圆形或方形用户图像占位，带首字母回退、尺寸、可选徽标、图像来源与圆环。用于个人或账户身份；只有在计数属于别处时才额外配 `Badge`——头像本身已经有 `badge` 文本。

**属性.** 除四个共享属性外共 7 项：`initials`: String · `square`: Bool · `size`: UInt · `image_source`: String · `badge`: String · `ring_color`: Color · `ring_width`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `avatar_clicked`

**命令.** `set_initials`, `set_image_source`

**Rust API.** `create_avatar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("avatar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `avatar_widget`, `user_avatar`

### SegmentedButton (`segmented_button`)

带 id 与图标、支持单选或多选与组 id 的横向可选分段组。用于 Material 3 风格图标分段或多选；单选、带标签、指示器滑动时用 `SegmentedControl`，iOS 胶囊外观用 `CupertinoSegmentedControl`。

**属性.** 除四个共享属性外共 5 项：`selected_index`: UInt · `selected_indices`: String · `allows_multiple`: Bool · `segment_count`: UInt (只读) · `group_id`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_segmented_button(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("segmented_button", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `segmented_btn`

### DateRangePicker (`date_range_picker`)

基于日历的起止范围选择器，带预设、双月视图与最少/最多夜数。用于预订或报表区间；预设需要放在选择器之外时配 `RangePresetBar`，单个日期用 `Calendar`。

**属性.** 除四个共享属性外共 6 项：`start_date`: String · `end_date`: String · `presets`: StringList · `double_month`: Bool · `min_nights`: UInt · `max_nights`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `preset_selected`: UInt · `range_changed`: String

**命令.** `set_start_date`, `set_end_date`

**Rust API.** `create_date_range_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("date_range_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `range_picker`

### BarcodeScanner (`barcode_scanner`)

扫描取景器，带扫描标志与 `barcode_detected` 载荷。摄像头与解码由宿主负责；本控件持有扫描状态并报告解码文本。`QRCode` 用于渲染码，不用于读取。

**属性.** 除四个共享属性外共 1 项：`is_scanning`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `barcode_detected`: String

**命令.** `set_is_scanning`

**Rust API.** `create_barcode_scanner(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("barcode_scanner", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `scanner`

### BezierCurveEditor (`bezier_curve_editor`)

用于自定义缓动曲线的交互式三次 Bézier 编辑器：控制点、段数、吸附与曲线变化载荷。用于运动或动画设置面板；保存发出的曲线文本并交给动画引擎，而不是重新计算。

**属性.** 除四个共享属性外共 3 项：`snap_to_grid`: Bool · `segments`: UInt (只读) · `curve_points`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `curve_changed`: mixed (String parts)

**命令.** `set_snap_to_grid`

**Rust API.** `create_bezier_curve_editor(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("bezier_curve_editor", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `curve_editor`

### RangePresetBar (`range_preset_bar`)

范围选择器旁的一行快速日期区间 chip，带方向、紧凑模式与选中标签。日期由选择器持有；该条只报告所选预设，因此把 `preset_selected` 接到 `DateRangePicker::apply_preset` 或你自己的映射。

**属性.** 除四个共享属性外共 4 项：`presets`: StringList · `orientation`: Enum (`horizontal` | `vertical`) · `compact`: Bool · `selected_label`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `preset_selected`: String ×2

**命令.** 未发布命令。

**Rust API.** `create_range_preset_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("range_preset_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `preset_bar`

## 16. 播放与动画

媒体族渲染随时间变化的内容：逐帧动画与 Lottie/Rive 动画、Hero 转场、视频播放、音频电平与相机预览。

逐帧序列用 `AnimatedImage`，矢量动画资源用 `LottieWidget` 或 `RiveWidget`，两个布局间的共享元素转场用 `HeroAnimation`，带传输状态的视频用 `VideoPlayer`，电平或频谱读数用 `AudioVisualizer`，取景器用 `CameraPreview`。交互式动画图（Rive）不要用 `AnimatedImage`；纯音频播放也不要用 `VideoPlayer`——它的画面与帧信号都白费。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `media` | `src/widget/media_widgets/` | 7 |

### AnimatedImage (`animated_image`)

播放动图（GIF、APNG 或 WebP 帧序列），支持播放/暂停、方向、帧缓存上限与帧/动画完成信号。用于逐帧序列资源；矢量动画用 `LottieWidget` 或 `RiveWidget`，定时媒体用 `VideoPlayer`。

**属性.** 除四个共享属性外共 3 项：`playing`: Bool · `direction`: Enum (`forward` | `reverse`) · `frame_cache_limit`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished` · `frame_changed`: UInt

**命令.** `set_playing`

**Rust API.** `create_animated_image(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("animated_image", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `anim_image`

### HeroAnimation (`hero_animation`)

在两个布局之间插值位置、尺寸与透明度的共享元素转场，带进度值与完成信号。同一对象出现在两个屏幕时使用；宿主驱动 `animation_progress`，结束时换回真实内容。

**属性.** 除四个共享属性外共 1 项：`animation_progress`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_completed`

**命令.** `set_animation_progress`

**Rust API.** `create_hero_animation(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("hero_animation", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `hero`

### LottieWidget (`lottie_widget`)

Lottie JSON 动画播放器，支持播放/暂停、循环次数、方向与速度。用于设计工具导出的矢量动效资源；动画需要交互式状态机与输入时用 `RiveWidget`。

**属性.** 除四个共享属性外共 5 项：`playing`: Bool · `loop_count`: Int · `loops_completed`: Int (只读) · `direction`: Enum (`forward` | `reverse`) · `speed`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished`

**命令.** `set_playing`

**Rust API.** `create_lottie_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("lottie_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `lottie`

### RiveWidget (`rive_widget`)

Rive 运行时表面，带播放状态、循环、状态字符串与输入触发/状态变化信号。用于带状态机的交互动画；线性导出动画用 `LottieWidget`，逐帧序列用 `AnimatedImage`。

**属性.** 除四个共享属性外共 4 项：`is_playing`: Bool · `loop_count`: Int · `loops_completed`: Int (只读) · `state`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `animation_finished` · `input_fired`: String · `state_changed`: String

**命令.** `set_is_playing`

**Rust API.** `create_rive_widget(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("rive_widget", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `rive`

### VideoPlayer (`video_player`)

视频表面，带播放状态、音量、静音、播放速率、控件、循环与帧/元数据/错误信号集。宿主提供媒体管线时使用；配传输配套控件，并且只通过控件自身方法停止/定位，以保持其时钟权威。

**属性.** 除四个共享属性外共 7 项：`is_playing`: Bool · `volume`: Float · `muted`: Bool · `playback_rate`: Float · `controls_visible`: Bool · `looping`: Bool · `pipeline`: String (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `buffering_progress_changed`: Float · `duration_changed`: Float · `error_occurred`: String · `frame_available`: UInt · `frame_presented`: Float · `fullscreen_changed`: Bool · `media_status_changed`: String · `metadata_changed` · `mute_changed`: Bool · `playback_ended` · `playback_paused` · `playback_rate_changed`: Float · `playback_started` · `time_updated`: Float · `volume_changed`: Float

**命令.** `set_is_playing`, `set_volume`

**Rust API.** `create_video_player(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("video_player", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `video`

### AudioVisualizer (`audio_visualizer`)

实时音频电平或频谱视图，带柱数、峰值阈值、模式与周期性电平报告。用于音频播放时的反馈；分析由宿主完成并喂入电平；不要把它当音频编辑器。

**属性.** 除四个共享属性外共 4 项：`bar_count`: UInt · `peak_threshold`: Float · `mode`: Enum (`waveform` | `spectrum`) · `level_report_interval`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `level_changed`: Float · `mode_changed`: String · `peak_exceeded`: Float

**命令.** `set_bar_count`

**Rust API.** `create_audio_visualizer(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("audio_visualizer", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `audio_viz`

### CameraPreview (`camera_preview`)

摄像头取景器，带活动标志、像素格式、目标帧率与摄像头 id，并报告帧、格式协商、权限拒绝与错误。用于拍摄或扫描；设备会话由宿主持有，控件只呈现会话报告的事实。

**属性.** 除四个共享属性外共 4 项：`is_active`: Bool · `pixel_format`: String · `target_fps`: UInt · `camera_id`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `error_occurred`: String · `format_negotiated`: UInt ×2 · `frame_available`: UInt · `frame_presented`: Float · `permission_denied`

**命令.** `set_is_active`

**Rust API.** `create_camera_preview(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("camera_preview", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `camera`

## 17. 嵌入式 Web 内容

Web 族只暴露一个控件 `WebEngineView`，以及定位同一视图不同部分的命名包装类型（`WebView`、`WebEnginePage`、`WebEngineSettings`、`WebEngineContextMenuRequest`、`WebEngineCookieStore`、`WebEngineDownloadItem`、`WebEngineFindTextResult`、`WebEngineNotification`、`WebEngineScriptDialog`、`WebEngineWebChannel`）。

宿主必须渲染自己不拥有的 HTML 时才用 `WebEngineView`——帮助页、第三方登录、嵌入仪表盘。crate 自己能渲染的内容不要用它：富文本 `Label`、`Canvas` 或原生视图可以避免引擎的进程、内存与安全成本。把导航、下载与脚本对话框当作宿主必须应答的事件。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `web` | `src/widget/web_widgets/` | 1 |

### WebEngineView (`web_engine_view`)

内嵌 Web 视图：URL、加载状态、标题、前进/后退、用户代理、透明背景与 Cookie 持久化，并带导航、加载、控制台、下载、证书与崩溃信号。仅用于宿主不拥有的 HTML；把每个信号都当作宿主必须决定的事项。

**属性.** 除四个共享属性外共 8 项：`url`: String · `loading`: Bool (只读) · `title`: String · `can_go_back`: Bool · `can_go_forward`: Bool · `user_agent`: String · `background_transparent`: Bool · `cookie_persistence_enabled`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `certificate_error`: String · `channel_message`: String ×2 · `console_message`: mixed (String parts) · `download_requested`: String · `error_occurred`: String · `js_evaluated`: mixed (String parts) · `link_hovered`: String · `load_progress_changed`: UInt · `loading_finished`: String · `loading_started`: String · `navigation_state_changed`: Bool ×2 · `page_created`: UInt · `page_destroyed`: UInt · `render_process_crashed`: String · `title_changed`: String · `url_changed`: String

**命令.** `set_url`, `load_url`, `reload`, `go_back`, `go_forward`, `stop`

**Rust API.** `create_web_view(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("web_engine_view", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `webview`, `web_view`

## 18. Cupertino 与 Material 呈现

本族提供平台风格的呈现：iOS 风格的开关、警告框、滑块、导航栏、分段控件与日期选择器，以及补全自适应组合的 Material snackbar 与导航栏。

界面必须在 Apple 平台上看原生、且宿主已通过运行时查询确定平台时，选 Cupertino 控件。需要 iOS 拇指与轨道外观时用 `CupertinoSwitch` 而不是 `Switch`；它有自己的类型、capability 行与构造函数，不是别名。不要在共享代码里按目标 OS 分支——运行时询问平台层——也不要在同一界面混用 Cupertino 与 Material 外壳。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `cupertino` | `src/widget/cupertino/` | 8 |

### CupertinoSwitch (`cupertino_switch`)

iOS 风格开关，拥有自己的类型、capability 行与构造函数；它不是 `Switch` 的别名。需要 iOS 拇指与轨道呈现时使用；只有几何不同时用 `Switch` 并设置 `switch_geometry`。

**属性.** 除四个共享属性外共 4 项：`checked`: Bool · `thumb_color`: Color · `on_track_color`: Color · `switch_geometry`: Enum (`material` | `cupertino` | `macos`) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `toggled`: Bool

**命令.** `set_checked`, `toggle`

**Rust API.** `create_cupertino_switch(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_switch", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ios_switch`

### CupertinoAlertDialog (`cupertino_alert_dialog`)

iOS 风格模态警告框，带标题、消息与确认/取消结果。用于二选一的 iOS 警告；桌面警告族用 `AlertDialog`/`ConfirmDialog`，经典固定按钮形式用 `MessageBox`。

**属性.** 除四个共享属性外共 2 项：`title`: String · `message`: String — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `canceled` · `confirmed`

**命令.** `set_title`, `set_message`

**Rust API.** `create_cupertino_alert_dialog(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_alert_dialog", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ios_alert`

### CupertinoSlider (`cupertino_slider`)

iOS 风格滑块，圆角轨道与圆形把手，带浮点值与上下限。用于 iOS 风格界面上的连续值；整数步进用 `Slider`，双边界用 `RangeSlider`。

**属性.** 除四个共享属性外共 3 项：`value`: Float · `min`: Float · `max`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Float

**命令.** `set_value`, `set_min`, `set_max`

**Rust API.** `create_cupertino_slider(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_slider", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ios_slider`

### CupertinoNavigationBar (`cupertino_navigation_bar`)

iOS 风格导航栏，带标题、可选大标题、提示、前后置条目与折叠偏移。用于 iOS 屏幕顶部；标题需要跟随页面栈时配 `CupertinoNavigationStack` 配套控件。

**属性.** 除四个共享属性外共 4 项：`title`: String · `large_title`: Bool · `prompt`: String · `collapse_offset`: Float — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `back_pressed` · `leading_item_pressed`: UInt · `trailing_item_pressed`: UInt

**命令.** `set_title`, `set_large_title`

**Rust API.** `create_cupertino_navigation_bar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_navigation_bar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `cupertinonavbar`, `ios_nav_bar`

### CupertinoSegmentedControl (`cupertino_segmented_control`)

iOS 风格胶囊分段控件，带选中索引与值变化信号。用于 iOS 屏幕上两到五个互斥选择；桌面呈现用 `SegmentedControl`，多选用 `SegmentedButton`。

**属性.** 除四个共享属性外共 2 项：`selected_index`: UInt · `segment_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_cupertino_segmented_control(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_segmented_control", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `cupertinosegmented`, `ios_segmented`

### CupertinoDatePicker (`cupertino_date_picker`)

iOS UIPickerView 风格滚动轮日期选择器，支持日期、时间、日期时间或倒计时模式、24 小时制与分钟间隔。用于 iOS 风格面板内；Material 触屏布局用 `MobileDatePicker`，桌面用 `DatePicker`。

**属性.** 除四个共享属性外共 4 项：`selected_date`: String · `mode`: Enum (`date` | `time` | `datetime` | `countdown`) · `use_24_hour`: Bool · `minute_interval`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `date_changed`: mixed (String parts) · `time_changed`: UInt ×2

**命令.** `set_selected_date`

**Rust API.** `create_cupertino_date_picker(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("cupertino_date_picker", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `ios_date_picker`

### MaterialSnackbar (`material_snackbar`)

Material snackbar，带消息、动作标签、持续时间与滑入进度。用于带一个动作的简短确认；持久通知用 `Banner`，多条消息需要排队时用 `ToastStack`/`ToastHost`。

**属性.** 除四个共享属性外共 4 项：`message`: String · `action_text`: String · `duration_ms`: UInt · `slide_progress`: Float (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `action_pressed` · `dismissed`

**命令.** `show`, `dismiss`, `set_message`, `set_action_text`

**Rust API.** `create_material_snackbar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("material_snackbar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### MaterialNavigationRail (`material_navigation_rail`)

平板宽度布局的 Material 侧导航栏，带选中索引与当前变化信号。窗口太窄放不下抽屉、又太宽不适合底部栏时使用；长目的地列表用 `NavigationDrawer`，手机用 `BottomNavigationBar`。

**属性.** 除四个共享属性外共 1 项：`selected_index`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `current_changed`: UInt

**命令.** `set_selected_index`

**Rust API.** `create_material_navigation_rail(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("material_navigation_rail", geometry, text)`；注册表用 `WidgetFactory::shared()`。

**别名.** `nav_rail`, `navigation_rail`

## 19. macOS 呈现

macOS 族提供显式命名、原生外观的 macOS 外壳控件：分段控件、带重复的步进器、带溢出的统一工具栏、源列表与窗口外壳条。它们各自报告已有的 `WidgetKind`，但通过自己的工厂名访问，由宿主显式选择。

macOS 界面需要匹配平台度量与交互习惯时选它们。不要在其他平台默认使用，也不要假设它们与通用对应物可互换：属性表不同（`mac_stepper` 多了重复延迟，`mac_toolbar` 多了 `selectable`，`mac_window_chrome` 多了 `document_edited`）。

| 控件族 | 源码目录 | 控件数 |
|---|---|---|
| `macos` | `src/widget/macos/` | 5 |

### SegmentedControl (`mac_segmented_control`)

分段控件的 macOS 呈现：选中索引、分段数与值变化信号，采用平台度量。用于 macOS 原生外壳；通用控件用 `SegmentedControl`，iOS 用 `CupertinoSegmentedControl`。

**属性.** 除四个共享属性外共 2 项：`selected_index`: UInt · `segment_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: UInt

**命令.** 未发布命令。

**Rust API.** `create_mac_segmented_control(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mac_segmented_control", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Stepper (`mac_stepper`)

步进器的 macOS 呈现，在通用值/范围/步长模型上增加 `wraps`、`repeat_delay_ms` 与 `repeat_interval_ms`。用于 macOS 原生数字外壳；可移植控件用 `Stepper`，需要键入时用 `SpinBox`。

**属性.** 除四个共享属性外共 7 项：`value`: Int · `minimum`: Int · `maximum`: Int · `step`: Int · `wraps`: Bool · `repeat_delay_ms`: UInt · `repeat_interval_ms`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `value_changed`: Int

**命令.** 未发布命令。

**Rust API.** `create_mac_stepper(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mac_stepper", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ToolBar (`mac_toolbar`)

macOS 统一样式的工具栏呈现：条目、图标尺寸、方向、可选/选中条目与溢出数。用于 macOS 窗口顶部；可移植条用 `ToolBar`，命令需要带标签分组时用 `RibbonBar`。

**属性.** 除四个共享属性外共 6 项：`item_count`: UInt (只读) · `icon_size`: Float · `orientation`: String (只读) · `selectable`: Bool · `selected_index`: UInt · `overflow_count`: UInt (只读) — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `item_activated`: UInt · `overflow_activated`: UInt · `overflow_changed`: UInt

**命令.** 未发布命令。

**Rust API.** `create_mac_toolbar(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mac_toolbar", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### ListView (`mac_source_list`)

列表的 macOS 源列表呈现：分区、行、选中行、行高与徽标/选择信号。用于 macOS 侧栏；可移植列表用 `ListView`，使用抽屉时配 `DrawerSectionHeader`。

**属性.** 除四个共享属性外共 4 项：`section_count`: UInt (只读) · `row_count`: UInt (只读) · `row_height`: UInt (只读) · `selected_row`: UInt — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `badge_changed`: mixed (String parts) · `selection_changed`: String

**命令.** 未发布命令。

**Rust API.** `create_mac_source_list(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mac_source_list", geometry, text)`；注册表用 `WidgetFactory::shared()`。

### Window (`mac_window_chrome`)

macOS 窗口外壳：标题、外壳高度、交通灯按钮、拖拽/全屏/文档已编辑标志与窗口生命周期请求。macOS 窗口需要匹配平台标题栏时使用；可移植窗口用 `Window`。

**属性.** 除四个共享属性外共 7 项：`title`: String · `chrome_height`: UInt (只读) · `buttons_visible`: Bool · `draggable`: Bool · `fullscreen`: Bool · `document_edited`: Bool · `focused`: Bool — 外加四个共享属性 `enabled`、`visible`、`tooltip`、`geometry`。

**事件.** `close_requested` · `fullscreen_requested` · `minimize_requested` · `window_focus_changed`: Bool · `zoom_requested`

**命令.** 未发布命令。

**Rust API.** `create_mac_window_chrome(geometry, text)` —— 注册的构造函数。通过工厂：`WidgetFactory::create("mac_window_chrome", geometry, text)`；注册表用 `WidgetFactory::shared()`。

## 复合控件：状态归谁

有些控件是设计成与伙伴一起使用的，而不是单独使用。下表说明每一对中哪一侧持有状态、哪一侧负责绘制，以及把它们接起来的调用。相同信息也出现在相关控件条目中；这里集中列出，方便宿主在接线整屏时一次看全。

| 主控件 | 配套控件 | 状态归属 | 绘制方与接线方式 |
|---|---|---|---|
| `ColorWell` | `ColorPicker`, `ColorDialog` | 色块持有当前颜色；选择器持有编辑状态。 | `picker_requested` 打开选择器或对话框；接受选择后由 `color_selected` 调用 `ColorWell::set_color`。 |
| `ColorDialog` | `Swatch` | 对话框持有当前颜色；色格持有调色板与选中索引。 | `swatch_selected` 通过 `set_hex` 写入对话框颜色；`Swatch::set_selected_index` 反映别处选中的颜色。 |
| `ColorPicker` | `ColorHistory` | 选择器持有当前颜色；历史持有最近颜色列表。 | 提交时把颜色加入历史；历史的 `color_selected` 设置选择器颜色。 |
| `DataGrid` | `FilterRow` | 网格持有模型与已应用筛选；行持有条件编辑器。 | `filter_changed` 携带条件文本；网格重新查询，并同步行的 `live` 标志。 |
| `GridTable` | `GridViewHeader` | 网格持有行列与滚动；表头持有排序/筛选入口与区段宽度。 | 把 `section_clicked` 转给 `toggle_sort_column`，`section_resized` 转给 `set_column_width`，`filter_clicked` 转给网格筛选状态。 |
| `GridTable` | `GridFindBar`, `ColumnChooser` | 网格持有数据与选择；查找条持有查询；列选择器持有列可见性。 | 查找条扫描网格可见列；列选择器的开关调用网格的列可见性设置方法。 |
| `LineChart`, `BarChart`, `PieChart` | `Legend` | 图表持有序列与可见性标志；图例持有条目顺序与高亮。 | `series_toggled` 调用图表的序列可见性设置方法；图例根据图表的序列名重建条目。 |
| any chart | `ChartTooltip` | 图表持有数据与几何；工具提示只持有文本行与可见性。 | 悬停时图表用吸附点调用 `show_at`，离开时调用 `hide`，并用悬停数据填充工具提示的行。 |
| any control | `TooltipManager` | 控件持有自己的 `tooltip` 文本；管理器持有延迟、持续时间与队列。 | 把悬停进入/离开喂给管理器；它在 `delay_ms` 后显示 `Tooltip`，并在 `duration_ms` 后隐藏。 |
| `ImageView` | `ZoomPanController` | 图像视图持有像素与绘制；控制器持有缩放、平移与 ROI。 | `zoom_changed` 调用 `ImageView::set_zoom_level`，`pan_changed` 调用 `set_pan_offset`，`roi_changed` 驱动裁剪或详情视图。 |
| `ImageGallery` | `ThumbnailCache` | 画廊持有当前索引与缩略图条；缓存持有降采样位图。 | 画廊按索引向缓存索取缩略图并绘制；缓存负责淘汰，从不绘制。 |
| `MapView` | `TileLoader`, `ZoomControl`, `ScaleBar`, `Compass` | 地图持有四个配套控件（访问器 `tile_loader()`、`zoom_control()`、`scale_bar()`、`compass()`）；瓦片抓取由宿主持有。 | `TileLoader::load_tile` 产生 `TileRequest`；宿主抓取后调用 `MapView::deliver_tile` 或 `reject_tile`；`zoom_requested` 与 `reset_north_requested` 由地图应用。 |
| `GanttWidget` | `TimelineAxis`, `DependencyEditor` | 甘特图持有两个配套控件（`timeline_axis()`、`dependency_editor()`）与任务列表。 | `set_dependencies` / `sync_dependencies_from_tasks` 保持编辑器与条形一致；`pixel_to_date` 与 `date_to_pixel` 来自轴。 |
| `DiffViewer` | `DiffFileList` | 查看器持有两段文本、差异与选择；文件列表只持有选中的路径。 | `file_selected` 告诉宿主读取哪个路径；随后宿主调用 `DiffViewer::set_texts`。 |
| `MarkdownEditor` | `MarkdownOutlinePanel`, `FindBar` | 编辑器持有文档、标题与光标；面板只持有活动大纲行；查找条持有查询。 | 用 `outline()` 构建面板；`entry_selected` 调用 `goto_outline`；`search_changed` 调用 `find_next` / `find_previous`。 |
| `KanbanBoard` | `KanbanCardView`, `KanbanColumnHeader` | 看板持有列、卡片与拖放；卡片视图持有一张卡片的内容；表头持有自己的重命名与折叠状态。 | 从 `columns()`/`cards` 构建行；`column_renamed` 调用看板的列修改方法；`card_edited` 通过看板写回，以免丢失拖拽状态。 |
| `DateRangePicker` | `RangePresetBar` | 选择器持有起止日期；预设条持有预设标签与选中 chip。 | `preset_selected` 映射到 `DateRangePicker::apply_preset`（或你自己的日期计算）；`range_changed` 更新预设条的选中标签。 |
| `FAB` | `SpeedDial` | FAB 持有折叠的主操作；快速拨号持有展开的次级动作。 | FAB 的 `clicked` 切换快速拨号；每个动作发出自己的意图，触发后由宿主收起拨号。 |
| `Button`, `ToggleButton`, `ToolButton` | `MenuPanel` | 按钮持有按下/选中状态与 `menu_open` 标志；面板持有条目列表与高亮行。 | 设置 `menu_open`，把面板锚定在按钮下方，并把 `item_clicked` 转给命令分发器；面板在 `cancelled` 时自行关闭。 |
| `SwipeToDismiss` | `SwipeActions` | 容器持有滑动偏移与已关闭状态；动作持有背景按钮及其颜色。 | 容器报告露出的侧与偏移；动作绘制该条带，并在被点击时发出自己的意图。 |
| `NavigationDrawer` | `DrawerSectionHeader` | 抽屉持有打开状态、选择与宽度；分区标题是不可选择的标签行。 | 在条目行之间插入标题；它们不报告选择，因此键盘导航会跳过。 |
| `Canvas` | `LayerPanel` | 画布持有图层及其顺序；面板是按图层 id 寻址的只读镜像。 | 面板读取图层 id 与名称；点击请求画布选中该 id，图层顺序的唯一写入者仍是画布。 |
| `ScrollArea` | `StickyHeader` | 滚动区域持有视口与滚动偏移；粘性标题持有它在条带中绘制的内容。 | 用 `ScrollArea::set_sticky_header` 挂载；区域偏移子控件，但把标题保持在条带处。 |
| `VideoPlayer` | `MediaTransportBar`, `MediaScrubber`, `SubtitleBar` | 播放器持有时钟、来源与播放状态；配套控件持有自己的控件并绘制自己的条带。 | 按钮调用 `play`、`pause`、`seek_to`、`set_volume`；`time_updated` 与 `duration_changed` 驱动进度条与字幕时序。 |
| `WebEngineView` | `WebFindBar` | 视图持有文档与导航状态；查找条持有查询与匹配数。 | `search_changed` 驱动视图的查找操作；视图把匹配数报告回查找条。 |
| `CupertinoNavigationBar` | `CupertinoNavigationStack` | 导航栏持有标题/提示呈现；页面栈持有页面列表与返回历史。 | `stack_changed` 更新导航栏标题与返回可见性；`back_pressed` 调用页面栈的 `pop`。 |
| `RichEdit` | `FormatToolbar` | 编辑器持有文档与选择；工具栏持有格式动作集及其启用状态。 | `FormatAction` 应用到当前选择；工具栏状态跟随选择的格式，而不是反过来。 |
| `ComboBox`, `AutoCompleteEdit` | `SuggestionPopup` | 字段持有文本与已提交值；弹层持有候选列表与高亮索引。 | 字段从自己的模型填充弹层候选；`suggestion_selected` 把所选值写回字段。 |

## WidgetKind 覆盖表

`src/widget/kind.rs` 中的枚举声明了 221 个变体。下面列出每一个及其所属族。有 11 个变体没有自己的 capability 行：它们是别名或子项类型，条目中说明了它们如何解析。

| 变体 | 记录于 |
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

## 有意省略与边界

* **Profile 门控。** 大多数变体声明在 `#[cfg(widgets_unstripped)]` 下。精简（`mini` 或 `embedded`）构建只保留始终可用的子集，因此面向此类构建的宿主必须确认名称可构造，而不能假设本章的每个控件都存在。

* **`key_sequence_editor` 不发布属性行。** 除四个共享属性外，它的 schema 为空。条目如实说明，而不是编造名称；通过构造函数与信号驱动它。

* **别名与子项类型。** `Panel`、`DockPanel`、`MenuItem`、`ContextMenu`、`CheckListBox`、`DoubleSpinBox`、`Wizard`、`DirectoryDialog`、`ActivityIndicator`、`ColumnView`、`UndoView` 没有自己的 capability 行。它们在所属族中记录，并说明解析到的名称。

* **配套控件是控件，不是主控件的零件。** 配套控件有自己的构造函数、信号，并且（除非另有说明）有自己的 capability 行；配对表说明谁持有哪部分状态。绘制总是由拥有该几何的控件完成。

* **名称自解释的属性不做转述。** 名为 `row_count`、类型为 `UInt` 的属性就是行数；为 700 个名称重复这句话会淹没真正需要解释的名称。不自解释的名称会在控件条目中注释。

* **本章不编译代码片段。** 它们是使用真实构造函数与工厂形状的片段；`mdbook test` 要求每段都是完整程序。`tools/check_cookbook.sh` 校验标识符，并以零警告构建三本书。
