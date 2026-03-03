//! Main demo entry.

use rust_widgets::platform::{get_platform, runtime_gui_mode, RuntimeGuiMode};
use rust_widgets::render::{last_auto_render_backend, AutoRenderBackend};
use rust_widgets::{init, run};

fn main() {
    // Initialize the runtime before creating widgets.
    init();

    let platform = get_platform();
    match runtime_gui_mode() {
        RuntimeGuiMode::NativeInteractive => {
            eprintln!(
                "[rust_widgets] backend '{}' running in native-interactive mode",
                platform.backend_name()
            );
        }
        RuntimeGuiMode::PreviewOrStub => {
            eprintln!(
                "[rust_widgets] backend '{}' is preview/stub mode; visible native window may be unavailable",
                platform.backend_name()
            );
        }
    }
    let auto_backend = match last_auto_render_backend() {
        AutoRenderBackend::GpuWgpu => "GpuWgpu",
        AutoRenderBackend::CpuSoftware => "CpuSoftware",
    };
    eprintln!(
        "[rust_widgets] auto_render_backend='{}' (last selected)",
        auto_backend
    );

    let window = platform.create_window("rust_widgets main demo", 80, 80, 900, 600);

    // Create native child controls in the window.
    let _label = platform.create_label(
        window,
        "Cross-platform native GUI architecture",
        24,
        24,
        420,
        32,
    );
    let _button = platform.create_button(window, "Start", 24, 72, 120, 36);

    // Show the main demo window and enter the event loop.
    platform.show_widget(window);

    run();
}
