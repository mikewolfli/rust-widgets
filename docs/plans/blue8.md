# BLUE8 — Rust Widgets v0.7.1 未完成工作 + 触摸屏与多形态设备支持

> **版本**: v0.7.1
> **基线**: BLUE6 + BLUE7 全部 181 项修复完成
> **编制日期**: 2026-05-02
> **规则参考**: BLUE7.md（同标准，PUA 质量门禁 + 冰山法则 + ICEBERG 跨模块扫描 + 原生优先自绘兜底原则）

---

## 架构原则

与 BLUE6/BLUE7 保持一致：

1. **原生优先，自绘兜底**：所有控件优先使用平台原生 API。只有原生不支持或需要深度定制时，才 fallback 到软件/GPU 渲染。
2. **系统决策，用户无感**：原生 vs 自绘路径由编译时 feature flags 自动选择。
3. **多形态设备适配**：从 PC 桌面到平板/手机/嵌入式/投影屏，同一 API 层覆盖不同交互模式。触摸优先设计，鼠标兼容。
4. **架构层级**：`app/handle.rs` → `platform/` → `render/` → `wgpu_backend/`。

---

## 🔴 P0 — 未完成的功能阻断问题

BLUE6/BLUE7 已闭合全部 181 项 P0-P4，当前无未修复的 P0 级别问题。以下为**已知架构限制**，存在于当前代码中但不属于"bug"：

1. **Platform trait 全量实现，但 4 个后端为状态模拟**：
   - Wayland：零 Wayland 协议调用，事件循环 `thread::sleep(16ms)` 轮询
   - Harmony：零 HarmonyOS API 调用，同上轮询
   - Mobile (Android)：14 个 combo/list 方法返回硬编码值
   - macOS objc2：零 Cocoa/AppKit 原生调用
   - 所有后端 IME、无障碍、拖放功能均使用 trait 默认实现（返回 false/None）

2. **Widget 单元测试覆盖严重不足**：多个核心模块零单元测试（详见 P3-1）。

3. **ControlBackend 97 方法零默认实现**：设计决定，但第三方实现门槛极高。

---

## 🟠 P1 — 架构改进（从 BLUE7 继承）

### P1-1: ControlBackend 默认方法模式（原 P2-15）

ControlBackend trait 的 97 个方法全部是必需的，无默认实现。这导致第三方实现难度极高。

**建议方案**（非代码，仅评估）：
- 评估方向 A：添加少量 `create_widget_base()` 辅助方法
- 评估方向 B：使用宏生成模板代码（类似 `impl_handle!`）
- 评估方向 C：保持现状，`custom.rs` 和 `native.rs` 作为参考实现

### P1-2: Platform trait 可选方法分离（原 P4-13）

IME、无障碍、拖放等方法是 trait 强制方法但所有后端返回 false/None。

**建议方案**：
- 将这些方法从 `Platform` trait 移到独立的 extension traits
- 或标记 trait 方法默认实现为已废弃
- 当前设计可接受，但文档需明确标注

### P1-3: render/web/ 模块连接（原 P2-16）

`src/render/web/engine.rs` 和 `view.rs` 包含完整的 `WebEngine`/`WebView` 实现但未被任何上层模块导出或引用。

**建议方案**：
- 评估是否需要在 `RenderContext` 层暴露 web 渲染功能
- 如果不需要，标记模块为废弃并计划移除
- 如果未来 WebView 自绘需要，则在 `render/mod.rs` 中添加 `pub use web::*`

---

## 🟡 P2 — 平台后端实施缺口

### P2-1: Wayland 后端 — 全后端状态模拟

- 零 Wayland 协议调用
- `dpi_scale_factor()` 含 TODO（需查询 wl_output）
- `run()` 含 TODO（需进入 Wayland 事件循环）
- 69+ 个 Platform 方法全部状态模拟

### P2-2: macOS objc2 后端 — 全后端状态模拟

- `objc2-macos` feature 现在正确拉入 `objc2-foundation` 依赖
- 但所有 create_* 方法只操作 `BackendState`，无任何 NSView/NSButton 创建
- 适用于迁移回归测试，不适用于实际交互

### P2-3: Harmony 后端

- `run()` 使用 `thread::sleep(16ms)` 轮询循环
- 无任何 HarmonyOS API 调用
- 标注为预览状态

### P2-4: Mobile 后端

- 14 个 ComboBox/ListBox 方法返回 false/None/0
- 无实际移动端 FFI 集成
- 标注为预留状态

### P2-5: Windows 后端扩展控件

- MessageBox、FileDialog、ColorDialog、FontDialog 为 state surrogate（无 MessageBoxW/IFileOpenDialog/CHOOSECOLORW/CHOOSEFONTW 调用）
- SpinBox、ListView、ScrollArea 为 state surrogate
- DPI scale 硬编码为 1.0（需 `GetDpiForWindow`/`GetDeviceCaps` 查询）
- 这些已在 BLUE7 中从 `return 0` 改为 state-backed，但非原生实现

---

## 🔵 P3 — 测试覆盖不足

### P3-1: 核心模块零单元测试

以下模块完全或几乎无单元测试：

| 模块 | 测试覆盖 | 建议优先级 |
|------|----------|-----------|
| `widget/` — Button, CheckBox, Label, Slider 等 40+ 控件 | 零测试 | 🔴 高 |
| `widget/advanced_widgets/` — Calendar, Dial, PieMenu, RibbonBar, TabBar | 零测试 | 🔴 高 |
| `widget/special_widgets/` — FreeformShapeWidget, LCDNumber | 零测试 | 🟡 中 |
| `widget/dialog/` — 6 种对话框 | 零测试 | 🟡 中 |
| `render/backend/` — paint.rs, batch.rs, scene.rs | 零测试 | 🟡 中 |
| `control_backend/` — custom.rs, native.rs | 零测试 | 🟡 中 |
| `web/` — WebEngine, WebView, JsEngine, plugins | 零测试 | 🟡 中 |
| `json/` — loader.rs, element.rs | 仅少量集成测试 | 🟡 中 |

**建议测试策略**：
1. 优先添加 `widget/` 基础控件的单元测试（构造 → 绘制 → 事件处理 → 信号发射）
2. 每个 `Draw::draw()` 实现至少有一个快照/SVG 测试
3. 每个 `WidgetKind` 变体至少有一个对应的类型测试
4. 使用现有 `#[cfg(test)] mod tests` 模式，不引入新的测试框架

---

## ⚪ P4 — 触摸屏与多形态设备支持

### 设计原则

本项目在 v0.7.1 之前以桌面鼠标交互为默认设计。BLUE8 引入触摸屏支持，遵循以下原则：

1. **渐进增强**：所有现有控件继续支持鼠标。触摸支持在鼠标工作的基础上叠加。
2. **触摸优先**：新控件设计时先考虑手指操作（最小 44×44pt 触摸目标），再考虑鼠标精度。
3. **姿态识别**：系统级处理单指点击、双指缩放/旋转、长按、滑动等基础手势。
4. **设备适配**：同一 API 层适配 PC 触屏（大屏高精度）、平板（中屏手指）、手机（小屏单手）、嵌入式（小屏低资源）、投影屏（大屏只读/遥控）。

### P4-1: 触摸事件系统

**当前状态**：`Event` 枚举只有 `MouseDown`/`MouseUp`/`MouseMove`，无触摸专用事件。

**建议添加的事件类型**：

| 事件 | 触发条件 | 用途 |
|------|----------|------|
| `TouchBegin(Point, TouchId)` | 手指触屏 | 替代 MouseDown |
| `TouchEnd(Point, TouchId)` | 手指离开 | 替代 MouseUp |
| `TouchMove(Point, TouchId)` | 手指滑动 | 替代 MouseMove |
| `Tap(Point)` | 快速点击 | 按钮/链接激活 |
| `DoubleTap(Point)` | 双击 | 缩放/编辑 |
| `LongPress(Point)` | 按住 500ms | 上下文菜单 |
| `Swipe(Point, Point, f32)` | 快速滑动 | 翻页/滚动 |
| `Pinch(f32)` | 双指缩放 | 缩放控件 |
| `Rotate(f32)` | 双指旋转 | 图像旋转 |
| `Drag(Point, TouchId)` | 拖拽 | 拖放操作 |

**设备映射**：
- 触摸屏 PC：TouchBegin/TouchEnd/TouchMove + Tap/DoubleTap + Swipe
- 平板：同上 + Pinch/Rotate
- 手机：同上 + Swipe 为主要导航手势
- 嵌入式：TouchBegin/TouchEnd + Tap
- 投影屏：仅遥控输入，通过 Swipe 翻页

**设计决策**：不新增 `TouchEvent` 枚举，而是扩展 `Event` 枚举。保持 `EventHandler::handle_event` 签名不变。

### P4-2: 手势识别器

**建议模块**：`src/gesture/`（新建）

| 识别器 | 识别条件 | 输出事件 |
|--------|----------|----------|
| `TapGesture` | 触摸点无显著移动 + 持续时间 < 300ms | `Tap` |
| `LongPressGesture` | 触摸点无显著移动 + 持续时间 >= 500ms | `LongPress` |
| `SwipeGesture` | 触摸点快速移动 + 速度 > 阈值 | `Swipe` |
| `PinchGesture` | 双指距离变化 | `Pinch` |
| `RotateGesture` | 双指角度变化 | `Rotate` |
| `DoubleTapGesture` | 两次 Tap 在 400ms 内 | `DoubleTap` |

**架构**：手势识别器在 `EventLoop` 层或 `Platform` 层解析原始触摸事件，发射高层语义事件。widget 只响应 `Tap`/`LongPress` 等语义事件，无需自己处理原始触摸坐标。

**建议方案**：
- `GestureRecognizer` trait：`fn process(&mut self, event: &Event) -> Option<Event>`
- `GestureEngine` 持有多个识别器，按优先级链式处理

### P4-3: 触摸感知控件清单

以下控件按交互性质分组，标注触摸支持推荐的优先级别：

#### 🔴 高优先级（常用控件，直接影响可用性）

| 控件 | 触摸交互 | 当前鼠标支持 | 触摸适配工作量 |
|------|----------|-------------|---------------|
| `Button` | Tap 点击 | ✅ MouseDown/Up | 极小：将 Tap 映射到 Click |
| `CheckBox` | Tap 切换 | ✅ MouseDown/Up | 极小：同上 |
| `RadioButton` | Tap 选择 | ✅ MouseDown/Up | 极小：同上 |
| `LineEdit` | 触摸定位光标 + 虚拟键盘 | ✅ 键盘输入 | 中：光标定位 + 焦点管理 |
| `Slider` | 手指拖拽滑块 | ✅ MouseDrag | 小：扩展拖拽区域 + 手指尺寸 |
| `ProgressBar` | 只读，无需触摸 | ✅ 只读 | 无 |
| `ComboBox` | Tap 展开列表 + 选择 | ✅ MouseClick | 小：列表项触摸尺寸 |
| `ListBox` | 触摸滚动 + 选择 | ✅ MouseClick | 小：列表项触摸尺寸 |
| `ScrollArea` | 触摸滑动滚动 | ✅ 滚轮 | 小：Swipe→Scroll 映射 |
| `Panel` | 容器，本身无交互 | ✅ 容器 | 无 |

#### 🟡 中优先级（常用但交互复杂度高）

| 控件 | 触摸交互 | 当前鼠标支持 | 触摸适配工作量 |
|------|----------|-------------|---------------|
| `SpinBox` | Tap 加减按钮 | ✅ MouseClick | 小：按钮触摸尺寸 |
| `TabWidget` | Tap 切换标签 | ✅ MouseClick | 小 |
| `StackedWidget` | Swipe 翻页 | ✅ MouseClick | 中：Swipe→IndexChange |
| `Menu/MenuBar` | Tap 展开 + 选择 | ✅ MouseHover+Click | 中：悬停→点击模式切换 |
| `ToolBar` | Tap 工具按钮 | ✅ MouseClick | 小 |
| `MessageBox` | Tap 按钮响应 | ✅ MouseClick | 小 |
| `FileDialog` | Tap 浏览 + 选择 | ✅ MouseClick | 中：文件列表触摸尺寸 |
| `ColorDialog` | 触摸颜色选择 | ✅ MouseClick+Drag | 大：颜色盘触摸操作 |
| `FontDialog` | 触摸选择 | ✅ MouseClick | 中 |
| `InputDialog` | 触摸输入 | ✅ 键盘 | 中 |
| `ProgressDialog` | 无交互 | ✅ 只读 | 无 |
| `TreeView` | 触摸展开/折叠 | ✅ MouseClick | 小：节点触摸尺寸 |
| `ListView` | 触摸选择 + 滚动 | ✅ MouseClick | 小 |
| `TableView` | 触摸选择 | ✅ MouseClick | 中：单元格触摸尺寸 |

#### 🟢 低优先级（高级控件，使用频率低）

| 控件 | 触摸交互 | 当前鼠标支持 | 触摸适配工作量 |
|------|----------|-------------|---------------|
| `Dial` | 触摸旋转 | ✅ MouseDrag | 中：手指旋转映射 |
| `Calendar` | Tap 选择日期 | ✅ MouseClick | 小 |
| `DateEdit` | Tap 弹出日历 | ✅ MouseClick | 小 |
| `TimeEdit` | Tap 选择时间 | ✅ MouseClick | 小 |
| `DateTimeEdit` | Tap 弹出选择器 | ✅ MouseClick | 小 |
| `KeySequenceEdit` | 触摸快捷键输入 | ✅ 键盘 | 小 |
| `GroupBox` | 容器，标题可点 | ✅ 仅标题 | 小 |
| `CollapsiblePane` | Tap 展开/折叠 | ✅ MouseClick | 小 |
| `DockWidget` | 触摸拖拽停靠 | ✅ MouseDrag | 大：拖拽手势 |
| `MdiArea` | 触摸窗口管理 | ✅ MouseDrag | 大 |
| `Splitter` | 触摸拖拽分割 | ✅ MouseDrag | 小 |
| `ToolBox` | Tap 切换页 | ✅ MouseClick | 小 |
| `WebView` | 浏览器级触摸 | ✅ 委托 OS | 依赖平台实现 |
| `RibbonBar` | Tap 切换标签 | ✅ MouseClick | 小 |
| `TabBar` | Tap 切换 + 拖拽 | ✅ MouseClick+Drag | 小：拖拽手势 |
| `PieMenu` | 触摸径向选择 | ✅ MouseClick | 小 |
| `FreeformShapeWidget` | Tap + Drag | ✅ Mouse | 小 |
| `LCDNumber` | 只读 | ✅ 只读 | 无 |
| `CommandLink` | Tap 点击 | ✅ MouseClick | 小 |
| `FontComboBox` | Tap 选择字体 | ✅ MouseClick | 小 |
| `Action` | 抽象触发 | ✅ 通过宿主 | 无 |

### P4-4: 触摸目标最小尺寸

所有交互式控件应保证触摸目标不小于以下尺寸：

| 设备类型 | 最小触摸目标 | 间距 |
|----------|-------------|------|
| PC 触屏 | 32×32 pt | 8pt |
| 平板 | 44×44 pt | 12pt |
| 手机 | 48×48 pt | 16pt |
| 嵌入式 | 40×40 pt | 10pt |
| 投影屏 | 24×24 pt（遥控） | 6pt |

**实施建议**：在 `WidgetStyle` 中添加 `touch_target: Option<Size>` 字段。如果控件几何小于最小触摸目标，渲染时扩展点击区域但不改变视觉几何。

### P4-5: 投影屏支持（实验性）

投影屏场景特征是：**大画幅（1920×1080+）、只读展示、遥控器/激光笔输入、无触摸**。

**建议支持方式**：
- 新增 `PlatformFamily::Projector` 平台变体
- `ProjectorPlatform` 不接受鼠标/触摸事件，仅支持翻页遥控
- `run()` 事件循环只处理 Swipe（翻页）和 Quit
- 所有控件进入"展示模式"（只读、无悬停效果、字号增大 20%）
- 幻灯片模式：`Window` 自动全屏，隐藏标题栏，显示翻页指示器
- 新增 `PresentationController` 结构体管理幻灯片状态（当前页、总页数、过渡动画）

**支持的控件子集**（投影屏只读模式）：

| 控件 | 投影模式行为 |
|------|-------------|
| Window | 全屏无边框，底部翻页指示器 |
| Label | 可读文本，字号自适应 |
| Button | 隐藏或转为导航键 |
| Image | 全屏显示，双指缩放 |
| Chart | 全屏最大化，无交互 |
| FreeformShapeWidget | 展示自由形状绘制 |
| Panel | 容器，不影响展示 |

### P4-6: 设备检测与自适应布局

**建议新增模块**：`src/platform/detector.rs`

| 检测维度 | 检测方法 | 影响 |
|----------|----------|------|
| 触摸能力 | 平台 API 查询（Windows `GetSystemMetrics(SM_TOUCH)` / macOS `NSTouchDevice`） | 启用/禁用触摸事件 |
| 屏幕尺寸 | 逻辑分辨率 | 选择触摸目标尺寸规范 |
| 设备形态 | 宽高比 + DPI | 桌面/平板/手机布局模式 |
| 投影模式 | 检测外接显示器 + 无输入设备 | 切换展示模式 |
| 嵌入式 | feature flag `embedded` | 简化控件集 |

**自适应布局规则**：
- 手机（宽 < 480pt）：StackedWidget 默认 Swipe 导航，TabWidget 简化标签
- 平板（480~1024pt）：TabWidget 显示标签栏，Splitter 可触摸分割
- 桌面（> 1024pt）：完整控件集，鼠标 hover 效果
- 投影（检测到）：展示模式

### P4-7: 虚拟键盘集成

触摸屏需要虚拟键盘用于 `LineEdit`/`TextEdit`/`SpinBox`/`ComboBox` 编辑。

**建议方案**：
- 新增 `VirtualKeyboard` 模块
- 平台集成：Windows `TabTip.exe` / 移动端 OSK / 嵌入式自定义
- 当触摸编辑控件获得焦点时自动弹出
- 布局自动调整（窗口升高 + 控件上移）避免键盘遮挡

---

## 🎯 修复优先级路线图

| Round | 内容 | 工作量 | 优先级 |
|-------|------|--------|--------|
| Round 1 | P3-1 基础控件单元测试（Button/CheckBox/RadioButton/Label/Slider） | 中 | 🔴 高 |
| Round 2 | P4-1/P4-2 触摸事件系统 + 手势识别器 | 大 | 🔴 高 |
| Round 3 | P4-3 高优先级控件触摸适配（Button/CheckBox/Slider/ScrollArea/ComboBox/ListBox） | 中 | 🔴 高 |
| Round 4 | P4-6 设备检测与自适应布局 | 中 | 🟡 中 |
| Round 5 | P4-4 触摸目标最小尺寸实施 + WidgetStyle 扩展 | 小 | 🟡 中 |
| Round 6 | P4-3 中优先级控件触摸适配（Menu/TabWidget/TreeView/Dialogs） | 中 | 🟡 中 |
| Round 7 | P4-7 虚拟键盘集成 | 中 | 🟡 中 |
| Round 8 | P1-1/P1-2 Platform trait 可选方法分离 + ControlBackend 辅助方法 | 小 | 🟡 中 |
| Round 9 | P4-5 投影屏实验性支持 | 中 | 🟢 低 |
| Round 10 | P1-3 render/web/ 模块连接决策与实施 | 小 | 🟢 低 |
| Round 11 | P2-1~P2-5 平台后端真实原生实施（逐一，每个后端独立） | 极大 | ⚪ 架构 |
| Round 12 | P3-1 剩余模块测试覆盖（render/control_backend/web/json） | 大 | ⚪ 改进 |

---

## 🏔️ 冰山模式扫描

### 模式 1: 触摸事件=鼠标事件别名

当前代码中不存在触摸事件，所有触摸输入必须模拟为鼠标事件。这导致双指缩放、长按、旋转等手势完全不可用。

**受影响控件**: 全部交互控件（40+）
**影响范围**: 全项目

### 模式 2: 测试覆盖缺失

从 BLUE7 继承。widget/ 零测试、render/backend 零测试、control_backend/ 零测试。每次手动测试而非自动化，回归风险高。

**受影响模块**: 8 个核心模块
**影响范围**: 全项目

### 模式 3: Platform 后端实现深度不均

Windows 有真实 Win32 调用，Linux 有部分 GTK 集成，其余 4 个后端全为状态模拟。同一 Platform trait 下实现质量相差 10 倍。

**受影响后端**: Wayland, Harmony, Mobile, macOS objc2
**影响范围**: 平台层

---

## 📊 统计

| 优先级 | 数量 | 关键项 |
|--------|------|--------|
| 🔴 P0 | 0 | 无未修复功能阻断问题 |
| 🟠 P1 | 3 | ControlBackend 默认方法、Platform 可选分离、render/web 连接 |
| 🟡 P2 | 5 | 5 个平台后端实施缺口（Wayland/objc2/Harmony/Mobile/Windows） |
| 🔵 P3 | 1 | 测试覆盖不足（8 个模块零测试） |
| ⚪ P4 | 7 | 触摸事件系统、手势识别器、40+ 控件触摸适配、触摸目标尺寸、投影屏、设备检测、虚拟键盘 |
| **合计** | **16** | |

---

## 📈 预期质量评分基线

| 维度 | 当前分数 | 说明 |
|------|----------|------|
| 编译可靠性 | 10/10 | 已完成 |
| 触摸交互完整度 | 0/10 | 新增维度，当前完全无触摸支持 |
| 手势识别能力 | 0/10 | 新增维度 |
| 设备自适应 | 0/10 | 新增维度 |
| 测试覆盖 | 3/10 | 继承 BLUE7 |
| 平台后端正交性 | 5/10 | 继承 BLUE7 |
| Widget 基础模式 | 8/10 | 继承 BLUE7 |
| **综合** | **3.7/5.0** | 新增 3 个触摸维度拉低均分 |

预期 BLUE8 Round 1-6 完成后可提升至 **4.2/5.0**。

---

> **BLUE8 范围**: 16 项 P1-P4，不含代码实现，仅设计/规划/评估。
> **下一阶段**: Round 1（测试）→ Round 2（触摸事件）→ Round 3（高优控件触摸适配）的顺序推进。
