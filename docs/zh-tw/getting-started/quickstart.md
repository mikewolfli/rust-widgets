# 快速開始指南

本指南將協助您在幾分鐘內開始使用 rust_widgets。

## 先決條件

在開始之前，請確保您已安裝：

- **Rust**（1.70 或更高版本）- [安裝 Rust](https://rustup.rs/)
- 適用於您平台的 C 編譯器（通常隨 Rust 一起提供）

## 安裝

將 rust_widgets 新增至您的 `Cargo.toml`：

```toml
[dependencies]
rust_widgets = "0.5"
```

或使用 cargo add：

```bash
cargo add rust_widgets
```

## 您的第一個應用程式

建立一個新的 Rust 專案：

```bash
cargo new my_first_app
cd my_first_app
```

編輯 `src/main.rs`：

```rust
use rust_widgets::{
    create_window, create_label, create_button, show_widget, run, init,
    connect_clicked, set_widget_text
};

fn main() {
    // 初始化框架
    init();
    
    // 建立主視窗
    let window = create_window("My First App", 100, 100, 400, 300);
    
    // 建立標籤
    let label = create_label(window, "Hello, rust_widgets!", 20, 20, 200, 30);
    
    // 建立按鈕
    let button = create_button(window, "Click Me!", 20, 60, 100, 30);
    
    // 連線按鈕點選事件
    connect_clicked(button, move || {
        set_widget_text(label, "Button clicked!");
    });
    
    // 顯示視窗並啟動事件迴圈
    show_widget(window);
    run();
}
```

執行您的應用程式：

```bash
cargo run
```

## 後續步驟

- 了解[基礎控制項](../widgets/basic.md)
- 探索[事件處理](../concepts/events.md)
- 檢視[示範](../demos/basic.md)
- 閱讀[架構概述](../concepts/architecture.md)

## 疑難排解

### 建置錯誤

如果您遇到建置錯誤：

1. 確保您的 Rust 版本是最新的：`rustup update`
2. 檢查您是否安裝了所需的系統程式庫
3. 請參閱[安裝](installation.md)中的平台特定說明

### 執行階段問題

如果應用程式無法啟動：

1. 檢查您的顯示環境是否已正確設定
2. 在 Linux 上，確保您已安裝 GTK 開發程式庫
3. 在 Windows 上，確保您已安裝 Windows SDK

## 取得說明

- 瀏覽[常見問題](../appendix/faq.md)
- 檢視 [GitHub Issues](https://github.com/your-org/rust-widgets/issues)
- 加入我們的社群討論
