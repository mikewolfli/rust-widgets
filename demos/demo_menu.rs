//! Menu action demo.

use std::thread;
use std::time::Duration;

use rust_widgets::platform::get_platform;
use rust_widgets::{init, run};

fn main() {
    // Initialize runtime and acquire platform abstraction.
    init();

    let platform = get_platform();

    // Build the native window and menu hierarchy.
    let window = platform.create_window("Menu Demo", 120, 120, 760, 460);
    let menu_bar = platform.create_menu_bar(window, 0, 0, 760, 28);
    let _ = platform.attach_menu_bar_to_window(window, menu_bar);

    let file_menu = platform.create_menu(menu_bar, "File", 0, 0, 0, 0);
    let edit_menu = platform.create_menu(menu_bar, "Edit", 0, 0, 0, 0);
    let recent_menu = platform.create_menu(file_menu, "Recent", 0, 0, 0, 0);

    let _new_item = platform.menu_add_item(file_menu, "New", Some("cmd+n"));
    let _open_item = platform.menu_add_item(file_menu, "Open", Some("cmd+o"));
    let _recent_1 = platform.menu_add_item(recent_menu, "Project-A", Some("cmd+shift+1"));
    let _recent_2 = platform.menu_add_item(recent_menu, "Project-B", Some("cmd+shift+2"));
    let quit_item = platform.menu_add_item(file_menu, "Quit", Some("cmd+q"));
    let _copy_item = platform.menu_add_item(edit_menu, "Copy", Some("cmd+c"));
    let _paste_item = platform.menu_add_item(edit_menu, "Paste", Some("cmd+v"));

    // Show window and poll menu triggers in a helper thread.
    platform.show_widget(window);

    thread::spawn(move || loop {
        if let Some(item_id) = get_platform().poll_menu_triggered() {
            println!("menu item triggered: {item_id}");
            if item_id == quit_item {
                get_platform().quit();
                break;
            }
        }
        thread::sleep(Duration::from_millis(16));
    });

    run();
}
