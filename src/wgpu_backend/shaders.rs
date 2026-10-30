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

/// One direction of the shadow layer's **box** blur, matching the CPU rasteriser's kernel exactly.
///
/// # Bindings
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `texture_2d<f32>` | `source` | fragment | the shadow layer being filtered |
/// | 0 | 1 | uniform | `params` | fragment | direction and radius |
///
/// The 32-byte uniform is the same shape [`blur_bind_group_layout`] already declares —
/// `direction: vec2f`, `radius: f32`, one unused `f32` — so the shadow passes reuse the backdrop
/// blur's layout and vertex shader ([`BLUR_VS`]) and differ only in this fragment stage.
///
/// # Why a box and not the backdrop blur's Gaussian
///
/// [`super::super::raster`]'s `box_blur_layer_rgba8` filters the shadow layer with a separable
/// moving average whose window is **clamped to the layer** (`lo = x - r`, `hi = x + r`, both
/// clamped to the layer's own extent) and divided by that clamped tap count. This shader performs
/// the same integer arithmetic — eight-bit values recovered with `round(texel * 255)`, accumulated
/// as integers, divided with truncation — so the two paths agree instead of merely looking alike.
/// A Gaussian would feather the same shadow differently, which is why the shadow passes cannot
/// reuse [`BLUR_FS`].
///
/// The layer is allocated at exactly the CPU's layer extent, so "clamped to the texture" and
/// "clamped to the layer" are the same bounds, and no separate bound uniform is needed.
pub(crate) const SHADOW_BLUR_FS: &str = r#"
struct ShadowBlurParams {
    direction: vec2f,
    radius: f32,
    unused: f32,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: ShadowBlurParams;

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let dimensions = vec2<i32>(textureDimensions(source));
    let center = vec2<i32>(position.xy);
    let radius = i32(params.radius);
    if (radius <= 0) {
        return textureLoad(source, center, 0);
    }
    // The window is clamped once, per axis, exactly as the CPU's sliding window is: a window that
    // stops at the layer edge averages fewer taps rather than repeating the border texel.
    let vertical = params.direction.y > 0.5;
    var start = center;
    var span = 0;
    if (vertical) {
        start = vec2<i32>(center.x, max(center.y - radius, 0));
        span = min(center.y + radius, dimensions.y - 1) - start.y;
    } else {
        start = vec2<i32>(max(center.x - radius, 0), center.y);
        span = min(center.x + radius, dimensions.x - 1) - start.x;
    }
    let step = vec2<i32>(vec2f(1.0 - params.direction.y, params.direction.y));
    var sum = vec4<i32>(0);
    for (var tap = 0; tap <= span; tap = tap + 1) {
        let texel = textureLoad(source, start + step * tap, 0);
        sum = sum + vec4<i32>(round(texel * 255.0));
    }
    let count = span + 1;
    return vec4f(vec4<f32>(sum / vec4<i32>(count))) / 255.0;
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

/// Image vertex shader: one axis-aligned quad per image draw, positioned from the draw's own
/// uniform — there is no vertex buffer.
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `uniform ImageParams` | `uParams` | vertex | `dest` = (x, y, w, h) in framebuffer pixels |
/// | 1 | 0 | `uniform mat4x4f` | `uMVP` | vertex | pixel space to clip space |
///
/// Group 1 is the shared matrix every other 2D pipeline in this renderer uses, so the quad covers
/// exactly the destination rectangle. The fragment half is [`IMAGE_FRAG`].
pub(crate) const IMAGE_VS: &str = r#"
struct ImageParams {
    dest: vec4f,
    source: vec4f,
};

@group(0) @binding(0) var<uniform> uParams: ImageParams;
@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) pixel: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let d = uParams.dest;
    var corners = array<vec2f, 6>(
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y + d.w),
    );
    let pixel = corners[vertex_index];
    var output: VertexOutput;
    output.position = uMVP * vec4f(pixel, 0.0, 1.0);
    output.pixel = pixel;
    return output;
}"#;

/// Image fragment shader: one texel fetch per fragment, with the **integer** source mapping the
/// software rasteriser uses.
///
/// # Bind group layout (group 0)
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform ImageParams` | `uParams` | `dest` = (x, y, w, h) in pixels; `source` = (w, h) |
/// | 1 | `texture_2d<f32>` | `uSource` | the source image, in its own grid |
///
/// # Why `textureLoad` and not `textureSample`
///
/// The software rasteriser computes `src = local * source_dim / dest_dim` in integers, so a scaled
/// draw picks exactly one texel per destination pixel and applies no filter. A sampler would let
/// the driver choose linear filtering and round the boundary pixels differently; `textureLoad` at
/// an integer coordinate reproduces the crate's rule, which is what makes a GPU frame comparable
/// to a software one byte for byte instead of "close enough".
///
/// Fragments are shaded at pixel centres (`@builtin(position)` is `x + 0.5`), so `floor` of the
/// interpolated pixel-space position is the destination pixel index — the same `local` the CPU
/// loop uses. The output is straight RGBA; source-over compositing is the pipeline's blend state,
/// not this shader's job.
///
/// This module is exposed through [`ShaderModule::DrawImage`].
pub(crate) const IMAGE_FRAG: &str = r#"
struct ImageParams {
    dest: vec4f,
    source: vec4f,
};

@group(0) @binding(0) var<uniform> uParams: ImageParams;
@group(0) @binding(1) var uSource: texture_2d<f32>;

@fragment
fn fs_main(@location(0) pixel: vec2f) -> @location(0) vec4f {
    let d = uParams.dest;
    let s = uParams.source.xy;
    let local = floor(pixel) - d.xy;
    let last = max(s - vec2f(1.0), vec2f(0.0));
    let src = clamp(floor(local * s / d.zw), vec2f(0.0), last);
    return textureLoad(uSource, vec2i(i32(src.x), i32(src.y)), 0);
}"#;

/// Text vertex shader: one axis-aligned quad per text box, positioned from the box's uniform —
/// there is no vertex buffer.
///
/// | group | binding | type | name | stage | meaning |
/// |---|---|---|---|---|---|
/// | 0 | 0 | `uniform TextParams` | `uParams` | vertex | `dest` = (x, y, w, h) in framebuffer pixels |
/// | 1 | 0 | `uniform mat4x4f` | `uMVP` | vertex | pixel space to clip space |
///
/// The fragment half is [`TEXT_FRAG`].
pub(crate) const TEXT_VS: &str = r#"
struct TextParams {
    dest: vec4f,
    color: vec4f,
};

@group(0) @binding(0) var<uniform> uParams: TextParams;
@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) pixel: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let d = uParams.dest;
    var corners = array<vec2f, 6>(
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y + d.w),
    );
    let pixel = corners[vertex_index];
    var output: VertexOutput;
    output.position = uMVP * vec4f(pixel, 0.0, 1.0);
    output.pixel = pixel;
    return output;
}"#;

/// Text fragment shader: one coverage fetch per destination pixel, multiplied by the text colour.
///
/// # Bind group layout (group 0)
///
/// | binding | type | name | meaning |
/// |---|---|---|---|
/// | 0 | `uniform TextParams` | `uParams` | `dest` = (x, y, w, h) in pixels; `color` = straight RGBA |
/// | 1 | `texture_2d<f32>` | `uCoverage` | one coverage byte per pixel of the text box (`R8Unorm`) |
///
/// # Why coverage is uploaded rather than sampled from an atlas
///
/// The CPU rasteriser paints glyphs through the font stack into an 8x8 coverage cell and then
/// composites each pixel. Uploading the box's coverage keeps the glyph painting on exactly that
/// code — so a glyph has the same shape on both paths — and moves only the per-pixel colour
/// multiply and the source-over composite to the GPU. There is no sampler: the fetch is at the
/// destination pixel's own coordinate, so no filtering can blur a stem.
///
/// The output is straight alpha (`color.rgb`, `color.a * coverage`); compositing is the pipeline's
/// blend state, which is why this shader has no blend arithmetic of its own.
///
/// This module is exposed through [`ShaderModule::DrawText`].
pub(crate) const TEXT_FRAG: &str = r#"
struct TextParams {
    dest: vec4f,
    color: vec4f,
};

@group(0) @binding(0) var<uniform> uParams: TextParams;
@group(0) @binding(1) var uCoverage: texture_2d<f32>;

@fragment
fn fs_main(@location(0) pixel: vec2f) -> @location(0) vec4f {
    let d = uParams.dest;
    let local = vec2i(i32(floor(pixel.x) - d.x), i32(floor(pixel.y) - d.y));
    let last = vec2i(max(i32(d.z) - 1, 0), max(i32(d.w) - 1, 0));
    let coord = clamp(local, vec2i(0, 0), last);
    let coverage = textureLoad(uCoverage, coord, 0).r;
    return vec4f(uParams.color.rgb, uParams.color.a * coverage);
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

/// Fragment shader that composites a blend run over the backdrop it was drawn against.
///
/// # Why this exists
///
/// WebGPU has no framebuffer fetch and fixed-function blending cannot use the destination as an
/// operand, so a mode such as `Multiply` cannot be expressed as a blend state. The renderer instead
///
/// 1. copies the frame into a backdrop texture,
/// 2. draws the command into a transparent offscreen layer, and
/// 3. composites `f(source, backdrop)` here.
///
/// # Bindings
///
/// | group | binding | type | name | meaning |
/// |---|---|---|---|---|
/// | 0 | 0 | `uniform BlendParams` | `params` | `mode` tag, and whether the layer holds premultiplied values |
/// | 0 | 1 | `texture_2d<f32>` | `uSource` | the run's layer, read with `textureLoad` |
/// | 0 | 2 | `texture_2d<f32>` | `uBackdrop` | the frame as it was before the run |
///
/// # What it reproduces
///
/// [`super::super::raster`]'s `blend_channels`, on the straight (un-premultiplied) values the CPU
/// compares, with the result's alpha taken from the source — the CPU's own rule. Both textures are
/// `Rgba8Unorm`, so `textureLoad` returns the exact `u8 / 255` the CPU's `blend_channels` divides by.
///
/// A layer pixel with zero alpha is a pixel the run did not paint, so the backdrop is returned
/// unchanged: the pipeline writes with `REPLACE`, and rewriting the backdrop's own value is the one
/// way to leave it alone without a stencil test.
///
/// Three families reach a blend layer (see [`super::renderer::dispatch::draws_under_blend`]): the
/// rectangle class, whose `REPLACE` pipeline writes straight colour; images and gradients, whose
/// source-over pipeline stores premultiplied values; and text, whose coverage rides in the alpha of
/// a straight-coloured layer. `layer_kind` tells this shader which one it is looking at, and the
/// backdrop's own alpha is divided out too, because a translucent frame stores premultiplied pixels
/// as well.
pub(crate) const BLEND_COMPOSITE_FS: &str = r#"
struct BlendParams {
    mode: u32,
    // How the layer was written: 0 straight, 1 premultiplied, 2 straight + coverage in alpha.
    layer_kind: u32,
    pad: vec2<u32>,
};

@group(0) @binding(0) var<uniform> params: BlendParams;
@group(0) @binding(1) var uSource: texture_2d<f32>;
@group(0) @binding(2) var uBackdrop: texture_2d<f32>;

const EPSILON: f32 = 1.1920929e-7;

fn luminance(c: vec3f) -> f32 {
    return 0.3 * c.x + 0.59 * c.y + 0.11 * c.z;
}

fn saturation(c: vec3f) -> f32 {
    return max(max(c.x, c.y), c.z) - min(min(c.x, c.y), c.z);
}

fn set_luminosity(c: vec3f, goal: f32) -> vec3f {
    let current = luminance(c);
    let spread = max(max(c.x, c.y), c.z) - min(min(c.x, c.y), c.z);
    if (abs(spread) < EPSILON) {
        return vec3f(goal);
    }
    return c + vec3f(goal - current);
}

fn clip_color(c: vec3f, goal: f32) -> vec3f {
    let low = min(min(c.x, c.y), c.z);
    let high = max(max(c.x, c.y), c.z);
    if (low < 0.0) {
        let denom = goal - low;
        if (abs(denom) < EPSILON) {
            return vec3f(goal);
        }
        return vec3f(goal) + (c - vec3f(goal)) * (goal / denom);
    }
    if (high > 1.0) {
        let denom = high - goal;
        if (abs(denom) < EPSILON) {
            return vec3f(goal);
        }
        return vec3f(goal) + (c - vec3f(goal)) * ((1.0 - goal) / denom);
    }
    return c;
}

fn set_saturation(c: vec3f, goal: f32) -> vec3f {
    let low = min(min(c.x, c.y), c.z);
    let high = max(max(c.x, c.y), c.z);
    if (high <= low) {
        return c;
    }
    let current = saturation(c);
    if (abs(current) < EPSILON) {
        return c;
    }
    let scale = goal / current;
    let mid = luminance(c);
    return vec3f(mid) + (c - vec3f(mid)) * scale;
}

fn separable(mode: u32, s: f32, d: f32) -> f32 {
    switch (mode) {
        case 1u: { return s * d; }
        case 2u: { return s + d - s * d; }
        case 3u: {
            if (d <= 0.5) { return 2.0 * s * d; }
            return 1.0 - 2.0 * (1.0 - s) * (1.0 - d);
        }
        case 4u: { return min(s, d); }
        case 5u: { return max(s, d); }
        case 6u: {
            if (d >= 1.0) { return 1.0; }
            return min(1.0, s / (1.0 - d));
        }
        case 7u: {
            if (d <= 0.0) { return 0.0; }
            return 1.0 - min(1.0, (1.0 - s) / d);
        }
        case 8u: {
            if (s <= 0.5) { return 2.0 * s * d; }
            return 1.0 - 2.0 * (1.0 - s) * (1.0 - d);
        }
        case 9u: {
            if (d <= 0.25) {
                let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                return d - (1.0 - 2.0 * s) * delta;
            }
            if (d <= 0.5) {
                let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                return d + (2.0 * s - 1.0) * (delta - d);
            }
            return d + (2.0 * s - 1.0) * (sqrt(d) - d);
        }
        case 10u: { return abs(s - d); }
        case 11u: { return s + d - 2.0 * s * d; }
        default: { return s; }
    }
}

fn blend_rgb(mode: u32, s: vec3f, d: vec3f) -> vec3f {
    let goal = luminance(d);
    if (mode == 12u) {
        return set_luminosity(clip_color(s, goal), goal);
    }
    if (mode == 13u) {
        return set_luminosity(set_saturation(d, saturation(s)), goal);
    }
    if (mode == 14u) {
        return set_luminosity(s, goal);
    }
    if (mode == 15u) {
        return set_luminosity(d, luminance(s));
    }
    return vec3f(
        separable(mode, s.x, d.x),
        separable(mode, s.y, d.y),
        separable(mode, s.z, d.z),
    );
}

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let coord = vec2<i32>(floor(position.xy));
    let layer = textureLoad(uSource, coord, 0);
    let backdrop = textureLoad(uBackdrop, coord, 0);
    if (layer.a <= 0.0) {
        return backdrop;
    }
    var dest = backdrop;
    if (backdrop.a > 0.0) {
        dest = vec4f(backdrop.rgb / backdrop.a, backdrop.a);
    }
    // Text: the layer holds the glyph colour as straight RGB and its coverage as alpha, so the
    // mode is applied to the colour first and the result composited by that coverage — the same
    // order `blend_cpu_rgba8_with_mode` uses.
    if (params.layer_kind == 2u) {
        let blended = blend_rgb(params.mode, layer.rgb, dest.rgb);
        let coverage = layer.a;
        return vec4f(
            blended * coverage + dest.rgb * (1.0 - coverage),
            dest.a + coverage * (1.0 - dest.a),
        );
    }
    var source = layer;
    if (params.layer_kind == 1u) {
        source = vec4f(layer.rgb / layer.a, layer.a);
    }
    return vec4f(blend_rgb(params.mode, source.rgb, dest.rgb), source.a);
}"#;

/// Shape vertex shader for the ring/wedge/outline family: one quad per draw, positioned from the
/// draw's own `rect` uniform — there is no vertex buffer.
///
/// The fragment half is [`SHAPE_FRAG`]; `rect` is the shape's bounding box for circles and arcs and
/// the outlined rectangle for a rounded stroke.
pub(crate) const SHAPE_VS: &str = r#"
struct ShapeParams {
    center_radius: vec4f,
    window: vec4f,
    color: vec4f,
    rect: vec4f,
};

@group(0) @binding(0) var<uniform> p: ShapeParams;
@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) pixel: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let d = p.rect;
    var corners = array<vec2f, 6>(
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y + d.w),
    );
    let pixel = corners[vertex_index];
    var output: VertexOutput;
    output.position = uMVP * vec4f(pixel, 0.0, 1.0);
    output.pixel = pixel;
    return output;
}"#;

/// Shape fragment shader: the ring / pie-wedge / rounded-outline family, one SDF per mode.
///
/// # Modes (`window.z`)
///
/// | mode | shape | inside | uniform use |
/// |---|---|---|---|
/// | 0 | circle stroke | `\|d − r\| ≤ w` | `center_radius` = (cx, cy, r, w) |
/// | 1 | filled arc (pie wedge) | `d ≤ r` within the angular window | `center_radius`, `window.xy` |
/// | 2 | stroked arc | `\|d − r\| ≤ 1` within the window | `center_radius`, `window.xy` |
/// | 3 | rounded-rectangle outline | `−t ≤ sdf ≤ 0` | `rect`, `window.w` = corner radius, `half_width` = t |
///
/// # Why the coverage carries the anti-aliasing
///
/// Each mode reduces to "signed distance to the shape's boundary, negative inside". The fragment
/// returns the colour with `1 − smoothstep(0, 1, distance)` as alpha, so the pipeline's source-over
/// blend feathers the edge over one pixel. The software rasteriser fills these shapes with integer
/// predicates (no anti-aliasing), so the two agree on *which shape* is drawn but not on the edge
/// pixel's exact value — the same, already documented, difference the SDF circle fill has.
///
/// The angular test is hard-edged: an arc's cut ends are exact, so a wedge's straight sides are not
/// feathered. That matches how the software path treats them and keeps the sweep endpoints testable.
///
/// This module is exposed through [`ShaderModule::Shape`].
pub(crate) const SHAPE_FRAG: &str = r#"
struct ShapeParams {
    center_radius: vec4f,
    window: vec4f,
    color: vec4f,
    rect: vec4f,
};

@group(0) @binding(0) var<uniform> p: ShapeParams;

const TAU: f32 = 6.283185307179586;

fn rounded_box_sdf(q: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let d = abs(q) - half_size + vec2f(radius);
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

@fragment
fn fs_main(@location(0) pixel: vec2f) -> @location(0) vec4f {
    let mode = i32(round(p.window.z));
    let px = floor(pixel);
    let center = p.center_radius.xy;
    let radius = p.center_radius.z;
    let half_width = p.center_radius.w;
    var edge = 0.0;
    if (mode == 3) {
        let rect = p.rect;
        let box_center = rect.xy + rect.zw * 0.5;
        let sdf = rounded_box_sdf(px - box_center, rect.zw * 0.5, p.window.w);
        // Outside the outer shape (sdf > 0) and inside the hole (sdf < -t) are both "outside".
        edge = max(sdf, -half_width - sdf);
    } else {
        let d = distance(px, center);
        if (mode == 0) {
            edge = abs(d - radius) - half_width;
        } else {
            let angle = atan2(px.y - center.y, px.x - center.x);
            let turned = select(angle + TAU, angle, angle >= 0.0);
            if (turned < p.window.x || turned > p.window.y) {
                return vec4f(0.0);
            }
            edge = select(abs(d - radius) - 1.0, d - radius, mode == 1);
        }
    }
    let coverage = 1.0 - smoothstep(0.0, 1.0, edge);
    if (coverage <= 0.0) {
        return vec4f(0.0);
    }
    return vec4f(p.color.rgb, p.color.a * coverage);
}"#;

/// Radial-gradient vertex shader: one quad per gradient, positioned from the draw's uniform.
pub(crate) const RADIAL_VS: &str = r#"
struct RadialParams {
    center_radius: vec4f,
    rect: vec4f,
    stops: vec4f,
};

@group(0) @binding(0) var<uniform> p: RadialParams;
@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) pixel: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let d = p.rect;
    var corners = array<vec2f, 6>(
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y + d.w),
    );
    let pixel = corners[vertex_index];
    var output: VertexOutput;
    output.position = uMVP * vec4f(pixel, 0.0, 1.0);
    output.pixel = pixel;
    return output;
}"#;

/// Radial-gradient fragment shader: distance to the centre, scaled to `0..=1`, then the **nearest**
/// stop — the same rule [`super::super::raster`]'s `gradient_stop` applies.
///
/// # Layout
///
/// | group | binding | type | name | meaning |
/// |---|---|---|---|---|
/// | 0 | 0 | `uniform RadialParams` | `p` | `center_radius` = (cx, cy, radius, stops), `rect`, `stops` = (count, 0, 0, 0) |
/// | 0 | 1 | `texture_2d<f32>` | `p.stops_tex` is bound separately | the stop table, one texel per stop |
/// | 1 | 0 | `uniform mat4x4f` | `uMVP` | pixel space to clip space |
///
/// # Why the index is computed here rather than sampled from a ramp
///
/// An axis-aligned gradient has one colour per row (or column), so the software rasteriser's ramp
/// can be uploaded and repeated. A radial gradient's colour changes with the **distance**, so the
/// only way to keep it one draw is to compute `t` and pick the stop in the shader. The arithmetic is
/// deliberately the same expression the CPU uses — `sqrt(dx² + dy²) * (1 / radius)` in `f32`, then
/// `floor((stops − 1) * t)` — so the two agree wherever the two `sqrt` implementations round alike;
/// the test measures that rather than assuming it.
pub(crate) const RADIAL_FRAG: &str = r#"
struct RadialParams {
    center_radius: vec4f,
    rect: vec4f,
    stops: vec4f,
};

@group(0) @binding(0) var<uniform> p: RadialParams;
@group(0) @binding(1) var uStops: texture_2d<f32>;

@fragment
fn fs_main(@location(0) pixel: vec2f) -> @location(0) vec4f {
    let px = floor(pixel);
    let d = px - p.center_radius.xy;
    let distance = sqrt(d.x * d.x + d.y * d.y);
    let t = clamp(distance * p.center_radius.w, 0.0, 1.0);
    let count = i32(p.stops.x);
    let last = max(count - 1, 0);
    let index = clamp(i32(floor(f32(last) * t)), 0, last);
    return textureLoad(uStops, vec2i(index, 0), 0);
}"#;

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
/// Path-fill vertex shader: fan triangles in pixel space, transformed by the shared mvp.
///
/// The geometry comes from a vertex buffer (two `f32` per point) because a fan has one vertex per
/// polygon point, not a fixed quad. The stencil passes write nothing to the colour target; the cover
/// pass that follows does the painting.
pub(crate) const PATH_FAN_VS: &str = r#"
@group(0) @binding(0) var<uniform> uMVP: mat4x4f;

@vertex
fn vs_main(@location(0) pos: vec2f) -> @builtin(position) vec4f {
    return uMVP * vec4f(pos, 0.0, 1.0);
}"#;

/// Path-fill fragment shader for the stencil passes: the colour target is masked off, so this only
/// has to exist for the pipeline to be valid.
pub(crate) const PATH_STENCIL_FS: &str = r#"
@fragment
fn fs_main() -> @location(0) vec4f {
    return vec4f(0.0);
}"#;

/// Path-cover vertex shader: one quad over the path's bounding box, from the draw's uniform.
pub(crate) const PATH_COVER_VS: &str = r#"
struct CoverParams {
    rect: vec4f,
    color: vec4f,
};

@group(0) @binding(0) var<uniform> p: CoverParams;
@group(1) @binding(0) var<uniform> uMVP: mat4x4f;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    let d = p.rect;
    var corners = array<vec2f, 6>(
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y),
        vec2f(d.x + d.z, d.y + d.w),
        vec2f(d.x, d.y + d.w),
    );
    return uMVP * vec4f(corners[vertex_index], 0.0, 1.0);
}"#;

/// Path-cover fragment shader: the fill colour. The stencil test on the pipeline is what decides
/// which pixels survive, so this shader has no geometry test of its own.
pub(crate) const PATH_COVER_FS: &str = r#"
struct CoverParams {
    rect: vec4f,
    color: vec4f,
};

@group(0) @binding(0) var<uniform> p: CoverParams;

@fragment
fn fs_main() -> @location(0) vec4f {
    return p.color;
}"#;

/// [`ShaderModule::FillRect`], [`ShaderModule::FillCircle`], [`ShaderModule::DrawImage`] and
/// [`ShaderModule::DrawText`] are consumed by pipelines: the single-rectangle path builds `rect_fs`
/// from [`FILL_RECT_FS`], the SDF circle pipeline (G-219) builds `circle_fs` from [`CIRCLE_FRAG`],
/// the image pipeline builds `image_fs` from [`IMAGE_FRAG`] (vertex [`IMAGE_VS`]), and the text
/// pipeline builds `text_fs` from [`TEXT_FRAG`] (vertex [`TEXT_VS`]).
/// The rounded-rect program remains reserved — its WGSL and bind-group table are complete, but
/// filled rounded rectangles are drawn as row-span instances and the outline still comes from the
/// CPU rasteriser, so no pipeline is created for it.
/// `ShaderModule::pipeline_cache_key` records exactly that split and the
/// `shader_module_registry_matches_pipelines` test fails if a `Some(key)` entry does not
/// correspond to a shader the renderer actually registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderModule {
    /// `FILL_RECT_FS`: a solid rectangle's colour from a uniform.
    FillRect,
    /// `IMAGE_FRAG`: one texel fetch per destination pixel, at the source coordinate the
    /// software rasteriser's integer rule selects; needs the draw's params uniform and the
    /// source texture as well.
    DrawImage,
    /// `TEXT_FRAG`: the text box's coverage texture, fetched at each destination pixel's own
    /// coordinate and multiplied by the text colour; needs the box's params uniform and the
    /// coverage texture as well.
    DrawText,
    /// `ROUNDED_RECT_FRAG`: a rounded box SDF over a colour and a
    /// `radius`/`width`/`height` parameter vector.
    FillRoundedRect,
    /// `CIRCLE_FRAG`: a circle SDF over a colour and a
    /// `radius`/`centre` parameter vector.
    FillCircle,
    /// `SHAPE_FRAG`: the ring / pie-wedge / rounded-outline family, one SDF selected by mode;
    /// needs the shape's params uniform only.
    Shape,
    /// `RADIAL_FRAG`: distance-to-centre lookup into the stop table; needs the gradient's params
    /// uniform and the stop texture.
    RadialGradient,
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
    /// For [`ShaderModule::DrawImage`], [`ShaderModule::DrawText`] and
    /// [`ShaderModule::FillRoundedRect`] the module declares more than one binding, and the
    /// caller must supply a bind group layout that matches the tables on each constant. The
    /// image module expects the source texture this backend uploads per draw; the text and
    /// rounded-rect modules expect inputs the CPU rasteriser still supplies on its own.
    pub fn source(&self) -> &'static str {
        match self {
            ShaderModule::FillRect => FILL_RECT_FS,
            ShaderModule::DrawImage => IMAGE_FRAG,
            ShaderModule::DrawText => TEXT_FRAG,
            ShaderModule::FillRoundedRect => ROUNDED_RECT_FRAG,
            ShaderModule::FillCircle => CIRCLE_FRAG,
            ShaderModule::Shape => SHAPE_FRAG,
            ShaderModule::RadialGradient => RADIAL_FRAG,
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
            ShaderModule::DrawImage => Some("image_fs"),
            ShaderModule::DrawText => Some("text_fs"),
            ShaderModule::Shape => Some("shape_fs"),
            ShaderModule::RadialGradient => Some("radial_fs"),
            ShaderModule::FillRoundedRect => None,
        }
    }
}
