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

/// Fragment shader for the instanced quad pipeline.
///
/// The colour is a per-instance vertex output ([`FILL_RECT_VS_TRANSFORMED`], location 0). Every
/// vertex of a quad repeats the same value, so interpolation returns that colour unchanged and
/// the fragment needs no uniform and no bind group.
pub(crate) const FILL_RECT_INSTANCED_FS: &str = r#"
@fragment
fn fs_main(@location(0) color: vec4f) -> @location(0) vec4f {
    return color;
}"#;

/// Vertex shader for the depth-tested `3d` mesh pipeline.
///
/// One vertex buffer carries the mesh, interleaved at a **32-byte stride**:
///
/// | location | format | offset | meaning |
/// |---|---|---|---|
/// | 0 | `Float32x4` | 0 | model-space position `(x, y, z, 1)` |
/// | 1 | `Float32x4` | 16 | straight RGBA colour, `0.0..=1.0` |
///
/// The vertex stage multiplies the position by the model-view-projection matrix the caller
/// composed. That matrix already includes the viewport placement and the `-1..=1` to `0..=1` depth
/// remap that [`crate::render::mesh3d::viewport_matrix`] performs, because WebGPU's depth
/// attachment compares in `0..=1` while [`crate::render::math3d::Mat4::perspective`] emits the
/// conventional `-1..=1` range.
///
/// Bind group layout:
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `uniform mat4x4f` | `uMVP` | vertex | model space to clip space |
///
/// The matching fragment shader is [`MESH3D_FS`], and the pipeline that uses them owns a
/// `Depth32Float` attachment cleared to `1.0` and compared with `Less`, so a nearer fragment wins
/// regardless of the order the triangles arrive in.
#[cfg(feature = "3d")]
pub(crate) const MESH3D_VS: &str = r#"
struct MeshInput {
    @location(0) position: vec4f,
    @location(1) color: vec4f,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
};

@group(0) @binding(0) var<uniform> uMVP: mat4x4f;

@vertex
fn vs_main(input: MeshInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uMVP * input.position;
    output.color = input.color;
    return output;
}"#;

/// Fragment shader for [`MESH3D_VS`]: the interpolated per-vertex colour.
///
/// The software rasteriser flat-shades each triangle with its mean vertex colour. A cube's faces
/// use one colour per face, so the two paths agree on such a mesh; a mesh with a colour gradient
/// across a face is drawn smoothly on the GPU and flat in software, which is the documented limit of
/// the software path rather than a depth difference.
#[cfg(feature = "3d")]
pub(crate) const MESH3D_FS: &str = r#"
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

/// Full-screen triangle vertex shader for the separable blur pass.
///
/// The three vertices cover the whole target, so the blur needs no vertex buffer and no
/// per-quad data: one draw call filters the entire texture. The fragment stage derives its texel
/// from `@builtin(position)` instead of an interpolated UV, which keeps the shader independent of
/// the target dimensions (it reads them from `textureDimensions`).
pub(crate) const BLUR_VS: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    var corners = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(corners[vertex_index], 0.0, 1.0);
}"#;

/// One direction of the separable Gaussian blur, with an optional tint composite.
///
/// # Bindings
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `texture_2d<f32>` | `source` | fragment | the input frame, read with `textureLoad` |
/// | 0 | 1 | uniform | `params` | fragment | direction, radius, and the optional tint |
///
/// The kernel is a Gaussian with `sigma = radius / 2`, the same 2-sigma convention
/// [`crate::render::RenderCommand::Blur`] documents. `textureLoad` is used rather than a sampler
/// because the pass needs exact texel addresses at the frame's own resolution and no filtering;
/// the texture's `sample_type` is therefore `float, filterable = false`.
///
/// The two passes (horizontal then vertical) make the cost `O(radius)` per pixel rather than
/// `O(radius²)`, which is the whole reason for separating the axes. Edge taps clamp to the
/// nearest border texel, matching the software box blur's edge rule.
///
/// # Tint (G-220)
///
/// `apply_tint` selects whether this pass composites `tint_color` over the filtered result.
/// Only the **second** (vertical) pass sets it, so the tint rides the pass the frame is
/// already flowing through instead of a read-back-to-CPU tinting step: the composite is the
/// straight-alpha over operator the former CPU tint step performed, with the same
/// `weight`-scaled alpha and the same tiny-alpha flush to transparent. When `apply_tint`
/// is zero the shader returns the filtered colour unchanged, so a plain blur is unaffected.
pub(crate) const BLUR_FS: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;

struct BlurParams {
    direction: vec2f,
    radius: f32,
    apply_tint: f32,
    tint_color: vec3f,
    tint_alpha: f32,
};

@group(0) @binding(1) var<uniform> params: BlurParams;

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let dimensions = vec2<i32>(textureDimensions(source));
    let center = vec2<i32>(position.xy);
    let radius = i32(params.radius);
    var color: vec4f;
    if (radius <= 0) {
        color = textureLoad(source, center, 0);
    } else {
        let sigma = max(params.radius * 0.5, 0.5);
        let two_sigma_squared = 2.0 * sigma * sigma;
        var sum = vec4f(0.0);
        var weight_sum = 0.0;
        for (var offset = -radius; offset <= radius; offset = offset + 1) {
            let delta = vec2<i32>(vec2f(f32(offset)) * params.direction);
            let sample_at = clamp(center + delta, vec2<i32>(0), dimensions - vec2<i32>(1));
            let weight = exp(-f32(offset * offset) / two_sigma_squared);
            sum = sum + textureLoad(source, sample_at, 0) * weight;
            weight_sum = weight_sum + weight;
        }
        color = sum / weight_sum;
    }
    if (params.apply_tint <= 0.0) {
        return color;
    }
    let one_minus = 1.0 - params.tint_alpha;
    let out_alpha = params.tint_alpha + color.a * one_minus;
    if (out_alpha <= 1.1920929e-7) {
        return vec4f(0.0);
    }
    let rgb = (params.tint_color * params.tint_alpha + color.rgb * color.a * one_minus) / out_alpha;
    return vec4f(rgb, out_alpha);
}"#;

/// Vertex shader for the SDF circle pipeline (G-219).
///
/// One **draw** covers one circle: the vertex buffer carries the six vertices of the circle's
/// bounding quad as **pixel-space** positions (`vec2f`, 8-byte stride, `Float32x2` at location
/// 0), and this shader does two things with each one:
///
/// 1. Transforms it to clip space through the group-1 `uMVP` matrix (the same pixel-to-clip
///    matrix every other 2D pipeline in this renderer uses, so `x` maps `0..width` to `-1..1`
///    and `y` maps `0..height` to `1..-1`).
/// 2. Passes the raw pixel position through to [`CIRCLE_FRAG`] at location 0, whose
///    interpolation across the quad is exactly the fragment's pixel-space position.
///
/// Bind group layout:
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 1 | 0 | `uniform mat4x4f` | `uMVP` | vertex | pixel space to clip space |
///
/// Group 0 is left to [`CIRCLE_FRAG`]'s colour and SDF parameter uniforms. The matching fragment
/// shader is [`CIRCLE_FRAG`].
pub(crate) const CIRCLE_VS: &str = r#"
struct CircleVertexInput {
    @location(0) pixel_pos: vec2f,
};

struct CircleVertexOutput {
    @builtin(position) position: vec4f,
    @location(0) pos: vec2f,
};

@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

@vertex
fn vs_main(input: CircleVertexInput) -> CircleVertexOutput {
    var output: CircleVertexOutput;
    output.position = uMVP * vec4f(input.pixel_pos, 0.0, 1.0);
    output.pos = input.pixel_pos;
    return output;
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
///
/// # Zero-coverage flush (G-219)
///
/// A fragment whose coverage reaches zero must return a fully transparent **black**: the
/// pipeline writes with `REPLACE` blending, so returning the fill colour with zero alpha
/// would still paint its RGB over pixels the software disc never touches. Zero-coverage
/// fragments occur across the whole bounding quad outside the disc, and the quad is the only
/// geometry submitted, so the flush is what keeps "the disc draws, the quad does not" true.
pub(crate) const CIRCLE_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;
@group(0) @binding(1) var<uniform> params: vec4f;  // x=radius, y=center.x, z=center.y, w=unused

@fragment
fn fs_main(@location(0) pos: vec2f) -> @location(0) vec4f {
    let center = vec2f(params.y, params.z);
    let dist = distance(pos, center) - params.x;
    let coverage = 1.0 - smoothstep(0.0, 1.0, dist);
    if (coverage <= 0.0) {
        return vec4f(0.0);
    }
    return vec4f(color.rgb, color.a * coverage);
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
///
/// # Current pipeline coverage (G-213, G-219)
///
/// [`ShaderModule::FillRect`] and [`ShaderModule::FillCircle`] are consumed by pipelines:
/// the single-rectangle path builds `rect_fs` from [`FILL_RECT_FS`], and the SDF circle
/// pipeline (G-219) builds `circle_fs` from [`CIRCLE_FRAG`]. The image, text and rounded-rect
/// programs are reserved — their WGSL and bind-group tables are complete, but this backend's
/// CPU rasteriser draws those commands, so no pipeline is created for them.
/// `ShaderModule::pipeline_cache_key` records exactly that split and the
/// `shader_module_registry_matches_pipelines` test fails if a `Some(key)` entry does not
/// correspond to a shader the renderer actually registers.
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

    /// The `WgpuRenderer::shader_cache` key of the pipeline that consumes this module, or `None`
    /// when the module is reserved and no pipeline is built from it (G-213).
    ///
    /// The match is exhaustive on purpose: adding a variant forces a decision about whether the
    /// new shader is wired into a pipeline instead of silently advertising an unused module.
    ///
    /// Only the `shader_module_registry_matches_pipelines` test reads this today; it lives beside
    /// the enum so the mapping is part of the enum's contract rather than the test's copy.
    #[allow(dead_code)]
    pub(crate) const fn pipeline_cache_key(self) -> Option<&'static str> {
        match self {
            ShaderModule::FillRect => Some("rect_fs"),
            ShaderModule::FillCircle => Some("circle_fs"),
            ShaderModule::DrawImage | ShaderModule::DrawText | ShaderModule::FillRoundedRect => {
                None
            }
        }
    }
}
