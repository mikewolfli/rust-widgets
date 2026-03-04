# 文本输入 (Text Input)

文本输入控件用于接收用户输入的文本，包括单行文本输入（LineEdit）和多行文本输入（TextEdit）。

## 创建单行文本输入

```rust
use rust_widgets::create_line_edit;

let line_edit = create_line_edit(parent, "默认文本", x, y, width, height);
```

## 设置文本

```rust
use rust_widgets::set_widget_text;

set_widget_text(line_edit, "新的文本");
```

## 获取文本

```rust
use rust_widgets::get_widget_text;

let text = get_widget_text(line_edit);
println!("输入的文本: {}", text);
```

## 启用/禁用文本输入

```rust
use rust_widgets::set_widget_enabled;

// 启用文本输入
set_widget_enabled(line_edit, true);

// 禁用文本输入
set_widget_enabled(line_edit, false);
```

## 显示/隐藏文本输入

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示文本输入
show_widget(line_edit);

// 隐藏文本输入
hide_widget(line_edit);
```

## 设置文本输入几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置文本输入位置和大小
set_widget_geometry(line_edit, new_x, new_y, new_width, new_height);
```

## 事件处理

要处理文本输入事件，您可以轮询小部件触发事件：

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::ValueChanged && event.widget_id == line_edit {
        let text = get_widget_text(line_edit);
        println!("文本已更改: {}", text);
    }
}
```

## 示例

```rust
use rust_widgets::{create_window, create_line_edit, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind, get_widget_text};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("文本输入示例", 100, 100, 400, 300);
    
    // 创建文本输入
    let line_edit = create_line_edit(window, "请输入文本", 100, 120, 200, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    loop {
        // 检查文本更改事件
        if let Some(event) = poll_widget_trigger_event() {
            if event.kind == WidgetTriggerKind::ValueChanged && event.widget_id == line_edit {
                let text = get_widget_text(line_edit);
                println!("文本已更改: {}", text);
            }
        }
        
        // 运行一次事件循环
        run();
    }
}
```