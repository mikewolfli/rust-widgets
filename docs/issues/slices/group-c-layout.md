| R163 | 布局系统 | **AbsoluteLayout** ↔ declarative UI anchors：仅 9 点锚 + min/max/aspect；缺 `fill`、`centerIn`、逐边 margin、baseline 锚；锚定子项无动画（declarative UI 有 Behavior）。 | 406 | ⬜ |
| R164 | 布局系统 | **AspectRatioLayout**：整型除法奇偶舍入不对称（:117-118）→ div_round；子项无 cover/contain 模式枚举。 | 406 | ⬜ |
| R165 | 布局系统 | **BorderLayout**：边区 extent 显式给定（:209-224）不读子项 sizeHint → 接入 hint 通道作默认。 | 406 | ⬜ |
| R166 | 布局系统 | **BoxLayout** ↔ box layout：SizePolicy 仅 Fixed/Preferred/Expanding（types.rs:11-18，参考工具包 7 值缺 4）；缺 `insert_widget_at`/insertStretch（仅 add_widget 追加 :554-563）；缺 `set_orientation` 运行时换向；兜底分配 `while total_assigned > primary` 每轮减 1（:357-368）最坏 O(分配总量) → 按比例一次截断。 | 406 | ⬜ |
| R167 | 布局系统 | **ConstraintLayout**：自认"非完整 Cassowary 求解器"（:7-8）——无优先级/强度、无过约束仲裁；约束链无环检测证据（resolve_rect :110-212）→ 加环检测并文档化解析顺序。 | 406 | ⬜ |
| R168 | 布局系统 | **DeclarativeLayout**：布局注册表 `thread_local`（:56-57）跨线程 UI 树不可用；无失效-延迟激活（invalidate→合并重算）→ 补脏标记。 | 406 | ⬜ |
| R169 | 布局系统 | **FlexLayout**：`AlignItems::Baseline` 已声明（:68）但 compute_cross_positions（:710-754）无文本基线测量证据，疑似普通对齐冒充 → 真实现或删除变体；缺 `flex-basis`/`order`。 | 406 | ⬜ |
| R170 | 布局系统 | **FlowLayout/WrapLayout**：完整 ≈优（run_spacing 超越 参考工具包 Flow）；RTL 不镜像（全 crate 明示不做，core/text_direction.rs:19-20；declarative UI RTL 自动反向）。 | 406 | ⬜ |
| R171 | 布局系统 | **FormLayout**：策略枚举齐（:16-42）≈；**标签列固定 1/3 宽不测量内容**（:385）→ 用 hint 通道取 pref 宽；缺 labelAlignment/buddyFor/itemAt(role)/跨单元 setLayout。 | 406 | ⬜ |
| R172 | 布局系统 | **GridLayout** ↔ grid layout：高对齐（stretch/span/对齐/hint 通道 :639-644）；缺 setOriginCorner/全量 getItemPosition 导出。 | 406 | ⬜ |
| R173 | 布局系统 | **hints.rs**：`AxisHints` min/pref/max（:94-114）是优秀收敛，但 **Widget trait 无 `minimum_size_hint()`**（全项目零命中）——参考工具包 由各控件覆写 minimumSizeHint；下游症状：Splitter 可压碎窗格、FormLayout 标签列固定、Accordion 体高恒定。 | 406 | ⬜ |
| R174 | 布局系统 | **SplitterLayout**：无最小窗格尺寸（权重仅 max(0.01) 地板 :69/:176，可拖到 0 宽）；无 collapsible 语义（等价永远 true 且不可关）；**无 saveState/restoreState**（全项目零命中）。 | 406 | ⬜ |
| R175 | 布局系统 | **StackLayout**：缺 `StackAll` 平铺模式；页切换零动画（:119-123）。 | 406 | ⬜ |
| R176 | 布局系统 | **types.rs**：`Layout` trait 无 sizeHint/invalidate/expandingDirections（:114-219）——布局不能向上聚合需求，窗口最小尺寸只能靠平台 set_window_min_size（app/handle.rs:1448-1452）；3/7 SizePolicy；无 heightForWidth 协商链。 | 406 | ⬜ |
| R177 | 容器控件 | **Accordion** ↔ tool box：体高固定常量 BODY_HEIGHT（:34）不按内容 sizeHint；无展开/折叠动画（CollapsiblePane 已有可复用 :128-138）；无键盘导航。 | 422 | ⬜ |
| R178 | 容器控件 | **Carousel** ↔ declarative UI SwipeView：指示器/autoplay/loop/drag 状态机/速度测量（:597-627）超越同类；**松手判定后直接切页，无回弹补间、无边缘 rubber-band**（loop=false 硬停 :1847-1857）→ AnimationDriver 加 200ms 回弹。 | 422 | ⬜ |
| R179 | 容器控件 | **CollapsiblePane**：open_progress+tick 动画 ≈优；无状态持久化。 | 422 | ⬜ |
| R180 | 容器控件 | **DockWidget** ↔ dock widget：features/allowedAreas/floating/信号齐（:51-133）；**拖拽仅移动自身（drag_offset :47），无拖到边缘的落坞指示/吸附**；**无 main window/DockArea 宿主容器**（全项目无等价物——结构性缺口）；无 tabify/saveState/setTitleBarWidget。 | 422 | ⬜ |
| R181 | 容器控件 | **GroupBox**：checkable/checked/alignment 齐；缺 `flat` 模式；size_hint（:415-433）与内容无关不协商。 | 422 | ⬜ |
| R182 | 容器控件 | **linear_layouts.rs** ↔ declarative UI Row/Column/Flow：**子项不是真实 Widget**（FlexItemSpec 仅 label/size/grow :53-57，合成 id :91-93）——无子控件事件/可达性/焦点 → 允许挂真实子件 id；`item_rects` 每次全量重算（:269-285）无缓存。 | 422 | ⬜ |
| R183 | 容器控件 | **MasonryLayout**：**每次 draw 重算全量布局**（:208 draw 内调用）、无虚拟化、无脏区（:230-232 仅绘制时裁）→ 缓存+视口提前 break；卡片无命中测试/点击事件。 | 422 | ⬜ |
| R184 | 容器控件 | **MdiArea** ↔ mdi area：级联/平铺（√cols 算法与 参考工具包 一致）/arrange_icons/activateNext-Previous/TabbedView 齐备；**ActivationOrder 仅存取，next/previous 遍历不读它**（:564-580 固定数组序）；**子窗不可拖边缩放**（resizable 标志存在 :52 但 handle_event 只做激活+关闭钮）；无标题栏双击最大化/还原、无滚动背景。 | 422 | ⬜ |
| R185 | 容器控件 | **SafeArea**：发布 content_rect 但**不排布子项**（:30-34）→ 提供 apply() 或包装布局变体。 | 422 | ⬜ |
| R186 | 容器控件 | **ScrollArea** ↔ scroll area/NSScrollView：widgetResizable/alignment/三种 ScrollBarPolicy 与 参考工具包 同名/ensure_visible/children_clip ≈齐；**无惯性/物理滚动**：Wheel 固定×20（:992-993）、Swipe `velocity: _` 显式丢弃（:997）、Drag 1:1（:1004-1009）；gesture 层有 Fling 识别（gesture/swipe.rs:248-249）但未被消费；**滚动条 thumb 不可拖**（有 thumb_metrics :662-676 但 handle_event 无命中分支 :988-1011）；无键盘滚动（PgUp/PgDn/方向键不在 match 内）。 | 422 | ⬜ |
| R187 | 容器控件 | **Splitter 控件**：拖拽阈值会话/绝对增量幂等/释放归一 ≈质量高；无 collapsible、无每窗格最小尺寸、无双击均分、无 saveState。 | 422 | ⬜ |
| R188 | 容器控件 | **StackedWidget**：语义与 参考工具包 对齐 + 禁用抑制可查询（超出 参考工具包）；页切换无过渡（与 widget toolkit 持平，落后 declarative UI StackView）。 | 422 | ⬜ |
| R189 | 容器控件 | **Stepper** ↔ the reference stepper：完整。 | 422 | ⬜ |
| R190 | 容器控件 | **TabWidget** ↔ tab widget/tab bar：4 方位/3 形状/closable/movable 拖重排/宽度指纹缓存 ≈强；**溢出无滚动按钮**（放不下均分压到 MIN_TAB_WIDTH/2 :751-757；参考工具包 出左右滚动箭头）；**无省略号截断 setElideMode**（长标题夹断不加 "…"）；无 Ctrl+Tab 循环、无 cornerWidget、tooltip 存（:104）但无悬停显示通路。 | 422 | ⬜ |
| R191 | 容器控件 | **ToolBox** ↔ tool box：增删插/orientation 可换（超集）/overflow 滚动/键盘导航 ≈；ITEM_HEIGHT 固定不随内容/字体（:25）；选中切换无滚动补间（:449-464 直接钳位）。 | 422 | ⬜ |
