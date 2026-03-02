//! RadioButton demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("RadioButton Demo", 120, 120, 600, 320);

    // Create and select a radio button.
    let _radio = platform.create_radio_button(window, "Option A", 24, 24, 220, 32);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
