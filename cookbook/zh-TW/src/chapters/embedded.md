# 嵌入式 / 資源受限支援

> **本章已重寫。** 先前它記錄的是一套 `rust_widgets::embedded` 模組，包含
> `EmbeddedConfig`、`ResourceManager`、`WidgetPool`、`DpiScaler`、
> `LightweightStyle`、`InputFilter`、`TouchPoint`、`init_embedded`、
> `init_desktop`。**crate 中不存在該模組** —— `grep -n "pub mod" src/lib.rs | grep -i embed`
> 沒有任何輸出，而舊章節裡每一處 `use rust_widgets::embedded::…` 都無法解析，
> 約 660 行範例**無法編譯**。下文描述的是真實存在的介面。

rust-widgets 透過兩個 **device profile** 支援資源受限目標，兩者在建置期選擇：

| Profile | Feature | 改變什麼 |
|---|---|---|
| `mini` | `--features mini` | 分配節儉檔。`mini` 會讓 crate 切換到 `no_std`；控制項集被裁剪，且不連結平台執行期。 |
| `embedded` | `--features embedded` | 嵌入式繪製面，控制項集被裁剪。 |

這兩個 profile 與 `desktop`、`tablet`、`mobile` 是**互斥**的，因此一次建置只指定一個：

```bash
cargo check --no-default-features --features mini
cargo check --no-default-features --features embedded
```

> **不要**用 `--all-features` 驗證：它會同時開啟 `desktop` 與 `mini`，而 `mini` 是
> `no_std`，該組合**無法編譯**，不構成有效檢查。

## 裁剪檔究竟去掉了什麼

門控名在 `build.rs` 中統一定義，模組按**意圖**引用，而不是各自重推一遍條件：

| 別名 | 條件 | 含義 |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | 是否裝置建置？ |
| `full_widgets` | `device_profile && !(mini \| embedded)` | 完整控制項集 + 裝置傳輸層 |
| `widgets_unstripped` | `!(mini \| embedded)` | 控制項集未被裁剪 |
| `alloc_frugal` | `mini` | 適用分配預算 |
| `embedded_surface` | `embedded` | 嵌入式繪製面 |

最關鍵的區別是**行為**上的，而非結構上的：裁剪檔會**如實**報告它無法承載繪製面，
而不是先掛載再交回一個空白視窗。

```rust
// `supports_surfaces()` 是現行名稱（`supports_custom_widgets` 已是帶棄用警告的別名）。
// 在 `mini`/`embedded` 上它回傳 `false`，宿主據此可以拒絕掛載，而不是顯示空白面。
// 這是一個**執行期事實** —— 呼叫方程式碼裡沒有 `cfg(target_os)`，也沒有 profile 分支。
if !rust_widgets::supports_surfaces() {
    eprintln!("該設定無法承載繪製面");
}
```

## 查詢目前建置支援什麼

不要按 profile 分支，而是直接詢問你需要的能力：

```rust
use rust_widgets::platform::{backend_name, capabilities};

// 編譯進的是哪個後端 —— 一個執行期答案。
println!("backend: {}", backend_name());

// 後端報告的能力集。
let caps = capabilities();
println!("ime: {}, accessibility: {}, dpi_scaling: {}",
         caps.ime, caps.accessibility, caps.dpi_scaling);
```

這是整個函式庫遵循的模式：平台與 profile 差異都是透過 platform trait 查詢的**執行期事實**，
從不在呼叫方程式碼裡做編譯期分支（見 `docs/ARCHITECTURE.md`）。

## 用嵌入式繪製面重繪

`embedded_surface` 提供宿主用於 blit 的繪製面。幾何資訊裡攜帶顯式的 `stride`，
它與 `width * 4` 分開保存 —— GPU backbuffer 往往按對齊填充，若按緊密排列做整行複製，
恰好會在這些宿主上產生影像錯切：

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// 宿主從自己的顯示驅動填入這些值。
let geometry = SurfaceGeometry::tight(320, 240);
let mut buffer = FrameBuffer::new();
assert!(buffer.resize(geometry));

// `frame_mut()` 只交出目前區域；超出目前幾何的內容永遠不會被曝露。
if let Some(frame) = buffer.frame_mut() {
    println!("待 blit 的位元組數：{}", frame.len());
}
```

## `alloc_frugal` 下的記憶體行為

`mini` 會設定 `alloc_frugal`，從而選中節儉分配路徑。它們的行為刻意為**透明**而非偽裝：

> 在 `alloc_frugal` 下，arena 記帳是 no-op 且回報 0 位元組，其文件明確寫道：
> 「不要用它作為記憶體用量度量。」一個看起來合理的值，正是本專案禁止的那類偽造報告。

如果你在受限目標上需要真實的分配資料，請從目標自身的 allocator 取，而不要取節儉檔的樁值。

## 小結

- 用 `--no-default-features --features mini` 或 `--features embedded` 建置。
- 掛載繪製面前先問 `supports_surfaces()`。
- 用 `capabilities()` 查詢能力，而不是按 profile 分支。
- 在 `alloc_frugal` 下應預期被如實標註的 no-op，而不是看似合理的數字。
