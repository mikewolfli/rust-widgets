# rust_widgets — 纯 Rust GUI 库

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

一个用纯 Rust 编写的跨平台 GUI 库。**所有控件都由库自己绘制**——整个 crate 里没有一处
`CreateWindowExW`、`NSButton`、`gtk_button_new` 或 `android.widget.Button`——并且可以渲染到窗口、
PNG 或 SVG。支持桌面、平板、移动、嵌入式，以及最小化的 `mini` 配置。

自绘控件换来的是：

| | 自绘（本库） | 原生控件 |
|---|---|---|
| 外观 | 每个操作系统完全一致 | 随工具链与版本变化 |
| 控件数量 | 各平台都是 180 种 | 只有工具链提供的那些 |
| 依赖 | 不链接任何 GUI 工具链 | GTK / AppKit / Win32 / Android SDK |
| 无头 / 嵌入式 | 完全不需要操作系统（`mini`、SVG） | 不可能 |
| 测试 | 像素与 SVG 快照 | 需要真实显示器 |

后端仍然负责真正属于操作系统的部分：窗口创建与事件循环、输入转换，以及平台服务（输入法、
剪贴板、文件对话框、DPI）。若某后端连绘制面都提供不了（例如裸帧缓冲），库会改为绘制到内存
缓冲区。详见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

## 安装

```toml
[dependencies]
rust_widgets = "2.6.1"
```

设备配置**只能选一个**。它们互斥——`mini` 和 `embedded` 会把 crate 的一部分**编译掉**，所以把
它们和 `desktop` 叠在一起不是「取最小公分母」，而是构建失败：

```toml
rust_widgets = { version = "2.6.1", features = ["desktop"] }                       # 默认
rust_widgets = { version = "2.6.1", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.6.1", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.6.1", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.6.1", default-features = false, features = ["mini"] }
```

> `cargo check --features embedded` 是**错的**：`desktop` 是默认特性，这条命令会同时打开两个互斥
> 配置。选择 `desktop` 以外的任何配置时，都必须加 `--no-default-features`。

## 第一个控件

```rust
use rust_widgets::core::{Color, Rect};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::Widget;   // 把 `set_style` 带入作用域

fn main() {
    // 控件就是放在 `Rect` 里的普通值。渲染成 SVG 是看到它最短的路径；
    // 换后端只改变像素去哪里，不改变控件怎么画。
    let mut button = Button::new("点击我".to_string(), Rect::new(0, 0, 160, 36));

    // 样式是一个字段全为可选值的普通结构体，因此未设置的字段是「继承」，
    // 而不会覆盖主题为该控件已解析出的颜色。
    let style = WidgetStyle {
        background_color: Some(Color::rgb(33, 150, 243)),
        text_color: Some(Color::WHITE),
        border_radius: Some(6),
        ..WidgetStyle::default()
    };
    button.set_style(style);

    println!("{}", rust_widgets::widget::svg::render_to_svg(&mut button));
}
```

窗口创建、事件循环、布局、主题、国际化与 C ABI 各有独立章节，从
[`cookbook/zh-CN/src/README.md`](cookbook/zh-CN/src/README.md) 开始。

## 渲染

| 后端 | 用途 | 说明 |
|---|---|---|
| 软件光栅化器 | 各处的默认后端 | 抗锯齿，不需要 GPU，可无头运行 |
| SVG | 快照、打印、矢量输出 | 逐字节可复现；`snapshots/svg/` 就是它产出的 |
| wgpu | GPU 加速 | 显式降级阶梯：Vulkan/Metal/DX12/WebGPU → GL → CPU |

渲染器的文本原点是字形框的**左上角**，不是基线。两个后端对这一契约的实现完全一致（SVG 后端会写
`dominant-baseline="text-before-edge"`），因此按其中一个后端测出的标签位置，在另一个后端也落在
同一处。正是这个性质让 `snapshots/svg/` 成为可用的评审产物，而不是第二个会漂移的渲染器。

## 控件的尺寸

控件拿到的矩形是「**可以使用**的区域」，不是「应该画多大」——这是两个问题。回答第二个，才是开关
不会被画成 240×120 体育场的原因：

```rust
implicit_size = max(floor, content + padding)     // Qt Quick 的 Button.qml 公式
```

承重的是那个 `max`：**地板 = 最小可点区**，所以 5px 文字的文字按钮仍然是 `64×40`，而不是 `64×18`。
`rust_widgets::widget::ControlMetrics` 提供这个公式及建立在其上的几何 helper，
`rust_widgets::widget::metrics::dimensions` 集中所有控件尺寸（轨道、手柄半径、输入框高度、内边距），
同一事实只有一处定义：

| helper | 回答的问题 |
|---|---|
| `implicit_size` / `content_box` | 我该多大？ / 我的内容能放到哪？ |
| `center_in` / `centered_disc` / `centered_square` | 固定尺寸 chrome 居中，**只夹不撑** |
| `centered_band` / `full_width_band` | 全宽、自己的高度、垂直居中 |
| `top_band` / `bottom_band` / `band_inset` | 把条带钉在某一边，内容跟在后面 |
| `focus_ring_rect` / `focus_ring_color` | 键盘焦点环，内缩因而不会压住邻居 |

## 控件的布局

布局是**向每个子控件要尺寸**，而不是被调用方提前告知。每个控件声明自己的诉求为「地板 / 期望 / 上限」
三个值，因为「最小能压到多少」与「最大能拉到多少」是一个尺寸回答不了的：

```rust
use rust_widgets::layout::{AxisHints, Hints, LayoutParams, ChildInfo, Layout};

// 最小 120、期望 200、不超过 400；允许被横向拉伸。
let hints = Hints { width: AxisHints::new(120, 200, 400), height: AxisHints::fixed(32) };
let children = [ChildInfo::new(widget_id, hints).with_params(LayoutParams::filled())];

let mut out = Vec::new();
layout.arrange(rect, &children, &mut |id, child_rect| out.push((id, child_rect)));
```

`fill` 与尺寸**刻意分开**：滑块与按钮的期望尺寸可以相同，但只有其中一个该被拉伸铺满表单。
`AxisHints::new` 在构造时就把 `min <= pref <= max` 归一化，因此**非法状态不可表示**。
既有布局全部照常工作——`Layout::arrange` 的默认实现转 `Layout::update`。

## 控件的行为

三条靠外形看不出来的契约：

- **`clicked` 要求释放点落在控件内部**。从按钮上按下、拖出后释放，会发 `canceled` 而不是 `clicked`；
  且只有主键能激活。
- **`pressed` 是连续量**。按住拖出会清除它，拖回会恢复它，因此控件反映指针的真实位置。
- **焦点环在「用户在用键盘」时画，而不只是「控件有焦点」时画**。`Event::FocusGained` 携带
  `FocusReason`，`FocusReason::draws_focus_ring()` 对鼠标点击返回 `false`——光标下画环会读成卡住的
  高亮。

## 验证一次改动

```bash
cargo test --no-default-features --features desktop            # 全量测试
cargo clippy --no-default-features --features desktop --all-targets -- -D warnings
cargo run  --no-default-features --features desktop --example export_control_svgs
bash tools/run_all_gates.sh                                    # 全部门禁，输出 PASS/FAIL 表
```

[`snapshots/svg/`](snapshots/svg/) 下的 376 个 SVG 是「每个控件 × 明暗两种外观」各一份。它们**被提交
也被重新生成**，只要控件的绘制变了而快照没更新，`tools/check_svg_snapshots.sh` 就会逐字节失败。
于是一个「看起来不对」的控件会在评审里以 diff 的形式出现；而两个文件完全相同的控件，就是肉眼可见
的主题盲。

`tools/run_all_gates.sh` 会跑遍 `tools/check_*.sh` 并打印每个门禁的通过情况与耗时。每个门禁都必须
**能失败**；其中最关键的那些都用反向注入验证过——故意把缺陷改回去，确认门禁会变红。

## 「180 个控件」覆盖什么

控件库覆盖：文本与输入（按钮、开关、输入框、掩码/一次性验证码/日期/时间编辑器、搜索框、标签输入、
虚拟键盘、富文本、Markdown、代码编辑器、终端）；选择与展示（列表、表格与支持冻结列虚拟化的数据网格、
树、chip、badge、评分、进度、骨架屏）；容器与框架（标签页、分割条、停靠面板、MDI、工具栏、菜单、
状态栏、功能带、工具箱）；对话框与浮层（消息框、文件/字体/取色器、向导、气泡、提示、横幅、吐司、
Snackbar、底部面板）；导航、媒体，以及 Material 没有对应物的图表族：K 线、成交量、盘口深度、指标、
雷达、热力图、仪表、迷你走势图。

会裁剪控件集合的配置（`mini`、`embedded`）保留精简核心；每个配置的确切集合是生成并被门禁校验的，
列在 [`docs/plans/platform_capability_matrix.md`](docs/plans/platform_capability_matrix.md)。

## 平台说明

- **Windows / macOS / Linux** —— 完整配置，真实绘制面与事件循环。
- **Linux/Wayland** —— 合成相关测试在 `tools/run_wayland_compositor_tests.sh`。
- **iOS / Android** —— 完整配置；JNI 测试 APK 用 `tools/build_android_testapp.sh` 构建。
- **Web (wasm32)** —— WebGL/WebGPU 绘制面；`cfg(target_arch = "wasm32")` 描述的是沙箱约束而非操作
  系统，操作系统相关的知识全部留在 `src/platform/` 内。
- **嵌入式 / mini** —— 仅软件光栅化器，无操作系统服务。

目前确实做不到的能力，会通过运行时能力查询诚实地返回 `false`，而不是假装支持。
`supports_custom_widgets()`、`supports_web_engine()`、`has_real_engine()` 给的是真实答案，不是
编译期桩。

## 语言绑定

`C ABI` 位于 `src/bindings/`，通过 **130 个 `rw_*` 函数**暴露每个控件，并提供基于能力的属性与事件模型。C、C++、
Python 与 Java（JNI）绑定都在 CI 中运行；生成的头文件由 `tools/check_abi.sh` 检查漂移。

见 [`cookbook/zh-CN/src/chapters/language-bindings.md`](cookbook/zh-CN/src/chapters/language-bindings.md)。

## 文档

| 文档 | 内容 |
|---|---|
| [`cookbook/`](cookbook/) | 手册 —— 英文、简体中文、繁體中文 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | 模块划分与分层规则 |
| [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md) | 从 1.x 迁移（2.0 起移除原生控件创建） |
| [`CHANGELOG.md`](CHANGELOG.md) | 发行说明，含每项修复的证据 |
| [`docs/reports/`](docs/reports/) | 审计与质量报告 |
| [`docs/log/`](docs/log/) | 逐轮的工程日志 |

## 环境要求

Rust **1.87+**。默认构建不需要任何系统 GUI 库；Linux 额外用 Wayland/X11 提供绘制面。图像编解码器是
纯 Rust 的，因此交叉编译到 Android、iOS 或 wasm 不需要 `pkg-config` sysroot。

## 许可证

MIT —— 见 [LICENSE](LICENSE)。

## 支持

- 问题反馈：[GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.6.1-blue)]()
[![tests](https://img.shields.io/badge/tests-5600%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()

<p align="center">
  <a href="README.md">
    <img src="https://img.shields.io/badge/English-English-blue" alt="English">
  </a>
</p>
