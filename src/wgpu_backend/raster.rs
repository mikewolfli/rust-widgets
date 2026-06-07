//! CPU rasterization helpers for WGPU backend.
use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
use font8x8::{UnicodeFonts, BASIC_FONTS};
pub fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
pub fn rasterize_draw_commands_rgba8(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("width/height must be > 0".to_string());
    }
    let framebuffer = PixelRect { x: 0, y: 0, width, height };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for (index, command) in commands.iter().enumerate() {
        match command {
            WgpuDrawCommand::Clear { color } => {
                clear_cpu_rgba8(&mut pixels, *color);
            }
            WgpuDrawCommand::FillRect { rect, color, clip } => {
                let mut draw_rect = match rect.intersect(framebuffer) {
                    Some(value) => value,
                    None => continue,
                };
                if let Some(clip_rect) = clip {
                    draw_rect = match draw_rect.intersect(*clip_rect) {
                        Some(value) => value,
                        None => continue,
                    };
                }
                fill_rect_cpu_rgba8(&mut pixels, width, draw_rect, *color);
            }
            WgpuDrawCommand::StrokeRect { rect, color, thickness, clip } => {
                if *thickness == 0 {
                    continue;
                }
                for edge_rect in stroke_rect_edges(*rect, *thickness) {
                    let mut draw_rect = match edge_rect.intersect(framebuffer) {
                        Some(value) => value,
                        None => continue,
                    };
                    if let Some(clip_rect) = clip {
                        draw_rect = match draw_rect.intersect(*clip_rect) {
                            Some(value) => value,
                            None => continue,
                        };
                    }
                    fill_rect_cpu_rgba8(&mut pixels, width, draw_rect, *color);
                }
            }
            WgpuDrawCommand::DrawText { rect, text, color, clip } => {
                if rect.width == 0 || rect.height == 0 || text.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_text_cpu_rgba8(&mut pixels, width, *rect, text, *color, clip_rect);
            }
            WgpuDrawCommand::DrawImage { rect, rgba8, image_width, image_height, clip } => {
                if rect.width == 0 || rect.height == 0 || *image_width == 0 || *image_height == 0 {
                    continue;
                }
                if rgba8.len() != (*image_width as usize) * (*image_height as usize) * 4 {
                    return Err(format!(
                        "invalid DrawImage payload at index {index}: expected {} bytes, got {}",
                        (*image_width as usize) * (*image_height as usize) * 4,
                        rgba8.len()
                    ));
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_image_scaled_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    rgba8,
                    *image_width,
                    *image_height,
                    clip_rect,
                );
            }
        }
    }
    Ok(pixels)
}
fn effective_clip(
    framebuffer: PixelRect,
    rect: PixelRect,
    clip: Option<PixelRect>,
) -> Option<PixelRect> {
    let mut clipped = rect.intersect(framebuffer)?;
    if let Some(clip_rect) = clip {
        clipped = clipped.intersect(clip_rect)?;
    }
    Some(clipped)
}
fn clear_cpu_rgba8(pixels: &mut [u8], color: Rgba8) {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = color.r;
        chunk[1] = color.g;
        chunk[2] = color.b;
        chunk[3] = color.a;
    }
}
fn fill_rect_cpu_rgba8(pixels: &mut [u8], width: u32, rect: PixelRect, color: Rgba8) {
    let row_bytes = width as usize * 4;
    let x_start = rect.x as usize;
    let y_start = rect.y as usize;
    let x_end = (rect.x as usize) + rect.width as usize;
    let y_end = (rect.y as usize) + rect.height as usize;
    for y in y_start..y_end {
        let row_start = y * row_bytes;
        for x in x_start..x_end {
            let offset = row_start + (x * 4);
            pixels[offset] = color.r;
            pixels[offset + 1] = color.g;
            pixels[offset + 2] = color.b;
            pixels[offset + 3] = color.a;
        }
    }
}
fn stroke_rect_edges(rect: PixelRect, thickness: u32) -> [PixelRect; 4] {
    let t = thickness.min(rect.width).min(rect.height);
    [
        PixelRect { x: rect.x, y: rect.y, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.bottom() - t as i32, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.y, width: t, height: rect.height },
        PixelRect { x: rect.right() - t as i32, y: rect.y, width: t, height: rect.height },
    ]
}
fn draw_text_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    text: &str,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let glyph_w = 8i32;
    let glyph_h = 8i32;
    let columns = (rect.width as i32 / glyph_w).max(1);
    let rows = (rect.height as i32 / glyph_h).max(1);
    for (char_index, scalar) in text.chars().enumerate() {
        let grid_index = char_index as i32;
        if grid_index >= columns * rows {
            break;
        }
        let col = grid_index % columns;
        let row = grid_index / columns;
        let origin_x = rect.x + col * glyph_w;
        let origin_y = rect.y + row * glyph_h;
        let glyph = BASIC_FONTS.get(scalar).or_else(|| BASIC_FONTS.get('?')).unwrap_or([0; 8]);
        for (gy, bits) in glyph.iter().enumerate() {
            for gx in 0..8 {
                if ((bits >> gx) & 1) == 0 {
                    continue;
                }
                let px = origin_x + gx;
                let py = origin_y + gy as i32;
                if px < clip_rect.x
                    || py < clip_rect.y
                    || px >= clip_rect.right()
                    || py >= clip_rect.bottom()
                {
                    continue;
                }
                set_pixel_cpu_rgba8(pixels, width, px as u32, py as u32, color);
            }
        }
    }
}
fn draw_image_scaled_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    source_rgba8: &[u8],
    source_width: u32,
    source_height: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let x_start = clip_rect.x;
    let y_start = clip_rect.y;
    let x_end = clip_rect.right();
    let y_end = clip_rect.bottom();
    for y in y_start..y_end {
        let local_y = (y - rect.y) as u32;
        let src_y = ((local_y as u64 * source_height as u64) / rect.height as u64)
            .min(source_height.saturating_sub(1) as u64) as u32;
        for x in x_start..x_end {
            let local_x = (x - rect.x) as u32;
            let src_x = ((local_x as u64 * source_width as u64) / rect.width as u64)
                .min(source_width.saturating_sub(1) as u64) as u32;
            let src_offset = ((src_y * source_width + src_x) * 4) as usize;
            let color = Rgba8 {
                r: source_rgba8[src_offset],
                g: source_rgba8[src_offset + 1],
                b: source_rgba8[src_offset + 2],
                a: source_rgba8[src_offset + 3],
            };
            set_pixel_cpu_rgba8(pixels, width, x as u32, y as u32, color);
        }
    }
}
fn set_pixel_cpu_rgba8(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Rgba8) {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset] = color.r;
    pixels[offset + 1] = color.g;
    pixels[offset + 2] = color.b;
    pixels[offset + 3] = color.a;
}
