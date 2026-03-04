# 面板 (Panel)

面板是最基本的容器控件，用于组织和布局其他控件。

## 创建面板

```rust
use rust_widgets::create_panel;

let panel = create_panel(parent, x, y, width, height);
```

## 启用/禁用面板

```rust
use rust_widgets::set_widget_enabled;

// 启用面板
set_widget_enabled(panel, true);

// 禁用面板
set_widget_enabled(panel, false);
```

## 检查面板是否启用

```rust
use rust_widgets::is_widget_enabled;

let enabled = is_widget_enabled(panel);
println!("面板是否启用: {}", enabled);
```

## 显示/隐藏面板

```rust
use rust_widgets::{show_widget, hide_widget};

// 显示面板
show_widget(panel);

// 隐藏面板
hide_widget(panel);
```

## 设置面板几何属性

```rust
use rust_widgets::set_widget_geometry;

// 设置面板位置和大小
set_widget_geometry(panel, new_x, new_y, new_width, new_height);
```

## 示例

```rust
use rust_widgets::{create_window, create_panel, create_button, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("面板示例", 100, 100, 400, 300);
    
    // 创建面板
    let panel = create_panel(window, 50, 50, 300, 200);
    
    // 在面板上添加按钮
    let button = create_button(panel, "点击我", 100, 80, 100, 30);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```