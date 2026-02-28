//! RadioButton demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{RadioButton, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "RadioButton Demo".to_string(),
        Rect { x: 120, y: 120, width: 600, height: 320 },
    );

    let mut radio = RadioButton::new(Rect { x: 24, y: 24, width: 220, height: 32 });
    radio.set_checked(true);
    window.add_child(radio.id());

    window.show();
    run();
}
