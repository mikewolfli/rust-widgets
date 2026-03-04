# TableView

The `TableView` is a widget that displays tabular data in rows and columns.

## Creating a TableView

```rust
use rust_widgets::create_table_view;

let table_view = create_table_view(parent, x, y, width, height);
```

## Setting Columns

```rust
use rust_widgets::table_view_add_column;

// Add columns to the TableView
table_view_add_column(table_view, "Name", 150);
table_view_add_column(table_view, "Age", 80);
table_view_add_column(table_view, "Email", 200);
```

## Adding Rows

```rust
use rust_widgets::table_view_add_row;

// Add a row to the TableView
let row = vec!["John Doe", "30", "john@example.com"];
table_view_add_row(table_view, row);
```

## Getting the Selected Row

```rust
use rust_widgets::table_view_get_selected_row;

if let Some(selected_row) = table_view_get_selected_row(table_view) {
    println!("Selected row index: {}", selected_row);
}
```

## Getting Cell Value

```rust
use rust_widgets::table_view_get_cell_value;

if let Some(value) = table_view_get_cell_value(table_view, row_index, column_index) {
    println!("Cell value: {}", value);
}
```

## Setting Cell Value

```rust
use rust_widgets::table_view_set_cell_value;

// Set the value of a cell
table_view_set_cell_value(table_view, row_index, column_index, "New Value");
```

## Example

```rust
use rust_widgets::{create_window, create_table_view, table_view_add_column, table_view_add_row, show_widget, run};

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("TableView Example", 100, 100, 800, 600);
    
    // Create a TableView
    let table_view = create_table_view(window, 10, 10, 780, 580);
    
    // Add columns
    table_view_add_column(table_view, "Name", 150);
    table_view_add_column(table_view, "Age", 80);
    table_view_add_column(table_view, "Email", 200);
    
    // Add rows
    table_view_add_row(table_view, vec!["John Doe", "30", "john@example.com"]);
    table_view_add_row(table_view, vec!["Jane Smith", "25", "jane@example.com"]);
    table_view_add_row(table_view, vec!["Bob Johnson", "35", "bob@example.com"]);
    table_view_add_row(table_view, vec!["Alice Brown", "28", "alice@example.com"]);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```