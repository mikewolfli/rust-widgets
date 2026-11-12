use super::super::paint::take_image_bytes_drawn_without_copy;
use super::*;
use crate::compat::{lock, MutexGuard};
// `String`/`ToString` are not in scope under the `mini` profile (no std
// prelude here), so the reference implementation in the fit test must import
// the crate's own aliases explicitly.
use crate::compat::{MiniToString, String};
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::SoftwarePaintBackend;
use crate::render::{PaintBackend, RenderCommand};

/// GV4-10: a three-vertex path takes the inline command and still paints.
#[test]
fn a_three_vertex_path_paints_through_the_inline_variant() {
    let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        context.draw_path(
            &[Point::new(2, 2), Point::new(14, 2), Point::new(2, 14)],
            true,
            Color::BLACK,
            true,
            1,
        );
    }
    backend.end_frame();
    assert!(
        backend.frame_rgba().as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0),
        "the inline three-vertex path must reach the rasteriser"
    );
}

struct SoftwareRenderConfigTestGuard {
    pub(crate) _lock: MutexGuard<'static, ()>,
    pub(crate) original: SoftwareRenderConfig,
}

impl SoftwareRenderConfigTestGuard {
    fn new() -> Self {
        let lock = lock(software_render_config_test_lock());
        let original = default_software_render_config();
        Self { _lock: lock, original }
    }
}

impl Drop for SoftwareRenderConfigTestGuard {
    fn drop(&mut self) {
        set_default_software_render_config(self.original);
    }
}

// ── BackBuffer ──────────────────────────────────────────────────────

#[test]
fn back_buffer_new_creates_buffers() {
    let bb = BackBuffer::new(Size::new(10, 10), 1.0);
    assert_eq!(bb.size(), Size::new(10, 10));
    assert!((bb.dpi_scale() - 1.0).abs() < 1e-6);
    assert_eq!(bb.front().len(), 10 * 10 * 4);
    assert_eq!(bb.back.len(), 10 * 10 * 4);
}

#[test]
fn back_buffer_zero_size() {
    let bb = BackBuffer::new(Size::new(0, 0), 1.0);
    assert_eq!(bb.size(), Size::new(0, 0));
    assert!(bb.front().is_empty());
    assert!(bb.back.is_empty());
}

/// Writes one opaque pixel straight into a back buffer row.
fn put(pixels: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

/// Reads one pixel from a presented frame.
fn got(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
}

/// A damage present copies exactly the reported rectangle and nothing else.
///
/// The front buffer starts as a full white frame. Only the top-left 2×2 is reported damaged
/// after the back buffer is painted red there, so a correct region present publishes the red
/// square and leaves the rest of the previous frame in place.
#[test]
fn present_damage_copies_only_the_reported_region() {
    let mut buffer = BackBuffer::new(Size::new(4, 4), 1.0);
    crate::render::fill_pixels(buffer.back_mut(), Color::WHITE);
    buffer.present();
    for y in 0..2 {
        for x in 0..2 {
            put(buffer.back_mut(), 4, x, y, [255, 0, 0, 255]);
        }
    }
    buffer.mark_damage(Rect::new(0, 0, 2, 2));
    assert!(buffer.present_damage(), "a reported region is published");
    assert_eq!(got(buffer.front(), 4, 0, 0), [255, 0, 0, 255], "the damaged pixel updates");
    assert_eq!(got(buffer.front(), 4, 3, 3), [255, 255, 255, 255], "the rest is the old frame");
    assert!(buffer.damage_regions().is_empty(), "the list resets after publishing");
}

/// Nothing reported means nothing published, not a full-frame swap.
#[test]
fn present_damage_with_no_regions_is_a_no_op() {
    let mut buffer = BackBuffer::new(Size::new(2, 2), 1.0);
    crate::render::fill_pixels(buffer.back_mut(), Color::WHITE);
    buffer.present();
    crate::render::fill_pixels(buffer.back_mut(), Color::RED);
    assert!(!buffer.present_damage(), "an empty damage list publishes nothing");
    assert_eq!(got(buffer.front(), 2, 0, 0), [255, 255, 255, 255], "the front is untouched");
}

/// Damage is clipped to the buffer and duplicate sub-regions are folded away.
#[test]
fn mark_damage_clips_and_deduplicates() {
    let mut buffer = BackBuffer::new(Size::new(4, 4), 1.0);
    buffer.mark_damage(Rect::new(-2, -2, 4, 4));
    buffer.mark_damage(Rect::new(0, 0, 2, 2));
    buffer.mark_damage(Rect::new(8, 8, 2, 2));
    assert_eq!(
        buffer.damage_regions(),
        &[(0, 0, 2, 2)],
        "the clipped region survives, its contained duplicate and the off-screen region do not"
    );
}

/// Overlapping damage reports fold into one covering region, so a present copies one band
/// instead of every report; touching edges stay separate. (G-29)
#[test]
fn mark_damage_merges_overlapping_regions() {
    let mut buffer = BackBuffer::new(Size::new(8, 8), 1.0);
    buffer.mark_damage(Rect::new(0, 0, 2, 2));
    buffer.mark_damage(Rect::new(1, 1, 2, 2));
    assert_eq!(buffer.damage_regions(), &[(0, 0, 3, 3)], "overlapping reports become their union");

    buffer.clear_damage();
    buffer.mark_damage(Rect::new(0, 0, 2, 2));
    buffer.mark_damage(Rect::new(2, 0, 2, 2));
    assert_eq!(
        buffer.damage_regions(),
        &[(0, 0, 2, 2), (2, 0, 2, 2)],
        "rectangles that only touch do not overlap"
    );
}

/// The damage list is bounded: many scattered reports collapse to one covering region.
#[test]
fn mark_damage_is_bounded() {
    let mut buffer = BackBuffer::new(Size::new(64, 64), 1.0);
    for index in 0..(MAX_DAMAGE_REGIONS + 1) {
        let x = (index % 8) as i32 * 8;
        let y = (index / 8) as i32 * 8;
        buffer.mark_damage(Rect::new(x, y, 1, 1));
    }
    assert_eq!(buffer.damage_regions().len(), 1, "the list collapses at the bound");
    let (x, y, width, height) = buffer.damage_regions()[0];
    assert_eq!((x, y), (0, 0));
    assert!(width >= 8 && height >= 8, "the covering region keeps every report");
}

/// A full-surface damage region takes the cheaper swap path and still publishes.
#[test]
fn whole_surface_damage_presents_by_swapping() {
    let mut buffer = BackBuffer::new(Size::new(2, 2), 1.0);
    crate::render::fill_pixels(buffer.back_mut(), Color::RED);
    buffer.mark_damage(Rect::new(0, 0, 2, 2));
    assert!(buffer.present_damage());
    assert_eq!(got(buffer.front(), 2, 1, 1), [255, 0, 0, 255]);
}

/// The surface forwards the damage API and a frame can be presented region by region.
#[test]
fn a_surface_presents_a_damaged_region() {
    let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
    surface.begin_frame(Color::WHITE);
    surface.end_frame();
    let previous = surface.frame_rgba().to_vec();
    // The surface's own test helper is in `paint`; seeding the back buffer directly is what an
    // incremental caller does when it adopts the previous frame.
    let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
    assert!(backend.seed_from(&previous));
    backend.execute_command(&RenderCommand::FillRect {
        rect: Rect::new(0, 0, 2, 2),
        color: Color::RED,
    });
    backend.mark_damage(Rect::new(0, 0, 2, 2));
    assert!(backend.present_damage());
    assert_eq!(backend.damage_regions(), &[] as &[(i32, i32, u32, u32)]);
}

#[test]
fn back_buffer_resize() {
    let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
    bb.resize(Size::new(20, 20));
    assert_eq!(bb.size(), Size::new(20, 20));
    assert_eq!(bb.front().len(), 20 * 20 * 4);
    assert_eq!(bb.back.len(), 20 * 20 * 4);
}

#[test]
fn back_buffer_resize_to_zero() {
    let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
    bb.resize(Size::new(0, 0));
    assert_eq!(bb.size(), Size::new(0, 0));
    assert!(bb.front().is_empty());
    assert!(bb.back.is_empty());
}

#[test]
fn back_buffer_set_dpi_scale() {
    let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
    bb.set_dpi_scale(2.5);
    assert!((bb.dpi_scale() - 2.5).abs() < 1e-6);
}

#[test]
fn back_buffer_set_dpi_scale_clamps_minimum() {
    let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
    bb.set_dpi_scale(0.0);
    assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
}

#[test]
fn back_buffer_back_mut_returns_writable_slice() {
    let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
    let back = bb.back_mut();
    assert_eq!(back.len(), 5 * 5 * 4);
    back[0] = 255;
    assert_eq!(bb.back[0], 255);
}

#[test]
fn back_buffer_present_swaps_front_and_back() {
    let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
    // Write to back
    bb.back_mut()[0] = 42;
    assert_ne!(bb.front()[0], 42);

    bb.present();

    // After present, back is now front
    assert_eq!(bb.front()[0], 42);
    // The old front (zeros) is now back
    assert_eq!(bb.back[0], 0);
}

#[test]
fn back_buffer_present_swaps_again() {
    let mut bb = BackBuffer::new(Size::new(2, 2), 1.0);
    bb.back_mut()[0] = 10;
    bb.present();
    bb.back_mut()[0] = 20;
    bb.present();

    // Second present should show 20 on front
    assert_eq!(bb.front()[0], 20);
}

#[test]
fn back_buffer_dpi_scale_initial_clamp() {
    let bb = BackBuffer::new(Size::new(1, 1), 0.0);
    assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
}

// ── SoftwareSurface ─────────────────────────────────────────────────

#[test]
fn software_surface_new_creates_surface() {
    let surface = SoftwareSurface::new(Size::new(100, 100), 1.0);
    assert_eq!(surface.size(), Size::new(100, 100));
    assert!((surface.dpi_scale() - 1.0).abs() < 1e-6);
}

#[test]
fn software_surface_zero_size() {
    let surface = SoftwareSurface::new(Size::new(0, 0), 1.0);
    assert_eq!(surface.size(), Size::new(0, 0));
}

#[test]
fn software_surface_begin_frame_clears() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.begin_frame(Color::RED);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    for chunk in rgba.chunks(4) {
        assert_eq!(chunk[0], 255);
        assert_eq!(chunk[1], 0);
        assert_eq!(chunk[2], 0);
        assert_eq!(chunk[3], 255);
    }
}

#[test]
fn software_surface_fill_rect() {
    let mut surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
    surface.begin_frame(Color::WHITE);

    surface.fill_rect(Rect::new(2, 2, 10, 10), Color::BLUE);

    surface.end_frame();

    let rgba = surface.frame_rgba();
    let stride = 20 * 4;
    let idx = 5 * stride + 5 * 4;
    assert_eq!(rgba[idx], 0); // R
    assert_eq!(rgba[idx + 1], 0); // G
    assert_eq!(rgba[idx + 2], 255); // B
}

/// A rotated image keeps its pixels inside the mapped quad and paints none outside it.
///
/// The axis-aligned path stretches a source grid over a rectangle; under an affine the grid
/// lands on a parallelogram, and the only correct reading of "draw this image here" is to
/// sample the source through the inverse mapping. A bounding-box fill would paint the four
/// corners the rotation just removed.
#[cfg(feature = "3d")]
#[test]
fn a_quad_image_fills_the_parallelogram_and_nothing_around_it() {
    let mut surface = SoftwareSurface::new(Size::new(40, 40), 1.0);
    surface.begin_frame(Color::WHITE);

    // A diamond: the 0..2 source grid mapped onto a square rotated 45 degrees about (20, 20).
    let corners = [Point::new(20, 0), Point::new(40, 20), Point::new(20, 40), Point::new(0, 20)];
    let pixels = [255u8, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255];
    surface.draw_image_quad(corners, 2, 2, &pixels);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    let at = |x: u32, y: u32| {
        let index = ((y * 40 + x) * 4) as usize;
        [rgba[index], rgba[index + 1], rgba[index + 2]]
    };
    assert_eq!(at(20, 20), [255, 0, 0], "the centre of the quad is inside the image");
    assert_eq!(at(2, 2), [255, 255, 255], "a corner outside the diamond is untouched");
    assert_eq!(at(37, 37), [255, 255, 255], "and so is the opposite corner");
}

/// A scaled quad resamples bilinearly, not as blocks.
///
/// The plan for the affine layer specifies bilinear resampling for transformed images. A
/// nearest-neighbour copy would paint only the source's own values, so the evidence is a
/// channel value strictly between the black and white it started from — something no copy can
/// produce.
#[cfg(feature = "3d")]
#[test]
fn a_scaled_quad_image_resamples_bilinearly() {
    let mut surface = SoftwareSurface::new(Size::new(8, 8), 1.0);
    surface.begin_frame(Color::BLACK);

    // 2x2 source, one black column and one white column, stretched over 4x4 pixels.
    let pixels = [
        0u8, 0, 0, 255, 255, 255, 255, 255, // top row: black, white
        0, 0, 0, 255, 255, 255, 255, 255, // bottom row: black, white
    ];
    surface.draw_image_quad(
        [Point::new(0, 0), Point::new(4, 0), Point::new(4, 4), Point::new(0, 4)],
        2,
        2,
        &pixels,
    );
    surface.end_frame();

    let rgba = surface.frame_rgba();
    let red = |x: u32, y: u32| rgba[((y * 8 + x) * 4) as usize];
    assert_eq!(red(0, 0), 0, "the first column is the source's black texel");
    assert_eq!(red(3, 0), 255, "the last column is the source's white texel");
    let blended = red(1, 0);
    assert!(blended > 0 && blended < 255, "a stretched quad must interpolate, got {blended}");
}

/// An image that hangs off the surface and the clip is cropped exactly, not wrapped.
///
/// The draw path intersects the destination with the surface and the clip once and then
/// walks only that rectangle, so the row offsets into the source have to stay in step with
/// the crop. A mistake there shifts or wraps the picture — the class of defect the old
/// per-pixel bounds test could not have.
#[test]
fn a_partly_offscreen_image_is_cropped_to_the_visible_rectangle() {
    let mut surface = SoftwareSurface::new(Size::new(6, 6), 1.0);
    surface.begin_frame(Color::WHITE);
    // 2x2 opaque red image drawn at (-1, -1): only its bottom-right pixel is inside.
    let red = [255u8, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255];
    surface.draw_image(-1, -1, 2, 2, &red);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    let at = |x: u32, y: u32| {
        let index = ((y * 6 + x) * 4) as usize;
        [rgba[index], rgba[index + 1], rgba[index + 2]]
    };
    assert_eq!(at(0, 0), [255, 0, 0], "the one visible pixel is the source's last one");
    assert_eq!(at(1, 0), [255, 255, 255], "nothing is shifted to the right");
    assert_eq!(at(0, 1), [255, 255, 255], "nothing is shifted down");
    assert_eq!(at(5, 5), [255, 255, 255], "the rest of the surface is untouched");
}

/// A clip narrower than the image crops it, and the cropped rows still read the right source
/// bytes.
#[test]
fn an_image_inside_a_clip_reads_the_source_columns_it_covers() {
    let mut surface = SoftwareSurface::new(Size::new(4, 1), 1.0);
    surface.begin_frame(Color::WHITE);
    // 4x1 image: red, green, blue, black — one pixel per column.
    let pixels = [255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255];
    surface.push_clip(1, 0, 2, 1);
    surface.draw_image(0, 0, 4, 1, &pixels);
    surface.pop_clip();
    surface.end_frame();

    let rgba = surface.frame_rgba();
    let at = |x: u32| {
        let index = (x * 4) as usize;
        [rgba[index], rgba[index + 1], rgba[index + 2]]
    };
    assert_eq!(at(0), [255, 255, 255], "outside the clip");
    assert_eq!(at(1), [0, 255, 0], "the clip starts at the source's second column");
    assert_eq!(at(2), [0, 0, 255], "and ends after the third");
    assert_eq!(at(3), [255, 255, 255], "outside the clip");
}

/// An axis-aligned quad is the same picture as a plain image draw.
#[cfg(feature = "3d")]
#[test]
fn an_axis_aligned_quad_matches_a_plain_image_draw() {
    let pixels = [0u8, 128, 0, 255, 0, 128, 0, 255, 0, 128, 0, 255, 0, 128, 0, 255];
    let mut plain = SoftwareSurface::new(Size::new(8, 8), 1.0);
    plain.begin_frame(Color::WHITE);
    plain.draw_image(2, 2, 2, 2, &pixels);
    plain.end_frame();

    let mut quad = SoftwareSurface::new(Size::new(8, 8), 1.0);
    quad.begin_frame(Color::WHITE);
    quad.draw_image_quad(
        [Point::new(2, 2), Point::new(4, 2), Point::new(4, 4), Point::new(2, 4)],
        2,
        2,
        &pixels,
    );
    quad.end_frame();

    assert_eq!(
        plain.frame_rgba(),
        quad.frame_rgba(),
        "an identity transform must not change the pixels"
    );
}

/// A fill covers every byte of the surface, including the short tail.
///
/// The fast path writes sixteen bytes at a time. A length that is not a multiple of four
/// pixels, or of sixteen bytes, is the case a wide-store loop silently drops — which shows up
/// as a stale stripe at the end of the last row rather than as an error.
#[test]
fn fill_pixels_covers_every_length() {
    use crate::render::fill_pixels;
    let color = Color::rgb(10, 20, 30);
    for length in [0usize, 1, 2, 3, 4, 5, 7, 8, 15, 16, 17, 31, 33, 64, 65] {
        let mut pixels = vec![0u8; length];
        fill_pixels(&mut pixels, color);
        for (index, byte) in pixels.iter().enumerate() {
            let expected = [color.r, color.g, color.b, color.a][index % 4];
            assert_eq!(
                *byte, expected,
                "byte {index} of a {length}-byte fill must carry the colour pattern"
            );
        }
    }
}

/// The clear of a whole surface is the pattern repeated with no gaps.
#[test]
fn fill_pixels_fills_a_whole_surface() {
    use crate::render::fill_pixels;
    let color = Color::rgba(1, 2, 3, 4);
    let mut pixels = vec![255u8; 1280 * 960 * 4];
    fill_pixels(&mut pixels, color);
    assert!(
        pixels.as_chunks::<4>().0.iter().all(|pixel| *pixel == [1, 2, 3, 4]),
        "every pixel of the clear must be the requested colour"
    );
}

/// `SetBlendMode` must actually composite on the software backend.
///
/// The backend **stored** the mode and never read it, so a `SetBlendMode { Multiply }` frame
/// rendered as plain source-over on the rasteriser while the SVG snapshot emitted a real
/// `mix-blend-mode` group — one command, two pictures. This asserts the mode reaches the pixels.
///
/// The expected value follows the W3C formula the SVG backend's `mix-blend-mode` also uses:
/// a white backdrop (`1.0`) times a mid grey source (`0x80 / 255`) is `0x80` per channel.
#[test]
fn set_blend_mode_composites_on_the_software_backend() {
    use crate::render::RenderCommand;
    let mut surface = SoftwareSurface::new(Size::new(8, 8), 1.0);
    surface.begin_frame(Color::WHITE);
    surface.set_blend_mode(BlendMode::Multiply);
    surface.fill_rect(Rect::new(0, 0, 4, 8), Color::rgb(128, 128, 128));
    surface.end_frame();

    let rgba = surface.frame_rgba();
    let blended = &rgba[..4];
    assert_eq!(
        (blended[0], blended[1], blended[2]),
        (128, 128, 128),
        "multiply against a white backdrop keeps the source colour"
    );

    // And against a non-white backdrop the two inputs genuinely multiply, which is the part a
    // stored-but-unread field could never produce.
    let mut dark = SoftwareSurface::new(Size::new(8, 8), 1.0);
    dark.begin_frame(Color::rgb(128, 128, 128));
    dark.set_blend_mode(BlendMode::Multiply);
    dark.fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(128, 128, 128));
    dark.end_frame();
    // 0.502 * 0.502 = 0.252 -> 64.
    assert_eq!(dark.frame_rgba()[0], 64, "two mid greys multiply to a quarter of full");

    // `Normal` restores plain source-over, and going back to it must undo the multiply.
    let mut restored = SoftwareSurface::new(Size::new(8, 8), 1.0);
    restored.begin_frame(Color::rgb(128, 128, 128));
    restored.set_blend_mode(BlendMode::Multiply);
    restored.set_blend_mode(BlendMode::Normal);
    restored.fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(128, 128, 128));
    restored.end_frame();
    assert_eq!(restored.frame_rgba()[0], 128, "Normal replaces the backdrop");

    // `SetBlendMode` is frame state and reaches the backend through the `SoftwarePaintBackend`
    // command, not only the direct setter, which is the path a real frame takes.
    use crate::render::PaintBackend;
    let mut via_command = crate::render::SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
    via_command.begin_frame(Color::rgb(128, 128, 128));
    via_command.execute_command(&RenderCommand::SetBlendMode { mode: BlendMode::Multiply });
    via_command.execute_command(&RenderCommand::FillRect {
        rect: Rect::new(0, 0, 8, 8),
        color: Color::rgb(128, 128, 128),
    });
    via_command.end_frame();
    assert_eq!(via_command.surface().frame_rgba()[0], 64, "the command path must set the mode too");
}

#[test]
fn software_surface_fill_rect_zero_size() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.begin_frame(Color::WHITE);
    surface.fill_rect(Rect::new(0, 0, 0, 0), Color::RED);
    surface.end_frame();

    let rgba = surface.frame_rgba();
    for chunk in rgba.chunks(4) {
        assert_eq!(chunk[0], 255); // Still white, no fill
    }
}

#[test]
fn software_surface_draw_rect() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.begin_frame(Color::WHITE);

    surface.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);

    surface.end_frame();

    let rgba = surface.frame_rgba();
    // Top-left corner should be green (stroke edge)
    assert_eq!(rgba[0], 0); // R
    assert_eq!(rgba[1], 255); // G
    assert_eq!(rgba[2], 0); // B
}

#[test]
fn software_surface_resize() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.resize(Size::new(50, 50));
    assert_eq!(surface.size(), Size::new(50, 50));
}

#[test]
fn software_surface_set_dpi_scale() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.set_dpi_scale(1.5);
    assert!((surface.dpi_scale() - 1.5).abs() < 1e-6);
}

#[test]
fn software_surface_aa_samples_per_axis_default() {
    let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    assert_eq!(surface.aa_samples_per_axis(), 4);
}

#[test]
fn software_surface_set_aa_samples() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.set_aa_samples_per_axis(2);
    assert_eq!(surface.aa_samples_per_axis(), 2);
}

#[test]
fn software_surface_apply_render_config_clamps() {
    let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    surface.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 99 });
    assert_eq!(surface.aa_samples_per_axis(), 8);
}

#[test]
fn software_surface_render_config_roundtrip() {
    let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
    let config = surface.render_config();
    assert_eq!(config.aa_samples_per_axis, 4);
}

// ── RenderContext ───────────────────────────────────────────────────

/// Builds a context over a throwaway surface, purely for measuring against.
fn measurement_context<'a>(backend: &'a mut SoftwarePaintBackend) -> RenderContext<'a> {
    RenderContext::new(backend)
}

#[test]
fn text_line_centres_the_line_box_in_the_band() {
    // The defect this primitive removes: `band.y + band.height / 2` places the glyph
    // box's TOP edge on the band's middle line, which draws the label half a line low.
    // Centring is `band.y + (band.height - line_height) / 2`.
    let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
    let ctx = measurement_context(&mut backend);
    let font = Font::default_ui();
    let line_height = ctx.measure_text("M", &font).height as i32;
    assert!(line_height > 0, "the measuring font must report a positive line height");

    let band = Rect::new(0, 0, 240, 120);
    let line = ctx.text_line(band, &font);
    assert_eq!(line.y, (120 - line_height) / 2, "the line box is centred, not top-dropped");
    assert_eq!(line.height, line_height as u32);
    assert_eq!((line.x, line.width), (band.x, band.width), "horizontally unchanged");

    // And it differs from the wrong expression, which is what makes the assertion
    // meaningful rather than a restatement of the implementation.
    assert_ne!(line.y, band.y + band.height as i32 / 2);
}

#[test]
fn text_line_honours_each_vertical_alignment() {
    let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
    let ctx = measurement_context(&mut backend);
    let font = Font::default_ui();
    let line_height = ctx.measure_text("M", &font).height as i32;
    let band = Rect::new(0, 10, 240, 120);

    let top = ctx.text_line_aligned(band, &font, VerticalAlignment::Top);
    assert_eq!(top.y, 10, "top alignment pins the line box to the band's top edge");

    let bottom = ctx.text_line_aligned(band, &font, VerticalAlignment::Bottom);
    assert_eq!(bottom.y, 10 + 120 - line_height);

    let centre = ctx.text_line(band, &font);
    assert_eq!(centre.y, 10 + (120 - line_height) / 2);
    assert!(top.y < centre.y && centre.y < bottom.y, "the three orders must differ");
}

#[test]
fn text_line_survives_degenerate_bands() {
    // A band that cannot hold a line, or has no extent at all, must not panic and must
    // not hand back a negative origin: a negative `y` would be a drawing instruction
    // outside the control, which is a defect rather than a graceful degradation.
    let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
    let ctx = measurement_context(&mut backend);
    let font = Font::default_ui();

    for band in [
        Rect::new(0, 0, 0, 0),
        Rect::new(5, 5, 0, 0),
        Rect::new(0, 0, 100, 1),
        Rect::new(0, 120, 240, 0),
        Rect::new(-30, -30, 10, 4),
    ] {
        let line = ctx.text_line(band, &font);
        assert!(line.y >= band.y, "a clamped band never starts above itself: {band:?}");
        assert_eq!(line.x, band.x);
        assert_eq!(line.width, band.width);
        assert!(line.height >= 1, "a zero-height line box would be un-drawable");
    }
}

#[test]
fn draw_text_line_places_ink_at_the_top_of_a_taller_band() {
    // The end-to-end reading of the contract: ink must land in the band's middle
    // third, not along its top edge. A 120 px band with a 14 px font leaves `line.y`
    // at 53, so row 54 is inside the glyph box and row 2 is not.
    let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
    backend.begin_frame(Color::WHITE);
    let font = Font::default_ui();
    let band = Rect::new(0, 0, 240, 120);
    let line_y;
    {
        let mut ctx = RenderContext::new(&mut backend);
        line_y = ctx.text_line(band, &font).y;
        ctx.draw_text_line_ellipsis(band, "MMMM", &font, Color::BLACK, HorizontalAlignment::Left);
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 240 * 4;
    let row_has_ink = |y: usize| {
        (0..240).any(|x| {
            let idx = y * stride + x * 4;
            rgba[idx] != 255 || rgba[idx + 1] != 255 || rgba[idx + 2] != 255
        })
    };
    assert!(row_has_ink(line_y as usize + 1), "the line box row carries ink");
    assert!(!row_has_ink(2), "the band's top edge must stay clear of a centred line");
}

#[test]
fn render_context_new_wraps_backend() {
    let mut backend = SoftwarePaintBackend::new(Size::new(50, 50), 1.0);
    let ctx = RenderContext::new(&mut backend);
    assert_eq!(ctx.size(), Size::new(50, 50));
}

#[test]
fn render_context_backend_accessor() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    let mut ctx = RenderContext::new(&mut backend);
    let be = ctx.backend();
    assert_eq!(be.size(), Size::new(10, 10));
}

#[test]
fn render_context_push_offset_translates_primitives() {
    let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
    backend.begin_frame(Color::WHITE);
    let offset_after;
    {
        let mut ctx = RenderContext::new(&mut backend);
        // Draw at (2, 2) then translate by (6, 8): fill must land at (8, 10).
        ctx.fill_rect(Rect::new(2, 2, 4, 4), Color::RED);
        ctx.push_offset(6, 8);
        ctx.fill_rect(Rect::new(2, 2, 4, 4), Color::BLUE);
        ctx.pop_offset();
        offset_after = ctx.offset();
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 16 * 4;
    let pixel = |x: usize, y: usize| {
        let idx = y * stride + x * 4;
        (rgba[idx], rgba[idx + 1], rgba[idx + 2])
    };
    // (2,2) contains only the red fill.
    assert_eq!(pixel(3, 3), (255, 0, 0));
    // (8,10) region contains the translated blue fill.
    assert_eq!(pixel(9, 11), (0, 0, 255));
    // Offset is fully restored after pop.
    assert_eq!(offset_after, (0, 0));
}

#[test]
fn render_context_offset_nesting_restores_previous() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.push_offset(1, 2);
    ctx.push_offset(3, 4);
    assert_eq!(ctx.offset(), (4, 6));
    ctx.pop_offset();
    assert_eq!(ctx.offset(), (1, 2));
    ctx.pop_offset();
    assert_eq!(ctx.offset(), (0, 0));
    // Popping an empty stack is a safe no-op.
    ctx.pop_offset();
    assert_eq!(ctx.offset(), (0, 0));
}

#[test]
fn render_context_push_offset_translates_points_lines_and_text() {
    let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_offset(3, 4);
        // Line from (0,0) to (4,4) shifted to (3,4)-(7,8): pixel (5,6) is red.
        ctx.draw_line(Point::new(0, 0), Point::new(8, 8), Color::RED);
        ctx.pop_offset();
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 16 * 4;
    let idx = 6 * stride + 5 * 4;
    assert_eq!(rgba[idx], 255); // R on the translated diagonal
}

#[test]
fn render_context_fill_rect() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
    // Frame is still zero-initialized (not presented yet)
    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 0);

    // After begin/end frame via the backend directly
    let mut backend2 = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend2.begin_frame(Color::WHITE);
    let mut ctx2 = RenderContext::new(&mut backend2);
    ctx2.fill_rect(Rect::new(0, 0, 5, 5), Color::BLUE);
    backend2.end_frame();

    let rgba = backend2.frame_rgba();
    assert_eq!(rgba[0], 0); // R
    assert_eq!(rgba[2], 255); // B
}

#[test]
fn borrowed_non_zero_fill_applies_offset_and_preserves_holes() {
    let points = [
        Point::new(1, 1),
        Point::new(11, 1),
        Point::new(11, 11),
        Point::new(1, 11),
        Point::new(4, 4),
        Point::new(4, 8),
        Point::new(8, 8),
        Point::new(8, 4),
    ];
    let contours = [(0, 4), (4, 8)];
    let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        context.push_offset(2, 2);
        context
            .fill_path_non_zero(&points, &contours, Color::rgb(255, 0, 0))
            .expect("valid borrowed contours should fill");
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let pixel = |x: usize, y: usize| {
        let index = (y * 16 + x) * 4;
        &rgba[index..index + 4]
    };
    assert_eq!(pixel(3, 3), &[255, 0, 0, 255]);
    assert_eq!(pixel(7, 7), &[0, 0, 0, 0]);
}

#[test]
fn render_context_draw_rect() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 0); // R
    assert_eq!(rgba[1], 255); // G
}

#[test]
fn render_context_draw_rect_stroke() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_rect_stroke(Rect::new(0, 0, 10, 10), Color::RED, 2);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 255); // R on edge
}

#[test]
fn render_context_fill_rounded_rect() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.fill_rounded_rect(Rect::new(1, 1, 8, 8), 2, Color::BLUE);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_fill_rounded_rect_aa() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.fill_rounded_rect_aa(Rect::new(1, 1, 8, 8), 2, Color::RED);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

// ── The face path: gradient and radius ───────────────────────────────

/// A declared gradient must change the pixels `face_with_gradient` paints. If the helper still
/// routed to a solid fill, a `LEFT` pixel and a `RIGHT` pixel of a left-to-right black→white
/// ramp would be identical; a gradient makes them differ.
#[test]
fn face_with_gradient_paints_a_ramp_not_a_solid() {
    use crate::core::Point;
    use crate::render::SurfaceStyle;
    use crate::style::Gradient;

    let rect = Rect::new(0, 0, 16, 4);
    let ramp = Gradient::linear(Point::new(0, 0), Point::new(16, 0))
        .add_stop(0.0, Color::BLACK)
        .add_stop(1.0, Color::WHITE);

    let mut backend = SoftwarePaintBackend::new(Size::new(16, 4), 1.0);
    backend.begin_frame(Color::rgb(1, 2, 3));
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.face_with_gradient(
            rect,
            Color::rgb(200, 0, 0),
            Some(&ramp),
            SurfaceStyle::solid(),
            0,
            Color::BLACK,
        );
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 16 * 4;
    let left = rgba[2 * 4];
    let right = rgba[2 * stride + 14 * 4];
    assert!(
        (right as i32 - left as i32).abs() > 100,
        "a left-to-right ramp must differ across the face, got left={left} right={right}"
    );
}

/// `face` is documented as the identity of `face_with_gradient(None)`: a solid radius-0 face
/// must emit exactly the same pixels either way, so teaching controls about gradients cannot
/// have changed what a gradient-free control draws.
#[test]
fn face_is_the_identity_of_face_with_gradient_without_a_gradient() {
    use crate::render::SurfaceStyle;

    let rect = Rect::new(1, 1, 8, 8);
    let fill = Color::rgb(40, 90, 160);

    let mut plain = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    plain.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut plain);
        ctx.face(rect, fill, SurfaceStyle::solid(), 0, Color::BLACK);
    }
    plain.end_frame();

    let mut via_gradient = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    via_gradient.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut via_gradient);
        ctx.face_with_gradient(rect, fill, None, SurfaceStyle::solid(), 0, Color::BLACK);
    }
    via_gradient.end_frame();

    assert_eq!(
        plain.frame_rgba(),
        via_gradient.frame_rgba(),
        "face must be byte-for-byte identical to face_with_gradient(None)"
    );
}

/// The rounded-corner path: a radius must leave the corner un-painted by the face fill, so the
/// backdrop shows through where a square face would have painted. This is the clipping half of
/// the issue, exercised on the shared helper the controls call.
#[test]
fn face_radius_clips_the_corner_of_the_fill() {
    use crate::render::SurfaceStyle;

    let rect = Rect::new(0, 0, 12, 12);
    let fill = Color::rgb(10, 120, 220);

    let mut square = SoftwarePaintBackend::new(Size::new(12, 12), 1.0);
    square.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut square);
        ctx.face(rect, fill, SurfaceStyle::solid(), 0, Color::BLACK);
    }
    square.end_frame();

    let mut rounded = SoftwarePaintBackend::new(Size::new(12, 12), 1.0);
    rounded.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut rounded);
        ctx.face(rect, fill, SurfaceStyle::solid(), 5, Color::BLACK);
    }
    rounded.end_frame();

    // The very top-left pixel is inside a square face and outside a rounded one.
    let square_corner = &square.frame_rgba()[..4];
    let rounded_corner = &rounded.frame_rgba()[..4];
    assert_eq!(&square_corner[..3], &[10, 120, 220], "a square face fills its corner");
    assert_eq!(&rounded_corner[..3], &[255, 255, 255], "a radius must clip the corner");
}

#[test]
fn render_context_draw_rounded_rect_stroke() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_rounded_rect_stroke(Rect::new(1, 1, 8, 8), 2, Color::GREEN, 1);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_draw_rounded_rect_stroke_aa() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_rounded_rect_stroke_aa(Rect::new(1, 1, 8, 8), 2, Color::BLUE, 1);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_draw_line() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_line(Point::new(0, 0), Point::new(9, 9), Color::RED);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 255);
}

#[test]
fn render_context_draw_line_aa() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_line_aa(Point::new(0, 0), Point::new(9, 9), Color::BLUE);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_draw_line_stroke() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_line_stroke(Point::new(0, 0), Point::new(9, 9), Color::GREEN, 2);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[1], 255);
}

#[test]
fn render_context_draw_line_stroke_aa() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_line_stroke_aa(Point::new(0, 0), Point::new(9, 9), Color::RED, 2);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_fill_circle() {
    let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.fill_circle(Point::new(10, 10), 5, Color::BLUE);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 20 * 4;
    let idx = 10 * stride + 10 * 4;
    assert_eq!(rgba[idx + 2], 255); // B
}

#[test]
fn render_context_fill_circle_aa() {
    let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.fill_circle_aa(Point::new(10, 10), 5, Color::RED);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 255);
}

#[test]
fn render_context_draw_circle() {
    let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_circle(Point::new(10, 10), 5, Color::GREEN);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_draw_circle_stroke() {
    let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.draw_circle_stroke(Point::new(10, 10), 5, Color::RED, 2);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_draw_text() {
    let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    let font = Font::simple("Arial", 12.0);
    ctx.draw_text(Point::new(10, 20), "Hello", &font, Color::BLACK, HorizontalAlignment::Left);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

#[test]
fn render_context_measure_text() {
    let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
    let ctx = RenderContext::new(&mut backend);
    let font = Font::simple("Arial", 12.0);
    let metrics = ctx.measure_text("Hello", &font);
    assert!(metrics.width > 0);
    assert!(metrics.height > 0);
}

#[test]
fn render_context_shape_text() {
    let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
    let ctx = RenderContext::new(&mut backend);
    let font = Font::simple("Arial", 12.0);
    let shaped = ctx.shape_text("Hi", &font);
    assert!(!shaped.clusters.is_empty());
}

#[test]
fn render_context_push_clip() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.push_clip(0, 0, 5, 5);
    ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
    ctx.pop_clip();
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 10 * 4;
    let idx = 2 * stride + 2 * 4;
    assert_eq!(rgba[idx], 255); // R inside clip region
}

#[test]
fn render_context_clip_applies_to_non_rect_primitives() {
    let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.push_clip(0, 0, 8, 8);
    ctx.fill_circle(Point::new(6, 6), 6, Color::BLUE);
    ctx.draw_line(Point::new(0, 0), Point::new(19, 19), Color::RED);
    let image = (0..16).flat_map(|_| [0, 255, 0, 255]).collect::<Vec<_>>();
    ctx.draw_image(5, 5, 4, 4, &image);
    ctx.pop_clip();
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 20 * 4;
    let pixel = |x: usize, y: usize| {
        let index = y * stride + x * 4;
        (rgba[index], rgba[index + 1], rgba[index + 2])
    };
    assert_eq!(pixel(6, 6), (0, 255, 0));
    assert_eq!(pixel(7, 7), (0, 255, 0));
    assert_eq!(pixel(15, 15), (255, 255, 255));
    assert_eq!(pixel(10, 10), (255, 255, 255));
}

#[test]
fn render_context_pop_clip() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    ctx.push_clip(0, 0, 5, 5);
    ctx.pop_clip();
    // After pop, subsequent commands are not clipped
    ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let stride = 10 * 4;
    let idx = 7 * stride + 7 * 4; // Outside original clip
    assert_eq!(rgba[idx], 255); // R should be visible
}

#[test]
fn render_context_draw_image() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    backend.begin_frame(Color::WHITE);
    let mut ctx = RenderContext::new(&mut backend);
    let data = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
    ctx.draw_image(0, 0, 2, 2, &data);
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert_eq!(rgba[0], 255); // R from top-left pixel
}

#[test]
fn render_context_size_and_dpi_scale() {
    let mut backend = SoftwarePaintBackend::new(Size::new(80, 60), 1.5);
    let ctx = RenderContext::new(&mut backend);
    assert_eq!(ctx.size(), Size::new(80, 60));
    assert!((ctx.dpi_scale() - 1.5).abs() < 1e-6);
}

// ── SoftwareRenderConfig ─────────────────────────────────────────────

#[test]
fn software_render_config_default_values() {
    let config = SoftwareRenderConfig::default();
    assert_eq!(config.aa_samples_per_axis, 4);
}

#[test]
fn software_render_config_normalized_clamps_low() {
    let config = SoftwareRenderConfig { aa_samples_per_axis: 0 };
    let norm = config.normalized();
    assert_eq!(norm.aa_samples_per_axis, 1);
}

#[test]
fn software_render_config_normalized_clamps_high() {
    let config = SoftwareRenderConfig { aa_samples_per_axis: 100 };
    let norm = config.normalized();
    assert_eq!(norm.aa_samples_per_axis, 8);
}

#[test]
fn software_render_config_normalized_preserves_valid() {
    let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
    let norm = config.normalized();
    assert_eq!(norm.aa_samples_per_axis, 2);
}

#[test]
fn software_render_config_normalized_edge_cases() {
    let config_low = SoftwareRenderConfig { aa_samples_per_axis: 1 };
    assert_eq!(config_low.normalized().aa_samples_per_axis, 1);

    let config_high = SoftwareRenderConfig { aa_samples_per_axis: 8 };
    assert_eq!(config_high.normalized().aa_samples_per_axis, 8);
}

#[test]
fn software_render_config_equality() {
    let a = SoftwareRenderConfig { aa_samples_per_axis: 4 };
    let b = SoftwareRenderConfig { aa_samples_per_axis: 4 };
    let c = SoftwareRenderConfig { aa_samples_per_axis: 2 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ── Global config functions ──────────────────────────────────────────

#[test]
fn default_software_render_config_returns_default() {
    let _guard = SoftwareRenderConfigTestGuard::new();
    set_default_software_render_config(SoftwareRenderConfig::default());
    let config = default_software_render_config();
    assert_eq!(config.aa_samples_per_axis, 4);
}

#[test]
fn set_default_software_render_config_updates_global() {
    let _guard = SoftwareRenderConfigTestGuard::new();
    let custom = SoftwareRenderConfig { aa_samples_per_axis: 2 };
    set_default_software_render_config(custom);
    let retrieved = default_software_render_config();
    assert_eq!(retrieved.aa_samples_per_axis, 2);
}

#[test]
fn set_default_software_render_config_clamps() {
    let _guard = SoftwareRenderConfigTestGuard::new();
    let custom = SoftwareRenderConfig { aa_samples_per_axis: 99 };
    set_default_software_render_config(custom);
    let retrieved = default_software_render_config();
    assert_eq!(retrieved.aa_samples_per_axis, 8);
}

// ── PaintBackend integration via RenderContext ───────────────────────

#[test]
fn render_context_dpi_scale_propagates() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 2.0);
    let ctx = RenderContext::new(&mut backend);
    assert!((ctx.dpi_scale() - 2.0).abs() < 1e-6);
}

#[test]
fn render_context_backend_mutability() {
    let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rect(Rect::new(0, 0, 5, 5), Color::RED);
    }
    // After ctx drops, backend can still be used
    backend.begin_frame(Color::WHITE);
    backend.execute_command(&RenderCommand::FillRect {
        rect: Rect::new(0, 0, 10, 10),
        color: Color::BLUE,
    });
    backend.end_frame();

    let rgba = backend.frame_rgba();
    assert!(!rgba.is_empty());
}

// ── D09-RENDER-03: text fitting must not re-shape every prefix ──────────

/// A [`PaintBackend`] that forwards to a real software backend while counting how many
/// times `measure_text`/`shape_text` are called.
///
/// # Why counting calls is the right observable
///
/// The defect is an *algorithmic* one — the fitter re-shaped each candidate prefix, so the
/// number of shaping calls grew with the square of the clusters that fit. Wall-clock timing
/// on a shared CI machine is not a stable way to see that; the number of shaping calls is a
/// direct, deterministic witness of the complexity, so the test asserts on it.
struct CountingBackend<'a> {
    pub(crate) inner: &'a SoftwarePaintBackend,
    pub(crate) shapes: core::cell::Cell<usize>,
    pub(crate) measures: core::cell::Cell<usize>,
}

impl CountingBackend<'_> {
    fn shapes(&self) -> usize {
        self.shapes.get()
    }

    fn measures(&self) -> usize {
        self.measures.get()
    }
}

impl PaintBackend for CountingBackend<'_> {
    fn begin_frame(&mut self, _clear: Color) {}
    fn end_frame(&mut self) {}
    fn execute_command(&mut self, _command: &RenderCommand) {}
    fn size(&self) -> Size {
        self.inner.size()
    }
    fn set_size(&mut self, _size: Size) {}
    fn dpi_scale(&self) -> f32 {
        self.inner.dpi_scale()
    }
    fn set_dpi_scale(&mut self, _dpi_scale: f32) {}
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.measures.set(self.measures.get() + 1);
        self.inner.measure_text(text, font)
    }
    fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        self.shapes.set(self.shapes.get() + 1);
        self.inner.shape_text(text, font)
    }
    fn frame_rgba(&self) -> &[u8] {
        self.inner.frame_rgba()
    }
}

fn counting_backend() -> SoftwarePaintBackend {
    let mut backend = SoftwarePaintBackend::new(Size::new(400, 40), 1.0);
    backend.begin_frame(Color::WHITE);
    backend
}

/// `fit_text_to_width` must find the split point in a bounded number of shaping calls,
/// independent of how many clusters fit — the old loop shaped once per accepted cluster.
#[test]
fn fitting_a_long_label_shapes_bounded_times() {
    let backend = counting_backend();
    let counter = CountingBackend {
        inner: &backend,
        shapes: core::cell::Cell::new(0),
        measures: core::cell::Cell::new(0),
    };
    let font = Font::default();

    // A label long enough that a narrow budget keeps many clusters. The width fits roughly
    // 10 ems of this font, so the old loop would have shaped ~10 times (and copied each
    // prefix); the new one shapes once.
    let long = "The quick brown fox jumps over the lazy dog repeatedly";
    let _fitted = fit_text_to_width(long, 60.0, &font, &counter);

    assert!(
        counter.shapes() <= 1,
        "fitting must shape the source at most once, but shaped {} times",
        counter.shapes()
    );
}

/// A label that already fits must measure once and hand that width back, so the draw path
/// does not repeat the same backend measurement to centre it.
#[test]
fn a_fitting_label_reports_its_measured_width_without_a_second_measurement() {
    let backend = counting_backend();
    let counter = CountingBackend {
        inner: &backend,
        shapes: core::cell::Cell::new(0),
        measures: core::cell::Cell::new(0),
    };
    let font = Font::default();
    let text = "OK";
    let expected = backend.measure_text(text, &font).width as f32;

    let (fitted, width) = fit_text_to_width_measured(text, 1000.0, &font, &counter);

    assert_eq!(fitted, text);
    assert_eq!(width, Some(expected), "the fit check's own measurement is the advance");
    assert_eq!(counter.measures(), 1, "the width must not be measured twice");
    assert_eq!(counter.shapes(), 0, "a label that fits needs no shaping");
}

/// The number of shaping calls must not grow as the kept prefix grows: doubling the budget
/// (which keeps roughly twice as many clusters) must not double the work.
#[test]
fn fitting_steps_do_not_grow_with_the_kept_prefix() {
    let backend = counting_backend();
    let font = Font::default();
    let long = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let measure = |max_width: f32| {
        let counter = CountingBackend {
            inner: &backend,
            shapes: core::cell::Cell::new(0),
            measures: core::cell::Cell::new(0),
        };
        let fitted = fit_text_to_width(long, max_width, &font, &counter);
        (counter.shapes(), fitted.chars().count())
    };

    let (narrow_shapes, narrow_len) = measure(40.0);
    let (wide_shapes, wide_len) = measure(200.0);

    assert!(wide_len > narrow_len, "the wider budget must keep more clusters");
    assert!(
            wide_shapes <= narrow_shapes && wide_shapes <= 1,
            "shaping calls must stay bounded as the kept prefix grows: narrow={narrow_shapes}, wide={wide_shapes}"
        );
}

/// The linear fitter must produce the same fitted string as a straightforward reference
/// implementation, across Latin, CJK, combining characters and exact-boundary budgets.
///
/// This is the pixel-equivalence half of the fix: a faster search that dropped or added a
/// cluster would still return *a* string, so only comparing against the reference catches it.
#[test]
fn linear_fit_matches_a_prefix_walk_for_all_scripts() {
    let backend = counting_backend();
    let font = Font::default();

    // Byte-for-byte reference of the original per-prefix algorithm.
    fn reference(text: &str, max_width: f32, font: &Font, backend: &dyn PaintBackend) -> String {
        if text.is_empty() || max_width <= 0.0 {
            return String::new();
        }
        if backend.measure_text(text, font).width as f32 <= max_width {
            return text.to_string();
        }
        const ELLIPSIS: char = '\u{2026}';
        let ellipsis_width = backend.measure_text(&ELLIPSIS.to_string(), font).width as f32;
        if ellipsis_width > max_width {
            return ELLIPSIS.to_string();
        }
        let budget = max_width - ellipsis_width;
        let mut kept = String::new();
        for ch in text.chars() {
            let mut next = kept.clone();
            next.push(ch);
            if backend.measure_text(&next, font).width as f32 > budget {
                break;
            }
            kept = next;
        }
        kept.push(ELLIPSIS);
        kept
    }

    let samples = [
        "The quick brown fox jumps over the lazy dog",
        "中文标签测试文本内容在这个控件里显示",
        "e\u{301}cole cafe\u{301} nai\u{308}ve re\u{301}sume\u{301}",
        "short",
        "oneverylongsinglewordwithoutanyspacesatall",
    ];
    // A range of budgets exercises the exact boundary, the ellipsis-only case and the fits
    // case. The step per sample is small so several boundaries are hit for each string.
    for sample in samples {
        let mut width = 0.0f32;
        while width <= 240.0 {
            let expected = reference(sample, width, &font, &backend);
            let actual = fit_text_to_width(sample, width, &font, &backend);
            assert_eq!(
                actual, expected,
                "fitted text differs from the prefix walk at width {width} for {sample:?}"
            );
            width += 3.0;
        }
    }
}

/// The ellipsis-only and no-fit boundaries are preserved by the linear path.
#[test]
fn fit_boundaries_match_the_prefix_walk() {
    let backend = counting_backend();
    let font = Font::default();
    let text = "hello world";

    // A budget that fits the whole string returns it unchanged.
    assert_eq!(fit_text_to_width(text, 1000.0, &font, &backend), text);

    // A budget too small for even the ellipsis yields just the ellipsis.
    assert_eq!(fit_text_to_width(text, 1.0, &font, &backend), "\u{2026}");

    // A non-positive budget yields an empty string, not an ellipsis.
    assert_eq!(fit_text_to_width(text, 0.0, &font, &backend), "");
}

// ── D09-RENDER-02: image draws must not copy the source on the immediate path ──

/// Building a 2x2 image of four distinct opaque pixels, so a wrong stride or channel order
/// shows up as a colour mismatch rather than as an all-same buffer.
fn distinct_image() -> ([u8; 16], [(u8, u8, u8, u8); 4]) {
    let pixels = [(255u8, 0u8, 0u8, 255u8), (0, 255, 0, 255), (0, 0, 255, 255), (255, 255, 0, 255)];
    let mut data = [0u8; 16];
    for (index, px) in pixels.iter().enumerate() {
        data[index * 4] = px.0;
        data[index * 4 + 1] = px.1;
        data[index * 4 + 2] = px.2;
        data[index * 4 + 3] = px.3;
    }
    (data, pixels)
}

/// The borrowed draw must land the same pixels as the owning-command draw.
#[test]
fn borrowed_draw_image_matches_the_command_path_pixel_for_pixel() {
    let (data, pixels) = distinct_image();
    let size = Size::new(8, 8);

    // Reference: build an owning DrawImage command and execute it, exactly as the old
    // `RenderContext::draw_image` did.
    let mut reference = SoftwarePaintBackend::new(size, 1.0);
    reference.begin_frame(Color::BLACK);
    reference.execute_command(&RenderCommand::DrawImage {
        x: 2,
        y: 3,
        width: 2,
        height: 2,
        data: Arc::from(data),
    });
    reference.end_frame();
    let expected = reference.frame_rgba().to_vec();

    // Borrowed: the same draw through the context, which now takes the no-copy path.
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    backend.begin_frame(Color::BLACK);
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_image(2, 3, 2, 2, &data);
    }
    backend.end_frame();

    assert_eq!(backend.frame_rgba(), expected.as_slice(), "borrowed and owning draws must agree");

    // And the pixels really are the source's, at the requested offset.
    let stride = 8 * 4;
    for (index, px) in pixels.iter().enumerate() {
        let (x, y) = (2 + (index % 2), 3 + (index / 2));
        let at = y * stride + x * 4;
        let got = backend.frame_rgba();
        assert_eq!(
            (got[at], got[at + 1], got[at + 2], got[at + 3]),
            *px,
            "pixel {index} must come from the source at ({x}, {y})"
        );
    }
}

/// The immediate path must draw from the borrowed slice rather than building an owning
/// command — this is the copy D09-RENDER-02 removes.
#[test]
fn the_immediate_path_draws_the_source_without_copying_it() {
    let (data, _) = distinct_image();
    let mut backend = SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
    backend.begin_frame(Color::BLACK);

    // Reset the counter, then draw through the context (the immediate path).
    let _ = take_image_bytes_drawn_without_copy();
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_image(0, 0, 2, 2, &data);
    }
    let drawn = take_image_bytes_drawn_without_copy();

    assert_eq!(
        drawn,
        data.len(),
        "the software backend must draw the full source through the no-copy path"
    );
    backend.end_frame();
}

/// Glyph-level text paints a shaped run and turns a ligature on for the whole surface.
///
/// The first half asserts the shaping engine really merged a cluster when the face has the
/// ligature; if the active face does not, the test reports the skip rather than pretending a
/// face applies a feature it does not have. The second half paints the run, which is the new
/// path, and checks that ink reached the surface.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex"))]
#[test]
fn glyph_run_text_paints_the_shapers_ligature() {
    use crate::render::text::font_assets::active_faces;

    let Some(face) = active_faces().first().copied() else {
        eprintln!("skipping glyph-run test: no vector face in this build");
        return;
    };
    let Some(shaper) = crate::render::RustybuzzShaper::new(face) else {
        eprintln!("skipping glyph-run test: the face did not parse");
        return;
    };
    let plain = shaper.shape_face("fi", 16.0, 1.0).expect("the face shapes its text");
    let ligated =
        shaper.shape_face_with_ligatures("fi", 16.0, 1.0).expect("the face shapes its text");
    if ligated.glyph_ids.len() >= plain.glyph_ids.len() {
        eprintln!("skipping glyph-run test: {} has no `fi` ligature", face.name);
        return;
    }

    let font = Font::simple(face.name, 16.0);
    let mut surface = SoftwareSurface::new(Size::new(80, 32), 1.0);
    surface.begin_frame(Color::WHITE);
    assert!(!surface.glyph_run_text(), "the glyph path is opt-in");
    surface.set_glyph_run_text(true);
    assert!(surface.glyph_run_text());
    surface.draw_text(Point::new(2, 2), "fi", &font, Color::BLACK, HorizontalAlignment::Left);
    surface.end_frame();
    let frame = surface.frame_rgba();
    assert!(
        frame.as_chunks::<4>().0.iter().any(|pixel| pixel[0] < 200),
        "the shaped glyph run must leave ink on the surface"
    );
}

/// A family the build does not ship keeps the cluster renderer, with identical pixels.
///
/// This is what makes the glyph path safe to expose: turning it on for a theme whose family is
/// not one of the compiled faces changes nothing at all.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex"))]
#[test]
fn glyph_run_text_falls_back_for_an_unknown_family() {
    let font = Font::simple("no-such-family", 16.0);
    let mut plain = SoftwareSurface::new(Size::new(64, 24), 1.0);
    plain.begin_frame(Color::WHITE);
    plain.draw_text(Point::new(2, 2), "ffi", &font, Color::BLACK, HorizontalAlignment::Left);
    plain.end_frame();

    let mut glyphs = SoftwareSurface::new(Size::new(64, 24), 1.0);
    glyphs.begin_frame(Color::WHITE);
    glyphs.set_glyph_run_text(true);
    glyphs.draw_text(Point::new(2, 2), "ffi", &font, Color::BLACK, HorizontalAlignment::Left);
    glyphs.end_frame();

    assert_eq!(
        plain.frame_rgba(),
        glyphs.frame_rgba(),
        "an unavailable face must fall back to the cluster path byte for byte"
    );
}

/// A projectively divided quad gets edge coverage from its projected edge.
///
/// The right edge runs from `(10, 0)` to `(9, 10)`, so the pixel at `(9, 5)` is only about
/// half covered. A bounding-box fill (or a hard-edged polygon) would paint it either fully or
/// not at all; a value between the fill colour and the white backdrop is what proves the
/// coverage comes from the edge.
#[cfg(feature = "3d")]
#[test]
fn a_projectively_divided_quad_antialiases_its_projected_edge() {
    let mut surface = SoftwareSurface::new(Size::new(12, 12), 1.0);
    surface.begin_frame(Color::WHITE);
    surface.fill_quad_aa(
        [Point::new(0, 0), Point::new(10, 0), Point::new(9, 10), Point::new(0, 10)],
        Color::RED,
    );
    surface.end_frame();

    let frame = surface.frame_rgba();
    let at = |x: u32, y: u32| {
        let index = ((y * 12 + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    };
    assert_eq!(at(2, 5), [255, 0, 0, 255], "the interior is fully covered");
    let edge = at(9, 5);
    assert_eq!(edge[0], 255, "the red channel is the fill's");
    assert!(
        edge[1] > 0 && edge[1] < 255,
        "a half-covered edge pixel must blend with the backdrop, got {edge:?}"
    );
    assert_eq!(at(11, 5), [255, 255, 255, 255], "outside the quad stays untouched");
}

/// A general (non-parallelogram) quad maps its texture with a homography.
///
/// The destination is a trapezoid: the top edge spans x 6..14 and the bottom edge spans 2..18.
/// A row near the top is therefore blank at `x = 4` while the same column is painted near the
/// bottom. An affine/parallelogram inverse would paint the full width on both rows.
#[test]
fn a_projective_quad_image_follows_its_narrow_edge() {
    let mut surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
    surface.begin_frame(Color::WHITE);
    // 2x2 source: left column red, right column blue.
    let pixels = [
        255u8, 0, 0, 255, 0, 0, 255, 255, //
        255, 0, 0, 255, 0, 0, 255, 255,
    ];
    surface.draw_image_quad(
        [Point::new(6, 2), Point::new(14, 2), Point::new(18, 18), Point::new(2, 18)],
        2,
        2,
        &pixels,
    );
    surface.end_frame();

    let frame = surface.frame_rgba();
    let at = |x: u32, y: u32| {
        let index = ((y * 20 + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    };
    assert_eq!(at(4, 3), [255, 255, 255, 255], "outside the narrow top edge");
    let wide = at(3, 17);
    assert!(
        wide[0] > 0 || wide[2] > 0,
        "the wide bottom edge must reach this column, got {wide:?}"
    );
    let middle = at(10, 10);
    assert!(
        middle[0] > 0 && middle[2] > 0,
        "the middle samples both source columns, got {middle:?}"
    );
}

// ── G-014: the shared dash kernel ───────────────────────────────────

/// The dash kernel reproduces the border walk's output on the inputs the
/// border feeds it.
///
/// `draw_edge_pattern` used to inline the mark/gap walk. The kernel replaced
/// it, and this test is the evidence the replacement changed nothing: on
/// every axis-aligned edge (the only kind a rounded rect's straight section
/// has) and several mark/gap periods, the kernel's segments equal the
/// historical loop's, byte for byte, including the truncating interpolation
/// both use.
#[test]
fn the_dash_kernel_matches_the_historical_border_walk() {
    fn legacy_walk(from: (i32, i32), to: (i32, i32), mark: i64, gap: i64) -> Vec<(Point, Point)> {
        let (dx, dy) = ((to.0 - from.0) as i64, (to.1 - from.1) as i64);
        let length = dx.abs().max(dy.abs());
        if length <= 0 {
            return Vec::new();
        }
        let point_at = |t: i64| {
            Point::new(from.0 + (dx * t / length) as i32, from.1 + (dy * t / length) as i32)
        };
        let mut out = Vec::new();
        let mut position = 0i64;
        while position < length {
            let end = (position + mark).min(length);
            out.push((point_at(position), point_at(end)));
            position = end + gap;
        }
        out
    }
    let edges = [((0, 0), (40, 0)), ((10, 5), (10, 60)), ((30, 30), (30, 31)), ((0, 0), (1, 0))];
    for &(mark, gap) in &[(3i64, 2i64), (1, 1), (7, 3)] {
        for &(from, to) in &edges {
            let kernel = dash_line_segments(
                &[Point::new(from.0, from.1), Point::new(to.0, to.1)],
                false,
                crate::render::core::command::DashPattern { mark: mark as u32, gap: gap as u32 },
            );
            let legacy = legacy_walk(from, to, mark, gap);
            assert_eq!(
                kernel, legacy,
                "kernel and historical walk disagree on {from:?}->{to:?} mark {mark} gap {gap}"
            );
        }
    }
}

/// A zero mark produces no segments: no ink is possible without a mark.
#[test]
fn a_zero_mark_dashes_nothing() {
    let segments = dash_line_segments(
        &[Point::new(0, 0), Point::new(40, 0)],
        false,
        crate::render::core::command::DashPattern { mark: 0, gap: 4 },
    );
    assert!(segments.is_empty(), "a zero mark cannot carry ink");
}

/// A dashed path draws strictly less ink than the same path solid.
#[test]
fn a_dashed_path_draws_less_ink_than_a_solid_one() {
    let ink = |dashed: bool| {
        let mut backend = SoftwarePaintBackend::new(Size::new(60, 60), 1.0);
        backend.begin_frame(Color::WHITE);
        let points = [Point::new(10, 10), Point::new(50, 10)];
        if dashed {
            backend.execute_command(&RenderCommand::DrawPathDashed {
                points: points.into(),
                closed: false,
                color: Color::RED,
                width: 2,
                dash: crate::render::core::command::DashPattern { mark: 4, gap: 4 },
            });
        } else {
            backend.execute_command(&RenderCommand::DrawPath {
                points: points.into(),
                closed: false,
                color: Color::RED,
                filled: false,
                width: 2,
            });
        }
        backend.end_frame();
        let frame = backend.frame_rgba();
        frame.as_chunks::<4>().0.iter().filter(|px| **px != [255, 255, 255, 255]).count()
    };
    assert!(ink(true) < ink(false), "dashes must leave gaps the solid stroke fills");
    assert!(ink(true) > 0, "dashes must still carry ink");
}

// ── G-013: text decoration ────────────────────────────────────────

/// Counts non-white pixels, i.e. everything either text or a rule drew.
fn opaque_ink(frame: &[u8]) -> usize {
    frame.as_chunks::<4>().0.iter().filter(|px| **px != [255, 255, 255, 255]).count()
}

/// An underline draws a rule below the baseline that plain text (without
/// descenders) leaves blank, and `NONE` draws exactly what `DrawText` does.
#[test]
fn an_underlined_text_adds_a_rule_below_the_baseline() {
    let font = Font::default_ui();
    let metrics = TextMetrics::for_font(&font, 1.0);
    let origin = Point::new(4, 4);
    let draw = |decoration: crate::render::core::command::TextDecoration| {
        let mut backend = SoftwarePaintBackend::new(Size::new(120, 40), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::DrawTextDecorated {
            origin,
            text: "HI".into(),
            font: font.clone(),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
            decoration,
        });
        backend.end_frame();
        backend.frame_rgba().to_vec()
    };
    let plain = draw(crate::render::core::command::TextDecoration::NONE);
    let underlined = draw(crate::render::core::command::TextDecoration::UNDERLINE);
    assert!(opaque_ink(&underlined) > opaque_ink(&plain), "an underline must add ink");
    // The rule's own row: baseline plus the underline offset, which plain
    // "HI" (no descenders) never touches.
    let at = |frame: &[u8], x: u32, y: u32| {
        let index = ((y * 120 + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2]]
    };
    let baseline = origin.y + metrics.ascent as i32;
    let rule_y = (baseline + (metrics.descent as i32 / 2).max(1)) as u32;
    let run_ink = (0..80u32).filter(|x| at(&underlined, *x, rule_y)[0] < 255).count();
    assert!(
        run_ink >= 8,
        "the underline must span the run at y={rule_y}, found {run_ink} inked columns"
    );
    assert!(
        (0..80u32).all(|x| at(&plain, x, rule_y) == [255, 255, 255]),
        "plain text with no descenders must leave the rule's row blank"
    );
}

/// A wavy underline zigzags below the baseline: ink appears at the trough
/// row, which a straight underline never reaches.
#[test]
fn a_wavy_underline_reaches_below_a_straight_rule() {
    let font = Font::default_ui();
    let metrics = TextMetrics::for_font(&font, 1.0);
    let origin = Point::new(4, 4);
    let mut backend = SoftwarePaintBackend::new(Size::new(120, 40), 1.0);
    backend.begin_frame(Color::WHITE);
    backend.execute_command(&RenderCommand::DrawTextDecorated {
        origin,
        text: "HI".into(),
        font: font.clone(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
        decoration: crate::render::core::command::TextDecoration::WAVY,
    });
    backend.end_frame();
    let frame = backend.frame_rgba().to_vec();
    let at = |x: u32, y: u32| {
        let index = ((y * 120 + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2]]
    };
    let baseline = origin.y + metrics.ascent as i32;
    let trough = (baseline + (metrics.descent as i32 / 2).max(1) * 2) as u32;
    let run_ink = (0..80u32).filter(|x| at(*x, trough)[0] < 255).count();
    assert!(run_ink > 0, "the wave's troughs must ink y={trough}, found none");
}

// ── G-015: rounded clip ────────────────────────────────────────────

/// A rounded clip excludes what a rectangle clip includes: the corner pixel
/// inside the bounding box but outside the radius stays untouched, while
/// the shape's interior and edge midpoints fill normally. A `PopClip`
/// releases the mask, and an empty radius list behaves as a rectangle.
#[test]
fn a_rounded_clip_excludes_the_corners_a_rectangle_keeps() {
    let fill_command = RenderCommand::FillRect { rect: Rect::new(5, 5, 30, 30), color: Color::RED };
    let frame_with = |clip: &RenderCommand| {
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(clip);
        backend.execute_command(&fill_command);
        backend.execute_command(&RenderCommand::PopClip);
        backend.end_frame();
        let frame = backend.frame_rgba().to_vec();
        move |x: u32, y: u32| {
            let index = ((y * 40 + x) * 4) as usize;
            [frame[index], frame[index + 1], frame[index + 2]]
        }
    };
    let rounded = frame_with(&RenderCommand::PushClipRoundedRect {
        x: 5,
        y: 5,
        width: 30,
        height: 30,
        radii: [10, 10, 10, 10],
    });
    let square = frame_with(&RenderCommand::PushClip { x: 5, y: 5, width: 30, height: 30 });
    // Inside the bounding box but outside the corner radius (arc centre is
    // (15,15), distance from (6,6) is ~12.7 > 10).
    assert_eq!(rounded(6, 6), [255, 255, 255], "the corner must be clipped away");
    assert_eq!(square(6, 6), [255, 0, 0], "the rectangle clip keeps the same corner");
    // The interior and a straight-section point on the left edge fill. The
    // probes sit one pixel inside the outline: x = 5 lies exactly on the
    // polygon's left edge, and an on-boundary sample has no inside to
    // belong to, so the coverage rule gives it nothing.
    assert_eq!(rounded(15, 20), [255, 0, 0], "the interior fills");
    assert_eq!(rounded(6, 20), [255, 0, 0], "the edge's straight section fills");
}

// ── G-016: rounded box shadow ──────────────────────────────────────

/// A rounded shadow follows the caster's corners: the pixel inside the
/// bounding box but outside the radius stays clean, where the plain command
/// paints it. This contrast is the defect G-016 fixes.
#[test]
fn a_rounded_shadow_leaves_the_corners_a_plain_shadow_fills() {
    let shadow = Color::rgba(0, 0, 0, 255);
    let frame_with = |command: &RenderCommand| {
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(command);
        backend.end_frame();
        let frame = backend.frame_rgba().to_vec();
        move |x: u32, y: u32| {
            let index = ((y * 40 + x) * 4) as usize;
            [frame[index], frame[index + 1], frame[index + 2]]
        }
    };
    let rounded = frame_with(&RenderCommand::BoxShadowRounded {
        rect: Rect::new(10, 10, 20, 20),
        radii: [8, 8, 8, 8],
        color: shadow,
        offset_x: 0,
        offset_y: 0,
        blur_radius: 0,
        spread: 0,
    });
    let plain = frame_with(&RenderCommand::BoxShadow {
        rect: Rect::new(10, 10, 20, 20),
        color: shadow,
        offset_x: 0,
        offset_y: 0,
        blur_radius: 0,
        spread: 0,
    });
    // The top edge's midpoint is shadow in both.
    assert_eq!(rounded(20, 10), [0, 0, 0], "the edge midpoint is shadow");
    assert_eq!(plain(20, 10), [0, 0, 0], "the edge midpoint is shadow");
    // Inside the bounding box, outside the corner radius (arc centre
    // (18,18), distance from (11,11) is ~9.9 > 8).
    assert_eq!(rounded(11, 11), [255, 255, 255], "the corner must stay clean");
    assert_eq!(plain(11, 11), [0, 0, 0], "the plain shadow fills the same corner");
}

/// G-802: a recorded path/label carries arena indices, and the flush paints them without an
/// owning payload. This is the observable form of the change: the queued command is the
/// recorder variant, and the pixels still arrive.
#[test]
fn recorded_paths_and_labels_are_resolved_from_the_frame_arenas() {
    let mut backend = SoftwarePaintBackend::new(Size::new(32, 32), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut ctx = RenderContext::new(&mut backend);
        ctx.begin_region_buckets(&[Rect::new(0, 0, 32, 32)]);
        ctx.draw_path(
            &[Point::new(2, 2), Point::new(20, 2), Point::new(20, 20), Point::new(2, 20)],
            true,
            Color::BLACK,
            true,
            1,
        );
        ctx.draw_text(
            Point::new(2, 28),
            "A",
            &Font::default(),
            Color::BLACK,
            crate::core::HorizontalAlignment::Left,
        );
        let recorded = ctx
            .buckets
            .as_ref()
            .expect("the traversal is recording")
            .groups
            .iter()
            .flat_map(|group| &group.commands)
            .filter(|command| {
                matches!(
                    command,
                    RenderCommand::DrawPathRecorded { .. } | RenderCommand::DrawTextRecorded { .. }
                )
            })
            .count();
        assert_eq!(recorded, 2, "the path and the label live in the arenas");
        ctx.end_region_buckets();
    }
    backend.end_frame();
    let rgba = backend.frame_rgba();
    assert!(
        rgba.as_chunks::<4>().0.iter().any(|pixel| pixel[0] < 128),
        "the recorded filled path painted ink"
    );
    assert!(
        rgba.as_chunks::<4>().0.iter().filter(|pixel| pixel[0] < 128).count() > 50,
        "the recorded path covers an area, not a single pixel"
    );
}

/// RD6-01: the recording path shares one `Arc` per unchanged buffer, and never serves stale
/// pixels for a buffer that was rewritten in place.
///
/// This is the observable form of the change: a bucketed frame used to copy the whole image
/// into a fresh `Arc` on every draw. The second frame must reuse the allocation (that identity
/// is also what lets the GPU backend find the texture it already uploaded), and a buffer whose
/// contents changed under the same address must **not** hit.
#[test]
fn a_recorded_image_is_shared_across_frames_and_rechecked_against_the_buffer() {
    fn record(pixels: &[u8]) -> crate::compat::Arc<[u8]> {
        let mut backend = SoftwarePaintBackend::new(Size::new(4, 4), 1.0);
        backend.begin_frame(Color::WHITE);
        let recorded = {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.begin_region_buckets(&[Rect::new(0, 0, 4, 4)]);
            ctx.draw_image(0, 0, 2, 2, pixels);
            let recorded = ctx
                .buckets
                .as_ref()
                .expect("the traversal is recording")
                .groups
                .iter()
                .flat_map(|group| &group.commands)
                .find_map(|command| match command {
                    RenderCommand::DrawImage { data, .. } => Some(crate::compat::Arc::clone(data)),
                    _ => None,
                })
                .expect("the image was recorded as an owning command");
            ctx.end_region_buckets();
            recorded
        };
        backend.end_frame();
        recorded
    }

    let mut pixels = [10u8, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 1, 2, 3, 255];
    let first = record(&pixels);
    let second = record(&pixels);
    assert!(
        crate::compat::Arc::ptr_eq(&first, &second),
        "an unchanged buffer must reuse the recorded allocation"
    );

    // A rewritten buffer is a different picture, not a stale hit on the same address.
    pixels[0] = 200;
    let third = record(&pixels);
    assert!(
        !crate::compat::Arc::ptr_eq(&second, &third),
        "a rewritten buffer must be recorded again"
    );
    assert_eq!(&third[..4], &[200, 20, 30, 255], "the recorded bytes follow the buffer");
}
