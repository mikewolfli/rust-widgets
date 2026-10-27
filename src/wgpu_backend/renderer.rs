// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WGPU renderer implementation.
use super::commands::WgpuDrawCommand;
use super::raster::{align_to, rasterize_draw_commands_into_rgba8, rasterize_draw_commands_rgba8};
use super::shaders::ShaderModule;
use super::types::{PixelRect, Rgba8};
use crate::render::gpu::{GpuCapability, GpuRenderer};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

/// Upper bound on a single blocking GPU readback.
///
/// The readback is synchronous by design: every entry point returns owned pixels. Before this
/// bound existed, `device.poll` waited forever, so a lost device or a stuck submission hung the
/// caller instead of reporting an error (G-63).
const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(10);

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
    ///
    /// Only the per-rectangle path reads this; an instanced run carries colour in its instance
    /// records instead.
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
///
/// The same cache holds both the per-rectangle vertex positions and the instanced quad records:
/// both uses are `VERTEX`-usage buffers written once per call, and sharing one grow-only
/// allocation is what keeps an instanced run from allocating once per quad.
struct VertexCache {
    buffer: wgpu::Buffer,
    capacity: u64,
}

/// A grow-only readback buffer reused across calls.
struct ReadbackCache {
    buffer: wgpu::Buffer,
    size: u64,
}

/// The two ping-pong targets and per-direction bind groups of the blur pass.
///
/// The pass reads a texture and writes one that is not also bound as its attachment, so two
/// textures are required. Both are `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_*`: the first is
/// uploaded into and read back from, the second is only ever the horizontal pass's target. Cached
/// per `(width, height)` and rebuilt when the size changes (REND-02's reuse rule).
struct BlurTargets {
    width: u32,
    height: u32,
    source: wgpu::Texture,
    source_view: wgpu::TextureView,
    scratch_view: wgpu::TextureView,
    horizontal: wgpu::BindGroup,
    vertical: wgpu::BindGroup,
    horizontal_params: wgpu::Buffer,
    vertical_params: wgpu::Buffer,
}

/// The model-view-projection uniform used by the instanced quad pipeline.
///
/// One matrix serves a whole call, so this is not a dynamic-offset arena. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
struct TransformedUniform {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// The colour-and-depth pair the `3d` mesh pipeline renders into.
///
/// The view owns its texture, so the texture handle itself does not need to be stored. Cached per
/// `(width, height)` like the other targets (REND-02) and rebuilt when the size changes.
#[cfg(feature = "3d")]
struct DepthTarget {
    width: u32,
    height: u32,
    view: wgpu::TextureView,
}

/// Grow-only uniform arena for the SDF circle pipeline (G-219).
///
/// Each circle consumes two 16-byte slots at the dynamic-offset stride — the fill colour at
/// `2 * i` and the radius/centre SDF parameters at `2 * i + 1` — so one buffer and one bind
/// group serve every circle of a frame. The bind group binds the same buffer at both
/// entries; the per-draw dynamic offsets select the slots.
struct CircleUniformArena {
    buffer: wgpu::Buffer,
    capacity: u64,
    bind_group: wgpu::BindGroup,
}

/// Grow-only CPU frame storage for one render path (G-221).
///
/// The rasteriser zero-fills the frame, so a reused allocation is byte-identical to a fresh
/// one; only the capacity is retained. A separate cell per path lets a caller keep one frame
/// borrowed (for an upload) without blocking a different path's borrow.
struct CpuFrame {
    bytes: RefCell<Vec<u8>>,
}

impl CpuFrame {
    fn new() -> Self {
        Self { bytes: RefCell::new(Vec::new()) }
    }

    /// Rasterises `commands` into this path's buffer and lends the frame back to the caller.
    ///
    /// The buffer grows only. `bytes` is re-borrowed immutably for the returned guard after
    /// the rasteriser has finished, so the caller cannot observe a half-drawn frame.
    fn rasterize(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Ref<'_, [u8]>, String> {
        let byte_len = rgba_buffer_len(width, height)?;
        {
            let mut bytes = self.bytes.borrow_mut();
            if bytes.len() < byte_len {
                bytes.resize(byte_len, 0);
            }
            rasterize_draw_commands_into_rgba8(width, height, commands, &mut bytes[..byte_len])?;
        }
        Ok(Ref::map(self.bytes.borrow(), |bytes| &bytes[..byte_len]))
    }
}

/// One encoded draw of the command-stream pass (G-218).
///
/// The dispatcher packs the frame's geometry first and records the draw order here, so the
/// render pass replays painter order exactly without re-deciding anything while encoding.
enum DrawStep {
    /// Full-screen repaint through the clear pipeline; the payload is the colour slot index
    /// in the shared uniform arena.
    Clear(usize),
    /// Instanced quad run over the instance range `[start, end)` of the packed instance
    /// region.
    Rects(usize, usize),
    /// One SDF circle draw over the bounding quad at `quad_index`.
    Circle(usize),
}

/// CPU-side packing buffers reused across calls (G-216).
///
/// Every entry point used to build a fresh `Vec` for the bytes it uploads: instance records,
/// per-rect NDC vertices, uniform staging, mesh vertices and indices. The packing itself is
/// unavoidable — the GPU consumes a byte layout — but the allocation is not: these buffers
/// are cleared and refilled each call, so the capacity the peak frame required is retained
/// across frames instead of being freed and re-grown.
#[derive(Default)]
struct PackScratch {
    /// Instanced quad records, `INSTANCE_BYTES` per instance.
    instances: Vec<u8>,
    /// Per-rect NDC vertices, 48 bytes (twelve `f32`) per rectangle.
    vertices: Vec<u8>,
    /// Colour slots staged for the shared uniform arena, one aligned slot per rectangle.
    uniform: Vec<u8>,
    /// SDF circle bounding quads, six 8-byte vertices per circle (G-219).
    circle_vertices: Vec<u8>,
    /// SDF circle uniform slots, two aligned slots per circle (G-219).
    circle_uniforms: Vec<u8>,
    /// The command-stream dispatcher's draw order (G-218).
    plan: Vec<DrawStep>,
    /// Mesh vertices, 32 bytes per vertex (`3d`).
    #[cfg(feature = "3d")]
    mesh_vertices: Vec<u8>,
    /// Mesh indices as little-endian `u32` (`3d`).
    #[cfg(feature = "3d")]
    mesh_indices: Vec<u8>,
}

/// Creates a 64-byte matrix uniform and the bind group that exposes it at group 0, binding 0.
///
/// Both the instanced quad pipeline and the mesh pipeline use one matrix per call rather than a
/// dynamic-offset arena, so this is the one shape their resources share. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
fn create_matrix_uniform(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
) -> TransformedUniform {
    let buffer_label = format!("{label}_ubo");
    let bind_group_label = format!("{label}_bind_group");
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(buffer_label.as_str()),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(bind_group_label.as_str()),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(64),
            }),
        }],
    });
    TransformedUniform { buffer, bind_group }
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
    /// Ping-pong targets and bind groups for the separable blur pass.
    blur: Option<BlurTargets>,
    /// Mvp uniform and bind group for the instanced quad pipeline.
    transformed_uniform: Option<TransformedUniform>,
    /// Grow-only uniform arena for the SDF circle pipeline (G-219).
    circle_uniforms: Option<CircleUniformArena>,
    /// CPU-side packing buffers refilled each call instead of reallocated (G-216).
    pack: PackScratch,
    /// The `Depth32Float` attachment for mesh draws.
    #[cfg(feature = "3d")]
    depth_target: Option<DepthTarget>,
    /// Mvp uniform and bind group for the mesh pipeline.
    #[cfg(feature = "3d")]
    mesh_uniform: Option<TransformedUniform>,
    /// Grow-only index buffer for mesh draws.
    #[cfg(feature = "3d")]
    mesh_indices: Option<VertexCache>,
}

// Retain headroom without exceeding the device limit; buffers remain grow-only.
fn grown_buffer_capacity(current: u64, needed: u64, limit: u64) -> u64 {
    let grown = current.saturating_add(current / 2).max(needed).max(4);
    let rounded = grown.checked_add(65_535).map(|size| size / 65_536 * 65_536).unwrap_or(limit);
    rounded.min(limit).max(needed.max(4))
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
            if needed > device.limits().max_buffer_size {
                return Err(format!("{needed} uniform bytes exceed the device buffer limit"));
            }
            let capacity = grown_buffer_capacity(
                self.uniform.as_ref().map_or(0, |arena| arena.capacity),
                needed,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_color_ubo"),
                size: capacity,
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
            self.uniform = Some(UniformArena { buffer, bind_group, capacity, stride });
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
            let capacity = grown_buffer_capacity(
                self.vertices.as_ref().map_or(0, |cache| cache.capacity),
                bytes,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("rect_vertex_buffer"),
                size: capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertices = Some(VertexCache { buffer, capacity });
        }
    }

    /// Ensures the readback buffer can hold `size` bytes.
    fn ensure_readback(&mut self, device: &wgpu::Device, size: u64) {
        let needs_new = self.readback.as_ref().map(|cache| cache.size < size).unwrap_or(true);
        if needs_new {
            let capacity = grown_buffer_capacity(
                self.readback.as_ref().map_or(0, |cache| cache.size),
                size,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("wgpu_readback"),
                size: capacity,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.readback = Some(ReadbackCache { buffer, size: capacity });
        }
    }

    /// Ensures the blur ping-pong targets and both directional bind groups match `width × height`.
    fn ensure_blur_targets(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        width: u32,
        height: u32,
    ) {
        let matches = self
            .blur
            .as_ref()
            .map(|targets| targets.width == width && targets.height == height)
            .unwrap_or(false);
        if matches {
            return;
        }
        let make_texture = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let source = make_texture("wgpu_blur_source");
        let scratch = make_texture("wgpu_blur_scratch");
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());
        let make_params = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 32,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let horizontal_params = make_params("wgpu_blur_horizontal_params");
        let vertical_params = make_params("wgpu_blur_vertical_params");
        let make_bind_group = |label: &str, view: &wgpu::TextureView, buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(32),
                        }),
                    },
                ],
            })
        };
        let horizontal = make_bind_group("wgpu_blur_horizontal", &source_view, &horizontal_params);
        let vertical = make_bind_group("wgpu_blur_vertical", &scratch_view, &vertical_params);
        self.blur = Some(BlurTargets {
            width,
            height,
            source,
            source_view,
            scratch_view,
            horizontal,
            vertical,
            horizontal_params,
            vertical_params,
        });
    }

    /// Ensures the instanced-quad mvp uniform and its bind group exist.
    ///
    /// The buffer never grows: every call writes one 64-byte matrix. It is created once per
    /// renderer, matching the rest of this cache (REND-02).
    fn ensure_transformed_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) {
        if self.transformed_uniform.is_some() {
            return;
        }
        self.transformed_uniform = Some(create_matrix_uniform(device, layout, "transformed_mvp"));
    }

    /// Ensures the mesh mvp uniform and its bind group exist.
    ///
    /// Separate from [`Self::ensure_transformed_uniform`] so a 2D transformed draw and a mesh draw
    /// never race for the same 64-byte buffer within one frame.
    #[cfg(feature = "3d")]
    fn ensure_mesh_uniform(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
        if self.mesh_uniform.is_some() {
            return;
        }
        self.mesh_uniform = Some(create_matrix_uniform(device, layout, "mesh_mvp"));
    }

    /// Ensures the `Depth32Float` attachment matches `(width, height)`.
    #[cfg(feature = "3d")]
    fn ensure_depth_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let matches = self
            .depth_target
            .as_ref()
            .map(|target| target.width == width && target.height == height)
            .unwrap_or(false);
        if matches {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mesh_depth_texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth_target = Some(DepthTarget { width, height, view });
    }

    /// Ensures the mesh index buffer holds at least `bytes`.
    #[cfg(feature = "3d")]
    fn ensure_mesh_indices(&mut self, device: &wgpu::Device, bytes: u64) {
        if self.mesh_indices.as_ref().map(|cache| cache.capacity >= bytes).unwrap_or(false) {
            return;
        }
        let capacity = bytes.max(256).next_power_of_two();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh_indices"),
            size: capacity,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.mesh_indices = Some(VertexCache { buffer, capacity });
    }

    /// Ensures the circle arena can hold `circles` two-slot records (G-219).
    ///
    /// The two bind-group entries point at the same buffer; the per-draw dynamic offsets
    /// select the colour and parameter slots. The stride is the device's dynamic-offset
    /// alignment, exactly like the colour arena, so every offset stays legal on any backend.
    fn ensure_circle_uniforms(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        circles: usize,
    ) -> Result<(), String> {
        let stride = Self::uniform_stride(device);
        let slots = (circles.saturating_mul(2)).max(1) as u64;
        let needed = stride.checked_mul(slots).ok_or_else(|| {
            format!("{slots} circle uniform slots overflow the uniform buffer size")
        })?;
        let needs_new =
            self.circle_uniforms.as_ref().map(|arena| arena.capacity < needed).unwrap_or(true);
        if needs_new {
            if needed > device.limits().max_buffer_size {
                return Err(format!("{needed} uniform bytes exceed the device buffer limit"));
            }
            let capacity = grown_buffer_capacity(
                self.circle_uniforms.as_ref().map_or(0, |arena| arena.capacity),
                needed,
                device.limits().max_buffer_size,
            );
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("circle_sdf_ubo"),
                size: capacity,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("circle_sdf_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(16),
                        }),
                    },
                ],
            });
            self.circle_uniforms = Some(CircleUniformArena { buffer, capacity, bind_group });
        }
        Ok(())
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

/// Quantises a unit-interval colour to the 8-bit RGBA the rasteriser consumes.
fn rgba8_from_unit(color: [f64; 4]) -> Rgba8 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0) as u8;
    Rgba8 { r: channel(color[0]), g: channel(color[1]), b: channel(color[2]), a: channel(color[3]) }
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

/// Packs a row-major [`crate::render::math3d::Mat4`] as the column-major bytes WGSL expects.
///
/// WGSL's `mat4x4f` uniform layout is column-major, while the maths module stores rows to
/// match the affine matrix's natural reading order. Transposing here is what keeps the two
/// conventions from being silently swapped — the failure mode would be a transposed transform
/// that still compiles and still draws *something*.
#[cfg(feature = "3d")]
#[allow(clippy::needless_range_loop)] // row and column index the source row-major matrix.
fn mvp_column_major_bytes(m: &crate::render::math3d::Mat4) -> [u8; 64] {
    let mut bytes = [0u8; 64];
    for column in 0..4 {
        for row in 0..4 {
            let value = m.m[row * 4 + column];
            let start = (column * 4 + row) * 4;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

/// Bytes of one instanced quad record: two `vec4f` corner pairs and one `vec4f` colour.
///
/// Kept as a named constant because the vertex-buffer stride, the writer and the chunk offsets
/// must agree exactly; a mismatch is a pipeline validation error rather than a silent misread.
const INSTANCE_BYTES: usize = 48;

/// Upper bound on the instances submitted by one instanced draw.
///
/// A longer run is streamed in cap-sized chunks from the same grow-only buffer. The chunks are
/// drawn in order, so splitting never reorders the run and never allocates per quad.
const MAX_INSTANCES_PER_DRAW: usize = 4096;

/// Column-major bytes of the pixel-to-clip matrix for a `width × height` target.
///
/// `x` maps `0..width` to `-1..1` and `y` maps `0..height` to `1..-1`, matching the renderer's
/// top-left origin. These are the bytes `Mat4::from_affine(ortho)` produces, written directly so
/// the 2D rectangle path can use the instanced pipeline without the `3d` feature's maths module.
fn ortho_mvp_bytes(width: u32, height: u32) -> [u8; 64] {
    let w = width as f32;
    let h = height as f32;
    // Column-major: scale x, flip and scale y, keep z, then translate to the clip-space corner.
    let values =
        [2.0 / w, 0.0, 0.0, 0.0, 0.0, -2.0 / h, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0];
    let mut bytes = [0u8; 64];
    for (index, value) in values.iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Bytes of one SDF circle draw's vertex data: six `vec2f` vertices bounding the disc (G-219).
const CIRCLE_QUAD_BYTES: usize = 48;

/// Clamped bounding quad of a disc, in the pixel-centre frame the SDF expects (G-219).
///
/// Returns `(x0, y0, x1, y1)` for a disc whose centre sits at `(cx + 0.5, cy + 0.5)`: a pixel
/// centre at `(k + 0.5, j + 0.5)` lies within `radius` of that centre exactly when the software
/// rasteriser paints pixel `(k, j)`, so this quad covers precisely the pixels the disc can
/// touch. The quad is clamped to the viewport — fragments outside it are clipped anyway, and
/// clamping keeps visible-pixel positions exact because interpolation is affine in screen
/// space — and a disc that covers no pixel returns `None`, matching the software path's
/// off-screen skip. The arithmetic runs in `i64` so extreme centres and radii cannot wrap.
fn circle_quad(
    center: (i32, i32),
    radius: u32,
    width: u32,
    height: u32,
) -> Option<(i64, i64, i64, i64)> {
    let (cx, cy) = (center.0 as i64, center.1 as i64);
    let r = radius as i64;
    let x0 = (cx - r).max(0);
    let y0 = (cy - r).max(0);
    let x1 = (cx + r + 1).min(width as i64);
    let y1 = (cy + r + 1).min(height as i64);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    Some((x0, y0, x1, y1))
}

/// Writes one circle's bounding quad (six vertices) and its two uniform slots — the fill
/// colour, then the radius/centre parameters [`super::shaders::CIRCLE_FRAG`] reads — into the
/// reused packing buffers (G-219).
///
/// `uniforms` is indexed at the aligned slot stride: slot `2 * index` is the colour, slot
/// `2 * index + 1` the `(radius, centre.x, centre.y)` parameter vector. The centre carries the
/// half-pixel offset so the SDF's pixel-centre coordinates line up with the vertex-frame
/// positions the vertex stage interpolates.
fn pack_circle(
    pack_vertices: &mut Vec<u8>,
    pack_uniforms: &mut Vec<u8>,
    stride: usize,
    index: usize,
    center: (i32, i32),
    radius: u32,
    colour: &[f64; 4],
    quad: (i64, i64, i64, i64),
) {
    let (x0, y0, x1, y1) = quad;
    let mut vertices = [0u8; CIRCLE_QUAD_BYTES];
    // Two triangles from the quad: (0, 1, 2) and (0, 2, 3) in corner order
    // (x0,y0), (x1,y0), (x1,y1), (x0,y1).
    for (slot, (x, y)) in
        [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)].iter().enumerate()
    {
        vertices[slot * 8..slot * 8 + 4].copy_from_slice(&(*x as f32).to_le_bytes());
        vertices[slot * 8 + 4..slot * 8 + 8].copy_from_slice(&(*y as f32).to_le_bytes());
    }
    pack_vertices.extend_from_slice(&vertices);

    let mut colour_slot = [0u8; 16];
    for (channel, value) in colour.iter().enumerate() {
        colour_slot[channel * 4..channel * 4 + 4]
            .copy_from_slice(&((*value).clamp(0.0, 1.0) as f32).to_le_bytes());
    }
    let mut params_slot = [0u8; 16];
    params_slot[0..4].copy_from_slice(&(radius as f32).to_le_bytes());
    params_slot[4..8].copy_from_slice(&((center.0 as f32) + 0.5).to_le_bytes());
    params_slot[8..12].copy_from_slice(&((center.1 as f32) + 0.5).to_le_bytes());
    let base = index * 2 * stride;
    pack_uniforms[base..base + 16].copy_from_slice(&colour_slot);
    pack_uniforms[base + stride..base + stride + 16].copy_from_slice(&params_slot);
}

/// Whether a command stream can be drawn entirely by the GPU pipelines (G-218).
///
/// Only the classes the existing pipelines cover qualify: whole-frame clears, unclipped filled
/// rectangles (the instanced quad pipeline) and unclipped filled circles (the SDF circle
/// pipeline). [`WgpuDrawCommand::SetBlendMode`] is allowed only at `Normal` (mode `0`),
/// because every pipeline writes with `REPLACE` blending, which is exactly the Normal write;
/// any other mode needs blending the pipelines do not implement. Clips, text, images, paths,
/// gradients, shadows and strokes stay on the CPU rasteriser, which remains the fallback.
fn commands_gpu_dispatchable(commands: &[WgpuDrawCommand]) -> bool {
    commands.iter().all(|command| match command {
        WgpuDrawCommand::Clear { .. } => true,
        WgpuDrawCommand::FillRect { clip: None, .. } => true,
        WgpuDrawCommand::FillCircle { clip: None, .. } => true,
        WgpuDrawCommand::SetBlendMode { mode } => *mode == 0,
        _ => false,
    })
}

/// Packs a run of quads into the 48-byte-per-instance layout described on
/// [`super::shaders::FILL_RECT_VS_TRANSFORMED`], appending to the reused scratch buffer
/// `out` (G-216).
///
/// Each record is the four corners in perimeter order, then straight RGBA with every channel
/// clamped to `0.0..=1.0`. Returns an explicit error rather than wrapping if the run is too
/// large to address, so a hostile count cannot produce a short buffer. `out` is cleared first;
/// the capacity a peak frame built up is retained across calls.
fn pack_instanced_quads(
    quads: &[([crate::core::Point; 4], [f64; 4])],
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let capacity = quads
        .len()
        .checked_mul(INSTANCE_BYTES)
        .ok_or_else(|| format!("{} quads overflow the instance buffer size", quads.len()))?;
    out.clear();
    out.reserve(capacity);
    for (corners, colour) in quads {
        for point in corners {
            out.extend_from_slice(&(point.x as f32).to_le_bytes());
            out.extend_from_slice(&(point.y as f32).to_le_bytes());
        }
        for channel in colour {
            out.extend_from_slice(&((*channel).clamp(0.0, 1.0) as f32).to_le_bytes());
        }
    }
    Ok(())
}

/// Lightweight GPU renderer context backed by `wgpu`.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    clear_pipeline: wgpu::RenderPipeline,
    /// The 2D per-rectangle pipeline. Used only for the single-rectangle case; a longer run goes
    /// through the instanced quad pipeline instead (G-64).
    rect_pipeline: wgpu::RenderPipeline,
    /// The separable-blur pipeline: one full-screen triangle, no vertex buffer, two directional
    /// passes. A 2D effect, so it is compiled in every build rather than behind `3d`.
    blur_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout of the blur pass: a non-filterable input texture and a 32-byte
    /// uniform (`direction`, `radius`, and the G-220 tint block the vertical pass composites).
    blur_bind_group_layout: wgpu::BindGroupLayout,
    /// Instanced quad pipeline. It renders a whole run of rectangles (and, with `3d`, arbitrary
    /// transformed quads) in `MAX_INSTANCES_PER_DRAW`-sized draws (G-64).
    transformed_pipeline: wgpu::RenderPipeline,
    /// The vertex-stage layout that carries the mvp matrix (group 0 of the instanced pipeline).
    transformed_bind_group_layout: wgpu::BindGroupLayout,
    /// The SDF circle pipeline: one bounding quad per circle, shaded by a distance field in
    /// the fragment stage (G-219).
    circle_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the SDF circle pipeline: two dynamic-offset uniforms (the fill
    /// colour and the radius/centre parameters) at group 0; the mvp matrix rides group 1.
    circle_bind_group_layout: wgpu::BindGroupLayout,
    /// Depth-tested triangle pipeline for mesh draws (`3d` only).
    #[cfg(feature = "3d")]
    mesh_pipeline: wgpu::RenderPipeline,
    /// Cached shader module pipeline references (BLUE11 R5.2).
    shader_cache: HashMap<String, wgpu::ShaderModule>,
    /// Device resources reused across calls (REND-02).
    cache: RefCell<RenderCache>,
    /// Reusable CPU frame for whole-frame clears (G-221).
    clear_frame: CpuFrame,
    /// Reusable CPU frame for command streams (G-221).
    commands_frame: CpuFrame,
    /// Reusable CPU frame for the dispatcher's inner fallback, which runs while `cache` is
    /// already borrowed, so it cannot share `commands_frame` (G-221).
    fallback_frame: CpuFrame,
    /// What `wgpu` reported for the selected adapter, kept so [`GpuRenderer::vendor_info`] can
    /// answer with the real device name instead of a constant (G-232).
    adapter_info: wgpu::AdapterInfo,
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
        let transformed_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_vs_transformed"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::FILL_RECT_VS_TRANSFORMED.into()),
        });
        let instanced_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_fs_instanced"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::FILL_RECT_INSTANCED_FS.into()),
        });
        #[cfg(feature = "3d")]
        let mesh_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh3d_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::MESH3D_VS.into()),
        });
        #[cfg(feature = "3d")]
        let mesh_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh3d_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::MESH3D_FS.into()),
        });
        let blur_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::BLUR_VS.into()),
        });
        let blur_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::BLUR_FS.into()),
        });
        let circle_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("circle_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::CIRCLE_VS.into()),
        });
        let circle_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("circle_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::CIRCLE_FRAG.into()),
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

        // The matrix lives in its own group because the instanced fragment stage takes its
        // colour per instance rather than from a group-0 uniform. With group 0 free, the
        // instanced pipeline binds the matrix at group 0 directly.
        let transformed_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("transformed_mvp_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(wgpu::BufferSize::new(64).unwrap()),
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

        // ── Create the separable blur pipeline ──
        //
        // The input is read with `textureLoad`, so its sample type is non-filterable and no
        // sampler is bound. One draw call per direction filters the whole target; the two
        // bind groups differ only in the uniform that names the axis and in which texture is
        // the source.
        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blur_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            // One `BlurParams`: direction, radius and the G-220 tint block.
                            min_binding_size: Some(wgpu::BufferSize::new(32).unwrap()),
                        },
                        count: None,
                    },
                ],
            });
        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[Some(&blur_bind_group_layout)],
            immediate_size: 0,
        });
        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur_pipeline"),
            layout: Some(&blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blur_fs_module,
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
                cull_mode: None,
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

        // ── Create the SDF circle pipeline (G-219) ──
        //
        // One draw covers one circle: the vertex stage transforms the six vertices of the
        // circle's bounding quad from pixel space, and the fragment stage evaluates a distance
        // field against per-circle uniforms, replacing the per-column one-pixel strips. Group 0
        // carries two dynamic-offset uniforms per draw — the fill colour and the radius/centre
        // parameters — both pointing into one shared arena, so a frame of N circles is one
        // uniform upload and two offset bindings per circle. Group 1 is the shared mvp matrix
        // layout, the same one the instanced quad pipeline uses. Culling is disabled: the quad
        // is expanded by hand and its winding is not a mirror signal.
        let circle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("circle_sdf_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(wgpu::BufferSize::new(16).unwrap()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(wgpu::BufferSize::new(16).unwrap()),
                        },
                        count: None,
                    },
                ],
            });
        let circle_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("circle_sdf_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&circle_bind_group_layout),
                    Some(&transformed_bind_group_layout),
                ],
                immediate_size: 0,
            });
        // One `vec2f` pixel-space position per vertex, six vertices per circle.
        let circle_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let circle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("circle_sdf_pipeline"),
            layout: Some(&circle_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &circle_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(circle_vertex_buffer_layout)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &circle_fs_module,
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
                cull_mode: None,
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

        // ── Create the instanced quad pipeline ──
        //
        // One instance carries the four corners and the colour, so a whole run of quads is a
        // single draw (`0..6` vertices, `0..N` instances). Culling is disabled because a
        // negative scale reverses the winding order, which is a valid mirror rather than a
        // reason to drop the quad. No depth attachment is added: this is a 2D affine composed
        // with an orthographic matrix, not a depth-tested 3D scene. The 2D rectangle path uses it
        // for every run longer than one rectangle (G-64).
        let transformed_pipeline = {
            let transformed_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("rect_transformed_pipeline_layout"),
                    bind_group_layouts: &[Some(&transformed_bind_group_layout)],
                    immediate_size: 0,
                });
            // Locations 0/1 are the four corners as two vectors; location 2 is the colour.
            let instance_buffer_layout = wgpu::VertexBufferLayout {
                array_stride: INSTANCE_BYTES as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 1,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 32,
                        shader_location: 2,
                    },
                ],
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rect_instanced_pipeline"),
                layout: Some(&transformed_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &transformed_vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(instance_buffer_layout)],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &instanced_fs_module,
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
                    cull_mode: None,
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
            })
        };

        // ── Create the depth-tested `3d` mesh pipeline ──
        //
        // One vertex buffer, one index buffer, one mvp uniform. Culling is disabled to match the
        // software rasteriser's documented policy: the depth test, not the winding order, decides
        // what is visible. Depth is `Depth32Float`, cleared to `1.0` per pass and compared with
        // `Less`, because every matrix this pipeline receives has already been remapped to the
        // `0..=1` range WebGPU expects.
        #[cfg(feature = "3d")]
        let mesh_pipeline = {
            let mesh_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("mesh_pipeline_layout"),
                    bind_group_layouts: &[Some(&transformed_bind_group_layout)],
                    immediate_size: 0,
                });
            // 32-byte stride: position `(x, y, z, 1)` then colour.
            let vertex_buffer_layout = wgpu::VertexBufferLayout {
                array_stride: 32,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 1,
                    },
                ],
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mesh_pipeline"),
                layout: Some(&mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &mesh_vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(vertex_buffer_layout)],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &mesh_fs_module,
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
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            })
        };

        let mut shader_cache = HashMap::new();
        shader_cache.insert("clear_vs".to_string(), clear_vs_module);
        shader_cache.insert("clear_fs".to_string(), clear_fs_module);
        shader_cache.insert("rect_vs".to_string(), rect_vs_module);
        shader_cache.insert("rect_fs".to_string(), rect_fs_module);
        shader_cache.insert("blur_vs".to_string(), blur_vs_module);
        shader_cache.insert("blur_fs".to_string(), blur_fs_module);
        shader_cache.insert("rect_vs_transformed".to_string(), transformed_vs_module);
        shader_cache.insert("rect_fs_instanced".to_string(), instanced_fs_module);
        shader_cache.insert("circle_vs".to_string(), circle_vs_module);
        shader_cache.insert("circle_fs".to_string(), circle_fs_module);
        #[cfg(feature = "3d")]
        shader_cache.insert("mesh3d_vs".to_string(), mesh_vs_module);
        #[cfg(feature = "3d")]
        shader_cache.insert("mesh3d_fs".to_string(), mesh_fs_module);

        Ok(Self {
            device,
            queue,
            clear_pipeline,
            rect_pipeline,
            blur_pipeline,
            circle_pipeline,
            circle_bind_group_layout,
            bind_group_layout,
            blur_bind_group_layout,
            transformed_pipeline,
            transformed_bind_group_layout,
            #[cfg(feature = "3d")]
            mesh_pipeline,
            shader_cache,
            cache: RefCell::new(RenderCache::default()),
            clear_frame: CpuFrame::new(),
            commands_frame: CpuFrame::new(),
            fallback_frame: CpuFrame::new(),
            adapter_info,
        })
    }
    /// Render one offscreen RGBA frame by clearing a texture with the given color and read pixels back.
    ///
    /// A solid clear needs no GPU work, so this fills the buffer on the CPU and returns it
    /// directly. The previous implementation routed the same fill through a texture upload and a
    /// blocking readback, which could only reproduce the bytes it was given (G-66). A caller
    /// that only uploads or reads the frame can use [`Self::render_clear_cached_rgba8`] instead
    /// of taking this owned copy.
    pub fn render_clear_rgba8(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        rasterize_draw_commands_rgba8(
            width,
            height,
            &[WgpuDrawCommand::Clear { color: rgba8_from_unit(color) }],
        )
    }

    /// Rasterises a clear frame into the renderer's reusable buffer and lends it back (G-221).
    ///
    /// The zero-copy counterpart of [`Self::render_clear_rgba8`]: a caller that uploads or
    /// inspects the frame can use the borrow instead of taking an owned copy each frame. The
    /// buffer grows only and the rasteriser clears it, so every call sees a fresh frame.
    pub fn render_clear_cached_rgba8(
        &self,
        width: u32,
        height: u32,
        color: [f64; 4],
    ) -> Result<Ref<'_, [u8]>, String> {
        let clear = [WgpuDrawCommand::Clear { color: rgba8_from_unit(color) }];
        self.clear_frame.rasterize(width, height, &clear)
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }
    /// Render filled rectangles using the GPU pipeline with vertex buffer shaders.
    ///
    /// Each rect is defined as `(x, y, width, height, [r, g, b, a])` where:
    /// - `(x, y)` is the top-left corner in pixel coordinates
    /// - `(width, height)` is the rectangle size in pixels
    /// - `color` is an RGBA float array with values in `[0.0, 1.0]`
    ///
    /// A single rectangle goes through the `FILL_RECT_VS`/`FILL_RECT_FS` pipeline, which selects
    /// its colour from a per-rect uniform. A longer run is expanded to quads and drawn in
    /// `MAX_INSTANCES_PER_DRAW`-sized instanced draws, which produces the same picture with a
    /// draw count that no longer grows with the run (G-64).
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
        // A single rectangle is already one draw, and the per-rect pipeline avoids the matrix
        // uniform upload for that common case. Two or more are packed into instance records and
        // drawn in `MAX_INSTANCES_PER_DRAW`-sized chunks, so the draw count no longer grows with
        // the run (G-64). Chunk order preserves painter order.
        if rects.len() <= 1 {
            self.encode_fill_rects_per_rect(width, height, rects)
        } else {
            self.encode_fill_rects_instanced(width, height, rects)
        }
    }

    /// The single-rectangle path: two triangles and one draw, with the colour selected by a
    /// dynamic uniform offset.
    ///
    /// A run of more than one rectangle takes the instanced path below instead, which is what
    /// keeps the draw count bounded (G-64).
    fn encode_fill_rects_per_rect(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        let w = width as f32;
        let h = height as f32;

        // Cached resources for the whole call: one colour arena, one bind group, one vertex
        // buffer and one target (REND-02).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, rects.len())?;
        cache.ensure_target(&self.device, width, height);
        let stride = cache.uniform.as_ref().expect("the arena was just ensured").stride as usize;

        // Generate transformed vertex positions in NDC and one clamped colour slot per rect,
        // directly into the reused packing buffers: the previous locals allocated the vertex
        // bytes, the colour list and the uniform staging on every call (G-216). Only the 16
        // colour bytes of each slot are written, so the alignment padding `resize` fills with
        // zero stays zero.
        let vertex_len = {
            let pack = &mut cache.pack;
            pack.vertices.clear();
            pack.uniform.resize(stride * rects.len(), 0);
            for (index, &(x, y, rw, rh, color)) in rects.iter().enumerate() {
                // Convert pixel coordinates to NDC: [-1, 1]
                // WebGPU/wgpu NDC: y = +1 maps to top of viewport, y = -1 to bottom
                // (OpenGL convention, not Vulkan).
                let x0 = (x as f32 / w) * 2.0 - 1.0;
                let y0 = 1.0 - (y as f32 / h) * 2.0;
                let x1 = ((x.saturating_add(rw as i32)) as f32 / w) * 2.0 - 1.0;
                let y1 = 1.0 - ((y.saturating_add(rh as i32)) as f32 / h) * 2.0;

                // Two triangles: bottom-left, bottom-right, top-left, then bottom-right, top-right, top-left
                for value in [x0, y0, x1, y0, x0, y1, x1, y0, x1, y1, x0, y1] {
                    pack.vertices.extend_from_slice(&value.to_le_bytes());
                }

                let slot = index * stride;
                for (channel, value) in color.iter().enumerate() {
                    pack.uniform[slot + channel * 4..slot + channel * 4 + 4]
                        .copy_from_slice(&((*value).clamp(0.0, 1.0) as f32).to_le_bytes());
                }
            }
            pack.vertices.len()
        };
        cache.ensure_vertices(&self.device, vertex_len as u64);
        let RenderCache { uniform, target, vertices, pack, .. } = &mut *cache;
        let uniform = uniform.as_ref().expect("the arena was just ensured");
        let target = target.as_ref().expect("the target was just ensured");
        let vertices = vertices.as_ref().expect("the vertices were just ensured");

        // One upload for the whole colour arena and one for every vertex, instead of one of
        // each per rectangle (REND-02).
        if !pack.uniform.is_empty() {
            self.queue.write_buffer(&uniform.buffer, 0, &pack.uniform);
        }
        if !pack.vertices.is_empty() {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.vertices);
        }

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
            rp.set_vertex_buffer(0, vertices.buffer.slice(..vertex_len as u64));

            for index in 0..rects.len() {
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

    /// The batched path: turns every rectangle into a quad and renders the whole run as an
    /// instanced draw through the quad pipeline.
    ///
    /// The identity affine composed with the orthographic pixel-to-clip matrix is exactly the
    /// matrix [`ortho_mvp_bytes`] writes, so the geometry is identical to the per-rect path; only
    /// the draw count changes (G-64).
    fn encode_fill_rects_instanced(
        &self,
        width: u32,
        height: u32,
        rects: &[(i32, i32, u32, u32, [f64; 4])],
    ) -> Result<(), String> {
        let mut quads: Vec<([crate::core::Point; 4], [f64; 4])> = Vec::with_capacity(rects.len());
        for &(x, y, rw, rh, colour) in rects {
            // Saturating addition mirrors the 2D path's treatment of an extreme width/height.
            let x1 = x.saturating_add(rw as i32);
            let y1 = y.saturating_add(rh as i32);
            quads.push((
                [
                    crate::core::Point::new(x, y),
                    crate::core::Point::new(x1, y),
                    crate::core::Point::new(x1, y1),
                    crate::core::Point::new(x, y1),
                ],
                colour,
            ));
        }
        let mvp_bytes = ortho_mvp_bytes(width, height);
        self.encode_instances(width, height, &mvp_bytes, &quads)
    }

    /// Renders a run of quads into the cached target with instanced draws.
    ///
    /// `mvp_bytes` is the column-major model-view-projection matrix every instance is multiplied
    /// by. All instance records are written once into the grow-only buffer and the run is drawn in
    /// `MAX_INSTANCES_PER_DRAW`-sized chunks. Each chunk reads its own slice of that one upload,
    /// so no chunk overwrites another and no quad allocates a buffer. Chunks are submitted in
    /// order, which is what keeps overlapping quads in painter order.
    ///
    /// The target is cleared to transparent first, so an empty run is a transparent frame rather
    /// than stale pixels from a previous call.
    fn encode_instances(
        &self,
        width: u32,
        height: u32,
        mvp_bytes: &[u8; 64],
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<(), String> {
        // Cached resources for the whole call: one grow-only instance buffer, one target and
        // one mvp uniform (REND-02). The records are packed into the reused scratch buffer so
        // the frame allocates nothing on the CPU side (G-216).
        let mut cache = self.cache.borrow_mut();
        let instance_len = {
            let pack = &mut cache.pack;
            pack_instanced_quads(quads, &mut pack.instances)?;
            pack.instances.len()
        };
        cache.ensure_target(&self.device, width, height);
        cache.ensure_vertices(&self.device, instance_len as u64);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let RenderCache { target, vertices, transformed_uniform, pack, .. } = &mut *cache;
        let target = target.as_ref().expect("the target was just ensured");
        let instances = vertices.as_ref().expect("the instance buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, mvp_bytes);
        if !pack.instances.is_empty() {
            self.queue.write_buffer(&instances.buffer, 0, &pack.instances);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_quads_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_quads_pass"),
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
            rp.set_pipeline(&self.transformed_pipeline);
            rp.set_bind_group(0, &transformed.bind_group, &[]);
            for (chunk_index, chunk) in quads.chunks(MAX_INSTANCES_PER_DRAW).enumerate() {
                let start = chunk_index * MAX_INSTANCES_PER_DRAW * INSTANCE_BYTES;
                let end = start + chunk.len() * INSTANCE_BYTES;
                rp.set_vertex_buffer(0, instances.buffer.slice(start as u64..end as u64));
                rp.draw(0..6, 0..chunk.len() as u32);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Renders a run of quads through an arbitrary affine transform (`3d` feature).
    ///
    /// The affine is applied to the local corners before the orthographic pixel-to-clip matrix,
    /// matching the software `FillQuad` path.
    #[cfg(feature = "3d")]
    fn encode_quads_instanced(
        &self,
        width: u32,
        height: u32,
        affine: &crate::render::math3d::Mat3,
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<(), String> {
        let w = width as f32;
        let h = height as f32;
        let ortho = crate::render::math3d::Mat3::scaling(2.0 / w, -2.0 / h)
            .then(&crate::render::math3d::Mat3::translation(-1.0, 1.0));
        let mvp = affine.then(&ortho);
        let mvp_bytes = mvp_column_major_bytes(&crate::render::math3d::Mat4::from_affine(mvp));
        self.encode_instances(width, height, &mvp_bytes, quads)
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
    /// fragment shader (`FILL_RECT_FS`) with a per-rect uniform colour for a single rectangle.
    /// A longer run is drawn as instanced quad batches through the matrix pipeline (G-64).
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Renders arbitrary quadrilaterals through the instanced `3d` matrix pipeline.
    ///
    /// Each quad is a `([Point; 4], [r, g, b, a])` pair. The four corners are **local logical
    /// pixels** in perimeter order; `affine` is applied to them in the vertex stage, and the
    /// renderer composes that with an orthographic pixel-to-clip matrix. The result is the same
    /// geometry the software backend draws for
    /// [`crate::render::RenderCommand::FillQuad`], which is what lets a 45-degree rotated
    /// rectangle agree across the two paths.
    ///
    /// The whole run is one upload and, below the per-draw cap, one instanced draw; a longer run
    /// streams in order from the same buffer. Culling is disabled so a mirrored (negative-scale)
    /// transform still draws. There is no depth attachment: this path is a 2D affine, and scene
    /// layers keep their existing painter's-order responsibility for control-level occlusion.
    #[cfg(feature = "3d")]
    pub fn render_fill_quads_gpu(
        &self,
        width: u32,
        height: u32,
        affine: &crate::render::math3d::Mat3,
        quads: &[([crate::core::Point; 4], [f64; 4])],
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if quads.is_empty() {
            return Ok(vec![0u8; expected_len]);
        }

        self.encode_quads_instanced(width, height, affine, quads)?;

        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().expect("the target was just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_quads_readback"),
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Renders an indexed triangle mesh through the depth-tested `3d` pipeline.
    ///
    /// `model` places the mesh, `camera` supplies the projection and `viewport` selects the part of
    /// the `width × height` target the camera looks through. The composition is
    /// `model × view-projection × viewport-matrix`, the same one
    /// [`crate::render::mesh3d::project_mesh`] uses, so the GPU and software pictures describe the
    /// same geometry.
    ///
    /// The pass clears colour to transparent and depth to `1.0`, then runs one indexed draw with a
    /// `Depth32Float` attachment compared by `Less`. No triangle is culled: the depth test decides
    /// visibility, matching the documented software policy. The target is read back and returned as
    /// tightly packed RGBA8.
    ///
    /// An empty mesh or an empty viewport returns a transparent frame rather than an error, so a
    /// caller may drop a mesh without special-casing the render call.
    #[cfg(feature = "3d")]
    pub fn render_mesh_3d_gpu(
        &self,
        width: u32,
        height: u32,
        mesh: &crate::render::mesh3d::Mesh3D,
        model: &crate::render::math3d::Mat4,
        camera: &crate::render::mesh3d::Camera,
        viewport: crate::core::Rect,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if mesh.triangle_count() == 0 || viewport.width == 0 || viewport.height == 0 {
            return Ok(vec![0u8; expected_len]);
        }

        let aspect = viewport.width as f32 / viewport.height as f32;
        let mvp = model.then(&camera.view_projection(aspect)).then(
            &crate::render::mesh3d::viewport_matrix(
                viewport,
                crate::core::Size::new(width, height),
            ),
        );
        let mvp_bytes = mvp_column_major_bytes(&mvp);
        let index_count = mesh.indices().len() as u32;
        if index_count == 0 {
            return Ok(vec![0u8; expected_len]);
        }

        let mut cache = self.cache.borrow_mut();
        // Pack straight into the reused scratch buffers so a mesh frame allocates nothing on
        // the CPU side (G-216). The strides, the shader's attribute offsets and these writers
        // must agree exactly; a mismatch is a pipeline validation error rather than a silent
        // misread.
        let (vertex_len, index_len) = {
            let pack = &mut cache.pack;
            pack.mesh_vertices.clear();
            pack.mesh_indices.clear();
            for vertex in mesh.vertices() {
                for value in [vertex.position.x, vertex.position.y, vertex.position.z, 1.0] {
                    pack.mesh_vertices.extend_from_slice(&value.to_le_bytes());
                }
                for channel in [vertex.color.r, vertex.color.g, vertex.color.b, vertex.color.a] {
                    pack.mesh_vertices.extend_from_slice(&((channel as f32) / 255.0).to_le_bytes());
                }
            }
            for index in mesh.indices() {
                pack.mesh_indices.extend_from_slice(&index.to_le_bytes());
            }
            (pack.mesh_vertices.len(), pack.mesh_indices.len())
        };
        cache.ensure_target(&self.device, width, height);
        cache.ensure_depth_target(&self.device, width, height);
        cache.ensure_vertices(&self.device, vertex_len as u64);
        cache.ensure_mesh_indices(&self.device, index_len as u64);
        cache.ensure_mesh_uniform(&self.device, &self.transformed_bind_group_layout);
        let RenderCache {
            target, depth_target, vertices, mesh_indices, mesh_uniform, pack, ..
        } = &mut *cache;
        let target = target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        let depth = depth_target.as_ref().ok_or_else(|| "depth target missing".to_string())?;
        let vertices = vertices.as_ref().ok_or_else(|| "vertex buffer missing".to_string())?;
        let indices = mesh_indices.as_ref().ok_or_else(|| "index buffer missing".to_string())?;
        let uniform = mesh_uniform.as_ref().ok_or_else(|| "mvp uniform missing".to_string())?;

        self.queue.write_buffer(&uniform.buffer, 0, &mvp_bytes);
        if !pack.mesh_vertices.is_empty() {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.mesh_vertices);
        }
        if !pack.mesh_indices.is_empty() {
            self.queue.write_buffer(&indices.buffer, 0, &pack.mesh_indices);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_mesh_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_mesh_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.mesh_pipeline);
            rp.set_bind_group(0, &uniform.bind_group, &[]);
            rp.set_vertex_buffer(0, vertices.buffer.slice(..vertex_len as u64));
            rp.set_index_buffer(
                indices.buffer.slice(..index_len as u64),
                wgpu::IndexFormat::Uint32,
            );
            rp.draw_indexed(0..index_count, 0, 0..1);
        }
        drop(cache);
        self.queue.submit(Some(encoder.finish()));
        self.read_target(width, height, expected_len, "gpu_mesh_readback")
    }

    /// Copies the cached render target into a tight RGBA8 buffer.
    ///
    /// The target must already hold the picture; this helper only arranges the copy and the blocking
    /// read. Kept separate so an entry point that renders through a new pipeline does not repeat the
    /// row-padding arithmetic.
    #[cfg(feature = "3d")]
    fn read_target(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        label: &str,
    ) -> Result<Vec<u8>, String> {
        let frame_bytes = readback_frame_bytes(width, height)?;
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_readback(&self.device, frame_bytes);
        let target = cache.target.as_ref().ok_or_else(|| "render target missing".to_string())?;
        let readback =
            cache.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Maps a readback buffer, copies its rows into a caller-provided RGBA8 image and unmaps it.
    ///
    /// The output goes through `out` rather than being allocated here (G-217): a caller that
    /// renders repeatedly can carry the vector across calls and only ever pay for growth, not
    /// a fresh allocation per frame. The entry points that must hand an owned frame back pass
    /// a fresh vector — the mapping, polling and row-copying work is identical either way.
    /// `out` is grown to `expected_len` and every row is overwritten, so no stale byte from a
    /// previous frame can leak through.
    ///
    /// The buffer belongs to [`RenderCache`], so it must be unmapped before it can be reused:
    /// leaving it mapped made the next `map_async` fail silently, which is why this is one
    /// helper rather than a copy in every entry point.
    ///
    /// The wait is bounded by [`GPU_READBACK_TIMEOUT`]; on expiry the pending mapping is
    /// cancelled with `unmap` and an error is returned, so a lost device reports a failure
    /// instead of hanging the process (G-63).
    fn read_back_into(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        expected_len: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), String> {
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
        if let Err(error) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(GPU_READBACK_TIMEOUT),
        }) {
            // Cancel the still-pending mapping so the cached buffer is usable next call.
            buffer.unmap();
            return Err(format!(
                "GPU readback did not complete within {GPU_READBACK_TIMEOUT:?}: {error}"
            ));
        }
        let map_result = match receiver.recv_timeout(GPU_READBACK_TIMEOUT) {
            Ok(result) => result,
            Err(error) => {
                buffer.unmap();
                return Err(format!(
                    "GPU map callback did not arrive within {GPU_READBACK_TIMEOUT:?}: {error}"
                ));
            }
        };
        if let Err(error) = map_result {
            buffer.unmap();
            return Err(format!("GPU buffer could not be mapped for CPU readback: {error:?}"));
        }
        let mapped = match buffer_slice.get_mapped_range() {
            Ok(mapped) => mapped,
            Err(error) => {
                buffer.unmap();
                return Err(format!("mapped GPU buffer range could not be read back: {error:?}"));
            }
        };

        // Copy row by row, stripping padding.
        out.resize(expected_len, 0);
        for row in 0..height as usize {
            let src_start = row * padded;
            let dst_start = row * unpadded;
            out[dst_start..dst_start + unpadded]
                .copy_from_slice(&mapped[src_start..src_start + unpadded]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(())
    }

    /// Reads the cached readback buffer into a freshly owned RGBA8 frame.
    ///
    /// Every public entry point returns the frame to its caller, so the output vector cannot
    /// be carried across calls; this is the thin adapter from [`Self::read_back_into`] to
    /// that contract (G-217).
    fn read_back_owned(
        &self,
        buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        expected_len: usize,
    ) -> Result<Vec<u8>, String> {
        let mut frame = Vec::new();
        self.read_back_into(buffer, width, height, expected_len, &mut frame)?;
        Ok(frame)
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

    /// Render filled circles using the SDF circle pipeline (G-219).
    ///
    /// Each circle is **one draw**: a bounding quad is transformed by the shared pixel-to-clip
    /// matrix and [`super::shaders::CIRCLE_FRAG`] shades it with a distance-field test, so the
    /// silhouette is evaluated per fragment instead of being approximated by per-column
    /// one-pixel strips. Where the strips needed `2r + 1` rectangles per circle, this path
    /// needs one 48-byte quad, two uniform slots and one draw call, and the SDF edge is
    /// anti-aliased rather than stair-stepped.
    ///
    /// Behaviour notes: a zero-radius circle draws nothing, matching the software rasteriser's
    /// skip, and a circle that covers no pixel is skipped exactly like the software path. The
    /// anti-aliased edge means boundary pixels take partial alpha where the strip approximation
    /// painted hard steps — that difference is the point of the item.
    #[allow(clippy::type_complexity)]
    pub fn render_fill_circle_gpu(
        &self,
        width: u32,
        height: u32,
        circles: &[((i32, i32, u32), [f64; 4])], // ((cx, cy, radius), color)
    ) -> Result<Vec<u8>, String> {
        // Validate dimensions before any allocation (N-S-66).
        let expected_len = rgba_buffer_len(width, height)?;
        // A zero-radius circle draws nothing on the software path either, so a frame of only
        // zero-radius circles is a transparent frame with no GPU work, exactly as before.
        if circles.iter().all(|((_, _, radius), _)| *radius == 0) {
            return Ok(vec![0u8; expected_len]);
        }

        // Cached resources for the whole call: one two-slot-per-circle uniform arena, one
        // vertex buffer, one mvp uniform and one target (REND-02, G-216).
        let mut cache = self.cache.borrow_mut();
        cache.ensure_circle_uniforms(
            &self.device,
            &self.circle_bind_group_layout,
            circles.len(),
        )?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let stride = RenderCache::uniform_stride(&self.device) as usize;

        // Pack every drawable circle into the reused scratch buffers: one vertex upload and
        // one uniform upload for the whole frame (G-216). `packed` stays dense: a circle
        // skipped as fully off-screen occupies no vertex bytes and no uniform slots, so the
        // draw loop's indices line up with the packed data exactly.
        let (vertex_len, uniform_len, packed) = {
            let pack = &mut cache.pack;
            pack.circle_vertices.clear();
            pack.circle_uniforms.resize(stride * 2 * circles.len(), 0);
            let mut packed = 0usize;
            for &((cx, cy, radius), colour) in circles {
                if radius == 0 {
                    continue;
                }
                let Some(quad) = circle_quad((cx, cy), radius, width, height) else {
                    continue;
                };
                pack_circle(
                    &mut pack.circle_vertices,
                    &mut pack.circle_uniforms,
                    stride,
                    packed,
                    (cx, cy),
                    radius,
                    &colour,
                    quad,
                );
                packed += 1;
            }
            (pack.circle_vertices.len(), pack.circle_uniforms.len(), packed)
        };
        cache.ensure_vertices(&self.device, vertex_len as u64);
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let RenderCache {
            target,
            vertices,
            transformed_uniform,
            circle_uniforms,
            readback,
            pack,
            ..
        } = &mut *cache;
        let target = target.as_ref().expect("the target was just ensured");
        let vertices = vertices.as_ref().expect("the vertex buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");
        let circle_arena = circle_uniforms.as_ref().expect("the circle arena was just ensured");
        let readback = readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, &ortho_mvp_bytes(width, height));
        if vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.circle_vertices);
        }
        if uniform_len > 0 {
            self.queue.write_buffer(&circle_arena.buffer, 0, &pack.circle_uniforms);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_circle_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_circle_pass"),
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
            rp.set_pipeline(&self.circle_pipeline);
            // The mvp matrix is group 1 for every draw; group 0's dynamic offsets select each
            // circle's colour and parameter slots. One draw per circle (G-219).
            rp.set_bind_group(1, &transformed.bind_group, &[]);
            for index in 0..packed {
                let quad_start = (index * CIRCLE_QUAD_BYTES) as u64;
                let quad_end = quad_start + CIRCLE_QUAD_BYTES as u64;
                let colour_offset = (index * 2 * stride) as u32;
                let params_offset = ((index * 2 + 1) * stride) as u32;
                rp.set_bind_group(0, &circle_arena.bind_group, &[colour_offset, params_offset]);
                rp.set_vertex_buffer(0, vertices.buffer.slice(quad_start..quad_end));
                rp.draw(0..6, 0..1);
            }
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Render command list with deterministic ordering and clipping.
    ///
    /// # GPU dispatch (G-218)
    ///
    /// A stream the existing pipelines can cover — whole-frame clears, unclipped filled
    /// rectangles and unclipped filled circles, with blending left at `Normal` — is drawn
    /// entirely on the GPU in one render pass: clears repaint through the full-screen clear
    /// pipeline, rectangle runs go through the instanced quad pipeline, and each circle is
    /// one SDF circle draw. Painter order is replayed exactly, so mid-stream clears and
    /// overlaps behave as on the CPU. The circles' SDF edge is anti-aliased, which is the
    /// mandated difference from the CPU rasteriser's hard-edged disc.
    ///
    /// Every other stream falls back to [`rasterize_draw_commands_rgba8`], **returned
    /// directly**: the old upload-and-readback round trip reproduced those exact bytes
    /// (`upload_round_trips_the_given_pixels` pins the identity), so skipping it also skips
    /// the texture upload, the copy and the blocking readback for frames that never needed
    /// the GPU (G-217). A caller that only uploads or reads the frame can use
    /// [`Self::render_draw_commands_cached_rgba8`] to reuse the renderer's buffer instead.
    pub fn render_draw_commands_rgba8(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if commands_gpu_dispatchable(commands) {
            return self.encode_draw_commands_gpu(width, height, expected_len, commands);
        }
        rasterize_draw_commands_rgba8(width, height, commands)
    }

    /// Rasterises `commands` into the renderer's reusable CPU frame and lends it back (G-221).
    ///
    /// The zero-copy counterpart of [`Self::render_draw_commands_rgba8`] for a caller that
    /// uploads or reads the frame without needing ownership. It always uses the CPU rasteriser,
    /// including for streams [`Self::render_draw_commands_rgba8`] would dispatch to the GPU, so
    /// the frame is the software picture (hard-edged circles) rather than the GPU one. The
    /// buffer grows only, and the rasteriser clears it, so every call sees a fresh frame.
    pub fn render_draw_commands_cached_rgba8(
        &self,
        width: u32,
        height: u32,
        commands: &[WgpuDrawCommand],
    ) -> Result<Ref<'_, [u8]>, String> {
        self.commands_frame.rasterize(width, height, commands)
    }

    /// Draws a dispatchable command stream entirely on the GPU (G-218).
    ///
    /// One render pass covers the frame. The pass loads transparent — the software
    /// rasteriser's zeroed start — and every [`WgpuDrawCommand::Clear`] repaints the whole
    /// target through the clear pipeline, so position in the list decides the result exactly
    /// as the command documentation promises. Geometry is packed once into the reused scratch
    /// buffers (one instance upload, one circle-quad upload, one uniform upload) and the draw
    /// order is recorded in `DrawStep`s, which the pass replays in list order.
    fn encode_draw_commands_gpu(
        &self,
        width: u32,
        height: u32,
        expected_len: usize,
        commands: &[WgpuDrawCommand],
    ) -> Result<Vec<u8>, String> {
        let framebuffer = PixelRect { x: 0, y: 0, width, height };
        let clear_count = commands
            .iter()
            .filter(|command| matches!(command, WgpuDrawCommand::Clear { .. }))
            .count();
        let circle_count = commands
            .iter()
            .filter(|command| {
                matches!(command, WgpuDrawCommand::FillCircle { radius, clip: None, .. } if *radius > 0)
            })
            .count();

        // Cached resources for the whole call (REND-02). The clear pipeline's colour slots
        // and the circles' two-slot records share the dynamic-offset stride.
        let mut cache = self.cache.borrow_mut();
        cache.ensure_uniform(&self.device, &self.bind_group_layout, clear_count)?;
        cache.ensure_circle_uniforms(&self.device, &self.circle_bind_group_layout, circle_count)?;
        cache.ensure_target(&self.device, width, height);
        cache.ensure_transformed_uniform(&self.device, &self.transformed_bind_group_layout);
        let stride = RenderCache::uniform_stride(&self.device) as usize;

        // Pack the frame: instance records for rectangles, bounding quads and two uniform
        // slots per circle, one colour slot per clear, and the draw order (G-216).
        let (instance_len, circle_vertex_len, uniform_len, circle_uniform_len, plan_len) = {
            let pack = &mut cache.pack;
            pack.instances.clear();
            pack.circle_vertices.clear();
            pack.uniform.resize(stride * clear_count, 0);
            pack.circle_uniforms.resize(stride * 2 * circle_count, 0);
            pack.plan.clear();
            let mut clear_slot = 0usize;
            let mut circle_index = 0usize;
            let mut run_start = 0usize;
            for command in commands {
                match command {
                    WgpuDrawCommand::Clear { color } => {
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        let slot = clear_slot;
                        clear_slot += 1;
                        let base = slot * stride;
                        for (channel, value) in
                            [color.r, color.g, color.b, color.a].iter().enumerate()
                        {
                            pack.uniform[base + channel * 4..base + channel * 4 + 4]
                                .copy_from_slice(&((*value as f32 / 255.0).to_le_bytes()));
                        }
                        pack.plan.push(DrawStep::Clear(slot));
                    }
                    WgpuDrawCommand::FillRect { rect, color, .. } => {
                        // The CPU rasteriser clips every fill to the framebuffer first; the
                        // same clip here keeps the instance coordinates in range.
                        let Some(draw_rect) = rect.intersect(framebuffer) else {
                            continue;
                        };
                        if draw_rect.width == 0 || draw_rect.height == 0 {
                            continue;
                        }
                        let (x, y) = (draw_rect.x, draw_rect.y);
                        let (x1, y1) = (draw_rect.right(), draw_rect.bottom());
                        let mut record = [0u8; INSTANCE_BYTES];
                        for (corner, (px, py)) in
                            [(x, y), (x1, y), (x1, y1), (x, y1)].iter().enumerate()
                        {
                            record[corner * 8..corner * 8 + 4]
                                .copy_from_slice(&(*px as f32).to_le_bytes());
                            record[corner * 8 + 4..corner * 8 + 8]
                                .copy_from_slice(&(*py as f32).to_le_bytes());
                        }
                        for (channel, value) in
                            [color.r, color.g, color.b, color.a].iter().enumerate()
                        {
                            record[32 + channel * 4..32 + channel * 4 + 4]
                                .copy_from_slice(&((*value as f64 / 255.0) as f32).to_le_bytes());
                        }
                        pack.instances.extend_from_slice(&record);
                    }
                    WgpuDrawCommand::FillCircle { center, radius, color, .. } => {
                        if *radius == 0 {
                            continue;
                        }
                        let Some(quad) = circle_quad(*center, *radius, width, height) else {
                            continue;
                        };
                        if run_start < pack.instances.len() / INSTANCE_BYTES {
                            pack.plan.push(DrawStep::Rects(
                                run_start,
                                pack.instances.len() / INSTANCE_BYTES,
                            ));
                            run_start = pack.instances.len() / INSTANCE_BYTES;
                        }
                        pack_circle(
                            &mut pack.circle_vertices,
                            &mut pack.circle_uniforms,
                            stride,
                            circle_index,
                            *center,
                            *radius,
                            &[
                                color.r as f64 / 255.0,
                                color.g as f64 / 255.0,
                                color.b as f64 / 255.0,
                                color.a as f64 / 255.0,
                            ],
                            quad,
                        );
                        pack.plan.push(DrawStep::Circle(circle_index));
                        circle_index += 1;
                    }
                    // Normal blending is the pipelines' `REPLACE` write; anything else was
                    // filtered out before this pass was chosen.
                    WgpuDrawCommand::SetBlendMode { mode: 0 } => {}
                    // `commands_gpu_dispatchable` filters the stream, so no other command can
                    // reach this match; if one ever does, the CPU rasteriser still produces
                    // the correct frame, in the fallback frame that `cache`'s borrow cannot
                    // reach (G-221).
                    _ => {
                        return Ok(self
                            .fallback_frame
                            .rasterize(width, height, commands)?
                            .to_vec());
                    }
                }
            }
            if run_start < pack.instances.len() / INSTANCE_BYTES {
                pack.plan.push(DrawStep::Rects(run_start, pack.instances.len() / INSTANCE_BYTES));
            }
            (
                pack.instances.len(),
                pack.circle_vertices.len(),
                pack.uniform.len(),
                pack.circle_uniforms.len(),
                pack.plan.len(),
            )
        };

        // One grow-only vertex buffer serves both the instance records and the circle quads:
        // instances at `[0, instance_len)`, circle quads appended after.
        let circle_vertex_offset = instance_len as u64;
        cache.ensure_vertices(&self.device, (instance_len + circle_vertex_len) as u64);
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let RenderCache {
            uniform,
            target,
            vertices,
            transformed_uniform,
            circle_uniforms,
            readback,
            pack,
            ..
        } = &mut *cache;
        let uniform = uniform.as_ref().expect("the colour arena was just ensured");
        let target = target.as_ref().expect("the target was just ensured");
        let vertices = vertices.as_ref().expect("the vertex buffer was just ensured");
        let transformed = transformed_uniform.as_ref().expect("the mvp was just ensured");
        let circle_arena = circle_uniforms.as_ref().expect("the circle arena was just ensured");
        let readback = readback.as_ref().expect("the readback was just ensured");

        self.queue.write_buffer(&transformed.buffer, 0, &ortho_mvp_bytes(width, height));
        if uniform_len > 0 {
            self.queue.write_buffer(&uniform.buffer, 0, &pack.uniform);
        }
        if circle_uniform_len > 0 {
            self.queue.write_buffer(&circle_arena.buffer, 0, &pack.circle_uniforms);
        }
        if instance_len > 0 {
            self.queue.write_buffer(&vertices.buffer, 0, &pack.instances);
        }
        if circle_vertex_len > 0 {
            self.queue.write_buffer(&vertices.buffer, circle_vertex_offset, &pack.circle_vertices);
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_draw_commands_encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_draw_commands_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // The software rasteriser starts from zeroed pixels; mid-stream
                        // clears repaint through the clear pipeline instead (G-218).
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for step in &pack.plan[..plan_len] {
                match step {
                    DrawStep::Clear(slot) => {
                        rp.set_pipeline(&self.clear_pipeline);
                        rp.set_bind_group(0, &uniform.bind_group, &[(*slot * stride) as u32]);
                        rp.draw(0..3, 0..1);
                    }
                    DrawStep::Rects(start, end) => {
                        rp.set_pipeline(&self.transformed_pipeline);
                        rp.set_bind_group(0, &transformed.bind_group, &[]);
                        let mut cursor = *start;
                        while cursor < *end {
                            let chunk_end = (cursor + MAX_INSTANCES_PER_DRAW).min(*end);
                            let byte_start = (cursor * INSTANCE_BYTES) as u64;
                            let byte_end = (chunk_end * INSTANCE_BYTES) as u64;
                            rp.set_vertex_buffer(0, vertices.buffer.slice(byte_start..byte_end));
                            rp.draw(0..6, 0..(chunk_end - cursor) as u32);
                            cursor = chunk_end;
                        }
                    }
                    DrawStep::Circle(index) => {
                        rp.set_pipeline(&self.circle_pipeline);
                        let colour_offset = (*index * 2 * stride) as u32;
                        let params_offset = ((*index * 2 + 1) * stride) as u32;
                        rp.set_bind_group(
                            0,
                            &circle_arena.bind_group,
                            &[colour_offset, params_offset],
                        );
                        rp.set_bind_group(1, &transformed.bind_group, &[]);
                        let quad_start = circle_vertex_offset + (*index * CIRCLE_QUAD_BYTES) as u64;
                        let quad_end = quad_start + CIRCLE_QUAD_BYTES as u64;
                        rp.set_vertex_buffer(0, vertices.buffer.slice(quad_start..quad_end));
                        rp.draw(0..6, 0..1);
                    }
                }
            }
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }
    /// Applies a separable Gaussian blur to an RGBA8 image on the GPU.
    ///
    /// # Why two passes
    ///
    /// A Gaussian kernel is separable: blurring rows and then columns costs `O(radius)` per pixel
    /// instead of `O(radius²)`. Each pass is one full-screen draw through [`super::shaders::BLUR_FS`],
    /// ping-ponging between two cached textures. The input is uploaded once, the result read back
    /// once, and every intermediate stage stays on the GPU.
    ///
    /// `radius` is the Gaussian's 2-sigma point, matching [`crate::render::RenderCommand::Blur`], and is clamped
    /// to the same 100-pixel ceiling the software path uses. A zero radius is an identity and
    /// returns the input bytes unchanged (no GPU round trip).
    ///
    /// This is the hardware path the backdrop-blur layer uses. It is a 2D effect, so it is compiled
    /// in every build rather than behind the `3d` feature.
    pub fn render_blur_rgba8(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
        radius: u32,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if rgba8.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                rgba8.len(),
            ));
        }
        let radius = radius.min(100);
        if radius == 0 {
            return Ok(rgba8.to_vec());
        }
        self.encode_blur(width, height, rgba8, radius, None)?;
        let readback = self.cache.borrow();
        let readback =
            readback.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        self.read_back_owned(&readback.buffer, width, height, expected_len)
    }

    /// Encodes the two-pass separable blur — and, when `tint` is present, the tint composite
    /// folded into the vertical pass — into the cached blur targets, queues the copy of the
    /// result into the readback buffer and submits (G-220).
    ///
    /// Split from [`Self::render_blur_rgba8`] so the backdrop entry point can ride the same
    /// two passes with the tint on board: the composite happens in the fragment shader
    /// ([`super::shaders::BLUR_FS`]) instead of a read-back-to-CPU shading step, so the frame
    /// never leaves the GPU between the convolution and the tint.
    ///
    /// The tint carries the exact semantics the former CPU tint step implemented: a non-finite
    /// weight is ignored (rather than clamped, since `f32::clamp` propagates `NaN`), the weight
    /// clamps to `0.0..=1.0`, and a non-positive resulting alpha leaves the frame untouched.
    fn encode_blur(
        &self,
        width: u32,
        height: u32,
        rgba8: &[u8],
        radius: u32,
        tint: Option<(Rgba8, f32)>,
    ) -> Result<(), String> {
        let padded_bytes_per_row = padded_row_bytes(width)?;
        let mut cache = self.cache.borrow_mut();
        cache.ensure_blur_targets(&self.device, &self.blur_bind_group_layout, width, height);
        cache.ensure_readback(&self.device, readback_frame_bytes(width, height)?);
        let targets = cache.blur.as_ref().expect("the blur targets were just ensured");
        let readback = cache.readback.as_ref().expect("the readback was just ensured");

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.source,
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
        let tint_state = tint.and_then(|(tint, weight)| {
            if !weight.is_finite() {
                return None;
            }
            let weight = weight.clamp(0.0, 1.0);
            let alpha = (tint.a as f32 / 255.0) * weight;
            (alpha > 0.0).then_some((
                [tint.r as f32 / 255.0, tint.g as f32 / 255.0, tint.b as f32 / 255.0],
                alpha,
            ))
        });
        let write_params = |buffer: &wgpu::Buffer, direction: [f32; 2], apply_tint: bool| {
            // `BlurParams` is 32 bytes: `direction: vec2f`, `radius: f32`, `apply_tint: f32`,
            // then (16-byte aligned) `tint_color: vec3f` and `tint_alpha: f32`. Only the
            // vertical pass composites the tint.
            let mut bytes = [0u8; 32];
            bytes[0..4].copy_from_slice(&direction[0].to_ne_bytes());
            bytes[4..8].copy_from_slice(&direction[1].to_ne_bytes());
            bytes[8..12].copy_from_slice(&(radius as f32).to_ne_bytes());
            if let Some((colour, alpha)) = tint_state.filter(|_| apply_tint) {
                bytes[12..16].copy_from_slice(&1.0f32.to_ne_bytes());
                for (channel, value) in colour.iter().enumerate() {
                    bytes[16 + channel * 4..16 + channel * 4 + 4]
                        .copy_from_slice(&value.to_ne_bytes());
                }
                bytes[28..32].copy_from_slice(&alpha.to_ne_bytes());
            }
            self.queue.write_buffer(buffer, 0, &bytes);
        };
        write_params(&targets.horizontal_params, [1.0, 0.0], false);
        write_params(&targets.vertical_params, [0.0, 1.0], true);

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("wgpu_blur_encoder"),
        });
        let mut blur_pass = |view: &wgpu::TextureView, group: &wgpu::BindGroup, label: &str| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        };
        blur_pass(&targets.scratch_view, &targets.horizontal, "wgpu_blur_horizontal");
        blur_pass(&targets.source_view, &targets.vertical, "wgpu_blur_vertical");

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &targets.source,
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
        Ok(())
    }

    /// Applies one backdrop-blur pass to a frame that has already been composed.
    ///
    /// # Where the passes run
    ///
    /// The convolution and the tint both run on the GPU: the frame is uploaded straight into
    /// the blur source texture, the two [`super::shaders::BLUR_FS`] passes ping-pong between
    /// the cached targets, and the tint — when present — is composited in the second pass's
    /// fragment shader, so the frame never comes back to the CPU between the blur and the
    /// tint (G-220). The tint semantics are the former CPU composite's: the weight scales the
    /// tint's alpha, a non-finite weight is ignored, and a non-positive effective alpha leaves
    /// the frame untouched.
    ///
    /// `radius` is the Gaussian's 2-sigma point, matching [`crate::render::RenderCommand::Blur`].
    ///
    /// Only the final result is read back. An earlier form uploaded and read the backdrop first
    /// and then uploaded it again for the blur, paying two round trips for one result (G-65).
    /// A zero radius keeps the tint: the shader's `radius <= 0` branch is a passthrough of the
    /// input plus the tint composite, matching the software layer composition where the tint
    /// sits on the unblurred backdrop.
    pub fn render_backdrop_blur_rgba8(
        &self,
        width: u32,
        height: u32,
        backdrop: &[u8],
        radius: u32,
        tint: Option<(Rgba8, f32)>,
    ) -> Result<Vec<u8>, String> {
        let expected_len = rgba_buffer_len(width, height)?;
        if backdrop.len() != expected_len {
            return Err(format!(
                "rgba8 input is {} bytes but a {width}x{height} image needs {expected_len} bytes",
                backdrop.len(),
            ));
        }
        self.encode_blur(width, height, backdrop, radius.min(100), tint)?;
        let readback = self.cache.borrow();
        let readback =
            readback.readback.as_ref().ok_or_else(|| "readback buffer missing".to_string())?;
        self.read_back_owned(&readback.buffer, width, height, expected_len)
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
        self.read_back_owned(&readback.buffer, width, height, expected_len)
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

    fn memory_usage(&self) -> Option<u64> {
        // `wgpu` exposes no portable bytes-in-use query: `AdapterInfo` carries a device name and
        // limits, not an allocation total. Reporting `None` is the honest answer (G-232).
        None
    }

    fn vendor_info(&self) -> Option<&str> {
        // The real adapter name, captured at construction. An empty name means the backend had
        // nothing to report, which is also `None` rather than an invented string (G-232).
        (!self.adapter_info.name.is_empty()).then_some(self.adapter_info.name.as_str())
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

    /// G-213: the enum's pipeline table and the shader cache the renderer registers must agree.
    ///
    /// Every member that names a pipeline key must have that key in the cache, and the cache's
    /// key set is asserted exactly, so adding a shader without wiring it into a pipeline — or
    /// deleting one — fails here instead of drifting silently.
    #[test]
    fn shader_module_registry_matches_pipelines() {
        let Some(renderer) = renderer() else { return };

        let mut expected = vec![
            "clear_vs",
            "clear_fs",
            "rect_vs",
            "rect_fs",
            "blur_vs",
            "blur_fs",
            "rect_vs_transformed",
            "rect_fs_instanced",
            "circle_vs",
            "circle_fs",
        ];
        #[cfg(feature = "3d")]
        expected.extend(["mesh3d_vs", "mesh3d_fs"]);
        let mut registered: Vec<&str> = renderer.shader_cache.keys().map(String::as_str).collect();
        expected.sort_unstable();
        registered.sort_unstable();
        assert_eq!(
            registered, expected,
            "the registered shader cache drifted from the pipeline table; wire or remove the \
             entry and update the table in this test"
        );

        for module in renderer.shader_modules() {
            if let Some(key) = module.pipeline_cache_key() {
                assert!(
                    renderer.get_shader_module(key).is_some(),
                    "{module:?} advertises pipeline key {key:?}, but no such module is \
                     registered"
                );
            }
        }
    }

    /// Growth is geometric and block-rounded, so a frame that needs a few more bytes reuses the
    /// existing buffer instead of recreating it at the exact size (G-09).
    #[test]
    fn buffer_growth_leaves_headroom_and_respects_the_limit() {
        assert_eq!(grown_buffer_capacity(0, 1, u64::MAX), 65_536);
        let first = grown_buffer_capacity(0, 100 * 1024, u64::MAX);
        assert!(first >= 100 * 1024);
        assert_eq!(first % 65_536, 0, "capacities are whole 64 KiB blocks");

        // A small increase must not force a new allocation.
        let grown = grown_buffer_capacity(first, first + 16, u64::MAX);
        assert!(grown > first, "growth is geometric, not exact-size");
        assert_eq!(grown, first + first / 2, "the next step is 1.5x before rounding");

        // The device limit caps the rounded capacity; the caller rejects a request that cannot
        // fit at all before calling this.
        assert_eq!(grown_buffer_capacity(0, 3, 4), 4);
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

    /// A larger run must still place every rectangle's colour on its own pixels.
    ///
    /// Three rectangles now take the instanced batched path (G-64), so this is the guard that the
    /// batching change did not collapse or reorder their colours.
    #[test]
    fn a_larger_run_renders_every_colour() {
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

    /// The reusable CPU frame must match the owned raster byte for byte, keep its allocation
    /// across calls, and be uploadable without an owned copy (G-221).
    #[test]
    fn reusable_cpu_frame_matches_the_owned_raster_and_is_reused() {
        let Some(renderer) = renderer() else { return };
        let commands = vec![
            WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 2, y: 2, width: 6, height: 6 },
                color: opaque(255, 0, 0),
                clip: None,
            },
        ];
        let owned = rasterize_draw_commands_rgba8(16, 16, &commands).expect("owned raster");
        let first =
            renderer.render_draw_commands_cached_rgba8(16, 16, &commands).expect("cached raster");
        assert_eq!(&*first, &owned[..], "the reusable frame must be byte-identical");
        // The upload runs while the frame is still borrowed: the two caches are independent.
        let uploaded =
            renderer.upload_rgba8_and_readback(16, 16, &first).expect("upload the borrowed frame");
        assert_eq!(uploaded, owned, "upload must see the reusable frame's bytes");
        let address = first.as_ptr();
        drop(first);
        let second =
            renderer.render_draw_commands_cached_rgba8(16, 16, &commands).expect("cached again");
        assert_eq!(second.as_ptr(), address, "the frame allocation must be reused");
        drop(second);
        // A smaller frame must not inherit the previous frame's bytes.
        let small = vec![WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 1, height: 1 },
            color: opaque(0, 255, 0),
            clip: None,
        }];
        let smaller =
            renderer.render_draw_commands_cached_rgba8(8, 8, &small).expect("smaller frame");
        assert_eq!(pixel(&smaller, 8, 7, 7), [0, 0, 0, 0], "the reused prefix must be cleared");
        assert_eq!(pixel(&smaller, 8, 0, 0), [0, 255, 0, 255]);
    }

    /// A clear through the reusable path must match the owned clear exactly.
    #[test]
    fn reusable_clear_frame_matches_the_owned_clear() {
        let Some(renderer) = renderer() else { return };
        let owned = renderer.render_clear_rgba8(4, 3, [0.25, 0.5, 0.75, 1.0]).expect("owned clear");
        let cached =
            renderer.render_clear_cached_rgba8(4, 3, [0.25, 0.5, 0.75, 1.0]).expect("cached clear");
        assert_eq!(&*cached, &owned[..]);
    }

    #[cfg(feature = "3d")]
    #[test]
    fn fill_quads_empty_is_transparent() {
        let Some(renderer) = renderer() else { return };
        let pixels = renderer
            .render_fill_quads_gpu(8, 8, &crate::render::math3d::Mat3::IDENTITY, &[])
            .expect("an empty quad batch must produce a frame");
        assert_eq!(pixels.len(), 8 * 8 * 4);
        assert!(pixels.iter().all(|&byte| byte == 0));
    }

    /// A rectangle rotated by 45 degrees must land on the same pixels through the matrix
    /// pipeline as it does through the software polygon fill.
    ///
    /// The comparison deliberately samples the interior and the hidden corners rather than the
    /// edge pixels: both rasterisers are hard-edged but use different fill rules, so exact
    /// agreement on a sloped boundary is not something either path promises.
    #[cfg(feature = "3d")]
    #[test]
    fn a_45_degree_rotated_rect_agrees_with_the_software_path() {
        use crate::core::{Color, Point};
        use crate::render::math3d::Mat3;
        use crate::render::SoftwareSurface;

        let Some(renderer) = renderer() else { return };
        let affine = Mat3::translation(-16.0, -16.0)
            .then(&Mat3::rotation(core::f32::consts::FRAC_PI_4))
            .then(&Mat3::translation(16.0, 16.0));
        let local = [Point::new(8, 8), Point::new(24, 8), Point::new(24, 24), Point::new(8, 24)];

        let gpu = renderer
            .render_fill_quads_gpu(32, 32, &affine, &[(local, [1.0, 0.0, 0.0, 1.0])])
            .expect("the matrix pipeline must render");

        let mapped = local.map(|point| affine.map_point(point));
        let mut surface = SoftwareSurface::new(crate::core::Size::new(32, 32), 1.0);
        surface.begin_frame(Color::TRANSPARENT);
        surface.draw_path(&mapped[..], true, Color::RED, true, 0);
        surface.end_frame();
        let software = surface.frame_rgba();

        for (x, y, what) in [
            (16, 16, "centre"),
            (10, 16, "left interior"),
            (22, 16, "right interior"),
            (16, 10, "top interior"),
            (16, 22, "bottom interior"),
        ] {
            assert_eq!(
                pixel(software, 32, x, y),
                pixel(&gpu, 32, x, y),
                "{what} must agree between the software and matrix paths"
            );
        }
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(
                pixel(software, 32, x, y),
                pixel(&gpu, 32, x, y),
                "a corner outside the rotated square must remain transparent in both paths"
            );
        }
    }

    /// Builds an axis-aligned quad in perimeter order, matching the corner order the instanced
    /// shader expands into two triangles.
    #[cfg(feature = "3d")]
    fn quad_rect(x: i32, y: i32, x1: i32, y1: i32) -> [crate::core::Point; 4] {
        [
            crate::core::Point::new(x, y),
            crate::core::Point::new(x1, y),
            crate::core::Point::new(x1, y1),
            crate::core::Point::new(x, y1),
        ]
    }

    /// Converts a `Color` to the `0.0..=1.0` tuple the GPU entry points take.
    #[cfg(feature = "3d")]
    fn gpu_colour(color: crate::core::Color) -> [f64; 4] {
        [
            color.r as f64 / 255.0,
            color.g as f64 / 255.0,
            color.b as f64 / 255.0,
            color.a as f64 / 255.0,
        ]
    }

    /// An instanced run of axis-aligned quads must produce exactly the software picture at the
    /// sampled pixels, including overlapping quads, whose later instances paint over earlier
    /// ones.
    ///
    /// The samples are interiors rather than edges: the software scanline fill and the GPU both
    /// cover a shape's interior identically, but their boundary conventions differ, so an edge
    /// pixel is not a fair pixel-for-pixel comparison for either path.
    #[cfg(feature = "3d")]
    #[test]
    fn instanced_run_matches_the_software_reference() {
        use crate::core::{Color, Point};
        use crate::render::math3d::Mat3;
        use crate::render::SoftwareSurface;

        let Some(renderer) = renderer() else { return };
        let run = [
            (quad_rect(2, 2, 10, 10), Color::RED),
            (quad_rect(8, 2, 16, 10), Color::GREEN),
            (quad_rect(2, 8, 10, 16), Color::BLUE),
            (quad_rect(8, 8, 16, 16), Color::YELLOW),
        ];
        let quads: Vec<([Point; 4], [f64; 4])> =
            run.iter().map(|(corners, colour)| (*corners, gpu_colour(*colour))).collect();

        let gpu = renderer
            .render_fill_quads_gpu(18, 18, &Mat3::IDENTITY, &quads)
            .expect("the instanced run must render");

        let mut surface = SoftwareSurface::new(crate::core::Size::new(18, 18), 1.0);
        surface.begin_frame(Color::TRANSPARENT);
        for (corners, colour) in run {
            surface.draw_path(&corners, true, colour, true, 0);
        }
        surface.end_frame();
        let software = surface.frame_rgba();

        // One sample per quad, one per pair overlap, and one where all four overlap. The
        // expected colour is the last quad in `run` that covers the point, so this also proves
        // the instanced draw preserved painter order.
        for (x, y, expected) in [
            (4, 4, Color::RED),
            (12, 4, Color::GREEN),
            (4, 12, Color::BLUE),
            (12, 12, Color::YELLOW),
            (9, 4, Color::GREEN),
            (4, 9, Color::BLUE),
            (12, 9, Color::YELLOW),
            (9, 12, Color::YELLOW),
            (9, 9, Color::YELLOW),
        ] {
            assert_eq!(
                pixel(&gpu, 18, x, y),
                pixel(&software, 18, x, y),
                "pixel ({x}, {y}) must match the software reference"
            );
            assert_eq!(
                pixel(&gpu, 18, x, y),
                [expected.r, expected.g, expected.b, expected.a],
                "pixel ({x}, {y}) must show the last quad in the run that covers it"
            );
        }
    }

    /// An empty instanced run must produce a transparent frame, not stale pixels.
    #[cfg(feature = "3d")]
    #[test]
    fn instanced_empty_run_is_transparent() {
        let Some(renderer) = renderer() else { return };
        let pixels = renderer
            .render_fill_quads_gpu(8, 8, &crate::render::math3d::Mat3::IDENTITY, &[])
            .expect("an empty instanced run must produce a frame");
        assert_eq!(pixels.len(), 8 * 8 * 4);
        assert!(pixels.iter().all(|&byte| byte == 0));
    }

    /// A run longer than the per-draw cap must stream across chunks without reordering: the
    /// lone instance in the second chunk must read its own record, not the first chunk's.
    #[cfg(feature = "3d")]
    #[test]
    fn an_over_cap_instanced_run_streams_every_chunk_in_order() {
        let Some(renderer) = renderer() else { return };
        // Quads are 2x1 so no pixel centre lies on the shared triangle edge; 1024 of them fit
        // in one row, and the extra instance lands on the next row.
        let per_row = 1024u32;
        let total = MAX_INSTANCES_PER_DRAW + 1;
        let width = per_row * 2;
        let height = (total as u32).div_ceil(per_row);
        let mut quads = Vec::with_capacity(total);
        for index in 0..total {
            let x = ((index as u32 % per_row) * 2) as i32;
            let y = (index as u32 / per_row) as i32;
            let colour =
                if index + 1 == total { [0.0, 0.0, 1.0, 1.0] } else { [1.0, 0.0, 0.0, 1.0] };
            quads.push((quad_rect(x, y, x + 2, y + 1), colour));
        }
        let pixels = renderer
            .render_fill_quads_gpu(width, height, &crate::render::math3d::Mat3::IDENTITY, &quads)
            .expect("an over-cap run must render");

        let sample = |index: usize| -> [u8; 4] {
            let x = (index as u32 % per_row) * 2;
            let y = index as u32 / per_row;
            pixel(&pixels, width, x, y)
        };
        let last = total - 1;
        assert_eq!(sample(last), [0, 0, 255, 255], "the second chunk must draw its own quad");
        assert_eq!(sample(last - 1), [255, 0, 0, 255], "the first chunk's last quad must draw");
        assert_eq!(sample(0), [255, 0, 0, 255], "the first chunk's first quad must draw");
    }

    /// A run of 45-degree rotated quads must land on the same interior pixels through the
    /// instanced matrix pipeline as it does through the software polygon fill.
    ///
    /// The comparison samples each square's mapped centre and the frame corners rather than a
    /// sloped boundary: the two rasterisers are hard-edged but use different fill rules, so
    /// exact agreement on an edge is not something either path promises.
    #[cfg(feature = "3d")]
    #[test]
    fn a_45_degree_instanced_run_agrees_with_the_software_path() {
        use crate::core::{Color, Point};
        use crate::render::math3d::Mat3;
        use crate::render::SoftwareSurface;

        let Some(renderer) = renderer() else { return };
        let affine = Mat3::translation(-16.0, -16.0)
            .then(&Mat3::rotation(core::f32::consts::FRAC_PI_4))
            .then(&Mat3::translation(16.0, 16.0));
        let run = [
            (quad_rect(6, 6, 14, 14), Color::RED),
            (quad_rect(18, 6, 26, 14), Color::GREEN),
            (quad_rect(6, 18, 14, 26), Color::BLUE),
        ];
        let quads: Vec<([Point; 4], [f64; 4])> =
            run.iter().map(|(corners, colour)| (*corners, gpu_colour(*colour))).collect();

        let gpu = renderer
            .render_fill_quads_gpu(32, 32, &affine, &quads)
            .expect("the instanced matrix pipeline must render");

        let mut surface = SoftwareSurface::new(crate::core::Size::new(32, 32), 1.0);
        surface.begin_frame(Color::TRANSPARENT);
        for (corners, colour) in run {
            let mapped = corners.map(|point| affine.map_point(point));
            surface.draw_path(&mapped, true, colour, true, 0);
        }
        surface.end_frame();
        let software = surface.frame_rgba();

        for ((corners, _colour), what) in run.iter().zip(["top-left", "top-right", "bottom-left"]) {
            let centre = affine.map_point(Point::new(
                (corners[0].x + corners[2].x) / 2,
                (corners[0].y + corners[2].y) / 2,
            ));
            let (x, y) = (centre.x as u32, centre.y as u32);
            assert_eq!(
                pixel(software, 32, x, y),
                pixel(&gpu, 32, x, y),
                "the {what} square centre must agree between the software and instanced paths"
            );
        }
        for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(
                pixel(software, 32, x, y),
                pixel(&gpu, 32, x, y),
                "a corner outside the rotated run must remain transparent in both paths"
            );
        }
    }

    /// A zero radius is the identity, and it must not even touch the GPU.
    #[test]
    fn a_zero_radius_blur_returns_the_input() {
        let Some(renderer) = renderer() else { return };
        let input = vec![9u8; 4 * 4 * 4];
        let output = renderer.render_blur_rgba8(4, 4, &input, 0).expect("identity blur");
        assert_eq!(output, input);
    }

    /// A uniform field is a fixed point of a normalised blur kernel.
    ///
    /// This is the property a separable GPU pass is most likely to break with a bad weight or an
    /// edge clamp: a field of one colour must stay that colour, because every tap reads the same
    /// value and the weights sum to one.
    #[test]
    fn a_uniform_field_survives_the_gpu_blur() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let output = renderer.render_blur_rgba8(8, 8, &input, 3).expect("the blur pass runs");
        for y in 0..8 {
            for x in 0..8 {
                let seen = pixel(&output, 8, x, y);
                for channel in 0..4 {
                    assert!(
                        seen[channel].abs_diff(input[channel]) <= 1,
                        "a uniform field must stay uniform at ({x},{y}), got {seen:?}"
                    );
                }
            }
        }
    }

    /// A blur moves colour across a hard edge and stays symmetric about it.
    ///
    /// The left half is red and the right half blue. After the blur the pixels either side of the
    /// seam must both contain some of each colour, and their values must mirror: the kernel is
    /// symmetric, so sampling one pixel left of the seam and one right of it must produce the same
    /// two-colour mixture.
    #[test]
    fn the_gpu_blur_spreads_an_edge_symmetrically() {
        let Some(renderer) = renderer() else { return };
        let width = 16u32;
        let height = 4u32;
        let mut input = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;
                if x < width / 2 {
                    input[offset..offset + 4].copy_from_slice(&[255, 0, 0, 255]);
                } else {
                    input[offset..offset + 4].copy_from_slice(&[0, 0, 255, 255]);
                }
            }
        }
        let output = renderer.render_blur_rgba8(width, height, &input, 4).expect("the blur runs");
        let left = pixel(&output, width, width / 2 - 1, 2);
        let right = pixel(&output, width, width / 2, 2);
        assert!(left[2] > 0, "blue must bleed left of the seam: {left:?}");
        assert!(right[0] > 0, "red must bleed right of the seam: {right:?}");
        // Symmetry about the seam swaps the two colours, so the red/blue channels mirror each
        // other rather than matching channel for channel.
        assert!(
            left[0].abs_diff(right[2]) <= 2 && left[2].abs_diff(right[0]) <= 2,
            "the two sides of a symmetric edge must mirror: {left:?} vs {right:?}"
        );
    }

    /// The depth-tested mesh path draws the near face of a cube and leaves the background clear.
    ///
    /// This runs only on a host with an adapter; on a headless host it compiles and skips, which is
    /// what keeps the pipeline type-checked without making the suite GPU-dependent.
    #[cfg(feature = "3d")]
    #[test]
    fn the_gpu_mesh_pipeline_draws_a_cube_through_a_depth_attachment() {
        use crate::render::math3d::Mat4;
        use crate::render::mesh3d::{Camera, Mesh3D, Vec3};

        let Some(renderer) = renderer() else { return };
        let mesh = Mesh3D::cube(Vec3::ZERO, 2.0, [crate::core::Color::RED; 6]);
        let camera = Camera::new(Vec3::new(3.0, 2.0, 6.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let pixels = renderer
            .render_mesh_3d_gpu(
                64,
                64,
                &mesh,
                &Mat4::IDENTITY,
                &camera,
                crate::core::Rect::new(0, 0, 64, 64),
            )
            .expect("the mesh pipeline runs");
        assert_eq!(pixels.len(), 64 * 64 * 4);
        let painted = pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        assert!(painted > 100, "the cube must cover part of the target, got {painted} pixels");
    }

    // ── G-219: SDF circle pipeline ──

    /// The SDF circle must paint the disc's interior opaquely and take partial alpha on the
    /// edge — including a pixel the old per-column strip approximation left untouched
    /// (G-219's mandated difference).
    #[test]
    fn sdf_circle_paints_an_anti_aliased_edge_the_strips_did_not() {
        let Some(renderer) = renderer() else { return };
        let pixels = renderer
            .render_fill_circle_gpu(32, 32, &[((16, 16, 8), [1.0, 0.0, 0.0, 1.0])])
            .expect("the SDF circle must render");
        // Centre: fully covered on both paths.
        assert_eq!(pixel(&pixels, 32, 16, 16), [255, 0, 0, 255]);
        // An interior pixel the strip path also painted at full colour.
        assert_eq!(pixel(&pixels, 32, 16, 12), [255, 0, 0, 255]);
        // (22, 10): distance 0.485 beyond the radius, so the SDF shades partial alpha. The
        // strip approximation painted nothing here (its column at x = 22 spans y 11..21).
        let edge = pixel(&pixels, 32, 22, 10);
        assert_eq!(&edge[..3], &[255, 0, 0], "the SDF edge keeps the fill colour: {edge:?}");
        assert!(
            edge[3] > 0 && edge[3] < 255,
            "the SDF edge must be anti-aliased (partial alpha), got {edge:?}"
        );
        // A corner of the bounding quad outside the disc stays transparent.
        assert_eq!(pixel(&pixels, 32, 8, 8), [0, 0, 0, 0]);
        assert_eq!(pixel(&pixels, 32, 31, 31), [0, 0, 0, 0]);
    }

    /// A zero-radius circle draws nothing, matching the software rasteriser's skip — and a
    /// frame of only zero-radius circles is a transparent frame.
    #[test]
    fn a_zero_radius_circle_draws_nothing() {
        let Some(renderer) = renderer() else { return };
        let pixels = renderer
            .render_fill_circle_gpu(8, 8, &[((4, 4, 0), [1.0, 0.0, 0.0, 1.0])])
            .expect("a zero-radius circle must not error");
        assert!(pixels.iter().all(|&byte| byte == 0));
    }

    /// The circle quad must clamp to the viewport in `i64`, so extreme centres and radii
    /// neither wrap nor panic; a disc covering no pixel reports `None`.
    #[test]
    fn circle_quad_clamps_to_the_viewport() {
        assert_eq!(circle_quad((16, 16), 8, 32, 32), Some((8, 8, 25, 25)));
        assert_eq!(circle_quad((0, 0), 2, 32, 32), Some((0, 0, 3, 3)));
        assert_eq!(circle_quad((-100, -100), 4, 32, 32), None);
        assert_eq!(circle_quad((40, 16), 8, 32, 32), None);
        // Extreme coordinates must not wrap: the arithmetic runs in `i64`.
        assert_eq!(circle_quad((i32::MAX, 0), 1, 32, 32), None);
        assert_eq!(circle_quad((i32::MIN, 0), u32::MAX, 32, 32), Some((0, 0, 32, 32)));
    }

    /// The instance packer must write the documented 48-byte layout: corners as little-endian
    /// `f32`, then the clamped colour, appending to whatever the scratch buffer already holds.
    #[test]
    fn pack_instanced_quads_writes_the_documented_layout() {
        let quads = [
            (
                [
                    crate::core::Point::new(1, 2),
                    crate::core::Point::new(3, 2),
                    crate::core::Point::new(3, 4),
                    crate::core::Point::new(1, 4),
                ],
                [0.25, 0.5, 0.75, 1.0],
            ),
            (
                [
                    crate::core::Point::new(5, 6),
                    crate::core::Point::new(7, 6),
                    crate::core::Point::new(7, 8),
                    crate::core::Point::new(5, 8),
                ],
                [1.0, 0.0, 0.0, 0.5],
            ),
        ];
        let mut packed = Vec::new();
        pack_instanced_quads(&quads, &mut packed).expect("packs");
        assert_eq!(packed.len(), 2 * INSTANCE_BYTES);
        let f32_at = |record: usize, offset: usize| {
            f32::from_le_bytes(
                packed[record * INSTANCE_BYTES + offset..record * INSTANCE_BYTES + offset + 4]
                    .try_into()
                    .expect("four bytes"),
            )
        };
        assert_eq!(f32_at(0, 0), 1.0);
        assert_eq!(f32_at(0, 4), 2.0);
        assert_eq!(f32_at(0, 24), 1.0);
        assert_eq!(f32_at(0, 28), 4.0);
        assert_eq!(f32_at(0, 32), 0.25);
        assert_eq!(f32_at(0, 44), 1.0);
        assert_eq!(f32_at(1, 0), 5.0);
        assert_eq!(f32_at(1, 44), 0.5);
        // Packing again into the same buffer clears rather than appends.
        pack_instanced_quads(&quads[..1], &mut packed).expect("repacks");
        assert_eq!(packed.len(), INSTANCE_BYTES);
    }

    // ── G-220: tint rides the vertical blur pass ──

    /// A full-weight opaque tint composites straight-alpha over the blurred field, entirely on
    /// the GPU: a uniform field must come out at the tint colour (G-220).
    #[test]
    fn a_full_weight_tint_reaches_the_gpu_vertical_pass() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let output = renderer
            .render_backdrop_blur_rgba8(
                8,
                8,
                &input,
                2,
                Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 1.0)),
            )
            .expect("the tinted blur runs");
        for y in 0..8 {
            for x in 0..8 {
                let seen = pixel(&output, 8, x, y);
                assert!(
                    seen[0].abs_diff(255) <= 2
                        && seen[1].abs_diff(0) <= 2
                        && seen[2].abs_diff(0) <= 2
                        && seen[3].abs_diff(255) <= 2,
                    "a full-weight opaque tint must dominate at ({x},{y}), got {seen:?}"
                );
            }
        }
    }

    /// A half-weight tint must blend halfway between the blurred field and the tint. The
    /// tolerance covers the GPU float pipeline's rounding, which differs from the CPU
    /// composite by at most a couple of counts.
    #[test]
    fn a_half_weight_tint_blends_halfway() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let output = renderer
            .render_backdrop_blur_rgba8(
                8,
                8,
                &input,
                2,
                Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 0.5)),
            )
            .expect("the tinted blur runs");
        for y in 0..8 {
            for x in 0..8 {
                let seen = pixel(&output, 8, x, y);
                assert!(
                    seen[0].abs_diff(178) <= 2
                        && seen[1].abs_diff(75) <= 2
                        && seen[2].abs_diff(100) <= 2
                        && seen[3].abs_diff(255) <= 2,
                    "a half-weight tint must blend halfway at ({x},{y}), got {seen:?}"
                );
            }
        }
    }

    /// A zero-weight tint must leave the blur byte-identical to the untinted path, and a
    /// non-finite weight must be ignored rather than poisoning the frame.
    #[test]
    fn a_zero_or_non_finite_weight_tint_leaves_the_blur_untouched() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let plain = renderer.render_blur_rgba8(8, 8, &input, 3).expect("plain blur");
        let zero = renderer
            .render_backdrop_blur_rgba8(
                8,
                8,
                &input,
                3,
                Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 0.0)),
            )
            .expect("zero-weight blur");
        assert_eq!(zero, plain, "a zero-weight tint must not change the blur");
        let nan = renderer
            .render_backdrop_blur_rgba8(
                8,
                8,
                &input,
                3,
                Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, f32::NAN)),
            )
            .expect("nan-weight blur");
        assert_eq!(nan, plain, "a non-finite weight must be ignored");
    }

    /// A zero-radius backdrop blur must still apply the tint: the shader's `radius <= 0`
    /// branch is a passthrough of the input plus the tint composite, matching the software
    /// layer composition where the tint sits on the unblurred backdrop.
    #[test]
    fn a_zero_radius_backdrop_blur_still_applies_the_tint() {
        let Some(renderer) = renderer() else { return };
        let input: Vec<u8> = (0..4 * 4).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let output = renderer
            .render_backdrop_blur_rgba8(
                4,
                4,
                &input,
                0,
                Some((Rgba8 { r: 0, g: 255, b: 0, a: 255 }, 1.0)),
            )
            .expect("the passthrough blur runs");
        for y in 0..4 {
            for x in 0..4 {
                let seen = pixel(&output, 4, x, y);
                assert!(
                    seen[0].abs_diff(0) <= 2
                        && seen[1].abs_diff(255) <= 2
                        && seen[2].abs_diff(0) <= 2
                        && seen[3].abs_diff(255) <= 2,
                    "the tint must composite over the unblurred backdrop at ({x},{y}), got {seen:?}"
                );
            }
        }
    }

    // ── G-218: command-list GPU dispatch ──

    fn opaque(r: u8, g: u8, b: u8) -> Rgba8 {
        Rgba8 { r, g, b, a: 255 }
    }

    /// A stream the pipelines cover must render on the GPU and agree with the software
    /// rasteriser at interior samples. Edge pixels are deliberately avoided: the SDF circle's
    /// anti-aliased edge is the mandated difference from the CPU's hard-edged disc.
    #[test]
    fn a_dispatchable_stream_matches_the_software_raster() {
        let commands = vec![
            WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 10, height: 10 },
                color: opaque(255, 0, 0),
                clip: None,
            },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 10, y: 0, width: 10, height: 10 },
                color: opaque(0, 255, 0),
                clip: None,
            },
            WgpuDrawCommand::SetBlendMode { mode: 0 },
            WgpuDrawCommand::FillCircle {
                center: (15, 20),
                radius: 6,
                color: opaque(0, 0, 255),
                clip: None,
            },
        ];
        let Some(renderer) = renderer() else { return };
        let gpu = renderer
            .render_draw_commands_rgba8(32, 32, &commands)
            .expect("the dispatchable stream must render on the GPU");
        // Interior samples only: rect interiors, the circle centre and one more interior
        // circle pixel, plus two pixels that must show the clear colour.
        assert_eq!(pixel(&gpu, 32, 5, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&gpu, 32, 15, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&gpu, 32, 15, 20), [0, 0, 255, 255]);
        assert_eq!(pixel(&gpu, 32, 15, 17), [0, 0, 255, 255]);
        assert_eq!(pixel(&gpu, 32, 5, 31), [10, 20, 30, 255]);
        assert_eq!(pixel(&gpu, 32, 31, 31), [10, 20, 30, 255]);
    }

    /// A stream containing a command class no pipeline covers must fall back to the software
    /// rasteriser — returned directly, so the result is byte-identical to the CPU picture and
    /// no upload/readback round trip happens in between (G-217).
    #[test]
    fn a_non_dispatchable_stream_falls_back_to_the_software_raster() {
        let commands = vec![
            WgpuDrawCommand::PushClip { rect: PixelRect { x: 0, y: 0, width: 5, height: 10 } },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 10, height: 10 },
                color: opaque(255, 0, 0),
                clip: None,
            },
            WgpuDrawCommand::PopClip,
        ];
        let Some(renderer) = renderer() else { return };
        let gpu = renderer
            .render_draw_commands_rgba8(16, 16, &commands)
            .expect("the fallback must render");
        let software = rasterize_draw_commands_rgba8(16, 16, &commands).expect("rasterises");
        assert_eq!(gpu, software, "the fallback returns the software picture directly");
    }

    /// A blend mode other than `Normal` is not dispatchable: the pipelines' `REPLACE` write
    /// cannot reproduce it, so the stream must fall back to the CPU rasteriser.
    #[test]
    fn a_non_normal_blend_mode_falls_back() {
        let commands = vec![
            WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
            WgpuDrawCommand::SetBlendMode { mode: 1 },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
                color: opaque(128, 128, 128),
                clip: None,
            },
        ];
        let Some(renderer) = renderer() else { return };
        let gpu =
            renderer.render_draw_commands_rgba8(8, 8, &commands).expect("the fallback must render");
        let software = rasterize_draw_commands_rgba8(8, 8, &commands).expect("rasterises");
        assert_eq!(gpu, software, "a non-Normal blend mode must fall back to the CPU picture");
    }

    /// A mid-stream `Clear` must erase everything before it, exactly as the command's
    /// documentation promises, with later draws landing on the new background.
    #[test]
    fn a_mid_stream_clear_repaints_the_frame() {
        let commands = vec![
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 20, height: 20 },
                color: opaque(255, 0, 0),
                clip: None,
            },
            WgpuDrawCommand::Clear { color: opaque(0, 0, 255) },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
                color: opaque(0, 255, 0),
                clip: None,
            },
        ];
        let Some(renderer) = renderer() else { return };
        let gpu =
            renderer.render_draw_commands_rgba8(24, 24, &commands).expect("the stream must render");
        assert_eq!(pixel(&gpu, 24, 10, 10), [0, 0, 255, 255], "the clear erases the first fill");
        assert_eq!(
            pixel(&gpu, 24, 2, 2),
            [0, 255, 0, 255],
            "later draws sit on the new background"
        );
        assert_eq!(pixel(&gpu, 24, 23, 23), [0, 0, 255, 255]);
    }

    /// An over-cap run of rectangles through the dispatcher must stream across chunks without
    /// reordering, the same contract the instanced quad pipeline promises (G-218).
    #[test]
    fn an_over_cap_dispatch_run_streams_every_chunk_in_order() {
        let Some(renderer) = renderer() else { return };
        let per_row = 1024u32;
        let total = MAX_INSTANCES_PER_DRAW + 1;
        let width = per_row * 2;
        let height = total.div_ceil(per_row as usize) as u32;
        let mut commands = Vec::with_capacity(total);
        for index in 0..total {
            let x = ((index as u32 % per_row) * 2) as i32;
            let y = (index as u32 / per_row) as i32;
            let color = if index + 1 == total { opaque(0, 0, 255) } else { opaque(255, 0, 0) };
            commands.push(WgpuDrawCommand::FillRect {
                rect: PixelRect { x, y, width: 2, height: 1 },
                color,
                clip: None,
            });
        }
        let pixels = renderer
            .render_draw_commands_rgba8(width, height, &commands)
            .expect("an over-cap dispatch run must render");
        let sample = |index: usize| -> [u8; 4] {
            let x = (index as u32 % per_row) * 2;
            let y = index as u32 / per_row;
            pixel(&pixels, width, x, y)
        };
        let last = total - 1;
        assert_eq!(sample(last), [0, 0, 255, 255], "the second chunk must draw its own rect");
        assert_eq!(sample(last - 1), [255, 0, 0, 255], "the first chunk's last rect must draw");
        assert_eq!(sample(0), [255, 0, 0, 255], "the first chunk's first rect must draw");
    }
}
