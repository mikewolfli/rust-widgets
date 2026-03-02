//! ComboBox demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("ComboBox Demo", 120, 120, 700, 320);

    // Create combo-box placeholder control.
    let _combo = platform.create_combo_box(window, 24, 24, 220, 36);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
