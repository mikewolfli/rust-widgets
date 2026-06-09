//! Optional WGPU-based GPU renderer backend.
//!
//! Enable with Cargo feature: `gpu-wgpu`.
mod commands;
mod raster;
mod renderer;
pub mod shaders;
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
    #[test]
    fn test_render_clear_gpu_produces_valid_pixels() {
        // This test only runs on systems with a GPU
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer.render_clear_gpu(64, 64, [1.0, 0.0, 0.0, 1.0]).unwrap();
            assert_eq!(pixels.len(), 64 * 64 * 4);
            // All pixels should be red (255, 0, 0, 255)
            assert_eq!(pixels[0], 255);
            assert_eq!(pixels[1], 0);
            assert_eq!(pixels[2], 0);
            assert_eq!(pixels[3], 255);
        }
    }
    #[test]
    fn test_render_fill_rect_gpu_produces_colored_rects() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer
                .render_fill_rect_gpu(
                    16,
                    16,
                    &[(0, 0, 8, 8, [1.0, 0.0, 0.0, 1.0]), (8, 8, 8, 8, [0.0, 1.0, 0.0, 1.0])],
                )
                .unwrap();
            assert_eq!(pixels.len(), 16 * 16 * 4);
            // Top-left quarter should be red
            assert_eq!(pixels[0], 255, "(0,0) should be red");
            assert_eq!(pixels[1], 0, "(0,0) G should be 0");
            assert_eq!(pixels[2], 0, "(0,0) B should be 0");
            assert_eq!(pixels[3], 255, "(0,0) A should be 255");
            // Bottom-right quarter should be green
            let g_off = ((12 * 16 + 12) * 4) as usize;
            assert_eq!(pixels[g_off], 0, "(12,12) R should be 0");
            assert_eq!(pixels[g_off + 1], 255, "(12,12) should be green");
            // Gap between rects should be transparent
            let t_off = (12 * 4) as usize;
            assert_eq!(pixels[t_off + 3], 0, "gap should be transparent");
        }
    }
    #[test]
    fn test_render_fill_rect_gpu_empty_returns_transparent() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer.render_fill_rect_gpu(8, 8, &[]).unwrap();
            assert_eq!(pixels.len(), 8 * 8 * 4);
            assert!(pixels.iter().all(|&b| b == 0), "empty rects should produce all-zero pixels");
        }
    }
    #[test]
    fn test_render_stroke_rect_gpu_produces_outline() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let rects = &[((6, 6, 4, 4, 1), [1.0, 0.0, 0.0, 1.0])]; // red stroke
            let pixels = renderer.render_stroke_rect_gpu(16, 16, rects).unwrap();
            assert_eq!(pixels.len(), 16 * 16 * 4);
        }
    }
    #[test]
    fn test_render_stroke_rect_gpu_empty_returns_frame() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer.render_stroke_rect_gpu(8, 8, &[]).unwrap();
            assert_eq!(pixels.len(), 8 * 8 * 4);
        }
    }
    #[test]
    fn test_render_draw_line_gpu_produces_pixels() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let lines = &[((2, 2, 14, 14, 2), [0.0, 1.0, 0.0, 1.0])]; // green diagonal
            let pixels = renderer.render_draw_line_gpu(16, 16, lines).unwrap();
            assert_eq!(pixels.len(), 16 * 16 * 4);
        }
    }
    #[test]
    fn test_render_fill_circle_gpu_produces_circle() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer
                .render_fill_circle_gpu(32, 32, &[((16, 16, 8), [1.0, 0.0, 0.0, 1.0])])
                .unwrap();
            assert_eq!(pixels.len(), 32 * 32 * 4);
            // Center pixel should be red
            let center_idx = (16 * 32 + 16) * 4;
            assert_eq!(pixels[center_idx], 255); // R
            assert_eq!(pixels[center_idx + 1], 0); // G
            assert_eq!(pixels[center_idx + 2], 0); // B
        }
    }
    #[test]
    fn test_render_fill_circle_gpu_empty_returns_frame() {
        if let Ok(renderer) = super::WgpuRenderer::new() {
            let pixels = renderer.render_fill_circle_gpu(8, 8, &[]).unwrap();
            assert_eq!(pixels.len(), 8 * 8 * 4);
        }
    }
}
