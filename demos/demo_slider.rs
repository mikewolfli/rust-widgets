//! Slider demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Slider, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Slider Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 300 },
    );

    let mut slider = Slider::new(Rect { x: 24, y: 24, width: 320, height: 28 });
    slider.set_value(35);
    window.add_child(slider.id());

    window.show();
    run();
}
