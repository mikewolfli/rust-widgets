# Text Input Widgets

Text input widgets allow users to enter and edit text.

## LineEdit

Single-line text input field.

### Creating a LineEdit

```rust
use rust_widgets::widget::LineEdit;

let line_edit = create_line_edit(parent, "Initial text", x, y, width, height);

// With builder
let line_edit = LineEdit::new(parent)
    .placeholder_text("Enter name...")
    .max_length(50)
    .echo_mode(EchoMode::Normal)
    .build();
```

### Properties

```rust
// Text
set_widget_text(line_edit, "New text");
let text = get_widget_text(line_edit);

// Placeholder
set_line_edit_placeholder(line_edit, "Hint text");

// Maximum length
set_line_edit_max_length(line_edit, 100);

// Read-only
set_line_edit_read_only(line_edit, true);

// Echo mode (for passwords)
set_line_edit_echo_mode(line_edit, EchoMode::Password);
set_line_edit_echo_mode(line_edit, EchoMode::NoEcho);
```

### Validation

```rust
// Set validator
set_line_edit_validator(line_edit, Validator::Integer);
set_line_edit_validator(line_edit, Validator::Double);

// Custom validation
set_line_edit_validator_fn(line_edit, |text| {
    text.len() >= 3 && text.contains('@')
});
```

### Signals

```rust
// Text changed
platform::connect_text_changed(line_edit, |text| {
    println!("Text: {}", text);
});

// Editing finished (Enter pressed or focus lost)
platform::connect_editing_finished(line_edit, || {
    println!("Editing finished");
});

// Return pressed
platform::connect_return_pressed(line_edit, || {
    println!("Return pressed");
});
```

## TextEdit

Multi-line text editor.

### Creating a TextEdit

```rust
use rust_widgets::widget::TextEdit;

let text_edit = create_text_edit(parent, x, y, width, height);

// With builder
let text_edit = TextEdit::new(parent)
    .plain_text("Initial content")
    .read_only(false)
    .line_wrap_mode(LineWrapMode::WidgetWidth)
    .build();
```

### Properties

```rust
// Text
set_widget_text(text_edit, "Plain text content");
set_text_edit_html(text_edit, "<p>HTML content</p>");

let plain_text = get_widget_text(text_edit);
let html = get_text_edit_html(text_edit);

// Read-only
set_text_edit_read_only(text_edit, true);

// Line wrap
set_text_edit_line_wrap_mode(text_edit, LineWrapMode::NoWrap);
set_text_edit_line_wrap_mode(text_edit, LineWrapMode::WidgetWidth);
set_text_edit_line_wrap_mode(text_edit, LineWrapMode::FixedPixelWidth(200));

// Tab changes focus
set_text_edit_tab_changes_focus(text_edit, true);

// Accept rich text
set_text_edit_accept_rich_text(text_edit, true);
```

### Cursor and Selection

```rust
// Get/set cursor position
let pos = text_edit_cursor_position(text_edit);
set_text_edit_cursor_position(text_edit, 100);

// Selection
set_text_edit_selection(text_edit, 10, 50);
let selected_text = text_edit_selected_text(text_edit);

text_edit_clear_selection(text_edit);
text_edit_select_all(text_edit);
```

### Signals

```rust
// Text changed
platform::connect_text_changed(text_edit, || {
    println!("Text changed");
});

// Selection changed
platform::connect_selection_changed(text_edit, || {
    println!("Selection changed");
});

// Cursor position changed
platform::connect_cursor_position_changed(text_edit, |pos| {
    println!("Cursor at: {}", pos);
});
```

## Example

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_login_form(parent: ObjectId) {
    // Username
    let username_label = create_label(parent, "Username:", 10, 10, 80, 25);
    let username_edit = create_line_edit(parent, "", 100, 10, 200, 25);
    set_line_edit_placeholder(username_edit, "Enter username");
    
    // Password
    let password_label = create_label(parent, "Password:", 10, 45, 80, 25);
    let password_edit = create_line_edit(parent, "", 100, 45, 200, 25);
    set_line_edit_echo_mode(password_edit, EchoMode::Password);
    set_line_edit_placeholder(password_edit, "Enter password");
    
    // Bio (multi-line)
    let bio_label = create_label(parent, "Bio:", 10, 80, 80, 25);
    let bio_edit = create_text_edit(parent, 100, 80, 200, 100);
    set_text_edit_placeholder(bio_edit, "Tell us about yourself...");
    
    // Connect signals
    platform::connect_return_pressed(password_edit, || {
        println!("Login submitted");
    });
}
```

## Platform Notes

### Windows
- LineEdit: Native EDIT control
- TextEdit: RichEdit or plain EDIT

### macOS
- Native NSTextField / NSTextView
- Supports spell checking

### Linux
- Native GTK Entry / TextView
- Supports input methods

## See Also

- [Label](label.md) - Static text display
- [SpinBox](spinbox.md) - Numeric input
