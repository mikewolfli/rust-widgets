# Label

The Label widget displays static or dynamic text.

## Creating a Label

```rust
use rust_widgets::widget::Label;

// Create a simple label
let label = create_label(parent, "Hello World", x, y, width, height);

// Create with builder pattern
let label = Label::new(parent)
    .text("Welcome")
    .position(10, 10)
    .size(200, 25)
    .alignment(Alignment::Center)
    .build();
```

## Properties

### Text

```rust
// Set label text
set_widget_text(label, "New Text");

// Get label text
let text = get_widget_text(label);

// Set rich text (HTML subset)
set_label_rich_text(label, "<b>Bold</b> and <i>italic</i>");
```

### Alignment

```rust
use rust_widgets::Alignment;

// Horizontal alignment
set_label_alignment(label, Alignment::Left);
set_label_alignment(label, Alignment::Center);
set_label_alignment(label, Alignment::Right);

// Vertical alignment
set_label_vertical_alignment(label, Alignment::Top);
set_label_vertical_alignment(label, Alignment::VCenter);
set_label_vertical_alignment(label, Alignment::Bottom);
```

### Word Wrap

```rust
// Enable word wrap
set_label_word_wrap(label, true);

// Set wrap mode
set_label_wrap_mode(label, WrapMode::WordWrap);
set_label_wrap_mode(label, WrapMode::CharWrap);
```

### Text Interaction

```rust
// Allow text selection
set_label_text_interaction(label, TextInteraction::TextSelectable);

// Open links
set_label_open_external_links(label, true);
```

### Buddy Widget

```rust
// Associate with input widget for accessibility
set_label_buddy(label, line_edit);
```

## Example

```rust
use rust_widgets::*;

fn create_form(parent: ObjectId) {
    // Simple labels
    let title = create_label(parent, "User Information", 10, 10, 200, 30);
    set_widget_style(title, "font-size: 16px; font-weight: bold;");
    
    // Form labels
    let name_label = create_label(parent, "Name:", 10, 50, 80, 25);
    let name_input = create_line_edit(parent, "", 100, 50, 200, 25);
    set_label_buddy(name_label, name_input);
    
    // Right-aligned label
    let email_label = create_label(parent, "Email:", 10, 80, 80, 25);
    set_label_alignment(email_label, Alignment::Right);
    
    // Multi-line label with wrap
    let description = create_label(parent, "This is a long description...", 10, 120, 300, 60);
    set_label_word_wrap(description, true);
}
```

## Platform Notes

### Windows
- Native static control
- Supports SS_NOTIFY for click events

### macOS
- Native NSTextField (non-editable)
- Supports attributed strings

### Linux
- Native GTK Label
- Supports markup

## See Also

- [LineEdit](text-input.md#lineedit) - Editable text input
- [TextEdit](text-input.md#textedit) - Multi-line text
