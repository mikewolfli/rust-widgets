//! P0 end-to-end native demo:
//! window + button + line-edit + menu + event-loop + signal-slot bridge.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rust_widgets::event::NativeSignalBridge;
use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize runtime and create the native control tree.
    init();

    let platform = get_platform();
    let window = platform.create_window("P0 Native E2E", 140, 120, 900, 600);

    let button = platform.create_button(window, "Increment", 24, 46, 160, 36);
    let line_edit = platform.create_line_edit(window, "0", 24, 96, 220, 34);

    let menu_bar = platform.create_menu_bar(window, 0, 0, 900, 28);
    let _ = platform.attach_menu_bar_to_window(window, menu_bar);
    let file_menu = platform.create_menu(menu_bar, "File", 0, 0, 0, 0);
    let quit_item = platform.menu_add_item(file_menu, "Quit", Some("cmd+q"));

    platform.show_widget(window);

    // Bridge native trigger queues to signal-slot callbacks.
    let bridge = Arc::new(NativeSignalBridge::new());
    let mut counter: i32 = 0;

    {
        // Button click updates line edit text with a counter.
        let bridge_ref = Arc::clone(&bridge);
        let _ = bridge_ref.connect_clicked(button, move || {
            counter += 1;
            get_platform().set_widget_text(line_edit, &counter.to_string());
            println!("button clicked -> counter={counter}");
        });
    }

    {
        // Line edit value-change callback for logging.
        let bridge_ref = Arc::clone(&bridge);
        let _ = bridge_ref.connect_value_changed(line_edit, move || {
            let text = get_platform().get_widget_text(line_edit);
            println!("line-edit changed -> text={text}");
        });
    }

    {
        // Quit menu action closes the event loop.
        let bridge_ref = Arc::clone(&bridge);
        let _ = bridge_ref.connect_menu_trigger(quit_item, move || {
            println!("quit menu triggered");
            get_platform().quit();
        });
    }

    // Keep pumping bridge queues from a lightweight worker thread.
    thread::spawn(move || {
        loop {
            let _ = bridge.pump_all();
            thread::sleep(Duration::from_millis(8));
        }
    });

    run();
}
