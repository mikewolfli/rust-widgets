# Rust Widgets 文档

欢迎使用 **rust_widgets** 文档！这是使用 rust_widgets 框架构建跨平台 GUI 应用程序的综合指南。

## 什么是 rust_widgets？

**rust_widgets** 是一个强大的 Rust 跨平台 GUI 框架，使您能够构建原生桌面和移动应用程序。它提供了：

- **跨平台**：一次编写，在 Windows、macOS、Linux、鸿蒙系统和嵌入式系统上运行
- **原生性能**：直接平台集成，无虚拟化开销
- **丰富的控件集**：全面的基础和高级 UI 组件集合
- **多语言支持**：内置国际化（i18n）支持运行时语言切换
- **多种语言绑定**：支持从 Rust、C、Python、Java 等语言使用
- **GPU 加速**：可选的 GPU 渲染后端，实现高性能图形

## 快速链接

- [快速开始](getting-started/quickstart.md) - 开始构建您的第一个应用程序
- [控件参考](widgets/basic.md) - 浏览可用的 UI 组件
- [API 文档](api/rust.md) - 探索 API 参考
- [演示](demos/basic.md) - 查看示例应用程序

## 示例

这是一个简单的 "Hello World" 应用程序：

```rust
use rust_widgets::{create_window, create_label, show_widget, run, init};

fn main() {
    init();
    
    let window = create_window("Hello World", 100, 100, 400, 300);
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    show_widget(window);
    run();
}
```

## 功能特性

### 控件类型

- **基础控件**：按钮、标签、行编辑、文本编辑、复选框、单选按钮、组合框、滑块、进度条、数字框
- **容器控件**：面板、分割器、标签页、停靠面板、滚动区域、分组框
- **高级控件**：树形视图、表格视图、列表视图、多文档界面、图表、画布
- **对话框控件**：消息框、文件对话框、输入对话框、自定义对话框

### 平台支持

| 平台 | 状态 | 说明 |
|------|------|------|
| Windows | ✅ 支持 | Win32 API，支持 DPI 感知 |
| macOS | ✅ 支持 | Cocoa/AppKit 集成 |
| Linux | ✅ 支持 | GTK3/GTK4 后端 |
| 鸿蒙系统 | ✅ 支持 | 原生 ArkUI 集成 |
| 嵌入式 | ✅ 支持 | 自定义渲染引擎 |

### 语言绑定

- **Rust**：原生 API（主要）
- **C**：完整的 ABI 兼容性
- **Python**：具有类型提示的 Pythonic 包装器
- **Java**：基于 JNI 的集成
- **C++**：仅头文件的包装器

## 架构

rust_widgets 遵循分层架构：

```
┌─────────────────────────────────────┐
│        应用层                       │
│    （您的 GUI 应用程序）             │
├─────────────────────────────────────┤
│        控件层                       │
│    （按钮、标签等）                  │
├─────────────────────────────────────┤
│        平台抽象层                   │
│    （跨平台接口）                    │
├─────────────────────────────────────┤
│        原生后端                     │
│  （Win32/Cocoa/GTK/ArkUI）          │
└─────────────────────────────────────┘
```

## 获取帮助

- **问题**：在 [GitHub Issues](https://github.com/your-org/rust-widgets/issues) 上报告错误
- **讨论**：加入社区讨论
- **文档**：浏览本综合指南

## 许可证

rust_widgets 使用 MIT 许可证。详情请参见 [LICENSE](../LICENSE)。

---

**准备开始？** 前往 [快速开始指南](getting-started/quickstart.md)！
