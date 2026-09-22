# BLUE21 — 退化/降级/不规范/不完整控件登记表（外形·尺寸·字体·外观·布局·主题·属性·方法）

> 状态：**登记完成，待修复**（本文件是**缺陷登记表**，不是完成报告）
> 触发：用户指令（2026-09-22）——「依据 `docs/plans/principle.md` 为原则，对控件实际外形、尺寸、
> 字体、外观、布局、主题、属性、方法进行仔细核对，将退化降级、不规范、不完整控件列到 `blue21.md`，
> 最好比对 svg 和其他库（flutter, qt-qml, swiftui）；**重点组合控件**；
> **声明式 + 保留混合，完整条件也需要检查补齐**」
> 原则依据：[`principle.md`](principle.md)（继承 BLUE1–BLUE20 全部规则，含 #1–#111）
> 取证方法：`snapshots/svg/`（376 个 SVG，188 控件 × 明暗两态）+ 源码逐处核对 +
> Flutter / Qt(QML) / SwiftUI 对位比对，**每条均附源码行号或 SVG 原文**
> 上轮日志：[`docs/log/log-20260922-1.md`](../log/log-20260922-1.md)（第 62 轮）
> 本轮日志：[`docs/log/log-20260922-2.md`](../log/log-20260922-2.md)

---

## 零、判据（本表用什么标准判定「退化/降级/不规范/不完整」）

遵循原则 #102–#111（BLUE20）与 #108 的三分类。八条判据，**每条都能失败**：

| # | 判据 | 缩写 | 依据 |
|---|---|---|---|
| J1 | 文本原点语义：`origin.y` 是字形框**上边缘**，因此单行标签在其色带内必须满足 `y == band.y + (band.h - line_h)/2`；写成 `band.y + band.h/2`、`band.y + band.h/2 + k`、或 `band.y`（HUG-TOP）都是缺陷 | **J1-居中** | #102 / 第 62 轮 §14.1 |
| J2 | 单行文本**不得**在框内出现未绑定的 `ascent`（源码级判据已有门禁） | **J2-ascent** | #102 |
| J3 | 颜色的三类归属（外观色 / 语义色 / 数据色）必须逐处成立；「读主题但被写成永远命中的 `or()` 链」= 意图不可达 | **J3-配色** | #108 |
| J4 | 声明了就必须有人读：属性/事件/枚举 token，凡公开则在 `get`/`set`/`Draw`/`event_signal_dyn` 全链可达 | **J4-声明闭环** | #109 / BLUE19 #97 |
| J5 | 空白态与默认态必须画出**该有的 chrome**（分隔条、指示器、面板面、刻度），不得与背景同色而「看不见」 | **J5-可见性** | #103 |
| J6 | 尺寸与位置必须**由构造推出**，不得由两个各自手调的字面量相加/相减而得（`h - <设计高度常量>` 在 h 等于该常量时归零即属此类） | **J6-构造性** | #50 / 第 62 轮 §14.3 |
| J7 | 组合控件的**子区域关系**必须有结构（子项和 ≤ 容器、面板有最小值、溢出有出口），不得线性相减 | **J7-组合** | #50 |
| J8 | 声明式（`src/view/`）与保留（retained）两条路径对**同一属性/事件**必须等价：写法不请求重绘、diff 产出必然被拒的 patch、发布名不可解析 = 不完整 | **J8-混合等价** | #101 / 用户指令 #2 |

**不判为缺陷**（防误报，沿用 BLUE20 §4.5 / #108 ③）：
数据色（系列调色板、色相光谱、K 线涨跌红绿、热力图色阶、仪表阈值带）、
刻意淡化的次级文字（占位符/水印/未选中星）、
容器铺满 240×120 普查矩形（canvas 就是控件 rect）、
`window` 只画客户区（标题栏归窗口管理器，**刻意且有据**）、
动画不在声明式层实现（`mod.rs` 已声明边界，属设计决定）。

---

## 一、一句话结论

**本轮抓到的最大一类不是「某个控件画错了」，而是「一个写法被复制到了 N 个文件」——
因此修法的单位是「写法」，不是「控件」。**

三类跨文件复制最严重：

1. **`band.y + band.h/2` 当居中用** —— 76 处以上，横跨 base/input/dialog/data-table；
   而正确写法在本仓**已经存在**（`find_replace_dialog.rs:287-291` 的 `text_line`、
   `tab_bar.rs:622`、`badge.rs:294`、`roller.rs:331`），所以这是「修一处、漏同类」的标准例（原则 #2）。
2. **`Surface` role 兜底把「窗口底色」当控件底色** —— 一个控件名不在 `WidgetRole::for_kind_name`
   （`src/theme/types.rs:104-156`）里，就拿到 `theme.colors.background`，
   于是 `or(themed_x)` 之后的**所有 rung 都不可达**，`badge` / `scroll_bar` 的滑块 /
   `otp_input` / `drop_zone` / `signature_pad` / `tool_button` / `color_dialog` 的按钮
   **在明暗两态下都隐形**。
3. **`Color::rgb(18, 22, 28)` 被复制到 4 个金融图表** —— `candlestick_chart` / `volume_chart` /
   `depth_chart` / `indicator_chart` 的绘图面是同一个字面量，四份 SVG 的 `diff` 各自只有 4 行
   （2 行注释 + 1 行导出器底色 + 1 行自己），即**四个整控件主题盲**。

---

## 二、缺陷登记表（按判据分组）

字号说明：`🔴` = 用户可见的功能/外观错误；`🟠` = 不一致或不可达的意图；`🟡` = 卫生/可维护性。

### A. J1-居中：文本原点用了色带中线（或中线 + 手调常量）

> **共同根因**：渲染器契约是「`origin.y` = 字形框**上边缘**」（`src/render/backend/surface.rs:456-460`、
> `:507-533`；SVG 后端配 `dominant-baseline="text-before-edge"`）。写成 `band.y + band.h/2`
> 就等于把**字形框的上边缘放在色带中线上**，整行下移半个行盒（14 px 字 ≈ 7 px）。
> **该写法在全仓 76+ 处**（`grep -rn "height as i32 / 2"` 命中 110，其中约 3/4 属此类）。

| # | 控件 | 严重度 | 证据（源码 / SVG） | 平台对位 | 修法方向 |
|---|---|---|---|---|---|
| A1 | `button` | 🔴 | `base_widgets/button.rs:404` `y: rect.y + rect.height as i32 / 2`；`button.svg` `<text x="6" y="60">` 于 `[0,120]` 带（应为 `53`） | Flutter `Text`+`Center`；Qt `QStyle::alignedRect(AlignVCenter)`；SwiftUI 默认 `.center` | 走共享 `text_line(band, font, ctx)` |
| A2 | `check_box` | 🔴 | `base_widgets/checkbox.rs:354-357`（`checkbox_rect.y + h/2`）；`check_box.svg` `<text x="20" y="60">`，且复选框本身 `y=52..68` | Qt `QCheckBox` 用 `Qt::AlignVCenter` 于内容矩形 | 同上；复选框 16 px 是字面量（见 B6） |
| A3 | `radio_button` | 🔴 | `base_widgets/radiobutton.rs:220` `center.y`；`radio_button.svg` `<text x="154" y="60">` | Qt `PE_IndicatorRadioButton` | 同上 |
| A4 | `combo_box` | 🔴 | `input_widgets/combobox.rs:352`（用于 `:389`、`:398`）；`combo_box.svg` `<text x="4" y="60">(Empty)` | Qt `QComboBox` / Flutter `DropdownButton` 居中当前值 | 同上；箭头与文本共用一个锚 |
| A5 | `date_edit` / `time_edit` / `date_time_edit` | 🔴 | `advanced_widgets/date_edit.rs:598`、`time_edit.rs:556`、`date_time_edit.rs:561`；三份 SVG 均 `<text y="60">` | Qt `QDateEdit`/`QTimeEdit` 居中 | 同上 |
| A6 | `shortcut_editor` | 🔴 | `advanced_widgets/key_sequence_edit.rs:406` | Qt `QKeySequenceEdit` | 同上 |
| A7 | `status_bar` | 🔴 | `menu_toolbar/status_bar.rs:188`、`:203`；`status_bar.svg` `<text x="6" y="60">` | Qt `QStatusBar`（`WindowText` 居中） | 同上 |
| A8 | `banner` | 🔴 | `overlay_widgets/banner.rs:536`；`banner.svg` `<text y="60" font-size="16">`（应为 `52`） | Flutter `MaterialBanner` / Qt `QMessageBox` | 同上 |
| A9 | `rating` | 🔴 | `display_widgets/rating.rs:189`、`:219`；`rating.svg` 五个 `<text y="60">☆</text>` | Flutter `RatingBar` / SwiftUI 星星行居中 | 同上 |
| A10 | `tool_button` | 🔴 | `menu_toolbar/tool_button.rs:392`；`tool_button.svg` `<text x="120" y="60">` | Qt `QToolButton` 用 `Qt::AlignCenter` | 见 A11（同一处，横向亦错） |
| A11 | `tool_button` 标签**横向**从控件中点左对齐 | 🔴 | `tool_button.rs:384-398`：`point.x = rect.x + (text_right - rect.x)/2` 配 `HorizontalAlignment::Left`。`tool_button.svg` 240 px 控件里 `<text x="120">` | 标签要么贴内容边左对齐、要么居中；**不存在「从中点起左对齐」** | 改 `HorizontalAlignment::Center`，或把原点放到 `rect.x + padding` |
| A12 | 对话框按钮行（6 个） | 🔴 | `dialog/color_dialog.rs:385-401`、`input_dialog.rs:600-616`、`message_box.rs:618-628`、`file_dialog.rs:525-541`、`progress_dialog.rs:441-445`、`font_dialog.rs`。按钮 `y=80 h=28`，`draw_text_fitted` 收到未居中的 bounds ⇒ `y=80`（应为 `87`）。`audit_text_y.py`：6 个控件 × 明暗 = 12 条 `HUG-TOP` | Qt `QDialogButtonBox` 用 `QStyle::alignedRect`；Flutter `TextButton` 居中 | 传入由 `text_line` 推出的行盒矩形 |
| A13 | `mdi_area` 子窗口标题 | 🔴 | `container_widgets/mdiarea.rs:633` `frame_rect.y + title_bar_height / 2`（`=12`），14 px 字 ⇒ 字形框 `12..26` 越过 24 px 标题栏下边框。**同文件 `:643` 的 ✕ 却用 `(bar - close_size)/2`**，一条栏里两套规则 | Qt `QMdiSubWindow` 标题按 `fontMetrics().height()` 居中 | 抄 `dockwidget.rs:613-621`（已修好的同形代码） |
| A14 | `properties_panel` 全部行 | 🔴 | `view_widgets/properties_panel.rs:374`、`:398`、`:415`：`y + ROW_HEIGHT/2 + 4`（12 px 字 ⇒ 字形框 `17..29` 于 26 px 行内，**压过 `:424` 的分隔线**） | Qt `QTreeView` 行代理 / SwiftUI `Form` 居中 | 用 `find_replace_dialog.rs:287-291` 的 `text_line` |
| A15 | 数据表族（5 个） | 🔴 | `table_widget.rs:386`、`data_grid.rs:655`、`tree_table.rs:494`、`virtual_table.rs:452`、`grid_table.rs:781`（表头 `header_h/2`）：行高 20 / 14 px 字 ⇒ `y+10`，字形框 `10..24`，压过行底分隔线（`table_widget.rs:405` `y + row_h`） | Qt `QTableView`/`QHeaderView` 逐格居中 | 一个共享 `text_line(cell, font, ctx)`，六个调用点同改 |
| A16 | `menu_bar` 条目 | 🔴 | `menu_toolbar/menu_bar.rs:365-371`：`Point::new(x + w/2, rect.y + rect.height/2)` 配 `Left`。`w = len*8+16`，`"File"` ⇒ `w=48`、原点 `x+24`，4 字跨 `24..57.6` 而**下一条目起于 `x+48`** ⇒ 重叠 9.6 px | Qt `CE_MenuBarItem` 居中于条目 rect | 测量标签并居中；竖轴同理 |
| A17 | `tool_bar` 条目 | 🔴 | `menu_toolbar/tool_bar.rs:602-608`：与 A16 同形（`item_r.x + width/2` 配 `Left`，`button_size=32`，4 字跨 `16..49.6`） | Qt `CE_ToolButtonLabel` | 同上 |
| A18 | `tab_widget` 标签 | 🔴 | `container_widgets/tabwidget.rs:563-572`：`tab_rect.x + w/2` 配 `Left`；且 `:273` `tab_width = 100` 是字面量（**全文件无 `measure_text`**） | Qt `QTabBar::tabSizeHint` 按 `fontMetrics` 逐 tab 定宽 | 抄 `tab_bar.rs:486-495` 的 `measured_title_widths` + `compute_tab_width` |
| A19 | `navigation_stack` 导航栏 | 🟠 | `nav_widgets/navigation_stack.rs:306/309/323`：`nav_rect.y + 14` 是字面量，被 13 px（back）和 15 px（title）**两种字号共用**；`NAV_BAR_HEIGHT = 44`（`:33`），正确值分别是 15 和 14 | UIKit `UINavigationBar` 按 `sizeThatFits` 居中 | `context.measure_text(text, &font).height` 逐标签推 |
| A20 | `keyboard` 键帽 | 🔴 | `input_widgets/keyboard.rs:548-571`：`draw_text_fitted(inner, …)` 的 `inner.y = key_rect.y + 2`，而注释写「centred both ways」—— **`draw_text_fitted` 只做横向居中**（`surface.rs:518-532` 原样取 `bounds.y`）。`keyboard.svg` 四行键帽全部 `y = key_top + 2`（30 px 键内偏高 6 px） | 虚拟键盘键帽双向居中 | 传一行盒矩形，或给 `draw_text_fitted` 加纵向对齐参数 |
| A21 | `tag_input` 占位符 | 🔴 | `input_widgets/tag_input.rs:545-553`：`input_band` 未居中；**同文件 `:463-465` 的 chip 文本却是对的**（`chip_y + (TAG_HEIGHT - h)/2`）。`tag_input.svg` `<rect y="4" h="24">` + `<text y="4">` | Flutter `InputChip` / Qt `TextField` 提示居中 | 复用同文件 `:464` 的写法 |
| A22 | `popover` 占位符 | 🔴 | `dialog/popover.rs:352-370`：把 `content_rect`（已含 padding）整体交给 `draw_text_fitted` ⇒ 标签贴在卡片顶部 8 px 处（`popover.svg` `<text y="8">`，应为 `53`） | Flutter `showMenu` / Qt `QMenu` 空态居中 | 先在 `content_rect` 内居中得到行盒 |
| A23 | `toast` 消息 | 🟠 | `special_widgets/toast/single.rs:293-300`：`rect.y + (rect.height + 12)/2 = 66`（半个行盒 nudge），而**同一 toast 的 ✕ 在真正的中心 `60`**；`material_snackbar.rs:368` 却用 `(h - metrics.height)/2` 做对了 | Flutter `SnackBar` 内容居中 | 改用 `material_snackbar` 的写法 |
| A24 | `chart` 空态标签 | 🟠 | `special_widgets/chart.rs:416-418`：`y = rect.y + rect.height/2`（12 px 字 ⇒ `60..72`） | 同 A1 | `(h - metrics.height)/2` |
| A25 | `range_slider` / 其他以 `h/2` 定位**非文本图元** | ✅ 不判 | `range_slider.rs:442/461`、`search_box.rs:243/378/535`、`ascii.rs` 等：`h/2` 对**圆心/轨道中线**是**正确**语义 | — | **不改**（防误报：只有文本原点才是缺陷） |

### B. J3-配色：意图不可达 / 主题盲 / 面板不可见

| # | 控件 | 严重度 | 证据 | 平台对位 | 修法方向 |
|---|---|---|---|---|---|
| B1 | `badge` 药丸不可见 | 🔴 | `display_widgets/badge.rs:245-248`：`style.background_color.or(themed_bg).unwrap_or(self.level.color())`。`badge` 不在 role 表 ⇒ `Surface` ⇒ `theme.colors.background`；**`or()` 恒命中，故 `BadgeLevel::Info/Success/Warning/Error` 完全不可达**。`badge.svg` 药丸填充 `rgba(18,18,18)` = 导出器底色 | Flutter `Badge` 默认 `colorScheme.error`；Qt/SwiftUI 徽章是语义色药丸 | 以 `level.color()` 为基、推离窗口底色；severity 兜底不得不可达 |
| B2 | `scroll_bar` 滑块 = 槽色 | 🔴 | `display_widgets/scrollbar.rs:443`（滑块）与 `:447-450`（槽）**都读 `style.background_color`**；`scroll_bar` 不在 role 表 ⇒ 两处同值。`scroll_bar.svg` 第 5、7 行逐字节相同（`rgba(18,18,18)`）；`.light.svg` 亦然 | Flutter `ScrollbarThemeData.thumbColor` vs `trackColor`；Qt `SC_ScrollBarSlider` vs `SC_ScrollBarAddPage` | 加 role，并按 `slider.rs:676-681` 的 `!= window_fill` 守卫推离 |
| B3 | `scroll_bar` 箭头尺寸取**长度** | 🟠 | `scrollbar.rs:480` `arrow_size = min(rect.height, rect.width*0.2) = 48`；`scroll_bar.svg` 左箭头顶点 `x=48`，而滑块起于 `x=24` ⇒ 箭头**伸到滑块底下** | Qt 用 `SC_ScrollBarSubLine` 格（厚度量级，约 16 px） | 由 `min(w,h)` 定尺寸，钉在两端并预留格 |
| B4 | 金融四图绘图面字面量 | 🔴 | `finance/candlestick_chart.rs:747`、`volume_chart.rs:226`、`depth_chart.rs:368`、`indicator_chart.rs:533` 全是 `Color::rgb(18, 22, 28)`；四份 SVG 的 `diff` ←/→ 各 **4 行**（=注释 2 + 导出器底色 1 + 自己 1） | Qt `QChart` 绘图面随 `QPalette::Base`；Flutter/SwiftUI chart 背景随主题 | 抽 `finance/layout.rs` 一处解析绘图面，四处共用（**冰山法则**：同一字面量 4 份，改 1 漏 3 必再返工） |
| B5 | `indicator_chart` 边距与其他窗格不一致 | 🟠 | 其窗格 `x=52 w=180`，其余三图 `x=48 w=184`；`finance/layout.rs` 的模块文档正说共享 `IndexAxis` 就是为了防止「两个叠放窗格左边界不同、bar 对不齐」 | 多窗格图表共享左轴宽 | 走同一 `PlotArea::with_margins` |
| B6 | `switch` 的 on 态**永远拿不到 accent** | 🔴 | `display_widgets/switch.rs:187-194`：`style.background_color.or(themed_accent)`——主题一定给 `Choice` role 写入 `Some(input_background())`（`theme/manager.rs:407-411`），故 `themed_accent`（`:171` 的 `Success` 语义色）与字面量均**不可达**。`switch.svg` 轨道 `rgba(69,69,69)` = `input_background()` 而非绿。`:157-158` 的注释「主题不为 Choice 设 background」与 `role_colors` **相反** | Flutter `Switch` on/off **换色相**（`activeTrackColor`），不只是旋钮位移；iOS `UISwitch` 绿色 | 用 `style.theme_derived`（`banner.rs:521` 有先例）区分「调用方设」与「主题设」，`checked` 时落到 `themed_accent` |
| B7 | `radio_button` 指示器尺寸/位置由控件矩形决定 | 🔴 | `base_widgets/radiobutton.rs:195-196`：`radius = min(w,h)/4` ⇒ 240×120 下 `r=30`（60 px 圆），且圆心是**整流中心** `(120,60)`；`check_box` 的对应指示器是 16 px 居左（`checkbox.rs:281`）。`radio_button.svg` `<circle cx="120" cy="60" r="30">` | Qt `PE_IndicatorRadioButton` 是固定小指示器 + 前置对齐；Flutter `Radio` 20 px 于 48 px 触区 | 固定直径（16–20 px，或从字体行盒推）前置对齐 |
| B8 | `progress_bar` 用**主题不变量** | 🔴 | `display_widgets/progressbar.rs:265` 轨道 = `style.background_color`，`:273-274` 填充 = `semantic_color(Info)`。`progress_bar` 是 `Accent` role ⇒ 轨道 = `theme.colors.accent`；`progress_bar.svg` 整幅 `rgba(255,171,64)`（轨道）而填充 `rgba(138,180,248)` 宽 0 | Flutter `LinearProgressIndicator` 轨道是低emphasis 中性色，填充一支 tint；Qt `QProgressBar` 用 `Highlight` on `Base` | 轨道改为承载面派生的低强调色（`range_slider.rs:405` 的写法） |
| B9 | `progress_bar` 进度墨色不随承载面 | 🔴 | `progressbar.rs` 百分比文字固定 `Color::rgb(0,0,0)`；`progress_bar.svg` 黑字压在 `rgba(255,171,64)` 轨道上 | 进度文字取填充的对比色 | `fill.contrast_color()`（`roller.rs:291` 有先例） |
| B10 | `progress_circle` 值弧是字面量而轨道是主题色（与 B8 相反） | 🟠 | `display_widgets/progress_circle.rs:49` `progress_color: Color::PRIMARY`，`:292-293` 直接用；轨道读主题 | Flutter `CircularProgressIndicator` 轨道与值一支 `ProgressIndicatorTheme` | 默认值与 `progress_bar` 同一 token；`progress_color` 仍作调用方覆盖 |
| B11 | `color_dialog` OK/Cancel **填充相同** | 🔴 | `dialog/color_dialog.rs:382-394`：`accent = theme.background_color`（`color_dialog` 不在 role 表 ⇒ 窗口底色），两按钮同填，只有 Cancel 有描边。`color_dialog.svg` 两按钮 `fill="rgba(18,18,18)"` | Qt `QDialogButtonBox` 接受键为 default（高亮）；Flutter `FilledButton` vs `TextButton`；SwiftUI `.alert` 的 `.default`/`.cancel` | 接受键取 primary token（`input_dialog.rs:598-599` 已如此），拒绝键走承载面 + 描边 |
| B12 | `tool_bar` 条目 chrome 全字面量 | 🔴 | `menu_toolbar/tool_bar.rs:586-601`：checked `rgb(180,210,255)`、hover `rgb(210,230,255)`、resting `rgb(245,245,245)`、焦点描边 `rgb(0,120,215)`、墨 `rgb(0,0,0)`；竖分隔线 `:579` 也是字面量（同一函数 `:571` 的横分隔线却读 `style.border_color`）。`:546-549` 的注释声称背景已修，**同一循环里 6 处未修** | Qt `CE_ToolButtonLabel` 用 palette 的 `Button`/`Highlight`/`ButtonText` | 从解析后的 pair 派生（`tab_bar.rs:555-566` 是范式），两条分隔线共用 `style.border_color` |
| B13 | `scroll_area` 滚动条 8 处字面量 | 🟠 | `container_widgets/scrollarea.rs:684/688/702/711/715/729/739`：轨道 `rgb(240,240,240)`、轨道描边 `rgb(200,200,200)`、滑块 `rgb(180,180,180)`。**空态快照看不见**（`AsNeeded` + `content_size=(0,0)`，`:205-206`），但 `set_horizontal_scroll_bar_policy("always_on")` 是已发布属性（`:547`） | Qt `PE_IndicatorScrollBar*` 用 palette | 同文件 `draw_sticky_band`（`:453-468`）已做对，抄它 |
| B23 | `bottom_sheet` 遮罩把暗底**照亮**，且明暗两态数值巧合相同 | 🔴 | `dialog/bottom_sheet.rs:213` `scrim_color = ink.blend(&sheet_color, 0.55)`。代入两套预设的 `foreground`/`background`（dark `225`/`18`、light `0`/`240`）得 **121** 与 **122**——**已实测**：`bottom_sheet.svg` 遮罩 `rgba(121,121,121)` 压在 `rgba(18,18,18)` 上，`bottom_sheet.light.svg` 是 `rgba(122,122,122)` 压在 `rgba(240,240,240)` 上。① **方向错**：暗态遮罩（121）**亮于**它所覆盖的面（18），模态背板被点亮而不是压暗，且面板本身（`sheet_color = window_fill.blend(&ink,0.08)`）反而比自己的背板更暗；② **两态几乎同值**：因为 `ink` 与 `sheet_color` 在两套预设里角色互换，两个 blend 落到 121/122——明暗成对快照本就是为了让「忽略外观的控件」现形，这里它显示的是该表达式**是两套预设的巧合**而非派生量（换第三套预设就会任意分离） | 各平台模态遮罩都是**固定的暗色**：Material `scrimColor = Colors.black54`、UIKit `UIModalPresentationStyle` 压暗为黑、Qt `QGraphicsOpacityEffect`、SwiftUI `.presentationBackground` 暗化——**没有一个把背板往前景色方向调** | 改为朝**绝对暗色**混（`sheet_color.blend(&Color::BLACK, 0.32)`，即「面板压暗」），或引入 `theme.colors.scrim` token；分层（先遮罩后面板）与部分透明**保留**（原注释对这两点的论证是对的） |
| B14 | `mini_chart` 网格字面量 `rgb(220,220,220)` | 🔴 | `display_widgets/mini_chart.rs:215`；`mini_chart.svg` 5 条 `stroke="rgba(220,220,220)"` 压在 `rgba(18,18,18)` 上——**最亮的东西是最不重要的网格**。`:195` 的 `.unwrap_or(rgb(255,255,255))` 在主题路径上不可达（假安全网） | Flutter/`fl_chart`、Qt `QChart` 网格随 palette | 用 `charts.rs::axis_chrome()` 一处推导 |
| B15 | `masked_edit` 正文字面量 | 🔴 | `input_widgets/masked_edit.rs:451-452/498/516-522/533`：正文 `rgb(33,33,33)`、掩码 `rgba(160,160,160,200)`、占位 `rgba(180,180,180,200)`、光标 `rgb(25,118,210)`。暗色字段 `rgb(69,69,69)` 上 `rgb(33,33,33)` ≈ **1.35:1**。`:397-400` 的注释声称状态阶梯已主题化——**只对背景成立** | Qt `QLineEdit` 墨 = `QPalette::Text` | 五个字面量全是 chrome，读 `style.text_color`/主题 token |
| B16 | `status_bar` 讯息按承载带源 token 混合 | 🟠 | `menu_toolbar/status_bar.rs:180`（整条带 = 窗口底色，与背景同色）、`:208-212` `style.text_color.blend(&style.background_color, 0.4)` ⇒ 暗态把浅字**往暗处拉 40%，降低对比**；`.unwrap_or(rgb(80,80,80))` 是亮态字面量，**两个分支对「现在是什么外观」说法相反** | Qt `QStatusBar` 用 `WindowText`/`Disabled` | 读主题 muted token + `legible_on(resolved, 4.5)` |
| B17 | `drop_zone` 静止态无可见面 | 🔴 | `misc_widgets/drop_zone.rs:175-191`：`surface.or(themed).unwrap_or(rgb(248,250,252))`；`drop_zone` 不在 role 表 ⇒ 窗口底色。`drop_zone.svg` zone 填充 == 背景 | Flutter `DragTarget` 常规包一层有填充的 `Container`；桌面拖放区总有可见「井」 | 过滤 `== window_fill` 后向 ink 推一步（`dial.rs:391` 范式） |
| B18 | `signature_pad` 画布 = 窗口底色 | 🔴 | `special_widgets/signature_pad.rs`；`signature_pad.svg` pad 填充 == 背景，只有底色基线可见 | iOS `PKCanvasView`、Qt 签名控件都是白/`Base` 画布 | 给 pad 一个 `Input`/`Surface` 派生的面 |
| B19 | `otp_input` 6 格里 5 格不可见 | 🔴 | `input_widgets/otp_input.rs:556/567/574-585`：字段与格子都用 `background`（`Surface` ⇒ 窗口底色），只有聚焦格填 `background.blend(accent,0.10)`。`otp_input.svg` 只有第 0 格有填充 | Flutter `pin_code_fields`、iOS 一次性验证码：空态**每格**都要有框 | 格子面从窗口底色推一步；`:598-604` 的 `else` 分支不可达（`:595` 已 `continue`）——死代码 |
| B20 | `emoji_picker` / `cascader` 等 `Surface` 兜底 | 🟡 | `emoji_picker.rs:543-548`、`cascader` 面板同形——面板 = 窗口底色 | — | 见 §三「根因 R2」的统一修法 |
| B21 | `button` 墨色与背景不成对 | 🔴 | `button.rs:394-400`：暗态 `primary=rgb(100,181,246)` 上画**黑字**（`rgb(0,0,0)` 只按「disabled?」选，不看底）。`button.svg` `fill="rgba(0,0,0)"` 于 `rgba(100,181,246)` | 按钮墨取背景的对比色 | `bg.contrast_color()`（`badge.rs:251`、`role_colors` 都如此） |
| B22 | `group_box` 可勾选态的指示器 3 处字面量，含**纯黑勾** | 🔴 | `container_widgets/groupbox.rs:313` 框 `Color::rgb(100,100,100)`、`:325`/`:336` 两段勾 `Color::rgb(0,0,0)`（**已实测**）。暗态下（背景 `18,18,18`、标题带同色系）黑勾 = **最不可读的一笔**，而它正是用户唯一会去切换的部件。**快照看不见**：`create_group_box`（`constructors.rs:127-133`）只 `set_title`，从不 `set_checkable`，故 `checkable` 分支永不进（这是「快照覆盖不到的状态」的又一例） | Qt `QGroupBox` 指示器走 `QStyle::PE_IndicatorCheckBox`，随 palette 反转；Flutter/SwiftUI 用 `primary`/`onPrimary` | 框取 `style.border_color`，勾取**框填充的对比色**（`mdiarea.rs:631` 的 `primary.contrast_color()` 是同仓先例） |

### C. J4-声明闭环：声明了但没人读 / 读了但不可达

| # | 控件 | 严重度 | 证据 | 平台对位 | 修法方向 |
|---|---|---|---|---|---|
| C1 | `dial` 的 `notches_visible` / `notch_target` | 🔴 | `advanced_widgets/dial.rs:30-31` 字段、`:92-101` 访问器、`:173-186` setter、`:236-270` get/set、`:340-415` 整个 `Draw` **不引用**。`dial.svg` 面盘上无任何刻度。访问器自己的注释承认「draw 只画 body 和 needle，此 flag 尚无视觉效果」 | Qt `QDial::notchesVisible`/`notchTarget` **正是这套 API 的原型**且真画刻度；QtQuick `Dial.tickCount`；Flutter `Slider(divisions)` | 实现刻度环；注意默认 `3.7` 的**单位**要定（Qt 是「像素间距」，本仓注释写「度」） |
| C2 | `tab_widget` 的 `movable` | 🔴 | `container_widgets/tabwidget.rs:28/257-263`；`grep drag` **0 命中**，`Draw`/`handle_event` 均不提。`:374-388` 却把它列进 `property_names` | Qt `QTabBar::setMovable(true)` 可拖动重排并发 `tabMoved`；本仓 `tab_bar` 已有 `tab_moved` 信号（`tab_bar.rs:10`） | 实现拖动（复用 `splitter.rs` 的拖拽会话）并发信号，或从 `property_names`/`set` 里删掉并去掉访问器 |
| C3 | `candlestick_chart.overlay_count` 标 `readable:false` | 🟠 | `capability/properties_other.in.rs:862` `PropertySchema::new("overlay_count", UInt, false, true)`，而 `finance/candlestick_chart.rs:771` 的 `get` **明确回答它**、`:823` 也列进 `property_names`。`capability.rs:763-765` **强制**该 flag（不可读 ⇒ `UnsupportedOnWidget`）——所以调用方 `set` 之后**读不回来** | 「写了就能读」是属性契约的基本对称性 | `readable: true`，并把 `assert_contract` 覆盖到金融控件（该类型目前**无任何契约测试**） |
| C4 | 枚举 `accepted_tokens` 与 `get` 返回**不是同一套词**（6 处） | 🔴 | ① `properties_base.in.rs:35` `check_box.state` 发布 `off/indeterminate/on`，而 `coercion.rs:775-781` 返回 `unchecked/partially_checked/checked`（`partially_checked` **根本不在发布集里**；`access.rs:310` 的默认值 `unchecked` 也不在）② `properties_base.in.rs:58` ↔ `toggle_button.rs:199-205`（发布 checkbox 的词，控件返回 `normal/checked/disabled`，且三个都不可写）③ `properties_base.in.rs:26`/`properties_container.in.rs:16` 发布 `centre`，`coercion.rs:761-769` 返回 `center` ④ `properties_input.in.rs:21-26` `slider.tick_position` 发布 `noticks/left/right/ticksbothsides`，`slider.rs:75-82` 返回 `none/above/below/both` ⑤ `properties_advanced.in.rs:33-38` `calendar.first_day_of_week` 发布 `monday..sunday`，`access.rs:204-214` 返回 `mon..sun`（`:481` 的默认值 `mon` 也不在发布集）⑥ `properties_container.in.rs:151-156` `mdi_area.view_mode` 发布 `list/icon/details/thumbnails`（= `list_view` 的拷贝），`mdiarea.rs:429-432` 返回 `sub_window_view`/`tabbed`；`:417-419` 的注释**写明「发布的就是 reader 产出的那两个」**，注释对、表错 | schema 是设计器/JSON 的唯一契约来源 | 逐条把发布词表换成 reader 的词表；补「`get` 必须返回 `accepted_tokens` 成员」的测试（现测试只测写方向，`properties_tests.rs:875-916`） |
| C5 | `qr_code.quiet_zone` 有字段无访问器、无 schema 条目 | 🟠 | `misc_widgets/qr_code.rs:32-33` 字段；`:165-167` `property_names_of!["data","size",…]` **不含**；`:198-207` quiet-zone 面被钳到**控件**而非符号（`total_pixels.min(rect.width)`），模块循环却仍用未钳的 `offset_x` | Qt/`qrencode` 的 margin、Flutter `QrImageView.padding` 都是头等参数 | 发布 `quiet_zone` + setter；`bg_rect` 与模块循环共用同一钳后原点 |
| C6 | `lcd_number.num_digits` 只是宽度除数 | 🟠 | `display_widgets/lcd_number.rs:204-211`（`Dec` 用 f64 `Display`，产生 `"3.5"`/`"1e20"`，而 Hex/Oct/Bin 是整数）、`:356-360`（`digit_width = w/num_digits`，但文本按**值长度**居中）。`lcd_number.svg` 只亮 A–F = `'0'` 一个数字，却在 240 px 里按 6 位布局 | Qt `QLCDNumber` 真渲染 `digitCount()` 位并右对齐补零 | 按 `num_digits` 右对齐补位；`get_segments` 补 `'.'` 或把 `Dec` 归一到整数字段 |
| C7 | `cascader::visible_options_at` 算了 filter 又丢弃 | 🟡 | `input_widgets/cascader.rs:335-358`：文档写「honouring the filter」，函数体 `let needle = self.filter.to_lowercase(); let _ = needle;` 然后返回未过滤切片 | 文档与实现二选一 | 删 `needle` 并改文档，或真过滤 |
| C8 | `candlestick_chart::draw_price_labels` 死绑定 | 🟡 | `finance/candlestick_chart.rs:476-505`：`let bar = ...first()` 绑定后 `let _ = bar;`（`:503`）；注释说 `decimals` 来自「第一个 bar 的价格」，实际来自 `self.inferred_decimals()`（扫全序列） | — | 改 `if bars().is_empty() { return; }`，删误导注释 |

### D. J5/J6-可见性与构造性：空态无 chrome、尺寸由字面量相加

| # | 控件 | 严重度 | 证据 | 平台对位 | 修法方向 |
|---|---|---|---|---|---|
| D1 | `color_dialog` 取色区高度为 **0** | 🔴 | `dialog/color_dialog.rs:129-137`：`rect.height.saturating_sub(120)` 而对话框**设计高就是 120** ⇒ 0。`color_dialog.svg` 两条 `<rect y="38" width="220" height="0">` | Qt `QColorDialog` 取色区取标题栏与按钮行之间的剩余；Flutter/SwiftUI 同 | 从「标题栏底 → 按钮行顶」的剩余空间推高度 |
| D2 | `font_dialog` 三列列表高度为 **0** | 🔴 | `dialog/font_dialog.rs:274-293`：`col_area = (button_top - 46 - list_y).max(0)`，而 `list_y` 本身就是 46 ⇒ 0。`font_dialog.svg` 三个 `<rect y="46" height="0">`，列头仍在 `y=30` | Qt `QFontDialog` 给三个 `QListView` 弹性中段 | 减数改为由 `list_y` 与预览区实测高推出（与 D3 同改） |
| D3 | `file_dialog` 列表高 8 px、占位符溢出 12 px | 🔴 | `dialog/file_dialog.rs:442-460`：`list_h = (button_top - 34 - list_y) = 8`；`:460` 把 14 px 行盒放起于 `y=44` ⇒ 字形框 `44..58` 越出列表底 46。`file_dialog.svg` `<rect y="38" h="8">` + `<text y="44">` | Qt `QFileDialog` 列表取弹性中段，至少留一行 | 同 D1/D2 一族的「减数与被减数单位不一致」 |
| D4 | `date_range_picker` 表头/星期行/网格三者错位 | 🔴 | `misc_widgets/date_range_picker.rs:227-229`（`HEADER_HEIGHT=40`、`DAY_HEADER_HEIGHT=20`）、`:209`（`grid_top = y+60`）、`:394`（标题带 `y+6..y+24`）、`:447`（星期行 `cell_y = grid_top - 20 = y+40`，`:449` 再 `+4` ⇒ `y+44`）。20 px 的预留带只用了 9 px 行盒 + 4 px 偏移，**11 px 死区**；且 `available_height = h - (HEADER+DAYHEADER)` 是**字面量相加**，行数变 6 时 `cell_size` 被压到 8 px 而最后一行压到底边 | Qt `QCalendarWidget` 月头按字体行盒、星期行紧贴导航栏、日格按剩余高推 | 让 `grid_top` 从**已画出的范围**推（`header_band.y + h + 星期行行盒 + 2`），`available_height` 成为推论 |
| D5 | `candlestick` / `volume` / `depth` 空态无轴无网格无提示 | 🔴 | 三文件 `draw` 在填充字面量后立刻 `bars.is_empty() → return`（`candlestick_chart.rs:741-753` 及其 `:478-481` 的 `draw_price_labels` 早退）。SVG 只有一块 `rgba(18,22,28)` 板 | Qt `QChart` 轴是 chart 的属性、先于 series；Flutter/SwiftUI 空态画轴 | 空态也画轴 + 一行 "No data"（`chart.rs:405-417` 已有范式） |
| D6 | `chart`（ChartWidget）**完全没有值轴** | 🔴 | `special_widgets/chart.rs:475-489`（`PlotArea::of` 只有 8 px padding + 22 px 底行）、`:639-679`（柱 + 类目标签，无 y 轴/刻度/值标签）。`chart.svg` 最高柱 D 顶到 `y=8` = `rect.y + PADDING`，**贴顶零余量** | Flutter `fl_chart` 默认开 `leftTitles`；Qt `QChart` 必有 `QValueAxis`；SwiftUI `Chart` 默认 `AxisMarks` | 补左刻度列（4 档）+ 值标签；`size_hint`/面板文档已声称有 y 轴（`:310-312`、`:387-393`）——**文档说了、draw 没画** |
| D7 | `tool_bar` 条目占满页面（`toolbox`） | 🟠 | `container_widgets/toolbox.rs:218-243`：竖排 `item_height=32`、`content_height = (rect.height - 32*n).max(0)` ⇒ 120 px 下 **4 项归零**；`:199-216` 的 `item_rect` 不做钳制，越界项画到控件外 | Qt `QToolBox` 固定 tab 列、页面占**剩余主体** | 给页面保底宽/高，条带做溢出出口（滚动或 more 钮） |
| D8 | `tab_widget` 构造后**零 tab** ⇒ 整条 tab 带是空的 | 🔴 | `capability/constructors.rs:832-834` `label(geometry, text, Box::new(TabWidget::new(geometry)))`，而 `TabWidget::set`（`:374-384`）**没有 `text`/`title` 分支** ⇒ 标题落不下去，也不 `add_tab`。`Draw` 的 `for i in 0..self.tabs.len()`（`:506`）0 次迭代。`tab_widget.svg` 只有 `y=24` 的页面块和边框 | Qt `QTabWidget::QTabWidget()` 直接建两个 tab；本仓 `create_tab_bar`（`constructors.rs:200-207`）**已经这么干**并附理由 | 抄 `create_tab_bar`：加两个 tab；给 `TabWidget::set` 补 `text`/`title` 分支 |
| D9 | `tab_bar` 三个 `TabShape` 画得**一模一样** | 🟠 | `advanced_widgets/tab_bar.rs:584-610`：`Rounded`/`Triangular`/`Rectangular` 三臂都是 `fill_rect + draw_rect`；`Triangular` 的注释还写「clip 顶角」——**注释描述了没做的活**。`tab_shape` 是可读写已发布属性（`:996-998`） | Qt `QTabBar::Shape` 真按 shape 分支画（`CE_TabBarTabShape`） | 抄 `tabwidget.rs:519-560`（那里三种形状是**真画**的），让同一枚举值在两个 tab 控件里含义一致 |
| D10 | `tab_bar` tab 永不换行/裁剪/溢出 | 🟠 | `tab_bar.rs:440-468`（`x = rect.x + (tab_width+spacing)*index`，**不含 `rect.width` 任何引用**）、`:486-495`（宽只钳在 `[40,200]`）。6 个 200 px tab 在 240 px 条上 ⇒ 第 5、6 个完全画到控件外 | Qt `QTabBar` 溢出时出滚动箭头；Flutter `isScrollable` 或均分；SwiftUI 分页 | 溢出后改 `rect.width / tab_count`（`tab_view.rs:227` 已这么解），或加滚动偏移 + 裁剪 |
| D11 | `barcode_scanner` 四角括号一半画到遮罩上 | 🟠 | `misc_widgets/barcode_scanner.rs:331-387`：`bracket_len=20`，右下角两臂从 `(216,102)` 起算。`barcode_scanner.svg` `y=102` 已是 letterbox 首行（`:5` `<rect y="102" h="18">`），故水平臂落在遮罩里、竖直臂落在井内 | ZXing `ViewfinderView` 的四条 `drawRect` 全在框**内** | 四角统一锚到 viewfinder 的**内侧**角 |
| D12 | `masonry_layout` 不裁剪、标签不 fit | 🟠 | `container_widgets/masonry_layout.rs:76-106`（`col_heights` 单调累加无上限）、`:187-207`（**整个 `Draw` 无 `push_clip`**）；`:199-206` `text_y = item_rect.y + h/2 - 6`（`-6` 就是 12 px 字体的一半行盒，只对该字号成立）且用 `draw_text` 而非 `draw_text_fitted` | Flutter `MasonryGridView`/SwiftUI `LazyVGrid` 裁到视口并滚动 | 加 `push_clip`/`pop_clip` + 丢弃 `y ≥ rect.bottom`；标签走 `text_line` + `draw_text_fitted` |
| D13 | `dock_widget` 的 24 px 标题栏在**两个函数**各写一遍 | 🟡 | `container_widgets/dockwidget.rs:351`（`title_bar_rect`）与 `:357`（`content_rect`）各一个 `let title_bar_height = 24;`，无字段无访问器（对比 `window.rs:52` 的 `Window::title_bar_height`） | — | 提为 `pub const`（`popup_window.rs:256-274` 已这么做） |
| D14 | `grid_table` 列头是 `Col {i}` 占位 + 竖轴错误 | 🟠 | `view_widgets/grid_table.rs:779-786`：`format!("Col {ci}{label}")` 用**循环下标**当列名；`:781` `rect.y + header_h/2`（28 px 表头 ⇒ 字形框 `14..28` 压过 `:754` 的下描边） | Qt `QHeaderView` 读 `model()->headerData()`；Flutter `DataColumn(label:)` | 从数据源取列名；纵轴走共享 `text_line` |
| D15 | `gantt_widget` / `timeline_widget` 标签无宽度约束 | 🟡 | `special_widgets/gantt_widget.rs:351-357`（`rect.x+8` 起、`Left`、**无 bound**）而轨道起于 `:342` 的 `rect.x+150`；`timeline_widget.rs:389-395` 同形（轨道 `rect.x+120`，预算更小） | 标签应 elide 到轨道左缘 | 改 `draw_text_fitted` |
| D16 | `meter` 刻度与自己的弧**相差 90°** | 🔴 | `display_widgets/meter.rs:645` `tick_angle_deg = arc_start_deg + tick_step*i`，而弧用 `:313-314` `deg_to_rad(arc_start_deg + offset)`（`:504` `offset = -90`）。**刻度漏了 `+ offset`**：`meter.svg` 弧首顶点 `(71,71)` 而 tick0 外端 `(70,70)` 起于 135°，两者不同相；另 `:648-651` 两端各自独立取整 ⇒ 45° 刻度长 5 px、90° 刻度长 6 px，对称表盘看起来「有个刻度短」 | Qt `QDial::drawTicks()` 与 groove 共用同一 `span`/`startAngle` 映射；Flutter/SwiftUI 刻度与轨道同源 | 刻度角走**与弧顶点相同的** `snap_to_grid`，内端由同一取整半径推 |

### E. J8-声明式 + 保留混合的完整性

> 用户指令 #2 专项。「完整条件」= 两条路径对同一属性/事件**必须等价**，且声明式层缺的原语要补齐。

| # | 项 | 严重度 | 证据 | 平台对位 | 修法方向 |
|---|---|---|---|---|---|
| E1 | **声明式写属性不请求重绘** | 🔴 | `view/apply.rs:52-68` 走 `with_widget_mut` 后**不**调 `runtime::request_repaint`，而 `runtime.rs:1026-1034` 明文要求调用方在 `with_widget_mut` 后自行请求重绘；对比 `capability/access.rs:85-87`（id 路径/JSON/C ABI）**都调了**。⇒ 同一条 `SetProperty`，走 `Node` 与走名字得到不同屏幕结果 | — | `Ok` 分支补 `request_repaint(id)` |
| E2 | **161/188 个控件的已发布事件无法接线** | 🔴 | `signal/event_bridge.rs:310-337`：通用入口 `forward_widget_events` **只接 `clicked`**；`forward_all` 需要 `event_signal_dyn`，而 trait 默认返回 `None`（`widget_trait.rs:533-537`），只有 3 个控件实现（`tools/check_event_signal_dyn.py:47-51` 的 `CONVERTED` 只有 button/check_box/slider）。`connect_event`（`capability.rs:947-970`）却对任何已发布名都返回 `Ok` ⇒ **订阅被接受、永不触发**。`tools/event_published_census.txt` 188 条中 163 条有事件、**161 条有非 `clicked` 事件** | 设计器生成的连线运行时才确定，库必须能一次调用接完全部已发布事件（原则 #98） | 要么为每个有事件的控件实现 `event_signal_dyn`，要么让 `forward_all` 报出 `capability.events.len() - wired` 使缺口可计数；**门禁的 `CONVERTED` 名单必须扩到「有非空 `events:` 的全部控件」** |
| E3 | `Patch` 产出一个**契约保证会被拒**的 patch | 🟠 | `view/diff.rs:325-341`：名字被删除时发 `Patch::SetProperty{name, Null}`；若该名字是 `geometry`，`access.rs:252-254` 会把它替换成 `"0,0,0,0"`，而 `properties_trait.rs:106` 对 `geometry` **无条件**返回 `ReadOnlyProperty` ⇒ 必然 `PropertyRefused`。唯一消费者 `engine.rs:186-191` 把它降级成 `log::warn!` 丢弃 | — | diff 跳过 schema 声明为不可写的名字，或把 `geometry` 变成真的可写属性 |
| E4 | `diff` 对两棵**结构不同**的树报「未变化」 | 🟠 | `view/diff.rs:302` 的 `(None, None) => {}`：旧根无控件、新根是真实控件（须创建）时，既不设 `root_replaced` 也不产 patch。`diff(&Node::new("spacer"), &Node::new("button"), &|_,_| None)` ⇒ `patches == []`、`is_unchanged() == true`；而隔壁 `(Some(id), None)` 臂**是**设 `root_replaced` 的（`:303-306`） | — | 拆臂：新根是旧树没有的控件时置 `root_replaced` |
| E5 | `positional_matches` 的**文档与实现相反** | 🟠 | `view/diff.rs:136-140` 文档：「按位置匹配（而非按键）的次数」；`:435-440` 实现：**只在 keyless 子节点「找不到匹配」时**递增 ⇒ 它数的是**新节点**。文档描述的「稳定 keyless 列表被位置匹配」场景里计数器**恒为 0**；`b4_8c`（`:930-937`）正是该场景却**没断言**计数器 | 该字段是调用方「要不要加 key」的唯一信号 | 在真正位置匹配处递增，另立 `new_keyless` 计数 |
| E6 | **缺「分组/片段」原语**，且三处注释声称已有 | 🟠 | `view/mod.rs:42`、`diff.rs:211`、`diff.rs:280` 都写「`spacer` 不产出控件」；但 `spacer` **只在** `src/json/loader.rs:244-248` 特殊处理，**无 capability 无构造器**（`grep canonical_name: "spacer"` 零命中）⇒ `WidgetFactory::create` 返回 `None` ⇒ `engine.rs:243-248` 报 `UnknownWidgetType` 并**丢掉整个子树**（`apply.rs:315-321` 同）。因此「N 个兄弟共用一个身份（key）」**无法表达**：`Node` 必须命名一个控件，容器控件会进渲染树改布局 | React `<>…</>` / `keyed fragment`；SwiftUI `Group`；Flutter 不需要（有 `if` 且列表即身份） | 让 `create` 返回 `None` 的节点成为**透明节点**（子项挂到 `parent`/`index`、保留自己的 `key`），或加 `Node::group(key)`；**三处注释必须变成真的** |
| E7 | `diff_children` 的死分配 + 注释与代码相反 | 🟡 | `view/diff.rs:447-451` 每次迭代构造 `child_path`，`:487` 只被 `let _ =` 引用；实际递归传的是 `old_child_path`（`:473-475`），而注释写「传的是**新**索引路径」 | — | 删 `child_path`，改正注释 |
| E8 | 声明式层尚缺的原语（**列而不修**，属新功能） | 🟡 | portal/overlay（`Patch::Insert` 明说根不可插入 `diff.rs:60-62`，故 `dialog`/`tooltip`/`bottom_sheet` 这类「逻辑上是子、视觉上脱离父裁剪」的控件无法声明）；生命周期钩子（`View::build` 纯函数，无 `on_mount`/`on_unmount`）；上下文传播（`Node` 只有 `widget/key/props/children`）；错误边界（`ViewError` 扁平、`engine.rs:186-191` 整份丢弃）；`children_if` **除自身单测外零消费者**（`node.rs:147`） | React `createPortal`/`useEffect`/`createContext`/`ErrorBoundary`；Flutter `Overlay`/`initState`/`InheritedWidget`；SwiftUI `@Environment`/`.task` | 按需排期，不阻塞本表其余修复 |
| E9 | 动画/过渡**故意不在声明式层** | ✅ 不判 | `view/mod.rs:52-55`、`reactive.rs:45-48` 明确把动画划给 `PropertyAnimation` | — | **不改**（设计决定，属边界声明） |

---

## 三、根因归纳（修「写法」，不修「控件」）

| 根因 | 命中判据 | 影响面 | 一句话 |
|---|---|---|---|
| **R1 文本原点当中线** | J1 | **76+ 处 / 40+ 文件** | `band.y + band.h/2` 在「上边缘」契约下就是低了半个行盒。**正确写法本仓已有**（`find_replace_dialog.rs:287-291`、`tab_bar.rs:622`、`badge.rs:294`、`roller.rs:331`），所以修法是**抽一个 `text_line(band, font, ctx)` 并全量迁移**，不是逐控件调数字 |
| **R2 `Surface` role 兜底** | J3 | 8+ 控件 | 控件名不在 `WidgetRole::for_kind_name`（`theme/types.rs:104-156`）⇒ 拿窗口底色 ⇒ `or(themed_x)` 之后全不可达。**修法分两步**：① role 表补名；② 通用守卫——解析出的背景 `== window_fill` 时向 ink 推一步（`slider.rs:676-681`、`dial.rs:391` 已有范式） |
| **R3 字面量复制** | J3/J6 | 金融 4 图、`tool_bar` 6 处、`scroll_area` 8 处、`masked_edit` 5 处 | 同一字面量出现在 N 个文件 ⇒ 必须抽共享函数，让字面量「无处可回」（原则 #2 冰山法则 / #51） |
| **R4 `draw_text_fitted` 被当成会纵向居中** | J1 | ~20 调用点 | 它的契约是 **fit + 横向对齐**，`origin.y = bounds.y` 原样（`surface.rs:518-532`）。调用方传一个带 padding 的盒子就得到「贴顶」 |
| **R5 手调锚点代替测量** | J1/J6 | `menu_bar`/`tool_bar`/`tab_widget`/`navigation_stack`/`masonry_layout` | 一个 `+4`/`+6`/`+14`/`-6` 只对一种字号成立；`measure_text(...).height` 才是由构造成立 |
| **R6 声明不闭环** | J4 | C1–C6 | 属性/事件/token 声明了却没人读、或读了却读不回。**门禁只测了单向**（写方向），读方向没测 |
| **R7 混合两层不等价** | J8 | E1–E5 | 同一条 `SetProperty` 走 `Node` 不重绘；diff 产出必然被拒的 patch；`positional_matches` 语义反了 |

---

## 四、按用户指令 #1「重点组合控件」的专项覆盖

**结构容器 12**：`group_box`（**B22 可勾选态指示器 3 处字面量，含纯黑勾**）、
`panel` ✅、`frame` ✅（斜角按 `border_color` 派生，`frame.rs:198-208` 有据）、`tab_widget`（**D8 零 tab / A18 中点+固定 100 px / C2 `movable` 空转**）、
`stacked_widget` ✅（`window_fill` 重推 + 裁剪）、`splitter` ✅（空态补分隔条已在第 62 轮修）、
`dock_widget`（D13 双字面量）、`mdi_area`（**A13 标题越过下边框**）、`scroll_area`（**B13 滚动条 8 处字面量**）、
`collapsible_pane` ✅、`tool_box`（**D7 页面被线性减到 0**）、`grid` ✅（`:394` 的 `== rgb(220,220,220)` 哨兵是**潜在**问题，无证据，不判）

**布局/编排 4**：`carousel` ✅、`masonry_layout`（**D12 不裁剪 + 标签不 fit**）、`tab_view` ✅（`tab_width = w/count` 是对的）、`navigation_stack`（**A19 字面量 14 被两种字号共用**）

**骨架/区域 6**：`adaptive_scaffold` ✅（字号按比例 + clamp）、`safe_area` ✅、`bottom_sheet`（**B23 遮罩朝 ink 混 ⇒ 暗态把背景「照亮」**，详见下表）、`navigation_drawer`（闭合态 `:253-260` `min(panel_width, rect.width)` 在 240 px 下变成整幅——**存疑**，其文档称几何应覆盖全屏，故记为 UNVERIFIED）、`properties_panel`（**A14 全部行低 10 px 压分割线**）、`popup_window` ✅

**chrome/条 3**：`tool_bar`（**B12 6 处字面量 + A17 标签错位**）、`menu_bar`（**A16 条目重叠 9.6 px**）、`tab_bar`（**D9 三形状同画 / D10 无溢出出口**）

**顶层 2**：`window` ✅（只画客户区**刻意且有据**，`window.rs:191-241`）、`dialog` ✅（标题栏居中正确）

**数据编排 4**：`table_widget` / `data_grid` / `tree_table` / `virtual_table`（**A15 行文本压分割线**）、`grid_table`（**D14 占位列名 + 表头压描边**）

---

## 五、验证方式（每条修复的闭环判据）

| 判据 | 命令 | 期望 |
|---|---|---|
| J1 | `python3 tools/audit_text_y.py` | `suspicious placements` 中 `HUG-TOP` / `PUSHED-DOWN` 归零（当前 38 条，含已知假阳性：`label`/`ime_preedit`/`swipe_to_dismiss` 无自带色带、`bar_chart` 的 `4.0` 是柱上值标签） |
| J1/J2 | `bash tools/check_text_origin_is_a_top_edge.sh` | `failed: 0`（**必须附带一次反向注入**，否则只是输出绿色） |
| J3 | `python3 tools/audit_text_contrast.py` | 低于 4.5:1 的出现次数**只降不升**（当前 28） |
| J3/J5 | `bash tools/check_control_rendering.sh` | `checked=188 skipped=0 failed=0`（P1–P5）；修好「主题盲」后**门禁会主动要求删除失效豁免**（`tools/control_color_exemptions.txt` 现 14 条） |
| J1/J6 | `bash tools/check_svg_snapshots.sh` | 376 文件逐字节一致（改动会显示为 diff） |
| J8 | `cargo test --lib view::` | 现有 157 条 + 新增条件/键/根替换用例 |
| 全量 | `cargo test --no-default-features --features desktop` / `cargo clippy -- -D warnings` | 0 failed / 0 warning（**只在最后跑一次**，原则 #55/#56） |

**必须新增的门禁**（当前完全没有覆盖，是本轮发现的**门禁缺口**）：

1. **`check_text_vertically_centred`**（源码级）：扫描 `draw_text`/`draw_text_fitted` 的 `y` 实参，回溯到 `let`，凡形如 `band.y + band.height/2` 或 `band.y + band.height/2 + k` 且 `band` 是文本自身色带者失败。
2. **`check_style_derived_short_circuit`**（源码级）：`style.X.or(themed_Y)` 形态——若控件名不在 role 表且 role 会写 `X`，则该 `or` 的第二臂不可达，失败。
3. **`check_enum_schema_matches_reader`**（契约级）：对每个 `readable` 枚举属性断言 `get` 的返回是 `accepted_tokens` 成员（现测试 `properties_tests.rs:875-916` **只测写方向**）。
4. **`check_readable_flag_is_true_when_get_answers`**：`readable:false` 而控件 `get` 有分支 ⇒ 失败（`overlay_count` 即此类）。
5. **`check_events_are_wireable`**：把 `check_event_signal_dyn.py` 的 `CONVERTED` 从 3 个控件扩到「全部有非空 `events:` 的控件」，或改为「未实现的必须显式 allowlist 附理由」。
6. **`check_declarative_path_repaints`**：断言 `view/apply.rs` 的属性写入路径调用 `runtime::request_repaint`。

---

## 六、改善计划（按优先级，含依赖顺序与完成判据）

> 本节是**全部 81 条 + 附录 A 的逐条落位表**（无一条遗漏）。每条给出：优先级、涉及条目、
> **前置依赖**（哪些必须先做，否则会返工）、修法、**完成判据**（可执行的验证）。
> 原则依据：#2（修一处扫同类，所以「抽原语」永远排在「改调用点」之前）、#16（每条须附证据）、
> #51（修复量随层级下降）、#55–#57（逐轮只做定点证据，全量只跑一次）。

### 6.1 P0 — 阻断级：屏幕上就是错的 / 一次覆盖最多控件

> **排序依据**：覆盖控件数 × 是否用户可见。P0 内部**必须按「阶段」串行**，
> 因为阶段 1/2 产出的原语是阶段 3 的前置（否则 13 条会变成 76 次返工）。

#### P0-1 🔴 【阶段1｜抽原语】`render` 层新增文本行盒原语 `text_line()`

| 项 | 内容 |
|---|---|
| 涉及条目 | **A 组全部**（A1–A24，即 J1 居中缺陷）**+ R4**（`draw_text_fitted` 被当成会纵向居中） |
| 前置依赖 | **无**（这是所有 A 组条目的前置） |
| 现状证据 | 正确写法**本仓已有 4 处**：`find_replace_dialog.rs:287-291` 的 `text_line`、`tab_bar.rs:622`、`badge.rs:294`、`roller.rs:331` |
| 修法 | 在 `src/render/backend/surface.rs`（`draw_text_fitted` 旁）加一个公开原语：给定「色带矩形 + 字体」，返回该带内居中的**行盒矩形**。两个可选实现：① 新增 `text_line(band, font) -> Rect`；② 给 `draw_text_fitted` 加 `VerticalAlignment` 参数（R4 的根因正是它只有横向对齐）。**建议 ① 先做**（纯加法、不动既有调用契约），② 后做（会改签名） |
| 完成判据 | 原语有单测（带宽 < 行高、宽 = 0 等退化输入不 panic）；**先用它把 `find_replace_dialog` 改一遍并有快照不变**（证明原语与既有正确写法等价） |
| 预估改动量 | 1 处新增 + 1 处替换验证 |

#### P0-2 🔴 【阶段2｜接线已有机制】触控区（AR1）

| 项 | 内容 |
|---|---|
| 涉及条目 | **AR1**（附录 A）；覆盖 **16+ 控件**（附录 §A.1 清单） |
| 前置依赖 | 无 |
| 现状证据 | `TouchTargetSize` 数值**全对**（`style/primitives.rs:118-143`）；`contains_point_with_touch_expansion` **已实现**（`widget/base.rs:403-407`）；但 `role_base_style`（`theme/manager.rs:332-346`）**从不设置 `touch_target`**，且 `grep` 证明**零个控件**调用扩展函数（只命中定义/trait 转发/测试） |
| 修法 | 四步：① `role_base_style` 按当前 profile 的 `TouchTargetSize::dimensions()` 写入 `touch_target`；② `checkbox`/`radio_button`/`switch`/`slider`/`toggle_button`/`tool_button`/`group_box`/`tab_widget` 的命中测试改用 `contains_point_with_touch_expansion`；③ `Theme` 加 `visual_density`（Flutter `VisualDensity`：`-4.0..=4.0`，`comfortable (-1,-1)`）；④ 把 `LayoutContext.min_touch_size`（`layout/types.rs:45`，**当前零读取点**）接上或删除 |
| 完成判据 | 新增测试：对上述控件在 `size_hint` 尺寸下，断言距边缘 1 px 的**带外**点仍命中；反向注入（把命中测试改回 `contains_point`）必须 FAIL |
| 特别注 | `checkbox.rs:262` 的 `MousePress` 臂**完全不看 `pos`**（`{ pos: _, button }`）—— 这是「控件矩形内任意点都切换」，与「扩展触控区」是**两个不同**语义，须先裁定要哪个 |

#### P0-3 🔴 【阶段2｜接线已有机制】状态化主题（AR4a）

| 项 | 内容 |
|---|---|
| 涉及条目 | **AR4a**、**AR6**（三个 `pressed` 存了不画）、**A.4.3** 的 `switch` 9 态 / `check_box` 9+ 态 |
| 前置依赖 | P0-4（动画管线）—— 状态切换**无动画就是硬切**，先接状态再接动画会得到「闪变」 |
| 现状证据 | `WidgetState` **12 变体**（`manager.rs:440-453`）+ `resolve_style_for_state()`（`:303-317`）+ `"{kind}:{state}"` 键**全都存在**；但 `grep resolve_style_for_state src/ \| grep -v ^src/theme/` → **零调用**；`resolved_theme_style` 硬编码 `None`（`:582-599`）；`apply_active_theme` 不传状态（`theme/apply.rs:67`） |
| 修法 | ① 给 `apply_active_theme` 增加可选 `WidgetState`；② 先接 **3 个**控件：`Button`（hover/pressed/focus）、`CheckBox`（+2 个 `pressed` 字段）、`LineEdit`（focus）；③ 再推广到 `slider`/`switch`/`toggle_button`；④ 把状态从 `Option<WidgetState>` 升级为 **bitset**（Flutter `WidgetState` 是 `Set`，`focused \| hovered` 合法，`widget_state.dart:37`） |
| 完成判据 | 写一条 `"button:hover"` 覆盖 → 断言 hover 时 `draw` 取到该色（当前**不可能**通过）；反向注入（去掉状态传参）必须 FAIL |

#### P0-4 🔴 【阶段2｜接线已有机制】动画管线（AR7 + AR6）

| 项 | 内容 |
|---|---|
| 涉及条目 | **AR7**、**AR6**（三个 `pressed` 字段）、**AR8**（光标闪烁，见 P2-3） |
| 前置依赖 | 无（但与 P0-3 互为一对） |
| 现状证据 | `src/style/animation.rs` **1582 行**完整引擎（`AnimationConfig`/`Animation`/`ColorAnimation`/`FloatAnimation`/`EasingFunction`）—— 但 `grep -rln "style::animation\|AnimationConfig\|ColorAnimation" src/widget/` → **空**；全仓唯一动画约定是 `Spinner::tick(delta_ms)`（`spinner.rs:114`），涉及的控件**无一使用** |
| 修法 | ① `Theme` 加 `motion { fast, normal, slow, easing }`，默认 **100 / 200 / 300 ms** + `EasingFunction::EaseOut`；② 确立**全仓唯一**的 `tick(delta_ms) -> bool` 契约（`floating_label.rs:141` 已是正确范式，文档 `:133-139` 说明「只在还在动时调度下一帧」）；③ 接入顺序：`floating_label`（已有）→ `button` 状态过渡 → `checkbox`/`switch`/`slider` 按压涟漪 → `progress_*` 不确定态 |
| Flutter 参考常量 | `kThemeChangeDuration` **200 ms**（`constants.dart:39`）、`kRadialReactionDuration` **100 ms** + `kRadialReactionRadius` **20.0**（`:42-45`）、开关 `toggleDuration` M3 **300**/M2 **200 ms** + `Curves.easeOutBack`（`switch.dart:2370-2396,800-805`）、不定进度 **1800 ms**（`progress_indicator.dart:23`）、`Easing.standard = Cubic(0.2,0,0,1)`（`motion.dart:192`） |
| 完成判据 | `tick()` 在动画结束后**返回 false**（不得永久调度）；有单测断言 200 ms 后状态色到达终值；反向注入（不调度下一帧）必须 FAIL |

#### P0-5 🔴 【阶段3｜批量替换】A 组 25 条的机械迁移（用 P0-1 的原语）

| 优先级 | 涉及条目 | 对象 |
|---|---|---|
| **P0-5a** | A13, A14, A15, A16, A17, A18, A20, A21, A22 | **组合控件 + 文本族**（用户指令 #1 的重点）：`mdi_area`、`properties_panel`、5 个数据表、`menu_bar`、`tool_bar`、`tab_widget`、`keyboard`、`tag_input`、`popover` |
| **P0-5b** | A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, A12 | base/input/dialog 族的单行标签 + 6 个对话框按钮行 |
| **P0-5c** | A19, A23, A24 | `navigation_stack`、`toast`、`chart` 空态（后两条是「半个行盒 nudge」） |
| 共性修法 | 全部改为「先由 P0-1 原语求出带内行盒，再画」；**A11/A16/A17/A18 另有一项**：`point.x` 取的是控件/条目**中点**却配 `HorizontalAlignment::Left` ⇒ 须同时改 `Center` 或把原点移到 `padding` 处 |
| 完成判据 | `python3 tools/audit_text_y.py` 的 `HUG-TOP` / `PUSHED-DOWN` **归零**（当前 38 条；已知假阳性 3 条需同步加白名单：`label`/`ime_preedit`/`swipe_to_dismiss` 无自带色带、`bar_chart` 的 `4.0` 是柱上值标签）；**同时必须新增源码级门禁**（见 P3-1） |

#### P0-6 🔴 【阶段3｜单点】「隐形控件」与「零高度」——最刺眼的一类

| 子项 | 条目 | 修法 |
|---|---|---|
| a | **B1** `badge` 药丸 = 窗口底色 | `badge.rs:245-248` 的 `or()` 链使 `level.color()` **不可达**；以 `level.color()` 为基并推离窗口底色 |
| b | **B2** `scroll_bar` 滑块 = 槽色 | `scrollbar.rs:443` 与 `:447-450` **同读一个字段**；加 role + `!= window_fill` 守卫 |
| c | **B4** 金融四图主题盲 | 同一字面量 `Color::rgb(18, 22, 28)` 在 4 文件（`candlestick_chart.rs:747`/`volume_chart.rs:226`/`depth_chart.rs:368`/`indicator_chart.rs:533`）；抽 `finance/layout.rs` 一处（**冰山法则：改 1 漏 3 必再返工**） |
| d | **B11** `color_dialog` 两按钮同填 | `color_dialog.rs:382-394` 的 `accent` = 窗口底色；接受键取 primary token（`input_dialog.rs:598-599` 已如此） |
| e | **B17/B18/B19** `drop_zone`/`signature_pad`/`otp_input` | 三者的「面」都是窗口底色；按 `dial.rs:391` 范式推一步 |
| f | **D1/D2/D3** 三个零高度对话框 | `color_dialog.rs:135` `h - 120`（120 既是设计高又是当前高 ⇒ 0）、`font_dialog.rs:274-293`、`file_dialog.rs:442-460`；三者同型：**减数与被减数单位不一致** ⇒ 从「已画出的元素」向下堆叠 |
| g | **D8** `tab_widget` 零 tab | 抄 `create_tab_bar`（`constructors.rs:200-207` 已加两个 tab 并附理由）；给 `TabWidget::set` 补 `text`/`title` 分支 |
| h | **D16** `meter` 刻度与自己弧**相差 90°** | `meter.rs:645` `tick_angle_deg` **漏了 `+ offset`**（弧用 `:313-314` 的 `arc_start_deg + offset`） |
| i | **D6** `chart` 完全没有值轴 | `chart.rs:475-489` 的 `PlotArea` 无左槽；补 4 档刻度 + 值标签（`size_hint`/面板文档已声称有 y 轴，`:310-312`/`:387-393`） |
| j | **A.3.9** `floating_label` **演示浮动标签的控件没有浮动标签** | ① `label` 进 `property_names`/`get`/`set`；② 让 `label()` 取**最具描述性**属性而非首个命中（`control_backend/custom/mod.rs:15`）；③ 补 `floating_label_behavior` |
| k | **B22** `group_box` 可勾选态的**纯黑勾** | `groupbox.rs:313` 框 `rgb(100,100,100)`、`:325`/`:336` 两段勾 `rgb(0,0,0)`（**已实测**）—— 暗态下是**最不可读的一笔**，而它正是用户唯一会切换的部件。框取 `style.border_color`，勾取**框填充的对比色**（`mdiarea.rs:631` 先例） |
| 完成判据 | 逐条：`diff x.svg x.light.svg` 的行数必须**大于** 4（证明它真的响应主题）；门禁 P3 会主动报出失效豁免要求删除；快照再生后人工复核 |

> **B22 的一个附带发现**：其可勾选态在**快照里看不见**，因为 `create_group_box`（`constructors.rs:127-133`）从不 `set_checkable`。
> 这是「**快照覆盖不到的状态即长期未审**」的又一例（与 `scroll_area` 的滚动条同型），已入 P3-1h 门禁。

### 6.2 P1 — 意图不可达（声明了却到不了，等于空声明 #109）

| # | 优先级 | 条目 | 修法 | 完成判据 |
|---|---|---|---|---|
| 1 | **P1-1** | **B6** `switch` 的 on 态**永远拿不到绿** | `switch.rs:187-194` `style.background_color.or(themed_accent)` —— 主题必为 `Choice` 写 `Some`（`manager.rs:407-411`），故 `themed_accent` 与字面量**全不可达**；用 `style.theme_derived`（`banner.rs:521` 有先例）区分「调用方设」/「主题设」。**同时改正 `:157-158` 与 `role_colors` 相矛盾的注释** | 断言 `checked == true` 时轨道色 ≠ 未选中色；注释与 `role_colors` 一致 |
| 2 | **P1-2** | **B8/B9/B10** 进度族 | `progress_bar` 轨道改为承载面派生（`range_slider.rs:405` 范式）、进度墨取 `fill.contrast_color()`、`progress_circle` 的 `progress_color` 默认值改与 `progress_bar` 同 token | 三条各自的 `fill`/`track` 在明暗下都不同；进度墨对比 ≥ 4.5:1 |
| 3 | **P1-3** | **B21** `button` 暗态黑字压蓝底 | `button.rs:394-400` 的墨只按 `disabled` 选；改 `bg.contrast_color()`（`badge.rs:251` 先例） | `audit_text_contrast.py` 该项消失 |
| 4 | **P1-4** | **B7** `radio_button` 指示器尺寸由 rect 定 | 见 P0-7（AR2）—— 与尺寸钉死同修 | 快照 `r` 变为固定值 |
| 5 | **P1-5** | **C1** `dial` 的 `notches_visible`/`notch_target` **完全空转** | 实现刻度环；**先裁定单位**（Qt 语义是「像素间距」，本仓注释写「度」，`dial.rs:97-98`） | 断言设置后快照出现刻度元素 |
| 6 | **P1-6** | **C2** `tab_widget.movable` 空转 | 实现拖动（复用 `splitter.rs:48-54` 的 `HandleDrag` 会话）+ 发 `tab_moved` 信号；或从 `property_names` 删掉并去访问器 | 二选一，不得悬空 |
| 7 | **P1-7** | **C3** `overlay_count` 标 `readable:false` 而 `get` 回答它 | `properties_other.in.rs:862` 改 `true`；把 `assert_contract` 覆盖到金融控件（**当前无任何契约测试**） | 门禁可读回 |
| 8 | **P1-8** | **C4** 六处枚举 `accepted_tokens` 与 `get` 返回**不是同一套词** | ① `check_box.state`（发布 `off/indeterminate/on`，返回 `unchecked/partially_checked/checked`）② `toggle_button.state`（发布 checkbox 词表）③ `centre` vs `center` ④ `slider.tick_position` ⑤ `calendar.first_day_of_week` ⑥ `mdi_area.view_mode`（发布 `list_view` 的词表，且 `:417-419` 注释**写明就是 reader 那两个**） | 逐条逐 token 对齐；**补「`get` 必须返回 `accepted_tokens` 成员」的反向测试** |
| 9 | **P1-9** | **E1** 声明式写属性**不请求重绘** | `view/apply.rs:52-68` 不调 `runtime::request_repaint`，而 `runtime.rs:1026-1034` 明文要求；`capability/access.rs:85-87` 调了 | 同一 `SetProperty` 走 `Node` 与走名字屏幕结果一致 |
| 10 | **P1-10** | **E2** **161/188 控件**的已发布事件**无法接线** | `event_bridge.rs:310-337` 的通用入口**只接 `clicked`**；`forward_all` 需 `event_signal_dyn`，trait 默认 `None`（`widget_trait.rs:533-537`），仅 3 控件实现。要么逐控件实现，要么让 `forward_all` **报出** `events.len() - wired` 使缺口可计数 | **门禁 `check_event_signal_dyn.py` 的 `CONVERTED` 从 3 扩到「全部有非空 `events:` 的控件」**，否则该门禁在自己假绿 |
| 11 | **P1-11** | **A.4.1** 文本输入缺整个**装饰槽模型** | `line_edit` 5 属性 vs `InputDecoration` 8 区域（`prefixIcon`/`prefix`/input/`suffix`/`suffixIcon`/`label`/`helperError`/`counter`）；加 `Decoration` 子结构 | 至少 `label_text`/`prefix_text`/`suffix_text`/`helper_text`/`error_text`/`counter_text` 可设且可见 |
| 12 | **P1-12** | **A.4.2** 光标位置**算错**（可证） | `lineedit.rs:649` 量的是**整串** ⇒ `cursor_position = 0` 时光标在末尾 | 断言 `cursor_position = 0` 时光标 x == 文本起点 |
| 13 | **P1-13** | **E6** 缺**分组/片段**原语（且三处注释声称已有 `spacer`） | `view/mod.rs:42`/`diff.rs:211`/`:280` 写「`spacer` 不产出控件」，但 `spacer` **只在** `json/loader.rs:244-248` 特殊处理、无 capability ⇒ `engine.rs:243-248` 报 `UnknownWidgetType` 并**丢掉整个子树** | ① 让 `create → None` 的节点成为**透明节点**，或加 `Node::group(key)`；② **三处注释必须变成真的** |

### 6.3 P2 — 一致性、组合结构与尺寸钉死

| # | 优先级 | 条目 | 修法 |
|---|---|---|---|
| 1 | **P2-1** | **AR2 / B7** 几何由 `rect` 反推（**11 控件**） | 分离「画多大」与「占多大」：`rect` 作触控区，内部按常量绘制居中。覆盖：`switch`（52×32 轨道）/`radio_button`（r 8/4.5）/`progress_bar`（高 4）/`chip`（高 32）/`badge` 点（6）/`progress_circle`/`navigation_drawer`（宽 304）/`mobile_date_picker`（行距 32）/`calendar`（行高 42）/`segmented_control`/`app_bar`/`bottom_navigation_bar`。**实测后果已在快照里**：`switch.svg` = 240×120 体育场、`radio_button.svg` = r=30、`progress_bar.svg` = 240×120 板 |
| 2 | **P2-2** | **B3/B5/B12/B13/B14/B15/B16** 字面量复制与窗格不一致 | 抽共享函数（`tool_bar` 6 处、`scroll_area` 8 处、`mini_chart` 网格、`masked_edit` 5 处、`status_bar` 混合方向）；**B5**：`indicator_chart` 窗格 `x=52 w=180` vs 其余三图 `x=48 w=184`，而 `finance/layout.rs` 的文档正说共享 `IndexAxis` 就是为了防「左边界不同”。bar 对不齐」—— 走同一 `PlotArea::with_margins` |
| 2b | **P2-2b** | **B20** `emoji_picker`/`cascader` 等 `Surface` 兜底的**面板**（低危） | 「面板 = 窗口底色」除已列出的 8 个控件外，`emoji_picker.rs:543-548`、`cascader` 面板同形；统一按 §三 R2 的一步守卫处理（与 **B1/B2/B17/B18/B19** 同一修法，只是可见性后果不同） |
| 3 | **P2-3** | **AR8** 光标闪烁**零实现**（3 处注释声称存在） | 用 P0-4 的 `tick(delta_ms)`；500 ms 半周期（`editable_text.dart:113`）；**顺带恢复 `PasswordEchoOnEdit`**（`_kObscureShowLatestCharCursorTicks = 3`，同一个计时器）；修 `tag_input.rs:93` 的**孤立注释** |
| 4 | **P2-4** | **D4/D5/D7/D9/D10/D11/D12/D14/D15** 组合结构 | `date_range_picker` 表头/星期/网格三者错位；金融空态无轴；`toolbox` 页面被线性减到 0；`tab_bar` 三形状同画 + 无溢出出口；`barcode_scanner` 四角括号一半画到遮罩上；`masonry_layout` 不裁剪 + `:84` 死绑定；`grid_table` 占位列名；`gantt/timeline` 标签无 width 约束 |
| 5 | **P2-5** | **A.3.3/A.3.4/A.3.5/A.3.6/A.3.7/A.3.8** 尺寸偏小（附录 A） | 见 P2-1 的常量表 |
| 6 | **P2-6** | **AR3** 主题 token 扩面 | 加 `outline`/`outline_variant`（修「分隔线与焦点环同色」）/`scrim`（修 **B23** 遮罩照亮暗底）/`inverse_surface`/`on_inverse_surface`/`surface_container*`/`surface_tint`/`on_error` + `Font.letter_spacing`/`line_height`（后者是 2× 文本缩放的前置） |
| 7 | **P2-7** | **AR4b/c** `font_scale`/`min_touch_size` | 两者**当前零读取点**（全仓各 1 处命中=声明自己）；在 `role_base_style` 读 `font_scale`；`app_bar.rs:193` 的 clamp 22 上限使 2× 缩放**静默封顶**，须解 |
| 8 | **P2-8** | **A.5** a11y | `A11yState` 有 `value` 但**零控件填充**，且结构**缺 `checked`/`mixed`**；补结构 + 为 `slider`/`progress_bar`/`progress_circle`/`rating`/`code_editor` 填 `value` |
| 9 | **P2-9** | **A.4.5** 契约过薄 | `auto_complete_edit` 只发布 `suggestion_count`（无法枚举候选/读高亮/得知选中）；`drop_zone` 只有 **1/5** 反馈态（缺 `onMove` 落点偏移/`onLeave`/reject）；`rating` 的 `Float` 属性**静默取整**；`shortcut_editor` 存物理键码而非逻辑 activator；缺 `selectableDayPredicate`（含**范围版**，须接收待定 range） |
| 10 | **P2-10** | **AR5** RTL（**先修 `slider`**） | `slider.rs:285-303` 取值方向**单调相反**、方向键亦然（`:573-587`）—— 这是唯一**功能性** RTL 缺陷；加 `TextDirection` + `start`/`end` 应趁 API 未广泛依赖 |
| 11 | **P2-11** | **A.4.5a/g** 命名冲突 | `stepper`：本仓=**数值微调器**，Flutter `Stepper`=**分步向导**（`StepState` 5 态 + 连接线）⇒ 本仓**缺一个整控件**。裁定：要么改本仓命名（`spinbox` 角色已由 `spinbox.rs`/`number_picker.rs` 覆盖），要么补真正的向导控件 |
| 12 | **P2-12** | **C5/C6/C8** 未完成面 | `qr_code.quiet_zone` 有字段无访问器无 schema；`lcd_number.num_digits` 只是**宽度除数**（6 位布局只画 1 位数字）；`candlestick::draw_price_labels` 的死绑定 + 与实现矛盾的注释 |

### 6.4 P3 — 卫生与门禁（让同类缺陷**写不出来**）

> **P3-1 的 6 条门禁必须与对应修复同批落地**：否则下一轮会重新引入同类缺陷（这正是前几轮的教训）。

| # | 优先级 | 门禁 / 清理项 | 判据 | 依据 |
|---|---|---|---|---|
| 1 | **P3-1a** | **新增 `check_text_vertically_centred`**（源码级） | 扫描 `draw_text`/`draw_text_fitted` 的 `y` 实参并**回溯 `let`**；凡形如 `band.y + band.height/2`、`+ k`、或裸 `band.y` 者 FAIL | #102 / P0-1 的闭环 |
| 2 | **P3-1b** | **新增 `check_style_derived_short_circuit`** | `style.X.or(themed_Y)` 形态：若控件名不在 role 表且 role 会写 `X` ⇒ 第二臂**不可达** ⇒ FAIL | #109 / B1/B6 |
| 3 | **P3-1c** | **新增 `check_enum_schema_matches_reader`** | 对每个 `readable` 枚举属性，断言 `get` 返回 ∈ `accepted_tokens`（现测试 `properties_tests.rs:875-916` **只测写方向**） | #16 / C4 |
| 4 | **P3-1d** | **新增 `check_readable_flag_is_true_when_get_answers`** | `readable:false` 而控件 `get` 有分支 ⇒ FAIL（`overlay_count` 恰是此类）；现测试 `:734-766` 因 `if readable \|\| writable { continue }` 而**看不见它** | #16 / C3 |
| 5 | **P3-1e** | **扩 `check_event_signal_dyn.py` 的 `CONVERTED`** | 从 3 个控件扩到「全部有非空 `events:`」；或改为「未实现的必须显式 allowlist 附理由」 | #97 / E2 |
| 6 | **P3-1f** | **新增 `check_declarative_path_repaints`** | 断言 `view/apply.rs` 的属性写入路径调用 `runtime::request_repaint` | #101 / E1 |
| 7 | **P3-1g** | **新增 `check_mechanism_has_a_consumer`** | 凡新增抽象（trait 方法/新枚举/新字段）**必须至少有一个生产调用点**；`touch_target`/`resolve_style_for_state`/`font_scale`/`min_touch_size`/`animation.rs` 五套均会因此被拦住 | #17 的另一种形态（迁移幻觉） |
| 8 | **P3-1h** | **新增 `check_control_feature_visible_in_own_snapshot`** | 断言控件的**特征部件**在其快照里可见（`floating_label` 的浮动标签、`tab_widget` 的 tab 带、`badge` 的药丸）—— 直接拦住「演示控件不演示自己」 | #107 / A.3.9 |
| 9 | **P3-2** | `C7` `cascader::visible_options_at` 算了 filter 又丢弃（`let _ = needle;`） | 删 `needle` 并改文档，或真过滤 | #5 / #18 |
| 10 | **P3-3** | `E3/E4/E5/E7` 声明式层其余 | diff 产出**必然被拒**的 patch（`geometry`）；`(None,None)` 根臂对**结构不同**的树报「未变化」；`positional_matches` **文档与实现相反**；`diff_children` 死分配 + 注释与代码相反 | 逐条改正 + 补断言 |
| 11 | **P3-4** | `D13/D15` 卫生 | `dock_widget` 的 24 px 在**两个函数各写一遍**；`gantt/timeline` 标签无 width 约束 | 提为 `pub const`；改 `draw_text_fitted` |
| 12 | **P3-5** | 同步更新 `blue21.md` 的「不判为缺陷」清单 | 每次新增白名单必须附理由（#108） | — |

### 6.5 P4 — 新功能（另立计划，不混入「修外形错误」）

| # | 条目 | 性质 | 说明 |
|---|---|---|---|
| 1 | **E8** 声明式层缺的原语 | 新功能 | portal/overlay（`Patch::Insert` 明说根不可插入）、生命周期钩子、上下文传播、错误边界；`children_if` 目前**除自测外零消费者** |
| 2 | **A.4.3** 缺的整控件 | 新功能 | 真正的**分步向导**（`StepState` 5 态 + 连接线 + `controlsBuilder`）；`chart` 的商业化图表族以外的部分 |
| 3 | **A.6** 无外部基准控件的自查清单 | 审计任务 | 图表 6 项（脏标记/刻度数学/图例**是属性**/指针逆变换/`data_range`/无障碍摘要）、编辑器选中子系统（手柄/`TextSelectionOverlay`/放大镜/平台化 `Controls`）、富文本**跨度感知** caret 几何（**勿继承等宽 `cell_width` 捷径**）、签名板时间戳增量流、QR 纠错级、扫码器生命周期 |
| 4 | `code_editor` 的 `SyntaxPalette::default()` 整套浅色 | 新功能 | 第 61/62 轮已同样记录，本轮复核仍未变 |
| 5 | **A.7** 本仓**优于** Flutter 的 12 项 | **保留，勿动** | 为避免「迁就 Flutter」而丢失桌面方向优势；详见 §A.7 |

### 6.6 批次的划分（每批 = 一轮，#55–#57）

| 批次 | 内容 | 前置 | 交付判据 |
|---|---|---|---|
| **批 1** | P0-1（抽原语）+ P0-5c（用它替换 3 处验证等价） | 无 | 原语有单测；3 处快照不变 |
| **批 2** | P0-5a + P0-5b（A 组剩余 22 条迁移）+ **P3-1a 门禁** | 批 1 | `audit_text_y.py` 归零；门禁反向注入实测 FAIL |
| **批 3** | P0-2（触控区）+ P0-3（状态主题）+ P0-4（动画） | 无 | 三条各有反向注入证据 |
| **批 4** | P0-6（隐形控件 + 零高度 + `floating_label`）+ **P3-1b 门禁** | 无 | 各 `diff` > 4 行；P3 主动报失效豁免 |
| **批 5** | AR2 / P2-1（尺寸钉死，11 控件） | 批 3（触控区分离） | 快照几何符合常量表 |
| **批 6** | P1 全部（13 条）+ **P3-1c/d/e/f 门禁** | 批 3 | 每条：声明到不了 ⇒ 门禁可拦 |
| **批 7** | P2 其余（token 扩面 / a11y / RTL / 契约过薄） | 批 5 | 逐条证据 |
| **批 8** | P3 其余 + P4 立项 | — | — |
| **收尾** | `cargo test --no-default-features --features desktop` + `clippy -D warnings` + `bash tools/check_profiles.sh` + `bash tools/run_all_gates.sh` | 全部 | **只在最后跑一次**（#55/#56）；每个门禁**必须反向注入验证它会红** |

### 6.7 修法层次一览（为何这个顺序最省工）

| 层 | 一次投入 | 覆盖条目 | 若不在此层修的代价 |
|---|---|---|---|
| `render` 原语（P0-1） | 1 个函数 | **A 组 25 条** | 76 处逐点调数字，且下次还会漂 |
| theme 接线（P0-2/3/4） | 3 处接线 | **AR1 16+ 控件 / AR4a 12 态 / AR7 全控件** | 五套子系统继续空转（现在就是） |
| 尺寸常量表（P2-1） | 一张表 + 11 控件 | **AR2 + B7 + A.3.3–A.3.8** | 快照继续画出「不像该控件」的形状 |
| 门禁（P3-1） | 8 条 | **防止全部回归** | 下一轮重新引入同类缺陷（前几轮已发生两次） |

---

## 七、明确**不判为缺陷**的（防误报，附理由）

| 项 | 理由 | 取证 |
|---|---|---|
| 容器铺满 240×120 | canvas **就是**控件 rect | `census.rs:52` `CENSUS_RECT` |
| `window` 只画客户区 | 标题栏归窗口管理器；画第二条才是缺陷 | `window.rs:191-241` |
| 数据色（系列/光谱/涨跌/色阶/阈值带） | 编码信息，主题化会抹掉信息 | #108 ③ + `tools/control_color_exemptions.txt`（14 条，`sparkline` 明确「无 chrome 可主题化」） |
| 占位符/水印/未选中星较淡 | 「这是提示而非值」的语义 | 第 62 轮 §6 归类 |
| `range_slider.rs:442/461`、`search_box.rs:243/378/535` 等 `h/2` | 那是**圆心/轨道中线**的正确语义，不是文本原点 | — |
| `grep "height as i32 / 2"` 的 110 命中里约 3/4 | 上一条同一理由 | — |
| 动画不在声明式层 | `view/mod.rs:52-55` 显式边界声明 | — |
| 每个控件各自 `resolved != window_fill` 守卫 | 那是**合法的、已被多次验证的**手法（`splitter`/`stacked_widget`/`mdi_area`/`carousel`/`dial`/`tool_button` 都用），不是缺陷 | — |

---

## 八、本文件的一句话结论

**本次核对的产出不是「找到 60 个坏控件」，而是「找到 7 个坏写法」——**
`band.h/2` 当居中（76 处）、`Surface` 兜底吞掉窗口底色（8+ 控件）、
同一字面量复制 N 份（金融 4 / toolbar 6 / scrollarea 8）、
`draw_text_fitted` 被当成会纵向居中（~20 处）、手调锚点代替测量、
声明不闭环（门禁只测了写方向）、混合两层不等价（`Node` 不重绘 / 事件 161 个不可接线）。

**这正是原则 #51 的形态：修复量应当随「层」下降，而不是随「调用点」上升。**
A 组 22 条若逐控件调数字，是 76 次返工；若先在 `render` 层立一个 `text_line`，
则是**一处新增 + 76 处机械替换**。因此 §六 的 P0 里，A 组必须先做「抽原语」这一步。

---

# 附录 A — 与 Flutter SDK 的逐控件比对（用户 2026-09-22 追加指令）

> 指令：「如下是 flutter 源码 `/home/mikeli/workspace/flutter`，请仔细比对分析，将可改进 + 控件或其他方面不足，
> 追加到 `blue21.md`；对控件实际外形、尺寸、字体、外观、布局、主题、属性、方法进行仔细比对校核」
>
> **取证方式**：Flutter 只读访问经项目内符号链接 `flutter_ref` → `/home/mikeli/workspace/flutter`
> （见 §A.0）。每条 Flutter 结论都带 `文件:行号` 与**真实常量值**；每条本仓结论都带 `文件:行号`。
> 标 **[改进]** 的可采纳、**[不足]** 是本仓的缺陷、**[无对应]** 是 Flutter 有而本仓没有的整块能力（并注明是否要紧）。
>
> **与 A–E 组的关系**：A–E 组是「本仓自证」的缺陷（对照 SVG 与自身契约即可判定）；
> 本附录是「有外部基准」的缺陷（对照 Flutter 才能判定标准值）。两者**部分重叠**（已在各条注明），
> 但本附录带来了 A–E 组不可能有的东西：**标准数值**（48 px 触控下限、42 px 日历行、52×32 开关轨道、
> 300 ms 开关时长……）。

## A.0 前置说明（需要用户确认的一项）

| 项 | 内容 |
|---|---|
| 访问方式 | 我的工具只能读项目根目录内。为读取 Flutter 源码，我在项目内建了符号链接 `flutter_ref` → `/home/mikeli/workspace/flutter` |
| 性质 | 符号链接不进 git（`git status` 不显示，`.gitignore` 未改）；它只是**读取通道**，不修改 Flutter 任何文件 |
| 建议 | 若不想保留，`rm flutter_ref` 即可（本附录不依赖它存在）。若后续还要对照，可加入 `.gitignore` |
| Flutter 规模 | `packages/flutter/lib/src/material/` **184 个文件**、`cupertino/` 48 个、`widgets/` 100+；本附录核对了两棵树共约 180 个控件映射 |

## A.1 四个「跨控件」根因（本附录最高价值的部分）

> 与 §三 的 R1–R7 并列。这四条的**修复量随层级下降**——每条覆盖数十个控件，且**都是本仓已有机制未接线**。

### AR1 🔴 `touch_target` 机制完整存在，但**没有任何控件在使用它**

| 证据 | 内容 |
|---|---|
| 本仓① | `src/style/primitives.rs:118-143` `TouchTargetSize { Desktop 32, Tablet 44, Phone 48, Embedded 40 }` —— **数值全对，且已有 `dimensions()`** |
| 本仓② | `src/widget/base.rs:403-407` `contains_point_with_touch_expansion()` —— **实现存在** |
| 本仓③ | **实测**：`grep -rn contains_point_with_touch_expansion src/` 只命中它自己的定义、一处 trait 转发（`widget_trait.rs:588`）与**测试**；**零个控件调用** |
| 本仓④ | **实测**：`touch_target` 在 `role_base_style`（`src/theme/manager.rs:332-346`）**从不设置**，只有 CSS token（`:486-487`）或显式 builder（`primitives.rs:290`）会写 |
| 本仓⑤ | **实测**：命中测试用的是裸 `geometry().contains_point` —— `checkbox.rs:262`（甚至**完全不看 `pos`**）、`switch.rs:257`、`toggle_button.rs:318` |
| Flutter | `kMinInteractiveDimension = 48.0`（`material/constants.dart:27`）；`materialTapTargetSize ??= padded`（`theme_data.dart:404-408`），`padded` 的含义即「把最小点击区扩到 48×48」（`theme_data.dart:175-180`） |
| **后果** | 一套为 32/44/48/40 设计的触控机制**一个控件都没接**。而且它与 189 个控件里的若干尺寸问题**是同一件事的两面**：控件画得小（视觉）与点击区不能扩（交互）各自独立，但解决机制是同一个 |
| **[改进]** | ① `role_base_style` 按当前 profile 的 `TouchTargetSize::dimensions()` 写入 `touch_target`；② `checkbox`/`radio`/`switch`/`slider`/`toggle_button` 的命中测试改用 `contains_point_with_touch_expansion`；③ 加 `Theme::visual_density`（Flutter `VisualDensity`：`minimumDensity -4.0`、`maximumDensity 4.0`、`comfortable (-1,-1)`，`theme_data.dart:3183-3223`） |
| 关联 | 这是 A–E 组**完全没有的一类**（E 组是声明式；A–E 无一条讲触控区）。也是本次比对**性价比最高**的一条 |

**同一根因下的「小于 48」清单**（本仓值 → Flutter 值，均实测）：

| 控件 | 本仓 | Flutter | 位置 |
|---|---|---|---|
| `check_box` | `size_hint` 高 **24** | 48（`padded`）/ 40（`shrinkWrap`） | `checkbox.rs:183` / `checkbox.dart:516-521` |
| `radio_button` | 高 **24** | 48 / 40 | `radiobutton.rs:100` / `radio.dart:739-746` |
| `switch` | 高 **28** | 48×40（`switchMinSize`） | `switch.rs:79` / `switch.dart:2071-2091` |
| `slider` | 高 **20**（手柄 16） | 触控 overlay **r=24**（画 r=10） | `slider.rs:380-385,70` / `slider_value_indicator_shape.dart:188`、`slider_parts.dart:676-681` |
| `line_edit` | 高 **24** | 48（`input_decorator.dart:1116-1118`） | `lineedit.rs:353-356` |
| `combo_box` | 高 **24** | 48 | `combobox.rs:208-212` |
| 数据表行 | 高 **20** | 48（`dataRowMinHeight`） | `data_grid.rs:79-89` / `data_table.dart:985-992` |
| `tab_widget` tab | 高 **24** | 46（`_kTabHeight`）/ 48（`kTextTabBarHeight`） | `tabwidget.rs:265-300` / `tabs.dart:30-32`、`constants.dart:36` |
| `toolbox` 页签 | 高 **32** | 48（`_kPanelHeaderCollapsedHeight`） | `toolbox.rs:197-210` / `expansion_panel.dart:17-21` |
| `collapsible_pane` 头 | 高 **24** | 44（Cupertino `_kHeaderHeight`）/ 48 | `collapsible_pane.rs:38-46` / `cupertino/expansion_tile.dart:39` |
| `toggle_button` | 高 **28** | 48 | `toggle_button.rs:181-184` / `toggle_buttons.dart:762-769` |
| `group_box` 复选框 | **12** | 48 | `groupbox.rs:142-154` |
| `tab_widget` 关闭钮 | **12** | —（Flutter `Tab` 无关闭钮） | `tabwidget.rs:415-434` |
| `dock_widget` 标题钮 | **16** | 48 | `dockwidget.rs:366-393` |
| `mdi_area` 关闭钮 | **12** | 48 | `mdiarea.rs:640-648` |
| `splitter` 手柄 | **5**（**两处独立写法**：`:292` 写 `5`、`:415-427` 写 `HANDLE_WIDTH: f32 = 5.0`） | 48 | `splitter.rs:292,415` |

### AR2 🔴 **几何由 `rect` 反推，而 Flutter 钉死常量** —— 快照已把后果拍出来

| 控件 | 本仓（由 rect 推） | Flutter（钉死） | 实测后果（SVG） |
|---|---|---|---|
| `switch` | `track_width = rect.width`、`track_height = min(rect.height, w/2)`（`switch.rs:144-145`） | 轨道 **52×32**、拇指 r **14**、整体宽 **60**、最小 **48×40**（`switch.dart:2014-2046,2370-2396`） | `switch.svg` = **240×120 的体育场** + 116×116 拇指 |
| `radio_button` | `radius = min(w,h)/4`（`radiobutton.rs:196`） | 外 r **8**、内 r **4.5**（`radio.dart:31-32`） | `radio_button.svg` = `r=30` 的 **60 px 圆** |
| `progress_bar` | 高 = `rect.height`（`progressbar.rs:284`） | `linearMinHeight = 4.0`、圆角 **2**、`trackGap 4`（`progress_indicator.dart:1568,1627,1636`） | `progress_bar.svg` = **240×120 实心板** |
| `chip` | 高 = `rect.height - 8`（`chip.rs:166`）；**`fill_rect` 方角** | 高 **32**、stadium（`radius = h*0.45`）（`chip.dart:36,1312`） | 24 px 高下 chip 只有 16 px 且是方的 |
| `badge` 点 | r = `max(min(w,h)/2, 4)`（`badge.rs:256`） | `smallSize = 6.0`（`badge.dart:490`） | 24 格子里成 12 px 点 |
| `progress_circle` | r = `min(w,h)/2 - stroke/2 - 1`（`progress_circle.rs:138`） | 断言 `size.width == size.height`、最小 **40**、`strokeAlign: inside`（`progress_indicator.dart:1259,1599-1602`） | — |
| `navigation_drawer` | `panel_width.min(rect.width)`（`:253-261`） | `BoxConstraints.expand(width: _kWidth = 304.0)`（`drawer.dart:59,274-276`） | `navigation_drawer.svg` 闭合态 = **整幅 240×120** |
| `mobile_date_picker` | `row_height = rect.height / 5`（`:234`）、字号 `row_height*0.38` clamp(10,15) | `_kItemExtent = 32.0`、`_kPickerWidth = 320`、`_kPickerHeight = 216`（`cupertino/date_picker.dart:21-24`） | 实测行高 24、字号 10 |
| `calendar` | `cell_h = grid.height / 6`（`calendar.rs:302`） | 行高 **42**（M2）/ **48**（M3）、`_maxDayPickerHeight 294/336`（`calendar_date_picker.dart:34-42`） | `calendar.svg` 日格 = **11 px** |
| `segmented_control` | `seg_w = rect.width / count`（`special_widgets/segmented_control.rs:160-161`） | 每段内边距 **16**（`cupertino/segmented_control.dart:17`）、最小高 **28**（`:22`） | 长标签 `text_x` 走负、越界重叠 |
| `app_bar` | 字号 `bar_height*0.38` clamp(14,22)（`app_bar.rs:193`） | `kToolbarHeight 56` + 固定 `titleLarge`（`app_bar.dart:74-80`） | clamp 上限 22 使 2× 文本缩放**永远无法生效** |
| `bottom_navigation_bar` | 图标 `h*0.32` clamp(14,28)、标签 `h*0.18` clamp(9,14)（`:224-228`） | 图标 **24**、选中 **14**、未选中 **12**（`bottom_navigation_bar.dart:233,238-239`） | 指标从 **3 px 下划线** vs M3 **64×32 药丸**（`navigation_bar.dart:29-30`） |
| `masonry_layout` | — | 无对应（Flutter 无 masonry，属本仓优势） | 实测 `:84` `_corner_radius: u32 = 4` 是**死绑定**，而 `:160` 真画 `corner_radius = 6` —— 同一文件两个值 |

**[改进]** 把「画多大」与「占多大」分离：`rect` 作为**触控区**，内部按常量绘制并居中。这正是 Flutter 的模型（`switchMinSize`/`kMinInteractiveDimension` + 固定 `switchWidth`）。这一步同时修好 AR1 的一半。

### AR3 🟠 主题 token 面：7 个 role ↔ Flutter 每控件 7–44 个 token

| 本仓 | Flutter | 差额 |
|---|---|---|
| `WidgetRole` **7** 变体（`src/theme/types.rs:70-86`）→ 每控件一个 `(bg, fg, border)` 三元组（`manager.rs:390-421`） | 每控件一个 `*ThemeData`：`SliderThemeData` **31** 字段、`ChipThemeData` **23**、`TabBarThemeData` **17**、`DataTableThemeData` **15**、`ToggleButtonsThemeData` **15**、`TooltipThemeData` **15**、`DialogThemeData` **14**、`ExpansionTileThemeData` **13**、`BottomSheetThemeData` **13**、`ScrollbarThemeData` **11**、`SnackBarThemeData` ~16、`DatePickerThemeData` **44**、`TimePickerThemeData` **24** | 本仓**无法表达**「活动轨道 X / 空轨道 Y / 禁用拇指 Z / 刻度 W」这类同控件内多部件、多状态取值 |
| `Colors` **10** token（`types.rs:163-185`） | `ColorScheme` **49** role（`color_scheme.dart:833-1144`）：`outline`/`outlineVariant`/`surfaceContainer{L,L,·,H,Highest}`/`scrim`/`inverseSurface`/`onInverseSurface`/`surfaceTint`/`shadow`/`onError`/`errorContainer`/`onErrorContainer`…… | **实测**：`role_colors` 给**每个** role 都发 `border_color: Some(theme.colors.secondary)`（`manager.rs:402/410/418`）—— 因为**没有 `outlineVariant`**。于是列表行分隔线与按钮焦点环是**同一个灰** |
| 语义色 4 个裸 token（`error/warning/success/info`） | `error` 是**三元组**（`error`/`onError`/`errorContainer`/`onErrorContainer`，`color_scheme.dart:985-999`） | **双向**：本仓有 Flutter 没有的**严重度轴**（应保留，切勿为了对齐而删）；但缺 `on_*`/`*_container` —— 于是 `Danger` role 只能 `error.contrast_color()` 现算（`manager.rs:413`），而行内校验提示（浅红底 + 深红字）**在主题里根本不存在** |
| 无 `scrim` token | `Colors.black54` | `dialog`/`bottom_sheet` 各自现拼遮罩 —— 这正是 **B23**（`bottom_sheet` 把暗底照亮）的根因 |
| 无 `inverseSurface`/`onInverseSurface` | 有 | `snackbar`/`toast`/`tooltip` 无法相对页面反转，浅色下三者与背景无从区分 |
| 无 `surfaceContainer*`/`surfaceTint` | 有（6 级） | 暗色下**升降级不可表达**（见 AR4） |
| `Font` 只有 `{family, size, weight, italic}`（`src/core/font.rs:10-22`） | `TextStyle` 有 `letterSpacing`/`height`/`wordSpacing`/`fontFeatures` | **无行高 token ⇒ 2× 文本缩放时无法预留正确的纵向空间**（AR1/§A.5 的连锁） |
| `Fonts` 9 role | `TextTheme` 15 role（`text_theme.dart:122-136`） | 缺 `labelLarge`(14 w500，**按钮标签本该用它**)/`labelMedium`(12)/`bodySmall`(12)/`bodyLarge`(16)/`titleLarge`(22)…；而 `role_base_style` 给**所有**控件发同一个 `fonts.body`（`manager.rs:344`），故按钮与标签同字号 |

### AR4 🟠 三个「基础设施已建、无人接线」的情形（与 AR1 同型）

| # | 机制 | 已有 | 实测接线情况 | 后果 |
|---|---|---|---|---|
| a | **状态化主题** | `WidgetState` **12** 变体（含 `Hover/Pressed/Focused/Checked/Error/Warning/Success`，`manager.rs:440-453`）+ `resolve_style_for_state()`（`:303-317`）+ `"{kind}:{state}"` 覆盖键 | **实测**：`grep resolve_style_for_state src/ \| grep -v ^src/theme/` → **零调用**。`resolved_theme_style` 硬编码 `None`（`:582-599`），`apply_active_theme` 不传状态（`theme/apply.rs:67`） | 「`"button:hover"` 覆盖」**在控件层不存在**。与 Flutter 的差距不只是「state 是集合而非单值」（Flutter `WidgetState` 是 `Set`，`focused \| hovered` 合法，`widget_state.dart:37`），而是**根本没人查** |
| b | **文本缩放** | `LayoutContext.font_scale`（`src/layout/types.rs:42-49`，默认 1.0） | **实测**：全仓仅 3 处命中，即声明 + 默认值 + 文档注释 —— **零个读取点**。无任何控件按 scale 放大字号 | Flutter `TextScaler`/`MediaQuery.textScaler`（`media_query.dart:221`、`text_scaling.dart:20`）。本仓实测 8 个控件在 2× 下破功（24 px 高的 check_box/radio/line_edit/combo_box 装 28 px 字；`app_bar` clamp 22 上限**静默封顶**） |
| c | **最小触控尺寸** | `LayoutContext.min_touch_size`（`types.rs:45`，默认 32×32） | **实测**：全仓仅 1 处命中（它自己的声明/默认值）—— **零个读取点**；四个布局的 `update_with_context` 只缩放 spacing/padding/gap | 与 AR1 同义 |

### AR5 🟡 RTL / 方向性：**零支持**

| 证据 | 内容 |
|---|---|
| 本仓① | `src/core/alignment.rs:5-29` `enum HorizontalAlignment { Left, Center, Right }` —— **物理方向**，无 `Start`/`End`；全仓**没有 `TextDirection` 类型** |
| 本仓② | `EdgeOffsets` = `{top,right,bottom,left}`（`primitives.rs:60-62`）；CSS 解析只认 `left`/`right` |
| 本仓③ 🔴 **功能性缺陷（非仅外观）** | `slider.rs:285-303` `value_to_pixel_pos` 硬编码 `rect.x + inset + fraction*travel` ⇒ **RTL 下取值方向单调相反**；方向键亦然（`:573-587`，`37`→减、`39`→加） |
| 本仓④ | `app_bar.rs:206,234,252` back/title/action 全按物理边计算；`safe_area.rs:24-34` 四边物理（`left`/`right` 默认 0） |
| Flutter | `Directionality`（`widgets/basic.dart:171`）；`EdgeInsetsDirectional.fromSTEB(start, top, end, bottom)`（`painting/edge_insets.dart:742-744`） |
| **[改进]** | 现在加 `TextDirection` + `start`/`end` 是**最便宜的时刻**（API 未广泛依赖）。**优先修 `slider`** —— 它是唯一有**功能**（非外观）RTL 缺陷的控件 |

## A.2 交互状态与动画：本仓几乎为零

### AR6 🔴 三个控件把 `pressed` 存起来却从不绘制

**实测**：`Slider::mouse_pressed`（`slider.rs:32`，由 `handle_event` 写）、`Switch::pressed`（`switch.rs:40`）、`ToggleButton::pressed`（`toggle_button.rs:38`）—— 三者的 `draw` 都**不读该字段**（`slider.rs:621-859` 只读 style/theme/value/tick/orientation）。

| 控件 | 本仓状态数 | Flutter 状态数 | 缺的是 |
|---|---|---|---|
| `slider` | **2**（enabled/disabled，`slider.rs:676-697`） | **5**（disabled/hovered/focused/dragged + 默认，`slider.dart:848-853`） | hover/focus/drag 光晕（Flutter M3 透明度：drag 0.1 / hover 0.08 / focus 0.1，`slider.dart:2360-2372`）；且 Flutter 有 **4 个独立 disabled token**（`:647-660`） |
| `switch` | **3** 颜色分支（`switch.rs:187-194`） | **9** 状态组合（`switch.dart:2184-2236`） | 见 B6 —— 更糟：`style.background_color.or(themed_accent)` 使 on 态**永远**拿不到绿 |
| `check_box` | **4** 分支（`checkbox.rs:291-316`） | **9+** 组合 + M3 `side` 的 error 态（`checkbox.dart:865-890,936-945`） | hover/focus/pressed/**error**；且 `checkbox.dart:943-945` 的 error 描边是 `colors.error` —— **本仓有 `colors.error` token，但零个控件路由到它** |
| `toggle_button` | `ToggleButtonState` 只有 **3**（`Normal/Checked/Disabled`，`toggle_button.rs:19-27`） | 15 token（`toggle_buttons_theme.dart:60-114`） | 枚举本身缺 Hover/Pressed/Focused，故 draw **无法**表达（与上一条「存了不读」互补） |

### AR7 🔴 动画：188 个控件、一套完整动画引擎、**零处交互反馈**

**实测**：`src/style/animation.rs`（1582 行）有 `AnimationConfig`/`Animation`/`ColorAnimation`/`FloatAnimation`/`EasingFunction`/`AnimationDirection`/`AnimationFillMode`；但 `grep -rln "style::animation\|AnimationConfig\|ColorAnimation" src/widget/` → **空**。全仓唯一的动画约定是 `Spinner::tick(delta_ms)`（`spinner.rs:114`），而**本次比对涉及的控件无一使用**。

| 应采纳的 Flutter 常量 | 值 | 出处 | 使用者 |
|---|---|---|---|
| `kThemeChangeDuration` | **200 ms** | `material/constants.dart:39` | 每个按钮的状态过渡（`button.dart:73` 等） |
| `kRadialReactionDuration` | **100 ms**（`kRadialReactionRadius = 20.0`，同处 `:42-45`） | `constants.dart:42-45` | `checkbox.dart:448`、`slider.dart:688`、`range_slider.dart:507` 的按压涟漪 |
| `_kSwitchToggleDuration` | M3 **300 ms** / M2 **200 ms**，曲线 `Curves.easeOutBack` | `switch.dart:2370-2396,800-805,954` | 开关拇指位移 + 200 ms 涟漪 |
| `_kIndeterminateLinearDuration` | **1800 ms**（4 段关键帧） | `progress_indicator.dart:23,191-206` | 线性不确定进度 |
| Scrollbar 淡出 | 闲置 **600 ms** 后 **300 ms** 淡出 | `scrollbar.dart:17-18` | 滚动条自动隐藏 |
| `_kCursorBlinkHalfPeriod` | **500 ms** | `editable_text.dart:113` | 光标闪烁（见 AR8） |
| `Easing.standard` | `Cubic(0.2, 0.0, 0.0, 1.0)` | `motion.dart:192` | M3 标准曲线 |
| `Durations.*` 阶梯 | short1-4 = **50/100/150/200**；medium1-4 = **250/300/350/400**；long1-4 = **450/500/550/600** | `motion.dart:28-148` | 时长体系 |

**[改进]** 两步：① `Theme` 加 `motion { fast, normal, slow, easing }` 默认 **100/200/300 ms** + `EasingFunction::EaseOut`；
② 把 AR6 的 `pressed` 等既有状态通过 `ColorAnimation` 驱动。**引擎已存在，缺的只是 token 与接线。**

### AR8 🟠 光标闪烁：三个文档注释说它存在，实际一个计时器都没有

**实测**：`inplace_editor.rs:7/26/367` 注释写「blinking cursor」「Draw cursor (blinking vertical line)」；`tag_input.rs:93` 有一行**孤立的** `/// Cursor blink interval in milliseconds.`，**下面没有常量**（`TAG_CHIP_RADIUS`/`MIN_INPUT_WIDTH` 在其上下，该注释悬空）。全仓 `grep blink` 只命中注释，无 `Duration`、无 `tick`、无可重绘调度。

| 后果 | 说明 |
|---|---|
| 光标不闪 | 文本框/代码编辑器/终端的光标静止 —— 而 Flutter 是 `Timer.periodic(500ms)`（`editable_text.dart:4888`） |
| `PasswordEchoOnEdit` 不可恢复 | 本仓**已删除**该 EchoMode（理由是当时实现是占位，`lineedit.rs:49-61`）。Flutter 的实现只是 `_kObscureShowLatestCharCursorTicks = 3`（`editable_text.dart:117`）个闪烛 —— **同一个计时器**。⇒ 补 blink 即同时拿回该特性 |
| 代码编辑器/终端 | `code_editor/render.rs:848` `draw_caret` 画静态光标；终端**最不能没有**闪烁（不闪读起来像卡死） |
| 可复用的正确范式 | `floating_label.rs:141` `pub fn tick(&mut self, delta_ms: u32) -> bool` —— 返回「是否仍需重绘」，文档 `:133-139` 明确说明**只在还在动时调度下一帧**。这就是该用的形状与经济学 |

## A.3 本仓 SVG 快照直接暴露的比对结论（用户指令 #2 的「外形/尺寸」）

> 这些不是「与 Flutter 风格不同」，而是**快照本身即证明**：本仓在该控件上画出的东西，与 Flutter 同名控件不是同一种形状。

| # | 控件 | 快照原文 | 本仓值 | Flutter 值 | 判定 |
|---|---|---|---|---|---|
| A.3.1 | `slider` 🔴 | `<rect x="0" y="0" width="16" height="120" fill="rgba(100,181,246)"/>` | 手柄 = **16×120 整高直板**，轨道 4 px 方角 | 画 **r=10 圆盘** + 触控 overlay **r=24**；轨道圆角 `trackHeight/2`；M3 轨道高 **16** + `trackGap 6` | **同控件不同形状**。同仓 `range_slider.svg` 反而画的是圆盘（`<circle r="8"/>` + 圆角轨道），两个同族控件**互相矛盾** |
| A.3.2 | `radio_button` 🔴 | `<circle cx="120" cy="60" r="30"/>` | r=**30**（60 px 圆），且圆心 = **控件中心** | 外 r **8**、内 r **4.5**（比值 0.5625，非本仓的 0.5） | 尺寸由 rect 定义（AR2）。且本仓半径比 Flutter 大 **3.75×** |
| A.3.3 | `switch` 🔴 | `<rect ... width="240" height="120" rx="60"/>` | 轨道 **240×120**、拇指 **116×116** | 轨道 **52×32**、拇指 r **14**、整体 **60×~40** | 快照就是 240 px 体育场 —— 已不像开关 |
| A.3.4 | `progress_bar` 🔴 | `<rect ... width="240" height="120" fill="rgba(255,171,64)"/>` | 高 = rect（**120**）；轨道 = `accent`；圆角 **0** | `linearMinHeight` **4.0**、圆角 **2**、`trackGap` **4**、`stopIndicator r=2` | 三重偏差（高 30×、圆角、轨道色），见 B8 |
| A.3.5 | `chip` 🔴 | 只有底 + 描边（无 chip） | **方角** `fill_rect`（`chip.rs:352-353`）；高 = `rect.height-8` | **stadium**，高 **32**，标签内边距 **8** | 默认 chip 在快照里**完全不可见**（= B 组「面无」类）；即便可见也是方的 |
| A.3.6 | `progress_circle` 🟠 | `<circle r="57" stroke-width="4"/>` 单环无缺口 | 无 `trackGap`，无 inside 对齐 | `trackGap` **4**、`strokeAlign: inside`、`circularTrackPadding` **4** | 值为 0 时，进度弧与轨道**无法区分** |
| A.3.7 | `calendar` 🔴 | 6 行日格各**11 px**高 | `cell_h = grid.height/6`（`calendar.rs:302`） | 行高 **42**（M2）/ **48**（M3）；`_maxDayPickerHeight` **294/336** | 日格仅为 Flutter 的 **1/4**（11 vs 42） |
| A.3.8 | `mobile_date_picker` 🟠 | 三列 `w=72`，行距 **24**，选中带 `rx=6`，item 字号 **10** | 行高 `rect.height/5`；字号 `h*0.38` clamp(10,15) | `_kItemExtent` **32**、选中 overlay 圆角 **8**、`_kPickerWidth` **320**、`_kPickerHeight` **216** | 行距 24 vs 32；且 10 px 字装 24 px 行 = 可读性缺陷（同 A.3.7 一族） |
| A.3.9 | `floating_label` 🔴🔴 | `<text x="8" y="20" font-size="14">Sample</text>` —— **没有浮动标签** | `label` 不是发布属性（`floating_label.rs:222-224` 只有 `text`/`placeholder`/`focused`）；`create_floating_label` 走 `label()`（`constructors.rs:1210`），而 `LABEL_PROPERTY_NAMES = ["text","title","message"]`（`control_backend/custom/mod.rs:15`）**首个命中即返回** ⇒ `"Sample"` 落进 `text`，`label` 恒空 | `InputDecoration.label/labelText` 是**头等 slot** | **一个控件专门用来演示浮动标签，而它的快照里没有浮动标签。** 审阅这套 artifact 的人会直接判定该特性坏了。修法三步：① `label` 进 `property_names`/`get`/`set`；② 让 `label()` 取**最具描述性**的属性而非首个命中；③ 补 `floating_label_behavior`(auto/always/never) |
| A.3.10 | `cupertino_switch` 🔴 | 未选中轨道 = `rgba(18,18,18)` = 窗口底色（**不可见**） | `CupertinoSwitch` 是 Material `Switch` 的**纯类型别名**（`cupertino/core.rs:41-47`），无 iOS 几何 | Flutter 是独立控件：**59×39** 整体 / **51×31** 轨道 / 拇指 r **14**；按压横向拉伸 **7.0**；**ON/OFF 标签**；拖动判定 **0.7/0.2**；200 ms `Curves.ease` | 与 B6 同源（`cupertino_switch` 不在 role 表）；另需 iOS 自有几何 |
| A.3.11 | `sparkline` ✅ 豁免正确 | 仅线 + 末点 | — | — | 快照 `diff` 仅导出器底色 —— **与 `control_color_exemptions.txt` 的说明完全一致**，反证豁免表是诚实工作的 |

## A.4 属性 / 方法 / 契约层面的不足（用户指令 #2 的「属性、方法」）

### A.4.1 🔴 文本输入缺**整个装饰槽模型**（1 个控件 vs 8 个区域）

| | 本仓 `line_edit` | Flutter `InputDecoration` |
|---|---|---|
| 属性面 | **5** 个：`text`/`placeholder_text`/`max_length`/`read_only`/`cursor_position`（`lineedit.rs:409-419`，`access.rs:504-511`） | `_RenderDecoration` 的 **8 个独立区域**：`prefixIcon`/`prefix`/input/`suffix`/`suffixIcon`/`label`/`helperError`/`counter`（`input_decorator.dart:588-688`） |
| 实测 | `grep prefix_icon\|suffix_icon\|helper_text\|error_text\|counter_text\|filled\|is_dense\|content_padding\|label_text` × `lineedit.rs`/`textedit.rs`/`textarea.rs` → **零命中** | 另有 `floatingLabelBehavior`/`floatingLabelAlignment`/`isCollapsed`/`isDense`/`contentPadding`/`filled`（`:2775-2815,3306-3350`） |
| 后果 | 无法表达计数器、辅助/错误行、前后缀图标、`prefixText` —— 且**调用方也表达不了**，因为没有属性可设 | Flutter 的 `errorText` 与主题 `colors.error` 联动，而本仓**有 `colors.error` 却零消费者路由**（见 AR3） |

### A.4.2 🔴 光标位置算错：`cursor_position` 存了，`draw` 不用它

```rust
// src/widget/input_widgets/lineedit.rs:649 —— 实测
let caret_x = text_x + context.measure_text(display_text, font).width as i32;
//                          ↑ 量的是【整个】字符串 ⇒ 光标【永远在末尾】
```

`cursor_position` 字段存在（`:26`）、有 getter/setter（`:205-213`）、被 `backspace`/`delete` 使用 —— 但 `draw` 不读它。
**实测**：`text = "Sample"`、`cursor_position = 0` 时，光标画在 `e` **之后**。
Flutter 不可能有此 bug，因为光标几何来自 `TextPosition` 派生的 `getOffsetForCaret`。
**[改进]** `caret_x = text_x + measure_text(&display_text[..byte_index(cursor_position)], font).width` —— 该文件已为此 import 了 `floor_char_boundary`（`:16`）。

### A.4.3 🟠 本仓有、Flutter 没有的控件（**是优势，应记录并保留**）

| 本仓控件 | Flutter 状态 | 为什么要紧 |
|---|---|---|
| `spinbox`/`number_picker`/`stepper`（数值步进） | Material **无** 数值 spinbox | ★ 如实纠正一个**命名冲突**：本仓 `stepper` 是**数值微调器**，而 Flutter `Stepper` 是**分步向导**（`StepState{indexed,editing,complete,disabled,error}`、连接线、`_kStepSize=24`、`connectorColor`；`stepper.dart:32-42,122-132,204-231`）。**两者不是同一个控件** —— 本仓缺**整个向导控件** |
| `cascader`（多级列联选） | 无（`MenuAnchor` 是嵌套菜单，非常驻列联） | 本仓优势；但应补 `menu_anchor.dart` 的 `FocusScope` 逐级焦点语义 |
| `editable_combo_box`/`multi_select_combo_box` | `DropdownButton` **不可编辑**（`dropdown.dart:1172` 明说去用 `DropdownMenu`），**多选完全没有** | 本仓优势 |
| `otp_input`（带 `length`/`masked`/`separator`/`is_complete`） | 无（典型做法是 N 个 `TextField` + N 个 `FocusNode`） | 本仓设计更好；建议改 `obscuringCharacter = '•'`（`text_field.dart:273`）并确认 `focused_index` 可被指针直接设定 |
| `color_well`+`color_history`+`color_picker`+`color_dialog`（完整取色工作流） | Material **与** Cupertino **都没有**取色器 | 本仓优势；但选色应能表达为 `ColorScheme` role 赋值而非裸值 |
| 图表族（14+ 控件，含 K 线/深度/盘口/雷达/热力图） | Material **无**图表（只有 `CustomPaint`） | 本仓优势；**但无外部基准 ⇒ 正确性全靠自身测试**，建议按 §A.6 的 6 项清单自查 |
| `rich_edit`/`markdown_editor` | 无（`SelectableText` 只读） | 本仓优势 |
| `terminal_view` | 无 | 本仓优势 |
| `signature_pad` | 无 | 本仓优势 |
| `qr_code`/`barcode_scanner` | 无 | 本仓优势 |
| `masonry_layout` | 框架内无（`SliverMasonryGrid` grep 零命中，在第三方包） | 本仓优势 |
| `splitter`/`dock_widget`/`mdi_area` | 无（Flutter 无分割条/dock/MDI；桌面窗口管理在插件层） | 本仓 Qt 血统的**真实优势** |
| `pagination`（`1 … 47 48 49 … 100`） | 无独立分页控件 | 本仓优势 |
| `chart_widgets` 的虚拟化 | Material `DataTable` **非虚拟化**（1 万行建 1 万个 `TableRow`，官方建议改用 `ListView.builder`） | 本仓 `data_grid` 的 `overscan_rows`/`visible_row_capacity` 是**真实的可伸缩性优势**；`frozen_columns` 也是 Flutter 没有的 |

### A.4.5 🟠 本仓有「声明」但 Flutter 有**完整契约**的接口（「方法」维度的不足）

| # | 本仓 | Flutter 的完整契约 | 后果 |
|---|---|---|---|
| a | `shortcut_editor` 捕获按键序列 | `ShortcutActivator`（`LogicalKeySet`/`SingleActivator`/`CharacterActivator`，`widgets/shortcuts.dart`） | 若本仓存的是**物理键码**，换键盘布局即失效。Flutter 的 activator 把「按了什么」与「逻辑键集」解耦 |
| b | `auto_complete_edit` 只发布 `suggestion_count`（`auto_complete_edit.rs:312-314`） | `optionsBuilder`/`optionsViewBuilder`/`displayStringForOption`/`optionsMaxHeight = 200.0`/`optionsViewOpenDirection`（`autocomplete.dart:59-120`） | 调用方**无法枚举候选项、读高亮索引、得知选中的 `T`**，也无法自定义选项渲染 |
| c | `drop_zone` 只有 `hovered` + `accepted_type`（`misc_widgets/drop_zone.rs:141-143`） | **5 个反馈态**：`onWillAcceptWithDetails`/`onAcceptWithDetails`（带 drop **offset**）/`onLeave`/`onMove`（带 pointer offset）/`builder(candidate, rejected)` + `HitTestBehavior`（`drag_target.dart:625-733,822`） | **只表达 1/5。** 无法表达「不能放这个」、插入线指示、拖出反馈、**落点位置**（列表重排必需） |
| d | `rating` 发布 `Float` 但**静默取整**（`rating.rs:129` `set_rating(value.round() as u32)`） | `ToggleButtons` 的 `isSelected: List<bool>` + 每段 token 集 | 写 `3.7` 得 `4`；半星不可表达；「Float 属性」在说谎 |
| e | 无 `selectableDayPredicate` | `bool Function(DateTime)`，且有**范围版** `SelectableDayForRangePredicate`（**接收待定 range**，故能拒绝跨越封锁日的区间；`date_picker.dart:381,1197`） | 只能 min/max 盒，无法表达「不可选周末/节假日」—— 这是最常见的真实需求 |
| f | `date_edit`/`time_edit` 是**纯文本框**（`date_edit.svg` = 一行 `2024-01-01`；`time_edit.svg` = 一行 `00:00:00`） | 字段是**对话框触发器**，且 calendar/input 两种入口模式都是一等公民（`DatePickerEntryMode`，`date_picker.dart:202-258`） | **选择器不可达。** 另实测一处**契约不一致**：`access.rs:494` 给 `DatePicker` 定义了 `calendar_popup`，而 `:497-503` 的 `TimePicker` 分支**没有它** —— capability 与默认值表不一致 |
| g | 无 `Stepper`（向导） | 见 A.4.3 —— `StepState` 5 态 + 连接线 + `controlsBuilder` | 本仓**缺一个整控件**（也说明「step」一词在本仓指数值步进） |

## A.5 无障碍（a11y）：`A11yState` 定义了但**无人填充**

| 证据 | 内容 |
|---|---|
| 本仓① | `src/platform/accessibility/types.rs:78-97` `A11yState { role, label, description, enabled, focused, selected, expanded, value, children }` —— **`value: String` 存在**（注释：`slider position, progress percent`） |
| 本仓② | **实测**：`grep A11yState src/` 只命中该文件自身 —— **零个控件构造它** |
| 本仓③ | role 映射**是**做的（`types.rs:571-685`：`Slider => A11yRole::Slider`、`CheckBox => CheckBox`、`Switch => Switch`…），但 `value`/`selected`/`focused` 从不填 |
| 本仓④ | **`A11yState` 结构本身缺 `checked`/`mixed`** —— 三态复选框的第三态**不可表达** |
| Flutter | `SemanticsProperties` 发布 `value`/`increasedValue`/`decreasedValue`（`slider.dart:1962-1976` 默认 `'${(value*100).round()}%'`）、`checked`/`mixed`、`selected`、`isTextField`/`isObscured`/`isMultiline`、选中范围；`onIncrease`/`onDecrease` 语义动作（`slider.dart:1963-1964`） |
| 后果 | 屏幕阅读器拿到「一个滑块」但**没有位置**；三态复选框的第三态丢失；代码编辑器只发布 `accessible_name`/`accessible_description` 两个字符串（`code_editor/editor.rs:2997-3014`），而它**已经画出** `Ln 1, Col 1` 状态栏（`code_editor.svg:27`）—— 信息在内部存在，只是没发布 |
| **[改进]** | ① 先给 `A11yState` 补 `checked: Option<bool>` / `mixed: bool`；② 为 `slider`/`progress_bar`/`progress_circle`/`rating`/`code_editor` 实现 `a11y_state()`，填 `value`；③ 补 `increasedValue`/`decreasedValue` 与增减动作（滑块的屏幕阅读器上下滑动调节） |

## A.6 无外部基准的控件：给「自查清单」而不是「无结论」

Flutter **没有**图表 / 取色器 / 富文本编辑器 / 终端 / 签名板 / QR / 条码 / 分页 / 数值步进 / cascader。
对这些，本附录不写「Flutter 有而我们没有」（那是假的），而是从 Flutter 的**约定**推出自查清单：

| 控件族 | 自查项（来自 Flutter 的哪些约定） |
|---|---|
| **图表族（14+）** | ① 有无 `CustomPainter.shouldRepaint` 等价的**脏标记**？（Flutter `isComplex`/`willChange`/`RepaintBoundary` 存在正是因为「每帧重绘」是性能缺陷）② 坐标轴刻度取整（nice numbers）、标签碰撞规避、数据↔像素变换**是否有单测**？（轴 bug 在静态快照里**不可见**）③ 图例是不是**属性**而非烤进绘制？④ **指针→最近数据点的逆变换**是否存在？（这是 tooltip/十字线的前提；本仓 `code_editor/render.rs:1344 position_at_point` 是同型先例）⑤ `data_range`（缩放/平移窗口）是否作为状态暴露？（可抄本仓 `calendar.rs:62-64` 的 `display_month` vs `selected_date` 分离范式）⑥ **有无无障碍摘要**？（`CustomPaint` 无语义，图表全靠视觉，缺这条影响最大：如「柱状图，12 系列，按月销售，峰值 3 月 40k」） |
| **`code_editor` / `terminal_view`** | Flutter 给出的**子系统**级清单：选中手柄、`TextSelectionOverlay`、放大镜（`MagnifierController`/`TextMagnifier`）、平台化 `TextSelectionControls`（desktop/material/cupertino 三套）。本仓 `code_editor/input.rs:312-358` 是「按下→拖→松开」的指针算术，**无手柄、无端点调整、无放大镜** |
| **`rich_edit`/`markdown_editor`** | ⚠️ **不要继承 code editor 的等宽捷径**：`render.rs:1206 cell_width()` + `:167 measured_cell_width` 对等宽字体正确（且是优化），但按 `cell_width * N` 算光标在**变宽跨度**（粗体/斜体/链接）下必然错位。`render.rs:1365 rect_for_range` 需变为**跨度感知**。建议把 caret/range 几何抽成 trait：等宽编辑器给快实现，富文本给精确实现 |
| **`signature_pad`** | ② 笔画捕获是否消费**带时间戳的指针增量流**？（Flutter `PointerMoveEvent` 有 `position`/`delta`/`pressure`/`timeStamp`）若按帧边界采样，快速输入会**多边形化**，而修法（在相邻 delta 间插值）**需要时间戳**；② 平滑是否作为**独立可测步骤**而非内联在 `draw`？ |
| **`qr_code`** | ① 纠错级别**可设**吗？（QR「差不多对」就扫不出来）；② 模块数 × 纠错级别 × 版式的快照测试（现有 `qr_code.svg` 只覆盖一个输入） |
| **`barcode_scanner`** | ① 相机权限；② 生命周期暂停/恢复（Flutter 约定是 `WidgetsBindingObserver.didChangeAppLifecycleState`）—— 后台仍开相机是**耗电 + 隐私**缺陷 |

## A.7 本仓**优于** Flutter 的设计（应保留，勿为对齐而删）

| # | 本仓 | 对比 | 依据 |
|---|---|---|---|
| 1 | **命令对象式撤销栈**（每命令有 `id()`/`description()`；`code_editor/editor.rs:61-100` 的 `TabSwitchCommand` 连「当前是哪个 tab」都进同一栈） | Flutter `UndoHistory`/`UndoHistoryValue` 是**值快照**，需启发式合并以免每次击键一条；且**无法**把非文本状态放进同一栈 | 抄 Flutter 的一点：`UndoHistoryController` 让工具栏与字段**共享**一个栈（本仓栈私有，菜单栏绑不上） |
| 2 | `carousel` 的手势释放**距离 ∨ 速度**且带显式**防抖下限**（`SWIPE_THRESHOLD_FRACTION 0.18` / `FLICK_VELOCITY_PX_PER_SEC 400` / `MIN_FLICK_FRACTION 0.02`，`carousel.rs:262-288`） | Flutter 由 physics 常量驱动；本仓的 2% 下限正是「一次普通点击在单帧里算出极高瞬时速度」的修法 | 保留 |
| 3 | `splitter` 的 `DragSession` **绝对比例快照**（`:47-54,454-458`） | Flutter `dismissible.dart` 的 `_dragExtent` 是**相对累加**，对重复投递敏感 | 这是**正确性**属性，非风格 |
| 4 | `stacked_widget` 禁用时**抑制信号**并给出原因（`:106-123`） | Flutter `IndexedStack` 无对应（`index == null` 直接全不显示） | 保留 |
| 5 | `frame` 的**七种形状**（含 `WinPanel`）+ 两色斜角**派生**而非双写（`frame.rs:29-45,196-200`） | Flutter `Container`/`Card` 无中线样式、无 `WinPanel` | 保留 |
| 6 | 语义色**严重度轴**（`error/warning/success/info`）+ `ALL` 常量 + `every_semantic_token_is_reachable` 门禁 | Flutter `ColorScheme` **无** warning/success/info | **切勿为了「对齐 Flutter」而删掉严重度轴**；要做的是补 `on_*`/`*_container` |
| 7 | `banner`/`notification_center` 的严重度取自**语义 token 再推离承载面**（`banner.rs:78-87,120-130`） | Flutter `MaterialBanner` **无严重度概念** | 保留（第 62 轮的 #109 修复） |
| 8 | `ThresholdExemption`/`legible_on`/`contrast_color` 的**按承载面推导**；`calendar` 按亮度**翻转混合方向**（`calendar.rs:558-580`） | Flutter 靠 **token 对**（`dayForegroundColor`/`todayForegroundColor`…）免费获得，但那是**设计者预先写好的**；本仓是**运行时推导**，对任意主题都成立 | 保留；建议**同时**补 token 对（`day_*`/`today_*`），让调用方能直接指定 |
| 9 | `DockWidget`/`MdiArea`/`Splitter`/`ToolBox`/`Frame`/`Pagination`/`Cascader`/`data_grid` 虚拟化 + `frozen_columns` | Flutter 全无 | 记录为**能力优势** |
| 10 | `ScrollArea::thumb_metrics` 是**纯函数**（`u64` 中间量防溢出 + 显式钳制 + 单测，`scrollarea.rs:474-491`） | Flutter 同类逻辑埋在 `ScrollbarPainter`/`ScrollPosition` 与 render object 里 | 可测性更好 |
| 11 | `popover` 阴影溢出时**宁可不画**而不是钳（`:317-335`，理由是「半条阴影读起来像渲染故障」） | Flutter 从 `Material` box 画并交给框架裁 | 有据的取舍，且是 census 基础设施在起作用 |
| 12 | `BottomSheet` 遮罩用**混合**而非固定色（`bottom_sheet.rs:210-218`） | Flutter 默认 `Colors.black54` —— 固定色在深色面上**会变亮** | 构造上更正确。⚠️**但本仓当前实现有 bug**，见 B23（方向错了） |

## A.8 追加后的修订优先级

> **权威执行计划在 [§6.6 批次的划分](#66-批次的划分每批--一轮55-57)**：
> 本节只说明「附录 A 的条目落在 §六 的哪一档」，逐条的修法/前置/判据均在 §六。

本节内容已**全部收入 [§六](#六改善计划按优先级含依赖顺序与完成判据)**（无遗漏），映射如下：

| 附录条目 | 落入 | 具体位置 |
|---|---|---|
| AR1 触控区零接线 | **P0-2** | §6.1 P0-2（阶段2 接线） |
| AR2 尺寸钉死 | **P2-1** | §6.3 P2-1（依赖 P0-2 的「画/占分离」） |
| AR3 主题 token 面 | **P2-6** | §6.3 P2-6 |
| AR4a 状态化主题 | **P0-3** | §6.1 P0-3（阶段2 接线） |
| AR4b/c `font_scale`/`min_touch_size` | **P2-7** | §6.3 P2-7 |
| AR5 RTL | **P2-10** | §6.3 P2-10（先修 `slider`） |
| AR6 三个 `pressed` | **P0-3 + P0-4** | §6.1（状态与动画互为一对） |
| AR7 动画零使用 | **P0-4** | §6.1 P0-4（阶段2 接线） |
| AR8 光标闪烁 | **P2-3** | §6.3 P2-3（依赖 P0-4 的 `tick`） |
| A.3.1–A.3.8 快照几何 | **P2-1 / P0-6c** | §6.1 P0-6c（`switch`/`radio`/`progress` 等） |
| A.3.9 `floating_label` | **P0-6j** | §6.1 P0-6j |
| A.4.1 装饰槽模型 | **P1-11** | §6.2 P1-11 |
| A.4.2 光标算错 | **P1-12** | §6.2 P1-12 |
| A.4.3 本仓优势 / `stepper` 命名冲突 | **P4-2 / P2-11** | §6.4 P4-2、§6.3 P2-11 |
| A.4.5 契约过薄 | **P2-9** | §6.3 P2-9 |
| A.5 a11y 零填充 | **P2-8** | §6.3 P2-8 |
| A.6 无基准自查清单 | **P4-3** | §6.4 P4-3（审计任务） |
| A.7 优于 Flutter 的 12 项 | **P4-5** | §6.4 P4-5（**保留，勿动**） |

**批 1 从这里开始：** P0-1（抽 `text_line` 原语）—— 它无前置、1 处新增，
且是 A 组 25 条的公共前置（§6.6）。

## A.9 附录结论

**Flutter 比对带来了 A–E 组不可能有的一类东西：标准数值。**
`48`（`kMinInteractiveDimension`）、`42/48`（日历行）、`52×32`（开关轨道）、`4.0`（进度条高）、
`32`（chip 高 / Cupertino 行距）、`300/100/200/1800/500 ms`（时长）、`56/48/80`（三种导航条高）——
有了这些，「这个尺寸偏小」不再是我的审美判断，而是**与一个被数亿设备验证过的规范可比**。

**而最大的收获是四个「机制已建、无人接线」：**
`touch_target`+`contains_point_with_touch_expansion`（AR1）、`WidgetState`+`resolve_style_for_state`（AR4a）、
`font_scale`/`min_touch_size`（AR4b/c）、`src/style/animation.rs` 1582 行 + 三个 `pressed` 字段（AR6/AR7）。
**它们的共同形状是：本仓已经写出了正确的抽象，只是没有一个控件去调用它。**
这与 §三 的 R1（正确写法已有 4 处却未共享）是**同一种病**，只是层级更高——
R1 漏的是「一个函数」，本附录漏的是「五套子系统」。

**最后一个应当记住的判断**：本仓在**桌面/设计器/工控**方向（dock、MDI、分割条、多级列联、
取色工作流、14 种图表、富文本、终端、虚拟化表格 + 冻结列、数值步进、分页）**明显强于 Flutter**；
在**移动/Material 规范符合度**（触控区、状态层、动画、主题 token 面、a11y、编辑子系统）**明显弱于 Flutter**。
因此「对齐 Flutter」不应是全面目标 —— **应当是：借它的数值与子系统清单来补齐交互与无障碍，
同时保住本仓在桌面方向已经领先的部分**（§A.7）。
