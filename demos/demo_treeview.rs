//! TreeView demo.

use rust_widgets::core::Rect;
use rust_widgets::widget::{TreeView, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    init();

    let mut window = Window::new(
        "TreeView Demo".to_string(),
        Rect { x: 120, y: 120, width: 720, height: 460 },
    );

    let mut tree = TreeView::new(Rect { x: 24, y: 24, width: 320, height: 260 });
    tree.add_node("Root");
    tree.add_node("Root/Child-1");
    tree.add_node("Root/Child-2");
    window.add_child(tree.id());

    window.show();
    run();
}
