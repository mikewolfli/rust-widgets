# BLUE25 — 全控件缺陷清理与图标库落地

> **本计划的来源**：用户要求「全面扫描所有控件，仔细分析，找出所有缺陷和不足」，
> 并就图标库给出一个完整方案。
>
> **本计划不新增任何"看起来更整齐"的重构**。每一条都是**实测到的**缺陷或缺口，
> 附 `file:line` 证据。凡未取证的一律进入 §6 的「未取证/需复核」区，
> **不登记为待办**（原则 #1：结论必须有证据）。

---

## 0. 取证摘要（本轮扫描范围与方法）

### 0.1 扫描范围

```text
src/widget/            13 个子目录，188 个已注册控件
src/render/  src/text/  src/style/  src/theme/  src/core/
src/image/   src/event/ src/gesture/ src/layout/
src/platform/types.rs  （仅 trait 定义）
```

### 0.2 方法（可复现）

1. **主题键 × 消费者矩阵**：对 `src/theme/preset_states.rs:63-172` 生成的**每一个**
   `"<kind>:<state>"` 键，逐个回答三问：① 控件是否从 `widget_state()` 报告该状态？
   ② 该键是否被解析到 style 上？③ 控件的 `draw` 是否读取该键写入的**那个字段**？
   三问缺一即为缺陷（②③都通过才算"消费"）。
2. **死分支扫描**：`draw` 内 `unwrap_or` / `unwrap_or_else` / `or_else` 链中，
   后一个分支是否可能被到达（若前一个**恒**为 `Some`，则后一个是死代码）。
3. **setter 重解析扫描**：遍历所有会改变 `widget_state()` 输入
   （`enabled` / `checked` / `selected` / `hovered` / `pressed` / `focused`）的 setter，
   检查是否调用 `crate::style::reapply_active_theme_state(self)`。
4. **字段丢弃扫描**：grep `if style.theme_derived { None } else { ... }` —— 每一处
   都是"解析出来的状态值被丢掉"的候选（孤儿键）。
5. **同概念双路径扫描**（原则 #101）：对每一对"做同一件事"的机制 grep 取证
   **真实重复量**，而不是凭名字相似判断（原则 #51）。
6. **层次/门控纪律**：grep `cfg(target_os|unix|windows)`、`/proc`、`/sys`、
   `Command::new`、`.join(".config")`、平台库名，检查是否越出 `src/platform/`
   （原则 #35/#36/#44）；grep 手写合取式（原则 #47）。
7. **占位符扫描**：`todo!` / `unimplemented!` / 空体 / 返回假值的常量。

### 0.3 本轮修复的**同一根因**（它解释了多数缺陷）

> **`apply_active_theme` 只在两个创建漏斗里跑一次**，用的是构造器产生的状态。
> 而状态 setter 只 `request_redraw()` —— **重绘重跑 `draw`，不重跑主题应用**。
> 因此凡是"创建之后才改变的状态"，其主题键都不会生效。

这条根因在 BLUE24 §49 已被单独记录并部分修复（四个 latch 控件）。
**本轮发现它远不止那四个控件**：它是 §1 里 A 组全部条目的共同根因。

---

## 1. A 组 — 主题状态解析（根因见 §0.3）

### A-1 【BLOCKER】一个控件都不能用的 `:hover` / `:pressed` / `:disabled`

**证据**：

```text
src/style/preset_states.rs:69-92   为 4 个 kind × 3 个状态 = 12 个键
   button / toggle_button / tool_button / split_button  ×  hover / pressed / disabled

src/widget/base.rs:348-350   pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
src/widget/base.rs:556-558   pub fn set_hovered(&mut self, hovered: bool) { self.hovered = hovered; }
src/widget/base.rs:568-570   pub fn set_pressed(&mut self, pressed: bool) { self.pressed = pressed; }
src/widget/widget_trait.rs:163-165   set_enabled → base_mut().set_enabled（无钩子）

crate 内 reapply_active_theme_state(self) 调用点**只有 5 个文件**：
   checkbox.rs / radiobutton.rs / toggle_button.rs / switch.rs / chip.rs
```

**后果**：这 12 个键**全部**只在"控件恰好在创建时就是那个状态"时才生效。
一个运行中的应用把按钮禁用（`set_enabled(false)`，一个**可写已发布属性**，
`properties_base.in.rs` 里 `button`/`checkbox`/`radio_button`/`toggle_button` 都是可写的，
所以 JSON / 设计器路径可达）—— **画面上什么都不变**。

**为什么这是 BLOCKER**：`enabled` 是已发布可写属性，因此这是**用户可见的正确性缺陷**，
不是一个抽象问题。

**修法（**必须修在基类**，否则 188 个控件要各修一遍）**：
在 `BaseWidget::set_enabled` / `set_hovered` / `set_pressed` 内部重解析。
但这三个方法**只有 `&mut self`（BaseWidget）而没有 `&mut dyn Widget`**，拿不到 `kind()`。
所以正确形状是**在 trait 层**给一个默认实现：

```rust
// src/widget/widget_trait.rs —— 默认实现同时写 base 与重解析，188 个控件自动获得
fn set_enabled(&mut self, enabled: bool) {
    if self.base().is_enabled() == enabled { return; }   // 幂等：避免每帧重解析
    self.base_mut().set_enabled(enabled);
    crate::style::reapply_active_theme_state(self);
}
```

同形处理 `set_hovered` / `set_pressed`。
**幂等性是必须的**：`set_hovered` 在 `MouseMove` 上每帧都可能被调用，
不做 early-return 会把"每帧一次角色分类 + 样式解析"变成常态（原则 #51 的反面）。

**验收**：新增测试（`theme/apply.rs`），对 `Button` 走
"先 `apply_active_theme` → 后 `set_enabled(false)`"，断言
`style.background_color` 变成 `button:disabled` 的值。
**注入**：删掉 trait 层重解析 ⇒ 断言红。
**冰山**：同一测试扩到 `tool_button` / `split_button`（同一键族）。

### A-2 【DEFECT】`switch:checked` 是孤儿键（BLUE24 §51 已诊断，未修）

**证据**：

```text
preset_states.rs:105-113   switch:checked { background: primary, foreground: primary.contrast }
switch.rs:376-383          报告 Checked                    ✅
switch.rs:141              set_checked 重解析              ✅
switch.rs:496              let theme = resolved_theme_style("switch");   ← 基类，state = None
switch.rs:497              caller_background = if theme_derived { None }  ← 丢弃 checked 的解析值
```

`resolved_theme_style` → `resolve_style_for_state(name, None)`（`manager.rs:204-208`），
**永不读 `switch:checked`**。实测（探针）：ON 与 OFF 的 track 同为 `rgb(180,180,180)`。

**为什么不是"设计使然"**：`on_track` 的确由 `SemanticColor::Success` 决定（这是设计），
但 `switch:checked` 声明的 `background` **被解析后丢弃** —— 一个主题作者改它不会看到任何变化。
**两者是不同的事实**：绿色是"开关的事实状态"，`switch:checked` 是"主题对该状态的声明"。

**修法**：`draw` 读 `resolved_theme_style_for_state("switch", Checked)`，作为 ON 端的**声明层**，
`SemanticColor::Success` 降为**兜底**（与 `radiobutton.rs:512-525` 已采用的梯级完全同形）。

### A-3 【BLOCKER】9 个 `:error` 键是孤儿（`line_edit` 独活）

**证据**：

```text
preset_states.rs:149-169   为 10 个 kind 写 "<kind>:error"
grep "fn semantic_state" src/   →  只有 widget_trait.rs:427（默认 None）+ lineedit.rs:689
grep "resolved_semantic_border" →  唯一调用点 lineedit.rs:1022
```

**后果**：`text_edit` / `text_area` / `combo_box` / `editable_combo_box` / `spin_box` /
`date_edit` / `time_edit` / `date_time_edit` / `masked_edit` 的 `:error` 键
**声明了、生成了、没人读**。一个表单校验失败的可编辑字段**画不出错误态**。

**注意**：这不是"少写了几行"—— `manager.rs:301-311` 明确文档化：语义通道**不在**
交互链上，所以它**必须**由控件实现 `semantic_state()` + 在 `draw` 读 `resolved_semantic_border`。
9 个控件一个都没有。

**修法（二选一，**必须选**，不能维持现状）**：
- **推荐：补齐**。每个可编辑控件实现 `semantic_state()`，反映它**自己已有的**校验事实
  （`masked_edit` 已经会拒绝非法输入、`line_edit` 已有 `max_length`；
  没有校验的控件返回 `SemanticState::None`，然后**从预设里删掉它那个键**）。
  判定标准：一个控件有 `:error` 键 ⟺ 它实现了 `semantic_state()`。
- 备选：把 9 个键从 `preset_states.rs` 删掉。**但那会让"预设声明了错误态样式"
  这个能力只剩一个控件** —— 与本仓"表单控件完整"的方向相反。

**门禁**：新增 `tools/check_semantic_state_has_a_consumer.sh` —— 断言
「`preset_states.rs` 里每个 `:error` 键的 kind，都有一个实现了 `semantic_state()` 的控件」。
**注入**：删掉 `lineedit.rs` 的实现 ⇒ 门禁红。

### A-4 【DEFECT】`chip:selected` 被消费到了**错误的表面**

**证据**：

```text
chip.rs:386-398   draw 读 resolved_theme_style("chip")（基类）+ style.*（已被 selected 覆盖）
chip.rs:410-411   context.fill_rect(band, background);   ← 整条**行带**被涂成 primary
chip.rs:425-431   每个 chip 的底 = background.blend(...)；标签 = primary.contrast_color()
```

**后果**：选中一个 chip，**整条 chip 行**变成 primary，所有 chip 的底色都从 primary 派生，
所有标签都用 `primary.contrast_color()`。而 `chip.rs:258-260` 的文档声称
「`chip:selected` —— a declared state with a consumer, in both directions」。
**文档与代码不一致（原则 #18），且视觉结果是错的。**

**修法**：`draw` 把 `chip:selected` 的解析值**只用于被选中的那一个 chip 的矩与墨**；
行带用基类 `chip` 的解析值。文档同步改写。

### A-5 【DEFECT】`split_button` 整族键无消费者

**证据**：`preset_states.rs:69` 把 `split_button` 列入键族；
`grep reapply_active_theme_state src/widget/special_widgets/split_button.rs` → **0 命中**；
`split_button.rs:601` 读 `resolved_theme_style("split_button")`（基类）。

`split_button` **与 `button` 同类**，所以 A-1 的 trait 层修法**顺带修掉它** ——
但要在 §5 验收里**显式列出**，否则"修了 Button"会被误当成"修了按钮族"。

### A-6 【SMELL】`groupbox::set_checkable` / `set_checked` 不请求重绘

**证据**：

```text
groupbox.rs:117-120  set_title      → request_redraw ✅
groupbox.rs:126-129  set_alignment  → request_redraw ✅
groupbox.rs:135-137  set_checkable  → 只有赋值 ❌
groupbox.rs:142-149  set_checked    → 只 emit toggled ❌
```

**后果**：程序化切换（以及已发布的 `toggle` 命令）**在画面上不可见**，
直到某个无关的重绘发生。

---

## 2. B 组 — 主题数据层的缺陷

### B-1 【BLOCKER】主题的 `padding` / `margin` **一个控件都收不到**

**这是本轮扫描发现的**最严重**的单点缺陷**，因为它影响**全部 188 个控件**。

**证据**：

```text
src/theme/manager.rs:401-402   role_base_style 写入：
                                   padding: Padding::all(theme.spacing.medium),
                                   margin:  Margin::all(theme.spacing.small),

src/style/primitives.rs:454-477  merge_theme 的 theme_derived 分支复制字段清单：
                                   background_color / background_gradient / text_color / font /
                                   border_color / border_width / border_radius / shadow /
                                   surface / touch_target / opacity
                                   ↑ 没有 padding、没有 margin（grep 计数 = 0）
src/style/primitives.rs:591-625  merge() 同样是这份清单
```

**实测 grep**：`grep -c "padding\|margin\|spacing" merge_theme 的函数体` → **0**。

**后果**：主题里 `spacing.medium` / `spacing.small` 是**死数据**。
每一个主题化控件的 `style.padding` 恒为 `Padding::all(0)`。
`command_link.rs:226` 读 `&style.padding` —— 它永远读到 0。
`widget_trait.rs:576` 的 `padding()` 访问器同样永远返回 0。

**为什么没人发现**：**没有任何测试断言 padding 能到达控件**。
（实测：`grep padding src/theme/apply.rs src/style/primitives.rs` 只命中 `Padding` 的定义与文档。）

**修法**：把 `padding` / `margin` / `spacing` 加进 `merge_theme` 的 theme_derived 赋值块
**和** `merge` 的字段清单。注意 `padding`/`margin` 的类型是 `Padding`/`Margin`（非 Option），
所以"未设置"的表示是 `Padding::default()` —— **合并规则必须显式定义**：
沿用其余字段的语义 ⇒「theme_derived 时整体替换；否则只在 `== Default` 时填充」。
`spacing` 是 `Option<u32>`，直接沿用既有规则。

**验收**：新增测试断言「`apply_active_theme` 之后，一个控件的 `padding()`
等于主题 `spacing.medium`」。**注入**：复原 `merge_theme` ⇒ 断言红。

**冰山（原则 #2）**：这条修完后**必须**重跑 `check_svg_snapshots.sh`，
因为 padding 一旦生效，**几乎所有控件的几何都会变** —— 这是一次**大范围快照位移**，
是本计划风险最高的一步，所以**排在 §7 批次的最后一步**。

### B-2 【DEFECT】`style/theme.rs` 为 stripped profile **手抄了一份**主题类型

**证据**：

```text
src/style/theme.rs:186-193  文档：「Where the theme module exists the names are re-exports of it
                                    (rule #54: one definition)」
src/style/theme.rs:203-275  not(device_profile) 分支**手写** SemanticColor / AppearanceMode /
                            Theme / Colors
theme/types.rs:14-75        正版 Theme 有 9 个字段；stripped 版只有 1 个（colors）
```

**后果**：文档说"只有一份定义"，实际有两份，且**字段数不同**。
这是原则 #54 的直接违反，也是 #18（文档欺骗）的实例。

**修法**：要么让 stripped 分支 `pub use` 一个**裁剪过的单一类型**，
要么在**定义处**（`theme/types.rs`）写清"两形状是刻意的"，并让 `style/theme.rs` 只转发。
**判定**：`SemanticColor` 的 stripped 版**没有任何方法**（`ALL` / `token` / `of` 都缺），
所以它现在只能被构造不能被使用 —— 说明它**没有真实消费者**，
倾向于**删掉重复定义、让缺方法成为编译错误**。

### B-3 【SMELL】`theme_test_guard` 上方有一段**重复的文档**

**证据**：`src/theme/manager.rs:721-726` 与 `727-735` 是近乎逐字重复的两段。
**修法**：删掉前一段。

---

## 3. C 组 — 双路径 / 死代码（原则 #101、#4、#51）

每一条都**已实测**"另一条有多少消费者"，而不是凭名字判断。

| # | 概念 | 路径 A（在生产中） | 路径 B（死/并行） | 实测消费者数 | 处置 |
|---|---|---|---|---|---|
| **C-1** | 文本截断/省略 | `render/backend/surface.rs::fit_text_to_width`（约 90 个控件用） | `render/text_overflow.rs::apply_text_overflow` + `TextClamp` | **1**（仅自身 + re-export） | 删除，或明确指定为唯一路径并把 A 迁移过去 |
| **C-2** | 字素簇切分 | `render/text/line.rs::for_each_cluster` | `render/grapheme.rs::GraphemeProcessor` + `split_graphemes` | **0 / 1** | 删除。且 `line.rs:40-47` 声称"本仓唯一的切分规则" —— **文档已说谎** |
| **C-3** | 文本宽度估算 | `widget/metrics.rs::estimate_text_width`（按簇、宽字 1.0 em、最小 1.0） | `advanced_widgets/tab_bar.rs:44-46` 私有版（`chars().count() * size`） | 1（仅 tab_bar 自用） | 改为调用共享版。**这正是 BLUE24 U-14 的同一缺陷类**（用错的度量） |
| **C-4** | 事件队列 | `EventQueue`（生产） | `PriorityQueue` / `BoundedQueue` / `FixedSizeQueue` | **0 / 0 / 1** | 删除无用三个，或写明消费者 |
| **C-5** | 事件别名 | `event/types.rs::Event` | `event/legacy_types.rs::KeyEvent`/`MouseEvent` | **0** | 删除，除非能指出外部消费者 |
| **C-6** | 图表布局 | —— | `chart_widgets/layout.rs::ChartLayout` | **0** | 删除，或接入并写明分层（原则 #49） |
| **C-7** | `WidgetKind`→名字 | `alias_factory_name`（变体匹配） | `alias_for_name`（字符串匹配） | 两者都用 | 两张独立维护的表；文档称"不会不一致"但无机制保证 ⇒ 加**对拍测试** |

### C-8 【DEFECT】`factory_name_for_kind` 在无 registry 构建里返回 `""`

**证据**：

```text
src/widget/capability.rs:357-375   canonical_name_for_kind 只匹配 18 个拼写
src/widget/capability.rs:405-427   "+ ~9 个 alias"
src/theme/apply.rs:69-78            factory_name_for_kind 返回 "" ⇒ **静默跳过主题化**
```

**后果**：在 `widgets_unstripped && !full_widgets` 构建（例如 `--features windows`，
`Cargo.toml:228 windows = []`）里，约 **150 个 kind 的主题化被静默跳过**。
这是原则 #37 的违反（用假值掩盖未实现），且**无日志**（`log::debug!` 在 `""` 分支里，
debug 级别默认不可见）。

**修法**：让 `factory_name_for_kind` 返回 `Option<&'static str>`，
或者让无 registry 路径**从同一张表**派生名字（原则 #101）。
**绝不能**继续返回 `""` 并静默跳过。

### C-9 【DEFECT】`reapply_active_theme_state` 的**门控与实现体不一致**

**证据**：

```text
src/theme/apply.rs:48,66   apply_active_theme 本体门控 = all(not(alloc_frugal), widgets_unstripped)
src/theme/apply.rs:143     reapply_active_theme_state 门控 = full_widgets
src/theme/apply.rs:153     no-op 臂 = not(full_widgets)

build.rs:90   widgets_unstripped = !is_stripped
build.rs:96   full_widgets       = has_profile && !is_stripped     ← **严格更窄**
```

**后果**：`--features windows`（无 device profile、但 unstripped）这条构建里，
**真版 `apply_active_theme` 被编译，而 `reapply_active_theme_state` 是空实现** ——
状态变化重解析**静默什么都不做**。这正是原则 #47 要防的门控漂移。

**修法**：两处门控统一为 `all(not(alloc_frugal), widgets_unstripped)`。

---

## 4. D 组 — 错误的代码结构 / 纪律

### D-1 【BLOCKER】`Icon::draw` **修改控件自己的模型字段**，破坏调用者设置的颜色

**证据**：

```text
icon.rs:1358-1369
    if !self.base.is_enabled() {
        let resolved = self.resolve_color();
        self.color = Some(grey);      ← 覆盖调用者 set_color 的值
        self.draw_icon(context);
        self.color = None;            ← 恢复成 None，**而不是恢复原值**
        return;
    }
icon.rs:1359-1362  文档：「The pin is removed afterwards, so the pre-draw resolution
                            (theme or explicit setter) is unchanged」  ← **与代码相反**
icon.rs:1414       测试 colour_resolution_prefers_an_explicit_setter **从不画**禁用态，抓不到
```

**后果**：一个 `set_color(red)` 的图标，**只要曾以禁用态画过一次**，
就**永久失去** red —— 重新启用后画主题色，不是调用者的颜色。
**绘制路径绝不能写控件的解析状态**（这条本身就该写成一条纪律）。

**修法**：`let saved = self.color;` / 恢复 `self.color = saved;`。
文档随之变为真。

### D-2 【DEFECT】`image/cache.rs` 有 `cfg(windows)` —— 平台分支越出 `src/platform/`

**证据**：

```text
src/image/cache.rs:665   #[cfg(windows)]
src/image/cache.rs:669   #[cfg(not(windows))]
```

**这是本轮在 `src/platform/` 之外发现的唯一一处 `cfg(target_os)`**，违反原则 #35/#36。

**修法**：用**与分隔符无关**的键（走 `std::path::Component`），
而不是"判断自己是哪个 OS 然后替换 `\\`"。
**判定**：替换 `\\`→`/` 这个动作本身是"规范化路径"，与 OS 无关，
所以它**不该**用 `cfg` 表达。

### D-3 【DEFECT】`capability/registration.rs` 的模块文档**点名了一个不存在的测试**

**证据**：

```text
registration.rs:11-13  「`every_widgetkind_variant_is_registered_or_declared_child_only`
                          in the tests is the thing that keeps this file complete」
grep -rn "every_widgetkind_variant_is_registered" src/ tools/  →  只有这一行文档命中
该文件**没有任何** #[cfg(test)] mod tests
```

**后果**：原则 #27（每个 `WidgetKind`→模块可审计）**没有任何机械门禁**。
实测：180 个变体 vs 169 个有 capability 行的 kind，
11 个变体（`Panel`/`DockPanel`/`MenuItem`/`ContextMenu`/`CheckListBox`/`DoubleSpinBox`/
`Wizard`/`DirectoryDialog`/`ActivityIndicator`/`ColumnView`/`UndoView`）
靠 `alias_factory_name` 覆盖 —— **正确，但无门禁保证**。

**修法**：把这个测试**真正写出来**（文档已经描述了它的判据，只是没实现）。
这与 BLUE24 修掉的 `cached_from_path`（文档承诺、零存在）是**同一类**缺陷。

### D-4 【DEFECT】`platform/types.rs` 用 `family()` **推断**能力

**证据**：

```text
src/platform/types.rs:407-416   capabilities() → native_menu: desktop  （从 family 推出）
src/platform/types.rs:428-437   embedded_capability_contract() → low_memory_mode: true（硬编码）
该文件自己的文档称这是「a silent over-claim」
```

**后果**：一个忘了覆写的后端会**报告自己有原生菜单**，且 `low_memory_mode` 恒 true
（一个"看起来合理的假值"，正是原则 #37 点名禁止的）。

**修法**：默认返回 `false` / `None`（诚实的能力缺失），要求后端显式声明。

### D-5 【DEFECT】`invoke_command` 校验用**规范化后**的名字，派发用**原始**名字

**证据**：

```text
src/widget/capability.rs:901   let normalized = ...            ← 校验用
src/widget/capability.rs:919   properties.command(command_name) ← 派发用**原始**
```

**后果**：调用 `invoke_command("clear-selection")` **通过校验**，
然后到达 `UnknownCommand` → 返回 `UnsupportedOnWidget` —— 一个**误导性的错误**。

### D-6 【DEFECT】`combobox` 的 `popup_visibility_changed` **已发射但未发布**

**证据**：

```text
combobox.rs:337-339  pub popup_visibility_changed: Signal1<bool>
combobox.rs:553      .emit(...)                                  ✅ 有生产者
combobox.rs:871-874  文档声称它「worth publishing」
tools/event_published_census.txt  combo_box 只有 3 个名字，**没有它**
```

**后果**：`connect_event("combo_box", "popup_visibility_changed")` → `UnknownCommand`。
这是原则 #97 的**反向**实例（已发射但不可订阅），且文档说谎。

### D-7 【DEFECT】`status_bar` 三处

```text
status_bar.rs:47-49  字段文档「on every message change」—— 禁用时**不发射**（方法文档已改正，
                     字段文档没跟）
status_bar.rs:73-79  size_grip_enabled 是**已记录的 no-op**（draw 不画 grip）—— 原则 #5
status_bar.rs:80-81  一行重复的普通注释
```

### D-8 【DEFECT】`floating_label` 有两份"focused"

**证据**：`floating_label.rs:93` 私有 `is_focused` 字段；`floating_label.rs:206` `set_focused`
只写它；而 `widget_state()`（trait 默认）读 `base`。**两个 focus 概念在一个控件上。**
今天没有 `floating_label:focused` 键，所以不画错 —— 但这是**潜伏漂移**。

### D-9 【DEFECT】`gesture/swipe.rs` 一个**不可达**的 `unwrap_or`

**证据**：

```text
swipe.rs:142-144   centroid_start 与 start_time 在**同一条语句**里一起被设为 Some
swipe.rs:168-169   now_ms.saturating_sub(self.start_time.unwrap_or(now_ms))
                   ↑ 只有 centroid_start 是 Some 时才执行，所以 start_time 必为 Some
```

**后果**：死回退；若哪天真的被取到，会得到 `elapsed = 0 → max(1)` 与**荒谬的速度**。
**修法**：把两者存成一个 `Option<(u64, Point)>`，让"成对"成为**类型事实**。

### D-10 【SMELL】两个后端对同一事实的推导**不一致**

**证据**：

```text
src/render/pipeline/containers.rs:81   effective_line_height() * scale
src/render/svg/backend.rs:982          effective_line_height().max(1.0) * scale
两处注释都声称「the backends must agree」
```

**修法**：抽一个共享 helper，把 `.max(1.0)` 的钳制放进**唯一**那一份。

**实施（2026-09-28）**：`TextMetrics::for_font(font, scale)` 成为唯一推导，两个后端均改调它。
**实测差异**：`size = 0` 时软件后端 `ascent = 0`、SVG 后端 `ascent = 1`（`height` 相同，
**baseline 位置不同**）。反向注入：把软件后端改回无下限 ⇒ `both_backends_agree_on_a_fonts_line_box` 红。

### D-11 【SMELL】两处 `unreachable!()`

`image/format.rs:303`、`image/decoder.rs:213`。守卫是真的，但原则 #5/#12
不鼓励"用 panic 表达不可能"，且同 crate 的 `image/cache.rs:749-751` 已采用"写出该分支"的写法。

**实施（2026-09-28）**：两处均改为非 panic —— `format.rs` 写出幂等的 `Rgba8(d.clone())` 分支；
`decoder.rs` 的 catch-all 改为**返回与调用方同一条错误**，使保证只在一处陈述。
反向注入：`a_static_format_is_refused_by_the_animation_decoder_without_panicking` 断言 PNG 输入
返回含 “GIF or WebP” 的错误。

---

## 5. 验收与门禁（本计划的**完成判据**）

### 5.1 每条修复必须带的证据

| 类别 | 要求 |
|---|---|
| 每一条 A/B/D | 一条**新测试**，断言**用户可见的输出**（几何/style 字段），不是模型字段 |
| 每一条 A/B/D | **反向注入**：删掉修复 ⇒ 测试必须变红（并在日志里贴出红的那一行） |
| 门禁类（A-3、C-7、D-3） | 门禁脚本本身必须**先被证明会失败**（`tools/README.md:25-29` 明确要求） |
| B-1 | 必须重跑 `check_svg_snapshots.sh` 并**逐条审阅**位移，因为它是全控件几何位移 |

### 5.2 执行纪律（原则 #55-#60，本轮必须遵守）

- **中间轮次不跑门控、不跑全量测试**；只在**整个批次结束后**跑一次。
- 每条修复的定点证据 = 单条 `cargo test <filter>`（≥10 分钟超时）或单条 `cargo check`。
- **不裸跑**「全量门禁 + 全量测试」的串行循环（原则 #59）。
- 任何可能挂死的命令**必须**设 `timeout_ms`（原则 #58）。

### 5.3 完成率（每轮结束回写原则 #7）

```text
A 组（主题状态解析）：A-1 A-2 A-3 A-4 A-5 A-6        6/6  ✅
B 组（主题数据层）  ：B-1 B-2 B-3                     3/3  ✅
C 组（双路径/死码） ：C-1…C-9                         9/9  ✅
D 组（结构/纪律）   ：D-1…D-11                       11/11 ✅（D-10/D-11 于 2026-09-28 补完）
§6 图标库           ：ICON-1…ICON-8                   8/8  ✅
§8 待取证项         ：1…5                            5/5  ✅（§17）
                                    合计 42/42 闭环
```

> **一条纪律的教训**：D 组曾一度被回写为 11/11，而实际 D-10/D-11 **从未实施**。
> 汇总数字会把未做的事写成熟。**完成率必须逐条对着代码取证，不能相信上一轮的汇总。**

**§8 五项待取证的最终处置**（§8 原文说“不登记为待办”，但本轮按要求逐条取证并落实）：

| §8 项 | 取证结果 | 处置 |
|---|---|---|
| 1 `WidgetKind` 179 vs 180 | 门禁现解析为 180，22 份文档一致 | 已闭环 |
| 2 `text_overflow.rs` 归属 | `fit_text_to_width` 是唯一路径 | 已删除（C-1）|
| 3 `SemanticColor` stripped 缺失 | 该形状**永不被实例化**，故缺方法不是漂移 | 已文档化闭环（B-2）|
| 4 `ROW_ROWS` 恒等表 | **原假设为假**：实跑探测器，`const fn` 索引 `static` 可编译；且 332 项确是 `0..n` | **已删除**（同步改生成器）|
| 5 魔数键码 | 19 个文件、41 处 | 已建 `event::key_codes` 并全量迁移 |

### 5.4 附带的门禁修复

`check_implicit_size_uses_metrics.sh` 长期错报（`failed=101`），根因是豁免表以
`path:行号` 为键——226 条中 **156 条已失效**。已改为 `path:所属类型` 键，并在改变的文件
（`keyboard.rs`/`search_bar.rs`）上验证了旧键法的不可用。详见日志 §17.3。

**计划外修出的真缺陷 6 项**（均由新门禁/普查自己抓出，见 `docs/log/log-20260927-1.md` §15.4、§16）：

1. SVG **裸数字续命令**未实现 ⇒ `cross` 缩成一个点、画不出来；
2. `T`/`t` 平滑二次曲线未支持 ⇒ `cross` 解析失败；
3. `A` 弧被“推给不存在的调用方” ⇒ 含弧的图标缺一块；
4. 导出器**未 settle 动画** ⇒ `switch.svg` 定格在 travel ≈ 0.10 的中间帧；
5. `switch_on` 额外外观与 `switch` **逐字节相同** ⇒ 删掉（extras 7 → 6）；
6. `icons` feature 在 **CI 中从未被编译** ⇒ 两个图标测试永远被 `#![cfg]` 掉、不可能失败。

另有 B-1 引起的 2 个控件**几何位移**（`calendar` / `command_link`），属计划 §7 预告的善意位移，**逐条审阅后**重生。

---

## 6. 图标库 —— 完整实施方案

> 详细调研（含许可证核验、选项对比、数据布局、字节估算）见
> **`docs/plans/research-icons.md`（837 行）**。本节是**可执行的实施计划**。

### 6.0 现状（实测，这是"缺失"的来源）

```text
icon.rs:32-95      IconName 31 个变体
icon.rs:343-1277   28 个 draw_* 方法 —— **几何就是代码**，没有图标数据
icon.rs:585-588    Close 直接调 draw_cross ⇒ close 与 cross **画出来一样**
icon.rs:28-30      模块文档**自己承认**："visually distinct names do not always produce
                   distinct output"
icon.rs:247-250     set_icon 接受**任意字符串**，draw 时匹配
icon.rs:750,1264-1277  匹配不上 ⇒ draw_unknown（一个问号占位）—— **静默**
icon.rs:1509-1536  31 个图标里**只有 3 个**有快照
Cargo.toml         没有 icons-* feature；没有图标相关依赖
NOTICE             覆盖 5 张字体表，**零**图标素材
```

**结论**：`"svg 里很多是缺失的"` 这句观察是**准确的**，
而且缺的根因是**结构性的**：**声明一个图标名与拥有可渲染数据之间没有任何链接**。

### 6.1 方案选择

#### 6.1.1 先分清两条**正交的轴**（这是本节最容易搞错的地方）

“内置 SVG”与“MSDF / GPU / 位图图集 / 图标字体”**不是同一道选择题的两个选项**。
它们是**两个不同的轴**，必须分开定：

```text
轴 A：图标**数据从哪来**   ── Material Symbols / Lucide / Tabler / 自绘
轴 B：数据**怎么变成像素** ── SVG 路径 + CPU 展平  /  MSDF  /  位图图集  /  图标字体
```

**轴 B 的选择独立于轴 A**：无论数据来自哪一套图标集，它都要经过同一条栅格化路径。
把两轴混为一谈，会得出“选 Material Symbols 就不能用 SVG”这类**错误结论**（本节早期草稿
d就犯过这个错：把 Material Symbols 归入“字体方案”，见 §6.1.3 的更正）。

#### 6.1.2 轴 B 的裁定：**SVG 路径 + CPU 展平**

**因为本仓已经有曲线光栅化器，所以 MSDF 是多余的一层。**

实测证据（这是本节最重要的一条事实）：

```text
src/render/text/raster.rs:19-22
  1. Flatten each contour (a move plus a run of line/quadratic/cubic segments)
     into a polygon, subdividing curves until a segment's deviation from its
     chord is below a tolerance tied to the device pixel.
     A fixed segment count cannot work: it under-samples a 12 px glyph
     and wastes work on a 400 px one.
```

即本仓**已经**具备：① deviation-adaptive 展平；② 非零环绕填充；③ 8×8 子采样 AA。
这三件正是画**填充曲线**所必需的，而且已经为字形调优、测过、注释详实。

而 `RenderCommand::DrawPath`（`render/core/command.rs:291-295`）是**点列**，
走 `fill_polygon`（`render/pipeline/primitives.rs:518`）——**它不接曲线**。

**所以缺的不是光栅化能力，是一次“转换”：**

```text
SVG path `d`  ──解析成──▶ 二次/三次贝塞尔段
                            │
                            ├─▶ 喂给 raster.rs 已有的「deviation 自适应展平」
                            │
                            └─▶ 折线 ──▶ 现有 DrawPath / fill_polygon
```

**这使 ICON-1 比最初估计的小得多**：大头（展平精度、AA、环绕规则）不用写。

#### 6.1.3 轴 A 的裁定：**Material Symbols**

| 候选 | 授权链 | 与本仓 `IconName` 命名对齐 | 固定集合 | 结论 |
|---|---|---|---|---|
| **Material Symbols** | `Google LLC → Apache-2.0 → 你`，**无第三方穿插** | ✅ `close`/`search`/`arrow_upward` 与本仓 `Close`/`Search`/`ArrowUp` **基本 1:1** | ✅ 有 release | ✅ **选定** |
| Lucide | `Cole Bemis → Lucide → 你`，**再许可链不完整**（见 §6.1.4） | ⚠️ `x`/`search`/`arrow-up`，需改名映射 | ⚠️ 持续增删改名 | ❌ 不选 |
| Tabler / 自绘 | 单一 MIT / 自有无争议 | 需自行映射 | 自控 | 备选 |

**形态**：Material Symbols 上游**同时**提供字体与**逐图标 SVG 源**
（实测 `google/material-design-icons` 的 `symbols/web/<name>/` 每个图标一个目录）。
**本计划只取 SVG 路径**，不碰字体 —— 因此 §6.1.2 的轴 B 判决与之完全兼容。

**必须固定到 commit SHA**（不能跟 `master`）：否则上游增删会让 `tools/icon_census.txt` 漂移，
而该文件是生成物 + 门禁基准，漂移会表现为“莫名其妙的失败”。

**版权声明**：Apache-2.0 要求 ① 提供 LICENSE 副本；② 若上游有 `NOTICE` 则保留；
③ 声明修改。三条都是**可机械执行**的 —— 这是它优于 Lucide 的关键：
**Lucide 的问题需要人工法务判断，Material Symbols 的只需一个脚本核对。**

#### 6.1.4 本节的一处**更正**（记录在案，不要重犯）

本节的早期草稿曾写：

> “Material Symbols … 许可证 Apache-2.0 且 primarily a **font** → lands in Option B's problems”

**这句不准确，已更正**：它**有**完整的逐图标 SVG 源（上引实测）。
它**既可以**当字体，也**可以**当 SVG 路径源；本计划只需要后者。
把它归类为“字体方案”是**把轴 A 当成了轴 B** —— 正是 §6.1.1 描述的混淆。

同时更正 §6.1 早期草稿对 Lucide 的许可结论。上游 `LICENSE`（已 fetch）**原话**是：

```text
ISC License ... Copyright (c) 2026 Lucide Icons and Contributors
...
The following Lucide icons are derived from the Feather project:
  airplay, alert-circle, alert-triangle, check, x, ... （100 余个）
The MIT License (MIT) (for the icons listed above)
Copyright (c) 2013-present Cole Bemis
```

**坑在于**：Lucide 用**自己**的 ISC 正文覆盖 Feather 的 MIT，并声明 “and Contributors”，
但**未附带 Feather 上游许可证副本**，而 Feather 的 MIT 要求其通知
“**be included in all copies**”（已 fetch Feather 自家 `LICENSE` 核对，无委托条文）。
⇒ 采用 Lucide 时，“Lucide 有权再许可这些图标”这份证据**在上游并不完整**，
且“哪些图标算 derived”**没有客观判据**（只有一份人工维护的名单）。
**这一步法务判断本计划不应承担。**

### 6.2 数据表示

```rust
// src/widget/icons/icon_data.rs  （生成物，GENERATED FILE 头）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconData {
    pub name:  &'static str,      // 规范 token
    pub grid:  u8,                // 24
    pub paths: &'static [&'static str],  // 每条是一个 SVG `d`
}

// src/widget/icons/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName { AlertTriangle, ArrowDown, /* … */ }

impl IconName {
    /// **不返回 Option** —— 缺数据 ⇒ 编译错误（E0004），这是最强的一条保证。
    pub fn data(self) -> IconData { /* … */ }
    pub fn as_str(self) -> &'static str { /* … */ }
    pub fn from_name(name: &str) -> Option<Self> { /* … */ }
}
```

**关键设计（三条保证，见 research-icons.md §4.1）**：

1. **编译期**：`data()` 返回 `IconData` 而非 `Option` ⇒ 新增变体不补数据 = 编译失败。
2. **生成期**：`tools/gen_icon_data.py` 逐字复制上游 `d`，记录**每个上游文件的 SHA-256**，
   并提供 `--check` 重算比对（与 `tools/gen_cjk_bitmap.py` 同一模式）。
   它**拒绝**生成两个逐字节相同的轮廓 ⇒ 从源头消灭 `Close == Cross` 类。
3. **运行期**：census 门禁渲染每个名字，断言
   「解析成功 ∧ 墨非空 ∧ **两两互不相同** ∧ 每个声明名都有数据」。

**解析器（ICON-1，§6.1.2 的“转换层”）**：新增一个 `no_std` 的 SVG 路径访问器
（`M/L/H/V/C/Q/A/Z` + 相对形式），它**不自己展平**，而是把曲线段交给
`src/render/text/raster.rs` 已有的 deviation-adaptive 展平器（§6.1.2 的证据）。

**为什么不是新写一个展平器**：现有那一个已经为字形调优（`FLATTEN_TOLERANCE = 0.2`，
“fractions of a device pixel”）、已有 `MAX_POINTS` 拒绝策略（超过就返回 `None`
而不是静默截断），且已被全部字形路径验证过。**重写一份就是原则 #101 的“同概念两条路径”。**

**需要澄清的一件事**：`src/widget/svg.rs:279` 的 `path_bounds` 是一个 **私有、只认
`M/L/H/V/Z`、且是测试侧**的读取器 —— 它**不是**生产路径解释器，
所以不能直接复用它来解析图标 `d`；但**展平器**（`raster.rs`）可以复用。两者的区别是本阶段的关键判据。

### 6.3 主题化与尺寸

- **颜色**：`Icon::set_color` 优先；否则主题 `text_color`；否则 `Color::PRIMARY`
  （沿用 `icon.rs:321-329` 的既有梯级，**不新增机制**）。
  图标是**单色**的：路径用 `currentColor` 语义 ⇒ 一个图标只有一个色。
- **尺寸/对齐**：**必须按测得度量对齐**，不能猜常数。
  BLUE24 §12 U-14 记录的真实缺陷（用错 face 导致布局漂移）**正是**图标盒猜尺寸会重演的失败模式。
  规则：`Icon::size` 是方形盒；与文本并排时，盒的垂直中心对齐**文本行盒**的中心
  （用 `context.text_line(...)`，与 `tool_button` 弹出箭头那次修复同一原语）。

### 6.4 实施阶段（每阶段以一个**可验证门禁**收尾）

| 阶段 | 交付 | 文件 | 门禁 |
|---|---|---|---|
| **ICON-1** | **SVG `d` 解析 → 复用 `raster.rs` 已有展平器**的适配层（§6.1.2） | `src/render/path/parser.rs`（新）+ `src/render/path/flatten.rs`（几何与放置） | 单测：畸形 `d` ⇒ `Err`，不出 panic；**不断言展平精度**（那是 `raster.rs` 已有的责任） |
| **ICON-2** | `gen_icon_data.py` + `gen_icon_fallback.py` + 上游 vendoring（**固定 commit SHA**）+ `NOTICE` 记录 | `tools/gen_icon_data.py`、`tools/gen_icon_fallback.py`、`tools/material_symbols/*.svg`、`tools/material_symbols/LICENSE`、仓根 `NOTICE` | `gen_icon_data.py --check --license=apache-2.0` 字节一致；无 `--license` ⇒ 拒绝；**未固定 SHA ⇒ 拒绝运行** |
| **ICON-3** | `IconName` + `IconData` + `data()`（无 `None` 臂） | `src/widget/display_widgets/icon.rs`（`IconData` 的类型与绘制） + `src/widget/display_widgets/icon_names.rs`（**生成物**，由 `tools/gen_icon_names.py` 从 `tools/icon_tokens.txt` 生成） | 编译期保证 + `tests/icon_data_integrity_test.rs` |
| **ICON-4** | `Icon` 改存 `Option<IconName>`，**移除 draw 期的 `&str` 匹配** | `src/widget/display_widgets/icon.rs` | 「无静默占位」测试：census 里每个名字都**不走** `draw_unknown` |
| **ICON-5** | `Icon::draw` 走新数据；**删掉 28 个 `draw_*` 中已被数据取代的**（保留过程式兜底） | 同上 | 既有 3 个图标快照**逐字节不变**（证明迁移无回归） |
| **ICON-6** | census + 互异性门禁 | `tools/icon_census.txt`、`tests/icon_census_test.rs` | 注入：给两个名字同一轮廓 ⇒ 门禁红 |
| **ICON-7** | 图标**雪碧图**快照（**不**新增 400 个文件） | `examples/export_icon_sheet.rs`、`snapshots/svg/icon_sheet{,.light}.svg` | `check_svg_snapshots.sh` 新增一步，**不动**它既有的 `EXPECTED` 算式 |
| **ICON-8** | opt-in 门禁 + 许可证门禁 + **生成物生产者门禁** | `tools/check_icon_data_is_opt_in.sh`、`tools/check_icon_licences.sh`、`tools/check_generated_files_have_a_runnable_producer.sh` | 注入：在 `Cargo.toml` 的 `default` 里加 `icons` ⇒ 门禁红；删掉 `NOTICE` 里的 Apache 条目 ⇒ 门禁红；改一个 fallback 坐标字节 ⇒ 生成物门禁红 |

> **上表已于 2026-09-29 按实际文件系统校正**（第 102 轮，用户第 3/5 条）。原表有三处不可复现：
> ① ICON-3 写的是 `src/widget/icons/{mod.rs,icon_data.rs}` —— **该目录从未存在**，
> 实际位置是 `src/widget/display_widgets/`；
> ② ICON-3 把 `IconName` 的生成器写成 `gen_icon_data.py`，**实际是 `gen_icon_names.py`**
> （`gen_icon_data.py` 只写 `icon_data.rs`，它自己的文件头就说明了这一点）；
> ③ ICON-8 现多一条 `check_generated_files_have_a_runnable_producer.sh`。
> 当时的“42/42 闭环”因此**含一条不可复现的判据**，按原则 #18 属文档与代码不一致。
> 校正后，ICON-1…ICON-8 的每一行都能对着文件系统逐条核验。

### 6.7 许可证登记 —— **已核实并定稿**（2026-09-27）

> 本节是 ICON-2 开工前的外部资产决策。结论均取自**上游原文**，不是推测。

#### 6.7.1 上游事实（逐条取证）

| 事实 | 取证 |
|---|---|
| 许可证 = **Apache License 2.0** | fetch `master/LICENSE`，全文为标准 Apache-2.0 + APPENDIX，无附加条款 |
| 版权人 | Google LLC（上游 `README.md` 声明 “available … under the Apache License Version 2.0”）|
| **上游无 `NOTICE` 文件** | fetch 仓库根目录 contents 列表：只有 `LICENSE` / `README.md` / `src` / `symbols` / `font` / `png` / `ios` / `android` / `variablefont` / `update` / `.github` / `.gitignore`。**无 `NOTICE`** |
| 逐图标 SVG 可用 | `symbols/web/<name>/materialsymbols{outlined,rounded,sharp}/`，每个图标一个目录。已核 `close` 目录 |
| 仓库**持续更新** | master 最新 commit `bd8cb85bd4bad964fe6918f79665bb40c3a8efef`（2026-09-25）— 证实“必须固定 SHA” |

#### 6.7.2 Apache-2.0 四条义务 → 本仓具体动作

| Apache-2.0 | 义务 | 本仓动作 | 可机械核验 |
|---|---|---|---|
| §4(a) | 提供 LICENSE 副本 | `tools/material_symbols/LICENSE`（上游 LICENSE 的**逐字**副本）| `sha256sum` 与上游 digest 比对 |
| §4(b) | 修改过的文件要显著标注改动 | 每个 vendored `.svg` 头部保留上游原样；**生成物** `src/widget/icons/icon_data.rs` 顶部写 `GENERATED` 头 + “derived from Material Symbols” + SHA | 生成器头 + `--check` |
| §4(c) | 保留源码形式的版权/归属声明 | `NOTICE` 新增 Material Symbols 段 | `tools/check_icon_licences.sh` |
| §4(d) | 若上游有 `NOTICE` 则随附 | **上游无 `NOTICE`** ⇒ 义务**不触发**；但本仓仍主动在 `NOTICE` 记录，理由见 6.7.3 | 同上一行 |
| §6 | 不得用其商标 | 不在促销语境使用 “Material Symbols” 名称；仅在 NOTICE / 生成头用于归属 | 人工审阅 |

#### 6.7.3 本仓的登记方案（定稿）

1. **许可证副本**：`tools/material_symbols/LICENSE` = 上游 `LICENSE` 逐字副本（满足 §4a）。
2. **归属声明**：`NOTICE` 新增一节 `Material Symbols — SVG path subsets for \`icons\``，仿照现有 5 个字体节的格式（Origin / Source+SHA / What it is / Licence），写明：
   - 取的是 **路径数据（`d`）**，不是字体；
   - 固定 commit SHA（ICON-2 实施时填入）；
   - 生成物路径与 `--check` 命令；
   - 仅改**轴**：`d` 逐字保留，不做简化/重绘（如需裁剪视图框则写明）。
3. **上游 `NOTICE` 的处置**：`check_icon_licences.sh` **主动断言上游无 `NOTICE`**；若将来上游新增，门禁**报警**要求随附它的副本，而不是静默漏掉（§4d 从“不触发”变为“触发”）。
4. **opt-in**：`icons` feature（非 `default`），`check_icon_data_is_opt_in.sh` 保证默认构建不携带图标数据。

#### 6.7.4 不做的事（避免欠账）

- **不**在 `README` 或 UI 的 “about” 屏声明归属 —— 上游 `README` 明说 “We'd love attribution in your app's *about* screen, but it's **not required**”。`NOTICE` + 生成头已满足 Apache-2.0。
- **不**引入 “Material Symbols” 字体（轴 B 裁定为 SVG 路径 + CPU 展平，见 §6.1.2）。


| 项 | 为什么不做 |
|---|---|
| **图标字体** | 与 `tools/check_glyph_source_is_the_only_glyph_path.sh` 唯一路径门禁**直接冲突**；且 BLUE24 §12.2 已否决该类作为字形来源（图标不需继承它的**全部**理由，但字体路径的冲突是硬性的） |
| **MSDF / SDF** | 见 §6.6：本仓**已有**自适应曲线展平器，MSDF 是多余的一层 |
| **GPU 光栅化** | 见 §6.6：图标数量太少，GPU 的收益（图集驻存 + 每图标一个 quad + 一次 draw call）在这里**买不到东西**；且 `mini`/`embedded` **不启用 wgpu**，会变成两套实现 |
| 多色调/duotone | 需要主题层尚不存在的“颜色角色”模型 |
| 任意 SVG 文档的 `currentColor` 继承 | 只做静态单色 |
| 运行时下载图标 | 与“离线可用 + 可重现快照”冲突 |

### 6.6 闸门：为什么图标**不走 MSDF / GPU**

> 格式与 `blue24.md` §12.1 / §12.2 一致：**先把“什么条件下才真的需要”写清**，
> 否则会做成一个没有消费者的机制。

#### 6.6.1 MSDF

MSDF 的价值是“**一个纹理覆盖整个尺寸范围 + GPU 每个图元一个 quad**”。
前提是“**没有曲线光栅化器**”。**本仓有**（`raster.rs:19-22`，已测、已调优）。

而且曲线展平**本来就是分辨率自适应的**：

```text
subdividing curves until a segment's deviation from its chord is below
 a tolerance tied to the device pixel
```

光栅化到多大就展平到多细 —— 所以 MSDF **买不到缩放优势**，只带来预烘焙图集 +
尺寸离散化 + 近似质量。**这是原则 #51：必须带来真实消除才接入。**

#### 6.6.2 GPU

实测：

```text
Cargo.toml:80-84     desktop = [ …, "wgpu", … ]
Cargo.toml:121,138   tablet / mobile 也含 wgpu
Cargo.toml           mini / embedded **不启用 wgpu**

src/render/gpu/mod.rs:37
  **State:** Production callers: `src/render/gpu/mod.rs:1`
  (the software renderer reads the **GPU capability report**).
```

即 wgpu 后端**不是一条在跑的生产渲染路径**。要让图标走上它，需要补：
图标→GPU 纹理上传 + 图集管理 + 尺寸失效逻辑，而且**必须与软件后端像素一致**
（否则 `check_svg_snapshots.sh` 的字节比较失去意义）。

**而图标的工作量根本不吃 GPU**：

```text
一个 24px 图标：展平到 ~60 点 × 8×8 子采样 = 约 15K 次采样
一个工具栏 12 个图标         ≈ 180K 次
正文几百到几千个字形        ← 字形才痛，而图标不是
```

**结论：CPU + SVG 路径。GPU 与 MSDF 在图标这个规模上都是“为解决一个已经解决了的问题”而引入的成本。**

#### 6.6.3 什么时候该回头重新评估

| # | 条件 | 现状 |
|---|---|---|
| 1 | 出现**连续任意缩放**的渲染路径（设计器无限 zoom、地图标注） | ❌ 无 —— 图标只在几个离散尺寸（16/20/24/32/48）用 |
| 2 | 该路径**允许近似质量** | ❌ 无此路径 |
| 3 | 图标集**封闭**且可离线烘焙 | ✅ 已成立（这正是 §6.1.3 固定 SHA 的另一个好处） |

**只有条件 1 出现时，MSDF + GPU 才值得** —— 而且那时它**对图标和字形是同一个问题**，
应该与 BLUE24 U-13 一起重评，**不要分两次做**。

---

## 7. 执行顺序（按**依赖与风险**排序，不按编号）

> **排序原则**：先修**基类**（一次修好 188 个控件），再修**单点**；
> 把**会大范围移动快照**的一步放**最后**，以免它掩盖其它修复的验证。

```text
批 1（基类，收益最大）
  A-1  trait 层重解析（enabled/hovered/pressed）  ← 一次修好全部 188 个控件
  B-1  merge_theme 补 padding/margin/spacing      ← 影响全部 188 个控件
  C-9  门控统一
  C-8  factory_name_for_kind 返回 Option

批 2（单点正确性，可独立验证）
  D-1  icon draw 恢复调用者颜色
  A-2  switch:checked 接线
  A-4  chip:selected 只作用于选中项
  A-6  groupbox 补 request_redraw
  D-5  invoke_command 规范化
  D-9  swipe 成对 Option

批 3（门禁与文档诚实）
  A-3  :error 键 —— 补齐或删除 + 新门禁
  D-3  写出那个不存在的测试
  D-6  发布 popup_visibility_changed
  D-7  status_bar 三处
  B-2  style/theme.rs 去重
  B-3  重复文档
  D-4  platform capabilities 诚实化

批 4（清理，需先取证消费者）
  C-1…C-7  双路径/死代码

批 5（图标库）
  ICON-1 … ICON-8

批 6（收尾 —— **只在这里跑一次**）
  · 全量 cargo test / clippy
  · 全部 check_*.sh
  · 重生成快照（含 B-1 的几何位移，**逐条审阅**）
  · 回写完成率（原则 #7）
```

---

## 8. §6 未取证 / 需复核 —— **已逐条取证并闭环**

以下条目在扫描中被提及，但证据不足或判定标准未定，按原则 #1 **不进入待办**——
但它们不应悬空。本轮逐条取证，结果如下（实施细节见 `docs/log/log-20260927-1.md` §17）：

1. `WidgetKind` 变体计数：**已闭环** —— `check_widget_kind_count.sh` 现解析为 180，
   与枚举实际变体数一致，22 份文档同步。
2. `render/text_overflow.rs` 与 `surface.rs::fit_text_to_width` 谁**应该**是唯一路径：
   **已闭环** —— 实测 `text_overflow.rs` 只有 1 个消费者（它自己），`fit_text_to_width` 是生产路径，
   前者已删除（C-1）。
3. `theme/manager.rs` 中 `SemanticColor` 的 `ALL` / `token` / `of` 在 stripped 构建里缺失：
   **已闭环且已文档化** —— `style/theme.rs:197-216` 记录了“该形状**永不被实例化**”
   （`current_theme()` 恒 `None`，`semantic_color()` 恒 `None`），所以缺方法**不是漂移**，
   无需镜像、也不应删除。
4. `event_payloads.rs` 的 `ROW_ROWS` 恒等表（332 项）：**已删除**（§4 所述假设为**假**：
   实跑探测器证明 `const fn` 可以索引 `static`；且该表确为 `[0, 1, …]`，故 `ROW_ROWS[start] == start`）。
   宏现在直接写 `&EVENT_SCHEMAS[start..start + len]`，生成器同步修改。
5. 各控件用**魔数键码**：**已闭环** —— 新增 `src/event/key_codes.rs`（15 个具名常量），
   19 个文件、41 处字面量全部迁移；常量与 `Key::from_key_code` 的一致性由对拍测试钉住。
