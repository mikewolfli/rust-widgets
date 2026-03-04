# 停靠面板 (Dock Panel)

停靠面板允许用户将子控件停靠到容器的不同边缘。

## 创建停靠面板

```rust
use rust_widgets::create_dock_panel;

let dock_panel = create_dock_panel(parent, x, y, width, height);
```

## 停靠控件

```rust
use rust_widgets::{dock_panel_dock_widget, DockPosition};

// 停靠控件到左侧
dock_panel_dock_widget(dock_panel, widget, DockPosition::Left, "左侧面板");

// 停靠控件到右侧
dock_panel_dock_widget(dock_panel, widget2, DockPosition::Right, "右侧面板");

// 停靠控件到顶部
dock_panel_dock_widget(dock_panel, widget3, DockPosition::Top, "顶部面板");

// 停靠控件到底部
dock_panel_dock_widget(dock_panel, widget4, DockPosition::Bottom, "底部面板");

// 设置控件为中央控件
dock_panel_dock_widget(dock_panel, central_widget, DockPosition::Center, "中央面板");
```

## 取消停靠控件

```rust
use rust_widgets::dock_panel_undock_widget;

// 取消停靠控件
dock_panel_undock_widget(dock_panel, widget);
```

## 隐藏停靠控件

```rust
use rust_widgets::dock_panel_hide_widget;

// 隐藏停靠控件
dock_panel_hide_widget(dock_panel, widget);
```

## 显示停靠控件

```rust
use rust_widgets::dock_panel_show_widget;

// 显示停靠控件
dock_panel_show_widget(dock_panel, widget);
```

## 示例

```rust
use rust_widgets::{create_window, create_dock_panel, create_panel, dock_panel_dock_widget, DockPosition, show_widget, run, init};

fn main() {
    // 初始化库
    init();
    
    // 创建窗口
    let window = create_window("停靠面板示例", 100, 100, 800, 600);
    
    // 创建停靠面板
    let dock_panel = create_dock_panel(window, 10, 10, 780, 580);
    
    // 创建面板用于停靠
    let left_panel = create_panel(dock_panel, 0, 0, 200, 580);
    let right_panel = create_panel(dock_panel, 600, 0, 200, 580);
    let top_panel = create_panel(dock_panel, 200, 0, 400, 100);
    let bottom_panel = create_panel(dock_panel, 200, 500, 400, 100);
    let central_panel = create_panel(dock_panel, 200, 100, 400, 400);
    
    // 停靠面板
    dock_panel_dock_widget(dock_panel, left_panel, DockPosition::Left, "左侧面板");
    dock_panel_dock_widget(dock_panel, right_panel, DockPosition::Right, "右侧面板");
    dock_panel_dock_widget(dock_panel, top_panel, DockPosition::Top, "顶部面板");
    dock_panel_dock_widget(dock_panel, bottom_panel, DockPosition::Bottom, "底部面板");
    dock_panel_dock_widget(dock_panel, central_panel, DockPosition::Center, "中央面板");
    
    // 显示窗口
    show_widget(window);
    
    // 运行事件循环
    run();
}
```