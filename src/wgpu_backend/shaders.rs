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

/// Clear-color fragment shader.
/// Fills the render target with a uniform solid color.
pub(crate) const CLEAR_FS: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Image fragment shader.
/// Samples a texture and blends with uniform color.
/// Expects UV coordinates at @location(0) from the vertex shader.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderModule {
    FillRect,
    DrawImage,
    DrawText,
    FillRoundedRect,
    FillCircle,
}

impl ShaderModule {
    /// Get the WGSL shader source for this module.
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
