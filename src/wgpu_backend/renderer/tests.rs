// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GPU renderer tests: they submit work and read the pixels back, skipping when no adapter is
//! available.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::rasterize_draw_commands_rgba8;
use super::super::types::{PixelRect, Rgba8};
use super::dispatch::commands_gpu_dispatchable;
use super::encode::{circle_quad, pack_instanced_quads, INSTANCE_BYTES, MAX_INSTANCES_PER_DRAW};
use super::frame::WgpuRenderer;
use super::resources::grown_buffer_capacity;

/// A renderer on this host, or `None` when no adapter can be reached.
///
/// The GPU tests are real: they submit work and read the pixels back. A host with no
/// adapter cannot answer them, so they report the skip rather than failing — the same
/// contract the platform code uses for an unavailable capability.
fn renderer() -> Option<WgpuRenderer> {
    match WgpuRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(error) => {
            eprintln!("skipping GPU test: no wgpu adapter available ({error})");
            None
        }
    }
}

fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    [pixels[start], pixels[start + 1], pixels[start + 2], pixels[start + 3]]
}

/// G-213: the enum's pipeline table and the shader cache the renderer registers must agree.
///
/// Every member that names a pipeline key must have that key in the cache, and the cache's
/// key set is asserted exactly, so adding a shader without wiring it into a pipeline — or
/// deleting one — fails here instead of drifting silently.
#[test]
fn shader_module_registry_matches_pipelines() {
    let Some(renderer) = renderer() else { return };

    let mut expected = vec![
        "clear_vs",
        "clear_fs",
        "rect_vs",
        "rect_fs",
        "blur_vs",
        "blur_fs",
        "shadow_blur_fs",
        "rect_vs_transformed",
        "rect_fs_instanced",
        "circle_vs",
        "circle_fs",
        "image_vs",
        "image_fs",
        "text_vs",
        "text_fs",
        "shape_vs",
        "shape_fs",
        "radial_vs",
        "radial_fs",
        "path_fan_vs",
        "path_stencil_fs",
        "path_cover_vs",
        "path_cover_fs",
    ];
    #[cfg(feature = "3d")]
    expected.extend(["mesh3d_vs", "mesh3d_fs"]);
    let mut registered: Vec<&str> = renderer.shader_cache.keys().map(String::as_str).collect();
    expected.sort_unstable();
    registered.sort_unstable();
    assert_eq!(
        registered, expected,
        "the registered shader cache drifted from the pipeline table; wire or remove the \
         entry and update the table in this test"
    );

    for module in renderer.shader_modules() {
        if let Some(key) = module.pipeline_cache_key() {
            assert!(
                renderer.get_shader_module(key).is_some(),
                "{module:?} advertises pipeline key {key:?}, but no such module is \
                 registered"
            );
        }
    }
}

/// Growth is geometric and block-rounded, so a frame that needs a few more bytes reuses the
/// existing buffer instead of recreating it at the exact size (G-09).
#[test]
fn buffer_growth_leaves_headroom_and_respects_the_limit() {
    assert_eq!(grown_buffer_capacity(0, 1, u64::MAX), 65_536);
    let first = grown_buffer_capacity(0, 100 * 1024, u64::MAX);
    assert!(first >= 100 * 1024);
    assert_eq!(first % 65_536, 0, "capacities are whole 64 KiB blocks");

    // A small increase must not force a new allocation.
    let grown = grown_buffer_capacity(first, first + 16, u64::MAX);
    assert!(grown > first, "growth is geometric, not exact-size");
    assert_eq!(grown, first + first / 2, "the next step is 1.5x before rounding");

    // The device limit caps the rounded capacity; the caller rejects a request that cannot
    // fit at all before calling this.
    assert_eq!(grown_buffer_capacity(0, 3, 4), 4);
}

/// The texture/view path renders without asking for pixels back (REND-02).
#[test]
fn texture_path_renders_a_view_without_readback() {
    let Some(renderer) = renderer() else { return };
    let rects = [(0, 0, 4, 4, [1.0, 0.0, 0.0, 1.0])];
    let seen = renderer
        .with_fill_rect_texture(4, 4, &rects, |_view| true)
        .expect("the texture path must render");
    assert!(seen, "the closure receives the rendered view");
}

/// A clear must fill every pixel with the requested colour.
#[test]
fn clear_writes_the_requested_colour_across_the_frame() {
    let Some(renderer) = renderer() else { return };
    let pixels =
        renderer.render_clear_gpu(4, 3, [1.0, 0.0, 0.0, 1.0]).expect("the clear must render");
    assert_eq!(pixels.len(), 4 * 3 * 4);
    for y in 0..3 {
        for x in 0..4 {
            assert_eq!(pixel(&pixels, 4, x, y), [255, 0, 0, 255]);
        }
    }
}

/// Two rectangles must each land in their own corner — and the second call through the
/// cached buffers must produce exactly the same picture as the first (REND-02's reuse path).
#[test]
fn fill_rect_reuses_its_buffers_without_changing_the_picture() {
    let Some(renderer) = renderer() else { return };
    let rects = [(0, 0, 4, 4, [1.0, 0.0, 0.0, 1.0]), (4, 4, 4, 4, [0.0, 0.0, 1.0, 1.0])];
    let first = renderer.render_fill_rect_gpu(8, 8, &rects).expect("first fill");
    let second = renderer.render_fill_rect_gpu(8, 8, &rects).expect("second fill");
    assert_eq!(first, second, "cached resources must reproduce the same frame");
    assert_eq!(pixel(&first, 8, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&first, 8, 3, 3), [255, 0, 0, 255]);
    assert_eq!(pixel(&first, 8, 4, 4), [0, 0, 255, 255]);
    assert_eq!(pixel(&first, 8, 7, 7), [0, 0, 255, 255]);
    assert_eq!(pixel(&first, 8, 7, 0), [0, 0, 0, 0]);
}

/// A larger run must still place every rectangle's colour on its own pixels.
///
/// Three rectangles now take the instanced batched path (G-64), so this is the guard that the
/// batching change did not collapse or reorder their colours.
#[test]
fn a_larger_run_renders_every_colour() {
    let Some(renderer) = renderer() else { return };
    let one = [(0, 0, 4, 2, [1.0, 0.0, 0.0, 1.0])];
    renderer.render_fill_rect_gpu(4, 2, &one).expect("one rect");
    let three = [
        (0, 0, 1, 1, [1.0, 0.0, 0.0, 1.0]),
        (1, 0, 1, 1, [0.0, 1.0, 0.0, 1.0]),
        (2, 0, 1, 1, [0.0, 0.0, 1.0, 1.0]),
    ];
    let pixels = renderer.render_fill_rect_gpu(4, 2, &three).expect("three rects");
    assert_eq!(pixel(&pixels, 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 4, 1, 0), [0, 255, 0, 255]);
    assert_eq!(pixel(&pixels, 4, 2, 0), [0, 0, 255, 255]);
}

/// An upload must round-trip the exact bytes it was given.
#[test]
fn upload_round_trips_the_given_pixels() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..(4 * 4 * 4) as u8).collect();
    let output =
        renderer.upload_rgba8_and_readback(4, 4, &input).expect("the upload must round-trip");
    assert_eq!(output, input);
    // A second call reuses the cached upload texture and readback buffer.
    assert_eq!(renderer.upload_rgba8_and_readback(4, 4, &input).expect("second upload"), input);
}

/// The reusable CPU frame must match the owned raster byte for byte, keep its allocation
/// across calls, and be uploadable without an owned copy (G-221).
#[test]
fn reusable_cpu_frame_matches_the_owned_raster_and_is_reused() {
    let Some(renderer) = renderer() else { return };
    let commands = vec![
        WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 2, y: 2, width: 6, height: 6 },
            color: opaque(255, 0, 0),
            clip: None,
        },
    ];
    let owned = rasterize_draw_commands_rgba8(16, 16, &commands).expect("owned raster");
    let first =
        renderer.render_draw_commands_cached_rgba8(16, 16, &commands).expect("cached raster");
    assert_eq!(&*first, &owned[..], "the reusable frame must be byte-identical");
    // The upload runs while the frame is still borrowed: the two caches are independent.
    let uploaded =
        renderer.upload_rgba8_and_readback(16, 16, &first).expect("upload the borrowed frame");
    assert_eq!(uploaded, owned, "upload must see the reusable frame's bytes");
    let address = first.as_ptr();
    drop(first);
    let second =
        renderer.render_draw_commands_cached_rgba8(16, 16, &commands).expect("cached again");
    assert_eq!(second.as_ptr(), address, "the frame allocation must be reused");
    drop(second);
    // A smaller frame must not inherit the previous frame's bytes.
    let small = vec![WgpuDrawCommand::FillRect {
        rect: PixelRect { x: 0, y: 0, width: 1, height: 1 },
        color: opaque(0, 255, 0),
        clip: None,
    }];
    let smaller = renderer.render_draw_commands_cached_rgba8(8, 8, &small).expect("smaller frame");
    assert_eq!(pixel(&smaller, 8, 7, 7), [0, 0, 0, 0], "the reused prefix must be cleared");
    assert_eq!(pixel(&smaller, 8, 0, 0), [0, 255, 0, 255]);
}

/// A clear through the reusable path must match the owned clear exactly.
#[test]
fn reusable_clear_frame_matches_the_owned_clear() {
    let Some(renderer) = renderer() else { return };
    let owned = renderer.render_clear_rgba8(4, 3, [0.25, 0.5, 0.75, 1.0]).expect("owned clear");
    let cached =
        renderer.render_clear_cached_rgba8(4, 3, [0.25, 0.5, 0.75, 1.0]).expect("cached clear");
    assert_eq!(&*cached, &owned[..]);
}

#[cfg(feature = "3d")]
#[test]
fn fill_quads_empty_is_transparent() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_fill_quads_gpu(8, 8, &crate::render::math3d::Mat3::IDENTITY, &[])
        .expect("an empty quad batch must produce a frame");
    assert_eq!(pixels.len(), 8 * 8 * 4);
    assert!(pixels.iter().all(|&byte| byte == 0));
}

/// A rectangle rotated by 45 degrees must land on the same pixels through the matrix
/// pipeline as it does through the software polygon fill.
///
/// The comparison deliberately samples the interior and the hidden corners rather than the
/// edge pixels: both rasterisers are hard-edged but use different fill rules, so exact
/// agreement on a sloped boundary is not something either path promises.
#[cfg(feature = "3d")]
#[test]
fn a_45_degree_rotated_rect_agrees_with_the_software_path() {
    use crate::core::{Color, Point};
    use crate::render::math3d::Mat3;
    use crate::render::SoftwareSurface;

    let Some(renderer) = renderer() else { return };
    let affine = Mat3::translation(-16.0, -16.0)
        .then(&Mat3::rotation(core::f32::consts::FRAC_PI_4))
        .then(&Mat3::translation(16.0, 16.0));
    let local = [Point::new(8, 8), Point::new(24, 8), Point::new(24, 24), Point::new(8, 24)];

    let gpu = renderer
        .render_fill_quads_gpu(32, 32, &affine, &[(local, [1.0, 0.0, 0.0, 1.0])])
        .expect("the matrix pipeline must render");

    let mapped = local.map(|point| affine.map_point(point));
    let mut surface = SoftwareSurface::new(crate::core::Size::new(32, 32), 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_path(&mapped[..], true, Color::RED, true, 0);
    surface.end_frame();
    let software = surface.frame_rgba();

    for (x, y, what) in [
        (16, 16, "centre"),
        (10, 16, "left interior"),
        (22, 16, "right interior"),
        (16, 10, "top interior"),
        (16, 22, "bottom interior"),
    ] {
        assert_eq!(
            pixel(software, 32, x, y),
            pixel(&gpu, 32, x, y),
            "{what} must agree between the software and matrix paths"
        );
    }
    for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
        assert_eq!(
            pixel(software, 32, x, y),
            pixel(&gpu, 32, x, y),
            "a corner outside the rotated square must remain transparent in both paths"
        );
    }
}

/// Builds an axis-aligned quad in perimeter order, matching the corner order the instanced
/// shader expands into two triangles.
#[cfg(feature = "3d")]
fn quad_rect(x: i32, y: i32, x1: i32, y1: i32) -> [crate::core::Point; 4] {
    [
        crate::core::Point::new(x, y),
        crate::core::Point::new(x1, y),
        crate::core::Point::new(x1, y1),
        crate::core::Point::new(x, y1),
    ]
}

/// Converts a `Color` to the `0.0..=1.0` tuple the GPU entry points take.
#[cfg(feature = "3d")]
fn gpu_colour(color: crate::core::Color) -> [f64; 4] {
    [color.r as f64 / 255.0, color.g as f64 / 255.0, color.b as f64 / 255.0, color.a as f64 / 255.0]
}

/// An instanced run of axis-aligned quads must produce exactly the software picture at the
/// sampled pixels, including overlapping quads, whose later instances paint over earlier
/// ones.
///
/// The samples are interiors rather than edges: the software scanline fill and the GPU both
/// cover a shape's interior identically, but their boundary conventions differ, so an edge
/// pixel is not a fair pixel-for-pixel comparison for either path.
#[cfg(feature = "3d")]
#[test]
fn instanced_run_matches_the_software_reference() {
    use crate::core::{Color, Point};
    use crate::render::math3d::Mat3;
    use crate::render::SoftwareSurface;

    let Some(renderer) = renderer() else { return };
    let run = [
        (quad_rect(2, 2, 10, 10), Color::RED),
        (quad_rect(8, 2, 16, 10), Color::GREEN),
        (quad_rect(2, 8, 10, 16), Color::BLUE),
        (quad_rect(8, 8, 16, 16), Color::YELLOW),
    ];
    let quads: Vec<([Point; 4], [f64; 4])> =
        run.iter().map(|(corners, colour)| (*corners, gpu_colour(*colour))).collect();

    let gpu = renderer
        .render_fill_quads_gpu(18, 18, &Mat3::IDENTITY, &quads)
        .expect("the instanced run must render");

    let mut surface = SoftwareSurface::new(crate::core::Size::new(18, 18), 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    for (corners, colour) in run {
        surface.draw_path(&corners, true, colour, true, 0);
    }
    surface.end_frame();
    let software = surface.frame_rgba();

    // One sample per quad, one per pair overlap, and one where all four overlap. The
    // expected colour is the last quad in `run` that covers the point, so this also proves
    // the instanced draw preserved painter order.
    for (x, y, expected) in [
        (4, 4, Color::RED),
        (12, 4, Color::GREEN),
        (4, 12, Color::BLUE),
        (12, 12, Color::YELLOW),
        (9, 4, Color::GREEN),
        (4, 9, Color::BLUE),
        (12, 9, Color::YELLOW),
        (9, 12, Color::YELLOW),
        (9, 9, Color::YELLOW),
    ] {
        assert_eq!(
            pixel(&gpu, 18, x, y),
            pixel(&software, 18, x, y),
            "pixel ({x}, {y}) must match the software reference"
        );
        assert_eq!(
            pixel(&gpu, 18, x, y),
            [expected.r, expected.g, expected.b, expected.a],
            "pixel ({x}, {y}) must show the last quad in the run that covers it"
        );
    }
}

/// An empty instanced run must produce a transparent frame, not stale pixels.
#[cfg(feature = "3d")]
#[test]
fn instanced_empty_run_is_transparent() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_fill_quads_gpu(8, 8, &crate::render::math3d::Mat3::IDENTITY, &[])
        .expect("an empty instanced run must produce a frame");
    assert_eq!(pixels.len(), 8 * 8 * 4);
    assert!(pixels.iter().all(|&byte| byte == 0));
}

/// A run longer than the per-draw cap must stream across chunks without reordering: the
/// lone instance in the second chunk must read its own record, not the first chunk's.
#[cfg(feature = "3d")]
#[test]
fn an_over_cap_instanced_run_streams_every_chunk_in_order() {
    let Some(renderer) = renderer() else { return };
    // Quads are 2x1 so no pixel centre lies on the shared triangle edge; 1024 of them fit
    // in one row, and the extra instance lands on the next row.
    let per_row = 1024u32;
    let total = MAX_INSTANCES_PER_DRAW + 1;
    let width = per_row * 2;
    let height = (total as u32).div_ceil(per_row);
    let mut quads = Vec::with_capacity(total);
    for index in 0..total {
        let x = ((index as u32 % per_row) * 2) as i32;
        let y = (index as u32 / per_row) as i32;
        let colour = if index + 1 == total { [0.0, 0.0, 1.0, 1.0] } else { [1.0, 0.0, 0.0, 1.0] };
        quads.push((quad_rect(x, y, x + 2, y + 1), colour));
    }
    let pixels = renderer
        .render_fill_quads_gpu(width, height, &crate::render::math3d::Mat3::IDENTITY, &quads)
        .expect("an over-cap run must render");

    let sample = |index: usize| -> [u8; 4] {
        let x = (index as u32 % per_row) * 2;
        let y = index as u32 / per_row;
        pixel(&pixels, width, x, y)
    };
    let last = total - 1;
    assert_eq!(sample(last), [0, 0, 255, 255], "the second chunk must draw its own quad");
    assert_eq!(sample(last - 1), [255, 0, 0, 255], "the first chunk's last quad must draw");
    assert_eq!(sample(0), [255, 0, 0, 255], "the first chunk's first quad must draw");
}

/// A run of 45-degree rotated quads must land on the same interior pixels through the
/// instanced matrix pipeline as it does through the software polygon fill.
///
/// The comparison samples each square's mapped centre and the frame corners rather than a
/// sloped boundary: the two rasterisers are hard-edged but use different fill rules, so
/// exact agreement on an edge is not something either path promises.
#[cfg(feature = "3d")]
#[test]
fn a_45_degree_instanced_run_agrees_with_the_software_path() {
    use crate::core::{Color, Point};
    use crate::render::math3d::Mat3;
    use crate::render::SoftwareSurface;

    let Some(renderer) = renderer() else { return };
    let affine = Mat3::translation(-16.0, -16.0)
        .then(&Mat3::rotation(core::f32::consts::FRAC_PI_4))
        .then(&Mat3::translation(16.0, 16.0));
    let run = [
        (quad_rect(6, 6, 14, 14), Color::RED),
        (quad_rect(18, 6, 26, 14), Color::GREEN),
        (quad_rect(6, 18, 14, 26), Color::BLUE),
    ];
    let quads: Vec<([Point; 4], [f64; 4])> =
        run.iter().map(|(corners, colour)| (*corners, gpu_colour(*colour))).collect();

    let gpu = renderer
        .render_fill_quads_gpu(32, 32, &affine, &quads)
        .expect("the instanced matrix pipeline must render");

    let mut surface = SoftwareSurface::new(crate::core::Size::new(32, 32), 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    for (corners, colour) in run {
        let mapped = corners.map(|point| affine.map_point(point));
        surface.draw_path(&mapped, true, colour, true, 0);
    }
    surface.end_frame();
    let software = surface.frame_rgba();

    for ((corners, _colour), what) in run.iter().zip(["top-left", "top-right", "bottom-left"]) {
        let centre = affine.map_point(Point::new(
            (corners[0].x + corners[2].x) / 2,
            (corners[0].y + corners[2].y) / 2,
        ));
        let (x, y) = (centre.x as u32, centre.y as u32);
        assert_eq!(
            pixel(software, 32, x, y),
            pixel(&gpu, 32, x, y),
            "the {what} square centre must agree between the software and instanced paths"
        );
    }
    for (x, y) in [(0, 0), (31, 0), (0, 31), (31, 31)] {
        assert_eq!(
            pixel(software, 32, x, y),
            pixel(&gpu, 32, x, y),
            "a corner outside the rotated run must remain transparent in both paths"
        );
    }
}

/// A zero radius is the identity, and it must not even touch the GPU.
#[test]
fn a_zero_radius_blur_returns_the_input() {
    let Some(renderer) = renderer() else { return };
    let input = vec![9u8; 4 * 4 * 4];
    let output = renderer.render_blur_rgba8(4, 4, &input, 0).expect("identity blur");
    assert_eq!(output, input);
}

/// A uniform field is a fixed point of a normalised blur kernel.
///
/// This is the property a separable GPU pass is most likely to break with a bad weight or an
/// edge clamp: a field of one colour must stay that colour, because every tap reads the same
/// value and the weights sum to one.
#[test]
fn a_uniform_field_survives_the_gpu_blur() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
    let output = renderer.render_blur_rgba8(8, 8, &input, 3).expect("the blur pass runs");
    for y in 0..8 {
        for x in 0..8 {
            let seen = pixel(&output, 8, x, y);
            for channel in 0..4 {
                assert!(
                    seen[channel].abs_diff(input[channel]) <= 1,
                    "a uniform field must stay uniform at ({x},{y}), got {seen:?}"
                );
            }
        }
    }
}

/// A blur moves colour across a hard edge and stays symmetric about it.
///
/// The left half is red and the right half blue. After the blur the pixels either side of the
/// seam must both contain some of each colour, and their values must mirror: the kernel is
/// symmetric, so sampling one pixel left of the seam and one right of it must produce the same
/// two-colour mixture.
#[test]
fn the_gpu_blur_spreads_an_edge_symmetrically() {
    let Some(renderer) = renderer() else { return };
    let width = 16u32;
    let height = 4u32;
    let mut input = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            if x < width / 2 {
                input[offset..offset + 4].copy_from_slice(&[255, 0, 0, 255]);
            } else {
                input[offset..offset + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    let output = renderer.render_blur_rgba8(width, height, &input, 4).expect("the blur runs");
    let left = pixel(&output, width, width / 2 - 1, 2);
    let right = pixel(&output, width, width / 2, 2);
    assert!(left[2] > 0, "blue must bleed left of the seam: {left:?}");
    assert!(right[0] > 0, "red must bleed right of the seam: {right:?}");
    // Symmetry about the seam swaps the two colours, so the red/blue channels mirror each
    // other rather than matching channel for channel.
    assert!(
        left[0].abs_diff(right[2]) <= 2 && left[2].abs_diff(right[0]) <= 2,
        "the two sides of a symmetric edge must mirror: {left:?} vs {right:?}"
    );
}

/// The depth-tested mesh path draws the near face of a cube and leaves the background clear.
///
/// This runs only on a host with an adapter; on a headless host it compiles and skips, which is
/// what keeps the pipeline type-checked without making the suite GPU-dependent.
#[cfg(feature = "3d")]
#[test]
fn the_gpu_mesh_pipeline_draws_a_cube_through_a_depth_attachment() {
    use crate::render::math3d::Mat4;
    use crate::render::mesh3d::{Camera, Mesh3D, Vec3};

    let Some(renderer) = renderer() else { return };
    let mesh = Mesh3D::cube(Vec3::ZERO, 2.0, [crate::core::Color::RED; 6]);
    let camera = Camera::new(Vec3::new(3.0, 2.0, 6.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
    let pixels = renderer
        .render_mesh_3d_gpu(
            64,
            64,
            &mesh,
            &Mat4::IDENTITY,
            &camera,
            crate::core::Rect::new(0, 0, 64, 64),
        )
        .expect("the mesh pipeline runs");
    assert_eq!(pixels.len(), 64 * 64 * 4);
    let painted = pixels.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    assert!(painted > 100, "the cube must cover part of the target, got {painted} pixels");
}

// ── G-219: SDF circle pipeline ──

/// The SDF circle must paint the disc's interior opaquely and take partial alpha on the
/// edge — including a pixel the old per-column strip approximation left untouched
/// (G-219's mandated difference).
#[test]
fn sdf_circle_paints_an_anti_aliased_edge_the_strips_did_not() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_fill_circle_gpu(32, 32, &[((16, 16, 8), [1.0, 0.0, 0.0, 1.0])])
        .expect("the SDF circle must render");
    // Centre: fully covered on both paths.
    assert_eq!(pixel(&pixels, 32, 16, 16), [255, 0, 0, 255]);
    // An interior pixel the strip path also painted at full colour.
    assert_eq!(pixel(&pixels, 32, 16, 12), [255, 0, 0, 255]);
    // (22, 10): distance 0.485 beyond the radius, so the SDF shades partial alpha. The
    // strip approximation painted nothing here (its column at x = 22 spans y 11..21).
    //
    // This frame has no clear, so the destination is transparent and the pipeline composites
    // source-over: the stored colour is the source premultiplied by its coverage. Over an opaque
    // background the same coverage lands as `(255, 0, 0, ...)` — that case is asserted by
    // `a_circle_drawn_over_content_keeps_the_content_around_it` below.
    let edge = pixel(&pixels, 32, 22, 10);
    assert!(
        edge[3] > 0 && edge[3] < 255,
        "the SDF edge must be anti-aliased (partial alpha), got {edge:?}"
    );
    assert_eq!(
        &edge[..3],
        &[edge[3], 0, 0],
        "the edge is the fill colour premultiplied by its coverage over a transparent frame: {edge:?}"
    );
    // A corner of the bounding quad outside the disc stays transparent.
    assert_eq!(pixel(&pixels, 32, 8, 8), [0, 0, 0, 0]);
    assert_eq!(pixel(&pixels, 32, 31, 31), [0, 0, 0, 0]);
}

/// A zero-radius circle draws nothing, matching the software rasteriser's skip — and a
/// frame of only zero-radius circles is a transparent frame.
#[test]
fn a_zero_radius_circle_draws_nothing() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_fill_circle_gpu(8, 8, &[((4, 4, 0), [1.0, 0.0, 0.0, 1.0])])
        .expect("a zero-radius circle must not error");
    assert!(pixels.iter().all(|&byte| byte == 0));
}

/// The circle quad must clamp to the viewport in `i64`, so extreme centres and radii
/// neither wrap nor panic; a disc covering no pixel reports `None`.
#[test]
fn circle_quad_clamps_to_the_viewport() {
    assert_eq!(circle_quad((16, 16), 8, 32, 32), Some((8, 8, 25, 25)));
    assert_eq!(circle_quad((0, 0), 2, 32, 32), Some((0, 0, 3, 3)));
    assert_eq!(circle_quad((-100, -100), 4, 32, 32), None);
    assert_eq!(circle_quad((40, 16), 8, 32, 32), None);
    // Extreme coordinates must not wrap: the arithmetic runs in `i64`.
    assert_eq!(circle_quad((i32::MAX, 0), 1, 32, 32), None);
    assert_eq!(circle_quad((i32::MIN, 0), u32::MAX, 32, 32), Some((0, 0, 32, 32)));
}

/// The instance packer must write the documented 48-byte layout: corners as little-endian
/// `f32`, then the clamped colour, appending to whatever the scratch buffer already holds.
#[test]
fn pack_instanced_quads_writes_the_documented_layout() {
    let quads = [
        (
            [
                crate::core::Point::new(1, 2),
                crate::core::Point::new(3, 2),
                crate::core::Point::new(3, 4),
                crate::core::Point::new(1, 4),
            ],
            [0.25, 0.5, 0.75, 1.0],
        ),
        (
            [
                crate::core::Point::new(5, 6),
                crate::core::Point::new(7, 6),
                crate::core::Point::new(7, 8),
                crate::core::Point::new(5, 8),
            ],
            [1.0, 0.0, 0.0, 0.5],
        ),
    ];
    let mut packed = Vec::new();
    pack_instanced_quads(&quads, &mut packed).expect("packs");
    assert_eq!(packed.len(), 2 * INSTANCE_BYTES);
    let f32_at = |record: usize, offset: usize| {
        f32::from_le_bytes(
            packed[record * INSTANCE_BYTES + offset..record * INSTANCE_BYTES + offset + 4]
                .try_into()
                .expect("four bytes"),
        )
    };
    assert_eq!(f32_at(0, 0), 1.0);
    assert_eq!(f32_at(0, 4), 2.0);
    assert_eq!(f32_at(0, 24), 1.0);
    assert_eq!(f32_at(0, 28), 4.0);
    assert_eq!(f32_at(0, 32), 0.25);
    assert_eq!(f32_at(0, 44), 1.0);
    assert_eq!(f32_at(1, 0), 5.0);
    assert_eq!(f32_at(1, 44), 0.5);
    // Packing again into the same buffer clears rather than appends.
    pack_instanced_quads(&quads[..1], &mut packed).expect("repacks");
    assert_eq!(packed.len(), INSTANCE_BYTES);
}

// ── G-220: tint rides the vertical blur pass ──

/// A full-weight opaque tint composites straight-alpha over the blurred field, entirely on
/// the GPU: a uniform field must come out at the tint colour (G-220).
#[test]
fn a_full_weight_tint_reaches_the_gpu_vertical_pass() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
    let output = renderer
        .render_backdrop_blur_rgba8(
            8,
            8,
            &input,
            2,
            Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 1.0)),
        )
        .expect("the tinted blur runs");
    for y in 0..8 {
        for x in 0..8 {
            let seen = pixel(&output, 8, x, y);
            assert!(
                seen[0].abs_diff(255) <= 2
                    && seen[1].abs_diff(0) <= 2
                    && seen[2].abs_diff(0) <= 2
                    && seen[3].abs_diff(255) <= 2,
                "a full-weight opaque tint must dominate at ({x},{y}), got {seen:?}"
            );
        }
    }
}

/// A half-weight tint must blend halfway between the blurred field and the tint. The
/// tolerance covers the GPU float pipeline's rounding, which differs from the CPU
/// composite by at most a couple of counts.
#[test]
fn a_half_weight_tint_blends_halfway() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
    let output = renderer
        .render_backdrop_blur_rgba8(
            8,
            8,
            &input,
            2,
            Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 0.5)),
        )
        .expect("the tinted blur runs");
    for y in 0..8 {
        for x in 0..8 {
            let seen = pixel(&output, 8, x, y);
            assert!(
                seen[0].abs_diff(178) <= 2
                    && seen[1].abs_diff(75) <= 2
                    && seen[2].abs_diff(100) <= 2
                    && seen[3].abs_diff(255) <= 2,
                "a half-weight tint must blend halfway at ({x},{y}), got {seen:?}"
            );
        }
    }
}

/// A zero-weight tint must leave the blur byte-identical to the untinted path, and a
/// non-finite weight must be ignored rather than poisoning the frame.
#[test]
fn a_zero_or_non_finite_weight_tint_leaves_the_blur_untouched() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..8 * 8).flat_map(|_| [100u8, 150, 200, 255]).collect();
    let plain = renderer.render_blur_rgba8(8, 8, &input, 3).expect("plain blur");
    let zero = renderer
        .render_backdrop_blur_rgba8(
            8,
            8,
            &input,
            3,
            Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, 0.0)),
        )
        .expect("zero-weight blur");
    assert_eq!(zero, plain, "a zero-weight tint must not change the blur");
    let nan = renderer
        .render_backdrop_blur_rgba8(
            8,
            8,
            &input,
            3,
            Some((Rgba8 { r: 255, g: 0, b: 0, a: 255 }, f32::NAN)),
        )
        .expect("nan-weight blur");
    assert_eq!(nan, plain, "a non-finite weight must be ignored");
}

/// A zero-radius backdrop blur must still apply the tint: the shader's `radius <= 0`
/// branch is a passthrough of the input plus the tint composite, matching the software
/// layer composition where the tint sits on the unblurred backdrop.
#[test]
fn a_zero_radius_backdrop_blur_still_applies_the_tint() {
    let Some(renderer) = renderer() else { return };
    let input: Vec<u8> = (0..4 * 4).flat_map(|_| [100u8, 150, 200, 255]).collect();
    let output = renderer
        .render_backdrop_blur_rgba8(
            4,
            4,
            &input,
            0,
            Some((Rgba8 { r: 0, g: 255, b: 0, a: 255 }, 1.0)),
        )
        .expect("the passthrough blur runs");
    for y in 0..4 {
        for x in 0..4 {
            let seen = pixel(&output, 4, x, y);
            assert!(
                seen[0].abs_diff(0) <= 2
                    && seen[1].abs_diff(255) <= 2
                    && seen[2].abs_diff(0) <= 2
                    && seen[3].abs_diff(255) <= 2,
                "the tint must composite over the unblurred backdrop at ({x},{y}), got {seen:?}"
            );
        }
    }
}

// ── G-218: command-list GPU dispatch ──

fn opaque(r: u8, g: u8, b: u8) -> Rgba8 {
    Rgba8 { r, g, b, a: 255 }
}

/// A stream the pipelines cover must render on the GPU and agree with the software
/// rasteriser at interior samples. Edge pixels are deliberately avoided: the SDF circle's
/// anti-aliased edge is the mandated difference from the CPU's hard-edged disc.
#[test]
fn a_dispatchable_stream_matches_the_software_raster() {
    let commands = vec![
        WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 10, height: 10 },
            color: opaque(255, 0, 0),
            clip: None,
        },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 10, y: 0, width: 10, height: 10 },
            color: opaque(0, 255, 0),
            clip: None,
        },
        WgpuDrawCommand::SetBlendMode { mode: 0 },
        WgpuDrawCommand::FillCircle {
            center: (15, 20),
            radius: 6,
            color: opaque(0, 0, 255),
            clip: None,
        },
    ];
    let Some(renderer) = renderer() else { return };
    let gpu = renderer
        .render_draw_commands_rgba8(32, 32, &commands)
        .expect("the dispatchable stream must render on the GPU");
    // Interior samples only: rect interiors, the circle centre and one more interior
    // circle pixel, plus two pixels that must show the clear colour.
    assert_eq!(pixel(&gpu, 32, 5, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&gpu, 32, 15, 5), [0, 255, 0, 255]);
    assert_eq!(pixel(&gpu, 32, 15, 20), [0, 0, 255, 255]);
    assert_eq!(pixel(&gpu, 32, 15, 17), [0, 0, 255, 255]);
    assert_eq!(pixel(&gpu, 32, 5, 31), [10, 20, 30, 255]);
    assert_eq!(pixel(&gpu, 32, 31, 31), [10, 20, 30, 255]);
}

/// A stream containing a command class no pipeline covers must fall back to the software
/// rasteriser — returned directly, so the result is byte-identical to the CPU picture and
/// no upload/readback round trip happens in between (G-217).
#[test]
fn a_non_dispatchable_stream_falls_back_to_the_software_raster() {
    let commands = vec![
        WgpuDrawCommand::PushClip { rect: PixelRect { x: 0, y: 0, width: 5, height: 10 } },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 10, height: 10 },
            color: opaque(255, 0, 0),
            clip: None,
        },
        WgpuDrawCommand::PopClip,
    ];
    let Some(renderer) = renderer() else { return };
    let gpu =
        renderer.render_draw_commands_rgba8(16, 16, &commands).expect("the fallback must render");
    let software = rasterize_draw_commands_rgba8(16, 16, &commands).expect("rasterises");
    assert_eq!(gpu, software, "the fallback returns the software picture directly");
}

/// A blend mode other than `Normal` is not dispatchable: the pipelines' `REPLACE` write
/// cannot reproduce it, so the stream must fall back to the CPU rasteriser.
#[test]
fn a_non_normal_blend_mode_falls_back() {
    let commands = vec![
        WgpuDrawCommand::Clear { color: opaque(10, 20, 30) },
        WgpuDrawCommand::SetBlendMode { mode: 1 },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
            color: opaque(128, 128, 128),
            clip: None,
        },
    ];
    let Some(renderer) = renderer() else { return };
    let gpu =
        renderer.render_draw_commands_rgba8(8, 8, &commands).expect("the fallback must render");
    let software = rasterize_draw_commands_rgba8(8, 8, &commands).expect("rasterises");
    assert_eq!(gpu, software, "a non-Normal blend mode must fall back to the CPU picture");
}

/// A mid-stream `Clear` must erase everything before it, exactly as the command's
/// documentation promises, with later draws landing on the new background.
#[test]
fn a_mid_stream_clear_repaints_the_frame() {
    let commands = vec![
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 20, height: 20 },
            color: opaque(255, 0, 0),
            clip: None,
        },
        WgpuDrawCommand::Clear { color: opaque(0, 0, 255) },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
            color: opaque(0, 255, 0),
            clip: None,
        },
    ];
    let Some(renderer) = renderer() else { return };
    let gpu =
        renderer.render_draw_commands_rgba8(24, 24, &commands).expect("the stream must render");
    assert_eq!(pixel(&gpu, 24, 10, 10), [0, 0, 255, 255], "the clear erases the first fill");
    assert_eq!(pixel(&gpu, 24, 2, 2), [0, 255, 0, 255], "later draws sit on the new background");
    assert_eq!(pixel(&gpu, 24, 23, 23), [0, 0, 255, 255]);
}

/// An over-cap run of rectangles through the dispatcher must stream across chunks without
/// reordering, the same contract the instanced quad pipeline promises (G-218).
#[test]
fn an_over_cap_dispatch_run_streams_every_chunk_in_order() {
    let Some(renderer) = renderer() else { return };
    let per_row = 1024u32;
    let total = MAX_INSTANCES_PER_DRAW + 1;
    let width = per_row * 2;
    let height = total.div_ceil(per_row as usize) as u32;
    let mut commands = Vec::with_capacity(total);
    for index in 0..total {
        let x = ((index as u32 % per_row) * 2) as i32;
        let y = (index as u32 / per_row) as i32;
        let color = if index + 1 == total { opaque(0, 0, 255) } else { opaque(255, 0, 0) };
        commands.push(WgpuDrawCommand::FillRect {
            rect: PixelRect { x, y, width: 2, height: 1 },
            color,
            clip: None,
        });
    }
    let pixels = renderer
        .render_draw_commands_rgba8(width, height, &commands)
        .expect("an over-cap dispatch run must render");
    let sample = |index: usize| -> [u8; 4] {
        let x = (index as u32 % per_row) * 2;
        let y = index as u32 / per_row;
        pixel(&pixels, width, x, y)
    };
    let last = total - 1;
    assert_eq!(sample(last), [0, 0, 255, 255], "the second chunk must draw its own rect");
    assert_eq!(sample(last - 1), [255, 0, 0, 255], "the first chunk's last rect must draw");
    assert_eq!(sample(0), [255, 0, 0, 255], "the first chunk's first rect must draw");
}

// ── The image pipeline ──────────────────────────────────────────────────────────

/// A scaled image draw must produce the software rasteriser's bytes, not merely something that
/// looks like an image.
///
/// The two paths agree by construction — the fragment shader fetches the texel at the same
/// integer coordinate the CPU loop computes — so this is an exact equality, and the explicit
/// pixel expectations below keep it from passing on a shared mistake.
#[test]
fn a_scaled_image_draw_matches_the_software_rasteriser() {
    let Some(renderer) = renderer() else { return };
    let commands = [
        WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
        WgpuDrawCommand::DrawImage {
            rect: PixelRect { x: 1, y: 2, width: 8, height: 4 },
            // One row, two columns: red then blue.
            rgba8: vec![255, 0, 0, 255, 0, 0, 255, 255].into(),
            image_width: 2,
            image_height: 1,
            clip: None,
        },
    ];
    let gpu = renderer
        .render_draw_commands_rgba8(12, 8, &commands)
        .expect("the image stream must render on the GPU");
    let cpu = rasterize_draw_commands_rgba8(12, 8, &commands).expect("the CPU rasteriser renders");
    assert_eq!(gpu, cpu, "the GPU image must be the software rasteriser's picture");

    assert_eq!(pixel(&gpu, 12, 1, 2), [255, 0, 0, 255], "the left half is the first source column");
    assert_eq!(pixel(&gpu, 12, 4, 2), [255, 0, 0, 255], "the last left-half pixel is still red");
    assert_eq!(pixel(&gpu, 12, 5, 2), [0, 0, 255, 255], "the right half is the second column");
    assert_eq!(pixel(&gpu, 12, 8, 2), [0, 0, 255, 255], "the last destination column is blue");
    assert_eq!(pixel(&gpu, 12, 0, 2), [0, 0, 0, 255], "nothing left of the destination");
    assert_eq!(pixel(&gpu, 12, 1, 6), [0, 0, 0, 255], "nothing below the destination");
}

/// A clipped image draw is the same picture, restricted: the scissor the encoder derives from the
/// command's clip must match the region the CPU rasteriser writes.
#[test]
fn a_clipped_image_draw_matches_the_software_rasteriser() {
    let Some(renderer) = renderer() else { return };
    let commands = [
        WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
        WgpuDrawCommand::DrawImage {
            rect: PixelRect { x: 1, y: 2, width: 8, height: 4 },
            rgba8: vec![255, 0, 0, 255, 0, 0, 255, 255].into(),
            image_width: 2,
            image_height: 1,
            clip: Some(PixelRect { x: 5, y: 2, width: 2, height: 4 }),
        },
    ];
    let gpu = renderer
        .render_draw_commands_rgba8(12, 8, &commands)
        .expect("the clipped image stream must render on the GPU");
    let cpu = rasterize_draw_commands_rgba8(12, 8, &commands).expect("the CPU rasteriser renders");
    assert_eq!(gpu, cpu, "the clip must select the same pixels on both paths");
    assert_eq!(pixel(&gpu, 12, 4, 2), [0, 0, 0, 255], "left of the clip: untouched");
    assert_eq!(pixel(&gpu, 12, 5, 2), [0, 0, 255, 255], "inside the clip: drawn");
    assert_eq!(pixel(&gpu, 12, 6, 3), [0, 0, 255, 255], "inside the clip: drawn");
    assert_eq!(pixel(&gpu, 12, 7, 3), [0, 0, 0, 255], "the clip's right edge is exclusive");
}

/// A malformed payload is not dispatchable, so the caller gets the CPU rasteriser's explicit
/// error instead of a GPU frame that silently skipped the image.
#[test]
fn a_malformed_image_payload_is_reported_instead_of_dispatched() {
    let Some(renderer) = renderer() else { return };
    let error = renderer
        .render_draw_commands_rgba8(
            4,
            4,
            &[
                WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
                WgpuDrawCommand::DrawImage {
                    rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
                    // 12 bytes for a 2x2 image: short by four.
                    rgba8: vec![0; 12].into(),
                    image_width: 2,
                    image_height: 2,
                    clip: None,
                },
            ],
        )
        .expect_err("a short payload must fail, not paint");
    assert!(error.contains("invalid DrawImage payload"), "{error}");
}

/// The image arm of the dispatcher must actually claim an image stream.
///
/// The parity test above cannot tell "the GPU drew this" from "the CPU did": both produce the
/// same bytes by design. This asserts the dispatch decision itself, so a stream of clears and
/// images is proven to take the GPU pass, and a malformed payload is proven to stay on the CPU
/// path that reports it.
#[test]
fn an_image_stream_is_claimed_by_the_gpu_dispatcher() {
    let image = |pixels: Vec<u8>| WgpuDrawCommand::DrawImage {
        rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
        rgba8: pixels.into(),
        image_width: 2,
        image_height: 2,
        clip: None,
    };
    let clear = WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } };

    assert!(
        commands_gpu_dispatchable(&[clear.clone(), image(vec![0; 2 * 2 * 4])]),
        "a well-formed image stream must be dispatched"
    );
    assert!(
        !commands_gpu_dispatchable(&[clear, image(vec![0; 12])]),
        "a short payload must stay on the CPU path that reports it"
    );
    assert!(
        commands_gpu_dispatchable(&[WgpuDrawCommand::PushClip {
            rect: PixelRect { x: 0, y: 0, width: 4, height: 4 }
        }]),
        "a rectangle clip is expressible as narrowed geometry and a scissor"
    );
    assert!(
        commands_gpu_dispatchable(&[WgpuDrawCommand::DrawText {
            rect: PixelRect { x: 0, y: 0, width: 8, height: 8 },
            text: "A".into(),
            color: Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            clip: None,
        }]),
        "a text box is drawn from its own coverage texture"
    );
    assert!(
        !commands_gpu_dispatchable(&[WgpuDrawCommand::DrawPath {
            points: vec![(0, 0), (4, 0), (4, 4)].into(),
            closed: true,
            color: Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            filled: false,
            width: 1,
            clip: None,
        }]),
        "paths still need the CPU rasteriser"
    );
}

// ── Clips, strokes and rounded fills on the GPU ────────────────────────────────

/// Renders the same stream through both paths and requires the bytes to be identical.
///
/// It also asserts the stream is *claimed* by the GPU dispatcher: without that, this helper would
/// compare the CPU rasteriser with itself and pass while proving nothing.
fn assert_gpu_matches_software(width: u32, height: u32, commands: &[WgpuDrawCommand]) {
    let Some(renderer) = renderer() else { return };
    assert!(commands_gpu_dispatchable(commands), "the stream under test must be GPU-dispatchable");
    let gpu = renderer
        .render_draw_commands_rgba8(width, height, commands)
        .expect("the GPU path renders this stream");
    let cpu = rasterize_draw_commands_rgba8(width, height, commands)
        .expect("the CPU rasteriser renders this stream");
    if gpu != cpu {
        let first = gpu.iter().zip(&cpu).position(|(a, b)| a != b).unwrap_or(0) / 4;
        let (x, y) = (first as u32 % width, first as u32 / width);
        let differing = gpu.iter().zip(&cpu).filter(|(a, b)| a != b).count();
        panic!(
            "first difference at ({x},{y}): gpu {:?} cpu {:?} ({differing} differing bytes)",
            pixel(&gpu, width, x, y),
            pixel(&cpu, width, x, y)
        );
    }
}

/// As [`assert_gpu_matches_software`], but allows one level of difference per channel.
///
/// Fixed-function blending cannot reproduce the crate's un-premultiplied composite bit for bit:
/// the GPU rounds the blend result when it stores it, and the CPU rounds the same expression after
/// a division by the output alpha. On an opaque destination the two expressions are identical, so
/// the only places they can disagree are the rounding ties — measured here as a single LSB, never
/// more. Partial-coverage antialiasing (a glyph edge, a translucent colour) is exactly that case.
fn assert_gpu_matches_software_within_one_level(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
) {
    let Some(renderer) = renderer() else { return };
    assert!(commands_gpu_dispatchable(commands), "the stream under test must be GPU-dispatchable");
    let gpu = renderer
        .render_draw_commands_rgba8(width, height, commands)
        .expect("the GPU path renders this stream");
    let cpu = rasterize_draw_commands_rgba8(width, height, commands)
        .expect("the CPU rasteriser renders this stream");
    let mut worst = 0i32;
    let mut worst_at = (0u32, 0u32);
    for (index, (a, b)) in gpu.iter().zip(&cpu).enumerate() {
        let delta = (*a as i32 - *b as i32).abs();
        if delta > worst {
            worst = delta;
            worst_at = (index as u32 / 4 % width, index as u32 / 4 / width);
        }
    }
    assert!(
        worst <= 1,
        "the GPU frame differs by {worst} at {worst_at:?}; only a single LSB of blend rounding \
         is acceptable, and nothing else"
    );
}

/// A rectangle clip is stream state: `PushClip` narrows the fill, `PopClip` restores it.
#[test]
fn a_clipped_rectangle_stream_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        20,
        20,
        &[
            WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
            WgpuDrawCommand::PushClip { rect: PixelRect { x: 2, y: 2, width: 6, height: 6 } },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 0, y: 0, width: 20, height: 20 },
                color: Rgba8 { r: 255, g: 0, b: 0, a: 255 },
                clip: None,
            },
            WgpuDrawCommand::PopClip,
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 14, y: 14, width: 4, height: 4 },
                color: Rgba8 { r: 0, g: 255, b: 0, a: 255 },
                clip: None,
            },
        ],
    );
}

/// The four edges of a stroked rectangle are the same rectangles the CPU fills, so the picture
/// must be identical — including under a clip.
#[test]
fn a_stroked_rectangle_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
            WgpuDrawCommand::StrokeRect {
                rect: PixelRect { x: 2, y: 3, width: 16, height: 12 },
                color: Rgba8 { r: 40, g: 120, b: 255, a: 255 },
                thickness: 3,
                clip: Some(PixelRect { x: 0, y: 0, width: 12, height: 24 }),
            },
        ],
    );
}

/// The corner arcs must be *the same* arcs: both paths take their row spans from one function,
/// so this is an equality rather than a tolerance.
#[test]
fn a_rounded_rectangle_fill_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        28,
        24,
        &[
            WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
            WgpuDrawCommand::FillRoundedRect {
                rect: PixelRect { x: 2, y: 1, width: 22, height: 20 },
                radius: 6,
                color: Rgba8 { r: 255, g: 180, b: 0, a: 255 },
                clip: None,
            },
        ],
    );
}

/// A disc under a clip is scissored, and the scissor is the only thing the clip changes.
///
/// The disc itself is **not** compared against the CPU rasteriser: this backend's circle pipeline
/// anti-aliases its edge with an SDF while the CPU rasteriser fills a hard-edged disc, so the two
/// differ by construction on the rim (that was already true before any clipping was involved).
/// What must hold is that clipping takes away exactly the pixels outside the clip and leaves every
/// other pixel byte-identical to the unclipped draw.
#[test]
fn a_clipped_circle_is_the_unclipped_circle_minus_the_clip() {
    let Some(renderer) = renderer() else { return };
    let clear = WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } };
    let circle = WgpuDrawCommand::FillCircle {
        center: (12, 12),
        radius: 7,
        color: Rgba8 { r: 0, g: 200, b: 120, a: 255 },
        clip: None,
    };
    let clip = PixelRect { x: 6, y: 6, width: 10, height: 8 };

    let unclipped = renderer
        .render_draw_commands_rgba8(24, 24, &[clear.clone(), circle.clone()])
        .expect("the unclipped disc renders");
    let clipped = renderer
        .render_draw_commands_rgba8(
            24,
            24,
            &[clear, WgpuDrawCommand::PushClip { rect: clip }, circle, WgpuDrawCommand::PopClip],
        )
        .expect("the clipped disc renders");

    for y in 0..24u32 {
        for x in 0..24u32 {
            let inside = x >= clip.x as u32
                && y >= clip.y as u32
                && x < clip.right() as u32
                && y < clip.bottom() as u32;
            if inside {
                assert_eq!(
                    pixel(&clipped, 24, x, y),
                    pixel(&unclipped, 24, x, y),
                    "the clip must not change a pixel inside it (({x},{y}))"
                );
            } else {
                assert_eq!(
                    pixel(&clipped, 24, x, y),
                    [0, 0, 0, 255],
                    "outside the clip only the clear colour may remain (({x},{y}))"
                );
            }
        }
    }
}

// ── Text on the GPU ────────────────────────────────────────────────────────────

/// Text is the command that decides whether a real frame can stay on the GPU, so it must be an
/// equality and not a resemblance: the glyphs come from the same font-stack call, and the
/// compositing is source-over on an opaque background, which is the case fixed-function blending
/// reproduces exactly.
#[test]
fn a_text_box_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        40,
        16,
        &[
            WgpuDrawCommand::Clear { color: Rgba8 { r: 20, g: 20, b: 24, a: 255 } },
            WgpuDrawCommand::DrawText {
                rect: PixelRect { x: 2, y: 2, width: 36, height: 12 },
                text: "Hi!".into(),
                color: Rgba8 { r: 235, g: 225, b: 190, a: 255 },
                clip: None,
            },
        ],
    );
}

/// A half-transparent text colour still composites identically on an opaque background, and a clip
/// takes away exactly the pixels it covers.
#[test]
fn a_clipped_translucent_text_box_matches_the_software_rasteriser() {
    assert_gpu_matches_software_within_one_level(
        40,
        16,
        &[
            WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 255 } },
            WgpuDrawCommand::PushClip { rect: PixelRect { x: 4, y: 0, width: 20, height: 16 } },
            WgpuDrawCommand::DrawText {
                rect: PixelRect { x: 2, y: 2, width: 36, height: 12 },
                text: "Ag".into(),
                color: Rgba8 { r: 120, g: 200, b: 255, a: 160 },
                clip: None,
            },
            WgpuDrawCommand::PopClip,
        ],
    );
}

// ── Rings, wedges and rounded outlines (R1) ────────────────────────────────────

const RED: Rgba8 = Rgba8 { r: 255, g: 0, b: 0, a: 255 };
const BLACK: Rgba8 = Rgba8 { r: 0, g: 0, b: 0, a: 255 };

/// A stroked circle paints its band and leaves both the hole and the outside alone.
#[test]
fn a_circle_stroke_paints_its_band_only() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::DrawCircle {
                    center: (16, 16),
                    radius: 10,
                    color: RED,
                    width: 2,
                    clip: None,
                },
            ],
        )
        .expect("the ring renders");
    assert_eq!(pixel(&pixels, 32, 26, 16), [255, 0, 0, 255], "right of centre, on the band");
    assert_eq!(pixel(&pixels, 32, 16, 26), [255, 0, 0, 255], "below centre, on the band");
    assert_eq!(pixel(&pixels, 32, 16, 16), [0, 0, 0, 255], "the hole stays clear");
    assert_eq!(pixel(&pixels, 32, 30, 16), [0, 0, 0, 255], "outside the ring stays clear");
}

/// An arc is the same band limited to its angular window: the sweep start is included and the
/// opposite side of the circle is not painted at all.
#[test]
fn an_arc_band_is_limited_to_its_window() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::DrawArc {
                    center: (16, 16),
                    radius: 10,
                    start_angle: 0.0,
                    end_angle: core::f32::consts::FRAC_PI_2,
                    color: RED,
                    filled: false,
                    clip: None,
                },
            ],
        )
        .expect("the arc renders");
    // Screen space: +x is angle 0 and +y (down) is PI/2, so the 45-degree point of the window is
    // down-right of the centre.
    assert_eq!(pixel(&pixels, 32, 23, 23), [255, 0, 0, 255], "inside the sweep");
    assert_eq!(pixel(&pixels, 32, 9, 9), [0, 0, 0, 255], "the opposite quadrant is not painted");
}

/// A filled arc is a pie wedge: the same angular window, filled back to the centre.
#[test]
fn a_filled_arc_is_a_pie_wedge() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::DrawArc {
                    center: (16, 16),
                    radius: 10,
                    start_angle: 0.0,
                    end_angle: core::f32::consts::FRAC_PI_2,
                    color: RED,
                    filled: true,
                    clip: None,
                },
            ],
        )
        .expect("the wedge renders");
    assert_eq!(pixel(&pixels, 32, 20, 20), [255, 0, 0, 255], "inside the wedge");
    assert_eq!(pixel(&pixels, 32, 12, 12), [0, 0, 0, 255], "outside the wedge");
}

/// A rounded outline is the band between the outer rounded rectangle and the rectangle inset by
/// the thickness: the edge is painted, the interior is not.
#[test]
fn a_rounded_outline_paints_its_band_only() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::StrokeRoundedRect {
                    rect: PixelRect { x: 4, y: 4, width: 24, height: 20 },
                    radius: 6,
                    color: RED,
                    thickness: 3,
                    clip: None,
                },
            ],
        )
        .expect("the outline renders");
    assert_eq!(pixel(&pixels, 32, 16, 4), [255, 0, 0, 255], "the top edge is painted");
    assert_eq!(pixel(&pixels, 32, 16, 12), [0, 0, 0, 255], "the interior is not");
    assert_eq!(pixel(&pixels, 32, 0, 0), [0, 0, 0, 255], "outside the rectangle is not");
}

/// The SDF circle returns transparent coverage across most of its bounding quad, so its pipeline
/// must composite rather than replace — otherwise drawing a disc would erase a rectangular hole in
/// whatever the frame had already painted.
#[test]
fn a_circle_drawn_over_content_keeps_the_content_around_it() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::FillRect {
                    rect: PixelRect { x: 0, y: 0, width: 32, height: 32 },
                    color: Rgba8 { r: 0, g: 0, b: 200, a: 255 },
                    clip: None,
                },
                WgpuDrawCommand::FillCircle { center: (16, 16), radius: 6, color: RED, clip: None },
            ],
        )
        .expect("the frame renders");
    assert_eq!(pixel(&pixels, 32, 16, 16), [255, 0, 0, 255], "the disc is painted");
    assert_eq!(
        pixel(&pixels, 32, 11, 11),
        [0, 0, 200, 255],
        "inside the disc's bounding box but outside the disc, the background must survive"
    );
}

// ── Gradients on the GPU (R2) ──────────────────────────────────────────────────

/// A vertical ramp is computed once per row by the CPU's own stop rule and repeated across the
/// row by the image pipeline, so the frame must be the software rasteriser's picture exactly.
#[test]
fn a_vertical_gradient_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        16,
        12,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::DrawGradient {
                rect: PixelRect { x: 2, y: 1, width: 10, height: 8 },
                gradient_data: vec![255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255].into(),
                clip: None,
            },
        ],
    );
}

/// The horizontal counterpart, including a clip: the ramp must not be re-phased by the clip.
#[test]
fn a_horizontal_gradient_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        20,
        10,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::FillLinearGradient {
                rect: PixelRect { x: 1, y: 2, width: 16, height: 6 },
                gradient_data: vec![10, 20, 30, 255, 200, 210, 220, 255].into(),
                clip: Some(PixelRect { x: 5, y: 0, width: 10, height: 10 }),
            },
        ],
    );
}

/// A radial gradient computes `t` in the shader rather than sampling a ramp, so this is the one
/// gradient whose parity depends on the two `sqrt` implementations rounding alike. The assertion
/// is exact: if it ever stops being exact, the test fails and the difference is the finding.
#[test]
fn a_radial_gradient_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::FillRadialGradient {
                rect: PixelRect { x: 2, y: 3, width: 18, height: 16 },
                gradient_data: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255].into(),
                clip: None,
            },
        ],
    );
}

/// The same parity across stop counts and rectangle shapes, so the first case cannot be luck: the
/// stop count changes where the index boundaries fall, and a non-square rectangle changes the
/// radius against which `t` is scaled.
#[test]
fn radial_gradients_match_across_stop_counts_and_sizes() {
    for (stops, width, height) in [(2u8, 17u32, 13u32), (5, 32, 9), (9, 11, 11), (4, 5, 40)] {
        let mut gradient_data = Vec::new();
        for index in 0..stops {
            gradient_data.extend_from_slice(&[
                index.wrapping_mul(37),
                index.wrapping_mul(59),
                index.wrapping_mul(83),
                255,
            ]);
        }
        assert_gpu_matches_software(
            width + 4,
            height + 4,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::FillRadialGradient {
                    rect: PixelRect { x: 2, y: 2, width, height },
                    gradient_data: gradient_data.into(),
                    clip: None,
                },
            ],
        );
    }
}

// ── Filled paths on the GPU (R3) ──────────────────────────────────────────────

/// A convex polygon fills its interior and nothing else. The stencil buffer counts the fan's
/// triangles, so this is the base case the two rules have to agree on.
#[test]
fn a_filled_path_paints_its_interior() {
    let Some(renderer) = renderer() else { return };
    let pixels = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::DrawPath {
                    points: vec![(8, 8), (24, 8), (24, 24), (8, 24)].into(),
                    closed: true,
                    color: RED,
                    filled: true,
                    width: 1,
                    clip: None,
                },
            ],
        )
        .expect("the filled path renders");
    assert_eq!(pixel(&pixels, 32, 16, 16), [255, 0, 0, 255], "the interior is filled");
    assert_eq!(pixel(&pixels, 32, 4, 16), [0, 0, 0, 255], "left of the polygon is untouched");
    assert_eq!(pixel(&pixels, 32, 28, 16), [0, 0, 0, 255], "right of it is untouched");
    assert_eq!(pixel(&pixels, 32, 16, 4), [0, 0, 0, 255], "above it is untouched");
}

/// The even-odd and non-zero rules differ exactly where the shape is covered twice. A pentagram
/// is the canonical case: both rules paint its five points, but only non-zero paints the pentagon
/// at the centre, which the outline crosses twice.
///
/// Both fills use the **same** point list, so the difference is the rule and nothing else.
#[test]
fn even_odd_and_nonzero_disagree_on_a_pentagram() {
    let Some(renderer) = renderer() else { return };
    // Star order: each vertex already connects across the figure, so the centre is traversed twice.
    let points: Vec<(i32, i32)> = vec![(16, 3), (24, 28), (3, 12), (29, 12), (8, 28)];

    let even_odd = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::DrawPath {
                    points: points.clone().into(),
                    closed: true,
                    color: RED,
                    filled: true,
                    width: 1,
                    clip: None,
                },
            ],
        )
        .expect("the even-odd fill renders");
    let nonzero = renderer
        .render_draw_commands_rgba8(
            32,
            32,
            &[
                WgpuDrawCommand::Clear { color: BLACK },
                WgpuDrawCommand::FillPathNonZero {
                    points: points.into(),
                    contours: vec![(0usize, 5usize)].into(),
                    color: RED,
                    clip: None,
                },
            ],
        )
        .expect("the non-zero fill renders");

    // A point of the star: both rules fill it.
    assert_eq!(pixel(&even_odd, 32, 16, 6), [255, 0, 0, 255], "even-odd fills a point");
    assert_eq!(pixel(&nonzero, 32, 16, 6), [255, 0, 0, 255], "non-zero fills a point");
    // The centre pentagon: crossed twice, so even-odd leaves it empty and non-zero fills it.
    assert_eq!(pixel(&even_odd, 32, 16, 17), [0, 0, 0, 255], "even-odd leaves the centre");
    assert_eq!(pixel(&nonzero, 32, 16, 17), [255, 0, 0, 255], "non-zero fills the centre");
}

/// An unblurred shadow is a translated rectangle, so the GPU can paint it exactly: the assertion is
/// the usual byte-for-byte comparison, clip included.
#[test]
fn an_unblurred_shadow_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::BoxShadow {
                rect: PixelRect { x: 4, y: 4, width: 12, height: 10 },
                color: Rgba8 { r: 90, g: 90, b: 110, a: 255 },
                offset_x: 3,
                offset_y: 4,
                blur_radius: 0,
                clip: Some(PixelRect { x: 0, y: 0, width: 18, height: 24 }),
            },
        ],
    );
}

/// A blurred shadow is the first command whose GPU form is more than one pass: the body is painted
/// into an offscreen layer, filtered twice and composited. The layer is allocated at exactly the
/// CPU rasteriser's extent and the shader repeats its clamped integer moving average, so the two
/// paths must agree pixel for pixel, not merely look alike.
#[test]
fn a_blurred_shadow_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::BoxShadow {
                rect: PixelRect { x: 6, y: 6, width: 8, height: 6 },
                color: Rgba8 { r: 90, g: 90, b: 110, a: 255 },
                offset_x: 2,
                offset_y: 3,
                blur_radius: 3,
                clip: None,
            },
        ],
    );
}

/// The paint region bounds the feathered shadow: a clip that cuts through it must leave every pixel
/// outside exactly as the stream left it, while the layer's own blur still reaches the edge.
#[test]
fn a_clipped_blurred_shadow_keeps_the_pixels_outside_its_clip() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::BoxShadow {
                rect: PixelRect { x: 6, y: 6, width: 8, height: 6 },
                color: Rgba8 { r: 200, g: 40, b: 40, a: 255 },
                offset_x: 0,
                offset_y: 0,
                blur_radius: 2,
                clip: Some(PixelRect { x: 4, y: 4, width: 10, height: 8 }),
            },
        ],
    );
}

/// Two blurred shadows in one frame get their own layers, radii and positions, and a later draw
/// still paints over both: the per-layer parameter buffers exist so one pass cannot read another
/// shadow's radius, and the composite keeps its stream position.
#[test]
fn two_blurred_shadows_keep_their_own_layers_and_order() {
    assert_gpu_matches_software(
        32,
        32,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            WgpuDrawCommand::BoxShadow {
                rect: PixelRect { x: 2, y: 2, width: 8, height: 8 },
                color: Rgba8 { r: 0, g: 0, b: 255, a: 200 },
                offset_x: 0,
                offset_y: 0,
                blur_radius: 2,
                clip: None,
            },
            WgpuDrawCommand::BoxShadow {
                rect: PixelRect { x: 20, y: 18, width: 6, height: 6 },
                color: Rgba8 { r: 0, g: 200, b: 0, a: 255 },
                offset_x: 1,
                offset_y: 1,
                blur_radius: 4,
                clip: None,
            },
            WgpuDrawCommand::FillRect {
                rect: PixelRect { x: 10, y: 10, width: 6, height: 6 },
                color: RED,
                clip: None,
            },
        ],
    );
}

/// A blurred shadow is claimed by the dispatcher: without this the two tests above would compare the
/// CPU rasteriser with itself.
#[test]
fn a_blurred_shadow_is_claimed_by_the_gpu() {
    assert!(commands_gpu_dispatchable(&[WgpuDrawCommand::BoxShadow {
        rect: PixelRect { x: 0, y: 0, width: 4, height: 4 },
        color: RED,
        offset_x: 1,
        offset_y: 1,
        blur_radius: 4,
        clip: None,
    }]));
}

/// An axis-aligned line is a rectangle under the CPU's stamped-square thickness rule, so the GPU
/// can reproduce it exactly — the assertion is the usual byte-for-byte comparison.
#[test]
fn axis_aligned_lines_match_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        20,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            // Horizontal rule, thickness 4 → half_w = 2.
            WgpuDrawCommand::DrawLine {
                from: (3, 5),
                to: (18, 5),
                color: RED,
                width: 4,
                clip: None,
            },
            // Vertical rule, thickness 3 → half_w = 1 (integer halving of 3 is 1).
            WgpuDrawCommand::DrawLine {
                from: (8, 8),
                to: (8, 16),
                color: Rgba8 { r: 0, g: 200, b: 0, a: 255 },
                width: 3,
                clip: Some(PixelRect { x: 0, y: 10, width: 24, height: 10 }),
            },
        ],
    );
}

/// A diagonal line stays on the CPU: the dispatcher must not claim it, because a capsule would not
/// be the picture the software rasteriser's stamped squares produce.
#[test]
fn a_diagonal_line_stays_on_the_cpu() {
    assert!(!commands_gpu_dispatchable(&[WgpuDrawCommand::DrawLine {
        from: (0, 0),
        to: (8, 8),
        color: RED,
        width: 2,
        clip: None,
    }]));
}

/// A polyline whose segments are all axis-aligned is a run of rectangles under the CPU's
/// per-segment line rule, so the whole path is byte-identical.
#[test]
fn an_axis_aligned_stroke_path_matches_the_software_rasteriser() {
    assert_gpu_matches_software(
        24,
        24,
        &[
            WgpuDrawCommand::Clear { color: BLACK },
            // A rectangle outline drawn as a closed 4-segment polyline.
            WgpuDrawCommand::DrawPath {
                points: vec![(4, 4), (19, 4), (19, 16), (4, 16)].into(),
                closed: true,
                color: RED,
                filled: false,
                width: 2,
                clip: Some(PixelRect { x: 0, y: 0, width: 24, height: 18 }),
            },
        ],
    );
}

/// One diagonal segment sends the whole path back to the CPU, for the same reason a single
/// diagonal line does.
#[test]
fn a_diagonal_stroke_path_stays_on_the_cpu() {
    assert!(!commands_gpu_dispatchable(&[WgpuDrawCommand::DrawPath {
        points: vec![(0, 0), (8, 8), (8, 16)].into(),
        closed: false,
        color: RED,
        filled: false,
        width: 2,
        clip: None,
    }]));
}
