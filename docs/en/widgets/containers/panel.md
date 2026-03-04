# Panel

The Panel widget is a basic container that groups child widgets together.

## Creating a Panel

```rust
use rust_widgets::widget::Panel;

// Create a panel
let panel = create_panel(parent, x, y, width, height);

// With builder
let panel = Panel::new(parent)
    .position(10, 10)
    .size(400, 300)
    .auto_fill_background(true)
    .build();
```

## Properties

### Geometry

```rust
// Set position and size
set_widget_geometry(panel, x, y, width, height);

// Get geometry
let (x, y, w, h) = get_widget_geometry(panel);
```

### Background

```rust
// Set background color
set_panel_background_color(panel, Color::from_rgb(240, 240, 240));

// Enable auto-fill background
set_panel_auto_fill_background(panel, true);
```

### Border

```rust
// Set frame style
set_panel_frame_style(panel, FrameStyle::NoFrame);
set_panel_frame_style(panel, FrameStyle::Box);
set_panel_frame_style(panel, FrameStyle::Panel);
set_panel_frame_style(panel, FrameStyle::StyledPanel);

// Set frame shadow
set_panel_frame_shadow(panel, FrameShadow::Plain);
set_panel_frame_shadow(panel, FrameShadow::Raised);
set_panel_frame_shadow(panel, FrameShadow::Sunken);
```

## Layout

Panels can contain child widgets with absolute or layout-based positioning:

```rust
use rust_widgets::*;

fn create_form_panel(parent: ObjectId) -> ObjectId {
    let panel = create_panel(parent, 10, 10, 300, 200);
    
    // Add child widgets with absolute positioning
    let label = create_label(panel, "Name:", 10, 10, 80, 25);
    let input = create_line_edit(panel, "", 100, 10, 180, 25);
    
    let label2 = create_label(panel, "Email:", 10, 45, 80, 25);
    let input2 = create_line_edit(panel, "", 100, 45, 180, 25);
    
    let button = create_button(panel, "Submit", 100, 100, 100, 30);
    
    panel
}
```

## Nesting

Panels can be nested to create complex layouts:

```rust
fn create_nested_layout(parent: ObjectId) {
    // Main panel
    let main = create_panel(parent, 0, 0, 800, 600);
    
    // Header panel
    let header = create_panel(main, 0, 0, 800, 60);
    let title = create_label(header, "Application", 10, 10, 200, 40);
    
    // Content panel
    let content = create_panel(main, 0, 60, 800, 480);
    
    // Sidebar
    let sidebar = create_panel(content, 0, 0, 200, 480);
    let nav1 = create_button(sidebar, "Home", 10, 10, 180, 30);
    let nav2 = create_button(sidebar, "Settings", 10, 50, 180, 30);
    
    // Main area
    let main_area = create_panel(content, 200, 0, 600, 480);
    
    // Footer panel
    let footer = create_panel(main, 0, 540, 800, 60);
    let status = create_label(footer, "Ready", 10, 10, 200, 25);
}
```

## Platform Notes

### Windows
- Native static control or custom window
- Supports custom painting

### macOS
- Native NSView
- Supports layer backing

### Linux
- Native GTK Box or Frame
- Supports CSS styling

## Best Practices

1. **Use for visual grouping** of related controls
2. **Nest logically** to create clear hierarchies
3. **Set appropriate sizes** to avoid clipping
4. **Consider layouts** for responsive designs

## See Also

- [GroupBox](groupbox.md) - Panel with title
- [ScrollArea](scrollarea.md) - Scrollable panel
- [Splitter](splitter.md) - Resizable split panels
