# 高級控件

本章介紹 rust_widgets 庫中的高級控件，包括樹形視圖、表格視圖和列表視圖。

## 樹形視圖 (TreeView)

樹形視圖用於顯示層次結構數據。

```rust
use rust_widgets::{create_tree_view, tree_view_set_model};
use rust_widgets::widget::VecTreeModel;

let tree_view = create_tree_view(parent, x, y, width, height);

// 建立樹模型
let mut model = VecTreeModel::new();
model.add_item("根節點", None);
model.add_item("子節點 1", Some("根節點"));
model.add_item("子節點 2", Some("根節點"));

// 設定模型
tree_view_set_model(tree_view, model);
```

## 表格視圖 (TableView)

表格視圖用於顯示表格數據。

```rust
use rust_widgets::{create_table_view, table_view_add_column, table_view_add_row};

let table_view = create_table_view(parent, x, y, width, height);

// 新增列
table_view_add_column(table_view, "姓名", 150);
table_view_add_column(table_view, "年齡", 80);

// 新增行
table_view_add_row(table_view, vec!["張三", "30"]);
table_view_add_row(table_view, vec!["李四", "25"]);
```

## 列表視圖 (ListView)

列表視圖用於顯示列表數據。

```rust
use rust_widgets::{create_list_view, list_view_set_model};
use rust_widgets::widget::VecListModel;

let list_view = create_list_view(parent, x, y, width, height);

// 建立列表模型
let model = VecListModel::new(vec!["項目 1", "項目 2", "項目 3"]);

// 設定模型
list_view_set_model(list_view, model);
```