# Basic Controls Demo

The basic controls demo showcases the fundamental widgets provided by the rust_widgets library, including buttons, checkboxes, labels, and text inputs.

## Features

- **Button**: A clickable button that triggers an action
- **Checkbox**: A toggleable checkbox for binary choices
- **Label**: A text label for displaying information
- **Line Edit**: A single-line text input field
- **Radio Button**: A radio button for selecting one option from a group
- **Slider**: A slider for selecting a value within a range
- **Progress Bar**: A progress bar for displaying progress
- **Combo Box**: A dropdown list for selecting from multiple options
- **List Box**: A list of selectable items

## Event Logging

The demo includes an event log that records all widget events, including:
- **Timestamp**: When the event occurred
- **Widget Type**: The type of widget that triggered the event
- **Widget ID**: The unique identifier of the widget
- **Event Type**: The type of event that occurred

## Multi-language Support

The demo supports multiple languages, including:
- English
- Simplified Chinese
- Traditional Chinese
- French

## Usage

1. **Build the demo**: `cargo build --example demo_basic`
2. **Run the demo**: `cargo run --example demo_basic`

## Code Example

```rust
use rust_widgets::{create_window, create_button, create_checkbox, create_label, create_line_edit, show_widget, run, init, poll_widget_trigger_event, WidgetTriggerKind, get_widget_text, set_widget_text};

fn main() {
    // Initialize the library
    init();
    
    // Create window
    let window = create_window("Basic Controls Demo", 100, 100, 800, 600);
    
    // Create widgets
    let button = create_button(window, "Click Me", 100, 100, 100, 30);
    let checkbox = create_checkbox(window, "Check Me", 100, 150, 100, 30);
    let label = create_label(window, "Hello World", 100, 200, 200, 30);
    let line_edit = create_line_edit(window, "Enter text", 100, 250, 200, 30);
    
    // Show window
    show_widget(window);
    
    // Main loop
    loop {
        // Poll for events
        if let Some(event) = poll_widget_trigger_event() {
            match event.kind {
                WidgetTriggerKind::Clicked if event.widget_id == button => {
                    println!("Button clicked!");
                    set_widget_text(button, "Clicked!");
                }
                WidgetTriggerKind::Clicked if event.widget_id == checkbox => {
                    println!("Checkbox clicked!");
                }
                WidgetTriggerKind::ValueChanged if event.widget_id == line_edit => {
                    let text = get_widget_text(line_edit);
                    println!("Text changed: {}", text);
                    set_widget_text(label, &text);
                }
                _ => {}
            }
        }
        
        // Run event loop
        run();
    }
}
```

## Screenshot

![Basic Controls Demo](https://trae-api-cn.mchost.guru/api/ide/v1/text_to_image?prompt=A%20screenshot%20of%20a%20basic%20controls%20demo%20window%20with%20buttons%2C%20checkboxes%2C%20labels%2C%20and%20text%20inputs%20in%20a%20clean%20GUI%20layout&image_size=landscape_16_9)