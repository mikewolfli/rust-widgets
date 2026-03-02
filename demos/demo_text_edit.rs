//! TextEdit demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("TextEdit Demo", 120, 120, 720, 420);

    // Use the available native single-line edit primitive for this runtime demo.
    let _text_edit = platform.create_line_edit(window, "Multi-line text", 24, 24, 420, 36);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
