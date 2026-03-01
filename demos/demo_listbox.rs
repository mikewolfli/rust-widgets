//! ListBox demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{ListBox, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let mut window = Window::new(
        "ListBox Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 420 },
    );

    // Create and populate the list box entries.
    let mut listbox = ListBox::new(Rect { x: 24, y: 24, width: 260, height: 200 });
    listbox.add_item("Item 1");
    listbox.add_item("Item 2");
    listbox.add_item("Item 3");
    window.add_child(listbox.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
