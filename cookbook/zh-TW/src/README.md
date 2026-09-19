# 簡介

**rust-widgets** 是一套純 Rust 開發、跨平台的原生 GUI 函式庫，專為建構可在各種環境中執行的應用程式而設計——從桌面工作站到嵌入式微控制器，從行動裝置到網頁瀏覽器，皆可執行。

## 所有控件均為自繪

**rust-widgets 會自行繪製 100% 的控件，在任何平台上都不會建立作業系統的原生控件。**
這個 crate 中沒有 `CreateWindowExW`、沒有 `NSButton`、沒有 `gtk_button_new`，
也沒有 `android.widget.Button`。後端提供的是**繪製面**、**事件迴圈**、
**輸入轉譯**與**平台服務**（IME、剪貼簿、無障礙、原生選單、檔案對話框、DPI）
—— 除此之外別無其他。

在繼續閱讀之前，有兩個結果值得先內化：

1. **控件在每個作業系統上看起來完全相同。** 你的按鈕在 Windows、macOS、Linux、iOS、Android 與網頁上擁有相同的像素，因為它們都是由同一套 Rust 光柵化器所繪製。
2. **Widget 可用性屬於*設定檔*問題，而非作業系統問題。** 在 `desktop`/`tablet`/`mobile` 設定檔中，全部 179 種 widget 在每個平台上都可使用。只有資源受限的 `embedded`/`mini` 設定檔會編譯精簡的集合。

這就是為什麼平台章節記載的是[每個作業系統*提供*什麼](chapters/platform-support.md#13-平台服務確實會因作業系統而異)（DPI、IME、無障礙、原生選單），而不是列出哪些控件在哪些地方能用 —— 因為那張清單到處都一樣。

## 什麼是 rust-widgets？

rust-widgets 讓您只需一套 Rust 程式碼庫，即可在各大平台上產出一致的介面。它包含了豐富的控制項庫、硬體自適應渲染，以及深度的平台整合——所有功能皆透過簡潔、地道風格的 Rust API 提供。

> 下方程式碼片段僅用於說明預期的 API 形式。若需要能對 2.4.3 編譯的程式碼，請從 [`chapters/getting-started.md`](chapters/getting-started.md) 開始，
> 該檔案已針對目前的 crate 驗證過。

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

### 豐富的控制項函式庫 — 179 種控件

內建 179 種控件，涵蓋各種常見的 UI 需求，而且**全部都是自繪的，因此每個平台都能使用**：

- **核心控制項**：Button、CheckBox、RadioButton、Label、LineEdit、TextEdit、
  ComboBox、SpinBox、Slider、ScrollBar、ProgressBar
- **容器**：Window、Dialog、Frame、TabWidget、Splitter、GroupBox、StackedWidget、
  DockWidget、MdiArea、ToolBox、CollapsiblePane
- **清單與檢視**：ListView、TreeView、TreeTable、Table、Grid、Canvas
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
  TerminalView、MediaPlayer、CodeEditor、DiffViewer、SignaturePad、DropZone、
  Breadcrumb

### 硬體自適應渲染

提供三種渲染後端，會根據目標平台自動選擇：

| 後端 | 目標平台 | 說明 |
|---------|--------|-------------|
| **GPU (wgpu)** | 桌面、平板、行動裝置 | 透過 wgpu 實現硬體加速渲染 |
| **SoftwarePaintBackend** | 嵌入式、精簡裝置 | 以 CPU 光柵化輸出至 RGBA 幀緩衝區 |
| **SvgPaintBackend** | 測試、文件 | 輸出 SVG 管線，用於像素級精確驗證 |

### 九大平台，統一 API

下表列出的是**各平台如何提供繪製表面與事件迴圈**——而非哪些控件可用。
因為所有控件皆為自繪，下列每個平台都支援全部 179 種控件；只有
`embedded`/`mini` 設定檔會減少編譯進來的控件數量。

| 平台 | 後端提供 | 功能標記 |
|----------|------------------|:------------:|
| Windows (Win32) | Win32 視窗 + 訊息迴圈 | `windows` |
| macOS (Cocoa/objc2) | `NSView` 表面 | `macos` |
| Linux (GTK) | GTK3 視窗 + 事件迴圈 | `linux-gtk` |
| Linux (Wayland) | `wl_surface` + 輸入 | `linux-wayland` |
| iOS (UIKit) | UIKit 表面 | `ios` |
| Android (JNI) | JNI 表面 | `android` |
| HarmonyOS | NAPI 橋接 | `harmony` |
| Web (WASM) | DOM canvas + 瀏覽器事件 | `wasm` |
| Portable / 無頭 | 記憶體幀緩衝，無作業系統 | `embedded` / `mini` |

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
- **嵌入式環境的 no_std 就緒架構。** 同一套程式碼庫透過條件編譯服務多種目標；`compat.rs` 橋接層統一從 `core`/`alloc` 導入共享型別（`MiniVec`、`MiniString` 等）。目前 `mini` profile 在 std 上編譯，真正的 `#![no_std]` 開啟列為後續步驟。
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
| **記憶體管理** | Arena 分配、no_std 就緒記憶體模型、記憶體洩漏偵測 |
| **嵌入式支援** | `mini`/`embedded` 設定檔、軟體光柵化、資源限制 |
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
| **版本** | 2.4.3 |
| **授權條款** | [MIT](https://github.com/mikewolfli/rust-widgets/blob/main/LICENSE) |
| **儲存庫** | [github.com/mikewolfli/rust-widgets](https://github.com/mikewolfli/rust-widgets) |
| **測試數量** | 5200+ |
| **MSRV** | Rust 1.87 |

準備好開始了嗎？請前往[快速入門](chapters/getting-started.md)。
