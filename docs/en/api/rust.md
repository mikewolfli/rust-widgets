# Rust API Reference

This document provides a reference for the Rust API of the rust_widgets library.

## Core Functions

### Initialization and Runtime

```rust
// Initialize the library
pub fn init();

// Run the event loop
pub fn run();

// Quit the application
pub fn quit();
```

### Widget Creation

```rust
// Create a window
pub fn create_window(title: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a button
pub fn create_button(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a checkbox
pub fn create_checkbox(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a label
pub fn create_label(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a line edit
pub fn create_line_edit(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a radio button
pub fn create_radio_button(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a slider
pub fn create_slider(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a progress bar
pub fn create_progress_bar(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a combo box
pub fn create_combo_box(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a list box
pub fn create_list_box(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a panel
pub fn create_panel(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a message box
pub fn create_message_box(parent: crate::core::ObjectId, title: &str, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a file dialog
pub fn create_file_dialog(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a color dialog
pub fn create_color_dialog(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a font dialog
pub fn create_font_dialog(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a spin box
pub fn create_spin_box(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a list view
pub fn create_list_view(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a scroll area
pub fn create_scroll_area(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a menu bar
pub fn create_menu_bar(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a menu
pub fn create_menu(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a tool bar
pub fn create_tool_bar(parent: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;

// Create a status bar
pub fn create_status_bar(parent: crate::core::ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> crate::core::ObjectId;
```

### Widget Manipulation

```rust
// Show a widget
pub fn show_widget(widget_id: crate::core::ObjectId);

// Hide a widget
pub fn hide_widget(widget_id: crate::core::ObjectId);

// Set widget geometry
pub fn set_widget_geometry(widget_id: crate::core::ObjectId, x: i32, y: i32, width: u32, height: u32);

// Set widget text
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str);

// Get widget text
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String;

// Set widget enabled state
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool);

// Check if widget is enabled
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool;

// Set widget visible state
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool);

// Check if widget is visible
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool;
```

### ComboBox Operations

```rust
// Add an item to a combo box
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool;

// Clear items from a combo box
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool;

// Set current index of a combo box
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool;

// Get current index of a combo box
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize>;

// Get item count of a combo box
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize;

// Get item text of a combo box
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String>;
```

### ListBox Operations

```rust
// Add an item to a list box
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool;

// Remove an item from a list box
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool;

// Clear items from a list box
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool;

// Set current index of a list box
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool;

// Get current index of a list box
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize>;

// Get item count of a list box
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize;

// Get item text of a list box
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String>;
```

### Event Handling

```rust
// Poll widget trigger event
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent>;

// Inject widget trigger event
pub fn inject_widget_trigger_event(widget_id: crate::core::ObjectId, kind: WidgetTriggerKind) -> bool;

// Poll widget triggered
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId>;
```

### Clipboard Operations

```rust
// Set clipboard text
pub fn set_clipboard_text(text: &str) -> bool;

// Get clipboard text
pub fn get_clipboard_text() -> String;
```

### Menu Operations

```rust
// Attach menu bar to window
pub fn attach_menu_bar_to_window(window: crate::core::ObjectId, menu_bar: crate::core::ObjectId) -> bool;

// Add item to menu
pub fn menu_add_item(parent_menu: crate::core::ObjectId, text: &str, shortcut: Option<&str>) -> crate::core::ObjectId;

// Poll menu triggered
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId>;

// Inject menu trigger
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool;
```

### Drag and Drop

```rust
// Begin drag operation
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool;

// Poll drop event
pub fn poll_drop_event() -> Option<DropEvent>;

// Inject drop event
pub fn inject_drop_event(event: DropEvent) -> bool;
```

### IME and Accessibility

```rust
// Set widget IME enabled
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool;

// Check if widget IME is enabled
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool;

// Set widget accessibility name
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool;

// Get widget accessibility name
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String;
```

### Platform Information

```rust
// Get runtime GUI mode
pub fn runtime_gui_mode() -> RuntimeGuiMode;

// Get DPI scale factor
pub fn dpi_scale_factor() -> f32;

// Get platform capabilities
pub fn capabilities() -> PlatformCapabilities;
```

## Data Models

### VecTreeModel

```rust
use rust_widgets::widget::VecTreeModel;

// Create a new tree model
let mut model = VecTreeModel::new();

// Add items to the model
model.add_item("Root", None);
model.add_item("Child", Some("Root"));
model.add_item("Grandchild", Some("Root/Child"));
```

### VecListModel

```rust
use rust_widgets::widget::VecListModel;

// Create a new list model
let model = VecListModel::new(vec!["Item 1", "Item 2", "Item 3"]);
```

## Enums

### WidgetTriggerKind

```rust
pub enum WidgetTriggerKind {
    Unknown = 0,
    Clicked = 1,
    ValueChanged = 2,
    SelectionChanged = 3,
    Closed = 4,
}
```

### RuntimeGuiMode

```rust
pub enum RuntimeGuiMode {
    NativeInteractive,
    PreviewOrStub,
}
```

### DockPosition

```rust
pub enum DockPosition {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}
```

## Structures

### WidgetTriggerEvent

```rust
pub struct WidgetTriggerEvent {
    pub widget_id: ObjectId,
    pub kind: WidgetTriggerKind,
}
```

### DropEvent

```rust
pub struct DropEvent {
    pub source_widget_id: ObjectId,
    pub target_widget_id: ObjectId,
    pub mime: String,
    pub payload: Vec<u8>,
}
```

### PlatformCapabilities

```rust
pub struct PlatformCapabilities {
    pub dpi_scaling: bool,
    pub ime: bool,
    pub accessibility: bool,
    pub native_menu: bool,
    pub typed_widget_trigger: bool,
}
```