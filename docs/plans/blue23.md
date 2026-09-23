# BLUE23 — 从「度量正确」到「手感丝滑」：状态层、动效总线与 主流声明式实现 / 外部对标 / 参考工具包 三方配色对标

> 依据：[`principle.md`](principle.md)（继承 BLUE1–BLUE22，含 #1–#111）
> 前置：[`blue22.md`](blue22.md)（附录 F 执行进度、附录 G 文本层）、[`blue21.md`](blue21.md)（缺陷登记表 + 附录 A）
> 参考实现（**只读，不引入依赖**）：
> - 主流 material 实现：`<reference-tree>/material`（`<reference-tree>/material/*.dart`）
> - 参考工具包（控件与模板两层源码）：`<reference-tree>/`（`controls/`、`templates/`）
> - 主流声明式实现：**无可读源码**（Apple 闭源）。本文对 主流声明式实现 的引用一律标注为
> **「人机界面指南 / 公开 API 语义」**，不伪造文件行号——凡无出处的数值一律不写。
> 目标版本：**2.6.1 → 2.7.0**
>
> **一句话结论（先读这条）**：
> BLUE21 修的是「画错了」，BLUE22 修的是「没有度量体系」。
> **两者都已完成**——188 个控件的绘制盒已由 `ControlMetrics` 推导，尺寸通道（`Hints`/`arrange`）
> 与组合装配（`CompositeBuilder`）已接通。快照逐一核对后可以确认：
> **本仓现在「静止的一帧」已经接近 主流 material 实现 M3 的几何。**
>
> **但「丝滑顺畅」不在几何里，在几何与几何之间。**
> 本仓现在缺的是**三件事**，且三件都是「机制已建、通道已通、控件未接」：
>
> | # | 缺什么 | 现状（实测） | 后果 |
> |---|---|---|---|
> | 1 | **共享状态层** | `WidgetState` 12 变体 + `resolve_style_for_state` + `apply_active_theme(widget.widget_state())` **全链已通**，但 `fn widget_state` 只有 **1** 个实现（trait 默认值） | 主题作者写 `"button:hover"` **照样无效**——因为按钮从不上报自己 hovered |
> | 2 | **动效驾驶总线** | 11 个控件各有 `pub fn tick(delta_ms) -> bool`，**没有任何生产代码调用它们** | 动画引擎（1582 行）与控件里的动画状态机**永不前进** |
> | 3 | **层级 / 状态色 token 的消费者** | `Colors` 17 角色已齐，`scrim`/`surface_container*`/`inverse_*` 只有焦点环消费了 `outline` | 卡片、面板、模态遮罩、吐司**没有层级感**——这正是「不像 主流声明式实现」的最大单一原因 |

---

## 0A. **本计划继承的全部未实现项**（先读本节，它决定施工顺序）

> **本节是 `blue22.md` 附录 F/G 与 `FUTURE.md` 的未实现项的唯一下家。**
> `blue22.md` 自己的 F.2 已逐项关闭；从本轮起，**凡「计划了但未实现」的条目只在本节维护**，
> 不再散落在多份文件中（两份计划各写一半是 §4 已经指出的失败形态）。
>
> **施工顺序：本节排在 §2/§3/§5 之前。** 理由是本节里 **§0A.2（账实相符声明）**
> 是**全仓唯一一处「现在写就是错的」**——文档承诺面大于实际能力，
> 而它是零依赖、一次改动即可消除的。其余各项按依赖顺序接在它后面。

### 0A.0 本节的性质：**继承清单，不是新欠债**

| 来源 | 迁入的条目 |
|---|---|
| `blue22.md` 附录 G（文本层 5 期） | §0A.4 —— **塑形 / bidi / 字体覆盖，三样一样都没有** |
| `blue22.md` §F-15 / §F-16 | §0A.5 —— 两条**宿主限制**（非缺陷，但需在能跑它们的宿主上验证） |
| `blue22.md` §F-11（原「另立计划」） | 已在本计划 **§5A**（不必重复） |
| `docs/plans/FUTURE.md`（8 项） | §0A.6 —— 环境受限项，逐条带实测状态 |

> **为什么合到一起**：这四份来源互不知道对方存在，导致同一件事在两处各写一半。
> 合并后**本节是唯一入口**，每条都附实测取证（不是引用）。

### 0A.1 迁入时的逐条复核（实跑，非引用）

| 条目 | 复核方式 | 结论 |
|---|---|---|
| 附录 G-1 塑形层 | `ls src/text/`；`grep rustybuzz Cargo.toml` | **均无** ⇒ 未开工 |
| 附录 G-3 bidi | `grep unicode-bidi Cargo.toml` | **无** ⇒ 未开工 |
| 附录 G-4 字体数据 | `grep fonts- Cargo.toml` | **无任何 `fonts-*` feature** ⇒ 未开工 |
| 附录 G.8 的 6 个专属门禁 | `ls tools/` | **全部不存在** ⇒ 未开工 |
| 附录 G.8 判据 15 | `grep -i "latin\|ascii" README.md src/lib.rs` | 只命中无关行 ⇒ **声明缺失**（见 §0A.2） |
| FUTURE ITEM 0 | `bash tools/check_control_route_matrix.sh` | **PASS**（167 变体 / 0 缺失）；缺的只是**编译期**强制 ⇒ 见 §0A.6 |
| FUTURE ITEM 7 | 逐文件 `grep -c "#\[test\]"` | `ime_macos` **21** / `macos_objc2` **10** / `ios` **9** / `android` **7** 个测试仍在 `#[cfg(target_os)]` 之后 |
| THEME_BLIND 待修清单 | `grep KNOWN_THEME_BLIND tests/...` | **`&[]` 空表** ⇒ **不在本节**（已关闭，避免误列） |
| `TODO.md` | 自报计数 | `[x]` 127 / `[ ]` **0** / `[~]` **0** ⇒ **不在本节** |

### 0A.2 **P0 —— 账实相符声明：默认构建只支持拉丁/ASCII**

**这是本节排最前的一项，也是唯一「不改代码就写错」的一项。**

**现状（实测）**：

```
src/render/pipeline/pixel_ops.rs:8 use font8x8::{UnicodeFonts, BASIC_FONTS}; // 仅 U+0000–U+007F
src/render/pipeline/pixel_ops.rs:133 [0b11111111, 0b10000001, ...] // 未覆盖字符的豆腐块
src/render/pipeline/pixel_ops.rs:358 let factor = if has_wide { 1.0 } else { 0.6 }; // 固定 0.6 em
src/render/text_shaper.rs:66 let total = char_count as f32 * font_size * 0.6; // 逐字符推进
```

即：**本仓当前只支持 LTR 拉丁/ASCII**，且这一点**从未被声明**——README 与 `lib.rs`
都没有说。用户的指令是「多语言完美支持」，而**承诺面大于实际能力**本身就是一个缺陷：
一个把中文写进标签的宿主会得到豆腐块，且没有任何文档告诉它为什么。

**修法**（零依赖、零快照影响）：

1. `src/lib.rs` 的 crate 级文档加一节「Text coverage」，用 §0A.1 的四行实测说明现状，
 并写明「完美支持需显式开启字体 feature（见 §0A.4）」；
2. `README.md` 同名小节 + capability 表里注明；
3. **不谎报**：不写「支持 Unicode」，不写「多语言」，只写实测范围与豆腐块行为。

**判据**：

```text
1. grep -i "latin\|ascii" README.md src/lib.rs → 命中，且上下文是「默认范围」而非无关词
2. lib.rs 的文档测试或单测断言：默认构建下 `BASIC_FONTS.get('中')` 为 None（即豆腐块路径）
3. 不出现「Unicode 支持」「多语言」等超范围表述（人工/词表检查）
```

### 0A.3 施工顺序（本节的推荐接续）

| 顺序 | 条目 | 投入 | 可见收益 | 依赖 |
|---|---|---|---|---|
| **1** | §0A.2 账实相符声明 | 极小 | 消除「承诺 > 实际」 | — |
| **2** | §2 状态层（P0-1）+ §3 动效总线（P0-2） | 中 | **最大**：`hover`/动画同时生效 | — |
| **3** | §5 层级层（P0-3） | 中 | 观感层级 | §2 的状态色 |
| **4** | §0A.4 文本层（附录 G-1 → G-2b → G-4b） | **大** | 多语言 | **必须三样一起**（G.0 已论证） |
| **5** | §0A.6 环境受限项 | 视宿主 | 平台对等 | 需对应宿主 |

> **为什么文本层（4）排在状态/动效（2/3）之后**：G.0 的结论是**错字比豆腐块更危险**
> （阿拉伯不连写看起来「对」），所以它一旦开工就不能半途；
> 而状态/动效是本仓**引擎已建、只差接通**的一条，风险与投入都小得多。

### 0A.4 附录 G 正文（文本层，5 期 + 23 条判据）

> **原文已迁入 `blue22.md` 附录 G 不再保留**——本节继承其全部内容与判据，
> 并保留其分期编号（G-1 … G-6）以免与既有引用冲突。

| 期 | 内容 | 前置 | 现状 |
|---|---|---|---|
| **G-1** | `Shaper` trait + `RustybuzzShaper` + 通道门禁 | — | ⬜ 未开工 |
| **G-2** | `FontStack` 装载与回退链 | G-1 | ⬜ |
| **G-2b** | `GlyphSource` 抽象（点阵/矢量两种实现） | G-1 | ⬜ |
| **G-3** | `unicode-bidi` 接入 + `TextDirection` 统一 | G-1 | ⬜ |
| **G-4a** | 子集生成器 + 许可门禁 + `NOTICE` | G-2 | ⬜ |
| **G-4b** | 点阵 CJK（`fonts-cjk-bitmap`，~85 KB，**不需塑形**） | G-2b | ⬜ |
| **G-4c** | 矢量子集打包（`fonts-latin` / `fonts-cjk` / `fonts-complex`） | G-4a | ⬜ |
| **G-5** | 矢量光栅化（抗锯齿 + 真实 advance + kerning） | G-1, G-4c | ⬜ |
| **G-6** | 彩色 emoji | G-5 | ⬜ |

**推荐顺序**：`G-1 → G-2b → G-4b`（先把 `mini`/`embedded` 的中文拿下，代价最小且不依赖塑形）
`→ G-3 → G-2 → G-4a → G-4c → G-5 → G-6`。

**依赖选型**：`rustybuzz`（纯 Rust 的 HarfBuzz，**不用** `harfbuzz-sys`，后者需 C 工具链与
交叉 sysroot，会让 Android/iOS/wasm 三个门禁变红）/ `ttf-parser` / `unicode-bidi`。

**23 条验收判据**（原文照录，逐条可跑）：塑形与方向 6 条（含「阿拉伯文塑形后字形数 > 字符数」）、
字形来源 5 条（含「Font8x8Source 下现有 376 快照逐字节不变」）、
字体数据与声明边界 6 条（含判据 15 = §0A.2、判据 16/17 = **实测**体积）、
光栅化与回归 6 条。

**三条不可让的约束**：
1. **默认不带任何字体数据**（用户指令）⇒ 默认 = 拉丁/ASCII，完美是**可选完美**；
2. **塑形能力与字体数据是两个正交的轴**（中文只需数据、不需塑形）；
3. **`GlyphSource` 必须按需读取**（原文 G.3.4.4：字形**从未**能常驻 RAM，
 6763 个 CJK 字形常驻在 `mini` 上不可行）。

### 0A.5 附录 F 的两条宿主限制（非缺陷）

| 条目 | 实测状态 | 为什么不能在本机解 |
|---|---|---|
| **`check_android_cross.sh`** | 自我声明 `unsupported host`；`run_all_gates.sh` 归为 **SKIP** | 需 4 个 Android target（NDK/SDK） |
| **`mounted_control_follows_window_test`** | 已从「假红」改为**诚实 skip**（`RejectedByBackend` ⇒ 记 note 并返回，与 `control_backend_routing_test.rs:159` 同一处理） | 需一个**主线程窗口会话**的宿主；CI 上跑 macOS 目标时自然覆盖 |

> 两条都**不是代码缺陷**。它们列入本节是为了不静默（原则：留者不静默），
> 而不是为了「做完」——能做完的地方在宿主，不在本仓。

### 0A.6 `FUTURE.md` 的 8 项（逐条带实测状态）

| ITEM | 内容 | 实测状态 | 能否在本仓推进 |
|---|---|---|---|
| 0 | Hybrid 策略的**编译期**路由闭合 | 运行时矩阵门禁 **PASS**（`check_control_route_matrix.sh`，167 变体 / 0 缺失） | ✅ 可（差类型系统强制那一半） |
| 1 | Linux 无 `gtk-native` 的原生对等 | 现为 preview/state loop | ⚠️ 需定后端策略 |
| 2 | Harmony 桌面原生窗口/渲染/事件循环 | 未做 | ⚠️ 需 Harmony 宿主 |
| 2b | Windows 原生 SpinBox/ListView/ScrollArea | **已落地，运行时未验证** | ⚠️ 需 Windows 宿主 |
| 5 | macOS objc2 preview backend 转正 | 未做 | ⚠️ 需 macOS 宿主 |
| 5b | cocoa-legacy off-main-thread 崩溃 | 未修 | ⚠️ 需 macOS 宿主 |
| 6 | 跨平台控件对等矩阵闭合 | 未做 | ⚠️ 需多宿主 |
| 7 | **宿主不可见测试** | Round 7 已救回 26 个；**剩余 47 个**（`ime_macos` 21 / `macos_objc2` 10 / `ios` 9 / `android` 7） | ✅ **可**（逐模块把纯逻辑与 OS 部分拆开） |

> **ITEM 7 是本表里最值得先做的一条**：它是**纯重构**，不需要任何新宿主，
> 而它救回的是**当前完全不在任何机器上执行**的 47 条断言 —— 与「测试绿了但其实没跑」
> 是同一类风险（本仓已为此立过规矩）。
>
> **ITEM 0 的剩余半条同理**：矩阵门禁已在，缺的只是把它接进类型系统。

### 0A.7 本节验收判据

```text
--- 声明边界（§0A.2）---
1. README + lib.rs 写明「默认构建仅支持拉丁/ASCII」及豆腐块行为
2. 单测/文档测试断言默认构建下 CJK 走豆腐块路径
3. 不出现超范围表述（「Unicode 支持」「多语言」）

--- 迁入完整性（本节自身）---
4. blue22.md 不再含任何未实现项（附录 G 已迁出，F.2 已关闭）
5. 本节是「计划了但未实现」的唯一入口：FUTURE.md 8 项 + 附录 G 5 期 + 两条宿主限制
 都在本表内且带实测状态
6. 每条迁入项都有「复核方式 + 结论」，不是引用（原则 #56）
```

---

## 0. 本计划的定位，以及**为什么不是重做一遍 BLUE22**

### 0.1 BLUE22 已交付的部分（本计划**不重复**，逐条附独立判据）

| 条目 | 判据 | 本计划的态度 |
|---|---|---|
| **P0-1 度量体系** `ControlMetrics` + `dimensions` 表 | `src/widget/metrics.rs` 1310 行；`grep -rl ControlMetrics src/widget/` → **65 个文件** | ✅ 地基，直接用 |
| **P0-1b 尺寸通道** `AxisHints`/`Hints`/`LayoutParams`/`ChildInfo` | `src/layout/hints.rs` 500 行；`Layout::arrange` 默认转发 | ✅ 地基，直接用 |
| **P0-1c 组合装配** `CompositeBuilder` | `src/widget/composite.rs` 796 行 | ✅ 地基，直接用 |
| **P0-2 绘制盒由度量推导** | `switch.svg` 已是 `52×32` 轨道 + `r14` 拇指；`radio_button.svg` 已是 `r8`；`progress_bar.svg` 已是 `h4 r2`；`check_box.svg` 已是 `18×18` | ✅ **已达成**，见 §1.1 |
| **P0-3..P0-10** padding 级联 / `spacing` 语义 / `visual_focus` / 点击契约 / 幂等 setter / `Colors` 扩面 | blue22 附录 F.1 | ✅ 已完成 |
| **§B.8 组合控件（9 个家族）** | `grep -rl "FlexLayout::new" src/widget/` 由 0 → 6 | ✅ 已完成（余见 §4） |
| **`Font::letter_spacing`/`line_height`** | `src/core/font.rs:35,47`，**3 个渲染消费者**（`primitives.rs:646`/`containers.rs:123`/`svg/backend.rs:607`） | ✅ 已完成 |

> **BLUE22 剩余项**（`group_box`、`dialog` 按钮行、`scroll_area`、`list_view`、RTL 铺开、
> a11y 填充、契约加厚、F-13 三条门禁）见 §4。它们**不是**本计划的主体，但会在 §4 逐条落位，
> 因为它们与「美观」直接相关的那几条（`group_box` 的勾、`dialog` 的动作行）属于**视觉**问题。

### 0.2 为什么本计划必须另立

对标三家后可以确认：**「好看」与「丝滑」是两件不同的工程**。

| | 决定什么 | 载体 | 本仓现状 |
|---|---|---|---|
| **几何层** | 控件**长什么样** | 常量表 + 度量推导 | ✅ BLUE22 已交付 |
| **状态层** | 控件**在不同交互下长什么样** | 状态 → 样式解析 → 绘制 | ⚠️ 通道已通、**控件未接**（§2） |
| **时间层** | 控件**怎么从 A 变到 B** | 帧驱动 + 插值 + 曲线 | ⚠️ 引擎已建、**无人驱动**（§3） |
| **层级层** | 控件**谁在谁之上** | surface / scrim / elevation token | ⚠️ token 已加、**无人消费**（§5） |

**三家的共性（本计划必须遵守的三条）**：

1. **状态是控件自己的事实，不是主题推导出来的**——`reference-toolkit abstractbutton.cpp:179`
 在 `handleMove` 里 `setPressed(keepPressed || q->contains(point))`；
 `reference-toolkit control.cpp:2043-2056` 的 `hoverEnter/hoverMove/hoverLeave` 各写一行。
 参考工具包 是**控件自己维护 `pressed`/`hovered`**，主题只是**读取方**。本仓的机制方向是对的
 （`widget_state()` 放在 `Widget` trait 上），**缺的只是每个控件把它报出来**。
2. **动画由帧时钟驱动，不由「状态变化时启动一个定时器」驱动**。
 主流 material 实现 的 `AnimationController` 绑 `Ticker`；参考工具包 的 `Behavior` 由渲染线程的
 `QQuickWindow::update()` 循环推进。本仓已有 11 个 `tick(delta_ms)` 却**无驱动者**，
 这正是「引擎建了、没人开开关」。
3. **层级必须由 token 表达，不能各控件现拼**。Material 有 6 级 `surfaceContainer*` +
 `scrim`；本仓**已有这 7 个 token 且已写入 `themes/*.json`**，但没有消费者（§5）。

### 0.3 本计划的判据形态（**区别于 BLUE21 / BLUE22**）

| | BLUE21 | BLUE22 | **BLUE23** |
|---|---|---|---|
| 判据 | 「不报错」 | 「快照 diff 人眼可判更小更居中」 | **「连续两帧的 diff 人眼可判为中间态」** |
| 为什么 | 修的是错误 | 修的是几何 | **修的是时间** |

**一条动画无法用「一帧」来证明。** 所以本计划所有动效条目的判据统一为：
**在同一控件上取 t=0 / t=½ / t=1 三个快照，三个几何必须互不相同，且 t=½ 落在两者之间。**
这条判据能做门禁（对数值采样，不需要人眼），且**反向注入天然可失败**（把 `tick` 拆掉 ⇒ 三帧相同）。

---

## 1. 现状取证（本轮实跑，非引用）

### 1.1 「静止的一帧」已经对了——用快照证明

```text
$ head -6 snapshots/svg/switch.svg
<rect x="94" y="44" width="52" height="32" rx="16" ry="16" fill="rgba(69,69,69,1.00)" />
<rect x="96" y="46" width="28" height="28" rx="14" ry="14" fill="rgba(251,251,251,1.00)" />
 ↑ 52×32 轨道 + 28px 拇指，居中于 240×120 画布 —— 主流 material 实现 M3 的形状

$ head -5 snapshots/svg/radio_button.svg
<circle cx="9" cy="60" r="8" fill="none" stroke="rgba(225,225,225,1.00)" stroke-width="2" />
 ↑ r8 前置对齐（曾是 r30 居中）

$ head -5 snapshots/svg/progress_bar.svg
<rect x="0" y="58" width="240" height="4" rx="2" ry="2" fill="rgba(15,15,15,1.00)" />
 ↑ 高 4 / 圆角 2（曾是一块 240×120 实心板）

$ head -5 snapshots/svg/check_box.svg
<rect x="2" y="51" width="18" height="18" fill="rgba(69,69,69,1.00)" />
 ↑ 18×18（主流 material 实现 checkbox.dart:405）
```

**结论：`src/widget/metrics.rs:394` 起的 `dimensions` 表已被 65 个控件消费，
BLUE22 §2 的对照表**（按钮 `64×40` / 开关 `52×32` / 进度高 4 / chip 高 32 /
`TOUCH_TARGET_MIN 48` …）**数值已全部落地。本计划不需要再改这些数字。**

### 1.2 缺口一：状态通道**通了，但只有一个实现**

```text
$ grep -rn "fn widget_state" src/widget/ --include=*.rs
src/widget/widget_trait.rs:392: fn widget_state(&self) -> crate::style::WidgetState {

$ grep -rln "fn widget_state" src/widget/ | wc -l
1 ← 只有 trait 的默认实现，零个控件覆写
```

而**驱动侧是完整的**：

```rust
// src/theme/apply.rs:85-87 —— 已接线
let state = widget.widget_state();
let Some(theme_style) = crate::theme::resolved_theme_style_for_state(kind_name, state) else {
 return;
};
```

```rust
// src/widget/widget_trait.rs:392-398 —— 默认实现只报 disabled/normal
fn widget_state(&self) -> crate::style::WidgetState {
 if self.is_enabled() { crate::style::WidgetState::Normal }
 else { crate::style::WidgetState::Disabled }
}
```

**后果（可复现）**：主题作者写 `"button:hover"` **永远不会命中**，因为 `Button`
从不上报 `Hover`。BLUE21 的 P0-3 把「查询侧」修好了，**「供述侧」至今是空的**。

`WidgetState` 的 12 个变体（`src/style/theme_state.rs:19-46`）与
`state_suffix`（`src/theme/manager.rs:470-485`）的 12 个字符串**都是齐全的**，
`themes/dark.json` 的 `overrides.styles` 目前是 `{}`——**一个状态覆盖都没有**，
因为这个文件由 `tools/generate.sh` 生成，而生成器没人要求它写状态覆盖。

### 1.3 缺口二：11 个控件自己会动，但**没有任何生产代码调用 `tick`**

```text
$ grep -rn "pub fn tick" src/ --include=*.rs
src/style/animation.rs:1542: pub fn tick(&mut self, delta_ms: u32) -> bool ← Transition
src/style/animation.rs:1673: pub fn tick(&mut self, target, delta_ms) -> bool ← 带目标的过渡
src/widget/base_widgets/button.rs:228: pub fn tick(&mut self, delta_ms: u32) -> bool
src/widget/display_widgets/switch.rs:178: pub fn tick(&mut self, delta_ms: u32) -> bool
src/widget/display_widgets/spinner.rs:115: pub fn tick(&mut self, delta_ms: u32)
src/widget/display_widgets/floating_label.rs:251: pub fn tick(..) -> bool
src/widget/input_widgets/lineedit.rs:127: pub fn tick(..) -> bool ← 光标闪烁
src/widget/input_widgets/tag_input.rs:213: pub fn tick(..) -> bool
src/widget/input_widgets/inplace_editor.rs:109: pub fn tick(..) -> bool
src/widget/special_widgets/code_editor/editor.rs:302: pub fn tick(..) -> bool
src/widget/media_widgets/{video_player,lottie,drive,animated_image,hero_animation}: tick

$ grep -rn "\.tick(" src/app/ src/render/ src/platform/ --include=*.rs → 0 命中
$ grep -rn "tick" demo/control/src/ --include=*.rs → 0 命中
```

**「引擎已存在、缺的只是 token 与接线」**（BLUE21 AR7 的原话）**至今成立**：
- `Button` 有 `interaction_progress: Transition`（`button.rs:87`）与
 `interaction_target_progress()`（`button.rs:242`）——**状态机是完整的**，
 `tick` 也实现并测试过（`button.rs:1630` 起 4 条单测）；
- 但 `tick` 的调用者**一个都没有**。于是运行时 hover 一次：
 `interaction_progress` 从 0 到 1 的插值**永不发生**，hover 反馈其实是**硬切**
 （或更准确地说：**`draw` 里读到的 progress 恒为初始值**）。

**这就是「不丝滑」的机械定义：库里有动画，屏幕上没有。**

### 1.4 缺口三：17 个颜色角色，只有 8 个有消费者

```text
$ python3 -c "import json;print(list(json.load(open('themes/dark.json'))['colors'].keys()))"
background foreground primary secondary accent error warning success disabled info
outline outline_variant scrim surface_container surface_container_high
inverse_surface on_inverse_surface ← 17 个角色全部已写入预设
```

| token | 谁在读 | 状态 |
|---|---|---|
| `outline` | 焦点环 | ✅ 1 个消费者 |
| `outline_variant` | — | ❌ |
| `scrim` | — | ❌（`bottom_sheet` 用 `ink.blend(sheet, 0.55)` 现拼，见 BLUE21 B23） |
| `surface_container` | — | ❌ |
| `surface_container_high` | — | ❌ |
| `inverse_surface` / `on_inverse_surface` | — | ❌（`tooltip`/`toast`/`snackbar` 无从相对页面反转） |

**这是「不像 主流声明式实现」的最大单一原因。** 主流声明式实现 的观感来自 `Material` 的
`.regularMaterial` / `.thinMaterial` 分层——**同一块面上叠出 5 个亮度台阶**；
Material M3 用 `surfaceContainerLowest…Highest` 表达同一条轴；
参考工具包 用 `palette.window`/`button`/`base`/`alternateBase` 四档。
**本仓有这 6 档，但没有一个控件踩在上面。**

### 1.5 缺口四：单一状态（而非集合）的**具体后果**（可复现）

`Widget` trait 的文档自己记录了取舍（`widget_trait.rs:384-391`）：

> 主流 material 实现 models `WidgetState` as a `Set` because several states genuinely hold at once
> (`focused | hovered`). Encoding that here would change this type's public shape …
> A control that has several states true at once reports the one with the strongest
> visual claim, in this order: disabled > pressed > checked > hovered > resting.

**单值的后果**：一个「聚焦中且被 hover」的按钮，主题只能给其中一个上色。
主流 material 实现 的 `button_style_button.dart` 是**按优先级列表取第一个命中**
（`resolve()` 遍历 `WidgetStateProperty` 的 `Set<WidgetState>`），
所以 `{focused, hovered}` 能同时命中 `focused` 的描边与 `hovered` 的填充。

本计划**不改成 `Set`**（会动公开形状，违反 #21），而是：
**单值决定「填充」，另立一条独立的「叠加层」表达同时成立的状态**（§2.3）。

---

## 2. P0 —— 状态层：让每个控件**说出**自己现在的样子

> **这是本计划的地基。** 不接这一步，后面所有动效都无处落（动效的目标值来自状态）。

### 2.1 为什么必须在 `Widget` trait 层做，不能在控件里各写一遍

BLUE21 的教训（#51：修复量应随**层**下降）：
188 个控件若各自写 `if hovered { … }`，就是 188 次返工，且**下次主题仍漂**。

**正确的位置是 `widget_state()`——它已经在 trait 上，已经接到 `apply_active_theme`。**
要做的是**给它默认实现补上 hover/pressed/focus 三个通用事实**，让**不覆写**的控件也立即获得状态能力。

### 2.2 P0-1 「状态的通用来源」——`BaseWidget` 持有三个通用事实

**现状**：`hovered` 只有 18 个文件提及（且多为图表数据态），`pressed` 是每个控件的私有字段：

```text
$ grep -rln "hovered: bool\|is_hovered" src/widget/ | wc -l → 18（其中多数是图表）
$ grep -rn "impl Widget for" src/widget/ --include=*.rs | wc -l → 195
```

**修法**（**一处**，`src/widget/base.rs`）：

```rust
/// 指针是否停留在本控件上。
///
/// # 为什么放在 `BaseWidget` 而不是每个控件
///
/// `runtime.rs:798-813` 的 `dispatch_hover_transition` **已经在**产生
/// `Event::MouseEnter`/`MouseLeave` 这一对事件（先 MouseLeave 后 MouseEnter，
/// 一个 `last_hovered` Option 保证配对），也就是说**「谁被 hover 了」这个事实
/// 在运行时层已经完全确定**。控件只要在 `BaseWidget::handle_event` 里把它记下来，
/// 就**不需要**各自索引指针位置——那是 `Button` 现在做的事（`button.rs:204`），
/// 而 `CheckBox`/`ToggleButton`/`Switch` 都没做，于是它们永远拿不到 hover 态。
///
/// 参考工具包 的位置在这里：`QQuickControl` 的 `hoverEnter/Move/Leave`（`reference-toolkit control.cpp:2043-2056`）
/// 由**基类**维护 `hovered`，`QQuickAbstractButton` 只额外维护 `pressed`。
/// 本仓的 `Button` 已经把两件事都做了，只是做在**子类**——把它上提，188 个控件
/// 一起获得，而不是 188 次抄写。
```

`BaseWidget` 新增三个字段 + 三个访问器：

| 字段 | 谁写 | 语义 |
|---|---|---|
| `hovered: bool` | `MouseEnter` ⇒ true，`MouseLeave` ⇒ false | 「指针在我身上」（参考工具包 `Control::hovered`） |
| `pressed: bool` | `MousePress` ⇒ `contains_point_with_touch_expansion(pos)`，`MouseRelease`/`Ungrab` ⇒ false | 「该画凹陷」（参考工具包 `AbstractButton::pressed`） |
| `grabbed: bool` | `MousePress` ⇒ true，`MouseRelease`/`Ungrab` ⇒ false | 「手势归我」（参考工具包 `explicitDown`，BLUE22 P0-7 已在此仓建立同名概念） |
| `focus_reason: FocusReason` | `FocusGained { reason }` / `FocusLost` | 「焦点在不在，以及为何而来」（决定**画不画焦点环**） |

> **`pressed` 与 `grabbed` 必须分开**——这正是 BLUE22 P0-7 与 参考工具包
> `qquickabstractbutton_p.h:31-32` 的双字段设计。`Button` 已有这两个字段；
> 上提时**直接搬**，不改语义。

然后 `widget_state()` 的**默认实现**升级为（保持单值、保持优先级文档一致）：

```rust
fn widget_state(&self) -> WidgetState {
 let base = self.base();
 if !self.is_enabled() { WidgetState::Disabled }
 else if base.is_pressed() { WidgetState::Pressed }
 else if base.is_hovered() { WidgetState::Hover }
 else if base.focus_reason().draws_focus_ring() { WidgetState::Focused }
 else { WidgetState::Normal }
}
```

> **`focus_reason` 也必须一并上提。** 实测：`focus_reason` 现在只有
> `button.rs:79`、`switch.rs`、`radiobutton.rs:220` 三个控件各自持有
> （`grep -rn "visual_focus" src/` 只命中这三处 + 各自的 `FocusRing` 调用点），
> **trait 上没有它**。所以「不覆写的控件」连焦点态都报不出——与 `hovered` 是同一个缺口。
> 三字段（hovered/pressed/grabbed）+ `focus_reason` 一起上提，才是一次完整的上提。

**判据**：

```text
1. 单测：`CheckBox`（未覆写 widget_state）在收到 MouseEnter 后
 `widget_state() == WidgetState::Hover` —— 当前**不可能**通过
2. 单测：`MouseLeave` 后回到 Normal
3. 单测：disabled 优先于 hovered（顺序与 widget_trait.rs:390 的文档一致）
4. 门禁：`check_state_source_is_the_base` —— 除 `BaseWidget` 外，
 不得有第二个 `hovered: bool` 字段（现有 18 处需逐一裁定：
 图表的数据态 hover 不算，改造为 `hovered_item` 之类明确名字）
5. 反向注入：删掉 `MouseEnter` 分支 ⇒ 判据 1 变红
```

### 2.3 P0-2 「同时成立的状态」——叠加层，而不是把枚举改成集合

**问题**（§1.5）：单值无法表达 `focused | hovered`。

**为什么不改成 `Set<WidgetState>`**：`WidgetState` 是 `pub`、`Eq + Hash`、被
`StatefulTheme` 当 `HashMap` 键、被 `set_transition((from, to), ms)` 当**二元组**用
（`theme_state.rs:148,183`）。改成集合会让「过渡」变成「集合到集合」，
即 CSS transition 的组合爆炸——**这正是 主流 material 实现 不提供「状态集合间过渡」的原因**。

**修法**：单值决定**填充基色**，另立一条**独立的、可叠加的**绘制层：

```rust
/// 一个控件当前应当**叠加**在填充之上的状态层。
///
/// # 为什么要与 `widget_state()` 分开
///
/// `widget_state()` 回答「用哪一套颜色」（单值，可过渡，是主题的查找键）。
/// 本结构回答「还要叠什么」（可多项同时成立，**不过渡**，是绘制指令）。
///
/// 这条分界来自 参考工具包：`QQuickControl::hovered` 与 `AbstractButton::pressed` 是
/// **两个独立的 bool**，可同时为真，且 `reference: the base control's background/padding contract` 的 `background` 对二者的
/// 处理是**两个独立的 blend**（`reference: the button's implicit-size formula and padding cascade` 用 `down ? 0.5 : 0.0`
/// 而 `visualFocus` 走**描边**，互不覆盖）。
///
/// 而 Material 的 `WidgetStateProperty` 是**首次匹配的集合**，它用
/// 优先级列表回避了「同时成立」的颜色冲突——代价是样式作者**必须**为
/// `{hovered, focused}` 单独写一条，否则 hover 会盖掉 focus 的描边。
/// 本仓取 参考工具包 的形态：**填充与描边是两条独立的通道**，就不需要笛卡尔积。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StateOverlay {
 /// 指针悬停：填充向 ink 混 8%（主流 material 实现 M3 `hovered` 的 0.08，`slider.dart:2360-2372`）。
 pub hovered: bool,
 /// 按下：填充向 ink 混 10%（主流 material 实现 M3 `dragged` 0.1）。
 pub pressed: bool,
 /// 键盘焦点：画焦点环（走 `FocusRing`，**不是**填充）。
 pub focused: bool,
 /// 展开/勾选等持续事实，由控件自行决定叠加。
 pub checked: bool,
}
```

**判据**：单测 `{hovered: true, focused: true}` 同时产出「填充混色」与「焦点环」——
当前 `visual_focus()` 与 hover 是**两条早已独立的路径**，本项只是把 hover 也纳入
`BaseWidget`，使**不覆写任何方法的控件**同时拿到两者。

### 2.4 P0-3 把状态覆盖**真的写进预设主题**

**现状**：`themes/dark.json` 与 `themes/default.json` 的 `overrides.styles` **都是 `{}`**。

**后果**：即使 §2.2 接通，屏幕上也**不会有任何变化**——因为预设里没有状态覆盖，
`resolve_style_for_state` 只是返回 base style。

**修法**：在 `tools/generate.sh` 里为**高频交互控件**生成状态覆盖，
数值来源**必须是三家的公开规范之一**，不得自创：

| 状态键 | 目标控件 | 主流 material 实现 依据 | 参考工具包 依据 | 建议 blend |
|---|---|---|---|---|
| `button:hover` | button, toggle_button, tool_button, split_button | M3 hover **0.08** 向 onSurface | `Color.blend(button, mid, …)`（`reference: the button's implicit-size formula and padding cascade`） | fill 向 `contrast_color()` 混 **0.08** |
| `button:pressed` | 同上 | M3 pressed **0.10** | `down ? 0.5 : 0.0` 向 mid（`reference: the button's implicit-size formula and padding cascade`） | 混 **0.12** |
| `button:disabled` | 全部交互控件 | `onSurface.withOpacity(0.38)` | `palette.disabled` 组 | 现有 `Colors::disabled` + alpha 150（`button.rs:739` 已有先例） |
| `check_box:checked` / `switch:checked` / `radio_button:checked` | 三态控件 | `colorScheme.primary` | `palette.highlight` | 现有 `accent`/`Success` token（`switch.rs:312` 已如此） |
| `*:error` | line_edit, combo_box, date_edit… | `colors.error` 描边（`checkbox.dart:943-945`） | `palette` 无对应 | `Colors::error` |

> **`*:error` 是本仓已有的一个「零消费者」token**（BLUE21 AR6 已记）：
> 「本仓**有** `colors.error`，但零个控件路由到它」。输入框的**行内校验提示**
> 是 外部对标 / 主流声明式实现 最显眼的日常状态，本仓完全缺失。

**判据**：

```text
1. `themes/dark.json` 的 `overrides.styles` 非空，且**含至少 6 个 "kind:state" 键**
2. 单测：装上预设主题后，一个 hovered 的 Button 的**绘制填充** ≠ 静止态填充
3. 门禁：`check_declared_tokens_have_consumers` —— 每个 `Colors` 字段
 至少 1 个生产读取点（现 17 个角色中 9 个不满足）
4. 反向注入：把 `overrides.styles` 清空 ⇒ 判据 2 变红
```

---

## 3. P0 —— 动效总线：让 `tick` 有**唯一的**驱动者

> 「丝滑」的机械定义：**每一帧都推进动画状态，且没有动画时**不**推进、不**重绘。**
> 两半都要成立——只做前半是耗电，只做后半是静止。

### 3.1 现状：11 个 `tick`、0 个调用者

（见 §1.3 实跑。）**这不是「动画没实现」，而是「动画没有时钟」。**

### 3.2 P0-4 「动画帧」的唯一契约定在 `Widget` trait 上

```rust
/// 推进本控件的自驱动动画，报告「是否还需要下一帧」。
///
/// # 为什么放在 trait 而不是每个控件自便
///
/// 本仓现在**有 11 个同名同形的 `pub fn tick(&mut self, delta_ms) -> bool`**
/// （`button.rs:228`、`switch.rs:178`、`spinner.rs:115`、`lineedit.rs:127`、
/// `code_editor/editor.rs:302`、`floating_label.rs:251` …）——**形状已经统一，
/// 只是各自是固有方法，宿主持有 `&mut dyn Widget` 时无法调用**。
///
/// 所以这一步是**纯加法**：把已有的形状上提为 trait 方法，
/// 11 个固有方法原样成为实现，不写一行新逻辑。
///
/// # 为什么返回值是 bool 而不是 `Option<Duration>`
///
/// `floating_label.rs:133-139` 的文档已经定义了正确的经济学：
/// **只在还在动时调度下一帧**。`false` 的语义是「我不欠任何帧」，
/// 宿主据此停止为它排帧——一个静止的按钮**每帧成本为零**。
/// 这与 主流 material 实现 的 `Ticker` 由 `AnimationController` 自动 `stop()` 是同一件事。
fn tick(&mut self, delta_ms: u32) -> bool { false }
/// 本控件当前是否**正在**动画（用于宿主决定是否进入连续帧模式）。
fn is_animating(&self) -> bool { false }
```

> **默认 `false`** 让 195 个 `impl Widget for` 不必同时改完——这正是 BLUE22 P0-1b
> 处理 `Layout::arrange` 的同一手法（#51 的机械前提）。

### 3.3 P0-5 帧驱动**一处**：`runtime` 的帧末钩子

```text
$ grep -rn "\.tick(" src/app/ src/render/ src/platform/ --include=*.rs → 0 命中
```

**修法**（`src/widget/runtime.rs`，与 §2 的 `for_each_mounted_widget` 同族）：

```rust
/// 推进所有**正在动画**的控件，并返回「是否还需要下一帧」。
///
/// # 为什么必须是这一个函数
///
/// `src/style/animation.rs`（1582 行）与 11 个控件的自驱动动画**都已经正确实现**，
/// 缺的只是**一个调用者**。而「谁来调」有两个错误答案：
///
/// - **每个控件自建定时器** ⇒ 100 个控件 100 个定时器，且与帧率不同步
/// （参考工具包 专门提供 `QQuickWindow::update()` 作为唯一推进点，正是为了避免这个）。
/// - **宿主/后端各调一次** ⇒ 同一个按钮在一帧里被推进两次，动画速度随调用点数翻倍。
///
/// 所以推进点是**运行时唯一的一个**，由宿主的帧循环在**绘制之前**调用一次。
/// 「谁在被推进」的答案来自**每个控件自己**（`is_animating()`），
/// 而不是由运行时维护一张动画名册——名册会有「注册了没注销」的漏，
/// 而自述不会有（控件被 `unregister` 时自然消失）。
pub fn tick_animations(delta_ms: u32) -> bool;
```

**接入顺序**（每一档单独可交付）：

| 档 | 控件 | 用户可见的收益 | 主流 material 实现 时长依据 |
|---|---|---|---|
| **A** | `button` / `toggle_button` / `tool_button` | hover/按下**渐变**而非硬切 | `kThemeChangeDuration` **200**（`constants.dart:39`） |
| **B** | `switch` | 拇指**滑动**（`travel` 已实现，只差驱动） | `_kSwitchToggleDuration` M3 **300**（`switch.dart:2370-2396`） |
| **C** | `lineedit` / `code_editor` / `tag_input` / `inplace_editor` | **光标闪烁**（`CursorBlink` 已实现） | `_kCursorBlinkHalfPeriod` **500**（`editable_text.dart:113`） |
| **D** | `spinner` | 旋转 | 现有 |
| **E** | `floating_label` | 标签上浮（已有 `tick`） | 现有 |
| **F** | `progress_bar` 不确定态 | 1800 ms 循环 | `progress_indicator.dart:23` |

**判据**（全计划统一的「三帧」判据）：

```text
1. 单测：Button 收到 MouseEnter 后 `is_animating() == true`；
 连续 `tick_animations(100)` × 2 后 `is_animating() == false`（200ms 走完）
2. 单测：静止控件 `is_animating() == false`，且 `tick_animations` 返回 false
 —— 即**静止时零成本**（这条比动画本身更重要：它决定「丝滑」是否以耗电换来的）
3. 几何判据：同一 Switch 在 t=0 / t=150ms / t=300ms 三个快照的
 拇指 `x` **必须互不相同**，且 t=150 的 x 严格落在两者之间
4. 门禁：`check_animation_has_a_driver` —— 每个实现了 `tick` 的控件
 必须能被 `tick_animations` 推进（用计数型测试替身断言）
5. 门禁：`check_transition_durations_are_tokens`（已存在）扩展到
 控件自驱动动画的时长必须来自 `theme.motion`
6. 反向注入：让 `tick_animations` 直接返回 false ⇒ 判据 3 变红
```

### 3.4 P0-6 曲线：三家一致的那一条，以及本仓**已有的**原语

`EasingFunction` 已存在（`src/style/animation.rs`），`Motion.easing` 预设为 `EaseOut`。
**新增的唯一曲线是「进入用 easeOut、退出用 easeIn」**：

- 主流 material 实现：`toggleable.dart:159-163` **前向/反向各自一条曲线**；
- 参考工具包：`reference: the switch's track, thumb and transition` 的 `Behavior` 是 `SmoothedAnimation { velocity: 200 }`
 ——**速度驱动**而非时长驱动，但**进入/退出共用同一条**；
- 本仓：`Transition::tick(target, delta_ms)` 已能按目标方向走。

**本计划只采纳 主流 material 实现 的一条**（前向 `easeOut`、反向 `easeIn`），
因为 参考工具包 的 `SmoothedAnimation` 是「恒定速度」语义，**与时长 token 体系冲突**
（本仓的 `Motion` 是时长制）。这是**明确的取舍**，不两边都抄。

---

## 4. P1 —— BLUE22 遗留项中**属于视觉**的部分（逐条落位）

> BLUE22 F.5.3 的 F-1′～F-12 中，以下几项**直接改变外观**，纳入本计划本节。
>
> **不属本节但在本计划内**的：
> * **F-11 声明式原语**（portal / 生命周期 / 上下文 / 错误边界 / `SyntaxPalette`）
> ⇒ **§5A**（它是「能不能表达」，不是「好不好看」）；
> * **F-6 契约加厚 / F-5 a11y / F-4 RTL 剩余** ⇒ 已由 BLUE22 第 71 轮完成，
> 状态见 `blue22.md` 附录 F.8；
> * **F-13 三条门禁** ⇒ 仍属 BLUE22，**本计划不接**（避免两份计划各写一半）。
> * **F-12 输入装饰槽** ⇒ 已由 BLUE22 第 71 轮完成（`src/widget/decorations.rs`）。

### 4.1 P1-1 `group_box` 可勾选态：**纯黑勾**（BLUE21 B22，**至今未修**）

```text
$ grep -n "checkable\|Color::rgb" src/widget/container_widgets/groupbox.rs
```

**问题**：勾是字面量黑色，在暗态下是**最不可读的一笔**，而它正是用户唯一会切换的部件；
且 `constructors.rs` 从不 `set_checkable(true)` ⇒ **快照覆盖不到这个状态**
（BLUE21 已记：`check_dropdown_state_is_visible` 门禁管不到它）。

**修法**：框取 `style.border_color`，勾取**框填充的对比色**（`mdiarea.rs:631` 的
`primary.contrast_color()` 是同仓先例）；同时让导出器产出一个 `group_box_checked` 外观，
**使该状态进入快照**——这一半比改色更重要。

### 4.2 P1-2 `dialog` 按钮行的纵向居中（BLUE21 A12，6 个对话框）

按钮 `y=80 h=28` 而 `draw_text_fitted` 收到未居中的 bounds ⇒ `y=80`（应为 87）。
BLUE22 已建 `ActionRow`（`composite.rs`），但 8 个对话框**刻意未合并**（§5.4）。
**本项只需把 `ActionRow` 产出的行盒交给 `draw_text_fitted`**，不动结构。

### 4.3 P1-3 `scroll_area` 的滚动条（BLUE21 B13，8 处字面量）

同一文件 `draw_sticky_band`（`scrollarea.rs:453-468`）**已做对**，滚动条未抄。
**修法**：抄自己的 `draw_sticky_band`。同时按 参考工具包 `reference: the scroll bar's minimum-length and hide-delay rules`
补「最小长度是**分数**而非常量」的语义（防细条拇指消失），
本仓已有 `dimensions::SCROLLBAR_MIN_LENGTH = 48`——**两者应一致**：
`max(SCROLLBAR_MIN_LENGTH, track * content_ratio)`。

### 4.4 P1-4 `tab_bar` 三种形状画得一样（BLUE21 D9）+ 无溢出出口（D10）

`tabwidget.rs:519-560` 的三种形状是**真画**的；`tab_bar.rs:584-610` 三臂相同，
且注释描述了没做的活。**同一枚举值在两个 tab 控件里含义必须一致**。
溢出：参考工具包 出滚动箭头、主流 material 实现 `isScrollable`、`tab_view.rs:227` 已有
`rect.width / tab_count` 的解法——**三选一，不留悬空**。

### 4.5 P1-5 `meter` 刻度与弧相差 90°（BLUE21 D16）

`meter.rs:645` 的 `tick_angle_deg` **漏了 `+ offset`**（弧用 `:313-314` 的
`arc_start_deg + offset`）。刻度是仪表盘上**唯一的信息载体**，错 90° 等于刻度在量另一个量。

### 4.6 P1-6 `chart` 没有值轴（BLUE21 D6）+ 金融四图空态无轴（D5）

`chart.rs:475-489` 的 `PlotArea` 无左槽；`candlestick/volume/depth` 在
`bars.is_empty()` 时直接 `return`。**主流 material 实现 `fl_chart` 默认开 `leftTitles`；
参考工具包 `QChart` 必有 `QValueAxis`；主流声明式实现 `Chart` 默认 `AxisMarks`**——三家一致。

> 这里的对外一致性判据（BLUE21 A.6 已列）：`chart.svg` 的最高柱**贴顶零余量**
> 正是「没有值轴」的直接后果（有轴就要给轴和标签留位）。

---

## 5. P1 —— 层级层：让 17 个颜色角色**全部**有消费者

### 5.1 P1-7 `scrim`：模态遮罩

**修法**：`dialog`/`message_box`/`bottom_sheet` 的遮罩读 `style::scrim_color()`，
回退到 `sheet_color.blend(&Color::BLACK, 0.32)`（BLUE21 B23 已论证：方向必须朝**绝对暗色**，
而不是朝 ink——暗态下朝 ink 会把背板**照亮**）。

**判据**：暗态遮罩的亮度 **<** 它覆盖的面；明暗两态的 scrim 不再「数值巧合相同」。

### 5.2 P1-8 `surface_container*`：卡片与面板的层级

> **注意**：本仓**没有 `card` 控件**——`dimensions::CARD_RADIUS = 12` 是从 主流 material 实现
> 抄来的常量，但 `grep -rn CARD_RADIUS src/ | grep -v metrics.rs` **零命中**，
> 它现在是个无人读的常量（§5.5 的门禁会把它抓出来）。**承载层级的面是 `panel`**
> （`WidgetKind::Panel`，`kind.rs:80-86` 说明它是 `GroupBox` 的 `pub type`），
> 加上 `popover`/`menu`/`tooltip`/`dialog` 四个浮起面。

**修法**：上述五类控件（及 `GroupBox` 的框内面）的行**面**从 `background`
改为 `surface_container`（低阶）/`surface_container_high`（浮起层）。
**这正是 主流声明式实现 `Material` 的观感来源**：`Material.regularMaterial` 相对
页面底色有固定的亮度偏移，于是「浮起来的卡片」在视觉上**真的浮着**。

**判据**：`diff panel.svg panel.light.svg` 的行数 **> 4**（BLUE21 P0-6 的统一判据，
用来证明「它真的响应主题」）；且 `panel.svg` 的面色 ≠ `background`。

### 5.3 P1-9 `inverse_surface` / `on_inverse_surface`：相对页面反转

**修法**：`tooltip`/`toast`/`snackbar`/`notification` 四者读 `inverse_surface`。
**现状**：浅色下三者与背景无从区分（BLUE21 AR3 已记）。

**判据**：浅色态下 `tooltip.svg` 的面色 **≠** `background`（当前相等）。

### 5.4 P1-10 `outline_variant`：列表行分隔线

**修法**：数据表/列表的行分隔线从 `border_color`（= `secondary`）改为 `outline_variant`。
**现状**：焦点环与列表行分隔线是**同一个灰**（BLUE21 AR3 的实测结论）。

**判据**：`table_widget.svg` 的行线色 ≠ 焦点环色。

### 5.5 P1-11 门禁：`check_declared_tokens_have_consumers`

> BLUE21 §6.4 P3-1g 已立 `check_mechanism_has_a_consumer`，
> 但那是**源码级**的「有没有调用点」。本项是**token 级**的：
> 对 `Colors` 的每个字段断言「至少一个生产读取点」，未消费者必须进
> **显式 allowlist 并附理由**（#108 ③ 的形态）。

**这条门禁的价值**：BLUE21 的「7 个角色加了但只有焦点环用一个」这种状态
**从此写不出来**——加 token 就必须同时加消费者，或明写「预留」。

---

## 5A. P1 —— 声明式原语层：BLUE22 §F-11 的「另立计划」落位到这里

> **本节是追加的**（原文只有 §5「层级层」，`blue22.md` §F-11 与
> `blue21.md` §6.5 P4 都写着「另立计划」而**没有那个计划存在**）。
> 本节就是那个计划：把 F-11 的四项原语逐一立项。
>
> **为什么不并入 §5**：§5 是「让已有的 17 个 token 全都有消费者」——改的是**外观**；
> 本节是「给声明式层加它缺的能力」——改的是**能不能表达**。前者能让控件更好看，
> 后者能让**一整类控件根本无法声明**变成可以。两者的失败方式不同，验收判据也不同。

### 5A.0 为什么单独一节，以及它为什么是 P1 而不是 P0

`blue21.md` §6.5 把 E8 归为「**新功能，不混入『修外形错误』**」。那个划分是对的：
BLUE21 那轮的性质是修既有控件的错，混进加能力会让「修了多少缺陷」这个数失真。
本计划的主体（状态层/动效/层级）也是**修与补**，不是加新原语，所以同样不混。

**但它是 P1 而不是 P2**，理由是 `portal` 那一项：

> `diff.rs` 的 `Patch::Insert` 注释写着「**Insertion at the root is not expressible**:
> a document has one root」。于是 `dialog` / `tooltip` / `bottom_sheet` / `popover` 这类
> 「**逻辑上是子节点、视觉上必须脱离父裁剪**」的控件，在声明式层里**无法表达**。
> 这不是「还没写」，是**模型缺一个维度**。

### 5A.1 现状取证（本计划实跑，非引用）

```text
$ grep -rn "children_if" src/ --include=*.rs
 src/view/node.rs:147 pub fn children_if(...) ← 定义
 src/view/node.rs:349 fn children_if_does_not_evaluate... ← 只它自己的单测

$ grep -rn "portal\|Portal" src/view/
 （零命中）

$ grep -rn "on_mount\|on_unmount" src/view/
 （零命中；engine.rs 只有 `fn build(&self) -> Node`）

$ grep -n "pub struct Node" -A 12 src/view/node.rs
 widget / key / props / children ← 只有这四个字段
```

**五项全部确认未做**，且 `children_if` 的「零消费者」在 `blue21.md:175` 记录之后**至今未变**。

### 5A.2 P1-12 `portal`：让「逻辑子节点、视觉脱离父裁剪」可声明

**问题**：声明式层是**纯树**——一个 `Node` 的渲染位置由它在树里的位置决定，
并被父的裁剪矩形约束。而对话框/浮层/提示气泡**在语义上是某个控件的子**（“这个按钮弹出的菜单”），
**在视觉上必须画在祖先的裁剪之外**。这两个要求在一个纯树里是矛盾的。

**三家怎么解**：

| 参考 | 机制 | 出处 |
|---|---|---|
| 声明式 Web 框架 | `createPortal(children, container)` —— 在**树里**留在原位，在 **DOM 里**挂到别处 | `react-dom` |
| 主流 material 实现 | `Overlay` + `OverlayEntry`（`Overlay.of(context).insert(entry)`） | `overlay.dart` |
| 主流声明式实现 | `.overlay` / `.sheet` / `.popover` —— 修饰符携带，渲染由系统另开层 | `View.overlay(...)` |
| 参考工具包 | `Popup` 自带 `parent` 与 `Overlay.overlay`；`QQuickPopup` 用 `parentItem` 定位而 `z` 另算 | `qquickpopup.cpp` |

**四家一致**：**身份在树里、渲染在另一处**。这正是本仓缺的那一维。

**修法（最小可分步形态，不引入新树）**：

1. `Node` 加一个可选的**挂载域**字段（名字待定：`layer` 或 `host`），语义是
 「我属于这棵树的这一支（身份/上下文），但请把我创建到指定的**宿主层**」。
2. `Patch::Insert` 的 `parent` 因此可以是**宿主层节点**而不是祖先——
 `diff.rs` 现有注释说的「根不可插入」保持不变，因为宿主层**不是根**，它是一个
 由引擎拥有的、位于根之上的兄弟层。
3. 与已有的 3 个层（`dialog`/`tooltip`/`popover` 现在各自怎么画）**共存**：
 本项只加**声明路径**，不改已有命令式路径。

**判据**：

```text
1. 单测：一个 portal 声明的子节点，其 WidgetId 出现在宿主层，其 parent 指向声明处
2. 快照：portal 节点的 ink **不被**其声明父的裁剪矩形截断
 （做法：声明父故意设成 40x20，portal 内容 120x60，断言内容满幅出现）
3. 生命周期：portal 节点随声明父卸载而卸载（不泄漏）
```

**反向注入**：把 `layer` 字段的传递拆掉 ⇒ 判据 2 变红（内容被裁剪）。

### 5A.3 P1-13 生命周期钩子：`on_mount` / `on_unmount`

**问题**：`View::build` 是**纯函数**（`engine.rs:48`），一次构建产生一棵 `Node` 树，
没有任何「这个节点刚刚进入渲染树 / 即将离开」的通知点。于是：

* 「挂载时启动一个轮询」「卸载时取消订阅」这类事**无处可写**；
* `barcode_scanner` 的相机生命周期（`blue23.md` §A.8 记它缺「暂停/恢复」）
 在声明式层里正是缺这个钩子。

**三家怎么解**：a web UI framework `useEffect(cleanup)`；主流 material 实现 `initState`/`dispose`；主流声明式实现 `.task`/`.onDisappear`。

**为什么不是给 `View` 加 `&mut self` 方法**：那些钩子写的是**副作用**（订阅、定时器、句柄），
而 `build` 的纯函数性质是 `diff` 能工作（同一输入同一输出）的**前提**。
把副作用塞进 `build` 会让 diff 不再可预测——这是本仓已有的克制（`view/mod.rs` 把动画划给
`PropertyAnimation`，同一个理由），**必须保持**。

**修法**：`Node` 携带一对**回调**（而非在 `build` 里执行副作用），由 `engine` 在
**应用完 patch 之后**调用：

```text
Node::on_mount(f) // f 在节点首次进入已应用树后调用一次
Node::on_unmount(f) // f 在节点被移除后调用一次；与 on_mount 配对，必调
```

**判据**：

```text
4. 单测：节点插入后 on_mount 恰好调用 1 次；重建（key 未变）不重复调用
5. 单测：节点移除后 on_unmount 恰好调用 1 次；且**先于**其子树被丢弃
6. 单测：on_mount 里拿到的 WidgetId 可用（控件已创建）
7. 门禁：grep 证明没有 on_mount 在 build 阶段被调用（保持 build 纯净）
```

**反向注入**：把卸载路径的 `on_unmount` 调用删掉 ⇒ 判据 5 变红。

### 5A.4 P1-14 上下文传播：`Node` 的第五个维度

**问题**：`Node` 只有 `widget/key/props/children`。要么把配置逐层显式往下穿
（在**每一层**每个节点上重复写同一个值——这正是 a web UI framework 在 `createContext` 之前的状态），
要么无法表达「整棵子树共享一个值」。

**三家**：a web UI framework `createContext`/`useContext`；主流 material 实现 `InheritedWidget.of(context)`
（它明确是「沿树向上找最近的 provider」）；主流声明式实现 `@Environment`。

**修法（只加读取，不加写入）**：

1. 上下文值由**根 `View` 提供**（一个 `HashMap<TypeId/String, CapabilityValue>`）；
2. `Node` 在 **build 时**通过一个 `&Context` 参数读取，**解析成具体 prop**；
3. 于是 `diff` 看到的仍是一棵**值已固化**的普通树——**上下文不进入 diff**。

> 第 3 条是关键克制：让上下文**在 build 期解析完**，而不是让每个节点持有一个
> 「向上查找」的动态引用。后者会让 diff 需要上下文才能比较，把一个纯函数变成有环境依赖的函数。

**判据**：

```text
8. 单测：同一声明在 context 值改变前后，产生的 Node::props 不同（证明生效）
9. 单测：context 不出现在 Node 的序列化/比较键里（diff 不依赖它）
10. 单测：未提供某键时读取返回 None，不 panic
```

### 5A.5 P1-15 错误边界：让一棵子树的失败**不丢整份文档**

**问题**：`engine.rs:186-191` 在 `build`/应用失败时**整份丢弃**，`ViewError` 是扁平类型。
即「第 137 个控件写错了名字」与「整个窗口渲染不出来」**完全同一后果**。

**三家**：a web UI framework `ErrorBoundary`（`componentDidCatch`）；主流 material 实现 `ErrorWidget.builder`
（单个 widget 画成红框而页面仍在）；主流声明式实现 无对等物（这本身是一个数据点：
**a web UI framework/主流 material 实现 两家都要，说明这是真需求**）。

**修法**：

1. `ViewError` 携带**失败节点的路径**（`widget` 名 + key + 父链），而不只是一个消息；
2. 应用子树失败时，**只标记该节点为失败**并把失败渲染成一个占位（沿用 主流 material 实现 的做法：
 一个醒目的框 + 控件名），其余兄弟继续应用；
3. 失败集合可查询（类似 `diff` 的 `positional_matches` 计数——**代价可见，而非静默**）。

**判据**：

```text
11. 单测：一棵有 3 个子节点的树，中间那个用非法 widget 名 ⇒
 另两个**仍然出现在渲染树里**
12. 单测：ViewError 含失败节点的 `widget` 名与 key（可定位）
13. 单测：失败节点渲染成占位元素（非零 ink），而不是零尺寸
14. 门禁 check_view_failures_are_local：注入一个坏节点，断言好兄弟仍在树中
```

**反向注入**：把「只标记该节点」改回 `return Err(whole)` ⇒ 判据 11 变红。

> **这条与 `blue22.md` §F-9 的 F-7（`spacer` 丢子树）是同一条原则**：
> 「一个节点表达不出来」不得升级成「整棵子树消失」。F-7 已经在 `spacer` 这一处修好；
> 本项是把**同一条规则推广到任意失败**——冰山法则（#2）的一次完整应用。

### 5A.6 P1-16 `children_if` 的消费者（或删除）

**问题**：`blue21.md:175` 记「`children_if` **除自身单测外零消费者**」。本计划实测**仍然如此**
（§5A.1）。一个写好的原语没有任何真实用法，只有两种可能：
**要么它该被用上，要么它不该存在**。

**修法（二选一，**不留悬空**）**：

| 选项 | 做法 | 何时选它 |
|---|---|---|
| **A. 加消费者** | 在 `code_editor` / `query_builder` / `diff_viewer` 三处挑一处，把现有的命令式「按条件包含子节点」改成 `children_if` | 如果确实存在这种写法 |
| **B. 删除** | 删掉 `node.rs:147` 与它的单测 | 如果没有——那就该承认它是**投机性 API**（原则 #22）|

**判据**：

```text
15. 选 A：`grep -rn "children_if" src/ | grep -v node.rs` **必须非空**，
 且该调用点有单测证明条件为 false 时子节点不出现
16. 选 B：`grep -rn "children_if" src/` **恰好为零**
17. 无论 A/B：本节的这条不能留成「已知零消费者但保留」（那正是它现在的问题）
```

> **`code_editor` 语法配色（`SyntaxPalette::default()` 整套浅色）** 不放在本节：
> 它是**外观**问题（M4「字面量 → token」的同一形态），已在 §A.8 的第 1064 行
> 与 `blue21.md` P4-4 逐条登记，落点在批 6。**本节只管「能不能表达」。**

### 5A.7 批次归属与前置

| 项 | 批次 | 前置 | 为什么这样排 |
|---|---|---|---|
| **P1-12 `portal`** | **批 7** | 无 | 只有它**解锁新表达**，且它会让 `dialog`/`tooltip` 有声明式路径，**与 §5.1 的 `scrim` 天然相邻** |
| **P1-13 生命周期钩子** | 批 7 | P1-12（同改 `Node`+`engine`） | 同一次 `Node` 结构变更里做，避免两次改同一个结构体 |
| **P1-14 上下文** | 批 8 | 批 7 | 需 `Node`/`engine` 稳定后再加 `&Context` 参数（**它改 `View::build` 签名**，影响面最大）|
| **P1-15 错误边界** | **批 7** | 无 | 与 `portal` **无依赖**，可与它并行；`blue22` F-7 已证明这条规则的收益 |
| **P1-16 `children_if`** | **批 8** | P1-14（需先确定上下文能否当消费者） | 最后做，因为「加消费者」还是「删除」取决于前面加完后的形态 |

```text
批 7 = P1-12 portal + P1-13 生命周期 + P1-15 错误边界
 （三者都改 Node/engine，一次改完；顺序：先 15（最独立）→ 12 → 13）
批 8 = P1-14 上下文 + P1-16 children_if 裁定
```

### 5A.8 风险

| # | 风险 | 缓解 |
|---|---|---|
| 1 | **`Node` 加字段会改动 diff 的比较键** | 新字段全部**不参与** diff 比较（§5A.4 判据 9 明确断言）；`children_if` 是 **build 期**展开的，产生的树里没有它 |
| 2 | **`portal` 引入「第二个根」，动摇「一份文档一个根」** | `Patch::Insert` 的「根不可插入」**保持不变**；宿主层是引擎拥有的、位于根**之侧**的兄弟层，不是新的根。判据：`diff` 仍然只对根做一次 |
| 3 | **生命周期钩子让 `build` 不再纯** | 钩子是**携带**的回调，由 `engine` 在 patch 应用**之后**调用；§5A.3 判据 7 用 grep 钉住「build 阶段无钩子调用」 |
| 4 | **上下文传播改 `View::build` 签名 ⇒ 破坏所有现有 `impl View`** | 排在**批 8**（最后），且提供默认上下文参数；判据：改动后 `cargo test` 全绿 = 签名变更已完成迁移 |
| 5 | **错误边界可能掩盖真错误**（把 panic 变成红框） | 失败集**可查询且计数**，与 `diff` 的 `positional_matches` 同一形态：**代价可见，而非静默**（BLUE22 §9.2 的克制）|
| 6 | **本节四项是「新功能」，可能无限膨胀** | 每项都有**明确的停止线**：`portal` 只加声明路径不改命令式；生命周期只有 mount/unmount 两个；上下文**只读不写**；错误边界**只局部化不重试** |

### 5A.9 本节判据汇总（可直接接续）

```text
--- portal（§5A.2）---
1. 单测：portal 子节点的 WidgetId 在宿主层，parent 指向声明处
2. 几何：portal 内容不被声明父的裁剪矩形截断
3. 生命周期：声明父卸载 ⇒ portal 节点随之卸载

--- 生命周期（§5A.3）---
4. 单测：插入后 on_mount 恰好 1 次；key 未变的重建不重复
5. 单测：移除后 on_unmount 恰好 1 次，且先于子树丢弃
6. 单测：on_mount 里拿到的 WidgetId 可用
7. 门禁：build 阶段无钩子调用（grep 证明）

--- 上下文（§5A.4）---
8. 单测：context 值改变 ⇒ 产生的 props 不同
9. 单测：context 不进 diff 的比较键
10. 单测：缺键返回 None，不 panic

--- 错误边界（§5A.5）---
11. 单测：坏子节点不影响好兄弟（三者中中间坏 ⇒ 另两个在树里）
12. 单测：ViewError 含失败节点的 widget 名与 key
13. 单测：失败节点渲染成非零 ink 的占位
14. 门禁：check_view_failures_are_local 反向注入变红

--- children_if 裁定（§5A.6）---
15. 选 A：grep 非空且有条件为 false 的单测
16. 选 B：grep 恰好为零
17. 不存在「已知零消费者但保留」的第三态
```

> **本节不设「静止态快照逐字节不变」的判据**（§9 第 24 条），因为本节**不改任何控件的绘制**：
> 它加的是声明式层的表达能力。如果某项改动**意外**让快照变了，那说明它越界了 ——
> 这一点本身就是一条隐式判据。

---

## 6. 参考工具包的标记语言 / 主流 material 实现 三方对标：**本计划要抄的机制**（按性价比）

| # | 机制 | 出处 | 为什么值得抄 | 落点 |
|---|---|---|---|---|
| 1 | `hovered` 由**基类**维护，`pressed` 由按钮类维护 | `reference-toolkit control.cpp:2043-2056` + `reference-toolkit abstractbutton.cpp:179` | 一处接通，195 个控件同时获得 hover | **P0-1** |
| 2 | `hoverMoveEvent` 用 `contains(point)` 复核 | `reference-toolkit control.cpp:2050` | 指针**移出但仍收到事件**（如拖拽中）时 hover 必须清除 | P0-1 |
| 3 | `hoverLeave` 无条件清 hover | `reference-toolkit control.cpp:2056` | 与上一条**两条都要有**：只做 move 复核，快速移出会漏 | P0-1 |
| 4 | `setPressed` 幂等（同值直接 return） | `reference-toolkit abstractbutton.cpp:726-727` | 移动时每帧都调 `setPressed`，不幂等会每帧发信号 | P0-1 |
| 5 | `pressXChanged`/`pressYChanged` 仅在**模糊比较**不同时发 | `reference-toolkit abstractbutton.cpp:140-147` | `qFuzzyCompare` 防浮点抖动导致的重绘风暴 | P0-1 |
| 6 | 前向/反向**各自一条曲线** | `toggleable.dart:159-163` | `easeIn` 进 `easeOut` 出，是「自然」的来源 | P0-6 |
| 7 | 动画控制器以**当前值**初始化 | `toggleable.dart:156,171,180` | 中断不重启——**这是「丝滑」与「卡顿」的分界** | P0-4/6 |
| 8 | 停用器件时**主动清除**瞬时态 | `button_style_button.dart:359-362` | 禁用后不残留 pressed（本仓 `Button::state()` 已如此，需推广） | P0-1 |
| 9 | 时长阶梯（short/medium/long × 4） | `motion.dart:28-148` | 本仓 `Motion` 只有 fast/normal/slow 三档，**够用则不扩**（见 §7.1） | P0-4 |
| 10 | `Material` 分层（同一面上叠亮度台阶） | 主流声明式实现 公开语义 | 本仓 `surface_container*` 已备；**这是「不像 主流声明式实现」的最大单一原因** | **P1-8** |
| 11 | 模态遮罩是**固定的暗色**，不随前景色走 | Material `Colors.black54` / UIKit | 三家都朝暗走，本仓朝 ink 走（B23） | **P1-7** |
| 12 | 层的**投影片**由阴影承担，而非描边 | `Card`/`Dialog` elevation | 本仓 `Shadow` 已在 `role_base_style`（`manager.rs:325-329`）里给**每个**控件发**同一个**阴影 —— 于是 elevation 不能区分层级 | P1-8 |

### 6.1 明确**不抄**的（避免引入不需要的负担）

| 项 | 为什么不抄 |
|---|---|
| `WidgetState` 改成 `Set` | 会动公开形状（#21），且让「状态间过渡」组合爆炸（§2.3）。**取 参考工具包 的「填充/描边两条独立通道」形态** |
| Material tonal palette / `fromSeed` | 需 HCT 色彩空间与 9 变体生成器；本仓 `Colors` 是名字驱动的（BLUE22 §5.1 已裁定） |
| `InkWell` 水波纹 | 需独立墨迹层与裁剪；本仓是立即模式绘制。**悬停/按下用 `Color.blend` 即可**（参考工具包的标记语言 Basic 就是这么做的） |
| 参考工具包 的 `SmoothedAnimation { velocity }` | 速度制与时长 token 体系冲突；本仓已选时长制（§3.4） |
| 参考工具包 全套 `-1`/`+inf`/`NaN` 未设惯例 | 那是 `Option<T>` 被实现八遍（BLUE22 §B.6.1 已裁定一次） |
| 把 `motion` 扩成 12 档 | 三档用得上；加档但没有语义区分就是「数字摆设」（#104） |

---

## 7. 施工顺序与批次（每批 = 一轮）

> 排序依据：**状态层是所有动效的前置**（动效的目标值来自状态），
> **总线是所有动效的载体**，**层级层可完全并行**。

| 批次 | 内容 | 前置 | 交付判据 |
|---|---|---|---|
| **批 1** | **P0-1 状态来源上提**（`BaseWidget` 三字段 + `widget_state` 默认实现） | 无 | §2.2 的 5 条判据全过；`grep` 第二处 `hovered: bool` 归零 |
| **批 2** | **P0-3 状态覆盖写进预设**（`generate.sh` + 6 个状态键） | 批 1 | §2.4 的 4 条；快照**不变**（预设不覆盖静止态） |
| **批 3** | **P0-4/P0-5 动效总线**（`Widget::tick` 上提 + `tick_animations`） | 无（可与批 1 并行） | §3.3 的 6 条；A 档（按钮）三帧几何互异 |
| **批 4** | **B/C/D/E/F 档接入** | 批 3 | 每档一条三帧判据；`spinner`/光标闪烁可见 |
| **批 5** | **P1-7～P1-10 层级层**（scrim / surface / inverse / outline_variant） | 无（**完全并行**） | §5 逐条判据；`diff x x.light` > 4 行 |
| **批 6** | **P1-11 门禁** + §4 的视觉遗留（`group_box` 勾 / `dialog` 按钮行 / `scroll_area` / `meter` / `chart` 轴） | 批 1 | 门禁反向注入变红；6 条快照逐条评审 |
| **批 7** | **§5A 声明式原语（一）**：P1-15 错误边界 → P1-12 `portal` → P1-13 生命周期钩子 | 无（可与批 5/6 并行） | §5A.9 判据 1–7、11–14；三条门禁反向注入变红 |
| **批 8** | **§5A 声明式原语（二）**：P1-14 上下文传播 + P1-16 `children_if` 裁定 | 批 7 | §5A.9 判据 8–10、15–17；`View::build` 签名变更后全量测试仍绿 |
| **收尾** | 全量：`cargo test` + `clippy -D warnings` + 5 profile + `run_all_gates.sh` + 快照再生 | 全部 | §9 |

> **批 7/8 的排序理由**：批 7 三项都改 `Node`/`engine`，**一次改完**才不用两次动同一个结构体；
> 批 8 改 `View::build` 签名（影响面最大）并做 `children_if` 的最终裁定（它取决于批 7 加完后的形态）。
> 详见 §5A.7。

> **只在收尾跑一次全量**（原则 #55/#56）；每个门禁**必须反向注入证明它会红**。

---

## 8. 风险与克制

| # | 风险 | 缓解 |
|---|---|---|
| 1 | **`tick_animations` 变成每帧推进 195 个控件** ⇒ 静止窗口也在烧 CPU | `is_animating()` 是**每个控件自己的答案**，默认 `false`；判据 §3.3-2 **专门断言静止时零成本**。这条比动画本身更需要判据 |
| 2 | **hover 状态变更导致重绘风暴**（鼠标移动每像素一次） | 逐帧**只在状态真变时**置脏（参考工具包 `setPressed` 幂等 + `qFuzzyCompare`，§6 表 #4/#5）；门禁：统计一次鼠标横扫的重绘次数 |
| 3 | **快照大面积变化** | 批 2 与批 5 **预期**快照变化（这是产物）；但**每批只改一类**，diff 才可评审（BLUE22 §9.2 同一克制） |
| 4 | `BaseWidget` 加重载字段影响 `mini`/`embedded` 体积 | 三个 `bool` = 3 字节；`mini` 无动画模块，`is_animating()` 恒 false。**判据：五个 profile 全部编译 + 实测体积增量入交付物** |
| 5 | **`hovered` 与图表的「数据项 hover」同名冲突**（现有 18 处提及） | 批 1 的判据 4 逐处裁定：图表的 `hovered` 是**数据选择**，改名 `hovered_item`/`hovered_level`（语义不同，**不得共用**） |
| 6 | 状态覆盖写进预设 = **改变所有用户的外观** | 只在 `overrides.styles` 加**交互态**，静止态一字不动 ⇒ **不带 `.light`/状态键的快照逐字节不变**，这条本身就是判据 |
| 7 | `dialog` 的按钮行 BLUE22 刻意未合并 | **本计划也不合并**（§4.2 只改行盒），尊重 BLUE22 §5.4 的裁定 |
| 8 | 「丝滑」是主观词，无法验收 | 全计划统一为**三帧几何判据**（§0.3）——对数值采样，可做门禁，可反向注入 |

---

## 9. 验收判据（全计划共用）

```text
--- 状态层（§2）---
1. grep -rn "fn widget_state" src/widget/ | wc -l → > 1（控件的覆写），
 且默认实现能回答 Hover/Pressed
2. 单测：CheckBox（零覆写）在 MouseEnter 后报告 Hover
3. grep -rn "hovered: bool" src/widget/ → 仅 BaseWidget 一处（图表项已改名）
4. themes/{dark,default}.json 的 overrides.styles 含 ≥ 6 个 "kind:state" 键
5. 单测：hovered Button 的填充 ≠ 静止态填充
6. 门禁 check_declared_tokens_have_consumers：17 个颜色角色各有生产读取点
 （未消费者在 allowlist 中附理由）

--- 动效层（§3）---
7. 单测：Button hover 后 is_animating()==true，200ms 后 false
8. 单测：静止控件 is_animating()==false，tick_animations 返回 false（零成本）
9. 几何：switch 在 t=0/150/300 三帧拇指 x 互异且单调
10. 几何：button 在 t=0/100/200 三帧填充色互异
11. 门禁 check_animation_has_a_driver：每个 impl tick 的控件可被总线推进
12. 门禁 check_transition_durations_are_tokens：控件自驱动时长来自 theme.motion

--- 层级层（§5）---
13. panel.svg 的面色 ≠ background；diff panel.svg panel.light.svg 行数 > 4
14. 浅色 tooltip.svg 面色 ≠ background
15. 暗态模态遮罩亮度 < 它覆盖的面
16. 表格行线色 ≠ 焦点环色

--- 视觉遗留（§4）---
17. group_box 勾色 = 框填充的对比色；产出 group_box_checked 快照
18. meter 刻度与弧同相（tick0 外端在弧首顶点的角上）
19. chart/金融四图空态画轴 + "No data"

--- 声明式原语（§5A）---
26. portal：子节点 WidgetId 在宿主层、parent 指向声明处；内容不被声明父裁剪
27. 生命周期：on_mount/on_unmount 各恰好 1 次；key 未变的重建不重复；build 阶段无钩子
28. 上下文：值改变 ⇒ props 变；不进 diff 比较键；缺键返回 None 不 panic
29. 错误边界：坏子节点不影响好兄弟；ViewError 含 widget 名 + key；失败节点渲染成占位
30. children_if 裁定：grep 非空（选 A）或恰好为零（选 B），**不存在第三种状态**
31. 门禁 check_view_failures_are_local 反向注入变红

--- 回归（全计划）---
20. cargo test --no-default-features --features desktop → 0 failed
21. cargo clippy --no-default-features --features desktop --all-targets -- -D warnings → 0 warning
22. desktop/tablet/mobile/mini/embedded 五个 profile 全部 Finished
23. bash tools/run_all_gates.sh → FAIL=0（每条新门禁均已反向注入）
24. 静止态快照（不含状态键的 .svg）逐字节不变 —— 证明批 2 没有改静止外观
25. mini + 三个 bool 字段的二进制体积增量实测入交付物（≤ 64 B）
```

> **第 24 条是本计划的「安全绳」**：它证明新增的状态/动效/层级**没有**动到用户已经确认的
> 静止外观——这正是 BLUE22 交付的东西必须被保住的形态。

---

# 附录 A — 180 个控件逐组逐条审计（**用户指令：按分组，180 个控件都要检查改进**）

> **取证方式**：本附录的每一行都由**工具实跑**得出，不是人工阅读的印象：
>
> | 列 | 来源（可复跑） |
> |---|---|
> | 控件名 | `src/widget/capability/properties.rs` 的 `canonical_name`（**188 条**） |
> | 所属组 | `tools/generate_control_index.py` 的 `control_families()`（按**实现模块**分组，非手写表） |
> | 源文件 | 同上工具的 `constructor_targets()` → `declared_types()` → `source_families()` 三级解析 |
> | `字面量` | 该文件内 `Color::rgb(`/`Color::rgba(` 的**计数**（`tools/audit_appearance.py`） |
> | `style` | 该文件是否读 `style.{background,text,border}_color` 或 `resolved_theme_style` |
> | `tick` | 该文件是否有 `pub fn tick(`（即**已有动画状态机**） |
>
> ```text
> $ python3 tools/audit_appearance.py
> === files with a Draw impl: 182 ===
> === total colour literals in Draw files: 684 ===
> === Draw files reading no style colour, only literals: 66 (36%) ===
>
> $ python3 tools/audit_text_y.py → suspicious placements: 0 ← BLUE21 A 组已闭合
> $ python3 tools/audit_text_contrast.py → every glyph meets the 4.5:1 AA floor ← BLUE21 B 组已闭合
> $ python3 tools/check_control_has_tests.py → 186 / 186 每个控件都有测试
> $ python3 tools/audit_kind_sharing.py → 13 个 kind 被多个控件共用
> $ python3 tools/audit_control_gaps.py → candidate gaps: 25（见 §A.9）
> $ python3 tools/check_mechanism_has_a_consumer.py
> AnimationDriver: unconnected, acknowledged — BLUE21 P0-4 (AR7) — drive it from the widget tick
> AnimationGroup: unconnected, acknowledged
> ```
>
> **本附录与 §2–§5 的关系**：§2–§5 是**机制层**的修法（一处接通 N 个控件）；
> 本附录是**控件层**的落位表（每个控件**具体缺什么、改什么、怎么验**）。
> 两者是「同一件事的两个视角」——§2 决定**用什么方法**，本附录决定**对哪个控件做**。

## A.0 分组统计（实跑，非估算）

| 组 | 控件数 | 色彩字面量 | **只读字面量、完全不读 style** | 已有 `tick` | 有 `handle_event` | 有 `size_hint` |
|---|---:|---:|---:|---:|---:|---:|
| Base controls（`base_widgets`） | 7 | 65 | **0** | 1 | 7 | 7 |
| Input controls（`input_widgets`） | 26 | 96 | **2** | 3 | 26 | 25 |
| Display controls（`display_widgets`） | 25 | 92 | **5** | 3 | 25 | 25 |
| Containers（`container_widgets`） | 13 | 46 | 0 | 0 | 13 | 13 |
| Navigation（`nav_widgets`） | 7 | 8 | 0 | 0 | 7 | 7 |
| Menus and toolbars（`menu_toolbar`） | 8 | 21 | 0 | 0 | 8 | 8 |
| Dialogs（`dialog`） | 14 | 32 | 0 | 0 | 14 | 14 |
| Overlays（`overlay_widgets`） | 4 | 10 | 0 | 0 | 4 | 4 |
| Views（`view_widgets`） | 14 | 18 | 0 | 0 | 14 | 14 |
| Charts（`chart_widgets`） | 4 | 1 | **1** | 0 | 1 | 1 |
| **Specialised（`special_widgets`）** | **32** | **140** | **6** | 1 | 32 | 26 |
| Advanced（`advanced_widgets`） | 8 | 50 | 0 | 0 | 8 | 8 |
| Media and web（`media_widgets`） | 7 | 62 | **3** | 5 | 7 | 7 |
| Miscellaneous（`misc_widgets`） | 8 | 9 | 0 | 0 | 8 | 8 |
| **Cupertino（`cupertino`）** | **8** | **79** | 0 | 0 | 8 | 8 |
| Web（`web_widgets`） | 1 | — | — | — | — | — |
| Core（`root`） | 2 | 12 | 0 | 0 | 2 | 2 |
| **合计** | **188** | **742** | **17**（文件级）/ **66**（Draw 文件级） | 11 | 188 | 182 |

**三条从这张表直接读出的结论**：

1. **`special_widgets` 一个组占 140 个字面量（全仓 21%）**——32 个控件里 6 个完全不读主题。
 这与 `audit_appearance.py` 的「66 个 Draw 文件只读字面量」是同一事实的两种视角。
2. **`tick` 只有 11 个**，而其中 **5 个集中在 `media_widgets`**（视频/动图/Lottie/Rive/Hero）——
 即**媒体组的动画最完整，交互控件的动画最少**。这个分布本身就是缺陷的形状：
 一个按钮的 hover 反馈比一个 Lottie 播放器更常被用户看到。
3. **`handle_event` 188/188 齐全、`size_hint` 182/188**（缺的 6 个在 `special_widgets`）——
 说明 BLUE22 的度量体系覆盖已近完整，**本计划不缺「静态正确性」的地基**。

## A.1 通用修法代号（每组直接引用，不再重复长文）

> 每一组/每一个控件的「改进方式方法」列引用这些代号。代号定义**只在这里出现一次**
> （#2：修一处扫同类；#51：修复量随层下降）。

| 代号 | 名称 | 具体做法 | 判据 | 计划出处 |
|---|---|---|---|---|
| **M1** | 状态接线 | 依赖 `BaseWidget` 的 `hovered`/`pressed`/`grabbed`/`focus_reason`（**零控件代码**，自动获得）；控件若已有私有同名态，**删除私有字段改读 base** | 该控件在 `MouseEnter` 后 `widget_state()==Hover` | **§2.2** |
| **M2** | 状态覆盖上妆 | 预设主题加 `"<kind>:hover"`/`":pressed"`/`":disabled"`/`":checked"`/`":error"` | 主题覆盖后该控件 hover 填充 ≠ 静止填充 | **§2.4** |
| **M3** | 动效接线 | 已有 `tick` 的：改为 `impl Widget::tick`（1 行）；**无 `tick` 的**：新增一个 `Transition`/`CursorBlink` 字段 + `tick` | 三帧几何互异且单调（§0.3） | **§3.3/§3.4** |
| **M4** | 字面量 → token | 把 `Color::rgb(…)` 换成解析后的主题值；**同一字面量跨文件时先抽共享函数**（冰山法则） | `audit_appearance.py` 该文件计数字面量下降；明暗两态快照 diff > 4 行 | **§5**、BLUE21 R3 |
| **M5** | 层级上妆 | 面改读 `surface_container`（低阶）/`surface_container_high`（浮起）；遮罩读 `scrim`；反转面读 `inverse_surface` | 面色 ≠ `background`；暗态遮罩亮度 < 被覆盖面 | **§5.1–5.3** |
| **M6** | 分隔线上妆 | 行分隔线/内容描边从 `border_color` 改读 `outline_variant`（焦点环仍用 `outline`） | `diff` 显示行线色 ≠ 焦点环色 | **§5.4** |
| **M7** | 组合改组装 | 子控件几何改由 `CompositeBuilder` + `Layout::arrange` 推导（BLUE22 已建，剩余控件逐个迁移） | 单测：子控件变宽 ⇒ 相邻段起点移动（BLUE22 F.2.2 的共同判据） | BLUE22 §B.8 |
| **M8** | RTL 接线 | 方向敏感几何/取值改用 `TextDirection`；**两方向的映射必须共用同一个 inset**（BLUE21 §4.3 的教训） | 往返测试：`value → x → value` 一致 | BLUE22 F-4 |
| **M9** | a11y 填充 | 实现 `a11y_state()`，填 `value`/`checked`/`mixed`/`selected` | 屏幕阅读器可读出位置/三态 | BLUE22 F-5 |
| **M10** | 契约加厚 | 补齐该控件「声明了但读不回 / 该有没有」的属性与事件 | 逐条可读回；`get` 返回 ∈ `accepted_tokens` | BLUE22 F-6 |
| **M11** | 焦点环统一 | 复用 `FocusRing::for_control` + `visual_focus()`（指针焦点不画环） | `click` ⇒ 无环；`Tab` ⇒ 有环 | BLUE22 P0-5 |
| **M12** | 空态画 chrome | 空数据时仍画轴/网格/提示（不是 `return`） | 空态快照含轴元素 + `No data` | §4.6 |

**优先级约定**：**P0** = 用户每天看到且现在是错的/硬的；**P1** = 一致性；**P2** = 卫生。

---

## A.2 Base controls（`base_widgets`，7 个）

> 本组是**其他 181 个控件的模板**：这里定义的写法会被复制到各处。
> 因此本组**必须最干净**，且**每个控件都要示范 M1（状态）与 M3（动效）**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `button` | `button.rs` | 4 | ✅ 唯一已有完整交互过渡的控件（`interaction_progress` + `tick`，4 条单测） | ① 私有 `pressed`/`hovered`/`focus_reason` 上提（§2.2 的**原型**）② 硬编码 `Color::rgb(240,240,240)` 三态回落改 `theme` ③ `visual_focus` 上提 | **M1**（原型）**M2** **M4** **M11** | **P0** |
| `toggle_button` | `toggle_button.rs` | 8 | 有 `pressed` 字段但 **`draw` 不读**（BLUE21 AR6 原样）；`ToggleButtonState` 只有 3 态（Normal/Checked/Disabled），**枚举本身缺 Hover/Pressed/Focused** ⇒ draw 无法表达 | ① 状态枚举补 3 态 ② 接 M1/M2 ③ 加 `tick`（M3） | **M1** **M2** **M3** **M4** | **P0** |
| `check_box` | `checkbox.rs` | 0 | `MousePress` 臂**完全不看 `pos`**（控件矩形内任意点都切换）；无 hover/focus/error 态（主流 material 实现 有 9+ 组合） | ① 补 `:checked`/`:error` 覆盖（`colors.error` 至今**零消费者**）② 命中改 `contains_point_with_touch_expansion` ③ 勾色取**框填充的对比色** | **M1** **M2** **M9**（三态 `checked`/`mixed`） | **P0** |
| `radio_button` | `radiobutton.rs` | 0 | ✅ 几何已修（`r8` + `r5` 点，快照已证）；有 `visual_focus` | ① 接 M1/M2 ② a11y 补 `checked` ③ 组内互斥语义已有则保持 | **M1** **M2** **M9** | P1 |
| `label` | `label.rs` | 12 | 12 个字面量（纯文本控件却有色彩常量）；`swipe_to_dismiss` 曾被解析到此文件（分组工具的边界） | ① 文字色改读 `style.text_color`（12 → 0）② 与 `swipe_to_dismiss` 拆清 | **M4** | P1 |
| `frame` | `frame.rs` | 29 | **本组字面量最多（29）**；虽有七种形状（含 `WinPanel`，BLUE21 A.7 记为优势）但两色斜角是字面量 | ① 斜角色走 `border_color` 的**派生**（`frame.rs:196-208` 已有范式，扩展到 29 处）② 保留七形状（**优势不动**） | **M4** | P1 |
| `swipe_to_dismiss` | `overlay_widgets/swipe_to_dismiss.rs` | 3 | 无动画驱动（滑出是硬切） | ① 加 `tick`（跟 `dismiss_threshold` 的百分比）② 背景色接 `style` | **M3** **M4** | P1 |

> **本组的关键一点**：`button` 已经是 M1 的**原型**（它有那两个字段与那条过渡，只是做在子类）。
> 所以批 1 的工作是**把它上提到 `BaseWidget`**，然后**本组其余 6 个控件自动获得** M1。
> 这是「修一处、扫同类」最有价值的一次应用。

---

## A.3 Input controls（`input_widgets`，26 个）

> 本组是**用户与软件对话的地方**：焦点态、校验态、光标闪烁缺一不可。
> 实测：**26 个控件里只有 3 个有 `tick`**（`line_edit`/`tag_input`/`inplace_editor`），
> 而**这些 `tick` 又都没有驱动者**（§1.3）⇒ **26 个输入控件的文本光标全都不闪**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `line_edit` | `lineedit.rs` | 3 | 有 `tick`（光标闪烁）**无驱动者**；缺整个**装饰槽模型**（`prefix`/`suffix`/`helper`/`error`/`counter`，BLUE21 A.4.1） | ① 接 M3（一行）② 补 `:error` 覆盖 + `error_text` ③ `caret_x` 用 `cursor_position` 的字节前缀（BLUE21 A.4.2 的**功能**缺陷） | **M3** **M2** **M10** | **P0** |
| `spin_box` | `spinbox.rs` | 5 | ✅ 已由 `CompositeBuilder` 组装（BLUE22 第 70 轮，快照几何逐字节不变） | ① 接 M1（步进按钮 hover）② RTL padded 镜像（参考工具包 `reference: padding derived from the sibling's own width`） | **M1** **M8** | P1 |
| `number_picker` | `number_picker.rs` | 1 | 同 `spin_box` 但未接组装 | 接 **M1** + **M7** | **M1** **M7** | P2 |
| `combo_box` | `combobox.rs` | 3 | ✅ 指示器已由组装推导（BLUE22）；无 hover 态 | **M1** **M2** **M8**（镜像时指示器 padding 交换） | P1 |
| `editable_combo_box` | `editable_combo_box.rs` | 0 | 本仓**优势**（主流 material 实现 `DropdownButton` 不可编辑） | **M1** **M9** | P2 |
| `multi_select_combo_box` | `multi_select_combo_box.rs` | 0 | 本仓**优势**（主流 material 实现 **多选完全没有**） | **M1** **M10**（枚举候选/读高亮） | P2 |
| `font_combo_box` | `font_combo_box.rs` | 3 | 3 个字面量；无状态 | **M1** **M4** | P2 |
| `search_box` | `search_box.rs` | 8 | 8 个字面量；无状态 | **M1** **M2** **M4** | P1 |
| `search_bar` | `search_bar.rs` | 4 | 同族，与 `search_box` 字面量不同步 | **M1** **M4**（与 `search_box` 抽共享） | P2 |
| `text_area` | `textarea.rs` | 4 | **完全不读 style（`style=N`）** ⇒ 无主题响应 | **M4**（4 处）**M2** | P1 |
| `text_edit` | `textedit.rs` | 1 | 1 个字面量 | **M1** **M4** | P2 |
| `rich_edit` | `rich_edit.rs` | 1 | BLUE21 A.6：光标几何用**等宽捷径** `cell_width`，变宽跨度下必然错位 | ① 接 M3（光标闪烁）② caret/range 几何改为**跨度感知**（勿继承等宽） | **M3** **M10** | P1 |
| `masked_edit` | `masked_edit.rs` | 4 | BLUE21 B15：正文字面量 `rgb(33,33,33)` 在暗色字段上对比 **1.35:1**（注：`audit_text_contrast` 现已全绿——**该条已修**，此处仅存 4 个字面量待清） | **M4** **M1** | P2 |
| `otp_input` | `otp_input.rs` | 0 | BLUE21 B19：6 格里 5 格不可见（已修）；本仓**优势** | ① 接 M1/M2 ② `obscuringCharacter = '•'`（主流 material 实现 `text_field.dart:273`） | **M1** **M2** **M10** | P1 |
| `tag_input` | `tag_input.rs` | 7 | 有 `tick` 无驱动者；7 个字面量 | **M3** **M4** **M2** | P1 |
| `keyboard` | `keyboard.rs` | 6 | BLUE21 D20：键帽文字贴顶（**已修**）；仍 6 字面量 + 无按键反馈 | ① 键帽按下**变色 + 动效**（M1/M2/M3）② 字面量 → token | **M1** **M2** **M3** **M4** | P1 |
| `range_slider` | `range_slider.rs` | 4 | ✅ 双向手柄映射正确（BLUE22 §4.3 的参照）；无 hover/drag 发光 | **M1** **M3**（手柄按下放大，主流 material 实现 `slider_parts.dart:678`） | P1 |
| `list_box` | `listbox.rs` | 6 | 6 个字面量；行选中无统一来源 | **M1** **M6**（行线）**M4** | P1 |
| `dropdown` | `dropdown.rs` | 8 | **完全不读 style（`style=N`）** ⇒ 展开面板主题盲 | **M4** **M5**（面板读 `surface_container`） | **P0** |
| `cascader` | `cascader.rs` | 8 | BLUE21 C7：`visible_options_at` 算了 filter 又丢弃（`let _ = needle;`）；本仓**优势**控件 | ① 真过滤或删 `needle`（二选一，**不留悬空**）② 面板读 `surface_container` | **M5** **M10** | P1 |
| `auto_complete_edit` | `auto_complete_edit.rs` | 5 | 只发布 `suggestion_count`（无法枚举候选/读高亮） | **M10**（`optionsBuilder` 等价物）**M5** | P1 |
| `mention` | `mention.rs` | 7 | 7 个字面量；面板无层级 | **M4** **M5** | P2 |
| `command_link` | `command_link.rs` | 3 | **已有 `is_hovered` + `hovered` 信号**（本组少数已做状态的） | ① 改为读 base（M1 去重）② 接 M2 | **M1** **M2** | P1 |
| `inplace_editor` | `inplace_editor.rs` | 2 | 有 `tick` 无驱动者；注释声称 blink 已实现（BLUE21 AR8） | **M3** | P1 |
| `ime_preedit` | `ime_preedit.rs` | 1 | ✅ 属白名单（无自带色带） | **M4**（1 处） | P2 |
| `shortcut_editor` | `shortcut_editor.rs` | 2 | BLUE21 A.4.5a：存**物理键码**而非逻辑 activator ⇒ 换布局即失效；键帽无按下反馈 | ① 改逻辑键集 ② 按下反馈（M1/M2） | **M10** **M1** **M2** | P1 |

> **本组最刺眼的一条**：`line_edit` 的 `tick`（光标闪烁）**已经写好且测试过**，
> 但没有任何东西推进它 ⇒ **用户看到的是一个不闪的光标**。整组 26 个控件同理。
> 这就是 §3 那条总线存在的意义：**一行 `tick_animations`，本组 4 个控件的动画同时活过来。**

---

## A.4 Display controls（`display_widgets`，25 个）

> 本组是**值的可视化**（进度/滑块/开关/评分/图表），也是**动画价值最高**的一组。
> 实测：只有 3 个有 `tick`；**5 个完全不读 style**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `switch` | `switch.rs` | 5 | ✅ 几何已修（`52×32`/`r14`）；**有 `travel` 与 `tick` 但无驱动者** ⇒ 开关**硬切**；`off_track` 已正确读 `theme_derived` | ① **M3（最高价值：一行让开关滑动）** ② 5 字面量清理 | **M3** **M4** | **P0** |
| `slider` | `slider.rs` | 10 | `Slider::mouse_pressed` **存了 `draw` 不读**（BLUE21 AR6）；主流 material 实现 有 5 态（hover/focus/drag 光晕），本仓只有 enable/disable | ① **M1** ② 加 hover/drag 光晕（主流 material 实现 透明度 drag 0.1 / hover 0.08）③ 10 字面量清理 | **M1** **M2** **M4** | **P0** |
| `progress_bar` | `progressbar.rs` | 4 | ✅ 几何已修（`h4 r2`）；**不确定态无动画**（主流 material 实现 1800 ms）；轨道色已改派生态（BLUE21 B8 已修） | ① 不确定态加 `tick`（1800 ms）② `range_slider.rs:405` 的承载面派生已做对，保持 | **M3** **M2** | P1 |
| `progress_circle` | `progress_circle.rs` | 3 | BLUE21 A.3.6：无 `trackGap`（值 0 时弧与轨道**无法区分**） | ① 补 `trackGap = 4`（主流 material 实现 `progress_indicator.dart:1636`）② 不确定态旋转（M3） | **M10** **M3** | P1 |
| `spinner` | `spinner.rs` | 3 | 有 `tick` **无驱动者** ⇒ **转圈不转** | **M3**（一行） | **P0** |
| `rating` | `rating.rs` | 0 | BLUE21 A.4.5d：`Float` 属性**静默取整**（写 3.7 得 4）；半星不可表达 | ① 半星表示 ② `:hover` 预览（M1/M2）③ M9（`value` 朗读） | **M10** **M1** **M9** | P1 |
| `badge` | `badge.rs` | 0 | ✅ 药丸可见性已修（BLUE21 B1）；`badge` 点 r 已固定 | **M4**（确认零字面量） | P2 |
| `avatar` | `avatar.rs` | 1 | ✅ 已由 `AVATAR_SIZE` 固定（曾画半裁圆） | **M4** **M5** | P2 |
| `chip` | `chip.rs` | 0 | BLUE21 A.3.5：曾是方角且不可见（**已修**）；`selected`/`checked` 互斥（BLUE21 F-4 提示） | ① 接 M2（selected 态）② 语义 flag 互斥断言 | **M2** **M10** | P2 |
| `scroll_bar` | `scrollbar.rs` | 4 | BLUE21 B2/B3：滑块曾 = 槽色（已修）、箭头尺寸取长（已修）；主流 material 实现 有**闲置 600 ms 后 300 ms 淡出** | ① 加淡出（`tick` + `Motion`）② 保持 `thumb_metrics` 纯函数（BLUE21 A.7 #10 的**优势**） | **M3** | P1 |
| `meter` | `meter.rs` | 18 | BLUE21 D16：**刻度与自己的弧相差 90°**（漏 `+ offset`）；两端取整不同 ⇒ 45° 刻度 5 px、90° 刻度 6 px | ① 刻度角走**与弧顶点相同**的 `snap_to_grid` ② 18 个字面量 → 共享派生 | **M4** **M10** | **P0** |
| `icon` | `icon.rs` | 3 | **不读 style** ⇒ 图标不随前景色 | **M4**（墨取 `style.text_color`） | P1 |
| `image_view` | `image_view.rs` | 3 | **不读 style** | **M4** **M5**（占位面） | P2 |
| `font_preview` | `font_preview.rs` | 3 | **不读 style** | **M4** | P2 |
| `arc` | `arc.rs` | 3 | **不读 style** | **M4** | P2 |
| `line` | `line.rs` | 1 | 不读 style（1 处） | **M4** **M6** | P2 |
| `divider` | `divider.rs` | 1 | ✅ `DIVIDER_SPACING 16`/th 1 已对齐 主流 material 实现 | **M6**（`outline_variant`）**M4** | P1 |
| `mini_chart` | `mini_chart.rs` | 2 | BLUE21 B14：网格字面量 `rgb(220,220,220)` ⇒ 暗态**最亮的东西是最不重要的网格** | **M4**（用 `charts.rs::axis_chrome()` 一处推导） | **P0** |
| `lcd_number` | `lcd_number.rs` | 1 | BLUE21 C6：`num_digits` 只当**宽度除数**，不右对齐补位（画 1 位却按 6 位布局） | **M10**（按 `num_digits` 补位；`get_segments` 补 `'.'`） | P1 |
| `skeleton_loader` | `skeleton_loader.rs` | 2 | 加载态**无闪烁动画**（外部对标 / 主流声明式实现 都有 pulse） | **M3**（pulse，用 `Motion`）**M5** | P1 |
| `empty_state` | `empty_state.rs` | 1 | ✅ 重叠 28 px 已修（BLUE22 R-5） | **M4** | P2 |
| `roller` | `roller.rs` | 3 | `roller.rs:331` 是 BLUE21 的**正确写法参照**（文本居中） | **M3**（滚轮惯性/对齐动画）**M4** | P2 |
| `floating_label` | `floating_label.rs` | 5 | 有 `tick` 无驱动者（BLUE21 记为**正确范式**）；BLUE21 A.3.9：演示浮动标签的控件**没有**浮动标签（已部分） | **M3**（一行）**M4** | P1 |
| `emoji_picker` | `emoji_picker.rs` | 2 | BLUE21 B20：面板 = 窗口底色 | **M5** **M4** | P2 |
| `color_well` | `color_well.rs` | 0 | 本仓**优势**（外部对标 / Cupertino **都没有**取色器） | **M1**（hover 描边）**M5** | P2 |
| `color_history` | `color_history.rs` | 9 | 9 个字面量；有 hover 记录 | **M4** **M1** | P2 |
| `mini_canvas` | `mini_canvas.rs` | 6 | 6 个字面量；无交互反馈 | **M4** **M1** | P2 |

---

## A.5 Containers（`container_widgets`，13 个）

> 本组是**结构的载体**，也是 BLUE22 §B.8 组装改造的剩余队列。
> 实测：0 个 `tick`、0 个完全不读 style——**主题基础最好的一组**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `panel` | `groupbox.rs` | 6 | **`Panel` 是 `GroupBox` 的 `pub type`**（`kind.rs:80-86`）；其「面」= 窗口底色 ⇒ **无层级** | **M5**（面读 `surface_container`）**M4** | **P1** |
| `group_box` | `groupbox.rs` | 6 | BLUE21 B22：可勾选态**纯黑勾**（暗态最不可读的一笔）；且 `create_group_box` 从不 `set_checkable` ⇒ **快照覆盖不到该状态** | ① 勾取**框填充的对比色** ② 标题占位走 `top_padding = padding + label_h + spacing`（参考工具包 `reference: the title-band reserve: padding + label height + spacing`）③ **产出 `group_box_checked` 快照** | **M4** **M7** **§4.1** | **P0** |
| `tab_widget` | `tabwidget.rs` | 3 | BLUE21 D8：**构造后零 tab**（标题落不下去）；三种 `TabShape` 在 `tab_bar` 里画得一样 | ① 抄 `create_tab_bar` 加两个 tab ② `set` 补 `text`/`title` 分支 | **M7** **M10** | **P0** |
| `scroll_area` | `scrollarea.rs` | 7 | BLUE21 B13：滚动条 **8 处字面量**（同文件 `draw_sticky_band` 已做对） | **M4**（抄自己的 `draw_sticky_band`）**M3**（淡出） | **P0** |
| `splitter` | `splitter.rs` | 2 | `HANDLE_WIDTH` 曾在**两处**各写一遍（已由 `dimensions` 统一）；拖拽**无实时反馈** | **M3**（拖拽中高亮/光标）**M5** | P1 |
| `dock_widget` | `dockwidget.rs` | 0 | BLUE21 D13：24 px 标题栏曾在**两个函数**各写一遍（已提常量）；**无拖出/吸附动画** | **M3**（吸附预览 + 拖动浮影）**M5** | P1 |
| `mdi_area` | `mdiarea.rs` | 3 | BLUE21 A13：子窗口标题曾越过下边框（已修）；窗口**无最小化/还原动画** | **M3** **M5** | P2 |
| `tool_box` | `toolbox.rs` | 11 | BLUE21 D7：竖排 120 px 下 **4 项归零**（`item_rect` 不钳制，越界项画到控件外） | ① 给页面保底宽/高 ② 条带做**溢出出口**（滚动或 more 钮）③ RTL. | **M7** **M10** **M8** | P1 |
| `collapsible_pane` | `collapsible_pane.rs` | 4 | 展开/收起是**瞬时**（无高度动画）；头部高 24 < 主流 material 实现 44/48 | **M3**（高度动画）**M10**（头部高） | P1 |
| `stacked_widget` | `stackedwidget.rs` | 0 | ✅ BLUE21 A.7 #4：禁用时**抑制信号并给出原因**（优势） | **M3**（切页过渡，可选） | P2 |
| `stepper` | `stepper.rs` | 0 | BLUE21 P2-11：本仓 `stepper` = **数值微调器**，主流 material 实现 `Stepper` = **分步向导** ⇒ **缺一整个控件** | 二选一：**改名** 或 **补真正的向导控件**（`StepState` 5 态 + 连接线），**不留悬空** | **M10** | P1 |
| `safe_area` | `safe_area.rs` | 0 | ✅ 四边物理 inset（BLUE21 AR5 记为方向性问题） | **M8**（`start`/`end`） | P2 |
| `masonry_layout` | `masonry_layout.rs` | 6 | BLUE21 D12：**整个 Draw 无 `push_clip`**（不裁剪）；标签用 `draw_text` 非 `draw_text_fitted`，`y = item_y + h/2 - 6` 只对 12 px 字号成立 | ① 加 `push_clip`/`pop_clip` + 丢弃 `y ≥ rect.bottom` ② 标签走 `text_line` + `draw_text_fitted` ③ BLUE21 记 `corner_radius` 死绑定（`4` vs 真画 `6`） | **M10** **M4** | P1 |
| `carousel` | `carousel.rs` | 9 | ✅ BLUE21 A.7 #2：手势释放**距离 ∨ 速度** + 显式防抖下限（优势） | **M3**（吸附动画）**M4** | P2 |

---

## A.6 Navigation（`nav_widgets`，7 个）与 Menus/Toolbars（`menu_toolbar`，8 个）

> 两组共 **15 个控件、29 个字面量**——**全仓主题最干净的 15 个**。
> 但也是**动画最缺的**：0 个 `tick`。导航的本质是**空间移动**，没有动画就没有空间感。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `app_bar` | `app_bar.rs` | 1 | BLUE21 AR2：字号 `h*0.38` **clamp 上限 22** ⇒ 2× 文本缩放**静默封顶**；back/title/action 全按**物理边** | ① 解 clamp ② `leading`/`trailing` 按 `TextDirection` 交换 | **M8** **M9** | **P0** |
| `navigation_drawer` | `navigation_drawer.rs` | 3 | BLUE21 AR2：`panel_width.min(rect.width)` 在 240 px 下 = **整幅**；主流 material 实现 是固定 304 | ① 固定宽 ② 开合**滑动动画**（M3） | **M3** **M10** | P1 |
| `bottom_navigation_bar` | `bottom_navigation_bar.rs` | 0 | BLUE21 AR2：指标是 **3 px 下划线**，M3 是 **64×32 药丸**；图标/字号用 `h*0.32`/`h*0.18` clamp | ① 药丸指标 ② 图标 24 / 标签 14·12 固定 ③ 切换动画（M3） | **M10** **M3** | P1 |
| `tab_view` | `nav_widgets/tab_view.rs` | 1 | ✅ `tab_width = w/count` 是对的（BLUE22 第 70 轮已由组装推导） | **M3**（切页过渡）**M8** | P2 |
| `navigation_stack` | `navigation_stack.rs` | 0 | BLUE21 A19：`nav_rect.y + 14` 是字面量，被 **13 px 与 15 px 两种字号共用** | ① 逐标签 `measure_text(text,&font).height` ② 推入/推出**转场**（M3） | **M10** **M3** | P1 |
| `pagination` | `pagination.rs` | 2 | 本仓**优势**（主流 material 实现 无独立分页控件）；页切换无反馈 | **M1**（页码 hover）**M2** | P2 |
| `adaptive_scaffold` | `adaptive_scaffold.rs` | 1 | ✅ 字号按比例 + clamp（BLUE21 记 ✅） | **M8** | P2 |
| `menu` | `menu.rs` | 3 | ✅ 项带已由 `FlexLayout` 推导（BLUE22 第 70 轮）；**展开/收起瞬时** | ① 展开动画（M3）② 菜单项 hover（M1/M2）③ 方向键 `isMirrored` | **M3** **M1** **M8** | P1 |
| `menu_item` | `menu.rs` | 3 | 同 `menu`；BLUE21 A.16：`menu_bar` 条目曾**重叠 9.6 px**（已修） | **M1** **M2** | P1 |
| `menu_bar` | `menu_bar.rs` | 3 | 有 hover 追踪 | **M1**（去重改读 base）**M2** | P1 |
| `context_menu` | （`menu.rs`） | — | 无独立构造器（capability 上存在） | **M10** | P2 |
| `tool_bar` | `tool_bar.rs` | 3 | ✅ 项带已由布局推导（BLUE22）；BLUE21 B12：曾有 **6 处字面量** | **M1** **M2**（**M4 已完成**） | P1 |
| `tool_button` | `tool_button.rs` | 3 | BLUE21 A10/A11：标签曾「从中点起左对齐」（已修）；有 hover | **M1** **M2** | P1 |
| `status_bar` | `status_bar.rs` | 2 | ✅ 段盒已由 `FlexEnd` 布局推导（BLUE22）；BLUE21 B16：讯息按承载带源 token 混合 | **M4**（改指 muted + `legible_on(4.5)`） | P1 |
| `action` | `action.rs` | 1 | 有 `hovered` 信号 | **M1** | P2 |
| `menu_button` | `menu_button.rs` | 6 | 6 个字面量；无展开动画 | **M4** **M3** | P2 |
| `dropdown_menu` | `dropdown_menu.rs` | 0 | 无展开动画 | **M3** **M5**（面板层级） | P2 |

---

## A.7 Dialogs（14）、Overlays（4）、Views（14）—— **共 32 个控件**

> 实测：**32 个控件、60 个字面量、0 个 `tick`、0 个完全不读 style**。
> 本块的核心缺口**不在颜色也不在状态**，而在**层级（§5）与动画（§3）**：
> 对话框的出现/消失、遮罩的渐显、列表行的滚动，**全是硬切**。

### A.7.1 Dialogs（`dialog`，14 个）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `dialog` | `dialog_widget.rs` | 1 | ✅ 标题栏居中正确、`r28`/`minW280` 已对齐；**模态遮罩无 token** | ① 遮罩读 `scrim`（**M5**）② 出现/消失缩放 + 遮罩渐显（M3） | **M5** **M3** | **P0** |
| `message_box` | `message_box.rs` | 6 | 6 个字面量（对话框里最多） | **M4** **M5** **M3** | P1 |
| `file_dialog` | `file_dialog.rs` | 2 | BLUE21 D3：列表高曾 **8 px**、占位符溢出 12 px（**已修**） | **M5**（列表行 `outline_variant`）**M10**（列宽可拖） | P1 |
| `color_dialog` | `color_dialog.rs` | 9 | BLUE21 D1：取色区高 **0**（已修）；B11：OK/Cancel 曾同填（已修）；仍 9 字面量 | **M4** | P1 |
| `font_dialog` | `font_dialog.rs` | 2 | BLUE21 D2：三列列表高 **0**（已修） | **M4** **M5** **M10**（预览区） | P2 |
| `input_dialog` | `input_dialog.rs` | 2 | ✅ 接受键已取 primary token（BLUE21 B11 的范式） | **M1** **M5** | P2 |
| `progress_dialog` | `progress_dialog.rs` | 2 | 进度条**无不确定态动画** | **M3** **M5** | P1 |
| `find_replace_dialog` | `find_replace_dialog.rs` | 0 | ✅ `text_line` 的**正确写法参照**（BLUE21 R1 的范式来源） | **M1** **M5** | P2 |
| `wizard_dialog` | `wizard.rs` | 2 | 步骤切换无动画；BLUE21 P2-11 指出本仓缺**真正的向导** | **M3** **M10** | P1 |
| `popover` | `dialog/popover.rs` | 2 | BLUE21 A22：占位符曾贴卡片顶 8 px（已修）；BLUE21 A.7 #11：阴影溢出时**宁可不画**（**优势，保留**） | **M5**（`surface_container_high`）**M3**（淡入） | P1 |
| `tooltip` | `dialog/tooltip.rs` | 1 | 浅色态与背景**无从区分**（需 `inverse_surface`） | **M5**（`inverse_surface`/`on_inverse_surface`）**M3**（延迟 450 ms 淡出，参考工具包 `reference: the scroll bar's minimum-length and hide-delay rules` 的范式） | **P0** |
| `popup_window` | `popup_window.rs` | 1 | ✅ 24 px 常量已提取（BLUE21 D13 的范式） | **M5** | P2 |
| `bottom_sheet` | `bottom_sheet.rs` | 1 | BLUE21 B23（**至今未修**）：遮罩 `ink.blend(sheet, 0.55)` ⇒ 暗态把背板**照亮**（121 亮于 18） | ① 朝**绝对暗色**混或读 `scrim`（**M5**）② 上滑动画（M3） | **M5** **M3** | **P0** |
| `modal_bottom_sheet` | `modal_bottom_sheet.rs` | 1 | 同 `bottom_sheet`（同根因） | **M5** **M3** | **P0** |

### A.7.2 Overlays（`overlay_widgets`，4 个）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `banner` | `banner.rs` | 3 | ✅ 严重度取自语义 token 再推离承载面（BLUE21 A.7 #7 的**优势**）；且是 `theme_derived` 的**先例** | **M4** **M3**（滑入/滑出） | P1 |
| `fab` | `fab.rs` | 2 | BLUE21 P0-4：**不在 `check_click_requires_release_inside` 的合法清单**（门禁抓到的 2 处缺陷之一） | ① 修点击契约（释放必须在内）② 抬起/落下的**高度动画** + 涟漪 | **M3** **M10** | **P0** |
| `refresh_control` | `refresh_control.rs` | 1 | 有 `tick` 能力但无驱动者？下拉**无回弹动画** | **M3**（回弹曲线） | P1 |
| `splash_screen` | `splash_screen.rs` | 4 | 4 个字面量；淡出无动画 | **M4** **M3** | P2 |

### A.7.3 Views（`view_widgets`，14 个）

> **本组是全仓主题最干净的一组**：14 个控件总计只有 **18 个字面量**（`image_gallery` 独占 14）。
> 缺口**完全**在交互与动效：**0 个 `tick`**、行 hover 不一致、滚动无惯性。
> 用户每天与之互动最多的控件（列表、表格、树）全在这里。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `list_view` | `list_view.rs` | 0 | 行 hover/选中**无统一来源**；滚动无惯性；`list_view` 是 3 个控件共用的 kind（`command_palette`/`notification_center`） | ① 行 hover 统一（**M1**，靠 base 的 hover 命中）② 滚动惯性/吸附（M3）③ 行分隔线 `outline_variant`（M6） | **M1** **M3** **M6** | **P0** |
| `table_widget` | `table_widget.rs` | 0 | BLUE21 A15：行文本曾压分割线（已修）；`Table` kind 被 **5 个控件共用**（`table_widget`/`table`/`data_grid`/`virtual_table`/`diff_viewer`） | ① 行 hover 高亮（M1）② M6 行线 ③ 列宽可拖 | **M1** **M6** | **P0** |
| `table` | `table_widget.rs` | 0 | 与 `table_widget` **同为 `Table` kind**（同一 `Draw` 路径） | 同上（**修一次覆盖两个**） | **M1** **M6** | **P0** |
| `virtual_list` | `virtual_list.rs` | 0 | 与 `data_view` **共用同一文件**（同一 `DataView` kind）；同 `list_view` 的缺口 | **M1** **M3** | P1 |
| `data_view` | `virtual_list.rs` | 0 | 与 `virtual_list` **同为 `DataView` kind**（`audit_kind_sharing` 实测）；无行 hover | **M1** **M3** | P1 |
| `data_grid` | `data_grid.rs` | 0 | ✅ 虚拟化 + `frozen_columns` 是**本仓真实优势**（BLUE21 A.7 #9） | **M1** **M6** | P1 |
| `grid_table` | `grid_table.rs` | 4 | BLUE21 D14：列头是 `Col {i}` **占位符**（用循环下标当列名） | ① 从数据源取列名 ② **M6** | **M10** **M6** | P1 |
| `tree_view` | `tree_view.rs` | 0 | 展开/收起**瞬时**；无缩进引导线 | ① 展开动画（M3）② 子级缩进引导线（M6） | **M3** **M6** | P1 |
| `tree_table` | `tree_table.rs` | 0 | 同 `tree_view` | **M3** **M6** | P2 |
| `virtual_table` | `virtual_table.rs` | 0 | 同 `table` | **M1** **M6** | P1 |
| `virtual_list` / `data_view` | `virtual_list.rs` | 0 | 同 `list_view` | **M1** **M3** | P1 |
| `properties_panel` | `properties_panel.rs` | 0 | BLUE21 A14：行曾低 10 px 压分割线（已修） | **M1** **M6** | P2 |
| `property_grid` | `property_grid.rs` | 0 | 同 `properties_panel` | **M1** **M6** | P2 |
| `query_builder` | `query_builder.rs` | 0 | 条件行增删无动画 | **M3** **M5** | P2 |
| `image_gallery` | `image_gallery.rs` | 14 | **本组唯一字面量集中地（14/18）**；无缩放动画 | **M4** **M3**（缩放/淡入） | P1 |

> **本组共用 kind 的完整清单**（`tools/audit_kind_sharing.py` 实测）：
> `Table` → 5 个控件；`ListView` → 3 个（`list_view`/`command_palette`/`notification_center`）；
> `DataView` → 2 个；`RichEdit` → 3 个。**共用 kind 意味着共用 `Draw` 路径**，
> 因此上面每一行的 **M1/M6 修一次即同时覆盖同 kind 的全部控件**——这是本组性价比最高的地方。

---

## A.8 Charts（4）、Advanced（8）、Media（7）、Misc（8）、Cupertino（8）、Web（1）、Core（2）

### A.8.1 Charts（`chart_widgets`，4 个）

> 实测：**4 个控件只有 1 个字面量、全部不读 style、0 个 `tick`**。
> 这组看似「干净」，实则是**反面**：`charts.rs` 被 `bar_chart`/`line_chart`/`pie_chart` **三个控件共用**，
> 颜色可能来自参数而非 `style`——`audit_appearance.py` 因此**测不到**它。
> BLUE21 P4-3 已明确：本组**无外部基准**（外部对标 / Cupertino 都无图表），**正确性全靠自查清单**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `bar_chart` | `chart_widgets/charts.rs` | 0 | 与 `line_chart`/`pie_chart` **共用一个文件**；升级/过渡无动画 | ① 值变化**过渡动画**（M3，柱高插值）② M4（确认色来自 `style` 而非参数默认） | **M3** **M4** | P1 |
| `line_chart` | 同上 | 0 | 同上；曲线**无描画动画** | **M3**（路径描画）**M4** | P1 |
| `pie_chart` | 同上 | 0 | 同上；扇区无生长动画 | **M3** **M4** | P2 |
| `sparkline` | `sparkline.rs` | 1 | ✅ `tools/control_color_exemptions.txt` 明确「**无 chrome 可主题化**」——**豁免正确** | 保持豁免；**M3**（可选描画） | P2 |

### A.8.1b Specialised controls（`special_widgets`，32 个）

> **本组是最大的一个（32 个控件），也是字面量最多的一个（140 个 = 全仓 21%）。**
> 实测：**6 个完全不读 style**、只有 **1 个有 `tick`**、5 个字面量过 10。
> 本组承载本仓**最领先的能力**（图表族 14+ / 代码编辑器 / 终端 / 签名板 / 富文本 / 取色工作流），
> 也正因如此，**它们的外观必须配得上它们的功能**。

#### (a) 图表与金融族（**本仓优势，主流 material 实现 无对应**；BLUE21 P4-3 的自查清单适用）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `candlestick_chart` | `finance/candlestick_chart.rs` | **13** | **不读 style**；BLUE21 B4：绘图面字面量 `Color::rgb(18,22,28)` 与另三图**逐字节相同**（四份 SVG 的 diff 各只有 4 行）；D5：空态无轴无网格 | ① **M4**（抽 `finance/layout.rs` **一处**共享面板色）② **M12**（空态画轴 + `No data`）③ BLUE21 C8 死绑定 | **M4** **M12** | **P0** |
| `volume_chart` | `finance/volume_chart.rs` | 4 | **不读 style**；BLUE21 B4 同形；B5：窗格边距与其他三图**不一致**（`x=52 w=180` vs `x=48 w=184` ⇒ bar 对不齐） | ① **M4** ② 统一 `PlotArea::with_margins` ③ **M12** | **M4** **M12** | **P0** |
| `depth_chart` | `finance/depth_chart.rs` | 9 | **不读 style**；BLUE21 B4 同形；D5 空态 | **M4** **M12** | **P0** |
| `indicator_chart` | `finance/indicator_chart.rs` | **12** | **不读 style**；BLUE21 B4/B5 同形 | **M4** **M12** | **P0** |
| `order_book` | `finance/order_book.rs` | 9 | 9 字面量；**已有 `hovered: Option<(BookSide,usize)>` + `level_hovered` 信号**（本组少数已做状态的） | ① 行键改读 base（M1 去重）② **M4** ③ 价格跳动**闪烁动效**（M3） | **M1** **M4** **M3** | P1 |
| `quote_board` | `finance/quote_board.rs` | 9 | 同 `order_book`；**涨跌红绿是数据色**（BLUE21 §七 明确**不判为缺陷**） | **M1** **M3**（价格变化高亮淡出） | P1 |
| `radar_chart` | `radar_chart.rs` | **14** | **不读 style**；14 字面量（图表族最多） | **M4** ② 轴/网格走 `charts.rs::axis_chrome()` | **M4** | P1 |
| `heatmap` | `heatmap.rs` | **12** | 12 字面量；**色阶是数据色**（豁免，不判） | **M4**（仅 chrome 部分：轴/网格/刻度） | P1 |
| `chart`（ChartWidget） | `chart.rs` | 10 | BLUE21 D6（**至今未修**）：**完全没有值轴**（`PlotArea` 无左槽）；最高柱**贴顶零余量** | ① 补左刻度列（4 档）+ 值标签 ② **M12** ③ 数据变化过渡（M3） | **M10** **M12** **M3** | **P0** |
| `gantt_widget` | `gantt_widget.rs` | 2 | BLUE21 D15：行标签**无宽度约束**（轨道起于 `x+150` 而标签无 bound） | ① 标签改 `draw_text_fitted`（BLUE22 R-7 已部分）② 今日线/依赖箭头（M6） | **M10** **M6** | P1 |
| `timeline_widget` | `timeline_widget.rs` | 0 | 同 `gantt_widget`（同 `Chart` kind） | **M10** | P2 |
| `grid` | `special_widgets/grid.rs` | 5 | 5 字面量；BLUE21 A.7 #8 记 ``:394` 的 `== rgb(220,220,220)` 哨兵是**潜在**问题（无证据不判） | **M4**（哨兵本就是主题比较，改读 token 更稳） | P2 |
| `canvas` | `canvas.rs` | 1 | ✅ 1 字面量 | **M4** **M5**（画布底色读 `surface_container`） | P2 |
| `kanban_board` | `kanban_board.rs` | 0 | 0 字面量；卡片拖动**无落点指示** | **M3**（拖影 + 插入线）**M5**（列背景层级） | P1 |
| `freeform_shape` | `freeform_shape/shape.rs` | 2 | 2 字面量 | **M1** **M4** | P2 |

#### (b) 文本与编辑器族（**本仓优势，主流 material 实现 无对应**）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `code_editor` | `code_editor/editor.rs` | 0 | 有 `tick`（光标闪烁）**无驱动者**；BLUE21 P4-4：`SyntaxPalette::default()` **整套浅色**（第 61/62 轮已记，仍未变）；A.6：无手柄/无放大镜/无端点调整 | ① **M3**（一行，光标会闪）② 语法配色按**主题外观**给两套 ③ 选中子系统（手柄/放大镜） | **M3** **M4** | **P0** |
| `terminal_view` | `terminal_view.rs` | 1 | **最不能没有闪烁的控件**（不闪读起来像卡死）；**无 `tick`** | ① 加 `CursorBlink` + `tick` ② 滚动跟随 | **M3** | **P0** |
| `rich_edit` | `input_widgets/rich_edit.rs` | 1 | 见 §A.3：光标几何用**等宽捷径**，变宽跨度下必错 | **M10**（跨度感知 caret） | P1 |
| `markdown_editor` | `markdown_editor.rs` | 3 | 同 `rich_edit` 的跨度问题 | **M10** | P2 |
| `diff_viewer` | `diff_viewer.rs` | 0 | 0 字面量；**增删行用语义色**（本仓优势） | **M6**（行线）**M5** | P2 |
| `signature_pad` | `signature_pad.rs` | 1 | BLUE21 B18：画布 = 窗口底色（已修）；A.6：笔画捕获**按帧采样** ⇒ 快速输入多边形化（需**时间戳**） | ① 收带时间戳的指针增量流 + 相邻 delta 间**插值**② 平滑作为**独立可测步骤** | **M10** | P1 |
| `command_palette` | `special_widgets/command_palette.rs` | 0 | 与 `list_view` 同 kind；分类/高亮无动效 | **M1**（行 hover）**M3**（过滤动画） | P2 |

#### (c) 取色与图形族（**本仓优势**：主流 material 实现 与 Cupertino **都没有**取色器）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `color_picker` | `color_picker.rs` | 10 | 10 字面量；BLUE21 A.4.3f：选色应能表达为 `ColorScheme` role 赋值而非裸值 | ① **M4** ② 色相条/饱和度面读 token ③ 拖动**实时反馈**（M3） | **M4** **M3** | P1 |
| `map_view` | `map_view.rs` | 5 | 5 字面量；与 `canvas` **共用 `Canvas` kind**；无平移/缩放动画 | **M5**（地图面层级）**M3**（惯性平移） | P2 |
| `breadcrumb` | `breadcrumb.rs` | 0 | 0 字面量；分隔符与悬停无统一来源 | **M1** **M6**（分隔符） | P2 |
| `segmented_control` | `segmented_control.rs` | 3 | BLUE21 AR2：`seg_w = rect.width/count` ⇒ 长标签 `text_x` 走负、越界重叠 | ① 按 主流 material 实现 每段内边距 **16** / 最小高 **28** ② 选中段**滑动指示器**（M3） | **M10** **M3** | P1 |
| `split_button` | `split_button.rs` | 3 | ✅ 已由组装推导并 tile（BLUE22 第 70 轮）；仍 3 字面量 | **M1**（主体/箭头分别 hover）**M4** | P1 |

#### (d) 通知与吐司族

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `toast` | `toast/single.rs` | 2 | BLUE21 A23：`rect.y + (h+12)/2` 是**半个行盒 nudge**（同一 toast 的 ✕ 却在真中心 60）⇒ 与实现矛盾 | ① 改 `material_snackbar.rs:368` 的写法（`(h - metrics.height)/2`）② **M5** `inverse_surface` ③ **M3**（出入场） | **M10** **M5** **M3** | **P0** |
| `toast_stack` | `toast/stack.rs` | 2 | 与 `popup_window` **共用 `PopupWindow` kind**；堆叠无动画 | **M3**（堆叠位移）**M5** | P1 |
| `snackbar` | `snackbar.rs` | 2 | 与 `status_bar` **共用 `StatusBar` kind**（屏底带） | **M5** `inverse_surface` **M3** | P1 |
| `notification_center` | `notification_center.rs` | 0 | 与 `list_view` 同 kind；BLUE21 A.7 #7 记其严重度取自**语义 token 再推离承载面**（优势） | **M1** **M5** **M3**（新条目滑入） | P1 |
| `media_player` | `media_player.rs` | 6 | **不读 style**；与 `web_engine_view` **共用 `WebEngineView` kind** | **M4** **M1**（控件条 hover） | P1 |

---

### A.8.2 Advanced（`advanced_widgets`，8 个）

> 实测：**50 个字面量**（`pie_menu` 独占 24、`calendar` 14）、0 个 `tick`。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `calendar` | `calendar.rs` | 14 | ✅ BLUE21 R-3/R-4 已修（日号居中 + 表头同列）；BLUE21 A.3.7：行高 `grid.height/6` ⇒ 11 px（主流 material 实现 42/48） | ① 行高按 主流 material 实现 **42**（M10）② 月份切换**横滑动画**（M3）③ 14 字面量清理 | **M10** **M3** **M4** | **P0** |
| `dial` | `dial.rs` | 3 | BLUE21 C1（**至今未修**）：`notches_visible`/`notch_target` **完全空转**（`Draw` 不引用） | ① **实现刻度环** ② 先裁定 `notch_target` 单位（参考工具包 是像素间距，本仓注释写「度」）③ 指针拖拽带动效 | **M10** **M3** | **P0** |
| `tab_bar` | `tab_bar.rs` | 3 | BLUE21 D9：三种 `TabShape` **画得一样**（注释描述了没做的活）；D10：tab **永不换行/裁剪/溢出** | ① 抄 `tabwidget.rs:519-560`（那里三种形状是**真画**的）② 溢出出口 ③ 激活 tab 滑动指示器（M3） | **M10** **M3** | **P0** |
| `pie_menu` | `pie_menu.rs` | 24 | **本组字面量最多（24）**；有 hover 记录 | ① **M4（24 处）** ② 展开/收起**扇形动画**（M3） | **M4** **M3** | P1 |
| `ribbon_bar` | `ribbon_bar.rs` | 3 | 有 hover 记录 | **M1** **M2** | P1 |
| `date_edit` | `date_edit.rs` | 1 | BLUE21 A.4.5f：是**纯文本框**，**选择器不可达**（主流 material 实现 是对话框触发器） | ① 接日历弹出（`calendar_popup`）② 焦点态（M1/M2） | **M10** **M1** | P1 |
| `time_edit` | `time_edit.rs` | 1 | 同 `date_edit`，且 `access.rs:497-503` **缺 `calendar_popup`**（契约不一致） | **M10** **M1** | P1 |
| `date_time_edit` | `date_time_edit.rs` | 1 | 同族 | **M10** **M1** | P2 |

### A.8.3 Media and web（`media_widgets`，7 个）

> 实测：**62 个字面量、5 个 `tick`**——**唯一动画齐备的一组**（但同样无驱动者）。
> 3 个完全不读 style。这组的缺口**单一**：**颜色不进主题**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `video_player` | `video_player.rs` | 11 | **不读 style（`style=N`）**；11 字面量 | ① **M4（11 处 → 主题）** ② 控件条 hover（M1）③ M3（已有 `tick`，一行接线） | **M4** **M1** **M3** | **P0** |
| `camera_preview` | `camera_preview.rs` | 16 | **本仓字面量最多（16）且不读 style** | **M4（16 处）** **M5**（取景框层级） | **P0** |
| `audio_visualizer` | `audio_visualizer.rs` | 6 | **不读 style**；6 字面量 | **M4** | P1 |
| `hero_animation` | `hero_animation.rs` | 11 | 有 `tick` 无驱动者；11 字面量 | **M3** **M4** | P1 |
| `lottie_widget` | `lottie_widget.rs` | 6 | 有 `tick` 无驱动者 | **M3** **M4** | P1 |
| `rive_widget` | `rive_widget.rs` | 9 | 有 `tick` 无驱动者 | **M3** **M4** | P1 |
| `animated_image` | `animated_image.rs` | 3 | 有 `tick` 无驱动者 | **M3** **M4** | P1 |

### A.8.4 Miscellaneous（`misc_widgets`，8 个）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `drop_zone` | `drop_zone.rs` | 1 | BLUE21 B17：静止态**无可见面**（zone 填充 == 背景）；A.4.5c：只有 **1/5** 反馈态 | ① 面推离窗口底色（**M5**）② 补 `onMove`/`onLeave`/reject + 落点偏移 ③ 悬停高亮（M2） | **M5** **M10** **M2** | **P0** |
| `qr_code` | `qr_code.rs` | 0 | BLUE21 C5：`quiet_zone` 有字段**无访问器无 schema**；纠错级**不可设** | ① 发布 `quiet_zone` + setter ② 纠错级属性（QR「差不多对」就扫不出来） | **M10** | P1 |
| `barcode_scanner` | `barcode_scanner.rs` | 0 | BLUE21 D11：四角括号**一半画到遮罩上**；A.6 缺相机权限/生命周期 | ① 四角统一锚到 viewfinder **内侧**角 ② 生命周期暂停/恢复 | **M10** | P1 |
| `date_range_picker` | `date_range_picker.rs` | 0 | BLUE21 D4：表头/星期行/网格三者错位（11 px 死区） | **M10**（`grid_top` 从**已画出的范围**推） | P1 |
| `mobile_date_picker` | `mobile_date_picker.rs` | 4 | BLUE21 A.3.8：行距 24 vs 主流 material 实现 **32**；10 px 字装 24 px 行 | ① 行高 32 ② 选中带圆角 8 ③ 滚轮对齐动画（M3） | **M10** **M3** | P1 |
| `segmented_button` | `segmented_button.rs` | 3 | 与 `segmented_control`/`cupertino_segmented_control` **共用 `ToggleButton` kind**；32 高已统一 | ① 选中段**滑动指示器**（M3）② M1 | **M3** **M1** | P1 |
| `avatar` | `avatar.rs` | 1 | ✅ 已固定 `AVATAR_SIZE 40` | **M4** | P2 |
| `bezier_curve_editor` | `bezier_curve_editor.rs` | 0 | 有 hover 记录；控制点拖拽无反馈 | **M3**（手柄跟随）**M1** | P2 |

### A.8.5 Cupertino（`cupertino`，8 个）

> 实测：**79 个字面量**（集中于 `core.rs`），**4 个控件共用 `core.rs`**。
> 关键发现：**`CupertinoSwitch` 是 `Switch` 的纯类型别名**（`core.rs:34`，
> BLUE21 A.3.10 已记）⇒ 它**没有 iOS 几何**（主流 material 实现 是独立控件 59×39/51×31/拇指 r14）。
> 且实测 `core.rs:615-618` 已有 `Color::rgb(0,122,255)`（iOS 蓝）等**现成的 iOS 色**——
> **色有、几何与状态没有**。

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `cupertino_switch` | `cupertino/core.rs` | 14 | **是 `Switch` 的别名**（无 iOS 几何 59×39/51×31）；BLEU21 A.3.10：未选中轨道曾 **= 窗口底色（不可见）** | ① 给 iOS 自有几何 ② **按压横向拉伸 7.0**（主流 material 实现）③ ON/OFF 标签 ④ iOS 绿（`52,199,89`，**色已在文件里**） | **M4** **M3** **M10** | **P0** |
| `cupertino_slider` | `cupertino/core.rs` | 14 | iOS 拇指**不是圆盘**（应有更细的轨道与更大拇指视觉） | ① iOS 几何 ② M3 | **M4** **M3** | P1 |
| `cupertino_alert_dialog` | `cupertino/core.rs` | 14 | 无 iOS 圆角（**12** 而非 Material 28）；无遮罩层级 | ① iOS 圆角 ② **M5** ③ M3 | **M10** **M5** **M3** | P1 |
| `material_snackbar` | `cupertino/core.rs` | 14 | `material_snackbar.rs:368` 是 BLUE21 A23 的**正确写法参照** | **M5**（`inverse_surface`）**M3**（滑入 + 自动消失计时） | **M5** **M3** | P1 |
| `material_navigation_rail` | `cupertino/core.rs` | 14 | 无 rail 展开/收起动画 | **M3** **M5** | P1 |
| `cupertino_date_picker` | `cupertino/date_picker.rs` | 5 | ✅ 行距已在 metrics 表（`32`） | **M3**（滚轮吸附）**M4** | P2 |
| `cupertino_navigation_bar` | `cupertino/nav_bar.rs` | 2 | 大标题**收起动画**缺失（iOS 的标志性交互） | **M3**（标题缩放/收起） | P1 |
| `cupertino_segmented_control` | `cupertino/segmented_control.rs` | 2 | 段内边距 16/最小高 28（已在 metrics；BLUE21 AR2 曾记 `text_x` 走负） | **M3**（滑动指示器）**M1** | P2 |

### A.8.6 Web（1）与 Core（2）

| 控件 | 源文件 | 字面量 | 现状缺陷（实测） | **改进点** | **改进方式方法** | 优先级 |
|---|---|---:|---|---|---|---|
| `web_engine_view` | `web_widgets/` | — | `supports_web_engine()` 如实报 false（BLUE20 W1 删掉了从不显示的 WebKitGTK 包装）；本仓**诚实回答能力** | 维持诚实；文档明写 | — | — |
| `window` | `window.rs` | 1 | ✅ 只画客户区（标题栏归窗口管理器）——**刻意且有据** | 保持；M11（客户区内焦点环） | P2 |
| `tool_box` | `container_widgets/toolbox.rs` | 11 | 见 §A.5 `tool_box`（BLUE21 D7） | **M7** **M10** **M8** | P1 |

---

## A.9 `audit_control_gaps.py` 报出的 25 个「外部基准有、本仓无」（实跑）

```text
$ python3 tools/audit_control_gaps.py
candidate gaps: 25
```

**这 25 条不是缺陷登记**（BLUE21 §A.4.3 已确立：本仓在桌面/图表方向**领先** 主流 material 实现），
而是**外部基准的差集**。逐条裁定（**P4 新功能，不混入外形修复**，BLUE21 §6.5 同一处理）：

| 缺口 | 裁定 | 理由 |
|---|---|---|
| `password_edit` | ❌ 不补 | `line_edit` 的 `EchoMode` 已覆盖（BLUE21 曾删 `PasswordEchoOnEdit`，用 `CursorBlink` 兼得） |
| `accordion` | ❌ 不补 | `collapsible_pane` + `tool_box` 已覆盖 |
| `context_menu` | ⚠️ 部分 | `WidgetKind::ContextMenu` **已存在**（kind 表第 36 项），缺构造器 ⇒ **M10** |
| `alert_dialog`/`confirm_dialog`/`about_dialog` | ❌ 不补 | `message_box` 用 `MessageBoxLevel` 表达（语义化更清晰） |
| `time_picker`/`clock` | ⚠️ 部分 | `time_edit` 已存在但**选择器不可达**（§A.8.2）⇒ 补**弹出时钟**而非新 kind |
| `tag` | ❌ 不补 | `chip` 已覆盖 |
| `flex`/`flow_layout`/`wrap_layout`/`split_layout`/`aspect_ratio`/`spacer`/`center`/`align`/`sized_box`/`padding`/`expanded`/`flexible`/`intrinsic_width` | ✅ **本仓优势，勿动** | 这 13 个是 **主流 material 实现 的布局 widget**；本仓把它们做成了 `src/layout/` 的 15 种 `Layout` 实现（更少类型、更可测）。补成 180 个 kind 才是**退步** |
| `gauge` | ⚠️ 部分 | `meter` 已覆盖（§A.4，**需修刻度错相**） |
| `phone_input`/`email_input` | ❌ 不补 | 属**校验规则**而非控件；应做成 `masked_edit` 的 preset |

**结论**：25 条中 **13 条是本仓的更优形态**（布局作为引擎而非 widget）、
**3 条是「kind 在但能力缺」**（`context_menu`/`time_picker`/`gauge`，已归入上表）、
**9 条是不该补的别名**。**因此本计划的控件数维持 188，不新增 kind**——
与 `tools/check_widget_kind_count.sh`（22 份文档均声明 180）保持一致。

---

## A.10 按「改进方式方法」的**总工作量**（决定批次划分）

| 代号 | 命中控件数 | 一次投入 | 若不在此层做 |
|---|---:|---|---|
| **M1** 状态接线 | **188（全部）** | 1 处 `BaseWidget` 字段 + 1 处默认实现 | 188 个控件各写一遍 hover 分支 |
| **M2** 状态覆盖 | 188（全部） | 1 处 `generate.sh` + 6 个键 | 主题作者写的状态**永远不生效** |
| **M3** 动效接线 | **11 已有 + 约 40 应新增** | 1 处 `tick_animations` + 每控件 1 行 | 11 个动画**静默死亡** |
| **M4** 字面量 → token | **66 个 Draw 文件** | 每文件逐处（但**同字形可抽共享函数**） | 684 处字面量继续复制 |
| **M5** 层级上妆 | **约 20**（对话框/面板/遮罩/反转面） | 7 个 token 的消费者 | 「不像 主流声明式实现」 |
| **M6** 分隔线上妆 | **约 15**（列表/表格/树/分隔） | 1 处 `outline_variant` | 行线与焦点环同色 |
| **M7** 组合改组装 | **4 剩余**（`group_box`/`tool_box`/`number_picker`/`dialog` 按钮行） | BLUE22 已建 `CompositeBuilder` | 手算几何继续漂 |
| **M8** RTL 接线 | **6**（`progress_bar`/`range_slider`/`tab_bar`/`app_bar`/`scroll_bar`/`menu`） | `TextDirection` 已存在 | 镜像后取值反向 |
| **M9** a11y 填充 | **约 8**（滑块/进度/评分/复选框/开关/单选/编辑器/日历） | `A11yState` 结构已齐 | 屏幕阅读器拿不到值 |
| **M10** 契约加厚 | **约 25**（见各表） | 逐控件 | 声明了但读不回 |
| **M11** 焦点环统一 | **188（M1 顺带）** | 1 处 `FocusRing` | 鼠标点击画焦点环 |
| **M12** 空态画 chrome | **4**（`chart`/金融四图） | 1 处 `axis_chrome()` | 空态是黑板 |

**这张表是本计划「修复量随层下降」的机械证据**：
**M1/M2 各 1 处覆盖全部 188 个控件**；M4/M5/M6 是 66/20/15 个文件的逐处；
M7/M8/M9/M10 是 4/6/8/25 个控件的逐条。
**即：影响面最大的三条（M1/M2/M3）恰是投入最小的三条。**

---

## A.11 每组的批次归属（可直接接续）

| 批 | 内容 | 覆盖控件 | 判据 |
|---|---|---|---|
| **批 1** | M1 + M11（状态与焦点环上提） | **188** | §2.2 的 5 条 |
| **批 2** | M2（状态覆盖进预设） | **188** | §2.4 的 4 条；静止快照不变 |
| **批 3** | M3 总线 + 已有 11 个 `tick` 接线 | 11 | §3.3 的 6 条 |
| **批 4** | M3 新增（switch 滑动 / keyboard 按键 / 列表惯性 / 展开收起 / 对话框出现） | 约 40 | 每类一条三帧判据 |
| **批 5** | M5 + M6（层级与分隔线） | 约 35 | §5.1–5.4 |
| **批 6** | M4（66 个 Draw 文件，按组推进：先 `special_widgets` 再 `media` 再 `cupertino`） | 66 | `audit_appearance.py` 计数下降 |
| **批 7** | 各组 **P0 单点**（`group_box` 勾 / `meter` 刻度 / `switch` 滑动 / `dial` 刻度环 / `tab_widget` 零 tab / `tooltip` 层级 / `bottom_sheet` 遮罩 / `fab` 契约 / `drop_zone` 面 / `camera_preview` 字面量 / `calendar` 行高 / `app_bar` clamp） | 12 | 逐条快照/单测 |
| **批 8** | M7/M8/M9/M10/M12（组装剩余 + RTL + a11y + 契约 + 空态） | 约 60 | 逐条 |
| **批 9** | **§5A 声明式原语（一）**：错误边界 → `portal` → 生命周期钩子 | 0（层能力） | §5A.9 判据 1–7、11–14 |
| **批 10** | **§5A 声明式原语（二）**：上下文传播 + `children_if` 裁定 | 0（层能力） | §5A.9 判据 8–10、15–17 |
| **收尾** | 全量 + 快照再生 + `run_all_gates.sh` | — | §9 |

> **本表的批号与 §7 的批号不是同一套**：§7 是**计划主体的推进顺序**（状态→动效→层级→门禁），
> 本表是**按控件组/按工作类型的归堆**。两者覆盖的**控件**互不重叠，但**批号数字会撞**。
> §5A 的两批在本表里是 **批 9/批 10**，在 §7 里是 **批 7/批 8** —— 引用时请以**章节号**为准。

---

## 10. 一句话结论

**BLUE21 让控件「画对」，BLUE22 让控件「知道自己多大」，BLUE23 让控件「知道自己此刻是什么状态、
并把状态变化演给用户看」。**

三件事本仓**都已经有了抽象**——`WidgetState`（12 变体）、`Transition`/`CursorBlink`
（1582 行引擎）、17 个颜色角色——**唯一缺的是「说出来」与「推进它」**：

- 状态：`widget_state()` 已经在 trait 上、已经接到主题，**只有 1 个默认实现**；
- 动效：11 个控件已经实现了同形的 `tick`，**没有任何调用者**；
- 层级：7 个新角色已写进预设，**只有焦点环消费了 1 个**。

**它们的共同形状与 BLUE21 的 R1、BLUE22 的 AR1 完全相同：**
**本仓已经写出了正确的抽象，只是没有一个控件去调用它。**
所以本计划的修复量**随层下降**：一处 `BaseWidget` 字段、一处 `tick_animations`、
六条预设覆盖——而不是 195 个控件各写一遍。

**另有一类不同形状的工作：声明式层缺的原语（§5A）。**
它的共同形状不是「有抽象没调用」，而是**「模型里根本没有这一维」**——
`Patch::Insert` 无法表达「根侧层」，`Node` 只有四个字段，
`engine` 在任一子树失败时丢掉整份文档。
它与前三层的关系是：

> **前三层决定控件「长什么样、怎么动」；声明式原语决定这些东西「能不能被声明出来」。**

其中 `portal` 最紧迫，因为 `dialog`/`tooltip`/`bottom_sheet`/`popover` 这类
「逻辑上是子、视觉上要跳出父裁剪」的控件，**在声明式层里目前无法表达**。

**最后一条应当记住的判断**：本仓在**桌面/设计器/工控**方向
（dock / MDI / 分割条 / 多级列联 / 取色工作流 / 14 种图表 / 富文本 / 终端 /
虚拟化表格 + 冻结列 / 数值步进 / 分页）**明显强于 主流 material 实现**（BLUE21 §A.7 的 12 项）。
因此本计划的目标不是「变得像 主流 material 实现」，而是
**借 外部对标 / 参考工具包 的状态与动效机制、借 Material / 主流声明式实现 的层级语言，
把本仓已经领先的桌面能力做得同样「顺手」**——
因为一个 dock 面板能不能被顺畅地拖出与吸附，**取决于状态与动效，而不取决于它的几何有多准。**
