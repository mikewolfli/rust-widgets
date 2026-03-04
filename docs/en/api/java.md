# Java API Reference

This document provides a reference for the Java API of the rust_widgets library.

## Installation

Add the following dependency to your `pom.xml` file:

```xml
<dependency>
    <groupId>com.rustwidgets</groupId>
    <artifactId>rust_widgets</artifactId>
    <version>1.0.0</version>
</dependency>
```

## Core Functions

### Initialization and Runtime

```java
import com.rustwidgets.RustWidgets;

// Initialize the library
RustWidgets.init();

// Run the event loop
RustWidgets.run();

// Quit the application
RustWidgets.quit();
```

### Widget Creation

```java
// Create a window
long window = RustWidgets.createWindow("Window Title", 100, 100, 800, 600);

// Create a button
long button = RustWidgets.createButton(window, "Click Me", 100, 100, 100, 30);

// Create a checkbox
long checkbox = RustWidgets.createCheckbox(window, "Check Me", 100, 150, 100, 30);

// Create a label
long label = RustWidgets.createLabel(window, "Hello World", 100, 200, 200, 30);

// Create a line edit
long lineEdit = RustWidgets.createLineEdit(window, "Enter text", 100, 250, 200, 30);

// Create a radio button
long radioButton = RustWidgets.createRadioButton(window, "Option 1", 100, 300, 100, 30);

// Create a slider
long slider = RustWidgets.createSlider(window, 100, 350, 200, 30);

// Create a progress bar
long progressBar = RustWidgets.createProgressBar(window, 100, 400, 200, 30);

// Create a combo box
long comboBox = RustWidgets.createComboBox(window, 100, 450, 200, 30);

// Create a list box
long listBox = RustWidgets.createListBox(window, 100, 500, 200, 100);

// Create a panel
long panel = RustWidgets.createPanel(window, 350, 100, 400, 400);

// Create a message box
long messageBox = RustWidgets.createMessageBox(window, "Message", "Hello from rust_widgets", 200, 200, 400, 200);

// Create a file dialog
long fileDialog = RustWidgets.createFileDialog(window, 200, 200, 600, 400);

// Create a color dialog
long colorDialog = RustWidgets.createColorDialog(window, 200, 200, 400, 400);

// Create a font dialog
long fontDialog = RustWidgets.createFontDialog(window, 200, 200, 400, 400);

// Create a spin box
long spinBox = RustWidgets.createSpinBox(window, 100, 500, 100, 30);

// Create a list view
long listView = RustWidgets.createListView(window, 100, 550, 200, 100);

// Create a scroll area
long scrollArea = RustWidgets.createScrollArea(window, 100, 550, 200, 100);

// Create a menu bar
long menuBar = RustWidgets.createMenuBar(window, 0, 0, 800, 30);

// Create a menu
long menu = RustWidgets.createMenu(menuBar, "File", 0, 0, 100, 30);

// Create a tool bar
long toolBar = RustWidgets.createToolBar(window, 0, 30, 800, 40);

// Create a status bar
long statusBar = RustWidgets.createStatusBar(window, "Ready", 0, 560, 800, 40);
```

### Widget Manipulation

```java
// Show a widget
RustWidgets.showWidget(button);

// Hide a widget
RustWidgets.hideWidget(button);

// Set widget geometry
RustWidgets.setWidgetGeometry(button, 150, 150, 120, 35);

// Set widget text
RustWidgets.setWidgetText(button, "New Text");

// Get widget text
String text = RustWidgets.getWidgetText(button);
System.out.println("Button text: " + text);

// Set widget enabled state
RustWidgets.setWidgetEnabled(button, true);  // Enable
RustWidgets.setWidgetEnabled(button, false); // Disable

// Check if widget is enabled
boolean enabled = RustWidgets.isWidgetEnabled(button);
System.out.println("Button enabled: " + enabled);

// Set widget visible state
RustWidgets.setWidgetVisible(button, true);  // Show
RustWidgets.setWidgetVisible(button, false); // Hide

// Check if widget is visible
boolean visible = RustWidgets.isWidgetVisible(button);
System.out.println("Button visible: " + visible);
```

### ComboBox Operations

```java
// Add an item to a combo box
RustWidgets.comboBoxAddItem(comboBox, "Option 1");
RustWidgets.comboBoxAddItem(comboBox, "Option 2");

// Clear items from a combo box
RustWidgets.comboBoxClearItems(comboBox);

// Set current index of a combo box
RustWidgets.comboBoxSetCurrentIndex(comboBox, 0);

// Get current index of a combo box
int index = RustWidgets.comboBoxCurrentIndex(comboBox);
System.out.println("Current index: " + index);

// Get item count of a combo box
int count = RustWidgets.comboBoxItemCount(comboBox);
System.out.println("Item count: " + count);

// Get item text of a combo box
String text = RustWidgets.comboBoxItemText(comboBox, 0);
System.out.println("Item text: " + text);
```

### ListBox Operations

```java
// Add an item to a list box
RustWidgets.listBoxAddItem(listBox, "Item 1");
RustWidgets.listBoxAddItem(listBox, "Item 2");

// Remove an item from a list box
RustWidgets.listBoxRemoveItem(listBox, 0);

// Clear items from a list box
RustWidgets.listBoxClearItems(listBox);

// Set current index of a list box
RustWidgets.listBoxSetCurrentIndex(listBox, 0);

// Get current index of a list box
int index = RustWidgets.listBoxCurrentIndex(listBox);
System.out.println("Current index: " + index);

// Get item count of a list box
int count = RustWidgets.listBoxItemCount(listBox);
System.out.println("Item count: " + count);

// Get item text of a list box
String text = RustWidgets.listBoxItemText(listBox, 0);
System.out.println("Item text: " + text);
```

### Event Handling

```java
// Poll widget trigger event
WidgetTriggerEvent event = RustWidgets.pollWidgetTriggerEvent();
if (event != null) {
    System.out.println("Widget " + event.getWidgetId() + " triggered with kind " + event.getKind());
}

// Inject widget trigger event
RustWidgets.injectWidgetTriggerEvent(button, WidgetTriggerKind.CLICKED);

// Poll widget triggered
long widgetId = RustWidgets.pollWidgetTriggered();
if (widgetId != 0) {
    System.out.println("Widget " + widgetId + " triggered");
}
```

### Clipboard Operations

```java
// Set clipboard text
RustWidgets.setClipboardText("Hello from Java");

// Get clipboard text
String text = RustWidgets.getClipboardText();
System.out.println("Clipboard text: " + text);
```

### Menu Operations

```java
// Attach menu bar to window
RustWidgets.attachMenuBarToWindow(window, menuBar);

// Add item to menu
long menuItem = RustWidgets.menuAddItem(menu, "New", "Ctrl+N");

// Poll menu triggered
long menuItemId = RustWidgets.pollMenuTriggered();
if (menuItemId != 0) {
    System.out.println("Menu item " + menuItemId + " triggered");
}

// Inject menu trigger
RustWidgets.injectMenuTrigger(menuItem);
```

### Drag and Drop

```java
// Begin drag operation
byte[] payload = "Drag data".getBytes();
RustWidgets.beginDrag(sourceWidget, "text/plain", payload);

// Poll drop event
DropEvent event = RustWidgets.pollDropEvent();
if (event != null) {
    System.out.println("Drop event from " + event.getSourceWidgetId() + " to " + event.getTargetWidgetId());
    System.out.println("MIME type: " + event.getMime());
    System.out.println("Payload: " + new String(event.getPayload()));
}

// Inject drop event
DropEvent dropEvent = new DropEvent(
    sourceWidget,
    targetWidget,
    "text/plain",
    "Drop data".getBytes()
);
RustWidgets.injectDropEvent(dropEvent);
```

### IME and Accessibility

```java
// Set widget IME enabled
RustWidgets.setWidgetImeEnabled(lineEdit, true);

// Check if widget IME is enabled
boolean imeEnabled = RustWidgets.isWidgetImeEnabled(lineEdit);
System.out.println("IME enabled: " + imeEnabled);

// Set widget accessibility name
RustWidgets.setWidgetAccessibilityName(button, "Click me button");

// Get widget accessibility name
String name = RustWidgets.getWidgetAccessibilityName(button);
System.out.println("Accessibility name: " + name);
```

### Platform Information

```java
// Get runtime GUI mode
RuntimeGuiMode mode = RustWidgets.runtimeGuiMode();
System.out.println("Runtime GUI mode: " + mode);

// Get DPI scale factor
float scale = RustWidgets.dpiScaleFactor();
System.out.println("DPI scale factor: " + scale);

// Get platform capabilities
PlatformCapabilities capabilities = RustWidgets.capabilities();
System.out.println("DPI scaling: " + capabilities.isDpiScaling());
System.out.println("IME: " + capabilities.isIme());
System.out.println("Accessibility: " + capabilities.isAccessibility());
System.out.println("Native menu: " + capabilities.isNativeMenu());
System.out.println("Typed widget trigger: " + capabilities.isTypedWidgetTrigger());
```

## Enums

### WidgetTriggerKind

```java
WidgetTriggerKind.UNKNOWN
WidgetTriggerKind.CLICKED
WidgetTriggerKind.VALUE_CHANGED
WidgetTriggerKind.SELECTION_CHANGED
WidgetTriggerKind.CLOSED
```

### RuntimeGuiMode

```java
RuntimeGuiMode.NATIVE_INTERACTIVE
RuntimeGuiMode.PREVIEW_OR_STUB
```

### DockPosition

```java
DockPosition.LEFT
DockPosition.RIGHT
DockPosition.TOP
DockPosition.BOTTOM
DockPosition.CENTER
```

## Data Models

### VecTreeModel

```java
import com.rustwidgets.VecTreeModel;

// Create a new tree model
VecTreeModel model = new VecTreeModel();

// Add items to the model
model.addItem("Root", null);
model.addItem("Child", "Root");
model.addItem("Grandchild", "Root/Child");
```

### VecListModel

```java
import com.rustwidgets.VecListModel;

// Create a new list model
String[] items = {"Item 1", "Item 2", "Item 3"};
VecListModel model = new VecListModel(items);
```

## Example

```java
import com.rustwidgets.RustWidgets;
import com.rustwidgets.WidgetTriggerKind;
import com.rustwidgets.WidgetTriggerEvent;

public class Main {
    public static void main(String[] args) {
        // Initialize the library
        RustWidgets.init();
        
        // Create a window
        long window = RustWidgets.createWindow("Java Example", 100, 100, 600, 400);
        
        // Create a button
        long button = RustWidgets.createButton(window, "Click Me", 250, 150, 100, 30);
        
        // Show the window
        RustWidgets.showWidget(window);
        
        // Main loop
        while (true) {
            // Poll for events
            WidgetTriggerEvent event = RustWidgets.pollWidgetTriggerEvent();
            if (event != null && event.getWidgetId() == button && event.getKind() == WidgetTriggerKind.CLICKED) {
                System.out.println("Button clicked!");
                RustWidgets.setWidgetText(button, "Clicked!");
            }
            
            // Run the event loop
            RustWidgets.run();
        }
    }
}
```