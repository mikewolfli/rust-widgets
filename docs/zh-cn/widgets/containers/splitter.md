# 分割器 (Splitter)

分割器是一个容器控件，允许用户通过拖动分隔线来调整子控件的大小。

## 创建分割器

```rust
use rust_widgets::create_splitter;

// 创建水平分割器
let splitter = create_splitter(parent, true, x, y, width, height);

// 创建垂直分割器
let vertical_splitter = create_splitter(parent, false, x, y, width, height);
```

## 添加子控件

```rust
use rust_widgets::splitter_add_child;

// 添加子控件到分割器
splitter_add_child(splitter, child_widget, 50); // 50% 的空间
```

## 设置分割器位置

```rust
use rust_widgets::splitter_set_position;

// 设置分割器位置为 300 像素
splitter_set_position(splitter, 300);
```

## 获取分割器位置

```rust
use rust_widgets::splitter_get_position;

let position = splitter_get_position(splitter);
println!("分割器位置: {}", position);
```

## 设置分割器方向

```rust
use rust_widgets::splitter_set_orientation;

// 设置为水平方向
splitter_set_orientation(splitter, true);

// 设置为垂直方向
splitter_set_orientation(splitter, false);
```

## 获取分割器方向

```rust
use rust_widgets::splitter_get_orientation;

let is_horizontal = splitter_get_orientation(splitter);
println!("分割器是否为水平方向: {}", is_horizontal);
```

## 示例

```rust
use rust_widgets::{create_window, create_splitter, create_panel, splitter_add_child, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("分割器示例", 100, 100, 800, 600);
    
    // 创建水平分割器
    let splitter = create_splitter(window, true, 10, 10, 780, 580);
    
    // 创建左右面板
    let left_panel = create_panel(splitter, 0, 0, 390, 580);
    let right_panel = create_panel(splitter, 390, 0, 390, 580);
    
    // 添加面板到分割器
    splitter_add_child(splitter, left_panel, 50);
    splitter_add_child(splitter, right_panel, 50);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```