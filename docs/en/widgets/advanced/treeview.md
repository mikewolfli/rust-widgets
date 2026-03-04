# TreeView

The `TreeView` is a widget that displays hierarchical data in a tree structure.

## Creating a TreeView

```rust
use rust_widgets::create_tree_view;

let tree_view = create_tree_view(parent, x, y, width, height);
```

## Setting a Tree Model

```rust
use rust_widgets::widget::VecTreeModel;
use rust_widgets::tree_view_set_model;

// Create a tree model
let mut model = VecTreeModel::new();

// Add items to the model
model.add_item("Root", None);
model.add_item("Child 1", Some("Root"));
model.add_item("Child 2", Some("Root"));
model.add_item("Grandchild", Some("Root/Child 1"));

// Set the model to the TreeView
tree_view_set_model(tree_view, model);
```

## Getting the Selected Item

```rust
use rust_widgets::tree_view_get_selected_item;

if let Some(selected_item) = tree_view_get_selected_item(tree_view) {
    println!("Selected item: {}", selected_item);
}
```

## Expanding an Item

```rust
use rust_widgets::tree_view_expand_item;

// Expand an item
tree_view_expand_item(tree_view, "Root");
```

## Collapsing an Item

```rust
use rust_widgets::tree_view_collapse_item;

// Collapse an item
tree_view_collapse_item(tree_view, "Root");
```

## Example

```rust
use rust_widgets::{create_window, create_tree_view, tree_view_set_model, show_widget, run};
use rust_widgets::widget::VecTreeModel;

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("TreeView Example", 100, 100, 800, 600);
    
    // Create a TreeView
    let tree_view = create_tree_view(window, 10, 10, 780, 580);
    
    // Create a tree model
    let mut model = VecTreeModel::new();
    
    // Add items to the model
    model.add_item("Root", None);
    model.add_item("Child 1", Some("Root"));
    model.add_item("Child 2", Some("Root"));
    model.add_item("Grandchild 1", Some("Root/Child 1"));
    model.add_item("Grandchild 2", Some("Root/Child 1"));
    model.add_item("Grandchild 3", Some("Root/Child 2"));
    
    // Set the model to the TreeView
    tree_view_set_model(tree_view, model);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```