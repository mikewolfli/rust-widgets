//! GroupBox demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{GroupBox, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "GroupBox Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 420 },
    );

    // Create the group box container widget.
    let groupbox = GroupBox::new(Rect { x: 24, y: 24, width: 360, height: 220 });
    window.add_child(groupbox.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
