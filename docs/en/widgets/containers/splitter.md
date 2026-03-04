# Splitter

The `Splitter` is a container that allows users to resize child widgets by dragging a divider between them.

## Creating a Splitter

```rust
use rust_widgets::create_splitter;

// Create a horizontal splitter
let splitter = create_splitter(parent, true, x, y, width, height);

// Create a vertical splitter
let vertical_splitter = create_splitter(parent, false, x, y, width, height);
```

## Adding Children to a Splitter

```rust
use rust_widgets::splitter_add_child;

// Add a child widget to the splitter
splitter_add_child(splitter, child_widget, 50); // 50% of the splitter's space
```

## Setting Splitter Position

```rust
use rust_widgets::splitter_set_position;

// Set the splitter position to 300 pixels
splitter_set_position(splitter, 300);
```

## Getting Splitter Position

```rust
use rust_widgets::splitter_get_position;

let position = splitter_get_position(splitter);
println!("Splitter position: {}", position);
```

## Setting Splitter Orientation

```rust
use rust_widgets::splitter_set_orientation;

// Set to horizontal orientation
splitter_set_orientation(splitter, true);

// Set to vertical orientation
splitter_set_orientation(splitter, false);
```

## Getting Splitter Orientation

```rust
use rust_widgets::splitter_get_orientation;

let is_horizontal = splitter_get_orientation(splitter);
println!("Splitter is horizontal: {}", is_horizontal);
```

## Example

```rust
use rust_widgets::{create_window, create_splitter, create_panel, splitter_add_child, show_widget, run};

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("Splitter Example", 100, 100, 800, 600);
    
    // Create a horizontal splitter
    let splitter = create_splitter(window, true, 10, 10, 780, 580);
    
    // Create left and right panels
    let left_panel = create_panel(splitter, 0, 0, 390, 580);
    let right_panel = create_panel(splitter, 390, 0, 390, 580);
    
    // Add panels to splitter
    splitter_add_child(splitter, left_panel, 50);
    splitter_add_child(splitter, right_panel, 50);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```