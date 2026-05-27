//! WGPU draw commands.
use super::types::{PixelRect, Rgba8};
/// Feature-gated GPU draw command list.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}
