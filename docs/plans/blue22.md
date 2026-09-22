# BLUE22 — 让控件「真正能用、外形美观、简洁高效」：BLUE21 遗留收口 + Qt Quick Controls / Flutter 三方对标

> 依据：[`principle.md`](principle.md)（继承 BLUE1–BLUE21，含 #1–#111）
> 前置：[`blue21.md`](blue21.md)（缺陷登记表 + 附录 A + §六 改善计划）、[`../log/log-20260922-2.md`](../log/log-20260922-2.md)（第 64/65 轮执行记录）
> 参考实现（**只读，不引入依赖**）：
> - Qt Quick Controls 2：`/home/mikeli/workspace/qtdeclarative`（`src/quickcontrols/basic/*.qml` 为最可复制的一档）
> - Flutter：`/home/mikeli/workspace/flutter`（`packages/flutter/lib/src/material/*.dart`）
> 目标版本：**2.6.1 → 2.7.0**
>
> **📌 执行进度：[附录 F](#附录-f--执行进度与未完成项2026-09-23-第-67-轮实跑取证)**。
> 已交付到 2.6.1（P0-1 / P0-1b / P0-2 / P0-3 / P0-4 / P0-5..P0-10 / P2-2 已完成），
> **未完成的主体是「组合控件由布局组装」（F-1 / F-2），施工细则见 F.2.2**。

---

## 0. 本计划的两条来源，以及为什么必须合在一起

| 来源 | 内容 | 为什么不能单独做 |
|---|---|---|
| **A. BLUE21 遗留** | 第 64/65 轮明确「未做 / 未完成」的条目 | 它们是**已知缺陷**，不做就是欠账 |
| **B. QML / Flutter 对标** | 两份参考实现里**本仓没有**的机制与数值 | 只修缺陷**不会让控件变好看**——美观来自「一致的度量体系」，而本仓缺的正是这套体系 |

**核心判断（本轮最重要的结论）**：BLUE21 把缺陷从 81 条收敛到 7 个「坏写法」，方向正确；
但对标 QML/Flutter 后发现，**本仓真正的问题不是「画错了」，而是「没有度量体系」**——

> QML 的 `Button.qml` 之所以在 5px 文字下仍是一个 100×40 的按钮，是因为它的
> `implicitWidth = max(背景隐式宽 + inset, 内容隐式宽 + padding)`：**背景是一个「最小可点区」地板**，
> 而不是装饰。本仓反过来：控件尺寸由外部 `rect` 反推内容（BLUE21 P2-1 已记为「几何由 rect 反推，11 控件」）。

这就是为什么 `snapshots/svg/switch.svg` 是 **240×120**：导出画布是固定的 `CENSUS_RECT = 240×120`
（`src/widget/census.rs:52`），而控件把 `rect` 当作**绘制指令**而非**可用区域**。
后果：开关画成了 240×120 的体育场，单选按钮 `r=30`，进度条是一块 240×120 的板。

**因此本计划的顺序是：先立度量体系（P0），再用它刷新外形（P1），最后才做装饰性主题（P2）。**
反过来做（先加 `outline` 等 token）会在错误的几何上刷更漂亮的漆。

### 0.1 度量体系有**两根**、不只一根

上面讲的是**单控件**的度量（`implicit size`）。但对标后发现还有第二根，且同样断链：

| 根 | 问题 | 在哪 |
|---|---|---|
| **①单控件度量** | 控件把 `rect` 当绘制指令 ⇒ 尺寸不可内容驱动 | §6.1（`ControlMetrics`） |
| **②尺寸通道** | `Layout::update` **只能写不能读** ⇒ 布局无法问子控件要尺寸，只能要求调用方预存（`flex.rs:167` `set_child_sizes`） | **§B.5.2**（`Hints` + `Layout::arrange`） |
| **③提示形状** | `size_hint() -> Size` **无对轴参数、无地板、无上限** ⇒ 176 个实现只有 1 个消费者 | **§B.5.1**（`AxisHints{min,pref,max}`） |
| **④组合组装** | 组合控件**手算子控件几何**，不用已有的 15 种布局 | **§B.5.2 第三步**（`CompositeBuilder`） |

**四根中，②③ 是根因，④ 是结果。** 对标 Flutter/Qt 后确认：三家都是
**「父布局问子控件要尺寸」**（Flutter `layout_helper.dart:64`、Qt `qquickdialogbuttonbox.cpp:349`、
QML `qquickcontrol.cpp:1754`），**只有本仓是「调用方告知布局」**。

**它们必须一起立**，因为本仓过半的控件实际是**组合控件**（`dialog`/`spin_box`/`combo_box`/`menu`/
`tool_bar`/`tab_widget`/`scroll_area`/`list_view`/`grid_table`…）。
只立单控件度量，就会在 11 个组合控件里各写一遍内边距计算——
**与 BLUE21 付出 76 次返工的形态完全相同。**

---

## 1. 现状取证（本轮实跑，非引用）

```text
$ grep -rn "TextDirection" src/core/mod.rs src/widget/display_widgets/slider.rs
src/core/mod.rs:58:pub use text_direction::TextDirection;          ← 已建
src/widget/display_widgets/slider.rs:39:    direction: crate::core::TextDirection,  ← 已接线

$ grep -c "accessible_value" src/widget/widget_trait.rs      → 10（已建）
$ grep -c "CursorBlink" src/style/animation.rs               → 5（已建）
$ grep -rn "outline_variant|surface_container|scrim|inverse_surface" src/theme src/style → 0 命中
$ grep -n "suggestion_count" src/widget/capability/properties_input.in.rs:329  ← 仍只有它
$ ls src/widget/*/stepper.rs → src/widget/container_widgets/stepper.rs（本仓语义=数值微调）

$ bash tools/check_widget_kind_count.sh
WidgetKind variants (parsed from src/widget/kind.rs): 180
✅ check_widget_kind_count: 22 document(s) state the correct count (180)

$ grep -o 'width="[0-9]*" height="[0-9]*"' snapshots/svg/switch.svg | head -1
width="240" height="120"      ← 全部 188 个控件都是这个画布
```

**`Colors` 结构体的现状**（`src/theme/types.rs:216-240`）——共 **11** 个字段：

```
background  foreground  primary  secondary  accent  error  warning  success  disabled  info
```

（对照 Flutter `ColorScheme` 的 40+ 角色与 QML `QQuickColorGroup` 的 21 个角色，
本仓的角色面**极窄**——这正是「控件看起来单调」的根因之一，而不是绘制代码画错了。）

**`Motion` 已存在**（`src/theme/types.rs:65`）：`fast=100 / normal=200 / slow=300 / easing=EaseOut`，
注释已注明对齐 Flutter 的 `kThemeChangeDuration`(200) / `kRadialReactionDuration`(100) / switch 的 300ms。
**这一项是 BLUE21 已经做对的，且正是 QML/Flutter 双向印证的做法。**

---

## 2. 三方度量对照表（本计划的核心证据）

所有 QML 数值引自 `/home/mikeli/workspace/qtdeclarative/src/quickcontrols/basic/<C>.qml`，
Flutter 引自 `/home/mikeli/workspace/flutter/packages/flutter/lib/src/material/<f>.dart`。
**本仓列是实测的 `implicit`-等效值（由 `CENSUS_RECT` 反推）**。

### 2.1 按钮

| 维度 | QML Basic | Flutter M2 | Flutter M3 | 本仓现状 | 建议目标 |
|---|---|---|---|---|---|
| padding | `6`（`Button.qml:17`） | `EdgeInsets.all(8)`（`text_button.dart:444`） | `h12 v8`（`text_button.dart:443`） | 无（撑满 rect） | **h12 v8** |
| horizontalPadding | `padding + 2` = 8（`Button.qml:18`） | — | — | — | 由 padding 派生 |
| spacing（图标↔文字） | `6`（`Button.qml:19`） | — | — | 无 | **6** |
| icon 尺寸 | `24×24`（`Button.qml:21-22`） | — | `18`（`text_button.dart:561`） | 无 | **18**（M3） |
| 最小尺寸 | 背景地板 `100×40`（`Button.qml:39-40`） | `64×36`（`text_button.dart:394`） | `64×40`（`text_button.dart:555`） | 无 | **64×40** |
| 圆角 | — | `4`（`text_button.dart:396`） | StadiumBorder（`text_button.dart:590`） | 依 rect | **4** |
| 触控地板 | 背景 `40` 高（隐式） | `48`（`constants.dart:27`） | `48` | 已接 `touch_target` | **48** |

### 2.2 开关 / 复选框 / 单选

| 控件 | QML Basic | Flutter M3 | 本仓现状 | 建议目标 |
|---|---|---|---|---|
| Switch 轨道 | `56×28`，r`8`，padding`6`（`Switch.qml:22-31`） | `52×32`，thumb r`14`/inactive r`8`（`switch.dart:2355-2379`） | **240×120 体育场** | **52×32**，thumb r14 |
| Switch 手柄 | `28×28` r`16`（`Switch.qml:39-41`） | `28` 直径（`switch.dart:2358`） | — | r14（28 直径） |
| CheckBox 指示器 | `28×28`，border 1/2（`CheckBox.qml:23-24,30`） | 盒子 `18.0`，stroke `2.0`，r`2`（`checkbox.dart:405,651,1046`） | — | **18×18**，r2，stroke2 |
| Radio 指示器 | `28×28`，dot `20×20`（`RadioButton.qml:23,45`） | 外 r`8`，内 r`4.5`（`radio.dart:31-32`） | **r=30** | **外 r8**（16 直径） |
| 触控地板 | 28（隐式） | `48×48`（`checkbox.dart:516-520`） | 已接 | **48** |
| toggle 时长 | `SmoothedAnimation velocity 200`（`Switch.qml:54-57`） | `300ms`（`switch.dart:2387`）/ M2 `200` | `Motion::slow`=300 | **300** |

### 2.3 滑块 / 进度

| 控件 | QML Basic | Flutter | 本仓现状 | 建议目标 |
|---|---|---|---|---|
| Slider 手柄 | `28×28` r`14`（`Slider.qml:22-24`） | enabled r`10`，pressed elevation `6`（`slider_parts.dart:678-681`） | — | **r10**（20 直径） |
| Slider 轨道 | 高 `6`，r`3`（`Slider.qml:41,44`） | 高 `2`（M2，`slider_theme.dart:356`） | — | **高 4，r2** |
| **手柄行程 inset** | `availableWidth - handle.width`（`qquickslider.cpp:117`） | thumb 半径（`slider_parts.dart:678`） | ✅ 已修（第 65 轮，见 §4.3） | 保持 |
| ProgressBar | 高 `6`，宽 `200`（`ProgressBar.qml:27-30`） | M3 高 `4`，r`2`，gap `4`（`progress_indicator.dart:1624-1636`） | **240×120 板** | **高 4，r2** |
| Circular 进度 | — | stroke `4`（`progress_indicator.dart:1593`） | — | **stroke 4** |
| 不确定态时长 | — | linear `1800ms`（`progress_indicator.dart:23`） | 无 | **1800** |

### 2.4 文本 / 容器 / 表面

| 控件 | QML Basic | Flutter | 本仓现状 | 建议目标 |
|---|---|---|---|---|
| TextField padding | `6`，left `padding+4`=10（`TextField.qml:19-20`） | M3 outlined `12/20/12/12`（`input_decorator.dart:2625`） | — | **h12** |
| 输入框最小高 | 背景 `40`（`TextField.qml:50`） | `kMinInteractiveDimension`=48（`input_decorator.dart:1116`） | — | **48** |
| 浮动标签缩放 | — | `0.75`（`input_decorator.dart:41`） | `floating_label` 已实现 | 对齐 **0.75** |
| Card 圆角 / margin | — | r`12`，margin `4`，elev `1`（`card.dart:322,310`） | — | **r12** |
| Dialog padding / 圆角 | `12`（`Dialog.qml:21`） | M3 r`28`，elev `6`，minW `280`（`dialog.dart:1963-1966,275`） | — | **r28, minW 280** |
| Divider 间距 | — | `space 16`，thickness `1`（`divider.dart:360-365`） | — | **space16 th1** |
| ToolBar 高 | `40`（`ToolBar.qml:23-24`） | `56`（`constants.dart:30`） | — | **56**（M3 AppBar 是 64） |
| ScrollBar 厚 | 内容 `6`，min size 按比例（`ScrollBar.qml:19-25`） | `8`，min length `48`，r`8`（`scrollbar.dart:12-16`） | — | **厚 8，min 48** |
| Tooltip | — | 桌面高 `24` pad `8/4` font `12`（`tooltip.dart:425-448`） | — | **24 / 8,4 / 12** |

### 2.5 字号与缩放

| 维度 | Flutter | 本仓 | 建议 |
|---|---|---|---|
| 基准字号 | `kDefaultFontSize = 14.0`（`text_painter.dart:42`） | — | **14** |
| 触控地板 | `kMinInteractiveDimension = 48.0`（`constants.dart:27`） | `touch_target` 已接 | 保持，**桌面默认 `shrinkWrap`=40**（`theme_data.dart:408`） |
| 密度步长 | `4.0` px/单位（`theme_data.dart:3307-3314`） | `visual_density` 已有概念 | **4**；`comfortable=-4`，`compact=-8` |
| 文本缩放上限 | AppBar `1.34`，NavBar `1.3`（`app_bar.dart:44`） | P2-7 已放开 app_bar 22pt 上限 | **按控件设上限，勿全局** |

---

## 3. 优先级总表

> 判据列写的是**可执行命令 / 可观测产物**，不是形容词。所有条目按「先体系后装饰」排序。

| # | 优先级 | 条目 | 一句话 | 判据 |
|---|---|---|---|---|
| 1 | **P0-1** | **度量体系**：`implicit size = max(背景地板, 内容 + padding)` | 立 `ControlMetrics` 与「画/占分离」 | 新增 `src/widget/metrics.rs` + 单测：5px 文字的按钮仍是 `≥64×40` |
| 2 | **P0-1b** | **尺寸通道 + 三值提示**（**§B.5.2**）：`Widget::hints()` 替 `size_hint`，`Layout::arrange(.., &[ChildInfo], ..)` | 先接上「布局问控件」这条通道；这是组合控件一切问题的根因 | 单测：布局不再需要 `set_child_sizes` 就能排布；`grep set_child_sizes` 归零 |
| 3 | **P0-1c** | **`CompositeBuilder`**（**§B.5.2 第三步**） | 把「创建 + 收集提示 + 问布局 + 回写」固定成一条链 | 三个样板控件跑通 |
| 3 | **P0-2** | **背景即最小可点区地板**（修 BLUE21 P2-1 的 11 控件） | 控件不再把 `rect` 当绘制指令 | `switch.svg` 不再出现 240 宽轨道；11 控件逐个快照 diff |
| 4 | **P0-3** | **四层 padding 级联**：side → axis → uniform | `left/right/horizontal/padding` 各带 `has_*` | 单测：写 `left_padding` 只改左 |
| 5 | **P0-4** | **`spacing` 只做「指示器↔文字」间距** | 与 padding 语义分离 | 门禁：`spacing` 不得出现在兄弟布局 |
| 6 | **P0-5** | **`visual_focus` 语义**：焦点环只在键盘导航时画 | 鼠标点击不画焦点环 | 单测：`click` ⇒ 无环；`Tab` ⇒ 有环 |
| 7 | **P0-6** | **`clicked` 必须「释放于内部」** + `canceled` | `released ≠ clicked` | 按钮测试：拖出后释放 ⇒ `canceled`，无 `clicked` |
| 8 | **P0-7** | **`pressed` 是连续量**（拖回内部重新为真） | 不是边沿触发 | 单测：拖出 `pressed=false`，拖回 `pressed=true` |
| 9 | **P0-8** | **`hover`/`focus`/`press` 三条独立状态** | 不合并为 `highlighted` | 已部分（P0-3/P0-4）；补 `focused` 动画 |
| 10 | **P0-9** | **`toggled`/`moved` 只在真变化时发** | 幂等 setter 不发信号 | 单测：`set_value(same)` 零信号 |
| 11 | **P0-10** | **`Colors` 扩面**（P2-6 遗留） | 加 6 角色 + `Font` 2 字段 | 新增字段有默认值（不破坏反序列化） |
| 12 | **P1-1** | 按 §2 表刷新**每个控件的默认度量** | 用 P0-1 体系实现 | 188 快照逐个评审 |
| 13 | **P1-1b** | **组合控件逐个改用 `CompositeBuilder`**（**§B.8**，11 个） | 先 `dialog`/`spin_box`/`combo_box`/`split_button` | 快照 diff 可人眼判为「留白一致」 |
| 14 | **P1-2** | **RTL 铺开**（`slider` 已完成） | 其余方向敏感控件接 `TextDirection` | `progress_bar`/`range_slider`/`tab_bar`/`app_bar` |
| 15 | **P1-3** | a11y 铺开（`value` 已建） | 补 `checked`/`mixed` 填充 | `checkbox`/`switch`/`radio` 报三态 |
| 16 | **P1-4** | 契约加厚（P2-9 遗留） | `auto_complete`/`drop_zone`/`rating`/`shortcut_editor` | 逐条可读回 |
| 17 | **P1-5** | E6 **分组/片段**原语 | 透明节点 | `engine` 不再丢子树 |
| 18 | **P1-6** | `stepper` 命名裁定（P2-11） | 改名 or 补向导控件 | 二选一，不留悬空 |
| 19 | **P2-1** | 主题 `outline`/`scrim` 等装饰 token | 在正确几何上上色 | `outline_variant` ≠ focus ring |
| 20 | **P2-2** | 动效铺开（`Motion` 已有） | 用 token 驱动所有过渡 | 门禁：字面量时长归零 |
| 21 | **P2-3** | 字体 `letter_spacing`/`line_height` | 2× 文本缩放前置 | 缩放不裁剪 |
| 22 | **P2-4** | P4 类新功能 | 另立计划 | — |

> **为什么 P0-1b（组合体系）必须是 P0**：前 20 项里超过一半的控件实际是**组合控件**
> （`dialog`/`spin_box`/`combo_box`/`menu`/`tool_bar`/`tab_widget`/`scroll_area`/`list_view`…）。
> 不在组合层立规范，就会在 11 个控件里各写一遍几何——**正是 BLUE21 付出 76 次返工的那一形态。**

---

## 4. 现状已修（勿重复劳动）

第 65 轮已闭合，本计划**不重复**，仅登记以免返工：

| 条目 | 状态 | 证据 |
|---|---|---|
| **引擎完成帧回调丢失** | ✅ | `animation.rs` 两条路径同修；反向注入 2 测试变红 |
| **`CursorBlink`**（P2-3） | ✅ 4 控件接入 | `CURSOR_BLINK_HALF_PERIOD_MS = 500` |
| **`accessible_value`**（P2-8 结构） | ✅ | `A11yState.checked/mixed` + `to_announcement_string()` |
| **`TextDirection`**（P2-10 类型） | ✅ `slider` 已接 | `src/core/text_direction.rs` |
| **C8 死绑定 + 4 处同形** | ✅ | 冰山扫描已扫 |
| **`slider` 取值/绘制非互逆** | ✅ | 见 §4.3 |

### 4.3 第 65 轮的一个额外收获（写进记录以免再犯）

修 `slider` RTL 时，**往返测试**（`value → x → value`）暴露出一个**与 RTL 无关的既有缺陷**：
`value_to_pixel_pos` 按半个手柄 inset，而 `pixel_pos_to_value` 用**全宽**——两者不是互逆，
导致「点击手柄得不到手柄显示的值」，且误差随范围增大。
**这正是 QML `qquickslider.cpp:117` 显式 `- handle.width/2` 的原因**：
`availableWidth - handle.width` 与 `positionAt` 必须共用同一个 inset。
本仓现已对齐；`range_slider` 本来就是对的（共用 `handle_radius`），说明**slider 是那个离群者**。

**教训：两个方向的映射必须由同一个 inset 推导，并有一条往返测试钉住。**

---

## 5. QML / Flutter 中「本仓值得抄」的机制清单

> 全部为**机制**（几行代码可落地），不是「引入依赖」。按性价比排序。

| # | 机制 | 出处 | 为什么值得抄 | 落点 |
|---|---|---|---|---|
| 1 | `implicitWidth = max(bg + inset, content + padding)` | `Button.qml:12-15` | 背景是**触控地板**，不是装饰；一行公式解决「小控件太小」 | P0-1 |
| 2 | `visualFocus = activeFocus && (Tab\|Backtab\|Shortcut)` | `qquickcontrol.cpp:1433,126` | 鼠标点击**不画焦点环**；手写工具箱最常漏 | P0-5 |
| 3 | `clicked` 需 `contains(point)`；否则 `canceled` | `qquickabstractbutton.cpp:204,198` | `released ≠ clicked`，拖出不触发 | P0-6 |
| 4 | `pressed` 随指针回入恢复为真 | `qquickabstractbutton.cpp:179` | 拖出后按钮必须回弹 | P0-7 |
| 5 | `down` 与 `pressed` 分离（`explicitDown`） | `qquickabstractbutton_p.h:31-32` | 「指针在我身上」≠「我该画凹陷」 | P0-8 |
| 6 | `toggled` 仅真实变化时发 | `qquickabstractbutton.cpp:267-275` | 冗余 set 不发信号 | P0-9 |
| 7 | 互斥组不可被点掉 | `qquickabstractbutton.cpp:249-265` | 朴素实现会把 radio 关掉 | P0-9 |
| 8 | 四层级联 padding（side→axis→uniform） | `qquickcontrol_p_p.h:68-74` | `leftPadding` 写一次即可 | P0-3 |
| 9 | `spacing` = 指示器↔文字，绝不用于兄弟 | `CheckBox.qml:61`, `ComboBox.qml:21` | 语义分离 | P0-4 |
| 10 | 内容盒 = `availableWidth/Height` @ `(leftPadding,topPadding)` | `qquickcontrol.cpp:382-383` | 背景用另一套公式（减 inset） | P0-1 |
| 11 | `minimumSize` 是**分数**而非常量（ScrollBar） | `ScrollBar.qml:19` | `height/width` 防细条拇指消失 | P1-1 |
| 12 | 淡出 = `Pause 450ms` + `Number 200ms` | `ScrollBar.qml:35-47` | 即时出现、延迟消失 | P2-2 |
| 13 | `visualPosition = 1 - position`（含竖向） | `qquickslider.cpp:395-400` | 绘制值与逻辑值分离 | P1-2 |
| 14 | 镜像**重算坐标**而非负 scale | `qquickslider.cpp:126` | 负 scale 会让拖拽反向 | P1-2 |
| 15 | `Color.blend(a, b, factor)` 单一混色原语 | `Basic/*.qml` 全场 | 悬停/按下/禁用都是 blend | P0-10 |
| 16 | 禁用态切换 `palette.disabled` 组 | `qquickpalette_p.h:32-34` | 不必逐控件写 `enabled?x:y` | P0-10 |
| 17 | 自动重复 `300ms` 延迟 / `100ms` 间隔 | `qquickabstractbutton_p_p.h:98-99` | 数值可直接采用 | P2-2 |
| 18 | `pressAndHold` 与 `autoRepeat` **互斥** | `qquickabstractbutton.cpp:160-166` | 否则长按连发 | P2-2 |
| 19 | 按住时长按**受拖拽距离取消** | `qquickabstractbutton.cpp:183-184` | 用平台 `startDragDistance` | P2-2 |
| 20 | `key.isAutoRepeat()` 必须过滤 | `qquickcombobox.cpp:2205` | 否则长按键盘机枪式重绘 | P0-6 |
| 21 | `focusPolicy` 平台相关（macOS=TabFocus） | `qquickabstractbutton.cpp:105-112` | 桌面点击不应夺焦点 | P0-5 |
| 22 | `KeyboardActivation` 来自平台主题 | `qquickabstractbutton.cpp:261-264` | 不硬编码 Space/Enter | P0-6 |
| 23 | `zero thickness = 1 device pixel` | `divider.dart:86-87` | DPI 正确 | P1-1 |
| 24 | 停用器件时**主动清除**瞬时态 | `button_style_button.dart:359-362` | 禁用后不残留 pressed | P0-7 |
| 25 | 动画控制器以**当前值**初始化 | `toggleable.dart:156,171,180` | 中断不重启 | P2-2 |
| 26 | 前向/反向**各自一条曲线** | `toggleable.dart:159-163` | `easeIn` 进 `easeOut` 出 | P2-2 |
| 27 | 密度**不得**压缩横向 padding | `button_style_button.dart:502` | 否则 desktop compact 变 0 | P1-1 |
| 28 | 触控地板**另起渲染对象**（不画） | `button_style_button.dart:632-634` | 扩展命中区不改视觉 | 已部分 |
| 29 | `spacing` 在文本变大时**反向变小**（chip 8→4） | `chip.dart:2554-2562` | 大字号下不溢出 | P2-3 |
| 30 | `selected` 与 `checked` **互斥** | `chip.dart:1511-1512` | 语义 flag 不可同置 | P1-3 |
| 31 | `increasedValue`/`decreasedValue` 供滑块朗读 | `slider.dart:1958-1975` | 屏幕阅读器要能增减 | P1-3 |
| 32 | 主题缺色时**回退到另一角色**（非黑非 null） | `color_scheme.dart:845,894,1032` | 局部主题仍可渲染 | P0-10 |
| 33 | **每轴 min/pref/max 三值提示** | `qquicklayout.cpp:1176` 真值表 | 一值提示无法表达「可缩但不小于 X」；且不引入 Flutter 的 O(N²) | **P0-1b** |
| 34 | **父布局「问」子控件，而非被子控件告知** | `layout_helper.dart:64`、`qquickdialogbuttonbox.cpp:349` | 本仓布局被迫 `set_child_sizes(..)`（`flex.rs:167`），正是断链形状 | **P0-1b** |
| 35 | **控件自带的默认布局策略**（`sizePolicy`）可被使用方覆写 | `qquicklayout_p.h:221` | 滑块该被拉宽、按钮不该；两者 `pref` 相同，无法从尺寸区分 | **P0-1b** |
| 36 | **提示变化主动通知父布局**（而非轮询） | `qquickcontrol.cpp:390` `addImplicitSizeListener` | 内容变了尺寸才知道 | P0-1b |
| 37 | **归一化提示：`min <= pref <= max` 在构造时完成** | `qquicklayout.cpp:1218` `normalizeHints` | 让非法状态不可表示，比 Qt 事后修正便宜 | P0-1b |
| 38 | **组合控件的固有尺寸 = max(背景地板, 内容 + padding)** | `qquickcontrol.cpp:1754` | 组合控件能被父布局测量 | P0-1c |

### 5.1 明确**不抄**的（避免引入不需要的负担）

| 项 | 为什么不抄 |
|---|---|
| M3 tonal palette / `fromSeed` | 需要 HCT 色彩空间与 9 变体生成器，我们的 `Colors` 是**名字驱动**的，抄了会把「主题=一组命名角色」变成「主题=一个算法」 |
| `WidgetStateProperty` 首次匹配 + `null as T` 抛错 | 我们的 `Style` 是**字段覆盖链**（`style.X.or(themed_Y)`），已由门禁 `check_style_derived_short_circuit` 守护；引入 map 解析是两套机制 |
| QML `LayoutMirroring` 全量镜像 | 会牵动 padding/border/图标/文本行序/滚动条侧，是独立工程；本仓只做**方向敏感控件**层面（`TextDirection`） |
| `InkWell` 水波纹 | 需要独立的墨迹层与裁剪；本仓是立即模式绘制，性价比低。**悬停/按下用 `Color.blend` 即可**（QML Basic 就是这么做的） |
| Android 触感/haptics | 平台差异大、需权限；BLUE21 §七 已明确不判为缺陷 |
| Flutter 的 `SemanticsRole` 全套 | 许多角色在 Flutter 里也未实现（`semantics.dart:195-196` 明确 `_unimplemented`），不值得追 |

---

## 6. 每一条的施工细则

### 6.1 P0-1 立度量体系（**这是本轮的地基，必须先做**）

新增 `src/widget/metrics.rs`：

```rust
/// 一个控件在**内容驱动**下的固有尺寸。
///
/// 为什么不直接用 `rect`：`rect` 是**可用区域**（可能是 240×120 的画布），
/// 而控件要画多大是另一件事。两者混用就是 `switch.svg` 变成 240×120 体育场的原因。
pub struct ControlMetrics;

impl ControlMetrics {
    /// 固有尺寸 = max(背景地板, 内容 + padding)。
    ///
    /// 这个 `max` 是 QML `Button.qml:12-15` 的全部要点：**背景是一个最小可点区地板**，
    /// 所以 5px 文字仍得到 100×40 的按钮，而不是 100×18。
    pub fn implicit_size(content: Size, padding: EdgeInsets, floor: Size) -> Size;

    /// 内容盒 = 可用区域减去 padding（QML `availableWidth/Height`）。
    pub fn content_box(rect: Rect, padding: EdgeInsets) -> Rect;
}
```

**判据**：单测断言「内容很小 ⇒ 尺寸仍等于地板」与「内容很大 ⇒ 尺寸由内容决定」。

### 6.2 P0-2 用 P2-1 的常量表修 11 个控件

按 §2 的表逐个改。**每个控件一条快照 diff**，DIFF 必须显示「变小且居中」。

目标常量（可直接落为 `const`）：

```rust
pub const SWITCH_TRACK: Size = Size { width: 52, height: 32 };   // Flutter M3
pub const SWITCH_THUMB_RADIUS: u32 = 14;
pub const CHECKBOX_BOX: u32 = 18;                                 // Flutter checkbox.dart:405
pub const RADIO_OUTER_RADIUS: u32 = 8;                            // Flutter radio.dart:31
pub const PROGRESS_HEIGHT: u32 = 4;                               // Flutter M3
pub const PROGRESS_RADIUS: u32 = 2;
pub const SLIDER_TRACK_HEIGHT: u32 = 4;
pub const SLIDER_THUMB_RADIUS: u32 = 10;                          // Flutter slider_parts.dart:678
pub const BUTTON_MIN: Size = Size { width: 64, height: 40 };      // Flutter M3
pub const TOUCH_TARGET_MIN: u32 = 48;                             // Flutter constants.dart:27
```

### 6.3 P0-5 `visual_focus`

```rust
/// 焦点环是否应当绘制。
///
/// QML `qquickcontrol.cpp:1433` + `:126`：`activeFocus && (Tab | Backtab | Shortcut)`。
/// **鼠标点击不应画焦点环**——这是手写工具箱最常漏掉的一条，
/// 因为「有焦点」和「用户正用键盘导航」是两件事。
pub fn should_draw_focus_ring(has_focus: bool, reason: FocusReason) -> bool;
```

需要给事件层加 `FocusReason`（`Pointer` / `Tab` / `Backtab` / `Shortcut` / `Programmatic`）。

### 6.4 P0-6 / P0-7 按钮交互契约

按 QML 的 `handlePress/Move/Release/Ungrab` 四段改写：

| 事件 | 结果 |
|---|---|
| press | `pressed = true`，发 `pressed()`（已按下则不发） |
| move | `pressed = contains(point)` |
| release（内含） | `pressed=false`，发 `released()` + `clicked()` |
| release（外） | `pressed=false`，发 `released()` + **`canceled()`**，**不发 `clicked()`** |
| ungrab | 清定时器，`pressed=false`，发 `canceled()` |

### 6.5 P0-10 `Colors` 扩面 + `Font` 补字段

```rust
// 新增（全部带默认值，保证反序列化向前兼容）
pub outline: Color,            // 分隔线；与 focus ring 区分（修 BLUE21 §七 B「同色」）
pub outline_variant: Color,    // 更弱的次级分隔
pub scrim: Color,              // 遮罩（修 B23「遮罩照亮暗底」）
pub surface_container: Color,  // 卡片/面板
pub surface_container_high: Color,
pub inverse_surface: Color,
pub on_inverse_surface: Color,
```

`Font` 增 `letter_spacing: f32` 与 `line_height: f32`（2× 缩放的**前置**）。

### 6.6 P1-2 RTL 铺开

`TextDirection` 已存在，`slider` 已接。其余按「方向敏感」逐个接：

| 控件 | 需要改什么 |
|---|---|
| `progress_bar` | 填充从哪端起（`ProgressBar.qml:20` 用 `scale: -1`） |
| `range_slider` | 两个手柄的取值/绘制方向 |
| `tab_bar` | tab 顺序与「溢出」方向 |
| `app_bar` | leading/trailing 交换 |
| `scroll_bar` | 条的位置（`ScrollView.qml:19`） |
| `menu` 方向键 | `Menu.qml` 的 `isMirrored() == (key == Right)` |

### 6.7 门禁（与修复**同批**落地，否则下一轮重新引入）

| 门禁 | 拦什么 | 反向注入 |
|---|---|---|
| `check_implicit_size_uses_metrics` | 控件自己算绝对尺寸而不走 `ControlMetrics` | 必做 |
| `check_spacing_is_not_sibling_layout` | `spacing` 被用于兄弟间距 | 必做 |
| `check_click_requires_release_inside` | 按下即发 `clicked` | 必做 |
| `check_transition_durations_are_tokens` | 硬编码时长（除 token 定义处） | 必做 |
| `check_focus_ring_respects_reason` | 无条件画焦点环 | 必做 |
| **复核已有** `check_svg_snapshots`（188） | 快照必须仍可复现 | — |

---

## 6B. 组合控件：如何由基本控件用 `layout` + `create` 组装（**用户特别要求，与 §6 并列**）

> **这是本计划最欠账的一块。** 前 6 节都在讲「单个控件怎么画对」，但一个工具箱的
> 可用性主要由**组合控件**决定（表单、对话框、工具条、树、分栏）。本节先说清本仓**已有**
> 的机械，再指出**缺口**，最后给出可直接施工的组装规范。

### B.1 本仓已有的三块机械（实跑取证）

| # | 机械 | 位置 | 现状 |
|---|---|---|---|
| 1 | `Layout` trait（`add_widget`/`remove_widget`/`update(&self, rect, cb)`） | `src/layout/types.rs:101-116` | ✅ 已实现 15 种布局 |
| 2 | 声明式布局存储 API（`store_layout`/`add_widget_to_layout`/`apply_layout`/`preview_layout`） | `src/layout/declarative.rs:65-205` | ✅ **完整实现，但消费者只有 JSON 加载** |
| 3 | 控件工厂（`WidgetFactory::create(kind_or_name, geometry, text)`） | `src/widget/capability.rs:582-601` | ✅ 180 个 kind 可创建 |
| 4 | 父/子关系（`BaseWidget::add_child`/`children`/`set_parent`） | `src/widget/base.rs:194-230` | ✅ 有，但**不自动同步反向链接** |
| 5 | 固有尺寸（`Widget::size_hint`） | `src/widget/widget_trait.rs:634` | ⚠️ **176 个控件实现了，但只有 `flow.rs` 一个布局读它** |

### B.2 缺口一：组合控件**没有用**布局，而是手算几何

```text
$ grep -rln "BoxLayout::new|FlexLayout::new|GridLayout::new" src/ --include=*.rs
src/json/layout.rs      ← JSON 路径
src/app/handle.rs       ← 窗口/面板路径
src/layout/*.rs         ← 布局自身

（src/widget/ 下一个都没有）

$ grep -rn "store_layout|add_widget_to_layout|apply_layout" src/ examples/ | grep -v declarative.rs
src/json/loader.rs:32,265,299,306,437,471,479   ← 唯一消费者
```

**结论：组合控件（`stepper`/`split_button`/`array`/`dialog`/...）是从 `self.geometry()` 手算
每个子控件的位置，而不是把子控件交给一个 `Layout`。** 这带来三个具体后果：

1. **`size_hint` 无人消费**（176 个实现，1 个消费者）——「内容决定尺寸」这条链是断的。
2. 组合控件无法被**父布局**当作一个整体测量：父布局只能拿到它的 `rect`，拿不到它的固有尺寸。
3. 同一套「图标 + 间距 + 文字」的排布在每个组合控件里**各写一遍**，这正是 BLUE21 §七
   反复出现的「同一事实多处推导」形态的根源。

### B.3 缺口二（**真正的那条**）：`Layout::update` 拿不到子控件的尺寸

```rust
// src/layout/types.rs:107
fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
```

`Layout` 只持 `ObjectId`，**没有 `&dyn Widget`**，所以它无法问「这个子控件想要多大」。

**而本仓的应急办法把它坐实了**——布局只能**要求调用方提前把尺寸塞给自己**：

```rust
// src/layout/flex.rs:116-167
/// Size hints indexed by position within items (set before update).
pub fn set_child_sizes(&mut self, sizes: Vec<Size>);   // 注释：“call before update for proper sizing”
```

同一形态还有 `wrap.rs:106`（`set the size hint for a child widget (call before update)`）
与 `absolute.rs:320`（从自己存的字段读 `w.size_hint()`）。

**这就是「176 个实现、1 个消费者」的真正原因**：不是「没人用」，而是
**布局无法问控件**，所以只能反过来让调用方先告诉布局。于是：

- `flow.rs` 存了 `Box<dyn Widget>` 才能读 `size_hint` ⇒ **它成了那个唯一的「消费者」**；
- 其余 14 种布局各用各的传递方式（参数、缓存、setter）⇒ **同一事实三套存储**。

> **对标印证**：
> - QML：`implicitWidth = max(implicitBackgroundWidth + …, implicitContentWidth + …)`
>   （`qquickcontrol.cpp:1754`）——**布局在协议层就能拿到内容尺寸**。
> - Flutter：`child.layout(constraints, parentUsesSize: true)` 后读 `child.size`
>   （`layout_helper.dart:64-66`）——父向子要。
> - Qt：`item->implicitWidth()` 直接问（`qquickdialogbuttonbox.cpp:349-366`）。
>
> **三家都是「问子控件」，只有本仓是「告知布局」。**

### B.4 缺口三：`create` 与 `layout` 之间没有「声明 → 组装」的桥

JSON 路径是完整的（`loader.rs` 里 `store_layout` → `add_widget_to_layout` → `apply_layout` 三段齐全），
但**代码路径没有**等价的桥：一个组合控件想声明「我要一个横排、两个子控件、8px 间距」时，
没有 API 把 `WidgetFactory::create` 的产物 + `Layout` 组装起来。

**重要**：这个缺口是**结果**而不是原因。B.3 的通道一旦接上，这个桥只需 ~100 行「创建 + 收集提示
+ 问布局 + 回写」。反过来，如果只修 B.4 而不修 B.3，就会得到一个**把「手算尺寸」从控件
搬进构建器**的东西——缺陷位置变了，性质没变。

### B.4.1 缺口的优先级重排（对标后的修正）

| 缺口 | 是不是根因 | 为什么 |
|---|---|---|
| **B.3 尺寸通道** | ✅ **是根因** | 不接上，任何「组合」方案都只能靠猜尺寸 |
| **B.5 `size_hint` 形状不足** | ✅ **是根因** | 一值提示无法表达地板（`min`）与拉伸上限（`max`） |
| B.4 组装桥 | ⚠️ 是结果 | 通道接上后很轻 |
| B.2 组合控件手算几何 | ⚠️ 是症状 | 前两条修好后的自然收敛 |

### B.5 对标结论：**我先前给的 `CompositeBuilder` 方案是错的**

本节初稿提了一个「`CompositeBuilder` 持有 `Box<dyn Widget>` 子控件 + 一个 `Layout`」的方案。
**读完 Flutter 与 Qt 的布局协议后，该方案被否决**，因为它**解决了错的问题**：

> 组合控件的真正缺口不是「缺一个容器」，而是**缺一条从子控件到父布局的尺寸通道**。

**证据（三家一致）**：

| 事实 | Flutter | Qt | 本仓 |
|---|---|---|---|
| 尺寸提示的**形状** | 4 个函数，**每个都带对轴参数**：`getMinIntrinsicWidth(double height)`（`box.dart:1651`） | 每轴 **3 个值** min/pref/max（`qquicklayout.cpp:1176` 的真值表） | `size_hint() -> Size`，**无参数**（`widget_trait.rs:634`） |
| 布局**向谁**要尺寸 | 向子 render object 要（`child.layout(constraints, parentUsesSize: true)`） | 向子 item 要（`item->implicitWidth()`） | **向自己存的变量要** |
| 本仓的实证 | — | — | `FlexLayout::set_child_sizes(Vec<Size>)`（`flex.rs:167` 注释：「call **before** update」） |

**「布局自己存尺寸」是本仓那条断链的真正形状**：

```rust
// src/layout/flex.rs:116
/// Size hints indexed by position within items (set before update).
// 调用方必须先算好尺寸再交给布局 ⇒ 布局永远无法向控件要尺寸
```

⊦ **所以先前的 `CompositeBuilder` 即使写出来，也只能把「手算」从控件搬到构建器，
每个子控件的尺寸仍由调用者猜。** 这是「把缺陷移动位置」而不是修复。

### B.5.1 两家对「尺寸提示不足」的分歧与共识

两边都指出本仓 `size_hint() -> Size` 不够用，但**提出的替代不同**——这一点必须说清，
因为它是本计划最大的一个设计决定：

| | Flutter | Qt |
|---|---|---|
| 形状 | 4 个**对轴参数化**的函数（min/max × w/h） | 每轴 **3 值**：`min / pref / max` |
| 表达「可以缩但不小于 X」 | 靠 `ConstrainedBox` 包一层 | 直接由 `min` 表达 |
| 表达「按窗口拉长但我保持厚度」 | `Expanded` + `CrossAxisAlignment.stretch` | `Layout.fillWidth: true` + `Fixed` 的另一轴 |
| 代价 | O(N²) 风险；Flutter 自己的文档说 `IntrinsicWidth` 是「O(N²) in the depth of the tree」（`basic.dart:3804-3807`） | 需要 `Layout.*` 附着属性与 invalidate 机制 |

**本仓应选 Qt 的形态（每轴 min/pref/max）**，理由：

1. **它直接修好我们的缺陷**：缺的正是 `min`（地板）与 `max`。Flutter 的 4 函数仍无法表达地板，
   必须靠额外包一层——而我们的组合控件**就是要那个地板**（§2 全表都是地板）。
2. **无 O(N²) 风险**：min/pref/max 是**每个控件自己算出的定值**，不需要「给定对轴再重算」的往返。
3. **`spacing`/`padding` 体系已存在**，与 min/pref/max 天然契合。

### B.5.2 修正后的施工方案（真正的 P0）

**第一步：把 `size_hint` 换成三值提示（`Hints`）**，并让**所有**布局消费它。

```rust
// src/layout/hints.rs （新建）

/// 一个控件在某一方向上的尺寸诉求。
///
/// # 为什么是三个值而不是一个
///
/// `size_hint() -> Size` 只能回答「我想要多大」，无法回答两个组合控件**每天都要问**的问题：
///
/// - 「最多能把我压到多小？」→ `min`。按钮/开关/进度条都有一条**地板**。
/// - 「最多能把我拉到多大？」→ `max`。图标、徽标不该被无限拉伸。
///
/// 本仓当前用「背景隐式尺寸」当唯一的地板，而它只影响**自己**——无法向上传播。
/// Qt 的真值表（`qquicklayout.cpp:1176`）把这三个值放在一起，正是因为它们必须一起决胜：
///
/// ```text
///             | minimum              | preferred              | maximum
/// USER        | Layout.minimumWidth  | Layout.preferredWidth  | Layout.maximumWidth
/// HINT        | implicit min         | implicitWidth          | implicit max
/// FALLBACK    | 0                    | width                  | +infinity
/// ```
///
/// 不变式：`min <= pref <= max`（Qt 的 `normalizeHints`，`qquicklayout.cpp:1218`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisHints {
    pub min: u32,
    pub pref: u32,
    pub max: u32,
}

impl AxisHints {
    /// 一个新的提示，自动满足 `min <= pref <= max`。
    ///
    /// **构造时即归一化**，而不是靠调用方记得：Qt 因为允许未归一化的值，
    /// 不得不在每次读取前跑一遍 `normalizeHints`/`expandSize`/`boundSize`（`qquicklayout.cpp:1218-1288`）。
    /// 让非法状态**不可表示**比事后修正便宜。
    pub fn new(min: u32, pref: u32, max: u32) -> Self;
    /// 固定尺寸（min == pref == max）。
    pub fn fixed(size: u32) -> Self;
    /// 只要求地板（min = 地板, pref = max = 不限）。
    pub fn at_least(min: u32) -> Self;
}

/// 两轴提示（组件的固有尺寸诉求）。
#[derive(Debug, Clone, Copy, Default)]
pub struct Hints {
    pub width: AxisHints,
    pub height: AxisHints,
}

/// 孩子希望怎样被父布局对待。
///
/// # 为什么「拉伸」不能从提示里推出来
///
/// 「我想变大」与「父布局该把我拉长」**是两件事**：一个滑块的 `pref` 是固定的，
/// 但它**应该**被拉宽；一个按钮的 `pref` 也固定，却**不该**被拉宽。
/// 两者无法从尺寸区分，所以必须单独一个位。Qt 的 `Layout.fillWidth`（`qquicklayout.cpp:303`）
/// 就是这一位，且它的默认值来自控件自带的 policy（`GrowFlag`，`qquicklayout_p.h:221`）——
/// 控件声明自己的默认，使用者可覆写。
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutParams {
    /// 拉满主轴剩余空间（Qt `Layout.fillWidth/fillHeight`）。
    pub fill: bool,
    /// 主轴分配权重（本仓已有的 `stretch`，与 Qt 的 `stretchFactor` 同义）。
    pub stretch: u32,
    /// 单边外边距（在布局计算前从可用空间里扣除）。
    pub margins: EdgeInsets,
}

/// 控件声明自己的尺寸诉求。
///
/// # 为什么不是 Flutter 的 4 个对轴函数
///
/// Flutter 的 `getMinIntrinsicWidth(height)` 要给定对轴才能回答，因此它的 `IntrinsicWidth`
/// 是「a speculative layout pass」且自认「O(N²) in the depth of the tree」（`basic.dart:3804-3807`）。
/// min/pref/max 是**控件自己算出的定值**，布局直接读，无往返、无 O(N²)。
/// 代价是它不能表达「宽度依赖高度」的强耦合——本仓的控件几乎全是定高的（按钮/开关/输入框），
/// 真正需要宽高联动的是自动换行的文本，而文本有独立的测量入口。
///
/// 默认返回全 0（等同旧 `size_hint() -> Size::ZERO`），所以现有 176 个实现不会因此报错，
/// 可逐个迁移（与 BLUE21 「先立原语再机械替换」同一手法）。
trait Widget {
    fn hints(&self) -> Hints {
        Hints::default()
    }
    /// 控件自带的默认布局策略（Qt 的 `sizePolicy`，`qquickitem_p.h:804`）。
    fn default_layout_params(&self) -> LayoutParams {
        LayoutParams::default()
    }
}
```

**第二步：让 `Layout` 能读到子控件的提示——这是最关键的一行**

```rust
// src/layout/types.rs —— 改签名：从「只写」变成「可读写」

/// 布局所需的子控件信息。
///
/// # 为什么必须有这个参数
///
/// 旧签名 `update(&self, rect, &mut dyn FnMut(ObjectId, Rect))` 只能**写**子控件的位置，
/// 无法**读**子控件想要多大。于是每个布局只能要求调用方提前把尺寸塞进自己：
/// `FlexLayout::set_child_sizes(Vec<Size>)`（`flex.rs:167`）就是这么来的，
/// 它的注释写着“set **before** update”——布局被迫在做布局前先被人告知答案。
///
/// 后果是 `Widget::hints()` 有 176 个实现却只有 1 个消费者。
/// 这个参数把那条通道接上：布局**问**子控件，而不是**被告知**。
pub struct ChildInfo {
    pub id: ObjectId,
    /// 子控件自报的尺寸诉求（`Widget::hints()`）。
    pub hints: Hints,
    /// 该子控件在当前父布局中的参数（`fill`/`stretch`/`margins`）。
    pub params: LayoutParams,
}

pub trait Layout {
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    fn remove_widget(&mut self, widget_id: ObjectId);

    /// 排布子控件。`children` 给出每个子控件的尺寸诉求，`out` 回写几何。
    ///
    /// 默认实现忽略提示，转发到旧的 `update`，让 15 个现有布局**不必同时改完**。
    fn arrange(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        out: &mut dyn FnMut(ObjectId, Rect),
    ) {
        // 退化：无提示信息时仍能工作。
        self.update(rect, out);
    }

    /// 只写几何（旧路径，保留以兼容）。
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
}
```

**第三步：组合控件用工厂 + 布局组装（`CompositeBuilder` 仍有用，但角色降级）**

读者注意：`CompositeBuilder` **仍然要做**，但它的价值不再是「提供布局」，而是
**把「创建 + 收集提示 + 问布局 + 回写」这四步固定成一条链**，避免每个组合控件各写一遍：

```rust
// src/widget/composite.rs

/// 由基本控件 + 布局组装组合控件。
///
/// # 它只做四件事，且都不含布局算法
///
/// 1. 持子控件（`Box<dyn Widget>`，因此能读 `hints()`）
/// 2. 按 `Hints + LayoutParams + padding + floor` 计算自己的 `Hints`（= QML 的 `implicitWidth` 公式）
/// 3. 把子控件的 `hints()` 收成 `ChildInfo` 交给 `Layout::arrange`
/// 4. 把 `arrange` 回写的几何应用到子控件
///
/// **排布算法完全在 `src/layout/` 里，此处一行都没有**（§B.10 风险 2）。
pub struct CompositeBuilder {
    layout: Box<dyn Layout>,
    children: Vec<ChildEntry>,
    padding: EdgeInsets,
    /// 背景/触控地板：`implicit_size` 的 `max` 另一臂（QML `implicitBackgroundWidth`）。
    floor: Size,
}
```

> **实控证据**：Qt 的 `QQuickControl` 用**同一套**公式算控件自身的固有尺寸：
> ```
> implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
>                         implicitContentWidth + leftPadding + rightPadding)
> ```
> （`qquickcontrol.cpp:1754`）且**订阅子控件的 implicit-size 通知**（`addImplicitSizeListener`，
> `qquickcontrol.cpp:390`）——尺寸变化会主动回调，而不是靠轮询。
> 这就是我们这个 `CompositeBuilder` 要做的事。

### B.6 组装规范（**每个组合控件都必须遵守**）

| # | 规则 | 为什么 | 判据 |
|---|---|---|---|
| 1 | **子控件必须由 `WidgetFactory::create` 创建**，不得 `new` 具体类型 | 否则 `mini/embedded` 裁剪 profile 下该类型可能不存在（本仓已有 `TOGGLE_BUTTON_KIND` 这类替身机制） | 门禁：组合控件内不得出现 `Xxx::new(` |
| 2 | **子控件位置必须由 `Layout` 计算**，不得 `rect.x + k` 手算 | 手算会让 `layout_scale`/`font_scale` 失校 | 门禁：不得在 `draw`/`arrange` 里出现魔法偏移 |
| 3 | **固有尺寸必须向上传播**（`Hints`） | 父布局要能测量它；否则「内容决定尺寸」链断 | 单测：子控件变大 ⇒ 组合控件变大 |
| 4 | **`padding` 与 `spacing` 分开**：padding 管边到内容，spacing 管相邻元素 | QML 全场如此（`CheckBox.qml:61`、`ComboBox.qml:21`）；混用会漂 | 沿用 §6.7 的 `spacing` 门禁 |
| 5 | **父子链接双向同步** | `set_parent` 的文档明说「不更新旧的/新的父的 child 列表」（`base.rs:189-192`），必须显式两边都写 | 单测：`children()` 与 `parent()` 一致 |
| 6 | **子控件 id 不得泄漏进公开 API** | 组合控件的使用者不应知道内部结构 | 接口只暴露语义属性 |
| 7 | **触控地板由布局落实**（`grow_to_min_touch_size`） | 已存在（第 65 轮），组合控件须让子控件走 `update_with_context` | 单测：4px 高的子控件得到 48px 命中区 |
| 8 | **`mini/embedded` 下不得依赖 `Vec` 无界增长** | `BaseWidget::children` 在 `alloc_frugal` 下是 `heapless::Vec<_, 64>`（`base.rs:212-216`） | 编译 + 测试两个 profile |
| 9 | **地板（`min`）与「拉伸」（`fill`）必须分开声明** | Qt 明确区分 `Layout.minimumWidth` 与 `Layout.fillWidth`（`qquicklayout.cpp:303`）：前者是尺寸诉求，后者是父布局该不该拉长我 | 单测：两个同 `pref` 的控件，一个 `fill` 一个不，拉伸行为必须不同 |
| 10 | **提示变化必须能触发重排**（不是轮询） | Qt 用 `invalidate()` + 延迟的 `updatePolish()`（`qquicklayout.cpp:857`）避免每次改提示就重排；本仓只需要一个「脏」标记 + 宿主在帧末消费 | 单测：子控件 hint 变大 ⇒ 下一次 `arrange` 得到更大矩形 |

### B.6.1 明确**不抄**的（Flutter / Qt 都过于重）

| 不抄 | 出处 | 为什么 |
|---|---|---|
| 约束下发/尺寸上传（`BoxConstraints`） | `box.dart:164-176` | 它解决的是「父可以压缩子」，而我们的 `min` 已经表达了地板；全套约束会连同 `parentUsesSize`、relayout boundary、`sizedByParent` 一起进来（~3500 行） |
| `shouldRelayout` / `polish()` 延迟管线 | `custom_layout.dart:297`、`qquickitem.cpp:4686` | 本仓已有帧循环与 `request_repaint`；再接一套延迟通道只会多一条失效路径 |
| `Layout.fillWidth` 之类**附着属性** | `qquicklayout_p.h:160` | Qt 需要它是因为 QML 无法给已有 item 添字段；Rust 直接把 `LayoutParams` 放进构建器的 `.child(.., params)` |
| 全套 `QLayoutPolicy` 位标志 | `qquicklayout_p.h:228` | 整棵树里**只用到 `GrowFlag` 一个位** ⇒ 我们用一个 `bool fill` 即可 |
| 锚点（anchors）约束系统 | `qquickanchors_p.h:59` | 它会与布局互相冲突（Qt 自己必须检测并警告：`quicklayout.cpp:891`）；我们只提供 `align_in(rect, alignment)` 一次性对齐 |
| Qt 的 `-1`/`+inf`/`NaN` 三种「未设」惯例 | `qquicklayout.cpp:64` | 那是 `Option<T>` 被实现了九遍；本仓用 `AxisHints` 的归一化构造器一次性解决 |

### B.7 按上述规范做的三个样板（先做这三个，验证规范）

> **注意**：这三个样板必须在 **§B.5.2 前两步（`Hints` + `arrange`）做完之后**才做，
> 否则会重复「手算尺寸」的老路。

| 样板 | 组成 | 验证什么 |
|---|---|---|
| `button_with_icon` | `icon` + `label`，横排，`spacing=6`，地板 `64×40` | `Hints` 能否向上传播（最小样板） |
| `spin_box` | `line_edit` + `up` + `down`（按钮宽度驱动左侧 padding） | `min` 地板 + `fill` 与 `min` 分开声明（对齐 Qt `SpinBox.qml:20-21`） |
| `dialog_with_actions` | 标题 + 内容 + 右对齐按钮行 | 嵌套组合（组合里嵌组合）+ 触控地板 |

**为什么按这个顺序**：`button_with_icon` 只有两个子控件，能把机制本身验证干净；
一旦它对了，其余组合控件就是同一套写法的重复——这正是 BLUE21 「抽原语 vs 76 次返工」的同一教训。

### B.8 组合控件清单（按 §B.6 规范逐个改造）

| 组合控件 | 现状 | 应改为什么 | 优先级 |
|---|---|---|---|
| `split_button` | 手算「主体 + 箭头」 | `HBox` + `floor(64×40)` | **P0** |
| `spin_box` / `number_picker` | 手算上/下按钮 | `HBox`/`VBox` + `spacing=0` | **P0** |
| `combo_box` | 手算指示器 padding | `HBox` + `left/rightPadding` 按方向 | **P0** |
| `dialog` / `message_box` | 手算标题/内容/按钮行 | `VBox` + 按钮行 `HBox` + 右对齐 | **P0** |
| `group_box` | 手算标题占位 | `VBox` + `topPadding = padding + label_h + spacing`（`GroupBox.qml:20`） | **P1** |
| `tool_bar` | 手算（BLUE21 B 记「6 处字面量」） | `HBox` + `spacing=6` | **P1** |
| `menu` / `menu_item` | 手算 check/arrow 两段 padding | `leftPadding = padding + indicator + spacing`（`MenuItem.qml:25-28`） | **P1** |
| `tab_widget` | 手算 tab 宽 | `HBox` + `spacing=1`（`TabBar.qml:16`） | **P1** |
| `status_bar` | 手算多段 | `HBox` + 段间 `spacing` | **P1** |
| `scroll_area` | 手算条的位置 | `Stack` + `Absolute`（条叠在内容上） | **P2** |
| `list_view` / `grid_table` | 手算行高 | `UniformGridLayout` 已有 | **P2** |

### B.9 关键洞察：组合控件的「美观」来自**同一套 padding/spacing 推导**

对标 QML 后最值得抄的一条：

```qml
// SpinBox.qml:20-21 —— 子控件的位置**由兄弟宽度推导**，不是写死的
leftPadding:  padding + (mirrored ? up.width : down.width)
rightPadding: padding + (mirrored ? down.width : up.width)
```

**本仓的组合控件恰恰相反**：子控件的 padding 是写死的数字。当按钮变宽（或字变大）时，
文本区不会让位，于是重叠或溢出。

> **因此 §6B 的真正价值不是「少写代码」，而是让组合控件的每一段留白都由**
> **兄弟的实际尺寸推导**——这才是「外形美观」在组合控件层面的定义。

### B.10 判据与风险

**判据**：

```text
--- 根因类（P0-1b）---
1. grep -rn "set_child_sizes|set_size_hint" src/layout/            → 归零（布局不再要求调用方预存尺寸）
2. 单测：一个不知道尺寸的布局能仅凭 &[ChildInfo] 完成排布（即 flex/grid 不再需要预存）
3. 单测：`AxisHints::new(30, 10, 5)` 自动归一化为 min<=pref<=max
4. 单测：两个同 pref 的子控件，`fill: true` 的被拉长、`fill: false` 的不变

--- 组装类（P0-1c）---
5. grep -rn "BoxLayout::new|FlexLayout::new|GridLayout::new" src/widget/   → 非空
6. grep -rn "XxxWidget::new(" src/widget/container_widgets/ | wc -l         → 0（全走工厂）
7. 单测：子控件 hints 变大 ⇒ 组合控件 implicit_size 变大（固有尺寸传播）
8. 单测：`children()` 与每个孩子的 `parent()` 一致
9. 单测：mini + embedded 两个 profile 均可编译且组合控件可创建
10. 三个样板控件各自的快照 diff 可被人眼判为「留白一致」
```

**风险**：

1. **`Layout::update(&self)` 是 `&self`**，所以 `arrange` 不能在布局内部改控件——
   必须保持「布局只算、调用方写」的两段式（`apply_layout` 已如此，`declarative.rs:171-182`）。
2. **不要让 `CompositeBuilder` 变成第二套布局系统**。它只做「持有 + 测量 + 转发」，
   实际排布**必须**委托给 `src/layout/` 里已有的 15 种实现之一。
3. **提示传播不得递归**：组合控件的 `hints()` 只沿**直接子控件**取，不得递归到孙控件
   （否则树深时是 O(深度) 且可能环）。这是 Qt 用「允许循环、检测后中止」换来的教训——
   它为了 height-for-width 不得不把 polish 循环封顶在 2 次（`qquicklayout.cpp:866`）。
   **我们不支持宽高联动，就没有这个环的风险**（§B.5.1 选 Qt 形态而非 Flutter 四函数的又一个好处）。
4. **不要删掉手算路径就宣布完成**：先并存（新写法 + 旧写法），等三个样板验证过、
   快照评审通过后再逐个迁移，避免 BLUE21 「76 处机械迁移」那种一次性风险。
5. **`arrange` 的默认实现必须转发旧 `update`**，否则 15 个布局要同时改完——
   那正是 BLUE21 反复证明会翻车的做法。

---

## 7. 与 BLUE21 遗留项的对应关系（原样保留，加优先级）

| BLUE21 条目 | 本计划编号 | 状态 |
|---|---|---|
| P0-2 触控区 | 已部分（`touch_target` 已接） | 保留收口 |
| P0-3 状态化主题 | **P0-8** | 未做 |
| P0-4 动画管线 | 已部分（`Motion` + `tick`） | **P2-2** 铺开 |
| P2-1 尺寸钉死 11 控件 | **P0-2** | 升为 P0（是美观的地基） |
| P2-2 字面量复制 | **P0-4 / P1-1** | 未做 |
| P2-3 光标闪烁 | ✅ **已完成** | — |
| P2-4 组合结构 9 处 | **P2-4** | 未做 |
| P2-5 尺寸偏小 | **P0-2** | 合并 |
| P2-6 主题 token 扩面 | **P0-10** | 升为 P0（是「美观」的最短路径） |
| P2-7 `font_scale`/`min_touch` | 已部分 | **P1-1** |
| P2-8 a11y | 结构 ✅；**P1-3** 铺开填充 | 部分 |
| P2-9 契约过薄 | **P1-4** | 未做 |
| P2-10 RTL | 类型 ✅；**P1-2** 铺开 | 部分 |
| P2-11 `stepper` 命名 | **P1-6** | 未做（需裁定） |
| P2-12 C5/C6/C8 | ✅ **已完成** | — |
| E6 分组原语 | **P1-5** | 未做 |
| P4 声明式原语 | **P2-4**（另立） | 未做 |

---

## 8. 验收判据（全计划共用）

```text
1. cargo test --no-default-features --features desktop          → 0 failed
2. cargo clippy --no-default-features --features desktop --all-targets -- -D warnings → 0 warning
3. for p in desktop tablet mobile mini embedded; do cargo check --no-default-features --features $p; done → 0 error
4. cargo run --no-default-features --features desktop --example export_control_svgs
5. bash tools/check_svg_snapshots.sh        → checked=188 failed=0
6. bash tools/check_control_rendering.sh    → checked=188 failed=0
7. 新增门禁逐条「反向注入 ⇒ 变红」（§6.7）
8. 每个 P0 条目的快照 diff 必须能被人眼判为「更小更居中」
--- 组合控件专项（§6B）---
9. grep -rn "BoxLayout::new|FlexLayout::new|GridLayout::new" src/widget/  → 非空
10. grep -rn "Widget::new(" src/widget/container_widgets/              → 0（全走工厂）
11. 单测：子控件 size_hint 变大 ⇒ 组合控件 implicit_size 变大
12. 单测：组合控件的 children() 与每个子的 parent() 一致（双向链接）
13. mini + embedded 两个 profile 下，三个样板组合控件均可创建且不 panic
```

**第 8 条是本计划区别于 BLUE21 的地方**：BLUE21 的判据是「不报错」，本计划的判据包含
**「人眼看快照 diff 认为变好了」**——因为目标是「外形美观」，这必须由人（或至少由 diff）来判。

---

## 9. 风险与克制

1. **不要一次改完 188 个控件**。先 P0-1 立体系 + 用 3 个控件（`switch`/`radio`/`progress_bar`）验证，
   再铺开。BLUE21 的教训是「76 处机械迁移」只有在原语立好后才安全。
2. **快照会大面积变化**。这是**预期**的，不是回归；但每次只改一类控件，diff 才可评审。
3. **`Colors` 加字段是破坏性变更**（结构体非 `#[non_exhaustive]`）。全部新字段必须带默认值，
   且 `#[cfg_attr(not(alloc_frugal), serde(default))]`，保证 `mini/embedded` 与旧 JSON 都能加载。
4. **RTL 只做到「方向敏感控件」层**。不做全量 `LayoutMirroring`（见 §5.1）。
5. **不引入新依赖**。所有机制都是「几行代码 + 常量」，QML 的 Basic 风格本身就是纯 QML 无图片实现，
   说明这套东西不需要额外依赖。

---

# 附录 F — 执行进度与未完成项（2026-09-23 第 67 轮实跑取证）

> 执行记录全文见 [`../log/log-20260923-1.md`](../log/log-20260923-1.md)。
> 本节只记「已完成 / 未完成」两个清单，未完成项按优先级排序，供后续轮次直接接续。

## F.1 已完成（有独立判据，勿重复劳动）

| 计划条目 | 状态 | 判据 |
|---|---|---|
| **P0-1 度量体系** | ✅ | 新增 `src/widget/metrics.rs`（`ControlMetrics` + `dimensions` 常量表）；单测断言「5px 文字 ⇒ `64×40`」 |
| **P0-1b 尺寸通道** | ✅ | 新增 `src/layout/hints.rs`（`AxisHints`/`Hints`/`LayoutParams`/`ChildInfo`）；`Layout::arrange` 默认转发 `update`；`FlexLayout::arrange` 仅凭 hint 排布且与旧路径**几何逐字节相同** |
| **P0-1c 组合装配（部分）** | ✅ | `LayoutParams`（`fill` 与 `min` 分离）；`painted_box` 在 8 个对话框落地；`spacing` 语义分离 |
| **P0-2 绘制盒（11 控件 + 全仓）** | ✅ | 174 个控件的绘制盒由 `ControlMetrics` 推导；快照逐个人眼可判「更小更居中」 |
| **P0-3 四层 padding 级联** | ✅ | `PaddingSpec`（side→axis→uniform）；单测「写 `left` 只改左」 |
| **P0-4 `spacing` 语义** | ✅ | `WidgetStyle::spacing`；checkbox/radio 的 indicator↔label 间距读它，**兄弟间距仍归布局** |
| **P0-5 `visual_focus`** | ✅ | `FocusReason` 随 `FocusGained` 载荷传递；`draws_focus_ring()` 对 `Pointer` 返回 false |
| **P0-6 / P0-7 点击契约** | ✅ | 四段契约 + `canceled` + `grabbed`（`pressed` 与「手势归属」分离）；拖出再拖回可恢复 |
| **P0-8 状态独立 + 过渡原语** | ✅ | 新增 `style::Transition` + `TransitionTempo`；switch 的 `visualPosition`（`travel`）与逻辑值分离 |
| **P0-9 幂等 setter** | ✅ | checkbox/radio/switch 均已有「同值不发信号」守卫 |
| **P0-10 `Colors` 扩面** | ✅ | +7 角色（全部带 serde 默认值）+ `impl Default for Colors`；两个预设改用 `..Colors::default()` |
| **P2-2 动效 token 化** | ✅ | `floating_label` 的硬编码 150ms 改为 `theme.motion`；新门禁 `check_transition_durations_are_tokens` |
| **§6.7 门禁（2/5）** | ✅ | `check_click_requires_release_inside`（找到 `fab` + 2 处登记缺陷）+ `check_transition_durations_are_tokens`（找到 `floating_label`）；均经反向注入证明可失败 |
| **版本与文档** | ✅ | 2.6.1；CHANGELOG 双份逐字节一致；两个 README 新增 3 节；cookbook 三语 48 处 pin 同步；新增 `examples/readme_check.rs` 使 README 代码块**被编译** |

**量化结果**：

| 指标 | 前 | 后 |
|---|---|---|
| 零尺寸元素（不可见） | 3 | **0** |
| 空 `<text>` 元素 | 6 | **0** |
| 把 chrome 拉伸到全画布的控件 | ~65 | **0** |
| 文本贴顶（y≈0，应为 53） | 16+ | **0** |
| 编译失败的 profile | 2（mini/embedded） | **0** |
| 全量测试 | 5386 | **5513** |
| 门禁 | 56 | **58**（`PASS=58 FAIL=0 TIMEOUT=0 NOT-RUN=0 SKIP=1`） |

## F.2 未完成项（按优先级）

### F.2.1 P0 级（计划的核心目标，尚未铺开）

| # | 条目 | 计划出处 | 现状取证 | 施工要点 |
|---|---|---|---|---|
| **F-1** | **§B.8 组合控件逐个改造（11 个）** | §B.8 | `grep -rl "BoxLayout::new\|FlexLayout::new\|GridLayout::new" src/widget/` → **0 命中**，即组合控件**仍无一使用真实布局**；它们已改用共享几何 helper（这解决了「外观」），但「由布局组装」这层未接 | 见 F.2.2 |
| **F-2** | **§B.7 三个样板控件** | §B.7 | 未做 | `button_with_icon` / `spin_box` / `dialog_with_actions`：先各写一个，验证 §B.6 的 10 条规范；**`spin_box` 起手**，因为其 `leftPadding = padding + up.width`（`SpinBox.qml:20-21`）是本计划最有价值的一条洞察的载体 |
| **F-3** | **§B.6 规则 10「提示变化主动通知布局」** | §B.6-10 | 未做 | 需要「脏」标记 + 宿主帧末消费；`arrange` 已可读到 hint，但**内容变了尺寸不会重排** |

### F.2.2 F-1 的施工细则（后续轮次可直接照做）

计划 §B.8 的 11 个控件，按**性价比排序**（先做「兄弟尺寸驱动」收益最大且最易验证的）：

| 优先 | 控件 | 现状缺陷 | 目标推导（§B.9） |
|---|---|---|---|
| 1 | `spin_box` / `number_picker` | 值文本与 +/− 按钮列各自写死（`spinbox.rs:123` 的注释自己记录了「值画在 x=4，按钮列从 x=200 起」） | `text_area = content_box − button_column_width`，即**文本区让位于按钮列**（`SpinBox.qml:20-21`） |
| 2 | `split_button` | 主面 + 箭头手算 | `HBox` + `floor(64×40)`；箭头列宽由箭头自身尺寸推导 |
| 3 | `combo_box` 族（`combobox`/`editable_combo_box`/`multi_select_combo_box`/`font_combo_box`） | 指示器 padding 写死 | `trailing_padding = padding + indicator_width + spacing`（镜像时交换） |
| 4 | `group_box` | 标题占位手算 | `top_padding = padding + label_height + spacing`（`GroupBox.qml:20`） |
| 5 | `menu` / `menu_item` | check/arrow 两段 padding 写死 | `left_padding = padding + indicator_width + spacing`；`right_padding = padding + arrow_width + spacing`（`MenuItem.qml:25-28`） |
| 6 | `tool_bar` | 项位置用步长字面量 | 逐项累加「各自宽度 + `TOOLBAR_SPACING`」 |
| 7 | `status_bar` | 末段位置硬编码 | 各段按自身文本宽度 + spacing 累加，末段右锚 |
| 8 | `tab_widget` / `tab_view` | tab 宽手算 | `label_width + TAB_TEXT_PADDING`，夹在 `TAB_MIN_WIDTH..TAB_MAX_WIDTH`，间距 `TAB_SPACING` |
| 9 | `dialog` / `message_box` 的按钮行 | 按钮 x 写死 | 右对齐：从内容盒右缘反向累加各按钮宽度 + spacing |
| 10 | `scroll_area` | 条的位置手算 | `Stack` + `Absolute`（条叠在内容上） |
| 11 | `list_view` / `grid_table` | 行高手算 | `UniformGridLayout`（已有） |

**共同判据**：每个控件加一条单测断言「**子控件变宽（或字号变大）后，相邻段的起点随之移动**」——
这是 §B.9 的机械表述，也是唯一能钉住「留白由兄弟推导」的判据。
`cargo test --lib --no-default-features --features desktop <widget>` 必须全过。

### F.2.3 P1 级（计划列出但本轮未做）

| # | 条目 | 计划出处 | 说明 |
|---|---|---|---|
| **F-4** | **RTL 铺开**：`progress_bar` / `range_slider` / `tab_bar` / `app_bar` / `scroll_bar` / `menu` 方向键 | P1-2 | `TextDirection` 类型已存在且 `slider` 已接（BLUE21 第 65 轮）；其余**未接**。判据：每个控件一条「镜像后取值/绘制都反向、且往返一致」的测试——注意 §4.3 的教训：**两个方向的映射必须共用同一个 inset** |
| **F-5** | **a11y 三态填充** | P1-3 | `A11yState.checked/mixed` 结构已在（BLUE21 第 65 轮），`accessible_value` 已建；**但只有 10 处控件填充**。需让 `checkbox`/`switch`/`radio` 上报三态 |
| **F-6** | **契约加厚** | P1-4 | `auto_complete_edit` 只读 `suggestion_count`；`drop_zone` 只有 1/5 反馈态；`rating`/`shortcut_editor` 契约过薄 |
| **F-7** | **E6 分组/片段原语** | P1-5 | 三处注释声称 `spacer` 不产出控件，实际报 `UnknownWidgetType` 并**丢子树** ⇒ `engine` 丢子树 |
| **F-8** | **`stepper` 命名裁定** | P1-6 | 本仓 `stepper` = 数值微调器，Flutter `Stepper` = 分步向导 ⇒ **缺一整个控件**。二选一：改名 or 补向导控件，**不留悬空** |

### F.2.4 P2 级

| # | 条目 | 计划出处 | 说明 |
|---|---|---|---|
| **F-9** | 主题装饰 token 的消费者 | P2-1 | 7 个角色已在 `Colors` 里，但只有 `outline` 被焦点环消费。需让卡片/面板/模态遮罩/吐司真正读 `scrim`/`surface_container*`/`inverse_*` |
| **F-10** | `Font::letter_spacing` / `line_height`（2× 文本缩放的前置） | P2-3 | **未做** |
| **F-11** | 另立计划：声明式原语（portal/生命周期/上下文/错误边界）、`code_editor` 语法配色 | P2-4 | BLUE21 已明确「另立计划」 |
| **F-12** | A.4.1 文本输入的**装饰槽模型**（`prefix`/`suffix`/`helper`/`error`/`counter`） | BLUE21 A.4.1 | 整块未做 |

### F.2.5 门禁与工程债

| # | 条目 | 现状 | 说明 |
|---|---|---|---|
| **F-13** | §6.7 剩余 3 条门禁 | 未做 | `check_implicit_size_uses_metrics`（控件自己算绝对尺寸而不走 `ControlMetrics`）、`check_spacing_is_not_sibling_layout`（`spacing` 被用于兄弟间距）、`check_focus_ring_respects_reason`（无条件画焦点环）。**每条都必须配反向注入** |
| **F-14** | `check_mechanism_has_a_consumer` 的陈旧债 | **预存在失败项** | 一条陈旧的 `AnimationDriver` 债表条目。本轮未动（不属 blue22 范围），但门禁报它，需处理或更新债表 |
| **F-15** | `tests/mounted_control_follows_window_test.rs` | **预存在失败** | `RejectedByBackend("cocoa")`，需真实 macOS 窗口服务器会话。已在干净 HEAD 树上复现 ⇒ **非本轮引入**，但需在能提供窗口会话的 CI 上验证 |
| **F-16** | `check_android_cross.sh` | **host-limited skip** | 需要 Android NDK/SDK，本机跳过。非缺陷 |
| **F-17** | `spacing` 的消费者只有 checkbox/radio | 部分 | 计划 §6.7 要求「`spacing` 不得出现在兄弟布局」——需先有门禁再判断是否所有控件都遵守 |

## F.3 本轮确立、后续必须遵守的教训

1. **🚫 绝不用 `git checkout` 清理工作区**（多代理并行时尤其）。
   第 67 轮发生过一次：`git checkout` 把 12 个已完成的 `src/` 文件整体回退，
   而**未跟踪的新文件（`metrics.rs`/`hints.rs`）幸存** ⇒ 编译仍过、测试仍绿，
   **缺陷完全静默**。清理快照只能逐文件重新导出。
   （记录于 `log-20260923-1.md` §9）

2. **并行代理必须有互斥写域**。第 67 轮两个代理被派了重叠目录，
   是上述回退的隐患来源。

3. **门禁的正则也是判据**。`check_control_has_tests.py` 只认字面 `#[cfg(test)]`，
   于是 `#[cfg(all(test, full_widgets))]` 的模块被读成「不存在」，
   **有测试的控件被报成无测试**（假红）。修法是放宽为「`test` 必须在合取里」。
   该门禁的注释自己已记录过它的**假绿**版本（179/179 而 `Toast` 无测试），
   说明**假红与假绿是同一缺陷的两个方向**。

4. **timeout 机制本身要有判据**。`run_all_gates.sh` 的「跑死」不是缺 timeout，
   而是三个具体缺陷：per-gate 预算 1800s 过长（58×1800 = 最坏 29 小时）、
   缺整轮预算、以及 **`rw_kill_tree` 只杀 pid 导致 `cargo` 持 target-dir 锁
   ⇒ 下一个门禁阻塞在锁上**（**超时反而制造挂死**）。
   现在：per-gate 900s、整轮 2700s、`pkill -P` 递归杀后代、未跑到的门禁报 `NOT-RUN`
   （**既不算 PASS 也不算 FAIL**）。两者均可用 `RW_GATE_TIMEOUT` / `RW_RUN_TIMEOUT` 覆盖。

5. **README 是产物，产物要有判据**。新增 `examples/readme_check.rs` 后，
   它立刻抓到我自己的一个错误断言（把交叉轴 `Stretch` 说成「不被拉高」）。
   **同步文档不只是「改到看起来对」**。

## F.4 一句话结论

**「控件不再把自己当成容器」** —— 单控件层面本计划的目标已达成（174 个控件的绘制盒
由度量体系推导，零尺寸与空文本元素归零，五个 profile 全部编译干净）。
**「组合控件由布局组装」**（F-1/F-2）是本计划剩下的主体，施工细则已备于 F.2.2。
