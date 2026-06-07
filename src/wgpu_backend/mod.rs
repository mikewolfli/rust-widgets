//! Optional WGPU-based GPU renderer backend.
//!
//! Enable with Cargo feature: `gpu-wgpu`.
mod commands;
mod raster;
mod renderer;
mod types;
pub use commands::WgpuDrawCommand;
pub use renderer::WgpuRenderer;
pub use types::{PixelRect, Rgba8};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_raster_draw_text_honors_clipping() {
        let pixels = raster::rasterize_draw_commands_rgba8(
            16,
            16,
            &[
                commands::WgpuDrawCommand::Clear { color: types::Rgba8 { r: 0, g: 0, b: 0, a: 0 } },
                commands::WgpuDrawCommand::DrawText {
                    rect: types::PixelRect { x: 0, y: 0, width: 16, height: 16 },
                    text: "AB".to_string(),
                    color: types::Rgba8 { r: 220, g: 10, b: 40, a: 255 },
                    clip: Some(types::PixelRect { x: 0, y: 0, width: 8, height: 16 }),
                },
            ],
        )
        .expect("text raster should succeed");
        let mut painted_left = 0usize;
        let mut painted_right = 0usize;
        for y in 0..16u32 {
            for x in 0..16u32 {
                let offset = ((y * 16 + x) * 4) as usize;
                let alpha = pixels[offset + 3];
                if alpha == 0 {
                    continue;
                }
                if x < 8 {
                    painted_left += 1;
                } else {
                    painted_right += 1;
                }
            }
        }
        assert!(painted_left > 0);
        assert_eq!(painted_right, 0);
    }
    #[test]
    fn command_raster_draw_image_scales_with_deterministic_sampling() {
        let source = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let pixels = raster::rasterize_draw_commands_rgba8(
            4,
            4,
            &[commands::WgpuDrawCommand::DrawImage {
                rect: types::PixelRect { x: 0, y: 0, width: 4, height: 4 },
                rgba8: source,
                image_width: 2,
                image_height: 2,
                clip: None,
            }],
        )
        .expect("image raster should succeed");
        let sample = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * 4 + x) * 4) as usize;
            [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
        };
        assert_eq!(sample(0, 0), [255, 0, 0, 255]);
        assert_eq!(sample(3, 0), [0, 255, 0, 255]);
        assert_eq!(sample(0, 3), [0, 0, 255, 255]);
        assert_eq!(sample(3, 3), [255, 255, 255, 255]);
    }
    #[test]
    fn command_raster_draw_image_invalid_payload_is_explicit_error() {
        let error = raster::rasterize_draw_commands_rgba8(
            4,
            4,
            &[commands::WgpuDrawCommand::DrawImage {
                rect: types::PixelRect { x: 0, y: 0, width: 4, height: 4 },
                rgba8: vec![0; 12],
                image_width: 2,
                image_height: 2,
                clip: None,
            }],
        )
        .expect_err("invalid payload should fail");
        assert!(error.contains("invalid DrawImage payload"));
    }
}
