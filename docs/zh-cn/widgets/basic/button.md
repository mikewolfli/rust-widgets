# 按钮 (Button)

按钮是最常用的控件之一，用于触发操作。

## 创建按钮

```rust
use rust_widgets::create_button;

let button = create_button(parent, "点击我", x, y, width, height);
```

## 设置按钮文本

```rust
use rust_widgets::set_widget_text;

set_widget_text(button, "新文本");
```

## 获取按钮文本

```rust
use rust_widgets::get_widget_text;

let text = get_widget_text(button);
println!("按钮文本: {}", text);
```

## 启用/禁用按钮

```rust
use rust_widgets::set_widget_enabled;

// 启用按钮
set_widget_enabled(button, true);

// 禁用按钮
set_widget_enabled(button, false);
```

## 检查按钮是否启用

```rust
use rust_widgets::is_widget_enabled;

let enabled = is_widget_enabled(button);
println!("按钮是否启用: {}", enabled);
```

## 显示/隐藏按钮

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示按钮
show_widget(button);

// 隐藏按钮
hide_widget(button);
```

## 设置按钮几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置按钮位置和大小
set_widget_geometry(button, new_x, new_y, new_width, new_height);
```

## 事件处理

要处理按钮点击事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::Clicked && event.widget_id == button {
        println!("按钮被点击了！");
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_button, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("按钮示例", 100, 100, 400, 300);
    
    // 创建按钮
    let button = create_button(window, "点击我", 150, 120, 100, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    loop {
        // 检查按钮点击事件
        if let Some(event) = poll_widget_trigger_event() {
            if event.kind == WidgetTriggerKind::Clicked && event.widget_id == button {
                println!("按钮被点击了！");
            }
        }
        
        // 运行一次事件循环
        run();
    }
}
```