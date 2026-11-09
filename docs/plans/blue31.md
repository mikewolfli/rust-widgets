# BLUE31 — CLI 双向转换器 + 完整 Markdown 编辑器计划

> **来源**：用户 2026-11-10 要求：（1）创建 CLI 转换程序，实现 `json + css ↔ rust-widgets GUI 代码` 双向转换；
> （2）完整支持 Markdown 编辑器（Mermaid、LaTeX / MathJax）；（3）把上一轮（`log-20261109-2.md` 第 19/20 轮）
> 尚未闭合的项追加到本文件末尾。
>
> **本次范围声明**：本轮**只创建本文件**，不修改任何源码、不新增依赖、不提交。下文所有"实施"都是计划条目，
> 不是已完成的代码。
>
> **证据基线**（均为本仓库现存事实，已逐条核实）：
> - 声明式 JSON 引擎：`src/json/mod.rs`（双路径说明）、`src/json/project.rs:192`（`JsonProject`）、
>   `src/json/project.rs:449`（`collect_preorder`）、`src/json/project.rs:590`（`collect_properties`）、
>   `src/json/loader.rs:2123`（`apply_declared_styles` 的四层级联）、`src/json/loader.rs:2429`
>   （`is_loader_owned_key`，已 `pub use` 供生成器复用）。
> - 设计器生成器：`src/designer/generator.rs`（`generate` 619、`GenerationRequest` 591、
>   `is_style_only_property` 575、`availability` 749）、`src/designer/artifact.rs`
>   （`GENERATED_MARKER` 48、`plan_artifacts` 158、`regenerate_into` 245、`file_for` 74）。
> - 生成物两种方言（已提交样例）：`examples/generated_project/src/generated/ui_default.rs`
>   （`Node::new(...).key(...).prop(...).child(...)`）与 `ui_stripped.rs`
>   （`rust_widgets::widget::X::new(...)` + `set_*` + `try_add_child`）。
> - CSS：`src/style/css.rs`（`CssParser` 750、`parse` 765、`parse_prepared_rules` 886、
>   `CssDeclaration` 371、`CssRule` 385、`ParsedCssRule` 406、`apply_declaration_text` 1181、
>   `is_known_property` 1313）、`src/style/selector.rs`（`Selector` 222、`ElementRef` 369）、
>   `src/style/stylesheet.rs`（`StyleSheetManager` 56、`register` 68、`apply_to_context` 132）。
> - 能力/工厂：`src/widget/capability.rs:1506`（`WidgetFactory::create(kind_or_name, geometry, text)`）、
>   `src/widget/capability.rs:1394`（`shared()`）。
> - Markdown 编辑器：`src/widget/special_widgets/markdown_editor/`（9 个文件，共 6035 行；
>   `paint.rs:160-169` 一源行一文本行、`preview.rs:206` `parse_preview_line`、`preview.rs:53`
>   `PreviewRow`、`events.rs:155` 属性表、`markdown_toolbar.rs:64` 七个动作）；
>   `tests.rs:337` 证明图片目前只被当作 `PreviewStyle::Link` 文本。
> - 渲染原语：`src/render/backend/surface.rs`（`draw_path` 2389、`draw_text` 2492、
>   `measure_text` 2560、`push_clip` 2783、`fill_circle`、`draw_line_stroke` 等）。
> - 门控脚本（均已存在）：`tools/check_generated_sources.sh`、`check_generator_output_compiles.sh`、
>   `check_mode_consistency.sh`、`check_generator_agrees_with_registry.sh`、
>   `check_generator_reuses_wire_rules.sh`、`check_event_signal_dyn.sh`、
>   `check_setter_census.sh`、`check_source_structure.sh`。
>
> **判定口径（原则 #105）**：每条"完成"必须同时给出 ①可执行门控/测试名 + 预期输出，②具名排除项及原因。
> 没有门控的收编不算收编；"打算做"不计入闭合。

---

## 0. 结论（一页）

| 项 | 缺口本质 | 方案一句话 | 闭合判据 |
|---|---|---|---|
| **A. CLI `json+css ↔ rust`** | 现有两条路径各自可用但**首尾不相连**：运行时 JSON 路径已支持 `class`/`css` 与四层级联（`apply_declared_styles`），设计器生成器只把 `class`/`css` 当 loader-owned key 跳过，**不把 CSS 解析结果带进生成物**；反向（Rust → JSON）完全没有解析器 | 新建独立包 `crates/rust-widgets-cli`（二进制 `rwui`），**正向**复用 `JsonProject` + `CssParser` + `designer::generate` + `artifact`，**反向**写一个只接受"生成器方言"的受限解析器（不引入 `syn`），并用双向不变量门控锁死 | `rwui check`/`json2rust --check`/`rust2json`/`roundtrip` 全绿；生成的两种 profile 文件与已提交样例逐字节一致；任一不可表达构造都有 `file:line` 诊断 |
| **B. 完整 Markdown** | 编辑器已有 GFM 预览、代码高亮、查找/替换、大纲、工具栏，但**图片只显示为链接文本**、**Mermaid/LaTeX 完全不存在**；且预览是"一源行一文本行"的固定行高模型，无法容纳块级渲染 | 先改渲染模型为**可变高度块布局**（`DisplayRow` + 前缀和），再以 provider 化接入：`ImageProvider`、`DiagramProvider`（内置 Mermaid 子集）、`MathProvider`（内置 TeX 子集）；不可渲染内容一律"可见降级 + 报告"，绝不空白 | Markdown/Mermaid/Math 三组黄金用例 + 无帧内分配回归 + 新事件接线门控 + `demo/markdown_editor` 实跑 |
| **C. 未完成项** | 第 19/20 轮只完成了 gantt / finance / media / web_engine 的具名项，`terminal_view.rs`、`web_engine.rs` 其余分支仍是候选面 | 在本文件 §4 立账：逐面带状态、剩余范围、验收方式 | 每条有名字、有范围、有门控；没有"以后再说"的黑箱 |

两件事的共同纪律：**先复用现有实现路径（#101），再写新代码；先立账，再收编；每一处新 API 都必须被 demo / 测试 / 生成物真实调用**，不允许造死代码。

---

## 1. 共同约束（不得违反）

1. **一个概念一条实现路径（#101）**
   - JSON 文档含义 → `JsonProject::parse`，不许在 CLI 里重写文档语义。
   - 控件名 → 构造器 → `WidgetFactory`，不许维护第二张名字表。
   - loader 自己消费的键 → `crate::json::is_loader_owned_key`（已公开），生成器已在 1264-1271 行引用，CLI 继续复用。
   - CSS 解析/校验 → `CssParser`；样式落到 `WidgetStyle` 的字段级含义 → `style` 模块。
2. **不新增依赖**
   - 反向 Rust 解析器**不用 `syn`**（当前根 crate 无 `syn` 依赖），用无分配友好的手写 lexer + 递归下降，只覆盖生成器方言。
   - CLI 的 JSON 输出用 `serde_json`（根 crate 已有可选依赖；CLI 包显式开启）。
   - 不引入 clap：参数解析手写（`std::env::args_os`），保持零新依赖。
3. **只承认可核实子集**
   - Rust 侧只解析"生成器确实会生成的形状"+ 明确列出的手写扩集；其余语法**报错并给 span**，不做"尽量猜"。
   - Mermaid 只实现子集（§3.4 列明）；LaTeX 只实现 TeX 子集（§3.5 列明）。未实现命令进入报告，不静默。
4. **生成物必须可编译且可复现**
   - 正向输出必须通过 `tools/check_generator_output_compiles.sh`（5 个 profile）。
   - 同一输入两次运行逐字节一致（生成器已有该契约，CLI 不许引入 HashMap 迭代序）。
5. **失败必须可见**
   - 生成器已有 `GenerationReport { unsupported, resolved_at_generation, capacity_overflow }`；
     CLI 必须把三者全部映射成诊断，而不是只打印 `Ok`。
   - 反向解析的丢弃必须进 `LossReport`（kind + span + 原因）。
6. **内存/性能**
   - CLI 是批处理程序：允许一次性读取，但不得在循环里重复 parse CSS / 重复构造 probe 控件。
   - Markdown 的帧路径沿用现有纪律：帧内不分配、不重复 `measure_text`、缓存键必须包含全部输入。

---

## 2. CLI：`rwui`

### 2.1 定位与落点

| 项 | 决定 | 理由 |
|---|---|---|
| 包位置 | `crates/rust-widgets-cli/`（嵌套包，不是根 crate 的 `[[bin]]`） | 根 `Cargo.toml` 的 `include` 明确排除 `crates/**/*`；CLI 不应进入根 crate 的发布物 |
| 包名 / 二进制 | `rust-widgets-cli` / `rwui` | 与 suites 同级的独立工具包 |
| 依赖 | `rust_widgets = { path = "../..", features = ["desktop"] }` + `serde_json` | `designer` 由 `designer_tooling` 在 `desktop` 下默认开启（`src/lib.rs:185-193`）；`json` 由 `full_widgets` 开启（`src/lib.rs:220`）|
| 平台 | 仅 desktop 宿主工具 | `mini`/`embedded` 是生成目标，不是运行环境（`src/designer/mod.rs` 已写明） |
| 工作区 | 根 `Cargo.toml` 目前**没有** `[workspace]` 段；新包独立构建 | 与 `crates/rust-widgets-suites` 现有的"嵌套包 + 路径依赖"模式一致 |

> ⚠️ 备选：若后续要把 CLI 纳入统一 `cargo test --workspace`，需要在根 `Cargo.toml` 新增 `[workspace]` 并登记现有嵌套包；
> 这会改变发布/CI 口径，属于需人工确认项（§5-Q1）。

### 2.2 输入/输出格式（bundle）

**输入（可组合，均向后兼容现有文档）：**

1. **文档 JSON**：现有形状 `{ "<root_widget>": { ... } }`（`JsonProject::parse` 的契约）。
   - 已支持：`id`、`text`/`title`、`class`、节点级 `"css"`、`layout`、`children`、`events`、`actions`、
     loader-owned 键（`x/y/width/height`、`padding`、`margin`、`background`、`text_color`、
     `border_color`、`border_width`、`border_radius`、`min_*`、`max_*` 等，见 `is_loader_owned_key`）。
2. **样式表 CSS**（可选）：
   - `--css <file>` 可重复；
   - 或 bundle 目录下的 `theme.css`（约定名）；
   - 或节点自己的 `"css"` 字符串（已有能力）。
3. **bundle 目录**（可选，推荐）：
   ```
   my-ui.rwui/
     ui.json          # 文档（必需）
     theme.css        # 样式（可选）
     rwui.json        # 清单（可选）：targets、output、function_name、theme、strict
     assets/          # 未来图片/字体资源（本计划不消费，预留）
   ```
   - 清单用 JSON（不是 TOML），避免新增依赖。
   - 清单字段（草案）：
     ```json
     {
       "version": 1,
       "document": "ui.json",
       "stylesheets": ["theme.css"],
       "targets": ["default", "stripped"],
       "width": 640,
       "height": 480,
       "function_name": "build_ui",
       "output_dir": "src/generated",
       "strict": true
     }
     ```

**输出：**

| 产物 | 说明 |
|---|---|
| `ui_default.rs` / `ui_stripped.rs` | 直接调用 `designer::generate` + `artifact` 的既有产物 |
| `ui.resolved.json`（可选） | CSS 已解析进 loader-owned 键后的文档；供审计与二次生成 |
| `theme.generated.css`（可选） | `json2css` 的聚类结果 |
| 诊断（stderr / `--json`） | 结构化诊断列表（§2.8） |

### 2.3 命令面

| 命令 | 方向 | 输入 | 输出 | 退出码 |
|---|---|---|---|---|
| `rwui check <bundle-or-json>` | — | 文档 + CSS | 诊断列表；不写文件 | 0 无错误 / 1 有错误（`--strict` 也把 unsupported 当错误） |
| `rwui json2rust <in> [-o dir]` | JSON+CSS → Rust | 同上 | `ui_default.rs`、`ui_stripped.rs`（或 `--target default|stripped`） | 0/1 |
| `rwui json2rust --check` | 校验 | 同上 | 与已提交生成物逐字节比较，不写 | 0 一致 / 1 漂移 |
| `rwui rust2json <file.rs...> [-o ui.json]` | Rust → JSON | 生成器方言 Rust | 文档 JSON（+ 可选 `--split-css` 输出 CSS） | 0/1 |
| `rwui json2css <in> [-o theme.css]` | JSON → CSS | 文档（含/不含 CSS） | 聚类后的 CSS + 报告 | 0/1 |
| `rwui roundtrip <in>` | 双向不变量 | 文档或 Rust | 报告两个方向的不变量结果 | 0 全部成立 / 1 有反例 |
| `rwui fmt <in>` | 规范化 | 文档 JSON | 稳定排序/缩进后的 JSON（不改变语义） | 0/1 |

**非目标**：`rwui preview`（运行 GUI）、`rwui watch`（文件监视）不在本计划范围；宿主已有 `cargo run`。

### 2.4 正向：`json + css → rust`（步骤与真实 API）

```
[1] 读取 bundle
      └─ JsonProject::parse(&json)                 // 一次解析，失败即停（带 serde 行号）
[2] 收集样式源
      ├─ manifest.stylesheets / --css 文件          // 读取 + 记录 file:line
      └─ 每个 ProjectNode 的 "css" 与 "class"       // 已有键，不新增
[3] 解析 CSS
      └─ CssParser::parse_prepared_rules(text)      // 每条规则：selector + specificity + declarations
         └─ 失败：selector/declaration 位置入诊断    // 不中断整个文档，除非 --strict
[4] 建立元素的静态身份
      ├─ widget 名 → WidgetKind                    // 走 WidgetFactory::shared().capability(name)
      ├─ id / class                                 // 来自 ProjectNode.id / properties["class"]
      ├─ 祖先链                                     // 由 JsonProject 的 path/children 构造
      └─ state = PseudoState::Normal               // 生成期无交互；:hover/:active/:focus 记录为"不可静态解析"
[5] 级联（复用 loader 的次序，见 loader.rs:2123 文档）
      1) 主题：resolved_theme_style_for_shared(kind_css_name, class)
      2) 全局表：StyleSheetManager（CLI 里为空，除非 --stylesheets 注册）
      3) 节点级 "css"
      4) 文档显式键最后覆盖（loader 的既有规则：显式 JSON 键总是赢）
      └─ 产物：每个节点一份 WidgetStyle（或字段级 diff）
[6] 把样式投影成"可生成的属性"
      ├─ 对每个节点构造 probe 控件：WidgetFactory::create(name, Rect::new(0,0,0,0), "")
      ├─ 读 probe.property_names()（WidgetProperties）按名字匹配
      ├─ 有 setter 属性：写入 resolved JSON 的对应键（loader-owned 键优先）
      └─ 无对应属性：记入 StyleGap（字段 + 值 + 原因），不静默
[7] 生成
      └─ designer::generate(&GenerationRequest { json: resolved_json, target, width, height, function_name })
         └─ 返回 GeneratedSource { source, report }
[8] 写盘
      ├─ --check：与 ArtifactPaths::file_for(target) 的现存文件比较
      └─ 否则：artifact::regenerate_into(...)（保留 GENERATED_MARKER）
```

**关键设计点：为什么先生成 `resolved_json`，而不是直接改生成器**

- `is_loader_owned_key` 已经把"loader 自己消费的键"公开给生成器；把 CSS 解析结果写成同一组键，
  生成器无需知道 CSS 的存在，也无需在生成物里链接样式引擎（d-2 的现有约束，见 `generator.rs:567-585`）。
- 反向生成器（`json2css`）与正向解析器共享同一套"键 ↔ 样式字段"映射，避免两套语义。

**必须核实的落地细节（实施前先做探针）：**

- `WidgetCapability` 是否直接暴露 `WidgetKind`（`create_by_kind` 使用了 `capability_by_kind`，
  但节点只给名字）。若没有公开访问器，方案 A：用 `WidgetFactory::create(name, ...)` 后读
  `widget.kind()`；方案 B：在 capability 上补一个只读访问器。**优先 A**（不改库）。
- `property_names()` 对 `background`/`border_radius` 等是否覆盖；不覆盖的字段进 `StyleGap`。
- `ViewEngine` 是否应用 loader-owned 键（本计划假定它**不**应用，因此正向只写
  "控件属性可承载的字段" + 生成器已支持的 `Node.props`；`ui.resolved.json` 供 mode 1 使用）。
  若假定错误，改为在默认 target 的 `create_for` 里生成样式调用，并单独评审 d-2 约束。

### 2.5 CSS 的静态级联：明确支持与明确不支持

| 能力 | 状态 | 处置 |
|---|---|---|
| 类型选择器 `Button`、类 `.primary`、id `#ok` | 已有（`CssSelector`） | 静态匹配 |
| `And`（`Button.primary`）、`Descendant`、`Child` | 已有 | 静态匹配（祖先链来自文档树） |
| `:hover` / `:active` / `:focus` / `:disabled` | 已有运行时匹配 | **生成期不可解析**：进 `StyleGap`（`kind=dynamic-pseudo`），并在生成注释中列出 |
| 优先级（specificity） | `ParsedCssRule::specificity` 已算好 | 按升序应用（与 `StyleSheet` 的既有排序一致） |
| 源顺序 | 解析保序 | 同权重按文件/声明顺序 |
| `!important` | **解析器无此字段**（`CssDeclaration` 只有 property/value） | 计划内新增：解析时剥离 `! important` 并记 `important: bool`；级联按"普通 < important"两档；门控测试 |
| 简写（`padding`、`margin`、`border-radius` 1-4 值、`border`） | `CssParser::apply_declaration_text` 支持的部分 | 复用；不支持的值进诊断 |
| 未知属性 | `is_known_property` | 报 `unknown-property`，不写入、不中断 |
| 变量/`calc()`/媒体查询/`@import` | 不支持 | 报 `unsupported-css-construct`；`@import` 明确拒绝（避免隐藏 I/O） |

> **为何不直接调用 `StyleSheetManager::apply_to_context`**：它需要一个活控件与 `RenderContext`；
> CLI 没有渲染上下文。计划是把"匹配 + 合并"抽成纯函数（见 §2.10 阶段 P2），
> 让 loader 的活路径和 CLI 的静态路径共用实现（#101），而不是复制一份选择器逻辑。

### 2.6 反向：`rust → json + css`

#### 2.6.1 接受的方言（白名单）

**方言 A：默认 target（`ui_default.rs` 形状）**

```rust
let tree = Node::new("window").key("root")
    .prop("title", CapabilityValue::String(String::from("Demo")))
    .child(Node::new("button").key("n_1")
        .prop("text", CapabilityValue::String(String::from("Go")))
        .prop("enabled", CapabilityValue::Bool(false)));
```

- 允许：`Node::new(<string>)`、`.key(<string>)`、`.prop(<string>, <capability value>)`、`.child(<node>)`、
  `let <ident> = <node>;`、`pub fn <ident>() { ... }`、注释、尾逗号、换行。
- `CapabilityValue` 允许：`String(String::from("…"))`、`String("…".to_string())`、`Int(i64)`、
  `Float(f64)`、`Bool(bool)`、`UInt(u64)`、`Tuple(vec![...])`、`Records(vec![...])`（按生成器实际产物放开）。
- 忽略但校验：`GeneratedTree`、`View` impl、`create_for` 样板（生成器固定模板）。

**方言 B：Stripped target（`ui_stripped.rs` 形状）**

```rust
let mut root = rust_widgets::widget::Window::new(String::from("Demo"), Rect::new(0, 0, 640, 480));
let n_0 = rust_widgets::widget::Label::new(String::from("Hi"), Rect::new(0, 0, 640, 156));
let n_1 = {
    let mut n_1 = rust_widgets::widget::Button::new(String::from("Go"), Rect::new(0, 162, 640, 156));
    n_1.set_enabled(false);
    n_1
};
root.base_mut().try_add_child(n_1.base().id());
```

- 允许：`let [mut] <ident> = rust_widgets::widget::<Type>::new(<args>);`、块表达式返回控件、
  `set_<name>(<literal>)`、`root.base_mut().try_add_child(<ident>.base().id());`。
- 类型名 → JSON 键：`Widget::<Type>` 去 `rust_widgets::widget::` 后转小写（与 `JsonProject::parse`
  的 `to_lowercase` 一致）；无法解析的 Type 报 `unknown-widget`。
- 属性名：`set_<name>` → 用 probe 控件的 `property_names()` 验证 `<name>` 是否可写；
  不可写则按 RW-44 的四类出口（blue30 §1.2）记 `programmatic-only`/`decomposed`，并保留源码注释。

#### 2.6.2 解析器结构（不引入 `syn`）

| 层 | 职责 | 失败行为 |
|---|---|---|
| `lexer.rs` | token：ident、string（含转义、raw string 明确不支持）、number、punct、注释 | 报 `file:line:col` |
| `ast.rs` | `Expr::{Call, Method, Lit, Path, Block, Let}` 的最小树 | 未识别语法进 `LossReport` |
| `dialect_default.rs` | `Node` 建造链 → `NodeAst` | 非链式调用 → `unsupported-construct` |
| `dialect_stripped.rs` | `let`/块/`set_*`/`try_add_child` → `WidgetAst` | 控制流（`if`/`for`/`match`/闭包）→ 拒绝 |
| `emit_json.rs` | AST → `serde_json::Value`（与 `JsonProject` 同形） | 每次丢弃都带 span |
| `verify.rs` | 输出重新 `JsonProject::parse`，再走 `JsonLoader::load`（desktop） | 解析失败即门控失败 |

**明确拒绝的构造**（报 `unsupported-rust`，不尝试求值）：
宏（`format!` 等除白名单 `vec!` 外）、闭包、`match`/`if`/循环、`include!`、`unsafe`、
外部常量/函数调用（除 `Rect::new`、`String::from`、`CapabilityValue::*`、`vec!`）、
`cfg!`/条件编译、跨函数移动控件。

#### 2.6.3 往返不变量

- **R1（正 → 反）**：`canonical(json2rust(doc))` 再 `rust2json` 得到的文档，与 `doc` 在
  **生成器关心的字段集**上等价；CSS 解析结果以 `ui.resolved.json` 为准，差异必须显式列出
  （例如 `class` 被展开为具体键）。
- **R2（反 → 正）**：对生成器产物（两个 target）执行 `rust2json` 再 `json2rust`，
  输出与输入**逐字节相等**（格式化后）。
- **R3（CSS）**：`json2css(json2rust(doc))` 重新解析后，样式指纹（`WidgetStyle` 的规范化字段元组）
  与正向解析结果一致。

门控：`tests/rwui_roundtrip_test.rs` 对 `examples/generated_project` 与至少 6 个手写样例跑 R1-R3。

### 2.7 CSS 反向生成（`json2css`）

1. 对每个节点得到规范化 `WidgetStyle`（与正向同一路径）。
2. 只保留 CSS 能表达的字段（用 `CssParser::is_known_property` 反查声明名）。
3. 聚类：
   - 按 `(kind, 样式指纹)` 分组 → 生成 `.rw-<8 位 hash>` 类；
   - 只有一份的样式 → 生成 `#<id>` 规则（若节点无 id，则用 kind + 唯一类）；
   - 组内仍不同的字段 → 保留为该节点的内联 `"css"`（现有能力）。
4. 输出顺序：`Universal → Kind → Class → Id`，同层按名字排序；每类规则内声明按固定字段顺序。
5. 报告：不可表达的字段（如 `surface`、`shadow`、渐变）→ `StyleGap { field, reason }`。

闭合判据：生成 CSS 经 `CssParser::parse_prepared_rules` 重新解析、再与 `WidgetStyle`
指纹比较，**0 差异**（除报告中的字段）。

### 2.8 诊断与退出码

```rust
pub struct Diagnostic {
    pub code: &'static str,      // RWCLI-001 …
    pub severity: Severity,      // Error | Warning | Note
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub node_path: Option<String>, // "<root>/[0]/[2]" 或 ProjectNode.key
    pub message: String,
    pub hint: Option<String>,
}
```

| code | 含义 | 例 |
|---|---|---|
| RWCLI-001 | JSON 解析失败 | `ui.json:3:5` |
| RWCLI-002 | 文档结构不合契约（顶层多键/深度超限） | `JsonProject::parse` 的错误原文 |
| RWCLI-003 | CSS 解析失败 | 选择器/声明位置 |
| RWCLI-004 | 未知 CSS 属性 | `outline-style` |
| RWCLI-005 | 生成期不可解析的选择器 | `:hover` |
| RWCLI-006 | 样式字段无属性载体（StyleGap） | `border_radius` 在无 setter 的控件上 |
| RWCLI-007 | 生成器报告的 unsupported 节点 | `GenerationReport.unsupported` |
| RWCLI-008 | 容量溢出 | `GenerationReport.capacity_overflow` |
| RWCLI-009 | 反向解析：不支持的 Rust 构造 | `file.rs:12:9` |
| RWCLI-010 | 反向解析：setter 无对应属性 | `set_track_highlight` |
| RWCLI-011 | 生成物漂移（`--check`） | 文件路径 |
| RWCLI-012 | I/O 失败 | 路径 + OS 错误 |

- `--json`：一行一个 JSON 对象（NDJSON），便于 CI。
- 退出码：`0` 成功；`1` 有 Error；`2` I/O。
- `--strict`：Warning 中的 `unsupported`/`StyleGap` 也按 Error 计（默认只报不拦，与生成器"成功但列缺口"的既有契约一致）。

### 2.9 门禁与验收（CLI）

```
# 1. 现有样例必须零诊断
rwui check examples/generated_project                    → 0 diagnostics, exit 0

# 2. 已提交生成物不漂移
rwui json2rust examples/generated_project --check        → exit 0（两种 target）

# 3. 反向回到原文档
rwui rust2json examples/generated_project/src/generated/ui_default.rs \
                examples/generated_project/src/generated/ui_stripped.rs -o /tmp/out.json
#    与 project.json 的生成器字段集等价（测试断言，不靠人眼）

# 4. 双向不变量
rwui roundtrip examples/generated_project                → R1/R2/R3 全绿

# 5. 既有生成器门控不回归
bash tools/check_generator_output_compiles.sh
bash tools/check_mode_consistency.sh
bash tools/check_generator_agrees_with_registry.sh
bash tools/check_generator_reuses_wire_rules.sh
bash tools/check_generated_sources.sh

# 6. 新包自身质量门
cargo fmt --all -- --check
cargo clippy -p rust-widgets-cli --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p rust-widgets-cli --no-deps
```

测试文件（计划）：
- `crates/rust-widgets-cli/tests/forward_test.rs`：CSS 级联、`!important`、`:hover` 报告、StyleGap。
- `crates/rust-widgets-cli/tests/reverse_test.rs`：两种方言、拒绝构造的 span、setter 映射。
- `crates/rust-widgets-cli/tests/roundtrip_test.rs`：R1-R3。
- `crates/rust-widgets-cli/tests/diagnostics_test.rs`：code/severity/NDJSON/退出码。

### 2.10 分阶段（CLI）

| 阶段 | 内容 | 交付物 | 依赖 |
|---|---|---|---|
| **P0** | 包骨架 + `check` + 诊断模型 + 现有样例零诊断 | `crates/rust-widgets-cli` 可 `cargo run` | 无 |
| **P1** | 正向：CSS 收集/解析/静态匹配（不含主题）+ `json2rust` | 与已提交 `ui_*.rs` 逐字节一致 | P0 |
| **P2** | 抽出共享的"选择器匹配 + 样式合并"纯函数（loader 与 CLI 共用；#101） | loader 测试不变 + CLI 复用 | P1 |
| **P3** | 主题层 + `!important` + `json2css` 聚类 | R3 全绿 | P2 |
| **P4** | 反向方言 A（default） | R2(default) 全绿 | P0 |
| **P5** | 反向方言 B（stripped）+ setter 映射 + RW-44 分类 | R2(stripped) 全绿 | P4 |
| **P6** | `roundtrip` + CI 接入 + 文档（README/本计划回写） | 全部门控 | P1-P5 |

### 2.11 具名排除（CLI）

- 不解析任意 Rust（宏/控制流/闭包/外部 crate 类型）；不试图把手写业务逻辑转成 JSON。
- 不支持 `@media`/`@import`/CSS 变量/`calc()`；不支持伪元素（`::before`）。
- 不在 CLI 内渲染控件（没有窗口/渲染上下文）；`--preview` 明确不做。
- 不改变 mode 1（运行时 JSON）的既有语义；CLI 只增加生成期解析。
- 不新增 crate 依赖；不引入 `syn`/`clap`/`regex`。

---

## 3. 完整 Markdown 编辑器

### 3.1 现状事实（取证）

| 能力 | 现状 | 出处 |
|---|---|---|
| 文档模型 / 编辑 / 撤销 | 已有（`document.rs` 268 行、`editor.rs` 1536 行） | `markdown_editor/` |
| GFM 块：标题、列表、任务、引用、表格、分隔线、围栏 | 已有 | `preview.rs:206` `parse_preview_line` |
| 围栏代码高亮 | 已有（复用 `code_editor::BuiltinHighlighter`） | `preview.rs` 顶部 use |
| 行内强调 / 删除线 / 行内代码 / 链接 | 已有（`PreviewStyle`） | `preview.rs:27-45` |
| 图片 | **只作为 Link 文本**，不加载、不绘制 | `tests.rs:337` 断言 `PreviewStyle::Link` |
| Mermaid | **不存在** | 全目录 grep 无匹配 |
| LaTeX / MathJax | **不存在** | 全目录 grep 无匹配 |
| 预览行模型 | **一源行 = 一文本行**（`TEXT_ROW_HEIGHT` 固定） | `paint.rs:160-169` |
| 查找 / 替换 | 已有（`find.rs` 922 行） | `find.rs` |
| 大纲面板 | 已有（`outline.rs` 420 行） | `outline.rs` |
| 工具栏 | 已有 7 个动作 | `markdown_toolbar.rs:64` |
| 属性面 | `text`、`preview_mode`、`line_count`、`word_count`、`heading_count`、`cursor_line`、`show_scrollbar`、`gfm_extensions`、`code_highlight`、`undo_stack_limit` | `events.rs:155` |
| 事件面 | `text_changed`、`preview_mode_changed`、`selection_changed`、`heading_activated` | `editor.rs:35-41` |
| 演示程序 | **无**（仅测试引用） | 全仓 grep |
| 图片编解码 | 根 crate 的 `image` 模块只支持 PNG/JPEG/BMP/QOI/Farbfeld/PNM；GIF/WebP/TIFF/AVIF/ICO/SVG 明确返回 `Err` | `src/lib.rs:208-219` |

### 3.2 "完整"的定义（验收清单）

| # | 条目 | 现状 | 本次目标 |
|---|---|---|---|
| MD-1 | CommonMark 0.30 块级与行内语法（含嵌套强调、转义、代码跨度、引用嵌套） | 部分 | 补齐并用官方子集 fixture 锁死 |
| MD-2 | GFM：表格、任务列表、删除线、自动链接 | 已有 | 回归测试补全（编辑语义见 MD-10） |
| MD-3 | 围栏代码 + info string 高亮 + 未知语言回退 | 已有 | 未知语言显式标记，不静默降级 |
| MD-4 | 图片（块级/行内、alt、title、失败占位） | 缺 | `ImageProvider` + 占位/缓存 |
| MD-5 | Mermaid（flowchart + sequence 子集） | 缺 | 内置渲染器 + provider 接缝 |
| MD-6 | LaTeX / MathJax（行内 + 展示 + TeX 子集） | 缺 | 内置排版器 + provider 接缝 |
| MD-7 | 预览滚动与源行同步、点击链接发信号 | 缺 | `link_activated` + 双向滚动同步 |
| MD-8 | 编辑完整性：列表/引用自动续行、表格 Tab、围栏自动闭合、粘贴 URL/图片 | 部分 | 明确键位与行为，逐条测试 |
| MD-9 | 工具栏覆盖新增块（图片/数学/图表/表格/任务/分隔线） | 7/15 | 扩展 token 表，保持前 7 个不动 |
| MD-10 | 能力/属性/事件/命令全接线（含 `event_signal_dyn`） | 部分 | 新属性/事件/命令 + 门控 |
| MD-11 | 性能预算（大文档、长行、多图/多图块） | 无基线 | 帧内零分配 + 缓存键 + 上限 |
| MD-12 | Demo（打开/保存/工具栏/大纲/预览/图表/数学） | 无 | `demo/markdown_editor`（`main.rs` 桩 + `app.rs`） |

### 3.3 渲染模型改造：可变高度块（MD-4/5/6 的前提）

**问题**：`paint.rs` 用 `y = rect.y + TEXT_TOP + row * TEXT_ROW_HEIGHT` 直接由"源行号"算 y，
并且滚动、选择、光标都以源行为单位。块级渲染（一张图、一个流程图、一个展示公式）需要 N 行高度，
现在的模型无法表达。

**方案：`PreviewLayout`（显示行模型）**

```rust
pub(crate) struct PreviewLayout {
    /// 每个源行对应的显示行起点（前缀和），长度 = 源行数 + 1
    starts: Vec<u32>,
    /// 每个源行的显示高度（普通行 = 1，块 = N；0 表示被块吞并的后续源行）
    heights: Vec<u16>,
    /// 块跨度：源行区间 → 块种类
    blocks: Vec<BlockSpan>,
}

pub(crate) struct BlockSpan {
    pub first_line: usize,
    pub last_line: usize,   // 含
    pub kind: BlockKind,    // Image { .. } | Diagram { .. } | Math { .. } | Table?（表格暂按行渲染）
}
```

- **构建**：在 `blocks`/`table_header` 的既有 revision 计算处扩展；Mermaid / `$$` / ```math / 图片
  的块区间一次扫描得出；普通行高度 1。
- **绘制**：`y_for_line(line)` 由 `starts` 查表；块内部由 `render_block` 画进 `height * TEXT_ROW_HEIGHT`
  的矩形；`push_clip` 保证不越界。
- **滚动/可见行**：`visible_start/line_count` 改用显示行；`scroll_page`/`scroll_to_line` 换算。
- **选择与光标**：编辑模式不变（仍按源行/列）；预览模式下点击只触发链接/块激活，不产生文本选择。
- **缓存**：
  - 行解析缓存沿用 `PreviewKey`（内容 + block 上下文 + 宽度/DPI）；
  - 新增 `BlockKey { source_hash, width, theme_rev, dpi_bits, renderer_rev }` → `Rc<RenderedBlock>`；
  - 主题/DPI/宽度变化才重排块。

**兼容性**：`blocks` 的现有语义（围栏/表格）保留；新模型只是额外提供"高度表"。若 `PreviewLayout`
构建失败（极端深度），回退到旧的"一行一行"路径并报告。

### 3.4 Mermaid（MD-5）

#### 3.4.1 接缝

```rust
/// 图表渲染结果：要么是一组可直接绘制的场景对象，要么是"不支持"的报告。
pub enum DiagramOutcome {
    Scene(DiagramScene),
    Unsupported(Vec<DiagramIssue>),
    Failed(String),
}

pub trait DiagramProvider {
    fn render(&self, source: &str, theme: &DiagramTheme, max_width: u32) -> DiagramOutcome;
}

pub struct DiagramTheme { pub ink: Color, pub surface: Color, pub line: Color, pub accent: Color }
```

`DiagramScene` 只含渲染原语（矩形/圆角矩形/线/折线/文本/箭头），由编辑器用
`RenderContext::{fill_rect, fill_rounded_rect, draw_line_stroke, draw_path, draw_text, measure_text}`
绘制——不生成 SVG 字符串，不引入新依赖。

注册：`MarkdownEditor::set_diagram_provider(Arc<dyn DiagramProvider>)`；未注册时用内置实现。
内置实现同样实现该 trait，所以"内置"与"宿主覆盖"是同一条路径（#101）。

#### 3.4.2 内置 Mermaid 子集（分两阶段）

| 阶段 | 语法 | 布局 |
|---|---|---|
| **D1** | `flowchart TD/TB/LR/BT`：节点 `A[text]`、`A(text)`、`A{text}`、边 `A --> B`、`A -- text --> B`、`A --- B`（无箭头）、`subgraph`/`end`（仅分组框） | 分层（BFS 去环）→ 层内重心排序 → 网格定位 → 正交连线 + 箭头；节点尺寸由 `measure_text` + padding |
| **D2** | `sequenceDiagram`：`participant A`、`A->>B: msg`、`A-->>B: msg`、`Note over A,B: text`、`loop`/`alt`/`end`（框） | 参与者列 + 生命线 + 消息行 + 激活条；宽度按最长消息测量 |
| **D3**（可选） | `stateDiagram-v2` 基本状态/转移 | 复用 D1 的分层器 |

**明确不支持（报告 `DiagramIssue`，画占位框 + 源码摘要）**：`classDiagram`、`gantt`、`pie`、
`erDiagram`、`journey`、`mindmap`、`timeline`、`quadrantChart`、`xychart`、`--config`、
主题指令、点击/链接、动画。理由：这些需要全新的语义模型，且本计划不引入外部 crate。

**防护上限**（全部可配置并报告）：
- 最大节点数 200、最大边数 500、最大层数 40、最大源文本 64 KiB；
- 超限 → `DiagramIssue::TooLarge` + 占位，不 panic、不 O(n²) 爆炸；
- 布局必须处理环（BFS 分层天然去环）、自环（不画或画小回环）、重复边（合并/偏移）。

#### 3.4.3 主题

`DiagramTheme` 从编辑器 `WidgetStyle` 推导：
`ink = text_color`、`surface = background_color`、`line = border_color`、`accent = primary 导出色`。
所有颜色走主题，禁止硬编码（沿用项目主题纪律）。

### 3.5 LaTeX / MathJax（MD-6）

#### 3.5.1 接缝

```rust
pub enum MathOutcome {
    Layout(MathBox),
    Unsupported(Vec<MathIssue>),
    Failed(String),
}

pub trait MathProvider {
    fn layout(&self, tex: &str, display: bool, font: &Font, max_width: u32) -> MathOutcome;
}

pub struct MathBox { /* 宽高、基线、原子列表（文本/线/矩形） */ }
```

内置实现：`markdown_editor/math.rs`。

#### 3.5.2 TeX 子集（D1 交付）

| 类别 | 支持 |
|---|---|
| 分组/转义 | `{ }`、`\ `、`\,`、`\;`、`\!`、`\%`、`\$`、`\&`、`\#`、`\_` |
| 上下标 | `^`、`_`（含 `^{...}`）；连续上下标按 TeX 规则合并 |
| 分式 | `\frac{a}{b}`、`\dfrac`、`\tfrac` |
| 根式 | `\sqrt{x}`、`\sqrt[n]{x}` |
| 大型运算符 | `\sum`、`\prod`、`\int`、`\iint`、`\oint`、`\lim`（行内用侧标、展示用上下限） |
| 希腊/符号 | 常用希腊字母、关系符、箭头、集合符、`\infty`、`\partial`、`\nabla`、`\pm`、`\times`、`\cdot`、`\dots` |
| 定界符 | `\left( ... \right)`、`\left[`、`\left\{`、`\left|`、`\middle|`；自动伸缩 |
| 文本 | `\text{...}`、`\mathrm`、`\mathbf`、`\mathit`、`\operatorname` |
| 重音 | `\hat`、`\bar`、`\vec`、`\dot`、`\ddot`、`\tilde` |
| 环境 | `matrix`/`pmatrix`/`bmatrix`/`vmatrix`、`cases`、`aligned`（简单按 `&` 与 `\\`） |
| 颜色 | `\color{...}`（仅命名色/hex；未知报错） |

**D2 可选**：`\begin{align}`、`\overbrace`/`\underbrace`、`\stackrel`、化学式（不承诺）、
`\newcommand`（**明确不做**，避免宏展开失控）。

**明确不支持**：`\usepackage`/`\documentclass`/`\input`/`\includegraphics`/`\href`/`\begin{tikzpicture}`；
JS 版 MathJax 的扩展包（`\require`、`mhchem` 等）。这些一律 `MathIssue::Unsupported`。

#### 3.5.3 定界与排版语义

- 行内：`$...$`、`\(...\)`；展示：`$$...$$`、`\[...\]`；围栏：```` ```math ```` / ```` ```latex ````。
- 行内公式作为**原子**参与行内排版（不换行）；展示公式独占块，居中（可配置左对齐）。
- 基线：`MathBox` 记录 ascent/descent；行内公式与文本基线对齐；展示公式在块内垂直居中。
- 字体回退：符号优先 Unicode；缺字时用 ASCII 近似（`∑`→`S` 等）并在 `MathIssue` 记 `GlyphFallback`；
  绝不画空白。
- 嵌套深度上限 32；超出 → `MathIssue::DepthLimit` + 以文本形式显示源码。

#### 3.5.4 MathJax 兼容性口径

- **兼容的是"输入语法与定界符"**，不是 MathJax 运行时；不嵌入 JS 引擎（`markdown_editor` 属于
  根 crate，不依赖 `js-engine`）。
- 提供 `math_supported_commands() -> &'static [&'static str]` 与 `unsupported_math()` 报告，
  宿主可据此在工具栏禁用不支持的按钮（与 Lottie 的 `unsupported_shapes()` 同一模式）。

### 3.6 图片（MD-4）

- **`ImageProvider` 接缝**（宿主加载/解码，控件不阻塞 UI）：
  ```rust
  pub enum ImageState { Ready(Arc<Image>), Pending, Failed(String) }
  pub trait ImageProvider { fn get(&self, src: &str, width: u32) -> ImageState; }
  ```
- 块级图片：独占块，按 `max_width` 等比缩放；无 provider / `Pending` → 占位框（alt + 尺寸）；
  `Failed` → 错误框（alt + 原因），并进 `render_report()`。
- 行内图片：仅在字体行高内按比例缩放，超出即退化/占位（不撑破行高）。
- alt 文本在未加载/失败时始终可见（可访问性）。
- 缓存：`(src, width, dpi)`；解码由宿主/`image` 模块完成；编辑器不在帧内解码。
- 明确：GIF/WebP/TIFF/AVIF/ICO/SVG 在当前 `image` 模块返回 `Err`（见 `src/lib.rs:208-219`），
  本计划用占位图 + 报告，不新增编解码器。

### 3.7 编辑完整性（MD-8/9）

| 行为 | 规则 | 测试 |
|---|---|---|
| 回车续行 | 在 `- `、`1. `、`> `、`- [ ] ` 行尾回车自动续前缀；空前缀行回车清除前缀 | `enter_continues_list_markers` |
| 表格 | `Tab`/`Shift+Tab` 在单元格间移动；最后一格 `Tab` 追加一行；`Enter` 在单元格内不退出 | `table_tab_moves_between_cells` |
| 围栏 | 输入 ``` 后回车自动补闭合围栏；删除开围栏则同步删除闭围栏（仅当空块） | `fence_auto_closes` |
| 粘贴 | 选中的纯文本若形如 URL，粘贴时生成 `[选中](URL)`；文件路径以图片扩展名结尾时生成 `!(...)` | `paste_url_wraps_selection` |
| 快捷键 | `Ctrl+B/I/K`、`Ctrl+Shift+7`（有序列表）、`Ctrl+Shift+8`（无序）、`Ctrl+Shift+.`（引用）、`Ctrl+E`（行内代码） | 快捷键表测试 |
| 工具栏 | 新增 token：`strike`、`ordered_list`、`task`、`table`、`image`、`math`、`diagram`、`hr` | token 表 + `execute` |
| 兼容 | 前 7 个 token 与顺序不变（宿主已存的动作列表不失效） | 快照测试 |

> ⚠️ 快捷键需与全局 `shortcut` 系统的既有绑定做冲突检查（`src/shortcut`），冲突时以全局为准并报告。

### 3.8 预览联动与信号（MD-7）

- 新增信号：`link_activated: Signal1<String>`（点击预览中的链接；`event_signal_dyn` 必须加分支）。
- 新增信号：`block_activated: Signal1<(usize, String)>`（源行 + 块种类，供宿主打开图片/图表查看器）。
- 新增信号：`render_gap: Signal1<String>`（一个块降级时发一次，kind + 原因）。
- 滚动同步：编辑区滚动 → 预览定位到对应块；预览滚动 → 编辑区 `goto_line`（可关闭，
  属性 `preview_scroll_sync`）。
- 预览中的链接命中：`PreviewRun` 目前只存文本/样式；计划加 `RunLink { target: Option<String> }`
  或在 `ParsedLine` 增 `links: Vec<(Rect, String)>`（绘制时记录矩形，命中用矩形）。

### 3.9 能力 / 属性 / 事件 / 命令接线（MD-10）

**新增属性**（`events.rs` 的 `property_names`）：

| 属性 | 类型 | 说明 |
|---|---|---|
| `diagrams_enabled` | Bool | 关掉后 Mermaid 围栏按普通代码块显示 |
| `math_enabled` | Bool | 关掉后 `$...$` 按字面文本显示 |
| `images_enabled` | Bool | 关掉后图片按链接文本显示 |
| `preview_scroll_sync` | Bool | 双向滚动同步 |
| `diagram_max_width` | UInt | 块宽上限（0 = 控件宽） |
| `render_report` | String（只读） | 一次 JSON 摘要：降级块数/原因（或改为 `Tuple`） |

**新增事件**（`event_signal_dyn` + capability 发布名 + `check_event_signal_dyn.sh`）：
`link_activated`、`block_activated`、`render_gap`。

**新增命令**：
`insert_image`、`insert_math`、`insert_diagram`、`insert_table`（带 payload 的命令走
`default_command` 的既有形态）、`toggle_diagrams`、`toggle_math`。

**Provider 注册**（非能力：trait object 不能进 `CapabilityValue`）：
`set_image_provider(Arc<dyn ImageProvider>)`、`set_diagram_provider(Arc<dyn DiagramProvider>)`、
`set_math_provider(Arc<dyn MathProvider>)`。

**门控**：
- `tools/check_setter_census.sh`（新 setter 必须分类）；
- `tools/check_event_signal_dyn.sh`（新事件必须有 arm）；
- `cargo doc -D warnings`（新公开项必须有文档链接）。

### 3.10 性能预算（MD-11）

| 指标 | 预算 | 测量 |
|---|---|---|
| 帧内分配 | 0（预览/编辑滚动帧） | 现有 `tools/_scan_draw_alloc.py` 纪律 + 单测计数 |
| 1 MB 文档、200 行可见 | < 2 ms/帧（software backend，release） | 基准测试（`examples`/bench） |
| 块重排 | 仅在 source/width/theme/dpi 变化时 | 缓存命中测试 |
| 图片 | 不在帧内解码；同图同宽只解码一次 | provider 计数 |
| 图表 | 单块 ≤ 5 ms；节点/边上限见 §3.4.2 | 场景构建计时 |
| 数学 | 单公式 ≤ 2 ms；深度 ≤ 32 | 排版计时 |
| 内存 | 块缓存按"存活块"驱逐；行缓存沿用现有 bound | 缓存长度测试 |

### 3.11 门禁与验收（Markdown）

```
cargo fmt --all -- --check
cargo test --lib markdown_editor          # 现有 + 新增
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
bash tools/check_event_signal_dyn.sh
bash tools/check_source_structure.sh
# 新 demo
cargo run -p markdown-editor-demo         # demo/markdown_editor
```

新增测试组（计划名）：
- `commonmark_subset_fixtures`：标题/强调嵌套/链接/转义/引用嵌套/表格/任务/围栏。
- `preview_layout_maps_blocks_to_rows`：块高度/前缀和/滚动边界。
- `image_block_uses_provider_and_falls_back`：Ready/Pending/Failed 三态 + alt。
- `mermaid_flowchart_lays_out_and_draws`：节点/边/环/自环/上限。
- `mermaid_sequence_orders_messages`：参与者/消息/Note/框。
- `mermaid_unsupported_types_report`：gantt/pie 等 → 占位 + 报告。
- `math_fractions_scripts_and_roots_metrics`：黄金尺寸/基线。
- `math_environments_and_delimiters`：matrix/cases/`\left...\right`。
- `math_unsupported_command_is_visible`：`\usepackage` → 报告 + 可读降级。
- `preview_link_activation_emits`：命中/未命中/链接矩形。
- `capability_surface_covers_new_state`：属性/事件/命令 round-trip。
- `large_document_stays_within_frame_budget`：性能回归。

### 3.12 分阶段（Markdown）

| 阶段 | 内容 | 可独立交付 | 依赖 |
|---|---|---|---|
| **M0** | `PreviewLayout` + 块跨度 + 绘制重构（不含新块种类） | 现有预览行为不变、测试全绿 | 无 |
| **M1** | 图片块/行内 + `ImageProvider` + 三态占位 + 链接信号 | 图片可显示 | M0 |
| **M2** | 数学：行内 + 展示 + TeX D1 子集 + 报告 | LaTeX 可用 | M0 |
| **M3** | Mermaid D1（flowchart） | 流程图可用 | M0 |
| **M4** | Mermaid D2（sequence）+ 主题/上限 | 时序图可用 | M3 |
| **M5** | 编辑完整性 + 工具栏扩展 + 能力接线 | 完整编辑体验 | M1-M4 |
| **M6** | Demo `demo/markdown_editor` + 性能门控 + 文档 | 可运行样例 | M5 |

每阶段结束都必须：`cargo fmt` + 该模块测试 + clippy + doc + 结构门控；不允许把新 public API 留成死代码。

### 3.13 具名排除（Markdown）

- 不实现 Mermaid 的 class/gantt/pie/er/journey/mindmap/timeline/quadrant/xychart。
- 不实现 TeX 宏定义、`\usepackage`、外部文件、TikZ、化学式。
- 不嵌入 MathJax/JS 运行时；不做 HTML 渲染（原始 HTML 按现有行为作为文本显示）。
- 不新增图片编解码器（GIF/WebP/SVG 等仍走占位 + 报告）。
- 不做 WYSIWYG（编辑区仍是源码 + 预览切换）。
- 不做文件 I/O（由 demo/宿主负责）。

---

## 4. 未完成项（追加：`docs/log/log-20261109-2.md` 第 19/20 轮余项）

> 本节是"最后的未完成"的账。每一项都有名字、范围、验收方式；没有"以后再说"的黑箱。
> 基线门禁（第 20 轮末实跑）：fmt clean；suites all-features **1299**；suites 默认 **82**；
> suites clippy clean；root lib **9799**；`cargo doc -D warnings` clean；结构门控 PASS。

### 4.1 已闭合（记录，不计入未完成）

- 第十九轮：gantt 10 项、finance depth/layout 8 项、media rive/lottie 10 项，共 26 用例。
- 第二十轮：`form_widgets/rules.rs` 全文复核无缺陷；`webview/web_engine.rs` 5 项修复 + 4 用例。
- 文档：`docs/log/log-20261109-2.md` 已追加第 19/20 轮；`docs/issues/issue-20261106.md` 已追加 §13.13/§13.14。

### 4.2 未闭合：`media_playback` 的 `Widget::tick` 语义审计

- 范围：`LottieWidget::tick` / `RiveWidget::tick` 的 trait 桥接已改为"是否还欠帧"，
  但**同族其它动画控件**（`AudioVisualizer`、`CameraPreview`、`AnimatedImage`、`Spinner`、
  `ProgressCircle`、`CursorBlink` 家族）尚未逐一按同一契约复核。
- 验收：对每个实现 `Widget::tick` 的控件，断言"返回 true ⇔ 下一帧还会变化"；
  用 `tools/animation_driver_scan.py` + 新增表驱动测试；发现反例即修。
- 风险：低（不影响当前已闭合项），但属于"同类未扫完"。

### 4.3 未闭合：`devtools/terminal/terminal_view.rs`（4387 行）

- 已复核且无缺陷的局部：`set_grid`/`announce_grid`（264-285）、`set_scrollback_lines`/`trim_scrollback`
  （293-327）、`set_wrap_output`（335-344）、`tick`（905-924）、`total_output_rows`/`max_scroll_offset`
  （660-678）、`scroll_output`（684-695）。
- 未逐行复核：
  1. `handle_event`（1492 起）：修饰键、选择拖拽、preedit、IME、滚动条命中、快捷键冲突；
  2. `draw`（1799 起）：输出缓存与行高、缩放/裁剪、滚动条几何、光标绘制；
  3. 选择模型：`apply_output_selection`/`clamp_selection_offset`/anchor/head 的边界；
  4. 查找联动：`TerminalFindBar` 的队列/进度与 `revision` 的失效时机；
  5. 屏幕模式（`feed_screen`）与 scrollback 的 `revision` 一致性。
- 验收：全文精读 + 每个发现一条回归用例；`cargo test --all-features --lib -- devtools::terminal` 全绿；
  结构与 clippy 门控不变。
- 风险：中（文件大、状态多；按 §4.6 的分组顺序进行）。

### 4.4 未闭合：`webview/web_engine.rs`（3651 行）

- 已修复（第 20 轮）：加载进度信号、cookie 替换清理、下载命名、导航状态去重、滚轮饱和。
- 未逐行复核：
  1. `draw`（2570 起）：URL 栏/内容区布局、缩放命中矩形、滚动条、查找高亮带、空文档分支；
  2. `handle_event` 其余分支：非 URL 编辑状态下的按键/滚轮/指针、禁用态；
  3. 查找：`poll_find_requests` 的多次请求合并、命中矩形与滚动可见性；
  4. 下载：`download_state` 的完成/失败/取消边界、重复报告；
  5. 页面簿记：`create_page`/`destroy_page`/`reload_page` 的重复销毁与已销毁 reload（当前仅范围校验）；
  6. `WebEnginePage/Settings/DownloadItem/...` 门面：是否每一条都真实转发、有无死状态。
- 验收：分节精读 + 回归用例；`cargo test --all-features --lib -- webview::` 全绿；文档门控不变。

### 4.5 未闭合：其它候选面（按优先级）

| 面 | 范围 | 状态 |
|---|---|---|
| `webview/js_engine.rs` | 迭代上限、错误传播、DOM 绑定、无引擎特性下的拒绝路径 | 未排期（需单独立项） |
| `devtools/diff_viewer`（viewer/file_list） | 大文件分块、滚动/选择、行号缓存 | 未排期 |
| `finance/order_book`、`quote_board` | 闪烁/排序/属性写入的边角 | 第 19 轮只复核 depth/layout；其余未逐行 |
| `project/kanban_board` | 拖拽/滚动/宽度覆盖 | 第 17 轮部分覆盖；剩余边界未逐行 |
| `form_widgets/form.rs` | 规则安装是否立即重判的策略一致性 | 已记录为"按既有策略"，未改；若产品口径变化需重审 |
| 全仓 `Widget::tick` 契约 | 同 §4.2 | 未排期 |

### 4.6 跨切面待办（每项都必须有门控）

1. **新增公开 API 的账**：本计划新增的 CLI 包与 Markdown 属性/事件/命令，必须进
   `check_setter_census` / `check_event_signal_dyn` / `cargo doc -D warnings`。
2. **新二进制/包的发布**：若采纳 §2.1 的 `crates/rust-widgets-cli`，需确认根 `include` 不变、
   不进入根 crate tarball；若改为 `[[bin]]`，需同步 `check_declared_targets_ship.sh`（§5-Q1）。
3. **生成器门控**：CSS 解析写回 JSON 后，必须继续通过
   `check_generator_output_compiles.sh` / `check_mode_consistency.sh` / `check_generated_sources.sh`；
   任何"生成物与 mode 1 不一致"都视为失败。
4. **`!important` 落地**：改 `CssParser` 时，必须同时覆盖运行时 `StyleSheetManager` 与 CLI 静态级联，
   不能只改一边（#101）。
5. **Markdown 预览的行高契约**：`PreviewLayout` 改变滚动/可见行语义，必须补"选择/查找/大纲"
   三处回归，防止旧测试锁死旧语义。
6. **文档回写**：每个阶段结束时回写 `docs/log/` 与 `docs/issues/`；本计划本身不回写代码状态，
   直到对应阶段真正实施。

### 4.7 本次明确不做

- 不修改任何源码；不新增依赖；不提交。
- 不把 CLI 的"未实现"包装成成功（`check` 的严格模式必须能失败）。
- 不在 Markdown 里用硬编码颜色/尺寸；一切走主题与度量。
- 不为"完整"而引入 H 级依赖（MathJax/Mermaid 的 JS 运行时）；只做原生子集 + provider 接缝。

---

## 5. 需人工确认（⚠️ 计划信息不足）

| # | 问题 | 本计划假设 | 若不同意的替代方案 |
|---|---|---|---|
| Q1 | CLI 放 `crates/rust-widgets-cli`（独立嵌套包）还是根 crate 的 `[[bin]]`？ | 独立包（不污染根发布物） | 加根 `[[bin]]` + 同步 `check_declared_targets_ship.sh` |
| Q2 | "latex/**jx" 的确切含义？ | LaTeX 子集 + MathJax 兼容的定界符/命令，不嵌入 JS | 若指嵌入 MathJax JS，需要新依赖/引擎，另立项 |
| Q3 | Mermaid 的"完整"范围？ | flowchart + sequence（D1/D2），其余报告 | 若必须 class/gantt/pie 等，需要更大规模的解析/布局，单独立项 |
| Q4 | Markdown 原始 HTML 如何处理？ | 维持现状：按文本显示（不渲染、不执行） | 渲染子集或直接透传，需安全评审 |
| Q5 | 图片失败/不支持格式（GIF/WebP/SVG）如何处理？ | 占位 + `render_report` 报告 | 新增编解码器（新依赖/体积） |
| Q6 | CSS 是否需要 `@media`（响应式断点）？ | 不支持；断点已有 JSON `breakpoints` 键（loader-owned） | 把 `@media` 映射到 `breakpoints`，需设计映射规则 |
| Q7 | 反向 Rust 是否必须支持手写代码（非生成器产物）？ | 只承诺"生成器方言 + 白名单扩集" | 扩大解析范围会接近重写 `syn`，不建议 |
| Q8 | CLI 是否需要 `watch/preview`？ | 不做（宿主用 `cargo run`） | 需要事件循环/渲染上下文，另立项 |

---

## 6. 判定口径（DoD）

一个条目只有同时满足下列 4 条才可标记 ✅：

1. **实现**：代码/配置已落地，且被至少一个真实调用者（demo、CLI 命令、生成器、测试）使用；
   没有"只定义未接线"的 API。
2. **门控**：有可执行测试/脚本，并在回复中贴出**实跑输出**（不是"应该通过"）。
3. **回归**：既有门禁不退化（本文件基线：suites 1299 / 默认 82 / root 9799 / fmt / clippy / doc / 结构）。
4. **诚实报告**：所有不支持项都有名字、原因和可见降级；没有静默丢弃。

> 原则 #105 的直译：**"没跑过" = 没完成；"没接线" = 没完成；"没报告" = 没完成。**
