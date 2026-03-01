//! MenuBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{MenuBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "MenuBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    // Create a top menu bar area.
    let menubar = MenuBar::new(Rect { x: 0, y: 0, width: 760, height: 28 });
    window.add_child(menubar.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
