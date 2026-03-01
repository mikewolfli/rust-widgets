//! Layout demo.

use rust_widgets::core::Rect;
use rust_widgets::layout::{BoxLayout, Layout, Orientation};

fn main() {
    // Build a horizontal box layout with spacing and margins.
    let mut layout = BoxLayout::new(Orientation::Horizontal, 8, 8);
    layout.add_widget(1, 1);
    layout.add_widget(2, 2);
    layout.add_widget(3, 1);

    // Compute child rectangles for a fixed container size.
    layout.update(
        Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        &mut |id, rect| {
            // Print computed geometry for each widget id.
            println!("widget {id} => {:?}", rect);
        },
    );
}
