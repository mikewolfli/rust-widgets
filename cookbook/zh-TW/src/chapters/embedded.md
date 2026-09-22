# 嵌入式 / 資源受限支援

本章是面向受限目標的**實作指南**：如何選擇 profile、執行期能拿到什麼、在沒有 OS 的情況下
如何驅動幀迴圈，以及如何把像素送到你自己的螢幕上。

下面每個範例都有一個**可執行**的對應檔案 ——
[`examples/embedded_host.rs`](../../examples/embedded_host.rs) —— 你可以直接跑，而不用只憑信任：

```bash
cargo run --no-default-features --features embedded --example embedded_host
cargo run --no-default-features --features mini     --example embedded_host
```

> **本章已重寫。** 先前它記錄的是一套 `rust_widgets::embedded` 模組
> （`EmbeddedConfig`、`ResourceManager`、`WidgetPool`、`DpiScaler`、
> `LightweightStyle`、`InputFilter`、`TouchPoint`、`init_embedded`、`init_desktop`），
> 該模組在 crate 中**不存在**，約 660 行範例無法編譯。本章所有內容都對照真實 API 核驗過。
> 另見 [`docs/MIGRATION_GUIDE.md`](../../docs/MIGRATION_GUIDE.md)。

---

## 1. 選擇 profile

有兩個面向受限硬體的 profile。它們與 `desktop`、`tablet`、`mobile` **互斥**，
因此一次建置只指定一個。

| Profile | Feature | 你得到什麼 | 代價 |
|---|---|---|---|
| `mini` | `--features mini` | 軟體光柵、約 15 個核心控制項、`heapless`/`bumpalo`/`spin` 分配器、完全不連結 OS 執行期。 | crate 變為 `no_std`；平台執行期不會被連結。 |
| `embedded` | `--features embedded` | 軟體光柵、無 GPU、無觸控、控制項集被裁剪。 | 無 GPU 後端、無觸控。 |

```toml
# 你自己二進位的 Cargo.toml
[dependencies]
rust_widgets = { version = "2.5.3", default-features = false, features = ["embedded"] }
```

適合裝置的 release profile —— 優先體積而非速度，並用 `panic = "abort"`，因為沒有 unwinder：

```toml
[profile.release-embedded]
inherits = "release"
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

> **不要用 `--all-features` 驗證。** 它會同時開啟 `desktop` 與 `mini`，而 `mini` 的 `no_std`
> 會移除 crate 大部分程式碼解析 `String`/`Vec` 所依賴的 `alloc` prelude，因此該組合無法編譯。
> 它不是更強的檢查，而是**根本不是檢查**。

## 2. 詢問目前建置是什麼

宿主第一件該做的事是**詢問**，而不是假設。每個 profile 事實都有執行期存取器
（`platform::profile`），因此同一份呼叫方程式碼在各自標上原樣執行：

```rust
use rust_widgets::platform::profile;

println!("profile: {}", profile::profile_name());          // "mini" / "embedded" / "desktop"
println!("os runtime: {}", profile::has_os_runtime());      // mini 上為 false
println!("full widgets: {}", profile::full_widget_set());   // mini/embedded 上為 false
println!("alloc frugal: {}", profile::is_alloc_frugal());   // mini 上為 true
println!("surface usable: {}", rust_widgets::supports_surfaces());
```

各 profile 的實測輸出：

| 查詢 | `mini` | `embedded` | `desktop` |
|---|---|---|---|
| `profile_name()` | `mini` | `embedded` | `desktop` |
| `has_os_runtime()` | `false` | `false` | `true` |
| `full_widget_set()` | `false` | `false` | `true` |
| `is_alloc_frugal()` | **`true`** | `false` | `false` |
| `supports_surfaces()` | `false` | `false` | `true` |

這些門控名在 `build.rs` 中統一定義，模組按**意圖**引用，而不是各自重推一遍合取式：

| 別名 | 條件 | 含義 |
|---|---|---|
| `device_profile` | `desktop \| tablet \| mobile` | 是否裝置建置？ |
| `full_widgets` | `device_profile && !(mini \| embedded)` | 完整控制項集 + 裝置傳輸層 |
| `widgets_unstripped` | `!(mini \| embedded)` | 控制項集未被裁剪 |
| `alloc_frugal` | `mini` | 適用分配預算 |
| `embedded_surface` | `embedded` | 嵌入式繪製面 |

### 什麼時候 `cfg` 仍然是對的

本函式庫的規則是：**呼叫方程式碼對「行為」用執行期查詢分支**。但有一種情況編譯期門控是正確的
——當差異是符號的**存在性**而非行為時。平台執行期
（`backend_name`、`capabilities`、`get_platform`、`quit`）不會被連結進 `mini`，因此：

```rust
use rust_widgets::platform::profile;

// 始終可用 —— 這些是對 profile 事實的 const fn。
println!("{}", profile::profile_name());

// 僅當連結了平台執行期才存在。
#[cfg(not(alloc_frugal))]
println!("backend: {}", rust_widgets::platform::backend_name());
```

這是**能力存在性**門控，不是行為分支。這個區分值得記清：`#[cfg]` 決定*程式碼是否參與編譯*，
查詢決定*程式碼做什麼*。

## 3. 沒有 OS 的幀迴圈

受限目標通常沒有視窗系統來給你幀回呼，因此本函式庫提供**按幀節流的任務佇列**：
你提交工作，它在下一次幀執行一次，並收到該幀的序號。

```rust
use rust_widgets::render_engine;

// 目標幀率被鉗到 1..=240，並回傳*實際生效*的值 ——
// 因此請求 999 的呼叫方會知道自己真正拿到了什麼。
let applied = render_engine::set_embedded_target_fps(60);
assert!((1..=240).contains(&applied));
println!("target fps: {}", render_engine::embedded_target_fps());

// 把工作排到下一幀。
let task_id = render_engine::submit_embedded_task("tick", |frame_index| {
    // 在 `frame_index` 這一幀執行一次。
    let _ = frame_index;
});
println!("queued task {task_id}");
```

### 節流機制，以及為什麼它不是細節

兩個 profile 的節流方式不同，而這個差異在電池上很重要：

- **非 `mini`**（帶執行緒宿主上的 `embedded`）：迴圈在 condvar 上停泊到下一幀截止時間，
  因此幀間 CPU 會睡眠。
- **`mini`**：沒有第二個執行緒能喚醒迴圈，於是它**在同一個單調時鐘上按同一份幀預算忙等**。
  沒有這份預算，迴圈會全速空轉、任務執行頻率遠超目標幀率要求 —— 這在裝置上是發熱與耗電問題，
  而不是外觀問題。

無論哪種方式，迴圈都會在等待內部重新檢查執行旗標，因此從任務中發出的 `quit()`
會被及時觀察到，而不是等到下一個截止時間。

## 4. 讀取執行期狀態

`embedded_engine_stats()` 是診斷入口 —— 看門狗或啟動自檢斷言的就是這些數字：

```rust
use rust_widgets::render_engine;

let stats = render_engine::embedded_engine_stats();
println!("initialised: {}", stats.initialized);
println!("running:     {}", stats.running);
println!("frames:      {}", stats.frame_count);
println!("queued:      {}", stats.pending_task_count);
println!("windows:     {}", stats.window_count);
println!("buttons:     {}", stats.button_count);
println!("target fps:  {}", stats.target_fps);
```

`EmbeddedEngineStats` 派生 `Clone + Debug + PartialEq + Eq`，因此可以在測試裡直接比較，
而不必逐欄位判斷。

引擎是**行程級單例**（一份繪製預算、一份註冊表），因此兩個會改動它的測試不能交錯執行。
crate 內部的測試為此共享一把守衛鎖串行化；宿主通常只讀。

## 5. 把像素送到你的螢幕上

這是與桌面建置真正不同的地方：沒有視窗、沒有合成器，**緩衝區歸你所有**。

### 分配繪製面

```rust
use rust_widgets::platform::{FrameBuffer, SurfaceGeometry};

// 注意顯式的 `stride` —— 為什麼它不等於 `width * 4`，見下。
let geometry = SurfaceGeometry { width: 320, height: 240, stride: 320 * 4 + 16 };
let mut frame = FrameBuffer::new();

if !frame.resize(geometry) {
    // stride 比一個緊密排列的列還窄時無法描述一幀，呼叫會拒絕，
    // 而不是產出一幀錯切的資料。
    return;
}
println!("{} bytes", frame.frame().len());
```

當你的硬體沒有填充時，用 `SurfaceGeometry::tight(w, h)` 構造緊密排列的幾何。

### 為什麼 `stride` 與 `width * 4` 分開

真實面板常按對齊填充，GPU backbuffer 往往也是。若程式碼假定第 `n` 列從 `n * width * 4` 開始，
就會**恰好**在這些宿主上產生影像錯切。把 `stride` 顯式化，意味著正確做法可直接表達，
而錯誤做法必須被刻意寫出來。

### 重排外部緩衝區

當驅動佈局不同（有填充，或原點自下而上）時，不應強迫每個控制項去學那套佈局。
`copy_rows` 把這一次轉換收斂在一處：

```rust
use rust_widgets::platform::portable::copy_rows;
use rust_widgets::platform::SurfaceGeometry;

let geometry = SurfaceGeometry { width: 2, height: 2, stride: 12 }; // 4 位元組填充
let src = [1u8; 24];
let mut dst = [0u8; 24];

assert!(copy_rows(&src, &mut dst, geometry));
// 填充位元組保持不動，而不會被寫入陳舊資料。
assert_eq!(&dst[8..12], &[0u8; 4]);
```

它在**兩個方向**都做邊界檢查，並且寧可拒絕也不裁剪：放不下的區域是錯誤，
因為靜默裁剪出來的幀是**會把自己藏起來的**渲染 bug。零尺寸繪製面被視為合法的空操作，
而不是失敗。

### 沒有 OS 在背後的宿主

對 `mini` —— 以及對沒有後端模組的目標 —— 可攜式宿主就是那個 `Platform` 實作：

```rust
use rust_widgets::platform::portable;

let host = portable::instance();     // 一個 StubPlatform：記憶體態、無 OS
assert_eq!(host.backend_name(), portable::BACKEND_NAME); // "portable"
assert_eq!(portable::FAMILY, rust_widgets::core::PlatformFamily::Embedded);
```

它刻意回報 `PlatformFamily::Embedded`：回報 `Desktop` 會讓自適應程式碼為一個
沒有視窗管理器的宿主挑選它無法兌現的預設值。

## 6. `alloc_frugal` 下的記憶體行為

`mini` 會設定 `alloc_frugal`，從而選中節儉分配路徑。它們的行為刻意**透明而非看似合理**：

- arena 記帳是 no-op，回報 `0` 位元組，其文件明確寫了這一點 ——
  *「不要用它作為記憶體用量度量。」*
- `Condvar` 樁無法停泊呼叫方，兩個 notify 方法也都是空操作。

偽造一個數字正是本專案禁止的那類報告，因此樁選擇如實作答。
**如果你在受限目標上需要真實分配資料，請從目標自身的 allocator 取**，而不要取這些樁值。

## 7. 裁剪檔究竟去掉了什麼

除了記憶體策略，裁剪檔還會失去：

- **宣告式視圖層。** `crate::view` / `crate::json` / `crate::app` 門控在
  `declarative_view`（`device_profile && !stripped && !no-declarative-view`）上，
  因此 `mini`/`embedded` 上你能用的是命令式 `add_child` API。理由是分配預算，
  以及這些目標上沒有呼叫方會逐幀重新求值一個視圖。
- **自繪控制項繪製面**，而且它會說出來：`supports_surfaces()` 回傳 `false`，
  而不是先掛載再交回一個空白視窗。
- **平台執行期**（見 §2）。

選單與快速鍵**刻意不受影響** —— 它們的程式碼不帶 `mini` 門控 —— 因此 `mini` 建置最準確的描述是
「沒有自繪控制項繪製面，但選單完全可用」。

## 8. 檢查清單

- [ ] 選定**一個** profile；用 `--no-default-features --features <profile>` 驗證。
- [ ] 也跑 `cargo check --no-default-features --features mini --all-targets` 看警告。
- [ ] 掛載任何東西之前先問 `profile::*` 與 `supports_surfaces()`。
- [ ] 設定目標幀率，並讀回**實際生效**的值。
- [ ] 提供繪製面，且 `stride` 必須準確。
- [ ] 填充緩衝區用 `copy_rows`，不要手寫整列複製。
- [ ] 在 `alloc_frugal` 下預期被如實標註的 no-op；真實記憶體數字從你自己的 allocator 取。
- [ ] 在啟動自檢裡對 `embedded_engine_stats()` 做斷言。
