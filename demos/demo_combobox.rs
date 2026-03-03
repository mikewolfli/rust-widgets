//! ComboBox demo.

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let window = platform.create_window("ComboBox Demo", 120, 120, 700, 320);

    // Create combo box control.
    let combo = platform.create_combo_box(window, 24, 24, 220, 36);
    let items = ["Apple", "Banana", "Cherry", "Durian", "Grape"];
    for item in items {
        let _ = platform.combo_box_add_item(combo, item);
    }
    let _ = platform.combo_box_set_current_index(combo, 1);
    let current = platform
        .combo_box_current_index(combo)
        .and_then(|index| platform.combo_box_item_text(combo, index))
        .unwrap_or_else(|| "<none>".to_string());
    println!(
        "combo initialized: count={}, current={}",
        platform.combo_box_item_count(combo),
        current
    );

    // Show the demo window and enter the event loop.
    platform.show_widget(window);
    run();
}
