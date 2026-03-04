# MDI 区域 (MDI Area)

MDI（多文档界面）区域允许用户在单个父窗口中管理多个文档窗口。

## 创建 MDI 区域

```rust
use rust_widgets::create_mdi_area;

let mdi_area = create_mdi_area(parent, x, y, width, height);
```

## 添加子窗口

```rust
use rust_widgets::mdi_area_add_subwindow;

// 添加子窗口到 MDI 区域
let subwindow = mdi_area_add_subwindow(mdi_area, "文档 1", 100, 100, 400, 300);
```

## 激活子窗口

```rust
use rust_widgets::mdi_area_activate_subwindow;

// 激活子窗口
mdi_area_activate_subwindow(mdi_area, subwindow);
```

## 关闭子窗口

```rust
use rust_widgets::mdi_area_close_subwindow;

// 关闭子窗口
mdi_area_close_subwindow(mdi_area, subwindow);
```

## 获取活动子窗口

```rust
use rust_widgets::mdi_area_active_subwindow;

if let Some(active_window) = mdi_area_active_subwindow(mdi_area) {
    println!("活动子窗口: {}", active_window);
}
```

## 层叠子窗口

```rust
use rust_widgets::mdi_area_cascade_subwindows;

// 以层叠模式排列子窗口
mdi_area_cascade_subwindows(mdi_area);
```

## 平铺子窗口

```rust
use rust_widgets::mdi_area_tile_subwindows;

// 以平铺模式排列子窗口
mdi_area_tile_subwindows(mdi_area);
```

## 示例

```rust
use rust_widgets::{create_window, create_mdi_area, mdi_area_add_subwindow, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("MDI 区域示例", 100, 100, 800, 600);
    
    // 创建 MDI 区域
    let mdi_area = create_mdi_area(window, 10, 10, 780, 580);
    
    // 添加子窗口
    let subwindow1 = mdi_area_add_subwindow(mdi_area, "文档 1", 100, 100, 400, 300);
    let subwindow2 = mdi_area_add_subwindow(mdi_area, "文档 2", 150, 150, 400, 300);
    let subwindow3 = mdi_area_add_subwindow(mdi_area, "文档 3", 200, 200, 400, 300);
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```