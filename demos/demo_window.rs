//! Window demo

use rust_widgets::core::Rect;
use rust_widgets::widget::{Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    // Build the demo window.
    let mut window = Window::new(
        "Hello Window".to_string(),
        Rect { x: 100, y: 100, width: 800, height: 600 }
    );

    let _handle = window.closed.connect(|| {
        println!("Window closed");
    });

    // Show the demo window and enter the event loop.
    window.show();

    run();
}
