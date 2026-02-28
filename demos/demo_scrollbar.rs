//! ScrollBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{ScrollBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "ScrollBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 300 },
    );

    let mut scrollbar = ScrollBar::new(Rect { x: 24, y: 24, width: 320, height: 28 });
    scrollbar.set_value(48);
    window.add_child(scrollbar.id());

    window.show();
    run();
}
