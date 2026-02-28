//! ProgressBar demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{ProgressBar, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "ProgressBar Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 300 },
    );

    let mut progress = ProgressBar::new(Rect { x: 24, y: 24, width: 320, height: 28 });
    progress.set_value(72);
    window.add_child(progress.id());

    window.show();
    run();
}
