# Basic Widgets

Basic widgets are the fundamental building blocks of any GUI application. They provide essential user interaction capabilities and are available on all supported platforms.

## Available Basic Widgets

| Widget | Description | Platform Support |
|--------|-------------|------------------|
| [Button](basic/button.md) | Clickable button with text or icon | All |
| [Label](basic/label.md) | Static text display | All |
| [LineEdit](basic/text-input.md#lineedit) | Single-line text input | All |
| [TextEdit](basic/text-input.md#textedit) | Multi-line text input | All |
| [CheckBox](basic/checkbox.md) | Binary choice (checked/unchecked) | All |
| [RadioButton](basic/radio-button.md) | Mutually exclusive selection | All |
| [ComboBox](basic/combobox.md) | Dropdown selection list | All |
| [SpinBox](basic/spinbox.md) | Numeric value with up/down buttons | All |
| [Slider](basic/slider.md) | Horizontal or vertical value slider | All |
| [ProgressBar](basic/progressbar.md) | Visual progress indicator | All |

## Common Properties

All basic widgets share these common properties:

### Geometry

```rust
// Set widget position and size
set_widget_geometry(widget_id, x, y, width, height);

// Get widget geometry
let (x, y, width, height) = get_widget_geometry(widget_id);
```

### Visibility

```rust
// Show/hide widget
show_widget(widget_id);
hide_widget(widget_id);

// Check visibility
let is_visible = is_widget_visible(widget_id);
```

### Enable/Disable

```rust
// Enable/disable widget
set_widget_enabled(widget_id, true);  // Enable
set_widget_enabled(widget_id, false); // Disable

// Check enabled state
let is_enabled = is_widget_enabled(widget_id);
```

### Styling

```rust
// Set widget style (platform-dependent)
set_widget_style(widget_id, style);

// Apply theme
apply_widget_theme(widget_id, theme_name);
```

## Event Handling

Basic widgets emit various events that can be connected to handlers:

```rust
use rust_widgets::platform;

// Button clicked
platform::connect_clicked(button_id, || {
    println!("Button clicked!");
});

// Text changed
platform::connect_text_changed(line_edit_id, |text| {
    println!("Text changed to: {}", text);
});

// Value changed (for sliders, spin boxes)
platform::connect_value_changed(slider_id, |value| {
    println!("Value changed to: {}", value);
});
```

## Best Practices

1. **Consistent Sizing**: Use standard sizes for related widgets
2. **Tab Order**: Set logical tab order for keyboard navigation
3. **Accessibility**: Provide tooltips and accessible names
4. **Validation**: Validate user input before processing
5. **Feedback**: Provide visual feedback for user actions

## Example: Login Form

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_login_form(parent: ObjectId) -> ObjectId {
    let panel = create_panel(parent, 10, 10, 300, 200);
    
    // Username
    let username_label = create_label(panel, "Username:", 10, 10, 80, 25);
    let username_input = create_line_edit(panel, "", 100, 10, 180, 25);
    
    // Password
    let password_label = create_label(panel, "Password:", 10, 45, 80, 25);
    let password_input = create_line_edit(panel, "", 100, 45, 180, 25);
    platform::set_echo_mode(password_input, EchoMode::Password);
    
    // Remember me checkbox
    let remember_check = create_checkbox(panel, "Remember me", 100, 80, 150, 25);
    
    // Login button
    let login_btn = create_button(panel, "Login", 100, 120, 100, 30);
    platform::connect_clicked(login_btn, move || {
        let username = platform::get_widget_text(username_input);
        let password = platform::get_widget_text(password_input);
        let remember = platform::is_checked(remember_check);
        
        println!("Login: {}, Remember: {}", username, remember);
        // Process login...
    });
    
    panel
}
```

## Platform Notes

### Windows
- Native Win32 controls with visual styles
- Supports high DPI scaling
- Full accessibility support via MSAA/UIA

### macOS
- Native Cocoa controls
- Automatic dark mode support
- Full VoiceOver support

### Linux
- GTK3/GTK4 native controls
- Theme integration
- Accessibility via AT-SPI

### HarmonyOS
- ArkUI native components
- Adaptive layout support
- Touch-optimized interactions
