//! Table widget demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{TableWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Table Demo".to_string(),
        Rect {
            x: 120,
            y: 120,
            width: 800,
            height: 480,
        },
    );

    let table = TableWidget::new(Rect {
        x: 16,
        y: 16,
        width: 768,
        height: 420,
    });

    window.add_child(table.id());
    window.show();
    run();
}
