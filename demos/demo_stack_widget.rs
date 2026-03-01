//! StackWidget demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{StackWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "StackWidget Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    // Create a stack container widget.
    let stack = StackWidget::new(Rect { x: 24, y: 24, width: 420, height: 260 });
    window.add_child(stack.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
