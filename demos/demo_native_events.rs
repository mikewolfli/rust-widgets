//! Native trigger polling demo (menu + typed widget events).

use std::thread;
use std::time::Duration;

use rust_widgets::platform::{get_platform, WidgetTriggerKind};
use rust_widgets::{init, run};

fn main() {
    // Initialize runtime and create native controls through platform APIs.
    init();

    let platform = get_platform();
    let window = platform.create_window("Native Events Demo", 120, 120, 860, 560);

    let _button = platform.create_button(window, "Click me", 24, 40, 140, 36);
    let _line = platform.create_line_edit(window, "Type here", 24, 92, 280, 34);
    let _check = platform.create_checkbox(window, "Enable option", 24, 140, 180, 30);

    // Add a menu bar and quit action for event testing.
    let menu_bar = platform.create_menu_bar(window, 0, 0, 860, 28);
    let _ = platform.attach_menu_bar_to_window(window, menu_bar);
    let file_menu = platform.create_menu(menu_bar, "File", 0, 0, 0, 0);
    let _quit_item = platform.menu_add_item(file_menu, "Quit", Some("cmd+q"));
    let _backend = platform.backend_name();

    platform.show_widget(window);

    // Poll menu and widget trigger queues in a background loop.
    thread::spawn(move || {
        let mut ticks: u32 = 0;
        loop {
            if (_backend == "gtk" || _backend == "harmony-desktop") && ticks == 60 {
                // Inject synthetic events for backends that need deterministic demo input.
                let _ = get_platform().inject_widget_trigger_event(_button, WidgetTriggerKind::Clicked);
                let _ = get_platform().inject_widget_trigger_event(_line, WidgetTriggerKind::ValueChanged);
                let _ = get_platform().inject_menu_trigger(_quit_item);
            }

            if let Some(menu_item_id) = get_platform().poll_menu_triggered() {
                println!("menu triggered: {menu_item_id}");
                if menu_item_id == _quit_item {
                    get_platform().quit();
                    break;
                }
            }

            if let Some(event) = get_platform().poll_widget_trigger_event() {
                let kind = match event.kind {
                    WidgetTriggerKind::Clicked => "clicked",
                    WidgetTriggerKind::ValueChanged => "value-changed",
                    WidgetTriggerKind::SelectionChanged => "selection-changed",
                    WidgetTriggerKind::Closed => "closed",
                    WidgetTriggerKind::Unknown => "unknown",
                };
                println!("widget triggered: id={}, kind={kind}", event.widget_id);
            }

            thread::sleep(Duration::from_millis(16));
            ticks = ticks.saturating_add(1);
        }
    });

    run();
}
