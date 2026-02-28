//! Layout demo.

use rust_widgets::core::Rect;
use rust_widgets::layout::{BoxLayout, Layout, Orientation};

fn main() {
    let mut layout = BoxLayout::new(Orientation::Horizontal, 8, 8);
    layout.add_widget(1, 1);
    layout.add_widget(2, 2);
    layout.add_widget(3, 1);

    layout.update(
        Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        &mut |id, rect| {
            println!("widget {id} => {:?}", rect);
        },
    );
}
