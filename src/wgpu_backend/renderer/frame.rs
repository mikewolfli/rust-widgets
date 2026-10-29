// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The renderer itself: pipeline construction, the clear path, the readback plumbing and the
//! [`GpuRenderer`] implementation.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::rasterize_draw_commands_rgba8;
use super::super::shaders::ShaderModule;
use super::encode::INSTANCE_BYTES;
use super::resources::{
    padded_row_bytes, readback_frame_bytes, rgba8_from_unit, rgba_buffer_len, CpuFrame,
    RenderCache, GPU_READBACK_TIMEOUT,
};
use crate::render::gpu::{GpuCapability, GpuRenderer};
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::sync::mpsc;

/// Lightweight GPU renderer context backed by `wgpu`.
pub struct WgpuRenderer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) clear_pipeline: wgpu::RenderPipeline,
    /// The 2D per-rectangle pipeline. Used only for the single-rectangle case; a longer run goes
    /// through the instanced quad pipeline instead (G-64).
    pub(crate) rect_pipeline: wgpu::RenderPipeline,
    /// The separable-blur pipeline: one full-screen triangle, no vertex buffer, two directional
    /// passes. A 2D effect, so it is compiled in every build rather than behind `3d`.
    pub(crate) blur_pipeline: wgpu::RenderPipeline,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout of the blur pass: a non-filterable input texture and a 32-byte
    /// uniform (`direction`, `radius`, and the G-220 tint block the vertical pass composites).
    pub(crate) blur_bind_group_layout: wgpu::BindGroupLayout,
    /// Instanced quad pipeline. It renders a whole run of rectangles (and, with `3d`, arbitrary
    /// transformed quads) in `MAX_INSTANCES_PER_DRAW`-sized draws (G-64).
    pub(crate) transformed_pipeline: wgpu::RenderPipeline,
    /// The vertex-stage layout that carries the mvp matrix (group 0 of the instanced pipeline).
    pub(crate) transformed_bind_group_layout: wgpu::BindGroupLayout,
    /// The SDF circle pipeline: one bounding quad per circle, shaded by a distance field in
    /// the fragment stage (G-219).
    pub(crate) circle_pipeline: wgpu::RenderPipeline,
    /// Bind group layout of the SDF circle pipeline: two dynamic-offset uniforms (the fill
    /// colour and the radius/centre parameters) at group 0; the mvp matrix rides group 1.
    pub(crate) circle_bind_group_layout: wgpu::BindGroupLayout,
    /// Depth-tested triangle pipeline for mesh draws (`3d` only).
    #[cfg(feature = "3d")]
    pub(crate) mesh_pipeline: wgpu::RenderPipeline,
    /// Cached shader module pipeline references (BLUE11 R5.2).
    pub(crate) shader_cache: HashMap<String, wgpu::ShaderModule>,
    /// Device resources reused across calls (REND-02).
    pub(crate) cache: RefCell<RenderCache>,
    /// Reusable CPU frame for whole-frame clears (G-221).
    pub(crate) clear_frame: CpuFrame,
    /// Reusable CPU frame for command streams (G-221).
    pub(crate) commands_frame: CpuFrame,
    /// Reusable CPU frame for the dispatcher's inner fallback, which runs while `cache` is
    /// already borrowed, so it cannot share `commands_frame` (G-221).
    pub(crate) fallback_frame: CpuFrame,
    /// What `wgpu` reported for the selected adapter, kept so [`GpuRenderer::vendor_info`] can
    /// answer with the real device name instead of a constant (G-232).
    pub(crate) adapter_info: wgpu::AdapterInfo,
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

    /// Copies the cached render target into a tight RGBA8 buffer.
    ///
    /// The target must already hold the picture; this helper only arranges the copy and the blocking
    /// read. Kept separate so an entry point that renders through a new pipeline does not repeat the
    /// row-padding arithmetic.
    #[cfg(feature = "3d")]
    pub(crate) fn read_target(
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
    pub(crate) fn read_back_owned(
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
