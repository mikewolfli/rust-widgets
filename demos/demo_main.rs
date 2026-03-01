//! Main demo entry.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Button, Label, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "rust_widgets main demo".to_string(),
        Rect {
            x: 80,
            y: 80,
            width: 900,
            height: 600,
        },
    );

    // Create a title label for the window.
    let label = Label::new(
        "Cross-platform native GUI architecture".to_string(),
        Rect {
            x: 24,
            y: 24,
            width: 420,
            height: 32,
        },
    );

    // Create an action button.
    let button = Button::new(
        "Start".to_string(),
        Rect {
            x: 24,
            y: 72,
            width: 120,
            height: 36,
        },
    );

    window.add_child(label.id());
    window.add_child(button.id());
    // Show the main demo window and enter the event loop.
    window.show();

    run();
}
