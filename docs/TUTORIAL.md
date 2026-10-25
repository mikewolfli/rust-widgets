# Rust Widgets — Getting Started Tutorial

> Learn how to build a simple GUI application using rust_widgets.

## Prerequisites

- Rust 1.88 or newer
- A platform backend: macOS (Cocoa), Windows (Win32), Linux (GTK or Wayland), or the
  mobile backends (Android, iOS, HarmonyOS)

## Step 1: Add Dependency

```toml
[dependencies]
rust_widgets = "2.9.2"
```

## Step 2: Create a Window

```rust
use rust_widgets::app::App;

fn main() {
    let mut app = App::new();
    app.init();
    let window = app.new_window("Hello, Widgets!", 100, 100, 400, 300);
    window.set_title("Hello, Widgets!");
    app.run();
}
```

## Step 3: Add a Button

```rust
use rust_widgets::app::App;

fn main() {
    let mut app = App::new();
    app.init();
    let window = app.new_window("Hello, Widgets!", 100, 100, 400, 300);

    let button = window.new_button("Click me", 20, 20, 120, 36);
    button.on_click(|| println!("Button clicked!"));

    app.run();
}
```

## Step 4: Try a Switch Widget

```rust
use rust_widgets::core::Rect;
use rust_widgets::widget::Switch;

fn main() {
    let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
    // `connect_ref` hands the callback `&T`, so the emit allocates no `Arc`.
    sw.toggled.connect_ref(|checked: &bool| println!("Switch toggled: {checked}"));
    let svg = rust_widgets::widget::svg::render_to_svg(&mut sw);
    println!("switch svg bytes={}", svg.len());
}
```

## Step 5: Mobile Widgets

Mobile-first widgets are available in the default build:

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

## Step 6: Restore State on the Next Launch

Restoring a session is explicit. A control opts in by implementing `SaveState`; the host decides
which mounted instance a key names, because runtime object ids do not survive a restart:

```rust
use rust_widgets::{collect_save_state, restore_save_state, SavedState};

let key_of = |id| {
    if id == splitter_id { Some("workspace.split".to_string()) }
    else if id == pagination_id { Some("table.pager".to_string()) }
    else { None }
};

let saved: Vec<SavedState> = collect_save_state(window_id, key_of);
// Write `saved` with the host's own settings store, then on the next launch:
let applied = restore_save_state(window_id, &saved, key_of);
```

The walk covers tree children and surfaces mounted under the window. Each entry carries the
control's format version, so a format change refuses old bytes instead of misreading them. Ready
adopters: `Splitter`, `TabWidget`, `DockWidget`, `CollapsiblePane` and `Pagination`.

## Full Examples

Two places hold runnable code, and they answer different questions:

**`demo/` — complete applications.** These open a real window and exercise the whole
stack (app framework, layout, events, theming). Start here to see the library used the
way an application uses it:

- `demo/control` — the control gallery: inputs, containers, a pagination bar with jump and
  page-size cells, an eased bar chart, and an in-memory save/restore demonstration
- `demo/code_editor` — project tree plus editor, wired through the layout managers
- `demo/finance` — candlestick / depth / order-book widgets over live-shaped data

Run one with `cd demo/control && cargo run`.

**`examples/` — focused, single-purpose programs.** These are small enough to read in
one sitting and are what CI builds and runs:

- `examples/demo_button.rs` — one control, and the only example that also builds on the
  `embedded` profile (the `demo/` projects require `gtk-native`, so they cannot)
- `examples/view_counter.rs` — the declarative-retained loop end to end
- `examples/control_property_uniform.rs` — the property contract across controls
- `examples/widget_reachability.rs` — machine-readable registry report, consumed by the
  registration-fidelity gate

## Next Steps

- Read `docs/ARCHITECTURE.md` for the system design
- Check `CHANGELOG.md` for version history
- Browse the widget gallery in `docs/reports/`
