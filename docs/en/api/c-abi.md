# C ABI Reference

This document provides a reference for the C ABI of the rust_widgets library.

## Core Functions

### Initialization and Runtime

```c
// Initialize the library
void rust_widgets_init();

// Run the event loop
void rust_widgets_run();

// Quit the application
void rust_widgets_quit();
```

### Widget Creation

```c
// Create a window
ObjectId rust_widgets_create_window(const char* title, int x, int y, unsigned int width, unsigned int height);

// Create a button
ObjectId rust_widgets_create_button(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a checkbox
ObjectId rust_widgets_create_checkbox(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a label
ObjectId rust_widgets_create_label(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a line edit
ObjectId rust_widgets_create_line_edit(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a radio button
ObjectId rust_widgets_create_radio_button(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a slider
ObjectId rust_widgets_create_slider(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a progress bar
ObjectId rust_widgets_create_progress_bar(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a combo box
ObjectId rust_widgets_create_combo_box(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a list box
ObjectId rust_widgets_create_list_box(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a panel
ObjectId rust_widgets_create_panel(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a message box
ObjectId rust_widgets_create_message_box(ObjectId parent, const char* title, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a file dialog
ObjectId rust_widgets_create_file_dialog(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a color dialog
ObjectId rust_widgets_create_color_dialog(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a font dialog
ObjectId rust_widgets_create_font_dialog(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a spin box
ObjectId rust_widgets_create_spin_box(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a list view
ObjectId rust_widgets_create_list_view(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a scroll area
ObjectId rust_widgets_create_scroll_area(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a menu bar
ObjectId rust_widgets_create_menu_bar(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a menu
ObjectId rust_widgets_create_menu(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);

// Create a tool bar
ObjectId rust_widgets_create_tool_bar(ObjectId parent, int x, int y, unsigned int width, unsigned int height);

// Create a status bar
ObjectId rust_widgets_create_status_bar(ObjectId parent, const char* text, int x, int y, unsigned int width, unsigned int height);
```

### Widget Manipulation

```c
// Show a widget
void rust_widgets_show_widget(ObjectId widget_id);

// Hide a widget
void rust_widgets_hide_widget(ObjectId widget_id);

// Set widget geometry
void rust_widgets_set_widget_geometry(ObjectId widget_id, int x, int y, unsigned int width, unsigned int height);

// Set widget text
void rust_widgets_set_widget_text(ObjectId widget_id, const char* text);

// Get widget text
const char* rust_widgets_get_widget_text(ObjectId widget_id);

// Set widget enabled state
void rust_widgets_set_widget_enabled(ObjectId widget_id, bool enabled);

// Check if widget is enabled
bool rust_widgets_is_widget_enabled(ObjectId widget_id);

// Set widget visible state
void rust_widgets_set_widget_visible(ObjectId widget_id, bool visible);

// Check if widget is visible
bool rust_widgets_is_widget_visible(ObjectId widget_id);
```

### ComboBox Operations

```c
// Add an item to a combo box
bool rust_widgets_combo_box_add_item(ObjectId combo_box, const char* text);

// Clear items from a combo box
bool rust_widgets_combo_box_clear_items(ObjectId combo_box);

// Set current index of a combo box
bool rust_widgets_combo_box_set_current_index(ObjectId combo_box, size_t index);

// Get current index of a combo box
int rust_widgets_combo_box_current_index(ObjectId combo_box);

// Get item count of a combo box
size_t rust_widgets_combo_box_item_count(ObjectId combo_box);

// Get item text of a combo box
const char* rust_widgets_combo_box_item_text(ObjectId combo_box, size_t index);
```

### ListBox Operations

```c
// Add an item to a list box
bool rust_widgets_list_box_add_item(ObjectId list_box, const char* text);

// Remove an item from a list box
bool rust_widgets_list_box_remove_item(ObjectId list_box, size_t index);

// Clear items from a list box
bool rust_widgets_list_box_clear_items(ObjectId list_box);

// Set current index of a list box
bool rust_widgets_list_box_set_current_index(ObjectId list_box, size_t index);

// Get current index of a list box
int rust_widgets_list_box_current_index(ObjectId list_box);

// Get item count of a list box
size_t rust_widgets_list_box_item_count(ObjectId list_box);

// Get item text of a list box
const char* rust_widgets_list_box_item_text(ObjectId list_box, size_t index);
```

### Event Handling

```c
// Poll widget trigger event
bool rust_widgets_poll_widget_trigger_event(WidgetTriggerEvent* event);

// Inject widget trigger event
bool rust_widgets_inject_widget_trigger_event(ObjectId widget_id, WidgetTriggerKind kind);

// Poll widget triggered
ObjectId rust_widgets_poll_widget_triggered();
```

### Clipboard Operations

```c
// Set clipboard text
bool rust_widgets_set_clipboard_text(const char* text);

// Get clipboard text
const char* rust_widgets_get_clipboard_text();
```

### Menu Operations

```c
// Attach menu bar to window
bool rust_widgets_attach_menu_bar_to_window(ObjectId window, ObjectId menu_bar);

// Add item to menu
ObjectId rust_widgets_menu_add_item(ObjectId parent_menu, const char* text, const char* shortcut);

// Poll menu triggered
ObjectId rust_widgets_poll_menu_triggered();

// Inject menu trigger
bool rust_widgets_inject_menu_trigger(ObjectId menu_item_id);
```

### Drag and Drop

```c
// Begin drag operation
bool rust_widgets_begin_drag(ObjectId source_widget_id, const char* mime, const uint8_t* payload, size_t payload_size);

// Poll drop event
bool rust_widgets_poll_drop_event(DropEvent* event);

// Inject drop event
bool rust_widgets_inject_drop_event(const DropEvent* event);
```

### IME and Accessibility

```c
// Set widget IME enabled
bool rust_widgets_set_widget_ime_enabled(ObjectId widget_id, bool enabled);

// Check if widget IME is enabled
bool rust_widgets_is_widget_ime_enabled(ObjectId widget_id);

// Set widget accessibility name
bool rust_widgets_set_widget_accessibility_name(ObjectId widget_id, const char* name);

// Get widget accessibility name
const char* rust_widgets_get_widget_accessibility_name(ObjectId widget_id);
```

### Platform Information

```c
// Get runtime GUI mode
RuntimeGuiMode rust_widgets_runtime_gui_mode();

// Get DPI scale factor
float rust_widgets_dpi_scale_factor();

// Get platform capabilities
void rust_widgets_capabilities(PlatformCapabilities* capabilities);
```

## Types

### ObjectId

```c
typedef uint64_t ObjectId;
```

### WidgetTriggerKind

```c
enum WidgetTriggerKind {
    WIDGET_TRIGGER_KIND_UNKNOWN = 0,
    WIDGET_TRIGGER_KIND_CLICKED = 1,
    WIDGET_TRIGGER_KIND_VALUE_CHANGED = 2,
    WIDGET_TRIGGER_KIND_SELECTION_CHANGED = 3,
    WIDGET_TRIGGER_KIND_CLOSED = 4,
};
typedef enum WidgetTriggerKind WidgetTriggerKind;
```

### RuntimeGuiMode

```c
enum RuntimeGuiMode {
    RUNTIME_GUI_MODE_NATIVE_INTERACTIVE,
    RUNTIME_GUI_MODE_PREVIEW_OR_STUB,
};
typedef enum RuntimeGuiMode RuntimeGuiMode;
```

### DockPosition

```c
enum DockPosition {
    DOCK_POSITION_LEFT,
    DOCK_POSITION_RIGHT,
    DOCK_POSITION_TOP,
    DOCK_POSITION_BOTTOM,
    DOCK_POSITION_CENTER,
};
typedef enum DockPosition DockPosition;
```

### WidgetTriggerEvent

```c
typedef struct WidgetTriggerEvent {
    ObjectId widget_id;
    WidgetTriggerKind kind;
} WidgetTriggerEvent;
```

### DropEvent

```c
typedef struct DropEvent {
    ObjectId source_widget_id;
    ObjectId target_widget_id;
    char mime[256];
    uint8_t payload[1024];
    size_t payload_size;
} DropEvent;
```

### PlatformCapabilities

```c
typedef struct PlatformCapabilities {
    bool dpi_scaling;
    bool ime;
    bool accessibility;
    bool native_menu;
    bool typed_widget_trigger;
} PlatformCapabilities;
```