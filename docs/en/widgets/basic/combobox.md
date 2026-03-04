# ComboBox

The ComboBox widget provides a dropdown list of selectable items.

## Creating a ComboBox

```rust
use rust_widgets::widget::ComboBox;

// Create a combo box
let combo = create_combo_box(parent, x, y, width, height);

// With builder
let combo = ComboBox::new(parent)
    .editable(false)
    .max_visible_items(10)
    .build();
```

## Properties

### Items

```rust
// Add items
platform::combo_box_add_item(combo, "Item 1");
platform::combo_box_add_item(combo, "Item 2");
platform::combo_box_add_item(combo, "Item 3");

// Insert item at index
platform::combo_box_insert_item(combo, 0, "First Item");

// Remove item
platform::combo_box_remove_item(combo, 1);

// Clear all items
platform::combo_box_clear(combo);

// Get item count
let count = platform::combo_box_count(combo);

// Get item text
let text = platform::combo_box_item_text(combo, 0);
```

### Selection

```rust
// Set current index
platform::combo_box_set_current_index(combo, 0);

// Get current index
let index = platform::combo_box_current_index(combo);

// Get current text
let text = platform::combo_box_current_text(combo);

// Set current text (for editable combo)
platform::set_widget_text(combo, "Custom text");
```

### Editable

```rust
// Make editable
set_combo_box_editable(combo, true);

// Set edit text
set_combo_box_edit_text(combo, "Editable text");
```

### Size

```rust
// Set maximum visible items in dropdown
set_combo_box_max_visible_items(combo, 15);

// Set minimum contents length
set_combo_box_minimum_contents_length(combo, 10);
```

## Signals

### Current Index Changed

```rust
use rust_widgets::platform;

platform::connect_current_index_changed(combo, |index| {
    println!("Selected index: {}", index);
});
```

### Current Text Changed

```rust
platform::connect_current_text_changed(combo, |text| {
    println!("Selected text: {}", text);
});
```

### Activated

```rust
platform::connect_activated(combo, |index| {
    println!("Item activated: {}", index);
});
```

## Example

```rust
use rust_widgets::*;
use rust_widgets::platform;

fn create_country_selector(parent: ObjectId) {
    let label = create_label(parent, "Country:", 10, 10, 80, 25);
    
    let combo = create_combo_box(parent, 100, 10, 200, 25);
    
    // Add countries
    platform::combo_box_add_item(combo, "United States");
    platform::combo_box_add_item(combo, "United Kingdom");
    platform::combo_box_add_item(combo, "Canada");
    platform::combo_box_add_item(combo, "Australia");
    platform::combo_box_add_item(combo, "Germany");
    platform::combo_box_add_item(combo, "France");
    platform::combo_box_add_item(combo, "Japan");
    
    // Set default selection
    platform::combo_box_set_current_index(combo, 0);
    
    // Connect signal
    platform::connect_current_index_changed(combo, |index| {
        let country = platform::combo_box_item_text(combo, index);
        println!("Selected: {}", country);
    });
}
```

## Platform Notes

### Windows
- Native COMBOBOX control
- Supports owner-draw for custom items

### macOS
- Native NSPopUpButton
- Supports automatic sizing

### Linux
- Native GTK ComboBox
- Supports cell renderers

## Best Practices

1. **Sort items** alphabetically when appropriate
2. **Provide a default selection**
3. **Use editable combo** for user input with suggestions
4. **Limit visible items** for long lists
5. **Consider alternatives** for very long lists (use ListView)

## See Also

- [ListBox](listbox.md) - Static list selection
- [ListView](../advanced/list-view.md) - Advanced list with columns
