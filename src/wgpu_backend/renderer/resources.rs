// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Device resources the renderer keeps across calls: the grow-only buffer and texture caches,
//! their uniform arenas, and the byte-size helpers they share.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{align_to, rasterize_draw_commands_into_rgba8};
use super::super::types::Rgba8;
use std::cell::{Ref, RefCell};
use std::time::Duration;

/// Upper bound on a single blocking GPU readback.
///
/// The readback is synchronous by design: every entry point returns owned pixels. Before this
/// bound existed, `device.poll` waited forever, so a lost device or a stuck submission hung the
/// caller instead of reporting an error (G-63).
pub(crate) const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(10);

/// One persistent colour-uniform arena shared by every draw of one call.
///
/// WebGPU requires a dynamic uniform offset to be a multiple of the device's
/// `min_uniform_buffer_offset_alignment` (256 on the common backends, but a
/// device may report more), so each rectangle's 16-byte colour is padded to a
/// stride read from the device rather than assumed. One buffer and one bind
/// group serve the whole call instead of one of each per rectangle (REND-02).
pub(crate) struct UniformArena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) capacity: u64,
    /// Dynamic-offset stride: the device's `min_uniform_buffer_offset_alignment`,
    /// raised to hold the 16-byte colour, so every offset stays legal on any backend.
    ///
    /// Only the per-rectangle path reads this; an instanced run carries colour in its instance
    /// records instead.
    pub(crate) stride: u64,
}

/// The offscreen render target for one `(width, height)`.
pub(crate) struct TargetCache {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
}

/// A grow-only vertex buffer reused across calls.
///
/// The same cache holds both the per-rectangle vertex positions and the instanced quad records:
/// both uses are `VERTEX`-usage buffers written once per call, and sharing one grow-only
/// allocation is what keeps an instanced run from allocating once per quad.
pub(crate) struct VertexCache {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity: u64,
}

/// A grow-only readback buffer reused across calls.
pub(crate) struct ReadbackCache {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) size: u64,
}

/// The two ping-pong targets and per-direction bind groups of the blur pass.
///
/// The pass reads a texture and writes one that is not also bound as its attachment, so two
/// textures are required. Both are `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_*`: the first is
/// uploaded into and read back from, the second is only ever the horizontal pass's target. Cached
/// per `(width, height)` and rebuilt when the size changes (REND-02's reuse rule).
pub(crate) struct BlurTargets {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) source: wgpu::Texture,
    pub(crate) source_view: wgpu::TextureView,
    pub(crate) scratch_view: wgpu::TextureView,
    pub(crate) horizontal: wgpu::BindGroup,
    pub(crate) vertical: wgpu::BindGroup,
    pub(crate) horizontal_params: wgpu::Buffer,
    pub(crate) vertical_params: wgpu::Buffer,
}

/// The model-view-projection uniform used by the instanced quad pipeline.
///
/// One matrix serves a whole call, so this is not a dynamic-offset arena. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
pub(crate) struct TransformedUniform {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
}

/// The colour-and-depth pair the `3d` mesh pipeline renders into.
///
/// The view owns its texture, so the texture handle itself does not need to be stored. Cached per
/// `(width, height)` like the other targets (REND-02) and rebuilt when the size changes.
#[cfg(feature = "3d")]
pub(crate) struct DepthTarget {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) view: wgpu::TextureView,
}

/// Grow-only uniform arena for the SDF circle pipeline (G-219).
///
/// Each circle consumes two 16-byte slots at the dynamic-offset stride — the fill colour at
/// `2 * i` and the radius/centre SDF parameters at `2 * i + 1` — so one buffer and one bind
/// group serve every circle of a frame. The bind group binds the same buffer at both
/// entries; the per-draw dynamic offsets select the slots.
pub(crate) struct CircleUniformArena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity: u64,
    pub(crate) bind_group: wgpu::BindGroup,
}

/// Grow-only CPU frame storage for one render path (G-221).
///
/// The rasteriser zero-fills the frame, so a reused allocation is byte-identical to a fresh
/// one; only the capacity is retained. A separate cell per path lets a caller keep one frame
/// borrowed (for an upload) without blocking a different path's borrow.
pub(crate) struct CpuFrame {
    pub(crate) bytes: RefCell<Vec<u8>>,
}

impl CpuFrame {
    pub(crate) fn new() -> Self {
        Self { bytes: RefCell::new(Vec::new()) }
    }

    /// Rasterises `commands` into this path's buffer and lends the frame back to the caller.
    ///
    /// The buffer grows only. `bytes` is re-borrowed immutably for the returned guard after
    /// the rasteriser has finished, so the caller cannot observe a half-drawn frame.
    pub(crate) fn rasterize(
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
pub(crate) enum DrawStep {
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
pub(crate) struct PackScratch {
    /// Instanced quad records, `INSTANCE_BYTES` per instance.
    pub(crate) instances: Vec<u8>,
    /// Per-rect NDC vertices, 48 bytes (twelve `f32`) per rectangle.
    pub(crate) vertices: Vec<u8>,
    /// Colour slots staged for the shared uniform arena, one aligned slot per rectangle.
    pub(crate) uniform: Vec<u8>,
    /// SDF circle bounding quads, six 8-byte vertices per circle (G-219).
    pub(crate) circle_vertices: Vec<u8>,
    /// SDF circle uniform slots, two aligned slots per circle (G-219).
    pub(crate) circle_uniforms: Vec<u8>,
    /// The command-stream dispatcher's draw order (G-218).
    pub(crate) plan: Vec<DrawStep>,
    /// Mesh vertices, 32 bytes per vertex (`3d`).
    #[cfg(feature = "3d")]
    pub(crate) mesh_vertices: Vec<u8>,
    /// Mesh indices as little-endian `u32` (`3d`).
    #[cfg(feature = "3d")]
    pub(crate) mesh_indices: Vec<u8>,
}

/// Creates a 64-byte matrix uniform and the bind group that exposes it at group 0, binding 0.
///
/// Both the instanced quad pipeline and the mesh pipeline use one matrix per call rather than a
/// dynamic-offset arena, so this is the one shape their resources share. The 64-byte size is
/// `mat4x4f`'s uniform footprint.
pub(crate) fn create_matrix_uniform(
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
pub(crate) struct RenderCache {
    pub(crate) uniform: Option<UniformArena>,
    pub(crate) target: Option<TargetCache>,
    pub(crate) upload_target: Option<TargetCache>,
    pub(crate) vertices: Option<VertexCache>,
    pub(crate) readback: Option<ReadbackCache>,
    /// Ping-pong targets and bind groups for the separable blur pass.
    pub(crate) blur: Option<BlurTargets>,
    /// Mvp uniform and bind group for the instanced quad pipeline.
    pub(crate) transformed_uniform: Option<TransformedUniform>,
    /// Grow-only uniform arena for the SDF circle pipeline (G-219).
    pub(crate) circle_uniforms: Option<CircleUniformArena>,
    /// CPU-side packing buffers refilled each call instead of reallocated (G-216).
    pub(crate) pack: PackScratch,
    /// The `Depth32Float` attachment for mesh draws.
    #[cfg(feature = "3d")]
    pub(crate) depth_target: Option<DepthTarget>,
    /// Mvp uniform and bind group for the mesh pipeline.
    #[cfg(feature = "3d")]
    pub(crate) mesh_uniform: Option<TransformedUniform>,
    /// Grow-only index buffer for mesh draws.
    #[cfg(feature = "3d")]
    pub(crate) mesh_indices: Option<VertexCache>,
}

// Retain headroom without exceeding the device limit; buffers remain grow-only.
pub(crate) fn grown_buffer_capacity(current: u64, needed: u64, limit: u64) -> u64 {
    let grown = current.saturating_add(current / 2).max(needed).max(4);
    let rounded = grown.checked_add(65_535).map(|size| size / 65_536 * 65_536).unwrap_or(limit);
    rounded.min(limit).max(needed.max(4))
}

impl RenderCache {
    /// The dynamic-offset stride for the colour arena: the device's
    /// `min_uniform_buffer_offset_alignment`, raised to hold the 16-byte colour
    /// and rounded back up to a legal multiple. A hard-coded 256 would make every
    /// draw a validation error on a device that reports 512.
    pub(crate) fn uniform_stride(device: &wgpu::Device) -> u64 {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        alignment.max(16).next_multiple_of(alignment.max(1))
    }

    /// Ensures the colour arena can hold `slots` colours.
    pub(crate) fn ensure_uniform(
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
    pub(crate) fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
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
    pub(crate) fn ensure_upload_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
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
    pub(crate) fn ensure_vertices(&mut self, device: &wgpu::Device, bytes: u64) {
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
    pub(crate) fn ensure_readback(&mut self, device: &wgpu::Device, size: u64) {
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
    pub(crate) fn ensure_blur_targets(
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
    pub(crate) fn ensure_transformed_uniform(
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
    pub(crate) fn ensure_mesh_uniform(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) {
        if self.mesh_uniform.is_some() {
            return;
        }
        self.mesh_uniform = Some(create_matrix_uniform(device, layout, "mesh_mvp"));
    }

    /// Ensures the `Depth32Float` attachment matches `(width, height)`.
    #[cfg(feature = "3d")]
    pub(crate) fn ensure_depth_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
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
    pub(crate) fn ensure_mesh_indices(&mut self, device: &wgpu::Device, bytes: u64) {
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
    pub(crate) fn ensure_circle_uniforms(
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
pub(crate) fn rgba_buffer_len(width: u32, height: u32) -> Result<usize, String> {
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
pub(crate) fn rgba8_from_unit(color: [f64; 4]) -> Rgba8 {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0) as u8;
    Rgba8 { r: channel(color[0]), g: channel(color[1]), b: channel(color[2]), a: channel(color[3]) }
}

/// Bytes per row of an RGBA8 image, aligned to WebGPU's copy requirement.
pub(crate) fn padded_row_bytes(width: u32) -> Result<u32, String> {
    let unpadded = width
        .checked_mul(4)
        .ok_or_else(|| format!("row byte width for {width} pixels overflows u32"))?;
    Ok(align_to(unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}

/// Total bytes of a padded RGBA8 readback buffer.
pub(crate) fn readback_frame_bytes(width: u32, height: u32) -> Result<u64, String> {
    let padded = padded_row_bytes(width)?;
    (padded as u64)
        .checked_mul(height as u64)
        .ok_or_else(|| format!("readback buffer for {width}x{height} overflows u64"))
}
