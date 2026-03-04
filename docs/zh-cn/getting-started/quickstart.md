# 快速开始指南

本指南将帮助您在几分钟内开始使用 rust_widgets。

## 先决条件

在开始之前，请确保您已安装：

- **Rust**（1.70 或更高版本）- [安装 Rust](https://rustup.rs/)
- 适用于您平台的 C 编译器（通常随 Rust 一起提供）

## 安装

将 rust_widgets 添加到您的 `Cargo.toml`：

```toml
[dependencies]
rust_widgets = "0.5"
```

或使用 cargo add：

```bash
cargo add rust_widgets
```

## 您的第一个应用程序

创建一个新的 Rust 项目：

```bash
cargo new my_first_app
cd my_first_app
```

编辑 `src/main.rs`：

```rust
use rust_widgets::{
    create_window, create_label, create_button, show_widget, run, init,
    connect_clicked, set_widget_text
};

fn main() {
    // 初始化框架
    init();
    
    // 创建主窗口
    let window = create_window("My First App", 100, 100, 400, 300);
    
    // 创建标签
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    // 创建按钮
    let button = create_button(window, "Click Me!", 20, 60, 100, 30);
    
    // 连接按钮点击事件
    connect_clicked(button, move || {
        set_widget_text(label, "Button clicked!");
    });
    
    // 显示窗口并启动事件循环
    show_widget(window);
    run();
}
```

运行您的应用程序：

```bash
cargo run
```

## 下一步

- 了解[基础控件](../widgets/basic.md)
- 探索[事件处理](../concepts/events.md)
- 查看[演示](../demos/basic.md)
- 阅读[架构概述](../concepts/architecture.md)

## 故障排除

### 构建错误

如果您遇到构建错误：

1. 确保您的 Rust 版本是最新的：`rustup update`
2. 检查您是否安装了所需的系统库
3. 请参阅[安装](installation.md)中的平台特定说明

### 运行时问题

如果应用程序无法启动：

1. 检查您的显示环境是否已正确配置
2. 在 Linux 上，确保您已安装 GTK 开发库
3. 在 Windows 上，确保您已安装 Windows SDK

## 获取帮助

- 浏览[常见问题](../appendix/faq.md)
- 查看 [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- 加入我们的社区讨论
