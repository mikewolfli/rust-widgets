//! Optional WGPU-based GPU renderer backend.
//!
//! Enable with Cargo feature: `gpu-wgpu`.
use font8x8::{UnicodeFonts, BASIC_FONTS};
use std::sync::mpsc;
/// Integer rectangle in pixel space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl PixelRect {
    fn right(self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }
    fn bottom(self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }
    fn intersect(self, other: PixelRect) -> Option<PixelRect> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= left || bottom <= top {
            return None;
        }
        Some(PixelRect {
            x: left,
            y: top,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
}
/// 8-bit RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
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
/// Lightweight GPU renderer context backed by `wgpu`.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
}
impl WgpuRenderer {
    /// Create a new renderer by requesting a default GPU adapter and logical device.
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::new_async())
    }
    async fn new_async() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "wgpu adapter request failed".to_string())?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("rust_widgets_wgpu_device"),
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .map_err(|error| format!("wgpu request_device failed: {error}"))?;
        Ok(Self { device, queue })
    }
    /// Render one offscreen RGBA frame by clearing a texture with the given color and read pixels back.
    pub fn render_clear_rgba8(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        self.render_draw_commands_rgba8(
            width,
            height,
            &[WgpuDrawCommand::Clear {
                color: Rgba8 {
                    r: (color[0].clamp(0.0, 1.0) * 255.0) as u8,
                    g: (color[1].clamp(0.0, 1.0) * 255.0) as u8,
                    b: (color[2].clamp(0.0, 1.0) * 255.0) as u8,
                    a: (color[3].clamp(0.0, 1.0) * 255.0) as u8,
                },
            }],
        )
    }
    /// Render command list with deterministic ordering and clipping.
    pub fn render_draw_commands_rgba8(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let pixels = rasterize_draw_commands_rgba8(width, height, commands)?;
        self.upload_and_readback_rgba8(width, height, &pixels)
    }
    /// Upload a full RGBA8 frame to GPU texture and read it back deterministically.
    pub fn upload_rgba8_and_readback(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Result<Vec<u8>, String> {
        if rgba8.len() != (width * height * 4) as usize {
            return Err("rgba8 input length does not match width*height*4".to_string());
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rust_widgets_wgpu_offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let padded_bytes_per_row =
            align_to(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let output_buffer_size = padded_bytes_per_row as u64 * height as u64;
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba8,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(unpadded_bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rust_widgets_wgpu_readback_buffer"),
            size: output_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rust_widgets_wgpu_encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let buffer_slice = output_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(
            wgpu::MapMode::Read,
            move |result: Result<(), wgpu::BufferAsyncError>| {
                let _ = sender.send(result);
            },
        );
        self.device.poll(wgpu::Maintain::Wait);
        let map_result = receiver
            .recv()
            .map_err(|_| "wgpu map_async callback channel closed".to_string())?;
        map_result.map_err(|error| format!("wgpu buffer map failed: {error:?}"))?;
        let mapped = buffer_slice.get_mapped_range();
        let mut pixels = vec![0u8; (width * height * bytes_per_pixel) as usize];
        for row in 0..height as usize {
            let src_start = row * padded_bytes_per_row as usize;
            let src_end = src_start + unpadded_bytes_per_row as usize;
            let dst_start = row * unpadded_bytes_per_row as usize;
            let dst_end = dst_start + unpadded_bytes_per_row as usize;
            pixels[dst_start..dst_end].copy_from_slice(&mapped[src_start..src_end]);
        }
        drop(mapped);
        output_buffer.unmap();
        Ok(pixels)
    }
    fn upload_and_readback_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.upload_rgba8_and_readback(width, height, rgba8)
    }
}
fn clear_cpu_rgba8(pixels: &mut [u8], color: Rgba8) {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = color.r;
        chunk[1] = color.g;
        chunk[2] = color.b;
        chunk[3] = color.a;
    }
}
fn rasterize_draw_commands_rgba8(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("width/height must be > 0".to_string());
    }
    let framebuffer = PixelRect {
        x: 0,
        y: 0,
        width,
        height,
    };
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
            WgpuDrawCommand::StrokeRect {
                rect,
                color,
                thickness,
                clip,
            } => {
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
            WgpuDrawCommand::DrawText {
                rect,
                text,
                color,
                clip,
            } => {
                if rect.width == 0 || rect.height == 0 || text.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_text_cpu_rgba8(&mut pixels, width, *rect, text, *color, clip_rect);
            }
            WgpuDrawCommand::DrawImage {
                rect,
                rgba8,
                image_width,
                image_height,
                clip,
            } => {
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
        let glyph = BASIC_FONTS
            .get(scalar)
            .or_else(|| BASIC_FONTS.get('?'))
            .unwrap_or([0; 8]);
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
        PixelRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: t,
        },
        PixelRect {
            x: rect.x,
            y: rect.bottom() - t as i32,
            width: rect.width,
            height: t,
        },
        PixelRect {
            x: rect.x,
            y: rect.y,
            width: t,
            height: rect.height,
        },
        PixelRect {
            x: rect.right() - t as i32,
            y: rect.y,
            width: t,
            height: rect.height,
        },
    ]
}
fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_raster_draw_text_honors_clipping() {
        let pixels = rasterize_draw_commands_rgba8(
            16,
            16,
            &[
                WgpuDrawCommand::Clear {
                    color: Rgba8 {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    },
                },
                WgpuDrawCommand::DrawText {
                    rect: PixelRect {
                        x: 0,
                        y: 0,
                        width: 16,
                        height: 16,
                    },
                    text: "AB".to_string(),
                    color: Rgba8 {
                        r: 220,
                        g: 10,
                        b: 40,
                        a: 255,
                    },
                    clip: Some(PixelRect {
                        x: 0,
                        y: 0,
                        width: 8,
                        height: 16,
                    }),
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
        let source = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let pixels = rasterize_draw_commands_rgba8(
            4,
            4,
            &[WgpuDrawCommand::DrawImage {
                rect: PixelRect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                },
                rgba8: source,
                image_width: 2,
                image_height: 2,
                clip: None,
            }],
        )
        .expect("image raster should succeed");
        let sample = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * 4 + x) * 4) as usize;
            [
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            ]
        };
        assert_eq!(sample(0, 0), [255, 0, 0, 255]);
        assert_eq!(sample(3, 0), [0, 255, 0, 255]);
        assert_eq!(sample(0, 3), [0, 0, 255, 255]);
        assert_eq!(sample(3, 3), [255, 255, 255, 255]);
    }
    #[test]
    fn command_raster_draw_image_invalid_payload_is_explicit_error() {
        let error = rasterize_draw_commands_rgba8(
            4,
            4,
            &[WgpuDrawCommand::DrawImage {
                rect: PixelRect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 4,
                },
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
