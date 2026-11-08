// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pipeline construction for [`WgpuRenderer`].
//!
//! Every pipeline, bind-group layout and shader module the renderer draws with is built here, on
//! the device the caller supplies. Split out of `frame.rs` at the seam principle #9 points at:
//! **building** the renderer (this file, ~1300 lines of pipeline descriptions) is a different
//! subject from **using** it (frame encoding, readback and the public accessors in `frame.rs`).
//!
//! `from_device` exists because a presentation target owns its own device — a surface belongs to
//! the device that created it — so the `Smooth` path has to build these pipelines on that device.

use super::encode::INSTANCE_BYTES;
use super::frame::{passive_stencil_state, WgpuRenderer};
use super::policy::{CostGate, RenderQuality};
use super::resources::{
    CpuFrame, ParamsGroupCache, ParamsGroups, ParamsScratch, RenderCache, TextCoverageCache,
    TextureCache,
};
use std::cell::RefCell;
use std::collections::HashMap;

/// Builds this device's hardware-adaptive manager from the facts `wgpu` reported for it.
///
/// One adapter-selection path, not two: the renderer already ran the degradation ladder to obtain
/// the adapter, so the manager is constructed *from* that answer
/// ([`crate::gpu::GpuManager::from_adapter_info`]) rather than selecting again. A manager that
/// cannot be built is reported as a construction error instead of being silently replaced by a
/// stub: a renderer whose adaptive facts are made up is worse than one that refuses to start.
fn gpu_manager(adapter_info: &wgpu::AdapterInfo) -> Result<crate::gpu::GpuManager, String> {
    let facts = crate::gpu::AdapterInfo::from_wgpu(adapter_info);
    crate::gpu::GpuManager::from_adapter_info(facts).map_err(|error| {
        format!("the hardware-adaptive GPU manager could not be built for this adapter: {error}")
    })
}

impl WgpuRenderer {
    /// Builds the renderer's pipelines on an existing device and queue.
    ///
    /// # Why this exists
    ///
    /// A presentation target owns its own device (it must, because a surface has to be created
    /// against the device that will draw into it). Drawing a bridged `Smooth` command stream into
    /// that target therefore needs these pipelines **on the target's device** — a second
    /// `WgpuRenderer` with its own device could not touch the swapchain texture at all. This
    /// constructor is that seam; [`Self::new`] is the same thing plus instance, adapter and device
    /// creation.
    ///
    /// The device and queue are taken by value and cloned by the caller when it keeps its own
    /// handle: `wgpu`'s handles are reference counted.
    pub(crate) fn from_device(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: wgpu::AdapterInfo,
    ) -> Result<Self, String> {
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
        let shadow_blur_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_blur_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::SHADOW_BLUR_FS.into()),
        });
        let blend_composite_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blend_composite_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::BLEND_COMPOSITE_FS.into()),
        });
        let circle_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("circle_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::CIRCLE_VS.into()),
        });
        let circle_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("circle_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::CIRCLE_FRAG.into()),
        });
        let image_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::IMAGE_VS.into()),
        });
        let image_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::IMAGE_FRAG.into()),
        });
        let text_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::TEXT_VS.into()),
        });
        let text_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::TEXT_FRAG.into()),
        });
        let shape_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shape_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::SHAPE_VS.into()),
        });
        let shape_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shape_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::SHAPE_FRAG.into()),
        });
        let radial_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("radial_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::RADIAL_VS.into()),
        });
        let radial_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("radial_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::RADIAL_FRAG.into()),
        });
        let gradient_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::GRADIENT_VS.into()),
        });
        let gradient_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::GRADIENT_FRAG.into()),
        });
        let image_scaled_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image_scaled_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::SCALED_IMAGE_FRAG.into()),
        });
        let path_fan_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("path_fan_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::PATH_FAN_VS.into()),
        });
        let path_stencil_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("path_stencil_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::PATH_STENCIL_FS.into()),
        });
        let path_cover_vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("path_cover_vs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::PATH_COVER_VS.into()),
        });
        let path_cover_fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("path_cover_fs"),
            source: wgpu::ShaderSource::Wgsl(super::shaders::PATH_COVER_FS.into()),
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
            depth_stencil: Some(passive_stencil_state()),
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
            depth_stencil: Some(passive_stencil_state()),
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

        // ── Create the shadow blur pipeline ──
        //
        // Same layout, vertex shader and blend state as the backdrop blur; only the fragment stage
        // differs, because a shadow layer is filtered with the CPU rasteriser's clamped box kernel
        // rather than the backdrop's Gaussian (see `SHADOW_BLUR_FS`).
        let shadow_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_blur_pipeline"),
            layout: Some(&blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blur_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shadow_blur_fs_module,
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

        let (blend_composite_pipeline, blend_bind_group_layout) =
            super::effects::blend_composite(&device, &blur_vs_module, &blend_composite_fs_module);

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
                    // Source-over, **not** `REPLACE`: the quad is the disc's bounding box and most
                    // of it returns transparent coverage, so replacing would erase whatever the
                    // frame had drawn inside that box.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            depth_stencil: Some(passive_stencil_state()),
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
        let transformed_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("rect_transformed_pipeline_layout"),
                bind_group_layouts: &[Some(&transformed_bind_group_layout)],
                immediate_size: 0,
            });
        // Locations 0/1 are the four corners as two vectors; location 2 is the colour.
        let instance_buffer_layout = || wgpu::VertexBufferLayout {
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
        let transformed_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect_instanced_pipeline"),
            layout: Some(&transformed_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &transformed_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(instance_buffer_layout())],
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
            depth_stencil: Some(passive_stencil_state()),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // ── Create the shadow body pipeline ──
        //
        // The same instanced quad pipeline, minus the stencil state: a shadow body is painted into
        // its own offscreen layer, and a layer pass carries no stencil attachment (WebGPU requires
        // every attachment in a pass to have the same size, and the layer is smaller than the
        // frame). The layer is cleared first, so `REPLACE` writes the body exactly.
        let shadow_body_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_body_pipeline"),
            layout: Some(&transformed_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &transformed_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(instance_buffer_layout())],
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
        });

        // ── Create the image pipeline ──
        //
        // Group 0 carries the draw's params uniform and its source texture; group 1 is the shared
        // mvp matrix layout. There is no vertex buffer: the six corners come from the params, so
        // a frame of images uploads geometry-free draws. The fragment stage fetches one texel per
        // destination pixel, so the sampler this layout deliberately omits would have nothing to
        // do — and its absence is what keeps the result identical to the CPU rasteriser's.
        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("image_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(wgpu::BufferSize::new(32).unwrap()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("image_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&image_bind_group_layout),
                    Some(&transformed_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("image_pipeline"),
            layout: Some(&image_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &image_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &image_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    // Straight-alpha source-over: the shader returns the texel unchanged and
                    // compositing is this state's job, exactly as the CPU blend does it.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            depth_stencil: Some(passive_stencil_state()),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // ── Create the scaled-image pipeline ──
        //
        // The same bind group layout, vertex stage and blend state as the image pipeline; only the
        // fragment stage differs (four texels instead of one). Sharing the layout is what keeps
        // "one image draw" a single concept: `SCALED_IMAGE_FRAG` reads the same `ImageParams` and the
        // same texture, and the pass picks the pipeline from the draw's own flag.
        let image_scaled_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("image_scaled_pipeline"),
                layout: Some(&image_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &image_vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &image_scaled_fs_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
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
                depth_stencil: Some(passive_stencil_state()),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            });

        // ── Create the text pipeline ──
        //
        // The same shape as the image pipeline: group 0 carries the box's params (destination and
        // colour) and its coverage texture, group 1 the shared mvp. The coverage texture is
        // `R8Unorm` — one byte per destination pixel — and the fragment stage fetches it at the
        // pixel's own coordinate, so no sampler is declared and no filtering can blur a stem.
        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("text_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(wgpu::BufferSize::new(32).unwrap()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[
                Some(&text_bind_group_layout),
                Some(&transformed_bind_group_layout),
            ],
            immediate_size: 0,
        });
        // Two pipelines share that layout and the same shader pair: the frame's text composites
        // source-over, and a blend run needs the same coverage written *straight* into its
        // offscreen layer so the mode can be applied to the colour before the coverage.
        let make_text_pipeline = |label: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&text_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &text_vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &text_fs_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(blend),
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
                depth_stencil: Some(passive_stencil_state()),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let text_pipeline = make_text_pipeline(
            "text_pipeline",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        );
        let text_layer_pipeline =
            make_text_pipeline("text_layer_pipeline", wgpu::BlendState::REPLACE);

        // ── Create the shape pipeline (rings, wedges, rounded outlines) ──
        //
        // One pipeline covers the family: the fragment stage picks an SDF by the draw's mode and
        // feathers the boundary over one pixel. Blending is source-over — the shapes are partial
        // coverage, so the pipeline must composite rather than replace.
        let shape_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shape_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(wgpu::BufferSize::new(64).unwrap()),
                    },
                    count: None,
                }],
            });
        let shape_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shape_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&shape_bind_group_layout),
                    Some(&transformed_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shape_pipeline"),
            layout: Some(&shape_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shape_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shape_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            depth_stencil: Some(passive_stencil_state()),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // ── Create the radial-gradient pipeline ──
        //
        // Group 0 carries the gradient's params (centre, radius, stop count, destination) and its
        // stop texture; group 1 the shared mvp. Blending is source-over, since the gradient fills
        // the rectangle like any other opaque draw.
        let radial_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("radial_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(wgpu::BufferSize::new(48).unwrap()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let radial_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("radial_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&radial_bind_group_layout),
                    Some(&transformed_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let radial_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("radial_pipeline"),
            layout: Some(&radial_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &radial_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &radial_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            depth_stencil: Some(passive_stencil_state()),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        // ── Create the gradient pipeline ──
        //
        // The reference's own 256-entry ramp on the GPU, along an arbitrary linear / radial / conic
        // geometry (`GRADIENT_FRAG` decides which). Group 0 carries the params uniform (64 bytes:
        // rect, start, end, info) and the ramp texture; group 1 the shared mvp. Blending matches the
        // radial pipeline — source-over — because the ramp entries may carry alpha and the
        // reference composites through the same rule.
        let gradient_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gradient_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(wgpu::BufferSize::new(64).unwrap()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let gradient_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gradient_pipeline_layout"),
                bind_group_layouts: &[
                    Some(&gradient_bind_group_layout),
                    Some(&transformed_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let make_gradient_pipeline = |label: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&gradient_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &gradient_vs_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &gradient_fs_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(blend),
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
                depth_stencil: Some(passive_stencil_state()),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let gradient_pipeline = make_gradient_pipeline(
            "gradient_pipeline",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        );
        // The standalone conic command stores its ramp colour verbatim, so it needs the same
        // pipeline with `REPLACE` blending — exactly the pair the conic fill's two software arms
        // are (the fill composites, the command does not).
        let gradient_verbatim_pipeline =
            make_gradient_pipeline("gradient_verbatim_pipeline", wgpu::BlendState::REPLACE);

        // ── Create the path-fill pipelines ──
        //
        // A path cannot be expressed as a shape per draw: even-odd and non-zero winding both need a
        // per-pixel count, so the fill is three passes over the same fan — write the count into the
        // stencil buffer, cover where the count says "inside", then reset the count for the next
        // path. Only the cover pass writes colour.
        let cover_stencil =
            |pass_op: wgpu::StencilOperation, compare: wgpu::CompareFunction, write_mask: u32| {
                wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Stencil8,
                    depth_write_enabled: None,
                    depth_compare: None,
                    stencil: wgpu::StencilState {
                        front: wgpu::StencilFaceState {
                            compare,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op,
                        },
                        back: wgpu::StencilFaceState {
                            compare,
                            fail_op: wgpu::StencilOperation::Keep,
                            depth_fail_op: wgpu::StencilOperation::Keep,
                            pass_op,
                        },
                        read_mask: 0xFF,
                        write_mask,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }
            };
        let path_fan_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("path_fan_pipeline_layout"),
            bind_group_layouts: &[Some(&transformed_bind_group_layout)],
            immediate_size: 0,
        });
        let path_cover_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("path_cover_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(wgpu::BufferSize::new(32).unwrap()),
                    },
                    count: None,
                }],
            });
        let path_cover_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("path_cover_pipeline_layout"),
            bind_group_layouts: &[
                Some(&path_cover_bind_group_layout),
                Some(&transformed_bind_group_layout),
            ],
            immediate_size: 0,
        });
        // Two `f32` per fan vertex; the fan's triangle list is built on the CPU.
        let path_fan_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let make_path_stencil_pipeline =
            |label: &str,
             stencil: wgpu::DepthStencilState,
             front_pass: wgpu::StencilOperation,
             back_pass: wgpu::StencilOperation| {
                let mut stencil = stencil;
                stencil.stencil.front.pass_op = front_pass;
                stencil.stencil.back.pass_op = back_pass;
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&path_fan_layout),
                    vertex: wgpu::VertexState {
                        module: &path_fan_vs_module,
                        entry_point: Some("vs_main"),
                        buffers: &[Some(path_fan_vertex_layout.clone())],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &path_stencil_fs_module,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: Some(wgpu::BlendState::REPLACE),
                            // The stencil pass only counts; it must not touch the colour target.
                            write_mask: wgpu::ColorWrites::empty(),
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
                    depth_stencil: Some(stencil),
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                    cache: None,
                })
            };
        let path_stencil_evenodd_pipeline = make_path_stencil_pipeline(
            "path_stencil_evenodd_pipeline",
            cover_stencil(wgpu::StencilOperation::Invert, wgpu::CompareFunction::Always, 0xFF),
            wgpu::StencilOperation::Invert,
            wgpu::StencilOperation::Invert,
        );
        let path_stencil_nonzero_pipeline = make_path_stencil_pipeline(
            "path_stencil_nonzero_pipeline",
            cover_stencil(
                wgpu::StencilOperation::IncrementWrap,
                wgpu::CompareFunction::Always,
                0xFF,
            ),
            wgpu::StencilOperation::IncrementWrap,
            wgpu::StencilOperation::DecrementWrap,
        );
        let path_stencil_clear_pipeline = make_path_stencil_pipeline(
            "path_stencil_clear_pipeline",
            cover_stencil(wgpu::StencilOperation::Zero, wgpu::CompareFunction::Always, 0xFF),
            wgpu::StencilOperation::Zero,
            wgpu::StencilOperation::Zero,
        );
        let path_cover_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("path_cover_pipeline"),
            layout: Some(&path_cover_layout),
            vertex: wgpu::VertexState {
                module: &path_cover_vs_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &path_cover_fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            // Keep only where the winding count is non-zero, and leave the count alone.
            depth_stencil: Some(cover_stencil(
                wgpu::StencilOperation::Keep,
                wgpu::CompareFunction::NotEqual,
                0,
            )),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

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
        shader_cache.insert("shadow_blur_fs".to_string(), shadow_blur_fs_module);
        shader_cache.insert("blend_composite_fs".to_string(), blend_composite_fs_module);
        shader_cache.insert("rect_vs_transformed".to_string(), transformed_vs_module);
        shader_cache.insert("rect_fs_instanced".to_string(), instanced_fs_module);
        shader_cache.insert("circle_vs".to_string(), circle_vs_module);
        shader_cache.insert("circle_fs".to_string(), circle_fs_module);
        shader_cache.insert("image_vs".to_string(), image_vs_module);
        shader_cache.insert("image_fs".to_string(), image_fs_module);
        shader_cache.insert("text_vs".to_string(), text_vs_module);
        shader_cache.insert("text_fs".to_string(), text_fs_module);
        shader_cache.insert("shape_vs".to_string(), shape_vs_module);
        shader_cache.insert("shape_fs".to_string(), shape_fs_module);
        shader_cache.insert("radial_vs".to_string(), radial_vs_module);
        shader_cache.insert("radial_fs".to_string(), radial_fs_module);
        shader_cache.insert("gradient_vs".to_string(), gradient_vs_module);
        shader_cache.insert("gradient_fs".to_string(), gradient_fs_module);
        shader_cache.insert("image_scaled_fs".to_string(), image_scaled_fs_module);
        shader_cache.insert("path_fan_vs".to_string(), path_fan_vs_module);
        shader_cache.insert("path_stencil_fs".to_string(), path_stencil_fs_module);
        shader_cache.insert("path_cover_vs".to_string(), path_cover_vs_module);
        shader_cache.insert("path_cover_fs".to_string(), path_cover_fs_module);
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
            shadow_blur_pipeline,
            shadow_body_pipeline,
            blend_composite_pipeline,
            blend_bind_group_layout,
            blend_modes_on_gpu: false,
            quality: RenderQuality::default(),
            cost_gate: CostGate::new(),
            circle_pipeline,
            circle_bind_group_layout,
            image_pipeline,
            image_scaled_pipeline,
            image_bind_group_layout,
            text_pipeline,
            text_layer_pipeline,
            text_bind_group_layout,
            shape_pipeline,
            shape_bind_group_layout,
            radial_pipeline,
            radial_bind_group_layout,
            gradient_pipeline,
            gradient_verbatim_pipeline,
            gradient_bind_group_layout,
            path_stencil_evenodd_pipeline,
            path_stencil_nonzero_pipeline,
            path_cover_pipeline,
            path_stencil_clear_pipeline,
            path_cover_bind_group_layout,
            bind_group_layout,
            blur_bind_group_layout,
            transformed_pipeline,
            transformed_bind_group_layout,
            #[cfg(feature = "3d")]
            mesh_pipeline,
            shader_cache,
            cache: RefCell::new(RenderCache::default()),
            texture_cache: RefCell::new(TextureCache::default()),
            text_coverage_cache: RefCell::new(TextCoverageCache::default()),
            params_scratch: RefCell::new(ParamsScratch::default()),
            params_groups: RefCell::new(None::<ParamsGroups>),
            params_group_cache: RefCell::new(ParamsGroupCache::default()),
            clear_frame: CpuFrame::new(),
            commands_frame: CpuFrame::new(),
            gpu: gpu_manager(&adapter_info)?,
            adapter_info,
        })
    }
}
