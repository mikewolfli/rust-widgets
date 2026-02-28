# rust_widgets 使用說明（繁體中文）

## 關聯文件

- 架構文件：[ARCHITECTURE.md](ARCHITECTURE.md)
- 範例總覽：[../demos/README.md](../demos/README.md)
- 英文說明：[HELP.en.md](HELP.en.md)
- 簡體說明：[HELP.zh-CN.md](HELP.zh-CN.md)
- 法文說明：[HELP.fr.md](HELP.fr.md)
- 俄文說明：[HELP.ru.md](HELP.ru.md)
- C ABI 快速開始：[C_ABI_QUICKSTART.md](C_ABI_QUICKSTART.md)

## 功能摘要

- 純 Rust 跨平台 GUI 架構。
- 桌面平台：Windows、macOS、Linux、鴻蒙桌面。
- 嵌入式精簡版：核心視窗、基礎控制項、基礎版面配置。
- 手機端介面保留（Android / iOS / 鴻蒙手機）。
- 事件佇列、訊號槽、主題、版面配置、XML、i18n、列印、PDF、圖表。

## 版本配置

- 完整版：`default` + `full`。
- 精簡版：`embedded`。
- 手機端預留：`mobile-api`，提供統一行動端擴充點。

## 常用命令

```bash
cargo check
cargo check --examples
cargo run --example demo_main
```

## 功能開關示例

```bash
# 完整版（預設）
cargo check

# 嵌入式精簡版
cargo check --no-default-features --features embedded

# 完整版 + 手機端介面預留
cargo check --features "full,mobile-api"

# 嵌入式 + 手機端介面預留
cargo check --no-default-features --features "embedded,mobile-api"
```

## 範例

- 完整分類清單請參考 `demos/README.md`。
- 主入口與架構範例：`demo_main`、`demo_layout`、`demo_xml`、`demo_i18n`。
- 原生觸發輪詢範例：`demo_native_events`（選單觸發 + 控制項型別化觸發）。
- 控制項範例已覆蓋：視窗/對話框/彈出視窗、基礎輸入類、資料展示類、容器類、選單工具狀態類，以及表格/網格/圖表/畫布控制項。

## 綁定介面

C ABI 位置：`src/bindings/mod.rs`，已保留 Python/C++/Java 擴充入口。
同時提供原生觸發輪詢介面：`rust_widgets_poll_menu_triggered`、`rust_widgets_poll_widget_triggered`。
若需型別化控制項觸發，請使用 `rust_widgets_poll_widget_trigger_event(widget_id_out)`，回傳類型碼：`0` 無、`1` 點擊、`2` 值變更。
完整 C ABI 建置/執行命令請參考 `docs/C_ABI_QUICKSTART.md`。

快速建置/執行（在專案根目錄）：

```bash
# 建置動態函式庫
cargo build

# 在 macOS 上編譯 C 範例
clang -Iexamples examples/c_abi_poll_demo.c -Ltarget/debug -lrust_widgets -o target/debug/c_abi_poll_demo

# 在 macOS 上執行
DYLD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```

Linux 執行時載入路徑範例：

```bash
LD_LIBRARY_PATH=target/debug ./target/debug/c_abi_poll_demo
```
