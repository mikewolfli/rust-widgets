//! ToolBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{ToolBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "ToolBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    // Create a top toolbar container.
    let toolbar = ToolBar::new(Rect { x: 0, y: 0, width: 760, height: 40 });
    window.add_child(toolbar.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
