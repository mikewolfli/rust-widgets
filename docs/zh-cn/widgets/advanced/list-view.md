# 列表视图 (List View)

列表视图用于显示列表数据，如文件列表、选项列表等。

## 创建列表视图

```rust
use rust_widgets::create_list_view;

let list_view = create_list_view(parent, x, y, width, height);
```

## 设置列表模型

```rust
use rust_widgets::widget::VecListModel;
use rust_widgets::list_view_set_model;

// 创建列表模型
let model = VecListModel::new(vec!["项目 1", "项目 2", "项目 3", "项目 4", "项目 5"]);

// 设置模型到列表视图
list_view_set_model(list_view, model);
```

## 获取选中项目

```rust
use rust_widgets::list_view_get_selected_item;

if let Some(selected_item) = list_view_get_selected_item(list_view) {
    println!("选中的项目索引: {}", selected_item);
}
```

## 事件处理

要处理列表视图选择变更事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::SelectionChanged && event.widget_id == list_view {
        if let Some(selected_item) = list_view_get_selected_item(list_view) {
            println!("选中的项目索引: {}", selected_item);
        }
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_list_view, list_view_set_model, show_widget, run, init};
use rust_widgets::widget::VecListModel;

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("列表视图示例", 100, 100, 800, 600);
    
    // 创建列表视图
    let list_view = create_list_view(window, 10, 10, 780, 580);
    
    // 创建列表模型
    let model = VecListModel::new(vec![
        "项目 1",
        "项目 2",
        "项目 3",
        "项目 4",
        "项目 5",
        "项目 6",
        "项目 7",
        "项目 8",
        "项目 9",
        "项目 10"
    ]);
    
    // 设置模型到列表视图
    list_view_set_model(list_view, model);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```