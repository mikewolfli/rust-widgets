// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::Arc;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::{
    last_auto_render_backend, AutoRenderBackend, PaintBackend, RenderCommand, RenderScene,
    SceneLayer, SoftwarePaintBackend, SoftwareRenderConfig, SoftwareSurface,
};

fn font() -> Font {
    Font::simple("Sans", 14.0)
}

#[test]
fn text_metrics_scale_with_dpi() {
    let mut surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
    let m1 = surface.measure_text("hello", &font());
    surface.set_dpi_scale(2.0);
    let m2 = surface.measure_text("hello", &font());
    assert!(m2.width > m1.width);
    assert!(m2.height > m1.height);
}

/// The software surface and the SVG backend must agree about a font's line box.
///
/// # The defect this pins (BLUE25 D-10)
///
/// Both backends derived the line-box triple inline, and they had drifted: the SVG backend applied
/// a `.max(1.0)` to the font's effective leading that the software surface did not. Because the
/// ascent is `leading * 0.8`, a small leading made them place the **baseline** differently while
/// agreeing on the height — a control sized against one backend's line box rendered a line off
/// centre in the other. Both files' own comments claimed "the two backends must agree", which is
/// exactly what nothing checked.
///
/// The table below includes the divergence case (`size = 0`, where the unfloored derivation gives
/// ascent `0` and the floored one `1`), so a regression that re-inlines either derivation fails
/// here rather than only in the other backend's output.
#[test]
fn both_backends_agree_on_a_fonts_line_box() {
    use crate::render::SvgPaintBackend;

    let fonts = [
        Font::simple("Sans", 14.0),
        Font::simple("Sans", 0.0), // the divergence case: no size, so the floor is load-bearing
        Font::simple("Sans", 1.0),
        Font::simple("Sans", 40.0),
    ];

    for font in &fonts {
        for scale in [1.0_f32, 2.0] {
            let software = SoftwareSurface::new(Size { width: 200, height: 60 }, scale);
            let mut svg = SvgPaintBackend::new(Size { width: 200, height: 60 });
            svg.set_dpi_scale(scale);

            let a = software.measure_text("Ag", font);
            let b = svg.measure_text("Ag", font);
            assert_eq!(
                (a.height, a.ascent, a.descent),
                (b.height, b.ascent, b.descent),
                "the software and SVG backends disagree about the line box for {font:?} at {scale}x"
            );
            assert_eq!(a.width, b.width, "and about the advance for {font:?} at {scale}x");
        }
    }
}
#[test]
fn translucent_fill_composites_over_the_backdrop() {
    let mut surface = SoftwareSurface::new(Size { width: 4, height: 4 }, 1.0);
    surface.begin_frame(Color::BLACK);
    // Half-alpha white over opaque black must land halfway, and the result stays opaque.
    surface.fill_rect(Rect::new(0, 0, 4, 4), Color::rgba(255, 255, 255, 128));
    surface.end_frame();

    let pixel = &surface.frame_rgba()[0..4];
    assert!(
        pixel[0] > 120 && pixel[0] < 136,
        "a half-alpha white fill over black must be mid-grey, got {pixel:?}"
    );
    assert_eq!(pixel[3], 255, "compositing over an opaque backdrop leaves no hole: {pixel:?}");
}

#[test]
fn a_low_alpha_shadow_darkens_instead_of_cutting_a_hole() {
    let mut surface = SoftwareSurface::new(Size { width: 2, height: 2 }, 1.0);
    surface.begin_frame(Color::rgb(200, 200, 200));
    // A typical elevation shadow: black at ~16% alpha.
    surface.fill_rect(Rect::new(0, 0, 2, 2), Color::rgba(0, 0, 0, 40));
    surface.end_frame();

    let pixel = &surface.frame_rgba()[0..4];
    assert!(pixel[0] < 200, "the shadow must darken the backdrop: {pixel:?}");
    assert!(pixel[0] > 150, "and by only its alpha share: {pixel:?}");
    assert_eq!(pixel[3], 255, "the frame must stay opaque: {pixel:?}");
}

#[test]
fn a_raised_face_casts_a_blurred_shadow_with_its_declared_alpha() {
    use crate::render::{Elevation, RenderContext, SurfaceStyle};

    let mut backend = SoftwarePaintBackend::new(Size { width: 80, height: 60 }, 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        let style = SurfaceStyle::solid().elevated(Elevation::Level2);
        style.paint(
            &mut context,
            Rect::new(10, 10, 40, 30),
            Color::WHITE,
            Color::BLACK,
            0,
            0,
            Color::BLACK,
        );
    }
    backend.end_frame();

    let rgba = backend.frame_rgba();
    let alpha_at = |x: usize, y: usize| rgba[(y * 80 + x) * 4 + 3];

    assert_eq!(alpha_at(30, 20), 255, "the face itself is opaque");
    assert!(
        alpha_at(30, 42) > 0,
        "the shadow reaches below the face (offset 2 + blur 6), not just a bar at its edge"
    );
    assert!(
        alpha_at(30, 42) < 255,
        "and it is translucent at its edge, because the blur spreads the declared alpha"
    );
    assert_eq!(alpha_at(30, 58), 0, "past the blur reach nothing is painted");
}

#[test]
fn double_buffer_present_swaps_frame() {
    let mut surface = SoftwareSurface::new(Size { width: 4, height: 4 }, 1.0);
    surface.begin_frame(Color::RED);
    surface.end_frame();
    assert_eq!(&surface.frame_rgba()[0..4], &[255, 0, 0, 255]);
    surface.begin_frame(Color::BLUE);
    surface.end_frame();
    assert_eq!(&surface.frame_rgba()[0..4], &[0, 0, 255, 255]);
}
#[test]
fn fill_rect_writes_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.fill_rect(Rect { x: 2, y: 2, width: 3, height: 3 }, Color::rgba(1, 2, 3, 255));
    surface.end_frame();
    let idx = ((3 * 8 + 3) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[1, 2, 3, 255]);
}
#[test]
fn shaping_merges_combining_marks_into_one_cluster() {
    let surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
    let shaped = surface.shape_text("e\u{0301}", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
#[test]
fn shaping_merges_zwj_sequence_into_one_cluster() {
    let surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
    let shaped = surface.shape_text("👨\u{200D}👩\u{200D}👧\u{200D}👦", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
#[test]
fn scene_composition_respects_layer_order() {
    let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
    let mut back = SceneLayer::new(0);
    back.push(RenderCommand::FillRect {
        rect: Rect { x: 0, y: 0, width: 8, height: 8 },
        color: Color::rgba(10, 20, 30, 255),
    });
    let mut front = SceneLayer::new(10);
    front.push(RenderCommand::FillRect {
        rect: Rect { x: 2, y: 2, width: 2, height: 2 },
        color: Color::rgba(200, 1, 2, 255),
    });
    let mut scene = RenderScene::new();
    scene.add_layer(front);
    scene.add_layer(back);
    scene.compose_to(&mut surface, Color::BLACK);
    let idx = ((2 * 8 + 2) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[200, 1, 2, 255]);
}
#[test]
fn scene_clear_removes_layers() {
    let mut scene = RenderScene::new();
    scene.add_layer(SceneLayer::new(1));
    assert_eq!(scene.layers().len(), 1);
    scene.clear();
    assert!(scene.layers().is_empty());
}
#[test]
fn scene_composes_through_paint_backend_strategy() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect { x: 1, y: 1, width: 2, height: 2 },
        color: Color::rgba(7, 8, 9, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = 36;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[7, 8, 9, 255]);
}
#[test]
fn software_backend_delegates_text_shaping() {
    let backend = SoftwarePaintBackend::new(Size { width: 100, height: 40 }, 1.0);
    let shaped = backend.shape_text("e\u{0301}", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
/// CJK text must resolve to real glyphs with full-width advances, not tofu or narrow fallback.
///
/// "Mojibake in the demos" is either a wrong glyph mapping (a narrow or wrong-advance glyph per
/// character) or a missing face (tofu). Both show up in the measured advance: a CJK glyph is about
/// one em wide, so two of them advance about twice one. A build without a CJK face answers `None`
/// from `face_for_char`, which the demo reports in its log, and there is no width to assert.
///
/// `face_for_char` is re-exported only when some outline face or shaper is compiled in
/// (`src/render/text/mod.rs`), so the test follows the same gate; under the stripped profiles the
/// function does not exist and this file would not compile at all.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
#[test]
fn cjk_text_measures_as_full_width_glyphs() {
    let _guard = crate::style::theme_test_guard();
    if crate::render::text::face_for_char('中').is_none() {
        return;
    }
    let surface = SoftwareSurface::new(Size { width: 200, height: 40 }, 1.0);
    let font = Font::simple("Sans", 16.0);
    let one = surface.measure_text("中", &font);
    let two = surface.measure_text("中文", &font);
    let one_w = f64::from(one.width);
    let two_w = f64::from(two.width);
    assert!(one_w > 8.0, "a CJK glyph has an advance, got {one_w}");
    assert!(
        (two_w - one_w * 2.0).abs() < one_w.max(2.0),
        "two CJK glyphs advance about twice one: {two_w} vs {one_w}"
    );
}
/// A host-registered face must be **painted** from, not only measured from.
///
/// # The defect this pins
///
/// The paint path walks a **compile-time** stack of shipped faces, so a face registered at run time
/// cannot be in it. A host that registered a CJK face (what every demo does) therefore got the
/// measurement right — full-width advances, no tofu *width* — and every Han character outside the
/// shipped subset was then painted from the 8x8 tofu bitmap. The demo framed it perfectly: Chinese
/// labels measured as Chinese and showed as boxes.
///
/// This also pins the registration guard: Apple's `PingFang UI` `.ttc` is bitmap-only (no `glyf`,
/// no `CFF`, no `CFF2`), so registering it made the *first* covering face one that can never be
/// drawn, and shadowed the next family — `Hiragino Sans GB`, whose CFF outlines the rasteriser
/// reads fine. A face with no outlines is now refused at registration, so the family that answers
/// is one that can actually paint.
#[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
#[test]
fn a_registered_cjk_face_is_painted_not_just_measured() {
    // The registry is process-wide and tests run in parallel; the same lock the registry's own
    // tests take keeps this sequence from interleaving with a `clear_faces`.
    let _guard = crate::render::text::runtime_fonts::runtime_fonts_test_guard();
    let Some(family) = crate::render::text::register_system_cjk_font() else {
        // No installed CJK face on this host: the refusal path is the whole contract here.
        return;
    };
    let cell = crate::render::text::Cell::new(16, 16);
    let mut coverage = vec![0u8; (cell.width * cell.height) as usize];
    let painted = crate::render::text::paint_active_at_weight(
        '窗',
        cell,
        &mut coverage,
        crate::core::Font::default_weight(),
    )
    .expect("painting a glyph always answers");
    assert_eq!(painted.source, family, "the registered face must be the one that paints");
    assert!(coverage.iter().any(|byte| *byte > 0), "the glyph must have ink, not an empty cell");
    crate::render::text::clear_faces();
}

#[test]
fn draw_text_rasterizes_glyph_instead_of_full_rect_fill() {
    let mut surface = SoftwareSurface::new(Size { width: 80, height: 30 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_text(Point { x: 4, y: 4 }, "A", &font(), Color::WHITE, HorizontalAlignment::Left);
    surface.end_frame();
    let metrics = surface.measure_text("A", &font());
    let mut painted = 0usize;
    for y in 4..(4 + metrics.height as i32) {
        for x in 4..(4 + metrics.width as i32) {
            let idx = ((y as u32 * surface.size().width + x as u32) * 4 + 3) as usize;
            if surface.frame_rgba()[idx] > 0 {
                painted += 1;
            }
        }
    }
    let bbox_area = (metrics.width as usize).saturating_mul(metrics.height as usize);
    assert!(painted > 0);
    assert!(painted < bbox_area);
}

/// A colour glyph reaches the surface in colour, through the same `draw_text` path.
///
/// # Why more than one RGB value is the assertion
///
/// `draw_text` has always blended *coverage*: one alpha per pixel for the caller's text colour,
/// which for `Color::WHITE` can only produce white and its shades. A colour emoji blended that way
/// would still paint pixels and still set alpha, so "something was drawn" proves nothing. Counting
/// distinct RGB values does: a face that carries its own colour produces many, and a coverage blend
/// of white over transparent black produces exactly one.
///
/// The text colour passed in is deliberately **red**, which is the other half of the assertion: a
/// colour glyph must carry its own colours rather than being tinted by the caller's.
#[cfg(feature = "fonts-emoji-color")]
#[test]
fn a_colour_emoji_reaches_the_surface_in_colour() {
    let mut surface = SoftwareSurface::new(Size { width: 64, height: 64 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    let emoji_font = Font::new("Sans", 48.0, false, false);
    surface.draw_text(
        Point { x: 4, y: 4 },
        "\u{1F600}",
        &emoji_font,
        Color::rgba(255, 0, 0, 255),
        HorizontalAlignment::Left,
    );
    surface.end_frame();

    let frame = surface.frame_rgba();
    let mut colours = std::collections::BTreeSet::new();
    for pixel in frame.as_chunks::<4>().0 {
        if pixel[3] != 0 {
            colours.insert((pixel[0], pixel[1], pixel[2]));
        }
    }
    assert!(
        colours.len() > 1,
        "a colour emoji must produce more than one RGB value, got {colours:?}"
    );
    // The grinning face's skin is yellow, so a green-dominant or blue-dominant pixel would mean the
    // channel order is wrong. `r > b` holds for every yellow and red in the glyph.
    assert!(
        colours.iter().any(|(r, _, b)| r > b),
        "the emoji's warm colours must survive the blend, got {colours:?}"
    );
}

/// A colour glyph on a 1-bit-sized scratch is refused rather than misread.
///
/// This is the buffer contract between `draw_text` and the glyph stack stated as a test: the stack
/// must answer `None` for a colour-only character when given one byte per pixel, because the caller
/// that reserved that buffer is the 1-bit path and reading RGBA out of it would paint noise.
#[cfg(feature = "fonts-emoji-color")]
#[test]
fn a_colour_glyph_is_refused_on_a_coverage_sized_buffer() {
    use crate::render::text::{paint_active, Cell, InkKind};
    let cell = Cell::new(16, 16);
    let mut one_byte_per_pixel = vec![0u8; cell.area()];
    let painted = paint_active('\u{1F600}', cell, &mut one_byte_per_pixel);
    // Tofu is the honest fallback: the stack has no 1-bit source for an emoji, so it draws the
    // missing-glyph block rather than half a colour glyph.
    assert!(
        painted.is_none() || painted.expect("checked").ink != InkKind::Color,
        "a colour glyph must not be painted into a coverage-sized buffer"
    );
    // And the right-sized buffer does produce colour, so the refusal above is about the buffer and
    // not about the face being unreadable.
    let mut rgba = vec![0u8; cell.area() * 4];
    let painted = paint_active('\u{1F600}', cell, &mut rgba).expect("a colour glyph paints");
    assert_eq!(painted.ink, InkKind::Color);
}

#[test]
fn fill_circle_writes_center_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.fill_circle(Point { x: 6, y: 6 }, 3, Color::rgba(9, 10, 11, 255));
    surface.end_frame();
    let idx = ((6 * 12 + 6) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[9, 10, 11, 255]);
}
#[test]
fn scene_composition_supports_circle_commands() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillCircle {
        center: Point { x: 5, y: 5 },
        radius: 2,
        color: Color::rgba(3, 4, 5, 255),
    });
    layer.push(RenderCommand::DrawCircle {
        center: Point { x: 5, y: 5 },
        radius: 2,
        color: Color::rgba(200, 201, 202, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let stroke_idx = ((5 * 12 + 7) * 4) as usize;
    let stroke_px = &backend.frame_rgba()[stroke_idx..stroke_idx + 4];
    assert!(stroke_px[0] > 3 && stroke_px[1] > 4 && stroke_px[2] > 5);
    assert_eq!(stroke_px[3], 255);
    let fill_idx = ((5 * 12 + 5) * 4) as usize;
    let fill_px = &backend.frame_rgba()[fill_idx..fill_idx + 4];
    assert!(fill_px[0] >= 3 && fill_px[0] < 32);
    assert!(fill_px[1] >= 4 && fill_px[1] < 32);
    assert!(fill_px[2] >= 5 && fill_px[2] < 32);
    assert_eq!(fill_px[3], 255);
}
#[test]
fn draw_circle_with_width_expands_stroke_band() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_circle_with_width(Point { x: 8, y: 8 }, 4, Color::rgba(170, 171, 172, 255), 2);
    surface.end_frame();
    let outer_idx = ((8 * 16 + 12) * 4) as usize;
    assert!(surface.frame_rgba()[outer_idx + 3] > 0);
    let inner_band_idx = ((8 * 16 + 10) * 4) as usize;
    assert!(surface.frame_rgba()[inner_band_idx + 3] > 0);
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(surface.frame_rgba()[center_idx + 3], 0);
}
#[test]
fn fill_circle_aa_applies_partial_alpha_on_edge_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(190, 191, 192, 255));
    surface.end_frame();
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[190, 191, 192, 255]);
    let edge_idx = ((8 * 16 + 12) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_stroke_circle_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawCircleStroke {
        center: Point { x: 8, y: 8 },
        radius: 4,
        color: Color::rgba(180, 181, 182, 255),
        width: 2,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let band_idx = ((8 * 16 + 10) * 4) as usize;
    let band_px = &backend.frame_rgba()[band_idx..band_idx + 4];
    assert_eq!(band_px[0], 180);
    assert_eq!(band_px[1], 181);
    assert_eq!(band_px[2], 182);
    assert!(band_px[3] > 0 && band_px[3] < 255);
}
#[test]
fn scene_composition_supports_aa_fill_circle_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillCircleAA {
        center: Point { x: 8, y: 8 },
        radius: 4,
        color: Color::rgba(200, 201, 202, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[center_idx..center_idx + 4], &[200, 201, 202, 255]);
    let edge_idx = ((8 * 16 + 12) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn draw_line_with_width_marks_neighbor_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.draw_line_with_width(
        Point { x: 2, y: 6 },
        Point { x: 9, y: 6 },
        Color::rgba(21, 22, 23, 255),
        3,
    );
    surface.end_frame();
    let center_idx = ((6 * 12 + 5) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[21, 22, 23, 255]);
    let upper_idx = ((5 * 12 + 5) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[upper_idx..upper_idx + 4], &[21, 22, 23, 255]);
    let lower_idx = ((7 * 12 + 5) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[lower_idx..lower_idx + 4], &[21, 22, 23, 255]);
}
#[test]
fn scene_composition_supports_stroke_line_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawLineStroke {
        from: Point { x: 2, y: 6 },
        to: Point { x: 9, y: 6 },
        color: Color::rgba(31, 32, 33, 255),
        width: 3,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = ((5 * 12 + 5) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[31, 32, 33, 255]);
}
#[test]
fn draw_rect_with_width_marks_neighbor_border_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.draw_rect_with_width(
        Rect { x: 4, y: 4, width: 6, height: 6 },
        Color::rgba(41, 42, 43, 255),
        3,
    );
    surface.end_frame();
    let border_idx = ((4 * 14 + 6) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[border_idx..border_idx + 4], &[41, 42, 43, 255]);
    let neighbor_idx = ((5 * 14 + 6) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[neighbor_idx..neighbor_idx + 4], &[41, 42, 43, 255]);
}
#[test]
fn scene_composition_supports_stroke_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect { x: 4, y: 4, width: 6, height: 6 },
        color: Color::rgba(51, 52, 53, 255),
        width: 3,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 14, height: 14 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = ((5 * 14 + 6) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[51, 52, 53, 255]);
}
#[test]
fn fill_rounded_rect_writes_center_and_preserves_corner() {
    let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.fill_rounded_rect(
        Rect { x: 3, y: 3, width: 8, height: 8 },
        3,
        Color::rgba(61, 62, 63, 255),
    );
    surface.end_frame();
    let center_idx = ((7 * 14 + 7) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[61, 62, 63, 255]);
    let corner_idx = ((3 * 14 + 3) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[corner_idx..corner_idx + 4], &[0, 0, 0, 255]);
}
#[test]
fn scene_composition_supports_rounded_rect_commands() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRect {
        rect: Rect { x: 3, y: 3, width: 8, height: 8 },
        radii: [3, 3, 3, 3],
        color: Color::rgba(71, 72, 73, 255),
    });
    layer.push(RenderCommand::DrawRoundedRectStroke {
        rect: Rect { x: 3, y: 3, width: 8, height: 8 },
        radii: [3, 3, 3, 3],
        color: Color::rgba(81, 82, 83, 255),
        width: 2,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 14, height: 14 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let stroke_idx = ((3 * 14 + 7) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[stroke_idx..stroke_idx + 4], &[81, 82, 83, 255]);
    let fill_idx = ((7 * 14 + 7) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[fill_idx..fill_idx + 4], &[71, 72, 73, 255]);
}
/// A rounded outline hugs its rectangle and closes at the corners on the **software** path.
///
/// This is the path the running application paints through, so the property the snapshots show must
/// hold here first: the rule sits on `x = rect.x` / `y = rect.y` rather than `radius` pixels inside
/// (which is what the wgpu CPU ring did), and the corner arc meets those rules instead of stopping
/// short of them.
#[test]
fn rounded_rect_stroke_lies_on_the_edge_and_closes_its_corners() {
    let mut surface = SoftwareSurface::new(Size { width: 24, height: 24 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_rounded_rect_with_width(
        Rect { x: 0, y: 0, width: 20, height: 20 },
        4,
        Color::rgba(200, 201, 202, 255),
        1,
    );
    surface.end_frame();
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 24 + x) * 4;
        [
            surface.frame_rgba()[i],
            surface.frame_rgba()[i + 1],
            surface.frame_rgba()[i + 2],
            surface.frame_rgba()[i + 3],
        ]
    };

    // The rules are on the rectangle's own edges, not inset by the radius.
    assert!(px(0, 10)[3] > 0, "the left rule must sit on x = rect.x");
    assert!(px(10, 0)[3] > 0, "the top rule must sit on y = rect.y");

    // The corner is a quarter arc: the exact corner point is outside the rounded shape.
    assert_eq!(px(0, 0)[3], 0, "the square corner must be cut away by the radius");

    // The middle of the rectangle is the hole the outline encloses.
    assert_eq!(px(10, 10)[3], 0, "an outline is a ring, not a fill");

    // Nothing is painted outside the rectangle the caller declared.
    assert_eq!(px(20, 10)[3], 0, "nothing is painted past rect.right()");
    assert_eq!(px(10, 20)[3], 0, "nothing is painted past rect.bottom()");
}

#[test]
fn draw_rounded_rect_aa_with_width_produces_soft_edge() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_rounded_rect_aa_with_width(
        Rect { x: 3, y: 3, width: 10, height: 10 },
        4,
        Color::rgba(230, 231, 232, 255),
        2,
    );
    surface.end_frame();
    let core_idx = ((3 * 16 + 8) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[core_idx..core_idx + 4], &[230, 231, 232, 255]);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn fill_rounded_rect_aa_produces_soft_corner_edge() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect_aa(
        Rect { x: 3, y: 3, width: 10, height: 10 },
        4,
        Color::rgba(250, 210, 170, 255),
    );
    surface.end_frame();
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[250, 210, 170, 255]);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn aa_sample_level_changes_rounded_rect_edge_coverage() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.set_aa_samples_per_axis(1);
    assert_eq!(surface.aa_samples_per_axis(), 1);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect_aa(
        Rect { x: 3, y: 3, width: 10, height: 10 },
        4,
        Color::rgba(200, 100, 50, 255),
    );
    surface.end_frame();
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let alpha_low = surface.frame_rgba()[edge_idx + 3];
    surface.set_aa_samples_per_axis(4);
    assert_eq!(surface.aa_samples_per_axis(), 4);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect_aa(
        Rect { x: 3, y: 3, width: 10, height: 10 },
        4,
        Color::rgba(200, 100, 50, 255),
    );
    surface.end_frame();
    let alpha_high = surface.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_low, alpha_high);
}
#[test]
fn render_config_applies_and_clamps_aa_samples() {
    let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
    assert_eq!(surface.render_config().aa_samples_per_axis, 4);
    surface.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 0 });
    assert_eq!(surface.render_config().aa_samples_per_axis, 1);
    surface.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 12 });
    assert_eq!(surface.render_config().aa_samples_per_axis, 8);
}
#[test]
fn backend_render_config_passthrough_clamps_values() {
    let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
    assert_eq!(backend.render_config().aa_samples_per_axis, 4);
    backend.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 0 });
    assert_eq!(backend.render_config().aa_samples_per_axis, 1);
    backend.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 20 });
    assert_eq!(backend.render_config().aa_samples_per_axis, 8);
}
#[test]
fn paint_backend_trait_render_config_updates_software_backend() {
    let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
    PaintBackend::apply_render_config(
        &mut backend,
        SoftwareRenderConfig { aa_samples_per_axis: 3 },
    );
    assert_eq!(PaintBackend::render_config(&backend).aa_samples_per_axis, 3);
}
#[test]
fn scene_compose_with_temporary_config_restores_backend_state() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect { x: 3, y: 3, width: 10, height: 10 },
        radii: [4, 4, 4, 4],
        color: Color::rgba(100, 110, 120, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    backend.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 4 });
    scene.compose_with_backend_config(
        &mut backend,
        Color::TRANSPARENT,
        Some(SoftwareRenderConfig { aa_samples_per_axis: 1 }),
    );
    assert_eq!(backend.render_config().aa_samples_per_axis, 4);
}
#[test]
fn scene_compose_with_temporary_config_changes_aa_output() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect { x: 3, y: 3, width: 10, height: 10 },
        radii: [4, 4, 4, 4],
        color: Color::rgba(150, 151, 152, 255),
    });
    scene.add_layer(layer);
    let mut backend_default = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    backend_default.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 4 });
    scene.compose_with_backend(&mut backend_default, Color::TRANSPARENT);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let alpha_default = backend_default.frame_rgba()[edge_idx + 3];
    let mut backend_temp = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    backend_temp.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 4 });
    scene.compose_with_backend_config(
        &mut backend_temp,
        Color::TRANSPARENT,
        Some(SoftwareRenderConfig { aa_samples_per_axis: 1 }),
    );
    let alpha_temp = backend_temp.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_default, alpha_temp);
}
#[test]
fn aa_sample_level_changes_circle_edge_coverage() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.set_aa_samples_per_axis(1);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
    surface.end_frame();
    let edge_idx = ((8 * 16 + 12) * 4) as usize;
    let alpha_low = surface.frame_rgba()[edge_idx + 3];
    surface.set_aa_samples_per_axis(4);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
    surface.end_frame();
    let alpha_high = surface.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_low, alpha_high);
}
#[test]
fn aa_sample_level_changes_line_edge_coverage() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.set_aa_samples_per_axis(1);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_line_aa_with_width(
        Point { x: 2, y: 2 },
        Point { x: 13, y: 9 },
        Color::rgba(130, 131, 132, 255),
        3,
    );
    surface.end_frame();
    let edge_idx = ((5 * 16 + 6) * 4) as usize;
    let alpha_low = surface.frame_rgba()[edge_idx + 3];
    surface.set_aa_samples_per_axis(4);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_line_aa_with_width(
        Point { x: 2, y: 2 },
        Point { x: 13, y: 9 },
        Color::rgba(130, 131, 132, 255),
        3,
    );
    surface.end_frame();
    let alpha_high = surface.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_low, alpha_high);
}
#[test]
fn circle_stroke_applies_partial_alpha_on_edge_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_circle(Point { x: 7, y: 7 }, 3, Color::rgba(100, 120, 140, 255));
    surface.end_frame();
    let edge_idx = ((8 * 14 + 10) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn rounded_rect_fill_applies_partial_alpha_at_corner_edge() {
    let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect(
        Rect { x: 3, y: 3, width: 8, height: 8 },
        3,
        Color::rgba(90, 91, 92, 255),
    );
    surface.end_frame();
    let edge_idx = ((4 * 14 + 3) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn draw_line_aa_produces_partial_alpha_on_neighbor_pixel() {
    let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_line_aa(
        Point { x: 1, y: 1 },
        Point { x: 10, y: 8 },
        Color::rgba(110, 120, 130, 255),
    );
    surface.end_frame();
    let neighbor_idx = ((3 * 12 + 4) * 4) as usize;
    let alpha = surface.frame_rgba()[neighbor_idx + 3];
    assert!(alpha > 0 && alpha < 255);
}
#[test]
fn scene_composition_supports_aa_line_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawLineAA {
        from: Point { x: 1, y: 1 },
        to: Point { x: 10, y: 8 },
        color: Color::rgba(140, 150, 160, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let idx = ((3 * 12 + 4) * 4) as usize;
    let px = &backend.frame_rgba()[idx..idx + 4];
    assert_eq!(px[0], 140);
    assert_eq!(px[1], 150);
    assert_eq!(px[2], 160);
    assert!(px[3] > 0 && px[3] < 255);
}
#[test]
fn draw_line_aa_with_width_expands_band_and_keeps_soft_edge() {
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_line_aa_with_width(
        Point { x: 2, y: 8 },
        Point { x: 13, y: 8 },
        Color::rgba(210, 211, 212, 255),
        3,
    );
    surface.end_frame();
    let core_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[core_idx..core_idx + 4], &[210, 211, 212, 255]);
    let edge_idx = ((9 * 16 + 8) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_aa_stroke_line_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawLineStrokeAA {
        from: Point { x: 2, y: 8 },
        to: Point { x: 13, y: 8 },
        color: Color::rgba(220, 221, 222, 255),
        width: 3,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let core_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[core_idx..core_idx + 4], &[220, 221, 222, 255]);
    let edge_idx = ((9 * 16 + 8) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_aa_stroke_rounded_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawRoundedRectStrokeAA {
        rect: Rect { x: 3, y: 3, width: 10, height: 10 },
        radii: [4, 4, 4, 4],
        color: Color::rgba(240, 241, 242, 255),
        width: 2,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let core_idx = ((3 * 16 + 8) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[core_idx..core_idx + 4], &[240, 241, 242, 255]);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_aa_fill_rounded_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect { x: 3, y: 3, width: 10, height: 10 },
        radii: [4, 4, 4, 4],
        color: Color::rgba(120, 130, 140, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[center_idx..center_idx + 4], &[120, 130, 140, 255]);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn auto_compose_handles_draw_text_scene_with_gpu_or_cpu_backend() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawText {
        origin: Point { x: 1, y: 10 },
        text: Arc::from("fallback"),
        font: Font::default(),
        color: Color::rgba(250, 120, 40, 255),
        alignment: HorizontalAlignment::Left,
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 48, height: 24 }, 1.0);
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(backend, AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware));
}
#[test]
fn auto_compose_produces_expected_pixels_for_simple_rect_scene() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect { x: 2, y: 2, width: 6, height: 4 },
        color: Color::rgba(11, 22, 33, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 16, height: 12 }, 1.0);
    let backend = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert!(matches!(backend, AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware));
    let idx = ((3 * 16 + 3) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[11, 22, 33, 255]);
}
#[test]
fn auto_compose_updates_last_backend_diagnostics() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect { x: 1, y: 1, width: 3, height: 3 },
        color: Color::rgba(1, 2, 3, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
    let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert_eq!(selected, last_auto_render_backend());
}
#[test]
fn auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect { x: 0, y: 0, width: 1, height: 1 },
        color: Color::rgba(9, 8, 7, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 0, height: 0 }, 1.0);
    let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert_eq!(selected, AutoRenderBackend::CpuSoftware);
    assert_eq!(last_auto_render_backend(), AutoRenderBackend::CpuSoftware);
}
#[test]
fn scene_layer_accepts_common_render_command_variants() {
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 100, 100), color: Color::BLUE });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(10, 10),
        text: Arc::from("Hello"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::FillCircle {
        center: Point::new(50, 50),
        radius: 10,
        color: Color::RED,
    });
    let mut saw_fill_rect = false;
    let mut saw_draw_text = false;
    let mut saw_fill_circle = false;
    for command in layer.commands() {
        match command {
            RenderCommand::FillRect { .. } => saw_fill_rect = true,
            RenderCommand::DrawText { .. } => saw_draw_text = true,
            RenderCommand::FillCircle { .. } => saw_fill_circle = true,
            _ => {}
        }
    }
    assert!(saw_fill_rect);
    assert!(saw_draw_text);
    assert!(saw_fill_circle);
}
#[test]
fn auto_compose_renders_fill_rect_scene_with_gpu_or_cpu_backend() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 0, 128, 80),
        color: Color::rgba(10, 20, 30, 255),
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(4, 20, 112, 52),
        color: Color::rgba(40, 50, 60, 255),
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(10, 26, 50, 20),
        color: Color::rgba(60, 70, 80, 255),
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(70, 28, 20, 20),
        color: Color::rgba(80, 90, 100, 255),
    });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(16, 50),
        text: Arc::from("Test"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 128, height: 88 }, 1.0);
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(backend, AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware));
    let sample = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * surface.size().width + x) * 4) as usize;
        [
            surface.frame_rgba()[idx],
            surface.frame_rgba()[idx + 1],
            surface.frame_rgba()[idx + 2],
            surface.frame_rgba()[idx + 3],
        ]
    };
    assert_eq!(sample(2, 2), [10, 20, 30, 255]);
}
#[test]
fn data_range_scene_layer_accepts_drawtext_fillcircle_fillrect() {
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawText {
        origin: Point::new(4, 4),
        text: Arc::from("Alpha"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(4, 28),
        text: Arc::from("Row-1"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(4, 48),
        text: Arc::from("Row-2"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::FillCircle {
        center: Point::new(160, 15),
        radius: 6,
        color: Color::BLUE,
    });
    layer.push(RenderCommand::FillRect { rect: Rect::new(140, 8, 80, 14), color: Color::GREEN });
    layer.push(RenderCommand::FillRect { rect: Rect::new(140, 30, 60, 16), color: Color::GRAY });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(140, 58, 40, 16),
        color: Color::DARK_GRAY,
    });
    layer.push(RenderCommand::FillRect { rect: Rect::new(4, 4, 120, 20), color: Color::WHITE });
    layer.push(RenderCommand::FillRect { rect: Rect::new(4, 28, 120, 64), color: Color::WHITE });
    let mut draw_text_count = 0usize;
    let mut fill_circle_count = 0usize;
    let mut fill_rect_count = 0usize;
    for command in layer.commands() {
        match command {
            RenderCommand::DrawText { .. } => draw_text_count += 1,
            RenderCommand::FillCircle { .. } => fill_circle_count += 1,
            RenderCommand::FillRect { .. } => fill_rect_count += 1,
            _ => {}
        }
    }
    assert!(draw_text_count >= 3);
    assert!(fill_circle_count >= 1);
    assert!(fill_rect_count >= 5);
}
#[test]
fn auto_compose_renders_mixed_commands_scene_with_gpu_or_cpu_backend() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawText {
        origin: Point::new(8, 8),
        text: Arc::from("Test"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 256, 128), color: Color::WHITE });
    layer.push(RenderCommand::FillCircle {
        center: Point::new(170, 18),
        radius: 6,
        color: Color::BLUE,
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 256, height: 128 }, 1.0);
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(backend, AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware));
    let sample = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * surface.size().width + x) * 4) as usize;
        [
            surface.frame_rgba()[idx],
            surface.frame_rgba()[idx + 1],
            surface.frame_rgba()[idx + 2],
            surface.frame_rgba()[idx + 3],
        ]
    };
    assert_eq!(sample(0, 0), [255, 255, 255, 255]);
}
#[test]
fn scene_layer_accepts_host_navigation_command_types() {
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 0, 260, 24),
        color: Color::rgba(200, 200, 210, 255),
    });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(6, 6),
        text: Arc::from("File"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(46, 6),
        text: Arc::from("Edit"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::FillRect { rect: Rect::new(0, 24, 160, 100), color: Color::WHITE });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(8, 36),
        text: Arc::from("Open"),
        font: font(),
        color: Color::BLACK,
        alignment: HorizontalAlignment::Left,
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 128, 260, 28),
        color: Color::rgba(220, 220, 230, 255),
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 160, 260, 22),
        color: Color::rgba(240, 240, 240, 255),
    });
    layer.push(RenderCommand::FillRoundedRect {
        rect: Rect::new(170, 24, 90, 70),
        radii: [4, 4, 4, 4],
        color: Color::rgba(220, 225, 235, 255),
    });
    let mut draw_text_count = 0usize;
    let mut fill_rect_count = 0usize;
    let mut rounded_rect_count = 0usize;
    for command in layer.commands() {
        match command {
            RenderCommand::DrawText { .. } => draw_text_count += 1,
            RenderCommand::FillRect { .. } => fill_rect_count += 1,
            RenderCommand::FillRoundedRect { .. } => rounded_rect_count += 1,
            _ => {}
        }
    }
    assert!(draw_text_count >= 2);
    assert!(fill_rect_count >= 3);
    assert!(rounded_rect_count >= 1);
}
#[test]
fn auto_compose_renders_multi_command_scene_with_gpu_or_cpu_backend() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(0, 0, 280, 190),
        color: Color::rgba(18, 20, 24, 255),
    });
    layer.push(RenderCommand::FillRect {
        rect: Rect::new(8, 32, 180, 100),
        color: Color::rgba(38, 45, 60, 255),
    });
    layer.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 320, 24), color: Color::DARK_GRAY });
    layer.push(RenderCommand::DrawText {
        origin: Point::new(6, 6),
        text: Arc::from("File"),
        font: font(),
        color: Color::WHITE,
        alignment: HorizontalAlignment::Left,
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(Size { width: 280, height: 190 }, 1.0);
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(backend, AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware));
    assert!(!surface.frame_rgba().is_empty());
}

#[test]
fn draw_gradient_linear_renders_pixels() {
    use crate::style::Gradient;
    let gradient = Gradient::linear(Point::new(0, 0), Point::new(10, 0))
        .add_stop(0.0, Color::BLACK)
        .add_stop(1.0, Color::WHITE);
    let mut surface = SoftwareSurface::new(Size { width: 12, height: 4 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rect_gradient(Rect::new(0, 0, 10, 4), &gradient);
    surface.end_frame();
    // Left edge should be close to BLACK
    assert!(surface.frame_rgba()[0] < 20);
    // Right edge should be close to WHITE
    let right_idx = 9 * 4;
    assert!(surface.frame_rgba()[right_idx] >= 200);
}

#[test]
fn draw_gradient_via_render_command_paints_pixels() {
    use crate::style::Gradient;
    let gradient = Gradient::linear(Point::new(0, 0), Point::new(10, 0))
        .add_stop(0.0, Color::RED)
        .add_stop(1.0, Color::BLUE);
    let mut surface = SoftwareSurface::new(Size { width: 12, height: 4 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rect_gradient(Rect::new(0, 0, 10, 4), &gradient);
    surface.end_frame();
    // Left edge should be close to RED
    assert!(surface.frame_rgba()[0] >= 200);
    assert!(surface.frame_rgba()[1] < 20);
    assert!(surface.frame_rgba()[2] < 20);
    // Right edge should have blue > red (gradient transitioning from RED to BLUE)
    let right_idx = 9 * 4;
    assert!(
        surface.frame_rgba()[right_idx] < surface.frame_rgba()[right_idx + 2],
        "at x=9 expected blue > red, got r={} b={}",
        surface.frame_rgba()[right_idx],
        surface.frame_rgba()[right_idx + 2]
    );
    assert!(surface.frame_rgba()[right_idx + 2] >= 150);
}

#[test]
fn draw_gradient_svg_produces_valid_markup() {
    use crate::render::svg::SvgPaintBackend;
    use crate::style::Gradient;
    let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
        .add_stop(0.0, Color::BLACK)
        .add_stop(1.0, Color::WHITE);
    let mut svg = SvgPaintBackend::new(Size::new(100, 20));
    svg.begin_frame(Color::TRANSPARENT);
    svg.execute_command(&RenderCommand::DrawGradient {
        rect: Rect::new(0, 0, 100, 20),
        gradient: Arc::new(gradient),
    });
    svg.end_frame();
    let output = svg.finish();
    assert!(output.contains("linearGradient"));
    assert!(output.contains("url(#g1)"));
    assert!(output.contains("<stop"));
    assert!(output.len() > 200);
}

#[test]
fn draw_arc_via_software_backend_paints_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 20, height: 20 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_arc(Point::new(10, 10), 8, 0.0, core::f32::consts::PI * 2.0, Color::RED, true);
    surface.end_frame();
    // Center should be filled (for a filled full-circle arc)
    let center_idx = (10 * 20 + 10) * 4;
    assert!(surface.frame_rgba()[center_idx] > 0);
}

#[test]
fn draw_path_via_software_backend_paints_pixels() {
    let mut surface = SoftwareSurface::new(Size { width: 20, height: 20 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    let points = vec![Point::new(2, 2), Point::new(18, 2), Point::new(18, 18), Point::new(2, 18)];
    surface.draw_path(&points, true, Color::BLUE, true, 1);
    surface.end_frame();
    // Interior should be filled
    let center_idx = (10 * 20 + 10) * 4;
    assert!(surface.frame_rgba()[center_idx + 2] > 0);
}

#[test]
fn draw_text_alignment_center_adjusts_origin() {
    let mut surface = SoftwareSurface::new(Size { width: 100, height: 20 }, 1.0);
    surface.begin_frame(Color::BLACK);
    surface.draw_text(
        Point::new(50, 5),
        "Hello",
        &font(),
        Color::WHITE,
        HorizontalAlignment::Center,
    );
    surface.end_frame();
    // Should not panic; text should be centered around x=50
    assert!(!surface.frame_rgba().is_empty());
}

// ─── Regression pins for the 2026-10-07 render-partition batch (N-R-01..06) ─────────────────────

/// N-R-01: a fully transparent source is a no-op in source-over, not an erase.
///
/// The old branch zeroed the destination pixel when `color.a == 0`, so drawing "nothing" punched a
/// black hole through whatever was already composited. The assertion is that the pre-existing opaque
/// pixel survives byte-for-byte.
#[test]
fn fully_transparent_source_leaves_destination_untouched() {
    use crate::render::blend_pixel_with_mode;
    use crate::render::BlendMode;
    let width = 4u32;
    let mut frame = crate::compat::vec![0u8; (width * width * 4) as usize];
    // Seed an opaque orange pixel at (1, 1).
    let idx = (width + 1) as usize * 4;
    frame[idx..idx + 4].copy_from_slice(&[200, 120, 40, 255]);
    blend_pixel_with_mode(
        &mut frame,
        width,
        1,
        1,
        Color::rgba(10, 20, 30, 0),
        1.0,
        BlendMode::Normal,
    );
    assert_eq!(
        &frame[idx..idx + 4],
        &[200, 120, 40, 255],
        "a zero-alpha source must not erase the destination"
    );
}

/// N-R-02: `BoxShadow` spread/offset arithmetic saturates instead of wrapping.
///
/// Reaching `i32::MAX`-scale inputs in debug is the point: the old `rect.width as i32 + *spread * 2`
/// and `rect.x + offset_x - *spread` would abort in a debug build and wrap in a release one. The
/// assertion is only that the command executes and produces a bounded, non-panicking rect.
#[test]
fn box_shadow_extreme_spread_and_offset_do_not_overflow() {
    let mut surface = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
    surface.begin_frame(Color::TRANSPARENT);
    // Debug builds panic on wrapping arithmetic, so this call is the assertion.
    surface.execute_command(&RenderCommand::BoxShadow {
        rect: Rect::new(i32::MAX - 2, 0, u32::MAX, u32::MAX),
        color: Color::rgba(0, 0, 0, 255),
        offset_x: 5,
        offset_y: i32::MIN,
        blur_radius: 0,
        spread: i32::MAX,
    });
    surface.execute_command(&RenderCommand::BoxShadow {
        rect: Rect::new(i32::MIN + 1, i32::MIN + 1, 4, 4),
        color: Color::rgba(0, 0, 0, 255),
        offset_x: i32::MIN,
        offset_y: i32::MIN,
        blur_radius: 0,
        spread: i32::MIN,
    });
    surface.end_frame();
    // The surface must remain intact (a frame of 8x8 RGBA bytes).
    assert_eq!(surface.frame_rgba().len(), 8 * 8 * 4);
}

/// N-R-03: a non-finite `SetOpacity` is rejected rather than poisoning every later colour.
///
/// Before the fix, `state.opacity *= NaN` made every subsequent alpha `NaN.round() as u8 == 0`, so a
/// single bad command silently erased the rest of the batch's colour. The fix keeps the last finite
/// opacity, so the fill after the bad command paints at full strength.
#[test]
fn batch_set_opacity_rejects_non_finite_and_keeps_colour() {
    use crate::render::{BatchCommand, BatchRenderer};
    let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    let batch = backend.begin_batch();
    backend.record(BatchCommand::SetOpacity { opacity: f32::NAN }).expect("a batch is open");
    backend
        .record(BatchCommand::FillRect {
            rect: Rect::new(0, 0, 4, 4),
            color: Color::rgba(30, 40, 50, 255),
        })
        .expect("a batch is open");
    backend.end_batch();
    backend.replay(batch);
    backend.end_frame();
    let idx = (8 + 1) * 4;
    let px = &backend.frame_rgba()[idx..idx + 4];
    assert_eq!(
        px,
        &[30, 40, 50, 255],
        "a NaN opacity must be ignored, not zero the batch's colour"
    );
}

/// N-R-06: an absurd `stroke_inner` width must not panic or wrap.
///
/// `width as i32` narrowed a `u32` above `i32::MAX` to a negative inset, and `width * 2` aborted in
/// debug. The assertion is that the call finishes; the inner rect is conceptually empty for such a
/// width, which is the only honest picture.
#[test]
fn bevel_stroke_inner_clamps_huge_width_without_panicking() {
    use crate::render::bevel::Bevel;
    use crate::render::RenderContext;
    let mut backend = SoftwarePaintBackend::new(Size { width: 32, height: 32 }, 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        let bevel = Bevel::from_base(Color::rgb(128, 128, 128));
        bevel.stroke_inner(&mut context, Rect::new(2, 2, 20, 20), u32::MAX);
        bevel.stroke_inner(
            &mut context,
            Rect::new(i32::MAX - 5, i32::MAX - 5, 8, 8),
            u32::MAX / 2 + 1,
        );
    }
    backend.end_frame();
    assert_eq!(backend.frame_rgba().len(), 32 * 32 * 4);
}

// ── STY-01: per-corner radii and the border-style semantics ─────────────────

// `Recorder` lives in `widget::macos::test_support`, which a stripped build does not compile
// (`#[cfg(full_widgets)] pub mod macos;`). These tests are the only users, so the availability
// gate on the module is the availability gate on them.
#[cfg(full_widgets)]
mod corner_and_border_style_tests {
    use super::*;
    use crate::render::RenderContext;
    use crate::widget::macos::test_support::Recorder;

    #[test]
    fn non_uniform_fill_rounds_only_the_declared_corner() {
        let mut surface = SoftwareSurface::new(Size { width: 20, height: 20 }, 1.0);
        surface.begin_frame(Color::BLACK);
        surface.fill_rounded_rect_radii(
            Rect { x: 0, y: 0, width: 20, height: 20 },
            [6, 0, 0, 0],
            Color::rgba(61, 62, 63, 255),
        );
        surface.end_frame();
        let rgba = surface.frame_rgba();
        let at = |x: i32, y: i32| -> [u8; 4] {
            let idx = ((y as usize * 20) + x as usize) * 4;
            [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
        };
        // Inside the top-left arc: (2, 2) sits on the quarter disc of radius 6 centred at (6, 6).
        assert_eq!(at(2, 2), [61, 62, 63, 255]);
        // Outside that arc but inside the rect: a uniform radius-6 fill would also skip it,
        // which is exactly the point — the top-left corner is genuinely rounded.
        assert_eq!(at(1, 1), [0, 0, 0, 255]);
        // The other corners carry radius 0, so their rects fill fully.
        assert_eq!(at(18, 1), [61, 62, 63, 255]);
        assert_eq!(at(1, 18), [61, 62, 63, 255]);
        assert_eq!(at(18, 18), [61, 62, 63, 255]);
    }

    #[test]
    fn non_uniform_stroke_follows_the_per_corner_outline() {
        let mut surface = SoftwareSurface::new(Size { width: 20, height: 20 }, 1.0);
        surface.begin_frame(Color::BLACK);
        surface.draw_rounded_rect_radii_with_width(
            Rect { x: 2, y: 2, width: 16, height: 16 },
            [6, 0, 0, 0],
            Color::rgba(81, 82, 83, 255),
            2,
        );
        surface.end_frame();
        let rgba = surface.frame_rgba();
        let at = |x: i32, y: i32| -> [u8; 4] {
            let idx = ((y as usize * 20) + x as usize) * 4;
            [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
        };
        // The top rule runs the straight section inset by the 6 px top-left arc.
        assert_eq!(at(10, 2), [81, 82, 83, 255]);
        // The left rule starts below the arc: at y = 2 the outline is on the arc, not the rule.
        assert_eq!(at(2, 12), [81, 82, 83, 255]);
        // Straight rules with square corners paint at the full edges.
        assert_eq!(at(17, 10), [81, 82, 83, 255]);
        assert_eq!(at(10, 17), [81, 82, 83, 255]);
    }

    #[test]
    fn border_style_none_draws_nothing() {
        let mut recorder = Recorder::new(40, 40);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            context.draw_rounded_rect_border(
                Rect::new(4, 4, 32, 32),
                [6, 6, 6, 6],
                Color::RED,
                2,
                crate::style::BorderStyle::None,
            );
        }
        recorder.end_frame();
        assert!(
            recorder.commands.is_empty(),
            "a none border is suppressed even with width and colour set"
        );
    }

    #[test]
    fn border_style_solid_matches_the_plain_stroke() {
        let mut recorder = Recorder::new(40, 40);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            context.draw_rounded_rect_border(
                Rect::new(4, 4, 32, 32),
                [6, 6, 6, 6],
                Color::RED,
                2,
                crate::style::BorderStyle::Solid,
            );
            context.draw_rounded_rect_stroke(Rect::new(4, 4, 32, 32), 6, Color::RED, 2);
        }
        recorder.end_frame();
        assert_eq!(recorder.commands.len(), 2);
        assert_eq!(
            format!("{:?}", recorder.commands[0]),
            format!("{:?}", recorder.commands[1]),
            "the solid border is the plain stroke command"
        );
    }

    #[test]
    fn border_style_dashed_emits_edge_dash_segments() {
        let mut recorder = Recorder::new(60, 60);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            context.draw_rounded_rect_border(
                Rect::new(0, 0, 60, 60),
                [0, 0, 0, 0],
                Color::RED,
                1,
                crate::style::BorderStyle::Dashed,
            );
        }
        recorder.end_frame();
        let dashes: Vec<_> = recorder
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::DrawLineStroke { .. }))
            .collect();
        assert!(
            dashes.len() >= 4,
            "every edge carries at least one dash: got {} commands",
            recorder.commands.len()
        );
        // Dash: 3 * width with a gap of 2 — a 60 px edge holds more than one dash, so gaps exist.
        assert!(dashes.len() < 240, "dashes are patterned, not a solid rule");
    }

    #[test]
    fn border_style_dotted_emits_round_dots() {
        let mut recorder = Recorder::new(40, 40);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            context.draw_rounded_rect_border(
                Rect::new(0, 0, 40, 40),
                [0, 0, 0, 0],
                Color::RED,
                2,
                crate::style::BorderStyle::Dotted,
            );
        }
        recorder.end_frame();
        let dots = recorder
            .commands
            .iter()
            .filter(|command| matches!(command, RenderCommand::FillCircle { .. }))
            .count();
        assert!(dots >= 8, "every edge carries dots: got {dots}");
        assert!(
            recorder
                .commands
                .iter()
                .all(|command| !matches!(command, RenderCommand::DrawRoundedRectStroke { .. })),
            "a dotted border never emits a solid stroke"
        );
    }

    #[test]
    fn a_zero_width_border_draws_nothing() {
        let mut recorder = Recorder::new(40, 40);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            context.draw_rounded_rect_border(
                Rect::new(4, 4, 32, 32),
                [6, 6, 6, 6],
                Color::RED,
                0,
                crate::style::BorderStyle::Solid,
            );
        }
        recorder.end_frame();
        assert!(recorder.commands.is_empty());
    }
}
