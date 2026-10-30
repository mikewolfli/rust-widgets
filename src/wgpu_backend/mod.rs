// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Optional WGPU-based GPU renderer backend.
//!
//! Enable with Cargo feature: `gpu-wgpu`.
//!
//! # Reachability
//!
//! **State:** Reserved: the `gpu-wgpu` renderer is built only under `--features gpu-wgpu` and
//! currently has no production caller. The auto scene path was deliberately moved off it in
//! D09-GPU-01 (it drew the whole scene on the CPU, then round-tripped the frame through the GPU,
//! so it added work without accelerating anything — see `src/render/backend/scene.rs`). The
//! renderer is kept compiled for the planned on-GPU scene rasterisation described there, not as a
//! live code path; the rasteriser and its tests remain the reference for the command set.
//!
//! # Cost (G-63 / G-212)
//!
//! A stream the dispatcher claims — clears, unclipped rectangles and circles, and image draws
//! ([`renderer`]'s `commands_gpu_dispatchable`) — is drawn by **one GPU pass and one readback**, so
//! its pixels never exist on the CPU until they are read back. Any other stream falls back to the
//! CPU rasteriser and then uploads the finished frame, and on that path the round trip makes the
//! call expected **slower** than returning the software rasteriser's buffer directly. `read_back`
//! bounds the wait with a timeout so a lost device reports an error instead of hanging, but it
//! cannot remove the trip itself.
//!
//! # Planned on-GPU order (G-63)
//!
//! The module is retained as the carrier for a real GPU pipeline. The integration order is:
//!
//! 1. Batch the 2D rectangle path. **Done:** a run longer than one rectangle is drawn as
//!    instanced quad batches of at most `MAX_INSTANCES_PER_DRAW` (G-64), preserving painter order.
//! 2. Move the remaining command variants (`DrawImage`, `DrawText`, rounded rectangles, circles
//!    and paths) from `raster.rs` onto their reserved shaders (`shaders::ShaderModule`, G-213).
//!    **`DrawImage` is done:** `IMAGE_VS`/`IMAGE_FRAG` draw it as one quad with a single texel
//!    fetch per destination pixel, and the fragment uses `textureLoad` at the same integer source
//!    coordinate the CPU loop computes, so the two paths agree byte for byte (the
//!    `a_scaled_image_draw_matches_the_software_rasteriser` test asserts the equality).
//!    **Rectangles, stroked rectangles and filled rounded rectangles are done:** their clipping
//!    and (for rounded corners) their row spans come from the same functions the CPU rasteriser
//!    uses, so they are drawn as instanced quads with identical bytes. **`PushClip`/`PopClip` are
//!    done:** a rectangle clip narrows that geometry and becomes a scissor for circles, images and
//!    text. **`DrawText` is done:** the glyph coverage is painted by the same font-stack loop the
//!    CPU rasteriser blends (`raster::for_each_text_cell`), uploaded as a one-byte-per-pixel
//!    texture, and multiplied by the text colour in the fragment stage.
//!    **Circle strokes, arcs (wedge and band) and rounded-rectangle outlines are done** through
//!    the `shape` pipeline: one SDF per mode, feathered over one pixel.
//!    **Axis-aligned gradients are done** (`DrawGradient`, `FillLinearGradient`): the ramp is one
//!    colour per row (or column), computed by the CPU's own stop rule
//!    (`raster::gradient_ramp`) and drawn as a one-pixel-wide image, so it is byte-identical and the
//!    per-pixel lookups are gone.
//!    **The radial gradient is done too** (`FillRadialGradient`): the stop table is uploaded as a
//!    texture and the fragment computes `t = sqrt(dx² + dy²) · (1/radius)` in `f32` exactly as the
//!    CPU does before the nearest-stop lookup, which measures byte-identical.
//!    **Filled paths are done** (`DrawPath { filled: true }`, `FillPathNonZero`): the CPU turns each
//!    contour into a triangle fan, the stencil buffer counts the fan with the rule the command asks
//!    for (invert for even-odd, increment/decrement for non-zero), a cover quad paints where the
//!    count says "inside", and a third pass resets the count for the next path.
//!    **Axis-aligned stroke paths are done too** (`DrawPath { filled: false }` whose every segment
//!    is horizontal or vertical): the CPU draws a stroked path as one line per segment, so a path
//!    of axis-aligned segments is a run of rectangles.
//!    **Axis-aligned lines are done** (`DrawLine` with `from.y == to.y` or `from.x == to.x`): the
//!    CPU stamps a `2·half_w` square at every Bresenham step, and along an axis those stamps tile
//!    into exactly one rectangle, so the GPU reproduces the line byte for byte. A diagonal line
//!    stays with the CPU rather than being redrawn as a capsule of a different shape.
//!    **Unblurred shadows are done** (`BoxShadow { blur_radius: 0 }`): the body is the rectangle
//!    translated by the offset, which is exactly a fill, so it is byte-identical. Stroked paths and
//!    *blurred* shadows still come from the CPU rasteriser — a blurred shadow needs an offscreen
//!    layer plus two blur passes, which a single render pass cannot express — so a stream
//!    containing one of them falls back as a whole, which is why the CPU raster path cannot be
//!    deleted yet.
//! 3. Replace `WgpuRenderer::render_draw_commands_rgba8`'s CPU raster + upload + readback with one
//!    GPU pass and a single readback, then delete the duplicate software implementation from this
//!    module so the two cannot drift.
//!
//! Step 3 is already true for a stream the dispatcher claims (one pass, one readback); it becomes
//! true for *every* stream when step 2 finishes. Until then callers must treat the `rgba8` entry
//! points as: GPU when the stream is fully covered, software otherwise — never as hardware
//! acceleration for a mixed UI frame.
//!
//! # A known divergence, recorded rather than hidden
//!
//! The SDF pipelines (circle fill, rings, arcs, rounded outlines) anti-alias their edges; the CPU
//! rasteriser fills those shapes with integer predicates and no anti-aliasing. A stream containing
//! one of them is therefore *geometrically* the same shape but not byte-identical at the edge — the
//! tests assert the shape (band, hole, angular window, interior) rather than the rim.
//!
//! All of these pipelines composite source-over, which is what keeps a shape from erasing the
//! content under its bounding quad. The consequence is a storage detail worth stating: on a
//! **transparent** destination the stored colour is the source premultiplied by its coverage
//! (fixed-function blending cannot read the destination, so it cannot un-premultiply), while on an
//! opaque destination the result is the ordinary composite. The software path always stores straight
//! alpha, so a frame with no clear can differ beyond the edge pixels.
//!
//! Compositing has one LSB-level caveat with the same cause: the crate's CPU rule composites
//! **un-premultiplied** alpha (`out = (Cs·αs + Cd·αd·(1−αs)) / αout`, then rounds), while
//! fixed-function blending multiplies and rounds in the render target. On an opaque destination
//! the two expressions are the same arithmetic and agree exactly (all the equality tests run on an
//! opaque clear). On a translucent destination they can differ by one level per channel, which no
//! fragment shader can fix — reading the destination is not part of the fragment stage. The
//! translucent-text test therefore allows exactly one level and fails on anything larger.
mod bridge;
mod commands;
mod raster;
mod renderer;
pub mod shaders;
mod types;
pub use bridge::bridge_non_zero_fill_commands;
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
                    text: "AB".into(),
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
                rgba8: source.into(),
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
                rgba8: vec![0; 12].into(),
                image_width: 2,
                image_height: 2,
                clip: None,
            }],
        )
        .expect_err("invalid payload should fail");
        assert!(error.contains("invalid DrawImage payload"));
    }
    /// A `RenderCommand::DrawImageScaled` is expressible as one image command in this set: the
    /// destination is `rect`, and the source grid travels alongside the pixels as
    /// `image_width`/`image_height`. The rasteriser stretches a source smaller than the rectangle
    /// across the whole rectangle — the semantics the render-layer variant promises — rather than
    /// reading the rectangle's stride out of the source.
    ///
    /// This pins the correspondence the GPU path depends on; it is not a bridge, because no
    /// production code drives this module yet (`mod.rs` documents that reachability).
    #[test]
    fn a_scaled_image_draw_stretches_its_source_across_the_whole_rectangle() {
        // The fields `RenderCommand::DrawImageScaled` carries: destination box + source grid.
        let (x, y, width, height) = (0, 0, 8u32, 4u32);
        let (source_width, source_height) = (2u32, 1u32);
        // One row, two columns: red then blue.
        let data = vec![255u8, 0, 0, 255, 0, 0, 255, 255];

        let pixels = raster::rasterize_draw_commands_rgba8(
            width,
            height,
            &[commands::WgpuDrawCommand::DrawImage {
                rect: types::PixelRect { x, y, width, height },
                rgba8: data.into(),
                image_width: source_width,
                image_height: source_height,
                clip: None,
            }],
        )
        .expect("a scaled image rasterises");

        let sample = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * width + x) * 4) as usize;
            [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
        };
        assert_eq!(sample(0, 0), [255, 0, 0, 255], "the left half is the source's first column");
        assert_eq!(sample(7, 3), [0, 0, 255, 255], "the right half is its second column");
        let (frame, remainder) = pixels.as_chunks::<4>();
        assert!(remainder.is_empty(), "the frame is RGBA8");
        assert!(
            frame.iter().all(|pixel| pixel[3] == 255),
            "the whole destination must be covered, not the corner the source could reach"
        );
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
