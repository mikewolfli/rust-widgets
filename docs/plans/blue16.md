# BLUE16 — 从「全自绘」到「对外可用且无死重」：三条线的闭环 + 全仓可达性收敛

> 状态：**Phase A / B / C-1 / D 已完成；Phase E-1、E-3、E-4 已完成；Phase E-2 / C-2 未执行；Phase F（B-5a/B-5b/B-5c/B-5d）已完成**
> 完成率：**Phase A 100% · Phase B 100% · Phase C-1 100% · Phase D 100% · Phase E 80%（E-1/E-3/E-4 完成，E-2 未做）· Phase F 100%（B-5a–d 全部完成）**
> 执行日志：[`docs/log/log-20260917-2.md`](log-20260917-2.md)
> 目标基线：`rust_widgets v2.1.0`（`f451a15` + 第 19–21 轮未提交改动）；执行后版本 **2.2.0**
> 原则依据：[`docs/plans/principle.md`](principle.md)（继承 BLUE1–BLUE15 全部规则，含 #1–#69）
> 上轮计划：[`docs/plans/blue15.md`](blue15.md)（全自绘重构，§G/§H 已 100%）
> 上轮日志：[`docs/log/log-20260917-1.md`](log/log-20260917-1.md)（第 19–21 轮）
>
> 本文件是执行计划，不是完成报告。每条结论必须回填「构建/测试/代码」证据。
> **取证纪律**：本文件所有数字均在 `v2.1.0` 工作树上实跑取得，附带回执命令（原则 #64）。
> 写作过程中**由子代理提出的每条关键主张都在正文中独立复跑过**——未复跑的不写入。

---

## 核心规则（继承 BLUE15 全部，含 #1–#69）

### BLUE16 新增规则

70. **📡 对外契约必须以「生成物 = 发布物」为判据** — 当某份文件是对外消费的契约（C 头文件、语言绑定的符号表、能力矩阵），它必须**由生成器产出且被门禁逐字节校验**。只校验生成器的另一个输出（`examples/`）而放任发布路径（`include/`）漂移，等于没有门禁。判定：对发布物跑一次「重新生成 → `cmp`」，失败即为缺陷。

71. **🔌 能力可达性优先于能力新增** — 当中层已有能力而对外入口缺失时，**补入口优先于加新功能**。理由与 #24「基础设施先于控件」同源：不可达的能力等价于不存在，却仍消耗维护面。判定：`grep` 对外层（C ABI / 绑定）是否能触达该能力。

72. **☠️ 零消费者模块必须「三选一」，禁止长期悬置** — 任何 `pub` 模块必须在三态之一，并有明示证据：① **有生产调用者**（给出 file:line）；② **是刻意的对外面**（进 C ABI / 绑定，且该面有门禁）；③ **显式登记为实验/保留**（模块文档写明理由 + 预估删除条件）。三态之外即为死重，按 #59 删除。判定：模块文档含 `# Reachability` 段落，且该段落所述状态可由 `grep` 复核。

73. **🧪 「能构造」不是「能工作」** — 验证一条接线必须断言**可观测的行为结果**（几何、像素、颜色、状态回读），而非「函数返回 `Ok`」或「对象构造成功」。判定：测试的断言对象是**产物**（`Rect`/`Color`/`CapabilityValue`）而不是**容器**（`Box<dyn _>` 的存在性）。

74. **📏 同名近义模块必须写清关系，且不得互为对方的上游** — 当两个模块名近义（`performance` vs `gpu::performance`、`quality` vs `render::quality`、`embedded` vs `platform::portable`）时，必须在一个方向写明关系（谁是谁的哪一层），**且文档所述依赖方向必须与代码一致**。判定：按模块文档所述方向 `grep`，得到的引用方向与之相符。

75. **🌊 事件产生者与消费者必须成对存在** — 任何事件/信号类型都必须有**真实生产者**（平台层或库内可触发路径）与**真实消费者**。只有消费者没有生产者 = 一批永远收不到输入的识别器/信号，比死代码更危险：它看起来在工作，且测试能过（测试自造事件）。判定：对每个 `Event`/`Signal` 变体 `grep` 其**构造点**（`Event::X {` 且非 `=>`），若仅在 `#[cfg(test)]` 或文档中出现，即为无生产者。

76. **🔁 多态数据必须归一化跨界量** — 当连续量在周期空间（`atan2`、角度、时间戳）中做差分时，必须做**环绕归一化**与**单位/基准校验**，不得直接相减。判定：每一处 `a - b` 的差值，若 `a`/`b` 来自周期性函数，必须能指出归一化代码。

77. **✅ 反向断言与正向断言同等必要** — 当一条不变式是「集合 A 与集合 B 一致」时，必须**双向**断言。只检查 `A ⊆ B` 会放过 `B` 中无人实现的元素。判定：这类测试必须同时报出「A 有 B 无」与「B 有 A 无」两个列表。

---

## 一、目标与边界

### 1.1 本轮的判定基线

用户目标：**三条线补成最优，并整体达到「最优、统一、精炼、高效」的 GUI**。

「最优」在本项目中的可操作定义（本轮采用，替代空泛的「完美」）：

| 维度 | 可度量判据 |
|---|---|
| **可用（对外）** | 对外契约（C ABI / 绑定）能触达核心能力；发布物与生成物一致 |
| **可达（对内）** | 每个 `pub` 模块属于规则 #72 的三态之一，且状态可 `grep` 复核 |
| **统一** | 同语义类型/枚举只有一份（#54）；近义模块关系写清且方向正确（#74） |
| **精炼** | 无死重；删除必须伴随引用计数归零证据（#61） |
| **高效** | 关键路径有基准与门槛；无零消费者的热路径开销（#22/#24 的延伸） |

### 1.2 三条线（用户指定）

| 线 | 现状 | 本轮目标 |
|---|---|---|
| **JSON GUI** | 4,026 行，零生产调用者 | 按规则 #72 判定其状态并处置（§四 Phase C） |
| **CSS 解析** | 仅经 JSON 路径可达；C ABI 零覆盖 | 补对外入口（§四 Phase B）+ 补齐表达力（已部分完成） |
| **Theme 主题** | 已接通 C ABI 与窗口 API 两条主漏斗 | 补「可读可设」入口（§四 Phase B） |

### 1.3 明确不动（本轮非目标）

- 不改 `WidgetKind` 变体集合（#22/#23 已稳定）。
- 不改 C ABI 函数名与签名语义（`rw_create_*` 保持，属向前兼容 #21）。
- 不重写渲染引擎（`render` / 软件后端保持）。
- 不引入新 UI 框架依赖。
- **不做规范级完整**：CSS 不加 `@media`/`var()`/`calc()`，JSON 不做 QML 级表达式/绑定/状态机（见 §六 判例）。

---

## 二、现状取证（Step 0，已完成）

> 所有数字均为实跑，附回执命令。**每条关键主张均在本节标注是否已由本人独立复跑**。

### 2.1 三条线的真实可达性

```bash
$ grep -rn "apply_active_theme" src/ | grep -v "fn apply_active_theme" | grep -v test
src/lib.rs:656                                    ← C ABI + 窗口 API 漏斗
src/control_backend/custom/mod.rs:140             ← app API 漏斗
```
**结论：Theme 已闭环（两条主漏斗均接上）。** ✅ 已复跑

```bash
$ grep -rn "apply_css\|global_stylesheet_manager()" src/ | grep -v "^src/style/" | grep -v test
src/json/loader.rs:1023
src/json/loader.rs:1037
```
**结论：CSS 仅经 JSON 路径可达，而 JSON 无生产调用者。** ✅ 已复跑

```bash
$ grep -rn "crate::json" src/ --include=*.rs | grep -v "^src/json/" | grep -v test
src/lib.rs:104                                    ← 仅文档注释
```
**结论：JSON 零生产调用者。** ✅ 已复跑

```bash
$ grep -rn "crate::app\b" src/ | grep -v "^src/app/" | grep -v test
src/json/loader.rs:28
src/json/element.rs:23
```
**结论：`json` 无生产调用者（消费者仅为本仓的测试与基准）；`app` 反而有真实外部消费者。**

> **方法教训**：初稿用 `grep src/` 得出「`app` 与 `json` 构成 7,375 行闭环死链」。
> 纳入 `examples/`、`tests/`、`demo/`（独立 crate）后，`app` 有 **13 处**消费者 —— **该结论被推翻**。
> `json` 仍无真实使用者，但它的「零消费者」是**有回归测试但无应用**，与 `audio` 那种「连测试都没有」不同类。
> 这条更正直接改变了 §6.1 的选项权重（`app` 不随 `json` 存亡）。

### 2.2 C ABI 覆盖缺口（对外契约）

```bash
$ grep -o "rw_create_[a-z_0-9]*" src/bindings/binding_impl.rs | sort -u | wc -l
23                                                ← 167 个 kind 中可创建 23 个
$ grep -c "read_widget_property_by_id\|write_widget_property_by_id" src/bindings/binding_impl.rs
0                                                 ← 通用属性层完全未暴露
$ grep -c "add_widget\|set_layout" src/bindings/binding_impl.rs
0                                                 ← 无布局管理
$ grep -c "set_scroll\|scroll_to" src/bindings/binding_impl.rs
0                                                 ← 无滚动控制
$ grep -in "tooltip" src/bindings/binding_impl.rs
(空)                                              ← 连 base 属性都不可达
```
✅ 全部已复跑

**后果（可描述、非推测）**：一个 C/Python/Java 调用方**无法**设置任何控件的外观，无法用布局管理器，无法滚动，无法给 `list_view`/`table`/`tree` 加一项，无法设 tooltip。

### 2.3 发布物与生成物不一致（规则 #70 的实例）

```bash
$ grep -c "^[a-z].*(.*);" include/rw_generated.h
102
$ grep -c "^[a-z].*(.*);" examples/rust_widgets.generated.h
106
$ grep -c "rw_destroy_widget" include/rw_generated.h
0
$ grep -c "rw_destroy_widget" examples/rust_widgets.generated.h
1
$ grep -n "rw_generated.h" README.md README.zh-CN.md
README.md:337:      ... the C ABI (`include/rw_generated.h`, 100 `rw_*` functions) ...
README.zh-CN.md:324: ... C ABI（`include/rw_generated.h`，100 个 `rw_*` 函数）...
```
✅ 已复跑

**两个真缺陷**：
1. **`include/rw_generated.h` 缺 4 个函数**，含 `rw_destroy_widget` —— **唯一的析构入口**。外部消费者不能销毁控件。
2. `check_abi.sh` 只校验 `examples/rust_widgets.generated.h`，**发布路径 `include/` 零门禁**。两个头文件互不一致却都"通过"。
3. README 写的「100 个函数」也与实际 106 个不符（我上一轮引入的第二个错误）。

### 2.4 零消费者模块（规则 #72 的对象）

> **取证口径更正（写作过程中就地修正，保留记录）**：
> 初稿只 `grep src/`，得出「`app` 是闭环死链」——**该结论是错的**。
> `app` 被 `examples/`、`tests/`、`demo/`（均为**独立 crate**）使用，在 `src/` 作用域内看不见。
> 正确口径必须包含**所有消费方**。下表为更正后的复跑结果。

```bash
$ # 正确口径：包含 examples/ tests/ benches/ demo/*/src/ 与 src/
$ for m in json app web pdf audio video print data_binding menu_config performance asset embedded index; do
    grep -rl "rust_widgets::$m\b\|crate::$m\b" examples/ tests/ benches/ demo/*/src/ src/ | grep -v "^src/$m/" | wc -l
  done
app           13   ← ✅ **有真实消费者**（examples × 3、tests × 4、demo）
json           4   ← 仅 tests/ + benches/（**无 examples/demo**）
web            2   ← 待逐符号确认（去重后可能为 0）
index          2   ← 经 tests/integration_test.rs
performance    1   ← 待确认是否仅文档
asset          1   ← 待确认是否仅文档
pdf            0
audio          0
video          0
print          0
data_binding   0
menu_config    0
embedded       0
```

**关键区分（决定处置方式）：**

| 状态 | 模块 | 含义 |
|---|---|---|
| **有外部消费者（examples/tests/demo）** | `app` | 是**真实公开 API**。`lib.rs:160 pub mod app` 且在 `examples/control_property_uniform.rs:46` 等被 import。**不删，且应确认其文档声称（「primary entry-point」）与实际相符** |
| **仅测试/基准引用** | `json` | `tests/integration_test.rs` + `benches/json_bench.rs`。消费者是**本仓自己的测试**，不是应用 —— 属「无真实使用者」但有回归价值 |
| **零引用（连测试都没有）** | `audio` / `video` / `print` / `data_binding` / `menu_config` / `embedded` / `pdf` | 真正的零消费者 |

**各模块行数（实跑）：**

| 模块 | 行数 | 对外可达？ |
|---|---:|---|
| `web` | 6,119 | ⚠️ 部分（约 5,100 行待逐符号复跑） |
| `pdf` | 4,616 | ❌ |
| `json` | 4,026 | ⚠️ 仅本仓测试/基准作为消费者 |
| `audio` | 2,464 | ❌ |
| `print` | 2,172 | ❌ |
| `embedded` | 1,857 | ❌（转发层，已复跑 §2.6） |
| `video` | 1,821 | ❌ |
| `performance` | 1,231 | ❌（仅 1 处文档引用） |
| `data_binding` | 1,083 | ❌ |
| `menu_config` | 862 | ❌ |
| `asset` | 316 | ❌（1 处引用待确认是否仅文档） |
| `index` | 218 | ⚠️ 随 `json` 判定 |
| **合计（不含 `app`）** | **≈ 30,800** | —— |

> `app`（3,349 行）**已确认有真实消费者，不计入本表**。初稿把 `app` 计入并得出 ≈34,000，已更正。

### 2.5 与 BLUE15 §七「BLUE16 候选」的对照

BLUE15 登记的 6 项，本轮逐条判定（§五）：

| BLUE15 登记项 | BLUE16 判定 |
|---|---|
| 保留 `ControlRoutePreference` 枚举 | **保留**（复审后同意，见 §五.1） |
| 保留 `control_backend/native.rs` | **保留**（同上） |
| `macos-legacy` 下线 | **不做**（需 macOS 宿主验证，本机不可验） |
| `webkit-engine` 去留 | **不做**（产品决策） |
| 无障碍树生成 | **不做**（独立设计，R4） |
| `widget/gesture` 与 `embedded/input.rs` 合并 | **改判**：`src/embedded` 整体零消费者（§2.4），合并议题**消失**，转为删除议题 |

### 2.6 `src/embedded` 的精确状态（**写作过程中就地更正**）

§2.4 把 `src/embedded` 整体归为「零引用」，**该判定过粗**。逐符号复跑后真实情况是：

```bash
$ grep -n "pub fn\|pub const fn" src/embedded/flags.rs
is_embedded_mode / set_embedded_mode / is_low_memory_mode / set_low_memory_mode
recommended_buffer_size / max_texture_size / font_cache_size / event_queue_size / max_widgets
init_embedded / init_desktop

$ # 每个符号的外部调用者（排除自身、platform/profile、及 config.rs 的文档注释）
$ for fn in recommended_buffer_size max_texture_size font_cache_size event_queue_size max_widgets; do ...
recommended_buffer_size   1   ← 仅 src/embedded/config.rs:14 的**文档注释**
max_texture_size          0
font_cache_size           0
event_queue_size          0
max_widgets               0
```

**关键区别（影响处置方式）：**

| 事实 | 含义 |
|---|---|
| `flags.rs` **确实读取**活的 `platform::profile::surface_policy()`（`flags.rs:20,60,70,77,79,85,93`） | 它不是「错代码」—— BLUE15 §E 的收敛（策略表唯一）是真的 |
| 但它自己的 **5 个预算函数零调用者** | BLUE15 §E 写「`embedded/flags.rs` 四个预算改读它」描述的是**它读什么**，**不是**有人用它的输出 |
| `platform/profile.rs` 才是被消费的那一层（`profile.rs:322,559,621,636`） | **真相源是 `profile`，`embedded/flags` 是一个未被消费的转发层** |

**因此 `src/embedded` 的处置不是「删一个有价值的模块」而是「删一个转发层」**：
它的功能已被 `platform::profile::surface_policy()` 完整覆盖，且该函数有真实消费者。

> **方法论记录**：这正是我在§十.6 里登记的弱点——「§2.4 的可达性判定仅复跑了 9 个模块」。
> 就地跑完后发现 `embedded` 的判定需要修正。**保留此记录**（原则 #64）。

---

## 三、目标架构

### 3.1 三条线的目标形态

```text
                     ┌─────────────────────────────────────────┐
  外部消费者          │  C ABI（发布物 = 生成物，门禁校验）        │
  (C/C++/Py/Java)    │   create_by_kind · get/set_property      │
                     │   add_item · set_layout · scroll · style │
                     └────────────────┬────────────────────────┘
                                      │ 全部经同一漏斗
                     ┌────────────────▼────────────────────────┐
  内部上层            │  widget::runtime（唯一注册点）            │
                     │    ↑ apply_active_theme（主题）           │
                     │    ↑ apply_declared_styles（CSS）         │
                     └────────────────┬────────────────────────┘
                                      │
                     ┌────────────────▼────────────────────────┐
  中间层              │  WidgetFactory（167 kind 构造器）          │
                     │  WidgetProperties（253 属性名，逐控件自述） │
                     └─────────────────────────────────────────┘
```

**关键性质**：C ABI 不再为每个 kind 手写构造器，而是**转发到已存在的通用层**（`create_widget_of_kind` + 属性契约）。这样 167 个 kind 与 253 个属性名**一次接通**，且未来新增 kind 自动可达（原则 #71）。

### 3.2 可达性三态（规则 #72 的执行形态）

每个 `pub` 模块的模块文档新增：

```rust
//! # Reachability
//!
//! **State:** Exposed over the C ABI (`rw_*`), gated by `tools/check_abi.sh`.
//! 或
//! **State:** Production callers: `src/widget/runtime.rs:412`, `src/app/handle.rs:88`.
//! 或
//! **State:** Reserved experiment, no consumer. Rationale: <理由>.
//! Removal condition: <何时删>.
```

新增门禁 `tools/check_module_reachability.sh`：解析每个模块的 `# Reachability` 段落，
- 声称「Production callers」→ 该段落里的 file:line 必须存在且确实引用该模块；
- 声称「Exposed over the C ABI」→ 必须在 `binding_impl.rs` 找到对应的 `rw_*`；
- 声称「Reserved」→ 必须给出非空理由。
**无段落即为失败**（强制表态，禁止悬置）。

---

## 四、改造方案（分步、可回退）

> 排序依据原则 #3（先修功能阻断项）：Phase A 修**发布物不一致**（阻断外部消费者），
> Phase B 修**对外不可达**（阻断能力），Phase C 做**死重收敛**（精炼），Phase D 收尾。

### Phase A — 发布物一致性（阻断级，最小改动）

**A-1 `include/rw_generated.h` 重新生成（规则 #70）**

```bash
$ python3 tools/generate_c_header.py --output include/rw_generated.h
$ diff include/rw_generated.h examples/rust_widgets.generated.h   # 期望：无输出
```

**A-2 `check_abi.sh` 扩展为「双头文件 + 发布路径」校验**

- `cmp` 两个头文件（都必须等于生成器输出）；
- 断言必选符号清单**从 `examples/` 改为 `include/`**（发布物才是消费者用的）；
- 新增「README 声明的函数数 == 实际数」断言（消除 §2.3 的第 3 项）。

**A-3 语言绑定补 `rw_destroy_widget`**

Python / Node.js / C++ 三个绑定各缺此函数（经 §2.1 复跑确认为同一缺口）。补上，
并新增门禁 `check_binding_symbol_coverage.sh`：绑定的 `rw_*` 符号集必须等于
`binding_impl.rs` 的导出集（当前 105/106）。

**验收**：`check_abi.sh` 与 `check_binding_symbol_coverage.sh` 双 PASS；
`cmp` 两个头文件无差异；README 数字与实跑一致。

---

### Phase B — 对外能力可达（三条线的核心）

**B-1 通用构造入口（一次接通 167 个 kind）**

新增 C ABI（**一对函数，不是 144 个**）：

```c
/// Creates a control of any registered kind. `kind_name` is the canonical
/// factory name (`"button"`, `"tree_view"`, …); see `rw_widget_kind_names`.
uint64_t rw_create_widget_of_kind(uint64_t parent, const char* kind_name,
                                  const char* text, int x, int y,
                                  unsigned int w, unsigned int h);

/// Writes every registered kind name into `out`, space-separated. Returns the
/// number of bytes written, or the required size when `out` is NULL.
unsigned int rw_widget_kind_names(char* out, unsigned int cap);
```

- 转发到 `crate::create_widget_of_kind`（`src/lib.rs:702`，已存在）；
- `kind_name` 经 `WidgetFactory` 解析（`normalize_key` 已容忍大小写与分隔符）；
- **不暴露 `WidgetKind` 的整数判别值**：`WidgetKind` 无 `#[repr]` 保证（§2.2 子审计），
  暴露枚举编号会把内部布局固化成 ABI。字符串名字是稳定契约。
- 保留现有 23 个 `rw_create_*`（原则 #21 向前兼容），实现改为转发到同一入口。

**B-2 通用属性层（一次接通 253 个属性名）**

```c
/// Value kinds for the property ABI.
typedef enum {
    RW_VALUE_NULL = 0, RW_VALUE_BOOL = 1, RW_VALUE_INT = 2,
    RW_VALUE_UINT = 3, RW_VALUE_FLOAT = 4, RW_VALUE_STRING = 5,
} rw_value_kind;

/// Reads a property by name. On success writes the kind and (for STRING) a
/// heap string the caller frees with `rw_free_string`.
bool rw_get_widget_property(uint64_t widget_id, const char* name,
                            int* out_kind, int64_t* out_num, char** out_str);

/// Writes a property by name. `kind` selects which of `num`/`str` is read.
bool rw_set_widget_property(uint64_t widget_id, const char* name,
                            int kind, int64_t num, const char* str);

/// Lists the property names `widget_id` publishes, space-separated.
unsigned int rw_widget_property_names(uint64_t widget_id, char* out, unsigned int cap);
```

- 转发到 `read_widget_property_by_id` / `write_widget_property_by_id`
  （`src/widget/capability/access.rs:61,76`，已存在）；
- 失败经既有 `rw_error_code` / `rw_error_message` 报告（`access.rs` 的错误枚举
  已能区分 `UnknownWidget`/`UnknownProperty`/`ReadOnlyProperty`/`TypeMismatch`/
  `UnsupportedOnWidget`）；
- **这使 `tooltip`、`checked`、`value`、`min`/`max`、`items` 计数、选择模式等一并可达**，
  无需为每个属性加一个函数。

**B-3 集合与布局（补三类真实缺口）**

| 缺口 | 新增函数 | 转发目标 |
|---|---|---|
| 列表/表格无法填充 | `rw_widget_list_add(widget_id, text)` / `rw_widget_list_clear(widget_id)` / `rw_widget_list_count(widget_id)` | 经属性契约的 `items`/`item_count`；对 `list_box` 走既有 `add_item` |
| 无布局管理 | `rw_set_widget_layout(parent, kind_name, spec_json_or_kv)` → **简化为** `rw_widget_set_layout(parent, kind, spacing, margin)` + `rw_widget_layout_add(parent, child, stretch)` | `crate::layout::*`（14 种已实现）+ `json::layout::parse_layout_kind` 的解析逻辑**上移**到通用层 |
| 无滚动控制 | `rw_widget_set_scroll_position(widget_id, x, y)` / `rw_widget_scroll_to(widget_id, where_)` | `scrollarea::set_scroll_position` / `scroll_to_*`（`src/widget/container_widgets/scrollarea.rs:220,279,284,296,301`） |

> **B-3 的关键决定**：布局参数的解析逻辑目前在 `src/json/layout.rs`（死链模块）。
> 本轮把它**上移**到 `src/layout/declarative.rs`（新的通用层），`json` 侧改为调用它。
> 这样 C ABI 与 JSON 共用一份解析，且该逻辑不再依赖死链模块（原则 #24：基础设施先于控件）。

**B-4 样式与主题入口**

```c
/// Applies a style property by name to a widget. Same value encoding as
/// `rw_set_widget_property`, but routed through the style pipeline so a
/// stylesheet-applied value and a direct set are distinguishable.
bool rw_widget_set_style(uint64_t widget_id, const char* name, int kind, int64_t num, const char* str);

/// Selects the active theme by name. Returns false if no such theme is registered.
bool rw_set_theme(const char* name);

/// Names of registered themes, space-separated.
unsigned int rw_theme_names(char* out, unsigned int cap);

/// Sets the high-contrast override (0 = none).
void rw_set_high_contrast(int mode);
```

- `rw_widget_set_style` 复用 B-2 的编码；作用于 `WidgetStyle` 字段
  （`background-color`/`border-radius`/`font-size`/`opacity`/`shadow` 等已由 `CssParser::apply_one` 覆盖，
  直接调用 `CssParser::apply_declarations` 即可复用同一套解析，避免第二份实现，原则 #54）；
- `rw_set_theme` / `rw_theme_names` / `rw_set_high_contrast` 转发既有
  `global_theme_manager()`（已闭环）；
- **不暴露 `rw_register_stylesheet`（CSS 文本）**：理由见 §六.2 —— 跨 FFI 传 CSS 需要
  同时暴露 `class`/`id` 概念，收益低而抽象负担重。样式表仍是 Rust 侧能力，逐属性是 FFI 侧路径。

**验收（规则 #73：断言产物）**：
- `rw_create_widget_of_kind("tree_view")` 返回非 0，且 `rw_widget_property_names` 含 `item_count`；
- `rw_set_widget_property(id, "tooltip", ...)` 后 `rw_get_widget_property` 回读到同一值；
- `rw_set_widget_style(id, "background-color", "#FF0000")` 后经属性层回读为 `#FF0000FF`；
- `rw_widget_set_layout` + `rw_widget_layout_add` 后子控件几何**真的被布局**（断言 `Rect`，
  不是断言返回 `true`）。

---

### Phase C — 死重收敛（可达性三态）

**C-1 先分类，再处置（规则 #72）**

对 §2.4 的 13 个模块逐一判定，写入模块文档的 `# Reachability` 段落。判定矩阵：

| 模块 | 行数 | 初步判定 | 需 `grep` 确认的点 |
|---|---:|---|---|
| `json` | 4,026 | ⚠️ **待定** —— 见 §6.1 | 是否保留声明式（消费者仅为本仓测试/基准） |
| `app` | 3,349 | ✅ **已确认为真实 API，不删** | 已复跑：examples/tests/demo 共 13 处消费者（§2.4）|
| `web` | 6,119 | ⚠️ **部分可达（待逐符号复跑）** | 已确认仅 `BoaJsEngine`/`WebEngineViewEnhanced` 被 widget 用；其余 5,100 行待逐符号判定 |
| `pdf` | 4,616 | ⚠️ **待定** | 零引用（含 examples/tests）；是否有对外需求 |
| `print` | 2,172 | ⚠️ **待定** | 零引用；但打印是 UI 库常规能力且 README 声称支持（→ 要么接通要么删声明）|
| `audio` | 2,464 | ⚠️ **待定** | 零引用（含 examples/tests）|
| `video` | 1,821 | ⚠️ **待定** | 同 `audio` |
| `embedded` | 1,857 | ❌ **应删（转发层）** | 已复跑（§2.6） |
| `performance` | 1,231 | ❌ **应删** | 已复跑：`crate::performance` 仅 1 处文档引用；功能已被 `gpu::performance`（live）覆盖 |
| `data_binding` | 1,083 | ❌ **应删** | 零引用 + `#[macro_export]` 宏无人调用 |
| `menu_config` | 862 | ❌ **应删** | 1 处文档引用，功能已被 `menu_toolbar` 覆盖？需取证 |
| `asset` | 316 | ❌ **应删** | 仅测试引用；`CssWatcher` 已自带轮询（第 20 轮） |
| `index` | 218 | ⚠️ **随 `json` 判定** | 仅经死链可达 |

**C-2 删除纪律（规则 #61 + #72）**

对每个判定为「删」的模块：
1. `grep` 引用计数**归零**（含 `tests/`、`examples/`、`benches/`、`demo/`、`bindings/`）；
2. **逐个符号**取证（不是整模块 grep 一次）：导出符号、`Cargo.toml` feature、文档引用；
3. 删除其自带测试 —— 并**在日志登记删除的测试数**（BLUE15 第 7 轮已有先例：删死代码同时删 28 个测试）；
4. 清理 `Cargo.toml` 的对应 feature（用 `cargo tree -i` 反查依赖，规则 #63）。

**C-3 近义模块关系（规则 #74）**

详细取证与处置见 §十.4（已逐对复跑）。三项共性要求：

1. 文档所述依赖方向必须与代码一致；
2. 两个模块职责重叠时，保留有生产调用者的那个；
3. 保留方必须在模块文档写明「我是唯一入口」。

**验收**：`tools/check_module_reachability.sh` PASS（每个 `pub` 模块有 `# Reachability` 且状态可复核）；
`cargo tree -i` 确认无孤立依赖；净行数为负并记录。

---

**四个新门禁必须先验证它们能失败**（原则 #19）：手工删掉 `include/` 的一个函数、
注入一个无 `# Reachability` 的模块、在绑定里删一个符号、写一个语法错的 C 片段，
确认四个门禁各自报 FAIL 并记录。

### Phase D — 收尾与文档同步

**D-1 文档与代码一致（原则 #18）**
- README / README.zh-CN：修正函数数（§2.3 的第 3 项）、补 C ABI 能力表、补 Node.js 绑定行；
- cookbook 三语：新增 B-1/B-2 的 C 示例（**必须实际编译**——`check_cookbook.sh` 只校验声明名与 `mdbook build`，不编译散文代码块，故新增 `tools/check_c_code_examples.sh` 用 `cc -fsyntax-only` 编译 C 片段）；
- `docs/ARCHITECTURE.md`：补可达性三态说明。

**D-2 能力矩阵同步**
- `docs/plans/platform_capability_matrix.md` 与 `tools/generate_platform_capability_matrix.py`：
  新增「C ABI 覆盖」列，由 `binding_impl.rs` 派生（禁止手维，规则 #18/#64）。

**D-3 版本与发布物**
- 版本 `2.1.0` → `2.2.0`（新增 C ABI 函数属向后兼容的 minor）；
- `include/rw_generated.h` 与 `examples/rust_widgets.generated.h` 同步；
- CHANGELOG 记录新增 ABI 与删除的模块。

---

## 五、BLUE15 §七「BLUE16 候选」的逐条判定

| # | BLUE15 登记项 | 本轮判定 | 理由 |
|---|---|---|---|
| 1 | 保留 `ControlRoutePreference` 枚举 | ✅ **确认保留** | 复审同意：恒为单值的策略点比删除后重加更能表达「机制唯一」。且 BLUE15 §B-4 的论证（策略点 vs 删除）在本轮无新证据推翻。 |
| 2 | 保留 `control_backend/native.rs` | ✅ **确认保留** | 同上；其方法体已是自绘，是「默认自绘后端」的具名实现。 |
| 3 | `macos-legacy`（cocoa/objc）下线 | ⬜ **不做** | 需 macOS 宿主验证，本机（Linux）不可验证。原则 #16：不冒充完成。 |
| 4 | `webkit-engine` 去留 | ⬜ **不做** | 是产品决策（是否仍映射系统 WebKit）。但见 §六.3：本轮**登记**该决策的判据。 |
| 5 | 无障碍树生成 | ⬜ **不做** | 需独立设计（R4）。本轮只做 B-2 使无障碍属性可达（`accessibility_name` 已有 C 入口）。 |
| 6 | `widget/gesture` 与 `embedded/input.rs` 合并 | 🔄 **改判** | §2.4 取证：`src/embedded` **零消费者**，故「合并」议题不复存在，转为**删除**议题（Phase C-1）。这正是原则 #51 的正面用例：先取证重复，取证结果是「其中一个整体是死的」。 |

---

## 六、明确不做 / 判例登记（含理由与触发条件）

### 6.1 JSON GUI 是否保留（**本轮最重要的产品决策**）

**取证**：4,026 行。消费者是 `tests/integration_test.rs` 与 `benches/json_bench.rs` —— **本仓自己的测试与基准，无应用**。
（注：`app`（3,349 行）**不在这个议题里**：它有 examples/tests/demo 共 13 处真实消费者，不随 `json` 存亡。见 §2.4 更正。）

**三种处置，需用户选择**：

| 选项 | 代价 | 适用条件 |
|---|---|---|
| **① 保留并接通** | 中（已完整：167/167 kind、10 种布局、属性走契约、CSS 集成） | 仅缺一个真实使用者 |
| **② 保留但登记为实验** | 零成本，仅写 `# Reachability` 段落 | 短期内无消费者，但不想删 |
| **③ 删除** | 删 4,026 行；需同时删其测试与基准（并**登记删除的测试数**） | 定位为「命令式 widget 库」，且不打算做声明式 |

**本轮建议 ②**，理由：JSON 已完整（167/167 kind 可构造、10 种布局、属性走契约、CSS 集成），删除是**不可逆的产品收窄**；而 ① 需要设计事件/绑定/复用（等于做轻量 QML），代价远超本轮。② 的成本只是两段文档，且条件成熟时可转 ① 或 ③。

**触发条件（转 ① 的判据）**：出现第一个真实的 JSON 布局消费者，或明确决定支持「UI 与逻辑分离」的产品形态。

### 6.2 CSS 不经 C ABI 暴露样式表（只暴露逐属性）

**理由**：CSS 的价值在**级联与选择器**，而这要求应用侧组织 `class`/`id`。跨 FFI 传这层抽象，
要么让绑定语言维护一套字符串标识（负担重），要么收窄到「单规则无选择器」（等于逐属性但语法更绕）。
逐属性 setter 与现有 100 个函数风格一致，且**不需要改动任何设计**。
**触发条件**：若绑定语言侧出现「大批量主题化」的真实需求，再评估 `rw_register_stylesheet`。

### 6.3 `webkit-engine` 去留的判据（登记，不决策）

判据：① `WebEngineView` 是否要求系统 WebKit 的**真实网页兼容性**（vs `BoaJsEngine` 的受限能力）；
② 是否接受捆绑/依赖系统 WebKit。二者都是产品输入，非技术约束。本轮只登记，等产品决定。

### 6.4 不做规范级完整（对齐 Qt 判例，延续第 19–21 轮结论）

| 不做 | 判例依据 |
|---|---|
| `@media` / `@supports` / `@keyframes` | Qt 的 QSS 刻意无此；widget 树无 viewport 概念 |
| `var()` / `calc()` | 需表达式求值器；Qt 让 C++ 侧用 `QPalette` 表达 |
| `!important` | Qt 明确反对（破坏可预测性） |
| 文档流（`display`/float/inline） | 本库是 widget 树 + 自绘，**无文档流** |
| QML 级表达式/绑定/Repeater/状态机 | Qt 为此**另建运行时**（JS 引擎 + 场景图 + 新语言 + 工具链），是定位变更而非功能补全 |

### 6.5 不做：图标数据表重构、图标字体

延续第 19–21 轮判定（零真实使用者 / 与「无捆绑二进制资源」定位冲突），不变。

---

## 七、验证矩阵（每阶段必须全绿）

| # | 命令 | 期望 |
|---|---|---|
| V1 | `cargo check --no-default-features --features desktop` | Finished |
| V2 | `cargo check --no-default-features --features embedded` | Finished |
| V3 | `cargo check --no-default-features --features mini` | Finished |
| V4 | `cargo check --no-default-features --features tablet` / `mobile` | Finished |
| V5 | `cargo check --no-default-features --features desktop --all-targets` | Finished |
| V6 | `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` | 0 warning |
| V7 | `cargo test --no-default-features --features desktop --lib -q` | 0 failed（基线 **4218**） |
| V8 | `cargo test --no-default-features --features embedded --lib -q` | 0 failed（基线 **1534**） |
| V9 | `cargo test --no-default-features --features mini --lib -q` | 0 failed（基线 **1455**） |
| V10 | `cargo test --no-default-features --features desktop --doc -q` | 0 failed（基线 **35**） |
| V11 | 既有 17 个门禁 | 全 PASS |
| V12 | **新增 `check_abi.sh` 双头文件校验** | PASS |
| V13 | **新增 `check_binding_symbol_coverage.sh`** | PASS |
| V14 | **新增 `check_module_reachability.sh`** | PASS |
| V15 | **新增 `check_c_code_examples.sh`** | PASS |
| V16 | `cc -fsyntax-only` 编译 cookbook 的每个 C 片段 | 0 error |
| V17 | 净行数变化（`find src -name '*.rs' | xargs wc -l`） | 记录，Phase C 后应显著为负 |
| V18 | **新增 `check_widget_registration_fidelity.sh`**（§12.3 E-3）| PASS |
| V19 | **工厂 canonical_name 数**（§12.4）| **161**（当前 155）| 
| V20 | **`WidgetKind` 变体数**（§12.4）| **171**（当前 167）| 

**反例注入纪律（原则 #19）**：V12–V15 与 **V18** 五个新门禁，每个都必须**先用一次人为回归验证它能失败**
（如手工删掉 `include/` 的一个函数、注入一个无 `# Reachability` 的模块、**注释掉一条 `self.register(...)`**），
并在日志记录该验证。

### 执行结果（2026-09-17，见 [`log-20260917-2.md`](log-20260917-2.md)）

| # | 实测结果 |
|---|---|
| V1–V5 | ✅ 五个 profile 全 Finished（`desktop` / `embedded` / `mini` / `tablet` / `mobile` + `--all-targets`）|
| V6 | ✅ 0 warning（`-D warnings`）|
| V7 | ✅ **4232** failed 0（基线 4218，**+14**）|
| V8 | ✅ 1534 failed 0 |
| V9 | ✅ 1455 failed 0 |
| V10 | ✅ 35 passed / 9 ignored |
| V11 | ✅ 既有 17 个门禁全 PASS |
| V12 | ✅ `check_abi.sh` 已扩为 6 步（双头文件 + 发布路径 + README 计数）|
| V13 | ✅ `check_binding_symbol_coverage.sh`（含 `#include` 跟随，方能正确判定 C++ 头）|
| V14 | ✅ `check_module_reachability.sh`：**41/41** 个 `pub mod` 全部表态 |
| V15 | ⬜ `check_c_code_examples.sh` 未新建；改用 `cc -fsyntax-only` 直接验证两个头文件同时可用 |
| V16 | ✅ cookbook 的 C/C++ 头可直接编译（并因此修掉 2 个头文件真实缺陷）|
| V17 | ℹ️ `src/` 共 **245,983** 行（净增，因 Phases A/B/E-1 均为增量；C-2 未执行）|
| V18 | ✅ `check_widget_registration_fidelity.sh` 且已反向注入验证 |
| V19 | ✅ 工厂 canonical 名 **162**（目标 161，多出的 `grid_table` 由门禁发现）|
| V20 | ⬜ `WidgetKind` 仍为 **167**（E-2 未执行，故 +4 未发生）|

---

## 八、工作量与风险

### 8.1 工作量估算

| Phase | 内容 | 估行数 | 风险 |
|---|---|---:|---|
| A | 头文件重生成 + 门禁扩展 + 绑定补函数 | ~120 | 低（机械） |
| B-1 | 通用构造入口 | ~180 | 中（ABI 设计：字符串 name vs 枚举编号） |
| B-2 | 通用属性层 | ~320 | 中（值与错误编码） |
| B-3 | 集合/布局/滚动 | ~420 | 中（布局解析上移需保持行为） |
| B-4 | 样式与主题入口 | ~260 | 低（复用 `CssParser`） |
| C | 死重收敛 | **−20,000 ~ −30,000** | **高**（不可逆；需用户确认 §6.1） |
| **E-1** | **接通 6 个已有控件**（§12.3）| **~150** | **低**（与 `breadcrumb` 同构，无新逻辑）|
| **E-2** | **新建 4 个控件**（§12.3）| **~900** | **中**（需 `draw_bridge` 分派 + 交互测试）|
| **E-3/E-4** | **注册保真门禁 + 双向断言** | **~180** | **中**（后缀白名单需迭代收敛）|
| D | 文档与发布物 | ~300 | 低 |
| 门禁 | 4 个新脚本 | ~380 | 低 |

### 8.2 风险登记

| 风险 | 等级 | 缓解 |
|---|---|---|
| Phase C 删除不可逆，可能删掉有意保留的公开 API | 🔴 **高** | §6.1 决策前置；每个模块逐个符号取证；先在日志登记待删清单供复核 |
| `include/` 与 `examples/` 双头文件长期共存导致再次漂移 | 🟡 中 | A-2 的双向 `cmp` 门禁；长期应统一为一个发布路径 |
| 通用属性层的值编码成为新 ABI 包袱 | 🟡 中 | 用 `kind` 判别 + 独立 `out_num`/`out_str`（不塞联合体）；`STRING` 经既有 `rw_free_string` |
| 布局解析上移改变现有 JSON 行为 | 🟡 中 | 上移后 JSON 侧改为委托，跑既有 36 条 `json::layout` 测试作回归 |
| B-2 使 `geometry` 可写（原设计为只读） | 🟢 低 | 属性层的 `geometry` 是 `ReadOnlyProperty`（`properties_trait.rs:105`），保持；几何仍走 `rw_set_widget_geometry` |

### 8.3 建议的落地顺序（每步独立可验收）

```
Step 1  Phase A               发布物一致性（阻断外部消费者，改动最小）
Step 2  Phase E-1             接通 6 个已有控件（零新逻辑，立即扩大 JSON/CSS 可达面）
Step 3  Phase B-1/B-2         通用构造 + 通用属性（一次接通 167 kind / 253 属性）
Step 4  Phase B-3/B-4         集合/布局/滚动 + 样式/主题入口
Step 5  Phase C-1             可达性三态分类（只加文档，不删代码）
Step 6  §6.1 决策             用户确认 JSON/app 去留
Step 7  Phase C-2/C-3         按决策执行删除 + 近义模块关系
Step 8  Phase E-2             新建 4 个控件（§12.2 判定为“做”的四项）
Step 9  Phase E-3/E-4         注册保真门禁 + 双向断言
Step 10 Phase D               文档/发布物/版本
Step 11 日志回写              完成率 + 证据 + 反例注入记录
```

> Step 4 必须先于 Step 7：先强制每个模块表态，再删 —— 否则「该表态而未表态」的模块会被误删。
>
> Step 2（E-1）先于 Step 3（B）：B 的通用构造入口以工厂名称为索引，**工厂名越全，B 的覆盖面越广**。
> Step 8（E-2）放在 Step 7 之后：新建控件应发生在零消费者模块收敛之后，避免“刚删完又需要”。

---

## 九、一句话结论

**BLUE15 解决的是「控件由谁画」；BLUE16 要解决的是「画好的东西谁能用、死掉的谁能发现」。**

三条线的真实状态不对称：**Theme 已闭环**（第 21 轮接上 C ABI 与窗口 API 两条主漏斗），
**CSS 只挂在零消费者的 JSON 路径上**（`apply_css`/`global_stylesheet_manager` 的唯一生产调用点在 `json/loader.rs`，而 `json` 的消费者是本仓的测试与基准），**而 `app`（曾被误判为与 `json` 构成闭环死链）实际有 13 处外部消费者**。
更根本的问题在对外契约：**167 个控件中 C 调用方只能创建 23 个，253 个属性名一个都够不到，
连 `rw_destroy_widget` 都漏在发布头文件之外**——而所有文档都指向那份过期的头文件。

因此本轮不是「再做三个功能」，而是**四件事**：① 把发布物与生成物锁死（否则改对了也没人拿到）；
② 把已实现的能力一次接通对外（通用构造 + 通用属性，而不是再加 144 个函数）；
③ **接通 6 个已实现但未进工厂的控件**（§十二，零新逻辑，直接扩大 JSON/CSS 可达面）；
④ 对全仓每个 `pub` 模块强制可达性表态，把约 **30,800 行零消费者代码**按三态收敛 ——
其中 JSON 的去留是**产品决策**，必须先由用户拍板（§6.1），否则删除风险不可控。

**判例不变**：不做规范级完整（Qt 的 QSS 裁剪与 QML 另建运行时都是依据）。
本轮补的是**可达性与一致性**，不是功能面。

---

## 十、自我审查（Self-Review）

> 审查标准：**可用 / 可达 / 统一 / 精炼 / 高效**（§1.1）。

### 10.1 ✅ 已达标（取证后确认无需改）

| 项 | 证据 |
|---|---|
| 控件落地机制唯一 | BLUE15 §G/§H：167/167 `CustomRequired`，`Platform` 必需方法 75 → 6 |
| 主题解析链 | 第 21 轮：`explicit → theme → default` 三级合并，有 7 条测试断言产物颜色 |
| CSS 表达力 | 第 21 轮：`WidgetStyle` 全部可设字段均有属性（46 条测试） |
| `compat` 抽象层 | 112 处生产引用 / 61 文件，承载 std↔alloc 边界，且文档诚实（明确说明 `alloc_frugal` 尚未真正 `no_std`） |
| 既有 17 个门禁 | 全 PASS（本轮复跑） |

### 10.2 🔴 阻断级发现（本轮必须修）

| # | 发现 | 证据 | 影响 |
|---|---|---|---|
| 1 | **发布头文件缺 4 个函数（含唯一析构入口）** | `grep -c rw_destroy_widget include/` → **0**；`examples/` → 1 | 外部 C 消费者**不能销毁控件**（泄漏） |
| 2 | **发布路径零门禁** | `check_abi.sh` 只 `cmp` `examples/` | 两个头文件不一致却都"通过" |
| 3 | **通用属性层完全未暴露** | `grep -c read_widget_property_by_id src/bindings/` → **0** | 253 个属性名不可达；连 base 的 `tooltip` 都不可达 |
| 4 | **167 kind 仅 23 个可创建** | `rw_create_*` 去重 → 23 | tree/table/grid/tabs/rich-text/charts 全部不可达 |
| 5 | **列表/表格无法填充** | 仅 `list_box`/`combo_box` 有 `add_item` | `rw_create_list_view` 造出永远空的控件 |
| 6 | **无布局管理、无滚动控制** | `grep -c "add_widget\|set_layout"` → 0；`grep -c "set_scroll"` → 0 | C 侧只能绝对定位、无法滚动 |

### 10.3 🟡 精炼不达标：约 30,800 行零消费者

见 §2.4。**本轮不擅自删除**，因为其中多数是**产品决策**（§6.1）而非技术判断；
本轮做的是**强制表态**（规则 #72）+ 门禁，使「悬置」不再可能。

> 这与我前几轮的自我批评一致：**上一轮我论证了「不该扩张」，但没有把「已有的东西是否可达」
> 验证到底**。本轮把这条判据制度化（规则 #71/#72），使它不再依赖我某一次是否想起来检查。

### 10.4 统一性缺陷

| 项 | 证据 | 处置 |
|---|---|---|
| 两个 C 头文件并存且不一致 | §2.3 | Phase A：双向 `cmp` 门禁；长期应统一 |
| `embedded`（死） vs `platform::portable`（live） | 名字近义，且**职责也被 `platform::profile` 覆盖** | 已复跑（§2.6）：删 `embedded`；在 `platform::portable` 与 `platform::profile` 文档写明各自职责 |
| `render/gpu` 的模块文档方向**倒置**（规则 #74 违规） | ✅ **已复跑**：`render/gpu/mod.rs:7-9` 声称 "re-exports its types"，但 `gpu_types.rs:8,24` **定义**了 `GpuCapability`/`GpuRenderer`，而 `wgpu_backend/renderer.rs:9` **反过来 import 它** | 修正文档方向，或把定义移到 `wgpu_backend` 并让 `render/gpu` 真正 re-export |
| `performance`（1,231 行，死） vs `gpu::performance`（live） | ✅ **已复跑**：`crate::performance` 仅 1 处文档引用；`src/gpu/mod.rs:43` 用的是 `super::performance`（它自己的子模块，另一个文件） | 二者取一：若功能重复删除死的；若不同则重命名消除歧义 |
| `quality`（601 行，live） vs `render::quality`（238 行，死） | ✅ **已复跑**：`crate::quality` 有 3+ 生产引用（`gpu/adapter.rs:273-276`）；`render::quality` 外部引用 **0**，`AdaptiveRenderer`/`QualityAwareRender` 从未被构造 | 删 `render::quality`，并在 `crate::quality` 文档写明它是唯一入口 |
| `style` ↔ `theme` 双向依赖 | `theme/mod.rs:41` re-import `style::HighContrastMode`；`style/mod.rs:72` glob re-export `theme::*` | 可存活，但需在文档写明方向 |
| README 声明「100 个函数」，实际 106 | §2.3 第 3 项；**我上一轮引入** | Phase A-2 加断言 |

### 10.5 🟡 高效性：本轮未度量

计划 §1.1 把「高效」列为维度，但**本轮未做性能取证**。已有 `benches/` 与
`tools/check_perf.sh`，但本轮未跑基准、未设门槛。**登记为未完成**，不冒充。
触发条件：Phase B 的通用属性层**会在创建路径上增加一次字符串解析**，
届时必须用基准证明它不在热路径上（否则改为缓存 name→index 解析）。

### 10.6 本计划的已知弱点（主动登记）

| 弱点 | 说明 |
|---|---|
| §2.4 的行数与可达性判定**部分**来自子代理 | 我已**独立复跑**：9 个模块的引用计数、两个头文件对比、C ABI 的 4 项 grep、`render/gpu` 文档倒置、`performance`/`quality`/`embedded` 三对近义模块、`src/embedded/flags.rs` 的逐符号调用者，以及**纳入 `examples/`/`tests/`/`demo/` 的重跑**。 |
| **写作过程中推翻了子代理的一个结论** | 「`app` 与 `json` 构成 7,375 行闭环死链」——**错**。`app` 被 examples/tests/demo（共 13 处）使用，在 `src/` 作用域内看不见。已在 §2.4/§二.1 就地更正并保留记录（原则 #64）。 |
| 同样地，我**自己也推翻了初稿的 `embedded` 判定** | 初稿按「整模块零引用」归为重复；逐符号复跑后改为「**转发层**」——`flags.rs` 读的是活的 `profile::surface_policy()`，但它自己的 5 个预算函数零调用者。见 §2.6。 |
| **尚未复跑**：`src/web` 的 5,100 行逐符号可达性；`pdf`/`audio`/`video`/`menu_config`/`data_binding`/`asset` 的「公有类型从未被构造」清单；`performance`/`asset`/`index` 那 1–2 处引用是否仅为文档 | Phase C-1 的**第一步**就是把这些全部复跑再分类。**不在复跑前删除任何代码。** |
| Phase C 的删除量在用户决策前**不可估** | §6.1 未定时，净行数变化的区间无法收窄；但**已确认 `app` 不在删除候选内**。 |

### 10.7 自我审查结论

**本计划的核心不是加功能，而是补两类制度**：
① **发布物一致性门禁**（规则 #70）——否则一切修复到不了消费者手里；
② **强制可达性表态**（规则 #72）——否则「实现完整但无人可达」会继续以每轮数千行的速度累积。

**最需要用户输入的一点**：§6.1 的 JSON GUI 去留。它牵动 4,026 行的存续（不含 `app` —— `app` 已确认有真实消费者），
且**只有在定位明确时才有正确答案**。本轮建议「保留 + 登记为实验」（转 ① 或 ③ 皆可），
但这是产品判断，不是技术判断。

---

## 十一、补充取证：手势/触摸、控件属性、信号槽、新控件

> 来源：用户追问「是否需要增加新控件、手势/触摸是否需增强、已有控件属性是否需扩充、信号槽是否需改进」。
> 本节为**追加取证**，已并入 §九 的结论。
>
> **取证纪律（本节的真正价值）**：两个只读子代理共报出 ~25 项「缺陷」。我逐项复跑后
> **推翻了其中一项（最大的一项）**，并对另两项重新定性。下列表格区分三态：
> ✅ 已复跑确认 / ❌ 已复跑推翻 / ⚠️ 未复跑（登记）。

## 11.1 ✅ 手势与触摸：**引擎完全未被馈入**（最高优先级）

### 硬证据（✅ 已复跑）

```bash
$ grep -rn "TouchBegin" src/platform/ --include=*.rs | wc -l
0                                    ← 没有任何后端产生触摸事件

$ grep -rn "Event::TouchBegin {" src/ | grep -v "=>" | grep -v "^src/gesture/" | head
src/event/translator.rs:37   ← 仅文档注释
（其余均为测试夹具）
```

而手势引擎的调用点是**真实存在**的，只是条件永不满足：

```rust
// src/event/loop.rs:196, 227, 269
if event.is_touch() {           // ← is_touch() 只认 Touch*/手势变体，**排除鼠标**
    gesture_engine.process(event, now_ms());
}
```

**后果链**：无后端产生 `TouchBegin` → `is_touch()` 永不为真 → `GestureEngine::process` 永不被实际输入调用
→ **11 个识别器仅在单测中可达**（`src/gesture/` 共 4 个测试）。

### 另外三个独立缺陷（同根因，均已复跑）

| # | 缺陷 | 证据（已复跑） |
|---|---|---|
| 1 | **`Event::DoubleTap` 经引擎不可达** | `engine.rs:96-101` 首个命中即 `return`；`TapGesture` 是 index 0 且产出 `Tap`，而 `DoubleTapGesture`（index 1）**只接受** `Tap` 作为输入 —— 它永远收不到。 |
| 2 | **`PanGesture` 吞掉 Swipe/Fling** | `press.rs:140` 在**任何** `TouchMove` 上无条件发 `Drag`（**无位移阈值**），且 index 4 在 Swipe(5)/Fling(6) 之前。任何有位移的拖动都会提前返回。 |
| 3 | **`RotateGesture` 角度环绕未归一化** | `rotate.rs:55` `let delta = current_angle - prev;`，而 `angle_between` 用 `dy.atan2(dx)`（范围 `(-π, π]`）。跨 ±π 时 `delta` 突跳 ~±2π，超过 `0.05` 阈值 → 发出一次虚假的整圈旋转。 |

> 缺陷 3 正是新规则 **#76** 的实例：周期量差分未归一化。

### 是否要「增强手势」？

**不是增强，是接通。** 现状不是「手势不够强」，而是整条链路**输入端断裂**。
新增识别器只会增加不可达代码（违反 #71）。

**本计划的处置**（并入 Phase B）：

| 步骤 | 内容 |
|---|---|
| B-5a | **修缺陷 1/2/3**（纯 `src/gesture/` 内，不依赖平台）：让引擎把识别器产出的事件**回喂**给后续识别器（而非首个命中即返回）；给 `PanGesture` 加位移阈值；`RotateGesture` 做 `delta` 归一化到 `(-π, π]` |
| B-5b | **平台层产生触摸事件**：各后端把指针事件补上 `TouchBegin/Move/End`（或明确决定“桌面无触摸”，并交 `is_touch()` 改判） |
| B-5c | **新增门禁 `check_event_producers.sh`**（规则 #75）：每个 `Event` 变体必须有非测试构造点，否则 FAIL |
| B-5d | 为 9 个未测识别器补测试（含双指场景与环绕边界） |

**执行结果（2026-09-17，见 `log-20260917-2.md` §83–§91）**：**B-5a / B-5b / B-5c / B-5d 全部完成**。
用户拍板选 **① 真触摸**（三个后端都写）。

计划本身有 2 处判定不准，已就地更正（原则 #64）：

| 计划原文 | 复跑结论 |
|---|---|
| “缺陷 2 `PanGesture` 吞掉 Swipe/Fling” | ❌ **部分推翻**。`Swipe` **确实会发出**（`TouchEnd` 时），被吞掉的不是 Swipe 而是**用户的意图**：`PanGesture` 在**第一次** `TouchMove` 就发 `Drag`（`delta` 可达整段位移），而 `ScrollArea` 按 `Drag::delta` 滚动，于是“按下即滚”与“滑动”无法区分 |
| “11 个识别器仅在单测中可达” | ⚠️ **修正为 10 个手势事件 + 3 个后端**。可达性缺口是“事件无生产者”，不是“识别器不可达”；按事件计数才是可门禁的判据 |

**追加发现的第 4 个缺陷**（计划未列，由新测试抓出）：
`TwoFingerTapGesture` 用 `self.touches.is_empty()` 判定“两指都起来了”，但**单指抬起也会使列表为空**，
且从未校验手指数 → **每一次普通单指点击都会被同时报成 `TwoFingerTap`**。修复为补 `touch_ends.len() == 2` 校验，
并顺带修掉 `self.touches[0]` 在移除后索引的潜在 panic（改为按 id 取起始时间）。

#### B-5b 各后端的实测状态（区分已实跑与未实跑）

| 后端 | 接入的 API | 本机验证程度 |
|---|---|---|
| **Linux / GTK3** | `connect_touch_event` + `TOUCH_MASK` + `GdkEventSequence` 作 `TouchId` | ✅ `cargo check` + `clippy -D warnings` |
| **Windows** | `WM_TOUCH` + `RegisterTouchWindow` + `TOUCHINPUT.dwID` 作 `TouchId` | ✅ 交叉编译 `x86_64-pc-windows-msvc` + `clippy -D warnings` |
| **macOS** | `touchesBegan/Moved/Ended/CancelledWithEvent:` + `NSTouch.identity` 作 `TouchId` | ✅ 交叉编译 `aarch64-apple-darwin` + `clippy -D warnings` + `check_apple_thread_safety` |
| **实机触摸输入** | — | ❌ **未验证**（本机 Linux 且无触摸设备，且 GTK 后端需有触摸屏才生效）|

> 必须写清楚：**三个后端都“能编译且逻辑有测试”，但没有任何一个在真实触摸硬件上跑过。**
> 多指路径的正确性由 `two_independent_contacts_reach_pinch` / `..._reach_rotate` 两条测试证明
> （它们用后端会产生的 `touch_id` 形态驱动引擎，得到 `Pinch { scale: 1.6 }` 与 `Rotate { angle: 1.5707963 }`）。
> **这证明了「识别器收到了两个独立触点后能正确工作」，不等于「Windows/macOS 的触摸后端在实机上正确」。**

**决策点（需用户拍板）**：B-5b 有两条路：
- **① 真触摸**：逐后端接 OS 触摸 API（Windows `WM_TOUCH`/`POINTER`、macOS `NSTouch`、GTK `GtkGesture`）。工作量大，但双指手势（Pinch/Rotate）需要它才可能工作。
- **② 鼠标合成**：让 `is_touch()` 对鼠标也为真，单指手势（Tap/DoubleTap/LongPress/Swipe/Fling/Pan）可用，双指仍不可用。

我建议 **① 与 ② 都做**：② 作为降级路径使单指手势立即生效，① 作为真能力。但若只选一个，**②** 的性价比远高。

## 11.2 ✅ 多指支持：**不存在**

| 事实 | 证据（✅ 已复跑） |
|---|---|
| 捕获管理器只有**单**指针 | `src/event/capture.rs:9-14`：`capturing_widget: Option<ObjectId>` |
| 悬停全局唯一槽 | `runtime.rs:144-151`：一个 `RefCell<Option<ObjectId>>` |
| 鼠标事件**无指针标识** | `event/types.rs:101-127`：`MousePress/Move/Release` 只有 `pos` + `button` |
| `TouchEventTranslator` 能按 `TouchId` 跟踪，但**零生产调用者** | `translator.rs:44` 有 `HashMap<TouchId, _>`；全仓引用仅自身 + 测试 |

Pinch/Rotate 的识别器**逻辑是对的**（正确维护两个 `PinchTouch`、按 id 匹配、按 id 移除），
只是**收不到两个独立指针**。所以这是输入层缺口，不是识别器缺陷。

**处置**：并入 B-5b（接入真触摸后自然后续）；若选②鼠标合成，则需新定义 `PointerId` 并使
`MousePress` 携带它 —— 这会改 `Event` 枚举，属向**前兼容**的加法（新字段），但需与 `is_touch` 一并评估。
**本轮不承诺**，登记为 B-5b 的子项。

## 11.3 ✅ 控件属性：**主要是「未发布」，而非「未实现」（且子代理有一项误报）**

### ❌ 复跑推翻的一顶：`readable:false` 不是「schema 在说谎」

子代理报：「34 对 schema 声明可写但契约报错，schema 在欺骗消费者」。
**复跑后推翻了一半** —— 那些全 `false` 的条目是为 **共享 kind 的别名**标记的占位。

```rust
// src/widget/input_widgets/textedit.rs:220-226（原文）
//! `WidgetKind::TextEdit` is the kind the capability layer pairs with the
//! [`TerminalView`] control ..., so the old centralised `TextEdit` arms were
//! answered by `TerminalView`, not by this widget — despite `TEXT_EDIT_PROPERTIES`
//! existing, its names are all marked non-readable and non-writable, so the
//! multi-line editor below never served a property. ... this widget publishes none
//! of its own rather than claiming another control's.
```

且 `TEXT_EDIT_PROPERTIES` 的 5 个条目**全部** `readable:false, writable:false`（已复跑）。
**真正的缺陷不是「schema 说谎」，而是「这个佔位方式太隐晦」**——一个全 `false` 的表看起来像忘记填，
而非「此名字属于它的兄弟控件」。**处置**：Phase C 时给这类数组加显式文档说明（或换为 `ALIAS_PLACEHOLDER_PROPERTIES` 具名常量）。

### ✅ 复跑确认的真缺陷

| # | 缺陷 | 证据（✅ 已复跑） |
|---|---|---|
| 1 | **schema 声明但契约未实现的属性**：`Meter` 声明 `value`/`minimum`/`maximum`，契约**只发布 `value`**（且 `Meter` **根本没有** `minimum()`/`maximum()` 访问器，只有 `set_range()`） | `properties_other.in.rs:875-898` vs `meter.rs:133` |
| 2 | **6 个控件只发布 4 个 base 属性**，却拥有丰富状态：`Canvas`（`commands: Vec<RenderCommand>`）、`ChartWidget`（`data: Vec<f64>` + `labels`）、`MiniCanvas`、`CupertinoSegmentedControl`、`RichEdit`、`TextEdit`（后者有正当理由，见上） | `canvas.rs:204-217`、`chart.rs:141-154` |
| 3 | **42+ 控件的集合状态经通用层不可达**：`ComboBox.items`、`ListBox.items`、`TreeView.nodes`、`TableWidget` 行列、`GanttWidget.tasks`、`Chip.items` …… **全部只发布计数，发布不了内容** | `combobox.rs:19` vs `combobox.rs:208-261` |
| 4 | **双向断言缺失**（规则 #77）：`schema_and_contract_publish_the_same_names`（`properties_tests.rs:659`）**只检查 `published ⊆ declared`**（`:671-677`），从不检查反向 → 这是 16 个 schema-only 名字得以存活的原因 | 已读该测试源码 |
| 5 | **Enum 的合法 token 无公开发现途径**：`PropertySchema`（`types.rs:100-118`）**没有**接受值列表字段；无公开函数可枚举 token；只有私有 `expect_*(...)` 的 doc 注释 | 已复跑 |
| 6 | **颜色/几何以字符串传输**：`color`/`hex_rgba`/`fill_rgba`/`stroke_rgba`/`line_color` 为 `String`；**`geometry` 被拼成 `"x,y,w,h"` 字符串**（155 个 schema 数组 + `BASE_PROPERTY_NAMES` 都有它） | `properties_trait.rs:114-119` |
| 7 | `UnsupportedOnWidget` 用错位置（应为 `TypeMismatch`/`ReadOnlyProperty`） | `list_view.rs:381`、`tree_table.rs:351`、`tree_view.rs:238` |
| 8 | `MiniCanvas` 已注册但**无默认值臂** → `capability_manifest("mini_canvas")` 硬报错 | `access.rs:1213`（`_ => return None`） |

### 是否要「扩充控件属性」？

**需要，但方向是「发布已有」而非「新增能力」**（规则 #71）。并入 Phase B：

| 步骤 | 内容 |
|---|---|
| B-6a | `PropertySchema` 新增 `accepted_tokens: &'static [&'static str]`（Enum 专用，其他为 `&[]`）；新增公开 `rw_property_tokens(name)` 与 Rust 侧同义函数（消除 #5）|
| B-6b | 补 16 个 schema-only 属性的契约实现，或**从 schema 删除**（二者必选其一，不得悬置）（消除 #1）|
| B-6c | 集合属性：新增一类**序列属性**（如 `item.<i>` 或专用 `rw_widget_list_*`），使集合内容可达（消除 #3）—— 与 §四 B-3 的 `rw_widget_list_add` 合并设计 |
| B-6d | `PropertyValueKind` 增 `Color` / `Rect` 变体，或约定 `String` 为十六进制并在 ABI 层提供 `rw_parse_color`（消除 #6）；**必须先决定**，因为它直接决定 B-2 ABI 的形状 |
| B-6e | `schema_and_contract_publish_the_same_names` 改为**双向**断言（消除 #4，规则 #77）|
| B-6f | 修 4 处 `UnsupportedOnWidget` 误用（#7）；给 `MiniCanvas` 补默认值臂（#8）|

## 11.4 ✅ 信号槽：**机制正确（有 8 条测试背书），但有重复系统与未测不变式**

### ✅ 复跑确认做得对的部分（不应改）

- `emit` 的 take/restore 舞蹈真正保证了：自断开、断开后续槽、在回调中 `connect`、重入 `emit` 均安全；
  有 **8 条专门测试**（`core_signal.rs:419-750`）。
- 回调在**无锁**状态下执行，因此重入不会死锁。
- `CustomSignalHub::emit` 先克隆再发，避免了自死锁（`hub.rs:48-56`，有测试）。
- 速度单位 px/s 一致，且有回归测试（`engine.rs:163`）—— 上轮记录的缺陷**已修**。

### ⚠️ 真正的缺口

| # | 问题 | 定性 |
|---|---|---|
| 1 | **多线程 `emit` 安全性无测试**：文档声称 `Send + Sync` 且允许跳线程发射（`signal/mod.rs:35-36`），但 `src/signal/` 内 **零** `thread::spawn` | ⚠️ 已读源码，未跑（写测试即可验）|
| 2 | **`connect` 返回裸 `ConnectionHandle`，不是 RAII**；RAII 需显式用 `ConnectionScope` + `connect_scoped` | ✅ 已复跑 |
| 3 | **两套并行通知机制**：313 个 signal 字段 + 163 个 `impl EventHandler`。`BaseWidget` 同时有 11 个 signal 并实现 `EventHandler`，把**同一事实**（“按钮被点了”）送到消费者的路径有**三条** | ✅ 已复跑；**不违反原则 #33**（信号存的是 `Box<dyn FnMut>`，不是 C 函数指针），但是**第二个未经调和的机制** |
| 4 | **15 个 signal 从不发射**（含 `InputDialog` 的 3 个值变更信号）；**81 个从未被连接** | ✅ 已复跑；但 `lcd_number.rs:57-63` 与 `input_dialog.rs:53-62` **已在文档中诚实地写明“declared-but-inert”** |
| 5 | `BaseWidget.mouse_pressed` 由路由维护不了，但 `mouse_down`/`mouse_up` **照发** → 标志与信号可能不一致 | ✅ 已复跑；`base.rs:340-342,382-383` **已文档说明该陷阱** |

### 是否要「改进信号槽」？

**机制层面：不需要改。** 它是一个正确的、有测试背书的多播实现，改它是无收益风险。

**需要处理的是另三件事**，并入 Phase C：

| 步骤 | 内容 |
|---|---|
| B-7a | 补多线程 `emit` 的测试（微小，但文档已承诺）|
| B-7b | 15 个从不发射的 signal：**要么接上发射点、要么删除**（按 #72 同口径，不得悬置）|
| B-7c | 在 `docs/ARCHITECTURE.md` 写清 signal 与 `EventHandler` 的**分工**（谁负责“控件内部行为”、谁负责“对外通知”），消除“三条路径”。**不合并**——合并的收益未取证（原则 #51）|

## 11.5 是否需要新增控件？

**初稿结论“不需要新增”已在 §十二 就地更正**：拆分后实际是
「**0 个新建** + **6 个补齐注册**」，而非笼统的“不做”。当时把两类不同的工作混为一谈，
且把判据错误地建立在 C ABI 全量可达上（见 §12.0 的更正记录）。

保留仍成立的部分：**纯粹为“凑数量”而新建控件，本轮不做。** 依据：

1. 集合状态不可达（§11.3 #3），60+ 控件只发布个位数属性。**现有控件的可达性远未穷尽**：
   新增控件只会加大分母（违反 #71）。
2. **零消费者模块已约 30,800 行**（§2.4）。在此之上加控件是错序。
3. 与第 19–21 轮判例一致：先让已有的“能被用”，再谈更多。

## 11.6 本节新增的可度量目标

| 指标 | 当前 | 目标 |
|---|---:|---:|
| `Event` 变体有真实生产者 | 待建门禁度量（预计远低于 100%） | **100%**（或显式登记为“仅测试用”）|
| 手势识别器有行为测试 | **2 / 11** | **11 / 11** |
| schema-only 属性（声明无人实现） | **16 名字 / 41 对** | **0** |
| 双向名称断言 | 单向 | **双向** |
| 从不发射的 signal 字段 | **15** | **0**（发射或删除）|
| C ABI 可达的控件 kind | **23 / 167**（降级，见 §12.0）| **23 / 167**（非本库主线，按需扩展）|
| `PropertySchema` 可报告 Enum 合法值 | ❌ | ✅ |

---

## 十二、控件覆盖度：缺失与补齐任务

### 12.0 定位更正记录（**先于一切结论**）

本节写作过程中，我犯过一个方向性错误，就地更正如下（原则 #64：写错就地更正并保留记录）。

| 我的错误结论 | 实际 | 更正依据（实跑） |
|---|---|---|
| “C ABI 只能创建 22/167，这是最高优先级问题” | **判据错位**。本库是 **Rust 库**（`Cargo.toml:5` “Pure Rust cross-platform native GUI library”）。C ABI 是**可选绑定层**，服务 `examples/cpp/`、`python/`、`java/`、`harmony_napi`，不是主 API | `ls examples/` → `c_abi_*.c`、`cpp/`、`python/`、`java/` |
| “29 个控件隐身，语义塌缩” | **夸大 5 倍**。19 个疑似项中 **15 个已用自身 `canonical_name` 注册进工厂**，Rust 侧完全可达 | `grep -cE "register\(breadcrumb" src/widget/capability/registration.rs` → `REGISTERED`；`properties.rs:571` `canonical_name: "breadcrumb"` |
| “35 / 27 / 24 个 kind 无同名注册 → 缺口” | **再次错**。这些几乎全是 `pub type` **类型别名 kind**（如 `Panel = GroupBox`），**有意不注册** | `grep -n "^pub type " src/widget/mod.rs` → **23 条**（见 §12.7）|
| “`BaseWidget` 复用基类 kind 是缺陷” | **有意设计且已修复相关 bug**。源码已写明理由与踩坑史 | `src/widget/capability.rs:521-541`（`capability_by_kind` 的 doc 注释）|
| 上一条消息的 `grep -c "\\"breadcrumb\\""` → `0` | **搜索模式错**。代码形式是 `register(breadcrumb_capability(), ...)`，带引号搜索自然为 0 | 用 `register\(<name>` 复跑，命中 |

**教训（并入规则 #73 的实例）**：定位错误会**系统性放大**缺口数量。
先确认“这是给谁用的 API”，再数缺口。

**第二个教训（同轮再次踩坑）**：我随后又用“`WidgetKind` 变体应有同名工厂注册”
作判据，得到 35 / 27 / 24 个“缺口”——**仍然错**。因为项目大量 kind 是
`pub type` **别名 kind**（`grep -n "^pub type " src/widget/mod.rs` → **23 条**）。
正确判据必须是“**是否能通过某名字或别名构造出控件**”，而非“名字是否与 kind 同名”。
详见 §12.7 的完整更正与复跑结果。

**由此产生的优先级重排**：

| 原优先级 | 修正后 | 理由 |
|---|---|---|
| C ABI 22/167 → 167/167（P0）| **降级为按需** | 非 Rust 主线；绑定层扩展由真实 C/Python/Java 消费者驱动 |
| — | **P0：Rust 侧工厂名覆盖 155/167 → 161/167** | **真缺口，且是纯接线** |
| — | **P1：新建 4 个控件**（§12.3）| 三框架常用交集，有真实交互缺口 |

### 12.1 真缺口 A 类：有完整实现、但**未接进工厂**（6 个）

复跑命令与结果：

```bash
$ for n in timeline_widget command_palette notification_center diff_viewer markdown_editor toast_stack; do
    printf "%-20s register=%s ctor=%s\n" "$n" \
      "$(grep -rcE "register\($n" src/widget/capability/registration.rs)" \
      "$(grep -rc "fn create_$n" src/widget/capability/constructors.rs)";
  done
timeline_widget      register=0 ctor=0
command_palette      register=0 ctor=0
notification_center  register=0 ctor=0
diff_viewer          register=0 ctor=0
markdown_editor      register=0 ctor=0
toast_stack          register=0 ctor=0
```
✅ 已复跑

**但这 6 个控件本身是完整实现**（`pub struct` + `pub use` 导出）：

```bash
$ for n in TimelineWidget CommandPalette NotificationCenter DiffViewer MarkdownEditor ToastStack; do
    grep -rl "pub struct $n" src/widget/ --include=*.rs | head -1; done
src/widget/special_widgets/timeline_widget.rs
src/widget/special_widgets/command_palette.rs
src/widget/special_widgets/notification_center.rs
src/widget/special_widgets/diff_viewer.rs
src/widget/special_widgets/markdown_editor.rs
src/widget/special_widgets/toast.rs
$ grep -n "TimelineWidget\|CommandPalette\|DiffViewer" src/widget/mod.rs
399:    CommandEntry, CommandPalette, DiagnosticMarker, DiffKind, DiffLine, DiffViewer,
403:    TimelineWidget, ToastItem, ToastLevel, ToastStack,
```
✅ 已复跑

| # | 控件 | `pub struct` | `pub use` | capability | 构造函数 | 已注册 | 缺什么 |
|---|---|---|---|---|---|---|---|
| 1 | `TimelineWidget` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |
| 2 | `CommandPalette` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |
| 3 | `NotificationCenter` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |
| 4 | `DiffViewer` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |
| 5 | `MarkdownEditor` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |
| 6 | `ToastStack` | ✅ | ✅ | ❌ | ❌ | ❌ | **三项全缺** |

**后果（可描述、非推测）**：Rust 用户可以用 `TimelineWidget::new(rect)` 手工构造，但
`factory.create("timeline_widget", rect, "")` 返回 `None`。因此这 6 个控件：

- **不能从 JSON GUI 构造**（JSON 加载器经工厂解析名称）—— 直接削弱第 19–21 轮成果；
- **不能被 CSS 选择器命中**（选择器按 canonical name 匹配）；
- **不能经 `create_by_kind` 解析**（`kind_to_index` 无条目）。

**与 §11.5 的关系**：这**不是**“新增控件”，而是**接通已有实现**。属于 §11.5 保留结论
“先让已有的能被用”的正面执行。

### 12.2 真缺口 B 类：**连实现都没有**（对照三框架后仅 4 个该做）

判定方法：对照 Qt Widgets + Flutter Material 3 + GTK4 的常用控件清单，逐个 `grep` 本项目。

```bash
$ for n in NumberPicker OtpInput Pagination Banner Toast PopupButton ZoomControl; do
    printf "%-14s struct=%s\n" "$n" "$(grep -rl "pub struct $n" src/ --include=*.rs 2>/dev/null | head -1)"; done
NumberPicker   struct=
OtpInput       struct=
Pagination     struct=
Banner         struct=
Toast          struct=src/widget/special_widgets/toast.rs   ← 只有 ToastStack，无 Toast 本体
PopupButton    struct=
ZoomControl    struct=
```
✅ 已复跑

| # | 控件 | 对标 | 为什么算常用 | 本项目现状 | 判定 |
|---|---|---|---|---|---|
| 1 | **`NumberPicker`** | iOS `UIPickerView` / Android `NumberPicker` | 移动端数字滚轮；`SpinBox` 是桌面步进，**交互不同** | 无 | ✅ **做**（P1）|
| 2 | **`OtpInput`** | Flutter OTP / 各家验证码 | 登录流程高频；`MaskedEdit` 是整体掩码，**不覆盖分格** | 无 | ✅ **做**（P1）|
| 3 | **`Pagination`** | Ant Design / Bootstrap | 表格分页高频；`PagerPageView` 是**页面滑动**，非页码跳转 | 无 | ✅ **做**（P1）|
| 4 | **`Banner`** | Material 3 `Banner` | 需用户**显式关闭**的持久提示；`Snackbar` 会自动消失 | 无 | ✅ **做**（P2）|
| 5 | `PopupButton` | Qt `QPushButton` + menu | 有 `MenuButton`（点击弹菜单），缺“带默认值的分段弹出” | 无 | ❌ **不做**（`MenuButton` 覆盖 ~90% 场景）|
| 6 | `ZoomControl` | 图像/文档查看器 | `ImageView` 无缩放 UI | 无 | ❌ **不做**（可由 `Slider` + `ImageView` 组合）|
| 7 | `Toast`（非 Material）| Android `Toast` | 已有 `MaterialSnackbar` + `ToastStack` | 无独立 kind | ❌ **不做**（`ToastStack` 已覆盖）|
| 8 | `Magnifier` / `Ruler` | Windows Magnifier / 设计工具 | **长尾专业工具** | 无 | ❌ **不做**（登记为范围外）|

**“不做”的判例登记**（原则 #56：明确不做的也要有理由与触发条件）：
上述 4 项不做，触发条件为“某个真实消费者提出需求”，届时重新评估。

### 12.3 Phase E — 控件补齐（插入 §四 之后执行）

> **执行顺序**：Phase E 应在 **Phase A（发布物一致性）之后、Phase C（死重收敛）之前**执行。
> 理由：E 会新增工厂注册项，而 C 会删除零消费者模块；先 E 后 C 可避免“刚删完又需要”。

#### E-1：接通 6 个已有控件（无新逻辑，纯接线）

每个控件四步，与既有控件完全同构（复制 `breadcrumb` 的模式）：

| 步骤 | 文件 | 内容 |
|---|---|---|
| 1 | `src/widget/capability/properties_*.rs` | 新增 `<name>_capability()`：`canonical_name`、`aliases`、`properties`、`events`、`commands` |
| 2 | `src/widget/capability/constructors.rs` | 新增 `create_<name>(geometry, text) -> Box<dyn Widget>` |
| 3 | `src/widget/capability/registration.rs` | 新增 `self.register(<name>_capability(), create_<name>)` |
| 4 | `src/widget/draw_bridge.rs` | 确认绘制分派可达（多数复用基类分支，需实测）|

**归属文件建议**（按现有分组）：

| 控件 | capability 归属 | 建议 `canonical_name` | 建议 aliases |
|---|---|---|---|
| `TimelineWidget` | `properties_view.in.rs` | `timeline_widget` | `timeline`, `timeline_view` |
| `CommandPalette` | `properties_other.in.rs` | `command_palette` | `command_box` |
| `NotificationCenter` | `properties_other.in.rs` | `notification_center` | `notifications` |
| `DiffViewer` | `properties_view.in.rs` | `diff_viewer` | `diff` |
| `MarkdownEditor` | `properties_input.in.rs` | `markdown_editor` | `md_editor` |
| `ToastStack` | `properties_other.in.rs` | `toast_stack` | `toasts` |

**验收**：`factory.create("<name>", rect, "")` 返回 `Some(_)`，且 `create_by_kind` 可达。

#### E-2：新建 4 个控件（§12.2 判定为“做”）

**准入条件（四项缺一不可，沿用 §11.5）**：
`WidgetFactory` 注册 → 属性契约 → `draw_bridge` → 交互行为测试。

> **注意**：原四项中的“C ABI 可达”已按 §12.0 更正移除。Rust 库的新控件不因
> “C 侧暂时用不到”而不完整；C ABI 扩展单独在 binding 层按需补。

| # | 控件 | 新增文件 | 关键属性 | 关键交互 | 优先级 |
|---|---|---|---|---|---|
| 1 | `NumberPicker` | `src/widget/input_widgets/number_picker.rs` | `value`、`min`、`max`、`step`、`wrap` | 滚轮滚动 / 上下一格 / 长按连续 / 键盘 ↑↓ | P1 |
| 2 | `OtpInput` | `src/widget/input_widgets/otp_input.rs` | `length`、`value`、`masked`、`separator` | 单格输入自动前移 / 退格回退 / 粘贴分发 / 方向键 | P1 |
| 3 | `Pagination` | `src/widget/nav_widgets/pagination.rs` | `total`、`page`、`page_size`、`sibling_count` | 点页码 / 上下一页 / 省略号展开 | P1 |
| 4 | `Banner` | `src/widget/overlay_widgets/banner.rs` | `text`、`severity`、`dismissible`、`actions` | 关闭按钮 / 动作按钮 / 出入场动画 | P2 |

**每个新控件必须同时**：
1. 新增 `WidgetKind` 变体，并同步 §七 的 kind 计数门禁（`check_widget_kind_count.sh`）；
2. 补行为测试（≥1 条真实交互，非仅构造）；
3. 若纳入 CSS，则在 `src/style/css.rs` 的 kind 解析表加映射；
4. 若纳入 JSON，则确认 `src/json/loader.rs` 的 kind 字符串可解析。

**预期 kind 计数变化**：167 → **171**（+4）。此数字必须在四份文档中同步：
`docs/plans/blue16.md`、`README.md`、`README.zh-CN.md`、`docs/ARCHITECTURE.md`。

#### E-3：新增门禁 `tools/check_widget_registration_fidelity.sh`

**目的**：机器阻断“有 `pub struct` + `pub use`、却未注册进工厂”的控件。

**判据**：
1. 枚举 `src/widget/**/*.rs` 中所有 `pub struct <Name>`，`Name` 以 `Widget`/`View`/`Editor`/
   `Picker`/`Palette`/`Center`/`Input`/`Stack` 等控件后缀结尾（**白名单显式排除**内部类型：
   `*Item`、`*Level`、`*Entry`、`*Kind`、`*Config`、`*Builder`、`*State`、`*Marker`）；
2. 对每个候选，检查 `src/widget/capability/registration.rs` 是否存在对应的
   `register\(<snake_case_name>` 调用；
3. 未命中则失败，并打印缺失清单。

**必须反向注入验证**（原则 #19）：

| 注入方式 | 预期结果 |
|---|---|
| 临时注释掉 `registration.rs` 中 `self.register(breadcrumb_capability(), create_breadcrumb);` | 门禁 **FAIL**，并报 `breadcrumb` 缺失 |
| 恢复后 | 门禁 **PASS** |

**未通过反向注入的门禁不算门禁**——写入 §7 之前必须先跑这一步。

#### E-4：门禁落地后的双向断言

在 `check_capability_matrix_truthfulness.sh` 中增加**反向检查**：

- 每个已注册 `canonical_name` → 必须能解析回 `WidgetKind`（或显式登记为 alias-only）；
- 每个 `WidgetKind` → 必须显式登记为「已注册」或「故意未注册 + 理由」。

消除 §11.6 已登记的「单向断言」缺口。

### 12.4 Phase E 的可度量目标

| 指标 | 当前 | 目标 | 验证方式 | 实测 |
|---|---:|---:|---|---|
| 工厂 `canonical_name` 数 | **155** | **161**（+6）| `grep -c "self.register(" src/widget/capability/registration.rs` | ✅ **162**（+7：多找到 `grid_table`）|
| 6 个未接通控件（§12.1）| **6 未接通** | **0** | `check_widget_registration_fidelity.sh` | ✅ **0** |
| `WidgetKind` 变体 | **167** | **171**（+4）| `check_widget_kind_count.sh` | ⬜ 167（E-2 未做）|
| 新增控件有行为测试 | — | **4 / 4** | `check_behavior_matrix.sh` | ⬜ E-2 未做 |
| 注册保真门禁 | ❌ 不存在 | ✅ 且**反向注入已验证** | 手工注入 + 复跑 | ✅ 已验证（删别名→FAIL，加 kind→编译错误）|
| 每个 kind 的“可达 / 别名 / 基类”三态登记 | ❌ 无 | **167 个全覆盖** | 新门禁（§12.7 E-5）| ✅ **167/167 归类** |
| kind ↔ name 双向断言 | 单向 | **双向** | `check_capability_matrix_truthfulness.sh` | ✅ 双向（并因此抓出 18 个幻影属性）|

### 12.5 工作量与风险

| 任务 | 工作量 | 风险 |
|---|---|---|
| E-1 接通 6 个控件 | 🟢 小（约 1 轮，~150 行声明式代码）| 🟢 低：与 `breadcrumb` 同构，无新逻辑 |
| E-2 新建 4 个控件 | 🟡 中（约 2 轮）| 🟡 中：需 `draw_bridge` 分派 + 交互测试 |
| E-3 注册保真门禁 | 🟢 小（约 0.5 轮）| 🟡 中：候选后缀白名单可能误报，需迭代收敛 |
| E-4 双向断言 | 🟢 小 | 🟢 低 |

**最大风险**：E-3 的后缀启发式会误报（例如把 `ToastItem` 当控件）。
**缓解**：先跑出当前全量候选清单人工过一遍，把白名单写死并在脚本内注释理由。

### 12.6 本节结论

- **不需要新建 100 个控件**，也不需要把 C ABI 补齐到 167/167（§12.0）。
- 真缺口是 **6 个已实现但未接通的控件** + **4 个三框架交集的真空白控件**。
- 两者合计 **10 个工作项**，属 Phase E，**纯增量、可回退、与三条主线不冲突**。
- E-1 是**零新逻辑**的接线，应先做，能立刻提升 JSON/CSS 的可达面。

### 12.7 第三次更正：kind 与工厂名的关系是“多对一”，不是“一对一”

#### 错误复现（保留完整记录，原则 #64）

我在本节写作中用了三种判据，前两种都得出错的数字：

| 第几次 | 判据 | 结果 | 对错 |
|---|---|---|---|
| 1 | “`BaseWidget` 用了别人的 kind” | 29 个“隐身控件” | ❌ 搜错模式（带引号）|
| 2 | “`WidgetKind` 变体应有同名 `canonical_name`” | 35 / 27 / 24 个“缺口” | ❌ **忽略别名 kind** |
| 3 | “**是否能通过某名字或别名构造出控件**” | **§12.1 的 6 个** | ✅ |

#### 第 2 次错的证据（实跑）

```bash
$ grep -c "^pub type " src/widget/mod.rs
23
$ grep -n "^pub type Panel \|^pub type DockPanel \|^pub type CheckListBox " src/widget/mod.rs
182:pub type Panel = GroupBox;
187:pub type DockPanel = DockWidget;
412:pub type CheckListBox = ListBox;
```
✅ 已复跑

**项目已在源码里写明了这个设计**：

```
src/widget/mod.rs:412:  pub type CheckListBox = ListBox;
src/widget/capability/properties.rs:607:
    // `CheckListBox` — which is a *type alias for `ListBox`* — so the lookup for
    // `Chip` found nothing and the lookup for `CheckListBox` was ambiguous
```
✅ 已复跑

#### 24 个无同名注册的 kind 的完整三态分类

| 类别 | 数量 | 代表 | 为何不注册 |
|---|---:|---|---|
| **别名 kind**（`pub type`）| **15** | `Panel`→`GroupBox`、`DockPanel`→`DockWidget`、`CheckListBox`→`ListBox`、`ColumnView`→`TreeView`、`Dialog`→`PopupWindow`、`DoubleSpinBox`→`SpinBox`、`ActivityIndicator`→`ProgressBar`、`UndoView`→`ListView`、`DirectoryDialog`→`FileDialog`、`GridTable`→`GridTableWidget` | 本是同一控件的别名，注册会重复 |
| **基类 / 容器 kind** | **2** | `Frame`（`base_widgets/frame.rs`）、`Panel` | 仅作为父类供特化控件继承 |
| **子项 kind** | **1** | `MenuItem` | 由父 `Menu` 统一创建 |
| **已被特化名取代** | **2** | `ColorDialog`（注册名 `color_picker`）、`CupertinoSwitch`（注册名 `switch`）| 特化名已覆盖，待评估是否补别名 |
| **WebEngine 系列** | **11** | `WebEngineView`（注册名 `web_view`）、`WebEngineSettings` … | 11 个 kind 对应 1 个可选后端，非独立控件 |

**结论**：24 个中只有 **2 个**（`ColorDialog` / `CupertinoSwitch`）
**值得补别名**，其余 22 个是**有意的架构分类**，不应“补齐”。

#### E-5：把三态登记变成可门禁的产物

新增门禁 `tools/check_kind_reachability.sh`（与 E-3 合并或并列）：

**要求**：每个 `WidgetKind` 变体必须命中且仅命中以下三态之一，
且三态声明在源码中显式可查：

| 态 | 判据 | 要求 |
|---|---|---|
| `Registered` | 工厂有同名 `canonical_name` 或 `alias` | — |
| `AliasOf(other)` | 存在 `pub type X = Y;` 且 `Y` 为 `Registered` | 必须有 `pub type` 为证 |
| `BaseOrChild` | 注释标记 `// kind-role: base` / `child` | **需新增显式标记** |

**门禁逻辑**：对每个 kind，依次尝试上面三态；都未命中则 FAIL 并报 kind 名。

**反向注入验证**（原则 #19，与 E-3 同）：

| 注入 | 预期 |
|---|---|
| 删除 `pub type ColumnView = TreeView;` | FAIL，报 `ColumnView` 无法归类 |
| 新增 `enum Foo,` 到 `kind.rs` | FAIL，报 `Foo` 无法归类 |
| 恢复 | PASS |

> **为何要 `BaseOrChild` 显式标记**：当前“这是基类”只存在于开发者的头脑与注释里。
> 没有机器可读的标记，就无法区分“有意不注册”与“忘了注册”——这正是本轮
> 我连错两次的根本原因。

### 12.8 本节结论（终版）

| 项 | 数量 | 行动 |
|---|---:|---|
| 已实现但未接进工厂的特化控件 | **6** | **补注册**（E-1，零新逻辑）|
| 三框架交集且无实现 | **4** | **新建**（E-2，需交互测试）|
| 值得补别名 | **2** | 顺带做（`color_dialog`、`cupertino_switch`）|
| 别名 kind（有意不注册）| **15** | 不动，登记为 `AliasOf` |
| 基类 / 子项 kind | **3** | 不动，**补机器可读标记** |
| WebEngine 系列 kind | **11** | 不动，登记为自有后端 |

**最终一句话**：控件**不应增加数量**；要增加的是**“哪些能造出来”的确定性**。
