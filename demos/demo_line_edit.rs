//! LineEdit demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("LineEdit Demo", 120, 120, 700, 320);

    // Create and prefill the line edit control.
    let _line_edit = platform.create_line_edit(window, "Input text", 24, 24, 360, 36);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
