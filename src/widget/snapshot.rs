// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Offscreen RGBA snapshots at an arbitrary pixel density (EX4-05).
//!
//! # Why this exists
//!
//! The SVG snapshot chain renders vector output at one logical size, and the widget tests that
//! compare pixels build a `SoftwarePaintBackend` by hand each time. Neither can answer "what does
//! this control look like at 2x?" — the question a documentation figure, a retina comparison or a
//! visual-regression fixture asks. This is that entry point: one call, one RGBA buffer whose
//! dimensions are the logical geometry multiplied by the requested scale.
//!
//! # What the scale means here
//!
//! The control is laid out against the **physical** rectangle (`logical × scale`), so its
//! geometric ink — fills, borders and shapes — is drawn into a larger buffer rather than the
//! picture being a large canvas around unchanged ink. The backend's own `dpi_scale` is set to the
//! same factor, which is what its rasteriser reads. The **widget's** text metrics are not
//! rewritten: this function does not push the factor into the control, so a text run scales with
//! the requested factor only when the control itself derives it from `dpi_scale`. The widget's
//! previous geometry is restored before returning, so a snapshot is a read, not a reparent or a
//! resize.
//!
//! # Not on the steady-state path
//!
//! Nothing in a frame loop calls this: it allocates a surface and a buffer on every call, which is
//! exactly what a test/documentation producer wants and a frame must not pay. The function keeps
//! that boundary by living beside `svg` rather than inside the draw bridge.

use crate::compat::Vec;
use crate::core::{Color, Rect, Size};
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use crate::widget::{Draw, Widget};

/// Renders `widget` to RGBA pixels at `dpi_scale`, returning the buffer in row-major order.
///
/// The returned length is `physical_width × physical_height × 4`, where each dimension is the
/// logical dimension rounded after multiplication. A non-finite or non-positive scale is treated
/// as `1.0` rather than producing a zero-sized surface; a zero-sized geometry is clamped to one
/// pixel so the measurement has something to return.
///
/// The backdrop is transparent, so a control that deliberately paints no background (a label, a
/// separator) keeps its transparency instead of being composited over an invented colour.
pub fn render_snapshot_at_scale<W: Draw + Widget + ?Sized>(
    widget: &mut W,
    geometry: Rect,
    dpi_scale: f32,
) -> Vec<u8> {
    let scale = if dpi_scale.is_finite() && dpi_scale > 0.0 { dpi_scale } else { 1.0 };
    let physical = Size::new(
        ((geometry.width as f32) * scale).round().max(1.0) as u32,
        ((geometry.height as f32) * scale).round().max(1.0) as u32,
    );

    let previous = widget.geometry();
    widget.set_geometry(Rect::new(0, 0, physical.width, physical.height));
    let mut backend = SoftwarePaintBackend::new(physical, scale);
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        widget.draw(&mut context);
    }
    backend.end_frame();
    let pixels = backend.surface().frame_rgba().to_vec();
    // Restore before returning, including the layout/redraw side effects `set_geometry` has, so a
    // caller can snapshot a live control without changing what it will paint next frame.
    widget.set_geometry(previous);
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::BaseWidget;
    use crate::widget::WidgetKind;

    /// A control that fills its whole geometry, so the snapshot's ink extent is the geometry.
    struct Probe {
        base: BaseWidget,
    }

    impl Probe {
        fn new(geometry: Rect) -> Self {
            Self { base: BaseWidget::new(WidgetKind::Panel, geometry, "SnapshotProbe") }
        }
    }

    impl Widget for Probe {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        crate::impl_draw_bridge!();
    }

    impl crate::event::EventHandler for Probe {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }

    impl Draw for Probe {
        fn draw(&mut self, context: &mut RenderContext) {
            context.fill_rect(self.geometry(), Color::BLACK);
        }
    }

    #[test]
    fn the_buffer_grows_with_the_requested_scale() {
        let geometry = Rect::new(0, 0, 20, 10);
        let mut probe = Probe::new(geometry);
        let one = render_snapshot_at_scale(&mut probe, geometry, 1.0);
        let two = render_snapshot_at_scale(&mut probe, geometry, 2.0);
        assert_eq!(one.len(), 20 * 10 * 4);
        assert_eq!(two.len(), 40 * 20 * 4);
        assert_eq!(probe.geometry(), geometry, "the snapshot restores the previous geometry");
    }

    #[test]
    fn an_invalid_scale_and_a_zero_geometry_still_produce_a_measurable_buffer() {
        let mut probe = Probe::new(Rect::new(0, 0, 1, 1));
        let buffer = render_snapshot_at_scale(&mut probe, Rect::new(0, 0, 0, 0), f32::NAN);
        assert_eq!(buffer.len(), 4, "a degenerate request is clamped to one pixel");
    }
}
