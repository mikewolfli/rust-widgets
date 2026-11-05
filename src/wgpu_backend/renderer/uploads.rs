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
use super::resources::{ParamsFamily, TextureKey};
use std::sync::Arc;

/// One image draw in the command-stream pass.
///
/// The bind group keeps its texture, texture view and params buffer alive, so this is the only
/// handle the pass needs.
pub(super) struct ImageDraw {
    pub(super) bind_group: wgpu::BindGroup,
    /// Dynamic offset of this draw's 32-byte params in the shared arena.
    pub(super) offset: u32,
    /// Whether the pass shades it with the reference's four-texel bilinear sample instead of one
    /// texel fetch. The two pipelines share this bind group's layout.
    pub(super) bilinear: bool,
}

/// One text box in the command-stream pass.
///
/// The bind group keeps the coverage texture, its view and the params buffer alive, so this is the
/// only handle the pass needs.
pub(super) struct TextDraw {
    pub(super) bind_group: wgpu::BindGroup,
}

/// One ring / pie-wedge / rounded outline in the command-stream pass.
///
/// The 64-byte params live in the frame's shared params arena; `offset` is this draw's
/// device-aligned slice, passed as a dynamic offset when the pass binds the arena.
pub(super) struct ShapeDraw {
    pub(super) offset: u32,
}

/// One radial gradient in the command-stream pass.
pub(super) struct RadialDraw {
    pub(super) bind_group: wgpu::BindGroup,
    /// Dynamic offset of this draw's 48-byte params in the shared arena.
    pub(super) offset: u32,
}

/// One general gradient in the command-stream pass: the params uniform and the ramp texture it
/// samples.
pub(super) struct GradientDraw {
    pub(super) bind_group: wgpu::BindGroup,
    /// Dynamic offset of this draw's 64-byte params in the shared arena.
    pub(super) offset: u32,
    /// Whether the pass writes the ramp colour verbatim (`REPLACE`) instead of compositing it.
    pub(super) verbatim: bool,
}

/// One filled path in the command-stream pass.
///
/// The fan vertices live in the shared vertex buffer; this entry carries the range and the cover
/// quad's uniform.
pub(super) struct PathDraw {
    /// Dynamic offset of this path's 32-byte cover params in the shared arena.
    pub(super) cover_offset: u32,
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
    /// Carves `data` out of this frame's shared params uniform (RD2-01).
    ///
    /// Returns the buffer and the aligned offset the draw's bind group binds. One buffer per
    /// frame replaces one buffer per draw; the bytes are still written before the frame is
    /// submitted, so the queue's ordering keeps them valid for the pass.
    fn params_binding(&self, data: &[u8]) -> (wgpu::Buffer, u64) {
        self.params_scratch.borrow_mut().write(&self.device, &self.queue, data)
    }

    /// Rebuilds the dynamic-offset arena bind groups when the shared buffer was replaced.
    ///
    /// The groups bind the whole arena at offset 0 with the struct's own size; each draw supplies
    /// its aligned offset at `set_bind_group` time, so one group serves every shape (or path
    /// cover) in the frame instead of one group per draw.
    fn refresh_params_groups(&self) {
        let (generation, buffer) = {
            let scratch = self.params_scratch.borrow();
            (scratch.generation(), scratch.buffer().cloned())
        };
        let Some(buffer) = buffer else {
            return;
        };
        if self.params_groups.borrow().as_ref().map(|groups| groups.generation) == Some(generation)
        {
            return;
        }
        let make = |label: &str, layout: &wgpu::BindGroupLayout, size: u64| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(size),
                    }),
                }],
            })
        };
        let shape = make("shape_params_arena", &self.shape_bind_group_layout, 64);
        let path_cover = make("path_cover_params_arena", &self.path_cover_bind_group_layout, 32);
        *self.params_groups.borrow_mut() =
            Some(super::resources::ParamsGroups { generation, shape, path_cover });
    }

    /// The arena bind groups to record into the pass, or `None` when no params were written.
    pub(super) fn params_arena_groups(&self) -> Option<(wgpu::BindGroup, wgpu::BindGroup)> {
        self.refresh_params_groups();
        self.params_groups
            .borrow()
            .as_ref()
            .map(|groups| (groups.shape.clone(), groups.path_cover.clone()))
    }

    /// A bind group over the shared params arena for one textured family.
    ///
    /// When `key` names the texture's content, the group is reused for every draw that samples the
    /// same texture in this frame (RD2-01); the per-draw params offset is supplied at bind time.
    fn textured_group(
        &self,
        family: ParamsFamily,
        key: Option<TextureKey>,
        layout: &wgpu::BindGroupLayout,
        size: u64,
        buffer: &wgpu::Buffer,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let generation = self.params_scratch.borrow().generation();
        if let Some(key) = key {
            if let Some(group) = self.params_group_cache.borrow().get(generation, family, &key) {
                return group;
            }
        }
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("params_arena_textured"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
            ],
        });
        if let Some(key) = key {
            self.params_group_cache.borrow_mut().insert(generation, family, key, group.clone());
        }
        group
    }

    /// Uploads one image draw's pixels and returns the bind group the pass draws it with.
    ///
    /// This is the path for pixel data that is generated for the draw itself (a gradient ramp
    /// flattened from stops), so the bytes cannot be reused across frames and the texture is
    /// created here. Images that arrive as a shared `Arc` go through
    /// [`Self::upload_image_draw_with_sample`] and are uploaded once per content (RD2-01).
    ///
    /// The params uniform is 32 bytes (the destination rectangle and the source grid, each a
    /// `vec4f`). Rows are uploaded at the image's own pitch: `Queue::write_texture` stages the
    /// copy itself and accepts the tightly packed buffer, so no padded copy is made here.
    pub(super) fn upload_image_draw(
        &self,
        rect: PixelRect,
        pixels: &[u8],
        source_width: u32,
        source_height: u32,
    ) -> ImageDraw {
        let texture = self.image_texture(source_width, source_height);
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
            wgpu::Extent3d { width: source_width, height: source_height, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
        let (buffer, offset) = self.params_binding(&data);
        let bind_group = self.textured_group(
            ParamsFamily::Image,
            None,
            &self.image_bind_group_layout,
            32,
            &buffer,
            &view,
        );
        ImageDraw { bind_group, offset: offset as u32, bilinear: false }
    }

    /// Uploads one image draw, stating how its fragments sample the source.
    ///
    /// `sample_step` is `(u_dx, v_dy)` — the reference's per-pixel step of the normalized sample
    /// position — and is read only by the bilinear shader; `bilinear` selects that pipeline. Both
    /// numbers travel in the params uniform's `source.zw`, which the nearest shader ignores, so the
    /// two draws keep one bind group layout.
    ///
    /// The pixels are keyed by allocation and grid (RD2-01): an image that does not change between
    /// frames is uploaded on its first draw and sampled from the cached texture afterwards, so the
    /// per-frame cost is the 32-byte params write and a bind group.
    pub(super) fn upload_image_draw_with_sample(
        &self,
        rect: PixelRect,
        pixels: &Arc<[u8]>,
        source_width: u32,
        source_height: u32,
        sample_step: (f32, f32),
        bilinear: bool,
    ) -> ImageDraw {
        let key = TextureKey::new(pixels, source_width, source_height);
        let view = self.cached_texture(pixels, source_width, source_height);
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
                sample_step.0,
                sample_step.1,
            ],
        );
        let (buffer, offset) = self.params_binding(&data);
        let bind_group = self.textured_group(
            ParamsFamily::Image,
            Some(key),
            &self.image_bind_group_layout,
            32,
            &buffer,
            &view,
        );
        ImageDraw { bind_group, offset: offset as u32, bilinear }
    }

    /// An empty `Rgba8Unorm` texture with `TEXTURE_BINDING | COPY_DST` usage.
    fn image_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image_draw_texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    /// The view of `source` at `width` x `height`, uploading it only when the content is new.
    ///
    /// The cache keys on the source allocation (kept alive by the entry) plus the grid, which is
    /// the same content key the crate's other image caches use. A hit costs a hash lookup and a
    /// handle clone; a miss creates the texture and writes the pixels once.
    fn cached_texture(&self, source: &Arc<[u8]>, width: u32, height: u32) -> wgpu::TextureView {
        let key = TextureKey::new(source, width, height);
        if let Some(view) = self.texture_cache.borrow().view(&key) {
            return view;
        }
        let texture = self.image_texture(width, height);
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            source,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.texture_cache.borrow_mut().insert(key, Arc::clone(source), texture, view.clone());
        view
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
        self.upload_coverage_draw(rect, &coverage, color)
    }

    /// Uploads a caller-supplied coverage mask and returns the bind group that draws it.
    ///
    /// This is the shaped-text half of [`Self::upload_text_draw`]: the bytes are already in the
    /// layout the reference compositor used, so this method only uploads them. `coverage` must be
    /// exactly `rect.width * rect.height` bytes; the frame encoder checks that before calling, and
    /// a mismatch here would make `write_texture` read the wrong number of rows.
    pub(super) fn upload_coverage_draw(
        &self,
        rect: PixelRect,
        coverage: &[u8],
        color: Rgba8,
    ) -> TextDraw {
        debug_assert_eq!(coverage.len(), rect.width as usize * rect.height as usize);
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
            coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(rect.width),
                rows_per_image: Some(rect.height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
        let (params, offset) = self.params_binding(&data);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_draw_bind_group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &params,
                        offset,
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

    /// Writes one path's cover params into the shared arena and returns their dynamic offset.
    ///
    /// The cover pipeline binds the arena once per frame; each path supplies this offset at
    /// `set_bind_group` time (RD2-01).
    pub(super) fn upload_path_cover(&self, bounds: PixelRect, color: Rgba8) -> u32 {
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
        let (_, offset) = self.params_binding(&data);
        offset as u32
    }

    /// Uploads one radial gradient's stop table and params, and returns the bind group it draws
    /// with.
    ///
    /// The stop table is uploaded exactly as recorded — one texel per stop — so the shader's
    /// `textureLoad` returns the byte triplet the CPU's stop lookup would have read. Geometry is
    /// the one the CPU arm derives from `rect`: the ramp is centred in it and reaches the nearest
    /// edge, so a caller cannot supply a different centre (the command carries none).
    pub(super) fn upload_radial_draw(
        &self,
        rect: PixelRect,
        gradient_data: &Arc<[u8]>,
    ) -> RadialDraw {
        let stops = gradient_data.len() / 4;
        let key = TextureKey::new(gradient_data, stops as u32, 1);
        let view = self.cached_texture(gradient_data, stops as u32, 1);
        // The same geometry the CPU arm derives from `rect` (see `draw_radial_gradient_cpu_rgba8`).
        let centre_x = rect.x as f32 + (rect.width as f32 - 1.0) / 2.0;
        let centre_y = rect.y as f32 + (rect.height as f32 - 1.0) / 2.0;
        let radius = ((rect.width as f32).min(rect.height as f32) / 2.0).max(1.0);
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
        let (buffer, offset) = self.params_binding(&data);
        let bind_group = self.textured_group(
            ParamsFamily::Radial,
            Some(key),
            &self.radial_bind_group_layout,
            48,
            &buffer,
            &view,
        );
        RadialDraw { bind_group, offset: offset as u32 }
    }

    /// Uploads one general gradient's ramp and params, and returns the bind group it draws with.
    ///
    /// The ramp is uploaded verbatim — 256 `RGBA8` entries — so the shader's `textureLoad` returns
    /// exactly the bytes the reference's own table holds. `kind`/`start`/`end` are the geometry the
    /// command carries; `rect` is both the quad and the linear ramp's anchor (`max(x, 0)` is derived
    /// in the shader, as the reference derives it).
    pub(super) fn upload_gradient_draw(
        &self,
        rect: PixelRect,
        kind: u8,
        start: (f32, f32),
        end: (f32, f32),
        ramp: &Arc<[u8]>,
        verbatim: bool,
    ) -> GradientDraw {
        let key = TextureKey::new(ramp, 256, 1);
        let view = self.cached_texture(ramp, 256, 1);
        let mut data = [0u8; 64];
        write_f32s(
            &mut data,
            &[
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
                start.0,
                start.1,
                0.0,
                0.0,
                end.0,
                end.1,
                0.0,
                0.0,
                kind as f32,
                0.0,
                0.0,
                0.0,
            ],
        );
        let (buffer, offset) = self.params_binding(&data);
        let bind_group = self.textured_group(
            ParamsFamily::Gradient,
            Some(key),
            &self.gradient_bind_group_layout,
            64,
            &buffer,
            &view,
        );
        GradientDraw { bind_group, offset: offset as u32, verbatim }
    }

    /// Uploads one shape draw's 64-byte params and returns their dynamic offset into the shared
    /// arena.
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
        let (_, offset) = self.params_binding(&data);
        ShapeDraw { offset: offset as u32 }
    }
}
