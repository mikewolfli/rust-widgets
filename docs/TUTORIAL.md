# Rust Widgets — Getting Started Tutorial

> Learn how to build a simple GUI application using rust_widgets.

## Prerequisites

- Rust 1.87 or newer
- A platform backend: macOS (Cocoa), Windows (Win32), or Linux (GTK)

## Step 1: Add Dependency

```toml
[dependencies]
rust_widgets = "0.9"
```

## Step 2: Create a Window

```rust
use rust_widgets::app::App;

fn main() {
    let app = App::new();
    let window = app.new_window()
        .title("Hello, Widgets!")
        .size(400, 300)
        .build();
    app.run();
}
```

## Step 3: Add a Button

```rust
use rust_widgets::app::App;
use rust_widgets::widget::Button;
use rust_widgets::core::Rect;

fn main() {
    let app = App::new();
    let mut window = app.new_window()
        .title("Hello, Widgets!")
        .size(400, 300)
        .build();

    let mut button = Button::new("Click Me!".to_string(), Rect::new(50, 50, 120, 36));
    button.clicked_signal().connect(|| {
        println!("Button clicked!");
    });
    window.add_widget(Box::new(button));
    app.run();
}
```

## Step 4: Try a Switch Widget (BLUE11)

```rust
use rust_widgets::app::App;
use rust_widgets::widget::Switch;
use rust_widgets::core::Rect;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

fn main() {
    let app = App::new();
    let mut window = app.new_window()
        .title("Switch Demo")
        .size(300, 200)
        .build();

    let mut sw = Switch::new(Rect::new(50, 50, 60, 30));
    let state = Arc::new(AtomicBool::new(false));
    let s = state.clone();
    sw.toggled.connect(move |val: Arc<bool>| {
        s.store(*val, Ordering::SeqCst);
        println!("Switch toggled: {}", val);
    });
    window.add_widget(Box::new(sw));
    app.run();
}
```

## Step 5: Mobile Widgets

New in BLUE11, mobile-first widgets:

```rust
use rust_widgets::widget::{
    AppBar, BottomNavigationBar, BottomSheet, NavigationDrawer,
};
use rust_widgets::core::Rect;

// AppBar with back button and action
let mut app_bar = AppBar::new("My App", Rect::new(0, 0, 375, 56));
app_bar.set_show_back(true);
app_bar.set_action_text("Done");

// Bottom navigation bar
let mut nav_bar = BottomNavigationBar::new(Rect::new(0, 600, 375, 56));
nav_bar.add_item("🏠", "Home");
nav_bar.add_item("🔍", "Search");
nav_bar.add_item("⚙️", "Settings");
```

## Full Examples

See the `examples/` directory for complete runnable demos:
- `demo_button.rs` — Button interactions
- `demo_window.rs` — Window management
- `examples/demo_container.rs` — Layout containers

## Next Steps

- Read `docs/ARCHITECTURE.md` for the system design
- Check `CHANGELOG.md` for version history
- Browse the widget gallery in `docs/reports/`
