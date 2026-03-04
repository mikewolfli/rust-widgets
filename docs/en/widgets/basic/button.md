# Button

The Button widget provides a clickable control that triggers an action when pressed.

## Creating a Button

```rust
use rust_widgets::widget::Button;

// Create a simple button
let button = create_button(parent, "Click Me", x, y, width, height);

// Create with initial state
let button = Button::new(parent)
    .text("Submit")
    .position(10, 10)
    .size(100, 30)
    .enabled(true)
    .build();
```

## Properties

### Text

```rust
// Set button text
set_widget_text(button, "New Text");

// Get button text
let text = get_widget_text(button);
```

### Icon

```rust
// Set button icon (platform-dependent)
set_button_icon(button, icon_path);

// Set text and icon layout
set_button_layout(button, ButtonLayout::TextBesideIcon);
```

### Checkable

```rust
// Make button checkable (toggle button)
set_button_checkable(button, true);

// Check/uncheck
set_button_checked(button, true);

// Get checked state
let is_checked = is_button_checked(button);
```

### Auto-Repeat

```rust
// Enable auto-repeat when held down
set_button_auto_repeat(button, true);
set_button_auto_repeat_delay(button, 500);  // Initial delay (ms)
set_button_auto_repeat_interval(button, 100);  // Repeat interval (ms)
```

## Signals (Events)

### Clicked

Emitted when the button is clicked:

```rust
use rust_widgets::platform;

platform::connect_clicked(button, || {
    println!("Button clicked!");
});
```

### Pressed/Released

```rust
platform::connect_pressed(button, || {
    println!("Button pressed");
});

platform::connect_released(button, || {
    println!("Button released");
});
```

### Toggled (for checkable buttons)

```rust
platform::connect_toggled(button, |checked| {
    println!("Button toggled: {}", checked);
});
```

## Example

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_toolbar(parent: ObjectId) {
    // Create buttons
    let new_btn = create_button(parent, "New", 10, 10, 60, 30);
    let open_btn = create_button(parent, "Open", 80, 10, 60, 30);
    let save_btn = create_button(parent, "Save", 150, 10, 60, 30);
    
    // Connect signals
    platform::connect_clicked(new_btn, || {
        println!("New file");
    });
    
    platform::connect_clicked(open_btn, || {
        println!("Open file");
    });
    
    platform::connect_clicked(save_btn, || {
        println!("Save file");
    });
    
    // Toggle button example
    let bold_btn = create_button(parent, "Bold", 250, 10, 60, 30);
    set_button_checkable(bold_btn, true);
    
    platform::connect_toggled(bold_btn, |checked| {
        println!("Bold: {}", checked);
    });
}
```

## Platform-Specific Notes

### Windows
- Supports visual styles (themes)
- Default button gets special border
- Supports split buttons (with dropdown)

### macOS
- Native NSButton with bezel styles
- Supports gradient buttons
- Automatic dark mode support

### Linux
- Native GTK Button
- Supports image buttons
- Theme-aware styling

## Best Practices

1. **Use action verbs** for button text ("Save", "Delete", "Cancel")
2. **Provide keyboard shortcuts** (Alt+key)
3. **Set default button** for dialogs
4. **Disable when action unavailable**
5. **Show progress** for long operations

## See Also

- [ToolButton](../containers/toolbar.md) - Button for toolbars
- [RadioButton](radio-button.md) - Mutually exclusive selection
- [CheckBox](checkbox.md) - Binary state indicator
