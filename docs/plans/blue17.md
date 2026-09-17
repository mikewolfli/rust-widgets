# BLUE17 — 前沿控件补全：缺口目录 + 判定与执行计划

> 状态：**计划（未执行）**；现状取证（§二）**已完成并逐条实跑**
> 完成率：**取证 100% · 执行 0%**
> 原则依据：[`docs/plans/principle.md`](principle.md)（继承 BLUE1–BLUE16 全部规则，含 #1–#77）
> 上轮计划：[`docs/plans/blue16.md`](blue16.md)（Phase F 已 100%）
> 上轮日志：[`docs/log/log-20260917-2.md`](log/log-20260917-2.md)（第 23–27 轮）
> 目标基线：`rust_widgets v2.2.0`（`WidgetKind` **174** 个、控件测试覆盖 **172/172**、门禁 **28** 个）
>
> 本文件是执行计划，不是完成报告。
> **取证纪律（原则 #64）**：§二的每条「缺失/存在」都在 `v2.2.0` 工作树上实跑取得，
> 附带回执命令；**未复跑的不写入**。用户提问里的每一类都先验证再回答，不凭印象。

---

## 核心规则（继承 BLUE16 全部，含 #1–#77）

### BLUE17 新增规则

78. **🧭 控件缺口必须以「能力」而非「名字」判定** — 判定「本项目没有 X 控件」时，必须先排除
    ① **别名/同义**（`Panel = GroupBox`、`CheckListBox = ListBox`）；② **数据项误判**（`TimelineItem`
    是数据，`TimelineWidget` 才是控件）；③ **能力已由兄弟控件覆盖**（`Sparkline` 已覆盖单行趋势，
    新增 `MiniLine` 是重复）。判定：对候选名 `grep "pub struct X\b"` **且** `grep "impl Widget for X"`，
    两者都命中才算「已存在」。
79. **🎯 新增控件必须先回答「谁会调用」** — 一个控件即使实现完整，若无生产调用者、无 JSON/CSS 可达、
    无 C ABI 入口，则按 #72 属死重。判定：提案必须写明**至少一个**真实消费路径
    （JSON 声明 / CSS 选择器 / C ABI / 本仓 demo）。答不出即降级为「登记，不做」。
80. **📊 图表类控件优先「扩展枚举」，而非「新建控件」** — 当新图表与既有控件共享**同一数据模型**
    （`Vec<f64>` + labels）与**同一交互**（hover、选中、tooltip），差异只在绘制几何时，
    必须扩展 `ChartType`/`ChartWidget`，不得新建独立控件——否则每加一种图就多一份轴、图例、
    命中测试的重复实现。判定：提案需说明新图与既有 `ChartWidget` 的**数据模型是否相同**。
81. **🧪 前沿控件与 `WidgetKind` 的同步是强约束** — 每新增一个控件必须同时更新
    ① `WidgetKind` 变体、② 工厂注册（`check_widget_registration_fidelity.sh`）、
    ③ 一条构建它的测试（`check_control_has_tests.sh`）、④ 属性契约的双向断言。
    判定：四者缺一，门禁即 FAIL——且这四条门禁已存在，新增控件会自动被追责。

---

## 一、问题的边界

用户问：「还有什么比较**前沿**的控件，本项目没有？」

「前沿」在本轮的可操作定义（避免空泛）：

| 维度 | 含义 | 反例（不算前沿） |
|---|---|---|
| **范式新** | 近十年 UI 范式产生的新控件形态（看板、命令面板、视图过渡） | 再做一个 `TabWidget` 的变体 |
| **数据密度高** | 面向大数据/多字段的专用视图（热力图、树图、透视表） | 再加一个 `Label` |
| **专业领域** | 金融/运维/协作等垂直场景控件（K 线、日志流、协同光标） | 通用按钮的第 5 种样式 |
| **平台新交互** | 随新平台出现的手势/反馈（触觉、共享元素过渡、预测返回） | 已有的 hover/click |

**本轮的取舍**：只收录**与本项目定位相容**的项。本库是**纯 Rust、全自绘、无捆绑二进制资源**的
跨平台 GUI 库（`Cargo.toml:5`），因此：

- ❌ **不收录**需要外部运行时/大体积资源的（3D 引擎、Lottie JSON 运行时已有例外、
  内嵌浏览器增强、视频编解码器扩展）——见 §五「明确不做」。
- ✅ **收录**能用既有绘制/事件/布局基础设施实现的。

---

## 二、现状取证（全部已复跑）

### 2.0 基线数字

```bash
$ grep -c "^    [A-Z][A-Za-z0-9]*," src/widget/kind.rs
174
$ bash tools/check_control_has_tests.sh | tail -1
controls with a test that names them: 172 / 172
$ ls tools/check_*.sh | wc -l
28
```
✅ 已复跑

### 2.1 全部 174 个 `WidgetKind` 的完整清单（实跑输出）

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

用户提问中容易被误判为「缺失」的项，逐条实跑：

| 候选 | 实跑结论 | 证据 |
|---|---|---|
| `TimelineWidget` | ✅ **存在** | `src/widget/special_widgets/timeline_widget.rs`（注意 `TimelineItem` 是数据项） |
| `Kanban` | ❌ **缺失** | `grep "struct Kanban"` → 0 命中 |
| `CommandPalette` | ✅ **存在** | `src/widget/special_widgets/command_palette.rs` |
| `NotificationCenter` | ✅ **存在** | `src/widget/special_widgets/notification_center.rs` |
| `DiffViewer` | ✅ **存在** | `src/widget/special_widgets/diff_viewer.rs` |
| `MarkdownEditor` | ✅ **存在** | `src/widget/special_widgets/markdown_editor.rs` |
| `ToastStack` | ✅ **存在** | `src/widget/special_widgets/toast/stack.rs` |
| `CodeEditor` | ✅ **存在** | `src/widget/special_widgets/code_editor/editor.rs` |
| `Terminal` | ✅ **存在**（名为 `TerminalView`） | `grep "struct Terminal"` → `terminal_view.rs:18` |
| `GanttWidget` | ✅ **存在** | `src/widget/special_widgets/gantt_widget.rs` |
| `VirtualList` | ✅ **存在** | `src/widget/view_widgets/virtual_list.rs` |
| `Breadcrumb` | ✅ **存在** | `src/widget/special_widgets/breadcrumb.rs` |
| `GridLayout` / `FlexLayout` | ✅ **存在** | `src/layout/grid.rs` / `src/layout/flex.rs` |
| `MasonryLayout` | ✅ **存在** | `src/widget/container_widgets/masonry_layout.rs` |
| `DataGrid`（含**冻结列**） | ✅ **存在且已有 `set_frozen_columns`** | `data_grid.rs:215` |
| `PropertyGrid` / `TreeTable` | ✅ **存在** | `view_widgets/` |
| `Sparkline` / `MiniChart` | ✅ **存在** | `chart_widgets/sparkline.rs`、`display_widgets/mini_chart.rs` |
| `Animation` / `Easing` | ✅ **存在** | `src/style/animation.rs`（含 `PropertyAnimation`） |
| `HeroAnimation`（共享元素过渡） | ✅ **存在** | `WidgetKind::HeroAnimation` |
| 无障碍能力 | ✅ **存在**（三平台桥） | `src/platform/accessibility/{linux,windows,macos}.rs` |

**教训（并入规则 #78）**：用户在提问里列了若干「常见的控件」，其中**超过一半已经存在**。
若不先 grep 就写计划，会把「已有」写成「缺失」，制造一批虚假工作量。

---

## 三、缺口目录（按「前沿程度 × 可实现性」排序）

### 3.1 A 类：范式新 —— 现代应用架构的标配（建议做）

| # | 控件 | 为什么算前沿 | 本项目现状 | 可复用基础设施 |
|---|---|---|---|---|
| **A1** | **`KanbanBoard`** | 看板是近十年任务管理的主流范式；核心难点是**跨列拖拽 + 投递区高亮 + 卡片排序**，是**拖放基础设施**的试金石 | ❌ 缺失（零命中） | `ScrollArea`、`DropZone`(需新增)、`Drag` 事件、`CollapsiblePane` |
| **A2** | **`Heatmap`** | 数据密度最高的可视化之一（日历热力图、相关矩阵）；已有 `Calendar` 可复用日期网格 | ❌ 缺失 | `Canvas` 绘制、`Calendar` 的网格逻辑 |
| **A3** | **`Treemap`** | 层级 + 面积编码，是「用有限像素表达层级占比」的唯一常见解法 | ❌ 缺失 | `Canvas`、`layout` 的类型化几何 |
| **A4** | **`Cascader`**（级联选择） | 替代「多级下拉联动」的现代形态；难点是**路径回填 + 异步展开** | ❌ 缺失 | `Dropdown`、`TreeView` 的路径模型 |
| **A5** | **`Mention`**（@ 提及） | 协作应用的标配输入增强；与 `AutoCompleteEdit` **同族但触发模型不同**（字符触发 vs 全量补全） | ❌ 缺失 | `AutoCompleteEdit`、`TextEdit` |

### 3.2 B 类：专业领域 —— 垂直场景刚需（建议做，可分优先）

| # | 控件 | 场景 | 本项目现状 | 备注 |
|---|---|---|---|---|
| **B1** | **`Gauge`** | 仪表盘（运维/车载/工业）；与既有 `ProgressCircle` 同族但**有刻度与量程** | ❌ 缺失 | 见 §4.3 与 `ProgressCircle` 的关系判定 |
| **B2** | **`CandlestickChart`**（K 线） | 金融图表第一刚需 | ❌ 缺失 | **按规则 #80：应扩展 `ChartWidget` 而非新建控件**（见 §四） |
| **B3** | **`WaterfallChart`** | 财务/性能归因分析 | ❌ 缺失 | 同上，扩展 `ChartWidget` |
| **B4** | **`RadarChart`**（雷达图） | 多维评分对比 | ❌ 缺失 | 同上，扩展 `ChartWidget` |
| **B5** | **`BoxPlot`**（箱线图） | 统计分布 | ❌ 缺失 | 同上，扩展 `ChartWidget` |
| **B6** | **`SankeyChart`** | 流向分析（预算、能量、转化漏斗） | ❌ 缺失 | 同上，扩展 `ChartWidget` |
| **B7** | **`FunnelChart`**（漏斗图） | 转化率分析 | ❌ 缺失 | 同上，扩展 `ChartWidget`；与 `Sankey` 可共用数据模型 |
| **B8** | **`EmojiPicker`** | 聊天/评论输入 | ❌ 缺失 | **需决策**：emoji 字形来自系统字体还是内嵌数据（见 §4.4） |
| **B9** | **`QueryBuilder`**（条件构建器） | 报表/筛选器/规则引擎的前端 | ❌ 缺失 | 与既有 `ColumnFilter` 可统一（见 §4.5） |

### 3.3 C 类：交互增强 —— 不是新控件，而是**既有控件的能力补齐**（建议做）

用户问「手势或触摸功能是否需要增强」。实跑后发现这些是**能力缺口**而非控件缺口：

| # | 能力 | 现状 | 归属 |
|---|---|---|---|
| **C1** | **通用拖放框架**（`DropZone` / `DragPayload` / `DataTransfer`） | ❌ 缺失。`Event::Drag` 存在，但**没有「可放置目标」的抽象**，所以 `KanbanBoard`/`SortableList` 都做不了 | 需新建 `src/event/dnd.rs` + `DropTarget` trait（**A1 的前置**） |
| **C2** | **`SortableList`**（拖拽排序） | ❌ 缺失，同因 C1 | 依赖 C1 |
| **C3** | **触觉反馈**（`haptic_feedback`） | ❌ 缺失（`grep "fn .*haptic"` → 0） | 「前沿平台新交互」；需要平台层（iOS `UIFeedbackGenerator`、Android `Vibrator`），**属 OS 相关，见 §五** |
| **C4** | **`sticky` 吸顶/吸附滚动** | ⚠️ 部分（`AppBar` 可固定，但**无通用吸顶**） | `ScrollArea` 增强 |
| **C5** | **协同光标 / 在线状态** | ❌ 缺失（`PresenceAvatar` 零命中） | 协作场景；但**需要网络层**，超出 GUI 库边界 → §五 |

### 3.4 D 类：登记但本轮**不做**（附理由）

| 项 | 为何不做 |
|---|---|
| `WorkflowGraph`（节点连线编辑器） | 需**图布局算法**（分层/避让/美观布线），是独立算法项目，不是控件工作量 |
| `SpreadsheetSheet`（电子表格） | 等于重写一个公式引擎 + 依赖图 + 引用重算；`GridTable` 已覆盖表格展示 |
| `FormulaEditor` | 同上层，需要解析器 + 求值器 |
| `JSONEditor` / `SQL` 语法编辑器 | `CodeEditor` 已支持自定义高亮规则，**属配置而非新控件**（规则 #78 的「能力已覆盖」） |
| `3D` / 材质视图 | 与「全自绘、无捆绑资源」定位冲突；`gpu` 模块为 2D 加速 |
| `PresenceAvatar` / 协同光标 | 需网络/CRDT 层，超出 GUI 库边界 |
| `TreeDataGrid` 的虚拟化重写 | `VirtualList` + `TreeTable` 已覆盖，差异是性能而非能力 |

---

## 四、判定：五个必须先回答的设计问题

### 4.1 `KanbanBoard` 与拖放基础设施的先后（规则 #24）

**结论：先做 C1（通用拖放），再做 A1（看板）。**

理由：`KanbanBoard` 的核心是「把卡片从 A 列放到 B 列」，这要求三样目前**都不存在**的东西：
① 可放置目标（`DropTarget` trait）；② 拖拽期间的载荷（`DragPayload`）；③ 放置预览与拒绝反馈。
若为看板单独实现一套，则 `SortableList`、`TileView` 重排、`TreeView` 跨层移动都要再各写一套。
**这是「基础设施先于控件」的典型判例。**

### 4.2 图表：扩展 `ChartType` 还是新建控件（规则 #80）

**结论：B2–B7 全部走「扩展 `ChartWidget`」，不新建控件。**

取证（实跑）：

```bash
$ grep -n "pub enum ChartType" -A 12 src/widget/special_widgets/chart.rs
pub enum ChartType { Bar, Line, Pie, Scatter }      # 仅 4 种
$ grep -rl "pub struct LineChart\|pub struct BarChart\|pub struct PieChart" src/widget/
src/widget/chart_widgets/charts.rs                  # 已有独立控件
```

**现状是「两条腿」**：`ChartWidget{chart_type}` 有 4 种，而 `LineChart`/`BarChart`/`PieChart`
又是独立控件——同一职责存在两种表达。**新增 6 种图之前必须先决定收敛方向**，否则会变成 3 条腿。

| 方案 | 做法 | 代价 |
|---|---|---|
| **① 收敛到 `ChartWidget`**（推荐） | 6 种新图作为 `ChartType` 变体；既有 `LineChart` 等逐步改为一层薄包装或删除 | 需迁移既有 3 个控件（有 C ABI 与 JSON 名，属破坏性或需别名） |
| ② 继续双轨 | 新图也各建控件 | 轴/图例/tooltip/命中测试重复 6 份；违反 #54 |

**本计划的建议**：走 ①，并把「`ChartWidget` 是唯一图表控件」写进模块文档。
`K 线/箱线/瀑布/雷达/桑基/漏斗` 与 `Bar/Line` **共享同一数据模型**（`Vec<f64>` + labels）
与同一交互，符合规则 #80 的扩展条件。**桑基图例外**：它需要「节点 + 有向边」，
与 `Vec<f64>` 不同构，应作为**独立控件**（见 §4.3）。

### 4.3 `Gauge` 与 `ProgressCircle` 的关系（规则 #78）

`ProgressCircle` 已存在。二者的差别是**语义**而非几何：

| | `ProgressCircle` | `Gauge` |
|---|---|---|
| 表示 | 完成度（0→max 的进度） | 某个**测量值**在量程中的位置 |
| 刻度 | 无 | **有**（主/次刻度 + 数值） |
| 区间 | 无 | **有**（安全/警告/危险色带） |
| 语义 | 「任务完成 60%」 | 「CPU 温度 72°C，红区」 |

**结论：可作为独立控件**（`Gauge`），但需在模块文档写明与 `ProgressCircle` 的分工，
避免成为「第 4 条腿」（规则 #74）。

### 4.4 `EmojiPicker` 的字形来源（**需用户决策**）

这是唯一一个**必须先拍板**的项，因为它触及项目定位：

| 方案 | 做法 | 代价 |
|---|---|---|
| **① 系统字体** | 依赖 OS 的 emoji 字体渲染 | 跨平台**显示不一致**；Linux 常无 emoji 字体 → 空白框 |
| **② 内嵌子集** | 打包常用 emoji 的字形数据 | 与「无捆绑二进制资源」定位冲突；体积显著增加 |
| **③ 只做「选择器外壳」** | 提供网格/搜索/最近使用/分类的**交互与 API**，字形由调用方注入 | **推荐**：不绑定位，且外壳（搜索、分类、最近、键盘导航）才是真正的控件工作量 |

**建议 ③**，并把「字形来源」作为显式参数暴露给调用方。

### 4.5 `QueryBuilder` 与既有 `ColumnFilter` 的统一（规则 #54）

实跑：`DataGrid` 已有 `set_filters(Vec<ColumnFilter>)`（`data_grid.rs:243`）。
`QueryBuilder` 是它的**可视化编辑器**（可嵌套的 AND/OR 树）。

**结论：不新建「第二套筛选模型」。** `QueryBuilder` 应产出**同一套** `ColumnFilter`
（必要时扩展为递归结构），使「代码设筛选」与「用户建筛选」共用一条链路。

---

## 五、明确不做（附理由，避免反复提议）

| 项 | 理由 |
|---|---|
| **触觉反馈（C3）** | 需 iOS `UIFeedbackGenerator` / Android `Vibrator` / Windows、Linux 各自 API，**属 OS 相关问题**，本轮不承诺（且用户已表示先排除 OS 项） |
| **平台视图过渡（`ViewTransition`）** | iOS 有原生支持，其他平台需自绘实现；可先用既有 `HeroAnimation` + `PropertyAnimation` 组合 |
| **网络协同（在线状态、CRDT 光标）** | 超出 GUI 库边界 |
| **3D / WebGL / 内嵌浏览器增强** | 与定位冲突 |
| **`FormulaEditor` / `Spreadsheet`** | 是「公式引擎 + 依赖图」项目，不是控件项目 |
| **Lottie / Rive 运行时扩展** | 已有 `LottieWidget`/`RiveWidget`；再扩是解析器工作量，非控件 |

---

## 六、执行计划（按依赖顺序，每步独立可验收）

### Phase A — 拖放基础设施（C1，**前置**）

| 步骤 | 内容 | 验收 |
|---|---|---|
| A-1 | `src/event/dnd.rs`：`DragPayload`（类型化载荷）、`DropEffect`（Copy/Move/Link/None）、`DropTarget` trait（`can_accept` / `on_drop` / `preview_rect`） | 单元测试：拒绝的载荷不进 `on_drop` |
| A-2 | 与既有 `Event::Drag` 接线：拖拽期间查询指针下的 `DropTarget`，产出 `DragEnter/DragOver/DragLeave/Drop` 事件 | **反向注入**：删掉 `DropTarget` 查询，看板测试必须失败 |
| A-3 | `DropZone` 可视化（高亮 + 放置预览） | 像素断言（规则 #73：断言颜色而不是「函数返回 Ok」） |

### Phase B — 范式新控件（A 类）

| 步骤 | 控件 | 关键交互 | 归属 |
|---|---|---|---|
| B-1 | `KanbanBoard` | 卡片跨列拖拽 / 列内排序 / 列折叠 / WIP 上限提示 | 依赖 Phase A |
| B-2 | `Heatmap` | 单元格 hover 值提示 / 色阶图例 / 行列表头 | `Canvas` |
| B-3 | `Treemap` | 层级下钻 / hover 高亮路径 / 面积标签 | `Canvas` |
| B-4 | `Cascader` | 逐级展开 / 路径回填 / 键盘导航 / 异步加载占位 | `Dropdown` + `TreeView` 模型 |
| B-5 | `Mention` | 字符触发 / 候选浮层 / 键盘选择 / 插入后光标定位 | `AutoCompleteEdit` |

### Phase C — 图表收敛 + 扩展（B2–B7）

| 步骤 | 内容 |
|---|---|
| C-0 | **先决策 §4.2 的方案 ①/②**（用户拍板），再动手 |
| C-1 | 扩展 `ChartWidget`：`Candlestick` / `Waterfall` / `Radar` / `BoxPlot` / `Funnel` 变体 |
| C-2 | `SankeyChart` 作为**独立控件**（数据模型不同构，见 §4.2） |
| C-3 | 既有 `LineChart`/`BarChart`/`PieChart` 的收敛或别名登记（规则 #54） |

### Phase D — 专业控件与增强

| 步骤 | 内容 |
|---|---|
| D-1 | `Gauge`（含刻度与区间色带；文档写明与 `ProgressCircle` 分工） |
| D-2 | `SortableList`（依赖 Phase A） |
| D-3 | `EmojiPicker` 外壳（按 §4.4 方案 ③，字形由调用方注入） |
| D-4 | `QueryBuilder`（产出既有 `ColumnFilter` 模型，见 §4.5） |
| D-5 | `ScrollArea` 通用吸顶（C4） |

### Phase E — 每个新控件的强制同步（规则 #81，**门禁已存在，会被自动追责**）

```bash
# 每新增一个控件必须让这四条同时通过：
cargo test --no-default-features --features desktop --lib -q          # 行为测试
bash tools/check_widget_registration_fidelity.sh                       # 工厂注册 + kind 归类
bash tools/check_control_has_tests.sh                                  # 每个控件有构建测试
bash tools/check_capability_matrix_truthfulness.sh                     # 属性双向断言
```

### Phase F — 文档与发布物

| 步骤 | 内容 |
|---|---|
| F-1 | 同步 `WidgetKind` 计数（现 **174**）到 `README.md`、`README.zh-CN.md`、`docs/ARCHITECTURE.md` |
| F-2 | 每个新控件补 `# Reachability` 段（规则 #72 三态） |
| F-3 | 日志回写完成率与证据 |

---

## 七、验证矩阵

| # | 检查 | 期望 |
|---|---|---|
| V1–V5 | 5 档 `cargo check --lib` | 全 `Finished` |
| V6 | `clippy --all-targets -- -D warnings` | 0 warning |
| V7 | `test --features desktop --lib` | 0 failed（当前基线 **4374**） |
| V8–V10 | embedded / mini / doc | 0 failed |
| V11 | 全部门禁（当前 **28** 个） | 全 PASS（`check_apple_native` 需 macOS 主机） |
| V12 | `check_widget_kind_count.sh` | 计数与文档一致 |
| V13 | 新增控件的**行为测试**（非仅构造） | 每个 ≥1 条（规则 #73） |

---

## 八、优先级建议

若只做一件事：**Phase A（拖放基础设施）**。理由：

1. 它是 `KanbanBoard`、`SortableList`、以及未来任何「重排/移动」交互的**共同前置**；
2. 它是目前**最明显的基础设施空洞**——`Event::Drag` 有事件却没有「放置目标」的抽象；
3. 按规则 #24/#71，**基础设施先于控件**，且它的收益不依赖任何一个具体控件是否落地。

若做第二件事：**Phase C-0 的决策 + 图表收敛**。理由：现状已存在「双轨」（`ChartWidget` vs
独立 `LineChart` 等），**不收敛就继续新增，会固定一个违反 #54 的结构**。

---

## 九、本文件的一句话结论

本项目**已有 174 个控件**，覆盖了用户提问中**超过一半**的「常见控件」——
`TimelineWidget`、`CommandPalette`、`NotificationCenter`、`DiffViewer`、`MarkdownEditor`、
`ToastStack`、`GanttWidget`、`VirtualList`、`DataGrid`（含冻结列）、`CodeEditor`、`TerminalView`、
`HeroAnimation` 等**都已存在**。

真正的前沿缺口集中在**三处**：

1. **拖放基础设施**（缺 `DropTarget`/`Payload` 抽象）—— 卡住了整个「重排类」交互族；
2. **专业可视化**（K 线、瀑布、雷达、箱线、桑基、漏斗、热力图、树图）；
3. **范式新控件**（看板、级联选择、提及、查询构建器、仪表盘）。

**其中两项必须先由用户拍板，不能由执行者单方面决定**：
- §4.2 图表是「收敛到 `ChartWidget`」还是「继续双轨」——影响既有 3 个控件的 C ABI 与 JSON 名；
- §4.4 `EmojiPicker` 的字形来源——触及「无捆绑二进制资源」的项目定位。
