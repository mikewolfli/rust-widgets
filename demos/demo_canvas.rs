//! Canvas demo.

use rust_widgets::core::Rect;
use rust_widgets::render::{last_auto_render_backend, AutoRenderBackend};
use rust_widgets::widget::{Canvas, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let auto_backend = match last_auto_render_backend() {
        AutoRenderBackend::GpuWgpu => "GpuWgpu",
        AutoRenderBackend::CpuSoftware => "CpuSoftware",
    };
    eprintln!(
        "[rust_widgets] auto_render_backend='{}' (last selected)",
        auto_backend
    );

    let mut window = Window::new(
        "Canvas Demo".to_string(),
        Rect {
            x: 120,
            y: 120,
            width: 760,
            height: 500,
        },
    );

    let canvas = Canvas::new(Rect {
        x: 24,
        y: 24,
        width: 520,
        height: 340,
    });
    window.add_child(canvas.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
