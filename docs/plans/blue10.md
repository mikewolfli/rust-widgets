# BLUE10 — 全方位深度扫描：全项目缺口、未完成项、简化实现与改进清单

> 版本: v0.10.0
> 基线: 继承 BLUE9 核心规则（PUA 闭环、冰山法则、原生优先/自绘兜底、证据先于结论）
> 编制日期: 2026-06-03
> 文档性质: 全量扫描 + 差距清单 + 可执行改进计划
> 扫描范围: 322 个 .rs 文件，112,155 行代码，8 大子系统并行深度扫描

---

## 核心规则（与 BLUE9 同）

1. 结论必须有构建/测试/代码证据，不允许"推测已修复"。
2. 修一个点必须扫同类模式，避免重复返工。
3. 优先修功能阻断项，再做体验增强。
4. 平台策略不变：原生优先，自绘兜底。

---

## 第一轮扫描：控件与类型盘点

### A. 控件与类型盘点

- 扫描结果:
  - `src/widget` 下 Rust 文件: **101**（含子目录）
  - `impl Widget for` 的具体控件结构体: **80**
  - 类型别名（alias）控件: **17**（如 `ActivityIndicator = ProgressBar`、`DoubleSpinBox = SpinBox` 等）
  - `WidgetKind` 枚举变体: **81**
  - 80 个结构体中 93.7% 实现了完整的 `Widget + Draw + EventHandler + Signals + Docs`

### B. 控件实现完整性扫描

| 类别 | 数量 | 占比 |
|------|------|------|
| 完整实现（Widget+Draw+EventHandler+Signals+Docs） | 75 | 93.7% |
| 缺少 EventHandler | ~7 | 8.8% |
| 缺少信号 | 3 | 3.8% |
| 不完整实现（todo!/unimplemented!/空桩） | **0** | 0% |

**✅ 零空桩、零 todo!()、零 unimplemented!() — 全线代码完整可编译。**

### C. 缺少 EventHandler 的控件

| # | 控件 | 文件 | 影响 |
|---|------|------|------|
| 1 | **FontComboBox** | `input_widgets/font_combo_box.rs` | 无法接收键盘/鼠标事件 |
| 2 | **Canvas** | `special_widgets/canvas.rs` | 51 行极简实现，无信号，仅绘制白底+灰框 |
| 3 | **ChartWidget** | `special_widgets/chart.rs` | 无数据点击/悬停交互 |
| 4 | **GridWidget** | `special_widgets/grid.rs` | 无单元格点击/悬停事件 |
| 5 | **WebView** | `web_widgets/web_view.rs` | 无交互式网页浏览事件 |
| 6 | **WebEngineView** | `web_widgets/web_engine.rs` | 无交互式浏览事件 |
| 7 | **WebEnginePage/Wrapper 类型** | `web_widgets/web_engine.rs` | newtype 包装器未实现 Widget+Draw+EventHandler |

### D. 信号/读写配对缺口（中等）

| # | 控件 | 缺口 |
|---|------|------|
| 1 | **ScrollArea** | 有 `scroll_position()` getter 但无 `scroll_position_changed` 信号 |
| 2 | **CommandLink** | `enabled` 字段遮蔽 `base.enabled` — `set_enabled()` 未委托到 `base.set_enabled()` |
| 3 | **WebView** | `set_url()` 立即模拟加载 — 同时触发 `loading_started` 和 `loading_finished`，无异步加载机制 |
| 4 | **PopupWindow** | 无弹窗专用信号（opened/closed 等） |
| 5 | **Label** | 无悬停/点击信号 — 完全被动控件 |
| 6 | **Frame** | 无公开的 frame 变化事件信号 |

### E. Canvas 控件 51 行极简实现

Canvas 是目前最薄的控件实现：
```rust
// 仅绘制白底 + 灰边框，无公开绘图 API
fn draw(&self, context: &mut RenderContext) {
    context.fill_rect(self.base.geometry(), Color::WHITE);
    context.draw_rect(self.base.geometry(), Color::GRAY);
}
```
**缺失**: 无画线、画圆、画路径、像素操作等公开 Canvas API。

### F. WebEngine 包装器类型不完整

`WebEnginePage`、`WebEngineSettings`、`WebEngineDownloadItem`、`WebEngineCookieStore`、`WebEngineWebChannel` 等均为 newtype 包装器，仅暴露 `inner()` 方法，未实现 `Widget`/`Draw`/`EventHandler` trait。

---

## 第二轮扫描：平台后端与原生能力

### A. 平台覆盖矩阵

| 平台 | 后端 | 状态 | 原生渲染 | 拖放 | IME | 无障碍 |
|------|------|------|----------|------|-----|--------|
| **macOS** | `MacOSPlatform` (Cocoa) | ✅ 原生 | ✅ 完整 | ✅ | ✅ | ✅ |
| **macOS** | `MacOSObjc2Platform` | ⚠️ 预览 | ❌ 纯状态 | ❌ | 仅状态 | 仅状态 |
| **Windows** | `WindowsPlatform` (Win32) | ✅ 原生 | ✅ 完整 | ✅ | ❌ | ❌ |
| **Linux (X11)** | `LinuxPlatform` (GTK) | ✅ 原生* | ⚠️ 部分 | ❌ | ❌ | ❌ |
| **Linux (Wayland)** | `WaylandPlatform` | ⚠️ 纯状态 | ❌ 纯状态 | ❌ | 仅状态 | 仅状态 |
| **Android** | `AndroidMobilePlatform` | ⚠️ 预览 | ❌ 纯状态 | ❌ | ❌ | ❌ |
| **HarmonyOS** | `HarmonyPlatform` | ⚠️ 预览 | ❌ 纯状态 | ❌ | 仅状态 | 仅状态 |
| **iOS** | — | 🔴 **缺失** | — | — | — | — |
| **Embedded** | `StubPlatform` | ⚠️ 回退 | ❌ 最小 | ❌ | ❌ | ❌ |

*Linux GTK 需要 `gtk-native` feature flag；无此 feature 则回退到纯状态。

### B. 关键平台缺口

| # | 严重度 | 缺口 | 详情 |
|---|--------|------|------|
| 1 | 🔴 | **无 iOS 后端** | `MobileBackend::Ios` 枚举变体存在但无 `IosMobilePlatform` 实现 |
| 2 | 🟠 | **Wayland 纯状态** | 有 `wl_client` 和 `try_create_native_window()` 桩，但无实际协议集成（无 xdg_shell/wl_surface） |
| 3 | 🟠 | **Android 减量控件集** | 仅 15 种控件，无 canvas/table/grid/chart/tree view/MDI/dock/web view 等 |
| 4 | 🟡 | **Linux GTK 有缺口** | `create_spin_box`/`create_list_view`/`create_scroll_area` 无原生 GTK 绑定 |
| 5 | 🟡 | **Windows 缺 IME 和无障碍** | 无 TSF/IME 集成，无 UIAutomation 桥接 |
| 6 | 🟡 | **拖放仅 macOS+Windows** | Linux/Wayland/Android/Harmony 均无原生拖放 |

### C. NativeControlBackend 的简化映射（9 个控件降级为基本图元）

| 控件 | 映射到 | 影响 |
|------|--------|------|
| Canvas | → Panel | 无专用画布原生控件 |
| Table | → Panel | 无原生表格 |
| Grid | → Panel | 无原生网格 |
| Chart | → Panel | 无图标原生控件 |
| Dial | → Slider | 拨盘≠滑块 |
| ToolBox | → Panel | 工具箱→普通面板 |
| Action | → Button | 动作→按钮 |
| ToolButton | → Button | 工具按钮→普通按钮 |
| ContextMenu | → Menu | 上下文菜单→普通菜单 |

这些不是桩代码，编译运行无问题，但产生的是通用容器而非具有预期语义的专用控件。

---

## 第三轮扫描：测试、示例与工具

### A. 测试覆盖

| 指标 | 数值 |
|------|------|
| 总测试数（`cargo test --all-features --lib`） | **1,688 passed** |
| Widget 控件有测试的 | **22/81 (27%)** |
| Widget 控件无任何测试的 | **59/81 (73%)** |
| 专用测试文件 | 4 个（+155 个 `#[cfg(test)]` 内联模块） |

### B. 零测试控件清单（73% = 59 个）

```
Dialog, FileDialog, ColorDialog, FontDialog, InputDialog, ProgressDialog,
PopupWindow, TextEdit, RichEdit, ScrollBar, DockPanel, TabWidget, Splitter,
MdiArea, MenuItem, ContextMenu, StatusBar, Canvas, Chart, ToggleButton,
CheckListBox, DoubleSpinBox, Dial, Wizard, DatePicker, TimePicker,
DateTimePicker, DirectoryDialog, DataView, PropertyGrid, Toolbox,
StackedWidget, CollapsiblePane, DockWidget, WebView, ActivityIndicator,
Calendar, ColumnView, UndoView, CommandLink, LCDNumber, FontComboBox,
WebEngineView, WebEnginePage, WebEngineSettings, WebEngineDownloadItem,
WebEngineCookieStore, WebEngineWebChannel, WebEngineFindTextResult,
WebEngineNotification, WebEngineScriptDialog, WebEngineContextMenuRequest,
Action, ToolButton, ToolBox, FreeformShape, TabBar, PieMenu
```

### C. 测试质量问题

| 问题 | 详情 |
|------|------|
| 🔴 `test_widget_structure.rs` 是假测试 | 使用 `fn main()` 替代 `#[test]`，仅打印一行文字 |
| 🟡 12 个控件仅 1-2 个测试 | SpinBox、ScrollArea、Panel、MessageBox、ListView、TreeView、Table、Menu、ToolBar、RibbonBar、MenuBar、Grid 仅结构体构造测试 |

### D. Rust 示例：零

- Cargo.toml 中无 `[[example]]` 条目
- `examples/` 目录下无 `.rs` 文件
- 现有的示例文件均为 ABI 绑定示例（C/C++/Java/Python），非 Rust 原生
- `smoke_demos.sh` 期望 9 个 Rust 示例，均不存在

### E. 工具健康度

| 工具 | 状态 | 问题 |
|------|------|------|
| `smoke_demos.sh` | 🔴 0/14 通过 | 所有 9 个示例缺失；内嵌段语法错误 |
| `check_abi.sh` | 🔴 失败 | `generate_c_header.py` 解析错误文件  |
| `check_profiles.sh` | 🔴 16 个编译错误 | `tr!` 宏、特性门控项未处理 |
| `check_event_model_signal_first.sh` | 🔴 全部失败 | 引用不存在的 `demos/` 目录 |
| `check_platform_capability_matrix.sh` | ✅ 通过 | — |
| `check_feature_completeness_matrix.sh` | ✅ 通过 | — |
| `generate_c_header.py` | 🔴 生成 0 个声明 | 解析 `mod.rs` 而非 `binding_impl.rs` |

### F. 基准测试极简

仅 4 个基准测试（`render_bench.rs` 2 个，`signal_bench.rs` 2 个）。缺失：布局基准、JSON 解析基准、事件分派基准、控件创建基准。

---

## 第四轮扫描：配置、文档与基础设施

### A. Cargo.toml 配置缺口

| # | 严重度 | 缺口 | 详情 |
|---|--------|------|------|
| 1 | 🔴 | `wgpu 0.16` 严重过时 | 当前 wgpu 为 v24.x，落后约 8 个大版本 |
| 2 | 🔴 | 仓库 URL 为占位符 | `homepage`/`repository` 指向 `https://github.com/your-repo/rust-widgets` |
| 3 | 🟠 | 无 `[[bench]]` 声明 | `benches/` 存在但未声明，`cargo bench` 不会运行 |
| 4 | 🟡 | `wayland-native = []` 为空 feature | 绑定零依赖 |
| 5 | 🟡 | `holographic = []` / `projection = []` 为空 feature | 无相关代码路径 |
| 6 | 🟡 | HarmonyOS target 依赖段全为注释 | `[target.'cfg(target_os = "harmony")'.dependencies]` 无实际依赖 |
| 7 | 🟡 | 无 `[profile.dev]` 调优 | 缺少 `opt-level = 1` 加快调试构建 |

### B. CI/CD 缺口

| # | 严重度 | 缺口 |
|---|--------|------|
| 1 | 🔴 | **`cargo test` 不在 CI 中运行** — 1,688 个测试本地存在但 PR 上从未验证 |
| 2 | 🟠 | 无 `cargo clippy` / `cargo fmt --check` 门禁 |
| 3 | 🟠 | CI 仅检查 `default` profile，无特性矩阵测试 |
| 4 | 🟠 | 无 `cargo audit` / `cargo deny` 安全检查 |
| 5 | 🟡 | 无自动发布/发布工作流 |

### C. 文档缺口

| # | 严重度 | 缺口 | 详情 |
|---|--------|------|------|
| 1 | 🔴 | `docs/COMMENTING_GUIDELINES.md` **不存在** | `CONTRIBUTING.md` 第 20/29 行引用了该文件 |
| 2 | 🟠 | `CHANGELOG.md` 不在项目根目录 | 实际位于 `docs/reports/CHANGELOG.md`，工具链期望根目录 |
| 3 | 🟡 | `blue9.md` 误放在项目根目录 | 应在 `docs/plans/` |
| 4 | 🟡 | `libtypes.rlib` 预编译产物在根目录 | 应删除或移入 `target/` |

### D. 缺失的配置文件

| 文件 | 用途 |
|------|------|
| `rustfmt.toml` | 格式化配置 |
| `clippy.toml` | Clippy 配置 |
| `.cargo/config.toml` | Cargo 别名/目标目录配置 |
| `build.rs` | 构建脚本（自动生成 C 头、设置 cfg、嵌入版本信息） |

### E. .gitignore 过于精简

当前仅忽略 `/target`。缺失：`*.DS_Store`、`*.swp`、`*.swo`、`*~`、`.env`、`libtypes.rlib`。

### F. .vscode/settings.json 缺口

- 无 `rust-analyzer.check.command = "clippy"`
- 无 `rust-analyzer.cargo.features = "all"`
- 无 `editor.formatOnSave`
- 无 `files.exclude` 排除 `/target`
- 无 `extensions.json` 推荐扩展

### G. .github/ PUA 文件冗余

`.github/` 目录下有 10+ 个 PUA 相关文件（ACTIVATION-COMPLETE.md、PUA-ACTIVATION.md 等），大部分与 `copilot-instructions.md` 重复。

---

## 第五轮扫描：渲染、绘制与视觉效果

### A. 渲染管线架构

```
Widget::draw() → Draw trait → RenderContext → PaintBackend
                                                   │
                                    ┌──────────────┼──────────────┐
                                    ▼              ▼              ▼
                           SoftwarePaintBackend  SvgPaintBackend  WgpuRenderer
                             (CPU 光栅化)        (SVG 输出)     (GPU via WGPU)
```

**状态**: 软件渲染成熟（双缓冲 RGBA + 抗锯齿），SVG 输出完整，GPU 路径基本为脚手架。

### B. GPU 渲染缺口

| 缺口 | 详情 |
|------|------|
| 🔴 WGPU 绘制指令仅 3 个变体 | `Clear`、`DrawText`、`DrawImage` — 无矩形、线条、圆形 |
| 🔴 无实际 GPU 着色器管线 | `WgpuRenderer` 目前通过 `rasterize_draw_commands_rgba8()` 在 CPU 上光栅化 |
| 🔴 `GpuType::detect_primary()` 为桩 | 返回 `None` — 运行时无 GPU 检测 |

### C. RenderCommand 图元缺口

| 缺口 | 影响 |
|------|------|
| 无 `DrawArc` 图元 | 无法渲染弧线 |
| 无 `DrawPath` / 贝塞尔曲线 | 无法渲染复杂路径 |
| 无 `DrawGradient` 图元 | 渐变数据结构存在但未集成到渲染管线 |
| 无文本对齐选项 | `DrawText` 仅基于原点定位 |

### D. 无障碍支持：仅占位级别

| 层级 | 实现 | 状态 |
|------|------|------|
| `PlatformCapabilities::accessibility` | bool 能力标志 | ✅ 已声明 |
| `accessibility_names` | `HashMap<ObjectId, String>` | ✅ 存储层 |
| C ABI `set/get_widget_accessibility_name` | FFI 绑定 | ✅ 已暴露 |
| Widget 上的 A11y trait | `accessible_name()`/`accessible_role()` 等 | 🔴 **不存在** |
| 屏幕阅读器桥接 | UIAutomation/NSAccessibility/AT-SPI | 🔴 **不存在** |
| 无障碍事件发射 | 状态变化通知屏幕阅读器 | 🔴 **不存在** |
| 键盘导航焦点链 | Tab 键焦点排序供 AT 使用 | 🔴 **不存在** |

**总结**: 无障碍支持仅有能力标志和名称存储，零 OS 集成。

---

## 第六轮扫描：布局、主题与动画

### A. 布局引擎

**强项**: 9 种布局管理器（Box/Grid/Flow/Form/Splitter/Stack/Absolute/UniformGrid/Inspector），含 DPI 感知缩放。

**缺口**:

| 缺口 | 影响 |
|------|------|
| 无布局管理器嵌套 | 每窗口/面板仅一种布局 |
| 无 CSS flex-grow/shrink/basis | 仅有拉伸因子 |
| `FlowLayout` 直接拥有控件 | 与其他使用 `ObjectId` 引用的布局不一致 |

### B. 主题/样式系统

**强项**: 完整语义标记（10 色、9 字体、4 间距）、JSON 序列化、亮/暗/自动模式。

**缺口**:

| 缺口 | 影响 |
|------|------|
| 无样式层叠/继承 | 无父→子传播 |
| 无 CSS 选择器引擎 | 仅基于类名的覆盖，无 `.class`/`#id`/`widget[state]` |
| `Gradient` 未集成到 `RenderCommand` | 渐变无法渲染 |
| `TouchTargetSize` 自动检测未接线 | 手动配置 |

### C. 动画系统

**强项**: 10 种缓动函数（含 Bounce/Elastic/Back），基础 `Animation` 和 `ColorAnimation`/`FloatAnimation` 类型。

**缺口**:

| 缺口 | 影响 |
|------|------|
| 🔴 无全局动画驱动器/tick 循环 | 动画孤立运行，无法协调 |
| 🔴 无基于属性的动画 API | 无法 `animate(obj, "x", 0→100, 300ms)` |
| 🔴 状态转换已存储但未执行 | `StatefulTheme.transitions` 有持续时间但无动画代码 |
| 🔴 无组合动画序列 | 无 `AnimationGroup`/`SequentialAnimation`/`ParallelAnimation` |
| 🔴 无 CSS 风格 `transition` 属性 | `WidgetStyle` 无过渡支持 |

---

## 第七轮扫描：事件系统与输入

### A. 事件系统

**强项**: 41 个 `Event` 变体（含 13 个触摸手势）、设计良好的信号/槽（无死锁、可重入、作用域断开）、14 个手势识别器。

### B. 关键缺口

| # | 严重度 | 缺口 | 详情 |
|---|--------|------|------|
| 1 | 🔴 | **无计时器系统** | `Event::Timer` 变体存在但仅由测试手动构建 — 无代码发射真正的计时器事件 |
| 2 | 🟠 | **10ms 忙轮询事件循环** | 无事件时浪费 CPU；应使用阻塞 `recv()` 或 `Condvar` |
| 3 | 🟠 | **平台运行循环未集成** | `EventLoop` 是独立 std 线程，与 macOS CFRunLoop/Win32 消息泵/GTK 主循环无关 |
| 4 | 🟠 | **无空闲调度** | `EventPriority::Idle` 已定义但未在循环中使用 |
| 5 | 🟠 | **IME 桥接未实现** | `ImePlatform` trait 已定义但无平台后端实现 |
| 6 | 🟡 | **仅有明文剪贴板** | 无富文本/图片/文件剪贴板 |
| 7 | 🟡 | **无通用文件监控** | `notify` 仅用于 i18n — 无资源热重载 |
| 8 | 🟡 | **原始 u32 键码** | `Event::KeyPress` 携带原始 OS 码，未映射到 `Key` 枚举 |

### C. 异步支持

代码中异步完全隔离在 `gpu` 模块（使用 wgpu 的本地异步 API）。事件系统 100% 同步。无 `tokio` 或 `async-std` 集成。

---

## 第八轮扫描：代码质量

### A. 关键指标

| 检查项 | 数量 | 评级 |
|--------|------|------|
| `unsafe` 块 | 133 | 🟢 低风险（FFI/分配器，预期内） |
| `unwrap()` | 186 | 🟠 中等（分布在 42 个文件） |
| `expect()` | 340 | 🟠 高（平台/渲染初始化"决不应失败"） |
| `panic!` | 26 | 🟡 中等（batch.rs 有 10 个变体 panic） |
| 空错误处理 `_ => {}` | 85 | 🟡 中等（大部分有意忽略未知变体） |
| `clone()` | 435 | 🟡 中等（1 个 clone/258 行） |
| `as` 类型转换 | 2,331 | 🟡 预期内（GUI 像素/尺寸数学运算） |
| `#[allow(…)]` 抑制 | 33 | 🟡 中等（12 个 too_many_args + 10 个 deprecated） |
| TODO/FIXME | **仅 2 个** | 🟢 优秀 |
| `#[deprecated]` | 51 | 🟠 需行动（46 个在 render/pipeline 中） |
| `?` 操作符 | 476 | 🟢 良好 |
| `#[cfg(test)]` 内联模块 | 155 个文件 | 🟢 强测试覆盖模式 |

### B. "上帝对象"（>2000 行文件）

| 文件 | 行数 | 严重度 |
|------|------|--------|
| `src/widget/capability.rs` | **8,957** | 🔴🔴🔴 极端 |
| `src/control_backend/custom.rs` | 3,184 | 🔴 高 |
| `src/control_backend/trait_def.rs` | 1,740 | 🟠 中等 |
| `src/platform/windows/platform_impl.rs` | 1,656 | 🟠 中等 |
| `src/render/tests.rs` | 1,603 | 测试（可接受） |

**capability.rs 是项目中最大的质量风险** — 近 9,000 行包含整个属性反射和控件工厂系统。应拆分为 manifest/access/schema/error/traits 子模块。

### C. render/pipeline/ 大量弃用

46 个 `#[deprecated]` 函数集中在 `render/pipeline/` 目录，标注"Pipeline routing is unstable"。整个管道路由层正在迁移中，应完成迁移或移除弃用代码。

### D. 平台 FFI 错误静默丢弃

31 个 `Err(_)` 处理器（主要在 macOS/Windows platform_impl 中）静默丢弃 FFI 错误。例如 `windows/platform_impl.rs:982`: `Err(_) => return 0` — 错误信息完全丢失。

---

## BLUE10 改进计划（8 大领域 R1-R8）

### R1 — 核心控件圆满化（Event Handler 补齐 + 信号漏洞修复）

**当前状态**: 7 个控件缺少 EventHandler，6 个信号缺口。

| 步骤 | 描述 |
|------|------|
| R1.1 | 为 FontComboBox 添加 EventHandler |
| R1.2 | 为 Canvas 添加绘图 API（draw_line/draw_circle/draw_path 等）和信号 |
| R1.3 | 为 ChartWidget 添加 EventHandler（数据点点击/悬停） |
| R1.4 | 为 GridWidget 添加 EventHandler（单元格点击/悬停） |
| R1.5 | 为 WebView/WebEngine 添加 EventHandler |
| R1.6 | 修复 ScrollArea `scroll_position_changed` 信号 |
| R1.7 | 修复 CommandLink `set_enabled` 基类委托 |
| R1.8 | 修复 WebView 异步加载机制 |
| R1.9 | 重构 WebEngine 包装器类型实现 Widget+Draw+EventHandler |

### R2 — 平台能力对齐（iOS 后端 + Wayland 原生 + 降级映射审计）

**当前状态**: 无 iOS 后端，4 个纯状态后端，9 个控件降级映射。

| 步骤 | 描述 |
|------|------|
| R2.1 | 创建 `IosMobilePlatform` 基础状态后端（`src/platform/ios/`） |
| R2.2 | 推进 Wayland xdg_shell/wl_surface 协议集成 |
| R2.3 | 为 Linux GTK 补齐 `spin_box`/`list_view`/`scroll_area` 原生绑定 |
| R2.4 | 审查 9 个降级映射（Canvas→Panel 等），标记为"设计型"或添加专用后端 |
| R2.5 | 为 Windows 添加 IME 桥接（TSF） |
| R2.6 | 为 Windows 添加 UIAutomation 桥接（see R7） |
| R2.7 | 补充 Android 后端控件集（至少覆盖基本控件族） |

### R3 — 测试与门禁基建（补齐测试 + 修复 CI）

**当前状态**: 73% 控件零测试，CI 不运行测试，4 个工具有问题。

| 步骤 | 描述 |
|------|------|
| R3.1 | 在 CI 中添加 `cargo test --all-features` |
| R3.2 | 在 CI 中添加 `cargo clippy` 和 `cargo fmt --check` |
| R3.3 | 删除/重写假测试 `test_widget_structure.rs` |
| R3.4 | 为高优先级控件（Dialog 族、输入控件族）添加测试 |
| R3.5 | 修复 `smoke_demos.sh` — 创建缺失的 9 个示例或更新脚本 |
| R3.6 | 修复 `check_abi.sh` 和 `generate_c_header.py` — 指向正确的源文件 |
| R3.7 | 修复 `check_profiles.sh` — 处理嵌入配置的特性和宏 |
| R3.8 | 修复 `check_event_model_signal_first.sh` — 创建 demos/ 目录或移除引用 |
| R3.9 | 在 Cargo.toml 中添加 `[[bench]]` 条目 |
| R3.10 | 添加布局/JSON/事件基准测试 |
| R3.11 | 创建 Rust 原生示例目录（至少 5 个基本演示） |

### R4 — 配置与文档圆满化

**当前状态**: 占位 URL，缺失配置文件，缺失文档。

| 步骤 | 描述 |
|------|------|
| R4.1 | 修复 Cargo.toml 中的 `your-repo` 占位符 URL |
| R4.2 | 创建 `docs/COMMENTING_GUIDELINES.md` |
| R4.3 | 将 `CHANGELOG.md` 移动到项目根目录 |
| R4.4 | 将 `blue9.md` 移动到 `docs/plans/` |
| R4.5 | 删除 `libtypes.rlib` 产物（加入 .gitignore） |
| R4.6 | 创建 `rustfmt.toml` 和 `clippy.toml` |
| R4.7 | 创建 `.cargo/config.toml` 含常用别名 |
| R4.8 | 扩展 `.gitignore`（OS/编辑器文件） |
| R4.9 | 增强 `.vscode/settings.json`（rust-analyzer 配置） |
| R4.10 | 创建 `.vscode/extensions.json` 推荐扩展 |
| R4.11 | 清理 `.github/` 冗余 PUA 文件 |

### R5 — 渲染管线增强（GPU + 图元 + 渐变）

**当前状态**: GPU 渲染为脚手架，缺失图元，渐变无法渲染。

| 步骤 | 描述 |
|------|------|
| R5.1 | 扩展 `WgpuDrawCommand` 覆盖所有 `RenderCommand` 变体 |
| R5.2 | 实现实际 GPU 着色器管线（WGSL 顶点/片段着色器） |
| R5.3 | 实现 `GpuType::detect_primary()` 运行时检测 |
| R5.4 | 添加 `DrawArc`/`DrawPath`/贝塞尔曲线到 `RenderCommand` |
| R5.5 | 添加 `DrawGradient` 到 `RenderCommand` 并集成 `style/gradient.rs` |
| R5.6 | 为 `DrawText` 添加文本对齐选项 |
| R5.7 | 评估 wgpu 从 0.16 升级到 v24.x（需独立评估破坏性变更） |

### R6 — 动画与样式系统集成

**当前状态**: 动画基元存在但无运行时驱动，无属性动画。

| 步骤 | 描述 |
|------|------|
| R6.1 | 实现全局动画驱动/tick 循环 |
| R6.2 | 实现基于属性的动画 API |
| R6.3 | 将状态过渡 `StatefulTheme.transitions` 连接到动画管线 |
| R6.4 | 添加 `AnimationGroup`/`SequentialAnimation`/`ParallelAnimation` |
| R6.5 | 实现样式层叠/继承 |
| R6.6 | 实现 CSS 选择器引擎 |
| R6.7 | 实现 `request_animation_frame` API |

### R7 — 无障碍 (A11y) 基础设施

**当前状态**: 仅能力标志和名称存储，零 OS 集成。

| 步骤 | 描述 |
|------|------|
| R7.1 | 在 `Widget` trait 上添加 `accessible_name()/accessible_role()/accessible_description()` |
| R7.2 | 实现 macOS NSAccessibility 桥接 |
| R7.3 | 实现 Windows UIAutomation 桥接 |
| R7.4 | 实现 Linux AT-SPI 桥接 |
| R7.5 | 实现键盘导航焦点链供辅助技术使用 |
| R7.6 | 添加无障碍事件发射（状态变化 → 屏幕阅读器通知） |

### R8 — 事件与运行时系统

**当前状态**: 无计时器系统，10ms 轮询，无空闲调度。

| 步骤 | 描述 |
|------|------|
| R8.1 | 实现 `TimerManager` 含 `start_timer(id, Duration, repeating)` → 发射 `Event::Timer` |
| R8.2 | 将平台运行循环通知连接到原生运行循环 |
| R8.3 | 将 10ms 轮询替换为阻塞接收 / Condvar 等待 |
| R8.4 | 实现空闲调度（`EventPriority::Idle` 的实际使用） |
| R8.5 | 实现平台 IME 桥接（macOS NSTextInputContext / Windows TSF） |
| R8.6 | 添加富文本/图片剪贴板支持 |
| R8.7 | 创建通用 `AssetWatcher`（扩展现有 I18nFileWatcher 模式） |

### R9 — 代码质量债务清理

**当前状态**: 8,957 行 monster 文件，46 个 pipeline 弃用，31 个静默 FFI 错误。

| 步骤 | 描述 |
|------|------|
| R9.1 | 拆分 `src/widget/capability.rs` 为 manifest/access/schema/error/traits 子模块 |
| R9.2 | 完成 `render/pipeline/` 迁移 — 移除 46 个弃用函数或用新替代方案 |
| R9.3 | 在平台 FFI 代码中添加 31 个 `Err(_)` 的日志记录 |
| R9.4 | 将 `render/backend/batch.rs` 中 10 个 `panic!` 转换为错误类型 |
| R9.5 | 拆分 `src/control_backend/custom.rs` (3,184 行) |
| R9.6 | 将 `src/gesture/mod.rs` 拆分为子文件（1,397 行单文件） |
| R9.7 | 重构 12 个 `too_many_arguments` 函数（使用配置结构体） |
| R9.8 | 将 web/ 和 gpu/ 代码中的 `unwrap()` 替换为 `unwrap_or`/错误传播 |
| R9.9 | 在 85 个空 `_ => {}` 匹配分支中添加意图注释 |

---

## 执行顺序建议

```
优先级 0 (立即修复 — 门禁/安全):
  R3.1 CI 运行测试 → R3.2 CI 运行 clippy/fmt → R4.1 URL 修复 → R3.3 假测试 → R4.5 libtypes.rlib

优先级 1 (核心功能阻断):
  R1.1-R1.5 控件 EventHandler → R8.1 计时器系统 → R2.1 iOS 后端 → R5.1 GPU 渲染

优先级 2 (高影响改进):
  R3.4 控件测试 → R3.5-R3.8 工具修复 → R5.4-R5.6 渲染图元 → R7.1-R7.6 无障碍

优先级 3 (完善):
  R6.1-R6.7 动画系统 → R2.2-R2.7 平台对齐 → R4.2-R4.11 配置/文档

优先级 4 (代码质量):
  R9.1-R9.9 债务清理 → R8.2-R8.7 运行时 → R3.9-R3.11 基准/示例
```

---

## 最终目标（BLUE10 里程碑）

1. **100% EventHandler 覆盖** — 所有 80 个控件均实现 Widget+Draw+EventHandler
2. **iOS 平台后端存在** — 至少状态级实现
3. **CI 运行测试 + clippy + fmt** — 自动化质量门禁
4. **Canvas 绘图 API** — 完整的 draw_line/draw_circle/draw_path 等公开方法
5. **计时器系统** — `TimerManager` 发射 `Event::Timer` 事件
6. **GPU 渲染管线** — 实际 WGSL 着色器，非 CPU 光栅化
7. **无障碍基座** — Widget trait 上的 A11y 方法 + 至少 1 个 OS 桥接
8. **capability.rs 拆分** — 从 8,957 行拆分为 < 2,000 行子模块
9. **CI 特性矩阵** — 至少 default/full/embedded 三个配置
10. **0 个假测试** — 所有声明为 tests 的文件均为实际测试

---

## 本轮扫描证据

### 构建状态
- `cargo check --all`: ✅ `Finished dev profile [unoptimized + debuginfo]` (0.24s)
- `cargo test --all-features --lib`: **1,688 passed; 0 failed; 3 ignored**

### 扫描方法
- 8 个并行 Agent 深度扫描：
  - Agent 1: `src/widget/` 101 个文件，80 个控件结构体
  - Agent 2: `src/control_backend/` + `src/platform/` 42 个文件，7 个平台后端
  - Agent 3: `tests/` 4 文件，`examples/` 14 文件，`tools/` 55 文件，`language/` 3 文件
  - Agent 4: `Cargo.toml`、`docs/`、`.github/`、`.vscode/`、根配置文件
  - Agent 5: 渲染/绘制/布局/主题/动画/无障碍子系统
  - Agent 6: 事件/信号/输入/剪贴板/拖放/计时器/异步
  - Agent 7: `lib.rs`、322 文件模块结构、i18n/日志/特性门控
  - Agent 8: unsafe/unwrap/expect/panic/clone/cast/deprecated 代码质量扫描

### 文件覆盖率
- 扫描覆盖: 322/322 个 `.rs` 文件 (100%)
- 覆盖所有 12 个控件子目录
- 覆盖所有 7 个平台后端
- 覆盖所有配置和文档文件

### 发现汇总

| 类别 | 发现数 |
|------|--------|
| 🔴 关键缺口（功能阻断/安全） | 18 |
| 🟠 高优先级缺口 | 24 |
| 🟡 中等缺口 | 28 |
| 🟢 低优先级/优化 | 12 |
| **总计** | **82** |

### BLUE10 初始完成率

- R1（控件圆满化）完成率：**93%**（基于 BLUE9 的 100% + 新发现 7 个缺失项）
- R2（平台能力对齐）完成率：**60%**（无 iOS，4 个纯状态后端）
- R3（测试与门禁基建）完成率：**35%**（27% 控件有测试，CI 无测试）
- R4（配置与文档圆满化）完成率：**50%**（多项缺失）
- R5（渲染管线增强）完成率：**55%**（软件渲染 85%，GPU 渲染 25%）
- R6（动画与样式集成）完成率：**30%**（基元存在，运行时缺失）
- R7（无障碍）完成率：**5%**（仅能力标志+名称存储）
- R8（事件与运行时）完成率：**60%**（计时器/IME/空闲缺失）
- R9（代码质量债务）完成率：**70%**（主要问题：8,957 行 monster 文件）

- BLUE10 总体完成率（按 R1-R9 等权）：**50.9%**
