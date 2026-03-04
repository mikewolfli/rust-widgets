# 选项卡控件 (Tab Widget)

选项卡控件允许用户通过标签切换不同的面板。

## 创建选项卡控件

```rust
use rust_widgets::create_tab_widget;

let tab_widget = create_tab_widget(parent, x, y, width, height);
```

## 添加选项卡

```rust
use rust_widgets::tab_widget_add_tab;

// 添加选项卡
tab_widget_add_tab(tab_widget, "标签 1");
tab_widget_add_tab(tab_widget, "标签 2");
```

## 选择选项卡

```rust
use rust_widgets::tab_widget_set_current_index;

// 选择第二个选项卡（索引 1）
tab_widget_set_current_index(tab_widget, 1);
```

## 获取当前选项卡索引

```rust
use rust_widgets::tab_widget_current_index;

if let Some(index) = tab_widget_current_index(tab_widget) {
    println!("当前选项卡索引: {}", index);
}
```

## 移除选项卡

```rust
use rust_widgets::tab_widget_remove_tab;

// 移除第一个选项卡（索引 0）
tab_widget_remove_tab(tab_widget, 0);
```

## 获取选项卡数量

```rust
use rust_widgets::tab_widget_tab_count;

let count = tab_widget_tab_count(tab_widget);
println!("选项卡数量: {}", count);
```

## 获取选项卡文本

```rust
use rust_widgets::tab_widget_tab_text;

if let Some(text) = tab_widget_tab_text(tab_widget, 0) {
    println!("选项卡文本: {}", text);
}
```

## 事件处理

要处理选项卡选择变更事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::SelectionChanged && event.widget_id == tab_widget {
        if let Some(index) = tab_widget_current_index(tab_widget) {
            println!("选项卡已切换到索引: {}", index);
        }
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_tab_widget, tab_widget_add_tab, tab_widget_set_current_index, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("选项卡示例", 100, 100, 800, 600);
    
    // 创建选项卡控件
    let tab_widget = create_tab_widget(window, 10, 10, 780, 580);
    
    // 添加选项卡
    tab_widget_add_tab(tab_widget, "标签 1");
    tab_widget_add_tab(tab_widget, "标签 2");
    tab_widget_add_tab(tab_widget, "标签 3");
    
    // 选择第二个选项卡
    tab_widget_set_current_index(tab_widget, 1);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```