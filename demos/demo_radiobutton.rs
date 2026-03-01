//! RadioButton demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{RadioButton, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "RadioButton Demo".to_string(),
        Rect { x: 120, y: 120, width: 600, height: 320 },
    );

    // Create and select a radio button.
    let mut radio = RadioButton::new(Rect { x: 24, y: 24, width: 220, height: 32 });
    radio.set_checked(true);
    window.add_child(radio.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
