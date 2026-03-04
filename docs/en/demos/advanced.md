# Advanced Controls Demo

The advanced controls demo showcases the more complex widgets provided by the rust_widgets library, including TreeView, TableView, ListView, TabWidget, Splitter, DockPanel, and MdiArea.

## Features

### TreeView
- Hierarchical data display
- Expandable/collapsible nodes
- Item selection

### TableView
- Tabular data display
- Column headers
- Row selection
- Cell editing

### ListView
- List data display
- Item selection
- Scrollable content

### TabWidget
- Multiple tabs for organizing content
- Tab switching
- Tab management

### Splitter
- Resizable panes
- Horizontal and vertical orientation
- Multiple panes support

### DockPanel
- Dockable panels
- Multiple dock positions (left, right, top, bottom, center)
- Floatable panels

### MdiArea
- Multiple document interface
- Child window management
- Cascading and tiling layouts

## Event Logging

The demo includes an event log that records all widget events, including:
- **Timestamp**: When the event occurred
- **Widget Type**: The type of widget that triggered the event
- **Widget ID**: The unique identifier of the widget
- **Event Type**: The type of event that occurred

## Usage

1. **Build the demo**: `cargo build --example demo_advanced`
2. **Run the demo**: `cargo run --example demo_advanced`

## TreeView Example

```rust
use rust_widgets::{create_window, create_tree_view, tree_view_set_model, show_widget, run, init};
use rust_widgets::widget::VecTreeModel;

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("TreeView Demo", 100, 100, 800, 600);
    
    // Create tree view
    let tree_view = create_tree_view(window, 10, 10, 780, 580);
    
    // Create tree model
    let mut model = VecTreeModel::new();
    model.add_item("Root", None);
    model.add_item("Child 1", Some("Root"));
    model.add_item("Child 2", Some("Root"));
    model.add_item("Grandchild 1", Some("Root/Child 1"));
    model.add_item("Grandchild 2", Some("Root/Child 1"));
    model.add_item("Grandchild 3", Some("Root/Child 2"));
    
    // Set model to tree view
    tree_view_set_model(tree_view, model);
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## TableView Example

```rust
use rust_widgets::{create_window, create_table_view, table_view_add_column, table_view_add_row, show_widget, run, init};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("TableView Demo", 100, 100, 800, 600);
    
    // Create table view
    let table_view = create_table_view(window, 10, 10, 780, 580);
    
    // Add columns
    table_view_add_column(table_view, "Name", 150);
    table_view_add_column(table_view, "Age", 80);
    table_view_add_column(table_view, "Email", 200);
    
    // Add rows
    table_view_add_row(table_view, vec!["John Doe", "30", "john.doe@example.com"]);
    table_view_add_row(table_view, vec!["Jane Smith", "25", "jane.smith@example.com"]);
    table_view_add_row(table_view, vec!["Bob Johnson", "35", "bob.johnson@example.com"]);
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## ListView Example

```rust
use rust_widgets::{create_window, create_list_view, list_view_set_model, show_widget, run, init};
use rust_widgets::widget::VecListModel;

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("ListView Demo", 100, 100, 800, 600);
    
    // Create list view
    let list_view = create_list_view(window, 10, 10, 780, 580);
    
    // Create list model
    let model = VecListModel::new(vec![
        "Item 1", "Item 2", "Item 3", "Item 4", "Item 5",
        "Item 6", "Item 7", "Item 8", "Item 9", "Item 10"
    ]);
    
    // Set model to list view
    list_view_set_model(list_view, model);
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## TabWidget Example

```rust
use rust_widgets::{create_window, create_tab_widget, tab_widget_add_tab, show_widget, run, init};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("TabWidget Demo", 100, 100, 800, 600);
    
    // Create tab widget
    let tab_widget = create_tab_widget(window, 10, 10, 780, 580);
    
    // Add tabs
    tab_widget_add_tab(tab_widget, "Tab 1");
    tab_widget_add_tab(tab_widget, "Tab 2");
    tab_widget_add_tab(tab_widget, "Tab 3");
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## Splitter Example

```rust
use rust_widgets::{create_window, create_splitter, create_panel, splitter_add_child, show_widget, run, init};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("Splitter Demo", 100, 100, 800, 600);
    
    // Create splitter
    let splitter = create_splitter(window, true, 10, 10, 780, 580);
    
    // Create panels
    let left_panel = create_panel(splitter, 0, 0, 390, 580);
    let right_panel = create_panel(splitter, 390, 0, 390, 580);
    
    // Add panels to splitter
    splitter_add_child(splitter, left_panel, 50);
    splitter_add_child(splitter, right_panel, 50);
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## DockPanel Example

```rust
use rust_widgets::{create_window, create_dock_panel, create_panel, dock_panel_dock_widget, DockPosition, show_widget, run, init};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("DockPanel Demo", 100, 100, 800, 600);
    
    // Create dock panel
    let dock_panel = create_dock_panel(window, 10, 10, 780, 580);
    
    // Create panels
    let left_panel = create_panel(dock_panel, 0, 0, 200, 580);
    let right_panel = create_panel(dock_panel, 600, 0, 200, 580);
    let top_panel = create_panel(dock_panel, 200, 0, 400, 100);
    let bottom_panel = create_panel(dock_panel, 200, 500, 400, 100);
    let central_panel = create_panel(dock_panel, 200, 100, 400, 400);
    
    // Dock panels
    dock_panel_dock_widget(dock_panel, left_panel, DockPosition::Left, "Left Panel");
    dock_panel_dock_widget(dock_panel, right_panel, DockPosition::Right, "Right Panel");
    dock_panel_dock_widget(dock_panel, top_panel, DockPosition::Top, "Top Panel");
    dock_panel_dock_widget(dock_panel, bottom_panel, DockPosition::Bottom, "Bottom Panel");
    dock_panel_dock_widget(dock_panel, central_panel, DockPosition::Center, "Central Panel");
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## MdiArea Example

```rust
use rust_widgets::{create_window, create_mdi_area, mdi_area_add_subwindow, show_widget, run, init};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("MdiArea Demo", 100, 100, 800, 600);
    
    // Create MDI area
    let mdi_area = create_mdi_area(window, 10, 10, 780, 580);
    
    // Add subwindows
    let subwindow1 = mdi_area_add_subwindow(mdi_area, "Document 1", 100, 100, 400, 300);
    let subwindow2 = mdi_area_add_subwindow(mdi_area, "Document 2", 150, 150, 400, 300);
    let subwindow3 = mdi_area_add_subwindow(mdi_area, "Document 3", 200, 200, 400, 300);
    
    // Show window
    show_widget(window);
    
    // Run event loop
    run();
}
```

## Screenshot

![Advanced Controls Demo](https://trae-api-cn.mchost.guru/api/ide/v1/text_to_image?prompt=A%20screenshot%20of%20an%20advanced%20controls%20demo%20window%20with%20tree%20view%2C%20table%20view%2C%20list%20view%2C%20tab%20widget%2C%20splitter%2C%20dock%20panel%2C%20and%20MDI%20area%20in%20a%20professional%20GUI%20layout&image_size=landscape_16_9)