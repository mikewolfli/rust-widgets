//! TextEdit demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{TextEdit, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "TextEdit Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 420 },
    );

    let mut text_edit = TextEdit::new(Rect { x: 24, y: 24, width: 420, height: 220 });
    text_edit.set_text("Multi-line text".to_string());
    window.add_child(text_edit.id());

    window.show();
    run();
}
