# 伴生控件綁定

**伴生控件**（companion）是編輯另一個控件狀態、但不擁有文件的條帶、選單或列。
`ColumnChooser`、`HeaderMenu`、`GridStatusBar`、`SummaryRow`、`TreeFindBar`、
`Breadcrumb` 與 `FindReplaceBar` 都是伴生控件。

所有配對的綁定形制相同；新增伴生控件時照此模板實作。

## 三步形制

1. **伴生控件暴露 source trait 或訊號。** `TreeFindBar` 消費 `TreeSearchSource`；
   `FindReplaceBar` 透過訊號報告意圖；`HeaderMenu` 透過訊號報告排序、過濾與隱藏請求。
   伴生控件從不持有文件。
2. **宿主增加一個 `attach_*` 方法。** 用 `ConnectionScope` 訂閱，在附加時把伴生控件的
   token 解析到自己的欄/列，並把請求放入共享佇列。
3. **宿主每幀 poll 一次。** `poll_*` 排空佇列並套用到自己的狀態，因此伴生控件的一次點擊
   無需應用層手工轉發即可到達宿主。

## 宿主對應的附加方法

| 宿主 | 附加 | 輪詢 |
|---|---|---|
| `TreeView` | `attach_tree_find_bar`、`attach_path_bar` | `poll_tree_find_bar`、`poll_path_bar` |
| `TreeTable` | `attach_tree_find_bar`、`attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row`、`attach_path_bar` | `poll_tree_find_bar`、`poll_column_chooser`、`poll_header_menu`、`poll_grid_status_bar`、`poll_summary_row`、`poll_path_bar` |
| `GridTable` | `attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row` | `poll_column_chooser`、`poll_header_menu`、`poll_grid_status_bar`、`poll_summary_row` |
| `DataGrid` | `attach_column_chooser`、`attach_header_menu`、`attach_grid_status_bar`、`attach_summary_row`、`attach_find_bar` | 對應的 `poll_*` / `poll_find_bar` |
| `RichEdit` | `attach_find_replace_bar` | `poll_find_replace_bar` |

## 保持接線誠實的規則

- **附加時解析 token。** 宿主不認識的 token 直接忽略，絕不猜測。
- **每個伴生控件只保留一個訂閱。** 再次 `attach_*` 會替換舊綁定，不會留下兩條鏈路。
- **只支援宿主真正能兌現的能力。** `TreeTable` 與 `GridTable` 忽略 `HeaderMenu` 的過濾請求：
  它們沒有過濾模型，假裝套用會報告畫面並不存在的情境。
- **detach 保留最後狀態。** `detach_*` 只丟棄訂閱；伴生控件最後設定的排序、可見性或文字
  保留到宿主主動修改為止。

完整範例見 `TreeTable::attach_column_chooser` 與 `TreeTable::poll_column_chooser`；
header menu 鏈路使用同樣的佇列形制。
