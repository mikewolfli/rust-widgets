//! ComboBox demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{ComboBox, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "ComboBox Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 320 },
    );

    let mut combo = ComboBox::new(Rect { x: 24, y: 24, width: 220, height: 36 });
    combo.add_item("Option A");
    combo.add_item("Option B");
    combo.add_item("Option C");
    combo.set_current_index(1);
    window.add_child(combo.id());

    window.show();
    run();
}
