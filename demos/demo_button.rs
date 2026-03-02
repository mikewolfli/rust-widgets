//! Button demo

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("Button Demo", 100, 100, 400, 300);
    let _button = platform.create_button(window, "Click Me!", 150, 120, 100, 40);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);

    run();
}
