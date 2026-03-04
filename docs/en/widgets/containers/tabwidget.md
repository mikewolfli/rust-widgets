# TabWidget

The `TabWidget` is a container that allows users to switch between different panels using tabs.

## Creating a TabWidget

```rust
use rust_widgets::create_tab_widget;

let tab_widget = create_tab_widget(parent, x, y, width, height);
```

## Adding Tabs

```rust
use rust_widgets::tab_widget_add_tab;

tab_widget_add_tab(tab_widget, "Tab 1");
tab_widget_add_tab(tab_widget, "Tab 2");
```

## Selecting a Tab

```rust
use rust_widgets::tab_widget_set_current_index;

// Select the second tab (index 1)
tab_widget_set_current_index(tab_widget, 1);
```

## Getting the Current Tab Index

```rust
use rust_widgets::tab_widget_current_index;

if let Some(index) = tab_widget_current_index(tab_widget) {
    println!("Current tab index: {}", index);
}
```

## Removing a Tab

```rust
use rust_widgets::tab_widget_remove_tab;

// Remove the first tab (index 0)
tab_widget_remove_tab(tab_widget, 0);
```

## Getting Tab Count

```rust
use rust_widgets::tab_widget_tab_count;

let count = tab_widget_tab_count(tab_widget);
println!("Number of tabs: {}", count);
```

## Getting Tab Text

```rust
use rust_widgets::tab_widget_tab_text;

if let Some(text) = tab_widget_tab_text(tab_widget, 0) {
    println!("Tab text: {}", text);
}
```

## Event Handling

To handle tab selection changes, you can poll for widget trigger events:

```rust
use rust_widgets::{poll_widget_trigger_event, WidgetTriggerKind};

if let Some(event) = poll_widget_trigger_event() {
    if event.kind == WidgetTriggerKind::SelectionChanged {
        println!("Tab selection changed for widget: {}", event.widget_id);
    }
}
```

## Example

```rust
use rust_widgets::{create_window, create_tab_widget, tab_widget_add_tab, tab_widget_set_current_index, show_widget, run};

fn main() {
    // Initialize the library
    rust_widgets::init();
    
    // Create a window
    let window = create_window("TabWidget Example", 100, 100, 800, 600);
    
    // Create a TabWidget
    let tab_widget = create_tab_widget(window, 10, 10, 780, 580);
    
    // Add tabs
    tab_widget_add_tab(tab_widget, "Tab 1");
    tab_widget_add_tab(tab_widget, "Tab 2");
    tab_widget_add_tab(tab_widget, "Tab 3");
    
    // Select the second tab
    tab_widget_set_current_index(tab_widget, 1);
    
    // Show the window
    show_widget(window);
    
    // Run the event loop
    run();
}
```