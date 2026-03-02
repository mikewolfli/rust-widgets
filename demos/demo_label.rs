//! Label demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("Label Demo", 120, 120, 600, 320);

    // Create a simple static text label.
    let _label = platform.create_label(window, "This is a label", 24, 24, 260, 32);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
