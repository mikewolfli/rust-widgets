# rust_widgets — 纯 Rust GUI 库

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

纯 Rust 编写的跨平台 GUI 库。支持桌面、平板、手机、嵌入式以及精简特性（**mini**）目标。

## ✨ 所有控件均为自绘

**本库 100% 自绘全部控件，在任何平台上都不创建操作系统原生控件。**

整个 crate 中没有任何 `CreateWindowExW`／`NSButton`／`gtk_button_new`／
`android.widget.Button` 调用。后端唯一的职责是把一块**绘图表面**交给渲染器；
下文列出的每一个按钮、列表、编辑器、菜单与图表，都由同一套 Rust 光栅化器绘制，
因此无论在 Windows、macOS、Linux、iOS、Android 还是 Web 上，控件的外观与行为完全一致。

```
        ┌──────────────────────────────────────────┐
        │  rust_widgets  —  自绘全部控件            │
        └──────────────────────────────────────────┘
             │  光栅化输出（RGBA / SVG / GPU）
             ▼
  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
  │ Windows HWND │   │ macOS NSView │   │  GTK widget  │   … 每个后端一块表面
  └──────────────┘   └──────────────┘   └──────────────┘
```

### 为什么这很重要

| 特性 | 自绘（本库） | 原生控件 |
|---|---|---|
| 外观 | **跨 OS 完全一致** | 随各 OS 工具包与版本变化 |
| 控件数量 | **175 种，全平台可用** | 仅限该 OS 工具包提供的 |
| 依赖体积 | **不链接任何 GUI 工具包** | GTK / AppKit / Win32 / Android SDK |
| 无OS与嵌入式 | **无 OS 也能运行**（`mini`、SVG） | 不可能 |
| 测试确定性 | **像素／序列化快照** | 需要真实显示器 |

### 每个后端*仍*负责什么

自绘不等于「不需要后端」。后端仍拥有真正属于操作系统的部分，且仅限于此：

- **表面与事件循环** — 创建窗口、绘制回调、resize。
- **输入** — 键盘／鼠标／触摸，转换为统一的 `Event`。
- **平台服务** — IME、剪贴板、无障碍桥、文件对话框、DPI 缩放。

连表面都无法提供的目标（例如裸帧缓冲）同样可用：它改为绘制到内存缓冲区。
参见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

> **从 1.x 升级？** 2.0.0 已从全部十个后端移除原生控件构造。
> 参见 [`CHANGELOG.md`](CHANGELOG.md) 与 [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md)。

---

## OS 支持矩阵

### 1. 各 OS 的平台服务

以下是后端对*操作系统*能力的报告。全部可通过运行时接口
`PlatformCapabilities`（`rust_widgets::PlatformCapabilities`）查询——
请读取它而不要假设：后端若运行在编译时未匹配的 OS 上，会如实返回 `false`。

| OS | 后端 | 家族 | DPI 缩放 | IME | 无障碍 | 原生菜单 | 可配置 |
|----|------|------|:-------:|:---:|:------:|:-------:|:------:|
| **Windows** | `WindowsPlatform` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **macOS** | `cocoa` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **macOS**（objc2 预览） | `macos-objc2-preview` | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux / GTK** | GTK 后端 | Desktop | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux / Wayland** | `wayland` | Desktop | ✅ | ✅ | ✅ | ❌ | ✅ |
| **iOS** | `ios-state-backend` | Mobile | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Android** | `android-state-backend` | Mobile | ✅ | ✅ | ✅ | ❌ | ❌ |
| **HarmonyOS** | `harmony-desktop` | Desktop | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Web (WASM)** | `wasm-state-backend` | Embedded | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Portable / 无 OS** | `portable` | Embedded | ❌ | ❌ | ❌ | ❌ | ❌ |

**说明。** *原生菜单* 指该 OS 提供菜单栏协议。Wayland 没有该协议，
故其后端把菜单树保存在进程内、由宿主渲染——声称支持原生菜单将是虚假的。
*可配置* 指后端除能力标志外还暴露 OS 级设置（主题、强调色、通知器）。

> **如何读 `native_menu` 一列。** 未覆写 `Platform::capabilities` 的后端会继承 trait 默认值，
> 即「若后端报告 `Desktop` 家族则为 `true`」。Wayland、iOS、Android、HarmonyOS
> 显式覆写为 `false`，因为它们确实没有菜单协议；Windows、macOS、GTK 保持默认。
> 上表数值由测试（`published_os_capability_matrix_matches_the_trait_default`）钉住，不会脱节。
>
> **控件集刻意*不在*此表中。** 因为所有控件均自绘，控件可用性不随 OS 变化，
> 而是随 **profile** 变化——即下一张表。

### 2. 各 profile 的控件可用性

跨目标真正有差异的，是**编译进来多少控件**，而非 OS 能画什么。

| Profile | 控件集 | 注册表 | 自绘控件托管 | GPU | i18n |
|---------|-------|:------:|:-----------:|:---:|:----:|
| `desktop` | **175 种**（完整） | ✅ | ✅ | ✅ wgpu | ✅ |
| `tablet` | **175 种**（完整） | ✅ | ✅ | ✅ wgpu | ✅ |
| `mobile` | **175 种**（完整） | ✅ | ✅ | ✅ wgpu | ✅ |
| `embedded` | 精简核心集 | — | — | — 软件 | — |
| `mini` | 精简核心集 | — | — | — 软件 | — |

`—` 表示**被编译移除，而非降级**：模块不存在，
故 `supports_custom_widgets()` 返回 `false`，调用方应拒绝该操作，
而不是挂载到空白表面上。

精简集（`embedded`／`mini`）包含：Window、Button、CheckBox、RadioButton、Label、
LineEdit、ComboBox、SpinBox、ListBox、ProgressBar、Slider、ScrollBar、ScrollArea、
Panel、Frame、GroupBox、Line、Meter、MiniChart、ImageView、MiniCanvas、
Arc、Spinner、Roller、Dropdown、TextArea、Keyboard、Switch。

### 3. 「支持」在两表中的含义

| 关注点 | 随 OS 变化？ | 随 profile 变化？ |
|---|:---:|:---:|
| 控件外观 | ❌（自绘） | ❌ |
| 哪些控件存在 | ❌ | ✅ |
| DPI 缩放 / IME / 无障碍 | ✅ | ❌ |
| 原生菜单栏 | ✅ | ❌ |
| 文件／颜色／字体对话框 | ✅（宿主提供） | ❌ |
| 渲染后端 | ❌ | ✅（GPU 或软件） |

因此，避开 OS 专有 API 的应用天然可移植：按 profile 构建一次，处处渲染一致。

---

175 种控件全部为自绘。每一种都能通过 `factory_name_for_kind` 解析出构造器
（含别名共 **456** 个可解析名称）；
新增 kind 若无法归类、或解析不出任何构造器，`tools/check_widget_registration_fidelity.sh`
会直接失败 —— 后者已捕获 4 类 `create_*` 永远返回 id `0` 的缺陷
（`Frame`、`DockPanel`、`CupertinoSwitch` 与 9 个 WebEngine 名称），而当时其余门禁全绿；
平台能力矩阵
（`docs/plans/platform_capability_matrix.md`）由源码机械派生，并在 CI 中设有防脱节门禁。

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.3.2-blue)]()
[![tests](https://img.shields.io/badge/tests-4900%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

**2.3.2 实测：** `desktop` 档 **4916** 个库测试全通过（`tablet` **4688**、`mobile` **4716**、
`embedded` **1558**、`mini` **1497**）；`cargo test` 在全部 27 个测试二进制上 0 失败。
`cargo clippy --all-targets -- -D warnings` 0 warning，五个档位均可构建。
共 **32** 个门禁全通过；唯一 skip 需 macOS 主机且已自述原因。
详见
[`CHANGELOG.md`](CHANGELOG.md)（中文版见 [`docs/reports/CHANGELOG.md`](docs/reports/CHANGELOG.md)）。

<p align="center">
  <a href="README.md">
    <img src="https://img.shields.io/badge/lang-English-blue" alt="English">
  </a>
</p>

---

## 快速开始

```bash
# 桌面（默认）
cargo check

# Mini（精简 std 特性，最小控件集）
cargo check --no-default-features --features mini

# 嵌入式
cargo check --no-default-features --features embedded

# 测试（lib 套件；CI 实际命令为 `cargo test --all-features -q`）
cargo test --lib

# CI 使用的交叉编译检查（无需系统库）
cargo check --target wasm32-unknown-unknown --no-default-features --features wasm
cargo check --target x86_64-pc-windows-msvc --no-default-features \
  --features "windows desktop-runtime wgpu touch i18n controls-native controls-custom serde serde_json advanced-widgets quality-management"
```

> **Android**：用 `./tools/build_android_testapp.sh` 构建 JNI 测试 APK
> （`ANDROID_SDK_ROOT` 默认 `~/Android/Sdk`；NDK 取自 `$ANDROID_SDK_ROOT/ndk`）。
> 详见[构建要求](#构建要求)。

### 设备配置

**只能选一个。** 设备配置之间互斥：`mini`/`embedded` 会将部分模块**整体编译移除**，
因此与 `desktop` 同时开启不是「取最小集」，而是直接构建失败。

```bash
# ✅ 正确
cargo check                                        # desktop（默认）
cargo check --no-default-features --features mini
cargo check --no-default-features --features embedded

# ❌ 错误：desktop 仍然生效，精简配置要移除的模块照样被编译
cargo check --features mini
```

| 配置 | 命令 | 渲染后端 | 控件数 | i18n | GPU |
|------|------|----------|--------|------|-----|
| 桌面 | `cargo check` | 原生 OS | 完整控件集 | ✅ | ✅（desktop 默认启用 wgpu） |
| 平板 | `--no-default-features --features tablet` | 原生 OS | 完整控件集 | ✅ | ✅（tablet 默认启用 wgpu） |
| 手机 | `--no-default-features --features mobile` | 手机 API | 完整控件集 | ✅ | ✅（mobile 默认启用 wgpu） |
| 嵌入式 | `--no-default-features --features embedded` | 软件 | 核心控件集 | — | — |
| **Mini** | `--no-default-features --features mini` | **精简 std** + alloc | **核心控件集** | — | — |

#### 各配置关闭了什么

各配置的 API 完全一致，差别只在**能力是否存在**。只有同时具备平台后端且保留
`widget::runtime` 的配置，才能承载自绘型控件：

| 能力 | 桌面 | 嵌入式 | Mini |
|------|:----:|:------:|:----:|
| `widget::runtime`（控件注册表） | ✅ | — | — |
| 自绘型控件（`mount_custom_widget`） | ✅ | — | — |
| `supports_custom_widgets()` | `true` | `false` | `false` |
| 菜单 / 工具栏 / 状态栏 | ✅ | ✅ | ✅ |
| 菜单快捷键（显示） | ✅ | ✅ | ✅ |
| 菜单快捷键（真的能用） | ✅ | ✅ | ✅ |

表中 `—` 表示能力**不存在，而非降级**：模块已被编译移除，因此
`supports_custom_widgets()` 返回 `false`，调用方应据此拒绝操作，而不是挂载后得到一个
空白窗口（参见 `demo/code_editor` 的启动检查）。

菜单与快捷键**刻意不受影响**：它们的代码没有 `mini` 门控。所以 `mini` 准确说是
「**无自绘型控件承载能力，但菜单完整可用**」。

> CI 的 `cargo test --all-features` 会打开所有特性，即 `desktop` 与 `mini` **同时生效**。
> 这个组合就是本约束的回归探针；完整论证与验证矩阵见
> [`docs/plans/platform_differences.md`](docs/plans/platform_differences.md)。

#### `tablet` / `mobile` 需显式指定操作系统后端

与 `desktop` 不同，`tablet` 与 `mobile` 配置**自身不会选中任何 OS 后端** —— 它们唯一
的后端入口是 `os-auto`，而该 feature 目前是空的。使用时必须显式指定后端：

```bash
# ⚠️ 在所有 OS 上都会落到 stub 后端：没有任何真实控件
cargo check --no-default-features --features tablet

# ✅ 真实后端
cargo check --no-default-features --features "tablet,macos"
```

依赖这两个配置前需要注意两点：

* 不指定后端时**不会报错**，而是静默使用 `macos-fallback-stub`（其他 OS 同理）。
  不确定时可调用 `rust_widgets::backend_name()` 确认。
* 在 macOS 上，`tablet`/`mobile` 选中的是 **objc2 预览后端**，它**尚未实现自绘型控件的
  承载**。目前 macOS 上承载自绘型控件需使用 `desktop` 配置（`cocoa` 后端）。请查询
  `supports_custom_widgets()` 而不要臆测。

### 操作系统支持

| 系统 | 特性 | 自动检测 | 已在 macOS 上做交叉验证 |
|------|------|:--------:|:----------------------:|
| Windows (Win32) | `windows` | ✅ | ✅ `x86_64-pc-windows-gnu`（0 warning）|
| macOS (Cocoa/objc2) | `macos` | ✅ | 不适用（本机）|
| iOS (UIKit) | `ios` | ✅ | ✅ 真机与模拟器，含 `--all-targets` |
| Linux (GTK) | `linux-gtk` | — | 需要 cross sysroot（见 CI）|
| Linux (Wayland) | `linux-wayland` | — | 需要 cross sysroot（见 CI）|
| Android (JNI) | `android` | ✅ | 需要 NDK（见 CI）|
| Web (WASM) | `wasm` | — | ✅ `wasm32-unknown-unknown`，含 `--all-targets` |
| HarmonyOS | `harmony` | — | ✅ `aarch64`/`armv7`/`x86_64-unknown-linux-ohos`，**已编译并链接** |

> **「交叉验证」指的是真的编译过，不是声明。** HarmonyOS 一行不仅 `check`，还对着
> OpenHarmony SDK sysroot **完成链接**，并断言产物 `librust_widgets.so` 的机器类型
> （`AArch64` / `ARM` / `X86-64`）——`cargo check` 从不链接，因此它无法发现工具链缺失
> 或架构不对。
>
> `loongarch64-unknown-linux-ohos` **不可构建**：rustup 没有该 target 的 std（Tier 3），
> SDK 也没有该架构的 libc。`tools/check_harmony_cross.sh` 不会为一个它从未真正碰过的
> target 报告通过，而是**正向钉住这个特定结果**，一旦情况变化就报错。
>

---

## 架构

```
┌────────────────────────────────────────────────────────────┐
│  API 层 — lib.rs + compat.rs（core/alloc 桥接）          │
├────────────────────────────────────────────────────────────┤
│  控件库  │  事件系统    │  布局引擎                         │
│  (30-80) │  (EventLoop,│  (Box, Grid, Flow,               │
│          │   Gesture)  │   Stack, Absolute)                │
├──────────┴─────────────┴──────────────────────────────────┤
│  i18n │ 主题 │ 信号系统 │ 控制后端                          │
├────────────────────────────────────────────────────────────┤
│  渲染：SoftwarePaintBackend / SvgPaintBackend / GPU         │
├────────────────────────────────────────────────────────────┤
│  平台：Windows │ macOS │ Linux │ iOS │ Android │ WASM      │
└────────────────────────────────────────────────────────────┘
```

---

## 特性

### Rust 原生设计
- no_std 就绪架构：所有文件经 `compat.rs`（`core`/`alloc`）导入共享类型，启用 `#![cfg_attr(feature = "mini", no_std)]` 是已跟踪的后续步骤——当前 `mini` profile 在 std 上编译。
- `compat.rs` 桥接：`HashMap→BTreeMap`，轻量 profile 使用兼容锁实现，`MiniVec<T,64>`，`MiniString<256>`，`MiniArena`
- `enum WidgetKind` + `trait Widget/Draw/EventHandler` — 零成本抽象
- Builder 模式：`Style::new().bg_color(RED).pad_all(8).build()`

### 渲染后端
- **SoftwarePaintBackend**：CPU 光栅化（RGBA 帧缓冲），用于 mini/嵌入式
- **SvgPaintBackend**：SVG 管线输出，用于测试和文档
- **GPU (wgpu)**：硬件加速，用于桌面/平板/手机

### 触摸与手势
- 11 个手势识别器：点击、双击、长按、滑动、平移、甩动、双指点击、双指滑动、长按拖拽、捏合、旋转
- 触摸目标自动扩展，适配小控件

### 布局
- Box、HBox、VBox、Grid、Form、Stack、Flow、Absolute、Anchor、Masonry
- 设备自适应布局缩放、字体缩放、最小触摸尺寸

### CSS 样式
- CSS 解析器 + 选择器引擎（`CssParser`、`CssSelector`）
- `Widget::apply_css(css, class)` — 控件级 CSS 应用
- `StyleSheetManager` — 全局样式表注册
- `CssWatcher` — 轮询式 CSS 热加载

### 主题系统
- `ThemeManager` — 具名主题、明暗切换、JSON 存取
- 语义令牌（颜色、字体、间距、边框）按控件角色解析
- `HighContrastMode` — 强制前景/背景配对，对比度可实测
- 对所有由本库创建的控件**自动生效**

### 声明式 JSON UI（仅库 API）
- `JsonLoader` — 由 JSON 描述构建控件树
- 属性应用走控件**自身**的属性契约
- 节点可选 `class` / `css`，由样式表驱动外观
- **不经 C ABI 暴露** —— 加载器没有生成的入口点

### 声明式视图层（`view`，设备档位）

把控件树描述为**状态的函数**，由库算出变了什么。

```rust
use rust_widgets::view::{Node, View, ViewEngine};
use rust_widgets::widget::capability::CapabilityValue;

struct Counter { count: i64 }

impl View for Counter {
    fn build(&self) -> Node {
        Node::new("group_box").key("root").child(
            Node::new("label")
                .key("count")
                .prop("text", CapabilityValue::String(format!("Count: {}", self.count))),
        )
    }
}

let mut engine = ViewEngine::new();
engine.mount(&state, &create);                  // 建树
// ……状态变化……
let report = engine.update(&state, &create);    // 只施加差异
assert_eq!(report.patches.len(), 1);            // 一个 SetProperty，别无其它
```

**为什么用它。** 没有它时，每个改状态的地方还得同时够到正确的控件、知道该设哪个属性 ——
于是「`count == 3` 时屏幕该是什么样」在任何地方都得不到回答。有了它，这个问题只有一个答案：
`build`。更新也随之变便宜、变局部：引擎把新树与旧树求差，只碰不同的部分，
因此某个兄弟节点被编辑时，控件的焦点、滚动偏移与内部状态依然存活。

**保留式模型不变。** 控件仍是带 `ObjectId` 的长期对象，`add_child` 仍然可用，两者可混用。
本层增加的是对**结构**的描述，不取代任何东西。

**怎么用**

1. 实现 `View::build` —— 从状态返回一棵 `Node` 树。用与 JSON 加载器相同的工厂名与属性名。
2. 提供构造器：`Fn(&Node) -> Option<ObjectId>`，通常是 `WidgetFactory::create` 加
   `runtime::register`。它是**注入**的而非硬连，因此本层可以**无窗口**测试。
3. 先 `engine.mount(&state, &create)` 一次，之后状态变化时 `engine.update(&state, &create)`。
4. 给列表项加 **`key`**。key 是 diff 在重建之间认出同一控件的依据；没有 key 时匹配退化为按位置，
   头部插入会使其后每个节点的身份漂移到错误的控件上。`report.positional_matches > 0` 即「该补 key」。

**由响应式状态驱动** —— `ReactiveHost` 用 `Binding` 驱动 `update`。因为
`BindingListener` 必须 `Send` 而引擎是 `!Send`，监听器只把变更记入队列，
由 UI 线程的 `pump()` 做实际工作，因此工作线程里的 `Binding::set` 能抵达活控件。

| 档位 | `view` |
|---|---|
| `desktop` / `tablet` / `mobile` | ✅ **默认编译**；加 `no-declarative-view` 可排除 |
| `mini` / `embedded` | ❌ **不存在** —— 仅 `add_child`（分配预算 + 无逐帧重求值调用方） |

> 不经 C ABI 暴露 —— 与 JSON 加载器一样仅 Rust 可用。
> 完整指南：[cookbook/zh-CN/src/chapters/declarative-view.md](cookbook/zh-CN/src/chapters/declarative-view.md)。

> **金融控件：** 六个自绘行情控件 —— 带指标叠加的 K 线图、成交量面板、深度曲线、
> 盘口报价表、行情表与振荡指标面板 —— 以及它们共用的技术指标计算。
> 详见 [cookbook/zh-CN/src/chapters/finance.md](cookbook/zh-CN/src/chapters/finance.md)。

> **C ABI 覆盖范围。** C ABI（`include/rw_generated.h`，129 个 `rw_*` 函数）
> 覆盖窗口管理、控件创建、逐控件属性与主题选择。创建与属性访问都是**通用**的：
> `rw_create_widget_of_kind(parent, "tree_view", ...)` 可触及每一个已注册控件
> （`rw_widget_kind_names` 列出全部），`rw_set_widget_property(id, "tooltip", ...)`
> 可触及每一个已发布属性（`rw_widget_property_names` 列出这些名称）。
> 主题经 `rw_set_theme` / `rw_theme_names` / `rw_set_high_contrast` 可达。
>
> 仍为仅 Rust 的两项：**JSON 布局加载器**（无生成入口），以及
> **作为文档的 CSS 样式表**（单个样式属性可按控件设置，但没有传递样式表的 ABI）。

### C ABI 能力一览

| 能力 | 入口 |
|---|---|
| 窗口生命周期 | `rw_create_window`、`rw_run`、`rw_quit` |
| 通用创建 | `rw_create_widget_of_kind`、`rw_widget_kind_names` |
| 逐类型创建 | `rw_create_button`、`rw_create_slider` … |
| 生命周期 | `rw_destroy_widget`、`rw_show_widget`、`rw_hide_widget` |
| 通用属性 | `rw_get_widget_property`、`rw_set_widget_property`、`rw_widget_property_names` |
| 文本与几何 | `rw_set_widget_text`、`rw_get_widget_text`、`rw_set_widget_geometry` |
| 集合 | `rw_widget_list_add`、`rw_widget_list_clear`、`rw_widget_list_count`、`rw_list_box_add_item`、`rw_combo_box_add_item` … |
| 滚动 | `rw_widget_set_scroll_position`、`rw_widget_scroll_to` |
| 主题 | `rw_set_theme`、`rw_theme_names`、`rw_set_high_contrast` |
| 错误 | `rw_error_code`、`rw_error_message` |

`bindings/` 下的每个绑定都由 `tools/check_binding_symbol_coverage.sh` 按此清单校验，
因此新增的 ABI 函数不会在某个语言中静默地不可达。

### 局部刷新（自动判定，已连进帧循环）
- `DirtyRegionTracker` 脏矩形追踪与合并；`render_dirty_regions()` 基于 `push_clip` / `pop_clip` 的局部重绘
- **由 `widget::runtime::RepaintMode` 驱动**：`mark_dirty_rect` 记录损坏区域，
  `render_frame_incremental` 只重绘受损区域，其余部分沿用上一帧
- **自动判定：由库决定是否启用。** 挂载时会经 `should_track_damage` 判定，
  仅在区域化确实划算时才启用 `RepaintMode::Adaptive` —— 即面积够大、承载不止一个控件、
  且确实请求过重绘。面积低于 `AUTO_REPAINT_MIN_PIXELS`（≈500×500）或无子控件的表面保持
  `Full` 且不付任何簿记开销，因为对它们来说区域化的成本高于收益
- damage 由 `BaseWidget::request_redraw` 记录 —— 库中每一次外观变化汇聚的唯一咽喉点，
  因此局部重绘**由构造保证**正确，而不依赖一份变更点清单
- `Adaptive` 会自我修正：某一帧的 damage 覆盖整个表面时该帧回退为整幅重绘，
  待 damage 缩小后自动恢复区域重绘 —— 所以自动判定不可能产出错误的一帧，只有有界簿记开销
- 判定可观察，不是黑盒：`should_track_damage`、`enable_damage_tracking_if_useful`、
  `adaptive_large_damage_run`，以及可用 `set_repaint_mode` 覆盖

### 国际化（i18n）
- `tr!()` 宏实现编译期键值翻译
- 中 / 英 / 繁 翻译包（各 30+ UI 字符串）
- 上下文翻译与复数形式支持
- `audit_keys()` 翻译覆盖率验证

---

## 控件库

### 桌面/平板/手机（175 种控件）

**核心**：Window、Dialog、MessageBox、FileDialog、ColorDialog、FontDialog、InputDialog、ProgressDialog、PopupWindow、Button、CheckBox、RadioButton、Label、LineEdit、TextEdit、RichEdit、ComboBox、SpinBox、ListBox、ListView、TreeView、ProgressBar、Slider、ScrollBar、ScrollArea、TabWidget、Splitter、GroupBox、MenuBar、Menu、MenuItem、ContextMenu、ToolBar、StatusBar、Canvas、Table、Grid、Chart、ToggleButton

**日期与时间**：Calendar、DateEdit、TimeEdit、DateTimeEdit、DatePicker、TimePicker、DateTimePicker、CupertinoDatePicker、DateRangePicker、MobileDatePicker

**容器**：CollapsiblePane、DockWidget、MdiArea、StackedWidget、ToolBox、TabBar、NavigationStack、Carousel、BottomSheet、ModalBottomSheet

**移动端**：BottomNavigationBar、NavigationDrawer、AppBar、SafeArea、PullToRefresh、RefreshControl、SearchBar、CupertinoSwitch、CupertinoSlider、CupertinoNavigationBar、CupertinoSegmentedControl、AdaptiveScaffold

**输入**：CommandLink、FontComboBox、KeySequenceEdit、MaskedEdit、AutoCompleteEdit、MultiSelectComboBox、EditableComboBox、RangeSlider、FloatingLabel、TagInput、InplaceEditor、SearchBox、ShortcutEditor

**显示**：LCDNumber、Dial、ProgressCircle、Rating、Icon、Sparkline、Tooltip、Badge、Chip、Avatar、SkeletonLoader、EmptyState

**图表**：LineChart、BarChart、PieChart、Sparkline

**网页**：WebView、WebEngineView、WebEnginePage、WebEngineSettings、WebEngineDownloadItem、WebEngineCookieStore、WebEngineWebChannel、WebEngineFindTextResult、WebEngineNotification、WebEngineScriptDialog、WebEngineContextMenuRequest

**菜单**：PieMenu、RibbonBar、MenuButton、DropdownMenu、Popover、SegmentedButton

**特殊**：FreeformShape、QRCode、ColorHistory、ColorWell、MasonryLayout、Stepper、Divider、SwipeToDismiss、Toolbox、PropertiesPanel、PropertyGrid、WizardDialog、Wizard、AnimatedImage、HeroAnimation、BezierCurveEditor、LottieWidget、RiveWidget、VideoPlayer、ImageGallery、AudioVisualizer、CameraPreview、BarcodeScanner、Breakcrumb、CodeEditor、ColorPicker、CommandEntry、CommandPalette、DiffViewer、MapView、MediaPlayer、NotificationCenter、Snackbar、SplitButton、TerminalView、ToastStack

### Mini / Embedded（精简核心控件集）

Window、Button、CheckBox、RadioButton、Label、LineEdit、ComboBox、SpinBox、ListBox、ProgressBar、Slider、ScrollBar、ScrollArea、Panel、Frame、GroupBox、Line、Meter、MiniChart、ImageView、MiniCanvas、Arc、Spinner、Roller、Dropdown、TextArea、Keyboard、Switch

---

## C ABI 与语言绑定

```bash
cargo build --release
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/release -lrust_widgets -o target/release/c_abi_poll_demo
python examples/python/demo_basic.py
```

| 语言 | 状态 |
|------|:----:|
| C | ✅ |
| C++ | ✅ |
| Python | ✅ |
| Java (JNI) | ✅ |

---

## 核心模块

| 模块 | 说明 | 可用范围 |
|------|------|:--------:|
| `core` | Point、Rect、Size、Color、Font、ObjectId | 全部 |
| `widget` | 控件实现 | 全部 |
| `event` | 事件类型、EventLoop、GestureEngine | 全部 |
| `compat` | core/alloc 桥接、MiniVec、MiniString、MiniArena | 全部 |
| `render` | SoftwarePaintBackend、SvgPaintBackend、GPU | 全部 |
| `layout` | Box、Grid、Flow、Stack、Absolute、Masonry | 全部 |
| `signal` | GenericSignal、Signal1、ConnectionScope | 全部 |
| `style` | WidgetStyle、CSS 解析器、动画、主题状态 | 全部 |
| `object` | 对象/类名系统 | 全部 |
| `platform` | Windows、macOS、Linux、iOS、Android、WASM、Harmony | 桌面+ |
| `gesture` | 11 个手势识别器 | 桌面+ (touch) |
| `i18n` | `tr!()` 宏、I18nManager、中/英/繁 | 桌面+ |
| `theme` | 主题管理器、深色/浅色模式 | 桌面+ |
| `gpu` | GPU 适配器检测、缓冲池 | 桌面+ |
| `chart` | 折线图、柱状图、饼图、散点图 | 桌面+ |
| `web` | WebEngine、WebView、JS 引擎 | 桌面+ |
| `pdf` | PDF 文档创建 | 桌面+ |
| `print` | 打印支持 | 桌面+ |
| `performance` | 性能分析器、帧率监控 | 桌面+ |
| `memory` | ObjectPool、ArenaAllocator、BufferPool | 桌面+ |

---

## 构建要求

| 配置 | Rust 版本 | 依赖 |
|------|:---------:|------|
| 桌面 | 1.87+ | wgpu、GTK/Wayland (Linux)、objc2 (macOS) |
| Mini | 1.87+ | heapless、hashbrown、bumpalo（no_std 就绪；profile 在 std 上编译） |
| 嵌入式 | 1.87+ | 无 |

### 图像编解码与交叉编译

AVIF 支持使用**纯 Rust** 的 `avif` 编解码器（ravif），而非 `avif-native`，因此为
异种目标构建 `mobile`/`tablet`/`desktop` 时**不需要** `dav1d` sysroot，也无需手工配置
交叉 `pkg-config`。早期版本会引入 `dav1d-sys`，除非手工配好 pkg-config sysroot，
否则 Android/iOS/wasm 的交叉编译会失败。

代价是解码速度：纯 Rust 编解码器慢于 C 版 `dav1d` 后端，并增加约 15 个构建期
crate（`rav1e` 等）。

---

## 性能

| 指标 | 桌面 | Mini（目标值） |
|------|------|----------------|
| 二进制体积 | ~5MB | < 100KB |
| 内存占用（典型） | < 100MB | < 32KB |
| 帧率 | 60 FPS | 30 FPS |
| 控件创建耗时 | < 1ms | < 0.1ms |

---

## 许可

MIT License — 详见 [LICENSE](LICENSE)。

## 支持

- Issues：[GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)
- **Cookbook 手册**：[cookbook/](cookbook/) —— 主文档，提供英文（`cookbook/en/`）、
  简体中文（`cookbook/zh-CN/`）与繁体中文（`cookbook/zh-TW/`）三个版本
