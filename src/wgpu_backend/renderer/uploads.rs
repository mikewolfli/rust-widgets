// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Building a single draw's GPU payload: the texture (if any), the params uniform and the bind
//! group it needs.
//!
//! Split out of [`super::encode`] so that file is about the command stream — what is drawn, in what
//! order, with what clip — while this one is about how one draw's inputs reach the device. Every
//! entry point returns the one handle the pass needs; the bind group keeps the texture, its view
//! and the uniform alive for as long as the pass uses them.
use super::super::raster::text_coverage_into;
use super::super::types::{PixelRect, Rgba8};
use super::frame::WgpuRenderer;

/// One image draw in the command-stream pass.
///
/// The bind group keeps its texture, texture view and params buffer alive, so this is the only
/// handle the pass needs.
pub(super) struct ImageDraw {
    pub(super) bind_group: wgpu::BindGroup,
}

/// One text box in the command-stream pass.
///
/// The bind group keeps the coverage texture, its view and the params buffer alive, so this is the
/// only handle the pass needs.
pub(super) struct TextDraw {
    pub(super) bind_group: wgpu::BindGroup,
}

/// One ring / pie-wedge / rounded outline in the command-stream pass.
pub(super) struct ShapeDraw {
    pub(super) bind_group: wgpu::BindGroup,
}

/// One radial gradient in the command-stream pass.
pub(super) struct RadialDraw {
    pub(super) bind_group: wgpu::BindGroup,
}

/// One filled path in the command-stream pass.
///
/// The fan vertices live in the shared vertex buffer; this entry carries the range and the cover
/// quad's uniform.
pub(super) struct PathDraw {
    pub(super) cover_bind_group: wgpu::BindGroup,
    pub(super) first_vertex: u32,
    pub(super) vertex_count: u32,
    pub(super) even_odd: bool,
}

/// Writes `values` as little-endian `f32` into `data`, four bytes per value.
fn write_f32s(data: &mut [u8], values: &[f32]) {
    for (index, value) in values.iter().enumerate() {
        data[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

impl WgpuRenderer {
    /// Uploads one image draw's pixels and returns the bind group the pass draws it with.
    ///
    /// The texture is created per draw because the pixels belong to this frame's image. The
    /// params uniform is 32 bytes (the destination rectangle and the source grid, each a
    /// `vec4f`). Rows are uploaded at the image's own pitch: `Queue::write_texture` stages the
    /// copy itself and accepts the tightly packed buffer, so no padded copy is made here.
    pub(super) fn upload_image_draw(
        &self,
        rect: PixelRect,
        pixels: &[u8],
        source_width: u32,
        source_height: u32,
    ) -> ImageDraw {
        let size =
            wgpu::Extent3d { width: source_width, height: source_height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image_draw_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(source_width * 4),
                rows_per_image: Some(source_height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image_draw_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = [0u8; 32];
        write_f32s(
            &mut data,
            &[
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                source_width as f32,
                source_height as f32,
                0.0,
                0.0,
            ],
        );
        self.queue.write_buffer(&params, 0, &data);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image_draw_bind_group"),
            layout: &self.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &params,
                        offset: 0,
                        size: wgpu::BufferSize::new(32),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        ImageDraw { bind_group }
    }

    /// Uploads one text box's glyph coverage and returns the bind group the pass draws it with.
    ///
    /// Glyph rasterisation stays on the CPU side, through
    /// [`super::super::raster::text_coverage_into`] — the same glyph loop the CPU rasteriser
    /// blends — and the result is a one-byte-per-pixel `R8Unorm` texture. The fragment shader
    /// multiplies that coverage by the text colour, so what moves to the GPU is the per-pixel
    /// colour work and the composite, not the glyph shapes.
    pub(super) fn upload_text_draw(&self, rect: PixelRect, text: &str, color: Rgba8) -> TextDraw {
        let mut coverage = vec![0u8; rect.width as usize * rect.height as usize];
        text_coverage_into(rect, text, &mut coverage);
        let size =
            wgpu::Extent3d { width: rect.width, height: rect.height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text_coverage_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width),
                rows_per_image: Some(rect.height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_draw_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = [0u8; 32];
        write_f32s(
            &mut data,
            &[
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ],
        );
        self.queue.write_buffer(&params, 0, &data);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_draw_bind_group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &params,
                        offset: 0,
                        size: wgpu::BufferSize::new(32),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        TextDraw { bind_group }
    }

    /// Builds the bind group for one path's cover quad: its bounding box and fill colour.
    pub(super) fn upload_path_cover(&self, bounds: PixelRect, color: Rgba8) -> wgpu::BindGroup {
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("path_cover_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = [0u8; 32];
        write_f32s(
            &mut data,
            &[
                bounds.x as f32,
                bounds.y as f32,
                bounds.width as f32,
                bounds.height as f32,
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
            ],
        );
        self.queue.write_buffer(&params, 0, &data);
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("path_cover_bind_group"),
            layout: &self.path_cover_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &params,
                    offset: 0,
                    size: wgpu::BufferSize::new(32),
                }),
            }],
        })
    }

    /// Uploads one radial gradient's stop table and params, and returns the bind group it draws
    /// with.
    ///
    /// The stop table is uploaded exactly as recorded — one texel per stop — so the shader's
    /// `textureLoad` returns the byte triplet the CPU's stop lookup would have read. Geometry is
    /// the one the CPU arm derives from `rect`: the ramp is centred in it and reaches the nearest
    /// edge, so a caller cannot supply a different centre (the command carries none).
    pub(super) fn upload_radial_draw(&self, rect: PixelRect, gradient_data: &[u8]) -> RadialDraw {
        let stops = gradient_data.len() / 4;
        let texels = &gradient_data[..stops * 4];
        let size = wgpu::Extent3d { width: stops as u32, height: 1, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("radial_stops_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stops as u32 * 4),
                rows_per_image: Some(1),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // The same geometry the CPU arm derives from `rect` (see `draw_radial_gradient_cpu_rgba8`).
        let centre_x = rect.x as f32 + (rect.width as f32 - 1.0) / 2.0;
        let centre_y = rect.y as f32 + (rect.height as f32 - 1.0) / 2.0;
        let radius = ((rect.width as f32).min(rect.height as f32) / 2.0).max(1.0);
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("radial_draw_params"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = [0u8; 48];
        write_f32s(
            &mut data,
            &[
                centre_x,
                centre_y,
                radius,
                1.0 / radius,
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                stops as f32,
                0.0,
                0.0,
                0.0,
            ],
        );
        self.queue.write_buffer(&params, 0, &data);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("radial_draw_bind_group"),
            layout: &self.radial_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &params,
                        offset: 0,
                        size: wgpu::BufferSize::new(48),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        RadialDraw { bind_group }
    }

    /// Uploads one shape draw's 64-byte params and returns the bind group the pass draws it with.
    ///
    /// The layout is `[center_radius, window, color, rect]`, matching
    /// [`super::super::shaders::SHAPE_VS`] and [`super::super::shaders::SHAPE_FRAG`]. `rect` is
    /// both the quad's extent and, for the rounded-outline mode, the shape being outlined.
    pub(super) fn upload_shape_draw(
        &self,
        rect: PixelRect,
        center_radius: [f32; 4],
        window: [f32; 4],
        color: Rgba8,
    ) -> ShapeDraw {
        let params = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape_draw_params"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut data = [0u8; 64];
        write_f32s(
            &mut data,
            &[
                center_radius[0],
                center_radius[1],
                center_radius[2],
                center_radius[3],
                window[0],
                window[1],
                window[2],
                window[3],
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                color.a as f32 / 255.0,
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
            ],
        );
        self.queue.write_buffer(&params, 0, &data);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape_draw_bind_group"),
            layout: &self.shape_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &params,
                    offset: 0,
                    size: wgpu::BufferSize::new(64),
                }),
            }],
        });
        ShapeDraw { bind_group }
    }
}
