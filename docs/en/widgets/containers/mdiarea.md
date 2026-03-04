# MdiArea

The `MdiArea` is a container that allows users to manage multiple document windows within a single parent window.

## Creating an MdiArea

```rust
use rust_widgets::create_mdi_area;

let mdi_area = create_mdi_area(parent, x, y, width, height);
```

## Adding a Subwindow

```rust
use rust_widgets::mdi_area_add_subwindow;

// Add a subwindow to the MdiArea
let subwindow = mdi_area_add_subwindow(mdi_area, "Document 1", 100, 100, 400, 300);
```

## Activating a Subwindow

```rust
use rust_widgets::mdi_area_activate_subwindow;

// Activate a subwindow
mdi_area_activate_subwindow(mdi_area, subwindow);
```

## Closing a Subwindow

```rust
use rust_widgets::mdi_area_close_subwindow;

// Close a subwindow
mdi_area_close_subwindow(mdi_area, subwindow);
```

## Getting the Active Subwindow

```rust
use rust_widgets::mdi_area_active_subwindow;

if let Some(active_window) = mdi_area_active_subwindow(mdi_area) {
    println!("Active subwindow: {}", active_window);
}
```

## Cascading Subwindows

```rust
use rust_widgets::mdi_area_cascade_subwindows;

// Arrange subwindows in a cascading pattern
mdi_area_cascade_subwindows(mdi_area);
```

## Tiling Subwindows

```rust
use rust_widgets::mdi_area_tile_subwindows;

// Arrange subwindows in a tiled pattern
mdi_area_tile_subwindows(mdi_area);
```

## Example

```rust
use rust_widgets::{create_window, create_mdi_area, mdi_area_add_subwindow, show_widget, run};

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("MdiArea Example", 100, 100, 800, 600);
    
    // Create an MdiArea
    let mdi_area = create_mdi_area(window, 10, 10, 780, 580);
    
    // Add subwindows
    let subwindow1 = mdi_area_add_subwindow(mdi_area, "Document 1", 100, 100, 400, 300);
    let subwindow2 = mdi_area_add_subwindow(mdi_area, "Document 2", 150, 150, 400, 300);
    let subwindow3 = mdi_area_add_subwindow(mdi_area, "Document 3", 200, 200, 400, 300);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```