//! Slider demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{Slider, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "Slider Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 300 },
    );

    // Create and set an initial slider value.
    let mut slider = Slider::new(Rect { x: 24, y: 24, width: 320, height: 28 });
    slider.set_value(35);
    window.add_child(slider.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
