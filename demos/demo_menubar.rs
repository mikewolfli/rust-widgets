//! MenuBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{MenuBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "MenuBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 460 },
    );

    let menubar = MenuBar::new(Rect { x: 0, y: 0, width: 760, height: 28 });
    window.add_child(menubar.id());

    window.show();
    run();
}
