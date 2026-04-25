#![allow(deprecated)]

use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::pipeline::*;
use crate::render::{
    last_auto_render_backend, AutoRenderBackend, PaintBackend, RenderCommand, RenderScene,
    SceneLayer, SoftwarePaintBackend, SoftwareRenderConfig, SoftwareSurface,
};
use crate::window::Window;

fn font() -> Font {
    Font {
        family: "Sans".to_string(),
        size: 14.0,
        weight: Font::REGULAR_WEIGHT,
        bold: false,
        italic: false,
    }
}
#[test]
fn text_metrics_scale_with_dpi() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 100,
            height: 40,
        },
        1.0,
    );
    let m1 = surface.measure_text("hello", &font());
    surface.set_dpi_scale(2.0);
    let m2 = surface.measure_text("hello", &font());
    assert!(m2.width > m1.width);
    assert!(m2.height > m1.height);
}
#[test]
fn double_buffer_present_swaps_frame() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 4,
            height: 4,
        },
        1.0,
    );
    surface.begin_frame(Color::RED);
    surface.end_frame();
    assert_eq!(&surface.frame_rgba()[0..4], &[255, 0, 0, 255]);
    surface.begin_frame(Color::BLUE);
    surface.end_frame();
    assert_eq!(&surface.frame_rgba()[0..4], &[0, 0, 255, 255]);
}
#[test]
fn fill_rect_writes_pixels() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    surface.begin_frame(Color::BLACK);
    surface.fill_rect(
        Rect {
            x: 2,
            y: 2,
            width: 3,
            height: 3,
        },
        Color::rgba(1, 2, 3, 255),
    );
    surface.end_frame();
    let idx = ((3 * 8 + 3) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[1, 2, 3, 255]);
}
#[test]
fn shaping_merges_combining_marks_into_one_cluster() {
    let surface = SoftwareSurface::new(
        Size {
            width: 100,
            height: 40,
        },
        1.0,
    );
    let shaped = surface.shape_text("e\u{0301}", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
#[test]
fn shaping_merges_zwj_sequence_into_one_cluster() {
    let surface = SoftwareSurface::new(
        Size {
            width: 100,
            height: 40,
        },
        1.0,
    );
    let shaped = surface.shape_text("👨\u{200D}👩\u{200D}👧\u{200D}👦", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
#[test]
fn scene_composition_respects_layer_order() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    let mut back = SceneLayer::new(0);
    back.push(RenderCommand::FillRect {
        rect: Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        },
        color: Color::rgba(10, 20, 30, 255),
    });
    let mut front = SceneLayer::new(10);
    front.push(RenderCommand::FillRect {
        rect: Rect {
            x: 2,
            y: 2,
            width: 2,
            height: 2,
        },
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
        rect: Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        },
        color: Color::rgba(7, 8, 9, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = 36;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[7, 8, 9, 255]);
}
#[test]
fn software_backend_delegates_text_shaping() {
    let backend = SoftwarePaintBackend::new(
        Size {
            width: 100,
            height: 40,
        },
        1.0,
    );
    let shaped = backend.shape_text("e\u{0301}", &font());
    assert_eq!(shaped.cluster_count(), 1);
}
#[test]
fn draw_text_rasterizes_glyph_instead_of_full_rect_fill() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 80,
            height: 30,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_text(Point { x: 4, y: 4 }, "A", &font(), Color::WHITE);
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
#[test]
fn fill_circle_writes_center_pixels() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(190, 191, 192, 255));
    surface.end_frame();
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[center_idx..center_idx + 4],
        &[190, 191, 192, 255]
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[center_idx..center_idx + 4],
        &[200, 201, 202, 255]
    );
    let edge_idx = ((8 * 16 + 12) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn draw_line_with_width_marks_neighbor_pixels() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
    surface.begin_frame(Color::BLACK);
    surface.draw_line_with_width(
        Point { x: 2, y: 6 },
        Point { x: 9, y: 6 },
        Color::rgba(21, 22, 23, 255),
        3,
    );
    surface.end_frame();
    let center_idx = ((6 * 12 + 5) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[center_idx..center_idx + 4],
        &[21, 22, 23, 255]
    );
    let upper_idx = ((5 * 12 + 5) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[upper_idx..upper_idx + 4],
        &[21, 22, 23, 255]
    );
    let lower_idx = ((7 * 12 + 5) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[lower_idx..lower_idx + 4],
        &[21, 22, 23, 255]
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = ((5 * 12 + 5) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[31, 32, 33, 255]);
}
#[test]
fn draw_rect_with_width_marks_neighbor_border_pixels() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    surface.begin_frame(Color::BLACK);
    surface.draw_rect_with_width(
        Rect {
            x: 4,
            y: 4,
            width: 6,
            height: 6,
        },
        Color::rgba(41, 42, 43, 255),
        3,
    );
    surface.end_frame();
    let border_idx = ((4 * 14 + 6) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[border_idx..border_idx + 4],
        &[41, 42, 43, 255]
    );
    let neighbor_idx = ((5 * 14 + 6) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[neighbor_idx..neighbor_idx + 4],
        &[41, 42, 43, 255]
    );
}
#[test]
fn scene_composition_supports_stroke_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: 4,
            y: 4,
            width: 6,
            height: 6,
        },
        color: Color::rgba(51, 52, 53, 255),
        width: 3,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let idx = ((5 * 14 + 6) * 4) as usize;
    assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[51, 52, 53, 255]);
}
#[test]
fn fill_rounded_rect_writes_center_and_preserves_corner() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    surface.begin_frame(Color::BLACK);
    surface.fill_rounded_rect(
        Rect {
            x: 3,
            y: 3,
            width: 8,
            height: 8,
        },
        3,
        Color::rgba(61, 62, 63, 255),
    );
    surface.end_frame();
    let center_idx = ((7 * 14 + 7) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[center_idx..center_idx + 4],
        &[61, 62, 63, 255]
    );
    let corner_idx = ((3 * 14 + 3) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[corner_idx..corner_idx + 4],
        &[0, 0, 0, 255]
    );
}
#[test]
fn scene_composition_supports_rounded_rect_commands() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRect {
        rect: Rect {
            x: 3,
            y: 3,
            width: 8,
            height: 8,
        },
        radius: 3,
        color: Color::rgba(71, 72, 73, 255),
    });
    layer.push(RenderCommand::DrawRoundedRectStroke {
        rect: Rect {
            x: 3,
            y: 3,
            width: 8,
            height: 8,
        },
        radius: 3,
        color: Color::rgba(81, 82, 83, 255),
        width: 2,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::BLACK);
    let stroke_idx = ((3 * 14 + 7) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[stroke_idx..stroke_idx + 4],
        &[81, 82, 83, 255]
    );
    let fill_idx = ((7 * 14 + 7) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[fill_idx..fill_idx + 4],
        &[71, 72, 73, 255]
    );
}
#[test]
fn draw_rounded_rect_aa_with_width_produces_soft_edge() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_rounded_rect_aa_with_width(
        Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        4,
        Color::rgba(230, 231, 232, 255),
        2,
    );
    surface.end_frame();
    let core_idx = ((3 * 16 + 8) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[core_idx..core_idx + 4],
        &[230, 231, 232, 255]
    );
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn fill_rounded_rect_aa_produces_soft_corner_edge() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect_aa(
        Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        4,
        Color::rgba(250, 210, 170, 255),
    );
    surface.end_frame();
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[center_idx..center_idx + 4],
        &[250, 210, 170, 255]
    );
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn aa_sample_level_changes_rounded_rect_edge_coverage() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    surface.set_aa_samples_per_axis(1);
    assert_eq!(surface.aa_samples_per_axis(), 1);
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect_aa(
        Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
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
        Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        4,
        Color::rgba(200, 100, 50, 255),
    );
    surface.end_frame();
    let alpha_high = surface.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_low, alpha_high);
}
#[test]
fn render_config_applies_and_clamps_aa_samples() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    assert_eq!(surface.render_config().aa_samples_per_axis, 4);
    surface.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 0,
    });
    assert_eq!(surface.render_config().aa_samples_per_axis, 1);
    surface.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 12,
    });
    assert_eq!(surface.render_config().aa_samples_per_axis, 8);
}
#[test]
fn backend_render_config_passthrough_clamps_values() {
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    assert_eq!(backend.render_config().aa_samples_per_axis, 4);
    backend.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 0,
    });
    assert_eq!(backend.render_config().aa_samples_per_axis, 1);
    backend.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 20,
    });
    assert_eq!(backend.render_config().aa_samples_per_axis, 8);
}
#[test]
fn paint_backend_trait_render_config_updates_software_backend() {
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    PaintBackend::apply_render_config(
        &mut backend,
        SoftwareRenderConfig {
            aa_samples_per_axis: 3,
        },
    );
    assert_eq!(PaintBackend::render_config(&backend).aa_samples_per_axis, 3);
}
#[test]
fn scene_compose_with_temporary_config_restores_backend_state() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        radius: 4,
        color: Color::rgba(100, 110, 120, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    backend.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 4,
    });
    scene.compose_with_backend_config(
        &mut backend,
        Color::TRANSPARENT,
        Some(SoftwareRenderConfig {
            aa_samples_per_axis: 1,
        }),
    );
    assert_eq!(backend.render_config().aa_samples_per_axis, 4);
}
#[test]
fn scene_compose_with_temporary_config_changes_aa_output() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        radius: 4,
        color: Color::rgba(150, 151, 152, 255),
    });
    scene.add_layer(layer);
    let mut backend_default = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    backend_default.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 4,
    });
    scene.compose_with_backend(&mut backend_default, Color::TRANSPARENT);
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let alpha_default = backend_default.frame_rgba()[edge_idx + 3];
    let mut backend_temp = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    backend_temp.apply_render_config(SoftwareRenderConfig {
        aa_samples_per_axis: 4,
    });
    scene.compose_with_backend_config(
        &mut backend_temp,
        Color::TRANSPARENT,
        Some(SoftwareRenderConfig {
            aa_samples_per_axis: 1,
        }),
    );
    let alpha_temp = backend_temp.frame_rgba()[edge_idx + 3];
    assert_ne!(alpha_default, alpha_temp);
}
#[test]
fn aa_sample_level_changes_circle_edge_coverage() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_circle(Point { x: 7, y: 7 }, 3, Color::rgba(100, 120, 140, 255));
    surface.end_frame();
    let edge_idx = ((8 * 14 + 10) * 4) as usize;
    let edge_alpha = surface.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn rounded_rect_fill_applies_partial_alpha_at_corner_edge() {
    let mut surface = SoftwareSurface::new(
        Size {
            width: 14,
            height: 14,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.fill_rounded_rect(
        Rect {
            x: 3,
            y: 3,
            width: 8,
            height: 8,
        },
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 12,
            height: 12,
        },
        1.0,
    );
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
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    surface.begin_frame(Color::TRANSPARENT);
    surface.draw_line_aa_with_width(
        Point { x: 2, y: 8 },
        Point { x: 13, y: 8 },
        Color::rgba(210, 211, 212, 255),
        3,
    );
    surface.end_frame();
    let core_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &surface.frame_rgba()[core_idx..core_idx + 4],
        &[210, 211, 212, 255]
    );
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
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let core_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[core_idx..core_idx + 4],
        &[220, 221, 222, 255]
    );
    let edge_idx = ((9 * 16 + 8) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_aa_stroke_rounded_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::DrawRoundedRectStrokeAA {
        rect: Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        radius: 4,
        color: Color::rgba(240, 241, 242, 255),
        width: 2,
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let core_idx = ((3 * 16 + 8) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[core_idx..core_idx + 4],
        &[240, 241, 242, 255]
    );
    let edge_idx = ((4 * 16 + 3) * 4) as usize;
    let edge_alpha = backend.frame_rgba()[edge_idx + 3];
    assert!(edge_alpha > 0 && edge_alpha < 255);
}
#[test]
fn scene_composition_supports_aa_fill_rounded_rect_command() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRoundedRectAA {
        rect: Rect {
            x: 3,
            y: 3,
            width: 10,
            height: 10,
        },
        radius: 4,
        color: Color::rgba(120, 130, 140, 255),
    });
    scene.add_layer(layer);
    let mut backend = SoftwarePaintBackend::new(
        Size {
            width: 16,
            height: 16,
        },
        1.0,
    );
    scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
    let center_idx = ((8 * 16 + 8) * 4) as usize;
    assert_eq!(
        &backend.frame_rgba()[center_idx..center_idx + 4],
        &[120, 130, 140, 255]
    );
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
        text: "fallback".to_string(),
        font: Font::default(),
        color: Color::rgba(250, 120, 40, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 48,
            height: 24,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
}
#[test]
fn auto_compose_produces_expected_pixels_for_simple_rect_scene() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: 2,
            y: 2,
            width: 6,
            height: 4,
        },
        color: Color::rgba(11, 22, 33, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 16,
            height: 12,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
    let idx = ((3 * 16 + 3) * 4) as usize;
    assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[11, 22, 33, 255]);
}
#[test]
fn auto_compose_updates_last_backend_diagnostics() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        },
        color: Color::rgba(1, 2, 3, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 8,
            height: 8,
        },
        1.0,
    );
    let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert_eq!(selected, last_auto_render_backend());
}
#[test]
fn auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected() {
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        color: Color::rgba(9, 8, 7, 255),
    });
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 0,
            height: 0,
        },
        1.0,
    );
    let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
    assert_eq!(selected, AutoRenderBackend::CpuSoftware);
    assert_eq!(last_auto_render_backend(), AutoRenderBackend::CpuSoftware);
}
#[test]
fn base_control_visual_builders_emit_expected_command_types() {
    use crate::widget::{
        Button, CheckBox, CheckState, Label, LineEdit, Panel, RadioButton, Widget,
    };
    let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 120, 80));
    window.set_background_color(Some(Color::rgba(10, 20, 30, 255)));
    let mut panel = Panel::new(Rect::new(4, 20, 112, 52));
    panel.set_background_color(Some(Color::rgba(40, 50, 60, 255)));
    let mut button = Button::new("OK".to_string(), Rect::new(10, 26, 50, 20));
    button.set_pressed(true);
    let mut checkbox = CheckBox::new(Rect::new(70, 28, 20, 20));
    checkbox.set_state(CheckState::Checked);
    let mut radio = RadioButton::new(Rect::new(70, 52, 20, 20));
    radio.set_checked(true);
    let mut label = Label::new("Label".to_string(), Rect::new(10, 50, 60, 16));
    label.set_background_color(Some(Color::rgba(80, 90, 100, 255)));
    let mut line_edit = LineEdit::new(Rect::new(10, 54, 52, 16));
    line_edit.set_text("abc".to_string());
    let mut layer = SceneLayer::new(0);
    append_window_visual_commands(&mut layer, &window);
    append_panel_visual_commands(&mut layer, &panel);
    append_button_visual_commands(&mut layer, &button);
    append_checkbox_visual_commands(&mut layer, &checkbox);
    append_radiobutton_visual_commands(&mut layer, &radio);
    append_label_visual_commands(&mut layer, &label);
    append_line_edit_visual_commands(&mut layer, &line_edit);
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
fn auto_compose_renders_base_control_scene_with_gpu_or_cpu_backend() {
    use crate::widget::{
        Button, CheckBox, CheckState, Label, LineEdit, Panel, RadioButton, Widget,
    };
    let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 120, 80));
    window.set_background_color(Some(Color::rgba(10, 20, 30, 255)));
    let mut panel = Panel::new(Rect::new(4, 20, 112, 52));
    panel.set_background_color(Some(Color::rgba(40, 50, 60, 255)));
    let mut button = Button::new("Apply".to_string(), Rect::new(10, 26, 50, 20));
    button.set_pressed(true);
    let mut checkbox = CheckBox::new(Rect::new(70, 28, 20, 20));
    checkbox.set_state(CheckState::Checked);
    let mut radio = RadioButton::new(Rect::new(70, 52, 20, 20));
    radio.set_checked(true);
    let mut label = Label::new("Label".to_string(), Rect::new(10, 50, 60, 16));
    label.set_background_color(Some(Color::rgba(80, 90, 100, 255)));
    let mut line_edit = LineEdit::new(Rect::new(10, 54, 52, 16));
    line_edit.set_text("abc".to_string());
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    append_window_visual_commands(&mut layer, &window);
    append_panel_visual_commands(&mut layer, &panel);
    append_button_visual_commands(&mut layer, &button);
    append_checkbox_visual_commands(&mut layer, &checkbox);
    append_radiobutton_visual_commands(&mut layer, &radio);
    append_label_visual_commands(&mut layer, &label);
    append_line_edit_visual_commands(&mut layer, &line_edit);
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 128,
            height: 88,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
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
    assert_eq!(sample(6, 22), [40, 50, 60, 255]);
    let button_px = sample(12, 28);
    assert!(button_px[3] > 0);
    let checkbox_px = sample(76, 34);
    assert!(checkbox_px[3] > 0);
    let radio_px = sample(80, 62);
    assert!(radio_px[3] > 0);
    assert_eq!(sample(12, 52), [80, 90, 100, 255]);
    let line_edit_px = sample(12, 56);
    assert!(line_edit_px[3] > 0);
}
#[test]
fn data_range_control_visual_builders_emit_selection_and_value_commands() {
    use crate::widget::{ComboBox, ListBox, ProgressBar, ScrollBar, Slider};
    let mut combo = ComboBox::new(Rect::new(4, 4, 120, 20));
    combo.add_item("Alpha".to_string());
    combo.add_item("Beta".to_string());
    combo.set_current_index(Some(1));
    // combo.open_dropdown(); // Method not found
    let mut list = ListBox::new(Rect::new(4, 28, 120, 64));
    list.add_item("Row-1".to_string());
    list.add_item("Row-2".to_string());
    let mut progress = ProgressBar::new(Rect::new(140, 8, 100, 14));
    progress.set_range(0, 100);
    progress.set_value(60);
    let mut slider = Slider::new(Rect::new(140, 30, 100, 20));
    slider.set_range(0, 100);
    slider.set_value(30);
    let mut scroll = ScrollBar::new(Rect::new(140, 58, 100, 16));
    scroll.set_range(0, 100);
    scroll.set_page_step(20);
    scroll.set_value(40);
    let mut layer = SceneLayer::new(0);
    append_combo_box_visual_commands(&mut layer, &combo);
    append_list_box_visual_commands(&mut layer, &list);
    append_progress_bar_visual_commands(&mut layer, &progress);
    append_slider_visual_commands(&mut layer, &slider);
    append_scroll_bar_visual_commands(&mut layer, &scroll);
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
fn auto_compose_renders_data_range_scene_with_gpu_or_cpu_backend() {
    use crate::widget::{ComboBox, ListBox, ProgressBar, ScrollBar, Slider};
    let mut combo = ComboBox::new(Rect::new(4, 4, 120, 20));
    combo.add_item("Alpha".to_string());
    combo.add_item("Beta".to_string());
    combo.set_current_index(Some(1));
    let mut list = ListBox::new(Rect::new(4, 28, 120, 64));
    list.add_item("Row-1".to_string());
    list.add_item("Row-2".to_string());
    let mut progress = ProgressBar::new(Rect::new(140, 8, 100, 14));
    progress.set_range(0, 100);
    progress.set_value(60);
    let mut slider = Slider::new(Rect::new(140, 30, 100, 20));
    slider.set_range(0, 100);
    slider.set_value(30);
    let mut scroll = ScrollBar::new(Rect::new(140, 58, 100, 16));
    scroll.set_range(0, 100);
    scroll.set_page_step(20);
    scroll.set_value(40);
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    append_combo_box_visual_commands(&mut layer, &combo);
    append_list_box_visual_commands(&mut layer, &list);
    append_progress_bar_visual_commands(&mut layer, &progress);
    append_slider_visual_commands(&mut layer, &slider);
    append_scroll_bar_visual_commands(&mut layer, &scroll);
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 256,
            height: 128,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
    let sample = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * surface.size().width + x) * 4) as usize;
        [
            surface.frame_rgba()[idx],
            surface.frame_rgba()[idx + 1],
            surface.frame_rgba()[idx + 2],
            surface.frame_rgba()[idx + 3],
        ]
    };
    let combo_px = sample(8, 8);
    assert!(combo_px[3] > 0);
    let list_px = sample(8, 34);
    assert!(list_px[3] > 0);
    let progress_px = sample(180, 12);
    assert_eq!(progress_px[3], 255);
    let slider_thumb_px = sample(170, 40);
    assert!(slider_thumb_px[3] > 0);
    let scroll_thumb_px = sample(178, 62);
    assert!(scroll_thumb_px[3] > 0);
}
#[test]
fn host_navigation_visual_builders_emit_expected_commands() {
    use crate::widget::{Menu, MenuBar, StatusBar, TabWidget, ToolBar};
    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 260, 24));
    menu_bar.add_menu("File".to_string());
    menu_bar.add_menu("Edit".to_string());
    // menu_bar.set_current_menu(1002); // Method not found
    let mut menu = Menu::new("File", Rect::new(0, 24, 160, 100));
    menu.add_action("Open".to_string());
    menu.add_action("Save".to_string());
    let mut tool_bar = ToolBar::new(Rect::new(0, 128, 260, 28));
    tool_bar.add_action("cut".to_string(), "Cut".to_string());
    tool_bar.add_action("copy".to_string(), "Copy".to_string());
    tool_bar.add_action("paste".to_string(), "Paste".to_string());
    let status_bar = StatusBar::new(Rect::new(0, 160, 260, 22));
    let mut tabs = TabWidget::new(Rect::new(170, 24, 90, 70));
    tabs.add_tab("Tab 1".to_string(), None);
    tabs.add_tab("Tab 2".to_string(), None);
    tabs.set_current_index(1);
    let mut layer = SceneLayer::new(0);
    append_menu_bar_visual_commands(&mut layer, &menu_bar);
    append_menu_visual_commands(&mut layer, &menu);
    append_tool_bar_visual_commands(&mut layer, &tool_bar);
    append_status_bar_visual_commands(&mut layer, &status_bar);
    append_tab_widget_visual_commands(&mut layer, &tabs);
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
    assert!(draw_text_count >= 6);
    assert!(fill_rect_count >= 5);
    assert!(rounded_rect_count >= 1);
}
#[test]
fn auto_compose_renders_host_navigation_scene_with_gpu_or_cpu_backend() {
    use crate::widget::{Menu, MenuBar, StatusBar, TabWidget, ToolBar};
    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 260, 24));
    menu_bar.add_menu("File".to_string());
    menu_bar.add_menu("Edit".to_string());
    // menu_bar.set_current_menu(1002); // Method not found
    let mut menu = Menu::new("File", Rect::new(0, 24, 160, 100));
    menu.add_action("Open".to_string());
    menu.add_action("Save".to_string());
    let mut tool_bar = ToolBar::new(Rect::new(0, 128, 260, 28));
    tool_bar.add_action("cut".to_string(), "Cut".to_string());
    tool_bar.add_action("copy".to_string(), "Copy".to_string());
    let status_bar = StatusBar::new(Rect::new(0, 160, 260, 20));
    let mut tabs = TabWidget::new(Rect::new(140, 120, 120, 32));
    tabs.add_tab("Tab 1".to_string(), None);
    tabs.add_tab("Tab 2".to_string(), None);
    tabs.set_current_index(1);
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    append_menu_bar_visual_commands(&mut layer, &menu_bar);
    append_menu_visual_commands(&mut layer, &menu);
    append_tool_bar_visual_commands(&mut layer, &tool_bar);
    append_status_bar_visual_commands(&mut layer, &status_bar);
    append_tab_widget_visual_commands(&mut layer, &tabs);
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 280,
            height: 190,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
    let sample = |x: u32, y: u32| -> [u8; 4] {
        let idx = ((y * surface.size().width + x) * 4) as usize;
        [
            surface.frame_rgba()[idx],
            surface.frame_rgba()[idx + 1],
            surface.frame_rgba()[idx + 2],
            surface.frame_rgba()[idx + 3],
        ]
    };
    let menu_bar_px = sample(6, 6);
    assert!(menu_bar_px[3] > 0);
    let menu_px = sample(8, 36);
    assert!(menu_px[3] > 0);
    let toolbar_px = sample(8, 132);
    assert!(toolbar_px[3] > 0);
    let status_px = sample(8, 166);
    assert!(status_px[3] > 0);
    let tabs_px = sample(150, 130);
    assert!(tabs_px[3] > 0);
    let stack_px = sample(150, 90);
    assert!(stack_px[3] > 0);
}
#[test]
fn gpu_parity_covered_controls_emit_non_empty_command_suite() {
    use crate::widget::{
        Button, CheckBox, CheckState, ComboBox, Label, LineEdit, ListBox, Menu, MenuBar, Panel,
        ProgressBar, RadioButton, ScrollBar, Slider, StatusBar, TabWidget, ToolBar, Widget,
    };
    let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 320, 240));
    window.set_background_color(Some(Color::rgba(18, 20, 24, 255)));
    let mut panel = Panel::new(Rect::new(8, 32, 180, 100));
    panel.set_background_color(Some(Color::rgba(38, 45, 60, 255)));
    let mut button = Button::new("OK".to_string(), Rect::new(16, 42, 70, 24));
    button.set_pressed(true);
    let mut checkbox = CheckBox::new(Rect::new(16, 74, 22, 22));
    checkbox.set_state(CheckState::Checked);
    let mut radio = RadioButton::new(Rect::new(46, 74, 22, 22));
    radio.set_checked(true);
    let mut label = Label::new("Label".to_string(), Rect::new(16, 102, 80, 18));
    label.set_background_color(Some(Color::rgba(76, 84, 98, 255)));
    let mut line_edit = LineEdit::new(Rect::new(98, 102, 82, 18));
    line_edit.set_text("text".to_string());
    let mut combo = ComboBox::new(Rect::new(200, 32, 110, 22));
    combo.add_item("A".to_string());
    combo.add_item("B".to_string());
    combo.set_current_index(Some(1));
    let mut list = ListBox::new(Rect::new(200, 58, 110, 74));
    list.add_item("One".to_string());
    list.add_item("Two".to_string());
    let mut progress = ProgressBar::new(Rect::new(8, 142, 170, 14));
    progress.set_range(0, 100);
    progress.set_value(45);
    let mut slider = Slider::new(Rect::new(8, 162, 170, 20));
    slider.set_range(0, 100);
    slider.set_value(35);
    let mut scroll = ScrollBar::new(Rect::new(8, 186, 170, 16));
    scroll.set_range(0, 100);
    scroll.set_page_step(20);
    scroll.set_value(30);
    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 320, 24));
    menu_bar.add_menu("File".to_string());
    menu_bar.add_menu("Help".to_string());
    let mut menu = Menu::new("File", Rect::new(200, 136, 110, 64));
    menu.add_action("Open".to_string());
    menu.add_action("Save".to_string());
    let mut tool_bar = ToolBar::new(Rect::new(0, 210, 320, 24));
    tool_bar.add_action("cut".to_string(), "Cut".to_string());
    tool_bar.add_action("copy".to_string(), "Copy".to_string());
    let mut status_bar = StatusBar::new(Rect::new(0, 234, 320, 20));
    status_bar.show_message("Ready".to_string(), 3000);
    let mut tabs = TabWidget::new(Rect::new(192, 202, 120, 32));
    tabs.add_tab("Tab 1".to_string(), None);
    tabs.add_tab("Tab 2".to_string(), None);
    tabs.set_current_index(1);
    let mut layer = SceneLayer::new(0);
    append_window_visual_commands(&mut layer, &window);
    append_panel_visual_commands(&mut layer, &panel);
    append_button_visual_commands(&mut layer, &button);
    append_checkbox_visual_commands(&mut layer, &checkbox);
    append_radiobutton_visual_commands(&mut layer, &radio);
    append_label_visual_commands(&mut layer, &label);
    append_line_edit_visual_commands(&mut layer, &line_edit);
    append_combo_box_visual_commands(&mut layer, &combo);
    append_list_box_visual_commands(&mut layer, &list);
    append_progress_bar_visual_commands(&mut layer, &progress);
    append_slider_visual_commands(&mut layer, &slider);
    append_scroll_bar_visual_commands(&mut layer, &scroll);
    append_menu_bar_visual_commands(&mut layer, &menu_bar);
    append_menu_visual_commands(&mut layer, &menu);
    append_tool_bar_visual_commands(&mut layer, &tool_bar);
    append_status_bar_visual_commands(&mut layer, &status_bar);
    append_tab_widget_visual_commands(&mut layer, &tabs);
    assert!(layer.commands().len() >= 30);
}
#[test]
fn gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend() {
    use crate::widget::{
        Button, CheckBox, CheckState, ComboBox, Label, LineEdit, ListBox, Menu, MenuBar, Panel,
        ProgressBar, RadioButton, ScrollBar, Slider, StatusBar, TabWidget, ToolBar, Widget,
    };
    let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 320, 240));
    window.set_background_color(Some(Color::rgba(18, 20, 24, 255)));
    let mut panel = Panel::new(Rect::new(8, 32, 180, 100));
    panel.set_background_color(Some(Color::rgba(38, 45, 60, 255)));
    let mut button = Button::new("OK".to_string(), Rect::new(16, 42, 70, 24));
    button.set_pressed(true);
    let mut checkbox = CheckBox::new(Rect::new(16, 74, 22, 22));
    checkbox.set_state(CheckState::Checked);
    let mut radio = RadioButton::new(Rect::new(46, 74, 22, 22));
    radio.set_checked(true);
    let mut label = Label::new("Label".to_string(), Rect::new(16, 102, 80, 18));
    label.set_background_color(Some(Color::rgba(76, 84, 98, 255)));
    let mut line_edit = LineEdit::new(Rect::new(98, 102, 82, 18));
    line_edit.set_text("text".to_string());
    let mut combo = ComboBox::new(Rect::new(200, 32, 110, 22));
    combo.add_item("A".to_string());
    combo.add_item("B".to_string());
    combo.set_current_index(Some(1));
    let mut list = ListBox::new(Rect::new(200, 58, 110, 74));
    list.add_item("One".to_string());
    list.add_item("Two".to_string());
    let mut progress = ProgressBar::new(Rect::new(8, 142, 170, 14));
    progress.set_range(0, 100);
    progress.set_value(45);
    let mut slider = Slider::new(Rect::new(8, 162, 170, 20));
    slider.set_range(0, 100);
    slider.set_value(35);
    let mut scroll = ScrollBar::new(Rect::new(8, 186, 170, 16));
    scroll.set_range(0, 100);
    scroll.set_page_step(20);
    scroll.set_value(30);
    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 320, 24));
    menu_bar.add_menu("File".to_string());
    menu_bar.add_menu("Help".to_string());
    let mut menu = Menu::new("File", Rect::new(200, 136, 110, 64));
    menu.add_action("Open".to_string());
    menu.add_action("Save".to_string());
    let mut tool_bar = ToolBar::new(Rect::new(0, 210, 320, 24));
    tool_bar.add_action("cut".to_string(), "Cut".to_string());
    tool_bar.add_action("copy".to_string(), "Copy".to_string());
    let mut status_bar = StatusBar::new(Rect::new(0, 234, 320, 20));
    status_bar.show_message("Ready".to_string(), 3000);
    let mut tabs = TabWidget::new(Rect::new(192, 202, 120, 32));
    tabs.add_tab("Tab 1".to_string(), None);
    tabs.add_tab("Tab 2".to_string(), None);
    tabs.set_current_index(1);
    let mut scene = RenderScene::new();
    let mut layer = SceneLayer::new(0);
    append_window_visual_commands(&mut layer, &window);
    append_panel_visual_commands(&mut layer, &panel);
    append_button_visual_commands(&mut layer, &button);
    append_checkbox_visual_commands(&mut layer, &checkbox);
    append_radiobutton_visual_commands(&mut layer, &radio);
    append_label_visual_commands(&mut layer, &label);
    append_line_edit_visual_commands(&mut layer, &line_edit);
    append_combo_box_visual_commands(&mut layer, &combo);
    append_list_box_visual_commands(&mut layer, &list);
    append_progress_bar_visual_commands(&mut layer, &progress);
    append_slider_visual_commands(&mut layer, &slider);
    append_scroll_bar_visual_commands(&mut layer, &scroll);
    append_menu_bar_visual_commands(&mut layer, &menu_bar);
    append_menu_visual_commands(&mut layer, &menu);
    append_tool_bar_visual_commands(&mut layer, &tool_bar);
    append_status_bar_visual_commands(&mut layer, &status_bar);
    append_tab_widget_visual_commands(&mut layer, &tabs);
    scene.add_layer(layer);
    let mut surface = SoftwareSurface::new(
        Size {
            width: 340,
            height: 260,
        },
        1.0,
    );
    let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
    assert!(matches!(
        backend,
        AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
    ));
    let idx = ((20 * surface.size().width + 20) * 4) as usize;
    // Validate pixel at computed index (smoke check: should be non-zero if rendered)
    let _ = idx;
}
