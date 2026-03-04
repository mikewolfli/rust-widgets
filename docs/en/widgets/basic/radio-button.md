# Radio Button

The RadioButton widget provides a mutually exclusive selection option. Only one radio button in a group can be selected at a time.

## Creating a Radio Button

```rust
use rust_widgets::widget::RadioButton;

// Create a radio button
let radio = create_radio_button(parent, "Option 1", x, y, width, height);

// With builder
let radio = RadioButton::new(parent)
    .text("Option A")
    .checked(true)
    .build();
```

## Properties

### Text

```rust
// Set radio button text
set_widget_text(radio, "New label");

// Get text
let text = get_widget_text(radio);
```

### Checked State

```rust
// Set checked state
set_radio_button_checked(radio, true);

// Get checked state
let is_checked = is_radio_button_checked(radio);
```

### Grouping

Radio buttons are automatically grouped by parent widget. To create separate groups:

```rust
// Group 1
let group1_panel = create_panel(parent, 10, 10, 200, 100);
let radio1a = create_radio_button(group1_panel, "Group 1 - A", 10, 10, 150, 25);
let radio1b = create_radio_button(group1_panel, "Group 1 - B", 10, 40, 150, 25);

// Group 2
let group2_panel = create_panel(parent, 220, 10, 200, 100);
let radio2a = create_radio_button(group2_panel, "Group 2 - A", 10, 10, 150, 25);
let radio2b = create_radio_button(group2_panel, "Group 2 - B", 10, 40, 150, 25);
```

## Signals

### Toggled

```rust
use rust_widgets::platform;

// State changed
platform::connect_toggled(radio, |checked| {
    if checked {
        println!("Radio button selected");
    }
});
```

### Clicked

```rust
platform::connect_clicked(radio, || {
    println!("Radio button clicked");
});
```

## Example

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_theme_selector(parent: ObjectId) {
    let label = create_label(parent, "Theme:", 10, 10, 80, 25);
    
    // Theme options
    let light_radio = create_radio_button(parent, "Light", 100, 10, 100, 25);
    let dark_radio = create_radio_button(parent, "Dark", 100, 40, 100, 25);
    let auto_radio = create_radio_button(parent, "Auto", 100, 70, 100, 25);
    
    // Set default selection
    set_radio_button_checked(light_radio, true);
    
    // Connect signals
    platform::connect_toggled(light_radio, |checked| {
        if checked { println!("Light theme selected"); }
    });
    
    platform::connect_toggled(dark_radio, |checked| {
        if checked { println!("Dark theme selected"); }
    });
    
    platform::connect_toggled(auto_radio, |checked| {
        if checked { println!("Auto theme selected"); }
    });
}
```

## Platform Notes

### Windows
- Native BUTTON with BS_RADIOBUTTON style
- Supports visual styles

### macOS
- Native NSButton with radio style
- Automatic grouping

### Linux
- Native GTK RadioButton
- Theme-aware

## Best Practices

1. **Group logically** related options together
2. **Provide a default selection**
3. **Use labels** that clearly describe the option
4. **Consider alternatives** for many options (use ComboBox instead)

## See Also

- [CheckBox](checkbox.md) - Binary state (not mutually exclusive)
- [ComboBox](combobox.md) - Dropdown selection
- [GroupBox](../containers/groupbox.md) - Visual grouping
