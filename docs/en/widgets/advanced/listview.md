# ListView

The `ListView` is a widget that displays a list of items in a scrollable view.

## Creating a ListView

```rust
use rust_widgets::create_list_view;

let list_view = create_list_view(parent, x, y, width, height);
```

## Setting a List Model

```rust
use rust_widgets::widget::VecListModel;
use rust_widgets::list_view_set_model;

// Create a list model
let model = VecListModel::new(vec!["Item 1", "Item 2", "Item 3", "Item 4", "Item 5"]);

// Set the model to the ListView
list_view_set_model(list_view, model);
```

## Getting the Selected Item

```rust
use rust_widgets::list_view_get_selected_item;

if let Some(selected_item) = list_view_get_selected_item(list_view) {
    println!("Selected item index: {}", selected_item);
}
```

## Example

```rust
use rust_widgets::{create_window, create_list_view, list_view_set_model, show_widget, run};
use rust_widgets::widget::VecListModel;

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("ListView Example", 100, 100, 800, 600);
    
    // Create a ListView
    let list_view = create_list_view(window, 10, 10, 780, 580);
    
    // Create a list model
    let model = VecListModel::new(vec![
        "Item 1",
        "Item 2",
        "Item 3",
        "Item 4",
        "Item 5",
        "Item 6",
        "Item 7",
        "Item 8",
        "Item 9",
        "Item 10"
    ]);
    
    // Set the model to the ListView
    list_view_set_model(list_view, model);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```