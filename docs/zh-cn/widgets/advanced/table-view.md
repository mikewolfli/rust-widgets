# 表格视图 (Table View)

表格视图用于显示表格数据，如电子表格、数据库记录等。

## 创建表格视图

```rust
use rust_widgets::create_table_view;

let table_view = create_table_view(parent, x, y, width, height);
```

## 添加列

```rust
use rust_widgets::table_view_add_column;

// 添加列到表格视图
table_view_add_column(table_view, "姓名", 150);
table_view_add_column(table_view, "年龄", 80);
table_view_add_column(table_view, "邮箱", 200);
```

## 添加行

```rust
use rust_widgets::table_view_add_row;

// 添加行到表格视图
let row = vec!["张三", "30", "zhangsan@example.com"];
table_view_add_row(table_view, row);
```

## 获取选中行

```rust
use rust_widgets::table_view_get_selected_row;

if let Some(selected_row) = table_view_get_selected_row(table_view) {
    println!("选中的行索引: {}", selected_row);
}
```

## 获取单元格值

```rust
use rust_widgets::table_view_get_cell_value;

if let Some(value) = table_view_get_cell_value(table_view, row_index, column_index) {
    println!("单元格值: {}", value);
}
```

## 设置单元格值

```rust
use rust_widgets::table_view_set_cell_value;

// 设置单元格值
table_view_set_cell_value(table_view, row_index, column_index, "新值");
```

## 事件处理

要处理表格视图选择变更事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::SelectionChanged && event.widget_id == table_view {
        if let Some(selected_row) = table_view_get_selected_row(table_view) {
            println!("选中的行索引: {}", selected_row);
        }
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_table_view, table_view_add_column, table_view_add_row, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("表格视图示例", 100, 100, 800, 600);
    
    // 创建表格视图
    let table_view = create_table_view(window, 10, 10, 780, 580);
    
    // 添加列
    table_view_add_column(table_view, "姓名", 150);
    table_view_add_column(table_view, "年龄", 80);
    table_view_add_column(table_view, "邮箱", 200);
    
    // 添加行
    table_view_add_row(table_view, vec!["张三", "30", "zhangsan@example.com"]);
    table_view_add_row(table_view, vec!["李四", "25", "lisi@example.com"]);
    table_view_add_row(table_view, vec!["王五", "35", "wangwu@example.com"]);
    table_view_add_row(table_view, vec!["赵六", "28", "zhaoliu@example.com"]);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```