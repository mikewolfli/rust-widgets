# rust_widgets 使用帮助（简体中文）

## 关联文档

- 架构文档：[ARCHITECTURE.md](ARCHITECTURE.md)
- 示例总览：[../demos/README.md](../demos/README.md)
- 英文帮助：[HELP.en.md](HELP.en.md)
- 繁体帮助：[HELP.zh-TW.md](HELP.zh-TW.md)
- 法文帮助：[HELP.fr.md](HELP.fr.md)
- 俄文帮助：[HELP.ru.md](HELP.ru.md)
- C ABI 快速开始：[C_ABI_QUICKSTART.md](C_ABI_QUICKSTART.md)
- 鸿蒙原生桥接：[HARMONY_NATIVE_BRIDGE.zh-CN.md](HARMONY_NATIVE_BRIDGE.zh-CN.md)

## 功能概览

- 纯 Rust 跨平台 GUI 架构。
- 支持桌面平台：Windows、macOS、Linux、鸿蒙桌面。
- 支持嵌入式精简模式：核心窗口、基础控件、基础布局。
- 预留手机端统一接口（Android / iOS / 鸿蒙手机）。
- 内置事件队列、信号槽、主题、布局、XML、国际化、打印、PDF、图表模块。

## 版本说明

- 完整版：`default` + `full` feature。
- 精简版：启用 `embedded`，关闭非核心模块 feature。
- 手机端预留：`mobile-api` feature，用于统一移动端扩展点。

## 常用命令

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

## 功能开关示例

```bash
# 完整版（默认）
cargo check

# 嵌入式精简版
cargo check --no-default-features --features embedded

# 完整版 + 手机端接口预留
cargo check --features "full,mobile-api"

# 嵌入式 + 手机端接口预留
cargo check --no-default-features --features "embedded,mobile-api"
```

## 示例列表

- 完整分类清单请查看 `demos/README.md`。
- 主入口与架构示例：`demo_main`、`demo_layout`、`demo_xml`、`demo_i18n`。
- 原生触发轮询示例：`demo_native_events`（菜单触发 + 控件类型化触发）。
- 控件示例已覆盖：窗口/对话框/弹窗、基础输入类、数据展示类、容器类、菜单工具状态类，以及表格/网格/图表/画布控件。

## 绑定接口

C ABI 在 `src/bindings/mod.rs`，已预留 Python/C++/Java 标准扩展入口。
同时提供原生触发轮询接口：`rust_widgets_poll_menu_triggered`、`rust_widgets_poll_widget_triggered`。
如需类型化控件触发，请使用 `rust_widgets_poll_widget_trigger_event(widget_id_out)`，返回值类型码为：`0` 无、`1` 点击、`2` 值变更。
鸿蒙 ArkUI/NAPI 直连请使用 `rust_widgets_harmony_on_*` 与 `rust_widgets_harmony_on_node_*` 系列接口。
如需 `node_handle ↔ widget_id` 映射与回调接入流程，请参考 `docs/HARMONY_NATIVE_BRIDGE.zh-CN.md` 与 `examples/harmony_napi_bridge_sample.c`。
完整 C ABI 构建/运行命令请参考 `docs/C_ABI_QUICKSTART.md`。

快速构建/运行（在项目根目录）：

```bash
# 构建动态库
cargo build

# 在 macOS 上编译 C 示例
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# 在 macOS 上运行
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Linux 运行时加载路径示例：

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```
