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

> **⚠️ 第 68 轮更新（2026-09-23 同日续）**：F-2（§B.7 样板）与 §B.5.2 第三步
> （`CompositeBuilder`）**已完成**，并在此过程中修掉 `FlexLayout` 的三个共享缺陷。
> 详见 [`../log/log-20260923-1.md`](../log/log-20260923-1.md) 第 68 轮 §4.5 / §5。
> 新的状态见本附录末尾的 **F.5**。

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

---

## F.5 第 68 轮状态更新（2026-09-23 同日续，替代 F.2.1 中已完成的条目）

> 执行记录见 [`../log/log-20260923-1.md`](../log/log-20260923-1.md) 第 68 轮。

### F.5.1 本轮完成（有独立判据）

| 计划条目 | 状态 | 判据 |
|---|---|---|
| **§B.5.2 步 3 `CompositeBuilder`** | ✅ | 新增 `src/widget/composite.rs`（工厂创建 + 读子控件 `hints()` + 交给 `arrange` + 回写）；5 条单测，含「子控件变宽 ⇒ 组合控件变宽」 |
| **§B.7 样板 `dialog_with_actions`** | ✅ | `composite.rs` 的 `ActionRow`（4 条单测）；右对齐由 `justify_content = FlexEnd` 表达，**零行位置算术** |
| **§B.6 规则 3（固有尺寸向上传播）** | ✅ | `hints()` 的主轴取 `sum`、交叉轴取 `max`，聚合用 `layout::hints` 的 `total_bounds`/`total_minimum`/`max_preferred` |
| **§B.6 规则 9（`min` 与 `fill` 分开）** | ✅ | `fill` 在 `add()` 译成 grow 权重（`FlexLayout` 只读 `flex_grow`）；单测断言同 `pref` 的 `fill`/非 `fill` 拉伸行为不同 |
| **§6.7 门禁 `check_svg_snapshots`（复核 + 加厚）** | ✅ | 新增第 [5/5] 步：`control.md` 必须等于生成器输出；**反向注入证明可失败**（第一版是假绿，已修） |
| **用户指令 #4（`control.md` 控件图像总览）** | ✅ | `tools/generate_control_index.py`；188/188 控件归入 17 个分组，0 unclassified；376/376 链接可解析 |

### F.5.2 本轮**新发现并修复**的缺陷（不在原计划里）

| # | 缺陷 | 影响面 | 证据 |
|---|---|---|---|
| **R-1** | **SVG 后端丢弃 `HorizontalAlignment`** | 全仓**每一个**居中/右对齐标签在 SVG 输出里都是左对齐 | 探针实跑：`Left`/`Center`/`Right` 都发 `x=100` |
| **R-2** | `Button` 把 `BUTTON_PADDING_H` 当左锚点 | 按钮标签不水平居中 | `button.svg` 的 `x` 12 → 95 |
| **R-3** | `Calendar` 日期数字画在单元格**左上角 + 3px** | 数字与表格错行（用户报告） | 单测 `a_day_number_is_centred_in_its_cell` |
| **R-4** | `Calendar` 表头基于**假前提**（「1 em/字符 ⇒ Mon 需 36px」）而跨列 | 7 个列名与 7 列数字不同列 | 实测 `Mon` 推进 20px；单测 `a_weekday_heading_is_centred_in_its_column` |
| **R-5** | `EmptyState` 图标盒与标题**重叠 28px** | 标题横穿图标 | 单测 `the_stack_rows_do_not_overlap`；反向注入可失败 |
| **R-6** | `Keyboard` 键帽文字钉在顶边（注释声称居中） | 124 个键 | 快照 `y` 2 → 8 |
| **R-7** | `Gantt`/`Timeline` 行标签同上 | 每个任务名 | 改为 `draw_text_line` |
| **R-8** | `draw_arc_segments` 采样数写死 40 ⇒ **零长度线段**，115px 弧只剩 3 个像素 | spinner / progress_circle | 零长度 `<line>` 2 → 0；两条单测 + 两个反向注入 |
| **R-9** | **`FlexLayout` 剩余空间塞给最后一个孩子** ⇒ `justify_content` 不可达 | 所有走 `arrange` 的布局 | 3×60px 在 240px 带里 `x=0`（应 60） |
| **R-10** | **`FlexLayout` shrink 分支穿过 `min_size` 地板** | 子控件被压到低于自己申报的最小值 | 两个 100px 按钮各得 57px |
| **R-11** | **`FlexLayout` `consumed` 双倍计 margin** | 剩余空间被吃光 ⇒ `FlexEnd` 失效 | 220px 行的 20px 剩余被算成 8px |

> **R-9/R-10/R-11 是「被组合控件逼出来的共享缺陷」**。它们无法在纸面上推理出来：
> 只有当 `CompositeBuilder` **真的**把子控件交给 `arrange`，布局里沉睡的假设才会暴露。
> 这印证了 §B.3 的判断——**不接尺寸通道，这三条永远发现不了**。

### F.5.3 仍然未完成的（按优先级）

| # | 条目 | 出处 | 说明 |
|---|---|---|---|
| **F-1'** | §B.8 其余组合控件逐个改造 | §B.8 | `CompositeBuilder` 与规范和样板已就绪；`split_button`/`combo_box`/`menu` 等仍为手算。**注意 §5.4 的判断**：`dialog` 族的按钮行**刻意不合并**到 `ActionRow`（两者要求真的不同） |
| **F-3** | §B.6 规则 10（提示变化主动通知布局） | §B.6-10 | 仍未做：需要「脏」标记 + 宿主帧末消费 |
| **F-4 ~ F-12** | RTL 铺开 / a11y 三态 / 契约加厚 / E6 分组原语 / `stepper` 命名 / 主题装饰 token / `Font` 补字段 / 声明式原语 / 输入装饰槽 | F.2.3–F.2.4 | **本轮未动**，状态同 F.2.3/F.2.4 |
| **F-13** | §6.7 剩余 3 条门禁 | §6.7 | 仍未做 |
| **F-14** | `check_mechanism_has_a_consumer` 陈旧债 | — | 本轮全量门控已 **PASS**（58/58），该条已不再报红 |
| **F-15** | `tests/mounted_control_follows_window_test.rs` | — | 预存在，需真实 macOS 窗口会话 |
| **F-16** | `check_android_cross.sh` | — | host-limited skip，非缺陷 |

### F.5.4 本轮验证（全量一次）

| 项 | 结果 |
|---|---|
| `cargo test --lib --features desktop` | **5515 passed / 0 failed**（上轮 5513） |
| `cargo clippy --all-targets` | 0 warning / 0 error |
| 五个 profile（desktop/tablet/mobile/mini/embedded） | 全部编译通过 |
| `tools/run_all_gates.sh` | **PASS=58 FAIL=0 TIMEOUT=0 NOT-RUN=0 SKIP=1**（skip 为 Apple 主机限制） |

---

## F.6 第 70 轮状态更新（2026-09-23 同日续，第 70 轮）

> 执行记录见 [`../log/log-20260923-1.md`](../log/log-20260923-1.md) 第 70 轮。
> **本轮是 §B.8 的主体施工轮**：第一批量 2 个控件 + 第二批量 5 个家族，共 **9 个控件家族**。

### F.6.1 本轮完成（有独立判据）

| 计划条目 | 状态 | 判据 |
|---|---|---|
| **§B.8 优先级 1 `spin_box`** | ✅ | `assemble_row()` = `FlexLayout(Row)` + `CompositeBuilder`；value 列 `filled`、步骤列 `add_sized`。**快照几何逐字节不变** |
| **§B.8 优先级 2 `split_button`** | ✅ | `assemble_face()` 两列由布局排序并 tile；**快照几何逐字节不变** |
| **§B.8 优先级 3 `combo_box` 族** | ✅ | `IndicatorGeometry::for_band` 由组装产出两个盒子（4 个变体）；**快照几何逐字节不变** |
| **§B.8 优先级 5 `menu` / `menu_item`** | ✅ | `item_bands()`（FlexLayout Column）；`MENU_POPUP_PADDING` 改为列 padding；**快照几何逐字节不变** |
| **§B.8 优先级 6 `tool_bar`** | ✅ | `item_bands()`（Row/Column 随 orientation）；`item_rect` 退化为查表；删除了已死代码 `button_size` |
| **§B.8 优先级 7 `status_bar`** | ✅ | `segment_boxes()`（`justify_content = FlexEnd`）；右对齐成为布局属性 |
| **§B.8 优先级 8 `tab_widget` / `tab_view`** | ✅ | `tab_run()` 返回条带坐标下的矩形序列，四个 `TabPosition` 共用一份 |
| **§B.10 判据 9**（`grep FlexLayout::new src/widget/` → 非空） | ✅ | 由 0 变为 6 个文件 |
| **`CompositeBuilder` 的两处一般性缺口** | ✅ | `add_sized`（子控件自己的尺寸）；`fill` 权重从 `u32::MAX` 改为 **1**（否则同一行里第二个 `fill` 不可达） |
| **§B.9「留白由兄弟推导」的机械判据** | ✅ | 每个控件一条「子控件变宽 ⇒ 相邻段起点随之移动」的单测 |

### F.6.2 本轮**新发现并修复**的缺陷（不在原计划里）

| # | 缺陷 | 影响面 | 证据 |
|---|---|---|---|
| **G-1**（**已知缺陷，故意留下**） | `FlexLayout` 在「所有孩子的 `min_size` 之和 > band」时溢出，且位置从前沿排开 ⇒ 溢出全落在末尾孩子身上（可能被画到自己控件之外，SVG 下发绝对坐标 ⇒ 整块消失） | 所有走 `arrange` 的组合控件 | 单测 `floors_that_do_not_fit_overhang_rather_than_being_crossed`。**「按剩余房间封顶」的修法被实现后回退**：它会把第二个 100px 按钮压到 20px，即把失败从「溢出」换成「比标签窄的按钮」 |
| **R-13** | `fill` 的权重写成 `u32::MAX` ⇒ 同一行有两个 `fill` 时第一个独吞剩余，第二个的 `fill` **不可达** | 所有用 `LayoutParams::filled()` 的组合 | 改为 `max(1)`（= CSS `flex: 1` / Qt `stretchFactor: 1`） |
| **R-14** | **`FlexLayout` 的 shrink 只做一轮比例分配**：一个孩子顶到地板后，它没让出的房间**谁都不再让** ⇒ 行超出 band | 所有「可缩 + 不可缩」混排的行 | `split_button` 的 48px 快照：箭头列被推到 `x=48..70`（band 只有 48 宽）。第二轮向「地板上方还有空间」的孩子继续要，绝不越过任何地板 |
| **R-15** | **`AlignItems::Stretch` 无条件拉满交叉轴**，无视子控件申报的 `hints.height.pref` | 所有在交叉轴上声明了尺寸的组合（`tool_bar` 的 item 得 56 而不是 52） | CSS/Qt 的 `stretch` 语义只作用于「交叉轴无确定尺寸」的子项。修后有两条对偶测试（申报 ⇒ 用申报值；不申报 ⇒ 拉满） |
| **R-16** | 「列自己的尾部内边距」写成 trailing margin 会被前面的 `fill` 子控件**吃掉**（`leftover` 在 margins 之后才扣） | `combo_box` 的 indicator：尾部内边距消失，箭头漂到 band 最右缘 | 快照 `<line>` x1 由 210 → 222。修法：内边距并进列自己的宽度 |

### F.6.3 §F.2.2 队列现状

| 优先 | 控件 | 状态 |
|---|---|---|
| 1 | `spin_box` / `number_picker` | ✅ 第 70 轮 |
| 2 | `split_button` | ✅ 第 70 轮 |
| 3 | `combo_box` 族（4 个） | ✅ 第 70 轮 |
| 4 | `group_box` | ⬜ |
| 5 | `menu` / `menu_item` | ✅ 第 70 轮 |
| 6 | `tool_bar` | ✅ 第 70 轮 |
| 7 | `status_bar` | ✅ 第 70 轮 |
| 8 | `tab_widget` / `tab_view` | ✅ 第 70 轮 |
| 9 | `dialog` 按钮行 | 部分（`ActionRow` 就绪；8 个对话框**刻意不合并**，见 §5.4） |
| 10 | `scroll_area` | ⬜ |
| 11 | `list_view` / `grid_table` | ⬜ |

### F.6.4 本轮验证（全量一次）

| 项 | 结果 |
|---|---|
| `cargo test --lib --no-default-features --features desktop` | **5550 passed / 0 failed**（上轮 5531，+19） |
| `cargo clippy --all-targets --no-default-features --features desktop -- -D warnings` | **0 warning / 0 error** |
| 五个 profile（desktop/tablet/mobile/mini/embedded） | 全部 `Finished` |
| `tools/check_profiles.sh` | `All profile checks passed.` |
| `tools/check_android_cross.sh` | `android cross-target checks passed.` |
| `tools/run_all_gates.sh` | **PASS=58 FAIL=0 TIMEOUT=0 NOT-RUN=0 SKIP=1** |
| `tools/check_svg_snapshots.sh` | `checked=188 skipped=0 failed=0` |
| `tools/check_control_rendering.sh` | `checked=188 skipped=0 failed=0` |
| `tools/generate_control_index.py` | 188 控件，`control.md` **无变化** |
| **9 个控件家族的快照 `rect`/`line` 几何** | **全部逐字节不变**（比 §8 判据 8 的「人眼判为一致」更强） |

---

# 附录 G — 多语言文本：从「拉丁点阵」到「完整塑形」（**用户指令：多语言完美支持**）

> **立此附录的理由**：本附录不是 BLUE22 原有任何一条的延伸，而是一个**独立量级的工程**——
> 它要新建一个塑形层、引入 2–3 个第三方 crate、把字体从「代码里的表」变成「打包的资源」，
> 并重新定义本仓对「文本」的全部承诺。与 §F-11「声明式原语另立计划」同一处理。
>
> **优先于本附录的**：§F-11 / §F-12 等既有未完成项不受影响；本附录与它们**并行可做**。

## G.0 一句话结论

**当前文本层只支持「LTR + 拉丁/ASCII」，且这一点从未被声明。**
要「多语言完美支持」，必须补上三样**现在一样都没有**的东西：
**塑形引擎（shaping）、双向文本（bidi）、覆盖各脚本的字体**。

**两个必须现在就明确的取舍**：

1. **字体数据默认全不带**（用户指令）。因此**默认构建 = 拉丁/ASCII**，
   「完美支持」是**可选完美**（显式开启 `fonts-*` feature 后获得）。这一事实必须**写进文档**。
2. **塑形能力与字体数据是两个正交的轴**。中文是「只需数据、不需塑形」的脚本，
   所以 **`mini`/`embedded` 能用 ~85 KB 的点阵 CJK 拿到可读中文**，
   而不必把塑形引擎搬上 MCU。

## G.1 现状取证（本轮实跑，非引用）

### G.1.1 「塑形」现在的实际含义

```rust
// src/render/pipeline/containers.rs:93 —— shape_text
for scalar in text.chars() {
    // 相邻的组合符/ZWJ 合并成一个 cluster，这就是全部的「塑形」
}
```

```rust
// src/render/pipeline/primitives.rs:659 —— draw_text
for cluster in shaped.clusters() {
    let display_char = cluster.text.chars().find(|ch| !is_combining_mark(*ch) …);
    draw_bitmap_glyph(display_char);   // 一个 cluster -> 一个字形的位图
    pen_x += cluster.advance;          // 横向累加，无定位调整
}
```

**结论：一个字符映射到一个字形，按输入顺序横向排列。** 没有字形替换（GSUB）、
没有字形定位（GPOS）、没有双向重排（UBA）。

### G.1.2 字体覆盖（`font8x8` 的实际范围）

| 项目 | 事实 |
|---|---|
| 启用表 | 仅 `BASIC_FONTS`（`pixel_ops.rs:8` 的 `use`），即 **U+0000–U+007F** |
| 未启用 | `LATIN_FONTS` / `GREEK_FONTS` / `BLOCK_FONTS` 等**在 crate 里但本仓没接** |
| 字形 | 8×8 点阵，有效高 7 行 |
| 抗锯齿 | **无**（每个置位比特画实心矩形，`pixel_ops.rs:61` 的 `glyph_rects`） |
| 字宽 | **固定 0.6 em**（`estimate_cluster_advance`），与字形无关：`i` 与 `W` 同宽 |
| 未知字符 | `pixel_ops.rs:131` 的兜底「豆腐块」 |
| CJK / emoji / 阿拉伯 / 印度系 | **0 覆盖** |

### G.1.3 改动面（决定本计划的规模）

| 入口 | 消费点数 |
|---|---|
| `measure_text` | **176** |
| `shape_text` | 13 |
| `estimate_cluster_advance` | 9 |

**176 个 `measure_text` 消费点是本计划最大的风险面**：塑形一旦改变度量，
所有依赖「宽 = 字数 × 0.6 em」的布局与门禁都要重新校准。

### G.1.4 门禁对当前模型的依赖

| 门禁 / 工具 | 依赖的假设 |
|---|---|
| `tools/audit_text_y.py` | 从邻接 rect 推断文本带 |
| `tools/audit_text_contrast.py` | 从 SVG 的 `x`,`y` 读文本位置 |
| `tools/check_text_vertically_centred.py` | 源级检查 `y` 表达式 |
| `tests/control_rendering_census_test.rs` | **镜像 `estimate_cluster_advance`**（0.6 / 1.0 / 0.33 em） |
| `tools/check_svg_snapshots.sh` | 376 个快照**逐字节可复现** |

**其中 census 测试的那条镜像最脆弱**：它按「0.6 em/字符」推算每个标签的宽度，
换字体后必须同步改成读取真实度量，否则它会**假红**（要求截断本不需要截断的文本）。

## G.2 对标：两家怎么做到「多语言完美」

| | Flutter | QML / Qt |
|---|---|---|
| 塑形 | **HarfBuzz**（引擎内建） | **HarfBuzz**（`QTextLayout`） |
| 双向 | ICU / `unicode-bidi` 等价能力 | **ICU**（`QTextLayout` 的 bidi） |
| 字体发现 | 平台字体管理器 + 回退链 | `QFontDatabase` + 回退链 |
| 字体来源 | 引擎默认 + `pubspec` 打包 | 系统字体 |
| 测量/绘制同源 | 同一个 `Paragraph` 对象 | 同一个 `QRawFont` 对象 |
| 复杂脚本 | ✅ 天城文/阿拉伯/泰文 | ✅ |
| emoji | ✅ 彩色 | ✅ 彩色 |

**共性（本计划必须遵守的三条）**：

1. **塑形和绘制同源** —— 不允许「用 A 度量、用 B 绘制」。
2. **字体回退链是必需项，不是优化项** —— 一个字符串可以跨多个字体（拉丁 + 中文 + emoji）。
3. **bidi 是文本层的属性，不是控件的** —— 控件只说「我这一行是 LTR/RTL/自动」。

## G.3 施工方案（逐期可交付、可回退）

> **期号说明**：本节按**能力**组织（G.3.1–G.3.5），而 §G.7 按**交付单元**列出
> （G-1…G-6 含字体分档的子项）。两者的对应关系写在 §G.7 的推荐顺序里。

### G.3.0 依赖选型（先取证，不凭印象）

| 用途 | crate | 理由 |
|---|---|---|
| 塑形 | **`rustybuzz`**（HarfBuzz 的纯 Rust 移植） | 纯 Rust、无 C 依赖 → 不破坏 `check_*_cross.sh` 的 5 个 target |
| 字体解析 | **`ttf-parser`**（`rustybuzz` 的依赖） | 同上 |
| 双向 | **`unicode-bidi`** | 纯 Rust，UBA 的标准实现 |
| 字体数据 | **打包子集 TTF**（见 G.3.4） | 避免系统字体依赖导致的跨平台不一致 |
| 可选：彩色 emoji | `swash` 或预渲染位图 | **最后做**，收益最小 |

> **为什么是 `rustybuzz` 而不是 `harfbuzz-sys`**：本仓已经为「纯 Rust、跨平台可编译」
> 付过一次代价（AVIF 从 `dav1d-sys` 换成纯 Rust，见 `Cargo.toml` 的注释）。
> `harfbuzz-sys` 需要 C 工具链与交叉 sysroot，会让 Android/iOS/wasm 三个门禁变红。

### G.3.1 第 1 期：塑形层原语（不开新字体，先立通道）

**目标**：把「string → 字形序列」变成**可替换的 trait**，现有实现降为其中一个。

```rust
// src/text/shaping.rs（新建）

/// 一次塑形的结果：字形 id + 每个字形的定位 + 该 run 用的字体。
pub struct ShapedRun {
    pub font: FontId,
    pub glyphs: Vec<ShapedGlyph>,
}

pub struct ShapedGlyph {
    pub glyph_id: u32,
    /// 相对笔位的偏移（GPOS 的产物；纯位图字体下为 0）。
    pub offset: (f32, f32),
    pub advance: f32,
    /// 该字形来自哪个 cluster（用于光标准确定位与截断）。
    pub cluster: usize,
}

/// 文本塑形器。
pub trait Shaper {
    /// 塑形一段文本。`direction` 由调用方按 bidi 结果给出。
    fn shape(&self, text: &str, font: &Font, direction: Direction) -> Vec<ShapedRun>;
}
```

**判据**：

1. `SimpleTextShaper`（现有 0.6 em 模型）实现 `Shaper`，**行为逐字节不变**。
2. 新增 `RustybuzzShaper`，用**同一个** `font8x8` 生成的字形表也能塑形（证明通道通了）。
3. 门禁：`check_shaper_is_the_only_shaping_path` —— 源码中不得有第二处 `chars()` 逐字符推进。

**为什么先做这一期**：它让第 2–5 期都有地方落，且**不改变任何现有输出**，风险为零。

### G.3.2 第 2 期：字体装载与回退链

**目标**：字体从「代码里的表」变成「可装载的资源」，并支持**跨字体回退**。

```rust
// src/text/font.rs（新建）

pub struct FontId(u32);

/// 已装载的字体集合，按脚本覆盖建立回退链。
pub struct FontStack {
    faces: Vec<LoadedFace>,
}

impl FontStack {
    /// 为一个字形查找能渲染它的第一个 face。
    /// 返回 `None` 表示所有 face 都不覆盖 → 由调用方画豆腐块（并让门禁可见）。
    pub fn resolve(&self, ch: char) -> Option<(FontId, u32)>;
}
```

**判据**：

1. 一个字符串跨字体时，`Shape` 结果由**多个 run** 组成，每个 run 记录自己的 `FontId`。
2. 单测：`"A中B"` 在「拉丁 + CJK」两字体下产生 **3 个 run**，且每个 run 的 glyph 来自正确字体。
3. 未覆盖字符**不静默**：`FontStack::resolve` 返回 `None`，并且有一条门禁统计「本仓快照里出现了多少未覆盖字符」（当前应为 **0**，非 0 则逐条列出）。

### G.3.3 第 3 期：双向文本（bidi）

**目标**：`unicode-bidi` 接入，控件只需声明方向意图。

```rust
// src/text/bidi.rs（新建）

/// 控件对方向的声明。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    /// 按 UBA 自动判定（首强字符决定）。
    Auto,
    /// 强制 LTR。
    Ltr,
    /// 强制 RTL。
    Rtl,
}

/// 把一段逻辑序文本切成按视觉序排列的 run 序列。
pub fn reorder(text: &str, base: TextDirection) -> Vec<VisualRun>;
```

**判据**：

1. 单测：`"abc אבג def"` → 视觉序的 run 序列与 ICU/HarfBuzz 参考一致。
2. 单测：**往返一致** —— 对 RTL 文本先 reorder 再还原，得到原文（对应 §F-4 的教训：
   「两个方向的映射必须共用同一个 inset」）。
3. `TextDirection` **全仓只有一份定义**（原则 #54），`slider` 等活动控件由 `pub use` 接入。
4. 控件层只出现 `TextDirection`，**不得**出现「左/右」这类视觉词汇。

### G.3.4 第 4 期：字体数据（**本计划体积上最大的决定**）

**目标**：覆盖目标脚本，且**不破坏 `mini`/`embedded`**。

#### G.3.4.1 两个**正交**的轴，不是一个"Unicode 开关"

「多语言支持」在工程上是两件不同的事，它们**必须分开 gate**，否则就会出现
「为了要中文而被迫引入塑形引擎」或反之的错配：

| 轴 | 回答的问题 | 代价 |
|---|---|---|
| **能力轴：`glyph-shaping`** | 「能不能正确地**排列**字形」（取代、定位、双向、回退） | 代码：`rustybuzz` + `unicode-bidi`，几百 KB flash，**不需要堆**（可用 `heapless`/`bumpalo`） |
| **数据轴：`fonts-*`** | 「有没有那个脚本的**字形**可画」 | 体积，见下表 |

**为什么这个拆分是必要的**（本节的实证）：

| 脚本 | 不做塑形的后果 | 能否只要数据、不要塑形？ |
|---|---|---|
| **中文 / 日文 / 韩文** | 汉字是独立方块，**没有连写/重排问题** | ✅ **能** —— 不需要 GSUB/GPOS，**只要有字体数据** |
| 拉丁扩展 / 希腊 / 西里尔 | 无 | ✅ 能 |
| **阿拉伯 / 希伯来（RTL）** | 不连写 → **完全不可读**；不倒序 → **倒着读** | ❌ 必须能力轴 |
| **天城文 / 泰文（复杂塑形）** | 辅音簇不合成 → **错字** | ❌ 必须能力轴 |

> **这条结论直接决定了 `mini` 的可行性**：中文是本仓能"低成本拿下"的脚本，
> 因为它只需要**数据**，而不需要塑形引擎。

#### G.3.4.2 feature 矩阵（默认不带任何字体数据）

```toml
# ── 能力轴 ──
glyph-shaping = ["dep:rustybuzz", "dep:unicode-bidi"]

# ── 数据轴（每个都自己拉能力轴；默认全部关闭）──
fonts-latin   = ["glyph-shaping", "dep:font-latin-data"]     #  ~60 KB
fonts-cjk     = ["glyph-shaping", "dep:font-cjk-data"]       #  矢量子集 1.5-3 MB
fonts-complex = ["glyph-shaping", "dep:font-complex-data"]   #  阿拉伯/天城/泰 ~300 KB
fonts-emoji   = ["glyph-shaping", "dep:font-emoji-data"]     #  彩色 ~1-5 MB

# ── 点阵数据轴：**不拉能力轴**，因为点阵不需要塑形 ──
fonts-cjk-bitmap = ["dep:font-cjk-bitmap-data"]               #  见 G.3.4.3
```

**关键：`fonts-cjk` / `fonts-complex` / `fonts-emoji` 默认关闭**，由使用者自行开启。
这是一个**明确取舍**，必须写清而非含糊：

> **默认构建不是「完美支持」，而是「拉丁 + ASCII」。**
> 要完美支持，必须显式选字体 feature。
> 这个事实必须在 `README` 与 `lib.rs` 文档里写明，否则就是文档欺骗（原则 #18）。
> 门禁：`check_declared_script_coverage` —— 构建时按实际开启的 feature 打印
> 「本构建支持的脚本」，非零的未覆盖字符必须逐条可读（同原则 #100 的做法）。

#### G.3.4.3 点阵 CJK：给 `mini`/`embedded` 的中文方案（**实测数字**）

本仓已经会画点阵字形（`glyph_rects` + `font8x8`），所以**点阵 CJK 是同一模型的自然延伸**：
把「8×8 的 ASCII 表」换成可装载的点阵字体，**零塑形引擎、零新概念**。

实测体积（16×16 点阵，含 RLE 压缩估算）：

| 方案 | 原始 | 压后约 | 适用目标 |
|---|---|---|---|
| 16×16，GB2312 6763 字 | 211 KB | **~85 KB** | 有 512KB+ flash 的 MCU |
| 16×16，常用 3500 字 | 109 KB | **~44 KB** | 有 256KB flash 的 MCU |
| 12×12，GB2312 6763 字 | 119 KB | **~48 KB** | 小字号设备 |
| 24×24，GB2312 6763 字 | 476 KB | ~190 KB | 需要较大字号的设备 |
| 对比：现有 `font8x8` | 1.0 KB | — | — |

**`mini`/`embedded` 的中文方案（均为可选，默认关闭）**：

| feature | 塑形 | 数据 | 体积 | 中文效果 |
|---|---|---|---|---|
| （现状）`mini` | — | `font8x8` ASCII | 0 | ❌ 豆腐块 |
| **`fonts-cjk-bitmap`** | **不需要** | 16×16 GB2312 子集 | **~85 KB** | ✅ **可读**（推荐首选） |
| `fonts-cjk-bitmap-small` | 不需要 | 12×12 子集 | ~48 KB | ✅ 可读，字小 |
| `fonts-cjk`（矢量） | 需要 | 矢量子集 | 1.5–3 MB | ✅ 好，但重 |

> **推荐顺序**：`mini`/`embedded` 优先用 **`fonts-cjk-bitmap`**（~85 KB，且不需要塑形引擎）。
> 只有当设备具备 MB 级 flash **且**需要缩放/高质量排版时，才上矢量 `fonts-cjk`。

#### G.3.4.4 🚨 必须一并解决的：字形**从未**能常驻 RAM

**这是本计划里最容易在真机上翻车的一条。**

| 事实 | 后果 |
|---|---|
| 85 KB 点阵数据无法常驻 MCU RAM（典型 64–256 KB） | 必须**从 flash 按需读取** |
| 但 `font8x8` 现在是 `static` 表，**全程在内存** | 现有模型**不支持**"按需读取" |
| 点阵 CJK 每字 32 字节，一屏 200 字 = 6.4 KB | 只缓存**当前帧用到的字形**即可 |

**所以要新增一个抽象**（本仓现在没有）：

```rust
// src/text/glyph_source.rs（新建）

/// 字形位图的来源。
///
/// # 为什么必须有这一层
///
/// `font8x8` 是一张 `static` 表：它在 binary 里，但**全程占据地址空间**。
/// 这对 1 KB 的 ASCII 表没问题，对 85 KB 的 CJK 表则不行 —— MCU 没有那么多 RAM，
/// 而 flash 可以 memory-map 或分页读取。
///
/// 因此"取一个字形的位图"必须是一个**可替换的操作**，而不是一次数组索引。
pub trait GlyphSource {
    /// 取一个字符的点阵位图。
    ///
    /// 返回 `None` 表示本字体不含该字形 —— **不得**由实现方静默返回豆腐块，
    /// 否则"缺字"就成了不可观测的事实。由调用方统一决定降级方式。
    fn glyph(&self, ch: char) -> Option<Bitmap>;

    /// 该字形的推进宽度（点阵字体下与字形宽度相关，不是固定 0.6 em）。
    fn advance(&self, ch: char) -> u32;
}

/// 一次取字形的位图：点阵字体的通用表示。
pub struct Bitmap {
    pub width: u8,
    pub height: u8,
    /// 行优先，每行 `width` 个比特，MSB 在左。
    pub rows: heapless::Vec<u8, 32>,
}
```

**实现（按 profile）**：

| profile | `GlyphSource` 实现 | 数据位置 |
|---|---|---|
| `desktop`/`tablet`/`mobile` | `RustybuzzGlyphSource`（矢量，运行时光栅化） | 打包的字体文件 |
| `mini`/`embedded` + `fonts-cjk-bitmap` | `MmappedBitmapFont`（`include_bytes!` + 偏移索引） | binary 内，**按需读** |
| `mini`/`embedded`（现状） | `Font8x8Source`（包装现有 static 表） | binary 内 |

**判据**：

1. 单测：`Font8x8Source` 与现有行为**逐字节相同**（迁移不改变任何输出）。
2. 单测：点阵源**不一次装载全部字形** —— 用计数型测试替身断言"渲染 10 个字符只取了 10 次
   （及其缓存命中）"，而非 6763 次。
3. 单测：未覆盖字符返回 `None`，**不得**返回豆腐块位图。
4. 门禁：`check_glyph_source_is_not_a_static_table` —— 除 `Font8x8Source` 外，
   不得有第二处以 `static` 数组直接索引字形。

#### G.3.4.5 字体数据的其余约束

1. **字体必须随 crate 分发**，不读系统字体 —— 理由见 G.4。
2. **子集必须可重新生成**：`tools/build_font_subset.py`，输入完整字体 + 字符集，输出子集；
   子集文件是**产物**，由门禁验证「重新生成结果一致」（同 `check_generated_sources` 的形状）。
3. **许可**：所有打包字体必须为 OFL/Apache-2.0 等允许再分发的许可，并在 `NOTICE` 中列明。
   门禁：`check_packaged_fonts_are_licensed`。
4. **点阵 CJK 的字形来源必须写明**：建议用 `unifont`（GPL+字体例外）或 `wqy-bitmap`（GPL），
   或自行生成（从 OFL 矢量字体渲染点阵）—— **后者许可最干净**，推荐。

### G.3.5 第 5 期：光栅化质量（与 §G.3.1–G.3.4 并列，可独立交付）

**目标**：矢量字形的抗锯齿 + 真实字宽。

| 项 | 现状 | 目标 |
|---|---|---|
| 抗锯齿 | 无（实心矩形） | 覆盖率采样（复用现有 `blend_pixel(…, coverage)`） |
| 字宽 | 固定 0.6 em | 字体真实 advance（`rustybuzz` 给出） |
| hinting | 无 | 水平/垂直 subpixel 定位 |
| 字距 | 无 | GPOS kerning |

**判据**：快照人眼评审「从点阵变成矢量」，且 `check_svg_snapshots` 仍逐字节可复现。

## G.4 明确**不抄**的：系统字体

| 不抄 | 出处 | 为什么 |
|---|---|---|
| 读系统字体（fontconfig / CoreText / DirectWrite） | QML 的做法 | ① 5 个平台各一套代码，违反原则 #35–#41 的封装要求；② **同一控件在不同 OS 上外观不同**，而本仓 376 个快照的立命之本是「逐字节可复现」；③ 三个 cross 门禁需真机验证 |
| 依赖 `harfbuzz-sys` | Flutter 的引擎做法 | 需 C 工具链 + 交叉 sysroot；本仓已为纯 Rust 付过代价（AVIF） |
| 完整 ICU | — | 体积；本仓只需 UBA + 塑形 |

> **QML 用系统字体是因为它绑定桌面；Flutter 是显式指定字体才一致。**
> 本仓是「跨平台自绘 + 快照门禁」，两者都不是 —— 所以走 **打包字体**。

## G.5 风险（每条都对应一个已知的翻车形态）

| # | 风险 | 缓解 |
|---|---|---|
| 1 | **176 个 `measure_text` 消费点**被度量变化影响 | 第 1 期先把通道立起来且**输出不变**；第 5 期换度量时逐控件评审快照 diff |
| 2 | `census` 测试**镜像了 0.6 em 模型** | 换字体时同步改为读真实度量；先反向注入证明它会失败 |
| 3 | 门禁对「文本位置」的推断失效（`audit_text_y` 等） | 这些门禁按**从 SVG 读几何**的方式工作，而 SVG 输出已经改为字形几何（见 §G.6），因此不受字体变化影响 |
| 4 | `mini` 体积/行为回归 | 字体按 feature gate 分档，默认全关；`check_profiles.sh` 覆盖；体积以**实测**入判据（G.8 #16） |
| 5 | 字体许可 | `NOTICE` + `check_packaged_fonts_are_licensed` |
| 6 | 「测 A 绘 B」重新出现 | `check_shaper_is_the_only_shaping_path` + 「测量与绘制同源」单测 |
| 7 | 快照体积暴涨 | 只对**有文本**的控件重新生成；字形轮廓可复用一个 `<defs>` 符号表 |
| **8** | **🚨 点阵 CJK 无法常驻 RAM** —— 85 KB 数据放不进 64–256 KB 的 MCU RAM，而现有 `font8x8` 是全程在内存的 `static` 表。这是本计划**最容易在真机上直接跑不起来**的一条 | 新增 `GlyphSource` 抽象（G.3.4.4），字形**按需从 flash 读取**并只缓存当前帧用到的；判据 G.8 #8 用计数型测试替身**证明它确实没全量装载** |
| **9** | **默认构建不够「完美」引发期望差** —— 用户以为装上就有中文 | 在 `README` + `lib.rs` **明写**默认仅拉丁/ASCII；`check_declared_script_coverage` 在构建时**打印本构建支持的脚本** |
| **10** | `mini` 的 `no_std`/无堆约束与 `rustybuzz` 冲突 | `rustybuzz`/`ttf-parser` 均为 `no_std` 友好且不要求堆；塑形缓冲用 `heapless` 定长或 `bumpalo`（`mini` 已启用）。**第 1 期必须在 `mini` 上实编验证**，不能只在 desktop 上过 |

## G.6 与第 68 轮已完成工作的关系（**必读，避免重复劳动**）

第 68 轮已经把 **SVG 后端从 `<text>` 改为「同 font8x8 的字形几何」**。
这一步对本附录是**地基**，不是并行项：

| 第 68 轮的决定 | 对本附录的意义 |
|---|---|
| SVG 后端不再交给浏览器字体引擎 | 否则「完美多语言」在 SVG 里由浏览器实现、在窗口里由本仓实现 → **两个渲染器** |
| 抽出 `glyph_rects` 供两个后端共用 | 第 5 期的抗锯齿/矢量轮廓**只需改这一处** |
| 不再发 `dominant-baseline`（SVG 2 已移除该值） | 消除了「各家浏览器解释不同」的不确定性 |

**因此本附录的第 5 期是「把 `glyph_rects` 从点阵矩形升级为矢量覆盖率」，而不是重做后端。**

## G.7 分期待办清单（可直接接续）

| 期 | 内容 | 前置 | 可独立交付 |
|---|---|---|---|
| **G-1** | `Shaper` trait + `RustybuzzShaper` + 通道门禁 | — | ✅ |
| **G-2** | `FontStack` 装载与回退链 | G-1 | ✅ |
| **G-2b** | **`GlyphSource` 抽象**（字形按需读取；点阵/矢量两种实现） | G-1 | ✅ |
| **G-3** | `unicode-bidi` 接入 + `TextDirection` 统一 | G-1 | ✅ |
| **G-4a** | 子集生成器 + 许可门禁 + `NOTICE` | G-2 | ✅ |
| **G-4b** | **点阵 CJK**（`fonts-cjk-bitmap`，~85 KB，**不需塑形**） | G-2b | ✅ |
| **G-4c** | 矢量子集打包（`fonts-latin` / `fonts-cjk` / `fonts-complex`） | G-4a | ✅ |
| **G-5** | 矢量光栅化（抗锯齿 + 真实 advance + kerning） | G-1, G-4c | ✅ |
| **G-6** | 彩色 emoji | G-5 | ✅ |

**推荐施工顺序**：`G-1 → G-2b → G-4b`（先把 `mini` 的中文拿下，因为它的代价最小、
且不依赖塑形引擎）→ `G-3 → G-2 → G-4a → G-4c → G-5 → G-6`。

## G.8 验收判据（本附录专属）

```text
--- 塑形与方向（G-1 / G-2 / G-3）---
1. 单测："A中B" 跨两种字体 -> 3 个 run，各自 FontId 正确
2. 单测："abc אבג def" 的视觉序 run 序列符合 UBA
3. 单测：RTL reorder 往返一致
4. 单测：阿拉伯文 "سلام" 塑形后字形数 > 字符数（连写发生）
5. 单测：天城文辅音簇合成（"क्ष" 的字形数 < 字符数）
6. 门禁：check_shaper_is_the_only_shaping_path（无第二处逐字符推进）

--- 字形来源（G-2b / G-4b）---
7. 单测：Font8x8Source 与迁移前**逐字节相同**
8. 单测：渲染 10 个字符只取 10 次字形（非 6763 次）—— 证明点阵源按需读取
9. 单测：未覆盖字符返回 None，**不得**由实现方静默返豆腐块
10. 单测：Font8x8Source 下现有 376 快照**逐字节不变**（迁移不改变输出）
11. 门禁：check_glyph_source_is_not_a_static_table

--- 字体数据与声明边界（G-4a/b/c）---
12. 门禁：check_packaged_fonts_are_licensed（每个字体有 OFL/Apache 元数据 + NOTICE 列明）
13. 门禁：字体子集重新生成结果一致（同 check_generated_sources 形状）
14. 门禁：check_declared_script_coverage —— 打印本次构建支持的脚本，
    且快照中未覆盖字符数 == 0（非 0 逐条列出，同原则 #100）
15. 文档：README + lib.rs 明写「默认构建仅支持拉丁/ASCII，完美支持需显式开启字体 feature」
16. 体积：mini + fonts-cjk-bitmap 的 binary 增量 ≤ 120 KB（实测，非估算）
17. 体积：desktop + fonts-cjk（矢量）的 binary 增量实测并记录（预期 1.5-3 MB）

--- 光栅化与回归（G-5 / G-6）---
18. 门禁：census 测试的宽度模型 == 真实度量（而非 0.6 em 镜像）
19. 人眼：拉丁/CJK/阿拉伯 各一个控件的快照可判为「矢量、抗锯齿、字距正常」
20. 人眼：mini 的点阵 CJK 快照可判为「中文可读」
21. 回归：mini/embedded 两个 profile 仍编译通过
22. 回归：376 个快照由 check_svg_snapshots 逐字节复现
23. 回归：五个 profile × 三个 cross 门禁（android/ios/harmony）全绿
```

> **判据 16 与 17 是同一条规则**：字体体积是**本计划的核心代价**，必须以**实测**而非估算进入
> 交付物。一个「估计 85 KB」的字体如果实测 300 KB，它对 MCU 的可行性结论就变了。

## G.9 一句话结论

**本附录的目标可达，但它是「新建一个文本层」，不是「改几处绘制」。**

三样现在都没有的东西（**塑形 / bidi / 字体覆盖**）必须一起补上，缺一样都会输出**错字**
（阿拉伯不连写、天城文不合成、希伯来不倒序）——而错字比豆腐块更危险，因为它**看起来是对的**。

因此**不要先做第 5 期（美化）**：把点阵变好看，只会让「不连写的阿拉伯文」更好看，
缺陷反而更难被发现。

**三个一句话结论**：

1. **能力与数据分开 gate** —— 否则「要中文」会被迫连塑形引擎一起吃下去。
2. **默认不带字体数据** —— 于是默认不是「完美」，这个事实必须**写进 README**，
   而不是让使用者自己发现（原则 #18）。
3. **`mini` 的中文路径是点阵而不是矢量** —— ~85 KB、不需塑形引擎、复用本仓已有的点阵绘制模型，
   但**必须先有 `GlyphSource`**，否则 85 KB 在 64 KB RAM 的 MCU 上根本装不下。
