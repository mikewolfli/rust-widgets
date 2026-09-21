// Probe: what geometry does the demo's menu bar and toolbar end up with, and what
// colours do they carry?
use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::theme::{global_theme_manager, AppearanceMode};

fn main() {
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);
    let mut app = App::new();
    app.init();
    let win: WindowHandle = app.new_window("probe6", 100, 100, 1120, 620);

    // The demo's order exactly.
    let menu_bar = win.new_menu_bar(0, 0, 0, 0);
    let file_menu = win.new_menu(&menu_bar, "File", 0, 0, 0, 0);
    win.new_menu_item_with_shortcut(&file_menu, "Open", None);
    win.new_menu(&menu_bar, "View", 0, 0, 0, 0);
    win.attach_menu_bar(&menu_bar);

    let bar = win.new_tool_bar(0, 0, 760, 32);
    let status = win.new_status_bar("Ready", 0, 0, 760, 24);

    for (label, id) in [
        ("menu_bar", menu_bar.raw_id()),
        ("tool_bar", bar.raw_id()),
        ("status_bar", status.raw_id()),
    ] {
        rust_widgets::widget::runtime::with_widget(id, |w| {
            println!(
                "{label:<11} geom={:?} bg={:?} border={:?}",
                w.geometry(),
                w.style().background_color,
                w.style().border_color
            );
        });
    }

    // Render the tree and show which horizontal runs are light. This locates the
    // widget that paints the band rather than guessing from the demo's source.
    let width = 1120u32;
    let height = 620u32;
    let frame = rust_widgets::widget::runtime::render_frame_tree(
        win.raw_id(),
        rust_widgets::core::Size::new(width, height),
        rust_widgets::core::Color::rgb(240, 240, 240),
    )
    .expect("frame");
    for y in [1u32, 10, 20, 31, 33] {
        let mut runs: Vec<(u32, u32, [u8; 3])> = Vec::new();
        let mut prev: Option<[u8; 3]> = None;
        let mut start = 0u32;
        for x in 0..800u32 {
            let i = ((y * width + x) * 4) as usize;
            let c = [frame[i], frame[i + 1], frame[i + 2]];
            if Some(c) != prev {
                if let Some(p) = prev {
                    runs.push((start, x - 1, p));
                }
                prev = Some(c);
                start = x;
            }
        }
        if let Some(p) = prev {
            runs.push((start, 799, p));
        }
        let big: Vec<_> = runs.into_iter().filter(|r| r.1 - r.0 > 15).collect();
        println!("y={y:3} {big:?}");
    }

    // Walk the whole tree and print each widget's kind, geometry and style fill, so the
    // painter of the light band is named rather than deduced by elimination.
    walk(win.raw_id(), 0);
}

fn walk(id: u64, depth: usize) {
    rust_widgets::widget::runtime::with_widget(id, |w| {
        println!(
            "{:indent$}{:?} geom={:?} bg={:?}",
            "",
            w.kind(),
            w.geometry(),
            w.style().background_color,
            indent = depth * 2
        );
    });
    for child in rust_widgets::widget::runtime::children_of(id) {
        walk(child, depth + 1);
    }
}
