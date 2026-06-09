//! WGPU draw commands.
use super::types::{PixelRect, Rgba8};
/// Feature-gated GPU draw command list.
#[derive(Debug, Clone, PartialEq)]
pub enum WgpuDrawCommand {
    Clear {
        color: Rgba8,
    },
    FillRect {
        rect: PixelRect,
        color: Rgba8,
        clip: Option<PixelRect>,
    },
    StrokeRect {
        rect: PixelRect,
        color: Rgba8,
        thickness: u32,
        clip: Option<PixelRect>,
    },
    DrawText {
        rect: PixelRect,
        text: String,
        color: Rgba8,
        clip: Option<PixelRect>,
    },
    DrawImage {
        rect: PixelRect,
        rgba8: Vec<u8>,
        image_width: u32,
        image_height: u32,
        clip: Option<PixelRect>,
    },
    FillRoundedRect {
        rect: PixelRect,
        radius: u32,
        color: Rgba8,
        clip: Option<PixelRect>,
    },
    StrokeRoundedRect {
        rect: PixelRect,
        radius: u32,
        color: Rgba8,
        thickness: u32,
        clip: Option<PixelRect>,
    },
    DrawLine {
        from: (i32, i32),
        to: (i32, i32),
        color: Rgba8,
        width: u32,
        clip: Option<PixelRect>,
    },
    FillCircle {
        center: (i32, i32),
        radius: u32,
        color: Rgba8,
        clip: Option<PixelRect>,
    },
    DrawCircle {
        center: (i32, i32),
        radius: u32,
        color: Rgba8,
        width: u32,
        clip: Option<PixelRect>,
    },
    DrawArc {
        center: (i32, i32),
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Rgba8,
        filled: bool,
        clip: Option<PixelRect>,
    },
    DrawPath {
        points: Vec<(i32, i32)>,
        closed: bool,
        color: Rgba8,
        filled: bool,
        width: u32,
        clip: Option<PixelRect>,
    },
    DrawGradient {
        rect: PixelRect,
        gradient_data: Vec<u8>,
        clip: Option<PixelRect>,
    },
    PushClip {
        rect: PixelRect,
    },
    PopClip,
    /// Draw a filled gradient rectangle (BLUE11 R5.1 sync with RenderCommand::DrawGradient).
    FillLinearGradient {
        rect: PixelRect,
        gradient_data: Vec<u8>,
        clip: Option<PixelRect>,
    },
    FillRadialGradient {
        rect: PixelRect,
        gradient_data: Vec<u8>,
        clip: Option<PixelRect>,
    },
    /// Set compositing blend mode (BLUE11 R5.6).
    SetBlendMode {
        mode: u8,
    },
    /// Draw a drop shadow (BLUE11 R5.3).
    BoxShadow {
        rect: PixelRect,
        color: Rgba8,
        offset_x: i32,
        offset_y: i32,
        blur_radius: u32,
        clip: Option<PixelRect>,
    },
}
