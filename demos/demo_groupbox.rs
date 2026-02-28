//! GroupBox demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{GroupBox, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "GroupBox Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 420 },
    );

    let groupbox = GroupBox::new(Rect { x: 24, y: 24, width: 360, height: 220 });
    window.add_child(groupbox.id());

    window.show();
    run();
}
