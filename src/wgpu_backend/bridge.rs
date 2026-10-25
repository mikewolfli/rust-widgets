// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Explicit adapter for the render-layer non-zero fill command subset.

use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
use crate::render::RenderCommand;

/// Converts non-zero fills and their clip/blend state into the retained WGPU command format.
///
/// Geometry and clip coordinates are already expressed in the render command's pixel space;
/// conversion preserves them verbatim. The retained WGPU representation owns its vertices and
/// contour ranges, so this adapter necessarily copies those arrays. Commands outside this narrow
/// subset are rejected instead of being silently omitted or approximated.
///
/// This is a command-format bridge, not a GPU rasterizer. The current
/// [`super::WgpuRenderer::render_draw_commands_rgba8`] path rasterizes these commands on the CPU
/// before transferring the resulting frame.
pub fn bridge_non_zero_fill_commands(
    commands: &[RenderCommand],
) -> Result<Vec<WgpuDrawCommand>, String> {
    commands
        .iter()
        .enumerate()
        .map(|(index, command)| match command {
            RenderCommand::FillPathNonZero { path, color } => {
                Ok(WgpuDrawCommand::FillPathNonZero {
                    points: path
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect::<Vec<_>>()
                        .into(),
                    contours: path.contours().into(),
                    color: Rgba8 { r: color.r, g: color.g, b: color.b, a: color.a },
                    clip: None,
                })
            }
            RenderCommand::PushClip { x, y, width, height } => Ok(WgpuDrawCommand::PushClip {
                rect: PixelRect { x: *x, y: *y, width: *width, height: *height },
            }),
            RenderCommand::PopClip => Ok(WgpuDrawCommand::PopClip),
            RenderCommand::SetBlendMode { mode } => {
                Ok(WgpuDrawCommand::SetBlendMode { mode: *mode as u8 })
            }
            _ => Err(format!(
                "RenderCommand at index {index} is unsupported by the WGPU non-zero-fill bridge"
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::bridge_non_zero_fill_commands;
    use crate::core::{Color, Point};
    use crate::render::{CompoundPath, RenderCommand};
    use crate::wgpu_backend::raster::rasterize_draw_commands_rgba8;

    #[test]
    fn bridge_preserves_winding_clip_order_and_blend_state() {
        let path = CompoundPath::new(
            vec![
                Point::new(0, 0),
                Point::new(16, 0),
                Point::new(16, 16),
                Point::new(0, 16),
                Point::new(4, 4),
                Point::new(4, 12),
                Point::new(12, 12),
                Point::new(12, 4),
            ],
            vec![(0, 4), (4, 8)],
        )
        .expect("valid outer contour and oppositely wound hole");
        let commands = vec![
            RenderCommand::PushClip { x: 0, y: 0, width: 8, height: 16 },
            RenderCommand::SetBlendMode { mode: crate::render::BlendMode::Normal },
            RenderCommand::FillPathNonZero { path, color: Color::rgba(220, 30, 40, 255) },
            RenderCommand::PopClip,
        ];

        let bridged = bridge_non_zero_fill_commands(&commands).expect("supported command subset");
        let pixels = rasterize_draw_commands_rgba8(16, 16, &bridged).expect("valid WGPU commands");
        let pixel = |x: u32, y: u32| {
            let offset = ((y * 16 + x) * 4) as usize;
            [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
        };

        assert_eq!(pixel(2, 8), [220, 30, 40, 255], "outer fill remains visible in clip");
        assert_eq!(pixel(6, 8), [0, 0, 0, 0], "opposite winding preserves the hole");
        assert_eq!(pixel(10, 2), [0, 0, 0, 0], "clip limits the converted command");
    }

    #[test]
    fn bridge_rejects_commands_outside_its_declared_subset() {
        let unsupported = RenderCommand::FillRect {
            rect: crate::core::Rect::new(0, 0, 2, 2),
            color: Color::BLACK,
        };
        let error = bridge_non_zero_fill_commands(&[unsupported]).expect_err("must not drop input");
        assert!(error.contains("index 0"));
        assert!(error.contains("unsupported"));
    }
}
