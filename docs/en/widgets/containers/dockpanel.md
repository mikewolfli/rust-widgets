# DockPanel

The `DockPanel` is a container that allows users to dock child widgets to different edges of the container.

## Creating a DockPanel

```rust
use rust_widgets::create_dock_panel;

let dock_panel = create_dock_panel(parent, x, y, width, height);
```

## Docking a Widget

```rust
use rust_widgets::{dock_panel_dock_widget, DockPosition};

// Dock a widget to the left edge
dock_panel_dock_widget(dock_panel, widget, DockPosition::Left, "Left Panel");

// Dock a widget to the right edge
dock_panel_dock_widget(dock_panel, widget2, DockPosition::Right, "Right Panel");

// Dock a widget to the top edge
dock_panel_dock_widget(dock_panel, widget3, DockPosition::Top, "Top Panel");

// Dock a widget to the bottom edge
dock_panel_dock_widget(dock_panel, widget4, DockPosition::Bottom, "Bottom Panel");

// Set a widget as the central widget
dock_panel_dock_widget(dock_panel, central_widget, DockPosition::Center, "Central Panel");
```

## Undocking a Widget

```rust
use rust_widgets::dock_panel_undock_widget;

// Undock a widget from the dock panel
dock_panel_undock_widget(dock_panel, widget);
```

## Hiding a Dock Widget

```rust
use rust_widgets::dock_panel_hide_widget;

// Hide a docked widget
dock_panel_hide_widget(dock_panel, widget);
```

## Showing a Dock Widget

```rust
use rust_widgets::dock_panel_show_widget;

// Show a docked widget
dock_panel_show_widget(dock_panel, widget);
```

## Example

```rust
use rust_widgets::{create_window, create_dock_panel, create_panel, dock_panel_dock_widget, DockPosition, show_widget, run};

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("DockPanel Example", 100, 100, 800, 600);
    
    // Create a DockPanel
    let dock_panel = create_dock_panel(window, 10, 10, 780, 580);
    
    // Create panels for docking
    let left_panel = create_panel(dock_panel, 0, 0, 200, 580);
    let right_panel = create_panel(dock_panel, 600, 0, 200, 580);
    let top_panel = create_panel(dock_panel, 200, 0, 400, 100);
    let bottom_panel = create_panel(dock_panel, 200, 500, 400, 100);
    let central_panel = create_panel(dock_panel, 200, 100, 400, 400);
    
    // Dock the panels
    dock_panel_dock_widget(dock_panel, left_panel, DockPosition::Left, "Left Panel");
    dock_panel_dock_widget(dock_panel, right_panel, DockPosition::Right, "Right Panel");
    dock_panel_dock_widget(dock_panel, top_panel, DockPosition::Top, "Top Panel");
    dock_panel_dock_widget(dock_panel, bottom_panel, DockPosition::Bottom, "Bottom Panel");
    dock_panel_dock_widget(dock_panel, central_panel, DockPosition::Center, "Central Panel");
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```