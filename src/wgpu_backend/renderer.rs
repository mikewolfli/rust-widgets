// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WGPU renderer implementation.
use super::commands::WgpuDrawCommand;
use super::raster::{align_to, rasterize_draw_commands_rgba8};
use super::shaders::ShaderModule;
use super::types::Rgba8;
use crate::render::gpu::{GpuCapability, GpuRenderer};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc;

/// One persistent colour-uniform arena shared by every draw of one call.
///
/// WebGPU requires a dynamic uniform offset to be a multiple of the device's
/// `min_uniform_buffer_offset_alignment` (256 on the common backends, but a
/// device may report more), so each rectangle's 16-byte colour is padded to a
/// stride read from the device rather than assumed. One buffer and one bind
/// group serve the whole call instead of one of each per rectangle (REND-02).
struct UniformArena {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    capacity: u64,
    /// Dynamic-offset stride: the device's `min_uniform_buffer_offset_alignment`,
    /// raised to hold the 16-byte colour, so every offset stays legal on any backend.
    stride: u64,
}

/// The offscreen render target for one `(width, height)`.
struct TargetCache {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// A grow-only vertex buffer reused across calls.
struct VertexCache {
    buffer: wgpu::Buffer,
    capacity: u64,
}

/// A grow-only readback buffer reused across calls.
struct ReadbackCache {
    buffer: wgpu::Buffer,
    size: u64,
}

/// Device resources a renderer keeps across calls.
///
/// # Why this exists (REND-02)
///
/// Every entry point used to allocate its uniform buffer, bind group, vertex
/// buffer, offscreen texture and full-frame readback buffer per call. Those
/// allocations are not free — a `fill_rect` batch of 40 rectangles created 40
/// uniform buffers and 40 bind groups — and the per-call texture made the GPU
/// treat every call as a new render target. This cache keeps one of each, grown
/// on demand and overwritten with `write_buffer`, because the API is called at
/// frame rate by a host that renders offscreen. The data is fully rewritten on
/// every call, so no stale content can leak between calls.
///
/// `RefCell` rather than `&mut self`: the public entry points take `&self` (and
/// `GpuRenderer` calls them that way), so the cache is the only interior-mutable
/// piece. It is not `Sync`, and the renderer is not shared across threads.
#[derive(Default)]
struct RenderCache {
    uniform: Option<UniformArena>,
    target: Option<TargetCache>,
    upload_target: Option<TargetCache>,
    vertices: Option<VertexCache>,
    readback: Option<ReadbackCache>,
}

impl RenderCache {
    /// The dynamic-offset stride for the colour arena: the device's
    /// `min_uniform_buffer_offset_alignment`, raised to hold the 16-byte colour
    /// and rounded back up to a legal multiple. A hard-coded 256 would make every
    /// draw a validation error on a device that reports 512.
    fn uniform_stride(device: &wgpu::Device) -> u64 {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        alignment.max(16).next_multiple_of(alignment.max(1))
    }

    /// Ensures the colour arena can hold `slots` colours.
    fn ensure_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        slots: usize,
    ) -> Result<(), String> {
        let stride = Self::uniform_stride(device);
        let needed = stride
            .checked_mul(slots.max(1) as u64)
            .ok_or_else(|| format!("{slots} colour slots overflow the uniform buffer size"))?;
        let needs_new = self.uniform.as_ref().map(|arena| arena.capacity < needed).unwrap_or(true);
        if needs_new {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_color_ubo"),
                size: needed,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rect_bind_group"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(16),
                    }),
                }],
            });
            self.uniform = Some(UniformArena { buffer, bind_group, capacity: needed, stride });
        }
        Ok(())
    }

    /// Ensures the offscreen target matches `width × height`.
    fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !matches {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wgpu_offscreen_target"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.target = Some(TargetCache { width, height, texture, view });
        }
    }

    /// Ensures the upload-side texture matches `width × height`.
    ///
    /// Separate from [`Self::ensure_target`] because this one is writable
    /// (`COPY_DST`): `queue.write_texture` uploads the caller's bytes into it before the copy
    /// back, which a render-attachment-only texture cannot accept.
    fn ensure_upload_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .upload_target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if !matches {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rw_wgpu_offscreen_texture"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.upload_target = Some(TargetCache { width, height, texture, view });
        }
    }

    /// Ensures the vertex buffer can hold `bytes`.
    fn ensure_vertices(&mut self, device: &wgpu::Device, bytes: u64) {
        let needs_new = self.vertices.as_ref().map(|cache| cache.capacity < bytes).unwrap_or(true);
        if needs_new {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_vertex_buffer"),
                size: bytes.max(4),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertices = Some(VertexCache { buffer, capacity: bytes.max(4) });
        }
    }

    /// Ensures the readback buffer can hold `size` bytes.
    fn ensure_readback(&mut self, device: &wgpu::Device, size: u64) {
        let needs_new = self.readback.as_ref().map(|cache| cache.size < size).unwrap_or(true);
        if needs_new {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_readback"),
                size: size.max(4),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.readback = Some(ReadbackCache { buffer, size: size.max(4) });
        }
    }
}

/// Byte length of an RGBA8 image of `width × height` pixels.
///
/// Computed in `usize` with checked multiplication: in `u32` the product wraps
/// (e.g. 65536×16384 = 2^32), which would hand the GPU a buffer one byte long for
/// a 1 GiB image. Returns an explicit error on overflow, a zero dimension, or a
/// size beyond the largest supported framebuffer (N-S-66).
fn rgba_buffer_len(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err(format!("width/height must be > 0, got {width}x{height}"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("image {width}x{height} pixel count overflows usize"))?;
    let bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| format!("image {width}x{height} RGBA size overflows usize"))?;
    if bytes > u32::MAX as usize {
        return Err(format!(
            "image {width}x{height} needs {bytes} bytes, which exceeds the largest supported \
             framebuffer ({} bytes)",
            u32::MAX
        ));
    }
    Ok(bytes)
}

/// Bytes per row of an RGBA8 image, aligned to WebGPU's copy requirement.
fn padded_row_bytes(width: u32) -> Result<u32, String> {
    let unpadded = width
        .checked_mul(4)
        .ok_or_else(|| format!("row byte width for {width} pixels overflows u32"))?;
    Ok(align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}

/// Total bytes of a padded RGBA8 readback buffer.
fn readback_frame_bytes(width: u32, height: u32) -> Result<u64, String> {
    let padded = padded_row_bytes(width)?;
    (padded as u64)
        .checked_mul(height as u64)
        .ok_or_else(|| format!("readback buffer for {width}x{height} overflows u64"))
}
/// Lightweight GPU renderer context backed by `wgpu`.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    clear_pipeline: wgpu::RenderPipeline,
    rect_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Cached shader module pipeline references (BLUE11 R5.2).
    shader_cache: HashMap<String, wgpu::ShaderModule>,
    /// Device resources reused across calls (REND-02).
    cache: RefCell<RenderCache>,
}
impl WgpuRenderer {
    /// Create a new renderer by requesting a default GPU adapter and logical device.
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::new_async())
    }
    async fn new_async() -> Result<Self, String> {
        // Build the instance from the whole degradation ladder so a host with no
        // primary driver can still reach the GL (OpenGL ES) rung, and report
        // which tier we landed on rather than failing outright.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: crate::gpu::backend_ladder::instance_backends(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let (adapter, tier) = crate::gpu::backend_ladder::select_adapter_with_gl_fallback(
            &instance,
            wgpu::PowerPreference::HighPerformance,
            None,
        )
        .await
        .ok_or_else(|| {
            // Distinguish "this host has no GPU backend at all" from "you pinned
            // a backend this host cannot provide". The second case is a user
            // error worth naming, not a mystery to debug.
            match crate::gpu::backend_ladder::backends_from_env() {
                Some(pinned) => format!(
                    "wgpu adapter request failed: WGPU_BACKEND pinned {pinned:?}, \
                     which this host does not provide"
                ),
                None => "wgpu adapter request failed on every backend tier".to_string(),
            }
        })?;

        let adapter_info = adapter.get_info();
        if tier.is_degraded() {
            // Surface the degradation once, at start-up, so a slow session is
            // explainable instead of mysterious.
            log::warn!(
                "[wgpu] using degraded backend tier: {} ({:?}, adapter '{}')",
                tier.label(),
                adapter_info.backend,
                adapter_info.name,
            );
        } else {
            log::info!(
                "[wgpu] using {} backend ({:?}, adapter '{}')",
                tier.label(),
                adapter_info.backend,
                adapter_info.name,
            );
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rw_wgpu_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| {
                format!(
                    "the GPU device could not be requested: {error}; the adapter may have been \
                     lost or the requested limits unsupported"
                )
            })?;
        // ── Create shader modules ──
        let clear_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clear_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::FULLSCREEN_QUAD_VS.into()),
        });
        let clear_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("clear_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::CLEAR_FS.into()),
        });
        let rect_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::FILL_RECT_VS.into()),
        });
        let rect_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::FILL_RECT_FS.into()),
        });

        // Create bind group layout for uniform color (shared between clear and rect pipelines)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform_color_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(wgpu::BufferSize::new(16).unwrap()),
                },
                count: None,
            }],
        });

        // ── Create clear pipeline (full-screen triangle, no vertex buffer) ──
        let clear_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("clear_pipeline_layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let clear_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("clear_pipeline"),
            layout: Some(&clear_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &clear_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &clear_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // ── Create rect pipeline (vertex buffer for quad positions) ──
        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // Vertex buffer layout: one vec2f per vertex at location 0
        let rect_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 8, // 2 * f32 = 8 bytes
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect_pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(rect_vertex_buffer_layout)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let mut shader_cache = HashMap::new();
        shader_cache.insert("clear_vs".to_string(), clear_vs_module);
        shader_cache.insert("clear_fs".to_string(), clear_fs_module);
        shader_cache.insert("rect_vs".to_string(), rect_vs_module);
        shader_cache.insert("rect_fs".to_string(), rect_fs_module);

        Ok(Self {
            device,
            queue,
            clear_pipeline,
            rect_pipeline,
            bind_group_layout,
            shader_cache,
            cache: RefCell::new(RenderCache::default()),
        })
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
    /// Render a clear frame using the GPU pipeline with a WGSL shader.
    /// Uses a full-screen triangle and a uniform buffer for the clear color.
    pub fn render_clear_gpu(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before touching the GPU: a zero-sized texture is
        // not a valid render target and an overflowing byte count must not be
        // truncated (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        let mut colour_bytes = [0u8; 16];
        for (channel, value) in color.iter().enumerate() {
            let clamped = value.clamp(0.0, 1.0) as f32;
            colour_bytes[channel * 4..channel * 4 + 4].copy_from_slice(&clamped.to_le_bytes());
        }
        let frame_bytes = readback_frame_bytes(width, height)?;

        // One cached uniform arena, target and readback buffer for every call, rather than a new
        // set of GPU resources per call (REND-02).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, 1)?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_readback(&self.device, frame_bytes);
        let uniform = cache.uniform.as_ref().expect("the arena was just ensured");
        let target = cache.target.as_ref().expect("the target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&uniform.buffer, 0, &colour_bytes);

        // Encode render pass with full-screen triangle
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_clear_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.clear_pipeline);
            rp.set_bind_group(0, &uniform.bind_group, &[0]);
            rp.draw(0..3, 0..1); // 3 vertices for full-screen triangle
        }

        let padded_bytes_per_row = padded_row_bytes(width)?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        self.queue.submit(Some(encoder.finish()));
        self.read_back(&readback.buffer, width, height, expected_len)
    }
    /// Render filled rectangles using the GPU pipeline with vertex buffer shaders.
    ///
    /// Each rect is defined as `(x, y, width, height, [r, g, b, a])` where:
    /// - `(x, y)` is the top-left corner in pixel coordinates
    /// - `(width, height)` is the rectangle size in pixels
    /// - `color` is an RGBA float array with values in `[0.0, 1.0]`
    ///
    /// Uses the WGSL vertex shader (`FILL_RECT_VS`) with a vertex buffer and the
    /// fragment shader (`FILL_RECT_FS`) with a per-rect uniform color.
    /// Encodes the fill-rect pass into the cached target and submits it.
    ///
    /// Split out from [`Self::render_fill_rect_gpu`] so a compositor can use the rendered
    /// texture directly (REND-02) instead of paying for a full `copy_texture_to_buffer` and a
    /// blocking map. The readback buffer is deliberately **not** ensured here.
    fn encode_fill_rects(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        let w = width as f32;
        let h = height as f32;

        // Generate transformed vertex positions in NDC and one clamped colour per rect.
        // Both are built directly as bytes: the previous `Vec<f32>` + `flat_map().collect()`
        // pair allocated twice per call for data that is written once (REND-02).
        let mut vertex_bytes: Vec<u8> = Vec::with_capacity(rects.len() * 48);
        let mut colours: Vec<[f32; 4]> = Vec::with_capacity(rects.len());

        for &(x, y, rw, rh, color) in rects {
            // Convert pixel coordinates to NDC: [-1, 1]
            // WebGPU/wgpu NDC: y = +1 maps to top of viewport, y = -1 to bottom
            // (OpenGL convention, not Vulkan).
            let x0 = (x as f32 / w) * 2.0 - 1.0;
            let y0 = 1.0 - (y as f32 / h) * 2.0;
            let x1 = ((x.saturating_add(rw as i32)) as f32 / w) * 2.0 - 1.0;
            let y1 = 1.0 - ((y.saturating_add(rh as i32)) as f32 / h) * 2.0;

            // Two triangles: bottom-left, bottom-right, top-left, then bottom-right, top-right, top-left
            for value in [x0, y0, x1, y0, x0, y1, x1, y0, x1, y1, x0, y1] {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }

            colours.push([
                color[0].clamp(0.0, 1.0) as f32,
                color[1].clamp(0.0, 1.0) as f32,
                color[2].clamp(0.0, 1.0) as f32,
                color[3].clamp(0.0, 1.0) as f32,
            ]);
        }

        // Cached resources for the whole call: one colour arena, one bind group, one vertex
        // buffer and one target (REND-02).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, colours.len())?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_vertices(&self.device, vertex_bytes.len() as u64);
        let uniform = cache.uniform.as_ref().expect("the arena was just ensured");
        let target = cache.target.as_ref().expect("the target was just ensured");
        let vertices = cache.vertices.as_ref().expect("the vertices were just ensured");
        let stride = uniform.stride as usize;

        // One upload for every colour, laid out at the dynamic-offset stride.
        let mut uniform_bytes = vec![0u8; stride * colours.len()];
        for (slot, colour) in colours.iter().enumerate() {
            let start = slot * stride;
            for (channel, value) in colour.iter().enumerate() {
                uniform_bytes[start + channel * 4..start + channel * 4 + 4]
                    .copy_from_slice(&value.to_le_bytes());
            }
        }
        self.queue.write_buffer(&uniform.buffer, 0, &uniform_bytes);
        self.queue.write_buffer(&vertices.buffer, 0, &vertex_bytes);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_rect_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_rect_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.rect_pipeline);
            rp.set_vertex_buffer(0, vertices.buffer.slice(..vertex_bytes.len() as u64));

            for index in 0..colours.len() {
                let offset = (index * stride) as u32;
                rp.set_bind_group(0, &uniform.bind_group, &[offset]);
                let start = (index * 6) as u32;
                let end = ((index + 1) * 6) as u32;
                rp.draw(start..end, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Renders filled rectangles into the cached target and hands the **view** to `f`.
    ///
    /// This is the texture/view frame path REND-02 asked for: a caller that composites the
    /// result on the GPU never pays for `copy_texture_to_buffer` or a blocking `map_async`.
    /// [`Self::render_fill_rect_gpu`] remains for callers that genuinely need pixels (tests,
    /// screenshots, a host with no GPU compositor).
    pub fn with_fill_rect_texture<R>(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
        f: impl FnOnce(&wgpu::TextureView) -> R,
    ) -> Result<R, String> {
        rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Err("with_fill_rect_texture needs at least one rectangle".to_string());
        }
        self.encode_fill_rects(width, height, rects)?;
        let cache = self.cache.borrow();
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        Ok(f(&target.view))
    }

    /// Render filled rectangles using the GPU pipeline with vertex buffer shaders.
    ///
    /// Each rect is defined as `(x, y, width, height, [r, g, b, a])` where:
    /// - `(x, y)` is the top-left corner in pixel coordinates
    /// - `(width, height)` is the rectangle size in pixels
    /// - `color` is an RGBA float array with values in `[0.0, 1.0]`
    ///
    /// Uses the WGSL vertex shader (`FILL_RECT_VS`) with a vertex buffer and the
    /// fragment shader (`FILL_RECT_FS`) with a per-rect uniform color.
    pub fn render_fill_rect_gpu(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Reject zero/overflowing dimensions up front so the empty-rects fast
        // path cannot allocate a wrapped-around buffer either (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        self.encode_fill_rects(width, height, rects)?;

        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().expect("the target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_rect_readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back(&readback.buffer, width, height, expected_len)
    }

    /// Maps a readback buffer, copies its rows into a tight RGBA8 image and unmaps it.
    ///
    /// The buffer belongs to [`RenderCache`], so it must be unmapped before it can be reused:
    /// leaving it mapped made the next `map_async` fail silently, which is why this is one
    /// helper rather than a copy in every entry point.
    fn read_back(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        expected_len: usize,
    ) -> Result<Vec<u8>, String> {
        let unpadded = width
            .checked_mul(4)
            .ok_or_else(|| format!("row byte width for {width} pixels overflows u32"))?
            as usize;
        let padded = padded_row_bytes(width)? as usize;
        let buffer_slice = buffer.slice(..(padded * height as usize) as u64);
        let (sender, receiver) = mpsc::channel();
        buffer_slice.map_async(
            wgpu::MapMode::Read,
            move |result: Result<(), wgpu::BufferAsyncError>| {
                let _ = sender.send(result);
            },
        );
        let _ = self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let map_result =
            receiver.recv().map_err(|_| "wgpu map_async callback channel closed".to_string())?;
        map_result.map_err(|error| {
            format!("GPU buffer could not be mapped for CPU readback: {error:?}")
        })?;
        let mapped = buffer_slice.get_mapped_range().map_err(|error| {
            format!("mapped GPU buffer range could not be read back: {error:?}")
        })?;

        // Copy row by row, stripping padding.
        let mut pixels = vec![0u8; expected_len];
        for row in 0..height as usize {
            let src_start = row * padded;
            let dst_start = row * unpadded;
            pixels[dst_start..dst_start + unpadded]
                .copy_from_slice(&mapped[src_start..src_start + unpadded]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
    /// Render stroked (outlined) rectangles using the GPU pipeline.
    /// Each stroke rect is decomposed into 4 thin filled rectangles
    /// (top, bottom, left, right edges) and rendered via the existing FillRect pipeline.
    #[allow(clippy::type_complexity)]
    pub fn render_stroke_rect_gpu(
        &self,
        width: u32,
        height: u32,
        rects: &[((i32, i32, u32, u32, u32), [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rects.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        // Decompose each stroke rect into 4 fill rects
        let mut fill_rects: Vec<((i32, i32, u32, u32), [f64; 4])> =
            Vec::with_capacity(rects.len() * 4);

        for &((x, y, rw, rh, sw), color) in rects {
            let sw_i = sw as i32;
            // Top edge
            fill_rects.push(((x, y, rw, sw), color));
            // Bottom edge
            fill_rects.push(((x, y + rh as i32 - sw_i, rw, sw), color));
            // Left edge (excludes corners already covered by top/bottom)
            let inner_h = rh.saturating_sub(sw * 2);
            if inner_h > 0 {
                fill_rects.push(((x, y + sw_i, sw, inner_h), color));
            }
            // Right edge
            if inner_h > 0 {
                fill_rects.push(((x + rw as i32 - sw_i, y + sw_i, sw, inner_h), color));
            }
        }

        // Convert fill_rects (without stroke_width) back to the (i32, i32, u32, u32, [f64; 4]) format
        let rect_input: Vec<(i32, i32, u32, u32, [f64; 4])> =
            fill_rects.into_iter().map(|((x, y, w, h), color)| (x, y, w, h, color)).collect();
        self.render_fill_rect_gpu(width, height, &rect_input)
    }

    /// Render lines using the GPU pipeline.
    /// Each line is approximated as a thin filled rectangle.
    /// Horizontal/vertical-aligned lines are rendered accurately;
    /// diagonal lines are approximated as axis-aligned thin rects.
    #[allow(clippy::type_complexity)]
    pub fn render_draw_line_gpu(
        &self,
        width: u32,
        height: u32,
        lines: &[((i32, i32, i32, i32, u32), [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if lines.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        let mut fill_rects: Vec<(i32, i32, u32, u32, [f64; 4])> = Vec::with_capacity(lines.len());

        for &((x1, y1, x2, y2, lw), color) in lines {
            let lw_i = lw as i32;
            let dx = (x2 - x1).abs();
            let dy = (y2 - y1).abs();

            if dx >= dy {
                // More horizontal: render as horizontal thin rect
                let min_x = x1.min(x2);
                let line_len = dx.max(1) as u32;
                fill_rects.push((min_x, y1 - lw_i / 2, line_len, lw, color));
            } else {
                // More vertical: render as vertical thin rect
                let min_y = y1.min(y2);
                let line_len = dy.max(1) as u32;
                fill_rects.push((x1 - lw_i / 2, min_y, lw, line_len, color));
            }
        }

        self.render_fill_rect_gpu(width, height, &fill_rects)
    }

    /// Render filled circles using the GPU pipeline.
    /// Each circle is approximated as vertical strips (thin rectangles).
    /// The more strips, the smoother the circle.
    #[allow(clippy::type_complexity)]
    pub fn render_fill_circle_gpu(
        &self,
        width: u32,
        height: u32,
        circles: &[((i32, i32, u32), [f64; 4])], // ((cx, cy, radius), color)
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if circles.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        let mut fill_rects: Vec<(i32, i32, u32, u32, [f64; 4])> = Vec::new();

        for &((cx, cy, radius), color) in circles {
            let r = radius as i32;
            // Use vertical strips to approximate the circle
            // Each strip is 1 pixel wide
            for dx in -r..=r {
                let half_height = ((r * r - dx * dx) as f64).sqrt() as i32;
                let x = cx + dx;
                let y1 = cy - half_height;
                let strip_height = (half_height * 2) as u32;
                if strip_height > 0 {
                    fill_rects.push((x, y1, 1, strip_height, color));
                }
            }
        }

        // Convert to the format expected by render_fill_rect_gpu
        let rects: Vec<(i32, i32, u32, u32, [f64; 4])> = fill_rects;
        self.render_fill_rect_gpu(width, height, &rects)
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
        // Zero dimensions have no GPU texture representation and must be an
        // explicit error, not a zero-sized texture that silently succeeds
        // (N-S-66).
        if width == 0 || height == 0 {
            return Err(format!("width/height must be > 0, got {width}x{height}"));
        }
        // Compute the expected byte length with checked wide-type arithmetic: in
        // `u32` the product wraps (65536×16384 = 2^32), so this both reports
        // overflow and validates the input length against the real size (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        if rgba8.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                rgba8.len(),
            ));
        }
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let output_buffer_size = readback_frame_bytes(width, height)?;

        // Cached upload texture and readback buffer (REND-02). Both are fully overwritten by this
        // call — `write_texture` covers the whole extent — so no previous frame can leak through.
        let mut cache = self.cache.borrow_mut();
        cache.ensure_upload_target(&self.device, width, height);
        cache.ensure_readback(&self.device, output_buffer_size);
        let target = cache.upload_target.as_ref().expect("the upload target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rw_wgpu_encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        self.read_back(&readback.buffer, width, height, expected_len)
    }
    fn upload_and_readback_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.upload_rgba8_and_readback(width, height, rgba8)
    }

    /// Get the shader module enum for integration (BLUE11 R5.2).
    pub fn shader_modules(&self) -> Vec<ShaderModule> {
        vec![
            ShaderModule::FillRect,
            ShaderModule::DrawImage,
            ShaderModule::DrawText,
            ShaderModule::FillRoundedRect,
            ShaderModule::FillCircle,
        ]
    }

    /// Return a reference to the cached wgpu `ShaderModule` for the given module name.
    pub fn get_shader_module(&self, name: &str) -> Option<&wgpu::ShaderModule> {
        self.shader_cache.get(name)
    }
}

impl GpuRenderer for WgpuRenderer {
    fn initialize(&mut self) -> Result<(), String> {
        // Device and queue are already initialized in `new()`, so this is a no-op.
        Ok(())
    }

    fn capabilities(&self) -> &[GpuCapability] {
        &[GpuCapability::Basic2D, GpuCapability::Texture]
    }

    fn has_capability(&self, capability: GpuCapability) -> bool {
        matches!(capability, GpuCapability::Basic2D | GpuCapability::Texture)
    }

    fn render_frame(&mut self) -> Result<(), String> {
        // # Why this is an error and not a silent `Ok` (D09-GPU-01)
        //
        // This renderer exposes GPU draw entry points (`render_fill_rect_gpu`,
        // `render_draw_commands_rgba8`, …) that rasterise offscreen, but it has no swapchain or
        // present path: there is no way to put a frame on a window from here. Returning `Ok(())`
        // while drawing nothing is the same kind of untruth as reporting a GPU-accelerated scene
        // backend that only round-trips through the CPU — it tells the caller a frame was
        // presented when none was. Offscreen work belongs to the `render_*_rgba8` methods; a
        // real frame loop must be added before this can succeed.
        Err("WgpuRenderer has no present/swapchain path; render offscreen with the \
             render_*_rgba8 methods instead"
            .to_string())
    }

    fn memory_usage(&self) -> u64 {
        // No reliable GPU-memory query is available through the current wgpu backend.
        0
    }

    fn vendor_info(&self) -> &str {
        // Vendor/device info is available from `wgpu::Adapter::get_info()`, but we do not
        // store the adapter after construction. This will be plumbed in a follow-up.
        "wgpu (WGPU)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A renderer on this host, or `None` when no adapter can be reached.
    ///
    /// The GPU tests are real: they submit work and read the pixels back. A host with no
    /// adapter cannot answer them, so they report the skip rather than failing — the same
    /// contract the platform code uses for an unavailable capability.
    fn renderer() -> Option<WgpuRenderer> {
        match WgpuRenderer::new() {
            Ok(renderer) => Some(renderer),
            Err(error) => {
                eprintln!("skipping GPU test: no wgpu adapter available ({error})");
                None
            }
        }
    }

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let start = ((y * width + x) * 4) as usize;
        [pixels[start], pixels[start + 1], pixels[start + 2], pixels[start + 3]]
    }

    /// The texture/view path renders without asking for pixels back (REND-02).
    #[test]
    fn texture_path_renders_a_view_without_readback() {
        let Some(renderer) = renderer() else { return };
        let rects = [(0, 0, 4, 4, [1.0, 0.0, 0.0, 1.0])];
        let seen = renderer
            .with_fill_rect_texture(4, 4, &rects, |_view| true)
            .expect("the texture path must render");
        assert!(seen, "the closure receives the rendered view");
    }

    /// A clear must fill every pixel with the requested colour.
    #[test]
    fn clear_writes_the_requested_colour_across_the_frame() {
        let Some(renderer) = renderer() else { return };
        let pixels =
            renderer.render_clear_gpu(4, 3, [1.0, 0.0, 0.0, 1.0]).expect("the clear must render");
        assert_eq!(pixels.len(), 4 * 3 * 4);
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(pixel(&pixels, 4, x, y), [255, 0, 0, 255]);
            }
        }
    }

    /// Two rectangles must each land in their own corner — and the second call through the
    /// cached buffers must produce exactly the same picture as the first (REND-02's reuse path).
    #[test]
    fn fill_rect_reuses_its_buffers_without_changing_the_picture() {
        let Some(renderer) = renderer() else { return };
        let rects = [(0, 0, 4, 4, [1.0, 0.0, 0.0, 1.0]), (4, 4, 4, 4, [0.0, 0.0, 1.0, 1.0])];
        let first = renderer.render_fill_rect_gpu(8, 8, &rects).expect("first fill");
        let second = renderer.render_fill_rect_gpu(8, 8, &rects).expect("second fill");
        assert_eq!(first, second, "cached resources must reproduce the same frame");
        assert_eq!(pixel(&first, 8, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&first, 8, 3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(&first, 8, 4, 4), [0, 0, 255, 255]);
        assert_eq!(pixel(&first, 8, 7, 7), [0, 0, 255, 255]);
        assert_eq!(pixel(&first, 8, 7, 0), [0, 0, 0, 0]);
    }

    /// A growing batch must not keep the old, smaller allocation in use: three rects need a
    /// larger colour arena than one, and all three still have to reach their own pixels.
    #[test]
    fn a_growing_batch_uses_the_enlarged_colour_arena() {
        let Some(renderer) = renderer() else { return };
        let one = [(0, 0, 4, 2, [1.0, 0.0, 0.0, 1.0])];
        renderer.render_fill_rect_gpu(4, 2, &one).expect("one rect");
        let three = [
            (0, 0, 1, 1, [1.0, 0.0, 0.0, 1.0]),
            (1, 0, 1, 1, [0.0, 1.0, 0.0, 1.0]),
            (2, 0, 1, 1, [0.0, 0.0, 1.0, 1.0]),
        ];
        let pixels = renderer.render_fill_rect_gpu(4, 2, &three).expect("three rects");
        assert_eq!(pixel(&pixels, 4, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 4, 1, 0), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 4, 2, 0), [0, 0, 255, 255]);
    }

    /// An upload must round-trip the exact bytes it was given.
    #[test]
    fn upload_round_trips_the_given_pixels() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..(4 * 4 * 4) as u8).collect();
        let output =
            renderer.upload_rgba8_and_readback(4, 4, &input).expect("the upload must round-trip");
        assert_eq!(output, input);
        // A second call reuses the cached upload texture and readback buffer.
        assert_eq!(renderer.upload_rgba8_and_readback(4, 4, &input).expect("second upload"), input);
    }
}
