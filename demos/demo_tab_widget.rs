//! TabWidget demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{TabWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "TabWidget Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    // Create a tab widget container.
    let tabs = TabWidget::new(Rect { x: 24, y: 24, width: 420, height: 260 });
    window.add_child(tabs.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
