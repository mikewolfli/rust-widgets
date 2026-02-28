//! Popup window demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{PopupWindow, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "Popup Demo".to_string(),
        Rect { x: 120, y: 120, width: 700, height: 420 },
    );
    let popup = PopupWindow::new(Rect { x: 120, y: 90, width: 260, height: 140 });

    window.add_child(popup.id());
    window.show();
    run();
}
