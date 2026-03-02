//! Window demo

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("Hello Window", 100, 100, 800, 600);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);

    run();
}
