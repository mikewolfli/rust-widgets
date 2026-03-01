//! CheckBox demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{CheckBox, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "CheckBox Demo".to_string(),
        Rect { x: 120, y: 120, width: 600, height: 320 },
    );

    // Create and configure the checkbox control.
    let mut checkbox = CheckBox::new(Rect { x: 24, y: 24, width: 200, height: 32 });
    checkbox.set_checked(true);
    window.add_child(checkbox.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
