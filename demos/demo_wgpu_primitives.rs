use rust_widgets::render_engine::WgpuRenderer;
use rust_widgets::wgpu_backend::{PixelRect, Rgba8, WgpuDrawCommand};

fn main() {
    let renderer = WgpuRenderer::new().expect("wgpu renderer init failed");

    let width = 192;
    let height = 128;
    let image_2x2 = vec![
        255, 40, 40, 255, 40, 255, 40, 255, 40, 40, 255, 255, 255, 255, 255, 255,
    ];
    let commands = vec![
        WgpuDrawCommand::Clear {
            color: Rgba8 {
                r: 20,
                g: 24,
                b: 30,
                a: 255,
            },
        },
        WgpuDrawCommand::FillRect {
            rect: PixelRect {
                x: 12,
                y: 12,
                width: 100,
                height: 72,
            },
            color: Rgba8 {
                r: 40,
                g: 140,
                b: 230,
                a: 255,
            },
            clip: Some(PixelRect {
                x: 0,
                y: 0,
                width,
                height,
            }),
        },
        WgpuDrawCommand::StrokeRect {
            rect: PixelRect {
                x: 8,
                y: 8,
                width: 110,
                height: 80,
            },
            color: Rgba8 {
                r: 250,
                g: 210,
                b: 90,
                a: 255,
            },
            thickness: 3,
            clip: None,
        },
        WgpuDrawCommand::DrawText {
            rect: PixelRect {
                x: 20,
                y: 24,
                width: 64,
                height: 24,
            },
            text: "GPU".to_string(),
            color: Rgba8 {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            clip: Some(PixelRect {
                x: 20,
                y: 24,
                width: 64,
                height: 24,
            }),
        },
        WgpuDrawCommand::DrawImage {
            rect: PixelRect {
                x: 124,
                y: 28,
                width: 48,
                height: 48,
            },
            rgba8: image_2x2,
            image_width: 2,
            image_height: 2,
            clip: Some(PixelRect {
                x: 124,
                y: 28,
                width: 48,
                height: 48,
            }),
        },
    ];

    let pixels = renderer
        .render_draw_commands_rgba8(width, height, &commands)
        .expect("render_draw_commands_rgba8 failed");

    let checksum: u64 = pixels.iter().map(|value| *value as u64).sum();
    println!(
        "wgpu primitives ok: {}x{}, bytes={}, checksum={}",
        width,
        height,
        pixels.len(),
        checksum
    );
}
