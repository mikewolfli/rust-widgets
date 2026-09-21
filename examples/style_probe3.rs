// Probe: what does each widget's draw produce, and does render_frame_tree include the
// combo box's fill at all?
use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Color, Size};

fn main() {
    let mut app = App::new();
    app.init();
    let win: WindowHandle = app.new_window("probe3", 0, 0, 400, 300);
    let cb = win.new_combo_box(20, 142, 180, 26);
    cb.add_item("Red");
    cb.set_current_index(0);
    let id = cb.raw_id();

    // 1. Style as the registry sees it.
    rust_widgets::widget::runtime::with_widget(id, |w| {
        println!("combo bg = {:?}", w.style().background_color);
    });

    // 2. Render the whole window tree and look at the combo box's pixels.
    let frame = rust_widgets::widget::runtime::render_frame_tree(
        win.raw_id(),
        Size::new(400, 300),
        Color::rgb(1, 2, 3),
    )
    .expect("frame");

    let mut counter: Vec<([u8; 4], usize)> = Vec::new();
    for y in 142..168u32 {
        for x in 20..200u32 {
            let i = ((y * 400 + x) * 4) as usize;
            let p = [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]];
            match counter.iter_mut().find(|(c, _)| *c == p) {
                Some((_, n)) => *n += 1,
                None => counter.push((p, 1)),
            }
        }
    }
    counter.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("combo box pixels:");
    for (c, n) in counter.iter().take(6) {
        println!("  {:?} x{}", c, n);
    }
    println!("pixel(0,0) = {:?}", &frame[0..4]);
}
