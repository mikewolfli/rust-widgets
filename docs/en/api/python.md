# Python API Reference

This document provides a reference for the Python API of the rust_widgets library.

## Installation

```bash
pip install rust_widgets
```

## Core Functions

### Initialization and Runtime

```python
import rust_widgets

# Initialize the library
rust_widgets.init()

# Run the event loop
rust_widgets.run()

# Quit the application
rust_widgets.quit()
```

### Widget Creation

```python
# Create a window
window = rust_widgets.create_window("Window Title", 100, 100, 800, 600)

# Create a button
button = rust_widgets.create_button(window, "Click Me", 100, 100, 100, 30)

# Create a checkbox
checkbox = rust_widgets.create_checkbox(window, "Check Me", 100, 150, 100, 30)

# Create a label
label = rust_widgets.create_label(window, "Hello World", 100, 200, 200, 30)

# Create a line edit
line_edit = rust_widgets.create_line_edit(window, "Enter text", 100, 250, 200, 30)

# Create a radio button
radio_button = rust_widgets.create_radio_button(window, "Option 1", 100, 300, 100, 30)

# Create a slider
slider = rust_widgets.create_slider(window, 100, 350, 200, 30)

# Create a progress bar
progress_bar = rust_widgets.create_progress_bar(window, 100, 400, 200, 30)

# Create a combo box
combo_box = rust_widgets.create_combo_box(window, 100, 450, 200, 30)

# Create a list box
list_box = rust_widgets.create_list_box(window, 100, 500, 200, 100)

# Create a panel
panel = rust_widgets.create_panel(window, 350, 100, 400, 400)

# Create a message box
message_box = rust_widgets.create_message_box(window, "Message", "Hello from rust_widgets", 200, 200, 400, 200)

# Create a file dialog
file_dialog = rust_widgets.create_file_dialog(window, 200, 200, 600, 400)

# Create a color dialog
color_dialog = rust_widgets.create_color_dialog(window, 200, 200, 400, 400)

# Create a font dialog
font_dialog = rust_widgets.create_font_dialog(window, 200, 200, 400, 400)

# Create a spin box
spin_box = rust_widgets.create_spin_box(window, 100, 500, 100, 30)

# Create a list view
list_view = rust_widgets.create_list_view(window, 100, 550, 200, 100)

# Create a scroll area
scroll_area = rust_widgets.create_scroll_area(window, 100, 550, 200, 100)

# Create a menu bar
menu_bar = rust_widgets.create_menu_bar(window, 0, 0, 800, 30)

# Create a menu
menu = rust_widgets.create_menu(menu_bar, "File", 0, 0, 100, 30)

# Create a tool bar
tool_bar = rust_widgets.create_tool_bar(window, 0, 30, 800, 40)

# Create a status bar
status_bar = rust_widgets.create_status_bar(window, "Ready", 0, 560, 800, 40)
```

### Widget Manipulation

```python
# Show a widget
rust_widgets.show_widget(button)

# Hide a widget
rust_widgets.hide_widget(button)

# Set widget geometry
rust_widgets.set_widget_geometry(button, 150, 150, 120, 35)

# Set widget text
rust_widgets.set_widget_text(button, "New Text")

# Get widget text
text = rust_widgets.get_widget_text(button)
print(f"Button text: {text}")

# Set widget enabled state
rust_widgets.set_widget_enabled(button, True)  # Enable
rust_widgets.set_widget_enabled(button, False) # Disable

# Check if widget is enabled
enabled = rust_widgets.is_widget_enabled(button)
print(f"Button enabled: {enabled}")

# Set widget visible state
rust_widgets.set_widget_visible(button, True)  # Show
rust_widgets.set_widget_visible(button, False) # Hide

# Check if widget is visible
visible = rust_widgets.is_widget_visible(button)
print(f"Button visible: {visible}")
```

### ComboBox Operations

```python
# Add an item to a combo box
rust_widgets.combo_box_add_item(combo_box, "Option 1")
rust_widgets.combo_box_add_item(combo_box, "Option 2")

# Clear items from a combo box
rust_widgets.combo_box_clear_items(combo_box)

# Set current index of a combo box
rust_widgets.combo_box_set_current_index(combo_box, 0)

# Get current index of a combo box
index = rust_widgets.combo_box_current_index(combo_box)
print(f"Current index: {index}")

# Get item count of a combo box
count = rust_widgets.combo_box_item_count(combo_box)
print(f"Item count: {count}")

# Get item text of a combo box
text = rust_widgets.combo_box_item_text(combo_box, 0)
print(f"Item text: {text}")
```

### ListBox Operations

```python
# Add an item to a list box
rust_widgets.list_box_add_item(list_box, "Item 1")
rust_widgets.list_box_add_item(list_box, "Item 2")

# Remove an item from a list box
rust_widgets.list_box_remove_item(list_box, 0)

# Clear items from a list box
rust_widgets.list_box_clear_items(list_box)

# Set current index of a list box
rust_widgets.list_box_set_current_index(list_box, 0)

# Get current index of a list box
index = rust_widgets.list_box_current_index(list_box)
print(f"Current index: {index}")

# Get item count of a list box
count = rust_widgets.list_box_item_count(list_box)
print(f"Item count: {count}")

# Get item text of a list box
text = rust_widgets.list_box_item_text(list_box, 0)
print(f"Item text: {text}")
```

### Event Handling

```python
# Poll widget trigger event
event = rust_widgets.poll_widget_trigger_event()
if event:
    print(f"Widget {event.widget_id} triggered with kind {event.kind}")

# Inject widget trigger event
rust_widgets.inject_widget_trigger_event(button, rust_widgets.WidgetTriggerKind.CLICKED)

# Poll widget triggered
widget_id = rust_widgets.poll_widget_triggered()
if widget_id:
    print(f"Widget {widget_id} triggered")
```

### Clipboard Operations

```python
# Set clipboard text
rust_widgets.set_clipboard_text("Hello from Python")

# Get clipboard text
text = rust_widgets.get_clipboard_text()
print(f"Clipboard text: {text}")
```

### Menu Operations

```python
# Attach menu bar to window
rust_widgets.attach_menu_bar_to_window(window, menu_bar)

# Add item to menu
menu_item = rust_widgets.menu_add_item(menu, "New", "Ctrl+N")

# Poll menu triggered
menu_item_id = rust_widgets.poll_menu_triggered()
if menu_item_id:
    print(f"Menu item {menu_item_id} triggered")

# Inject menu trigger
rust_widgets.inject_menu_trigger(menu_item)
```

### Drag and Drop

```python
# Begin drag operation
rust_widgets.begin_drag(source_widget, "text/plain", b"Drag data")

# Poll drop event
event = rust_widgets.poll_drop_event()
if event:
    print(f"Drop event from {event.source_widget_id} to {event.target_widget_id}")
    print(f"MIME type: {event.mime}")
    print(f"Payload: {event.payload}")

# Inject drop event
event = rust_widgets.DropEvent(
    source_widget_id=source_widget,
    target_widget_id=target_widget,
    mime="text/plain",
    payload=b"Drop data"
)
rust_widgets.inject_drop_event(event)
```

### IME and Accessibility

```python
# Set widget IME enabled
rust_widgets.set_widget_ime_enabled(line_edit, True)

# Check if widget IME is enabled
ime_enabled = rust_widgets.is_widget_ime_enabled(line_edit)
print(f"IME enabled: {ime_enabled}")

# Set widget accessibility name
rust_widgets.set_widget_accessibility_name(button, "Click me button")

# Get widget accessibility name
name = rust_widgets.get_widget_accessibility_name(button)
print(f"Accessibility name: {name}")
```

### Platform Information

```python
# Get runtime GUI mode
mode = rust_widgets.runtime_gui_mode()
print(f"Runtime GUI mode: {mode}")

# Get DPI scale factor
scale = rust_widgets.dpi_scale_factor()
print(f"DPI scale factor: {scale}")

# Get platform capabilities
capabilities = rust_widgets.capabilities()
print(f"DPI scaling: {capabilities.dpi_scaling}")
print(f"IME: {capabilities.ime}")
print(f"Accessibility: {capabilities.accessibility}")
print(f"Native menu: {capabilities.native_menu}")
print(f"Typed widget trigger: {capabilities.typed_widget_trigger}")
```

## Enums

### WidgetTriggerKind

```python
rust_widgets.WidgetTriggerKind.UNKNOWN
rust_widgets.WidgetTriggerKind.CLICKED
rust_widgets.WidgetTriggerKind.VALUE_CHANGED
rust_widgets.WidgetTriggerKind.SELECTION_CHANGED
rust_widgets.WidgetTriggerKind.CLOSED
```

### RuntimeGuiMode

```python
rust_widgets.RuntimeGuiMode.NATIVE_INTERACTIVE
rust_widgets.RuntimeGuiMode.PREVIEW_OR_STUB
```

### DockPosition

```python
rust_widgets.DockPosition.LEFT
rust_widgets.DockPosition.RIGHT
rust_widgets.DockPosition.TOP
rust_widgets.DockPosition.BOTTOM
rust_widgets.DockPosition.CENTER
```

## Data Models

### VecTreeModel

```python
from rust_widgets import VecTreeModel

# Create a new tree model
model = VecTreeModel()

# Add items to the model
model.add_item("Root", None)
model.add_item("Child", "Root")
model.add_item("Grandchild", "Root/Child")
```

### VecListModel

```python
from rust_widgets import VecListModel

# Create a new list model
model = VecListModel(["Item 1", "Item 2", "Item 3"])
```

## Example

```python
import rust_widgets

# Initialize the library
rust_widgets.init()

# Create a window
window = rust_widgets.create_window("Python Example", 100, 100, 600, 400)

# Create a button
button = rust_widgets.create_button(window, "Click Me", 250, 150, 100, 30)

# Show the window
rust_widgets.show_widget(window)

# Main loop
while True:
    # Poll for events
    event = rust_widgets.poll_widget_trigger_event()
    if event and event.widget_id == button and event.kind == rust_widgets.WidgetTriggerKind.CLICKED:
        print("Button clicked!")
        rust_widgets.set_widget_text(button, "Clicked!")
    
    # Run the event loop
    rust_widgets.run()
```