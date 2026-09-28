#!/usr/bin/env python3
"""Writes the round-90/91 audit verdicts into the appendix-A rows that still lack one.

# What this is, and what it is not

Every verdict below is copied from a quoted `file:line` in `log-20260924-1.md` §90 — this tool
does not re-derive anything, it only files what was measured. That distinction matters: the
plan's value is that a row's defect cell states *which round compared it against the code*.
A row left stale is indistinguishable from one adjudicated, which is the drift §91.3 records.

# Why some rows are marked 留白 rather than resolved

`log §90` found ~10 genuinely open defects, and they are all *animation* work (a new state
machine, not a colour fix). Marking them 已修 would be false, and marking them with a generic
✅ would erase the distinction between "checked and fine" and "checked and still owed".
They get ⚠️ **仍留白** with the reason, so the next round can pick them up by grepping for it.

Usage:
  python3 tools/apply_round90_verdicts.py --check
  python3 tools/apply_round90_verdicts.py
"""

import re
import sys

PLAN = "docs/plans/archive/blue23.md"
LOG = "`log-20260924-1.md` §90"

# ── The verdicts ───────────────────────────────────────────────────────────────────────────
#
# Keyed by the control name in the row's first cell. Each value is
#   (defect-cell text, priority-cell text, marker)
# where `marker` picks the tail: "fixed" → ✅ 复核完成, "open" → ⚠️ 仍留白.
#
# Verdicts are transcribed from the audit reports in §90.1; the `file:line` that each rests on
# is quoted there, not repeated here, so there is one place to check.

FIXED = [
    # ── Containers ─────────────────────────────────────────────────────────────────────────
    (
        "`panel`",
        "✅ **第 90 轮实测复核**：计划记的「面无层级 ⇒ 应读 `surface_container`」**早已修** —— "
        "`groupbox.rs:593` 读 `LayerColor::SurfaceContainer`（且拒绝 theme_derived 的背景）。"
        "快照实测面板 `rgba(30,30,33)` ≠ 窗口 `rgba(18,18,18)`。原文「Panel 是 `pub type` ⇒ 无层级」"
        "**结构上仍真、视觉上已非缺陷**",
    ),
    (
        "`group_box`",
        "✅ **第 90 轮实测复核：三项都早已修** —— ① 勾取**框填充的 `contrast_color()`**"
        "（`groupbox.rs:666`，纯黑勾已除）；② `create_group_box` 仍不 `set_checkable`，但**导出器**"
        "的 `_checked` 额外外观已覆盖该状态（`export_control_svgs.rs:102`），两份快照都在；"
        "③ 标题占位走 `content_top()` = `FRAME_PADDING + title_band_height() + TITLE_CONTENT_SPACING`"
        "（`groupbox.rs:393`）",
    ),
    (
        "`tab_widget`",
        "✅ **第 90 轮实测复核：两项都早已修** —— ① `create_tab_widget` 已加两个 tab"
        "（`constructors.rs:841`：`add_tab(\"Tab 1\")`/`add_tab(\"Tab 2\")`）；"
        "② `set` 的 `text`/`title` 分支已补（`tabwidget.rs:822`：写第一个 tab，无 tab 则创建）",
    ),
    (
        "`scroll_area`",
        "✅ **第 90 轮实测复核：早已修** —— 滚动条的槽/框/滑块已从 `style` 解析"
        "（`scrollarea.rs:930-932`：`background_color`/`border_color`/`text_color`），"
        "与同文件 `draw_sticky_band` 的写法一致；余 3 处字面量是**无主题回落档**",
    ),
    (
        "`tool_box`",
        "✅ **第 90 轮实测复核：早已修** —— `item_rect` 已钳制（`toolbox.rs:360/370` 返回 "
        "`full.intersection(&strip)`），`content_rect` 保留 `MIN_CONTENT_EXTENT`，"
        "并有溢出滚动出口（`max_scroll`/`set_scroll_offset`）。BLUE21 D7 的「120 px 下 4 项归零」不再成立",
    ),
    (
        "`stacked_widget`",
        "✅ **第 90 轮实测复核：优势确认存在** —— `current_changed_suppression_reason`"
        "（`stackedwidget.rs:128`）对「禁用」与「索引越界」分别给出原因；`set_current_index` "
        "在禁用时**移索引但不 emit**（`:112-117`）",
    ),
    (
        "`masonry_layout`",
        "✅ **第 90 轮实测复核：三项中两项已修** —— ① 已有 `push_clip`/`pop_clip`"
        "（`masonry_layout.rs:213/243`），完全不可见的卡片被丢弃；② 标签走 `text_line`"
        "（`:233`，不再用 `+ h/2 - 6` 的 12 px 假设）；③ `corner_radius` 死绑定已除"
        "（单一 `CARD_CORNER_RADIUS = 6`，`:38`）。**余一项见下**",
    ),
]

OPEN = [
    (
        "`splitter`",
        "⚠️ **第 90 轮实测复核：两项都仍存在** —— ① **`HANDLE_WIDTH` 的去重没有落地**："
        "`dimensions::SPLITTER_HANDLE_THICKNESS`（`metrics.rs:731`）**零消费者**，而 "
        "`splitter.rs:303` `let handle_width = 5;` 与 `:427` `const HANDLE_WIDTH: f32 = 5.0;` "
        "**两处独立写法都还在**（`metrics.rs:729` 的注释声称已统一，代码与之矛盾）；"
        "② 拖拽**无实时反馈**：`draw`（`:220-351`）不读 `drag_session`/`active_pane`，"
        "拖拽中的把手与静止时**逐字节相同**",
    ),
    (
        "`dock_widget`",
        "⚠️ **第 90 轮实测复核**：① 24 px 标题栏**早已提取**为 `TITLE_BAR_THICKNESS`"
        "（`dockwidget.rs:208`，`title_bar_rect`/`content_rect` 共读）；"
        "② **无拖出/吸附动画仍成立** —— 全文件无 `PropertyDriver`/`tick`/动画",
    ),
    (
        "`mdi_area`",
        "⚠️ **第 90 轮实测复核**：① 标题越界**早已修**（`mdiarea.rs:828` 用 `text_line` + "
        "`draw_text_fitted`，并被关闭按钮约束）；② **无最小化/还原动画仍成立** —— "
        "`minimized` 只是个 `bool`（`:145`），无 `tick`/`PropertyDriver`",
    ),
    (
        "`collapsible_pane`",
        "✅ **第 90 轮实测复核：两项都早已修** —— ① 展开/收起**已有动画**"
        "（`collapsible_pane.rs:45` `open: PropertyDriver`，`:118` `tick`/`is_animating`/"
        "`open_progress`，内容带是 `open` 的函数）；② 头部高已改用 "
        "`dimensions::COLLAPSIBLE_HEADER_HEIGHT = 44`（`metrics.rs:541`），与 主流 material 实现 的 "
        "`ExpansionTile` 一致（计划的「24」是旧值）",
    ),
    (
        "`safe_area`",
        "⚠️ **第 90 轮实测复核：M8 仍留白** —— `safe_area.rs:24` 仍存四个**物理**边"
        "（`top`/`bottom`/`left`/`right`），`content_rect`（`:109`）用 `insets.left` 与 "
        "`left + right`，**无 `start`/`end`**，绘制也填物理左右条（`:255`），故 RTL 不镜像",
    ),
    (
        "`stepper`",
        "⚠️ **第 90 轮实测复核：计划判为准确的** —— `stepper.rs:40` 的字段是 "
        "`value`/`min`/`max`/`step`，API 是 `increment`/`decrement` ⇒ 它是**数值微调器**；"
        "主流 material 实现 的 `Stepper`（分步向导，`StepState` 5 态 + 连接线）**本仓没有**。"
        "二选一：**改名** 或 **补向导控件** —— 不留悬空",
    ),
]

# Rows the audit found already-fixed but whose claim was a different sentence than the ones
# above; kept separate so the reason each was dismissed is visible.
FIXED_MORE = [
    (
        "`virtual_list`",
        "✅ **第 90 轮实测复核：三项都齐** —— 读主题（`virtual_list.rs:422` "
        "`resolved_theme_style`）、行 hover（`:480` `hovered_row`）、base 转发"
        "（`:508` `self.base.handle_event`）。与 `data_view` **同 kind 同文件**（`:55` "
        "`WidgetKind::DataView`），故修一次覆盖两个",
    ),
    (
        "`data_view`",
        "✅ **第 90 轮实测复核**：与 `virtual_list` **同文件同 kind**（`virtual_list.rs:55`），"
        "同一 `Draw` 路径 ⇒ 随上一行一并覆盖",
    ),
    (
        "`table`",
        "✅ **第 90 轮复核**：与 `table_widget` **同为 `Table` kind**（同一 `Draw` 路径）⇒ "
        "修一次覆盖两个，已随第 74 轮完成",
    ),
    (
        "`properties_panel`",
        "✅ **第 90 轮实测复核：A14 早已修** —— 行几何已统一到 `row_rect_for`"
        "（`properties_panel.rs:262`），绘制（`:486`）与命中（`:598`）共读一处，"
        "标签走 `text_line`（`:519`）。**M1 行 hover 与 M6 `outline_variant` 仍留白**"
        "（分隔线是 `background.blend(&text_color, 0.14)`，`:461`）",
    ),
    (
        "`property_grid`",
        "✅ **第 90 轮实测复核：M6 已满足** —— `separator = surface.blend(&ink, 0.25)`"
        "（`property_grid.rs:323`）/ `row_separator = blend(&ink, 0.12)`（`:332`），"
        "两个派生色都随主题变且与焦点环（`outline` 角色）不同源 ⇒ 满足 M6 的**判据**"
        "（行线色 ≠ 焦点环色）。计划的「改读 `outline_variant`」是**手段**，不动",
    ),
    (
        "`image_gallery`",
        "✅ **第 90 轮实测复核：字面量已 14 → 7，且已有到达动画** —— "
        "`image_gallery.rs:379` 的 `arrive()` 由 `reveal` 缩放（`:245` `tick`/`is_animating`）。"
        "**注意它是「淡入」不是「缩放」**；余 7 处字面量仍待清",
    ),
    (
        "`message_box`",
        "✅ **第 90 轮实测复核：M3 已有** —— `reveal` 驱动器 + `scale_about_centre`"
        "（`message_box.rs:704`）；**M4/M5 仍留白**：6 处字面量是回落档，且「primary」是"
        "从 `background_color` 再推的（`:752`），不是 `primary` 角色",
    ),
    (
        "`file_dialog`",
        "✅ **第 90 轮实测复核：D3 早已修** —— 列表高 `list_h` 由 `button_top - list_y` 推导"
        "并有行盒下限（`file_dialog.rs:519`），占位符走 `text_line` 居中并被裁剪（`:548`）。"
        "**M10（列宽可拖）仍留白**：全文件无列模型与拖拽柄；M5 不适用（列表是占位符，无行线）",
    ),
    (
        "`color_dialog`",
        "✅ **第 90 轮实测复核：D1 与 B11 都早已修** —— 取色区高由 `below_top - picker_top - "
        "PICKER_GAP` 推导（`color_dialog.rs:222`，不再为 0）；OK 用 `accent`、Cancel 用 `surface`"
        "（`:518`/`:521`，不再同填）。**余 9 处字面量仍留白**（如 `:475` `rgb(200,200,200)`）",
    ),
    (
        "`font_dialog`",
        "✅ **第 90 轮实测复核：D2 部分已修** —— 三列已拿到真实高度"
        "（`font_dialog.rs:364` 推导 `list_h`/`preview_h`），快照不再有 `height=\"0\"` 列。"
        "**M4/M5 仍留白**（面是 `window_fill.blend(&ink, 0.06)`，`:265`，未读 `surface_container`）；"
        "**M10 部分**：预览是固定样本串（`:417`）",
    ),
    (
        "`popover`",
        "✅ **第 90 轮实测复核：A22 与 M3 都早已修** —— 标签走 `text_line` + `draw_text_fitted`"
        "（`popover.rs:463`，不再贴顶 8 px）；展开由 `reveal` 驱动高度（`:383`，`:236` "
        "`tick`/`is_animating`）。**M5 仍留白**：卡片面是 `window_fill.blend(&ink, 0.08)`"
        "（`:332`），未读 `surface_container_high`",
    ),
    (
        "`popup_window`",
        "✅ **第 90 轮实测复核：常量早已提取** —— `TITLE_BAR_HEIGHT`（`popup_window.rs:269`）"
        "被 `draw`（`:237`）与 `content_rect`（`:281`）共读。**M5 仍留白**：面未读 "
        "`surface_container`（`:203`）",
    ),
    (
        "`banner`",
        "✅ **第 90 轮实测复核：语义 token 早已接** —— `semantic_color(self.semantic())`"
        "（`banner.rs:107`）再推离承载面（`:127` `token.blend(&surface, 0.85)`）；3 处字面量是回落档。"
        "**M3（滑入/滑出）仍留白** —— 全文件无 `tick`/`is_animating`",
    ),
    (
        "`fab`",
        "✅ **第 90 轮实测复核：点击契约早已修** —— 释放时要求 `is_pressed()` **且** "
        "`contains_point_with_touch_expansion(pos)`（`fab.rs:295-304`），并有 `MouseLeave` 取消臂"
        "（`:306`）。**M3 仍留白**：无 `tick`/涟漪，按下是**瞬时** 10% 收缩（`:172`）",
    ),
    (
        "`splash_screen`",
        "✅ **第 90 轮实测复核：淡出早已实现且为 opt-in** —— `fade_out()` 设目标 0"
        "（`splash_screen.rs:181`），`tick`/`is_animating` 在 `:256`/`:260`，淡出作用于每个颜色"
        "（`:423-435`）；4 处字面量是主题前的回落档",
    ),
    (
        "`find_replace_dialog`",
        "✅ **第 90 轮实测复核：确认是 `text_line` 的正确写法参照** —— 每个标签都走 "
        "`self.text_line(...)`（`find_replace_dialog.rs:578`，助手在 `:419`），无手算 `y + h/2`",
    ),
    (
        "`input_dialog`",
        "✅ **第 90 轮实测复核：优势成立** —— 主题的 `primary` 被真正使用"
        "（`input_dialog.rs:517` 取 `active.colors.primary`，`:664` 填 OK 钮），"
        "Cancel 用 `surface.blend(&ink, 0.1)`；接受键在 `handle_event`（ENTER → accept）",
    ),
    (
        "`refresh_control`",
        "✅ **第 90 轮实测复核：假欠债** —— `tick`（`refresh_control.rs:265`）与 `is_animating`"
        "（`:270`）都在，trait 桥接在 `:300`/`:304`，并有一个 `settle` 驱动器",
    ),
    (
        "`bottom_sheet`",
        "✅ **第 90 轮实测复核：遮罩早已接 `scrim` 角色** —— "
        "`layer_color(LayerColor::Scrim)`（`bottom_sheet.rs:300`），`window_fill.blend(BLACK, "
        "SCRIM_DARKEN)` 只作主题无角色时的回落；升起同时驱动遮罩淡入（`:306`）",
    ),
    (
        "`modal_bottom_sheet`",
        "✅ **第 90 轮实测复核：同上** —— `layer_color(LayerColor::Scrim)`"
        "（`modal_bottom_sheet.rs:343`）。注意 `ink.blend(&sheet_fill, 0.55)` 仍存在，"
        "但**只在主题早于该角色时**走；暗预设下 `scrim = rgba(0,0,0,130)`，"
        "并有测试 `the_scrim_composites_darker_than_the_backdrop`（`:686`）",
    ),
    (
        "`dialog`",
        "✅ **第 90 轮实测复核：两项都早已修** —— 遮罩读角色"
        "（`dialog/mod.rs:70` `layer_color(LayerColor::Scrim)`）；出现/消失有缩放 + 遮罩渐显"
        "（`dialog_widget.rs:406` `scale` 由 `reveal` 驱动，`:251` `tick`/`is_animating`）",
    ),
    (
        "`virtual_table`",
        "✅ **第 90 轮实测复核：M6 已随第 77 轮完成** —— 格线改读 `outline_variant`，"
        "与 `table_widget`/`tree_table`/`data_grid` **同一来源**。它本身是**纯滚动视口**"
        "（有 `row_height`/`scroll_row`，**无选中概念**），故「M1 行 hover」是推广而非缺陷",
    ),
    (
        "`tree_table`",
        "✅ **第 90 轮复核：M6 已随第 77 轮完成**（格线读 `outline_variant`）。"
        "**展开动画（M3）仍留白**",
    ),
    (
        "`property_grid`",
        "✅ **第 90 轮复核**：见上文同项 —— M6 判据已满足（两个派生分隔色）",
    ),
]

# Rows whose audit verdict is "still open" for a reason other than animation.
OPEN_MORE = [
    (
        "`grid_table`",
        "⚠️ **第 90 轮实测复核：M6 仍留白** —— `grid_table.rs:782` 用 "
        "`grid_color = surface.blend(&ink, 0.15)`（派生色），未读 `outline_variant`。"
        "**列头 D14 已在本轮修**：协议加可选 `column_name`（`data_source.rs`），"
        "控件先问数据源、`Column {n}` 保留为诚实回落，断言 + 反向注入见 §90.3",
    ),
    (
        "`tree_view`",
        "⚠️ **第 90 轮实测复核：两项都仍存在** —— ① 展开/收起**不存在也无动画**："
        "`TreeModel` 只暴露扁平的 `node_count`/`node_path`（`tree_view.rs:39`），"
        "`draw` 遍历每个节点（`:398`），无展开操作；② **无缩进引导线**（`:413` 只画标签）。"
        "正面：读主题（`:358`）、有行 hover（`:406`）、转发 base（`:438`）",
    ),
    (
        "`query_builder`",
        "⚠️ **第 90 轮实测复核：仍存在** —— 增删行直接改向量（`query_builder.rs:237` "
        "`rows.push`、`:252` `rows.remove`），无 `tick`/`is_animating` ⇒ 行在帧之间突然出现/消失",
    ),
    (
        "`properties_panel`",
        "⚠️ **第 90 轮实测复核：M1/M6 仍留白** —— 无行 hover；分隔线是 "
        "`background.blend(&text_color, 0.14)`（`properties_panel.rs:461`），非 `outline_variant`",
    ),
    (
        "`wizard_dialog`",
        "⚠️ **第 90 轮实测复核：两项都仍存在** —— ① 步骤切换**无动画**"
        "（`wizard.rs:105`/`:145` 直接设 `current_step`，全文件无 `tick`/`PropertyDriver`）；"
        "② 「真正的向导」仍缺：`WizardStep` 只有 `title`/`completed`/`optional`（`:32`），"
        "步骤体不是可承载的子控件 ⇒ 它是进度/导航条，不是内容切换的向导",
    ),
    (
        "`progress_dialog`",
        "⚠️ **第 90 轮实测复核：仍存在** —— 填充是值的纯函数"
        "（`progress_dialog.rs:480` `bar_band.width * progress_fraction()`，无时间项），"
        "全文件无 `tick`/`is_animating` ⇒ 不确定态不转。**M5 也留白**：轨道/按钮是 "
        "`surface.blend(&ink, 0.12)`（`:383`）",
    ),
    (
        "`order_book`",
        "⚠️ **第 90 轮实测复核：三项中两项仍存在** —— ① `hovered: Option<(BookSide,usize)>` "
        "与 `level_hovered` **确认存在**（`order_book.rs:61`/`:65`，`:295` emit）；"
        "② **M4 仍留白**：`:345` `(rgb(240,240,240), BLACK, rgb(158,158,158))` 是无主题回落三元组，"
        "而 `:80` `text_color: rgb(214,220,228)` 在 `:458` 被**无条件**用作数量墨 ⇒ 主题改不动它；"
        "③ **M3（价格跳动闪烁）仍留白**：全文件无 `tick|is_animating|flash`",
    ),
    (
        "`candlestick_chart`",
        "✅ **第 90 轮实测复核：面板色早已接** —— `panel_colors(Some(self.base.style()))`"
        "（`candlestick_chart.rs:260`），共享入口在 `finance/layout.rs:649`。"
        "**M12 仍留白**：空态只画网格矩形 + 一条中线 + 「No data」（`:534-543`），**无价格/序号轴**",
    ),
    (
        "`volume_chart`",
        "✅ **第 90 轮实测复核：面板色早已接**（`volume_chart.rs:162`）。"
        "**M12 仍留白**：空态走 `draw_empty_pane`（`:253`，实现 `:311` 只画网格 + 中线 + 提示）；"
        "四条网格线循环（`:263`）只在**有数据**的路径上跑",
    ),
    (
        "`depth_chart`",
        "✅ **第 90 轮实测复核：面板色早已接**（`depth_chart.rs:96`）。"
        "**M12 仍留白**：空态走 `draw_empty_pane`（`:397`，与 `volume_chart` 同实现）",
    ),
    (
        "`indicator_chart`",
        "✅ **第 90 轮实测复核：面板色早已接**（`indicator_chart.rs:520`），"
        "且**空态是四个金融图里唯一画了刻度的**（`:485-495` 画 4 条网格线）。"
        "**仍缺数值刻度标签** ⇒ 比三个同族好，但未印值",
    ),
    (
        "`quote_board`",
        "✅ **第 90 轮补修**（`%s`）：第 88 轮修了**列标题**错列；本轮修**行值** —— "
        "`cell_text` 返回调用方无界的 `symbol`/`name`，而 `draw_text` 不问放不放得下 ⇒ "
        "长值横穿下一列。改为先算单元格框再 `draw_text_fitted`。"
        "**快照逐字节不变**（普查数据够短）⇒ 此项只有代码层面证据",
    ),
    (
        "`radar_chart`",
        "⚠️ **第 90 轮补修**（`%s`）：`chrome_colors()` 覆盖了面/框/网格/标签/空态**五色**，"
        "但**漏了三个消费者** —— 图例文字 `:706` `rgb(70,70,70)`、悬停辐条 `:730` "
        "`rgb(120,120,120)`、悬停读数 `:746` `rgb(60,60,60)`（暗底暗字）。"
        "已把 `legend_ink` 与 `hover_spoke` 加进 `RadarChrome`。实测三色在快照里**归零**",
    ),
    (
        "`emoji_picker`",
        "⚠️ **第 90 轮补修**（`%s`）：`\"emoji_picker\"` 不在 `WidgetRole` 表 ⇒ 归类 `Surface` ⇒ "
        "解析到 `theme.colors.background`（窗口自己的色）⇒ **面板与它浮在其上的窗口同色，弹层无边缘**。"
        "改为：窗口色的解析结果视为「没有答案」，退 `surface_container`（与 `drop_zone`/`otp_input` 同一守卫），"
        "显式设置的颜色仍最优先。实测面板 `rgba(30,30,33)` ≠ 窗口 `rgba(18,18,18)`",
    ),
]

VERDICTS = {}
for entry in FIXED + FIXED_MORE:
    VERDICTS[entry[0]] = (entry[1], "✅ **复核完成**")
for entry in OPEN + OPEN_MORE:
    VERDICTS[entry[0]] = (entry[1], "⚠️ **仍留白**")

SEPARATOR = re.compile(r"^\|[-: |]+\|$")


def cells(row):
    text = row.strip()
    if text.endswith("|"):
        text = text[:-1]
    return [part.strip() for part in re.split(r"(?<!\\)\|", text.strip("|"))]


def main(argv):
    check_only = "--check" in argv
    lines = open(PLAN, encoding="utf-8").read().splitlines(keepends=True)

    applied = 0
    seen = set()
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.startswith("| `") and " | " in line:
            parts = cells(line)
            name = parts[0]
            # Only touch rows that have not been adjudicated yet, so a later round's more
            # specific verdict is never overwritten by this one.
            if name in VERDICTS and name not in seen:
                defect, marker = VERDICTS[name]
                if "轮" not in parts[3]:
                    defect = defect.replace("%s", LOG)
                    parts[3] = defect
                    parts[-1] = marker
                    lines[index] = "| " + " | ".join(parts) + " |\n"
                    applied += 1
                    seen.add(name)
        index += 1

    print("rows written: %d" % applied)
    leftovers = sorted(set(VERDICTS) - seen)
    if leftovers:
        print("verdicts with no matching un-adjudicated row (%d):" % len(leftovers))
        for name in leftovers:
            print("  %s" % name)
    if not check_only and applied:
        open(PLAN, "w", encoding="utf-8").writelines(lines)
        print("wrote %s" % PLAN)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
