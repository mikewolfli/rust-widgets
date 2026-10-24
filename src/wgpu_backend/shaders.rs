// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Embedded WGSL shader sources for the WGPU renderer.

/// Full-screen quad vertex shader.
/// Draws a single triangle covering the entire NDC space [-1, 1].
///
/// Used by the `clear_pipeline`, with no vertex buffer and no bind groups
/// other than the colour uniform. It generates positions from
/// `@builtin(vertex_index)` alone, so it must be drawn with exactly
/// `0..3` vertices; any other count either fails validation or leaves part of
/// the viewport uncovered. The triangle's corners are `(-1, -1)`, `(3, -1)` and
/// `(-1, 3)`, so it overhangs the viewport on two sides and the rasterizer
/// clips the excess.
///
/// Output is in **clip space** (`[-1, 1]` on both axes, `+1` at the top of the
/// viewport), with the fragment colour supplied by [`CLEAR_FS`].
///
/// Note that the two pipelines in the renderer disagree about winding order:
/// this shader's triangle is counter-clockwise on screen while `clear_pipeline`
/// sets `front_face: Ccw` with back-face culling, and `rect_pipeline` sets
/// `front_face: Cw` for the quads built by `FILL_RECT_VS`.
///
/// The matching fragment shader is [`CLEAR_FS`].
pub(crate) const FULLSCREEN_QUAD_VS: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    // Full-screen triangle covering NDC: (-1,-1) to (3,1)
    // This draws a single large triangle that covers the entire viewport
    let positions = array(
        vec2f(-1.0, -1.0),
        vec2f(3.0, -1.0),
        vec2f(-1.0, 3.0),
    );
    return vec4f(positions[vertex_index], 0.0, 1.0);
}"#;

/// Rectangle vertex shader.
/// Takes per-vertex positions from a vertex buffer.
///
/// Used by the `rect_pipeline`. It reads one `vec2f` per vertex at shader
/// location 0, which the pipeline binds as `Float32x2` at offset `0` with
/// `array_stride == 8` and `step_mode: Vertex`. The stride must be exactly
/// `size_of::<[f32; 2]>()`; a mismatch is a pipeline validation error rather
/// than a silent misread.
///
/// Positions are expected already in **clip space** (`[-1, 1]` on both axes,
/// `+1` at the top of the viewport): the shader passes them through unchanged
/// and performs no transform, so the caller is responsible for converting
/// pixel coordinates. Nothing is read from a bind group here; the colour comes
/// from the fragment stage.
///
/// The matching fragment shader is `FILL_RECT_FS`.
pub(crate) const FILL_RECT_VS: &str = r#"
struct RectInput {
    @location(0) pos: vec2f,
};

@vertex
fn vs_main(input: RectInput) -> @builtin(position) vec4f {
    return vec4f(input.pos, 0.0, 1.0);
}"#;

/// Instanced, matrix-transformed quad vertex shader (`3d` feature only).
///
/// One **instance** carries everything a quad needs, and the six vertices of its two triangles
/// are generated from `@builtin(vertex_index)`; there is no per-vertex buffer. Corners are in
/// **logical pixel space** rather than clip space, and the shader multiplies the selected corner
/// by a model-view-projection matrix:
///
/// ```text
/// position = uMVP * vec4f(corner, 0.0, 1.0)
/// ```
///
/// Per-instance vertex buffer (`step_mode: Instance`, `array_stride == 48`):
///
/// | location | format | offset | meaning |
/// |---|---|---|---|
/// | 0 | `Float32x4` | 0 | corners 0 and 1, `(x0, y0, x1, y1)` |
/// | 1 | `Float32x4` | 16 | corners 2 and 3, `(x2, y2, x3, y3)` |
/// | 2 | `Float32x4` | 32 | straight RGBA colour, fed to [`FILL_RECT_INSTANCED_FS`] |
///
/// Corners are in perimeter order, so the generated triangles are `(0, 1, 2)` and `(0, 2, 3)`.
/// The pipeline must draw exactly `0..6` vertices per instance; `vertex_index % 6` keeps a
/// larger draw inside the six-entry corner table rather than reading out of bounds.
///
/// Bind group layout:
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `uniform mat4x4f` | `uMVP` | vertex | pixel space to clip space |
///
/// `uMVP` is one matrix per call, not a per-instance dynamic offset, so every quad in a drawn
/// run is transformed by the same matrix and differs only in its corners and colour. The
/// instance buffer carries no UV or clip data because this pipeline consumes neither; the
/// 48-byte stride is therefore exactly two corner vectors plus one colour vector.
///
/// This shader does **not** introduce a depth buffer: the transform is 2D-affine composed with
/// an orthographic pixel-to-clip matrix, and control-level occlusion stays with the scene
/// layers' stable z ordering. Because instances are drawn in buffer order within one draw, the
/// painter order of the recorded run is preserved.
#[cfg(feature = "3d")]
pub(crate) const FILL_RECT_VS_TRANSFORMED: &str = r#"
struct InstanceInput {
    @location(0) corners_lo: vec4f,
    @location(1) corners_hi: vec4f,
    @location(2) color: vec4f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
};

@group(0) @binding(0) var<uniform> uMVP: mat4x4f;

@vertex
fn vs_main(instance: InstanceInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let corner_order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let corner = corner_order[vertex_index % 6u];
    let p0 = instance.corners_lo.xy;
    let p1 = instance.corners_lo.zw;
    let p2 = instance.corners_hi.xy;
    let p3 = instance.corners_hi.zw;
    var local = p0;
    if corner == 1u {
        local = p1;
    } else if corner == 2u {
        local = p2;
    } else if corner == 3u {
        local = p3;
    }
    var output: VertexOutput;
    output.position = uMVP * vec4f(local, 0.0, 1.0);
    output.color = instance.color;
    return output;
}"#;

/// Fragment shader for the instanced `3d` pipeline.
///
/// The colour is a per-instance vertex output ([`FILL_RECT_VS_TRANSFORMED`], location 0). Every
/// vertex of a quad repeats the same value, so interpolation returns that colour unchanged and
/// the fragment needs no uniform and no bind group.
#[cfg(feature = "3d")]
pub(crate) const FILL_RECT_INSTANCED_FS: &str = r#"
@fragment
fn fs_main(@location(0) color: vec4f) -> @location(0) vec4f {
    return color;
}"#;

/// Rectangle fill fragment shader.
/// Takes per-instance color from a uniform buffer.
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | straight (non-premultiplied) RGBA, each channel in `0.0..=1.0` |
///
/// The uniform's `min_binding_size` is 16 bytes, matching `vec4f`; the binding
/// is fragment-visible only and has no dynamic offset. The colour is returned
/// as-is, so the pipeline's blend state decides the compositing: both built-in
/// pipelines use `BlendState::REPLACE`, i.e. the fragment replaces the target
/// rather than being blended over it.
///
/// The matching vertex shader is `FILL_RECT_VS`.
pub(crate) const FILL_RECT_FS: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Clear-color fragment shader.
/// Fills the render target with a uniform solid color.
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | straight (non-premultiplied) RGBA, each channel in `0.0..=1.0` |
///
/// Byte-for-byte the same shader as `FILL_RECT_FS` and, like it, shared with
/// one 16-byte `min_binding_size` uniform and the `REPLACE` blend state. The
/// two are kept as separate sources so the clear and rect pipelines can diverge
/// later without changing either one's identity in the shader cache.
///
/// The matching vertex shader is `FULLSCREEN_QUAD_VS`.
pub(crate) const CLEAR_FS: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Image fragment shader.
/// Samples a texture and blends with uniform color.
/// Expects UV coordinates at @location(0) from the vertex shader.
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | tint / base colour, straight RGBA in `0.0..=1.0` |
/// | 1 | `texture_2d<f32>` | `image_tex` | the image being drawn |
/// | 2 | `sampler` | `image_sampler` | filtering and addressing for `image_tex` |
///
/// All three bindings are required and must be declared in this order; a bind
/// group that omits binding 2, or a pipeline layout without it, fails wgpu
/// validation at draw time.
///
/// UVs are expected in the texture's `0.0..=1.0` range, with the origin at the
/// image's top-left. A source larger than the destination scales down under
/// whatever filter `image_sampler` provides.
///
/// The RGB output is a per-channel `mix` towards the texture weighted by the
/// texture's alpha, and the output alpha is the product `color.a * tex_color.a`.
/// The comment calls this "alpha-premultiplied", but the arithmetic is not: the
/// RGB result is not scaled by the output alpha and the texture's RGB is used
/// raw, so a texture carrying straight alpha produces an over-bright edge
/// rather than a premultiplied one. Treat it as a coverage-weighted lerp with
/// a straight-alpha texture until the shader is fixed.
///
/// This module is exposed through [`ShaderModule::DrawImage`].
pub(crate) const IMAGE_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@group(0) @binding(1) var image_tex: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let tex_color = textureSample(image_tex, image_sampler, uv);
    // Alpha-premultiplied blend of uniform color and texture
    return vec4f(mix(color.rgb, tex_color.rgb, tex_color.a), color.a * tex_color.a);
}"#;

/// Text fragment shader.
/// Renders glyphs using an SDF texture with smoothstep anti-aliasing.
/// Expects UV coordinates at @location(0) from the vertex shader.
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | glyph colour, straight RGBA in `0.0..=1.0` |
/// | 1 | `texture_2d<f32>` | `glyph_tex` | signed-distance field or coverage atlas for the glyphs |
/// | 2 | `sampler` | `glyph_sampler` | filtering and addressing for `glyph_tex` |
///
/// All three bindings are required. Only the texture's **red** channel is read,
/// which is a single-channel coverage/SDF atlas rather than a colour image; the
/// shader detects the glyph edge by thresholding that channel on `0.4..=0.6`,
/// so a plain 1-bit bitmap mask produces hard edges and a smooth distance field
/// produces anti-aliased ones.
///
/// UVs are in the texture's `0.0..=1.0` range, origin at the top-left. The
/// output RGB is the uniform colour unchanged and the output alpha is
/// `color.a * smoothstep(0.4, 0.6, glyph_alpha)`, so the glyph coverage
/// modulates opacity only.
///
/// This module is exposed through [`ShaderModule::DrawText`].
pub(crate) const TEXT_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;
@group(0) @binding(1) var glyph_tex: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let glyph_alpha = textureSample(glyph_tex, glyph_sampler, uv).r;
    // Smoothstep for anti-aliased SDF edge rendering
    let alpha = smoothstep(0.4, 0.6, glyph_alpha);
    return vec4f(color.rgb, color.a * alpha);
}"#;

/// Rounded rectangle fill fragment shader.
/// Uses signed-distance-field (SDF) rendering for smooth rounded corners.
/// Expects position coordinates at @location(0).
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | fill colour, straight RGBA in `0.0..=1.0` |
/// | 1 | `uniform vec4f` | `params` | `x = radius`, `y = width`, `z = height`, `w = unused` |
///
/// Both bindings are required and both are 16 bytes.
///
/// `@location(0)` must carry the fragment's position **within the shape's own
/// local space**, with the origin at the shape's top-left corner — not clip
/// space, and not framebuffer pixels. Offsets `params.yz` therefore describe
/// the shape's size in the same units as that input. The shape's centre is
/// taken to be `half_size` with no translation term, so an implementation that
/// feeds absolute coordinates will place the rounded rect at the origin.
///
/// The corner falloff is a one-unit-wide `smoothstep`, so anti-aliasing spans
/// one unit rather than one pixel; the caller must therefore choose the unit
/// scale. `radius` is not clamped here — the caller is responsible for keeping
/// it at or below half the shorter side, since a larger value produces a
/// distorted shape rather than a validation error.
///
/// This module is exposed through [`ShaderModule::FillRoundedRect`].
pub(crate) const ROUNDED_RECT_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;
@group(0) @binding(1) var<uniform> params: vec4f;  // x=radius, y=width, z=height, w=unused

@fragment
fn fs_main(@location(0) pos: vec2f) -> @location(0) vec4f {
    let half_size = params.yz * 0.5;
    let center = half_size;
    let p = abs(pos - center) - half_size + vec2f(params.x);
    let dist = length(max(p, vec2f(0.0))) - params.x;
    let alpha = 1.0 - smoothstep(0.0, 1.0, dist);
    return vec4f(color.rgb, color.a * alpha);
}"#;

/// Circle fill fragment shader.
/// Uses signed-distance-field (SDF) rendering for smooth circle edges.
/// Expects position coordinates at @location(0).
///
/// Bind group layout (group 0):
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform vec4f` | `color` | fill colour, straight RGBA in `0.0..=1.0` |
/// | 1 | `uniform vec4f` | `params` | `x = radius`, `y = center.x`, `z = center.y`, `w = unused` |
///
/// Both bindings are required and both are 16 bytes. Note that, despite the
/// apparent symmetry with `ROUNDED_RECT_FRAG`, `params.x` is a radius here
/// while in the rounded-rect shader it is a corner radius applied to a box, and
/// the centre arrives as an explicit `y`/`z` pair rather than being derived
/// from a size.
///
/// `@location(0)` must carry the fragment's position in the **same coordinate
/// space as `params.y`/`params.z`**. Only the distance to the centre is used,
/// and the one-unit-wide `smoothstep` from `0.0` to `1.0` anti-aliases the
/// edge, so that space is the caller's choice (pixels and normalized units both
/// work) as long as the centre and radius use it consistently.
///
/// This module is exposed through [`ShaderModule::FillCircle`].
pub(crate) const CIRCLE_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;
@group(0) @binding(1) var<uniform> params: vec4f;  // x=radius, y=center.x, z=center.y, w=unused

@fragment
fn fs_main(@location(0) pos: vec2f) -> @location(0) vec4f {
    let center = vec2f(params.y, params.z);
    let dist = distance(pos, center) - params.x;
    let alpha = 1.0 - smoothstep(0.0, 1.0, dist);
    return vec4f(color.rgb, color.a * alpha);
}"#;

/// Shader module identifiers for the WgpuRenderer pipeline (BLUE11 R5.2).
///
/// Each variant names a fragment shader and is the key callers use to fetch its
/// WGSL source with [`ShaderModule::source`]. The variants cover every fragment
/// shader exposed through this API; the vertex shaders are not addressable
/// through this enum, because the pipelines bind them directly from
/// `FULLSCREEN_QUAD_VS`, `FILL_RECT_VS` and (with the `3d` feature)
/// `FILL_RECT_VS_TRANSFORMED`. The instanced `3d` fragment shader
/// `FILL_RECT_INSTANCED_FS` is likewise internal: its input is the per-instance
/// colour, so it is paired with a specific vertex layout rather than offered as a
/// standalone module.
///
/// These identifiers are also the drift point between the two halves of the
/// backend: `WgpuRenderer::shader_modules` enumerates these variants, while the
/// pipelines it actually builds use the raw constants, so a variant can be
/// reachable here without any pipeline consuming it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderModule {
    /// `FILL_RECT_FS`: a solid rectangle's colour from a uniform.
    FillRect,
    /// `IMAGE_FRAG`: a sampled image tinted by a uniform colour; needs a
    /// texture and a sampler as well as the colour uniform.
    DrawImage,
    /// `TEXT_FRAG`: an SDF or coverage glyph atlas, sampled on its red
    /// channel and anti-aliased by a threshold.
    DrawText,
    /// `ROUNDED_RECT_FRAG`: a rounded box SDF over a colour and a
    /// `radius`/`width`/`height` parameter vector.
    FillRoundedRect,
    /// `CIRCLE_FRAG`: a circle SDF over a colour and a
    /// `radius`/`centre` parameter vector.
    FillCircle,
}

impl ShaderModule {
    /// Returns the WGSL source of this module's fragment shader as a `&'static
    /// str`.
    ///
    /// The returned text is a complete WGSL module whose fragment entry point is
    /// always named `fs_main` and which always takes its uniforms from bind
    /// group 0. It contains no vertex entry point, so a shader module built from
    /// it must be paired with a vertex module when creating a pipeline.
    ///
    /// For [`ShaderModule::DrawImage`], [`ShaderModule::DrawText`],
    /// [`ShaderModule::FillRoundedRect`] and [`ShaderModule::FillCircle`] the
    /// module declares more than one binding, and the caller must supply a bind
    /// group layout that matches the tables on each constant — the text and
    /// image samplers expect external textures that this backend's CPU
    /// rasterizer never creates.
    pub fn source(&self) -> &'static str {
        match self {
            ShaderModule::FillRect => FILL_RECT_FS,
            ShaderModule::DrawImage => IMAGE_FRAG,
            ShaderModule::DrawText => TEXT_FRAG,
            ShaderModule::FillRoundedRect => ROUNDED_RECT_FRAG,
            ShaderModule::FillCircle => CIRCLE_FRAG,
        }
    }
}
