//! Main demo entry.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("rust_widgets main demo", 80, 80, 900, 600);

    // Create native child controls in the window.
    let _label = platform.create_label(window, "Cross-platform native GUI architecture", 24, 24, 420, 32);
    let _button = platform.create_button(window, "Start", 24, 72, 120, 36);

    // Show the main demo window and enter the event loop.
    platform.show_widget(window);

    run();
}
