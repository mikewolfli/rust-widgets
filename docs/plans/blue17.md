# BLUE17 — 控件补全：缺口目录 + 判定与执行计划

> 状态：**已完成**；现状取证（§二）**已完成并逐条实跑**
> 完成率：**取证 100% · Phase A 100% · Phase B 100% · Phase C 100% · Phase D 100% · Phase E 100% · Phase F 100%**
> 执行日志：[`docs/log/log-20260917-3.md`](../log/log-20260917-3.md)（第 28 轮主体执行）
> 复核日志：[`docs/log/log-20260917-4.md`](../log/log-20260917-4.md)（第 29 轮独立复核 + B1-2/B1-4 补完 + `PagerPageView`/`TileView` 删除，含 6 次反向注入）
> 原则依据：[`docs/plans/principle.md`](principle.md)（继承 BLUE1–BLUE16 全部规则，含 #1–#77）
> 上轮计划：[`docs/plans/blue16.md`](blue16.md)（Phase F 已 100%）
> 目标基线：`rust_widgets v2.2.0`（`WidgetKind` **171** 个、门禁脚本 **28** 个）
>
> 📌 **第 29 轮更新**：`PagerPageView` / `TileView` 已按用户要求**彻底删除**（全链路：
> 文件 / kind / 工厂 / 属性 / CSS / JSON / a11y / 文档），故 `WidgetKind` 由 **171 → 169**。
> 同时本轮新增 2 个门禁（**28 → 30**）。证据见 [`log-20260917-4.md`](../log/log-20260917-4.md) §2。
>
> 🔄 **第 30 轮校正（2026-09-18，规则 #18/#91「文档不得与实现不符」）**：
> 第 29 轮的追加轮又新增了金融控件族（6 个 kind），`WidgetKind` 最终为 **175**，
> 门禁为 **32** 个可运行单元。本文档 §六 Phase E/F 的两处**完成率表格**当时写的是
> **171/171** 与 **169**，已过期且与实跑不符；现已按实跑输出校正为 **175/175** 与 **175**。
> 实跑证据（第 30 轮）：
> ```
> $ bash tools/check_widget_registration_fidelity.sh | tail -1
> ✅ widget registration fidelity: 175 kinds classified (...), 175/175 constructible
> $ bash tools/check_widget_kind_count.sh | tail -2
> WidgetKind variants (parsed from src/widget/kind.rs): 175
> ✅ check_widget_kind_count: documented variant counts match the enum
> ```
> 另修正一处**计划与实现的表述差异**（非缺陷）：Phase B 的 B1-5 行写作
> 「`autoplay` / `autoplay_interval`」两个属性，而实现在
> `properties_container.in.rs:189-210` 有意只发布 **`autoplay_interval`** 一个
> （毫秒；`null` 表示关闭）——理由已写入该处注释：拆成 bool + 间隔会允许二者矛盾。
> 现已把该行改为与实现一致，而不是新增一个冗余属性。
>
> ⚠️ **基线已变**：执行中删除了 9 个孤儿 `WebEngine*` kind，并新增了
> `RadarChart`、`KanbanBoard`、`Cascader`、`QueryBuilder`、`EmojiPicker`、`Mention`，
> 所以 `WidgetKind` 由 **174 → 171**。两处数字已同步到 README / codemap / 平台矩阵。
> 另：`check_control_has_tests` 的 **174** 统计的是「有 `impl Widget` 的
> `pub struct` 控件数」，与 `WidgetKind` 的 **167** 是两个不同的量
> （kind 里含 `WebEngineView`/`Frame`/`MenuItem` 等无独立 struct 或共享 struct 的项；
> 而 `WebEngine` 系列有 10 个 struct 共报 1 个 kind），
> **两个数字不该互相校验**；真正的判据是
> **每个 kind 都能解析出构造器**（已新增为门禁条件）。
>
> 📌 这正好回答了本轮用户提出的「174 vs 172」：两个数后来**恰好都变成 174**，
> 但这是巧合而非等价 —— 两个集合的成员一直不同。判据应是
> 「每个 kind 可构造」，而不是「两个数相等」。
>
> 本文件是执行计划，不是完成报告。
> **取证纪律（原则 #64）**：§二的每条「缺失/存在」都在当前工作树上实跑取得，
> 附带回执命令；**未复跑的不写入**。不凭印象回答。

---

## 核心规则（继承 BLUE16 全部，含 #1–#77）

### BLUE17 新增规则

78. **🧭 控件缺口必须以「能力」而非「名字」判定** — 判定「本项目没有 X 控件」时，必须先排除
    ① **别名/同义**（`Panel = GroupBox`、`CheckListBox = ListBox`）；② **数据项误判**（`TimelineItem`
    是数据，`TimelineWidget` 才是控件）；③ **能力已由兄弟控件覆盖**。
    判定：候选必须有**该能力确实不存在**的证据（`grep` 命中数 + 命名变体清单），
    而不是「我没搜到这个名字」。
79. **🎯 新增控件必须先回答「谁会调用」** — 一个控件即使实现完整，若无生产调用者、无 JSON/CSS 可达、
    无 C ABI 入口，则按 #72 属死重。判定：提案必须写明**至少一个**真实消费路径
    （JSON 声明 / CSS 选择器 / C ABI / 本仓 demo）。答不出即降级为「登记，不做」。
80. **📊 图表类优先「扩展，而非新建」** — 当新图表与既有控件共享**同一数据模型**与**同一交互**
    （hover、选中、tooltip），差异只在绘制几何时，必须扩展既有控件的枚举，不得新建独立控件。
    判定：提案需说明新图与既有控件的**数据模型是否相同**。共享则扩展，不共享才新建。
81. **🧪 新控件与 `WidgetKind` 的同步是强约束** — 每新增一个控件必须同时更新
    ① `WidgetKind` 变体、② 工厂注册（`check_widget_registration_fidelity.sh`）、
    ③ 一条**构建**它的测试（`check_control_has_tests.sh`）、④ 属性契约的双向断言。
    判定：四者缺一，门禁即 FAIL——这四条门禁已存在，新增控件会自动被追责。
82. **🔁 扩展既有控件时，同步成本低于新控件，但不是零** — 扩展枚举变体同样要更新
    ① `accepted_tokens` 与 `set` 解析器**成对**（`published_enum_tokens_are_accepted_by_their_control`
    会逐 token 回写并断言被接受）、② 该控件的绘制分支、③ 一条断言新变体可被 `set` 的测试。
    判定：只加枚举不接解析器，该测试必 FAIL。

---

## 一、问题的边界

用户问：「还有什么控件可以补？哪些是既有控件的扩展？」

本轮的分类定义（避免「新控件」与「扩展」混淆）：

| 类别 | 定义 | 判定依据 |
|---|---|---|
| **新控件** | 需要新的 `WidgetKind` 变体、新的工厂注册名、新的属性契约 | 无法落在任何既有控件的数据模型上 |
| **扩展** | 不新增 `WidgetKind`，只扩展既有控件的枚举 / 属性 / 交互 | 复用既有数据模型与属性契约 |

**本轮的取舍**：只收录**与本项目定位相容**的项。本库是**纯 Rust、全自绘、无捆绑二进制资源**的
跨平台 GUI 库，因此：

- ❌ **不收录**需要外部运行时/大体积资源的（3D 引擎、内嵌浏览器增强、视频编解码器）。
- ✅ **收录**能用既有绘制/事件/布局基础设施实现的。

---

## 二、现状取证

### 2.0 基线数字

```bash
$ grep -c "^    [A-Z][A-Za-z0-9]*," src/widget/kind.rs
174
$ bash tools/check_control_has_tests.sh | tail -2
controls with a test that names them: 172 / 172
✅ every control has at least one test that mentions it
$ ls tools/check_*.sh | wc -l
28
```

✅ 已复跑。注意：`check_control_has_tests` 的 **172** 统计的是「有 `impl Widget`
的 `pub struct` 控件数」，与 `WidgetKind` 的 **174** 是两个不同的量
（kind 里含 `WebEnginePage` 等无独立 struct 的项），**两个数字不该互相校验**。

### 2.1 全部 174 个 `WidgetKind`（实跑输出）

```
Window Dialog MessageBox FileDialog ColorDialog FontDialog InputDialog ProgressDialog
PopupWindow Button CheckBox RadioButton Label LineEdit TextEdit RichEdit ComboBox SpinBox
ListBox ListView TreeView ProgressBar Slider ScrollBar ScrollArea Panel Frame DockPanel
GroupBox TabWidget Splitter MdiArea MenuBar Menu MenuItem ContextMenu ToolBar StatusBar
Canvas Table Grid Chart ToggleButton CheckListBox DoubleSpinBox Dial Wizard DatePicker
TimePicker DateTimePicker DirectoryDialog DataView PropertyGrid Toolbox StackedWidget
CollapsiblePane DockWidget ActivityIndicator Calendar ColumnView UndoView CommandLink
LCDNumber FontComboBox WebEngineView WebEnginePage WebEngineSettings WebEngineDownloadItem
WebEngineCookieStore WebEngineWebChannel WebEngineFindTextResult WebEngineNotification
WebEngineScriptDialog WebEngineContextMenuRequest Action ToolButton FreeformShape TabBar
PieMenu RibbonBar TileView Line Meter MiniChart ImageView MiniCanvas Arc Spinner Roller
Dropdown TextArea Keyboard Switch SearchBox Chip Badge SkeletonLoader FAB BottomSheet
BottomNavigationBar NavigationDrawer AppBar MobileDatePicker Divider Stepper Rating Avatar
EmptyState Carousel ColorHistory ColorWell TagInput ImePreedit InplaceEditor QRCode
MasonryLayout CupertinoSwitch MaterialSnackbar AdaptiveScaffold WizardDialog SafeArea
CupertinoAlertDialog CupertinoSlider MaterialNavigationRail Tooltip SegmentedButton
NavigationStack ProgressCircle Icon DropdownMenu MaskedEdit MenuButton Popover
AutoCompleteEdit MultiSelectComboBox RangeSlider FloatingLabel FontPreview
CupertinoNavigationBar CupertinoSegmentedControl SwipeToDismiss PagerPageView TabView
SearchBar ShortcutEditor RefreshControl ModalBottomSheet LineChart Sparkline BarChart
FindReplaceDialog PropertiesPanel PieChart CupertinoDatePicker EditableComboBox
DateRangePicker AnimatedImage HeroAnimation BezierCurveEditor LottieWidget RiveWidget
VideoPlayer ImageGallery AudioVisualizer CameraPreview BarcodeScanner GridTable
NumberPicker OtpInput Banner Pagination ColorPicker Toast SplashScreen
```

✅ 已复跑（来源：`grep -oE "^    [A-Z][A-Za-z0-9]*," src/widget/kind.rs`）

### 2.2 ✅ 已存在、**不应**被当作缺口的（避免误报，规则 #78）

| 候选 | 实跑结论 | 证据 |
|---|---|---|
| `TimelineWidget` | ✅ **存在** | `src/widget/special_widgets/timeline_widget.rs` |
| `CommandPalette` | ✅ **存在** | `src/widget/special_widgets/command_palette.rs` |
| `NotificationCenter` | ✅ **存在** | `src/widget/special_widgets/notification_center.rs` |
| `DiffViewer` | ✅ **存在** | `src/widget/special_widgets/diff_viewer.rs` |
| `MarkdownEditor` | ✅ **存在** | `src/widget/special_widgets/markdown_editor.rs` |
| `CodeEditor` | ✅ **存在** | `src/widget/special_widgets/code_editor/editor.rs` |
| `TerminalView` | ✅ **存在** | `terminal_view.rs:18`（kind 为 `TextEdit`） |
| `GanttWidget` | ✅ **存在** | `gantt_widget.rs`（kind 为 `Chart`） |
| `Toast` + `ToastStack` | ✅ **存在** | `toast/single.rs`、`toast/stack.rs` |
| `VirtualList` / `VirtualTable` | ✅ **存在** | `src/widget/view_widgets/` |
| `Breadcrumb` | ✅ **存在** | `src/widget/special_widgets/breadcrumb.rs` |
| `MasonryLayout` | ✅ **存在** | `container_widgets/masonry_layout.rs` |
| `DataGrid`（含**冻结列**） | ✅ **存在** | `data_grid.rs`（`set_frozen_columns`） |
| `PropertyGrid` / `TreeTable` | ✅ **存在** | `view_widgets/` |
| `Sparkline` / `MiniChart` | ✅ **存在** | `chart_widgets/sparkline.rs`、`display_widgets/mini_chart.rs` |
| `HeroAnimation`（共享元素过渡） | ✅ **存在** | `WidgetKind::HeroAnimation` |
| `Carousel` | ✅ **存在**（但只有骨架，见 §四 B1） | `container_widgets/carousel.rs` |
| `PagerPageView` / `TileView` | ✅ **存在** | `container_widgets/` |
| `Pagination` | ✅ **存在** | `nav_widgets/pagination.rs` |
| `SkeletonLoader` | ✅ **存在** | `display_widgets/skeleton_loader.rs` |
| 无障碍能力 | ✅ **存在**（三平台桥） | `src/platform/accessibility/{linux,windows,macos}.rs` |

**教训（并入规则 #78）**：用户在提问里列举的「常见控件」，**多数已经存在**。
不先 grep 就写计划，会把「已有」写成「缺失」，制造虚假工作量。

---

## 三、A 类：可新增的控件

新增判据：**无法落在任何既有控件的数据模型上**，且可写明至少一条真实消费路径（规则 #79）。

### A1 `KanbanBoard`（看板）

| 项 | 内容 |
|---|---|
| 为什么值得做 | 跨列拖拽 + 投递区高亮 + 列内排序，是**拖放基础设施**的试金石 |
| 本项目现状 | ❌ 缺失（`grep "struct Kanban"` → 0 命中） |
| 数据模型 | 列 × 卡片的多对多移动，既有控件无一承载 |
| 消费路径 | JSON 声明 + C ABI 创建；`DropTarget` 的示范使用者 |
| 前置 | **A3 拖放基础设施**（先做 A3，再做本项） |

### A2 `Mention`（@ 提及）

| 项 | 内容 |
|---|---|
| 为什么值得做 | 协作输入的标配；**触发模型与 `AutoCompleteEdit` 不同**（字符触发 + 内联浮层 + 插入后光标定位） |
| 本项目现状 | ❌ 缺失 |
| 数据模型 | 候选来自「游标前一个 token」，而非整串前缀匹配 |
| 消费路径 | JSON 声明 |

### A3 拖放基础设施（`DropTarget` / `DragPayload`）— **前置项**

| 项 | 内容 |
|---|---|
| 现状（实跑） | ❌ 缺失。`Event::Drag` 存在（`event/types.rs:282`），但**仅指触摸手势**（`#[cfg(feature = "touch")]`，由 `gesture/press.rs` 的 `PanGesture`/`LongPressDragGesture` 产出）。**鼠标拖放**用的是 `MousePress` → `MouseMove` → `MouseRelease` 三件套，各控件各自实现，**没有「可放置目标」抽象** |
| 重复规模（实跑） | **28 个文件**同时处理 `MousePress`/`MouseMove`/`MouseRelease` 三件套（构成完整的拖拽循环），其中真正的拖拽状态机包括：`dockwidget`（拖窗口）、`splitter`（拖分隔条）、`slider` / `range_slider`（拖滑块）、`scrollbar`（拖滑块）、`canvas`、`bezier_curve_editor`（拖控制点）、`freeform_shape`、`modal_bottom_sheet`（下拖关闭）、`swipe_to_dismiss`（横滑）、`refresh_control`、`number_picker`、`collapsible_pane`、`groupbox` 等 |
| 为什么是前置 | 不统一的话，看板、列表重排、跟层移动要各写一套（重复规模已到 28 个文件） |
| 归属 | 新增 `src/event/dnd.rs` + `DropTarget` trait |
| 为什么不列入「新控件」 | 它是基础设施，不产生 `WidgetKind` |

### A4 `RadarChart`（雷达图）— **数据模型不同构，故为独立控件**

| 项 | 内容 |
|---|---|
| 现状（实跑） | ❌ 缺失 |
| 为什么不能并入 `ChartWidget` | 既有 `ChartWidget` 的数据模型是 `Vec<f64>` + `labels`（一维有序序列）。雷达图需要**维度轴**（多序列 × 多维度），与一维序列不同构 |
| 消费路径 | JSON 声明（`radar_chart`） |

### A5 `EmojiPicker` 外壳

| 项 | 内容 |
|---|---|
| 现状（实跑） | ❌ 缺失（`Icon` 只画**内置几何图形**，`IconName::from_name` 精确匹配固定 token 表，不是字形渲染） |
| 定位 | 只做**选择器外壳**：网格 / 搜索 / 最近使用 / 分类 / 键盘导航 |
| 字形来源 | **由调用方注入**（`Icon` 的几何图形集、或调用方自己的图片），不内嵌 emoji 数据 |
| 为什么不内嵌 | 与「无捆绑二进制资源」定位冲突 |

**不做**：`Treemap`、`Heatmap`、`SankeyChart`、`BoxPlot` 的**独立控件形态**——前两者与 `BoxPlot` 应作为 `ChartWidget` 的变体（§四 B2），`SankeyChart` 见 §五。

---

## 四、B 类：既有控件的扩展

扩展判据：**复用既有数据模型**，只加枚举变体 / 属性 / 交互，**不新增 `WidgetKind`**（规则 #80）。

### B1 轮播图：三控件收敛 + `Carousel` 能力补齐 ⭐

**这是本轮最该先做的一项**：本项目已有**三个**「一页一屏 + 圆点指示器」的控件，
但真正能装内容的那个不叫 `Carousel`，而叫 `Carousel` 的那个装不了内容。

实跑取证（三个控件的**全部公开方法**）：

```bash
$ grep -n "pub fn" src/widget/container_widgets/carousel.rs
new add_page set_current current page_count next previous current_page pages current_page_title

$ grep -n "pub fn" src/widget/container_widgets/pager_page_view.rs
new add_page remove_page page_count current_page set_current_page set_show_indicator show_indicator

$ grep -n "pub fn\|fn next_page\|fn prev_page" src/widget/container_widgets/tile_view.rs
new page_count set_page_count current_page set_current_page next_page(priv) prev_page(priv)
```

| 维度 | `Carousel` | `PagerPageView` | `TileView` |
|---|---|---|---|
| 内容 | ❌ **只有 `{title, color}`** | ✅ `add_page(Box<dyn WidgetAndDraw>)` | ❌ 只有 `page_count: u32` |
| 翻页交互 | 点击左/右半（`MouseRelease`） | **方向键**（`KeyPress`/`KeyDown` 37/39）；鼠标/触摸事件**透传给当前页**，不翻页 | 方向键 |
| 指示器 | ✅ 圆点（>20 页折叠为省略号） | ✅ `set_show_indicator(bool)` | ✅ 圆点 |
| 循环 | ❌ 两端停住 | ❌ 两端停住 | ❌ 两端停住 |
| 自动播放 | ❌ | ❌ | ❌ |
| 滑动/拖拽翻页 | ❌ | ❌ | ❌ |
| 属性契约 | `current_index` / `item_count` / `current_page_title` | `PAGER_PAGE_VIEW_PROPERTIES` | `current_page` / `page_count` |

**关键取证 — 轮播图的三个缺口在三个控件里各缺一块**：

| 缺口 | `Carousel` | `PagerPageView` | `TileView` |
|---|---|---|---|
| 内容槽位 | ❌ | ✅ | ❌ |
| 鼠标/触摸**滑动**翻页 | ❌ | ❌（事件透传） | ❌ |
| 自动播放 + 循环 | ❌ | ❌ | ❌ |

**所以「增加轮播图」的正确做法不是新建控件**，而是：

| 判定 | 结论 |
|---|---|
| 是否已有轮播语义的控件？ | ✅ 有，且**有三个**（`Carousel` / `PagerPageView` / `TileView`） |
| 是否该新建 `CarouselView` / `Banner` 之类？ | ❌ **不该**。新建即成为第 4 条腿（规则 #74），且 `check_widget_registration_fidelity` 会让第四种实现与前三者并存 |
| 该扩展谁？ | **`Carousel`**（名字最贴切、kind 最正统、有唯一的 `swipe_view` 别名），把它缺的三块补齐 |

**执行清单（按依赖排序）**：

| 步骤 | 内容 | 验收 |
|---|---|---|
| B1-0 | **收敛判定**：写一段模块文档说明 `Carousel` / `PagerPageView` / `TileView` 的分工；若判定 `PagerPageView` 应被 `Carousel` 吸收，则改为**别名登记**（`Panel = GroupBox` 先例）并删除重复实现 | 文档段落 + 门禁仍 PASS ✅ **第 29 轮：按用户要求直接物理删除这两个控件，而非别名登记** |
| B1-1 | **内容槽位**：`set_page_content(index, Box<dyn WidgetAndDraw>)`，绘制时按当前页绘制（复用 `PagerPageView::content_rect` + `swipe_to_dismiss.rs` 的 offset 绘制手法） | 测试：放置子控件后，`current_index` 切换时子控件几何随之更新 |
| B1-2 | **滑动翻页**：接 `MousePress` → `MouseMove` → `MouseRelease`，松手按**位移阈值 + 速度**决定前进/回退/吸附；与 B1-1 的内容事件转发的**冲突必须解决**（滑动与「点内容」的判定边界） | 像素断言：滑过 50% 宽度后松手，落到相邻页；⭐ **速度判据**（第 29 轮补完）：`swipe_direction_at(offset, velocity)`，400px/s 击发 + 2% 防抖下限，反向注入已证可失败 |
| B1-3 | **自动播放 + 循环**：`set_autoplay(Option<Duration>)` / `set_loop(bool)`；暂停条件（指针悬停、按下、不可见、`is_enabled() == false`） | 测试：autoplay 开启时 hover **不**推进；`loop` 为 false 时末页停住 |
| B1-4 | **指示器样式**：`set_indicator_style(Dots / Bars / Numeric / None)`、`set_indicator_position(Top/Bottom/Left/Right)` | 测试：`None` 时不绘制指示器（像素断言）；⭐ **`Numeric`**（第 29 轮补完）：绘制 `current/total`，不随页数扩展（像素比 < 2.0） |
| B1-5 | **属性契约扩展**：`loop` / `autoplay_interval` / `indicator_style` / `indicator_position` 进 `CAROUSEL_PROPERTIES`。**每个 enum 属性必须成对更新 `accepted_tokens` 与 `set` 解析器**（规则 #82）。📌 实现有意**不**发布独立的 `autoplay` 布尔属性：毫秒值 `null` 即关闭自动播放，拆成两者会允许互相矛盾的写法（理由见 `properties_container.in.rs:196-198`） | `published_enum_tokens_are_accepted_by_their_control` PASS |
| B1-6 | **测试空转（已实测证实，顺带修）**：`carousel_disabled_blocks_events` 发的是 `MousePress`，而翻页逻辑在 `MouseRelease`（`carousel.rs:273`）。**反向注入实验**：删掉 `handle_event` 开头的 `if !self.base.is_enabled() { return; }`（`carousel.rs:269`）后，该测试**仍然 13 passed / 0 failed**——证明它并未真的断言禁用态。补一条发 `MouseRelease` 的断言 | 补完后再做同样的反向注入，必须 FAIL |

**不要做的事**：

- ❌ 不新建第 4 个轮播类控件。
- ❌ 不在 `Carousel` 里做「图片轮播」专用逻辑——`ImageGallery` 已覆盖图片场景（缩略图条 + 左右箭头）；
  要做的是**通用内容槽**，图片只是其中一种内容。

### B2 图表：扩展 `ChartWidget` 而非新建（规则 #80）

实跑取证：

```bash
$ sed -n '28,39p' src/widget/special_widgets/chart.rs
pub enum ChartType { Bar, Line, Pie, Scatter }   # 4 种
$ grep -rn "const CHART_PROPERTIES" -A 6 src/widget/capability/properties_other.in.rs
PropertySchema::enumerated("chart_type", true, true, &["bar", "line", "pie", "scatter"])
$ sed -n '182,193p' src/widget/special_widgets/chart.rs
fn expect_chart_type(...) { "bar"|"line"|"pie"|"scatter" => Ok(..), _ => Err(TypeMismatch) }
```

**共享数据模型**（`Vec<f64>` + `labels`）与**共享交互**（`hovered_index`、
`data_point_clicked`、`data_point_hovered`）——符合规则 #80 的扩展条件：

| # | 变体 | 为什么可并入 | 数据模型 |
|---|---|---|---|
| B2-1 | `Candlestick`（K 线） | 每点 4 个值（开高低收）仍是**有序一维序列**，可编码为 `Vec<f64>` 的 4 元组分组 | 同构（需定分组约定） |
| B2-2 | `Waterfall` | 起始值 + 逐项增量，仍是 `Vec<f64>` + labels | 同构 |
| B2-3 | `BoxPlot` | 每点 5 个统计量，同 B2-1 的分组约定 | 同构（需定分组约定） |
| B2-4 | `Area`（面图） | **引擎层已有 `AreaChart`**（`chart_widgets/types.rs:72`），控制层未暴露 → 只差一个枚举变体 | 同构 |

**B2-1/B2-3 的数据模型有一个真问题**：`ChartWidget` 现在只存 `Vec<f64>`，
K 线需要每点 4 个值。开工前的判定：

| 方案 | 做法 | 代价 |
|---|---|---|
| **① 分组约定** | 把 `Vec<f64>` 按 4 个一组解释为 OHLC | 不改数据模型，但语义隐式、易错 |
| **② 增加可选序列** | `ChartWidget::set_series(Vec<Vec<f64>>)`，一维图用单序列 | 改数据模型，但语义显式 |
| **③ 不并入** | K 线/箱线独立成控件 | 轴/图例/tooltip 重复实现 |

**本计划建议 ②**：`Vec<f64>` 作为「单序列」的特例，`set_series` 提供多序列；
这样雷达图（A4）之外的多序列需求也能覆盖，而 `Vec<f64>` 的既有 API 不变。

**`SankeyChart`（桑基图）与 `FunnelChart`（漏斗图）**：

- `SankeyChart`：需要「节点 + 有向边」的图结构，与 `Vec<f64>` **不同构** → 若要做，属**新控件**
  （本计划登记为「暂不做」，见 §六）。
- `FunnelChart`：**可以**并入 `ChartWidget`（逐级递减的一维序列 + labels，同构），
  建议作为 B2-5 补入。

### B3 `Gauge`/`ProgressCircle`：扩展 `Meter`，不新建（规则 #78）

实跑取证（**旧版本计划在此处判断有误，本版更正**）：

```bash
$ grep -n "kind:\|canonical_name:\|aliases:" src/widget/capability/properties.rs  # Meter
kind: WidgetKind::Meter, canonical_name: "meter", aliases: &["meter_widget", "gauge"]
$ grep -n "tick_count\|fn set_minimum\|fn set_maximum" src/widget/display_widgets/meter.rs
30: tick_count: u32,  43: tick_count: 5,  91: set_minimum,  101: set_maximum,  106: set_tick_count
```

| 判定项 | 实跑结论 |
|---|---|
| 是否已有「仪表」语义 | ✅ **有**。`Meter` 模块文档自称「gauge with arc and needle」，**且已占用 `gauge` 作为别名** |
| 是否已有刻度 | ✅ **有**。`set_tick_count(n)`，绘制时在弧上画刻度线（`meter.rs:236-257`） |
| 是否已有量程 | ✅ **有**。`set_minimum` / `set_maximum` / `normalized_value()` |
| 是否已有指针 | ✅ **有**。needle + 中心枢轴（`meter.rs:259-266`） |
| 真正缺什么 | ❌ **区间色带**（安全/警告/危险），❌ 刻度**数值标签**，❌ 单位后缀 |

**结论：`Gauge` 不新建。** 若新建，会得到**两个都自称 `gauge` 的控件**（`Meter` 的别名 +
新控件名），这正是规则 #78 要防的重复。应扩展 `Meter`：

| 步骤 | 内容 | 验收 |
|---|---|---|
| B3-1 | `add_threshold_range(from, to, color)`：在弧上按区间着色（安全/警告/危险） | 像素断言：给定区间内的弧段颜色 |
| B3-2 | `set_show_tick_labels(bool)`：在刻度外侧绘制数值 | 像素断言：标注文本存在 |
| B3-3 | `set_unit(&str)`：值旁显示单位（`72°C`） | 测试：`value_text()` 拼接正确 |
| B3-4 | `thresholds` 进 `METER_PROPERTIES`（按既有「只读计数 + 可写序列」约定） | 属性双向断言 PASS |

**`ProgressCircle` 的分工**（一并写进 `Meter` 模块文档）：

| | `ProgressCircle` | `Meter` |
|---|---|---|
| 表示 | 完成度（0→max 的进度） | **测量值**在量程中的位置 |
| 刻度 | 无 | **有** |
| 区间色带 | 无 | **待补**（B3-1） |
| 语义 | 「任务完成 60%」 | 「CPU 温度 72°C，红区」 |

### B4 `Cascader`（级联选择）— 判定结论：**新控件**（不属于本节）

> 本节是「扩展」清单，但 `Cascader` 实际判定为新控件，**故其执行项在 Phase D**。
> 列出判定过程是为了证明「不新建」不是默认答案——判定必须先做。

| 项 | 内容 |
|---|---|
| 现状 | ❌ 缺失 |
| 实跑取证 | `Dropdown { items: Vec<String>, selected_index, expanded }`（`input_widgets/dropdown.rs:34`）——**数据模型是扁平字符串列表**，无层级、无路径 |
| 判定 | 级联需要「路径回填 + 逐级展开 + 异步加载」，**与扁平 `Vec<String>` 不同构** → **属新控件**（`WidgetKind::Cascader`），不得强行塞进 `Dropdown` |
| 可复用 | `Dropdown` 展开/收起与选中态绘制、`TreeView` 的路径模型 |
| 消费路径 | JSON 声明（`cascader`） |

### B5 `QueryBuilder`（条件构建器）— 判定结论：**新控件外壳 + 复用筛选模型**

> 渲染外壳是新控件（Phase D），但它产生的**数据模型是既有筛选链路的扩展**（归本节）。

实跑取证：

```bash
$ sed -n '34,39p' src/widget/view_widgets/data_grid.rs
pub struct ColumnFilter { pub column: usize, pub query: String }
```

| 项 | 内容 |
|---|---|
| 判定 | `ColumnFilter` 是**扁平的单列子串匹配**，无法表达嵌套 AND/OR 树 → 渲染外壳属**新控件** |
| **但** | 它**必须产出同一套筛选模型**（把 `ColumnFilter` 扩为可递归结构），使「代码设筛选」与「用户建筛选」共用一条链路（规则 #54） |
| 扩展部分 | `ColumnFilter` → 可递归的 `FilterExpr`（`And`/`Or`/`Predicate`），`DataGrid::set_filters` 接受它 |
| 消费路径 | JSON 声明（`query_builder`） |

### B6 `ScrollArea` 通用吸顶（`sticky`）

| 项 | 内容 |
|---|---|
| 现状（实跑） | ⚠️ **部分**。`grep -i sticky container_widgets/scrollarea.rs` → 0 命中；`AppBar` 可固定但无通用吸顶 |
| 归属 | **扩展 `ScrollArea`**（不是新控件） |
| 做法 | 子控件声明 `sticky: true` 后，滚动时固定在其所属分组的顶部边界 |
| 消费路径 | CSS 属性 / JSON 属性 |

---

## 五、C 类：登记但本轮**不做**（附理由）

| 项 | 为何不做 |
|---|---|
| **`SankeyChart`** | 需「节点 + 有向边」图结构，与 `ChartWidget` 数据模型不同构；且需有向图布局算法（分层、避让、边交叉最小化），是独立算法项目 |
| **`Treemap` / `Heatmap` 作为新控件** | 二者均为 **`Vec<f64>` + labels 的二维版本**，符合规则 #80 的扩展条件 → 若做，应作为 `ChartWidget` 的变体（`Heatmap` 需先定「矩阵」如何映射到 `Vec<f64>`）。本轮不做是为控制范围，**不是因为它们是新控件** |
| **`WorkflowGraph`（节点连线编辑器）** | 需图布局算法（分层/避让/美观布线），是独立算法项目，不是控件工作量 |
| **`SpreadsheetSheet`（电子表格）** | 等于重写公式引擎 + 依赖图 + 引用重算；`GridTable` 已覆盖表格展示 |
| **`FormulaEditor`** | 同上，需要解析器 + 求值器 |
| **`JSONEditor` / SQL 语法编辑器** | `CodeEditor` 已支持自定义高亮规则，**属配置而非新控件** |
| **3D / 材质视图** | 与「全自绘、无捆绑资源」定位冲突 |
| **触觉反馈（`haptic_feedback`）** | 需 iOS `UIFeedbackGenerator` / Android `Vibrator` 等**平台 API**，属 OS 相关问题 |
| **平台视图过渡（`ViewTransition`）** | 其他平台需自绘；可先用既有 `HeroAnimation` + `PropertyAnimation` 组合 |
| **网络协同（在线状态、CRDT 光标）** | 超出 GUI 库边界 |
| **`EmojiPicker` 内嵌字形数据** | 与「无捆绑二进制资源」定位冲突（外壳仍做，见 A5） |
| **Lottie / Rive 运行时扩展** | 已有 `LottieWidget`/`RiveWidget`；再扩是解析器工作量，非控件 |

---

## 六、执行计划

> 顺序原则：**基础设施 → 已存在控件的缺口 → 新控件**。
>
> **执行状态（第 28 轮，实跑取证见 [`log-20260917-3.md`](../log/log-20260917-3.md)）**：
>
> | 阶段 | 状态 | 说明 |
> |---|---|---|
> | **A** 拖放基础设施 | ✅ **100%**（A-1~A-4） | `src/event/dnd.rs` 已建；`splitter` 已迁移并反向注入验证；`KanbanBoard` 已接入并绘制插入预览 |
> | **B** `Carousel` 能力补齐 | ✅ **100%**（B1-0~B1-6） | 内容槽/滑动/自动播放/循环/指示器/属性契约/空转修复全部完成；**第 29 轮补完 B1-2 的速度判据与 B1-4 的 `Numeric`**；`PagerPageView`/`TileView` 已按用户要求物理删除 |
> | **C** 既有可视化控件扩展 | ✅ **100%**（C-1~C-5） | 多序列模型 + 图表 4→9 变体 + `Meter` 色带 + `ScrollArea` 吸顶 |
| **D** 新控件 | ✅ **100%**（D-1 ~ D-6） | `KanbanBoard` + `RadarChart` + `Cascader` + `QueryBuilder` + `EmojiPicker` + `Mention` |
| **E** 新控件的强制同步 | ✅ **100%** | 四条门禁全 PASS（**175/175** constructible；第 30 轮校正，见文首） |
| **F** 文档与发布物 | ✅ **100%**（F-1/F-2/F-3） | 计数已同步；四个新控件已补 `# Reachability` 段；日志已回写 |

### Phase A — 拖放基础设施（A3，**前置**）

| 步骤 | 内容 | 验收 |
|---|---|---|
| A-1 | `src/event/dnd.rs`：`DragPayload`、`DropEffect`（Copy/Move/Link/None）、`DropTarget` trait（`can_accept` / `on_drop` / `preview_rect`） | ✅ 单元测试：被拒绝的载荷不进 `on_drop`（14 条） |
| A-2 | 接线到既有鼠标三件套（`MousePress` → `MouseMove` → `MouseRelease`）并兼容触摸 `Event::Drag` | ✅ **反向注入已证**：删掉 `DropTarget` 查询，看板测试失败 |
| A-3 | 迁移既有拖拽实现中的**至少 1 处**（建议 `splitter`，它的拖拽状态机最小）到新抽象，证明可复用 | ✅ `splitter` 已迁移（6 处 `DragSession` 引用）；该控件测试全绿 |
| A-4 | `DropZone` 可视化（高亮 + 放置预览） | ✅ 像素断言（规则 #73）：`kanban_drag_overlay_differs_from_the_resting_frame` |

### Phase B — `Carousel` 能力补齐（B1，⭐ 建议最先做）

见 §四 B1 的 B1-0 ~ B1-6。**B1-0 的收敛判定必须先做完。**

> ⭐ **第 29 轮补充**：B1-2 的「**速度**」判据与 B1-4 的「**`Numeric`**」样式
> 在第 28 轮被记为完成但实现缺失，已于第 29 轮补齐并用反向注入证明可失败。
> 证据：`docs/log/log-20260917-4.md` §3（`Numeric`）、§4（速度判据）。

### Phase C — 既有可视化控件的扩展

| 步骤 | 内容 |
|---|---|
| C-1 | `ChartWidget` 数据模型判定（§四 B2 的方案 ①/②/③，**需用户拍板**） | ✅ 采用**方案 ②**：`set_series(Vec<Vec<f64>>)`，`Vec<f64>` 保持为单序列特例 |
| C-2 | `ChartType` 扩展：`Area`、`Waterfall`、`Funnel`、`Candlestick`、`BoxPlot` | ✅ 4 → **9** 变体；`as_str`/`from_name`/`values_per_point` 三处同步 |
| C-3 | `Meter` 扩展：区间色带、刻度标签、单位（B3） | ✅ `add_threshold_range` / `set_show_tick_labels` / `set_unit`（22 条测试） |
| C-4 | `ScrollArea` 通用吸顶（B6） | ✅ `StickyRegion` + `add_sticky_region` / `pinned_sticky_bands`（8 条测试） |
| C-5 | `published_enum_tokens_are_accepted_by_their_control` 对全部新变体 PASS（规则 #82） | ✅ 逐 token 回写并断言被接受 |

### Phase D — 新控件

| 步骤 | 控件 | 关键交互 | 依赖 |
|---|---|---|---|
| D-1 | `KanbanBoard` | 卡片跨列拖拽 / 列内排序 / 列折叠 / WIP 上限 | ✅ 34 条测试 |
| D-2 | `Mention` | 字符触发 / 候选浮层 / 键盘选择 / 插入后光标定位 | ✅ 45 条测试 |
| D-3 | `RadarChart` | 多序列多维度轴 / hover 值提示 / 图例 | ✅ 14 条测试 |
| D-4 | `EmojiPicker` 外壳 | 网格 / 搜索 / 最近使用 / 分类 / 键盘导航 | ✅ 45 条测试 |
| D-5 | `Cascader` | 逐级展开 / 路径回填 / 键盘导航 / 异步加载 | ✅ 40 条测试 |
| D-6 | `QueryBuilder` + `FilterExpr` | AND/OR 嵌套树编辑；产出可递归筛选模型 | ✅ 41 条测试 |

### Phase E — 每个新控件的强制同步（规则 #81，**门禁已存在，会被自动追责**）

```bash
# 每新增一个控件必须让这四条同时通过：
cargo test --no-default-features --features desktop --lib -q          # 行为测试
bash tools/check_widget_registration_fidelity.sh                       # 工厂注册 + kind 归类
bash tools/check_control_has_tests.sh                                  # 每个控件有构建测试
bash tools/check_widget_kind_count.sh                                  # 计数与文档一致
```

### Phase F — 文档与发布物

| 步骤 | 内容 |
|---|---|
| F-1 | 同步 `WidgetKind` 计数到 `README.md`、`README.zh-CN.md`、`docs/ARCHITECTURE.md` | ✅ README ×2 与 `docs/ARCHITECTURE.md` 均与实跑一致（**175**，第 30 轮实跑复核）；该文件已修正其两处架构描述错误（见下） |
| F-2 | 每个新控件补 `# Reachability` 段（规则 #72 三态） | ✅ 六个新控件均已含（实测逐个 grep） |
| F-3 | 日志回写完成率与证据 | ✅ `log-20260917-3.md` + `log-20260917-4.md` |

---

## 七、验证矩阵

| # | 检查 | 期望 |
|---|---|---|
| V1–V5 | 5 档 `cargo check --lib` | 全 `Finished` |
| V6 | `clippy --all-targets -- -D warnings` | 0 warning |
| V7 | `test --features desktop --lib` | 0 failed |
| V8–V10 | embedded / mini / doc | 0 failed |
| V11 | 全部门禁（当前 **28** 个） | 全 PASS（`check_apple_native` 需 macOS 主机） |
| V12 | `check_widget_kind_count.sh` | 计数与文档一致 |
| V13 | 新增/扩展的**行为测试**（非仅构造） | 每个变体 ≥1 条（规则 #73） |
| V14 | `published_enum_tokens_are_accepted_by_their_control` | 全 token 可回写（规则 #82） |

---

## 八、优先级建议

**第一件事：`Carousel` 能力补齐（Phase B）。** 理由：

1. 它是**已登记却只有骨架**的控件——`WidgetKind::Carousel` 与工厂注册都已存在，
   缺口是「不能装内容、不能滑动、不能自动播放」，**投入产出比最高**；
2. 它不需要任何前置（拖放基础设施还没做完也能先做，翻页只需鼠标三件套）。

**第二件事：拖放基础设施（Phase A）。** 理由：它是 `KanbanBoard` 与所有「重排/移动」交互的
**共同前置**，且现状是 **28 个文件各写一套鼠标拖拽循环**——这是最明显的结构性空洞。

**第三件事：图表的「数据模型判定」（C-1）。** 理由：`ChartType` 的 **4 个变体与
`accepted_tokens` 的 4 个 token 是成对的手写常量**（规则 #82），每加一种图要改
三处手写表；不先定数据模型就继续加，会把「扩展」做成一批易漂移的重复代码。

---

## 九、本文件的一句话结论

本项目**已有 174 个 `WidgetKind`**，用户提问中的多数「常见控件」**都已存在**
（`TimelineWidget`、`CommandPalette`、`NotificationCenter`、`DiffViewer`、`MarkdownEditor`、
`ToastStack`、`GanttWidget`、`VirtualList`、`DataGrid`（含冻结列）、`CodeEditor`、`TerminalView`、
`HeroAnimation`、`Pagination`、`Carousel` 等）。

真正的工作分两类：

1. **既有控件的能力补齐**（优先）——
   `Carousel`（内容槽 + 滑动 + 自动播放，**最该先做**）、`Meter`（区间色带 + 刻度标签，
   **不新建 `Gauge`**）、`ChartWidget`（`Area` 变体 + 数据模型判定）、`ScrollArea`（吸顶）；
2. **可新增的控件**——
   `KanbanBoard`（依赖拖放基础设施）、`Mention`、`RadarChart`（数据模型与 `ChartWidget` 不同构）、
   `EmojiPicker` 外壳（字形由调用方注入）。

**必须先拍板的两项**：
- `Carousel` 与 `PagerPageView` 的**归属判定**（扩展谁 / 谁是别名）；
- `ChartWidget` 的多序列数据模型（§4 B2 方案 ①/②/③）。
