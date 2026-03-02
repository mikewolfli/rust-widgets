//! ListBox demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("ListBox Demo", 120, 120, 700, 420);

    // Create list-box placeholder control.
    let _listbox = platform.create_list_box(window, 24, 24, 260, 200);

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
