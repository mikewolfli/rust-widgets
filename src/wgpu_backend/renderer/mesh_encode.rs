// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `3d` encoder: the mesh pass.
//!
//! Split out of `encode.rs` so the 2D command-stream encoder stays inside the source
//! structure limit (principle #9). The seam is the feature boundary: everything here is
//! compiled only with `3d`, and the 2D path never calls into it.

use super::encode::mvp_column_major_bytes;
use super::frame::WgpuRenderer;
use super::resources::{rgba_buffer_len, RenderCache};

impl WgpuRenderer {
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
}
