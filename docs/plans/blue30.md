# BLUE30 — RW-44 / RW-58 / RW-63 收编与扩展计划

> **来源**：用户要求把 RW-44（隐藏 setter 未收编）、RW-58（滚动原语缺行单位扩展）、
> RW-63（逐控件焦点订阅）的独立计划写入本文件（2026-11-02）。
>
> **证据基线**：`docs/issues/feedback-20261101.md` 第三轮 T2（RW-44 L539-540、RW-58 L533）、
> 第六/七轮 §2（L957-961 不闭合原因）；本仓库源码实据（逐条标注 file:line 性质的出处）。
>
> **状态（2026-11-02 第九轮）**：RW-63 已闭合；RW-44 的账、门控与 A 类收编已完成
> （census 未分类 0，A 类发布 746 条命令）；RW-58 的 trait/属性路由/handle 入口/门控已完成，
> **全部行单位容器均已接入**（census：rows=31 个注册控件 + `suggestion_popup` companion、
> pixels=2、未分类 0、ROW-UNIT GAP 0），
> 非行单位容器以单位理由具名排除。每处勾选都对应一条实跑证据，记录在
> `docs/log/log-20261102-3.md`。
>
> **判定口径（原则 #105）**：每个 RW 的"完成"必须同时给出
> ① 可执行门控/测试名 + 预期输出，② 具名排除项及原因。
> "具名排除/延后"不计入闭合；没有门控的收编不算收编。

---

## 0. 结论（一页）

| 项 | 缺口本质 | 方案一句话 | 闭合判据 |
|---|---|---|---|
| **RW-44** | 控件的 `pub fn set_*` 群里，只有"属性路由能服务的"进了命令表；其余**没有分类账**，设计器看不见、审计者分不清"程序化注入"与"漏发布" | 以**能力注册表为基准**做一次普查，把每个隐藏 setter 判入四类出口（属性化 / 程序化专用 / 标量分解 / 主题通道），并落一个 census 门控：**每个 setter 必须落在 {已发布, PROGRAMMATIC_ONLY} 二者之一** | `check_setter_census` PASS 且 0 未分类；新发布命令逐一可真实调度（不许 OutOfRange 谎报） |
| **RW-58** | `ScrollableSurface` 只有像素单位（ScrollArea/MdiArea），行索引容器（≥20 个）各有 `scroll_row / scroll_offset / scroll_to_item / scroll_to_node`，宿主恢复滚动位要逐控件写分支 | 新增**行单位** trait `ScrollableRows`（scroll_row / scroll_to_row / visible_row_count / row_height / ensure_row_visible），复用现有方法名桥接；属性路由 `scroll_row`/`scroll_column` 补齐到全部行容器；handle 层给统一入口 | census 门控：每个有滚动状态的容器实现两 trait 之一或具名排除；属性 round-trip 测试；宿主单一入口恢复 |
| **RW-63** | `WidgetHandle` 只有 `is_focused()` 读半边，无 `on_focus*` 写半边 | **不加新状态表**：订阅落在控件自己的 `BaseWidget::focus_gained/focus_lost` 信号上（焦点路由已驱动、JSON `on_focus/on_blur` 已走它），`WidgetHandle::on_focus_gained/on_focus_lost` 暴露它；RW-45 的 `subscriber_count` 经 `base_event_signal` 回退后可观测 | 已有测试 `a_handle_observes_focus_gained_and_lost`（9913 passed）；剩余：全量验证 + suites 冒烟 + 文档回写（§3.3） |

三件事的共同纪律：**先立账（普查/门控），再收编（逐控件分类），最后全量重跑**；
任何一类"暂不收编"都必须在账上有名字和原因，而不是沉默。

---

## 1. RW-44｜隐藏 setter 收编进命令账

### 1.1 现状事实（取证）

**发布机制（不改）**

- `WidgetCapability.commands: &'static [&'static str]`（`src/widget/capability/types.rs:1115`）。
- `set_*` 命令走属性路由：`WidgetProperties::default_command` 要求 `set_` 后缀命中
  `property_names()`（`src/widget/capability/properties_trait.rs:378-394`）。
- 命令 schema 由两处推导：`set_*` 查控件属性表；非 `set_` 动词查 `VERB_CLASSES`
  （`src/widget/capability/command_schema.rs:17-33,196`）。
- **绝对不变量（B26.03，不得违反）**：已发布的 `set_*` 命令必须命中该控件自己的属性表，
  schema 的参数与类型取自该属性声明
  （`src/widget/capability/properties_tests.rs:1365-1400`
  `a_published_set_command_takes_its_shape_from_the_control`）。
  旧的"复合 setter 表"（`set_range`/`set_viewport`/`set_steps`…描述成 pair/list）因 dispatcher
  一律答 `OutOfRange` 已被删除——**本计划不复活它**。
- 已有收编范例：slider 的 11 个 schema-backed setter 可见、3 个无属性载体 setter
  （`set_range`/`set_link_mode`/`set_track_highlight`）保持不发布
  （`src/widget/capability/properties_tests.rs:1312-1348`
  `slider_setters_are_visible_as_commands`）。RW-44 的剩余工作就是把这个模式推广到全部控件。

**缺口规模（第一轮静态普查，仅供估量；权威清单以 §1.3 步骤 1 的运行时普查为准）**

- 已发布 `set_*` 名：约 **167** 个（capability 目录字符串扫描）。
- 全仓 `pub fn set_*`：约 **1600** 个（含测试替身、平台探针、内部结构体，噪声大）。
  其中明显属于"程序化/内部"形态的约 32 个（`set_registry`、`set_a11y_callback`、
  `set_key_handler`、`set_close_handler`、`set_*_style` 族等），
  复合载荷形态的约 136 个（`set_range`/`set_viewport`/`set_content_size`/`set_margins` 族）。
  → **不能拿这个数字直接当账**；账必须以注册表为基准逐控件生成（§1.3）。

**为什么过去不闭合**：把 1454 个候选全部当命令发布会把"程序化注入"误报为设计器可调用，
且 dispatcher 对无属性载体的名字只能答 `OutOfRange`——谎报。出路是**分类账**，不是全量发布。

### 1.2 四类出口（每个隐藏 setter 必须判入其一）

| 类 | 判据 | 处置 | 例 |
|---|---|---|---|
| **A 属性化** | setter 写一个**标量**字段，设计器语义成立（值/范围/开关/枚举 token） | 补属性 schema（kind + default + 约束）→ 发布 `set_<property>` 命令 → 属性路由即可真实调度 | slider `set_minimum/set_maximum` 模式推广 |
| **B 程序化专用** | 注入回调、注册表、句柄、内部状态（`set_registry`、`set_a11y_callback`、`set_key_handler`、`set_*_delegate`…） | 不发布命令；收进静态表 `PROGRAMMATIC_ONLY_SETTERS`，每行**名字 + 一行原因** | `set_registry`（内部工厂注入） |
| **C 标量分解** | 复合载荷但控件**已有**等价标量属性（range↔minimum/maximum、content_size↔content_width/height、viewport↔…） | 不发布复合命令；确保标量属性存在且可写，命令由属性路由天然提供；控件文档注明"整体写走程序化" | `set_range` → 已有 `minimum`/`maximum` |
| **D 主题/样式通道** | `set_*_style` / `set_theme_*`：若已有主题/样式应用通道（`reapply_active_theme_state` 等） | 不发布；进 B 表并注明"走主题通道"；确属设计器可调外观的少数，按 A 用 `Enum`/`String` token 属性化 | `set_button_style`（主题通道） |

出口为**空**是明确禁止的：宁可进 B 表写原因，也不留"查无此账"的 setter。

### 1.3 实施步骤

1. `[x]` **生成权威清单**（只读脚本，不进运行时）：
   `tools/setter_census.py`——对 `WidgetFactory::new_with_defaults()` 注册的每个能力行，
   经 `constructors.rs` 的 `create_*` 解析到结构体文件，抽取 `pub fn set_*`，
   与该能力 `commands` 求差；输出三分类候选到 `docs/reports/setter_census.md`
   （控件 × setter × 文件 × 建议类别）。脚本先落地，账先于改码。
2. `[x]` **新增静态账表**：`src/widget/capability/programmatic_setters.rs`
   `pub(crate) static PROGRAMMATIC_ONLY_SETTERS: &[ProgrammaticSetter]`（`name` + `reason`），
   按 A/B/C/D 四类判入结果填写；测试保证表内名字真实存在、表外无悬空。
3. `[x]` **A 类收编（分批，照 slider 模式）**：
   - 实际执行：四批在同一次分类里完成（属性载体已存在，无需补 schema），共发布 **746** 条
     新 `set_*` 命令；无法发布的 777 个名字按 B/C/D 入账（B 683 / C 85 / D 9）。
   - 批 1：核心 input/display（progress_bar、scroll_bar、spin_box、combo_box、list_box、
     switch、check_box、radio_button、line_edit 族）；
   - 批 2：容器（group_box、stacked_widget、scroll_area、tool_box…）；
   - 批 3：advanced/special；批 4：suites 八组。
   每批：补属性 schema → 发布命令 → 表驱动测试（一个控件一个函数，断言
   `commands.contains` + 属性存在，形状照 `slider_setters_are_visible_as_commands`）。
4. `[x]` **census 门控**：`tools/check_setter_census.{py,sh}`——重跑步骤 1 的普查，
   规则：`每个 pub fn set_* ∈ (已发布 ∪ PROGRAMMATIC_ONLY_SETTERS)`，否则 FAIL；
   支持 `--inject=<控件>.<setter>` 反向注入（删一条账必须 FAIL，防"检查悄悄什么都没做"）。
   注册进 `check_gates_are_worth_running` 与门控索引文档。
5. `[x]` **计数同步**：命令数（当前 1191 实例 / 615 名，`command_schema.rs:17`）、
   属性数、README 与三语 cookbook 的命令表计数随批更新
   （`tools/icon_count_claim_scan.py` 同款思路，防文档漂移——RW-60 教训）。
6. `[x]` **全量重跑**（原则 #102）：`cargo test --lib`、`cargo test --tests`、
   suites `--all-features`、`clippy -D warnings`、五 profile 行为矩阵。

### 1.4 验收标准（完成 = 全部为真）

- `[x]` `tools/check_setter_census.sh` PASS，未分类计数 **0**，`--inject` 自检 FAIL。
- `[x]` 每个新发布 `set_*` 命令有一条**真实调度**测试（写属性→读回），不是只查表。
- `[x]` `a_published_set_command_takes_its_shape_from_the_control` 继续全量 PASS（不变量未破）。
- `[x]` `PROGRAMMATIC_ONLY_SETTERS` 每行有原因；B/C/D 类合计数写进 census 报告。
- `[x]` 文档计数（命令/属性）与实际一致，抽查脚本 PASS。

### 1.5 边界（明确不做）

- 不复活 B26.03 已删除的 Composite 命令表；`CommandSignature::Composite` 仅保留在 schema 语言里。
- 不把回调/注册表/模型句柄 setter 发布为命令（无属性载体可承载 `Arc<dyn ...>`）。
- 不为"发布好看"给控件补假属性（属性必须真被 setter 写的字段支持）。

---

## 2. RW-58｜滚动原语的行单位扩展

### 2.1 现状事实（取证）

**像素通道（已有，语义不动）**

- `ScrollableSurface`：`scroll_position / set_scroll_position / ensure_visible(Rect)`，
  内容像素坐标（`src/widget/container_widgets/scroll_surface.rs:27-43`）；
  实现者 `ScrollArea`、`MdiArea`。
- handle 层：`ScrollAreaHandle::set_scroll_position/scroll_position` 走
  `scroll_position_x/y` 属性 + 平台镜像（`src/app/handle/scroll.rs:36-73`）；
  `WidgetHandle::set_scroll_position/scroll_position` 同属性路由（`src/app/handle/types.rs:689-723`）。

**行通道（碎片化，这是缺口）**

- 属性路由**已部分落地**：`scroll_row`/`scroll_column` 的属性读写已在
  virtual_table、data_grid、grid_table、virtual_list 实现
  （如 `src/widget/view_widgets/virtual_table/widget/properties.rs:21-67`、
  `data_grid/widget_impl.rs:192-232`、`virtual_list.rs:942-964`、`grid_table.rs:1501`）；
  data_grid / virtual_table / grid_table 已发布 `set_scroll_row`/`set_scroll_column` 命令，
  number_picker 已有 `scroll_rows` 动词（`src/widget/capability/properties/view.rs:97,136,307,322`）。
- 但同一语义至少 **5 种拼写/单位**：`scroll_row()`(usize 行)、`scroll_offset()`(usize/i32 行、
  f32 比例)、`scroll_offset()->Point`(像素)、`first_visible_line/row`、`scroll_to_item/row/node`。
- 行索引容器普查（grep 实据，**权威清单以步骤 1 为准**）：
  list_view、tree_view、tree_table、table_widget、virtual_table、data_grid、grid_table、
  virtual_list、listbox、properties_panel、property_grid、query_builder、command_palette、
  timeline_widget、notification_center、toast/stack、toolbox、font_preview、font_combo_box、
  macos/source_list、cascader（按层）、text_entry/suggestion_popup、drop_down_popup_host、tab_bar。
- 七轮不闭合的原因依然成立：把 `y / row_height` 的量化语义伪装成像素 API 是谎报。
  → 扩展 trait 表达单位，而不是给行容器硬套 `ScrollableSurface`。

### 2.2 方案（单位显式化）

1. `[x]` **新增行单位 trait**（与 `ScrollableSurface` 同文件相邻，命名沿用现有词根）：
   `pub trait ScrollableRows {
        fn scroll_row(&self) -> usize;
        fn scroll_to_row(&mut self, row: usize);
        fn visible_row_count(&self) -> usize;
        fn row_height(&self) -> Option<u32>;        // 可变行高容器如实答 None
        fn ensure_row_visible(&mut self, row: usize);
    }`
   - **复用现有方法名**：实现体桥接既有 `scroll_row/scroll_offset/scroll_to_item/scroll_to_node/
     ensure_visible(row)`，不改任何现有签名（用户规则 6）。
   - `row_height` 返回 `Option<u32>`：均匀行高的容器答 `Some(h)`；`data_grid` 这类
     `row_height_at(row)` 可变行高容器答 `None`，避免谎报一个"平均数"。
   - trait 文档写死单位：**行索引**，`(0,0)` 语义、clamp 规则与 `ScrollableSurface` 对偶。
2. `[x]` **属性路由补齐**：给上表每个行容器补 `scroll_row`（必要时 `scroll_column`）的
   属性读写，照 virtual_table 的 `properties.rs` 模式（写→clamp→读回实际值）。
   已落地的 4 个容器只补缺的另一半。
   > **第八轮/第九轮**：全部有 capability 行的行容器已补齐 `scroll_row` 读写（原 9 个 +
   > 第九轮的 20 个具名 `ROW-UNIT GAP`）。`suggestion_popup` 是没有 capability 行的
   > companion，已实现 `ScrollableRows`（无属性路由，因为不属于工厂控件）。像素/视觉行/
   > 水平条/按层/f32 比例容器按 §2.5 具名排除。
3. `[x]` **handle 层统一入口**：在 `src/app/handle/scroll.rs` 增加与
   `ScrollAreaHandle::set_scroll_position` 对偶的
   `scroll_row()/set_scroll_row(row)`（走属性路由 + 镜像"控件接受值"），
   使宿主 `state_keep.rs` 型代码用**一个入口**恢复任意容器滚动位：
   像素容器走 `scroll_position_x/y`，行容器走 `scroll_row`，两者都不需要 downcast。
4. `[x]` **命令面**：已有 `set_scroll_row`/`set_scroll_column`/`scroll_rows` 命令的容器保持；
   其余行容器按 RW-44 的 A 类出口决定是否发布（有 `scroll_row` 属性即天然有命令），
   不新增动词表条目。
5. `[x]` **census 门控**：`tools/check_scrollable_census.{py,sh}`——
   每个持有滚动状态的容器必须：实现 `ScrollableSurface`（像素）或 `ScrollableRows`（行）之一，
   或在 `SCROLL_EXCLUDED` 表具名排除 + 原因（如 scrollbar 的视觉位置是只读派生）；
   支持 `--inject` 反向注入。
6. `[x]` **全量重跑**：同 §1.3 步骤 6，外加 stripped profile 下
   `TextArea/Roller/ScrollArea` 不可用时的 `#[cfg(full_widgets)]` 门控检查（迁移期教训）。

### 2.3 分批顺序（先账后码）

1. `[x]` 步骤 5 的普查脚本先落地 → `docs/reports/scrollable_census.md`（单位/方法名/属性有无）。
2. `[x]` trait + 4 个已通属性路由的容器（virtual_table、data_grid、grid_table、virtual_list）。
3. `[x]` 列表/树族（listbox、list_view、tree_view、tree_table、table_widget）。
4. `[x]` 行内杂项逐个确认单位后接（properties_panel、property_grid、query_builder、
   command_palette、timeline、notification_center、toast/stack、toolbox、font_preview、
   font_combo_box、source_list、cascader、suggestion_popup、drop_down_popup_host、tab_bar）。
   > **第九轮**：行单位容器全部接入——properties_panel / property_grid / query_builder /
   > command_palette / timeline / notification_center / toast_stack / tool_box /
   > font_combo_box / mac_source_list / menu / context_menu / dropdown_menu / menu_button /
   > menu_panel / layer_panel / playlist_panel / mention / multi_select_combo_box /
   > shortcut_editor；`suggestion_popup` 作为 companion 实现 trait。
   > 非行单位容器保留具名排除：font_preview（像素、变高）、cascader（按层）、
   > drop_down_popup_host（f32 比例）、tab_bar（水平条）、文本编辑器（视觉行）。
5. `[x]` handle 层 + census 门控 + 文档（`docs/ARCHITECTURE.md` 滚动一节若存在则同步）。

### 2.4 验收标准

- `[x]` census 门控 PASS，排除表每行有原因，`--inject` 自检 FAIL。
- `[x]` 每个行容器一条属性 round-trip 测试：`set_scroll_row(越界) → read == clamp 值`。
- `[x]` `ScrollableSurface` 现有两实现与像素语义零变化（既有测试 `a_scroll_area_round_trips_the_trait_offset` 等不动）。
- `[x]` 宿主恢复路径文档化：一段"任意容器滚动位恢复"示例仅用 handle 属性入口。

### 2.5 边界（明确不做）

- 不改 `ScrollableSurface` 的像素语义，不给行容器实现它。
- 不统一全部方法名（只桥接；重命名属于 API 破坏，另行立项）。
- 不覆盖纯视觉派生量（scrollbar 拇指位置等）。

---

## 3. RW-63｜逐控件焦点订阅

### 3.1 已实施（工作区未提交，诚实记录）

> 七轮时的"不闭合"结论基于"新增回调只能是第二份状态"。
> 复核发现**运行时事实已存在**：焦点路由 `focus_widget_with_reason / focus_next / clear_focus`
> 把 `FocusGained/FocusLost` 经 `dispatch_event → handle_event → base.handle_event` 投递，
> `BaseWidget::focus_gained/focus_lost` 就是 JSON `on_focus/on_blur` 已在走的通道
> （`src/json/event_route.rs:43-74`、`src/widget/base.rs:1207-1218 PUBLISHED_EVENTS`）。
> 因此"订阅"不需要新表，只需要**暴露既有通道**。

已落地的改动（全部有测试或编译证明）：

| 文件 | 内容 |
|---|---|
| `src/widget/runtime/focus.rs` | `pub fn subscribe_focus(id, gained, slot: FnMut()+Send+Sync) -> bool`——读控件 `base().focus_gained/focus_lost` 并 connect；未挂载答 `false` |
| `src/app/handle/types.rs` | `WidgetHandle::on_focus_gained/on_focus_lost(f) -> bool`（trait 默认实现，经 `register_focus_observer` 拒绝时 warn + `record_unwired_binding`）；`is_focused` 文档改为"读半边"；文档写明 `WindowHandle` 同名固有方法（窗口激活）遮蔽及 UFCS 出路 |
| `src/widget/widget_trait.rs` | `Widget::base_event_signal(name)`——与 per-kind `event_signal_dyn` override 无关地解析两个基事件；默认 `event_signal_dyn` 委托它 |
| `src/signal/event_bridge.rs` | `subscriber_count` 对基事件回退 `base_event_signal`——否则 Button 这类 override 控件上装了订阅却查不到（RW-45 与 RW-63 接线） |
| `src/app/handle/tests.rs` | `a_handle_observes_focus_gained_and_lost`：装订→focus 触发 1 次→重复 focus 不重发→clear 触发 lost→未挂载 `false`→`subscriber_count==Some(1)` |

已验证：`cargo test --lib` → **9913 passed**（含新测试）。
被中断未跑：`cargo test --tests`、`clippy`、suites（见 §3.3 第 1 条）。

**为什么这是"完美"路径（设计定案）**

- 零新增状态：一个通道（控件自身信号），没有可漂移的第二份账。
- 订阅计数可观测：RW-45 的 `subscriber_count/widget_id,"focus_gained"` 直接可见。
- 生命周期与 `clicked/closed` 观察者同规则：随控件销毁而消失；handle drop 无副作用（既有哲学）。
- 不带 reason 载荷：`focus_gained` 是 unit 信号，reason 已记录在控件上
  （`BaseWidget::focus_reason`），再传一份就是第二通道。

### 3.2 已知覆盖边界（冰山核查结论，写入文档不写代码）

- `TagGroup`、`PopupWindow`、`CupertinoSwitch(直接委托内部 Switch)` 的 `handle_event`
  不转发 `base.handle_event`（全仓扫描 245 个 `impl EventHandler`，非测试替身仅此三者）。
  前两者 `is_focusable=false`，路由本就不会向其投递焦点 → 订阅装得上但天然不触发，
  与"非可聚焦控件"一致；不改其事件簿记（避免顺手改 hover/press 行为）。
- `focus_next` 在"全 UI 仅一个可聚焦控件"时 next==previous 仍重发 `FocusGained`
  （既有行为，与 `focus_widget` 的去重不同）；订阅者会多收一次，文档注明即可，不改行为。
- `focus_gained` 的 reason 只在事件里；需要 reason 的宿主读控件自身记录。

### 3.3 剩余闭合步骤（全部待做）

1. `[x]` 全量验证：`cargo test --tests`、`cargo clippy --lib --tests -- -D warnings`、
   `cargo fmt --all`、suites `cargo test --all-features --lib` + clippy。
2. `[x]` suites 冒烟：任选一个 suites 控件 handle 验证 `on_focus_gained` 装订与触发
   （trait 默认实现自动覆盖全部 39 个 suites 控件，冒烟只为证明跨 crate 可用）。
3. `[x]` 卸载闭合测试：unregister 后旧槽不再触发（信号随控件 drop 的行为钉住）。
4. `[x]` 门控复跑：`check_source_structure`、`check_declaration_implementation_alignment`、
   `check_module_reachability`、`check_behavior_matrix`（stripped profile 下新代码路径）。
5. `[x]` 文档回写：issue 第八轮（RW-63 移入"已闭合"，附本节判定口径与 §3.2 排除记录）；
   `docs/log/log-20261102-2.md`。
6. `[x]` RW-63 在 issue 优先级表的口径更新（P2 → 已闭合）。

### 3.4 验收标准

- `[x]` §3.3 全部勾选；`a_handle_observes_focus_gained_and_lost` 及新增卸载测试 PASS。
- `[x]` `subscriber_count(id, "focus_gained")` 在 override 控件（Button）上答 `Some(n)`。
- `[x]` issue/log 回写后，feedback 的"具名未闭合"清单仅剩 RW-44、RW-58（转本计划）。

---

## 4. 执行顺序与全量验证矩阵

顺序：**§3.3（RW-63 收尾，改动已在工作区）→ §1（RW-44，先账后码）→ §2（RW-58，先账后码）**。
RW-63 先收尾是因为其代码已在树上，拖着不验证会让后续每轮全量重跑都背着一个未验证面。

每轮结束的固定验证（原则 #102）：

| 验证 | 命令 | 通过线 |
|---|---|---|
| 库测试 | `cargo test --lib` | 全绿（当前基线 9913） |
| 集成测试 | `cargo test --tests` | 全绿 |
| suites | `cargo test --all-features --lib`（在 crates/rust-widgets-suites） | 全绿 |
| 静态检查 | `cargo clippy --lib --tests -- -D warnings`（两 crate） | 0 告警 |
| 格式 | `cargo fmt --all` | 无 diff |
| 新门控 | `check_setter_census` / `check_scrollable_census` | PASS + `--inject` FAIL |
| 行为矩阵 | `tools/check_behavior_matrix.sh` | PASS（注意 stripped profile 门控） |

---

## 5. 全局纪律（适用三节）

- 遵循现有命名，不为收编重命名既有方法（用户规则 6）；桥接优先，别名仅经 `ALIAS_OF` 表。
- 每个新公开项必须有"谁消费它"的答案（原则 #58/#59）：命令→设计器/属性路由；
  trait→handle 层/宿主 state_keep；订阅→宿主 Inspector。
- 任何"暂不收编"必须在账表里具名 + 原因（原则 #105），门控保证账不落后。
