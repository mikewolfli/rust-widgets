# 高级控件

本章介绍 rust_widgets 库中的高级控件，包括树形视图、表格视图和列表视图。

## 树形视图 (TreeView)

树形视图用于显示层次结构数据。

```rust
use rust_widgets::{create_tree_view, tree_view_set_model};
use rust_widgets::widget::VecTreeModel;

let tree_view = create_tree_view(parent, x, y, width, height);

// 创建树模型
let mut model = VecTreeModel::new();
model.add_item("根节点", None);
model.add_item("子节点 1", Some("根节点"));
model.add_item("子节点 2", Some("根节点"));

// 设置模型
tree_view_set_model(tree_view, model);
```

## 表格视图 (TableView)

表格视图用于显示表格数据。

```rust
use rust_widgets::{create_table_view, table_view_add_column, table_view_add_row};

let table_view = create_table_view(parent, x, y, width, height);

// 添加列
table_view_add_column(table_view, "姓名", 150);
table_view_add_column(table_view, "年龄", 80);

// 添加行
table_view_add_row(table_view, vec!["张三", "30"]);
table_view_add_row(table_view, vec!["李四", "25"]);
```

## 列表视图 (ListView)

列表视图用于显示列表数据。

```rust
use rust_widgets::{create_list_view, list_view_set_model};
use rust_widgets::widget::VecListModel;

let list_view = create_list_view(parent, x, y, width, height);

// 创建列表模型
let model = VecListModel::new(vec!["项目 1", "项目 2", "项目 3"]);

// 设置模型
list_view_set_model(list_view, model);
```