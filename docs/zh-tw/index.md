# Rust Widgets 文件

歡迎使用 **rust_widgets** 文件！這是使用 rust_widgets 框架構建跨平台 GUI 應用程式的綜合指南。

## 什麼是 rust_widgets？

**rust_widgets** 是一個強大的 Rust 跨平台 GUI 框架，使您能夠構建原生桌面和行動應用程式。它提供了：

- **跨平台**：一次編寫，在 Windows、macOS、Linux、鴻蒙系統和嵌入式系統上執行
- **原生效能**：直接平台整合，無虛擬化開銷
- **豐富的控制項集**：全面的基礎和進階 UI 元件集合
- **多語言支援**：內建國際化（i18n）支援執行時語言切換
- **多種語言繫結**：支援從 Rust、C、Python、Java 等語言使用
- **GPU 加速**：可選的 GPU 彩現後端，實現高效能圖形

## 快速連結

- [快速開始](getting-started/quickstart.md) - 開始構建您的第一個應用程式
- [控制項參考](widgets/basic.md) - 瀏覽可用的 UI 元件
- [API 文件](api/rust.md) - 探索 API 參考
- [示範](demos/basic.md) - 檢視範例應用程式

## 範例

這是一個簡單的 "Hello World" 應用程式：

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

### 控制項類型

- **基礎控制項**：按鈕、標籤、行編輯、文字編輯、核取方塊、選項按鈕、下拉式方塊、滑桿、進度列、數字方塊
- **容器控制項**：面板、分割器、索引標籤、停駐面板、捲動區域、群組方塊
- **進階控制項**：樹狀檢視、表格檢視、清單檢視、多文件介面、圖表、畫布
- **對話方塊控制項**：訊息方塊、檔案對話方塊、輸入對話方塊、自訂對話方塊

### 平台支援

| 平台 | 狀態 | 說明 |
|------|------|------|
| Windows | ✅ 支援 | Win32 API，支援 DPI 感知 |
| macOS | ✅ 支援 | Cocoa/AppKit 整合 |
| Linux | ✅ 支援 | GTK3/GTK4 後端 |
| 鴻蒙系統 | ✅ 支援 | 原生 ArkUI 整合 |
| 嵌入式 | ✅ 支援 | 自訂彩現引擎 |

### 語言繫結

- **Rust**：原生 API（主要）
- **C**：完整的 ABI 相容性
- **Python**：具有型別提示的 Pythonic 包裝器
- **Java**：基於 JNI 的整合
- **C++**：僅標頭檔的包裝器

## 架構

rust_widgets 遵循分層架構：

```
┌─────────────────────────────────────┐
│        應用層                       │
│    （您的 GUI 應用程式）             │
├─────────────────────────────────────┤
│        控制項層                       │
│    （按鈕、標籤等）                  │
├─────────────────────────────────────┤
│        平台抽象層                     │
│    （跨平台介面）                    │
├─────────────────────────────────────┤
│        原生後端                       │
│  （Win32/Cocoa/GTK/ArkUI）          │
└─────────────────────────────────────┘
```

## 取得說明

- **問題**：在 [GitHub Issues](https://github.com/your-org/rust-widgets/issues) 上報告錯誤
- **討論**：加入社群討論
- **文件**：瀏覽本綜合指南

## 授權

rust_widgets 使用 MIT 授權。詳情請參見 [LICENSE](../LICENSE)。

---

**準備開始？** 前往 [快速開始指南](getting-started/quickstart.md)！
