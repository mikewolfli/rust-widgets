//! Label demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Label, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "Label Demo".to_string(),
        Rect { x: 120, y: 120, width: 600, height: 320 },
    );

    // Create a simple static text label.
    let label = Label::new(
        "This is a label".to_string(),
        Rect { x: 24, y: 24, width: 260, height: 32 },
    );
    window.add_child(label.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
