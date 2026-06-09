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

/// Image fragment shader stub.
/// Placeholder for texture-based image rendering.
pub(crate) const IMAGE_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Text fragment shader stub.
/// Placeholder for glyph/sdf-based text rendering.
pub(crate) const TEXT_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Rounded rectangle fill fragment shader stub.
/// Placeholder for rounded-rect SDF rendering.
pub(crate) const ROUNDED_RECT_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
}"#;

/// Circle fill fragment shader stub.
/// Placeholder for circle SDF rendering.
pub(crate) const CIRCLE_FRAG: &str = r#"
@group(0) @binding(0) var<uniform> color: vec4f;

@fragment
fn fs_main() -> @location(0) vec4f {
    return color;
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
