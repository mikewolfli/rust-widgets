# 标签 (Label)

标签用于显示文本信息，通常用于描述其他控件或提供说明。

## 创建标签

```rust
use rust_widgets::create_label;

let label = create_label(parent, "这是一个标签", x, y, width, height);
```

## 设置标签文本

```rust
use rust_widgets::set_widget_text;

set_widget_text(label, "新的标签文本");
```

## 获取标签文本

```rust
use rust_widgets::get_widget_text;

let text = get_widget_text(label);
println!("标签文本: {}", text);
```

## 启用/禁用标签

```rust
use rust_widgets::set_widget_enabled;

// 启用标签
set_widget_enabled(label, true);

// 禁用标签
set_widget_enabled(label, false);
```

## 检查标签是否启用

```rust
use rust_widgets::is_widget_enabled;

let enabled = is_widget_enabled(label);
println!("标签是否启用: {}", enabled);
```

## 显示/隐藏标签

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示标签
show_widget(label);

// 隐藏标签
hide_widget(label);
```

## 设置标签几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置标签位置和大小
set_widget_geometry(label, new_x, new_y, new_width, new_height);
```

## 示例

```rust
use rust_widgets::{create_window, create_label, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("标签示例", 100, 100, 400, 300);
    
    // 创建标签
    let label = create_label(window, "Hello, rust_widgets!", 150, 120, 200, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```