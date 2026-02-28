//! Panel demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Panel, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Panel Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 420 },
    );

    let panel = Panel::new(Rect { x: 24, y: 24, width: 360, height: 220 });
    window.add_child(panel.id());

    window.show();
    run();
}
