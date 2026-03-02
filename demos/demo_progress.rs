//! ProgressBar demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("ProgressBar Demo", 120, 120, 700, 300);

    // Create progress-bar placeholder control.
    let _progress = platform.create_progress_bar(window, 24, 24, 320, 28);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
