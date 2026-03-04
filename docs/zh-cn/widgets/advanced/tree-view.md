# 树形视图 (Tree View)

树形视图用于显示层次结构数据，如文件系统、组织架构等。

## 创建树形视图

```rust
use rust_widgets::create_tree_view;

let tree_view = create_tree_view(parent, x, y, width, height);
```

## 设置树模型

```rust
use rust_widgets::widget::VecTreeModel;
use rust_widgets::tree_view_set_model;

// 创建树模型
let mut model = VecTreeModel::new();

// 添加项目到模型
model.add_item("根节点", None);
model.add_item("子节点 1", Some("根节点"));
model.add_item("子节点 2", Some("根节点"));
model.add_item("孙节点", Some("根节点/子节点 1"));

// 设置模型到树形视图
tree_view_set_model(tree_view, model);
```

## 获取选中项目

```rust
use rust_widgets::tree_view_get_selected_item;

if let Some(selected_item) = tree_view_get_selected_item(tree_view) {
    println!("选中的项目: {}", selected_item);
}
```

## 展开项目

```rust
use rust_widgets::tree_view_expand_item;

// 展开项目
tree_view_expand_item(tree_view, "根节点");
```

## 折叠项目

```rust
use rust_widgets::tree_view_collapse_item;

// 折叠项目
tree_view_collapse_item(tree_view, "根节点");
```

## 事件处理

要处理树形视图选择变更事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::SelectionChanged && event.widget_id == tree_view {
        if let Some(selected_item) = tree_view_get_selected_item(tree_view) {
            println!("选中的项目: {}", selected_item);
        }
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_tree_view, tree_view_set_model, show_widget, run, init};
use rust_widgets::widget::VecTreeModel;

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("树形视图示例", 100, 100, 800, 600);
    
    // 创建树形视图
    let tree_view = create_tree_view(window, 10, 10, 780, 580);
    
    // 创建树模型
    let mut model = VecTreeModel::new();
    
    // 添加项目到模型
    model.add_item("根节点", None);
    model.add_item("子节点 1", Some("根节点"));
    model.add_item("子节点 2", Some("根节点"));
    model.add_item("孙节点 1", Some("根节点/子节点 1"));
    model.add_item("孙节点 2", Some("根节点/子节点 1"));
    model.add_item("孙节点 3", Some("根节点/子节点 2"));
    
    // 设置模型到树形视图
    tree_view_set_model(tree_view, model);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```