# 设计器白皮书（草案 v0.1）

> 状态：**草案**，尚未评审
> 关联：[`blue19.md`](blue19.md)（事件契约类型化 + 设计器就绪的执行计划）、
> [`principle.md`](principle.md)（项目原则 #1–#101）
> 本文档描述**设计器是什么、为什么这样设计**；`blue19.md` 描述**怎么做**。
>
> **取证说明**：本文所有「本库已具备 X」的陈述均在当前工作树上实跑验证，
> 关键处附命令或在 §6 汇总。**未验证的不写入。**

---

## 1. 一句话定位

**一个 JSON + CSS 的可视化 UI 设计器，产出可编译的 Rust 代码（以及可选的 JSON 运行时），
底层使用 rust-widgets 库。**

它不是运行环境，是**开发工具**：设计器本身永远跑在桌面宿主上，
`mini`/`embedded` 是它的**生成目标**，不是它的运行环境（§5.3）。

---

## 2. 设计目标

| # | 目标 | 判据 |
|---|---|---|
| G1 | **所见即所得**，改一处立即看到 | 反馈延迟毫秒级，不重编译 |
| G2 | **产物是干净的本项目代码** | 生成物可 `cargo build`，无设计器运行时依赖 |
| G3 | **AI 友好** | 反馈回路短；结构可由 AI 生成与修改 |
| G4 | **覆盖全部 profile** | desktop / tablet / mobile / mini / embedded |
| G5 | **不说谎** | 不可用的能力在画布上就标示，不在生成时才失败 |

---

## 3. 为什么是 JSON + CSS

**职责分离**，且与库既有分工一致（实跑取证）：

| 关注点 | 载体 | 本库模块 | 不重叠的证据 |
|---|---|---|---|
| **结构**（有哪些控件、怎么嵌套） | JSON | `src/json/` | — |
| **外观**（颜色、边框、字体） | CSS | `src/style/css.rs` | `grep -c "children\|layout" src/style/css.rs` = `0` |

依赖是**单向的**：`src/json/` 通过 `Widget::apply_css` 调用 CSS 解析器。
所以两者可以独立演进，且 CSS 不会因 JSON 层被移除而消失。

### 3.1 为什么不发明新格式

**JSON + CSS 是 AI 时代的最大公约数**：

- AI 对两者的训练数据量最大，生成质量最高
- 无需为设计器教 AI 一套新语法
- CSS 的样式直觉（选择器、层叠、变量）已被广泛理解

---

## 4. 架构

### 4.1 总览

```
┌─────────────────────────────────────────────────────────┐
│ 设计器（永远跑在 desktop 宿主）                            │
│                                                         │
│  ┌───────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ 画布           │  │ 属性面板      │  │ 事件连线      │  │
│  │ (模式 1 预览)  │  │ (PropertySchema)│ │ (EventSchema) │  │
│  └───────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│          │                 │                 │          │
│          └─────────────────┴─────────────────┘          │
│                            │                            │
│                  ┌─────────▼─────────┐                  │
│                  │ 文档模型 (JSON+CSS) │                  │
│                  └─────────┬─────────┘                  │
│                            │                            │
│                  ┌─────────▼─────────┐                  │
│                  │ 生成器 (多目标)    │                  │
│                  │  ① 查可用控件 (d-1)│                  │
│                  │  ② 解 CSS 内联 (d-2)│                 │
│                  │  ③ 跑布局 (d-3)    │                  │
│                  │  ④ 查容量 (d-4)    │                  │
│                  └─────────┬─────────┘                  │
└────────────────────────────┼────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │ desktop  │  │  mini    │  │ embedded │
        │ 产物      │  │ 产物      │  │ 产物      │
        └──────────┘  └──────────┘  └──────────┘
```

### 4.2 两套产物模式（D7）

| 模式 | 产物 | 何时生效 | 适用 profile |
|---|---|---|---|
| **模式 1** | JSON + CSS 资源，运行时加载 | 改文件即生效，**不重编译** | 仅 desktop/tablet/mobile |
| **模式 2** | 生成的 Rust 代码 | 需重新编译 | **全部**（mini/embedded **只能**用这个） |

**技术依据**：`ViewEngine::mount/update` 接受一个 `create: &dyn Fn(&Node) -> Option<ObjectId>`
回调——**它天然是两模式的公共接口**（模式 1 由 JSON 加载器提供，模式 2 由生成代码提供）。
因此两套共存**不需两套架构，只需两个 `create`**。

### 4.3 为什么两套并存，而不是只做一套

| | 模式 1（不重编译） | 模式 2（重编译） |
|---|---|---|
| 代表 | Flutter 热重载、Qt QML、Web | SwiftUI、Compose、React |
| 迭代速度 | **毫秒** | **Rust 重编译（秒到分钟）** |
| 产物干净度 | 需打包解析器 | **纯 Rust** |
| 类型安全 | 运行时校验 | **编译期** |

**「哪个更现代」不是判据**：React 是转代码的却最主流，Web 的 HTML/CSS 是不重编译的却最传统。
两种都大获成功。

真正的判据是三条，且指向不同答案：**迭代速度 → 模式 1；产物干净度 → 模式 2；类型安全 → 模式 2。**

所以**分层**（先例：Flutter 开发热重载 + 产物编译；Qt/QML 同理）：

- **开发期**：模式 1（反馈回路，AI 友好）
- **交付期**：模式 2 可选（去运行时开销，得编译期检查）

**AI 时代把「迭代速度」的权重放大了**——AI 的核心瓶颈是试错次数，模式 1 把每次试错
成本降两个数量级。但也要承认模式 1 的两个真实劣势：AI 易生成「看起来对、跑起来错」的 JSON
（这正是 `blue19.md` 要补 `EventSchema` 的根本理由），且缺结构性约束。

---

## 5. 关键约束

### 5.1 profile 可用性矩阵（实跑取证）

| 能力 | 门控别名 | desktop/tablet/mobile | `mini` | `embedded` |
|---|---|---|---|---|
| `BaseWidget::add_child` | **无** | ✅ | ✅ | ✅ |
| `crate::create_*` | `not(alloc_frugal)`（**118 处**） | ✅ | ⚠️ 多数不存在 | 需逐条核查 |
| `crate::json` | `full_widgets` | ✅ | ❌ | ❌ |
| `crate::view` | `declarative_view` | ✅ | ❌ | ❌ |
| `crate::layout` | **无** | ✅ | ✅ | ✅ |
| capability 表（`EventSchema`） | `full_widgets` | ✅ | ❌ | ⚠️ 部分（静态表） |

**这张表是设计器「兼容多 profile」的事实基础。**

### 5.2 四条硬约束（D7-d）

| # | 约束 | 理由 |
|---|---|---|
| **d-1** | 控件可用性按目标 profile **查询**，不自维护清单 | `embedded` 只支持 **18 个 kind**（`canonical_name_for_kind`），选错则生成物编译失败 |
| **d-2** | CSS 在**生成期**解析并内联为属性赋值 | 目标 profile 无样式引擎；生成物中 `grep "apply_css"` 应为 0 |
| **d-3** | 布局在**生成期**算出常量坐标 | embedded/mini 的 UI 是静态构造，**没有每帧重新求值的调用方** |
| **d-4** | 容量约束在生成期检查并**报错** | `mini` 下 `BaseWidget::children` 定容 **64**（`MiniVec`），超限会静默丢控件 |

### 5.3 d-3 的技术依据（最关键的一条）

D7-d-3 要求「设计器的布局引擎能在生成期独立运行」。**本库已满足**（实跑取证）：

1. `pub mod layout;` **无 cfg 门控**（对比：`json` 受 `full_widgets`、`view` 受 `declarative_view`）
2. 布局层**零平台依赖**（`grep -c "platform::" src/layout/*.rs` → 全部 `0`）
3. 接口是**回调式**（`src/layout/types.rs:54-60`）：

```rust
pub trait Layout {
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
}
```

**布局不需要真实控件存在**，只需 `ObjectId → Rect` 的回调。
⇒ 设计器可在生成期独立算出全部坐标。

**前提**：设计器**必须复用这套 `Layout` trait**，不得自己重写一套布局算法。
否则「设计期所见」与「运行期所得」会漂移——这正是设计器最致命的失败模式。

### 5.4 `mini` 与 `embedded` 不是同一种受限

| | `mini` | `embedded` |
|---|---|---|
| 门控 | `alloc_frugal`（`no_std`） | `embedded_surface` |
| capability 层 | ❌ 整体编译掉 | ✅ 保留，用静态表命名 |
| 控件名解析 | 不存在 | `canonical_name_for_kind`，**仅 18 kind** |

`src/widget/capability.rs:390-395` 的设计依据：

> Returns `""` for an unmatched kind, which the constructor lookup reads as
> "not available in this profile". **Inventing a name here would let the factory build a
> control the profile does not have.**

---

## 6. 取证汇总

| 断言 | 命令 | 结果 |
|---|---|---|
| `src/json` / `src/view` 受门控 | `grep -n "pub mod json\|pub mod view" src/lib.rs` | `full_widgets` / `declarative_view` |
| `src/layout` 不受门控 | 同上 | 无 `cfg` |
| 布局层零平台依赖 | `grep -c "platform::" src/layout/*.rs` | 全部 `0` |
| `create_*` 在 mini 下缺席 | `grep -c "cfg(not(alloc_frugal))" src/lib.rs` | `118` |
| `embedded` 控件集 | `canonical_name_for_kind` 匹配臂 | `18` 个 kind |
| `add_child` 始终可用 | `grep -n "pub fn add_child" -B3 src/widget/base.rs` | 无 `cfg` |
| 事件侧只有字符串 | `src/widget/capability/types.rs` | `events: &'static [&'static str]` |
| 已发布事件数 | `check_capability_events_are_emitted.sh` | 326 pairs / 186 names |
| 信号载荷种类 | `tools/list_event_payload_shapes.py` | 335 声明 / 60 种 |
| JSON 事件路径独立且无门禁 | `grep -rn "on_click" src/json/`；`tools/` 零命中 | 8 个硬编码键 |
| profile 门禁 | `check_view_platform_gate.sh` | EXIT=0 |

---

## 7. 与 `blue19.md` 的关系

| 本文档 | `blue19.md` |
|---|---|
| **是什么、为什么** | **怎么做** |
| 定位、目标、架构、约束 | 任务表（T-A~T-24）、决策项（D1~D7-d）、DoD |
| 稳定的设计意图 | 会随执行演进的计划 |

**执行时以 `blue19.md` 为准**；本文档回答「为什么这么设计」，`blue19.md` 回答「下一步做什么」。

---

## 8. 尚未确定的问题

| # | 问题 | 影响 |
|---|---|---|
| Q1 | 生成器的**输出粒度**：每个窗口一个 `.rs`，还是每控件一个？ | 影响可读性与增量编译 |
| Q2 | **生成物是否入库**（committed）？ | 入库便于 review；不入库避免漂移 |
| Q3 | **事件连线的目标**只限属性，还是也能调命令？ | 决定类型兼容表规模（`blue19.md` D6） |
| Q4 | 设计器**是否支持自定义控件**（用户自己写的 Widget）？ | 影响控件注册与代码生成 |
| Q5 | **多窗口 / 多页面**如何表达？ | 影响文档模型 |
| Q6 | 是否需要**版本迁移**（设计器升级后旧 JSON 仍可打开）？ | 影响 JSON 格式设计（须留版本字段） |

---

## 9. 明确不做

1. **不做可视化布局的「绝对自由拖拽」**——坐标必须能被 `Layout` trait 表达，
   否则 d-3 无法在生成期算死。
2. **不在嵌入式设备上跑设计器**——设计器是桌面工具。
3. **不发明新的 UI 描述语言**——用 JSON + CSS（§3.1）。
4. **不让设计器自己实现布局算法**——必须复用 `Layout` trait（§5.3）。
5. **不生成会静默丢控件的代码**——容量超限必须报错（d-4）。

---

## 10. 结论

**设计器的核心不是「画界面」，而是「把界面描述编译成可运行的本项目代码」。**

它的三个立论基础都已在本库中得到验证：

1. **结构 + 外观的分离**已有（`src/json` + `src/style/css`，单向依赖）；
2. **两套模式的公共接口**已有（`ViewEngine` 的 `create` 回调）；
3. **生成期算布局的能力**已有（`src/layout` 无门控、无平台依赖、回调式接口）。

**真正缺的只有一件事：事件侧的类型化契约**（`blue19.md` 步骤 1）。
没有它，属性面板能显示控件有哪些属性，却说不清事件带来什么值——连线无法校验，JSON 无法表达映射。

**所以 `blue19.md` 的步骤 1 是设计器的最小前置。**
