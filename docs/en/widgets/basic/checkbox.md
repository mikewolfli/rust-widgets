# CheckBox

The CheckBox widget provides a binary choice (checked/unchecked).

## Creating a CheckBox

```rust
use rust_widgets::widget::CheckBox;

// Create a checkbox
let checkbox = create_checkbox(parent, "Enable feature", x, y, width, height);

// With builder
let checkbox = CheckBox::new(parent)
    .text("Remember me")
    .checked(true)
    .tristate(false)
    .build();
```

## Properties

### Text

```rust
// Set checkbox text
set_widget_text(checkbox, "New label");

// Get text
let text = get_widget_text(checkbox);
```

### Check State

```rust
// Set checked state
set_checkbox_checked(checkbox, true);

// Get checked state
let is_checked = is_checkbox_checked(checkbox);

// Toggle
toggle_checkbox(checkbox);
```

### Tristate Mode

```rust
// Enable tristate (checked/unchecked/partial)
set_checkbox_tristate(checkbox, true);

// Set check state
set_checkbox_check_state(checkbox, CheckState::Unchecked);
set_checkbox_check_state(checkbox, CheckState::PartiallyChecked);
set_checkbox_check_state(checkbox, CheckState::Checked);

// Get check state
let state = get_checkbox_check_state(checkbox);
```

## Signals

### State Changed

```rust
use rust_widgets::platform;

// Checked state changed
platform::connect_state_changed(checkbox, |checked| {
    println!("Checked: {}", checked);
});

// For tristate
platform::connect_check_state_changed(checkbox, |state| {
    match state {
        CheckState::Unchecked => println!("Unchecked"),
        CheckState::PartiallyChecked => println!("Partial"),
        CheckState::Checked => println!("Checked"),
    }
});
```

### Clicked

```rust
platform::connect_clicked(checkbox, || {
    println!("Checkbox clicked");
});
```

## Example

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_settings_panel(parent: ObjectId) {
    // Simple checkboxes
    let auto_save = create_checkbox(parent, "Auto-save", 10, 10, 150, 25);
    let notifications = create_checkbox(parent, "Enable notifications", 10, 40, 200, 25);
    let dark_mode = create_checkbox(parent, "Dark mode", 10, 70, 150, 25);
    
    // Set initial states
    set_checkbox_checked(auto_save, true);
    set_checkbox_checked(notifications, false);
    
    // Connect signals
    platform::connect_state_changed(auto_save, |checked| {
        println!("Auto-save: {}", checked);
    });
    
    platform::connect_state_changed(dark_mode, |checked| {
        if checked {
            println!("Enabling dark mode...");
        } else {
            println!("Disabling dark mode...");
        }
    });
    
    // Tristate example (parent checkbox)
    let select_all = create_checkbox(parent, "Select all", 10, 120, 150, 25);
    set_checkbox_tristate(select_all, true);
}
```

## Platform Notes

### Windows
- Native BUTTON with BS_CHECKBOX style
- Supports visual styles

### macOS
- Native NSButton with switch style
- Automatic animation

### Linux
- Native GTK CheckButton
- Theme-aware

## Best Practices

1. **Use positive wording** ("Enable X" rather than "Disable X")
2. **Group related checkboxes** with a GroupBox
3. **Use tristate** for hierarchical selections
4. **Provide immediate feedback** on state change

## See Also

- [RadioButton](radio-button.md) - Mutually exclusive selection
- [GroupBox](../containers/groupbox.md) - Container for grouping
