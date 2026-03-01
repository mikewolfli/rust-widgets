//! Render quality demo for configurable anti-aliasing sample levels.

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{
    PaintBackend, RenderCommand, RenderScene, SceneLayer, SoftwarePaintBackend, SoftwareRenderConfig,
};

fn main() {
    // Build one scene that relies on AA fill to expose edge alpha differences.
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        radius: 4,
        color: Color::rgba(120, 180, 240, 255),
    });
    scene.add_layer(layer);

    // Compose with low AA sampling.
    let mut low_backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend_config(
        &mut low_backend,
        Color::rgba(0, 0, 0, 0),
        Some(SoftwareRenderConfig {
            aa_samples_per_axis: 1,
        }),
    );

    // Compose with high AA sampling.
    let mut high_backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend_config(
        &mut high_backend,
        Color::rgba(0, 0, 0, 0),
        Some(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        }),
    );

    // Compare one corner-edge pixel alpha to show quality delta.
    let edge_idx = ((4 * 16 + 3) * 4 + 3) as usize;
    let alpha_low = low_backend.frame_rgba()[edge_idx];
    let alpha_high = high_backend.frame_rgba()[edge_idx];

    println!(
        "AA sample comparison: low(1x1) alpha={} high(4x4) alpha={}",
        alpha_low, alpha_high
    );
    if alpha_low == alpha_high {
        println!("Result: this pixel is numerically equal; inspect nearby edge pixels for differences.");
    } else {
        println!("Result: higher AA sampling changed edge coverage as expected.");
    }
}