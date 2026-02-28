//! Label demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Label, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Label Demo".to_string(),
        Rect { x: 120, y: 120, width: 600, height: 320 },
    );

    let label = Label::new(
        "This is a label".to_string(),
        Rect { x: 24, y: 24, width: 260, height: 32 },
    );
    window.add_child(label.id());

    window.show();
    run();
}
