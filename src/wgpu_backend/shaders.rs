//! Embedded WGSL shader sources for the WGPU renderer.

/// Full-screen quad vertex shader.
/// Draws a single triangle covering the entire NDC space [-1, 1].
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
pub(crate) const FILL_RECT_VS: &str = r#"
struct RectInput {
    @location(0) pos: vec2f,
};

@vertex
fn vs_main(input: RectInput) -> @builtin(position) vec4f {
    return vec4f(input.pos, 0.0, 1.0);
}"#;

/// Rectangle fill fragment shader.
/// Takes per-instance color from a uniform buffer.
pub(crate) const FILL_RECT_FS: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Four corners of a unit rectangle centered at origin in NDC.
/// Used as the vertex buffer for rectangle rendering.
#[allow(dead_code)]
pub(crate) const UNIT_RECT_VERTICES: &[f32] = &[
    // First triangle
    -0.5, -0.5, // bottom-left
    0.5, -0.5, // bottom-right
    -0.5, 0.5, // top-left
    // Second triangle
    0.5, -0.5, // bottom-right
    0.5, 0.5, // top-right
    -0.5, 0.5, // top-left
];

/// Clear-color fragment shader.
/// Fills the render target with a uniform solid color.
pub(crate) const CLEAR_FS: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Texture-copy fragment shader.
/// Copies a source texture to the render target.
#[allow(dead_code)]
pub(crate) const TEXTURE_COPY_FS: &str = r#"
@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(@builtin(position) coord: vec4f) -> @location(0) vec4f {
    let uv = vec2f(coord.x / 800.0, coord.y / 600.0);
    return textureSample(src_texture, src_sampler, uv);
}"#;
