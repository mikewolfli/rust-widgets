// Probe: which control paints the light band? Render the tree, then hide each candidate
// in turn and watch the band's pixel count.
use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Color, Size};
use rust_widgets::theme::{global_theme_manager, AppearanceMode};

/// Light pixels in rows 1..31, x 0..760 — the band's rectangle in the 1120x620 window.
fn band_pixels(frame: &[u8], width: u32) -> usize {
    let mut n = 0;
    for y in 1..31u32 {
        for x in 0..760u32 {
            let i = ((y * width + x) * 4) as usize;
            if frame[i] == 240 && frame[i + 1] == 240 && frame[i + 2] == 240 {
                n += 1;
            }
        }
    }
    n
}

fn set_visible(id: u64, visible: bool) {
    rust_widgets::widget::runtime::with_widget_mut(id, |w| {
        if visible {
            w.show();
        } else {
            w.hide();
        }
    });
}

fn main() {
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);
    let mut app = App::new();
    app.init();
    let win: WindowHandle = app.new_window("probe7", 100, 100, 1120, 620);
    let bar = win.new_tool_bar(0, 0, 760, 32);
    let status = win.new_status_bar("Ready", 0, 0, 760, 24);

    let width = 1120u32;
    let height = 620u32;
    let render = || {
        rust_widgets::widget::runtime::render_frame_tree(
            win.raw_id(),
            Size::new(width, height),
            Color::rgb(240, 240, 240),
        )
        .map(|f| band_pixels(&f, width))
        .unwrap_or(0)
    };

    println!("both present      : band pixels = {}", render());

    set_visible(status.raw_id(), false);
    println!("status bar hidden : band pixels = {}", render());
    set_visible(status.raw_id(), true);

    set_visible(bar.raw_id(), false);
    println!("tool bar hidden   : band pixels = {}", render());
    set_visible(bar.raw_id(), true);
}
