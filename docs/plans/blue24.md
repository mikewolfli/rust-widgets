# BLUE24 — 从「控件会动」到「应用跑起来」：帧循环、属性动画、正交状态、一致的环境事实与 a11y 落地

> 依据：[`principle.md`](principle.md)（继承 BLUE1–BLUE23，含 #1–#101）
> 前置：[`blue23.md`](blue23.md)（状态层 / 动效总线 / 层级层 / 声明式原语）
> 参考实现（**只读，不引入依赖**）：同 BLUE23 的三棵树，沿用其引用纪律。
> 目标版本：**2.7.0 → 2.8.0**
>
> **BLUE23 的未完成项由另一个进程处理，本计划不登记、不依赖、不阻塞。**
> 本文件只写 BLUE24 自己的范围：本计划发现但不在本轮施工的条目写在 **§12**，
> 由后续轮次在本文件内推进——**不允许开新文件**（BLUE23 §0A.0 已确立该形态，
> 本计划把它延伸到本文件自身）。

---

## 0A. 本计划的定位：BLUE23 交付了「机制」，本计划交付「能跑起来的应用」

BLUE23 的结论句是「本仓已经写出了正确的抽象，只是没有一个控件去调用它」。
本计划实测后确认：**该句对控件层已经成立，对应用层尚未成立。**

一句话说明为什么必须另立一轮：

> BLUE23 让**一个控件**在**一帧**里正确。本计划要让**一个应用**在**一个进程**里连续正确地跑。

两者缺的不是同一门课。控件层的失败形态是「画错」；应用层的失败形态是
「什么都不动，而且没有任何东西报错」——**后者更难发现，因为它看起来像静止**。

### 0A.1 现状取证（本轮实跑，非引用）

以下每一条都是可复跑的，不引用任何计划原文。

**取证 1：动效总线没有宿主侧驱动者。**

```text
$ grep -rn "tick_animations\|animation_bus_needs_another_frame" \
      src/app/ src/render/ src/platform/ demo/ examples/ tests/ cookbook/ \
      --include=*.rs --include=*.md
  （零命中）

全仓命中仅：
  src/widget/runtime.rs          ← 定义
  src/widget/draw_bridge.rs      ← 注释
  src/widget/display_widgets/switch.rs ← 注释
  CHANGELOG.md / docs/           ← 文档
```

BLUE23 §1.3 的原始缺陷是「11 个 `tick`、0 个调用者」。现在 `tick` 有调用者了
（`runtime::tick_animations` 与 `draw_bridge::draw_of`），**但 `tick_animations` 自己仍然没有调用者**。
一个控件能回答「我还欠一帧」，而**没有任何代码会去排那一帧**。

**取证 2：每个平台的帧循环各自为政，且都不是动画循环。**

```text
src/platform/linux/platform_impl.rs:233
    glib::timeout_add_local(Duration::from_millis(16), || { crate::drain_triggers(); ... })
        ↑ 16 ms 心跳只干了「排空触发队列」，与动画无关

src/platform/linux/platform_impl.rs:245
    while self.runtime.running.load(SeqCst) { thread::sleep(Duration::from_millis(16)); }
        ↑ 无 GTK 时的循环：每 16 ms 睡一觉，什么都不做

src/platform/macos/canvas.rs:951
    extern "C" fn drain_triggers_impl(...) { crate::drain_triggers(); }
        ↑ 同样的心跳，同样的单一职责
```

三个平台、三个循环、三种节奏，而**动画需要的那一次 tick 一个都没有**。
这正是 BLUE23 §3.3 警告的「宿主/后端各调一次」的反面：现在是「谁都不调」。

**取证 3：一次绘制里最多推进一步。**

```text
src/widget/draw_bridge.rs:185
    let _ = widget.tick(ANIMATION_FRAME_DELTA_MS);   // 16 ms，一次
src/widget/draw_bridge.rs:211
    const ANIMATION_FRAME_DELTA_MS: u32 = 16;
```

且注释自己说明了为什么是常量：*"At this entry point the crate has no clock and will not grow one"*。
该结论对**单步推进**是对的（避免长帧把动画传送到底），但它同时意味着：
**库无法靠「多走几步」变快，只能靠「多被调用几次」变快** —— 于是帧循环的有无成了唯一变量。

**取证 4：`bg` 只在 `theme/motion` 里有，不在 `en.json` 里。**

```text
language/en.json
language/zh-cn.json
language/zh-tw.json
    ↑ 三个 locale，都是中文系；无 de / ja / ar / he 这类改变**布局规则**的语言

$ grep -n "EMBEDDED_EN_JSON" src/i18n/global.rs
src/i18n/global.rs:10  const EMBEDDED_EN_JSON: &str = include_str!("../../language/en.json");
    ↑ **只内嵌英文**：zh-cn / zh-tw 存在但要靠 `preload_dir` 从磁盘加载，
      而 `mini`/`embedded` 没有文件系统 ⇒ 那两个 locale 在这两个 profile 上不可达
```

`I18nManager` 本身是完整的（`set_language` / `translate_with_context` / 复数 / context /
热重载 / `audit_keys` 全在），**缺的是两件事**：

1. **默认 locale 的来源**（现在必须由宿主显式 `set_language`，否则永远 `en`）；
2. **非 ASCII 语系的覆盖面**（现有三个 locale 全在 CJK，它们**不检验**
   bidi 与连写——而 BLUE23 G-3/G-4c 造的正是为了这两件事，却没有任何 locale 去用它）。

`I18nManager` 本身是完整的（`set_language` / `translate_with_context` / 复数 / context /
热重载 / `audit_keys` 全在），**缺的只是「有哪些语言」这个事实**。

**取证 5：a11y 的三段里，「推导」与「桥」两段都在，中间那段不在。**

这一段与 BLUE23 附录 A 的 M9 记载**不同**，先量准确数字（BLUE22 §F.3 教训：先取证再引用）。

已经存在的（实跑确认）：

```text
src/platform/accessibility/types.rs:160  A11yState::from_widget<W: Widget>(&W) -> Self
    ↑ 从控件推导 role/label/description/value/checked/mixed，**唯一映射**
    ↑ 且它的文档自己写着这段历史：「no producer at all … no code ever asked it」

src/control_backend/custom/create_widgets_helpers.in.rs:544  fn widget_a11y_state(id)
    ↑ 后端侧的取用点：with_widget(id, A11yState::from_widget)，挂载中的控件可查

src/platform/accessibility/{linux,macos,windows}.rs
    ↑ 三个平台的桥：AT-SPI2/D-Bus、NSAccessibility、UI Automation，均完整
```

**不在的那一段（本计划的缺口）**：

```text
$ grep -rn "accessibility_bridge()" src/ --include=*.rs
src/platform/a11y_wiring.rs:17     ← 唯一命中，且只接 FocusManager

$ grep -rn "bridge.set_accessibility_name\|bridge.notify_" src/ --include=*.rs | grep -v tests
  （零命中）

$ grep -rn "widget_a11y_state" src/ --include=*.rs | grep -v tests
src/control_backend/custom/create_widgets_helpers.in.rs:544   ← 定义
src/control_backend/trait_def/trait_def.rs:1177               ← trait 声明
  （**零个宿主侧调用者**）
```

即：

| 段 | 状态 | 位置 |
|---|---|---|
| ① 从控件**推导**语义 | ✅ 已建（且已有单测，含三态） | `A11yState::from_widget` |
| ② 挂载期**自动提交**给桥 | ❌ **不存在** | 本计划 §6.3 |
| ③ 桥 → OS 的无障碍总线 | ✅ 已建（三平台） | `src/platform/accessibility/` |

**所以这不是「8 个控件要填字段」，而是「两段管道之间缺一个泵」**。
上面的对照还暴露第二件事：`widget_a11y_state` 是**拉取式**的，而屏幕阅读器需要的是
**推送式**的（焦点一动就要收到通知，而不是等它来问）——这是 §6.3 存在的第二个理由。

**取证 6：`AnimationDriver` 有三层引擎，`Widget` 层零接线。**

```text
src/style/animation.rs
  EasingFunction      10 变体（Linear…BackOut）
  struct PropertyAnimation   from/to/current/…
  impl AnimationDriver  animate / animate_linear / animate_ease / tick(delta)
  pub fn animate_state_transition(driver, theme, from, to, on_tick)
src/view/engine.rs:25
  「animation (which lives in PropertyAnimation rather than in build, for the same reason)」
```

结论：`Widget` 层用**每控件三个私有字段**（`interaction_progress` / `interaction_target` /
`interaction_target_progress()`）重新实现了一遍 `PropertyAnimation` 已经提供的东西。
BLUE23 §3.2 把 `tick` 上提为 trait 方法是对的，但**没有同时把「属性驱动」也上提**。

**取证 7：设置与状态存放在端口化（每平台一句），但环境事实没有端口。**

```text
src/layout/                        ← 端口化成功：15 种布局实现，一处
src/widget/metrics.rs              ← 端口化成功：一张 dimensions 表，一处
src/theme/                         ← 端口化成功：一层 token
src/i18n/                          ← 端口化成功：一个 manager
src/platform/accessibility/        ← 端口化成功：一个桥 trait
────────────────────────────────────────────────────────────
「哪个控制器在滚动」                  ← 未端口化：自绘走运行时，原生走宿主，互不知情
「文本缩放 / 缩放系数」               ← 未端口化：`implict_size(content, padding, floor)` 的
                                       两个入参就叫 `scale`，但没有任何地方持有它
| 「无障碍树」                          ← 未端口化：推导有、桥有，**中间那段没有**
```

---

## 0B. 缺口总表（本计划的八个部分）

| # | 缺口 | 实跑现状 | 用户能感知的后果 | 本计划节 |
|---|---|---|---|---|
| 1 | **没有连续帧循环** | `tick_animations` 全仓零调用者 | 库里的动画**永不发生** | §1 |
| 2 | **属性动画未上提到 `Widget` 层** | 11 个控件各持 3 个私有字段 | 每加一个动效就要再抄三遍 | §2 |
| 3 | **状态是单值 + 3 个正交态挤在同一条优先级链** | `error` / `warning` / `success` 与交互态互相覆盖 | 「校验失败的输入框」在 hover 时**丢掉错误色** | §3 |
| 4 | **环境事实（缩放/密度/偏好/区域）没有单一入口** | `metrics` 收 `scale` 参数，但无人持有它 | 系统改了字体大小，控件不变；系统开了「减少动效」，动画照播 | §4 §5 |
| 5 | **a11y 缺「推导→桥」之间的提交者** | `from_widget` 与三个桥都在；挂载期零提交 | 屏幕阅读器读不到任何控件（推也推不到、拉也无人拉） | §6 |
| 6 | **hit-test / 焦点的分层只覆盖自绘控件的顶层窗口** | 原生控件有自己的 hit-test 与焦点环 | 混合（原生+自绘）应用里两条输入通路互相不知情 | §7 §8 |
| 7 | **计划文件本身无单一入口** | `docs/plans/` 有 **47** 份 `*.md`，其中 `blue*` 系列 40 份 | 任意要求可在三处各写一半 | §9 |
| 8 | **「面」的材质没有声明通道**（立体/扁平/浮起） | `role_base_style` 给**每个**控件发**同一个** `Shadow{0,2,6}`；**43 个文件**各自手搓 `blend(&WHITE/BLACK, w)` 斜角；`WidgetStyle.background_gradient` 字段存在而**零控件读**、主题 schema 也没有 | 控件看着「平」且**彼此同层**：一个浮起的面和一个凹陷的面画得一样，主题既无法说「我是扁平风」也无法说「我是立体风」 | §10A |

> **这八条的性质与 BLUE23 相同**：不是「还没写」，而是「机制已建、端口未开」。
> 所以每条都能遵守「修复量随层下降」——**一处接通 N 个消费者**，而不是 188 次抄写。

---

## 1. P0 —— 帧循环：让应用连续地动起来

> **这是全计划的地基。** 不接这一步，§2 的属性动画、§3 的状态过渡、§4 的「减少动效」
> 都无处落（它们全都需要一个会被反复调用的推进点）。

### 1.1 为什么必须有，且只能有一处

现在的形态是「三平台三循环 + 零动画推进」（§0A.1 取证 2）。
错误答案有四个，逐条列出，因为**每一个看起来都合理**：

| 方案 | 为什么错 |
|---|---|
| 每个控件自建定时器 | 100 个控件 100 个定时器，且与帧率不同步（BLUE23 §3.3 已论证） |
| 每个后端各写一个 tick | 就是现在这样：三份循环，改一处要改三处，且新的后端会漏 |
| 在 `draw` 里多走几步 | §0A.1 取证 3 已证明：`draw_bridge` **故意**用固定 16 ms，因为多走几步会让长帧把动画传送到底 |
| 宿主自己写循环 | 那「不闪的光标」就是宿主的责任——而用户买的是库，不是「请你写个循环」 |

**正确的位置**：`src/widget/runtime.rs` 里一个**帧循环驱动的**函数，
被平台循环**每帧调用一次**，与 `drain_triggers` 同一个心跳、同一种「库拥有逻辑、平台拥有时钟」的分工。

### 1.2 P0-1 `runtime::drive_frame` —— 库侧的帧推进契约

```rust
/// 一个帧的所有库侧工作，按**固定顺序**做一次。
///
/// # 为什么是一个「帧」而不是一个 `tick`
///
/// 现存的三个入口各自只做了一半，而且顺序在三个平台上不一致：
/// `drain_triggers` 排空事件、`tick_animations` 推进动画、`draw_of` 又会在绘制路径上
/// **再**推进一步。这意味着「一个控件这一帧被推进了几次」取决于宿主怎么调，
/// 而正确答案是**恰好一次**，且**必须早于绘制**（否则画的永远是上一帧的状态）。
///
/// # 顺序，以及每一条为什么不能换
///
/// 1. **排空输入** —— 本帧收到的指针/键盘/焦点事件先落地，因为动画的目标值来自状态，
///    而状态来自这些事件。反过来的话，本帧的 hover 要等下一帧才动。
/// 2. **显式推进一次动画** —— `is_animating()` 的控件走 `delta_ms`。
///    放在绘制**之前**，所以本帧画的是推进后的状态。
/// 3. **消费宿主的重绘请求** —— `request_repaint` 累积的脏区在这里变成
///    `platform::invalidate_surface*` 的调用。放在动画之后，因为动画本身可能新增脏区。
/// 4. **汇报是否还需要下一帧** —— 返回值就是 [`animation_bus_needs_another_frame`]，
///    宿主据此决定「继续排下一帧」还是「睡到下一个事件」。
///
/// 第 4 条是「丝滑不以耗电换」的全部机制：一个静止窗口的每一帧成本 = 一次判空。
pub fn drive_frame(delta_ms: u32) -> FrameOutcome;

/// 本帧做了什么，用于诊断与测试（**不是**给宿主判断的，判断用 `needs_another_frame`）。
pub struct FrameOutcome {
    /// 从触发队列排空并分发的事件数。
    pub events_dispatched: usize,
    /// 显式推进过的控件数（`is_animating()` 为真的那些）。
    pub controls_ticked: usize,
    /// 本帧向平台提交的重绘请求数。
    pub repaints_submitted: usize,
    /// 是否需要下一帧。
    pub needs_another_frame: bool,
}
```

### 1.3 P0-2 平台侧：**假**帧循环的删除与**真**帧循环的接线

三个平台各改一处，且**只改「谁拥有时钟」这一件事**，不改各平台的窗口/事件逻辑。

| 平台 | 现在 | 改为 |
|---|---|---|
| Linux（`gtk-native`） | `glib::timeout_add_local(16ms, drain_triggers)` | 同一闭包改调 `drive_frame(16)`；返回 `ControlFlow::Continue` 当且仅当 `needs_another_frame` |
| Linux（无 GTK） | `while running { sleep(16) }` | `while running { if !drive_frame(16).needs_another_frame { /* 睡到下一事件 */ } }` |
| macOS | `drain_triggers_impl` 定时器 | 同一 `NSTimer` 回调改调 `drive_frame(16)`；`needs_another_frame` 决定定时器是否重排 |
| Android / iOS / Harmony | 已有的主线程心跳 | 同上（**只接线，不改各自的事件源**） |

> **`mini` / `embedded` 的处理是明确的**：这两个 profile 没有 `widget::runtime`
> （模块本身是 `#[cfg(not(alloc_frugal))]`），它们**按需整帧重绘**，
> 所以 `drive_frame` 对它们是空操作——**不提供 stub**（沿用 `draw_bridge` 的同一处理）。
> 判据：这两个 profile 编译干净且二进制体积不增。

**判据**：

```text
1. 单测：`drive_frame(16)` 在有一个 hovered Button 的树上返回 needs_another_frame == true，
   且 `controls_ticked == 1`
2. 单测：树上无动画时 needs_another_frame == false，且 repaints_submitted == 0
   —— 即**静止帧零提交**（这条比动画本身更重要）
3. 单测：顺序 —— 构造一个「本帧收到 MouseEnter」的树，`drive_frame` 之后
   该控件的过渡进度**已经前进了一步**（证明排空先于推进、推进先于绘制）
4. 单测：幂等 —— 「一个控件一帧被推进一次」，用计数型测试替身断言 tick 次数 == 1
5. 集成（本机可跑）：Linux 无 GTK 路径的循环 + 一个 hover 序列，
   断言至少在 N 帧后过渡到达终点（**这是「屏幕上真的动了」的第一条端到端判据**）
6. 门禁 `check_single_frame_driver`：
   - `drive_frame` 是全仓唯一同时调用 `tick_animations` 与 `drain_triggers` 的地方；
   - 零处 `pub fn tick(` 的按钮被 `draw_bridge` 与 `tick_animations` **同时**推进
     （BLUE23 §3.3 已警告的「一帧两次」，本轮把它变成门禁）
7. 反向注入：让 `drive_frame` 跳过第 2 步 ⇒ 判据 1 与 5 变红
```

---

## 2. P0 —— 属性动画：把「一个属性在两条值之间」抽成一个类型

> BLUE23 把 `tick` 上提了，但没有把**驱动动效的东西**上提（§0A.1 取证 6）。
> 结果是每加一个动效就要再抄三个私有字段。本计划把这一层补齐。

### 2.1 现状：`PropertyAnimation` 存在，`Widget` 层却重新发明了一遍

实跑对照（同一件事的两份实现）：

| | `src/style/animation.rs` | `src/widget/base_widgets/button.rs` |
|---|---|---|
| 起点/终点 | `from: f32` / `to: f32` | `interaction_target: f32` + 由状态推导 |
| 当前值 | `current: f32` | `interaction_progress.progress()` |
| 推进 | `AnimationDriver::tick(delta)` | `Transition::tick(target, delta)` |
| 曲线 | `EasingFunction`（10 变体） | `Transition` 自带 |
| 中断 | `animate()` 可从当前值重启 | 从当前值继续（`a_press_during_a_hover_re_aims_rather_than_restarting`） |

**两份实现的取舍恰好相反**：`AnimationDriver` 是**时长制、外部驱动、可多属性并发**；
`Transition` 是**目标制、控件自持、一次一个属性**。
本仓需要的是**后者**（它才有 `is_animating()` 的经济学），所以正确做法是
**把 `Transition` 的形态补全为「属性驱动」并让它成为唯一形态**，
而不是把控件迁到 `AnimationDriver`。

### 2.2 P0-3 `PropertyDriver` —— 一个属性、两个值、一条曲线、一个时长 token

```rust
/// 一个数值属性在「当前值」与「目标值」之间的插值，由每帧推进一次。
///
/// # 为什么不是 `AnimationDriver`
///
/// `AnimationDriver` 是**名册制**：调用方注册一个带 id 的动画，由驱动者按 id 推进。
/// 名册的失败形态 BLUE23 §3.3 已经写过（「注册了没注销」的漏）。
/// 本仓需要的是**自述制**：控件自己就是动画的持有者，`is_animating()` 是它对自己
/// 状态的回答。所以本类型与 `Transition` 同族，只是把「一个属性」这件事**类型化**。
///
/// # 为什么不引入泛型 `T` 的插值 trait
///
/// 现在需要驱动的属性只有两类：**标量进度**（0..1，按钮/开关/遮罩）与
/// **像素位移**（拇指 x、面板高度）。二者都是 `f32` 的线性插值，
/// 引入 `Interpolate` trait 只会让调用点多写一个 `where`（原则 #28）。
/// 若将来出现「颜色插值」，它应当是一个**新的具名方法**而不是一个泛型参数。
#[derive(Debug, Clone)]
pub struct PropertyDriver {
    current: f32,
    target: f32,
    /// 时长 token，取自 `theme.motion`（`crate::theme::Motion` 的
    /// `fast`/`normal`/`slow` 三档）。**不允许字面量**
    /// （BLUE23 §3.3 判据 5 要求门禁，本计划把它落地，见 §2.4）。
    tempo: MotionSlot,
    /// 进入与退出各自的曲线。离开用 easing、进入用 `Motion::easing`，
    /// 这是 BLUE23 §3.4 已采纳的唯一一条（前向/反向各自一条）。
    approach: EasingFunction,
    retreat: EasingFunction,
}

/// `Motion` 三档时长在动画侧的名字。
///
/// 不直接用 u32：一个 `u32` 参数无法回答「这是哪一档」，而门禁 §2.4 要断言的
/// 恰恰是「它来自 `Motion` 而不是一个恰好等于 200 的数」。
/// 也与 `Motion` 本身分开：`Motion` 是**主题数据**，它是**引用**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSlot {
    /// `Motion::fast` —— 对指针的直接反应。
    Fast,
    /// `Motion::normal` —— 控件自身的状态变化。
    Normal,
    /// `Motion::slow` —— 较大的展开/收起。
    Slow,
}

impl PropertyDriver {
    /// 从 `value` 出发，静止。`approach` 用于朝向目标的移动。
    pub fn at(value: f32, tempo: MotionSlot) -> Self;
    /// 设定目标。**同值不重启**——这是「丝滑」与「卡顿」的分界
    /// （BLUE23 §6 表 #7：中断不重启）。
    pub fn set_target(&mut self, target: f32);
    /// 推进 `delta_ms`，返回「是否还需要下一帧」。
    pub fn tick(&mut self, delta_ms: u32) -> bool;
    /// 当前值。
    pub fn value(&self) -> f32;
    /// 当前是否在移动（**不推进**，只回答）。
    pub fn is_moving(&self) -> bool;
    /// 直接跳到 `value`，下次 `tick` 不会再动。
    pub fn jump_to(&mut self, value: f32);
}
```

### 2.3 P0-4 三个控件的迁移（原型 → 上提 → 推广）

| 步骤 | 内容 | 判据 |
|---|---|---|
| **原型** | `Button` 的 `interaction_progress: Transition` → `PropertyDriver`；`interaction_target: f32` 字段**删除**（目标由 `widget_state()` 推导，不再缓存） | 现有 4 条 `button` 过渡单测**逐条仍绿**，不新增断言 |
| **第二个** | `Switch` 的 `travel` 与 `ToggleButton` 的过渡走同一类型 | 三帧几何判据（BLUE23 §0.3 的统一形态） |
| **推广判据** | `grep -rn "Transition" src/widget/` ⇒ 仅剩 `PropertyDriver` 的适配层；`interaction_progress` 这类字段名归零 | 门禁 `check_animation_state_is_one_type` |

### 2.4 P0-5 补上 BLUE23 §3.3 判据 5：自驱动时长必须来自 `theme.motion`

**这是一条「计划已写、从未执行」的判据**（`blue23.md` §3.3 判据 5；
`log-20260923-2.md` §4.4 只核对了判据 1/2/3，§4.5 落地的是另一条
`check_animation_has_a_driver`）。本计划把它补上，并扩展为**两条**：

```text
门禁 A `check_animation_durations_are_tokens`（扩展既有 check_transition_durations_are_tokens）：
  1. 控件自驱动动画的时长必须取自 `theme.motion`（`MotionSlot` ⇒ `Motion::fast/normal/slow`）；
  2. 允许的例外必须进显式 allowlist **并附理由**（#108 ③ 的形态），
     例如「帧步长常量 `ANIMATION_FRAME_DELTA_MS = 16`」是**节奏**不是**时长**，理由写进表。
  反向注入：把 `Button` 的时长换成字面量 200 ⇒ 变红。

门禁 B `check_first_value_not_zero`：
  1. 一个动画型控件的 `PropertyDriver` 初值必须是**静止端**而不是目标端
     （反例已存在：`PieMenu::animation_progress` 初值 1.0，于是构造即「已展开」）；
  2. 反向注入：把某个控件的初值改到目标端 ⇒ 变红。
```

> **门禁 B 的由来**：`button.rs:113` 的注释自己记过这个坑
> （"Starting at the interactive end would make every button fade *out* on its first frame"），
> 而 `PieMenu` 至今是这个形态。**同一条教训，一处写了、一处没写** —— 这正是门禁该拦的。

---

## 3. P0 —— 正交状态：让 `error` / `warning` / `success` 与交互态不再互相覆盖

> BLUE23 §2.3 用「单值决定填充、`StateOverlay` 表达同时成立」回避了集合爆炸。
> 那个取舍是对的。但它留下一个**具体的、可复现的**后果，本计划必须收掉。

### 3.1 缺陷（可复现）

`WidgetState` 的 12 个变体里，`Error` / `Warning` / `Success` 三个**与交互态正交**：
一个输入框可以**同时**是「hovered」与「校验失败」。而 BLUE23 §2.2 的默认实现是：

```rust
if !enabled { Disabled }
else if pressed { Pressed }
else if hovered  { Hover }        // ← hovered 的输入框在这里返回，Error 永远轮不到
else if draws_focus_ring() { Focused }
else { Normal }
```

**后果**：一个校验失败的输入框，只要鼠标停在上面，主题作者写的
`"line_edit:error"` 就**不生效**。这是「错误提示在用户把鼠标移开之前是看不见的」——
而用户把鼠标移上去，恰恰是因为想看哪里错了。

### 3.2 P0-6 状态分成三条通道，各有一条**独立**的解析路径

```rust
/// 与交互无关的语义状态。可以**叠加**在任一交互态之上。
///
/// # 为什么这不能挤进 `WidgetState` 的优先级链
///
/// `WidgetState` 的语义是「用哪一套颜色」，它的键是**单值**（BLUE23 §2.3 已论证
/// 改成集合会让状态间过渡组合爆炸）。而 `error` / `warning` / `success` 不是
/// 「另一种交互」，是**对同一个交互的补充信息**：一个被 hover 的失败输入框
/// 应当同时满足 `:hover`（填充）与 `:error`（描边）。
///
/// 这正是 BLUE23 引入 `StateOverlay` 的同一个理由——**填充与描边是两条通道**。
/// 本类型是第三条：**描边色的来源**与**填充色的来源**分开。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SemanticState {
    /// 无额外语义。
    #[default]
    None,
    /// 校验失败：**描边**取 `Colors::error`（而不是填充）。
    Error,
    /// 非致命警告。
    Warning,
    /// 动作成功确认。
    Success,
}
```

`Widget` trait 上新增一个**默认返回 `None`** 的方法（纯加法，遵守 #21）：

```rust
/// 本控件当前的语义状态，与交互态独立。
///
/// 默认 `None`，所以 187 个控件不必同时改完——与 `widget_state()` 上提时同一手法。
fn semantic_state(&self) -> SemanticState { SemanticState::None }
```

### 3.3 解析顺序（一处，写在文档里，并由门禁钉住）

```text
填充色：widget_state()（单值，可过渡）
描边色：semantic_state() 若有，否则 widget_state() 的描边
叠加层：StateOverlay（hovered / pressed / focused / checked，不过渡）
焦点环：始终最后画，不参与上面三条
```

**判据**：

```text
1. 单测：一个 `LineEdit` 同时 `set_error(true)` 与收到 `MouseEnter`：
   - `widget_state() == Hover`（填充走 hover）
   - `semantic_state() == Error`（描边走 error）
   - 两者**同时**体现在绘制输出里（描边色 == `Colors::error`，且填充 ≠ 静止填充）
   —— 当前**不可能**通过（`semantic_state` 不存在）
2. 单测：`semantic_state() == None` 时，描边色与 BLUE23 的产出**逐字节相同**
   （静止态快照不变的「安全绳」，§11 判据 24 的延续）
3. 单测：语义色不进入 `WidgetState` 的过渡表（`WidgetState::Error` 等三个变体
   仍存在以便向后兼容，但**不再由 `widget_state()` 返回**；见 §3.4）
4. 门禁 `check_semantic_state_is_not_in_the_interaction_chain`：
   断言 `impl Widget::widget_state` 的任何默认或覆写路径都不返回
   `Error|Warning|Success`（结构性断言，不是行为性）
5. 反向注入：让 `widget_state()` 在 error 时不返回 `Hover` ⇒ 判据 1 变红
```

### 3.4 P1 —— `WidgetState` 三个语义变体的去留（**已裁定：选 A**）

BLUE23 没有做这个裁定，于是它们现在处于第三态（存在、但没有任何控件返回）。

**裁定（用户确认）：选 A —— 保留并冻结。**

| 选项 | 做法 | 裁定 |
|---|---|---|
| **A. 保留并冻结** | 三个变体保留（`pub` 形状不动，#21），文档写明「由 `semantic_state()` 承载，`widget_state()` 永不返回它们」，并由门禁 4 钉住 | ✅ **采纳** |
| ~~B. 删除~~ | ~~从 `WidgetState` 移除三个变体~~ | ❌ 不采纳 |

**选 A 的理由**（三条，逐条可验）：

1. 它们是 `pub` 且 `Eq + Hash`，被 `StatefulTheme` 当 `HashMap` 键、被 `set_transition((from, to), ms)`
   当**二元组**用（`theme_state.rs:211,246`）；删除会动公开形状，违反 #21。
2. 已经有下游主题文件与测试**按名字**引用它们（`state_suffix` 的 12 个字符串，
   `manager.rs:470-485`）：删除会让旧主题文件**静默失效**而不是报错。
3. `semantic_state()` 是**新增**通道，两者并存没有歧义——
   只要门禁 4 能证明 `widget_state()` **永不返回**它们，就不存在「两个真相」。

**选 A 带来的义务**（不是「放着不管」）：

```text
A-1. `WidgetState::Error|Warning|Success` 的 rustdoc 必须写明它们的**唯一**归属：
     「由 `Widget::semantic_state()` 承载；`widget_state()` 的默认实现与任何覆写
      都不得返回这三个变体」（否则读者会以为它们还能用这条路径）
A-2. 三个变体保留在 `state_suffix` 表中，但 `resolve_style_for_state` 的文档必须
     说明「经 `widget_state()` 到达这里的调用不会命中这三条」
A-3. 门禁 4 是这条裁定的**执行者**（不是可选建议）：结构性断言而不是行为性断言
A-4. §3.4 本身不再留「待裁定」字样：本节现在是一个**已决结论**
```

**判据（补 §3.3）**：

```text
6. 三个变体仍在 `pub enum WidgetState` 中（编译期形状不变）
7. 三者的 rustdoc 均含「由 `semantic_state()` 承载」字样
   （由 `tools/check_docs.sh` 的词表或一条新断言覆盖）
8. 门禁 4 正向通过（`widget_state()` 零返回），反向注入仍能变红
```

---

## 4. P0 —— 环境事实：一个入口，回答关于设备的全部问题

> 这是本计划最重要的一条，因为它**同时**是 §5（自适应）与 §6（a11y「减少动效」）的前置，
> 也是「库在别人的设备上到底是什么样」这个问题的唯一答案。

### 4.1 现状：端口化在这个维度上**没有发生**

| 维度 | 现状 | 证据 |
|---|---|---|
| 缩放 | `implicit_size(content, padding, floor)` 的两个消费点各自传 `scale`，**无人持有它** | `metrics.rs:75` 的 `estimate_text_width(text, font, scale)` |
| 密度 | `dimensions` 表有「density step」条目，但**没有读取者** | `metrics.rs` 的 `dimensions` 表 |
| 主题模式 | `ThemeStateManager` **有**（`light`/`dark`/`auto`），但依赖壁钟猜小时 | `theme_state.rs:377` 用 UTC 小时窗口 |
| 区域 | `I18nManager` 有 `set_language`，**无默认值来源** | §0A.1 取证 4 |
| 「减少动效」 | 不存在 | 无 |
| 高对比度 | 不存在 | 无 |
| 文本方向 | `TextDirection` 有，**逐控件传参** | `slider.rs:39` `direction: TextDirection` |
| 左右手/镜像 | 不存在 | 无 |

**共同形状**：这八个问题在**每一个**消费点各被问一次，或者干脆没被问。
`theme_state.rs:377` 的 `should_use_dark` 是最典型的症状——
它为了回答「现在该不该暗」去读了 `SystemTime` 并与一个手写的 UTC 小时窗口比较。
`mini` 下读不到钟，于是它**永远返回 false**（即永远浅色），而这段取舍的过程
被写进了注释——**这说明作者知道这里不对，只是没有那个端口可以用**。

### 4.2 P1-1 `EnvironmentProvider` —— 关于设备的事实，全部走一个接口

```rust
/// 关于**这台设备此刻**的事实。
///
/// # 为什么这必须是一个接口，而不是一组全局变量或一堆 cfg
///
/// 上面八个维度的共同点是：**它们的权威都不在库里**。系统的文本缩放、用户的
/// 区域设置、系统的「减少动效」开关，这些都是宿主的事实，而库只能询问。
/// 用 `cfg`、环境变量或全局常量来回答它们，就会把「这台设备恰好如此」
/// 写成「所有设备都是如此」——`theme_state.rs:377` 的壁钟窗口就是这一形态，
/// 它在 `mini` 上退化为「永远浅色」而不是「我读不到，请宿主告诉我」。
///
/// # 与 `Platform` trait 的关系
///
/// **不合并**，理由是可测性：`Platform` 的每个方法都涉及真实窗口/OS 对象，
/// 于是在本机只能靠 `RecordingInvalidations` 这类替身。而本接口的**全部**方法
/// 都能在纯逻辑下构造（这正是「测试替身」要求的形态），
/// 所以它能在 `mini` 上编译、能在单测里给任意组合、能被 188 个控件的测试复用。
///
/// # 缺省的诚实
///
/// 每个方法都有缺省值，且缺省值是**中性且可声明的**（缩放 1.0、区域 `None`、
/// 动效 `Full`），而不是「假装的值」（原则 #37 的形态）。
pub trait EnvironmentProvider {
    /// 系统文本缩放（a11y 字号）。`1.0` = 不缩放。缺省 1.0。
    fn text_scale(&self) -> f32 { 1.0 }
    /// 布局缩放（DPR 之外的 UI 密度）。缺省 1.0。
    fn layout_scale(&self) -> f32 { 1.0 }
    /// 界面区域（BCP-47）。`None` = 库无法知道，由宿主显式 `set_language`。
    fn locale(&self) -> Option<&str> { None }
    /// 系统首选外观。`Auto` = 跟随（此时**不猜壁钟**，由宿主决定）。
    fn color_scheme(&self) -> ThemeMode { ThemeMode::Light }
    /// 「减少动效」偏好。`Reduced` 时**一切**过渡时长视作 0（见 §4.4）。
    fn motion_preference(&self) -> MotionPreference { MotionPreference::Full }
    /// 高对比度偏好：`true` 时层级色让位于 `outline`。
    fn high_contrast(&self) -> bool { false }
    /// 文本基底方向。缺省 LTR。
    fn text_direction(&self) -> TextDirection { TextDirection::Ltr }
    /// 是否镜像（左右手）。
    fn mirroring(&self) -> bool { false }
}
```

### 4.3 P1-2 一个进程一份，且**可替换**（这是可测性的全部）

```rust
/// 安装环境事实。返回被替换掉的那一份（便于测试还原）。
///
/// # 为什么是「安装」而不是「读取平台」
///
/// 单测要能造出「文本缩放 2.0 + 减少动效 + 高对比度」这种组合，而真实系统上
/// 造不出来。所以库读的是一个**已安装的** provider，缺省是
/// `DefaultEnvironment`（全部中性值）。宿主在启动时安装真正的那个。
pub fn install_environment(env: Box<dyn EnvironmentProvider>) -> Box<dyn EnvironmentProvider>;

/// 当前生效的环境事实。**唯一的读取点。**
pub fn environment() -> EnvironmentSnapshot;
```

`EnvironmentSnapshot` 是**值类型**（八个字段的拷贝），不是引用——
因为绘制路径在热路径上，逐个虚调用会把「静止帧零成本」这条性质破坏掉
（§1 判据 2）。**快照的刷新时机是帧首**（`drive_frame` 的第 0 步），
于是「本帧内环境一致」是一条可断言的性质。

### 4.4 P1-3 「减少动效」必须是**一处**短路，不是每个动效各写一个 `if`

```rust
/// 一个过渡在考虑环境偏好之后的**实际**时长。
///
/// 这是 `Reduced` 偏好的**唯一**实现点：时长归零 ⇒ `PropertyDriver::tick` 立刻到达终点
/// ⇒ 动画在**一帧**内完成，而 `is_animating()` 立刻转 false。
///
/// 为什么不「跳过动画直接设值」：那会让「减少动效」变成 188 个控件里各写一个
/// `if reduced { set_final() } else { animate() }`。这一条把两种行为**收敛成同一条代码路径**，
/// 只是时长不同——于是「减少动效下没有动画」是**数学结论**，不是逐控件的约定。
pub fn effective_duration(tempo: MotionSlot, env: &EnvironmentSnapshot) -> Duration;
```

**判据**：

```text
1. 单测：安装 text_scale = 2.0 的 provider ⇒ 一个 Button 的 `implicit_size`
   的高度**严格大于** 1.0 时的高度，且文字墨宽约为两倍（容差内）
2. 单测：安装 motion_preference = Reduced ⇒
   - `effective_duration(..) == 0`
   - 控件在 `drive_frame(16)` **一次**之后 `is_animating() == false`
   - 且**最终几何**与 Full 偏好下走完全程后的几何**相同**（不是「动到一半就不动」）
3. 单测：`environment()` 在 `drive_frame` 之内**恒等**（帧内一致）
4. 单测：未安装时全部取中性缺省，且 `mini` profile 下编译通过（`std`-free）
5. 单测：`should_use_dark` 在 `Auto` 下**不再读壁钟**：
   改为读 provider 的 `color_scheme()`；`mini` 的 `#[cfg(alloc_frugal)] false` 分支删除
   （它回答的是「我读不到钟」，而正确回答是「宿主告诉我」）
6. 门禁 `check_environment_is_single_sourced`：
   除 `EnvironmentSnapshot` 的字段外，零处 `SystemTime::now()` / 环境变量 /
   `cfg(target_os)` 参与决定上面八个维度
7. 反向注入：删掉 `effective_duration` 的 Reduced 短路 ⇒ 判据 2 变红
```

---

## 5. P1 —— 自适应：让「同一棵树在不同尺寸下都合理」成为可声明的

> 现代 GUI 框架的招牌能力是「一次声明，四种规格」。本仓**已有全部零件**
> （15 种布局、`Hints` 三值、`LayoutParams`、度量表），**缺的是把它们按尺寸选择**。

### 5.1 P1-4 `Breakpoint` + `adaptive!`：按尺寸选子树，而不是按尺寸算坐标

**为什么不是「一个控件内部 `if width < 600`」**：那会让「手机布局」藏在每个控件的
draw 里，无法整体检视；而且 `Hints` 的三值是**向上传播**的，一个控件在抽屉里和在
主区里的固有尺寸不同，`if` 写在 draw 里读不到这个上下文。

```rust
/// 一个尺寸断点。
///
/// 三档而不是四档：`Compact`（窄/手机）· `Medium`（平板）· `Expanded`（桌面）。
/// 为什么不是五档（如某框架的 compact/medium/expanded/large/extra-large）：
/// 本仓的 `dimensions` 表与 `Hints` 已经能表达「多宽」，再多一档只是**命名**
/// 而没有**行为**差异（原则 #104：没有语义区分的档位就是数字摆设）。
pub enum Breakpoint { Compact, Medium, Expanded }

impl Breakpoint {
    /// 由一个可用尺寸判定。阈值来自 `dimensions` 表，**不得是字面量**。
    pub fn of(available: Size) -> Self;
}
```

`Node` 上新增：

```text
Node::breakpoint(child)   // 只在当前断点渲染该子树
```

以及**一次**而非逐控件的判据：`Breakpoint` 只影响**选了哪棵子树**，
不影响任一控件的内部算术（那已由 `Layout` + `Hints` 负责）。

**判据**：

```text
1. 单测：同一棵树在 `Compact` 与 `Expanded` 下产出的 `Node` 树**节点数不同**
   （证明选的是子树，不是坐标）
2. 单测：切换断点不产生「半个控件」——被选中的子树通过 `Hints` 报出**它自己的**固有尺寸
3. 几何判据（BLUE23 §0.3 的形态）：同一 `View::build` 在 320 / 700 / 1200 三个宽度下
   各取一帧快照，**三个几何两两不同**，且无一个控件与邻居重叠
4. 门禁 `check_breakpoints_are_not_in_draw`：
   `src/widget/**` 的 `draw` 里不得出现「按 `rect.width` 选布局分支」的形态
   （允许按 `rect.width` 算**尺寸**，不允许选**结构**）
5. 反向注入：让 `Breakpoint::of` 恒返回 `Expanded` ⇒ 判据 1 变红
```

---

## 6. P0 —— 无障碍：把已建好的两段管道接通

> §0A.1 取证 5：**①推导在、③桥在、②中间那段不在**。
> 这个形状比「什么都没做」更难发现：`A11yState::from_widget` 有完整文档、有单测（含三态），
> 三个平台的桥也完整，于是任何静态阅读都会以为无障碍己经做完了。
> 缺的是「**什么时候推、推什么**」——而它恰恰是唯一不能由单测代替的部分。

### 6.1 为什么它是 P0 而不是 P1

因为它**同时**是三条线的共同前置：屏幕阅读器（读）、键盘导航的语义（走）、
以及 §4 的「减少动效 / 高对比度」（看）。三条线都要求「库知道每个控件的语义身份」，
而这个身份现在**能算出来但没有被送出去**。它也是唯一一个
**用户群体最窄、后果最严重**的缺口。

### 6.2 P0-7 已有的 `A11yState::from_widget` 就是那一处推导（**不重做**）

本轮**不新增** a11y 推导逻辑。已竣工的那一段遵守了「一处事实一处抄写」：
推导在 `A11yState::from_widget`，走的是控件自己的**属性契约**
（与 [`Widget::accessible_value`] 同源，所以二者不可能对同一个控件给出不同答案）。

本轮要补的是它旁边的两件事：

| 缺口 | 实跑证据 | 本计划节 |
|---|---|---|
| 挂载/卸载/状态变化时**没有推** | `bridge.set_accessibility_name` 零生产调用点 | §6.3 |
| `checked`/`mixed` 之外的三态**控件自己才知道**，但契约里已有则无需新增 | 已由 `from_widget` 处理 | —— |

### 6.3 P0-8 提交者：`runtime` 把树的变化**推**给当前平台的桥

为什么必须推（不能只靠 `widget_a11y_state` 这个拉取式入口）：

```
屏幕阅读器的工作方式：控件进入/改变 ⇒ 它收到通知 ⇒ 它去读。
若库只能「被问」，那个「问」永远不会发生 —— 因为屏幕阅读器的触发源就是本库的通知。
这是「一个开环」：没有推，拉取点就没有理由被调用。
```

提交点（**共五处**，尽量少，以便每一处都能配一条断言）：

```text
1. 控件挂载            ⇒ 建节点（role + label + bounds）
2. 控件卸载            ⇒ 拆节点
3. geometry / size 变化 ⇒ 更新 bounds
4. widget_state() 变化  ⇒ notify_state_changed（含 enabled/focused）
5. semantic_state() 变化 ⇒ notify_state_changed（§3 的第三个消费者）
```

第 5 条是本计划新增的：§3 把语义态从交互态里分出来之后，它需要一个消费者，
而 a11y 是它最自然的消费者（「这个字段校验失败」就是要被朗读的事）。

**判据**：

```text
1. 单测（用计数型桥替身，本机可跑）：挂载 3 个控件 ⇒ 桥收到 3 次建节点，
   每次带非空 `role` 与 `label`
2. 单测：`Label` 的 label == 它的文本；`Button` 的 == 它的标题
   （这两条测 `From<WidgetKind>` 的**语义正确性**，而不只是「有值」）
3. 单测：`CheckBox` 的三态（`false`/`true`/`mixed`）都能经由提交路径读出
4. 单测：`is_enabled` 变化 ⇒ 恰好一次 `notify_state_changed`；
   同值重复写入 ⇒ **零次**（幂等，与 §7.2 的合并同一原则）
5. 单测：未安装桥的后端 ⇒ 全部调用为 no-op，且**不 panic**（`mini`/`embedded` 同）
6. 门禁 `check_a11y_has_a_producer`：
   `AccessibilityBridge::set_accessibility_name` 在 `src/` 内
   **必须**有除 `a11y_wiring.rs` 之外的生产调用点。
   （现状：零个 ⇒ 门禁立刻报红，这就是它存在的理由）
7. 集成（本机可跑，不满足则**诚实 SKIP**）：Linux + `linux-a11y` feature
   + 可用的 a11y bus ⇒ 真实 D-Bus 上收到至少一次 `notify_*`
8. 反向注入：删掉挂载路径的提交 ⇒ 判据 1 变红
```

> **判据 7 的诚实**（沿用 BLUE22 §F.3 教训 9、BLUE23 §0A.5）：本机没有 a11y bus 时，
> 它必须报 `SKIP(host-limited)` —— **既不算 PASS 也不算 FAIL**，
> 与 `check_android_cross.sh` 同一处理，不造假绿。

---

## 7. P1 —— 原生控件参与重绘与状态（自绘/原生混合的第三阶段）

> BLUE23 让自绘控件有了正确的状态与动画。**原生控件（`gtk-native` / Win32 / AppKit）
> 仍然在库的视野之外**：库不知道它们的空闲状态，原生控件不知道库的状态。

### 7.1 缺陷的具体形态

| 事实 | 自绘 | 原生 |
|---|---|---|
| 谁拥有像素 | 库（`render_frame*`） | 平台 |
| 谁拥有 hover/press | `BaseWidget`（BLUE23） | 平台 |
| 谁拥有 hit-test | `runtime::widget_at` | 平台 |
| 谁拥有焦点环 | `FocusRing` | 平台 |
| 谁排下一帧 | `drive_frame`（§1） | 平台 |

**后果**：同一进程里两个世界。一个 `gtk::Button` 被 hover 时，
库的 `BaseWidget::hovered` 是 `false`，于是 `widget_state()` 报 `Normal`，
于是主题里写的 `"button:hover"` **对原生按钮不生效**。

**取证（实跑）**：平台层自己发起的重绘请求共 **26 处**，全部绕过库的脏表：

```text
$ grep -rn "queue_draw()\|setNeedsDisplay" src/platform/ --include=*.rs | wc -l
26

$ grep -rln "queue_draw()\|setNeedsDisplay" src/platform/ --include=*.rs
src/platform/linux/platform_impl.rs
src/platform/linux/canvas.rs
src/platform/macos/canvas.rs
```

这 26 处里的**大多数是正确的**（它们在同一次交互里既排了重绘、也把事件转给了库），
所以本项**不是**「把它们改成调用库」，而是「**让库知道**」。区别在判据上：
§7 判据 2 要求这些请求**计入帧账并被合并**，而不是要求它们消失。
allowlist 的意义在于把「哪些是平台自己必须做的」显式写下来，而不是默默放过 26 处。

### 7.2 P1-5 `RedrawSink`：平台侧的重绘请求进同一张脏表

```rust
/// 平台告诉库「这个表面需要重绘」的唯一入口。
///
/// # 为什么必须走库的脏表而不是平台自己 queue_draw
///
/// 三个后果，每一个都是「状态分层」被破坏：
/// 1. `performance::render_dirty_regions` 算出的脏区与实际提交的重绘**不一致**
///    ⇒ 「本帧画了什么」不可回答；
/// 2. `drive_frame` 的「静止帧零提交」判据（§1 判据 2）失效 ⇒ 无法证明不耗电；
/// 3. 自绘与原生各自计一份重绘 ⇒ 同一次交互提交两次。
///
/// 所以平台的原生控件在用任何方式请求重绘时，**先经过这里**。
pub fn notify_native_redraw(id: ObjectId, rect: Option<Rect>);
```

### 7.3 P1-6 原生控件的状态回报进 `BaseWidget`

原生后端在事件回调里已经有完整的输入事实（它必须处理输入才能工作），
所以这一步是**回报**而不是**探测**：

```text
GTK:     enter-notify-event / leave-notify-event / button-press-event / focus-in-event
         ⇒ runtime::report_state(id, StateFact::Hovered(true) | Pressed(true) | …)
Win32:   WM_MOUSEMOVE / WM_LBUTTONDOWN / WM_SETFOCUS ⇒ 同上
AppKit:  mouseEntered: / mouseDown: / becomeFirstResponder ⇒ 同上
```

于是 `widget_state()` 对**原生**控件也是真的，主题的 `"<kind>:hover"` 对两个世界同时生效。

**判据**：

```text
1. 单测（替身后端）：报告 `StateFact::Hovered(true)` 后，
   一个原生按钮的 `widget_state() == Hover`，且主题的 `:hover` 填充被采用
2. 单测：`notify_native_redraw` 之后，`drive_frame` 的 `repaints_submitted`
   **计入**这一次，且同一帧内重复调用被**合并**（不是两次提交）
3. 单测：平台既不回报状态也不请求重绘时，库侧行为**逐字节不变**
   （静止态快照的「安全绳」，§11）
4. 门禁 `check_native_redraw_goes_through_the_runtime`：
   `src/platform/**` 中零处直接调用平台自身的重绘 API
   （现在的 `queue_draw` 调用点需逐处裁定：允许的进 allowlist 并附理由）
5. 反向注入：删掉 `report_state` 的 Hovered 分支 ⇒ 判据 1 变红
```

> **诚实边界**：原生控件的**外观**由平台画，库不可控。本项只统一
> **状态与重绘的记账**，不承诺「原生与自绘看起来一样」——那是平台的事实。

---

## 8. P1 —— 帧的账：让「本帧画了什么、为什么」可回答

> §1 建立了帧循环，§7 把原生重绘纳入脏表。本节让这张账**可查**。

### 8.1 P1-7 `FrameStats` —— 每帧一份可导出的记录

```rust
/// 一帧的成本，按**产生它的东西**分组，而不是一个总数。
///
/// # 为什么不是「一个耗时数」
///
/// 「这一帧花了 12 ms」不能让人做任何决定：可能是某个控件重绘全屏，
/// 可能是脏区合并失败，也可能是动画的帧数比需要多。所以账要按
/// **消费者**记：谁被推进了、谁被重绘了、脏区合并省下了多少。
pub struct FrameStats {
    pub index: u64,
    pub delta_ms: u32,
    pub events: usize,
    pub controls_ticked: usize,
    pub controls_drawn: usize,
    /// 请求的重绘矩形数**减去**合并后的提交数：合并省下的次数。
    pub repaints_coalesced: usize,
    /// 每个控件最后一次「因为什么」被重绘（`Overlay`/`State`/`Animation`/`Explicit`）。
    pub last_repaint_reason: Vec<(ObjectId, RepaintReason)>,
}
```

`RepaintReason` 是**根因**而不是结果——它直接回答「这个动画为什么还在跑」。

**判据**：

```text
1. 单测：一个静止窗口连续 60 帧 ⇒ 每一帧 `controls_ticked == 0`
   且 `repaints_coalesced == 0`（没有可合并的，因为没有请求）
2. 单测：一个 hovered Button 从进入到达终点 ⇒ `controls_ticked` 递减到 0，
   且最后一帧因为「动画结束」而重绘（不是「还在动」）
3. 单测：同一帧内 5 个控件请求同一矩形 ⇒ 提交数 **1**，`repaints_coalesced == 4`
4. 单测：`last_repaint_reason` 对每个被重绘的控件都有条目（**无匿名重绘**）
5. 门禁 `check_no_anonymous_repaint`：`invalidate_surface*` 的每个调用点
   都必须带一个 `RepaintReason`（现在这些调用点是裸的 ⇒ 门禁报红，这是它的价值）
```

> **与 `performance` 模块的关系**：`src/performance/mod.rs` 已有
> `render_dirty_regions`。本项**不另立**一套脏区机制，而是把
> `RepaintReason` 作为**已有的**那条链的入参（原则 #101：同一概念只一条路径）。

---

## 9. P1 —— 计划与门禁自身的单一入口

> BLUE23 §0A.0 确立「未实现项只有一个入口」。本计划把它变成**全仓**规则。

### 9.1 P1-8 `docs/plans/` 收敛

现状（实跑）：`docs/plans/*.md` 共 **47** 份，其中 `blue*` 系列 **40** 份
（`ls docs/plans/*.md | wc -l` → 47；`ls docs/plans/blue*.md | wc -l` → 40）。
任何一条要求都能在 3 处各写一半。这不是文档洁癖问题，它有**可观测的后果**：
BLUE22 附录 G 与 BLUE23 §0A.4 曾各写一半同一件事，直到第 73 轮才合并。

| 动作 | 做法 |
|---|---|
| **本文件唯一入口** | 本文件 §12 是**本计划**未实现项的唯一登记处，不另开文件 |
| **历史归档** | 已完成的 `blue*.md` 移到 `docs/plans/archive/`，保留可追溯但不参与「当前要求」 |
| **一份索引** | `docs/plans/README.md`：一行一份计划 + 「已完成 / 进行中 / 已并入」+ 日期 |
| **并行计划不撞车** | 多份计划**同时活跃**是允许的（BLUE23 与 BLUE24 正在并行）；
收敛的目标是「同一件事不两处各写一半」，不是「全局只许一份文件」 |
| **不许新增** | 新增计划 = 在现有计划里加一节。**除非**该计划的主体已全部完成（此时它才该归档） |

> **本节的门禁必须能表达并行**。若写成「全仓只许一份活计划」，
> 它会在 BLUE23 还在跑的当下**立刻报错**，于是没人会把它接进 CI——
> 这正是 BLUE22 §F.3 教训 4 的同一种失败（门禁的形态决定了它会不会被运行）。
> 所以 `check_single_open_plan` 的语义是：**同一个主题**只能有一份活计划，
> 而不是「全仓只能有一份」。

**判据**：

```text
1. `ls docs/plans/*.md | wc -l` 显著下降：索引 1 + 活计划（可多份并行）+ 参考若干
2. 门禁 `check_single_open_plan`：**同一主题**至多一份未归档计划含 `[ ]` 未实现项标记
   （语义是按主题去重，不是「全仓一份」——两计划可并行）
3. 门禁 `check_plan_archive_has_index`：`docs/plans/archive/` 的每份文件在 README 里有一行
4. 反向注入：把同一件事在两份活计划里各写一半 ⇒ 判据 2 变红
```

### 9.2 P1-9 「门禁数量」自身要有判据

现状：`tools/` 下 **74 个** `check_*.sh` + **84 个** `.py`。

但两个方向都要看：门禁**太少**是 BLUE23 的教训（§2.2 判据 4 与 §3.3 判据 4/5 未落地），
门禁**太多**是另一个方向的失败（没人跑得完就没人跑）。所以判据是**一条元门禁**：

```text
门禁 `check_gates_are_worth_running`：
  1. 每个 `check_*.sh` 必须在 `run_all_gates.sh` 里有条目（无孤儿）；
  2. 每个 `check_*.sh` 的**整轮预算**使 `run_all_gates.sh` 的总时长 ≤ 45 分钟
     （BLUE22 §F.3 教训 4：58×1800s = 最坏 29 小时）；
  3. 每个门禁必须**证明过会红**：`tools/gates_reverse_injection.md` 一行一门禁，
     记录「注入什么、报什么」。缺行的门禁视为**未验证**（与 `NOT-RUN` 同等对待）。
```

---

## 10. 施工顺序与批次（每批 = 一轮）

> 排序依据：**帧循环是 §2/§3/§4 的共同前置**；**环境事实是 §5/§6 的共同前置**；
> §7/§8 互相依赖（脏表要求帧账）；§9 可以完全并行。
> **批 0（面的材质）无前置、也不被任何批次依赖** —— 它排在第一位是因为**用户指令
> 「最优先执行」**，且它只加新类型与新文件，可以在 BLUE23 余项收口前完成。

| 批次 | 内容 | 前置 | 交付判据 |
|---|---|---|---|
| **批 0** | **§10A 面的材质**（`SurfaceStyle` + 两个预设 + 三条门禁）。**只加类型与文件，不碰 `ThemeStyleToken`**（见 §10A.8 风险 5） | 无（**与 BLUE23 余项无依赖**） | §10A 的 13 条；**默认预设下 377 份快照逐字节不变** |
| **批 1** | **§1 帧循环**（`drive_frame` + 三平台接线 + 全仓唯一驱动者门禁） | 无 | §1 的 7 条；**端到端：Linux 无 GTK 循环上动画真的到达终点** |
| **批 2** | **§2 属性动画**（`PropertyDriver` + Button/Switch/ToggleButton 迁移 + 两条时长门禁） | 批 1 | §2 的判据；`interaction_target` 字段名归零 |
| **批 3** | **§4 环境事实**（`EnvironmentProvider` + 快照 + `effective_duration` + 删壁钟） | 无（可与批 1/2 并行） | §4 的 7 条 |
| **批 4** | **§3 正交状态**（`semantic_state` + `LineEdit` 原型 + 两条门禁 + §3.4 裁定**选 A** 已定，只需落实文档与门禁 4） | 批 2 | §3 的 8 条；静止态快照逐字节不变 |
| **批 5** | **§6 a11y**（`runtime` 提交者：五处提交点 + 门禁 + 可选的真实 D-Bus 集成）。**不重做 `from_widget`**（已在） | 批 3 | §6 的 8 条（判据 7 可诚实地 SKIP） |
| **批 6** | **§7 原生统一**（`RedrawSink` + `report_state` + 三平台各自接线） | 批 1 | §7 的 5 条；无原生后端的构建逐字节不变 |
| **批 7** | **§8 帧账**（`FrameStats` + `RepaintReason` + 两条门禁） | 批 6 | §8 的 5 条 |
| **批 8** | **§5 自适应**（`Breakpoint` + `Node::breakpoint` + 门禁 + 三宽度快照） | 批 3 | §5 的 5 条 |
| **并行** | **§9 计划与门禁收敛**（归档 + README + 三条元门禁） | 无 | §9 的判据；**不阻塞任何批次** |
| **收尾** | 全量：`cargo test` + `clippy -D warnings` + 5 profile + `run_all_gates.sh` + 快照再生 | 全部 | §11 |

> **每批只改一类**，diff 才可评审（BLUE22 §9.2 的克制）。
> **只在收尾跑一次全量**（原则 #55/#56）；每个新门禁**必须反向注入证明它会红**。

---

## 10A. P0 —— 面的材质：让「立体 / 扁平 / 浮起」成为可声明的

> **本节是追加的**，编号 `10A` 而不是新起 §14：它是 §10 批次表里的**批 0**，
> 且它和 §2（属性动画）同属「把已经写对的关系收敛成一处」。见 §12 的边界注。

### 10A.0 为什么这是 P0，而不是「美化」

用户的原话是：「**现代控件都有 3 维效果，还有苹果的扁平效果，我这能实现吗？**」

这不是审美请求，是一个**能力缺口**：本仓现在**既画不出立体，也画不出扁平**，原因是**同一处**——
「一个面朝哪边」这个事实**没有地方可以声明**。三个实测：

```text
$ grep -n "fn role_base_style" -A 6 src/theme/manager.rs
let shadow = if theme.borders.shadow {
    Some(Shadow { x: 0, y: 2, blur: 6, color: Color::rgba(0, 0, 0, 60) })   # ← 每个控件同一个
```

**一个阴影发给 188 个控件** ⇒ elevation（浮起层）在视觉上**无法表达**。
BLUE23 §6 表 #12 已经记过这条（「于是 elevation 不能区分层级」），本节是它的落地。

```text
$ grep -rl "blend(&Color::WHITE\|blend(&Color::rgb(255, 255, 255)\|blend(&Color::BLACK" src/widget/ | wc -l
43
```

**43 个文件各自手搓「朝白 / 朝黑」**。它们写的是同一个关系（一条亮边 + 一条暗边），
但**方向由两个几乎相同的代码块的顺序携带** —— 所以「把 inset 反过来」是一次
`copy-paste` 编辑，**没有任何东西能检查**。

```text
$ grep -rn "style.background_gradient" src/widget/ | wc -l
0                    # 字段存在，零个读者；主题 schema 里也没有它
```

**所以要解决的不是「加一个 3D 开关」**，而是给「面」补一个**声明通道**。

### 10A.1 一句话定位

> 本节让主题文件能说「**我是一个扁平风的主题**」或「**我是一个立体风的主题**」，
> 而不是让每个控件各自决定它长什么样。

这与 §0B 的其余七条**同构**：机制（渲染命令）已建，**端口未开**（没有 token 承载它）。

### 10A.2 现状取证（本节实跑，非引用）

| 能力 | 实测状态 | 出处 |
|---|---|---|
| `BoxShadow { offset, blur, spread }` | ✅ **软件后端会画**（偏移矩形 + `box_blur_region`）；SVG 后端 `feGaussianBlur` | `render/backend/paint.rs:208` |
| `DrawGradient` / `DrawConicGradient` | ✅ 两个后端都有 | `paint.rs:199,272` |
| `DrawPath` / `Blur` / `SetBlendMode` | ✅ 都有 | `paint.rs:205,238,269` |
| `Shadow` + `ShadowToken`（serde） | ✅ 已有，主题可改 | `style/primitives.rs:277`、`theme/types.rs:613` |
| `ThemeStyleToken` | ✅ 已能表达 background / border / radius / **shadow** / **opacity** | `theme/types.rs:534` |
| 斜角（bevel）原语 | ✅ **已有**（BLUE23 后追加轮）：`Bevel` + `BevelDirection::{Raised,Inset}`，方向可参数化 | `render/bevel.rs` |
| 斜角**声明通道** | 🔴 **没有**：43 个文件各自手搓 | 上表 grep |
| elevation 分层的**声明通道** | 🔴 **没有**：一个阴影发全部 | 上表 grep |
| 扁平「材质」的**声明通道** | 🔴 **没有**：`background_gradient` 零读者 | 上表 grep |

**结论**：本节**完全不需要新的渲染能力**。它需要的是**一个 token 形状**，
把已经存在的四项能力（斜角、阴影、渐变、透明度）接成一条可声明的通道。

### 10A.3 P0-9 `SurfaceStyle` —— 「一个面」的四个正交维度

**修法（只有一个类型，不加模式开关）**：

```rust
/// 一个「面」的材质。四个维度**正交**，各自可缺省、可单独声明。
pub struct SurfaceStyle {
    pub elevation: Elevation,          // 离页面多远：0 = 贴页，1..=5 = 浮起
    pub bevel: Option<BevelSpec>,      // None = 平；Some = 这个方向与这两个色调
    pub material: Material,            // Solid | Translucent { tint, blur }
    pub hairline: HairlineSpec,        // 边缘由谁画：Outline | Shadow | None
}
```

**四个维度各自解决一个已实测的缺口**：

| 维度 | 解决什么 | 为什么是**正交**的而不是一个 `style: Flat|Material3` 枚举 |
|---|---|---|
| `elevation` | 「浮起层不能区分」——`role_base_style` 现在发同一个阴影 | 一个**扁平但浮起**的面（现代 iOS 卡片）**存在**，所以 elevation 不能和 bevel 合并 |
| `bevel` | 43 个文件手搓的斜角，方向不可检查 | 一个**立体但不浮起**的面（Windows 95 按钮挤在工具条里）**存在** |
| `material` | 苹果的 `regularMaterial`：半透明 + 背景模糊 | 一个**浮起且半透明**的面（macOS 侧边栏）**存在** |
| `hairline` | 本仓**同时**有描边和阴影两套边缘画法，且 §6 表 #12 记过「投影片应由阴影承担，而非描边」 | 「扁平风」的现代做法常常是**只有阴影、没有描边**，这是独立于前三个的一个选择 |

> **这就是「更高明的方法」的实质**：不是把「3D」和「扁平」做成两条代码路径，
> 而是**把两者共同的那一个自由度提出来**（面朝哪边 / 离页多远），
> 让「扁平」= `bevel: None` + `elevation: 0..=2` + `material: Solid`，
> 「立体」= `bevel: Some(Raised)` + `elevation: 0`。
> **两者不是两种风格，是同一个参数空间的两个角** —— 所以中间的三万种组合**免费**得到。

### 10A.4 P0-10 主题侧的落位：**两个已有的覆盖层 + 一个 Rust 侧的角色默认**

BLUE23 §5.5 立过一条规矩：**加 token 就必须同时加消费者，或明写「预留」**。本节遵守它。

**先更正一个我在本节初稿里写错的假设**：我以为主题文件里有一张 `"roles": {...}` 表可以扩展。
实测没有 —— `WidgetRole` 是 **Rust 里的枚举**（`theme/types.rs:116`，`Surface`/`Card`/…），
角色 → 颜色的映射写在代码里（`role_colors`）。**所以「role 默认」这一层不是 JSON，是代码**：

```text
// 1. role 默认（Rust：`role_surface_style(role) -> SurfaceStyle`）
//    一次定义、N 个控件继承 —— 与 `role_colors` 同形、同处，不新增机制
Surface    => SurfaceStyle::solid(),                       // 贴页
Card       => SurfaceStyle::solid().elevated(1),
Toast      => SurfaceStyle::solid().translucent().elevated(3),
PushButton => SurfaceStyle::solid().beveled(Raised),        // 立体风的按钮
```

```text
// 2. class / kind 覆盖 —— **已有机制**（`overrides.styles`），只多四个键
"overrides": { "styles": { "tooltip": { "elevation": 4 } } }

// 3. state 覆盖 —— **已有机制**（"<kind>:<state>"，预设里已有 26 个键），与上同形
"overrides": { "styles": { "button:pressed": { "bevel": "inset" } } }
```

**第 3 层是这套设计最值钱的地方**：BLUE23 §2.4 建好的状态通道
**现在就能表达「按下时凹进去」** —— 而这正是所有 3D 风格按钮的核心交互反馈。
本仓已有这条通道（两套预设各 26 个状态键），只是它今天能改的**只有颜色**。

**所以本节新增的机制只有一处**：`SurfaceStyle` 这一个类型 + 它的 role 默认函数。
两个覆盖层、三个（现在是四个）可覆盖的键，都走**既有**的 `overrides.styles` 通道 ——
**没有新的主题层，也没有新的加载路径。**

### 10A.5 主题预设：把「风格」做成**预设**，而不是做成**代码**

**这里我改掉了自己初稿里的一个设计错误**：初稿提议加一个 `"surface_defaults"` 键。
那会是**第三个覆盖层**，与 §10A.4 的结论（两个已有层 + 一个 Rust 角色默认）自相矛盾，
而且它解决的是一个**不存在的问题** —— 本节要的「整仓换风格」用**既有的**
`overrides.styles` 就够了，且它是数据、可被门禁枚举。

**先量准已有的查找语义**（`resolve_style_for_state`，`manager.rs:303-317`）：

```text
$ sed -n '308,313p' src/theme/manager.rs
let mut style = self.resolve_base_style(class_name);
if let Some(state) = state {
    let key = format!("{class_name}:{}", state_suffix(state));   // 形如 "button:hover"
    ...overrides.styles.get(&key)
}
```

以及 `resolve_base_style` 里的两级：`get(class_name)` 然后 `get(role_key(role))`。
**没有任何通配机制** —— 键必须是具体的 kind 名或 role 名。

所以「整仓换风格」有**两条**可行路径，本节选 **(a)** 并要求先量代价：

| 路径 | 做法 | 代价 |
|---|---|---|
| **(a) 用已有的 role 键** | 主题覆盖**角色**（`surface`/`card`/`toast`/… 十来个）而不是 188 个 kind | **零新机制**：`role_key(role)` 查找已在。代价是「同角色的控件共享一个面」，这正是角色存在的意义 |
| **(b) 加通配匹配** | 让 `overrides.styles` 支持 `*:surface` | **新语义**：一次通配静默影响 188 个控件，必须配「通配命中数」门禁，否则没有账 |

**裁定：选 (a)。** 理由与 BLUE23 §6.1 拒绝 `Material` tonal palette 是同一条：
**加一个能力必须先用尽已有的那个**。role 键已经在、已经有查找路径、已经能被门禁枚举；
通配是**在已有层里塞第二套匹配规则**，而它解决的问题 (a) 已经解决了。

于是预设的差异落在**已有的 role 键**上：

```text
// themes/default.json —— 扁平（现代 iOS / 主流声明式实现 默认）
"overrides": { "styles": {
    "surface": { "bevel": null,     "material": "solid" },
    "card":    { "elevation": 1,    "bevel": null, "material": "solid" }
} }

// themes/dark.json —— 立体（参考工具包 / Windows 经典）
"overrides": { "styles": {
    "surface": { "bevel": "raised", "material": "solid" }
} }
```

> **这条路若量出来不够用，再回来加 (b)**，并把「不够用」写成具体数字
> （例如「N 个 kind 的 role 与其期望的面不一致」）—— 而不是先加通配再找理由。

从而：**同一个控件在两种预设下长成两种风格**，而**控件代码一字不改** ——
这就是「由主题文件实现」这个诉求的正确形态。

> **实现注意**：`themes/*.json` 是**生成物**（`themes/generate.sh` +
> `check_theme_fixtures.sh` 要求 in sync），所以预设要在 Rust 侧（`Theme::default()` /
> `Theme::dark()`）声明，再重新生成 JSON —— 手改 JSON 会被门禁打回。

### 10A.6 判据

```text
--- 面的四维（§10A.3）---
1. 单测：`SurfaceStyle::solid()` 是四个维度的**恒等元**（elevation 0 / bevel None /
   material Solid / hairline Outline），且**任何控件在默认主题下拿到的都是它**
   ⇒ 这是「改写不改变现有外观」的机制保证
2. 单测：同一个控件在「扁平预设」与「立体预设」下，**发射的几何不同**
   （扁平：无斜角线、有阴影；立体：有斜角线、无阴影）—— 断言两条轨迹，不是两个颜色
3. 门禁 `check_surface_style_is_declared_not_hand_rolled`：
   `grep -rl "blend(&Color::rgb(255, 255, 255)\|blend(&Color::WHITE" src/widget/`
   的命中数**必须只降不升**（当前 43），且每个命中必须在白名单里附理由
4. 门禁 `check_elevation_is_not_one_value`：
   `role_base_style` 里**不许**再出现单个 `Shadow{...}` 字面量；
   elevation 必须来自 `theme.elevation(n)`

--- 声明层（§10A.4）---
5. 单测：`"card"` role 的 elevation 覆盖能到达一个 `card` 类控件（role 层生效）
6. 单测：`"button:pressed"` 的 `bevel: "inset"` 能到达按下的 Button（**state 层生效**）
   —— 这条是「3D 按钮的按下反馈」的端到端判据
7. 单测：未知 token（`"bevel": "sunken"`）**被拒绝**，不静默降级
   （`BevelDirection::parse` 已返回 `None`，主题加载器必须把它变成错误）

--- 两种风格（§10A.5）---
8. 快照：`button.svg` 在两个预设下**几何不同**（立体有斜角线，扁平没有）
9. 快照：**默认预设下 377 份快照逐字节不变**（BLUE23 §9 判据 24 的安全绳延续）
10. 几何：扁平预设下，半透明面**真的**覆盖了下层（读像素，不是读字段）

--- 回归 ---
11. `cargo test … --features desktop` → 0 failed
12. `cargo clippy … -D warnings` → 0 warning
13. `cargo check` 五个 profile 全部 Finished（`mini`/`embedded` 无主题模块，
    必须走 `SurfaceStyle` 的默认值而不是 `todo!()`）
```

### 10A.7 反向注入（每条新断言都必须做）

| 注入 | 必须变红的判据 |
|---|---|
| 把 `SurfaceStyle::solid()` 的 `bevel` 从 `None` 改成 `Some(Raised)` | 判据 1、9 |
| 把扁平预设的 `bevel` 改成 `"raised"` | 判据 2、8 |
| 把 `button:pressed` 的 `bevel` 删掉 | 判据 6 |
| 把主题加载器的 token 校验去掉（未知 token 当 `None`） | 判据 7 |
| 把 `elevation(n)` 换回那个固定 `Shadow` 字面量 | 判据 4 |

### 10A.8 风险与克制

| # | 风险 | 缓解 |
|---|---|---|
| 1 | **改了「面的画法」⇒ 188 个控件的外观全动** | 判据 1 是机制保证：`SurfaceStyle::solid()` 是恒等元，**默认值就是今天的样子**；判据 9 用逐字节快照把它钉住 |
| 2 | **把「风格」做成代码里的枚举**（`ThemeStyle::Flat`） | 显式禁止：四个维度**正交**，没有 `style:` 键。理由写在 §10A.3 的表里——三种混合面都真实存在 |
| 3 | **无限膨胀**（渐变 / 内阴影 / 多层描边 / 光泽……） | **停止线写死**：本节只加**四个**维度。第五个必须先删掉一个，或证明它不能被前四个表达（原则 #22：投机性 API） |
| 4 | **`mini`/`embedded` 没有主题模块** | `SurfaceStyle` 定义在**没有主题也能编译**的层（同 `dimensions` 的形态），默认值可用；判据 13 |
| 5 | **与另一进程冲突**（它在改 `theme/types.rs` 同族文件） | 本节的第 1 步**只加新类型与新文件**，不碰 `ThemeStyleToken`；接线放在第 2 步，等对方停下来（§12 边界 4）。**✅ 已按此执行**：共享面停止后，token 接线（四个键 + 未知 token 报错）与 flat/dimensional 两个预设均已落地；中间那一段（`WidgetStyle.surface` 承载 + 100 个控件消费）也已补齐。见 §12 U-10 与 `log-20260924-2.md` §26–§27 |
| 6 | **`BoxShadow` 的软件实现是「偏移矩形 + 区域 box blur」**，不是真高斯 | 只做**声明通道**，**不改** `BoxShadow` 的实现。若要改模糊质量，那是另一条（`§12` U-11） |

### 10A.9 与「不引入不需要的负担」的关系（原则 #51）

**不抄**（明确列举，避免这一节无限生长）：

| 项 | 为什么不抄 |
|---|---|
| 主流声明式实现的 `Material` 完整 elevation 语义（含 tint 混色、overlay 叠加） | 那是 tonal palette 的产物；本仓 `Colors` 是名字驱动的（BLUE22 §5.1 已裁定） |
| 参考工具包的 `QStyle` 整套 `drawPrimitive`（几十个枚举） | 那是把「怎么画」交给样式引擎；本仓是立即模式绘制，四个正交维度已经够表达两种风格 |
| 真实的**模糊背景采样**（backdrop-filter） | 需要离屏合成与读回；本仓软件后端可以采样自己的 back buffer，但那是**实现**问题，不是**声明**问题——`material: Translucent` 先表达**意图**，实现可以先是「半透明 + 预乘 tint」 |
| 一个 `style: "flat" \| "material" \| "aero"` 枚举 | 模式开关是 BLUE23 §6.1 已经拒绝过的形态（「两个行为在一个名字里」） |

## 11. 验收判据（全计划共用）

```text
--- 帧循环（§1）---
 1. drive_frame 是全仓唯一同时调用 tick_animations 与 drain_triggers 的函数
 2. 静止帧：controls_ticked == 0 且 repaints_submitted == 0
 3. 顺序判据：本帧 MouseEnter ⇒ 本帧过渡进度已前进
 4. 幂等判据：一个控件一帧只被推进一次（替身计数断言）
 5. 端到端：Linux 无 GTK 路径上，hover 序列在 N 帧后到达终点
 6. 门禁 check_single_frame_driver + 反向注入

--- 属性动画（§2）---
 7. Button 的 4 条既有过渡单测在迁移后逐条仍绿（零断言改动）
 8. 门禁 check_animation_state_is_one_type：控件内零处自持 from/to/current 三件套
 9. 门禁 check_animation_durations_are_tokens（扩展 BLUE23 §3.3 判据 5）+ 反向注入
10. 门禁 check_first_value_not_zero（PieMenu 的反例变红）+ 反向注入

--- 正交状态（§3）---
11. LineEdit 同时 error 与 hover：填充 == hover，描边 == error（当前不可能通过）
12. semantic_state() == None 时绘制输出与 BLUE23 产出逐字节相同
13. 门禁 check_semantic_state_is_not_in_the_interaction_chain + 反向注入
14. §3.4 的裁定（**选 A：保留并冻结**）已落实：三个变体仍在公开形状里，
    三者的 rustdoc 含「由 semantic_state() 承载」，门禁 4 正向通过

--- 环境事实（§4）---
15. text_scale = 2.0 ⇒ 控件固有尺寸严格变大，墨宽约为两倍
16. motion_preference = Reduced ⇒ 一帧内到达终点，且**最终几何与 Full 相同**
17. environment() 在 drive_frame 之内恒等
18. 未安装时全部中性缺省，且 mini 编译通过
19. should_use_dark 不再读 SystemTime；mini 的 false 分支删除
20. 门禁 check_environment_is_single_sourced + 反向注入

--- 自适应（§5）---
21. 同一 View::build 在 320/700/1200 三宽度下节点数不同、三几何两两不同
22. 门禁 check_breakpoints_are_not_in_draw + 反向注入

--- 无障碍（§6）---
23. 挂载 3 个控件 ⇒ 桥收到 3 次带 role+label 的建节点
   （即：①推导与③桥之间现在有②泵了）
24. Label/Button 的 label 语义正确；CheckBox 三态可经**提交路径**读出
25. 门禁 check_a11y_has_a_producer（现状为红，修完为绿）
26. 未安装桥时全部 no-op 且不 panic（mini/embedded 同）；a11y bus 不可用时判据 7 报 SKIP 而非 PASS

--- 原生统一（§7）---
27. 原生控件 report Hovered ⇒ widget_state() == Hover，且 :hover 主题生效
28. notify_native_redraw 计入帧账且同帧合并
29. 平台不回报时不改变任何库侧行为（静止快照不变）
30. 门禁 check_native_redraw_goes_through_the_runtime + 反向注入

--- 帧账（§8）---
31. 静止 60 帧：controls_ticked == 0
32. 同帧 5 个同矩形请求 ⇒ 提交 1 次，合并 4 次
33. 门禁 check_no_anonymous_repaint + 反向注入

--- 计划与门禁（§9）---
34. docs/plans/*.md 显著减少；同一主题零份重复活跃
35. 门禁 check_single_open_plan / check_plan_archive_has_index / check_gates_are_worth_running
36. gates_reverse_injection.md 覆盖每个 check_*.sh（缺行 = 未验证）

--- 回归（全计划）---
37. cargo test --no-default-features --features desktop → 0 failed
38. cargo clippy --no-default-features --features desktop --all-targets -- -D warnings → 0 warning
39. desktop/tablet/mobile/mini/embedded 五个 profile 全部 Finished
40. bash tools/run_all_gates.sh → FAIL=0（每条新门禁均已反向注入；SKIP 仅限宿主限制）
41. 静止态快照（不含运行时状态键的 .svg）逐字节不变
```

> **第 41 条是本计划的「安全绳」**，与 BLUE23 §9 第 24 条同形：
> 新增的帧循环、属性驱动、环境快照**必须不改变**用户已经确认的静止外观。

---

## 12. 未实现项（**本计划的后续入口**）

> 本节登记的是**本计划发现但不在本轮施工**的条目，由后续轮次**在本文件内**推进，
> **不允许新建 `blue25.md`/`blue26.md`**。
>
> **本节的三条边界**（避免与正在并行推进的计划撞车）：
>
> 1. **BLUE23 的未完成项不在本节**，也不在本文件任何地方。
>    它们由**另一个进程**处理，迁入会把「同一件事两处各写一半」重新制造一遍
>    ——那正是 BLUE23 §0A.0 与 §9.1 花力气消除的失败形态。
> 2. **上表条目均不依赖 BLUE23 的未完成项**。凡依赖它们的，列为 U-3 这类
>    「手写一个阻塞点」而不是「把那个条目搬过来」；BLUE23 收口后，
>    只需回填「前置已满足」而不需要重新立项。
> 3. **BLUE24 的批次与 BLUE23 的余项无先后关系**：
>    §10 的批 1（帧循环）在 BLUE23 收口前就可以开工。
> 4. **§10A 的接线步要避开共享面**。另一个进程正在改 `theme/` 同族文件
>    （实测：`preset_states.rs`、`themes/*.json`、`census.rs`、以及若干控件的状态键）。
>    所以 §10A 的**第一步只加新类型与新文件**（`SurfaceStyle` 定义在无主题也能编译的层），
>    **不碰 `ThemeStyleToken`**；把 token 接线放到第二步，且以「对方停下来」为前置。
>    这与边界 2 是同一形态：**手写一个阻塞点，而不是把冲突引进本计划**。

| # | 条目 | 为什么本轮不做 | 需要什么才能推进 |
|---|---|---|---|
| **U-1** | **Flutter 级图片解码缓存**（`image::Image` 的内存/磁盘两级缓存 + 解码器池） | 本仓 `src/image/` 的解码是**同步**的，且没有「同一资源被多处请求」的记账。做成缓存**必须先有**「谁请求了同一份资源」这个事实，而它属于 §8 帧账的同一形状 | 🟡 **部分已完成，余项按 §51 保持不做**。已交付（已核实）：`src/image/cache.rs` 的**内容寻址**（非路径）LRU + 字节预算 + 逐出 + 中程帧账（`FrameStats.decode_requests/hits/misses`），`avatar`/`tool_button` 两处都已接入。**未做的两项已有理由**：① **磁盘层** —— 见下方“为什么不做磁盘层”；② **解码器池** —— 解码是同步 CPU 路径，没有 I/O 等待可重叠，线程池只会引入锁与调度开销而拿不到并行度（原则 #51：必须带来真实消除才接入）|
| **U-2** | **视频/音频的时钟与同步**（`media_player` 的 A/V 同步） | 需要 §1 的帧时钟作为时间源；本轮先把时钟建起来 | ⏸️ **前置已解除、未动工**。`drive_frame` 已达（批 1），但 A/V 同步是一个**独立子系统**（需要每个 stream 的 PTS 转换 + 重采样缓冲），不属本计划交付范围。若要做，应作为 `blue24.md` 之后的一条独立条目立项（§12 允许在本文件内推进）|
| **U-3** | **`ScrollPhysics`（惯性/回弹/吸附参数化）** | `carousel`/`scroll_area` 各有自己的手感常数。做成可配置的物理模型要先把「手势速度」的单位与来源统一（两个识别器曾分别发 px/ms 与 px/s，差 1000 倍） | 🚫 **不属本计划**。§12 边界 2：依赖 BLUE23 余项的条目列为「手写阻塞点」，由另一进程处理 |
| **U-4** | **文本选择的完整闭环**（手柄 / 放大镜 / 跨 run 选择） | 文本层的 G-1…G-6 给了**塑形**；选择是**另一个子系统**，且它与 `rich_edit` 的「跨度感知 caret」同源 | 🚫 **仍被阻塞**。已核实：跨度感知 caret **零命中**（`span_aware`/`caret_for_span` 无匹配），前置未落地 |
| **U-5** | **IME 组合文本的端到端闭环**（多语言下的组合输入正确性） | `ime_*` 的 OS 调用是真的，但「组合串如何进入文本模型」没有判据 | 🚫 **仍被阻塞**：依赖 U-4（同上未落地）|
| **U-6** | **热重载（状态保持的整树替换）** | `view::diff` 已能保身份；热重载还要「旧状态 → 新代码」的迁移，属于开发工具链 | ⏸️ **前置已解除、未动工**。`view::diff` 在（`src/view/diff.rs:227`），§9 也已完成；但「旧状态 → 新代码的迁移」是开发工具链，不属本计划交付范围 |
| **U-7** | **插件/扩展注册表** | 需要先有 §9 的「单一入口」纪律，否则每个插件会带来一份自己的计划文件 | ⏸️ **前置已解除、未动工**。§9 三条元门禁均已 PASS（`check_single_open_plan` / `check_plan_archive_has_index` / `check_gates_are_worth_running`）。插件注册表需先定「插件可声明什么”，属独立设计 |
| **U-8** | **云端/远端渲染后端** | 需要先把 §8 的帧账做成可序列化的产物；否则远端只能传像素，不能传「为什么这一帧这样」 | ⏸️ **前置已解除、未动工**。§8 帧账在（`FrameStats` 含 `RepaintReason`）。但**可序列化产物尚未存在**（帧账目前是内存值），所以真正的第一步是把 `FrameStats` 做成可序列化 —— 而那是一条独立的条目 |
| **U-9** | **`drive_frame` 在其余宿主上接线**（Harmony / Windows / macOS / Android / iOS 的原生帧循环） | **✅ 已完成**。六个宿主的循环**全部已接线**：Linux（GTK `glib::timeout_add_local` + 无 GTK 的 `while` 两条路径）、macOS（`NSTimer` 回调 `drain_triggers_impl`）、Windows（`PeekMessage` 循环）、Harmony / Android / iOS（各自的主线程心跳）。证据：`grep -rn "drive_frame" src/platform/` | ✅（原登记「需要对应宿主」已不成立）。**真正的剩余是判据而非接线**：§1 判据 5 的「真窗口上 hover 在 N 帧后到达终点」需要能起窗口的自动化宿主，那是一条**集成判据**，不是一条待写的接线；见 §11 回归项与 `FUTURE.md` |
| **U-10** | **§10A 的 token 接线**（把 `SurfaceStyle` 的四个维度接进 `ThemeStyleToken` 与两个预设） | **与另一进程共享的同一批文件**（`theme/types.rs`、`theme/manager.rs`、`themes/*.json`）。先把类型与门禁做完，接线在后 | ✅ **已完成**（`log-20260924-2.md`）。真正的前置不是「共享面空闲」，而是中间那一段：`WidgetStyle` 没有 `bevel`/`material`/`hairline` 字段，控件层也**零个** `role_surface_style` 读取者 —— 两端（类型、角色默认）都在，中间（承载字段 + 控件消费）不在。三步全部完成：① 类型与角色默认；② `WidgetStyle.surface` 承载字段 + **100 个控件**的 draw 经 `RenderContext::face` 读它（93 脚本 + 7 人工）；③ token 接线（4 键，未知 token **报错**）+ `flat`/`dimensional` 两个预设。**关键收敛**：`Theme.flat_surfaces: bool` 一条规则取代了 22 行 kind 表 —— 表是「把规则写成数据」，role 表兜底是 raised，所以表没枚举到的 kind（`adaptive_scaffold`/`app_bar`…）在自称扁平的预设里仍然浮起。 |
| **U-11** | **`BoxShadow` 的真实高斯模糊**（现在软件后端是偏移矩形 + 区域 box blur） | §10A 只做**声明通道**，不改渲染质量。改模糊算法要自己的判据（模糊半径 vs 像素的量化、边缘裁切）。**取证**：377 份快照里 **0 个 `feGaussianBlur`/`filter=`** —— 改模糊算法**不可能**动快照，所以它是一条**可独立验证**的改动 | ✅ **已完成**（`log-20260924-2.md` §55）。**判据量出来的是一个真缺陷**：`box_blur_widths` 把**全部**方差放进**每一**个 pass，而方差是**相加**的 ⇒ 核宽了 `√3` 倍。实测每个阴影比它自己的 elevation 说的软 **1.6–1.8×**（blur 6：σ 应为 3.0、实为 4.83）。修后实际方差落在整数宽量化的范围内（σ=3：8.00/9.00；σ=6：34.00/36.00；σ=12：140.00/144.00）。新增断言 `the_three_boxes_realise_the_requested_sigma_not_three_times_it` 钉住**尺度**（既有断言只钉形状，看不见这个 bug），并已反向注入验证。快照 **0 变动**（与预测一致：SVG 后端走 `feGaussianBlur` 属性，不栅格化这个核）|
| **U-12** | **`material: Translucent` 的背景采样**（真正的 backdrop-filter） | 需要离屏合成与 back buffer 读回。先让 `Translucent` 表达**意图**（半透明 + 预乘 tint），再谈采样 | 🚫 **不做（保持未实现）**。§12.1 的三个条件**同时满足**才做，而实测本仓 **377 份快照里没有一份**有「浮层下的动态内容」——本仓内**没有消费者**。已核实：`BackdropSource`/`backdrop_readbacks` **零命中**，与裁定一致 |
| **U-13** | **字形距离场（SDF / MSDF）** 取代逐尺寸光栅化 | **本仓当前没有消费者**：字号 97% 落在 9–18 px 的窄区间，且无 GPU 字形图集；小字下 SDF 的质量**低于**精确光栅化；进快照会使 377 份安全绳失效；CJK 图集不可行。它只解决「一个 glyph 怎么画」，不解决「glyph 怎么排」 | 🚫 **不做（保持未实现）**。§12.2 的三个条件（连续任意缩放路径 / 允许近似质量 / 字体集封闭可离线烘焙）**没有一个成立**。已核实：`msdf`/`distance_field` **零命中** |
| **U-14** | **度量与光栅化质量**（本仓「字不好看」的**真实**根因） | 两件独立的事：① **度量必须问对 face** —— 主题声明 `"Arial"` 而本仓发布 `"Open Sans"`，`face_for_family` 返回 `None` 导致退化为扁平 0.6 em，实测 `widget` 的 `w` 重叠 1.85 px、`i` 多占 4.87 px（**已修**）；② **小字清晰度** —— 覆盖率混合的 γ 校正（**已修**，只作用于字形墨，不作用于几何：`glyph_coverage_weight`）、`word_spacing` 独立旋钮（**已修**）、基线像素对齐 | ✅ **①② 均已完成**（`log-20260924-2.md` §28–§29）。**它先于 U-13**，因为它改善 97% 的用例、全部控件、每一帧 |

> **U-9 注**：本项的**接线**已全部完成（六个宿主各自的循环都在调 `crate::drive_frame`，
> 与 `drain_triggers` 同一个心跳）——这正是 §1 批 1 的延伸，只是接在**每个宿主已有的循环**上。
> 不再剩余的是「帧循环接一次线」，而是「一个能起真窗口的自动化宿主来验证 §1 判据 5」。
> 其余宿主上的原生能力（窗口/渲染/事件循环本身）**不属于本计划**，
> 也不在本节登记——它们不在同一个工作流里，混进来会让本节失焦。

### 12.1 U-12 的需求闸门：它在什么情况下才**真的**有需求

U-12（背景采样 / backdrop-filter）是本节唯一一项**需求侧尚不明确**的条目，所以在动手前要把「什么情况下才需要它」写清楚，否则会做成一个没有消费者的机制。

**它成立于三个条件同时满足时**：

| # | 条件 | 为什么其他情况不需要它 |
|---|---|---|
| 1 | **浮层下面有内容，且内容会变** | 一个盖在纯色页面上的模态框，用 `alpha` 与用真采样**像素等价** —— 采样只是把同一个常量算一遍。只有下面有文字/图片/动画时，两者才看得出区别 |
| 2 | **宿主有能力提供那一层像素** | 自绘后端可以（它自己合成）；原生窗口/合成器后端需要 OS 支持（macOS `NSVisualEffectView`、Windows Acrylic、Wayland `ext_background_effect`）。**没有这个能力时，`Translucent` 的语义只能是「半透明」，不可能是「采样」** |
| 3 | **每帧有预算** | 真采样 = 离屏合成 + back buffer 读回 + 一次模糊。§8 帧账要把这一次读回记在账上，否则「这一帧为什么慢」无法回答 |

**对本仓现状的判断（实测，非推断）**：本仓 377 份快照里**没有一份**表现出浮层下的内容变化 —— 快照是无背景的单控件渲染。所以**在本仓内**，U-12 现在没有消费者：它唯一的真实需求来自**宿主应用**（例如设计器的浮层面板）。

**结论**：U-12 保持**未实现**，但不再是一个「前置不足」的条目而是一个**等待需求方**的条目。触发它的最小条件是：**宿主报告「浮层下是动态内容」且「宿主合成器能提供该层像素」**。届时实施顺序为 ① `Material::Translucent` 现有语义（`alpha` + tint）保持为默认回退；② 新增一个 `BackdropSource` 由宿主注入；③ §8 帧账加 `backdrop_readbacks` 计数。

### 12.3 U-1 的磁盘层：它被 §51 挡住，而不是被「没时间」挡住

U-1 的两项余项（磁盘层、解码器池）看起来是「缓存还没做完」，所以值得把不做的**理由**写下来（否则后人会当欠账）。

**磁盘层为何不做**（按原则 #51：共享抽象必须先取证「到底消除了什么」）：

| 问题 | 实测 |
|---|---|
| 它省的是什么？ | 一次 `fs::read` + 一次解码。解码已在内存层被消除（同一内容只解一次）|
| 本仓的请求重复度 | 快照层单控件渲染，**无重复请求**；应用的重复来自「同一资源多处使用」，而那已由**内容寻址内存层**接住 |
| 它引入什么？ | 第二级失效语义（磁盘何时过期？）、第二处缓存目录（需 `dirs::cache_dir()`，原则 #44）、以及「磁盘有但内存没有」这条新的代码路径 |
| 唯一的真实收益 | **进程重启之间**的存活 —— 即冷启动快一点 |

**结论**：收益是「跨进程重启的冷启动」，而代价是**第二套失效语义 + 新的目录约定 + 新的代码路径**，且在本仓内**拿不出一个重复请求的实测样本**。按 §51，**不接入**，并在本行写明理由，以免后人“好心”补上而引入无收益的间接层。

**解码器池为何不做**：本仓解码是**同步 CPU 路径**（`decode_to_rgba8`），没有 I/O 等待可重叠。把同步 CPU 工作搬到线程池**不会增加并行度**（CPU 本来就是满的），只会加上锁与调度开销 —— 而内容寻址内存层已经消除了「同一个资源解两次」这个真正的重复。当事前条件是「解码变成 I/O-bound（例如网络图像源）」，本条应重新评估。

### 12.2 U-13 的需求闸门：字形距离场（SDF / MSDF），以及为什么现在**不做**

本节登记一个被反复提出、且**方向很容易搞错**的方案：用**有符号距离场**（SDF，多通道则为 MSDF）取代逐尺寸光栅化，作为字形纹理的来源。

写下来的目的是**防错**：这个方案在别的场景里是对的，所以将来一定会有人（或某个 AI）提议全局替换。本节给出「什么时候才对」的判据，以及本仓**现在为什么不符合**。

#### 12.2.1 它解决的是什么问题

SDF 把每个像素到字形轮廓的**有符号距离**编码进纹理（MSDF 用 RGB 三通道分别处理尖角），绘制时用 `median(r,g,b)` 重建一个标量距离，再用平滑阶跃出覆盖率。

它**唯一**的优势是：**同一份资源要覆盖很宽的尺寸范围，且不希望为每个尺寸重新光栅化**。典型受益者是「一套图集走天下」的 GPU 管线。

#### 12.2.2 为什么本仓**现在**不符合（实测，非推断）

| # | 事实（本仓实跑） | 结论 |
|---|---|---|
| 1 | **字号分布集中在窄区间**：全仓 `Font::new`/`Font::simple` 的字号统计为 `12px×47、11px×28、14px×24、13px×20、10px×19、16px×8、9px×7…`，**9–18 px 占 175 处中的 170 处（97%）**；仅 4 处超过 20 px | SDF 的核心卖点是「跨宽尺寸范围」，而本仓的尺寸范围**本来就窄**。为窄区间付全套代价而几乎拿不到「任意缩放」的收益 |
| 2 | **本仓没有 GPU 字形图集** | SDF 的收益主要来自「图集驻留显存 + 每字一个 quad + 一次 draw call」。本仓是 CPU 逐字形光栅化到 RGBA 缓冲（`blend_painted_glyph`）。**「不必重新光栅化」在这里省不下多少**，因为瓶颈不是光栅化次数 |
| 3 | **小字下 SDF 比矢量**差 | 一个 11 px 的 `e`，直接光栅化 11 px 的轮廓是**精确**的；从有限分辨率的距离场重建，字腔会糊。而 9–18 px 正是本仓 97% 的用例 |
| 4 | **SVG 快照会退化成位图** | 377 份逐字节快照是本法的主安全绳（§10A.6 判据 9）。SDF 进快照 = 快照变位图且不可缩放，与快照的**用途**（审阅控件外形）冲突 |
| 5 | **CJK 不可行** | `cjk.ttf` 361 KB 覆盖常用字；SDF 图集对几千到几万字会膨胀到几十 MB。本仓的 CJK 已有正确解法（`cjk_bitmap_data.rs` 的按需二分查找，从不驻留） |
| 6 | **它不解决排版** | SDF 只回答「一个 glyph 怎么画」，**不回答「glyph 之间怎么排」**。本仓曾经的 `wi` 重叠、词距异常（§12.2.4）**一个都不由此解决** —— 那是度量问题，属于 U-14 |

#### 12.2.3 触发闸门：三个条件**同时**满足才做

| # | 条件 | 为什么缺一不可 |
|---|---|---|
| 1 | **出现「连续任意缩放」的渲染路径** | 例如设计器画布的无限 zoom、地图标注。**这是唯一真正的触发条件** —— 只有「尺寸范围宽到无法逐尺寸光栅化」时，SDF 的优势才存在。静态 UI 控件永远不满足 |
| 2 | **该路径允许近似质量** | SDF 是近似，不是精确。正文小字不可接受；图形/标注/大字号可接受 |
| 3 | **字体集封闭，且可离线烘焙** | SDF 要求预生成图集，换字体/换语言必须重烘。宿主必须在构建期锁定字体集合 |

**并且**，即使三个条件都满足，实施也必须是**增量路径而非替换**：

① 复用本仓已有的 `GlyphSource` trait 与 `InkKind`（二者本就可扩展），新增一个 SDF 来源；
② 只在触发它的那一条渲染路径上启用；
③ **不进 `snapshots/svg/`** —— 快照继续走轮廓，否则安全绳失效；
④ §8 帧账加 `sdf_atlas_bytes`，让图集占用可回答。

#### 12.2.4 与它相邻、但**不属于它**的问题（指向 U-14）

「字不好看」在多数情况下**不是字形格式问题**。本仓实测过的两个真缺陷都出在**度量与光栅化**：

- **度量问错了 face**：主题声明 `"Arial"`，而本仓发布的是 `"Open Sans"`，于是 `face_for_family` 返回 `None`，所有拉丁 cluster 退化为**扁平 0.6 em 估算** —— 同一个宽度给 `i` 和 `w`。实测 `widget`：`w` 实宽 10.25 px 记 8.40（**重叠 1.85 px**）、`i` 实宽 3.53 px 记 8.40（**多占 4.87 px**）。已修（`face_for_metrics`）。
- **小字清晰度**是光栅化问题（覆盖率混合是否 γ 校正、窄笔画是否欠采样、基线是否落在整像素上），与字形格式无关。

#### 12.2.5 结论

**U-13 现在不做。** 它不是「前置不足」，而是**当前没有消费者**：本仓没有连续任意缩放的渲染路径。

**正确的顺序是先做 U-14（度量与光栅化质量），再考虑 U-13。** U-14 改善的是 97% 的用例、全部控件、每一帧；U-13 改善的是一条尚不存在的路径。

---

## 13. 一句话结论

**BLUE21 让控件「画对」，BLUE22 让控件「知道自己多大」，BLUE23 让控件「知道自己此刻是什么状态」。
BLUE24 让应用「真的动起来，并且在别人的设备上也对」。**

三件事本仓**都已有零件**，缺的是把它们接到**进程边界**上：

- **帧循环**：11 个 `tick` 有了驱动者，而驱动者**没有驱动者** —— 一处 `drive_frame`；
- **属性动画**：`PropertyAnimation` 与每控件的三件套是同一件事的两份 —— 一处 `PropertyDriver`；
- **环境事实**：八个维度各自被问或不被问 —— 一处 `EnvironmentProvider`；
- **a11y**：推导与桥两段都建好了，**中间没有泵** —— 一处挂载期提交；
- **面的材质**：渲染能画阴影/渐变/路径，主题能改颜色，**中间没有「面」这个概念** ——
  一处 `SurfaceStyle`（§10A：让「立体」与「扁平」成为同一个参数空间的两个角）。

**它们的共同形状与 BLUE23 的结论句完全相同**，只是上移了一层：
**本仓已经写出了正确的机制，只是没有一个东西在进程的边界上消费它们。**

最后一条应当记住的判断：本计划**不追求与任何框架的功能清单对齐**。
本仓在桌面/设计器/工控方向的优势（BLUE21 §A.7 的 12 项）不动；
本计划只做四件**任何 GUI 应用都必须有、而本仓现在恰好没有**的事：
**它会动、它会跟着系统设置变、它会被读到、它的账能对上、它的面能声明。**

—

**与 BLUE23 的关系**：BLUE23 的未完成项由另一个进程处理，
本计划**不登记、不依赖、不阻塞**（§12 的三条边界）。
两者可以并行：§10 的批 1（帧循环）在 BLUE23 收口前就可以开工。

---

## 14. 结论段与两份总表的对账（**可执行的操作步骤**）

> **本节为什么存在**：§13 是全文唯一一句对全计划的定调，它的作用是让读者
> **快速核对「计划覆盖了哪些维度」**。这件事只有在「结论段列的条数 == 它声称的条数」
> 时才成立。实测 §13 两处都不可靠（三条 bullet 写了五条；「四件事」列了五件），
> 且**漏了整个 §5 自适应**。这不是文案问题，是**同一份计划里四张表的行数互不相等**
> ——原则 #18（文档与代码必须一致）的等价形态：**文档与文档自己也不一致**。

### 14.1 取证（先量，不引用）

```text
$ sed -n '1451p;1465,1466p' docs/plans/blue24.md
L1451  三件事本仓**都已有零件**…          ← 紧随其后有 5 条 bullet
L1465  本计划只做四件…的事：               ← 同一句后列了 5 个短句
L1466  它会动、它会跟着系统设置变、它会被读到、它的账能对上、它的面能声明。

$ awk '/^## 0B/,/^## 1\./' docs/plans/blue24.md | grep -c '^| [0-9]'
8                                          ← §0B 缺口总表 = 8 行
$ awk '/^## 10\. /,/^## 10A/' docs/plans/blue24.md | grep -c '^| \*\*批'
9                                          ← §10 批次表 = 9 批（含批 0）
$
```

三处事实：**§13 说 3 / 说 4；它列的 bullet 是 5；它没有 §5**。
而 §0B 是 **8** 行、§10 是 **9** 批。**四个数各说各话**。

### 14.2 操作步骤（**按顺序执行，每步一条可复跑的判据**）

> **总原则：把「数」交给结构，而不是交给记忆。**
> 不要只把 `3→5`、`4→5` 改对——那样下次加一节又会错。
> 要消除**需要人维护的计数词**，改成**由 §0B 派生的表**。

#### 步骤 1 —— 删掉两处写死的计数词，不写新数字

| 位置 | 现在 | 改为 |
|---|---|---|
| §13 第 3 段首句 | 「**三件事**本仓都已有零件」 | 「本仓在这几个维度上**都已有零件**」（不写数字，5 条 bullet 原样保留） |
| §13 第 5 段首句 | 「本计划只做**四件**……的事：」 | 「本计划只做以下几件……的事：」（不写数字） |

**为什么是「删数字」而不是「改成 5」**：改对数字仍然要求下一个人**记得同步两处**；
删掉数字后，条数由**结构**决定（bullet 自己数得清），于是**永远不可能再错**。
这与 §10A.5 的裁定同形——「一条规则取代一张 22 行的表」。

#### 步骤 2 —— 把「四件事」改成一张**有对应关系**的五行表，并补回 §5

替换 §13 的「它会动、它会跟着系统设置变、……」那一句为：

```markdown
| 它必须 | 由谁交付 | 判据 |
|---|---|---|
| **会动** | §1 `drive_frame` + §2 `PropertyDriver` | §11 判据 1–10 |
| **会跟着系统设置变** | §4 `EnvironmentProvider` + §5 `Breakpoint`（`adaptive!`）| 判据 15–22 |
| **会被读到** | §6 a11y 挂载期提交 | 判据 23–26 |
| **它的账能对上** | §7 原生统一 + §8 `FrameStats` | 判据 27–33 |
| **它的面能声明** | §10A `SurfaceStyle` | §10A 的 13 条 |
```

**这一行是补漏，不是润色**：§5「自适应」在 §13 里**完全没有出现**，
而它同时是 §0B 第 4 行的一半与 §10 批 8 的全部。补进「它会跟着系统设置变」是因为
**尺寸与缩放同属「设备事实」**——一个说「字多大」，一个说「树多宽」，都由 §4 的
`EnvironmentProvider` 提供输入，由 §5 消费。**两者不该分到两句里。**

#### 步骤 3 —— 在 §13 段首加一行**指回 §0B** 的锚

```markdown
（下表与 §0B 缺口总表逐行对应；§0B 是**缺陷**视角，本表是**能力**视角。）
```

**为什么必须加**：§0B（缺陷）与本表（能力）本来就是同一件事的两面。
不写这句话，读者会以为它们是两份互不相关的清单——而它们**必须是同一份维度的两种叙述**，
否则「一处接通 N 个消费者」这个全计划的核心命题无法核对。

#### 步骤 4 —— 把 §5 补进 §12 的登记（如果它不在）

```text
$ grep -n '§5' docs/plans/blue24.md | head
（若 §5 已标 ✅ 且登记在批次表与 §11，则本步**无需改动**，只逐行核对存在）
```

**核对**：§0B 的每一行都必须能在 §13 的表里找到位置，或**写明它被合并进了哪一行**。
§0B 共 8 行 → 本表 5 行的合并关系是固定的：

| §0B 行 | 并入本表哪一行 |
|---|---|
| 1 帧循环 / 2 属性动画 | 会动 |
| 3 正交状态 | 会动（`PropertyDriver` 是它的前置） |
| 4 环境事实 | 会跟着系统设置变 |
| 5 a11y | 会被读到 |
| 6 原生统一 | 它的账能对上 |
| 7 计划入口 | **不并入本表**——它是**元层**（关于计划自身），不是应用能力；保留在 §9 |
| 8 面的材质 | 它的面能声明 |

> 上表就是**合并不是省略**的证明：8 行 → 5 行 + 1 行（元层）+ 2 行合并，**加得起来**。

### 14.3 判据

```text
1. §13 **段内**不再出现「（数字）件事…」作为**句首定调**的写法
   （检查法：`awk '/^## 13\./,/^---/' docs/plans/blue24.md | grep -nE '(^|[。；：] ?)[一二三四五六七八九十]件事'`
   → 应为 0。**不能只写 `grep '件事'`**——「同一件事」在别处是正常中文，
   且本节自己的正文就用它描述这个缺陷；否则这条判据会自我违背）
2. §13 覆盖 §0B 的**每一个**非元层字段：帧循环 / 属性动画 / 正交状态 /
   环境事实 / 自适应 / a11y / 原生统一 / 面的材质（八项都能在表里定位）
3. **§5 自适应在 §13 里可见**（现在不可见）
4. §13 的表有「它必须 → 由谁交付 → 判据」三列，**判据列引用的编号真实存在**
   （逐条对照 §11 的 1–41 与 §10A.6 的 1–13）
5. §13 的段首锚指向 §0B，且 §0B 的 8 行与 §13 的 5 行合并关系**能逐行加得起来**
6. `bash tools/check_single_open_plan.sh` 仍 PASS（结论段不属于 subject 声明）
```

### 14.4 反向注入（**每条新断言都必须能红**）

| 注入 | 必须变红的判据 |
|---|---|
| 把 §13 的表里「§5 `Breakpoint`」那一格删掉 | 判据 2、3 |
| 把「§0B 缺口总表逐行对应」的锚行删掉 | 判据 5 |
| 把计数词「五件事」加回 §13 | 判据 1 |
| 把「判据 15–22」改成一个不存在的编号（如「判据 99」）| 判据 4 |

### 14.5 一条元门禁（可选的自动化）

上表的 1–6 可以做成一条门禁，接进 §9.2 的元门禁家族：

```text
门禁 `check_conclusion_covers_the_gap_table`：
  1. 从 §0B 解析出缺口行数 N（正则：`^| [0-9]` 且在 §0B 与 §1 之间）；
  2. 从 §13 的「它必须」表解析出维度行数 M；
  3. 断言 N 的每一行都能在 M 里定位（或显式登记为元层/合并），
     且 §5「自适应」在 M 的某一格里**按名出现**；
  4. 断言 §13 不含「[一二三四五六七八九十]件事」这类**写死的计数词**。
```

> **要不要做这条门禁**：它符合 §9.2 的判据 3（必须证明过会红），
> 也符合 §14.4 的四个注入。但它的**消费者只有一个文件**（`blue24.md`），
> 所以按原则 #51，**先不做**——§14.4 的四条注入已经能手工跑，
> 且本节的判据 1 是一条 `grep`，成本极低。**等出现第二个同类结论段，再把它提上来。**

### 14.6 本节的完成定义

| 项 | 完成的样子 |
|---|---|
| §13 无写死计数词 | 见下方命令，命中 = 0 |
| §13 覆盖 §0B 的八个非元层字段 | 八项逐项可在表里定位 |
| §5 自适应可见 | 「§5 `Breakpoint`」出现在「会跟着系统设置变」那格 |
| 判据编号真实 | 表里引用的每个编号在 §11 / §10A.6 里找得到 |
| 合并关系可加 | 8 = 5 + 1（元层）+ 2（合并） |

```text
$ awk '/^## 13\./,/^---/' docs/plans/blue24.md | grep -nE '(^|[。；：] ?)[一二三四五六七八九十]件事'
（命中 = 0 则完成；命中行就是还没改的写死计数词）
```
