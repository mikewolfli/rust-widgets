//! LineEdit demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{LineEdit, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "LineEdit Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 320 },
    );

    // Create and prefill the line edit control.
    let mut line_edit = LineEdit::new(Rect { x: 24, y: 24, width: 360, height: 36 });
    line_edit.set_text("Input text".to_string());
    window.add_child(line_edit.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
