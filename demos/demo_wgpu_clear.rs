use rust_widgets::render_engine::WgpuRenderer;

fn main() {
    let renderer = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("wgpu init failed: {error}");
            std::process::exit(1);
        }
    };

    let width = 128;
    let height = 96;
    let pixels = match renderer.render_clear_rgba8(width, height, [0.10, 0.20, 0.80, 1.0]) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wgpu offscreen clear failed: {error}");
            std::process::exit(2);
        }
    };

    let first = pixels.get(0..4).unwrap_or(&[]);
    println!(
        "wgpu clear ok: {}x{}, bytes={}, first_rgba={:?}",
        width,
        height,
        pixels.len(),
        first
    );
}
