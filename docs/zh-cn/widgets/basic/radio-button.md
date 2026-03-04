# 单选按钮 (Radio Button)

单选按钮用于从多个选项中选择一个，通常成组使用，同一组中的单选按钮只能有一个被选中。

## 创建单选按钮

```rust
use rust_widgets::create_radio_button;

let radio_button = create_radio_button(parent, "选项 1", x, y, width, height);
```

## 设置单选按钮文本

```rust
use rust_widgets::set_widget_text;

set_widget_text(radio_button, "新的选项文本");
```

## 获取单选按钮文本

```rust
use rust_widgets::get_widget_text;

let text = get_widget_text(radio_button);
println!("单选按钮文本: {}", text);
```

## 启用/禁用单选按钮

```rust
use rust_widgets::set_widget_enabled;

// 启用单选按钮
set_widget_enabled(radio_button, true);

// 禁用单选按钮
set_widget_enabled(radio_button, false);
```

## 显示/隐藏单选按钮

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示单选按钮
show_widget(radio_button);

// 隐藏单选按钮
hide_widget(radio_button);
```

## 设置单选按钮几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置单选按钮位置和大小
set_widget_geometry(radio_button, new_x, new_y, new_width, new_height);
```

## 事件处理

要处理单选按钮点击事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::Clicked && event.widget_id == radio_button {
        println!("单选按钮被点击了！");
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_radio_button, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("单选按钮示例", 100, 100, 400, 300);
    
    // 创建单选按钮组
    let radio1 = create_radio_button(window, "选项 1", 150, 100, 100, 30);
    let radio2 = create_radio_button(window, "选项 2", 150, 140, 100, 30);
    let radio3 = create_radio_button(window, "选项 3", 150, 180, 100, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    loop {
        // 检查单选按钮点击事件
        if let Some(event) = poll_widget_trigger_event() {
            if event.kind == WidgetTriggerKind::Clicked {
                if event.widget_id == radio1 {
                    println!("选项 1 被选中！");
                } else if event.widget_id == radio2 {
                    println!("选项 2 被选中！");
                } else if event.widget_id == radio3 {
                    println!("选项 3 被选中！");
                }
            }
        }
        
        // 运行一次事件循环
        run();
    }
}
```