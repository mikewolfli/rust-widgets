# 复选框 (Checkbox)

复选框用于表示二进制状态（选中或未选中），通常用于允许用户选择一个或多个选项。

## 创建复选框

```rust
use rust_widgets::create_checkbox;

let checkbox = create_checkbox(parent, "选项", x, y, width, height);
```

## 设置复选框文本

```rust
use rust_widgets::set_widget_text;

set_widget_text(checkbox, "新的选项文本");
```

## 获取复选框文本

```rust
use rust_widgets::get_widget_text;

let text = get_widget_text(checkbox);
println!("复选框文本: {}", text);
```

## 启用/禁用复选框

```rust
use rust_widgets::set_widget_enabled;

// 启用复选框
set_widget_enabled(checkbox, true);

// 禁用复选框
set_widget_enabled(checkbox, false);
```

## 显示/隐藏复选框

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示复选框
show_widget(checkbox);

// 隐藏复选框
hide_widget(checkbox);
```

## 设置复选框几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置复选框位置和大小
set_widget_geometry(checkbox, new_x, new_y, new_width, new_height);
```

## 事件处理

要处理复选框点击事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::Clicked && event.widget_id == checkbox {
        println!("复选框被点击了！");
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_checkbox, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("复选框示例", 100, 100, 400, 300);
    
    // 创建复选框
    let checkbox = create_checkbox(window, "同意条款", 150, 120, 150, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    loop {
        // 检查复选框点击事件
        if let Some(event) = poll_widget_trigger_event() {
            if event.kind == WidgetTriggerKind::Clicked && event.widget_id == checkbox {
                println!("复选框被点击了！");
            }
        }
        
        // 运行一次事件循环
        run();
    }
}
```