//! CheckBox demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("CheckBox Demo", 120, 120, 600, 320);

    // Create and configure the checkbox control.
    let _checkbox = platform.create_checkbox(window, "Enable option", 24, 24, 200, 32);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
