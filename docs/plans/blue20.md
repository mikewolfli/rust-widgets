# BLUE20 — 控件外形/属性/实现/事件的全量核验（五层普查 + SVG 快照）

> 状态：**未开始（执行计划）**
> 原则依据：[`docs/plans/principle.md`](principle.md)（继承 BLUE1–BLUE19 全部规则，含 #1–#101；
> 本文件新增 #102–#111）
> 上轮计划：[`docs/plans/blue19.md`](blue19.md)
> 相关日志：[`docs/log/log-20260921-1.md`](../log/log-20260921-1.md)（第 58 轮：本计划的**触发取证**）
>
> 本文件是**执行计划**，不是完成报告。
> **取证纪律（原则 #64）**：§一 的每条数字均在当前工作树上**实跑取得**，附命令；
> **未复跑的不写入**。

---

## 零、实现者入口（新对话从这里开始读）

### 0.1 一句话目标

**给 187 个控件补上整整缺失的一个验证维度——「渲染结果」**，
分**五层**依次落地：**渲染黄金表 → 声明与实现三向对齐 → SVG 全量快照
→ 跨层一致性与缺口收敛 → WebEngine 诚实降级**。

### 0.2 为什么需要这个计划（第 58 轮的教训）

第 58 轮修了 4 个用户**亲眼看见**的缺陷：

| # | 缺陷 | 用户怎么发现 |
|---|---|---|
| A | 窗口子树坐标空间不一致，**所有控件被平移出画布** | 「根本没有控件，就左右 2 块白板」 |
| B | `Window::draw` 边框覆盖整个客户区 | 「一塌糊涂，根本就没控件」 |
| C | 切换主题后旧配色不更新（`merge` 只能填 `None`） | 「图像变化，但 5 个控件本是没有变大变小」 |
| D | `list_box`/`scroll_area` 等归类为「背景色」→ 不可见 | 「控件根本没变化」 |

**四个缺陷全部躲过了当时已有的 64 个门禁。** 原因不是门禁不够多，
而是门禁**全都建立在「声明」上，没有一条建立在「渲染结果」上**（§1.3 量化）。

第 58 轮的取证又挖出两个同类缺口：

| # | 缺口 | 量化 |
|---|---|---|
| E | 主题声明了 4 个语义 token（error/warning/success/info），但控件层引用 **0 次** | §1.4 |
| F | 41 个平台控件工厂中 **1 个**名字查不到控件（`checkbox`） | §1.7 |

### 0.3 开工前必须做的三件事

1. **跑一遍基线取证**（§一），确认数字仍成立（防止工作树漂移）：
   `python3 tools/audit_kind_sharing.py`、`python3 tools/audit_appearance.py`、
   `python3 tools/audit_platform_create_coverage.py`。
2. **确认第 1 层要先冻结现状**（§2.4 的取舍）——不要一上来就断言「全部必须通过」，
   那会让 136 个存量文件同时红，失去收敛能力。
3. **读 §二 的约束**（特别是 §2.1：**必须按名字遍历，不能按 kind**；
   §2.1.1：**判据分类，不齐一**）。

> **所有待裁定项已清空**：D-WEB 已裁定 **W1**（§3.5.3）；
> `heatmap`/`spin_box` 浮点/不加控件清单均已裁定（§1.10）。
> 可以直接开工。

### 0.4 本计划的约束（不得违反）

- **不得**为了让门禁变绿而删控件、删断言、放宽判据（原则 #12–#15）。
- **不得**用「大概齐了」描述覆盖率——每层必须有**具体计数**（原则 #100）。
- 每个修复必须**实测可达**才改，改完**反向注入**验证（revert → 必须 FAIL，原则 #64）。
- 每轮不跑门禁/全量测试；**只在最后跑一次**（原则 #55/#56）。
- 所有可能阻塞的命令**必须设超时**（原则 #58）。

---

## 核心规则（继承 BLUE19 全部，含 #1–#101）

### BLUE20 新增规则

102. **🎨 外形必须可验证，不能只可声明** — 一个控件「有 `impl Draw`」只证明**接口存在**，
     不证明**画出了东西**。判据：对每个控件，存在一条断言，说明它渲染后的像素
     **至少有一个可区分于背景的颜色**。理由：第 58 轮 4 个缺陷全部满足「接口存在」，
     ComboBox 的 `Draw` 被调用、几何正确、样式已设置，而屏幕上什么都没有。

103. **🧾 渲染结果必须相对背景断言，不得断言字面量** — 「这个控件是蓝色」是主题的实现细节；
     「这个控件的填充**不等于**它所在的背景」是用户看得见的事实。判据：外形断言写成
     **相对**判据（≠背景 / 与背景对比度 ≥ N），字面量只出现在基线表里。
     理由：字面量会随主题改而失效，相对判据不会；且第 58 轮的缺陷恰恰是
     「主题给的色与窗口同色」，绝对断言在两种主题下都会漏。

104. **🌗 每个控件必须在两种外观下渲染，且结果必须不同** — 主题机制的意义是「换主题控件跟着变」。
     一个控件若在 light 与 dark 下渲染**逐像素相同**，则它要么硬编码了颜色，
     要么没读主题。判据：存在门禁，对每个控件渲染两次并断言其**主色不同**。
     理由：这一条断言**一条顶 1457 条**——第 58 轮实测 `impl Draw` 中硬编码颜色字面量
     **1457 处**，分布于 **136/181（75%）** 个文件（§1.3）。逐个改是体力活，
     而「两种外观必须不同」能在 179 个控件的规模上**自动**找到全部漏网点。

105. **🔁 遍历单位是「控件名」不是「WidgetKind」** — `WidgetKind` 有 **179** 个变体，
     而 capability 表有 **187** 条记录：**13 个 kind 被 2–5 个控件共用**
     （如 `ToolButton` 由 `split_button`/`tool_button` 共用）。判据：任何普查/门禁
     必须按 `canonical_name` 遍历。理由：按 kind 遍历会**静默漏掉 19 个控件**，
     而漏掉的部分不会报错——正是本计划要消灭的失效模式。

106. **📸 快照必须入库且带再生门禁** — SVG 快照的价值在于「改动会显示为 diff」。
     判据：快照提交入库，且存在门禁能**重新生成并逐字节对比**（与
     `check_generated_sources.sh`、`check_abi.sh` 第 [2] 步同构）。
     理由：不入库的快照是「跑一次看一眼」，无法发现**两天后**的退化。

107. **🧽 门禁不得把「跳过」报成「通过」** — 控件需要 surface 支持、
     需要主题注册、需要 feature gate；条件不满足时必须**明确报 SKIP 并计入计数**，
     不得静默 return 而计入 PASS。判据：门禁输出 `checked / skipped / failed` 三个数，
     且 `skipped` 非零时逐条列出原因。理由：第 58 轮实测一条门禁**假通过**——
     窗口建在 `(0,0)` 使偏移为 0，缺陷不可观测，还原后测试仍 PASS
     （见 `log-20260921-1.md` §3.2.1）。

108. **🎯 「必须主题化」是分类判断，不是一句口号** — 控件里的颜色必须逐处归入三类之一，
     且**每类有不同的判据**：① **外观色**（背景/文字/边框/圆角）**必须**读 `style.*`；
     ② **语义色**（error/warning/success/info）**必须**读 `theme.colors.*` 的对应 token；
     ③ **数据色**（图表系列、色板光谱、K 线涨跌、计量阈值）**必须不读** `style.*`，
     由数据/常量驱动。判定：存在门禁能逐条输出「这个控件的这个颜色属于哪一类」，
     且 ③ 类必须**登记在豁免白名单**里并附「为什么它是数据而非外观」的理由。
     理由：笼统要求「全部主题化」会把涨跌红绿改成主题色，**抹掉信息本身**；
     而笼统允许「硬编码」会让 136 个文件里那 1457 处漏读长期合法化。
     两者都是错的，且错的代价不对称——前者丢信息，后者丢一致。

109. **🩺 主题声明了语义 token，就必须有人读它** — `Theme::colors` 声明了
     `error`/`warning`/`success`/`info` 四个语义 token。若控件层对其引用为 **0**，
     则「主题支持语义色」是一句**空声明**：主题作者改了 `error`，屏幕上没有任何东西变化。
     判据：每个语义 token 至少被一个控件的 `Draw` 读取，且该控件在 dark 下的
     语义色 ≠ light 下的语义色。理由：这与「已发布事件无人发射」（BLUE19 规则 #97）
     是同一类缺陷——**声明与实际消费者脱节**，且都不可查询、不可检测。

110. **🔗 「名字存在」不等于「它是个控件」** — 跨层核对时，**必须先判定那个名字是什么东西**，
     再判定它缺不缺。平台层的 `create_*` 不全是控件工厂：
     `create_web_engine(&self) -> Option<Box<dyn NativeWebEngine>>` 返回的是**原生引擎句柄**
     （给 `src/web/` 用），不是控件；把它报成「缺了对应 capability」是**假阳性**。
     判据：一个平台 `create_*` 只有「接受 `width`/`height` **且** 返回 `ObjectId`」
     才是控件工厂（用 `width`/`height` 而非 `parent` 作为标志，因为窗口是根、没有父）。
     理由：本计划里同一个错误犯了**两次**——`context_menu`（是 `Menu` 的类型别名）
     与 `create_web_engine`（不是工厂），两次都只因为 `grep` 到的名字不在表里。

111. **🧵 跨层缺口先判「补」还是「删」，不得默认删** — 发现名字链路不完整时，
     必须逐条判定属于哪一类，**不得一律删除**：

     | 情形 | 处置 | 理由 |
     |---|---|---|
     | 控件本身完整，只是某个**拼写不可达** | ✅ **补别名** | 删除会破坏已发布 API（规则 #21）；补别名是纯加法 |
     | 名字背后**无 capability、无别名、无实现**（纯悬空） | ✅ **删** | 悬空名字比缺失更坏：它看起来能用 |
     | 名字**不是控件**（如 `create_web_engine`） | ❌ **不改代码**，改**判据** | 报它是假阳性 |

     判定判据：先问「删了它，现有调用方会不会静默失败？」
     会 → 不能删（应补）；不会且无人引用 → 可删。
     理由：删除是不可逆且破坏性的，而跨层缺口的**默认正确做法**往往是把链路接上
     （用户 2026-09-21 裁定：「有必要改成完整接线，而不是删除」）。

---

## 一、现状取证（第 58 轮实跑）

> 所有数字来自当前工作树实跑，命令附于各条。

### 1.1 规模

```bash
$ python3 tools/audit_kind_sharing.py
capability records (canonical_name): 187
kind occurrences: 187
distinct kinds: 168
```

| 量 | 值 | 说明 |
|---|---|---|
| `WidgetKind` 变体 | **179** | `src/widget/kind.rs` 的 enum body（实跑） |
| capability 记录 | **187** | `canonical_name:` 出现次数（`src/widget/capability/properties.rs`） |
| **distinct kinds（在 capability 表中出现过的）** | **168** | 187 条记录覆盖 168 个不同 kind |
| 共用 kind 的控件 | **19** | 13 个 kind 被 2–5 个控件共用 |
| `impl Draw` 文件 | **181** | 外形维度 |
| `impl WidgetProperties` 文件 | **181** | 属性维度 |
| 声明 `events:` 的记录 | **187** | 事件维度 |

```bash
$ grep -rln "impl Draw for" src/widget/ --include=*.rs | wc -l
181
$ grep -rln "impl WidgetProperties for" src/widget/ --include=*.rs | wc -l
181
$ grep -c "events:" src/widget/capability/properties.rs
187
```

### 1.2 共用 kind 的完整清单（必须按名字遍历的证据）

```bash
$ python3 tools/audit_kind_sharing.py
kinds shared by more than one control: 13
  Canvas:        ['map_view', 'canvas']
  Chart:         ['gantt_widget', 'timeline_widget', 'chart']
  DataView:      ['virtual_list', 'data_view']
  GroupBox:      ['group_box', 'panel']
  ListView:      ['list_view', 'command_palette', 'notification_center']
  PopupWindow:   ['toast_stack', 'popup_window']
  RichEdit:      ['code_editor', 'markdown_editor', 'rich_edit']
  StatusBar:     ['snackbar', 'status_bar']
  Table:         ['table_widget', 'table', 'data_grid', 'virtual_table', 'diff_viewer']
  TextEdit:      ['terminal_view', 'text_edit']
  ToggleButton:  ['segmented_control', 'toggle_button']
  ToolButton:    ['split_button', 'tool_button']
  WebEngineView: ['media_player', 'web_engine_view']

controls that a kind-only sweep would miss: 19
```

### 1.3 🔴 核心缺口：外形维度 75% 不受主题控制

```bash
$ python3 tools/audit_appearance.py
=== files with a Draw impl: 181 ===
=== total colour literals in Draw files: 1457 ===
=== Draw files reading no style colour, only literals: 136 (75%) ===
   52 literals  src/widget/special_widgets/code_editor/render.rs
   28 literals  src/widget/advanced_widgets/pie_menu.rs
   26 literals  src/widget/advanced_widgets/ribbon_bar.rs
   24 literals  src/widget/dialog/wizard.rs
   22 literals  src/widget/cupertino/core.rs
   ...

=== Draw files a test renders as pixels: 17 / 181 ===
```

| 量 | 值 | 占比 |
|---|---|---|
| `impl Draw` 文件 | 181 | 100% |
| **完全不读 style、只用硬编码颜色** | **136** | **75%** |
| 硬编码颜色字面量总数 | **1457** | — |
| 被**像素级测试**覆盖的 `impl Draw` 文件 | **17** | **9%** |

> **取证工具**：`tools/audit_appearance.py`（本计划新增，可复跑）。
> 两个数字的用途：136/181 是「有多少文件根本不可能响应主题」的规模，
> 1457 是「主题被忽略了多少次」的规模。
> 两者都被判据 P3（light ≠ dark）一条断言覆盖。

### 1.4 🔴 「主题声明了语义色，但没人读」——空声明

上一条解释了「哪些该主题化」。本条的取证说明：**语义色这一类已经被漏了**。

`Theme::colors` 声明了 10 个 token（`src/theme/types.rs:163`），
其中 4 个是语义色：`error` / `warning` / `success` / `info`。

```bash
$ for t in error warning success info; do \
    echo "colors.$t : $(grep -rn "colors\.$t\b" src/widget/ --include=*.rs | wc -l) 次"; done
colors.error   : 0 次
colors.warning : 0 次
colors.success : 0 次
colors.info    : 0 次
```

**在全部 181 个控件的 `Draw` 实现里，这 4 个 token 被引用 0 次。**

而 `WidgetRole` 有 7 个变体，其中只有 **`Danger` 一个是语义 role**，
它只把 `error` 用在 3 个 kind 上：

```rust
// src/theme/types.rs:154
"errordialog" | "trash" | "deletebutton" => Self::Danger,
```

⇒ `warning` / `success` / `info` **完全没有任何消费者**。

**具体后果**——以 `banner` 为例，它有 4 种严重度，全部硬编码：

```rust
// src/widget/overlay_widgets/banner.rs:91
fn background(self) -> Color {
    match self {
        BannerSeverity::Info    => Color::rgb(219, 229, 249),
        BannerSeverity::Success => Color::rgb(214, 239, 221),
        BannerSeverity::Warning => ... ,
        BannerSeverity::Error   => ... ,
    }
}
```

四种严重度说的是「info / success / warning / error」——
**正是主题的 4 个语义 token**，但控件用的是自己的字面量。
于是主题作者改 `colors.warning`，横幅的警告色**不会变**。

**这是与 BLUE19 规则 #97 同类的缺陷**：「已发布不等于会响」 vs
「已声明不等于会读」——都是**声明与实际消费者脱节**，且都不可查询。
新规则 #109 就是为它立的。

**涉及控件**（表达状态、但当前硬编码；括号内为 `Color::rgb/rgba` 计数）：

| 控件 | 字面量数 | 状态语义 |
|---|---|---|
| `banner` | 12 | info / success / warning / error（4 种，与 token 一一对应） |
| `progress_dialog` | 12 | 完成/进度绿 |
| `calendar` | 16 | 今天 / 周末 / 选中 |
| `message_box` | 16 | 信息/警告/错误图标 |

---

### 1.5 颜色的三分类（规则 #108 的依据）

「所有控件是否都要与主题相关」**没有单一答案**——控件里的颜色分三类，
且**每类的判据相反**：

| 类 | 例子 | 必须做什么 | 为什么 |
|---|---|---|---|
| ① **外观色** | 背景、文字、边框、圆角 | **必须**读 `style.*` | 这是「换主题控件跟着变」的全部内容；第 58 轮 4 个缺陷都在此 |
| ② **语义色** | 错误红、警告黄、成功绿、信息蓝 | **必须**读 `theme.colors.{error,warning,success,info}` | 主题**已经声明**了这 4 个 token（§1.6），不读就是空声明 |
| ③ **数据色** | 图表系列、色板光谱、K 线涨跌、计量阈值 | **必须不读** `style.*`，由数据/常量驱动 | 它们**编码信息**。改成主题色会抹掉信息本身——涨跌无法区分、图表系列无法辨认 |

**两句口号都是错的，且错的代价不对称**：

- 「全部必须主题化」⇒ 把涨跌红绿改成主题色，**丢掉信息**。
- 「允许硬编码」⇒ 让 136 个文件里那 1457 处漏读**长期合法化**。

**所以判据必须是分类的，不是齐一的**（规则 #108）。
判 ③ 类的实际操作要点：它**必须登记在豁免白名单**里，
并附「为什么它是数据而非外观」的理由——否则**「漏读了」与「故意的」无法区分**，
这正是 §2.4「先冻结现状」要解决的问题。

---

### 1.6 门禁的覆盖版图（为什么 64 个门禁没抓到）

```bash
$ ls tools/check_*.sh tools/check_*.py | wc -l
64
```

现有门禁按维度归类：

| 维度 | 代表门禁 | 覆盖 |
|---|---|---|
| **声明层** | `check_event_payload_types` / `check_control_has_tests` / `check_widget_kind_count` / `check_capability_matrix_truthfulness` / `check_capability_events_are_emitted` / `check_event_producers` / `check_widget_registration_fidelity` | ✅ 密集 |
| **结构层** | `check_control_route_matrix` / `check_platform_capability_matrix` / `check_capability_feature_gates` | 🟡 中等（核对路由表，不核对行为） |
| **渲染层** | **（无）** | ❌ **真空** |

**结论**：门禁再密也抓不到渲染缺陷——**不是数量不够，是缺整整一个维度**。

### 1.7 平台 `create_*` 与 capability 的覆盖（第 4 层的取证）

```bash
$ python3 tools/audit_platform_create_coverage.py
platform control factories: 41
non-factory create_* skipped: 1 ['web_engine']
capability records:        187
alias names:               191

UNRESOLVED (1): ['checkbox']
```

**41 个控件工厂中 40 个有对应**，靠三张表：capability 名字、`aliases` 字段、
别名映射表（`src/widget/capability.rs`）。真缺口只有 **1 个**。

#### ⚠️ 但最初脚本报的是 `UNRESOLVED (2)`——多报的那个**不是缺陷**

第一版脚本的判据是「平台层有没有一个同名的 `create_*` 而 capability 表里没有对应」。
这个判据会**误报**——因为平台层的 `create_*` 不一定都是**控件工厂**：

| 名字 | 签名 | 性质 | 该怎么做 |
|---|---|---|---|
| `create_checkbox` | `(parent, text, x, y, w, h) -> ObjectId` | ✅ **真缺口**——是控件工厂，但 `check_box` 的 `aliases: &[]` 是空的 | **补别名** |
| `create_web_engine` | `() -> Option<Box<dyn NativeWebEngine>>` | ❌ **不是控件**——无几何、不返回 `id`，是平台原生引擎句柄（给 `src/web/` 用） | **改脚本判据**，不碰代码 |

**判据的修正**：一个平台 `create_*` 只有「接受 `width`/`height` **且** 返回 `ObjectId`」才是控件工厂。
（用 `width`/`height` 而非 `parent` 作为标志，因为窗口是根、本身没有父：
`create_window(title, x, y, width, height) -> ObjectId` 也是工厂。）

`create_web_engine` 不满足，所以它**不应该**出现在未解析清单里。

> 这是本计划里**第二次**由「名字存在」推出错误结论（第一次是 `context_menu`，§1.9）。

### 1.8 🔴 WebEngine 现状：原生只做了 5%（**已裁定 W1：删除**）

> 用户提问（2026-09-21）：「webengine 是原来系统原生，有必要改回原生，
> 还是用 rust 的 servo 补全？」
> **→ 已裁定 W1（删除）：裁定结果与删除清单见 §3.5.3 / §3.5.4。**

#### 1.8.1 取证：现状**不是**「原来是原生的」

| 项 | 实测值 | 取证位置 |
|---|---|---|
| `NativeWebEngine` trait 方法数 | **6**（`load_url`/`load_html`/`go_back`/`go_forward`/`reload`/`stop_loading`） | `src/platform/types.rs:165-179` |
| 实现它的后端 | **1 个**（仅 Linux） | `grep -rn "fn create_web_engine" src/platform/` |
| WebKit 实现规模 | **76 行 / 7 个函数** | `src/platform/linux/webkit_engine.rs` |
| 实质内容 | 全是**一行转发**给 `webkit2gtk::WebView` | 同上 |
| **是否挂进窗口** | ❌ **没有**——只 `WebView::new()`，从不加进根布局 | `grep -rn "WebView\b" linux/canvas.rs linux/platform_impl.rs` → 空 |
| 其他平台 | ❌ 无实现（trait 默认返回 `None`） | `src/platform/types.rs:798` |

```bash
$ wc -l src/platform/linux/webkit_engine.rs
76 src/platform/linux/webkit_engine.rs

$ grep -rn "fn create_web_engine" src/platform/*/platform_impl.rs
src/platform/linux/platform_impl.rs:98:    fn create_web_engine(...)

$ grep -rn "WebView\b" src/platform/linux/canvas.rs src/platform/linux/platform_impl.rs
（空 —— 从未挂进窗口）
```

**结论**：「原生」这条路目前只走了 **5%**：引擎能创建、能 `load_uri`，
但**从未显示出来**，且**单平台**。

#### 1.8.2 关键区分：JS 求值与网页渲染是**两套独立路径**

这是个容易混淆的点，必须写清：

| 能力 | 走的路径 | 与 `NativeWebEngine` 的关系 |
|---|---|---|
| **JS 求值**（`evaluate_javascript`） | **boa_engine**（纯 Rust JS 引擎，`js-engine` feature） | ❌ **无关**——不经过 trait |
| **网页渲染 / 导航** | `NativeWebEngine` → WebKitGTK（仅 Linux） | ✅ 全部 **6 个 trait 方法**都是导航 |

```bash
$ grep -n "backend\." src/web/web_engine.rs
184: backend.load_url(url)      # 导航
204: backend.load_url(&url)
221: backend.load_html(...)
247: backend.go_back()
256: backend.go_forward()
266: backend.reload()
275: backend.stop_loading()
# → 7 个调用点全部是导航，没有一个是 JS 求值
```

**含义**：选 Servo **不会**改善 JS 能力（那部分已经是纯 Rust 的 boa）；
它只影响**网页渲染**。

#### 1.8.3 三个选项的真实代价

| 选项 | 工作量 | 代价 |
|---|---|---|
| **A. 补完系统原生** | 每平台 500–1500 行 × 3 平台；**先要让它显示** | 依赖系统库（WebKitGTK/WebView2/WKWebView）⇒ 破坏 `mini`/`embedded`；三套行为不一致；WebView2 要带 Edge Runtime |
| **B. 用 Servo** | **数千行** | Servo 是**浏览器**不是可嵌入库；上游**长期声明不提供稳定嵌入 API**；需 pin commit 或 fork；升级=重做。**本项目已用 boa 处理 JS**，说明此处无人选 Servo |
| **C. 诚实降级 + 可查询**（推荐） | 小 | 让「有没有真引擎」**可查询**；未启用时如实报「模拟」 |

#### 1.8.4 推荐：C（先诚实降级，再按需补）

**理由**：现在有一个**没人用、也没法用**的功能（76 行、不显示、单平台）。
在它之上加重投入（Servo 或三平台原生）都是**在未验证的地基上加码**。

**分三步**：

| 步 | 内容 | 性质 |
|---|---|---|
| **C1** | 补 `supports_web_engine()`（命名对齐已有的 `supports_surfaces()`） | 让能力可查询 |
| **C2** | `webkit-engine` 未启用时，`WebEngineView` 如实报「模拟」，而非静默走假进度 | 消除静默失效 |
| **C3** | 裁定「到底要不要显示网页」：<br>• 只要占位 ⇒ **删掉那 76 行**，只留模拟路径（规则 #111）<br>• 确实要显示 ⇒ 补完**一个**平台（Linux，已有 76 行基础），做到**能显示 + 能回传进度**，再谈其他 | 决策点 |
| **C4** | Servo **仅当**「嵌入式也要渲染网页 **且**不能依赖系统库」时才考虑 | 那时原生结构性不可能 |

> **C2 的现状是个真实缺陷**：`WebEngineViewEnhanced::new` 的文档写着
> 「Which of the two happened is not exposed, so a caller that must know should query
> the platform directly」——**降级不可查询**。这与 BLUE19 规则 #97
> （「已发布≠会响」）是同一类：**替代路径静默生效**。

---

### 1.9 曾被怀疑、但取证后确认**不缺**的（防止重复造）

> 本节的存在理由：一份「待加控件」清单若不去考证，很容易包含**已经有的东西**。
> 下面每一条都附了取证位置，避免后来者重新走一遍这段弯路。

| 候选 | 实际状况 | 取证位置 |
|---|---|---|
| `flex` / `flow_layout` / `wrap_layout` / `aspect_ratio` / `center` / `stack` / `form` 等 | **已有**——`src/layout/` 下 **18 个文件**（布局引擎与支持模块），通过 `set_layout` 使用。布局不是「控件」，不在 187 里是**正确的** | `ls src/layout/*.rs \| wc -l` = 18 |
| `gauge` | **已有**——就是 `meter`，且带阈值色带 + 指针（`MeterThreshold` 就是 tri-band） | `src/widget/display_widgets/meter.rs` |
| `password_edit` | **已有**——`line_edit` + `EchoMode::Password` | `set_echo_mode` |
| `alert_dialog` / `confirm_dialog` / `about_dialog` | **已有**——`message_box` 带 **15 种** `StandardButton`（Ok/Cancel/Yes/No/YesAll/NoAll/Save/Discard/Apply/Close/Abort/Retry/Ignore/Help） | `src/widget/dialog/message_box.rs` |
| `directory_dialog` | **已有**——`file_dialog` + `FileDialogMode::SelectDirectory` | `src/widget/dialog/file_dialog.rs` |
| `accordion` | **已有**——`collapsible_pane`（header + 箭头 + 折叠） | `src/widget/container_widgets/collapsible_pane.rs` |
| `tag` | **已有**——`chip` / `tag_input` | capability 表 |
| `clock` / `time_picker` | **已有**——`time_edit`（`time_picker` 是它的别名） | 别名表 |
| `context_menu` | **已有**——是 `Menu` 的**类型别名**，已完整接线（`WidgetKind::ContextMenu => "menu"` + 名字别名 `"context_menu" => "menu"`） | `capability.rs:327`、`capability.rs:387` |

> ⚠️ **本节的 `context_menu` 一条曾是错的**：第一稿把它记成了「真缺口」，
> 因为 `grep -c "ContextMenu" capability/properties.rs` = 0。
> 那个 grep 问的是「有没有自己的 capability 记录」，而它作为别名**不需要**有。
> 教训：**「名字不在表里」≠「功能不存在」**——先查别名表，再下结论（原则 #64）。

### 1.10 已确认**不需要**加的（用户 2026-09-21 裁定）

| 候选 | 裁定 |
|---|---|
| `gantt` / `timeline` | ❌ **不加**——已有 |
| `phone_input` / `email_input` | ❌ **不加**——避免控件爆炸，可用 `masked_edit` + 校验表达 |

---

### 1.11 已有基础设施（第 3 层不需新造轮子）

| 能力 | 位置 | 状态 |
|---|---|---|
| SVG 渲染（走真实 `Draw` 管线） | `src/widget/svg.rs` → `render_widget_to_svg()` | ✅ 已有，`SvgPaintBackend` 保证与真实渲染一致 |
| 像素渲染窗口树 | `src/widget/runtime.rs` → `render_frame_tree()` | ✅ 第 58 轮已修正确性 |
| 遍历子控件 | `src/widget/runtime.rs` → `children_of()` | ✅ 第 58 轮新增（public） |
| 主题切换 + 重应用 | `theme::global_theme_manager()` / `reapply_active_theme()` | ✅ 第 58 轮已修「切换后不更新」 |
| 快照目录 | `snapshots/` | ✅ 已存在（含 4 个旧 SVG） |
| 控件工厂 | `WidgetFactory::new_with_defaults()` / `.create(name, rect, text)` | ✅ 按**名字**创建 |

---

## 二、本计划的约束（不得违反）

### 2.1 遍历单位

**必须按 `canonical_name`（187 条）遍历，不能按 `WidgetKind`（179 个）。**
否则静默漏掉 19 个控件（§1.2），且漏掉不报错。

### 2.1.1 判据必须分类，不得齐一（规则 #108）

**不得**笼统要求「所有控件的所有颜色都必须主题化」，也**不得**笼统允许硬编码。
控件里的每一处颜色必须归入三类之一（§1.5），且**每类判据相反**：

| 类 | 判据 |
|---|---|
| ① 外观色 | **必须**读 `style.*`，dark ≠ light（P3） |
| ② 语义色 | **必须**读 `theme.colors.{error,warning,success,info}`，dark ≠ light（P4） |
| ③ 数据色 | **必须不读** `style.*`，由数据驱动；**登记在豁免白名单**（§3.1.3） |

理由：「全部主题化」会把涨跌红绿改成主题色，**抹掉信息**；
「允许硬编码」会让 1457 处漏读长期合法化。两者都错，且代价不对称。

### 2.1.2 豁免必须附理由，不得凭感觉

数据色豁免（③ 类）**必须**在 `tools/control_color_exemptions.txt` 中登记，
且每条写明**为什么这是数据而非外观**。

- 可接受：「K 线涨跌红/绿 —— 编码涨跌方向，换成主题色就分不清」
- **不可接受**：「暂时先跳过」「以后再改」「暂时看不出问题」

理由：豁免是「允许不响应主题」的许可。无理由的豁免与「漏读了」无法区分，
且一旦开了口子，「主题声明了语义 token 但 0 个消费者」就会被永久合法化（§3.1.3）。

### 2.2 判据写成相对形式

外形断言写成「**≠ 背景**」「**两种外观下不同**」，不写成「== `rgb(100,181,246)`」。
理由：字面量是主题实现细节，且第 58 轮的缺陷恰是「主题给的色与窗口同色」，
绝对判据在两种主题下都会漏（规则 #103）。

### 2.3 五层**依次**落地，不并行

用户明确要求「按步骤一层一层完成」。每层有独立 DoD，
**上一层 DoD 未达成不得开始下一层**（分层是为了让每层的结果可被单独验证，
并行会让「哪层有效」不可判定）。

### 2.4 存量的处理方式（关键取舍）

第 1 层落地时，**136 个文件里确实有硬编码颜色**，判据会立刻让大量控件红。
**处理方式**：

1. **先冻结现状**——门禁先产出基线表并**记录现状**（哪些控件当前「两种外观渲染相同」），
   基线表提交入库。
2. **再逐格收敛**——每修一个控件，基线表 diff 减少一行；门禁从「记录」转为「断言」，
   条件是**该控件的基线项已被清零**。
3. **禁止**一次性断言全部通过——那会让收敛进度不可见，也无法区分
   「新引入的退化」与「未处理的存量」。

---

## 三、五层执行计划

### 3.1 第 1 层：渲染黄金表（ROI 最高，覆盖全部 187）

#### 3.1.1 做什么

遍历全部 187 个控件名，对每个控件：

1. 用 `WidgetFactory::create(name, rect, "")` 建出来；
2. 在 **light** 主题下渲染，取像素统计；
3. 在 **dark** 主题下渲染，取像素统计；
4. 产出基线行：

```
name            非背景像素  light主色     dark主色      light≠dark  边框  文字像素
button          4509       100,181,246   33,150,243    yes          yes   271
combo_box       4680       69,69,69      180,180,180   yes          yes   155
```

#### 3.1.2 四条判据（对的、可失败的）

| # | 判据 | 抓什么 |
|---|---|---|
| P1 | **非背景像素 > 0** | 抓「完全不可见」（第 58 轮缺陷 A：控件被平移出画布） |
| P2 | **主色 ≠ 所在背景色** | 抓「有范围但看不见」（缺陷 D：`list_box` 与窗口同色） |
| P3 | **light 主色 ≠ dark 主色**（**外观色**，豁免见 §3.1.3） | 抓「不响应主题」（缺陷 C：切换后不更新；覆盖 136 个硬编码文件） |
| **P4** | **语义色 token 至少被一个控件读取，且 dark ≠ light** | 抓「主题声明了但没人读」的空声明（§1.4；规则 #109） |

> **P3 与 P4 的分工**（规则 #108）：
> P3 管**外观色**（背景/文字/边框）——它们必须读 `style.*`。
> P4 管**语义色**（error/warning/success/info）——它们必须读 `theme.colors.*`。
> 两者都要求「dark ≠ light」，但**读的是不同的字段**：
> 一个控件可以 P3 通过（背景随主题变）而 P4 失败（它的错误图标还是硬编码红）。
> `banner` 就是后者：它的填充已主题化（第 58 轮子代理改过），
> 但 4 种严重度的**状态色仍硬编码**（§1.4）。

#### 3.1.3 数据色豁免白名单（P3 的必要配套）

P3 会要求**所有**控件在 light/dark 下不同。但**数据色不该变**（规则 #108 ③）。
没有白名单，就分不清「漏读了」（缺陷）与「故意的」（数据色）
——这正是 §2.4 要防的。

| 产物 | 路径 | 格式 |
|---|---|---|
| 豁免表 | `tools/control_color_exemptions.txt` | 每行：`<控件名> <颜色用途> <理由>` |
| 门禁行为 | 同上 | 白名单内的控件：P3 降为 **INFO**（打印但不判失败）；白名单外：仍为**断言** |

**登记要求**：每条豁免必须写明**为什么这是数据而非外观**。
可接受的例子：「K 线涨跌红/绿 —— 编码涨跌方向，换成主题色就分不清」；
不可接受的例子：「暂时先跳过」「以后再改」。

**初始候选**（需逐条人工确认后登记，**不得直接照抄**）：

| 控件 | 颜色用途 | 应否豁免 |
|---|---|---|
| 图表系列（`chart`/`pie_chart`/`bar_chart`/`radar_chart`） | 系列标识色 | ✅ 数据 |
| `candlestick_chart` | 涨/跌 | ✅ 数据 |
| `color_picker` | 色相环 / 光谱 | ✅ 数据 |
| `meter` | 阈值区间 | ✅ 数据 |
| `banner` | 四种严重度 | ❌ **不豁免**——对应 `colors.{info,success,warning,error}`（§1.4） |
| `calendar` | 今天/周末/选中 | ❌ **不豁免**——应走语义 token（`info`/`error`）或 state override |
| `progress_dialog` | 完成绿 | ❌ **不豁免**——应走 `colors.success` |
| `message_box` | 信息/警告/错误图标 | ❌ **不豁免**——应走语义 token |

> 表里后 4 行的「不豁免」是规则的**可失败性**所在：若把 `banner` 也豁免掉，
> 那么「主题声明了 4 个语义 token 但 0 个消费者」就会**永久合法化**。

#### 3.1.4 产出

| 产物 | 路径 | 说明 |
|---|---|---|
| 门禁脚本 | `tools/check_control_rendering.sh`（+ `.py`） | 输出 `checked / skipped / failed` 三个计数（规则 #107） |
| 基线表 | `tools/control_rendering_baseline.txt` | 187 行，入库，改动显示为 diff |
| 豁免表 | `tools/control_color_exemptions.txt` | 数据色白名单，每条附理由（§3.1.3） |
| 语义色普查 | `tools/semantic_color_census.txt` | 4 个 token 各自被哪些控件读取（规则 #109） |
| 测试 | `tests/control_rendering_census_test.rs` | 断言 P1/P2/P3/P4，失败时逐条列出控件名与原因 |
| 探针（可复跑） | `examples/control_rendering_census.rs` | 人工查看用，打印完整表 |

#### 3.1.5 分步（不得跳步）

| 步 | 内容 | DoD |
|---|---|---|
| 1a | 实现普查探针，能对 187 个控件各渲染两次并打印像素统计 | 探针跑通，打印 **187 行**（不是 179） |
| 1b | 生成基线表并入库 | 表有 187 行；`skipped` 逐条列出原因 |
| 1c | 落地 P1（非背景像素 > 0）为**断言** | P1 失败项逐条列出；存量的处理见 §2.4 |
| 1d | 生成**数据色豁免表**并逐条人工确认理由（§3.1.3） | 每条豁免有理由；`banner`/`calendar` 等**不在其中** |
| 1e | 落地 P3（light ≠ dark）为**断言**，豁免表内降为 INFO | 先记录、后收敛；每个修复附反向注入 |
| 1f | 落地 P4（语义色 token 有消费者且 dark ≠ light）为**断言** | 4 个 token 逐个断言；反向注入 → FAIL |
| 1g | 落地 P2（主色 ≠ 背景）为**断言** | 同上 |
| 1h | 收尾——P1/P2/P3/P4 全部为断言且基线项清零 | 门禁 `failed=0`，`skipped` 逐条有理由 |

> **反向注入（每步必做）**：把对应判据的实现改回失效态 → 门禁必须 FAIL。
> 第 58 轮实测一条门禁**假通过**（窗口建在 `(0,0)` 使偏移为 0），
> 教训写在规则 #107。
> P4 的反向注入尤其重要：把 `banner` 的严重度改回字面量 → P4 必须 FAIL；
> 若改成豁免它，则**门禁失去了它的存在理由**。

### 3.2 第 2 层：声明与实现的三向对齐

#### 3.2.1 做什么

第 1 层管「画出来能不能看见」，第 2 层管「**声明的东西是不是真的存在**」。

| 维度 | 声明处 | 实现处 | 要断言什么 |
|---|---|---|---|
| **属性** | `PropertySchema { name, value_kind, readable, writable }` | `impl WidgetProperties for X` 的 `get`/`set` 分支 | 每个声明的 `name` 在实现里有分支；`readable=true` ⇒ `get` 不是恒返回默认值 |
| **事件** | `EventSchema { name, payload }` | 信号字段 + 发射点 | 已有 `check_event_payload_types` 覆盖，此处补「**信号字段真的存在于结构体**」 |
| **方法** | `commands` | 命令实现 | 每个声明的命令有实现分支 |
| **外形** | `impl Draw for X` 存在 | `draw()` 方法体 | 方法体含**实际绘制调用**（非空、非仅注释） |

#### 3.2.2 判据

| # | 判据 | 抓什么 |
|---|---|---|
| Q1 | 每个 `PropertySchema.name` 在 `WidgetProperties::get` 有对应分支 | 声明了但读不到 |
| Q2 | `draw()` 方法体含 ≥1 个 `context.*` 绘制调用 | **空 `Draw` 实现**（原则 #5 禁止的形态） |
| Q3 | 每个 `EventSchema.name` 在结构体里有同名字段 | 声明了但信号不存在 |

#### 3.2.3 产出

| 产物 | 路径 |
|---|---|
| 门禁 | `tools/check_declaration_implementation_alignment.sh`（+ `.py`） |
| 测试 | `tests/declaration_alignment_test.rs` |
| 计数表 | `tools/declaration_alignment_census.txt`（`checked / skipped / failed`） |

### 3.3 第 3 层：SVG 全量快照（用户指定）

#### 3.3.1 用户要求（原文照录）

> 「最后一部我要求在项目 snapshots 目录下创建一个 svg 文件夹，把所有控件输出为 svg，
> 文件名就是控件名」

#### 3.3.2 规格（固化，不得改动）

| 项 | 值 |
|---|---|
| 输出目录 | **`snapshots/svg/`** |
| 文件名 | **`<canonical_name>.svg`**（如 `button.svg`、`combo_box.svg`、`code_editor.svg`） |
| 数量 | **187**（按名字，不是 179——见规则 #105） |
| 生成 API | `rust_widgets::widget::svg::render_widget_to_svg(&mut widget, rect)` |
| 尺寸 | 统一默认几何（每条 capability 的 `default_geometry`，无则用一个固定标准尺寸） |
| 主题 | 至少 **dark**（demo 的默认外观）；**light 另存 `<name>.light.svg`** 作为对照 |

#### 3.3.3 为什么这层不可替代

前两层是**机器断言**，只能验证「有像素 / 有颜色差异」。
它们**无法**发现：比例怪、布局丑、元素叠在一起、着色配色难看、
图标画得不像——即「**看着不对**」。

SVG 快照把这 187 个控件变成**可逐个人眼复核的工件**，
且入库后带再生门禁 ⇒ 未来任何控件的视觉改动都会**显示为 diff**。

#### 3.3.4 产出

| 产物 | 路径 | 说明 |
|---|---|---|
| 生成器 | `tools/export_control_svgs.rs`（`[[example]]`） | 遍历 187 个名字，写 `snapshots/svg/*.svg` |
| SVG 快照 | `snapshots/svg/<name>.svg` × 187 | 入库 |
| 对照 | `snapshots/svg/<name>.light.svg` × 187 | 入库（可选，但推荐——见 §3.3.2 主题行） |
| 再生门禁 | `tools/check_svg_snapshots.sh` | 重新生成 → 逐字节对比（规则 #106） |
| 索引 | `snapshots/svg/README.md` | 说明目录用途、如何再生、如何新增控件时更新 |

#### 3.3.5 分步

| 步 | 内容 | DoD |
|---|---|---|
| 3a | 生成器写出 187 个 SVG 到 `snapshots/svg/` | `ls snapshots/svg/*.svg \| wc -l` = **187** |
| 3b | 每个 SVG 非空且含 ≥1 个绘制元素 | 不能只有 `<svg></svg>` 骨架 |
| 3c | 再生门禁：重新生成逐字节对比 | 注入一个改动 → 门禁 FAIL |
| 3d | 索引 README + 与 `check_generated_sources.sh` 同构的标记机制 | 产物带 `GENERATED_MARKER`，防止门禁把它当非生成物跳过 |

### 3.4 第 4 层：跨层一致性与缺口收敛

> **为什么它是「层」而不是「收尾」**：它同样是一条**可失败、可取证**的完整判据链，
> 且与前三层正交——前三层管「已有的对不对」，本层管「该有的有没有、名字通不通」。

#### 3.4.1 做什么

| 任务 | 内容 | 依据 |
|---|---|---|
| **L1** | 修 **1 条**真别名缺口：`check_box` 加 `"checkbox"` | §1.7（实测 `UNRESOLVED` 中只有 `checkbox` 是真缺陷） |
| **L1b** | **修正脚本判据**：非控件工厂的 `create_*`（如 `create_web_engine`）应被排除，而非报为未解析 | §1.7——判据要「返回 `ObjectId` **且** 接受几何参数」 |
| **L2** | 把 `tools/audit_platform_create_coverage.py` 变成**门禁** | 现在它 exit=1，但不在 `run_all_gates.sh` 覆盖范围内 |
| **L3** | 新增控件 **`heatmap`**（二维矩阵可视化） | 图表家族缺口（用户已确认） |
| **L4** | `spin_box` 浮点支持（`set_decimals(n)`），**不**新增 `double_spin_box` | §1.7：`double_spin_box` 已是 `spin_box` 的别名；避免两个几乎相同的控件 |

#### 3.4.2 分步

| 步 | 内容 | DoD |
|---|---|---|
| 4a | 补 1 条真别名（`checkbox`）；同时修正脚本判据以排除非控件工厂 | `audit_platform_create_coverage.py` → exit 0；`create_web_engine` **不再**出现在未解析清单 |
| 4b | 把上一步的脚本接入 `tools/check_platform_create_coverage.sh` 并入 `run_all_gates.sh` | 反向注入（删一条别名）→ 门禁 FAIL |
| 4c | `spin_box` 加 `set_decimals(n)` + 浮点格式化 | 既有整数测试不回退；浮点往返测试逐个通过 |
| 4d | 新增 `heatmap` 控件 | 入 capability 表（187 → 188）；第 1 层普查覆盖它；第 3 层为它出 SVG |
| 4e | 新增控件的**设计器就绪**检查 | capability + 属性 + 事件 三者齐全（不能是空壳） |

#### 3.4.3 约束（本层特有）

- **4c 先于 4d**：改已有控件的 API 比新增控件风险高（规则 #21 向前兼容），
  先把风险高的做完并验证，再做新东西。
- **4d 必须全链完整**：新增一个控件不是「写一个 `impl Draw`」，而是
  capability 记录 + `WidgetKind`（若有新 kind）+ 构造函数 + 属性表 + 事件表 +
  平台层 `create_*` + 三端构建。**半接线的控件比没有更坏**（§1.7 的 `create_checkbox`
  就是个例子：控件完全可用，但一个拼写就找不到它）。
- **删 vs 补的判据**（规则 #111）：名字背后若无 capability / 无别名 / 无实现，才是**纯悬空**，
  应当删；若控件本身完整、只是某个拼写不可达，应当**补别名**
  （删除会破坏已发布 API，违反规则 #21）。
  若名字**不是控件**（如 `create_web_engine`），则**不改代码、改判据**（规则 #110）。
  本轮未发现纯悬空的例子。
- **4e 不得用「大概齐了」**：新控件的 4 项（外形/属性/事件/构建）必须逐项有测试。

### 3.5 第 5 层：WebEngine 诚实降级

> 取证与三个选项的代价对比见 **§1.8**。本节是可执行的分步。

#### 3.5.1 做什么

| 任务 | 内容 | 性质 |
|---|---|---|
| **C1** | 补 `supports_web_engine()`（命名对齐已有的 `supports_surfaces()`） | 让能力可查询 |
| **C2** | 未启用 `webkit-engine` 时，`WebEngineView` 对外如实报「模拟」 | 消除静默降级 |
| **C3** | 修正「降级不可查询」的文档 | 文档与实现一致 |
| **C4** | ✅ **已裁定 W1**：删掉那 76 行 WebKit 包装，只留模拟路径 | 用户 2026-09-21 裁定 |
| **C5** | Servo 评估 —— **不再需要**（W1 下不涉及渲染引擎） | — |

#### 3.5.2 分步

| 步 | 内容 | DoD |
|---|---|---|
| 5a | 新增 `supports_web_engine() -> bool`，实现读 `platform_facts().create_web_engine().is_some()` | 函数存在；有真引擎报 `true`，其余报 `false` |
| 5b | `WebEngineView` 暴露「当前是真引擎还是模拟」 | 新增查询可区分二者；反向注入（改成恒 `true`）→ FAIL |
| 5c | 修正 `WebEngineViewEnhanced::new` 的文档（现写着「不暴露给调用方」） | 文档不再声称「不可查询」 |
| **5d** | ✅ **执行 W1**：删除 WebKit 包装，只留模拟路径（精确清单见 §3.5.4） | 删除后 `full` / `desktop` / `webkit-engine` 各构建均 0 error；模拟路径行为不变 |

#### 3.5.4 ✅ W1 已裁定（2026-09-21）：删除范围（精确清单）

> 裁定理由（用户原话）：「没有 ⇒ W1（删掉那 76 行，更诚实——符合规则 #111）」。
> 即：**没有真实用户会在本库里看到一个网页** ⇒ 删掉那条**从未显示过**的代码。

**删除范围**（已逐处取证，引用面共 6 处）：

| # | 文件 | 动作 | 依据 |
|---|---|---|---|
| 1 | `src/platform/linux/webkit_engine.rs` | **删整文件**（77 行） | 唯一实现处 |
| 2 | `src/platform/linux/mod.rs:39` | 删 `pub(crate) mod webkit_engine;` 及上方注释 | 模块声明 |
| 3 | `src/platform/linux/platform_impl.rs:88-101` | 删 `create_web_engine` 实现（落到 trait 默认 `None`） | 唯一实现 |
| 4 | `src/platform/types.rs:165-179` | **删 `NativeWebEngine` trait**（6 个方法） | 无实现则无意义 |
| 5 | `src/platform/types.rs:798` | 删 `Platform::create_web_engine` 声明 | 同上 |
| 6 | `src/web/web_engine.rs:18,28,39,55,61` | 删 `webkit_backend` 字段、`use`、构造处与 6 个转发分支 | 消费者 |
| 7 | `Cargo.toml:295` | 删 `webkit-engine = ["dep:webkit2gtk"]` | 无引用 |
| 8 | `Cargo.toml:370` | 从 `full` 列表移除 `"webkit-engine"` | 同上 |
| 9 | `Cargo.toml:517` | 删 `webkit2gtk = { version = "2.0", optional = true }` | 无引用 |
| 10 | `Cargo.toml:70-74, 290` | 修正注释（不再提 `webkit-engine`） | 文档一致（原则 #18） |

**必须保留**（删了会破坏功能或 API）：

| 保留项 | 理由 |
|---|---|
| `simple_engine` 模拟路径（`WebEngineViewEnhanced` 本体） | 这是 `web_engine_view` **控件本身**，capability 记录在 187 里，有属性/事件 |
| `evaluate_javascript`（boa，`js-engine` feature） | **与渲染无关**（§1.8.2），是独立的、可用的 JS 能力 |
| `src/web/` 其余模块（history/navigation/privacy/plugins…） | 模拟路径仍需要它们 |
| capability 记录 `web_engine_view` | 控件的声明面（属性/事件/命令），与渲染引擎无关 |

**验收（必须贴输出）**：

```bash
cargo check --features desktop                             # 0 error
cargo check --no-default-features --features "desktop,full" # 含 full 的构建也要过
cargo check --no-default-features --features mini           # 不受影响
cargo test --features desktop --lib web                      # 模拟路径测试全绿
bash tools/check_profiles.sh                                 # exit=0
```

**反向验证（规则 #19）**：删除后应能用 `grep` 证明**零残留**：

```bash
grep -rn "NativeWebEngine\|webkit_engine\|webkit2gtk\|webkit-engine" src/ Cargo.toml
# → 必须无输出
```

> **为什么“删”是对的**（规则 #111）：它属于「名字背后无实现」以外的第三种情况——
> **有 76 行实现，但从未接入窗口**，因此**从未产生任何可观测行为**。
> 保留它只会让读者以为「本库支持网页渲染」，而实际上不会。
> 这与 BLUE19 #97「已发布≠会响」同类：**声明的能力与真实的消费者脱节**。

#### 3.5.3 ✅ 裁定结果（2026-09-21）

> **D-WEB：`WebEngineView` 是否需要真的显示网页？**
> **→ 已裁定：W1（删掉那 76 行）**

裁定经过（保留选项对比，因为 `W2` 将来仍可能重启）：

| 选项 | 后果 | 工作量 | 裁定 |
|---|---|---|---|
| **W1：只做占位** | 删掉 76 行 WebKit 包装，只留模拟路径；capability 如实标注 | 小（删代码） | ✅ **选此** |
| **W2：要真显示** | 补完**一个**平台（Linux），做到能显示 + 进度回传；再谈其他 | 中（数百行） | 未选 |
| **W3：三平台原生** | Linux + Windows(WebView2) + macOS(WKWebView) | 大（数千行 + 系统库依赖） | ❌ 不推荐 |
| **W4：Servo** | 纯 Rust，但需 pin commit / fork，且上游无稳定嵌入 API | 很大 | ❌ 不推荐 |

**裁定判据**（问自己一句）：**「有没有一个真实用户会在本库里看到一个网页？」**

- **没有 ⇒ W1** ✅ → 本次裁定
- 有，且只需要桌面 ⇒ W2
- 有，且嵌入式也要 ⇒ 才考虑 W4（因为那时原生结构性不可能）

不选 W3/W4 的理由：

1. **现状不可用**（§1.8.1：76 行、不显示、单平台）——在未验证的地基上加码是错的顺序。
2. **JS 不需要 Servo**（§1.8.2）：JS 求值已走 boa（纯 Rust），Servo 只影响**渲染**。
3. **W3/W4 与项目定位冲突**：系统库依赖会破坏 `mini`/`embedded`；
   Servo 是**浏览器**不是可嵌入库（上游长期声明不提供稳定嵌入 API）。

---

## 四、验证矩阵

### 4.1 五层计数（必须逐层给出具体数字，规则 #100）

| 层 | checked | skipped | failed | 判据 |
|---|---|---|---|---|
| 第 1 层 渲染黄金表 | 187 | 逐条列因 | 0（收敛后） | P1 / P2 / P3 / **P4** |
| 第 1 层 数据色豁免表 | 每条豁免 | — | 0（无理由的条目） | 每条必须附「为什么是数据而非外观」 |
| 第 1 层 语义色普查 | 4 个 token | — | 0（收敛后） | 每个 token 被 ≥1 个控件读取（规则 #109） |
| 第 2 层 声明对齐 | 187 | 逐条列因 | 0（收敛后） | Q1 / Q2 / Q3 |
| 第 3 层 SVG 快照 | 187 | — | 0 | 文件在、非空、可再生 |
| 第 4 层 平台控件工厂覆盖 | 41 | — | 0 | 每个必须有对应 capability 或别名 |
| 第 4 层 新增控件（`heatmap`） | 1 | — | 0 | capability + 属性 + 事件 + 三端构建，逐项有测试 |
| 第 5 层 WebEngine 可查询性 | 1 | — | 0 | `supports_web_engine()` 能区分真引擎与模拟 |

### 4.2 反向注入（每层必做，实测 FAIL）

| 层 | 注入什么 | 期望 |
|---|---|---|
| 1 | 把某控件的填充改回与背景同色 | P2 FAIL |
| 1 | 把某控件的颜色改回硬编码字面量 | P3 FAIL |
| 1 | 把窗口建在 `(0,0)`（重现第 58 轮的假通过） | 门禁必须**仍然**能抓到 —— 这条是本计划对 #107 的兑现 |
| 1 | **把 `banner` 的严重度改回硬编码字面量** | **P4 FAIL**（§1.4 的初始状态，这条必须真能抓住） |
| 1 | **把 `banner` 加入豁免表** | 应**被拒绝**：豁免表不接受语义色控件（§3.1.3） |
| 1 | 把某数据色控件（如 `candlestick_chart`）从豁免表移除 | P3 应**报出它**（证明豁免表真的在起作用） |
| 2 | 清空某个 `draw()` 方法体 | Q2 FAIL |
| 2 | 删掉一个 `get` 分支 | Q1 FAIL |
| 3 | 改动一个 SVG 一个字节 | 再生门禁 FAIL |
| 4 | 删掉一条别名（如 `"checkbox"`） | 平台覆盖门禁 FAIL |
| 4 | 新增控件只写 `impl Draw`、不写 capability | 就绪门禁 FAIL（不得是空壳） |
| 5 | 把 `supports_web_engine()` 改成恒 `true` | 可查询性测试 FAIL |

### 4.3 多平台

**五层**都必须在 **5 个 profile** 下可运行或如实 SKIP：

```
desktop / tablet / mobile / mini / embedded
```

- 第 1、2 层：`mini`/`embedded` 无控件工厂 ⇒ **明确 SKIP 并计入 skipped 计数**（规则 #107）。
- 第 3 层：SVG 生成器需 `full_widgets`；`mini`/`embedded` 下如实报告不可用。
- 第 4 层：新增控件必须按规则 #20 在 **desktop/tablet/mobile** 三端均可构建；
  `mini`/`embedded` 下若被门控掉，必须**在 capability 表里如实反映**（而非只在一端存在）。
- 第 5 层：`supports_web_engine()` 在**每个** profile 都要能回答
  （`mini`/`embedded` 报 `false` 是正确答案，不是 SKIP）。

---

## 五、明确**不做**的（附理由）

| # | 不做 | 理由 |
|---|---|---|
| 1 | 不试图一次性修完 136 个硬编码文件 | 收敛需要过程；一次性改动无法区分「新退化」与「存量」，且违反 §2.4 的取舍 |
| 2 | **不笼统地「全部主题化」** | 控件里的颜色分三类（§1.5），判据相反。把数据色（图表系列、K 线涨跌、色板光谱）改成主题色会**抹掉信息本身**（规则 #108） |
| 3 | **不笼统地「允许硬编码」** | 那会让 1457 处漏读长期合法化，且「主题声明了 4 个语义 token 但 0 个消费者」永远无法被发现 |
| 4 | **不把语义色控件（`banner` 等）加入豁免表** | 它们的 4 种严重度**就是**主题的 `info`/`success`/`warning`/`error`（§1.4）。豁免它们 = 永久关掉规则 #109 |
| 5 | 不引入新的主题字段来「解决」硬编码 | 绝大多数情况是控件**没读**已有的 `background_color`/`text_color`/`border_color`（外观）或 `colors.*`（语义），不是字段不够。加字段会掩盖真因 |
| 6 | 不用 `WidgetKind` 作为遍历单位 | 会漏 19 个控件（规则 #105） |
| 7 | 不把 SVG 快照做成「跑一次看一眼」 | 不入库 + 无再生门禁 = 无法发现两天后的退化（规则 #106） |
| 8 | **不补完三平台原生网页引擎，也不用 Servo**（D-WEB = W1） | 现状（76 行、从未显示、单平台）不可用，在未验证的地基上加码是错的顺序；Servo 是浏览器不是可嵌入库（§3.5.3） |
| 9 | **不删模拟路径与 boa JS 引擎** | 它们是 `web_engine_view` 控件本体与独立可用的 JS 能力，与渲染引擎无关（§1.8.2、§3.5.4） |

---

## 六、优先级建议

```
第 1 层（渲染黄金表）
   P1 非背景像素 > 0     ← 最先做：判据最硬、存量违规最少、能立刻抓「不可见」
   豁免表（数据色）        ← 紧接 P1：没有白名单，P3 就无法区分「漏读」与「故意」
   P3 light ≠ dark       ← 其次：一条断言覆盖 1457 处字面量
   P4 语义色有消费者       ← 再其次：4 个 token 当前 0 消费者（§1.4）
   P2 主色 ≠ 背景         ← 最后：与 P3 部分重叠，可复用同一份渲染数据
        │
        ▼
第 2 层（声明与实现三向对齐）
   Q2 draw() 非空        ← 先做：最接近第 1 层，机械可判
   Q1 属性分支齐全        ← 其次
   Q3 事件字段存在        ← 最后：与既有 check_event_payload_types 有重叠，需明确分工
        │
        ▼
第 3 层（SVG 全量快照）
   3a 生成 187 个 → 3b 非空断言 → 3c 再生门禁 → 3d 索引与标记
        │
        ▼
第 4 层（跨层一致性与缺口收敛）
   4a 补 1 条别名        ← 最先：最小改动、有现成脚本能证伪
   4b 别名脚本进门禁      ← 其次：把一次性修复变成防复发
   4c spin_box 浮点      ← 再次：改已有 API（风险高，先做）
   4d 新增 heatmap       ← 最后：新增控件，必须全链完整（§3.4.3）
   4e 新控件就绪检查
        │
        ▼
第 5 层（WebEngine 诚实降级 — W1）
   5a supports_web_engine()   ← 最先：纯加法，风险最低
   5b “真引擎/模拟”可查询     ← 其次：消除静默降级
   5c 修正文档               ← 成本极低
   5d 删 WebKit 包装（10 项） ← 按 §3.5.4；删除后必须零残留
   5e 全链构建验证           ← desktop / full / mini
```

**分层理由**：前三层是「修正已有的」（可能大量改动），
第 4 层是「补该有的」（改动小而需谨慎）——放在**后面做**是因为
新增控件若在前三层之前落地，会**沿用同样的缺陷模式**（不读主题、无像素测试），
等于把待修的文件从 136 个变成 137 个。

**最小可交付切片**：**第 1 层 P1 + 豁免表 + 第 4 层 4a/4b + 第 5 层 5a–5c + 第 3 层 3a**

> 理由：P1 判据最硬（「有没有画出东西」没有解释空间）；
> 豁免表是 P3 的前置（否则无法分辨存量中的「漏读」与「数据色」）；
> **4a/4b 只需改 2 行 + 接一个现成脚本**（ROI 最高，且能立刻把
> 「平台层自己的名字反而查不到」这类跨层缺口变成防复发）；
> **5a–5c 全是纯加法且成本极低**（新增两个查询 + 改一段文档），
> 却能把「降级不可查询」这个静默失效变可检测；
> 第 3 层能立刻给用户一张可人眼复核的全量图——
> 五者结合能同时回答「有没有不可见的控件」「名字通不通」「网页到底渲没渲」，
> 正是第 58 轮用户实际提出的问题。

---

## 附录：完整任务登记表（Task Register）

> **状态图例**：⬜ 未开始 · 🟡 进行中 · ✅ 已完成 · ⛔ 阻塞

### A. 第 1 层 — 渲染黄金表

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-1a** | 普查探针：187 个控件各渲染 light/dark 两次并打印像素统计 | 打印 **187 行**（不是 179） | ⬜ |
| **R-1b** | 生成并入库基线表 | `tools/control_rendering_baseline.txt` 有 187 行 | ⬜ |
| **R-1c** | 判据 P1（非背景像素 > 0）落地为断言 | 反向注入 → FAIL | ⬜ |
| **R-1d** | 生成**数据色豁免表**，逐条人工确认理由（§3.1.3） | 每条有理由；`banner`/`calendar`/`progress_dialog`/`message_box` **不在其中** | ⬜ |
| **R-1e** | 判据 P3（light ≠ dark，外观色）落地为断言，豁免表内降为 INFO | 反向注入 → FAIL | ⬜ |
| **R-1f** | 判据 **P4**（语义色 token 有消费者且 dark ≠ light）落地为断言 | 4 个 token 逐个断言；反向注入 → FAIL | ⬜ |
| **R-1g** | 判据 P2（主色 ≠ 背景）落地为断言 | 反向注入 → FAIL | ⬜ |
| **R-1h** | 逐格收敛至 `failed=0`，`skipped` 逐条列因 | 门禁 PASS | ⬜ |
| **R-1i** | 🔴 **修完第 1 层抓到的所有控件外形缺陷** | 基线表逐项清零 | ⬜ |
| **R-1j** | 🔴 **给 4 个语义 token 接上消费者**（先 `banner`，再 `calendar` / `progress_dialog` / `message_box`） | 每个 token 被 ≥1 个控件读取；dark ≠ light | ⬜ |

### B. 第 2 层 — 声明与实现三向对齐

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-2a** | 判据 Q2：`draw()` 方法体非空（含实际绘制调用） | 反向注入 → FAIL | ⬜ |
| **R-2b** | 判据 Q1：每个 `PropertySchema.name` 在 `get` 有分支 | 反向注入 → FAIL | ⬜ |
| **R-2c** | 判据 Q3：每个 `EventSchema.name` 在结构体有同名字段 | 反向注入 → FAIL | ⬜ |
| **R-2d** | 与 `check_event_payload_types` 的职责边界写入文档（原则 #101） | 文档 + 门禁互不重叠 | ⬜ |
| **R-2e** | 🔴 **修完第 2 层抓到的所有对齐缺陷** | 计数表 `failed=0` | ⬜ |

### C. 第 3 层 — SVG 全量快照（用户指定）

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-3a** | 生成器写出 `snapshots/svg/<name>.svg` × 187 | `ls snapshots/svg/*.svg \| wc -l` = **187** | ⬜ |
| **R-3b** | 每个 SVG 非空且含 ≥1 绘制元素 | 断言文件含 `<svg` 之外的绘制标签 | ⬜ |
| **R-3c** | 再生门禁 `check_svg_snapshots.sh`（重新生成逐字节对比） | 注入一字节改动 → FAIL | ⬜ |
| **R-3d** | `snapshots/svg/README.md` + `GENERATED_MARKER` | 门禁不把生成物当非生成物跳过 | ⬜ |
| **R-3e** | light 对照快照 `<name>.light.svg`（可选但推荐） | 存在则一并纳入再生门禁 | ⬜ |

### D. 第 4 层 — 跨层一致性与缺口收敛

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-4a** | 补真别名：`check_box` 加 `"checkbox"`；修正脚本判据排除非控件工厂 | `create_web_engine` 不再报未解析；`checkbox` 可创建 | ⬜ |
| **R-4b** | 把该脚本接入 `tools/check_platform_create_coverage.sh` 并登记进 `run_all_gates.sh` | 反向注入（删一条别名）→ 门禁 FAIL | ⬜ |
| **R-4c** | `spin_box` 浮点支持（`set_decimals(n)` + 浮点格式化） | 既有整数测试不回退；浮点往返测试通过；**不**新增 `double_spin_box` | ⬜ |
| **R-4d** | 新增 `heatmap` 控件（全链：capability + 构造 + 属性 + 事件 + 平台 + 三端构建） | capability 187 → 188；第 1 层普查覆盖它；第 3 层为它出 SVG | ⬜ |
| **R-4e** | 新增控件的**设计器就绪**检查（外形/属性/事件/构建 四项逐项有测试） | 四项逐项有证据，不得用「大概齐了」 | ⬜ |

### E. 第 5 层 — WebEngine 诚实降级（§1.8，**已裁定 W1**）

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-5a** | 补 `supports_web_engine()`（命名对齐 `supports_surfaces()`） | 函数存在；W1 删除后应**恒为 `false`**（无引擎） | ⬜ |
| **R-5b** | `WebEngineView` 对外如实报「模拟」 | 新增查询可区分「真引擎」与「模拟」；反向注入 → 测试 FAIL | ⬜ |
| **R-5c** | 修正「降级不可查询」的文档 | 文档与实现一致（原则 #18） | ⬜ |
| **R-5d** | ✅ **执行 W1**：按 §3.5.4 的 10 项清单删除 WebKit 包装 | `grep -rn "NativeWebEngine\|webkit_engine\|webkit2gtk\|webkit-engine" src/ Cargo.toml` → **无输出** | ⬜ |
| **R-5e** | 删除后全链构建验证 | `desktop` / `full` / `mini` 均 0 error；`check_profiles.sh` exit=0 | ⬜ |
| **R-5f** | 文档同步：§1.8、cookbook、README 中关于 webkit 的描述 | 无任何文档再声称「支持原生网页渲染」 | ⬜ |

### F. 纪律（每轮遵守）

| # | 纪律 | 依据 |
|---|---|---|
| **R-4** | 每个判据必须**反向注入**验证 | 原则 #64、本文件 #107 |
| **R-5** | 每轮不跑门禁、不跑全量测试；**只在最后跑一次** | 原则 #55/#56 |
| **R-6** | 所有可能阻塞的命令**必须设超时** | 原则 #58 |
| **R-7** | 五层计数必须打印具体数字，`skipped` 逐条列因 | 原则 #100、本文件 #107 |

### G. 文档与日志

| # | 任务 | 验证判据 | 状态 |
|---|---|---|---|
| **R-8** | 每层完成后写 `docs/log/`，逐条标识已修项 | 日志含「已修清单 + 验证证据」 | ⬜ |
| **R-9** | 新门禁登记进 `tools/run_all_gates.sh` 的覆盖范围 | `run_all_gates.sh` 计数增加 | ⬜ |
| **R-10** | 版本与文档同步 | `check_changelog_sync.sh` PASS | ⬜ |

---

## 附录二：每层的「完成定义」（Definition of Done）

> 判定「完成」时必须能贴出**实跑输出**，不接受形容词。

### DoD-R-1（渲染黄金表）

- [ ] 普查覆盖 **187** 个控件名（按名字遍历，非按 kind），`checked=187`。
- [ ] 基线表入库，187 行，每行含：非背景像素、light 主色、dark 主色、边框有无、文字像素。
- [ ] **四条**判据（P1/P2/P3/P4）各自是一条**断言**，且各自有一条**反向注入 FAIL** 的实测记录。
- [ ] **数据色豁免表**入库，每条附「为什么是数据而非外观」的理由；
      且 `banner` / `calendar` / `progress_dialog` / `message_box` **不在豁免表内**。
- [ ] **语义色普查**：`error` / `warning` / `success` / `info` 每个 token 至少被一个控件的
      `Draw` 读取，且 dark ≠ light（规则 #109）。
- [ ] **第 58 轮的假通过场景被专门覆盖**：门禁在窗口位于非零坐标时仍能抓到
      「控件不可见」（即不依赖 `(0,0)` 这个偶然条件）。
- [ ] 第 1 层抓到的控件外形缺陷**全部修复**，基线表逐项清零，`failed=0`。
- [ ] `skipped` 非零时逐条列出**原因**（哪个 profile / 缺什么能力）。

### DoD-R-2（声明与实现三向对齐）

- [ ] 三条判据 Q1/Q2/Q3 各自是一条断言 + 一条反向注入 FAIL。
- [ ] 计数表 `checked / skipped / failed` 三个数齐全。
- [ ] 与 `check_event_payload_types` 的职责边界写入模块文档（原则 #101）。
- [ ] 抓到的对齐缺陷**全部修复**，`failed=0`。

### DoD-R-3（SVG 全量快照）

- [ ] `ls snapshots/svg/*.svg | wc -l` = **187**。
- [ ] 每个文件含 ≥1 绘制元素（非空骨架）。
- [ ] 文件名 = `canonical_name`（含下划线，如 `combo_box.svg`）。
- [ ] 再生门禁存在，且**注入一字节改动后实测 FAIL**。
- [ ] 产物带 `GENERATED_MARKER`，门禁的「非生成物」跳过逻辑不会误跳过它。
- [ ] `snapshots/svg/README.md` 说明用途、再生方式、新增控件时的更新步骤。

### DoD-R-4（跨层一致性与缺口收敛）

- [ ] `python3 tools/audit_platform_create_coverage.py` → `UNRESOLVED (0)`，exit 0。
- [ ] 门禁 `tools/check_platform_create_coverage.sh` 存在，且**反向注入（删一条别名）实测 FAIL**。
- [ ] 该门禁已登记进 `tools/run_all_gates.sh`（否则等于没接）。
- [ ] `spin_box` 浮点：`set_decimals(2)` 后 `set_value(1.5)` 往返正确；
      既有整数测试 **0 回退**（规则 #21 向前兼容）。
- [ ] `heatmap` 在 capability 表中（`checked=188`），且：
      - 第 1 层普查能渲染它（非背景像素 > 0）；
      - 第 3 层为它生成 `snapshots/svg/heatmap.svg`；
      - **desktop / tablet / mobile** 三端构建通过（规则 #20）。
- [ ] 新控件的属性/事件逐项有测试，**不得**是只有 `impl Draw` 的空壳（规则 #5）。

### DoD-R-5（WebEngine 诚实降级 — **W1 已裁定**）

- [ ] `supports_web_engine()` 存在，且与 `supports_surfaces()` 同构（同一命名与语义风格）。
- [ ] 调用方能**查询**到当前是「模拟」而非真引擎（不再是「不暴露给调用方」）。
- [ ] 反向注入：把该查询改成恒报「真引擎」 → 测试必须 FAIL。
- [ ] **W1 删除完成且零残留**：
      `grep -rn "NativeWebEngine\|webkit_engine\|webkit2gtk\|webkit-engine" src/ Cargo.toml`
      → **无输出**。
- [ ] §3.5.4 的 10 项清单**逐项完成**（含 `full` 列表移除与注释修正）。
- [ ] 删除后构建全过：`desktop` / `full` / `mini` 均 **0 error**；`check_profiles.sh` **exit=0**。
- [ ] **保留项未被误删**：`WebEngineViewEnhanced` 本体、boa 的 `evaluate_javascript`、
      `web_engine_view` 的 capability 记录、`src/web/` 其余模块。
- [ ] 文档不再声称本库支持原生网页渲染（§1.8 / README / cookbook）。
- [ ] 模拟路径**行为不变**：`cargo test --features desktop --lib web` 全绿。

### 全局 DoD

- [ ] `bash tools/run_all_gates.sh` → `FAIL=0`（`SKIP` 逐条有理由）。
- [ ] 5 个 profile 全部通过：`desktop` / `tablet` / `mobile` / `mini` / `embedded`。
- [ ] `cargo clippy --no-default-features --features desktop --all-targets -- -D warnings` → 0 警告。
- [ ] 每个已修项在 `docs/log/` 中逐条标识。
- [ ] **五层**计数表写入日志（规则 #100）。

---

## 七、本文件的一句话结论

**第 58 轮修掉的 4 个缺陷，全部躲过了当时已有的 64 个门禁——因为那些门禁
检查的是「声明」，而缺陷在「渲染」。**

本计划补上缺失的维度，分**五层**：
**第 1 层**用「两种外观必须渲染不同」在 187 个控件规模上自动找到 1457 处硬编码颜色
（一条判据覆盖 75% 的文件）；
**第 2 层**把「声明了但不存在」的对称缺口补上；
**第 3 层**产出 187 个 SVG 到 `snapshots/svg/`，把机器断言不了的东西
（比例、配色、观感）变成可人眼复核、可 diff 追踪的工件；
**第 4 层**收敛**跨层不一致**（41 个平台控件工厂中 1 个名字查不到控件）
并补上确认的缺口（`heatmap`、`spin_box` 浮点）；
**第 5 层**让 WebEngine 的降级**可查询**，并按已裁定 W1 **删掉那条从未显示过的**
WebKit 包装（76 行、单平台、从未接入窗口，而文档却写着「不暴露给调用方」）。

**但「全部主题化」本身是错的**——控件里的颜色分三类且判据相反（§1.5，规则 #108）：
外观色**必须**主题化；语义色**必须**读 `colors.*`（而本轮取证发现 4 个语义 token
在 181 个控件里被引用 **0 次**，§1.4——与 BLUE19「已发布事件无人发射」同类）；
数据色**必须不**主题化（改成主题色会抹掉信息）。所以 P3 必须配一张**附理由的豁免表**，
否则「漏读了」与「故意的」无法区分。

**五个必须写死的技术约束**：按**名字**遍历（否则漏 19 个控件）、
判据**分类**不齐一（否则丢失信息或合法化漏读）、
豁免**必须附理由**（否则变成万能开锁）、
跨层缺口先判**补还是删**（不得默认删）、
**反向注入**验证（第 58 轮实测一条门禁假通过）。

**一条反复出现的方法论教训**：定「该加什么控件」时，
`grep -c "ContextMenu"` = 0 曾让我把 `context_menu` 误判为缺口——
而它只是 `Menu` 的别名，**不需要**有自己的 capability 记录；
同样的错误在 `create_web_engine`（不是控件工厂）与 WebEngine 的
「原生」（实际只做了 5%）上又各犯一次。
**「名字/形容词存在」≠「东西存在」**，先查清实际做了什么再下结论。

---

## 八、新对话的开场指令（可直接复制）

> 按 `docs/plans/blue20.md` 执行。先读 §零（实现者入口）与 §二（约束）。
> **五层依次落地，不并行**（用户明确要求「按步骤一层一层完成」）。
> 从 **第 1 层 R-1a** 开始：先写普查探针，确认能打印 **187 行**（按 `canonical_name`
> 遍历，不是按 `WidgetKind` 的 179——规则 #105）。
> **判据是分类的，不是齐一的**：外观色必须主题化（P3），语义色必须读 `colors.*`（P4，
> 当前 4 个 token 被引用 0 次），数据色必须**不**主题化且要登记豁免表（§1.5 / §3.1.3）。
> 写 P3 之前先生成豁免表，否则无法区分「漏读了」与「故意的」。
> 第 3 层的输出目录与文件名已由用户指定：**`snapshots/svg/<控件名>.svg`**，共 187 个。
> 第 4 层已裁定：补 `checkbox` 别名 + 修正脚本判据、接门禁、
> `spin_box` 加浮点、新增 `heatmap`；**不加** `gantt`/`timeline`/`phone_input`/`email_input`。
> 第 5 层已裁定 **W1**：先做诚实降级（`supports_web_engine()` + 「真引擎/模拟」可查询 + 修正文档），
> **然后按 §3.5.4 的 10 项清单删掉 WebKit 包装**（76 行、从未显示、单平台）；
> **不**用 Servo、**不**补三平台原生。
> 删除后必须能证明**零残留**：
> `grep -rn "NativeWebEngine\|webkit_engine\|webkit2gtk\|webkit-engine" src/ Cargo.toml` → 无输出。
> 遵守 §二.4 的存量处理方式（**先冻结现状、再逐格收敛**），
> 不要一上来就断言「全部必须通过」。
> 每条判据必须**反向注入**验证，验收按「附录二」逐条对照。
