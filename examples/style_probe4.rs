// Probe: in the demo's real configuration, what colour is inside the combo box and
// what is the window's own fill? Distinguishes "combo did not draw" from "window drew
// over it".
use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Color, Size};
use rust_widgets::theme::{global_theme_manager, AppearanceMode};

fn main() {
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);

    let mut app = App::new();
    app.init();
    let win: WindowHandle = app.new_window("probe4", 100, 100, 1120, 620);
    let cb = win.new_combo_box(20, 142, 180, 26);
    cb.add_item("Red");
    cb.set_current_index(0);
    let tb = win.new_tool_bar(0, 0, 760, 32);
    rust_widgets::widget::runtime::with_widget(tb.raw_id(), |w| {
        println!("toolbar bg = {:?} border = {:?}", w.style().background_color, w.style().border_color);
    });
    // The platform-level creator for a toolbar, which is what the window handle calls.
    println!("toolbar factory name = {:?}",
        rust_widgets::widget::capability::factory_name_for_kind(rust_widgets::widget::WidgetKind::ToolBar));
    println!("toolbar theme style  = {:?}",
        rust_widgets::theme::resolved_theme_style("tool_bar").map(|s| s.background_color));

    rust_widgets::widget::runtime::with_widget(cb.raw_id(), |w| {
        println!("combo bg = {:?}", w.style().background_color);
    });
    rust_widgets::widget::runtime::with_widget(win.raw_id(), |w| {
        println!(
            "window bg = {:?} border = {:?} w={:?} geom={:?}",
            w.style().background_color,
            w.style().border_color,
            w.style().border_width,
            w.geometry()
        );
    });

    let width = 1120u32;
    let height = 620u32;
    let frame = rust_widgets::widget::runtime::render_frame_tree(
        win.raw_id(),
        Size::new(width, height),
        Color::rgb(240, 240, 240),
    )
    .expect("frame");

    let mut counter: Vec<([u8; 4], usize)> = Vec::new();
    for y in 142..168u32 {
        for x in 20..200u32 {
            let i = ((y * width + x) * 4) as usize;
            let p = [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]];
            match counter.iter_mut().find(|(c, _)| *c == p) {
                Some((_, n)) => *n += 1,
                None => counter.push((p, 1)),
            }
        }
    }
    counter.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("inside combo box (20..200, 142..168):");
    for (c, n) in counter.iter().take(6) {
        println!("  {:?} x{}", c, n);
    }
    println!("pixel(0,0) = {:?}", &frame[0..4]);
    let last = ((height - 1) * width + (width - 1)) as usize * 4;
    println!("pixel(bottom-right) = {:?}", &frame[last..last + 4]);
}
