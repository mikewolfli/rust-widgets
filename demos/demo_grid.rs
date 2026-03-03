//! Grid widget demo.

use rust_widgets::core::Rect;
use rust_widgets::platform::{get_platform, runtime_gui_mode, RuntimeGuiMode};
use rust_widgets::widget::{GridWidget, Widget, Window};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    let runtime_mode = runtime_gui_mode();
    let runtime_mode_text = match runtime_mode {
        RuntimeGuiMode::NativeInteractive => "NativeInteractive",
        RuntimeGuiMode::PreviewOrStub => "PreviewOrStub",
    };
    let native_window_expected = false;
    eprintln!(
        "[demo_grid] backend='{}' runtime_mode='{}' native_window_expected={} (container path)",
        platform.backend_name(),
        runtime_mode_text,
        native_window_expected
    );

    let mut window = Window::new(
        "Grid Demo".to_string(),
        Rect { x: 120, y: 120, width: 760, height: 500 },
    );

    // Create the grid widget surface.
    let grid = GridWidget::new(Rect { x: 24, y: 24, width: 520, height: 340 });
    window.add_child(grid.id());

    // Show the demo window and enter the event loop.
    window.show();
    run();
}
