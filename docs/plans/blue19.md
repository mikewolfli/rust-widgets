# BLUE19 — 事件契约的类型化与设计器就绪（EventSchema + 自动接线 + 镜像判定）

> 状态：**计划（未执行）**
> 原则依据：[`docs/plans/principle.md`](principle.md)（继承 BLUE1–BLUE18 全部规则，含 #1–#94；
> 本文件新增 #95–#101）
> 上轮计划：[`docs/plans/blue18.md`](blue18.md)
> **设计意图（是什么/为什么）**：[`docs/plans/whitepaper.md`](whitepaper.md)（设计器白皮书草稿）
> **本文件是执行计划（怎么做）**；两者分工见白皮书 §7。
> 相关日志：[`docs/log/log-20260920-2.md`](../log/log-20260920-2.md)（第 10–12 轮：跨越区间缺陷类、
> 逐名核对全部已发布事件名）
>
> 本文件是**执行计划**，不是完成报告。
> **取证纪律（原则 #64）**：§二 的每条数字均在当前工作树上**实跑取得**，附命令；
> **未复跑的不写入**。
>
> **触发前提（用户已确认）**：本库将配一个 **JSON + CSS 的设计器**。
> 这个前提**反转了**第 12 轮的部分结论——详见 §1.3。

---

## 零、实现者入口（新对话从这里开始读）

> 本计划会在**新对话**中执行。本节是新对话需要的全部上下文，
> 读完本节 + §五（待决策项）即可开工，不必翻对话历史。

### 0.1 一句话目标

**把事件侧从「字符串数组」升级为「带类型的契约」，并让它能被 JSON 往返、能自动接线、
能被设计器消费；同时让开发期（热改）与交付期（生成代码）两套模式共存。**

### 0.1.1 已裁定的架构前提（不要重新讨论）

- **D7 = 两套模式共存**（用户已同意，见 §5.1）：
  - **开发期**：JSON + CSS 热改，**不重编译**（反馈回路毫秒级，AI 友好）
  - **交付期**：**可选**生成 Rust 代码（去运行时开销，得编译期检查）
  - 技术依据：`ViewEngine::mount/update` 的 `create` 回调是两模式**公共接口**
  - **`mini`/`embedded` 例外（D7-c，§5.2）**：那两个 profile 下**只能用模式 2**
    （模式 1 的 `json`/`view` 不编译，内存预算也装不下）——
    且生成物不得引用被门控的 `create_*`（`mini` 下有 118 处 `not(alloc_frugal)`）
- **CSS 走外观（`src/style/css.rs`），JSON 走结构（`src/json/`）**——库已定，不重叠。

### 0.2 开工前必须做的三件事

1. **确认 D1–D4 是否已裁定**（§五）。**未裁定则不得动实现代码**。
2. **跑一遍基线取证**，确认本计划的数字仍成立（防止工作树已漂移）：
   ```bash
   python3 tools/list_event_payload_shapes.py          # 应为 335 声明 / 60 种载荷
   bash tools/check_capability_events_are_emitted.sh   # 应为 326 pairs / 186 名字
   bash tools/run_all_gates.sh                         # 应为 PASS=33 FAIL=0
   ```
   不一致则先查清差异原因再继续（数字漂移说明前提变了）。
3. **读 §2 的取证**，特别是 §2.4（JSON 层已有另一条事件路径）。

### 0.3 本计划的约束（不得违反）

- **不得**自造第五个 profile 别名（规则 #47/#92）——一律用 `full_widgets`。
- **不得**改写既有控件 trait 签名（规则 #86）——只做**加法**。
- **不得**凭空发明载荷语义——D1–D4 未定就先问，不要猜。
- 每个修复必须**实测可达**才改，改完**反向注入**验证（revert → 必须 FAIL）。
- 每轮不跑门禁/全量测试；**只在最后跑一次**（规则 #55/#56）。
- 所有可能阻塞的命令**必须设超时**（规则 #58）。

### 0.4 建议的第一步（无需任何决策即可做）

**T-A（`WindowState` 13 个字段逐个测试化）+ 产出 D1–D4 裁定清单。**
两者都**不依赖 D1–D4**，且都有明确判据。

---

## 核心规则（继承 BLUE18 全部，含 #1–#94）

### BLUE19 新增规则

95. **🧭 事件与属性必须类型对称** — `WidgetCapability` 中，属性侧有
    `PropertySchema { name, value_kind, readable, writable }` + `PropertyValueKind` 枚举 +
    `CapabilityValue` 运行时值；**事件侧不得只是一个 `&'static str` 名字数组**。
    每个已发布事件必须声明其**载荷种类**（无载荷 / Int / Float / String / …）。
    判定：事件侧存在与 `PropertySchema` 对位的结构体，且门禁能逐条核对
    「声明的载荷种类 == 信号的真实 Rust 类型」。
    理由：名字里没有类型信息，面板无法显示「值：数字」，连线无法做类型校验，
    JSON 无法表达映射——这三件事都是设计器的**第一步**需求，不是最后一步。

96. **🖋️ 设计器消费的数据必须能完整往返（JSON 层）** — 设计器要读的清单（控件、
    属性、事件、命令）必须能**导出为可序列化结构**并**重新载入**，且往返后
    语义等价。判定：存在一个 JSON 往返测试，覆盖全部已发布事件名，
    断言「导出→载入→再导出」两次输出**逐字节相同**。
    理由：设计器保存的是 JSON 文件；不能往返的清单等于每次打开都丢失信息。

97. **🚫 「已发布」不等于「会响」，此差异必须可查询** — `connect_event(name, event)`
    返回 `Ok` 只证明**名字合法**，不证明**有人接线**。这个差异今天**无法查询**，
    因此「订阅成功但永不触发」是不可检测的静默失效。
    必须提供运行时查询入口（如 `event_is_wired(control, event)`），
    或让订阅返回值携带该事实。判定：存在一个测试，明确断言
    「未接线时查询返回『未接线』而非静默成功」。
    理由：原则禁止「reported success for something that did not happen」；
    当前形态正是该缺陷，只是尚无消费者踩到。

98. **🔌 接线必须由库完成，不得要求宿主手写** — 设计器生成的连线在**运行时**才确定
    （用户在这个面板上连了什么，编译期不知道），因此「宿主为每个控件手写
    `forward_*`」的模型与设计器**不兼容**。库必须提供单次调用完成某控件全部
    已发布事件接线的入口。判定：存在测试，对若干控件用**一次调用**接完，
    并逐个断言其已发布事件都能送达。
    理由：设计器无法预生成它还不知道的连线；这是第 12 轮「收益小」结论反转的直接原因。

99. **🪞 镜像字段必须逐个判定「回退」或「删除」，不得悬空** — `WindowState` 的每个
    字段必须属于且仅属于两类之一：① **回退值**（平台读取返回 `None` 时使用，
    且该回退被**生产代码实际读取**）；② **已删除**（无人读取且平台权威）。
    「写入后无人读」是**悬空镜像**，必须删除或补读取点。
    判定：每个字段存在一条测试，证明其「被读取」或「已删除」，二者必居其一。
    理由：悬空镜像会漂移成错误事实——平台是权威，镜像只能作为**有据可依**的回退。

100. **📋 设计器就绪度必须可量化** — 不得用「大概齐了」描述设计器可用程度。
     必须有门禁输出**具体计数**：多少控件、多少属性、多少事件已带类型、
     多少事件可接线、多少无法表达。判定：门禁打印计数表，
     且「无法表达」的条目**逐条列出**并附原因。
     理由：原则 #64 取证纪律在「就绪度」这种整体判断上尤其容易退化成印象。

101. **🔍 同一概念只能有一条实现路径** — 当发现两套并行机制做同一件事
     （如 JSON 的 `on_click` 专用键 vs capability 的 326 个已发布事件名），
     必须**取证重复的真实程度**（原则 #51），然后**合并或明确分工**，
     不得让两套长期并存。判定：两套机制的职责边界写入文档，
     且门禁能验证「一条路径新增的事件，另一条不会静默落后」。
     理由：并行机制必然漂移；本轮即发现 JSON 事件路径与 capability 事件表
     **完全不相交**（§2.4）。

---

## 一、问题的边界

### 1.1 本计划要解决什么

给定**前提**：本库将配 **JSON + CSS 的设计器**。设计器需要：

1. 列出控件有哪些**属性**、哪些**事件**、哪些**命令**；
2. 在面板上显示事件的**载荷类型**（用户要知道 `value_changed` 带来什么）；
3. 让用户连一条线（`on value_changed → 设置某控件某属性`）；
4. 把这条线**存成 JSON**，重新打开时**还原**；
5. **预览时这条线真的能跑**。

### 1.2 本计划要解决的四步（用户指定顺序）+ 一项独立任务

| 步骤 | 内容 | 依赖 |
|---|---|---|
| **步骤 1** | 给事件加 schema（与属性对称） | 无（地基） |
| **步骤 2** | 导出设计器可消费的 JSON + 往返 | 依赖 1 |
| **步骤 3** | CSS/JSON 绑定模型（事件 ↔ 属性的类型兼容） | 依赖 1、2 |
| **步骤 4** | 自动接线（**模式 1 需要**；模式 2 的接线已在生成代码里） | 依赖 1（载荷类型已知） |
| **任务 A** | `WindowHandle` 镜像与平台权威源的逐个判定 | 独立，可并行 |
| **任务 B** | **两模式共存**（D7）：开发期模式 1 + 交付期模式 2 | 依赖 1、3；衍生 T-23/T-24 |

### 1.3 ⚠️ 本前提**反转**了第 12 轮的结论（必须写明）

第 12 轮我判断：

> 「326 个已发布名字是空牌子，倾向**收窄**到有人用的；自动接线收益不大」

**该结论的前提是「没有消费者」——那时代码里确实零消费**（`connect_event` 只出现在文档与测试，
`examples/` 零次，`bindings/` 零次，C ABI 无对应函数）。**设计器把这个前提抽掉了**：

| | 无设计器（第 12 轮） | **有设计器（本计划）** |
|---|---|---|
| 326 个名字 | 空牌子 → 倾向收窄 | **面板可选项清单 → 刚需，越全越好** |
| 第 12 轮补的 24 个名字 | 兑现牌子 | **补全面板可选项** |
| 自动接线 | 收益小（清理自家落差） | **必须做**（设计器连线否则跑不起来） |
| 载荷类型 | 可回避 | **必须先设计**（面板要显示「值」） |

**所以本计划的方向是「加固送货」，不是「缩小牌子」。**
第 12 轮补的 24 个名字从「补齐空牌子」变为「补齐可选项」，**价值更硬**。

### 1.4 产物形态：两套模式共存（已裁定，见 §5.1）

设计器产出的 UI **两套模式共存**：

- **开发期**：JSON（结构）+ CSS（外观）**热改，不重编译**——反馈回路毫秒级，AI 友好；
- **交付期**：**可选**生成 Rust 代码——去运行时开销，得编译期检查。

> 技术依据：`ViewEngine::mount/update` 接受一个 `create` 回调，
> **它正是两模式的公共接口**。共存不需两套架构，只需两个 `create`。详见 §5.1。

对四个步骤的净影响见 §5.1.5：步骤 1/2 **不变**，步骤 3 **微增**（两份消费者），
步骤 4 **略降**（只有模式 1 需要）。

---

## 二、现状取证（本轮实跑）

> 所有数字来自当前工作树实跑，命令附于各条。

### 2.1 事件侧目前**只有字符串**（核心缺口）

实跑：`grep -n "pub events" src/widget/capability/types.rs`

```rust
pub struct WidgetCapability {
    pub kind: WidgetKind,
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Every property this kind publishes, for discovery and for validating a name.
    pub properties: &'static [PropertySchema],   // ← 有类型
    /// Names of the events the kind can emit, for wiring handlers by name.
    pub events: &'static [&'static str],          // ← 只有名字，无类型
    pub commands: &'static [&'static str],
}
```

**属性 vs 事件的不对称**（设计器第一步就撞上）：

| 能力 | 属性侧 | 事件侧 |
|---|---|---|
| 类型声明 | ✅ `PropertyValueKind`（Bool/Int/UInt/Float/String/Enum/Color/Rect） | ❌ 无 |
| schema 结构 | ✅ `PropertySchema { name, value_kind, readable, writable }` | ❌ 无 |
| 默认值 | ✅ `CapabilityPropertyManifest { schema, default_value }` | ➖ 不适用 |
| 运行时值 | ✅ `CapabilityValue`（含 `Color`/`Rect`） | ❌ 无 |
| 往返读写 | ✅ 读 + 写 | ❌ 只能订阅 |
| 类型校验 | ✅ 写错类型得 `TypeMismatch` | ❌ 只能查名字在不在 |

### 2.2 载荷类型的**真实规模**（决定步骤 1 的工作量）

实跑（脚本扫 `pub <name>: SignalN<T> / GenericSignal`，解析真实泛型参数）：

```
distinct payload types: 60   total signal declarations: 335
```

抽样（出现次数降序前 20）：

| 载荷类型 | 次数 | 例 |
|---|---|---|
| `()`（`GenericSignal`） | 91+4 | `focus_changed`、`back_pressed` |
| `String` | 75 | `shortcut_triggered` |
| `usize` | 46 | `triggered` |
| `bool` | 24 | `toggled` |
| `i32` | 10 | `value_changed` |
| `Option<usize>` | 6 | `current_index_changed` |
| `(usize, usize)` | 5 | `tab_moved` |
| `(u32, u32)` | 5 | `key_down` |
| `Color` | 4 | `color_selected` |
| `ObjectId` | 3 | `subwindow_activated` |
| `f64` | 3 | `double_value_changed` |
| `(String, u32, String)` | 2 | `console_message` |
| `Vec<String>` | 2 | `files_selected` |
| `Font` | 2 | `font_selected` |
| `chrono::NaiveDate` | 1 | `selection_changed` |
| `KeySequence` | 1 | `key_sequence_changed` |

> **规模结论**：60 种载荷类型，其中含**元组**（`(usize, usize)`、`(String, u32, String)`、
> `(usize, usize, usize, usize)`）、`Option<usize>`、`Vec<String>`、以及**领域类型**
> （`Color`、`Font`、`Date`、`Time`、`Shortcut`、`KeySequence`）。
> **这不是「加个枚举」的工作量**——它包含跨语言/跨 JSON 边界的**映射设计**。

### 2.3 已发布名 vs 信号总数（不相等，必须解释）

```
已发布事件名出现次数（当前）: 326 pairs / 186 distinct names   [check_capability_events_are_emitted]
全仓信号声明总数:            335
```

**335 ≠ 326**，差值来自框架信号（`BaseWidget` 的 `hover`/`mouse_down`/`focus_gained`/
`redraw_requested` 等，由 `BaseWidget` 自己发射，不归任何 capability 所有）。
**这个差值是正确的**，不是缺口；步骤 1 **只处理有 capability 归属的那部分**。

### 2.4 ⚠️ 发现：JSON 层**已有**一条独立的事件路径（与事件表**完全不相交**）

实跑：`grep -rn "on_click\|on_change\|extract_event_handlers" src/json/`

`src/json/events.rs` 模块文档：

> Event handler mapping for JSON-declared `on_click` / `on_change` handlers.
> When a JSON node declares `"on_click": "handler_name"`, the string `"handler_name"`
> is stored in the node's properties. After widget instantiation, the [`EventHandlerMap`]
> connects those names to Rust closures.

`src/json/loader.rs:272-299` 把它接到控件：

```rust
// ── Event binding: on_click / on_change / extended ──
let (on_click_name, on_change_name) = extract_event_handlers(obj);
if let Some(ref name) = on_click_name {
    let handle: ButtonHandle = ButtonHandle::from_raw(widget_id);
    handle.on_click(move || { /* invoke_global_handler(name, ctx) */ });
}
```

**问题（三条，都是设计器的直接障碍）**：

1. **是硬编码键名清单**，不是遍历 capability 的 `events`：目前只认
   `on_click`、`on_change`、`on_close`、`on_double_click`、`on_focus`、`on_blur`、
   `on_selection_changed`、`on_value_changed` **8 个**，而事件表有 **186** 个名字。
   **新增一个已发布事件，JSON 侧不会自动支持**——违反新规则 #101。
2. **强转成 `ButtonHandle`**（`ButtonHandle::from_raw(widget_id)`），即这条路径
   只对按钮形态成立；对 `slider`/`chart`/`code_editor` 等并不通用。
   `on_value_changed` 走的是 `WidgetHandle` trait 的 `ValueChanged` 触发种类，
   **不是**控件自己的 `value_changed` 强类型信号。
3. **没有任何门禁覆盖**：实跑 `grep -rn "on_click|EventHandlerMap" tools/` → **0 命中**。
   即这条路径可以静默漂移，无人察觉。

> **这说明设计器所需能力的「半个」已经存在，但是以另一套机制实现的。**
> 本计划的价值之一就是把它与事件表**合并**（新规则 #101），
> 而不是再写第三套。

### 2.5 `connect_event` 的消费现状（第 12 轮结论的依据）

实跑（`grep` 工具，项目源码）：

| 位置 | `connect_event` / `EventSignalBinder` 出现 |
|---|---|
| `src/` 生产代码 | **0**（仅文档注释） |
| `examples/` | **0** |
| `bindings/` | **0** |
| C ABI（`src/abi*`） | **0** |
| `tests/` | 有（`event_signal_bridge_test`、`capability_event_surface_test`） |

→ 第 12 轮「收益小」的**依据**即此。设计器前提使其失效（§1.3）。

### 2.6 `WindowHandle` 镜像现状（任务 A 的基线）

实跑：`grep -n "mirrored_\|WindowState" src/app/handle.rs`

`WindowState` 字段（`handle.rs:2495`，**共 13 个**）与**当前**读取状态：

| 字段 | 谁写 | 谁读 | 判定 |
|---|---|---|---|
| `x`/`y`/`w`/`h` | `set_geometry` | `apply_window_layout`（`handle.rs:1184`） | ✅ 有读取 |
| `icon` | `set_icon`（成功才写） | `icon()` 回退（`handle.rs:2629`） | ✅ 回退 |
| `min_w`/`min_h` | `set_min_size`（成功才写） | `min_size()` 回退（`handle.rs:2613`） | ✅ 回退 |
| `maximized`/`minimized`/`fullscreen`/`resizable`/`decorated` | `set_*`（**第 10 轮已修为成功才写**） | `is_*()` 走 `mirrored_flag`（`handle.rs:2682/2715/2745`） | ✅ 回退 |
| `close_callback` | `on_close`（`handle.rs:2821`） | `close()`（`handle.rs:2832`） | ✅ 有读取 |

**第 10 轮已修**的关键缺陷：`set_maximized`/`set_minimized`/`set_fullscreen`/`set_resizable`/
`set_decorated` 曾**无条件写入**镜像，丢弃平台返回的 `bool` —— 于是对**不存在的窗口**
调 `set_maximized(true)` 会让 `is_maximized()` 报 `true`。已改为成功才写。

**任务 A 剩余工作**：上述判定是**代码阅读结论**，尚未**逐字段测试化**。
新规则 #99 要求每个字段有测试证明「被读取」或「已删除」，二者必居其一。
**13 个字段全部需要测试背书**——`close_callback` 虽已正确读写，同样缺少测试。

---

## 三、目标架构

### 3.1 步骤 1：事件 schema（地基）

```rust
// src/widget/capability/types.rs

/// 一个已发布事件的元数据。与 `PropertySchema` 对位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSchema {
    /// 事件名，与 `connect_event` 接受的名字逐字一致。
    pub name: &'static str,
    /// 载荷种类。`None` = 无载荷事件（`clicked`）；
    /// `Some(kind)` = 载荷按 `PropertyValueKind` 描述。
    ///
    /// 复用 `PropertyValueKind` 而非新造枚举：payload 与属性值走同一个
    /// JSON/类型转换管道，两套枚举必然漂移（原则 #54）。
    pub payload: Option<PropertyValueKind>,
}
```

`WidgetCapability` 相应变为：

```rust
pub events: &'static [EventSchema],   // was: &'static [&'static str]
```

**必须保留的兼容面**：

- `connect_event` 的校验从「名字在数组里」变为「按 `name` 查找 schema」——
  **外部行为不变**（同名仍接受、异名仍 `UnknownCommand`）。
- `WidgetCapabilityManifest.events` 从 `Vec<&'static str>` 变为带类型的结构
  （步骤 2 一并处理）。

**60 种载荷类型如何落进 `PropertyValueKind`**（这是步骤 1 的**真正难点**）：

| 现有 Rust 类型 | 映射 | 备注 |
|---|---|---|
| `()` / `GenericSignal` | `None` | 无载荷 |
| `bool` | `Bool` | |
| `i32`/`i64`/`u32`/`usize`/`u64` | `Int`/`UInt` | 按有无符号 |
| `f32`/`f64` | `Float` | |
| `String` | `String` | |
| `Color` | `Color` | 已有变体 |
| `Rect` | `Rect` | 已有变体 |
| `Option<usize>` | `UInt` | **需决策**（D2，实跑 9 处）：`None` 如何表达？ |
| `(usize, usize)` 等元组 | **无对应** | **需决策**（D1，实跑 35 处）：元组如何表达？ |
| `(String, u32, String)` | **无对应** | 同上 |
| `Vec<String>` | **无对应** | **需决策**（D3，实跑 6 处） |
| `Font`/`Date`/`Time`/`Shortcut`/`KeySequence` | `String`（token） | **需决策**（D4）：序列化格式 |
| `BarcodeResult`/`DragPayload`/`NavigationEvent`/`FilterExpr`/`SignatureStroke` 等 | **无对应** | **需决策**（D4）：领域对象是否过 JSON 边界 |

> **§3.1 的核心决策（必须先定，否则步骤 2/3 无法进行）**：
> 元组、`Option`、`Vec`、领域类型**如何表达**。
> 三种候选见 §5「待决策项」，本计划**不预判**，但把它列为**阻塞项**。

#### 3.1.1 迁移的机械形式（实现者直接照做）

`events:` 从 `&[&str]` 变为 `&[EventSchema]` 会触及**每一处**构造 capability 的地方
（实跑：`properties.rs` 共 **187** 个 `events:` 行）。为把改动压到最小：

```rust
// 方案：用一个 const 宏把现有名字列表原样搬过去，避免 187 处手改结构体字面量。
// `property_names_of!` 是同类的既有先例（见 properties_trait.rs），有法可依。

/// 无载荷事件（只写名字，`payload: None`）。
const fn unit_events(names: &[&'static str]) -> &'static [EventSchema] { .. }
```

**但 `const fn` 无法做「名字 → 类型」推导**，所以更现实的两步走：

1. **先机械迁移**：写一个一次性脚本，把
   `events: &["a", "b"]` → `events: &[unit("a"), unit("b")]`，
   **全部标为无载荷**。此时门禁必须**暂时**允许（有注释说明阶段）。
2. **再逐条填载荷类型**：从源码信号声明反推真实类型（`tools/list_event_payload_shapes.py`
   已提供全量清单），**逐条**改为 `payload("value_changed", PropertyValueKind::Int)`。
   此步门禁**强制**：声明类型必须 == 信号真实类型。

> **为什么必须分两步**：一步到位会同时改 187 处 + 引入类型，
> 一旦编译错就分不清是迁移错还是类型错。分两步则每步都可单独验证。
> 这也让第 2 步的工作量**可度量**（186 个名字，其中 50 处非标量需 D1–D3 裁定）。

### 3.2 步骤 2：JSON 导出与往返

- `WidgetCapabilityManifest.events` → `Vec<EventManifest { name, payload }>`；
- 新增设计器入口（形如 `capability_manifest_json(name) -> String`），输出稳定排序；
- 新门禁 `check_designer_manifest_roundtrip.py`：导出 → 载入 → 再导出，
  断言两次输出**逐字节相同**（新规则 #96）；
- 覆盖**全部 186 个已发布名字**。

**已知障碍（实现前必须确认）**：

- `WidgetCapabilityManifest` 当前**未实现 `Serialize`**（实跑：`grep -rn "derive(Serialize"
  src/widget/capability/` → **0 命中**）。所以「导出为 JSON」意味着**先加 serde 派生**，
  或手写一份 JSON 序列化（后者避免给 capability 层引入 serde 依赖，但需自己保证稳定性）。
- 仓库已有 `serde_json`（`src/json/loader.rs:26` 在用），但**仅在 device profile 下**：
  实跑 `Cargo.toml` —— `serde`/`serde_json` 出现在 `desktop`/`tablet`/`mobile` 的
  feature 列表中，**不在 `mini`/`embedded`**。
  ⇒ JSON 导出入口必须门控在 `full_widgets`（与 §四 一致）；
  若希望 capability 层**不依赖 serde**（使 `manifest → 结构` 在任何 profile 可用），
  则手写序列化是更稳的选择（值集合有限，就那几个标量种类）。

### 3.2.1 往返测试的**具体断言形式**（避免写成同义反复）

「导出→载入→再导出」若两步都调同一个函数，测试就是同义反复。必须做到：

1. 导出 A 控件的 manifest → JSON 字符串 `s1`；
2. **用一个独立的解析器**读 `s1`（`serde_json::from_str` 或手写解析）得回结构；
3. 从该结构**重建** manifest → 再导出为 `s2`；
4. 断言 `s1 == s2`（逐字节）；
5. 并断言 `s1` 中**确实包含**预埋的哨兵（如 `slider` 的
   `slider_pressed` 与 `value_changed`，且后者带类型）——否则「两个空字符串相等」也会通过。

> 第 5 条是关键：只断言「两次相同」会被「导出器什么都不写」满足。

### 3.3 步骤 3：CSS/JSON 绑定模型

- 统一 JSON 键：**不再用 `on_click` 专用键**，改为
  `"events": { "<published_name>": "<handler>" }`，与事件表**同名**（新规则 #101）；
- **类型兼容规则**（事件载荷 → 目标属性）：
  - 同类型直通；
  - `Int`/`UInt` → `String` 允许（格式化）；
  - `String` → `Int` **拒绝**（除非显式转换节点），因为「解析失败」是运行时错误；
  - 无载荷事件只能接到不接收值的动作。
- 该规则表必须**数据化**（可被设计器读取），不得硬编码在 match 里；
- 现有 `src/json/events.rs` 的 `EventHandlerMap` **保留**（它是运行时的 handler 注册表），
  但**键的来源**改为遍历 capability 的 `events`。

### 3.4 步骤 4：自动接线

**前置**：步骤 1 完成（载荷类型已知）。

- `Widget` trait 增加按名取信号的入口，例如
  `fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef>`；
- `EventSignalRef` 需能承载「无载荷」与「有载荷」两种（`Signal1<CapabilityValue>` 或
  一个把强类型擦成 `CapabilityValue` 的适配层）；
- 库提供**单次调用**接完某控件全部已发布事件（新规则 #98），
  例如 `EventSignalBinder::forward_all(widget)`；
- 同时提供**可查询**「是否已接线」（新规则 #97）。

> **明确不做的**：不把 hub 变成「必须存在」的全局状态。`EventSignalBinder::detached()`
> 的既有语义保留（宿主可以显式选择不接）。

### 3.5 任务 A：镜像逐个判定

对 `WindowState` 每个字段产出一张表（§2.6 是**阅读结论**，本任务把它**测试化**）：

- 每个字段一条测试，断言其**被读取**（构造「平台返回 `None`」的场景，验回退值）
  或**已删除**；
- 把 §2.6 的判定表写入代码注释，使后来者不必重新推导；
- 加门禁（或测试）防止**新增悬空字段**：`WindowState` 的每个字段名必须在
  测试中被引用。

---

## 四、多平台门控（规则 #92/#47 继承）

本计划**不得**破坏既有 profile 体系：

- `EventSchema` 定义在 `src/widget/capability/types.rs` → 该模块已受 `full_widgets` 门控；
- 步骤 4 的自动接线依赖 capability 表 → **`mini`/`embedded` 下不存在**，
  必须 `cfg` 分叉或整体不编译，**不得**自造第五个别名（规则 #92/#47）；
- 步骤 2 的 JSON 导出依赖 `serde` → 检查 `mini` 下 `serde` 是否可用
  （`mini` feature 列表：`software`/`controls-custom`/`heapless`/`hashbrown`/`spin`/`bumpalo`
  —— **不含 `serde`**），因此导出入口必须门控在 `full_widgets`；
- **门控放底层**（用户一贯要求）：`EventSchema` 本身的类型定义与门控开关放在
  capability 层，调用点不写 `cfg`。

### 4.1 设计器两条路径的 profile 可用性矩阵（D7-c 的汇总）

| 能力 | 门控别名 | desktop/tablet/mobile | `mini` | `embedded` |
|---|---|---|---|---|
| 控件树本体（`add_child`/`try_add_child`） | 无（始终编译） | ✅ | ✅ | ✅ |
| `crate::create_*` 构造函数 | `not(alloc_frugal)`（**118 处**） | ✅ | ⚠️ **多数不存在** | 需逐条核查 |
| `crate::json`（结构描述） | `full_widgets` | ✅ | ❌ | ❌ |
| `crate::view`（diff + 保身份） | `declarative_view` | ✅ | ❌ | ❌ |
| `crate::style` CSS | `widgets_unstripped`（部分） | ✅ | 部分 | 部分 |
| capability 表（`EventSchema` 所在） | `full_widgets` | ✅ | ❌ | ❌ |

**⇒ 设计器的模式 1（JSON+CSS 热改）在 `mini`/`embedded` 下整体不可用；
那两个 profile 只能用模式 2（生成 Rust 代码）。**

**依据（全部实跑）**：
- `src/lib.rs`：`#[cfg(full_widgets)] pub mod json;`、`#[cfg(declarative_view)] pub mod view;`
- `build.rs`：`is_stripped = is_mini || is_embedded`；`full_widgets = has_profile && !is_stripped`；
  `declarative_view = full_widgets && !view_opted_out`
- `tools/check_view_platform_gate.sh` **双向断言**这两个模块的在场/缺席（本地实跑 EXIT=0）

---

## 五、待决策项（阻塞后续步骤，需用户裁定）

| # | 决策 | 影响 |
|---|---|---|
| D1 | **元组载荷如何表达**（实跑：**35** 处含 `(...)`） | 决定 `PropertyValueKind` 是否要加 `Tuple` 变体 |
| D2 | **`Option<T>` 载荷如何表达**（实跑：**9** 处） | 决定是否加 `Optional` 包装 |
| D3 | **`Vec<T>` 载荷如何表达**（实跑：**6** 处） | 决定是否加 `List` 变体 |
| D4 | **领域类型序列化格式**（`Font`/`Date`/`Time`/`Shortcut`/`KeySequence`） | 决定 JSON 表示，影响往返测试 |
| D5 | **CSS 语法形态**（事件绑定写在 CSS 还是 JSON？） | 决定步骤 3 的落点 |
| D6 | **设计器的连线目标**（只能是属性？还是也能调用命令？） | 决定步骤 3 的类型兼容表规模 |
| **D7** | **设计器产物形态**（拆为 D7-a / D7-b，见 §5.1） | 决定步骤 3 的**落点**；【已决】但需记录 |
| **D7-c** | **stripped profile 下只能用模式 2**（见 §5.2） | 决定 T-23 生成器的**双模板**与 T-24 的断言 |

#### 5.0.1 D1–D4 **已裁定**（2026-09-20 本轮执行）

**裁定原则：不发明语义——把「值是什么」与「值长什么样」拆成两个正交字段。**

- `PropertyValueKind` 回答「**值是什么**」（供连线类型校验、JSON 映射）；
- `EventPayloadShape` 回答「**值的排布**」（供面板显示「值：数字」还是「值：2 个数字」）。

| # | 裁定 | 理由 |
|---|---|---|
| **D1** | 元组**不**加 `PropertyValueKind` 变体，改由 `shape` 表达：`Tuple2`/`Tuple3`/`Tuple4`/`OptionalTuple2`；**混合元组**（各分量不同类）为 `shape = Mixed`、`payload = String` | 扁平元组方案不成立（实存 `Option<(usize,usize)>`、`(String,u32,String)`）；扩充 kind 会退化成 `Tuple2UInt` 组合爆炸。`Mixed` 段声明 `String` 是**该值跨边界时的载体**，`shape` 才是「它比标量更丰富」的诚实陈述 |
| **D2** | `Option<T>` → `payload = T 的 kind`，`shape = OptionalScalar`；`Option<(T,T)>` → `shape = OptionalTuple2`。缺失值用 `CapabilityValue::Null`（**已有变体**，不新增） | `Option<usize>` 的值域就是 `usize ∪ {无}`，而 `Null` 已承担「有值但为空」的语义 |
| **D3** | `Vec<T>` → `payload = T 的 kind`，`shape = ListScalar` | 空列表与 `Null` 是不同事实（前者「选中 0 个」，后者「无选择」），所以是独立 shape 而非与 D2 合并 |
| **D4** | 领域类型**全部**以 token 字符串入表（`payload = String`），清单 = `DOMAIN_AS_STRING` | 设计器**不需要**理解领域对象即可显示与转发它；给每个领域类型造一套 JSON 表示是凭空发明语义（原则 #5）。`Color`/`Rect` 例外——它们**已有** `CapabilityValue` 变体，走既有管道（原则 #54） |

**D4 逐类型清单**（`payload = String`，理由：值本身即其 token 拼写）：
`Font`/`Date`/`Time`/`DateTime`/`chrono::NaiveDate`/`Shortcut`/`KeySequence`/`Orientation`/
`DockWidgetArea`/`DockWidgetFeatures`/`StandardButton`/`ButtonState`/`CheckState`/
`ToggleButtonState`/`NavigationEvent`/`DragPayload`/`BarcodeResult`/`FilterExpr`/
`SignatureStroke`/`DateRange`/`CardPosition`/`PropertyValue`/`BookSide`；
`Point` 亦为字符串（`"x,y"`）；`ObjectId` 为 `UInt`（与属性侧同一表示，原则 #54）。

> **落地证据**：326 对 `(control, event)` 全部由 `tools/derive_event_payloads.py` 从信号声明
> 反推写入 `src/widget/capability/event_payloads.rs`；
> `tools/check_event_payload_types.py` 独立复核（含反向注入）。
>
> **D5/D6 仍待裁定**：它们只影响步骤 3（T-3），不阻塞 T-2/T-4。

> 上述 D1–D3 计数来自 `tools/list_event_payload_shapes.py` 实跑（扫 `pub <name>: SignalN<...>`）。
> **合计 50 处**（35 + 9 + 6）载荷不是标量——**占全仓信号声明的约 14%**。
> 这是步骤 1 不可回避的规模，不是边角情况。
>
> ⚠️ **实跑还揭示了两件比计数更要紧的事**（D1/D2 的难度来源）：
> 1. **元组会嵌套**：存在 `((f32, f32), (f32, f32))`、`Option<(usize, usize)>`、
>    `(String, Vec<String>)`、`(String, PropertyValue)`。
>    ⇒ 任何「扁平元组」方案（如只加 `Tuple2`/`Tuple3`）**不成立**，需要递归结构。
> 2. **载荷里有一批领域类型，不是标量**：`BarcodeResult`、`DragPayload`、`NavigationEvent`、
>    `FilterExpr`、`SignatureStroke`、`DateRange`、`CardPosition`、`PropertyValue`、
>    `StandardButton`、`Orientation`。
>    ⇒ D4 不是「几个日期类型怎么序列化」，而是**一批领域对象要不要跨 JSON 边界**。
>    这一项很可能需要**逐类型裁定**（哪些进设计器、哪些不进），是本计划最大的不确定度。

> D1–D4 是**步骤 1 的阻塞项**；D5/D6 影响步骤 3，可稍后定。

---

### 5.1 D7：设计器产物形态（**已裁定：两套共存**）

#### 5.1.1 为什么这不是「现代 vs 传统」之争（取证）

「哪种更现代」**不能作为判据**，因为两种模式今天都大获成功：

| | 不重编译（数据即真相） | 转代码重编译（源码即真相） |
|---|---|---|
| 代表 | Flutter（热重载）、Qt QML、Web（HTML/CSS） | SwiftUI、Jetpack Compose、React |

**反例必须说清**：React 是**转代码的**（JSX→JS，要重新构建），却是最主流的「现代」方案；
Web 的 HTML/CSS 是**不重编译**的，却是最「传统」的。

#### 5.1.2 真正的三条判据（且它们指向不同答案）

| 判据 | 胜方 | 具体理由（本库相关） |
|---|---|---|
| **① 迭代速度** | **模式 1** | 毫秒级 vs **Rust 重编译（秒到分钟）**。这是**数量级**差别 |
| **② 产物干净度 / 运行时开销** | **模式 2** | 模式 1 必须打包 `src/json/`（实测：162 个 kind 名、10 种 layout、属性路由、CSS 集成）+ CSS 解析器 |
| **③ 类型安全 / 可验证性** | **模式 2** | 模式 2 编译期暴露错误；模式 1 全靠运行时校验 |

> **Rust 的编译速度是决策的关键权重**。SwiftUI 能用模式 2，很大程度因为 Swift 增量编译
> 比 Rust 快得多。在 Rust 上，「每次改 UI 都重编译」的代价明显更高。

#### 5.1.3 AI 时代把判据①的权重放大了（但要说两面）

AI 的核心瓶颈是**试错次数**：AI 写代码 → 看结果 → 改。回路越快，迭代越多，质量越高。

| 维度 | 模式 1 | 模式 2 |
|---|---|---|
| AI 一轮反馈延迟 | **毫秒** | 秒~分钟（Rust 编译） |
| AI 一轮能试几个方案 | **几百个** | 几个 |

⇒ **模式 1 在 AI 时代优势被放大**。

**但必须同时承认模式 1 在 AI 时代的两个真实劣势：**

1. **AI 容易生成「看起来对、跑起来错」的 JSON**——没有编译器兜底。
   *这正是本计划要补 `EventSchema` 与渲染期校验的**根本理由***。
2. **AI 改 JSON 时缺结构性约束**——模式 2 的生成器可以把约束写进代码模板，
   模式 1 只能靠 schema 校验。

#### 5.1.4 裁定：**分层，两套共存**（用户已同意）

```
设计器开发期  → 模式 1（JSON + CSS 热改，不重编译）   ← 反馈回路，AI 友好
预览 / 调试   → 模式 1
交付产物      → 模式 2 可选（生成 Rust 代码）        ← 去运行时开销，得编译期检查
```

**本库已经同时具备两条路的零件**（实跑取证）：

| 零件 | 位置 | 作用 |
|---|---|---|
| 结构描述 | `src/json/`（JSON → 控件树） | 模式 1 的运行时 |
| 外观描述 | `src/style/css.rs` + `Widget::apply_css` | 外观，与结构单向依赖 |
| **CSS 热重载** | `src/style/css_watcher.rs`（`CssWatcher::poll`） | **已存在，毫秒级** |
| **diff + 保身份** | `src/view/`（`ViewEngine::mount` / `update`） | 焦点/滚动/内部状态在更新后存活 |
| **代码生成范式** | `tools/generate_*.py`（**5 个**）+ `check_abi.sh` 的「重生→对比」门禁 | 模式 2 的**现成工程范式** |

> ⚠️ **关键架构发现（决定两套共存的成本）**：
> `ViewEngine::mount/update` 的签名是
> ```rust
> pub fn mount(&mut self, view: &dyn View, create: &dyn Fn(&Node) -> Option<ObjectId>) -> ApplyReport
> ```
> 它接受一个 **`create` 回调**。这意味着 **`create` 就是模式 1 与模式 2 的公共接口**：
> - 模式 1：`create` 由 JSON 加载器提供（运行时查名字表）；
> - 模式 2：`create` 由**生成的 Rust 代码**提供（编译期已确定类型）。
>
> **两种模式不需要两套架构，只需要两个 `create`。** 这是本裁定成立的技术依据。

**先例**：Flutter（开发热重载 + 产物编译）、Qt/QML 同构。这不是折中，是成熟形态。

#### 5.1.5 D7 对后续步骤的影响

- **D7-a（开发期模式 1）**：**确定采用**，且零件已备（`css_watcher` 已存在）。
  → **T-3 的绑定模型必须有运行时形态**（handler 注册表 + 名字→信号解析）。
- **D7-b（交付期模式 2）**：**可选**，但一旦要做，它需要一个**代码生成器**（新任务，见 T-23）。
  → 生成器的输出模板**重用 T-3 的类型兼容规则**（同一套规则，两个消费者）。
- **对本计划四个步骤的净影响**：
  - 步骤 1（`EventSchema`）**两模式都需要** → 不变
  - 步骤 2（JSON 往返）**两模式都需要** → 不变
  - 步骤 3（绑定模型）**需同时产出「运行时规则表」与「生成器模板」两份消费者** → 范围微增
  - 步骤 4（自动接线）**只有模式 1 需要**（模式 2 的接线已在生成的代码里） → 范围略降

> **结论**：共存不显著增加前期成本，因为公共接口（`create`）已存在、
> 类型兼容规则可复用。真正的增量是两个新任务（T-23 代码生成器、T-24 模式一致性门禁）。

#### 5.1.6 待补充的裁定（D7-b 内部）

| # | 子决策 | 选项 |
|---|---|---|
| D7-b-1 | 交付形态 | (a) 打包 JSON 运行时 / (b) 生成 Rust / (c) 两者都支持 |
| D7-b-2 | 若生成 Rust：目标是 `Node` 构建代码还是 `add_child` 命令式代码？ | **已由 D7-c 回答：按 profile 分叉，两者都要** |
| D7-b-3 | 生成物是否入库（committed）？ | 入库便于 review；不入库避免漂移（参照 `check_abi.sh` 的「重生→对比」模式） |

---

### 5.2 D7-c：stripped profile 下**只能**用模式 2（已取证，需用户确认）

#### 5.2.1 问题

`mini` 与 `embedded` 是否也采用「两套模式共存」？

#### 5.2.2 取证：本库**已经裁定过**，并写入门禁

`src/view/mod.rs:82-86` 的模块文档直接给出了答案：

| Profile | 结构描述 | 状态归属 | 声明式层 |
|---|---|---|---|
| `desktop` / `tablet` / `mobile` | **declarative**（`View` + `diff`）**或** imperative（`add_child`），可混用 | retained | ✅ compiled |
| `embedded` | **imperative**（`add_child` / `create_*`） | retained | ❌ **absent** |
| `mini` | **imperative**，`alloc_frugal` 下 | retained | ❌ **absent** |

**门控实现**（`src/lib.rs` 实跑）：

```rust
#[cfg(full_widgets)]      pub mod json;   // mini/embedded 下不编译
#[cfg(declarative_view)]  pub mod view;   // mini/embedded 下不编译
```

其中（`build.rs` 实跑）：

```
full_widgets      = has_profile && !is_stripped        // is_stripped = mini || embedded
declarative_view  = full_widgets && !no-declarative-view  // 比 full_widgets 更窄
widgets_unstripped= !is_stripped
```

**门禁强制**：`tools/check_view_platform_gate.sh` 断言「`crate::view` 在 desktop/tablet/mobile 存在、
在 mini/embedded 缺席」**双向成立**。本地实跑：**EXIT=0 通过**。

文档的原话：

> A reader on `mini`/`embedded` should treat this module as **not existing**: the supported
> structure API there is `add_child` and the `create_*` functions.

#### 5.2.3 为什么必须这样（文档给的两条硬理由）

1. **内存预算装不下**：`mini` 是 `alloc_frugal`（定容 `heapless`），`embedded` 是
   `embedded_surface`；声明式层要 `String`/`Vec`/`HashMap` 的动态分配。
2. **没有消费者**：embedded/mini 的 UI 是**静态/手工构造**的，
   **没有「每帧重新求值 state」的调用方**，所以 `diff` 在那里**无消费者**
   （原则 #28 反过度抽象 / #93）。

#### 5.2.4 ⚠️ 取证新发现：`mini` 下**连 `create_*` 大多也不存在**

实跑：`grep -c "cfg(not(alloc_frugal))" src/lib.rs` → **118**。
即 `create_button`/`create_label` 等构造函数中，**大批在 `mini` 下被门控掉**。

| API | `mini` 下 | `embedded` 下 |
|---|---|---|
| `BaseWidget::add_child` / `try_add_child` | ✅ 可用（**无 cfg**，实跑确认） | ✅ 可用 |
| `crate::create_*`（118 处 `not(alloc_frugal)`） | ⚠️ **多数不存在** | 需逐条核查 |
| `crate::view` / `crate::json` | ❌ 不存在 | ❌ 不存在 |

> **这直接修正了对模式 2 输出模板的要求**：生成物在 `mini` 下不能假设
> `create_button` 存在——它必须回退到「构造控件对象 + `add_child`」，
> 且容量受 `MiniVec` 的 64 上限约束（见 `BaseWidget::child_capacity`）。

#### 5.2.5 裁定建议：**stripped profile 下模式 2 是唯一形态**

| 模式 | desktop/tablet/mobile | `mini` / `embedded` |
|---|---|---|
| **模式 1**（JSON + CSS 运行时热改） | ✅ | ❌ **不可能**（`json`/`view` 不编译，且内存预算不足） |
| **模式 2**（生成 Rust） | ✅ 可选 | ✅ **唯一形态** |

**原因**：模式 2 的产物是**纯 Rust 代码**，没有 JSON 解析器、没有 CSS 解析器、
没有 diff 引擎；编译期确定，**零运行时求值开销**——这正是 embedded/mini 所需的形态。

⇒ **设计器对 embedded/mini 的价值恰恰在模式 2**，而且在那里
**模式 2 不是「可选项」，是唯一可能的形式**。

#### 5.2.6 D7-c 对 T-23 / T-24 的硬约束

**T-23（代码生成器）的输出模板必须有两个变体**：

| 目标 profile | 生成内容 | 禁止生成 |
|---|---|---|
| `desktop`/`tablet`/`mobile` | 可用 `Node` 构建代码（复用 `crate::view`） | — |
| **`mini`/`embedded`** | **`add_child` 命令式代码**；不得引用 `crate::view`/`crate::json`/被门控的 `create_*` | ❌ `crate::view`、`crate::json`、`cfg(not(alloc_frugal))` 的 `create_*` |

**判据**：生成的 stripped-profile 代码必须能在 `--no-default-features --features mini` 与
`--features embedded` 下**实跑编译通过**——这是 T-23 的 DoD 条目，不是「应该没问题」。

**T-24（两模式一致性门禁）需额外断言**：生成物的**可用 API 集合**与目标 profile 一致——
即不得出现「在 mini 下生成 `create_button` 调用」这类**跨 profile 串味**。

---

### 5.3 D7-d：设计器如何**兼容** mini/embedded 开发

> §5.2 回答了「产物形态」，本节回答「设计器**侧**要做什么才能服务那两个 profile」。

#### 5.3.1 先纠正一个易错点：`mini` 与 `embedded` **不是同一种受限**

实跑取证（两者常被当作同一回事，实际不同）：

| | `mini` | `embedded` |
|---|---|---|
| 门控别名 | `alloc_frugal`（`no_std`） | `embedded_surface` |
| capability 层 | ❌ **整体编译掉**（`not(alloc_frugal)`） | ✅ **保留**，但用静态表命名 |
| 控件名解析 | 不存在 | `canonical_name_for_kind`（**手写静态 match**） |
| 该表覆盖 | — | **仅 18 个 kind**，其余返回 `""` |

`src/widget/capability.rs:390-395` 的注释是这个设计的依据：

> Returns `""` for an unmatched kind, which the constructor lookup reads as "not available in
> this profile" — the same answer it gives for a kind the `embedded` widget set does not ship.
> **Inventing a name here would let the factory build a control the profile does not have.**

⇒ **`embedded` 的控件集比 desktop 小得多，且是编译期确定的。**

#### 5.3.2 关键利好：布局引擎**在所有 profile 都可用**

实跑：`src/lib.rs` 中 `pub mod layout;` **无任何 `cfg` 门控**，而 `pub mod json;` 受
`full_widgets`、`pub mod view;` 受 `declarative_view`。

且布局层**零平台依赖**（实跑 `grep -c "platform::" src/layout/*.rs` → 全部为 0）。

更关键的是它的接口形状（`src/layout/types.rs:54-60`）：

```rust
fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
```

**这是一个纯函数式的回调**——布局**不需要真实控件存在**，只需 `ObjectId → Rect` 的回调。

⇒ **设计器可以在生成期独立跑布局，不需要目标机、不需要真实控件树。**
这是 D7-d-3（布局在生成期算死）**可行的技术依据**。

#### 5.3.3 四条硬约束（D7-d-1 ~ d-4）

| # | 约束 | 理由 | 验证判据 |
|---|---|---|---|
| **D7-d-1** | **控件可用性必须按目标 profile 查询**，设计器**不得**自维护清单 | `embedded` 只有 18 kind；选错了生成物编译失败 | 生成前能报告「哪些控件在目标 profile 不可用」 |
| **D7-d-2** | **CSS 在生成期解析并内联为属性赋值**，生成物**不含** `apply_css` 调用 | CSS 引擎部分受 `widgets_unstripped` 门控；目标 profile 上无样式引擎 | 生成物中 `grep "apply_css"` → 0 命中 |
| **D7-d-3** | **布局在生成期算出常量坐标**，生成物不依赖目标机运行时布局 | embedded/mini 的 UI 是静态构造，**没有每帧布局的调用方**（§5.2.3 理由 2） | 生成物中的 `set_geometry` 参数是常量 |
| **D7-d-4** | **容量约束在生成期检查并报错** | `mini` 下 `BaseWidget::children` 是定容 **64**（`MiniVec`），超限会静默丢子控件 | 超限时报告；可用 `child_capacity()`/`child_overflow_count()` |

> **D7-d-3 是最容易漏、也最贵的一条**：它要求设计器的布局引擎能在**生成期独立运行**，
> 而不依赖目标运行时。§5.3.2 已证明本库的布局层满足这个条件（无门控、无平台依赖、
> 回调式接口），但**必须先确认设计器用的是这套 `Layout` trait**，而不是自己重写一套。

#### 5.3.4 设计器的形态（关键认知）

```
┌───────────────────────────────────────────────┐
│ 设计器（**永远跑在 desktop 宿主上**）            │
│  ┌─────────────────────────────────────────┐  │
│  │ 画布 / 属性面板 / 事件连线                │  │
│  │  ↑ 用 JSON+CSS 预览（模式 1，热改）        │  │
│  └─────────────────────────────────────────┘  │
│                    ↓                          │
│  ┌─────────────────────────────────────────┐  │
│  │ 生成器（目标 profile 可选）               │  │
│  │  ① 查目标 profile 可用控件集  (d-1)       │  │
│  │  ② 解 CSS → 内联属性          (d-2)       │  │
│  │  ③ 跑 layout::Layout → 常量坐标 (d-3)      │  │
│  │  ④ 检查容量约束              (d-4)       │  │
│  │  ⑤ 输出：Node 代码 或 add_child 代码      │  │
│  └─────────────────────────────────────────┘  │
└───────────────────────────────────────────────┘
```

**关键认知：`mini`/`embedded` 是「生成**目标**」，不是「运行**环境**」。**
设计器**不需要**（也不可能）在 MCU 上跑。这使兼容成本大幅降低——
只需生成器多一个 profile 维度，不需设计器本体分叉。

#### 5.3.5 对 T-23 / T-24 的追加要求

- T-23 的输出**必须参数化目标 profile**，且四条约束各有验据（见 5.3.3 表）；
- T-24 需新增一条：**同一份 JSON 对三个目标（desktop / mini / embedded）生成，
  每个生成物都在真实 feature 下编译通过**；
- 若目标 profile 不支持某控件（d-1），设计器必须在**画布上**就标示出来，
  而不是等到生成时才报错（否则用户白拖了）。

---

## 六、明确**不做**的（附理由）

1. **不改 `Widget` trait 的既有方法签名**（规则 #86）：步骤 4 是**新增**方法，
   不是改写 `clicked_signal()` 等。
2. **不把 hub 变成全局单例**：`detached()` 语义保留。
3. **不移除 `src/json/events.rs` 的 `EventHandlerMap`**：它是运行时的 handler 注册表，
   与「已发布事件名」是**不同层**的东西；要做的是让它的**键**来自事件表。
4. **不为了「看起来完整」而发明载荷语义**：D1–D4 未定之前，不写死任何一种映射。
5. **不为 `mini`/`embedded` 实现设计器能力**：设计器是桌面工具（规则 #92）。

---

## 七、验证矩阵

| 步骤 | 验证 |
|---|---|
| 1 | 事件侧结构体存在；门禁逐条核对「声明载荷 == 信号真实类型」（可用脚本从源码推导）；186 个名字全覆盖 |
| 2 | JSON 往返**逐字节相同**；覆盖全部 186 个名字 |
| 3 | 类型兼容表数据化；每条规则一个测试；`on_*` 专用键迁移后行为不回退（既有 JSON 测试全绿） |
| 4 | 单次调用接完；逐事件送达断言；未接线可查询；**反向注入**（去掉接线 → 断言失败） |
| A | `WindowState` 每字段一条测试；无悬空字段（门禁或测试） |
| 全局 | 33 个既有门禁全绿；5 个 profile 全测试通过；clippy `-D warnings` 干净 |

---

## 八、优先级建议

**建议顺序：D1–D4 决策 → 步骤 1 → 任务 A（可并行）→ 步骤 2 → 步骤 4 → 步骤 3**

理由：

1. **D1–D4 必须最先定**——它们决定步骤 1 的数据形状，做错了要返工。
2. **步骤 1 是地基**，且**独立价值最高**：即使设计器推迟，类型化的事件契约本身
   就让「326 个名字」从字符串变成可用元数据。
3. **任务 A 与步骤 1 无依赖**，可并行，且它是**独立的缺陷类**（镜像悬空）。
4. **步骤 2 依赖 1**，实现直接。
5. **步骤 4 在步骤 1 之后**才可能（载荷类型已知）。
6. **步骤 3 最后**——它需要 D5/D6，且依赖 2、4 的产物。

---

## 九、与第 10–12 轮的关系

| 轮次 | 成果 | 与本计划的关系 |
|---|---|---|
| 第 10 轮 | 修 3 个跨越区间缺陷（2 个实测可达 panic） | 独立缺陷类，已完成 |
| 第 11 轮 | 清遗留探针，33/33 门禁转绿 | 本计划的验证基线 |
| 第 12 轮 | **逐名核对 326 个已发布事件名**；补 24 个「已发射未发布」名字；门禁补反方向 | **步骤 1 直接受益**：那 24 个名字现在是设计器的可选项 |
| 本计划 | 事件类型化 + JSON 往返 + 自动接线；镜像判定 | 设计器就绪 |

> 第 12 轮补的 24 个名字（`slider_pressed`、`tool_button::triggered`、
> `chart::data_point_unhovered`、`popup_window::{opened,closed}`、
> `web_engine_view` 的 5 个等）在设计器视角下从「兑现空牌子」升级为
> **「补全面板可选项」**——见 §1.3。

---

> ☞ **§十（一句话结论）与两个附录在本文件最末**，因为读者通常在翻完取证与任务表后才需要它们。

# 附录：完整剩余任务登记表（Task Register）

> 本附录把**本会话中提出的、尚未完成的全部任务**集中登记，避免散落在对话里丢失。
> 每条含：编号、任务、依赖、验证判据、状态。
>
> **状态图例**：⬜ 未开始 · 🟡 进行中 · ✅ 已完成 · ⛔ 阻塞（等待裁定）

## A. 用户指定四步 + 镜像判定（本计划主体）

| # | 任务 | 依赖 | 验证判据 | 状态 |
|---|---|---|---|---|
| **T-A** | `WindowHandle` 镜像与平台权威源的**逐个判定**（13 个字段） | 无 | 每字段一条测试证明「被读取」或「已删除」；无悬空字段 | ⬜ |
| **T-1** | 给事件加 `EventSchema`（与 `PropertySchema` 对称） | **D1–D4** | 结构体存在；门禁逐条核对「声明载荷 == 信号真实类型」；186 名字全覆盖 | ✅ 本轮完成（326 pairs / 187 controls；`check_event_payload_types.sh` PASS） |
| **T-2** | 导出设计器可消费的 JSON + **往返测试** | T-1 | 「导出→载入→再导出」逐字节相同，覆盖全部已发布名字 | ✅ 本轮完成（187 控件逐字节；反向注入已验证） |
| **T-3** | CSS/JSON 绑定模型（事件 ↔ 属性**类型兼容**） | T-1、T-2、D5、D6 | 兼容规则**数据化**；每条规则一个测试；既有 JSON 测试不回退 | ⬜ |
| **T-4** | **自动接线**（库提供单次调用接完） | T-1 | 单次调用接完并逐个断言送达；**反向注入**验证 | ⬜ |

## B. 前置决策（**D1–D4 已裁定**，见 §5.0.1）

| # | 决策 | 实跑规模 | 影响 | 状态 |
|---|---|---|---|---|
| **D1** | 元组载荷如何表达 | **35 处** | `PropertyValueKind` 是否加 `Tuple` | ✅ §5.0.1 |
| **D2** | `Option<T>` 载荷如何表达 | **9 处** | 是否加 `Optional` 包装 | ✅ §5.0.1 |
| **D3** | `Vec<T>` 载荷如何表达 | **6 处** | 是否加 `List` 变体 | ✅ §5.0.1 |
| **D4** | 领域类型序列化格式（**逐类型裁定**） | 一批领域对象 | 哪些进设计器、哪些不进 | ✅ §5.0.1（逐类型清单） |
| **D5** | CSS 语法形态（事件绑定写 CSS 还是 JSON） | — | T-3 落点 | ⛔ |
| **D6** | 连线目标（只能属性？也能调命令？） | — | T-3 兼容表规模 | ⛔ |

> **D1–D3 计数**来自 `tools/list_event_payload_shapes.py` 实跑；**D4 需逐类型过一遍**
> （建议产出一张「50 处非标量载荷 + 领域类型」清单，逐条标注「进设计器 / 不进 / 待定」）。

## C. 本会话中发现、但**独立于设计器**的剩余任务

| # | 任务 | 依据 | 验证判据 | 状态 |
|---|---|---|---|---|
| **T-5** | **`enabled` 契约收尾**——逐个判定各控件「禁用是否有语义」，并加门禁「处理输入事件的控件必须引用 `is_enabled()`」 | 用户第 52 轮遗留 | 门禁 PASS；新控件无理由不得进 `NON_INTERACTIVE` | ✅ 已建门禁（172 handler：145 guarded + 27 带理由豁免）；**逐控件复核**仍可加深 |
| **T-6** | **`events:` 声明与真实发射点的三方对齐**——逐名核对全部已发布事件名 | 用户指定 | 门禁 PASS；已发布 → 有 emit；**已发射 → 已发布** | ✅ 第 12 轮完成（326 pairs / 186 名字；补 24 个未发布名字；门禁补反方向） |
| **T-7** | **`WindowHandle` 镜像的 `icon`/`min_w`/`min_h` 是否该像 `is_maximized` 走 `mirrored_flag` 回退** | 用户指定 | 三字段判定 + 测试 | ✅ 第 10 轮判定为**回退**（6 个后端读取返回 `None`）；**逐字段测试化**归入 T-A |
| **T-8** | **JSON 事件路径与 capability 事件表的合并**（`on_click` 等 8 个硬编码键 vs 186 个名字） | 本轮取证（§2.4） | 两套职责边界写入文档；门禁验证「一侧新增事件另一侧不静默落后」 | ⬜ **新发现** |
| **T-9** | **`EventSignalBinder` 的「已发布但未接线」可查询化** | 新规则 #97 | 测试断言「未接线时返回未接线而非静默成功」 | ⬜ |
| **T-10** | **为 JSON 事件路径建门禁**（当前**零覆盖**） | 本轮取证 | 门禁存在且能抓「加了 `on_*` 键但无实现」 | ⬜ **新发现** |
| **T-23** | **代码生成器**（模式 2：JSON → Rust 源码） | D7-b、T-1、T-3 | 生成物可编译；与模式 1 行为等价 | ⬜ **D7 衍生** |
| **T-24** | **两模式一致性门禁** | T-23 | 同一 JSON 经模式 1 与模式 2 产出**行为等价**（控件树/事件/属性） | ⬜ **D7 衍生** |

## D. 多平台与工具链验证（本机能力内）

| # | 任务 | 工具 | 说明 | 状态 |
|---|---|---|---|---|
| **T-11** | Android 真机/模拟器验证 | `tools/build_android_testapp.sh` | 本机已装 Android Studio | ⬜ |
| **T-12** | iOS 验证 | `tools/build_ios_testapp.sh` | 本机已装 Xcode | ⬜ |
| **T-13** | HarmonyOS 验证 | `tools/check_harmony_cross.sh` | 本机已装鸿蒙套件；当前脚本走 `aarch64-unknown-linux-ohos` target，**未用完整 SDK 工具链** | 🟡 交叉编译 PASS，真机未验 |
| **T-14** | macOS / Windows / Linux 桌面运行时 | `cargo test` | macOS 本机可跑；Windows/Linux 需目标机 | 🟡 macOS 已跑 |
| **T-15** | 语言绑定逐参数契约核对（Python/Node/C++/Java/Android/iOS 共 6 个） | `tools/check_binding_symbol_coverage.sh` | 现只核对**符号存在**，未逐参数比对个数与所有权 | 🟡 符号覆盖 PASS，逐参数未做 |

## E. 文档与版本同步

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **T-16** | 版本升级与全仓同步（不只版本号，还有全部变化内容） | `tools/check_changelog_sync.sh` PASS；20 个文件同步 | ✅ 本会话已完成（2.4.6） |
| **T-17** | 本计划的文档同步 | 设计器相关变化写入 CHANGELOG / README / cookbook | ⬜ 待 T-1 落地后 |
| **T-18** | 日志记录（每个已修项必须标识，尤其结尾） | `docs/log/log-20260920-2.md` 逐轮含「已修清单 + 验证证据」 | ✅ 第 10–12 轮已记 |

## F. 质量门禁与验证纪律（每轮遵守）

| # | 纪律 | 依据 |
|---|---|---|
| **T-19** | 每个修复必须**实测可达**才改；改完**反向注入**验证（revert → 必须 FAIL） | 原则 #64 |
| **T-20** | 每轮不跑门禁、不跑全量测试；**只在最后跑一次**全量 | 原则 #55/#56 |
| **T-21** | 所有可能阻塞的命令**必须设超时**；绝不裸跑「全量门禁 + 全量测试」串行循环 | 原则 #58/#59 |
| **T-22** | 门控放**底层**，尽量精炼；不得自造第五个 profile 别名 | 原则 #47/#92，用户要求 |

---

# 建议执行顺序（依赖拓扑）

```
D1–D4 裁定（⛔ 阻塞）
   │
   ├──> T-1 事件 EventSchema ──┬──> T-2 JSON 往返 ──┐
   │                           │                    ├──> T-3 绑定模型（需 D5/D6）
   │                           └──> T-4 自动接线 ────┘
   │                                      │
   │                                      └──（D7：两模式共存）
   │                                             │
   │                              T-23 代码生成器 ──> T-24 模式一致性门禁
T-A 镜像判定（独立，可并行）
T-8 / T-9 / T-10（JSON 路径合并 + 可查询 + 门禁；T-9 依赖 T-1，T-8/T-10 可提前）
T-5 / T-6 / T-7（已基本完成，T-7 的测试化并入 T-A）
T-11 ~ T-15（真机与绑定验证，可随时并行）
T-16 ~ T-18（版本/文档/日志，随各步落地同步）
```

**最小可交付切片（若有时间压力，建议先做这一条）**：

> **T-A + D1–D4 裁定清单 + T-8**
>
> 理由：T-A 是独立缺陷类可立即做且判据明确；D1–D4 清单是 T-1 的前置且不依赖任何实现；
> T-8 是本轮新发现的「两套并行机制」缺陷，与设计器直接相关。
> 三者**互不阻塞**，且都不需要先写生产代码。

---

# 附录二：每步的「完成定义」（Definition of Done）

> 新对话以此为验收标准。每条都是**可执行、可失败**的判据，不是形容词。
> 判定「完成」时必须能贴出**实跑输出**。

## DoD-T-A（镜像逐字段判定）

- [ ] `WindowState` 的 **13 个字段**每个都有一条测试，且测试**真的能区分**两种情况：
  - 「回退」类：构造**平台读取返回 `None`** 的场景，断言回退值被返回
    （`is_maximized`/`is_minimized`/`is_fullscreen`/`icon`/`min_size`）。
  - 「有读取」类：断言写入后能被读回（`x`/`y`/`w`/`h` 经 `apply_window_layout`；
    `close_callback` 经 `close()`）。
- [ ] **反向注入**：删掉任一字段的读取点 → 对应测试必须 FAIL。
- [ ] 有一条防新增悬空字段的检查（门禁或测试），且**已知**它当前是 PASS。
- [ ] §2.6 的判定表已写入代码注释。

## DoD-T-1（事件 EventSchema）

- [x] `EventSchema` 结构体存在，且 `WidgetCapability.events` 类型已变为 `&[EventSchema]`。
- [x] **187 处** `events:` 构造点全部迁移编译通过（实跑 `grep -c "events:" properties.rs` = 187）。
- [x] 载荷类型**逐条**从信号声明反推并填入；`tools/list_event_payload_shapes.py`
      的输出与表内声明**逐条一致**。
- [x] 新门禁：对每个已发布事件，核对「`EventSchema.payload` == 该名字对应信号的
      真实 Rust 类型」。**反向注入**：把某条改成错的类型 → 门禁必须 FAIL。
- [x] `connect_event` 外部行为**不变**（既有 `capability_event_surface_test` 7 个测试全绿）。
- [x] `mini`/`embedded` 下不引入新编译错误（`check_profiles.sh` PASS）。

## DoD-T-2（JSON 往返）

- [x] 能导出某控件的 capability manifest 为 JSON 字符串（`capability_manifest_json`）。
- [x] 往返测试存在，且**用独立解析器**（`DesignerManifest::from_json`，非同一函数），
      断言 `s1 == s2` **且** `s1` 含预埋哨兵（见 §3.2.1）。
- [x] **反向注入**：让导出器漏掉一个事件 → 测试必须 FAIL（实测 187 控件逐个报出）。
- [x] 覆盖**全部 186 个已发布名字**（不是抽样）。

## DoD-T-3（绑定模型）

- [ ] 类型兼容规则**数据化**（可被设计器读取），不是硬编码在 `match` 里。
- [ ] 每条规则**至少一个测试**，含**拒绝**分支（`String → Int` 必须被拒）。
- [ ] JSON 键从 `on_*` 专用键迁移到 `events: { "<published_name>": ... }`，
      且**既有 JSON 测试全绿**（行为不回退）。
- [ ] `EventHandlerMap` 保留但键来源改为遍历 capability `events`（规则 #101）。

## DoD-T-4（自动接线）

- [ ] 存在**单次调用**接完某控件全部已发布事件（规则 #98）。
- [ ] 测试：对**若干**控件（至少覆盖 unit + payload 两种）一次调用接完，
      逐个断言其已发布事件**都能送达**（订阅 → 驱动控件 → 槽被调用）。
- [ ] 「是否已接线」**可查询**（规则 #97），且有测试断言未接线时返回未接线。
- [ ] **反向注入**：去掉自动接线 → 送达断言必须 FAIL。
- [ ] `detached()` 语义保留；`mini`/`embedded` 下整体不编译。

## DoD-T-8 / T-10（JSON 路径合并 + 门禁）

- [ ] 两套机制的**职责边界写入文档**（哪个名字走哪个键）。
- [ ] 门禁能抓「加了 `on_*` 键但无实现」（反向注入验证）。
- [ ] 门禁能验证「capability 新增事件 → JSON 侧不静默落后」（规则 #101）。

## DoD-T-23 / T-24（两模式共存，D7 衍生）

> **前提**：D7-b 已裁定要做模式 2。若裁定「只做模式 1」，本节置空。
> **注意**：只要设计器要对 `mini`/`embedded` 有产物，模式 2 就是**刚需**（D7-c，§5.2）。

- [ ] 代码生成器能把一份 JSON 转为**可编译的 Rust 源码**；
- [ ] 生成物中**不含** JSON/CSS 运行时依赖（若选 D7-b-1 = (b)）；
- [ ] 生成器复用 T-3 的类型兼容规则（**不是**另写一套）；
- [ ] **双模板**（D7-c）：
  - [ ] desktop/tablet/mobile 模板可生成 `Node` 构建代码（复用 `crate::view`）；
  - [ ] **`mini`/`embedded` 模板生成 `add_child` 命令式代码**，且
        **不引用** `crate::view`/`crate::json`/受 `cfg(not(alloc_frugal))` 门控的 `create_*`；
  - [ ] stripped 模板的产物**实跑编译通过**：
        `--no-default-features --features mini` 与 `--features embedded`（**不是**「应该没问题」）；
  - [ ] `mini` 下若控件数超 64，生成物会碰到 `BaseWidget` 的定容上限——
        生成器必须能报告这一点（`child_capacity()` / `child_overflow_count()`）；
- [ ] **模式一致性门禁（T-24）**：同一份 JSON 分别经模式 1 与模式 2，
      产出**行为等价**——断言控件树结构、已发布事件、属性值一致。
      **反向注入**：让生成器丢一个控件 → 门禁必须 FAIL。
- [ ] **T-24 的 profile 串味断言**：生成物用到的 API 集合必须与目标 profile 一致
      （如不得在 mini 产物里出现 `create_button` 调用）；**反向注入**验证。
- [ ] 若生成物入库（D7-b-3 = 入库），则有「重生→对比」门禁
      （参照 `tools/check_abi.sh` 第 1 步的现成范式）。

## 全局 DoD（整个计划完成时）

- [ ] `bash tools/run_all_gates.sh` → **PASS=33 FAIL=0 TIMEOUT=0**（含新门禁则为 33+n）。
- [ ] 5 个 profile 全测试通过：`desktop` / `tablet` / `mobile` / `mini` / `embedded`。
- [ ] `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` → 0 警告。
- [ ] 每个已修项在 `docs/log/` 的日志中**逐条标识**（尤其结尾的「已修清单」）。
- [ ] 版本号与文档同步（`check_changelog_sync.sh` PASS）。

---

## 十、本文件的一句话结论

**设计器前提把事件侧从「字符串数组」升级为「必须类型化的契约」：**
必须先定元组/Option/Vec/领域类型的表达方式（D1–D4），
再做 `EventSchema`（步骤 1），
然后用它同时解锁 JSON 往返（步骤 2）、自动接线（步骤 4）与类型兼容的绑定模型（步骤 3）；
`WindowHandle` 镜像的逐字段判定（任务 A）是独立缺陷类，可并行推进。

**产物形态已裁定为「两套模式共存」（D7）**：开发期 JSON+CSS 热改（AI 反馈回路毫秒级），
交付期可选生成 Rust 代码（去运行时开销、得编译期检查）。
技术依据是 `ViewEngine` 的 `create` 回调——它天然是两模式的公共接口，
因此共存**不需两套架构，只需两个 `create`**（详见 §5.1）。

**但 `mini`/`embedded` 是例外（D7-c，§5.2）**：那两个 profile 下 `crate::json` 与 `crate::view`
**整个不编译**（本库已裁定并有门禁 `check_view_platform_gate.sh` 双向断言），
所以**模式 1 不可能，模式 2 是唯一形态**；且 `mini` 下有 **118 处 `create_*` 被
`cfg(not(alloc_frugal))` 门控掉**，生成物不得引用它们。

**在 D1–D4 裁定之前，不应动任何实现代码**——那是凭空发明语义，正是本项目一直在删的东西。

---

## 十一、新对话的开场指令（可直接复制）

> 按 `docs/plans/blue19.md` 执行。先读 §零（实现者入口）与 §五（待决策项）。
> **D7 已裁定**：两套模式共存（§5.1）——开发期 JSON+CSS 热改，交付期可选生成 Rust；
> 不要重新讨论这个决策。
> 若 D1–D4 已裁定，从 T-1 开始；若未裁定，先做 T-A + 产出 D1–D4 裁定清单。
> 遵守 §零.3 的约束，验收按「附录二：每步的完成定义」逐条对照。
