# 容器控件

本章介绍 rust_widgets 库中的容器控件，包括面板、选项卡、分割器等。

## 面板 (Panel)

面板是最基本的容器控件，用于组织其他控件。

```rust
use rust_widgets::create_panel;

let panel = create_panel(parent, x, y, width, height);
```

## 选项卡控件 (TabWidget)

选项卡控件允许用户通过标签切换不同的面板。

```rust
use rust_widgets::{create_tab_widget, tab_widget_add_tab};

let tab_widget = create_tab_widget(parent, x, y, width, height);
tab_widget_add_tab(tab_widget, "标签 1");
tab_widget_add_tab(tab_widget, "标签 2");
```

## 分割器 (Splitter)

分割器允许用户通过拖动分隔线来调整子控件的大小。

```rust
use rust_widgets::{create_splitter, splitter_add_child};

// 创建水平分割器
let splitter = create_splitter(parent, true, x, y, width, height);

// 添加子控件
splitter_add_child(splitter, child_widget, 50); // 50% 的空间
```

## 停靠面板 (DockPanel)

停靠面板允许用户将子控件停靠到容器的不同边缘。

```rust
use rust_widgets::{create_dock_panel, dock_panel_dock_widget, DockPosition};

let dock_panel = create_dock_panel(parent, x, y, width, height);
dock_panel_dock_widget(dock_panel, widget, DockPosition::Left, "左侧面板");
```

## MDI 区域 (MdiArea)

MDI 区域允许用户在单个父窗口中管理多个文档窗口。

```rust
use rust_widgets::{create_mdi_area, mdi_area_add_subwindow};

let mdi_area = create_mdi_area(parent, x, y, width, height);
let subwindow = mdi_area_add_subwindow(mdi_area, "文档 1", 100, 100, 400, 300);
```