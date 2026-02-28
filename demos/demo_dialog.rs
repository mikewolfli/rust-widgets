//! Dialog demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Dialog, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Dialog Demo".to_string(),
        Rect { x: 100, y: 100, width: 700, height: 420 },
    );
    let dialog = Dialog::new(Rect { x: 80, y: 60, width: 360, height: 220 });

    window.add_child(dialog.id());
    window.show();
    run();
}
