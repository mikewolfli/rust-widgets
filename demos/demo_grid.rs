//! Grid widget demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{GridWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Grid Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 500 },
    );

    let grid = GridWidget::new(Rect { x: 24, y: 24, width: 520, height: 340 });
    window.add_child(grid.id());

    window.show();
    run();
}
