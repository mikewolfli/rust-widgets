# 簡介

**rust-widgets** 是一套純 Rust 開發、跨平台的原生 GUI 函式庫，專為建構可在各種環境中執行的應用程式而設計——從桌面工作站到嵌入式微控制器，從行動裝置到網頁瀏覽器，皆可執行。

## 什麼是 rust-widgets？

rust-widgets 讓您只需一套 Rust 程式碼庫，即可在各大平台上產出原生風格的介面。它包含了豐富的控制項庫、硬體自適應渲染，以及深度的平台整合——所有功能皆透過簡潔、地道風格的 Rust API 提供。

```rust
use rust_widgets::prelude::*;

fn main() {
    let mut app = Application::new();
    let window = Window::builder()
        .title("Hello, rust-widgets!")
        .size(800, 600)
        .build();
    let button = Button::builder()
        .label("Click Me")
        .on_click(|_| println!("Hello, world!"))
        .build();
    window.set_content(button);
    app.run(window);
}
```

## 主要特色

### 豐富的控制項函式庫 — 超過 140 種控制項

內建超過 140 種控制項，涵蓋各種常見的 UI 需求：

- **核心控制項**：Button、CheckBox、RadioButton、Label、LineEdit、TextEdit、
  ComboBox、SpinBox、Slider、ScrollBar、ProgressBar
- **容器**：Window、Dialog、TabWidget、Splitter、GroupBox、StackedWidget、
  DockWidget、MdiArea、ToolBox、CollapsiblePane
- **清單與檢視**：ListView、TreeView、Table、Grid、Canvas
- **日期與時間**：Calendar、DatePicker、TimePicker、DateTimePicker、
  DateRangePicker
- **選單**：MenuBar、ContextMenu、PieMenu、RibbonBar、DropdownMenu、Popover
- **行動裝置優先**：BottomNavigationBar、NavigationDrawer、AppBar、SafeArea、
  PullToRefresh、Cupertino 風格控制項
- **輸入**：MaskedEdit、AutoCompleteEdit、SearchBox、CommandPalette、
  KeySequenceEdit
- **顯示**：LCDNumber、Dial、ProgressCircle、Rating、Sparkline、Badge、Chip、
  Avatar、SkeletonLoader
- **特殊用途**：QRCode、VideoPlayer、CameraPreview、BarcodeScanner、MapView、
  TerminalView、MediaPlayer、CodeEditor、DiffViewer

### 硬體自適應渲染

提供三種渲染後端，會根據目標平台自動選擇：

| 後端 | 目標平台 | 說明 |
|---------|--------|-------------|
| **GPU (wgpu)** | 桌面、平板、行動裝置 | 透過 wgpu 實現硬體加速渲染 |
| **SoftwarePaintBackend** | 嵌入式、精簡裝置 | 以 CPU 光柵化輸出至 RGBA 幀緩衝區 |
| **SvgPaintBackend** | 測試、文件 | 輸出 SVG 管線，用於像素級精確驗證 |

### 八大平台，統一 API

| 平台 | 後端 | 功能標記 |
|----------|---------|:------------:|
| Linux (Wayland) | `linux-wayland` | `linux-wayland` |
| Windows (Win32) | `windows` | `windows` |
| macOS (Cocoa/objc2) | `macos` | `macos` |
| iOS (UIKit) | `ios` | `ios` |
| Android (JNI) | `android` | `android` |
| Web (WASM) | `wasm` | `wasm` |
| HarmonyOS | `harmony` | `harmony` |
| Embedded (no_std) | `embedded` / `mini` | `embedded` / `mini` |

### 觸控與手勢

內建十一種手勢辨識器——Tap、DoubleTap、LongPress、Swipe、Pan、Fling、
TwoFingerTap、TwoFingerSwipe、LongPressDrag、Pinch 與 Rotate——並支援自動觸控目標區域擴展，確保在小螢幕上的無障礙操作性。

### 國際化

`tr!()` 巨集提供編譯期的金鑰式翻譯支援，涵蓋英文、簡體中文與繁體中文，並支援上下文及複數形式的變體。翻譯覆蓋率審查工具（`audit_keys()`）可在建置時即時發現遺漏的翻譯。

### 圖表與資料視覺化

內建圖表控制項——LineChart、BarChart、PieChart 與 Sparkline——直接透過相同的渲染管線繪製，無需任何外部圖表依賴套件。

### PDF 與列印

可透過統一的 API 產生 PDF 文件，並將工作送至系統列印服務。SVG 管線精確輸出，確保螢幕所見與列印結果一致。

### 無障礙功能

`a11y` 功能可整合各平台的無障礙 API（Linux 上透過 zbus 使用 AT-SPI），將控制項樹結構暴露給螢幕報讀軟體及其他輔助技術。

### Web 引擎

完整的 WebView 整合，包含設定管理、Cookie 儲存、下載處理、WebChannel 通訊以及右鍵選單自訂功能。

## 設計理念

- **公開 API 中零 `unsafe` 程式碼。** 所有 `unsafe` 區塊僅限於平台 FFI 邊界，並經過詳盡的驗證與防恐慌安全處理。
- **嵌入式環境的 `no_std` 支援。** 同一套程式碼庫透過條件編譯同時服務 std 與 `no_std` 目標平台。`compat.rs` 橋接層將 std 型別（`HashMap`、`Mutex`）映射為基於 arena 分配及無堆疊的替代方案（`BTreeMap`、`RefCell`、`MiniVec`、`MiniString`）。
- **模組化功能系統。** 三個獨立的軸向——裝置設定檔、作業系統後端與功能能力——讓您能組合出完全符合需求的二進位檔。僅在需要使用時才引入圖表、列印或 i18n 等功能。
- **無處不在的 Builder 模式。** 透過 Rust 型別系統實現編譯期驗證。每個控制項、樣式與佈局都使用符合人體工學的 builder API。

## 本使用手冊涵蓋的內容

| 章節 | 主題 |
|---------|--------|
| **快速入門** | 環境設定、第一個應用程式、專案範本 |
| **架構概述** | 分層模型、功能系統、Crate 結構 |
| **核心型別** | `Widget`、`Style`、`Color`、`Rect`、`Size`、信號 |
| **控制項系統** | 控制項生命週期、組合、自訂控制項 |
| **佈局系統** | Box、Grid、Stack、Flow、Absolute、Masonry 佈局 |
| **事件系統** | 事件迴圈、輸入處理、手勢辨識 |
| **渲染系統** | GPU/CPU/SVG 後端、髒區域、部分重新整理 |
| **樣式與主題** | CSS 引擎、主題、熱載入、`StyleSheetManager` |
| **平台支援** | 各平台設定、條件編譯、後端 |
| **語言繫結** | C ABI、Python、Java/JNI、C++ 整合 |
| **國際化** | `tr!()` 巨集、翻譯檔案、複數規則 |
| **圖表與資料視覺化** | LineChart、BarChart、PieChart、Sparkline |
| **PDF 與列印** | 文件產生、系統列印服務 |
| **效能與品質** | 基準測試、SVG 回歸測試、效能剖析 |
| **記憶體管理** | Arena 分配、`no_std` 記憶體模型、記憶體洩漏偵測 |
| **嵌入式支援** | `no_std` 設定檔、軟體光柵化、資源限制 |
| **Web 引擎** | WebView 設定、設定管理、通道、安全性 |
| **進階主題** | 自訂後端、unsafe FFI、非同步整合 |
| **API 參考** | 模組層級文件、Trait 參考、型別索引 |

## 先決條件

- **Rust 1.87** 或更新版本（MSRV）
- **平台依賴套件**：
  | 平台 | 依賴套件 |
  |----------|-------------|
  | Linux (GTK) | `libgtk-3-dev` |
  | Linux (Wayland) | `libwayland-dev`、`wayland-protocols` |
  | macOS / iOS | Xcode Command Line Tools |
  | Windows | Visual Studio Build Tools (MSVC) |
  | Android | Android NDK、`cargo-ndk` |
  | WASM | `wasm-bindgen-cli`、`wasm-pack` |

## 專案狀態

| | |
|---|---|
| **版本** | 0.9.6 |
| **授權條款** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **儲存庫** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **測試數量** | 3400+ |
| **MSRV** | Rust 1.87 |

準備好開始了嗎？請前往[快速入門](chapters/getting-started.md)。
