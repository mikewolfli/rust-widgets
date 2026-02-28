//! Main demo entry.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Button, Label, Widget, Window};
use rust_widgets::{init, run};

fn main() {
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

    let label = Label::new(
        "Cross-platform native GUI architecture".to_string(),
        Rect {
            x: 24,
            y: 24,
            width: 420,
            height: 32,
        },
    );

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
    window.show();

    run();
}
