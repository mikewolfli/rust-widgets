//! StackWidget demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{StackWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "StackWidget Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    let stack = StackWidget::new(Rect { x: 24, y: 24, width: 420, height: 260 });
    window.add_child(stack.id());

    window.show();
    run();
}
