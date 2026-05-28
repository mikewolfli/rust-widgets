# BLUE9 — 控件属性圆满度、可扩展性与现代 GUI 覆盖度全量深度扫描

> 版本: v0.9.1+
> 基线: 继承 BLUE8 核心规则（PUA 闭环、冰山法则、原生优先/自绘兜底、证据先于结论）
> 编制日期: 2026-05-28
> 文档性质: 全量扫描 + 差距清单 + 可执行改进计划

---

## 核心规则（与 BLUE8 同）

1. 结论必须有构建/测试/代码证据，不允许“推测已修复”。
2. 修一个点必须扫同类模式，避免重复返工。
3. 优先修功能阻断项，再做体验增强。
4. 平台策略不变：原生优先，自绘兜底。

---

## 本轮扫描方法与证据

### A. 控件与类型盘点

- 扫描结果:
  - `src/widget` 下 Rust 文件: **78**
  - 含 `impl Widget for` 的具体控件文件: **59**
  - `WidgetKind` 枚举变体: **81**
- 说明:
  - 81 个类型标签 > 59 个具体实现，存在“别名型控件”和“标签先行”现象。

### B. 属性方法完整性扫描

- 对 `src/widget/**/*.rs` 的 `pub fn set_*` 与 getter（`x()`/`is_x()`/`has_x()`）做配对扫描。
- 扫描结果:
  - 识别到 **28 个未配对 setter 项**（分布于 21 个文件）
  - 其中一部分为“组合 setter”（如 `set_range`）或内部注入接口（如 `set_registry`），不一定是缺陷
  - 剩余为可改进的 API 圆满度缺口（见后文优先级分层）

### C. 可扩展性扫描

- 读取并核查:
  - `src/widget/widget_trait.rs`
  - `src/widget/registry.rs`
  - `src/control_backend/trait_def.rs`
  - `src/platform/*`
- 结论:
  - 扩展基座较完整（trait + signal + model + registry）
  - 但后端契约面仍偏“重实现门槛”，跨平台真实能力不对齐问题仍存在

### D. 现代 GUI 控件覆盖扫描

- 扫描当前控件族数量:
  - advanced 9, base 6, container 9, dialog 7, display 4, input 8, menu/toolbar 6, special 4, view 4, web 2
- 对常见现代控件关键词进行源码检索（Terminal/CodeEditor/MarkdownEditor/TreeTable/DataGrid/VirtualList/MapView/VideoPlayer/CommandPalette/Breadcrumb/SplitButton 等）: **无命中**

---

## 结论一：控件属性方法是否“圆满”

**结论: 尚未圆满，已达到“高可用但非满配 API”状态。**

### 已完成优势

1. 59/59 个具体 Widget 已具备 `base/base_mut` 实现，基础委派能力完整。
2. 大量控件已具备 getter/setter 成对接口，尤其基础输入、显示控件整体较稳。
3. `Widget` trait 提供统一样式、尺寸、信号、触摸命中扩展等默认能力，通用层成熟度高。

### 仍有差距（按优先级）

#### P0-P1（建议优先补齐）

1. 数据视图控件“写入有、读取弱”:
   - `ListView`: `set_model` 无对应模型查询接口
   - `TreeView`: `set_model` 无对应模型查询接口
   - `TableWidget`: `set_model` / `set_delegate` / `set_column_width` / `set_row_height` 缺少对称查询
2. 菜单与工具栏项状态“可写不可查”:
   - `Menu`: `set_item_enabled` / `set_item_checked` 无 `item_enabled` / `item_checked`
   - `ToolBar`: 同类缺口
   - `RibbonBar`: 同类缺口
   - `MenuBar`: `set_menu_enabled` 缺少查询接口

#### P2（可判定为“设计型缺口”，按需处理）

1. 组合 setter 与单项 getter 命名不对称:
   - `set_range` vs `minimum/maximum`
   - `set_date_range` vs `minimum_date/maximum_date`
   - `set_time_range` vs `minimum_time/maximum_time`
2. 内部装配 API 无反向 getter:
   - `set_registry`（容器类）
   - `set_translated_tooltip`（语义写入型）

---

## 结论二：可扩展性是否“拉满”

**结论: 中上强度，可扩展但未拉满。**

### 强项

1. `Widget` trait 默认行为丰富，扩展新控件成本可控。
2. `ListModel/TreeModel/TableModel + Signal` 的数据驱动链条可复用。
3. `SimpleRegistry` 让容器型控件可做事件/绘制转发，便于组合式扩展。

### 关键短板

1. `ControlBackend` 接口面非常大，第三方后端实现成本高。
2. `WidgetKind` 中存在较多别名/标签映射，不等于独立控件能力。
3. 缺少统一的“控件能力元数据与工厂注册”层（能力探测、属性反射、编辑器协议）。
4. 平台侧真实原生能力仍不均衡，直接限制“同一控件跨平台能力上限”。

---

## 结论三：是否覆盖现代 GUI 的“所有控件”

**结论: 未覆盖全部现代 GUI 控件族。**

当前项目已覆盖传统桌面 GUI 大类（按钮、输入、菜单、容器、对话框、基础视图、部分高级控件、WebView），但以下关键现代控件族仍缺失或仅有近似能力：

### 缺失族（高价值）

1. 高性能数据控件:
   - VirtualList / VirtualTable / DataGrid（虚拟滚动、列冻结、排序过滤分组）
   - TreeTable（树表融合）
2. 生产力控件:
   - CodeEditor（语法高亮、折叠、诊断）
   - TerminalView（终端仿真）
   - MarkdownEditor / DiffViewer
3. 业务常用导航与命令:
   - CommandPalette
   - Breadcrumb
   - SplitButton / SegmentedControl / Chip
4. 富媒体与空间类:
   - MediaPlayer（音视频）
   - MapView
   - Timeline / Gantt
5. 反馈与浮层生态:
   - Toast / Snackbar / NotificationCenter

### 覆盖但需增强族

1. 列表/树/表: 具备基础模型能力，但缺少虚拟化和企业级交互（排序过滤分组冻结）。
2. Web 组件: 有 WebView/WebEngine 基础，但缺少与控件生态深集成（命令/权限/下载/调试联动）。
3. 对话框/输入族: 功能齐全但仍偏基础形态，需增强工作流能力与组合式编辑体验。

---

## BLUE9 改进计划（详细步骤）

## R1 — 属性方法圆满化（先闭环 API 对称性）

目标: 把“真实缺口”从 28 项压降到 <= 5（仅保留设计型例外）。

步骤:
1. 为 ListView/TreeView/TableWidget 增加对称查询 API:
   - `model_ref()` 或 `has_model()`
   - `delegate_ref()`
   - `column_width()` / `row_height()`
2. 为 Menu/ToolBar/RibbonBar/MenuBar 增加 item/menu 状态查询 API:
   - `item_enabled(index)`
   - `item_checked(index)`
   - `menu_enabled(index)`
3. 对组合 setter 增加文档注释，明确“由 min/max getter 组合表达”，避免被误判缺口。
4. 为上述新增 API 补充单元测试（正常路径 + 越界路径）。

验收:
- `cargo test --all-features -q` 通过
- API 对称性扫描缺口显著下降

## R2 — 可扩展性增强（控件能力元数据层）

目标: 提升外部扩展控件与设计器联动能力。

步骤:
1. 引入控件能力描述结构（Capability/PropertySchema）:
   - 可查询属性、事件、命令、默认值
2. 增加 Widget 工厂注册接口:
   - 支持按 kind/name 动态构建
3. 为现有核心控件生成最小元数据（Button/LineEdit/ListView/TableWidget/Menu/ToolBar）。
4. 将 trait 中“重实现门槛”较高的方法分层为 extension traits（沿用 BLUE8 思路）。

验收:
- 核心控件可通过元数据被统一反射和实例化
- 第三方控件接入样板代码下降

## R3 — 现代数据控件包（高优先级）

目标: 补齐企业 GUI 最常用短板。

步骤:
1. 新增 VirtualList（窗口化渲染 + 可变行高）
2. 新增 DataGrid（排序/过滤/分组/冻结列）
3. 新增 TreeTable（树节点 + 表格列）
4. 抽象统一数据源协议（分页、增量刷新、脏区更新）

验收:
- 10 万行数据场景可交互
- 关键操作具备稳定事件与测试覆盖

## R4 — 现代生产力控件包

目标: 补齐开发工具/业务后台高频控件。

步骤:
1. CodeEditor（基础语法高亮 + 诊断 marker + 行号）
2. TerminalView（PTY 适配层 + 文本缓冲）
3. MarkdownEditor + DiffViewer
4. CommandPalette + Breadcrumb + SplitButton

验收:
- 形成“开发者工作台”最小控件闭环
- 与现有快捷键/事件系统联通

## R5 — 富媒体与空间控件包

目标: 把“现代 GUI 覆盖率”从传统桌面提升到综合应用层。

步骤:
1. MediaPlayer（播放控制、进度、音量、全屏）
2. MapView（基础平移缩放 + 标注层）
3. Timeline/Gantt（任务时间轴）
4. Toast/Snackbar/NotificationCenter（非阻塞反馈）

验收:
- 每类至少 1 个可用控件 + 基础测试
- 与主题/样式/触摸交互一致

## R6 — 平台能力对齐与质量门禁

目标: 防止“控件有 API、平台无能力”导致的假覆盖。

步骤:
1. 建立控件-平台能力矩阵（Windows/Linux/macOS/Wayland/Mobile/Harmony）
2. 对每个控件标注实现等级: Native / StateBacked / Placeholder
3. CI 增加多特性组合构建与 smoke 测试
4. 文档输出每轮“能力增量与剩余缺口”

验收:
- 能力矩阵可查询且持续更新
- 新增控件必须同时满足 API、测试、平台实现等级标注

---

## 执行顺序建议

1. 先做 R1（成本低、收益高，快速提升“属性圆满度”）
2. 再做 R2（为后续大规模控件扩展铺轨）
3. R3 与 R4 并行推进（数据控件与生产力控件）
4. R5、R6 作为平台化与产品化收口

---

## 最终目标（BLUE9 里程碑）

1. 属性 API 圆满度: 关键控件达到“可写可查可测”
2. 可扩展性: 新控件可通过统一元数据与工厂机制接入
3. 覆盖率: 从“传统 GUI 全覆盖”提升到“现代 GUI 主流控件族覆盖”
4. 质量门禁: 构建、测试、文档三线闭环，避免回归
