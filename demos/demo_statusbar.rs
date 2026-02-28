//! StatusBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{StatusBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "StatusBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    let statusbar = StatusBar::new(Rect { x: 0, y: 428, width: 760, height: 32 });
    window.add_child(statusbar.id());

    window.show();
    run();
}
