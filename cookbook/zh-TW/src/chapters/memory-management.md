# 記憶體管理

rust-widgets **不提供**手寫配置器層。配置遵循 Rust 的所有權模型（原則 #30）：控制項擁有自己的狀態，
快取是帶明確淘汰規則的有界集合，公開 API 中沒有任何 `alloc`/`free` 式的池化入口。

> **2026-11-11 變更（issue-20261111 R-24）。** 早期版本帶有 `rust_widgets::memory` 模組
> （`ArenaAllocator`、`StackAllocator`、`BufferPool`、`StringPool`、`VecPool`、`ObjectPool`、
> `SharedPool`、`PoolManager`、`MemoryMonitor`、`MemoryStats`）。它在函式庫、demo、綁定與測試中
> **零消費者**——模組文件自己寫著 "Reserved public API"——因此被刪除，而不是保留為第二套配置詞彙。
> 曾引用它的程式碼請改用標準函式庫（`Vec`、`Box`、`Arc`）或下文列出的有界快取。
> 刪除正是「不要建構無人使用的東西」的誠實形態。

## 真正管理記憶體的設施

| 關注點 | 位置 | 上界 |
|---|---|---|
| 解碼影像重用 | `src/image/cache.rs` | 可設定位元組預算，真 LRU |
| 控制項光柵層 | `src/widget/runtime/cache.rs` | `MAX_CACHED_WIDGET_LAYERS`（128）+ LRU-clock |
| 文字整形 / 度量 / 覆蓋 | `src/render/text/` | 有界表，依字型代次失效 |
| CSS 解析 | `src/style/` | 四級有界快取（G-161） |
| GPU 上傳 | `src/wgpu_backend/renderer/resources.rs` | LRU 紋理與覆蓋快取 |
| GPU 暫存緩衝 | `src/gpu/` | 只增 arena，跨幀重用 |

以上每一項都是**有明確上界與淘汰規則**的快取，而不是通用物件池。快取允許隨時丟棄項目，
因此函式庫從不發放「池化物件」，也從不要求宿主歸還。

## 撰寫低配置控制項

- 重用每幀暫存緩衝（`RenderContext` 自帶），只在幾何真正變化時配置。
- 像素酬載優先使用 `Arc<[u8]>`：記錄影像與圖層路徑可以共享同一份配置，而不是拷貝。
- 每控制項狀態放在控制項自身，而不是全域表；執行期註冊表已經擁有控制項，第二張表就是第二個事實來源
  （原則 #101）。
- 嵌入式 profile 透過 `alloc_frugal`/`mini` 門控**整體編譯掉**依賴配置器的子系統，而不是模擬它們。

## 嵌入式 profile

`mini`/`embedded` 建置會編譯掉控制項註冊表、主題以及所有需要配置器的快取。那裡的控制項由宿主直接驅動，
沒有池需要管理，也沒有背景保留。profile 門控見 `docs/plans/principle.md` #47。

## 小結

- 沒有手寫記憶體池 API；使用所有權 + 有界快取。
- 原 `memory` 模組已按證據刪除（R-24）。
- 每個快取的上界與淘汰規則都寫在實作它的程式碼旁邊。
