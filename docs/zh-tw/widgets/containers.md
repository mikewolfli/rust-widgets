# 容器控件

本章介紹 rust_widgets 庫中的容器控件，包括面板、選項卡、分割器等。

## 面板 (Panel)

面板是最基本的容器控件，用於組織其他控件。

```rust
use rust_widgets::create_panel;

let panel = create_panel(parent, x, y, width, height);
```

## 選項卡控件 (TabWidget)

選項卡控件允許使用者通過標籤切換不同的面板。

```rust
use rust_widgets::{create_tab_widget, tab_widget_add_tab};

let tab_widget = create_tab_widget(parent, x, y, width, height);
tab_widget_add_tab(tab_widget, "標籤 1");
tab_widget_add_tab(tab_widget, "標籤 2");
```

## 分割器 (Splitter)

分割器允許使用者通過拖動分隔線來調整子控件的大小。

```rust
use rust_widgets::{create_splitter, splitter_add_child};

// 建立水平分割器
let splitter = create_splitter(parent, true, x, y, width, height);

// 新增子控件
splitter_add_child(splitter, child_widget, 50); // 50% 的空間
```

## 停靠面板 (DockPanel)

停靠面板允許使用者將子控件停靠到容器的不同邊緣。

```rust
use rust_widgets::{create_dock_panel, dock_panel_dock_widget, DockPosition};

let dock_panel = create_dock_panel(parent, x, y, width, height);
dock_panel_dock_widget(dock_panel, widget, DockPosition::Left, "左側面板");
```

## MDI 區域 (MdiArea)

MDI 區域允許使用者在單個父視窗中管理多個文件視窗。

```rust
use rust_widgets::{create_mdi_area, mdi_area_add_subwindow};

let mdi_area = create_mdi_area(parent, x, y, width, height);
let subwindow = mdi_area_add_subwindow(mdi_area, "文件 1", 100, 100, 400, 300);
```