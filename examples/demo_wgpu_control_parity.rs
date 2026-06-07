use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{RenderCommand, RenderScene, SceneLayer, SoftwareSurface};

fn main() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 0, 120, 80),
        color: Color::from_rgb(40, 120, 220),
    });
    scene.add_layer(layer);

    let mut surface = SoftwareSurface::new(Size::new(120, 80), 1.0);
    scene.compose_to(&mut surface, Color::from_rgb(255, 255, 255));
    println!("demo_wgpu_control_parity: composed {} bytes", surface.frame_rgba().len());
}
