# rust_widgets — 纯 Rust GUI 库

<p align="center">
  <img src="snapshots/header.jpg" alt="rust_widgets" width="800">
</p>

一个用纯 Rust 写的跨平台 GUI 库，**每一个控件都由它自己绘制**。crate 里没有 `NSButton`、没有
`gtk_button_new`、没有 `android.widget.Button`——有个门控专门 grep 这些名字，一旦出现就失败；全仓唯一一次
`CreateWindowExW` 是用来创建窗口本身的那次。

它能渲染到真实窗口、PNG 或 SVG，也能跑在 GUI 工具链跑不了的地方：`mini` 与 `embedded` 完全不需要操作系统。

| | 自绘（本库） | 原生控件 |
|---|---|---|
| 外观 | 每个系统完全一致 | 随工具链**及其版本**而变 |
| 控件数 | 处处 **181 种控件** | 工具链提供多少算多少 |
| 依赖 | 不链接任何 GUI 工具链 | GTK / AppKit / Win32 / Android SDK |
| 无界面 / 嵌入式 | 无 OS 也能跑（`mini`、SVG） | 做不到 |
| 测试 | 字节可复现的 SVG 与像素快照 | 需要真实显示器 |

后端只保留真正属于操作系统的部分：创建窗口、事件循环、输入翻译，以及 IME、剪贴板、文件对话框、DPI 这类服务。
如果某个后端连绘图表面都给不了（比如一块裸 framebuffer），库里就画进内存缓冲，照样继续。详见
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

## 安装

```toml
[dependencies]
rust_widgets = "2.8.9"
```

**只能选一个**设备 profile。它们互斥：`mini` 与 `embedded` 会把 crate 的一部分**编译掉**，所以把它和 `desktop`
凑在一起不是"取最小公共集"，而是一个根本跑不起来的构建。

```toml
rust_widgets = { version = "2.8.9", features = ["desktop"] }                       # 默认
rust_widgets = { version = "2.8.9", default-features = false, features = ["tablet"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["mobile"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["embedded"] }
rust_widgets = { version = "2.8.9", default-features = false, features = ["mini"] }
```

> `cargo check --features embedded` 是**错的**，而且错得很安静：`desktop` 是默认特性，这条命令同时打开了两个
> 互斥 profile。只要选的不是 `desktop`，就必须带 `--no-default-features`。

## 第一个控件

```rust
use rust_widgets::core::{Color, Rect};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::Widget;   // 让 `set_style` 进入作用域

fn main() {
    // 控件就是放进一个 `Rect` 里的值。渲染成 SVG 是看到它最快的路径；
    // 换后端只改变像素去哪里，不改变控件怎么画。
    let mut button = Button::new("点我".to_string(), Rect::new(0, 0, 160, 36));

    // Style 是一组可选值：未设置的字段是"继承"，而不是覆盖主题为该控件解析出的结果。
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
| 软件光栅器 | 处处默认 | 抗锯齿、不需要 GPU、可无界面运行 |
| SVG | 快照、打印、矢量输出 | 字节可复现；`snapshots/svg/` 就是它产出的 |
| wgpu | GPU 加速 | 明确的降级阶梯：Vulkan/Metal/DX12/WebGPU → GL → CPU |

渲染器的文字原点是字形盒的**左上角**，不是基线。两个后端遵守同一条契约——SVG 后端写
`dominant-baseline="text-before-edge"`——所以在其中一个上量出来的标签，在另一个上落点相同。这正是
`snapshots/svg/` 能当作审阅依据、而不是"第二个悄悄画歪的渲染器"的原因。

## 尺寸、布局、行为

控件由放置它的东西给一个矩形，而这个矩形是它**可以**占据的区域，不是它应该画多大。把这两个问题分开，才不会把开关
画成一个 240x120 的跑道：

```text
implicit_size = max(floor, content + padding)
```

这个 `max` 是关键：**下限是最小可点按面积**，所以文字只有 5 px 的按钮仍然是 64x40，而不是 64x18。
`widget::ControlMetrics` 暴露该公式与基于它的几何助手；`widget::metrics::dimensions` 集中所有控件尺寸，
一个事实只有一处定义。

布局会**逐个询问子控件想要多大**，而不是事先被告知。子控件用"下限 / 期望 / 上限"回答，因为"最小能被压到多小"
和"最大能被拉到多大"不可能由同一个数字表达。`fill` 与尺寸是分开的：滑块和按钮可以期望同样的尺寸，但一个表单里
只有一个该被拉满。`AxisHints::new` 在构造时就规范化 `min <= pref <= max`，所以不合规的提示值根本无法表示。

行为是三条控件形状无法表达的契约：

- **`clicked` 要求释放点落在控件内。** 从按钮上按下、在别处松手发出的是 `canceled`；只有主键会激活。
- **`pressed` 是连续量。** 按住后拖出会清除，拖回会恢复——它反映指针真实的位置。
- **焦点环表示"用户在键盘上"，而不只是"控件有焦点"。** 鼠标按下时 `FocusReason::draws_focus_ring()` 为
  `false`——光标下的一圈高亮读起来像卡住的选中态。

## 主题、动画、禁用态

颜色沿一条阶梯解析——控件显式样式 → 该控件在主题里的解析结果 → 最后才是字面值——所以没被碰过的控件照样跟随
外观切换，而调用者刻意设的颜色永远优先。`WidgetState` 在 `hover`/`pressed`/`disabled` 时重新解析这条阶梯，
主题作者写的 `"button:hover"` 因此真的能到达绘制代码。

动画只有一个"存储目标"的 `PropertyDriver`（一个在 `0.0`–`1.0` 之间移动的值，速率由 `MotionSlot` 节奏令牌决定），
通过 `Widget::tick` / `Widget::is_animating` 暴露，于是帧循环能发现"某个控件欠帧"，而不需要每个控件去注册自己。
`set_animating` 刻意不是一个可以手动开关的东西：`is_animating()` 是推导出来的，控件与帧循环不可能对"有没有东西在动"
各说一套。

禁用态是**两件事**，分开命名——因为把其中一件用在另一件该在的地方，会得到与预期**相反**的效果：

| | 向谁靠拢以"退后" | 实测 |
|---|---|---|
| `disabled_ink_on` —— 文字、图标 | 表面自身的对比色 | 4.57–5.91:1 |
| `disabled_surface_near` —— 填充、面板 | 它们背后的页面 | 始终比启用态更不显眼 |

两个权重都来自同一个常量 `dimensions::DISABLED_VEIL_ALPHA`（`0.55`），它是在深色与浅色**两种**外观下都刚好
越过 4.5:1 正文门槛的最小值。半透明的中灰没有方向：浅色背景上它会变暗、深色背景上它会变亮，于是"禁用"在本来
最难读的那种外观上反而显得**更**显眼。

## 文本覆盖

**默认构建只绘制拉丁/ASCII 字符。** crate 不随附任何字体数据，因此字形来自一套固定的 8x8 位图字体，覆盖
`U+0000`–`U+007F`。

| 输入 | 默认构建画什么 |
|---|---|
| `A`、`z`、`7`、`!` | 真实字形 |
| CJK、西里尔、阿拉伯、emoji 等 | **回退字形**——一个空心方块 |

标签照样参与布局、控件照样渲染，只有字形是错的。绘制顺序由 Unicode 双向算法决定，所以阿拉伯文一行是从右往左的——
但**顺序不等于字形**：脚本相关的整形（阿拉伯连写、印度语系重排）完全没有应用。

因此本库**不**实现 Unicode 文本渲染，也不把自己描述为多语言。超出拉丁/ASCII 的覆盖是一条独立的、按名申请的
可选轴，它需要**字体数据**——而且必须显式申请，因为 `mini` 构建不该背着它永远画不到的字体：

| 特性 | 数据 | 增加什么 |
|---|---|---|
| `fonts-cjk-bitmap` | 84 996 字节，生成 | 16x16 CJK 位图字体——中日韩，无需整形 |
| `fonts-cjk` | 361 704 字节，OFL 子集 | 同一批脚本的**轮廓**版本：抗锯齿、任意尺寸 |
| `fonts-vector-latin` | 35 896 字节，OFL 子集 | 真实字宽与字距调整 |
| `fonts-complex` | 70 576 字节，OFL 子集 | 阿拉伯连写，让 `بيت` 是一个词而不是三个字母 |
| `fonts-emoji-color` | 1 602 492 字节，OFL 子集 | 彩色 emoji，317 个码位，含 26 个区域指示符 |

没有任何 profile 会启用它们。轮廓与彩色子集从二进制的只读段惰性加载，来源摘要记录在 [`NOTICE`](NOTICE)。

## 图标

`Icon` 提供 **69** 个 `IconName` token。开启 `icons`（**默认开启**）时每个 token 画的是真正的
**Material Symbols** 轮廓；关闭时画的是从**同一批**轮廓生成的降级几何，因此两条路径不可能对同一个图标给出
不同形状。

```console
cargo build                                            # 真实轮廓（默认）
cargo build --no-default-features --features desktop  # 生成的降级形状
```

`IconName::data()` 返回 `IconData` 而不是 `Option`，所以"加了 token 却没有几何"是编译错误，而不是屏幕上的一块
空白。token 集合只在 `tools/icon_tokens.txt` 里声明一次，`tools/gen_icon_names.py` 从它生成整个类型：
新增一个图标是"改两行 + 跑一次生成器"，不再是过去那种要改六个文件的苦役。

宿主可以在运行期注册**自己**的图标——内置集合是封闭词汇表，`register_icon` 是它旁边那个开放扩展点：

```rust
use rust_widgets::widget::register_icon;

// 960 单位设计网格上的 SVG 路径数据，y 轴向上为负（Material Symbols 的约定）。
assert!(register_icon("disclosure", &["M480-200 240-440l480 480-240-240Z"]));
icon.set_icon("disclosure");
```

`register_icon_on_grid` 接受网格参数，用于其它尺度的来源（如 Lucide、Tabler 的 24 单位）。图标数据为
Apache-2.0（Google LLC）；署名与许可门控见 [`NOTICE`](NOTICE) 与 `tools/check_icon_licences.sh`。

该特性刻意**不**属于任何设备 profile：`mini` 与 `embedded` 是**按尺寸裁剪**的，调用者没要求的载荷放在那里就是错的。
需要就显式申请：`--features mini,icons`。

## 验证一次改动

```bash
cargo fmt --all -- --check
cargo check --all-targets --no-default-features --features desktop
cargo clippy --all-targets --no-default-features --features desktop -- -D warnings
cargo test  --no-default-features --features desktop -q
cargo test  --no-default-features --features "desktop,icons" -q   # 可选开启的图标数据

for profile in full embedded mini tablet mobile; do                # 矩阵的其余部分
  cargo test --no-default-features --features "$profile" -q
done
for profile in desktop tablet mobile mini embedded; do             # 只构建，覆盖每个 profile
  cargo check --no-default-features --features "$profile"
done

bash tools/run_all_gates.sh   # 111 个门控：源码、契约与平台检查，输出 PASS/FAIL 表
```

不要改用 `--all-features` 来代替：`desktop` 与 `mini` 互斥，且 `mini` 会把 crate 切到 `no_std`。门控脚本会把
受宿主限制的检查报告为跳过；原生 GTK、Apple 与 Android 测试在各自的平台 CI 任务里跑。

[`snapshots/svg/`](snapshots/svg/) 下每个控件每种外观一个 SVG。它们是**提交进仓库并重新生成**的，
`tools/check_svg_snapshots.sh` 会在控件的绘制变了而快照没更新时逐字节失败——于是"画歪了"会以可审阅的 diff 出现，
而两个文件完全相同的控件则一眼可见地"不随主题变化"。[`control.md`](control.md) 是同一批快照按模块分族的阅读版，
由注册表生成，新增控件无需任何人手动编辑画廊。

快照并不总是合适的证据：它展示的是**静止**状态，因此 hover、禁用、动画路径上的缺陷在快照里看不见。那些由像素探针
钉住（`tests/disabled_text_contrast.rs`、`tests/disabled_surface_probe.rs`、`tests/m3_animation_probe.rs`），
断言的是承诺的那个量——对比度、绘制高度、分段数——而不是字节长度之类的替代指标。

每个门控都必须**有能力失败**；关键的那些用反向注入验证过：把缺陷故意放回去，确认门控变红。不能失败的门控只是装饰，
本仓库不收集装饰品。

## 「181 个控件」覆盖什么

文本与输入（按钮、开关、输入框、掩码/OTP/日期/时间编辑器、搜索、标签输入、键盘、富文本、markdown、代码编辑器、
终端）；选择与展示（列表、表格、带冻结列的虚拟表格、树、chip、徽标、评分、进度、骨架屏）；容器与外壳（标签页、
分割器、停靠面板、MDI、工具栏、菜单、状态栏、Ribbon、工具箱）；对话框与浮层（消息框、文件/字体/取色器、向导、
popover、tooltip、banner、toast、snackbar、底部弹出层）；导航；媒体；以及 Material 没有对应物的图表族——
K 线、成交量、深度、指标、雷达、热力图、仪表盘、sparkline。

`mini` 与 `embedded` 只保留裁剪后的核心。每个 profile 的确切集合由生成器产出并受门控，列在
[`docs/plans/platform_capability_matrix.md`](docs/plans/platform_capability_matrix.md)。

## 平台说明

- **Windows · macOS · Linux** —— 完整 profile，真实表面与事件循环。
- **Linux/Wayland** —— 合成相关测试在 `tools/run_wayland_compositor_tests.sh`。
- **iOS · Android** —— 完整 profile；JNI 测试 APK 用 `tools/build_android_testapp.sh` 构建。
- **Web (wasm32)** —— WebGL/WebGPU 表面。`cfg(target_arch = "wasm32")` 描述的是**沙箱**而不是操作系统，
  所有 OS 相关知识都留在 `src/platform/` 里。
- **Embedded · mini** —— 仅软件光栅器，无 OS 服务。

确实还做不到的事情会如实说：`supports_custom_widgets()`、`supports_web_engine()` 与 `has_real_engine()` 是运行期
的回答，不是假装成特性的编译期桩。

## 语言绑定

`src/bindings/` 里的 C ABI 用 **152 个 `rw_*` 函数**把每个控件暴露出去，配一套基于能力（capability）的属性与事件
模型。C、C++、Python、Java（JNI）与 Node 绑定都在 CI 里跑过；生成的头文件由 `tools/check_abi.sh` 检查漂移。
从 [`cookbook/zh-CN/src/chapters/language-bindings.md`](cookbook/zh-CN/src/chapters/language-bindings.md) 开始。

## 文档

| 文档 | 内容 |
|---|---|
| [`cookbook/`](cookbook/) | 手册——English、简体中文、繁體中文 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | 模块划分与分层规则 |
| [`docs/MIGRATION_GUIDE.md`](docs/MIGRATION_GUIDE.md) | 从 1.x 迁移（原生控件在 2.0 已移除） |
| [`CHANGELOG.md`](CHANGELOG.md) | 发布说明，每个修复都附证据 |
| [`docs/plans/`](docs/plans/) | 设计计划与背后的工程原则 |
| [`docs/log/`](docs/log/) | 逐轮工程日志 |
| [`docs/reports/`](docs/reports/) | 审计与质量报告 |

## 环境要求

Rust **1.88+**。默认构建不需要任何系统 GUI 库；在 Linux 上表面使用 Wayland/X11。图像编解码是纯 Rust，
因此交叉编译到 Android、iOS 或 wasm 不需要 `pkg-config` sysroot。

## 许可证

MIT —— 见 [LICENSE](LICENSE)。

## 支持

问题反馈：[GitHub Issues](https://github.com/mikewolfli/rust-widgets/issues)

[![build](https://img.shields.io/badge/build-passing-brightgreen)]()
[![version](https://img.shields.io/badge/version-2.8.9-blue)]()
[![tests](https://img.shields.io/badge/tests-7600%2B-brightgreen)]()
[![license](https://img.shields.io/badge/license-MIT-blue)]()
[![controls](https://img.shields.io/badge/controls-181-blue)]()

<p align="center">
  <a href="README.md">
    <img src="https://img.shields.io/badge/English-blue" alt="English">
  </a>
</p>
